#!/usr/bin/env bash
# Records upstream's debug output for every op script the port already
# compares with the C++ (the upstream fixtures in tests/fixtures/ and the
# audit scenes in tests/audit_scenes/), and writes the manifests the Rust
# tests compare against.
#
#   tools/oracle/svg_oracle.sh [script-name ...]
#   SVG_MODE=svg-high-precision tools/oracle/svg_oracle.sh [script-name ...]
#   SVG_MODE=hyperedge-debug tools/oracle/svg_oracle.sh [script-name ...]
#   SVG_MODE=debug-handler tools/oracle/svg_oracle.sh [script-name ...]
#   SVG_MODE=profile tools/oracle/svg_oracle.sh [script-name ...]
#   SVG_MODE=libavoid-debug tools/oracle/svg_oracle.sh [script-name ...]
#   SVG_MODE=path-debug tools/oracle/svg_oracle.sh [script-name ...]
#   SVG_MODE=nudge-debug tools/oracle/svg_oracle.sh [script-name ...]
#   SVG_MODE=estimated-cost-debug tools/oracle/svg_oracle.sh [script-name ...]
#
# Default mode: Router::outputInstanceToSVG() and Router::outputDiagramSVG()
# (tests/svg_oracle.rs).  The library is a copy of the tools/oracle/build.sh
# sources (the %.17g / junction / ORACLE_FINAL patches) with one more patch:
# when ORACLE_SVG is set, Router::outputDiagramText(name) first calls
#     outputInstanceToSVG(name);           -> name.svg
#     outputDiagramSVG(name + "-diagram"); -> name-diagram.svg
# outputDiagramText() is what every `output` op (outputDiagram()) and the
# final dump in ~Router (ORACLE_FINAL) call, so the SVGs are taken at
# exactly the points where the text goldens are, and the Rust test takes
# them at the same points.
#
# svg-high-precision mode: the same with -DHIGH_PRECISION_DEBUG (PREC ".16"
# in the reproduction code; tests/svg_oracle.rs with the
# high-precision-debug feature).
#
# hyperedge-debug mode (tests/hyperedge_debug_oracle.rs): the same library
# compiled with -DHYPEREDGE_DEBUG -DMAJOR_HYPEREDGE_IMPROVEMENT_DEBUG.
# HyperedgeImprover::outputHyperedgesToSVG() then writes
# DEBUG/hyperedges-NNNNN.svg (later passes overwrite earlier files of the
# same number, in the port too) and the topology reports go to stderr.
# Recorded for the scripts that write at least one such file: every
# DEBUG/*.svg and the whole stderr.
#
# debug-handler mode (tests/debughandler_oracle.rs): the library compiled
# with -DDEBUGHANDLER, plus tools/oracle/debughandler_oracle.cpp; the
# Router constructor is patched to install that recording DebugHandler when
# ORACLE_DEBUGHANDLER is set, which appends every call (vertices and routes
# printed by value, %.17g) to debughandler.log.  Recorded for every script.
#
# profile mode (tests/timer_oracle.rs): the library compiled with
# -DAVOID_PROFILE (Router::timers and the TIMER_* instrumentation).  When
# ORACLE_TIMERS is set, outputDiagramText(name) first writes
# name-timers.txt: per timer its name, tally and the two TIMER_VAR_*
# variables -- everything the timers record except the times themselves,
# which are not reproducible.
#
# libavoid-debug mode (tests/libavoid_debug_oracle.rs): the library
# compiled with -DLIBAVOID_DEBUG, so db_printf() prints.  The copy of
# debug.h is patched to send db_printf() to dbprintf.log instead of stdout,
# which keeps it apart from what the fixtures print themselves; the text is
# unchanged.  Recorded for every script.  path-debug mode: the same with
# -DPATHDEBUG -DASTAR_DEBUG as well (path search tracing through db_printf).
#
# estimated-cost-debug mode (tests/estimated_cost_debug_oracle.rs): the
# library compiled with -DESTIMATED_COST_DEBUG, which prints the A* cost
# targets to stderr.  Recorded: the whole stderr of every script.
#
# nudge-debug mode (tests/nudge_debug_oracle.rs): the library compiled with
# -DNUDGE_DEBUG -DNUDGE_DEBUG_SVG.  Recorded: stdout and stderr of every
# script that prints nothing itself (its golden stdout is empty), so that
# stdout is the library's output only.  NUDGE_DEBUG prints a ConnRef
# pointer ("Pos: %lX, ..."); it is replaced by "Pos: <ptr>," before
# hashing (the port prints a creation serial there; the test masks it the
# same way).
#
# All modes: -O2 -DNDEBUG -ffp-contract=off -fno-access-control, linked
# with the bump allocator (tools/oracle/bump_alloc.cpp: pointer order ==
# allocation order, the port's model; none of this output prints
# addresses).
#
# Output:
#   tests/golden-svg/manifest.txt   "<file> <sha256> <bytes>" for every SVG
#                                   (sorted), one section per script
#   tests/golden-svg/high-precision-manifest.txt    (svg-high-precision mode)
#   tests/golden-svg/hyperedge-debug-manifest.txt   (hyperedge-debug mode)
#   tests/golden-svg/debug-handler-manifest.txt     (debug-handler mode)
#   tests/golden-svg/profile-manifest.txt           (profile mode)
#   tests/golden-svg/libavoid-debug-manifest.txt    (libavoid-debug mode)
#   tests/golden-svg/path-debug-manifest.txt        (path-debug mode)
#   tests/golden-svg/nudge-debug-manifest.txt       (nudge-debug mode)
#   tests/golden-svg/estimated-cost-debug-manifest.txt (estimated-cost-debug)
#   tests/golden-svg/full/*.svg     a few complete files (SVG_FULL list
#                                   below), for readable diffs
# Scripts on which the C++ fails (no goldens for them) are skipped; the
# summary lists them.  Env: SVG_JOBS (default 6), SVG_TIMEOUT (seconds per
# script, default 900).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
MODE="${SVG_MODE:-svg}"
OUT="$ROOT/tests/golden-svg"
JOBS="${SVG_JOBS:-6}"
TIMEOUT="${SVG_TIMEOUT:-900}"
case "$MODE" in
  svg)
    BUILD="$ROOT/target/svg-oracle"
    EXTRA_FLAGS=()
    MANIFEST="$OUT/manifest.txt"
    # Complete SVGs committed for readable diffs (small, but covering
    # clusters, pins, junctions, checkpoints, fixed routes and polyline
    # routing).
    SVG_FULL="2junctions-1 checkpoints01-1 connectionpin01-1 junction01 audit_000-1"
    ;;
  svg-high-precision)
    BUILD="$ROOT/target/svg-oracle-high-precision"
    EXTRA_FLAGS=(-DHIGH_PRECISION_DEBUG)
    MANIFEST="$OUT/high-precision-manifest.txt"
    SVG_FULL=""
    ;;
  hyperedge-debug)
    BUILD="$ROOT/target/svg-oracle-hyperedge-debug"
    EXTRA_FLAGS=(-DHYPEREDGE_DEBUG -DMAJOR_HYPEREDGE_IMPROVEMENT_DEBUG)
    MANIFEST="$OUT/hyperedge-debug-manifest.txt"
    SVG_FULL=""
    ;;
  debug-handler)
    BUILD="$ROOT/target/svg-oracle-debug-handler"
    EXTRA_FLAGS=(-DDEBUGHANDLER)
    MANIFEST="$OUT/debug-handler-manifest.txt"
    SVG_FULL=""
    ;;
  profile)
    BUILD="$ROOT/target/svg-oracle-profile"
    EXTRA_FLAGS=(-DAVOID_PROFILE)
    MANIFEST="$OUT/profile-manifest.txt"
    SVG_FULL=""
    ;;
  estimated-cost-debug)
    BUILD="$ROOT/target/svg-oracle-estimated-cost-debug"
    EXTRA_FLAGS=(-DESTIMATED_COST_DEBUG)
    MANIFEST="$OUT/estimated-cost-debug-manifest.txt"
    SVG_FULL=""
    ;;
  nudge-debug)
    BUILD="$ROOT/target/svg-oracle-nudge-debug"
    EXTRA_FLAGS=(-DNUDGE_DEBUG -DNUDGE_DEBUG_SVG)
    MANIFEST="$OUT/nudge-debug-manifest.txt"
    SVG_FULL=""
    ;;
  path-debug)
    BUILD="$ROOT/target/svg-oracle-path-debug"
    EXTRA_FLAGS=(-DLIBAVOID_DEBUG -DPATHDEBUG -DASTAR_DEBUG)
    MANIFEST="$OUT/path-debug-manifest.txt"
    SVG_FULL=""
    ;;
  libavoid-debug)
    BUILD="$ROOT/target/svg-oracle-libavoid-debug"
    EXTRA_FLAGS=(-DLIBAVOID_DEBUG)
    MANIFEST="$OUT/libavoid-debug-manifest.txt"
    SVG_FULL=""
    ;;
  *) echo "unknown SVG_MODE $MODE"; exit 2 ;;
