#!/usr/bin/env bash
# Builds the three variants of the end-to-end fuzzer driver
# (tools/oracle/e2e_fuzz.cpp) against a copy of the vendored libavoid:
#
#   target/e2e_fuzz/assert/e2e_fuzz   COLA_ASSERTs enabled
#   target/e2e_fuzz/ndebug/e2e_fuzz   -DNDEBUG (the reference output)
#   target/e2e_fuzz/asan/e2e_fuzz     -DNDEBUG + AddressSanitizer + UBSan
#                                     (bump allocator with redzones and
#                                     poisoning; see e2e_fuzz_alloc.cpp)
#
# All with -O2 -ffp-contract=off -fno-access-control.  The vendored sources
# are never modified.  Object files are cached (rebuilt when newer sources).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
VENDOR="${VENDOR:-$ROOT/vendor/adaptagrams/cola}"
BUILD="${E2E_BUILD:-$ROOT/target/e2e_fuzz}"
mkdir -p "$BUILD/libavoid"
cp -u "$VENDOR"/libavoid/*.cpp "$VENDOR"/libavoid/*.h "$BUILD/libavoid/"

build_variant() {
  local dir="$1"; shift
  local flags="-std=c++11 -O2 -g -ffp-contract=off -fno-access-control -w -I$BUILD $*"
  mkdir -p "$BUILD/$dir"
  local objs=()
  local pids=()
  for f in "$BUILD"/libavoid/*.cpp; do
    local o="$BUILD/$dir/$(basename "${f%.cpp}").o"
    objs+=("$o")
    if [ ! -f "$o" ] || [ "$f" -nt "$o" ]; then
      g++ $flags -c "$f" -o "$o" &
      pids+=($!)
    fi
  done
  for p in "${pids[@]}"; do wait "$p"; done
  g++ $flags -c "$ROOT/tools/oracle/e2e_fuzz_alloc.cpp" -o "$BUILD/$dir/e2e_fuzz_alloc.o"
  g++ $flags -c "$ROOT/tools/oracle/e2e_fuzz.cpp" -o "$BUILD/$dir/e2e_fuzz.o"
  g++ $flags "$BUILD/$dir/e2e_fuzz.o" "$BUILD/$dir/e2e_fuzz_alloc.o" "${objs[@]}" \
      -o "$BUILD/$dir/e2e_fuzz"
}

build_variant assert & p1=$!
build_variant ndebug -DNDEBUG & p2=$!
# -fno-sanitize=bool: upstream copies ActionInfo::firstMove, which is
# uninitialised for non-move actions (and never read for them).
build_variant asan -DNDEBUG -DE2E_SANITIZE -fsanitize=address,undefined \
    -fno-sanitize=bool -fno-sanitize-recover=all -fno-omit-frame-pointer & p3=$!
wait $p1; wait $p2; wait $p3
echo "built: $BUILD/{assert,ndebug,asan}/e2e_fuzz"
