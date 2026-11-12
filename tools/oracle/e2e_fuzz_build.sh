#!/usr/bin/env bash
# Builds the three variants of the end-to-end fuzzer driver
# (tools/oracle/e2e_fuzz.cpp) against a copy of the vendored libavoid:
#
#   target/e2e_fuzz/assert/e2e_fuzz   COLA_ASSERTs enabled
#   target/e2e_fuzz/ndebug/e2e_fuzz   -DNDEBUG (the reference output)
#   target/e2e_fuzz/asan/e2e_fuzz     -DNDEBUG + AddressSanitizer + UBSan
#                                     (on ASan's malloc; see
#                                     e2e_fuzz_alloc.cpp)
#
# All with -O2 -ffp-contract=off -fno-access-control.  The vendored sources
# are never modified.  Object files are cached (rebuilt when newer sources).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
VENDOR="${VENDOR:-$ROOT/vendor/adaptagrams/cola}"
BUILD="${E2E_BUILD:-$ROOT/target/e2e_fuzz}"
mkdir -p "$BUILD/libavoid"
cp -u "$VENDOR"/libavoid/*.cpp "$VENDOR"/libavoid/*.h "$BUILD/libavoid/"
# Patch 1 to the copy: count rerouteAndCallbackConnectors() calls
# (e2eRerouteCount, defined in e2e_fuzz.cpp; mirrored by the Rust test hook
# avoid::router::e2e_reroute_count()).
python3 - "$VENDOR/libavoid/router.cpp" "$BUILD/libavoid/router.cpp" <<'PY'
import sys
s = open(sys.argv[1]).read()
old = "void Router::rerouteAndCallbackConnectors(void)\n{\n"
assert s.count(old) == 1
s = s.replace(old, "extern unsigned long e2eRerouteCount;\n" + old + "    ++e2eRerouteCount;\n")
try:
    if open(sys.argv[2]).read() == s:
        sys.exit(0)
except FileNotFoundError:
    pass
open(sys.argv[2], "w").write(s)
PY
# Second patch, identical to tools/oracle/build_hyperedge_oracle.sh: the
# port's stale-pathNext rule.  AStarPathPrivate::search() follows
# start->pathNext, which can point at a vertex freed since (a use-after-free
# upstream); the port reads such a link as null (router.rs, "Stale
# pathNext").  The copy ignores the link unless it is still in the router's
# vertex list, so those scenes are compared instead of excluded.  Live
# links are untouched.
python3 - "$VENDOR/libavoid/makepath.cpp" "$BUILD/libavoid/makepath.cpp" <<'PY'
import sys
s = open(sys.argv[1]).read()
old = """        if (start->pathNext)
        {
"""
assert s.count(old) == 1
s = s.replace(old, """        // E2E ORACLE: stale pathNext reads as null (see e2e_fuzz_build.sh).
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
try:
    if open(sys.argv[2]).read() == s:
        sys.exit(0)
except FileNotFoundError:
    pass
open(sys.argv[2], "w").write(s)
PY
# Third patch (detection only, ASan build): HyperedgeImprover iterates its
# std::set of tree roots while the loop body erases elements.  Erasing the
# element the loop stands on (a connector with both ends on one junction
# gives that junction two tree nodes, so removeOtherJunctionsFrom() reaches
# the root's own junction) makes the following ++curr read a freed rb-tree
# node -- undefined behaviour that ASan misses (_Rb_tree_increment lives in
# the uninstrumented libstdc++).  The bump builds then resume at whatever
# the stale node links point to (e.g. revisit an earlier root); the port
# continues with the next root (router.rs, "Public-API misuse").  The ASan
# build reports such scenes, so they are classified "#exclude ub".
python3 - "$VENDOR/libavoid/hyperedgeimprover.cpp" "$BUILD/libavoid/hyperedgeimprover.cpp" <<'PY'
import sys
s = open(sys.argv[1]).read()
check = """#ifdef E2E_SANITIZE
        if (m_hyperedge_tree_roots.count(e2eCurr) == 0)
        {
            fprintf(stderr, "ERROR: iterator-erased (std::set element erased "
                    "while iterated) @ Avoid::HyperedgeImprover::%s\\n", E2E_FN);
            abort();
        }
#endif
"""
reps = [
    ("""        HyperedgeTreeNode *node = m_hyperedge_tree_junctions[*curr];
        bool containsCycle = node->removeOtherJunctionsFrom(nullptr,
                m_hyperedge_tree_roots);
""", "execute"),
    ("""        HyperedgeTreeNode *node = m_hyperedge_tree_junctions[*curr];

        removeZeroLengthEdges(node, nullptr);
""", "removeZeroLengthEdges"),
]
for old, fn in reps:
    assert s.count(old) == 1
    s = s.replace(old, "        JunctionRef *e2eCurr = *curr;\n" + old +
                  check.replace("E2E_FN", '"%s"' % fn))
s = "#include <cstdio>\n#include <cstdlib>\n" + s
try:
    if open(sys.argv[2]).read() == s:
        sys.exit(0)
except FileNotFoundError:
    pass
open(sys.argv[2], "w").write(s)
PY

build_variant() {
  local dir="$1"; shift
  local flags="-std=c++11 -O2 -g -ffp-contract=off -fno-access-control -w -I$BUILD $*"
  mkdir -p "$BUILD/$dir"
  local objs=()
  local running=0
  # At most 3 compilers at once (memory: shared workstation).
  for f in "$BUILD"/libavoid/*.cpp; do
    local o="$BUILD/$dir/$(basename "${f%.cpp}").o"
    objs+=("$o")
    if [ ! -f "$o" ] || [ "$f" -nt "$o" ]; then
      g++ $flags -c "$f" -o "$o" &
      running=$((running + 1))
      if [ "$running" -ge 3 ]; then wait -n; running=$((running - 1)); fi
    fi
  done
  wait
  g++ $flags -c "$ROOT/tools/oracle/e2e_fuzz_alloc.cpp" -o "$BUILD/$dir/e2e_fuzz_alloc.o"
  g++ $flags -c "$ROOT/tools/oracle/e2e_fuzz.cpp" -o "$BUILD/$dir/e2e_fuzz.o"
  g++ $flags "$BUILD/$dir/e2e_fuzz.o" "$BUILD/$dir/e2e_fuzz_alloc.o" "${objs[@]}" \
      -o "$BUILD/$dir/e2e_fuzz"
}

# Variants one after the other.
build_variant assert
build_variant ndebug -DNDEBUG
# The sanitizer build uses ASan's malloc (see e2e_fuzz_alloc.cpp).
# -fno-sanitize=bool: upstream copies ActionInfo::firstMove, which is
# uninitialised for non-move actions (and never read for them).
build_variant asan -DNDEBUG -DE2E_SANITIZE -fsanitize=address,undefined \
    -fno-sanitize=bool -fno-sanitize-recover=all -fno-omit-frame-pointer
echo "built: $BUILD/{assert,ndebug,asan}/e2e_fuzz"
