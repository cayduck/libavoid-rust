#!/usr/bin/env bash
# Comparison of libavoid-rust with libavoid-js (https://github.com/Aksem/libavoid-js)
# and with native C++ builds of upstream libavoid and of the Adaptagrams fork
# that libavoid-js compiles. See README.md in this directory.
#
#   tools/libavoid-js-compare/run.sh [setup|routes|features|misuse|bench|all]   (default all)
#
# Needs: node, npm, git, g++, and a built package (npm run build:node).
# Everything is downloaded and built under target/libavoid-js-compare/.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
WORK="${WORK:-$ROOT/target/libavoid-js-compare}"
export WORK
SCENES="${SCENES:-600}"           # random scenes for the route comparison
JOBS="${JOBS:-8}"
# The versions compared. LJS_NEXT is a commit of libavoid-js's unreleased
# 0.5.0 work (branch new-emsdk-and-embind), whose dist/ is committed.
LJS_NEXT="${LJS_NEXT:-df689b5}"
FORK_TAG="${FORK_TAG:-v1.0.9}"     # Aksem/adaptagrams, built by that commit
UPSTREAM="$ROOT/vendor/adaptagrams/cola/libavoid"
mkdir -p "$WORK"

build_cpp() { # <name> <libavoid source dir> ; builds $WORK/cpp/<name>/drv
  local d="$WORK/cpp/$1"; mkdir -p "$d/libavoid"
  cp "$2"/*.cpp "$2"/*.h "$d/libavoid/"
  local f; for f in "$d"/libavoid/*.cpp; do
    g++ -O2 -std=c++14 -ffp-contract=off -w -DNDEBUG -I"$d" -c "$f" -o "${f%.cpp}.o" &
  done; wait
  ar rcs "$d/libavoid.a" "$d"/libavoid/*.o
  g++ -O2 -std=c++14 -ffp-contract=off -w -I"$d" "$HERE/driver.cpp" "$d/libavoid.a" -o "$d/drv"
}

setup() {
  local v
  for v in 0.4.5 0.5.0-beta.5; do
    if [ ! -d "$WORK/npm-$v/node_modules/libavoid-js" ]; then
      mkdir -p "$WORK/npm-$v"
      (cd "$WORK/npm-$v" && npm init -y >/dev/null && npm install --no-save --ignore-scripts "libavoid-js@$v" >/dev/null)
    fi
  done
  [ -d "$WORK/libavoid-js" ] || git clone -q https://github.com/Aksem/libavoid-js.git "$WORK/libavoid-js"
  git -C "$WORK/libavoid-js" fetch -q origin
  git -C "$WORK/libavoid-js" checkout -q "$LJS_NEXT"
  [ -d "$WORK/adaptagrams-fork" ] || git clone -q https://github.com/Aksem/adaptagrams.git "$WORK/adaptagrams-fork"
  git -C "$WORK/adaptagrams-fork" checkout -q "$FORK_TAG"
  [ -x "$WORK/cpp/upstream/drv" ] || build_cpp upstream "$UPSTREAM"
  [ -x "$WORK/cpp/fork/drv" ] || build_cpp fork "$WORK/adaptagrams-fork/cola/libavoid"
}

run_one() { # <impl> <scene> <outdir>
  local impl=$1 f=$2 d=$3/$1 n rc=0; n=$(basename "$f" .txt); mkdir -p "$d"
  case $impl in
    upstream|fork) timeout 120 "$WORK/cpp/$impl/drv" "$f" > "$d/$n.out" 2> "$d/$n.log" || rc=$?;;
    *) OUT=$d timeout 120 node "$HERE/drive.mjs" "$impl" "$f" > /dev/null 2> "$d/$n.log" || rc=$?;;
  esac
  echo "exit $rc" >> "$d/$n.log"
}
export -f run_one; export HERE

routes() {
  local sc="$WORK/scenes" out="$WORK/routes"
  mkdir -p "$sc" "$out"
  node "$HERE/gen.mjs" "$sc" 1 "$SCENES"
  local impl
  for impl in upstream fork rust ljs045 ljsbeta5 ljsnext; do
    ls "$sc"/*.txt | xargs -P "$JOBS" -I{} bash -c "run_one $impl {} $out"
  done
  node "$HERE/analyze.mjs" "$out" upstream fork rust ljs045 ljsbeta5 ljsnext
  echo
  echo "Shapes nested in a parent shape (scenes/nested-*.txt):"
  for f in "$HERE"/scenes/nested-*.txt; do
    echo "  $(basename "$f" .txt)"
    echo "    upstream: $("$WORK/cpp/upstream/drv" "$f" 2>/dev/null | tail -n +2 | tr '\n' ' ')"
    for impl in rust ljsnext; do echo "    $impl: $(node "$HERE/drive.mjs" $impl "$f" 2>/dev/null | tail -n +2 | tr '\n' ' ')"; done
  done
}

misuse() {
  local k impl
  for k in movePointCount deleteInSameTransaction pinClassZero useAfterDelete nanPoint; do
    for impl in rust ljs045 ljsbeta5 ljsnext; do
      timeout 60 node "$HERE/misuse.mjs" $impl $k 2>/dev/null | grep -v "already initialized" || echo "$impl $k: process exited"
    done
  done
}

bench() {
  # The scenes of `routes`, minus those that abort libavoid-js (segmentPenalty 0),
  # one build and one scene at a time, nothing else running in parallel.
  local sc="$WORK/scenes" out="$WORK/times" impl f
  [ -d "$WORK/routes/upstream" ] || routes
  mkdir -p "$out"
  local list; list=$(grep -L "^param 0 0$" "$sc"/*.txt || true)
  for impl in upstream fork; do
    for f in $list; do echo "$(basename "$f" .txt) $("$WORK/cpp/$impl/drv" "$f" 2>&1 >/dev/null | awk '/time_ms/{print $2}')"; done > "$out/native-$impl.txt"
  done
  for impl in rust ljs045 ljsbeta5 ljsnext; do
    # shellcheck disable=SC2086
    node "$HERE/drive.mjs" $impl $list 2>&1 >/dev/null | awk '/ ok /{print $1, $3} / error /{print $1, "ERR"}' > "$out/$impl.txt"
  done
  node "$HERE/bench-summary.mjs" "$out" "$WORK/routes"
}

features() {
  local impl
  for impl in rust ljs045 ljsbeta5 ljsnext; do node "$HERE/features.mjs" $impl; done
}

case "${1:-all}" in
  setup) setup;;
  routes) setup; routes;;
  misuse) setup; misuse;;
  features) setup; features;;
  bench) setup; bench;;
  all) setup; routes; echo; features; echo; misuse; echo; bench;;
  *) echo "usage: $0 [setup|routes|features|misuse|bench|all]" >&2; exit 2;;
esac
