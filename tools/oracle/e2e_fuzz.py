#!/usr/bin/env python3
"""Driver for the end-to-end differential fuzzer (see tools/oracle/e2e_fuzz.sh).

Subcommands (run from anywhere; paths are relative to the repository root):

  build                       build the C++ variants and the Rust replay test
  gen --seed S --first F --count N --out FILE
                              generate + run scenes on all three C++ builds
                              and write a classified golden file
  compare --golden FILE [--soft]
                              replay a golden file through the Rust port
  campaign --seed S --scenes N [--batch B] [--first F]
                              gen + compare in batches; divergent scenes are
                              saved under target/e2e_fuzz/campaign-S/
  reduce --golden FILE --scene NAME [--crash SIG] [--out FILE]
                              shrink a divergent scene (or, with --crash, a
                              scene on which a C++ build crashes with SIG in
                              the message) by delta debugging
  show --golden FILE --scene NAME
                              full dumps of C++ (NDEBUG) and Rust + diff

Classification of a scene (first output line of the golden):
  (none)              all three C++ builds agree; compared everywhere
  #assert <msg>       only the asserting build fails; NDEBUG output compared
                      in release builds
  #exclude crash ...  the NDEBUG build crashes or times out
  #exclude ub ...     the ASan/UBSan build reports an error (message and
                      first libavoid frame follow)
  #exclude memory     a C++ build exceeded the memory cap (bump arena
                      E2E_ARENA_MB, default 3072 MB, or RSS E2E_RSS_MB,
                      default 4096 MB) -- not a verdict on upstream
  #exclude assertdiff the asserting build finished with different output
  #exclude pinset-unsorted
                      transformConnectionPinPositions() mutated the keys of a
                      shape's std::set of pins so that it is no longer sorted
                      (undefined behaviour; libstdc++ tree descent not
                      modelled by the port)
For #exclude ub / memory (of the ASan build) / assertdiff the NDEBUG output
follows the marker (E2E_SOFT=1 replays those too, without failing).

Resources (shared workstation): the three builds run one after the other,
at most E2E_JOBS (default 8) C++ processes at once (6 for ASan), each scene
in its own forked process (its bump arena dies with it) whose memory is
capped as above (RSS default 4096 MB); no C++ process is started while
less than E2E_MIN_FREE_GB (default 6) of memory is available.
The ASan build runs on ASan's malloc with a 64 MB quarantine (the bump
builds never free), so it only answers "does this scene hit UB".
"""

import argparse
import json
import os
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
BUILD = os.environ.get("E2E_BUILD", os.path.join(ROOT, "target", "e2e_fuzz"))
JOBS = int(os.environ.get("E2E_JOBS", "8"))
VARIANTS = ("assert", "ndebug", "asan")
# Per-scene limits (seconds); scenes take ~0.1-3 s (ASan ~5x).
TIMEOUTS = {"assert": 60, "ndebug": 60, "asan": 300}


def exe(variant):
    return os.path.join(BUILD, variant, "e2e_fuzz")


def parse_scenes(text):
    """[(name, dsl_lines, out_lines)] of a golden-format text."""
    res = []
    name, dsl, out, in_out = None, [], [], False
    for line in text.split("\n"):
        if line.startswith("#scene "):
            name, dsl, out, in_out = line[7:], [], [], False
        elif line == "#out":
            in_out = True
        elif line == "#end":
            if name is not None:
                res.append((name, dsl, out))
            name = None
        elif name is not None:
            (out if in_out else dsl).append(line)
    return res


def format_scenes(scenes):
    parts = []
    for name, dsl, out in scenes:
        parts.append("#scene " + name)
        parts.extend(dsl)
        parts.append("#out")
        parts.extend(out)
        parts.append("#end")
    return "\n".join(parts) + "\n"


def run_cpp(variant, args, env_extra=None):
    env = dict(os.environ)
    env["E2E_TIMEOUT"] = str(TIMEOUTS[variant])
    env["ASAN_OPTIONS"] = ("detect_leaks=0:abort_on_error=0:allocator_may_return_null=1:"
                           "quarantine_size_mb=64:malloc_context_size=8")
    env.setdefault("E2E_ARENA_MB", "3072")
    env.setdefault("E2E_RSS_MB", "4096")
    env["UBSAN_OPTIONS"] = "print_stacktrace=1:halt_on_error=1"
    if env_extra:
        env.update(env_extra)
    wait_for_memory(quiet=True)
    r = subprocess.run([exe(variant)] + args, capture_output=True, text=True, env=env)
    if r.returncode != 0:
        raise RuntimeError("%s %s failed: %s" % (variant, args, r.stderr[-2000:]))
    return r.stdout


