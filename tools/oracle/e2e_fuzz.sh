#!/usr/bin/env bash
# End-to-end differential fuzzer: random scenes mixing every libavoid
# feature over long transaction sequences, run on the vendored C++ libavoid
# (asserting, -DNDEBUG and ASan+UBSan builds) and replayed through the Rust
# port (tests/e2e_fuzz.rs).  See tools/oracle/e2e_fuzz.cpp (generator, DSL,
# dumps) and tools/oracle/e2e_fuzz.py (classification, reducer).
#
#   tools/oracle/e2e_fuzz.sh --seed S --scenes N [--first F] [--batch B]
#       fuzz campaign: generate scenes F..F+N-1 of seed S, classify them,
#       replay them through the Rust port (release build); prints counts,
#       exclusion reasons and divergent scenes (saved under
#       target/e2e_fuzz/campaign-S/divergent-*.txt)
#   tools/oracle/e2e_fuzz.sh --golden
#       regenerate the committed golden tests/golden/e2e_fuzz.txt
#       (seed 1, scenes 0..E2E_GOLDEN_SCENES-1, default 1000)
#   tools/oracle/e2e_fuzz.sh --show NAME [--file GOLDEN]
#       full C++ (3 builds) and Rust dumps of one scene, plus a diff
#   tools/oracle/e2e_fuzz.sh --reduce NAME [--file GOLDEN] [--crash SIG]
#       shrink a divergent scene (or one whose ASan build reports SIG)
#
# Environment: E2E_JOBS (parallel C++ processes, default nproc),
# VENDOR (vendored adaptagrams/cola), E2E_BUILD (build dir).
#
# Every scene is classified (see e2e_fuzz.py): compared (all builds agree),
# #assert (only the asserting build fails; NDEBUG output compared in
# release), or #exclude crash/ub/heapdep/assertdiff/pinset-unsorted
# (upstream crashes, has undefined behaviour, or depends on heap layout).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PY="$ROOT/tools/oracle/e2e_fuzz.py"
SEED="" SCENES="" FIRST=0 BATCH=500 MODE="" NAME="" FILE="$ROOT/tests/golden/e2e_fuzz.txt" CRASH=""
while [ $# -gt 0 ]; do
  case "$1" in
    --seed) SEED="$2"; shift 2 ;;
    --scenes) SCENES="$2"; MODE=campaign; shift 2 ;;
    --first) FIRST="$2"; shift 2 ;;
    --batch) BATCH="$2"; shift 2 ;;
    --golden) MODE=golden; shift ;;
    --show) MODE=show; NAME="$2"; shift 2 ;;
    --reduce) MODE=reduce; NAME="$2"; shift 2 ;;
    --file) FILE="$2"; shift 2 ;;
    --crash) CRASH="$2"; shift 2 ;;
    *) echo "unknown argument $1" >&2; exit 2 ;;
  esac
done
python3 "$PY" build >/dev/null
case "$MODE" in
  campaign)
    python3 "$PY" campaign --seed "${SEED:?--seed}" --scenes "$SCENES" --first "$FIRST" --batch "$BATCH" ;;
  golden)
    N="${E2E_GOLDEN_SCENES:-1000}"
    python3 "$PY" gen --seed 1 --count "$N" --out "$ROOT/tests/golden/e2e_fuzz.txt"
    python3 "$PY" compare --golden "$ROOT/tests/golden/e2e_fuzz.txt" ;;
  show)
    python3 "$PY" show --golden "$FILE" --scene "$NAME" ;;
  reduce)
    if [ -n "$CRASH" ]; then
      python3 "$PY" reduce --golden "$FILE" --scene "$NAME" --crash "$CRASH"
    else
      python3 "$PY" reduce --golden "$FILE" --scene "$NAME"
    fi ;;
  *) sed -n '2,30p' "$0"; exit 2 ;;
esac
