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
#
# Detection added in the copy: in AStarPathPrivate::search() (makepath.cpp),
# `if (start->pathNext)` dereferences the source vertex's pathNext, which can
# be stale -- left by an earlier MTST or A* search -- and point at a vertex
# deleted since (an orthogonal visibility graph vertex freed by graph
# regeneration, a deleted connector's vertex, ...).  That is a use-after-free
# upstream.  The Rust port defines it (router.rs module docs, "Stale
# pathNext"): a pathNext link to a deleted vertex reads as null.  The copy
# applies exactly that rule -- it tests whether start->pathNext is still in
# the router's vertex list (every deleted vertex, including the MTST's own
# dummy vertices, has been removed from it) and ignores the link if not --
# so those scenes are compared too instead of being dropped.  Live links
# are untouched, i.e. upstream behaviour is unchanged wherever it is
# defined.
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
python3 - "$BUILD/libavoid/makepath.cpp" <<'PY'
import sys
p = sys.argv[1]
s = open(p).read()
old = """        if (start->pathNext)
        {
"""
assert s.count(old) == 1
s = s.replace(old, """        // ORACLE: the port's stale-pathNext rule (see
        // build_hyperedge_oracle.sh): a link to a deleted vertex (no longer
        // in the vertex list; the bump allocator never reuses addresses)
        // reads as null.  Nothing is dereferenced before the check.
        bool oracleLive = false;
        if (start->pathNext)
        {
            for (VertInf *k = lineRef->router()->vertices.connsBegin();
                    k; k = k->lstNext)
            {
                if (k == start->pathNext) oracleLive = true;
            }
        }
        if (oracleLive)
        {
""")
open(p, "w").write(s)
PY
g++ -std=c++11 -O2 -ffp-contract=off -fno-access-control -w -I"$BUILD" \
    "$ROOT/tools/oracle/hyperedge_oracle.cpp" "$BUILD"/libavoid/*.cpp \
    -o "$BUILD/hyperedge_oracle"
if [ "${1:-}" != "--no-run" ]; then
  python3 "$ROOT/tools/oracle/hyperedge_fixture_scenes.py" "$VENDOR/libavoid/tests" \
      "$ROOT/tests/golden" > "$ROOT/tools/oracle/hyperedge_fixtures.scenes"
  "$BUILD/hyperedge_oracle" "$ROOT/tools/oracle/hyperedge_fixtures.scenes" \
      > "$ROOT/tests/golden/hyperedge_oracle.txt"
fi