esac

ORACLE_LIB_ONLY=1 "$ROOT/tools/oracle/build.sh" > /dev/null
mkdir -p "$BUILD/libavoid" "$BUILD/run" "$OUT/full"
cp -u "$ROOT/target/oracle/libavoid/"*.cpp "$ROOT/target/oracle/libavoid/"*.h "$BUILD/libavoid/"
python3 - "$ROOT/target/oracle/libavoid/router.cpp" "$BUILD/libavoid/router.cpp" "$MODE" <<'PY'
import sys
s = open(sys.argv[1]).read()
if sys.argv[3] == "debug-handler":
    ctor = """    // At least one of the Routing modes must be set.
    COLA_ASSERT(flags & (PolyLineRouting | OrthogonalRouting));
"""
    assert s.count(ctor) == 1
    s = s.replace(ctor, ctor + """    if (getenv("ORACLE_DEBUGHANDLER"))
    {
        m_debug_handler = oracleDebugHandler();
    }
""")
    s = s.replace("namespace Avoid {", "namespace Avoid {\nDebugHandler *oracleDebugHandler(void);", 1)
if sys.argv[3] == "profile":
    old = """void Router::outputDiagramText(std::string instanceName)
{
"""
    assert s.count(old) == 1
    s = s.replace(old, old + """    if (getenv("ORACLE_TIMERS"))
    {
        static const char *names[] = { "Adds", "Dels", "Movs", "Pths",
            "OrthogGraph", "OrthogRoute", "OrthogCentre", "OrthogNudge",
            "HyperedgeForest", "HyperedgeMTST", "HyperedgeImprove",
            "HyperedgeAlt" };
        FILE *tfp = fopen((instanceName + "-timers.txt").c_str(), "w");
        for (int t = 0; t < tmCount; ++t)
        {
            fprintf(tfp, "%s %d %u %u\\n", names[t], timers.m_tally[t],
                    timers.m_variables[t][0], timers.m_variables[t][1]);
        }
        fclose(tfp);
    }
""")
old = """void Router::outputDiagramText(std::string instanceName)
{
"""
assert s.count(old) == 1
s = s.replace(old, old + """    if (getenv("ORACLE_SVG"))
    {
        outputInstanceToSVG(instanceName);
        outputDiagramSVG(instanceName + "-diagram");
    }
""")
s = "#include <cstdlib>\n" + s
try:
    if open(sys.argv[2]).read() == s:
        sys.exit(0)
