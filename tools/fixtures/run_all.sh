#!/usr/bin/env bash
# Runs every upstream-fixture test (tests/fixtures_ops.rs), including the
# #[ignore]d ones, each in its own process with a timeout, in release mode,
# and prints a status table:
#
#   fixture | status | detail
#
# status: PASS, TODO (hit a todo!() of an unported module), MISMATCH (golden
# diff / exit code / stdout / upstream assertion), PANIC (library panic),
# INTERP (bug in the op-script interpreter or converter), TIMEOUT, SKIP.
# The "marked" column shows the test's current #[ignore] reason, and
# PASS rows that are still ignored are flagged so the attribute can be
# removed.
#
#   tools/fixtures/run_all.sh [--no-slow] [--debug] [-j N] [fixture ...]
#
# Env: FIXTURE_TIMEOUT (seconds, default 300), SLOW_FIXTURE_TIMEOUT
# (default 3600, used for performance01, slowrouting, reallyslowrouting,
# unsatisfiableRangeAssertion), RUN_ALL_LOG_DIR (per-test logs; default
# target/fixtures-run).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
profile=(--release)
slow=1
jobs="$(( $(nproc) / 2 > 0 ? $(nproc) / 2 : 1 ))"
names=()
while [ $# -gt 0 ]; do
  case "$1" in
    --no-slow) slow=0 ;;
    --debug) profile=() ;;
    -j) shift; jobs="$1" ;;
    -h|--help) sed -n '2,22p' "$0"; exit 0 ;;
    *) names+=("$1") ;;
  esac
  shift
done

bin="$(cargo test "${profile[@]}" --test fixtures_ops --no-run --message-format=json 2>/dev/null \
  | python3 -c 'import sys, json
for l in sys.stdin:
    try: m = json.loads(l)
    except ValueError: continue
    if m.get("reason") == "compiler-artifact" and m.get("executable") and m["target"]["name"] == "fixtures_ops":
        print(m["executable"])' | tail -1)"
if [ -z "$bin" ]; then
  echo "build failed:"; cargo test "${profile[@]}" --test fixtures_ops --no-run; exit 2
fi

export FIXTURE_BIN="$bin" FIXTURE_SLOW="$slow" FIXTURE_JOBS="$jobs"
export RUN_ALL_LOG_DIR="${RUN_ALL_LOG_DIR:-$ROOT/target/fixtures-run}"
exec python3 - "${names[@]}" <<'PY'
import os, re, subprocess, sys
from concurrent.futures import ThreadPoolExecutor

BIN = os.environ["FIXTURE_BIN"]
SLOW = os.environ["FIXTURE_SLOW"] == "1"
JOBS = int(os.environ["FIXTURE_JOBS"])
LOGS = os.environ["RUN_ALL_LOG_DIR"]
TIMEOUT = int(os.environ.get("FIXTURE_TIMEOUT", "300"))
SLOW_TIMEOUT = int(os.environ.get("SLOW_FIXTURE_TIMEOUT", "3600"))
SLOW_NAMES = {"performance01", "slowrouting", "reallyslowrouting", "unsatisfiableRangeAssertion"}
os.makedirs(LOGS, exist_ok=True)

def listing(*flags):
    out = subprocess.run([BIN, "--list", *flags], capture_output=True, text=True, check=True).stdout
    return [l[:-len(": test")] for l in out.splitlines() if l.endswith(": test")]

all_tests = [t for t in listing() if t != "interpreter_dry_run"]  # self-test, not a fixture
ignored = set(listing("--ignored"))
# ignore reasons, read from the test source (the list in tests/fixtures_ops.rs)
reasons = {}
src = open("tests/fixtures_ops.rs").read()
for m in re.finditer(r'#\[ignore\s*=\s*"([^"]*)"\]\s*(?:#\[[^\]]*\]\s*)*\w+\s+(\w+)', src):
    reasons[m.group(2)] = m.group(1)
wanted = sys.argv[1:]
tests = [t for t in all_tests if not wanted or t in wanted or t.lstrip("_") in wanted]
if wanted and len(tests) != len(set(wanted)):
    sys.exit("unknown fixture(s): %s" % sorted(set(wanted) - set(tests) - {t.lstrip('_') for t in tests}))

