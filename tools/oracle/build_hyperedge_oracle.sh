#!/usr/bin/env bash
# Builds tools/oracle/hyperedge_oracle.cpp against a copy of the vendored
# libavoid sources and regenerates tests/golden/hyperedge_oracle.txt.
#
#   tools/oracle/build_hyperedge_oracle.sh [--no-run]
#
# The vendored sources are never modified.  In the copy, one assertion is
# removed: the COLA_ASSERT after `connectsWithoutBend(u, v)` in
# MinimumTerminalSpanningTree::constructSequential() (mtst.cpp), which does
# not hold whenever u is a terminal (sptfDist == 0; connectsWithoutBend then
# looks at hyperedge segments instead of pathNext).  constructSequential()
# is dead code upstream (HyperedgeRerouter uses constructInterleaved()); the
# oracle still exercises it and the Rust port does not port that assert.
# Also removed: COLA_ASSERT(currVert != nullptr) at the top of
# buildHyperedgeTreeToRoot(), which fails whenever a bridging edge ends at an
# original terminal (pathNext == nullptr); the code below it handles that
# case (the loop does not run), and the Rust port omits the assert too.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
VENDOR="${VENDOR:-$ROOT/vendor/adaptagrams/cola}"
BUILD="${ORACLE_BUILD_DIR:-$ROOT/target/hyperedge-oracle}"
mkdir -p "$BUILD/libavoid"
cp "$VENDOR"/libavoid/*.cpp "$VENDOR"/libavoid/*.h "$BUILD/libavoid/"
python3 - "$BUILD/libavoid/mtst.cpp" <<'PY'
import sys
p = sys.argv[1]
s = open(p).read()
old = """            COLA_ASSERT(!freeConnection == (u->pathNext && ! colinear(u->pathNext->point,
                    u->point, v->point)));
"""
assert s.count(old) == 1
s = s.replace(old, "")
old2 = """    COLA_ASSERT(currVert != nullptr);

    // This method follows branches in a shortest path tree back to the
    // root, generating hyperedge tree nodes and branches as it goes.
    while (currVert)
    {
        // Add the node, if necessary."""
assert s.count(old2) == 1
s = s.replace(old2, old2.replace("    COLA_ASSERT(currVert != nullptr);\n", ""))
open(p, "w").write(s)
PY
g++ -std=c++11 -O2 -ffp-contract=off -fno-access-control -w -I"$BUILD" \
    "$ROOT/tools/oracle/hyperedge_oracle.cpp" "$BUILD"/libavoid/*.cpp \
    -o "$BUILD/hyperedge_oracle"
if [ "${1:-}" != "--no-run" ]; then
  "$BUILD/hyperedge_oracle" > "$ROOT/tests/golden/hyperedge_oracle.txt"
fi