except FileNotFoundError:
    pass
open(sys.argv[2], "w").write(s)
PY

python3 - "$ROOT/target/oracle/libavoid/debug.h" "$BUILD/libavoid/debug.h" "$MODE" <<'PY'
import sys
s = open(sys.argv[1]).read()
if sys.argv[3] in ("libavoid-debug", "path-debug"):
    old = """        va_start(ap, fmt);
        vfprintf(stdout, fmt, ap);
"""
    assert s.count(old) == 1
    s = s.replace(old, """        va_start(ap, fmt);
        static FILE *oracleDbFile = fopen("dbprintf.log", "w");
        vfprintf(oracleDbFile, fmt, ap);
""")
try:
    if open(sys.argv[2]).read() == s:
        sys.exit(0)
except FileNotFoundError:
    pass
open(sys.argv[2], "w").write(s)
PY

FLAGS=(-O2 -std=c++11 -w -DNDEBUG -ffp-contract=off -fno-access-control "${EXTRA_FLAGS[@]}" -I"$BUILD")
objs=()
running=0
for f in "$BUILD"/libavoid/*.cpp; do
  o="${f%.cpp}.o"; objs+=("$o")
  if [ ! -f "$o" ] || [ "$f" -nt "$o" ]; then
    g++ "${FLAGS[@]}" -c "$f" -o "$o" &
    running=$((running + 1))
    if [ "$running" -ge 4 ]; then wait -n; running=$((running - 1)); fi
  fi
done
wait
if [ "$MODE" = debug-handler ]; then
  g++ "${FLAGS[@]}" -c "$ROOT/tools/oracle/debughandler_oracle.cpp" -o "$BUILD/debughandler_oracle.o"
  objs+=("$BUILD/debughandler_oracle.o")
fi
rm -f "$BUILD/lib.a"; ar rcs "$BUILD/lib.a" "${objs[@]}"
# 3 GiB arena: fits the 4 GiB `ulimit -v` of the runs below.
g++ "${FLAGS[@]}" -DORACLE_ARENA_SIZE=3221225472 -c "$ROOT/tools/oracle/bump_alloc.cpp" -o "$BUILD/bump_alloc.o"

ops_of() {
  case "$1" in
    audit_*) echo "$ROOT/tests/audit_scenes/$1.ops" ;;
    *) echo "$ROOT/tests/fixtures/$1.ops" ;;
  esac
}

if [ $# -gt 0 ]; then names=("$@"); else
  names=()
  for f in "$ROOT"/tests/fixtures/*.ops; do
    # Golden-mode fixtures only (smoke fixtures do not compile upstream).
    if awk '$1=="mode"{exit !($2=="golden")}' "$f"; then names+=("$(basename "$f" .ops)"); fi
  done
  for f in "$ROOT"/tests/audit_scenes/audit_*.ops; do
    n="$(basename "$f" .ops)"
    # Audit scenes the C++ is undefined on have no goldens.
    if [ -f "$ROOT/tests/audit_scenes/golden/$n.exit" ]; then names+=("$n"); fi
  done
fi

run_one() {
  local n="$1" d="$BUILD/run/$1"
  mkdir -p "$d/output" "$d/DEBUG"
  find "$d/output" "$d/DEBUG" -maxdepth 1 -type f \( -name '*.svg' -o -name '*.txt' \) -delete
  find "$d" -maxdepth 1 -type f \( -name debughandler.log -o -name dbprintf.log \) -delete
  python3 "$ROOT/tools/fixtures/convert.py" --emit-cpp "$(ops_of "$n")" "$d/$n.cpp" 2> "$d/emit.log" \
    || { echo "EMIT-FAIL $n"; return; }
  g++ "${FLAGS[@]}" "$d/$n.cpp" "$BUILD/bump_alloc.o" "$BUILD/lib.a" -o "$d/$n" 2> "$d/compile.log" \
    || { echo "COMPILE-FAIL $n"; return; }
  local code=0 svg="" dh="" tm=""
  case "$MODE" in svg|svg-high-precision) svg=1 ;; esac
  if [ "$MODE" = debug-handler ]; then dh=1; fi
  if [ "$MODE" = profile ]; then tm=1; fi
  (cd "$d" && ulimit -v 4194304 && ORACLE_SVG="$svg" ORACLE_DEBUGHANDLER="$dh" ORACLE_TIMERS="$tm" \
      ORACLE_FINAL="output/$n" \
      timeout -s KILL "$TIMEOUT" "./$n" > "$d/stdout" 2> "$d/stderr") || code=$?
  if [ "$code" -ne 0 ]; then echo "FAIL $n (exit $code)"; return; fi
  echo "OK $n"
}
for n in "${names[@]}"; do
  run_one "$n" > "$BUILD/run/$n.result" &
  while [ "$(jobs -r | wc -l)" -ge "$JOBS" ]; do sleep 0.1; done
done
wait

python3 - "$MODE" "$BUILD/run" "$MANIFEST" "$OUT/full" "$SVG_FULL" "$ROOT" "${names[@]}" <<'PY'
import hashlib, os, re, shutil, sys
mode, run, manifest, fulldir, full = sys.argv[1:5] + [sys.argv[5].split()]
root = sys.argv[6]
names = sys.argv[7:]
# Keep the entries of scripts not regenerated in this run.
entries = {}
if os.path.exists(manifest):
    cur = None
    for line in open(manifest):
        line = line.rstrip("\n")
        if line.startswith("script "):
            cur = line.split()[1]; entries[cur] = []
        elif line and not line.startswith("#"):
            entries[cur].append(line)
def entry(name, data):
    return "%s %s %d" % (name, hashlib.sha256(data).hexdigest(), len(data))
ok = fail = 0
for n in names:
    res = open(os.path.join(run, n + ".result")).read().split()
    entries.pop(n, None)
    if not res or res[0] != "OK":
        fail += 1
        print(" ".join(res))
        continue
    ok += 1
    d = os.path.join(run, n, "output" if mode in ("svg", "svg-high-precision", "profile") else "DEBUG")
    lines = []
    if mode == "nudge-debug":
        golden_stdout = (os.path.join(root, "tests/audit_scenes/golden", n + ".stdout") if n.startswith("audit_")
                         else os.path.join(root, "tests/golden-bump", n + ".stdout"))
        if os.path.exists(golden_stdout) and os.path.getsize(golden_stdout) > 0:
            continue
        out = open(os.path.join(run, n, "stdout"), "rb").read()
        out = re.sub(rb"Pos: [0-9A-F]+,", b"Pos: <ptr>,", out)
        err = open(os.path.join(run, n, "stderr"), "rb").read()
        entries[n] = [entry("stderr", err), entry("stdout", out)]
        continue
    if mode in ("debug-handler", "libavoid-debug", "path-debug", "estimated-cost-debug"):
        name = {"debug-handler": "debughandler.log", "libavoid-debug": "dbprintf.log",
                "path-debug": "dbprintf.log", "estimated-cost-debug": "stderr"}[mode]
        log = os.path.join(run, n, name)
        data = open(log, "rb").read() if os.path.exists(log) else b""
        entries[n] = [entry(name, data)]
        continue
    for f in sorted(os.listdir(d)):
        if not f.endswith("-timers.txt" if mode == "profile" else ".svg"):
            continue
        data = open(os.path.join(d, f), "rb").read()
        lines.append(entry(f, data))
        if f[:-4] in full:
            shutil.copy(os.path.join(d, f), os.path.join(fulldir, f))
    if mode == "hyperedge-debug":
        if not lines:
            continue
        lines.append(entry("stderr", open(os.path.join(run, n, "stderr"), "rb").read()))
    entries[n] = lines
with open(manifest, "w") as m:
    if mode in ("svg", "svg-high-precision"):
        m.write("# Generated by %stools/oracle/svg_oracle.sh: upstream outputInstanceToSVG()\n"
                % ("SVG_MODE=svg-high-precision (HIGH_PRECISION_DEBUG) " if mode != "svg" else ""))
        m.write("# (NAME.svg) and outputDiagramSVG() (NAME-diagram.svg) at every output\n")
        m.write("# and final dump of each script: file sha256 bytes.\n")
    elif mode == "profile":
        m.write("# Generated by SVG_MODE=profile tools/oracle/svg_oracle.sh: upstream's\n")
        m.write("# AVOID_PROFILE timer tallies and variables (NAME-timers.txt) at every\n")
        m.write("# output and final dump of each script: file sha256 bytes.\n")
    elif mode == "nudge-debug":
        m.write("# Generated by SVG_MODE=nudge-debug tools/oracle/svg_oracle.sh: stdout\n")
        m.write("# (\"Pos: <ptr>,\" masked) and stderr under NUDGE_DEBUG and NUDGE_DEBUG_SVG of\n")
        m.write("# every script that prints nothing itself: file sha256 bytes.\n")
    elif mode == "estimated-cost-debug":
        m.write("# Generated by SVG_MODE=estimated-cost-debug tools/oracle/svg_oracle.sh: the\n")
        m.write("# stderr (ESTIMATED_COST_DEBUG cost target reports) of each script:\n")
        m.write("# file sha256 bytes.\n")
    elif mode in ("libavoid-debug", "path-debug"):
        m.write("# Generated by SVG_MODE=%s tools/oracle/svg_oracle.sh: upstream's\n" % mode)
        m.write("# db_printf() output (LIBAVOID_DEBUG%s) of each script: file sha256 bytes.\n"
                % (" PATHDEBUG ASTAR_DEBUG" if mode == "path-debug" else ""))
    elif mode == "debug-handler":
        m.write("# Generated by SVG_MODE=debug-handler tools/oracle/svg_oracle.sh: the\n")
        m.write("# calls of upstream's DebugHandler (DEBUGHANDLER) as recorded by\n")
        m.write("# tools/oracle/debughandler_oracle.cpp: file sha256 bytes.\n")
    else:
        m.write("# Generated by SVG_MODE=hyperedge-debug tools/oracle/svg_oracle.sh:\n")
        m.write("# upstream HYPEREDGE_DEBUG files DEBUG/hyperedges-NNNNN.svg and the\n")
        m.write("# stderr of the run (MAJOR_HYPEREDGE_IMPROVEMENT_DEBUG reports) of every\n")
        m.write("# script that writes such files: file sha256 bytes.\n")
    for n in sorted(entries):
        m.write("script %s\n" % n)
        for l in entries[n]:
            m.write(l + "\n")
print("svg oracle (%s): %d scripts ok, %d failed; %d scripts and %d files in manifest" %
      (mode, ok, fail, len(entries), sum(len(v) for v in entries.values())))
PY
