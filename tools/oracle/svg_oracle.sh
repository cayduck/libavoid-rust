#!/usr/bin/env bash
# Records upstream's debug output -- Router::outputInstanceToSVG() and
# Router::outputDiagramSVG() -- for every op script the port already compares
# with the C++ (the upstream fixtures in tests/fixtures/ and the audit scenes
# in tests/audit_scenes/), and writes the manifest the Rust test
# tests/svg_oracle.rs compares against.
#
#   tools/oracle/svg_oracle.sh [script-name ...]
#
# The library is a copy of the tools/oracle/build.sh sources (the %.17g /
# junction / ORACLE_FINAL patches) with one more patch: when ORACLE_SVG is
# set, Router::outputDiagramText(name) first calls
#     outputInstanceToSVG(name);           -> name.svg
#     outputDiagramSVG(name + "-diagram"); -> name-diagram.svg
# outputDiagramText() is what every `output` op (outputDiagram()) and the
# final dump in ~Router (ORACLE_FINAL) call, so the SVGs are taken at
# exactly the points where the text goldens are, and the Rust test takes
# them at the same points.  Flags: -O2 -DNDEBUG -ffp-contract=off
# -fno-access-control, linked with the bump allocator
# (tools/oracle/bump_alloc.cpp: pointer order == allocation order, the
# port's model; the SVG output itself prints no addresses).
#
# Output:
#   tests/golden-svg/manifest.txt   "<golden-name>.svg <sha256> <bytes>" for
#                                   every SVG (sorted), one section per script
#   tests/golden-svg/full/*.svg     a few complete files (SVG_FULL list
#                                   below), for readable diffs
# Scripts on which the C++ fails (no goldens for them) are skipped; the
# summary lists them.  Env: SVG_JOBS (default 6), SVG_TIMEOUT (seconds per
# script, default 900).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BUILD="$ROOT/target/svg-oracle"
OUT="$ROOT/tests/golden-svg"
JOBS="${SVG_JOBS:-6}"
TIMEOUT="${SVG_TIMEOUT:-900}"
# Complete SVGs committed for readable diffs (small, but covering clusters,
# pins, junctions, checkpoints, fixed routes and polyline routing).
SVG_FULL="2junctions-1 checkpoints01-1 connectionpin01-1 junction01 audit_000-1"

ORACLE_LIB_ONLY=1 "$ROOT/tools/oracle/build.sh" > /dev/null
mkdir -p "$BUILD/libavoid" "$BUILD/run" "$OUT/full"
cp -u "$ROOT/target/oracle/libavoid/"*.cpp "$ROOT/target/oracle/libavoid/"*.h "$BUILD/libavoid/"
python3 - "$ROOT/target/oracle/libavoid/router.cpp" "$BUILD/libavoid/router.cpp" <<'PY'
import sys
s = open(sys.argv[1]).read()
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

FLAGS=(-O2 -std=c++11 -w -DNDEBUG -ffp-contract=off -fno-access-control -I"$BUILD")
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
  mkdir -p "$d/output"
  find "$d/output" -maxdepth 1 -type f \( -name '*.svg' -o -name '*.txt' \) -delete
  python3 "$ROOT/tools/fixtures/convert.py" --emit-cpp "$(ops_of "$n")" "$d/$n.cpp" 2> "$d/emit.log" \
    || { echo "EMIT-FAIL $n"; return; }
  g++ "${FLAGS[@]}" "$d/$n.cpp" "$BUILD/bump_alloc.o" "$BUILD/lib.a" -o "$d/$n" 2> "$d/compile.log" \
    || { echo "COMPILE-FAIL $n"; return; }
  local code=0
  (cd "$d" && ulimit -v 4194304 && ORACLE_SVG=1 ORACLE_FINAL="output/$n" \
      timeout -s KILL "$TIMEOUT" "./$n" > "$d/stdout" 2> "$d/stderr") || code=$?
  if [ "$code" -ne 0 ]; then echo "FAIL $n (exit $code)"; return; fi
  echo "OK $n $(ls "$d/output" | grep -c '\.svg$')"
}
for n in "${names[@]}"; do
  run_one "$n" > "$BUILD/run/$n.result" &
  while [ "$(jobs -r | wc -l)" -ge "$JOBS" ]; do sleep 0.1; done
done
wait

python3 - "$BUILD/run" "$OUT" "$SVG_FULL" "${names[@]}" <<'PY'
import hashlib, os, shutil, sys
run, out, full = sys.argv[1], sys.argv[2], sys.argv[3].split()
names = sys.argv[4:]
manifest = os.path.join(out, "manifest.txt")
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
ok = fail = 0
for n in names:
    res = open(os.path.join(run, n + ".result")).read().split()
    entries.pop(n, None)
    if not res or res[0] != "OK":
        fail += 1
        print(" ".join(res))
        continue
    ok += 1
    d = os.path.join(run, n, "output")
    lines = []
    for f in sorted(os.listdir(d)):
        if not f.endswith(".svg"):
            continue
        data = open(os.path.join(d, f), "rb").read()
        lines.append("%s %s %d" % (f, hashlib.sha256(data).hexdigest(), len(data)))
        if f[:-4] in full:
            shutil.copy(os.path.join(d, f), os.path.join(out, "full", f))
    entries[n] = lines
with open(manifest, "w") as m:
    m.write("# Generated by tools/oracle/svg_oracle.sh: upstream outputInstanceToSVG()\n")
    m.write("# (NAME.svg) and outputDiagramSVG() (NAME-diagram.svg) at every output\n")
    m.write("# and final dump of each script: file sha256 bytes.\n")
    for n in sorted(entries):
        m.write("script %s\n" % n)
        for l in entries[n]:
            m.write(l + "\n")
print("svg oracle: %d scripts ok, %d failed; %d svgs in manifest" %
      (ok, fail, sum(len(v) for v in entries.values())))
PY