def classify(name, rc, out, timed_out):
    fixture = name.lstrip("_") if name == "_2junctions" else name
    if timed_out:
        return "TIMEOUT", "no result within %ds" % (SLOW_TIMEOUT if fixture in SLOW_NAMES else TIMEOUT)
    if "skipping slow fixture" in out:
        return "SKIP", "slow fixture (run without --no-slow)"
    ctx = re.search(r"fixture-ops: panic while executing (\S+): `(.*)`", out)
    where = " [at %s `%s`]" % (ctx.group(1), ctx.group(2)[:60]) if ctx else ""
    if rc == 0 and re.search(r"test result: ok\. 1 passed", out):
        notes = re.findall(r"note: golden (\S+) matched within tolerance.*max \|diff\| = (\S+)\)", out)
        smoke = re.search(r"smoke fixture \S+: main\(\) returned (-?\d+)", out)
        if smoke:
            return "PASS", "smoke only (no golden), main() returned %s" % smoke.group(1)
        if notes:
            worst = max(notes, key=lambda n: float(n[1]))
            return "PASS", "within tolerance (%d dump(s) not bit-identical, max |diff| %s in %s)" % (
                len(notes), worst[1], worst[0])
        return "PASS", "bit-identical"
    m = re.search(r"panicked at ([^\n]*):\n(.*)", out)
    loc, msg = (m.group(1), m.group(2)) if m else ("?", "")
    if "not yet implemented" in msg:
        fn = re.search(r"upstream: (\S+ \S+)", msg)
        return "TODO", "todo! in %s (%s)%s" % (os.path.basename(loc.split(":")[0]) + ":" + loc.split(":")[1],
                                              fn.group(1) if fn else msg[:60], where)
    if "fixture-ops interpreter:" in msg:
        return "INTERP", msg.strip()[:160]
    g = re.search(r"golden (\S+): diagram mismatch in block #(\d+): (.*)", out)
    if g:
        d = re.search(r"\|diff\| = (\S+)\)", g.group(3))
        return "MISMATCH", "golden %s block %s: %s" % (g.group(1), g.group(2),
                                                       ("|diff| %s" % d.group(1)) if d else g.group(3)[:100])
    for pat, label in ((r"main\(\) returned .*", "exit code"), (r"stdout differs", "stdout"),
                       (r"assertion from upstream fixture failed: (.*)", "upstream assert")):
        mm = re.search(pat, out)
        if mm:
            return "MISMATCH", "%s: %s" % (label, mm.group(0)[:120])
    if m:
        return "PANIC", "%s at %s%s" % (msg.strip()[:100], loc, where)
    return "PANIC", "exit status %d without panic message (abort/signal?)" % rc

def run(name):
    fixture = "2junctions" if name == "_2junctions" else name
    env = dict(os.environ)
    if SLOW:
        env["LIBAVOID_SLOW_FIXTURES"] = "1"
    tmo = SLOW_TIMEOUT if fixture in SLOW_NAMES else TIMEOUT
    cmd = [BIN, "--exact", name, "--include-ignored", "--nocapture", "--test-threads=1"]
    try:
        p = subprocess.run(cmd, capture_output=True, text=True, timeout=tmo, env=env)
        out, rc, to = p.stdout + p.stderr, p.returncode, False
    except subprocess.TimeoutExpired as e:
        out = (e.stdout or b"").decode(errors="replace") if isinstance(e.stdout, bytes) else (e.stdout or "")
        rc, to = -1, True
    with open(os.path.join(LOGS, fixture + ".log"), "w") as fh:
        fh.write(out)
    status, detail = classify(name, rc, out, to)
    return fixture, name, status, detail

with ThreadPoolExecutor(max_workers=JOBS) as ex:
    rows = list(ex.map(run, tests))

w = max(len(r[0]) for r in rows)
print("%-*s  %-8s  %-58s  %s" % (w, "fixture", "status", "marked", "detail"))
print("-" * (w + 140))
counts = {}
for fixture, name, status, detail in sorted(rows, key=lambda r: r[0].lower()):
    marked = ("ignore: " + reasons.get(name, "?")) if name in ignored else "active"
    flag = ""
    if status == "PASS" and name in ignored and not reasons.get(name, "").startswith("slow"):
        flag = "  <-- passes: remove #[ignore]"
    if status not in ("PASS", "SKIP") and name not in ignored:
        flag = "  <-- active test fails"
    counts[status] = counts.get(status, 0) + 1
    print("%-*s  %-8s  %-58s  %s%s" % (w, fixture, status, marked[:58], detail, flag))
print()
print("total %d: %s" % (len(rows), ", ".join("%s %d" % kv for kv in sorted(counts.items()))))
print("logs: %s/<fixture>.log" % LOGS)
sys.exit(1 if counts.get("INTERP") else 0)
PY