def run_cpp_file(variant, scenes, env_extra=None):
    with tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False) as f:
        f.write(format_scenes([(n, d, []) for n, d, _ in scenes]))
        path = f.name
    try:
        return parse_scenes(run_cpp(variant, ["run", path], env_extra))
    finally:
        os.unlink(path)


def gen_variant(variant, seed, first, count):
    # The variants run one after the other, JOBS processes each (fewer for
    # the memory-hungry ASan build).
    jobs = max(1, min(JOBS, 6)) if variant == "asan" else max(1, JOBS)
    chunk = max(1, (count + jobs - 1) // jobs)
    chunk = min(chunk, 25)
    tasks = [(f, min(chunk, first + count - f)) for f in range(first, first + count, chunk)]
    with ThreadPoolExecutor(jobs) as ex:
        outs = list(ex.map(lambda t: run_cpp(variant, ["gen", str(seed), str(t[0]), str(t[1])]), tasks))
    res = []
    for o in outs:
        res.extend(parse_scenes(o))
    return res


def is_crash(out):
    return bool(out) and out[0].startswith("#crash")


def classify(a, n, z):
    """Golden output lines of one scene from assert / ndebug / asan outputs."""
    if n and n[0] == "#crash memory":
        return ["#exclude memory"]
    if is_crash(n):
        reason = z[0][len("#crash "):] if is_crash(z) else n[0][len("#crash "):]
        return ["#exclude crash " + reason]
    if z and z[0] == "#crash memory":
        return ["#exclude memory (asan)"] + [l for l in n if l != "#pinset-unsorted"]
    if is_crash(z):
        return ["#exclude ub " + z[0][len("#crash "):]] + [l for l in n if l != "#pinset-unsorted"]
    if n and n[0] == "#pinset-unsorted":
        # transformConnectionPinPositions() left a std::set unsorted (UB).
        return ["#exclude pinset-unsorted"] + n[1:]
    # (The ASan build runs on a different allocator, so its output is not
    # compared with the bump builds'.)
    if a and a[0] == "#crash memory":
        return n
    if is_crash(a):
        return ["#assert " + a[0][len("#crash "):]] + n
    if a != n:
        return ["#exclude assertdiff"] + n
    return n


def gen(seed, first, count):
    results = {}
    for v in VARIANTS:
        results[v] = gen_variant(v, seed, first, count)
    scenes = []
    for (na, da, a), (nn, dn, n), (nz, dz, z) in zip(results["assert"], results["ndebug"], results["asan"]):
        assert na == nn == nz and da == dn == dz
        scenes.append((nn, dn, classify(a, n, z)))
    return scenes


def classify_scenes(scenes):
    """Runs DSL-only scenes on all builds and classifies them."""
    res = {}
    for v in VARIANTS:
        res[v] = run_cpp_file(v, scenes)
    return [(n, d, classify(a, nn, z)) for (n, d, a), (_, _, nn), (_, _, z) in
            zip(res["assert"], res["ndebug"], res["asan"])]


_rust_bin = None


def rust_bin():
    global _rust_bin
    if _rust_bin:
        return _rust_bin
    r = subprocess.run(["cargo", "test", "--release", "--test", "e2e_fuzz", "--no-run",
                        "--message-format=json"], cwd=ROOT, capture_output=True, text=True)
    if r.returncode != 0:
        sys.stderr.write(r.stderr)
        raise RuntimeError("cargo build failed")
    for line in r.stdout.splitlines():
        try:
            m = json.loads(line)
        except ValueError:
            continue
        if m.get("reason") == "compiler-artifact" and m.get("target", {}).get("name") == "e2e_fuzz" \
                and m.get("executable"):
            _rust_bin = m["executable"]
    if not _rust_bin:
        raise RuntimeError("no e2e_fuzz test binary")
    return _rust_bin


def compare(golden_path, soft=False, scene=None, full=False, quiet=False, env_extra=None):
    """Returns {name: status} from the Rust replay of a golden file."""
    with tempfile.NamedTemporaryFile("r", suffix=".tsv", delete=False) as rep:
        rep_path = rep.name
    env = dict(os.environ)
    env["E2E_GOLDEN"] = golden_path
    env["E2E_REPORT"] = rep_path
    env["RUST_MIN_STACK"] = str(64 * 1024 * 1024)
    env.setdefault("E2E_THREADS", "3")
    if soft:
        env["E2E_SOFT"] = "1"
    if scene:
        env["E2E_SCENE"] = scene
    if full:
        env["E2E_FULL"] = "1"
    if env_extra:
        env.update(env_extra)
    r = subprocess.run([rust_bin(), "--nocapture", "--test-threads", "1"], capture_output=True,
                       text=True, env=env)
    status = {}
    try:
        for line in open(rep_path):
            name, st = line.rstrip("\n").split("\t", 1)
            status[name] = st
    except FileNotFoundError:
        pass
    os.unlink(rep_path) if os.path.exists(rep_path) else None
    if not status:
        status["<process>"] = "died: " + r.stderr[-500:].replace("\n", " | ")
    if not quiet:
        sys.stdout.write(r.stdout if full else "\n".join(l for l in r.stdout.splitlines()
                                                          if "scenes in golden" in l) + "\n")
    return status, r.stdout


def summarize(scenes, status):
    counts = {}
    for name, _, out in scenes:
        first = out[0] if out else ""
        if first.startswith("#exclude"):
            key = "excluded: " + " ".join(first.split()[1:2])
        elif first.startswith("#assert"):
            key = "compared (assert-only, NDEBUG output)"
        else:
            key = "compared (clean)"
        counts[key] = counts.get(key, 0) + 1
    return counts


def cmd_gen(args):
    scenes = gen(args.seed, args.first, args.count)
    with open(args.out, "w") as f:
        f.write("# e2e fuzz golden: seed %d, scenes %d..%d (tools/oracle/e2e_fuzz.sh)\n"
                % (args.seed, args.first, args.first + args.count - 1))
        f.write(format_scenes(scenes))
    for k, v in sorted(summarize(scenes, {}).items()):
        print("%6d %s" % (v, k))


def cmd_compare(args):
    status, _ = compare(args.golden, soft=args.soft)
    bad = {n: s for n, s in status.items() if s not in ("match", "skip-assert", "excluded",
                                                        "excluded-match")}
    for n, s in sorted(bad.items()):
        print("%s\t%s" % (n, s))
    return 1 if bad else 0


def exclusion_reasons(scenes):
    reasons = {}
    for name, _, out in scenes:
        if out and out[0].startswith("#exclude"):
            r = out[0][len("#exclude "):]
            # Normalise: strip line numbers in "file:line:col".
            reasons.setdefault(r, []).append(name)
    return reasons


CAMPAIGN_LOG = os.path.join(ROOT, "tools", "oracle", "e2e_fuzz_campaign.log")


def short_reason(r):
    """Exclusion reason without addresses, for aggregation."""
    import re
    r = re.sub(r"0x[0-9a-f]*", "", r)
    r = re.sub(r" \(pc .*T0\)", "", r)
    r = re.sub(r" at pc .*sp  ?", " ", r)
    return r[:160]


def available_gb():
    for line in open("/proc/meminfo"):
        if line.startswith("MemAvailable:"):
            return int(line.split()[1]) / (1 << 20)
    return 0.0


def wait_for_memory(quiet=False):
    """Blocks while less than E2E_MIN_FREE_GB (default 6) is available
    (called before every C++ process and every batch)."""
    import time
    need = float(os.environ.get("E2E_MIN_FREE_GB", "6"))
    while available_gb() < need:
        if not quiet:
            print("waiting: %.1f GB available (< %.1f)" % (available_gb(), need), flush=True)
        time.sleep(10)


def cmd_campaign(args):
    """Runs batches of scenes; every finished batch appends one JSON line to
    tools/oracle/e2e_fuzz_campaign.log, and batches already in the log are
    skipped, so an interrupted campaign resumes where it stopped."""
    outdir = os.path.join(BUILD, "campaign-%d" % args.seed)
    os.makedirs(outdir, exist_ok=True)
    done = {}
    if os.path.exists(CAMPAIGN_LOG):
        for line in open(CAMPAIGN_LOG):
            try:
                e = json.loads(line)
            except ValueError:
                continue
            done[(e["seed"], e["first"], e["count"])] = e
    for first in range(args.first, args.first + args.scenes, args.batch):
        count = min(args.batch, args.first + args.scenes - first)
        if (args.seed, first, count) in done:
            continue
        wait_for_memory()
        scenes = gen(args.seed, first, count)
        path = os.path.join(outdir, "batch-%06d.txt" % first)
        with open(path, "w") as f:
            f.write(format_scenes(scenes))
        status, _ = compare(path, quiet=True)
        # The same replay with arena slot reuse disabled must give the same
        # statuses (else a slot index leaks into behaviour).
        status2, _ = compare(path, quiet=True, env_extra={"E2E_NO_SLOT_REUSE": "1"})
        slot_diff = sorted(n for n in set(status) | set(status2) if status.get(n) != status2.get(n))
        divergent = sorted((n, s) for n, s in status.items()
                           if s.startswith(("differ", "panic", "died")))
        keep = [sc for sc in scenes if status.get(sc[0], "").startswith(("differ", "panic"))]
        if keep or divergent or slot_diff:
            with open(os.path.join(outdir, "divergent-%06d.txt" % first), "w") as f:
                f.write(format_scenes(keep))
            os.rename(path, os.path.join(outdir, "batch-%06d-kept.txt" % first))
        else:
            os.unlink(path)
        reasons = {}
        for r, names in exclusion_reasons(scenes).items():
            k = short_reason(r)
            reasons[k] = reasons.get(k, 0) + len(names)
        rust = {}
        for n, st in status.items():
            rust[st.split()[0]] = rust.get(st.split()[0], 0) + 1
        entry = {"seed": args.seed, "first": first, "count": count,
                 "classes": summarize(scenes, status), "rust": rust,
                 "reasons": reasons, "divergent": divergent, "slot_reuse_diff": slot_diff}
        with open(CAMPAIGN_LOG, "a") as f:
            f.write(json.dumps(entry, sort_keys=True) + "\n")
        done[(args.seed, first, count)] = entry
        print("batch seed %d %d..%d: %s | rust: %s | divergent: %d" % (
            args.seed, first, first + count - 1, sorted(entry["classes"].items()),
            sorted(rust.items()), len(divergent)), flush=True)
    report(done, args.seed)


def report(done, seed=None):
    total, rust, reasons, divergent, n = {}, {}, {}, [], 0
    for (s, first, count), e in sorted(done.items()):
        if seed is not None and s != seed:
            continue
        n += count
        for k, v in e["classes"].items():
            total[k] = total.get(k, 0) + v
        for k, v in e["rust"].items():
            rust[k] = rust.get(k, 0) + v
        for k, v in e["reasons"].items():
            reasons[k] = reasons.get(k, 0) + v
        divergent.extend(e["divergent"])
        divergent.extend([(n, "slot-reuse-dependent") for n in e.get("slot_reuse_diff", [])])
    print("\n== campaign%s: %d scenes ==" % ("" if seed is None else " seed %d" % seed, n))
    for k, v in sorted(total.items()):
        print("%6d %s" % (v, k))
    print("rust replay statuses: %s" % sorted(rust.items()))
    print("exclusion reasons:")
    for r, c in sorted(reasons.items(), key=lambda x: -x[1]):
        print("%6d %s" % (c, r))
    print("divergent scenes: %d" % len(divergent))
    for d in divergent:
        print("  %s\t%s" % tuple(d))


def cmd_report(args):
    done = {}
    for line in open(CAMPAIGN_LOG):
        e = json.loads(line)
        done[(e["seed"], e["first"], e["count"])] = e
    report(done, args.seed)


def find_scene(golden, name):
    for sc in parse_scenes(open(golden).read()):
        if sc[0] == name:
            return sc
    raise SystemExit("scene %s not found" % name)


def ddmin(lines, pred):
    """Delta debugging over lines[1:] (the 'router' line is kept)."""
    head, body = lines[:1], lines[1:]
    n = 2
    while len(body) >= 1:
        chunk = max(1, len(body) // n)
        reduced = False
        for start in range(0, len(body), chunk):
            cand = body[:start] + body[start + chunk:]
            if pred(head + cand):
                body = cand
                n = max(n - 1, 2)
                reduced = True
                print("  reduced to %d lines" % (len(body) + 1), flush=True)
                break
        if not reduced:
            if chunk == 1:
                break
            n = min(len(body), n * 2)
    return head + body


def divergence_pred(name):
    tmp = tempfile.mkdtemp()

    def pred(dsl):
        scenes = classify_scenes([(name, dsl, [])])
        out = scenes[0][2]
        if out and out[0].startswith(("#exclude", "#assert")) and not out[0].startswith("#assert"):
            return False
        path = os.path.join(tmp, "g.txt")
        with open(path, "w") as f:
            f.write(format_scenes(scenes))
        status, _ = compare(path, quiet=True)
        st = status.get(name, "")
        return st.startswith("differ") or st.startswith("panic") or st.startswith("died") or \
            "<process>" in status
    return pred


def crash_pred(name, sig, variant):
    def pred(dsl):
        out = run_cpp_file(variant, [(name, dsl, [])])[0][2]
        return is_crash(out) and sig in out[0]
    return pred


def cmd_reduce(args):
    name, dsl, _ = find_scene(args.golden, args.scene)
    if args.crash:
        pred = crash_pred(name, args.crash, args.variant)
    else:
        pred = divergence_pred(name)
    if not pred(dsl):
        raise SystemExit("predicate does not hold for the original scene")
    red = ddmin(dsl, pred)
    scenes = classify_scenes([(name + "-min", red, [])]) if not args.crash else [(name + "-min", red, [])]
    text = format_scenes(scenes)
    if args.out:
        open(args.out, "w").write(text)
    print(text)


def cmd_show(args):
    name, dsl, _ = find_scene(args.golden, args.scene)
    cpp = {}
    for v in VARIANTS:
        cpp[v] = run_cpp_file(v, [(name, dsl, [])], {"E2E_FULL": "1"})[0][2]
    tmp = tempfile.mkdtemp()
    path = os.path.join(tmp, "g.txt")
    with open(path, "w") as f:
        f.write(format_scenes([(name, dsl, cpp["ndebug"])]))
    _, rust_out = compare(path, scene=name, full=True, quiet=True)
    rust_lines = []
    on = False
    for l in rust_out.splitlines():
        if l.startswith("#scene "):
            on = True
            continue
        if on and (l.startswith("test ") or "scenes in golden" in l):
            break
        if on:
            rust_lines.append(l)
    for v in VARIANTS:
        open(os.path.join(tmp, "cpp-%s.txt" % v), "w").write("\n".join(cpp[v]) + "\n")
    open(os.path.join(tmp, "rust.txt"), "w").write("\n".join(rust_lines) + "\n")
    print("full dumps in %s (cpp-ndebug.txt, cpp-assert.txt, cpp-asan.txt, rust.txt)" % tmp)
    subprocess.run(["diff", os.path.join(tmp, "cpp-ndebug.txt"), os.path.join(tmp, "rust.txt")])


def cmd_build(args):
    subprocess.run([os.path.join(ROOT, "tools", "oracle", "e2e_fuzz_build.sh")], check=True)
    print(rust_bin())


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("build")
    s.set_defaults(fn=cmd_build)
    s = sub.add_parser("gen")
    s.add_argument("--seed", type=int, required=True)
    s.add_argument("--first", type=int, default=0)
    s.add_argument("--count", type=int, required=True)
    s.add_argument("--out", required=True)
    s.set_defaults(fn=cmd_gen)
    s = sub.add_parser("compare")
    s.add_argument("--golden", required=True)
    s.add_argument("--soft", action="store_true")
    s.set_defaults(fn=cmd_compare)
    s = sub.add_parser("campaign")
    s.add_argument("--seed", type=int, required=True)
    s.add_argument("--scenes", type=int, required=True)
    s.add_argument("--first", type=int, default=0)
    s.add_argument("--batch", type=int, default=500)
    s.set_defaults(fn=cmd_campaign)
    s = sub.add_parser("report")
    s.add_argument("--seed", type=int)
    s.set_defaults(fn=cmd_report)
    s = sub.add_parser("reduce")
    s.add_argument("--golden", required=True)
    s.add_argument("--scene", required=True)
    s.add_argument("--crash")
    s.add_argument("--variant", default="asan")
    s.add_argument("--out")
    s.set_defaults(fn=cmd_reduce)
    s = sub.add_parser("show")
    s.add_argument("--golden", required=True)
    s.add_argument("--scene", required=True)
    s.set_defaults(fn=cmd_show)
    a = p.parse_args()
    sys.exit(a.fn(a) or 0)


if __name__ == "__main__":
    main()
