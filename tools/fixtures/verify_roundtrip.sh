#!/usr/bin/env bash
# Round-trip proof of the fixture op scripts against the real C++ libavoid.
#
#   tools/fixtures/verify_roundtrip.sh [fixture-name ...]
#
# For every tests/fixtures/<name>.ops this regenerates a C++ program from the
# op script (tools/fixtures/convert.py --emit-cpp), compiles it against the
# oracle build of upstream libavoid (tools/oracle/build.sh, same flags and the
# same %.17g / ORACLE_FINAL patches) and runs it.  For golden-mode scripts the
# produced output/*.txt files, the exit code and stdout must be
# byte-identical to tests/golden/.  This proves that parsing + op semantics are
# faithful independently of the Rust core.  Smoke-mode scripts (fixtures that
# do not compile upstream) are compiled and run; their exit code is reported.
#
# Also checks that the converter output is up to date (convert.py --check)
# and that every fixture golden .txt is referenced by some op script.
#
# Env: ROUNDTRIP_TIMEOUT (seconds per fixture, default 1800),
#      ROUNDTRIP_JOBS (parallel fixtures, default nproc),
#      ROUNDTRIP_SKIP_SLOW=1 (skip performance01, slowrouting,
#      reallyslowrouting, unsatisfiableRangeAssertion).
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BUILD="${ORACLE_BUILD_DIR:-$ROOT/target/oracle}"
WORK="$ROOT/target/fixtures-roundtrip"
TIMEOUT="${ROUNDTRIP_TIMEOUT:-1800}"
JOBS="${ROUNDTRIP_JOBS:-$(nproc)}"
SLOW="performance01 slowrouting reallyslowrouting unsatisfiableRangeAssertion"

ORACLE_LIB_ONLY=1 "$ROOT/tools/oracle/build.sh" || { echo "oracle library build failed"; exit 2; }
python3 "$ROOT/tools/fixtures/convert.py" --check || { echo "op scripts are stale"; exit 2; }

rm -rf "$WORK"
mkdir -p "$WORK"

if [ $# -gt 0 ]; then names=("$@"); else
  names=(); for f in "$ROOT"/tests/fixtures/*.ops; do names+=("$(basename "$f" .ops)"); done
fi

check_one() {
  local n="$1" d="$WORK/$n" ops="$ROOT/tests/fixtures/$n.ops"
  mkdir -p "$d/output"
  local mode; mode="$(awk '$1=="mode"{print $2; exit}' "$ops")"
  python3 "$ROOT/tools/fixtures/convert.py" --emit-cpp "$ops" "$d/$n.cpp" 2> "$d/emit.log" \
    || { echo "FAIL $n emit-cpp: $(head -c 300 "$d/emit.log" | tr '\n' ' ')"; return; }
  g++ -O2 -std=c++11 -w -I"$BUILD" "$d/$n.cpp" "$BUILD/libavoid.a" -o "$d/$n" 2> "$d/compile.log" \
    || { echo "FAIL $n compile: $(grep -m3 error "$d/compile.log" | tr '\n' ' ')"; return; }
  local code=0
  (cd "$d" && ORACLE_FINAL="output/$n" timeout "$TIMEOUT" "./$n" > "$d/stdout" 2>&1) || code=$?
  if [ "$mode" = smoke ]; then
    echo "SMOKE $n exit=$code (no golden; $(ls "$d/output" | wc -l) dump(s))"
    return
  fi
  local problems=()
  [ "$code" = "$(cat "$ROOT/tests/golden/$n.exit")" ] || problems+=("exit $code != $(cat "$ROOT/tests/golden/$n.exit")")
  cmp -s "$d/stdout" "$ROOT/tests/golden/$n.stdout" || problems+=("stdout differs")
  # Every golden named by the script must have been produced identically ...
  local expected
  expected="$(awk '$1=="output"||$1=="output-unchecked"||($1=="delete"&&$3!="-"){print $3}' "$ops" | sort -u)"
  local g
  for g in $expected; do
    if [ ! -f "$d/output/$g.txt" ]; then problems+=("$g.txt not produced")
    elif ! cmp -s "$d/output/$g.txt" "$ROOT/tests/golden/$g.txt"; then problems+=("$g.txt differs"); fi
  done
  # ... and nothing else may have been produced.
  local f
  for f in "$d"/output/*.txt; do
    [ -e "$f" ] || continue
    g="$(basename "$f" .txt)"
    grep -qx "$g" <<< "$expected" || problems+=("unexpected output $g.txt")
  done
  if [ ${#problems[@]} -eq 0 ]; then
    echo "PASS $n ($(wc -w <<< "$expected") golden file(s), exit $code, stdout identical)"
  else
    echo "FAIL $n: ${problems[*]}"
  fi
}

for n in "${names[@]}"; do
  if [ -n "${ROUNDTRIP_SKIP_SLOW:-}" ] && grep -qw "$n" <<< "$SLOW"; then echo "SKIP $n (slow)"; continue; fi
  check_one "$n" > "$WORK/$n.result" &
  while [ "$(jobs -r | wc -l)" -ge "$JOBS" ]; do sleep 0.2; done
done
wait
cat "$WORK"/*.result | sort -k2 > "$WORK/summary.txt"

# Coverage: every fixture golden dump must be referenced by an op script.
if [ $# -eq 0 ]; then
  referenced="$(cat "$ROOT"/tests/fixtures/*.ops | awk '$1=="output"||$1=="output-unchecked"||($1=="delete"&&$3!="-"){print $3}' | sort -u)"
  for f in "$ROOT"/tests/golden/*.txt; do
    g="$(basename "$f" .txt)"
    case "$g" in *_oracle|*_oracle_*) continue;; esac  # module oracles, not fixtures
    grep -qx "$g" <<< "$referenced" || echo "FAIL coverage: golden $g.txt is not referenced by any op script" >> "$WORK/summary.txt"
  done
fi

cat "$WORK/summary.txt"
pass=$(grep -c '^PASS' "$WORK/summary.txt")
fail=$(grep -c '^FAIL' "$WORK/summary.txt")
smoke=$(grep -c '^SMOKE' "$WORK/summary.txt")
skip=$(grep -c '^SKIP' "$WORK/summary.txt")
echo "round trip: $pass passed, $fail failed, $smoke smoke (compiled+ran, no golden), $skip skipped"
[ "$fail" -eq 0 ]
