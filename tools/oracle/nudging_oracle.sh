#!/usr/bin/env bash
# Builds and runs the nudging differential oracle (tools/oracle/nudging_oracle.cpp).
#
#   tools/oracle/nudging_oracle.sh [NUM_SCENES]
#
# Copies the vendored libavoid sources into target/nudging_oracle/, patches
# the *copy* of orthogonal.cpp to call nudgeOracleTrace() (region segment
# lists and VPSC results of ImproveOrthogonalRoutes::nudgeOrthogonalRoutes),
# links tools/oracle/bump_alloc.cpp (pointer order == allocation order) and
# writes tests/golden/nudging_oracle.txt.  The vendored sources are never
# modified.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
VENDOR="$ROOT/vendor/adaptagrams/cola"
BUILD="$ROOT/target/nudging_oracle"
mkdir -p "$BUILD/libavoid"
cp -u "$VENDOR"/libavoid/*.cpp "$VENDOR"/libavoid/*.h "$BUILD/libavoid/"
python3 - "$VENDOR/libavoid/orthogonal.cpp" "$BUILD/libavoid/orthogonal.cpp" <<'PY'
import sys
src = open(sys.argv[1]).read()
decl = "namespace Avoid {\n"
i = src.index(decl)
src = src[:i] + decl + """
void nudgeOracleTrace(const std::string& s);
std::string nudgeOracleG17(double v);
""" + src[i + len(decl):]

region = """            currentRegion = linesort(nudgeFinalSegments, currentRegion,
                    lineSortComp);
        }
"""
assert src.count(region) == 1
src = src.replace(region, region + """
        {
            size_t oAltDim = (dimension + 1) % 2;
            nudgeOracleTrace("R d=" + std::to_string(dimension) + " u=" +
                    std::to_string((int) justUnifying) + " n=" +
                    std::to_string(currentRegion.size()));
            for (ShiftSegmentList::iterator oIt = currentRegion.begin();
                    oIt != currentRegion.end(); ++oIt)
            {
                NudgingShiftSegment *g = static_cast<NudgingShiftSegment *> (*oIt);
                std::string t = "S c=" + std::to_string(g->connRef->id()) + " i=";
                for (size_t k = 0; k < g->indexes.size(); ++k)
                {
                    t += (k ? "," : "") + std::to_string(g->indexes[k]);
                }
                t += " p=" + nudgeOracleG17(g->lowPoint()[dimension]) +
                        " a=" + nudgeOracleG17(g->lowPoint()[oAltDim]) + "," +
                        nudgeOracleG17(g->highPoint()[oAltDim]) +
                        " lim=" + nudgeOracleG17(g->minSpaceLimit) + "," +
                        nudgeOracleG17(g->maxSpaceLimit) + " f=" +
                        std::to_string((int) g->fixed) +
                        std::to_string((int) g->finalSegment) +
                        std::to_string((int) g->endsInShape) +
                        std::to_string((int) g->singleConnectedSegment) +
                        std::to_string((int) g->sBend) +
                        std::to_string((int) g->zBend) +
                        " cp=" + std::to_string(g->checkpoints.size());
                nudgeOracleTrace(t);
            }
        }
""")

solved = """        while (!satisfied && (sepDist > 0.0001));
"""
assert src.count(solved) == 1
src = src.replace(solved, solved + """
        {
            nudgeOracleTrace("X sat=" + std::to_string((int) satisfied) + " sep=" +
                    nudgeOracleG17(sepDist) + " nv=" + std::to_string(vs.size()) +
                    " nc=" + std::to_string(cs.size()));
            for (size_t k = 0; k < vs.size(); ++k)
            {
                nudgeOracleTrace("v " + std::to_string(vs[k]->id) + " " +
                        nudgeOracleG17(vs[k]->desiredPosition) + " " +
                        nudgeOracleG17(vs[k]->finalPosition) + " " +
                        nudgeOracleG17(vs[k]->weight));
            }
            for (size_t k = 0; k < cs.size(); ++k)
            {
                size_t l = 0, r = 0;
                for (size_t q = 0; q < vs.size(); ++q)
                {
                    if (vs[q] == cs[k]->left) l = q;
                    if (vs[q] == cs[k]->right) r = q;
                }
                nudgeOracleTrace("k " + std::to_string(l) + " " + std::to_string(r) +
                        " " + nudgeOracleG17(cs[k]->gap) + " " +
                        std::to_string((int) cs[k]->equality) + " " +
                        std::to_string((int) cs[k]->unsatisfiable));
            }
        }
""")
out = sys.argv[2]
try:
    if open(out).read() == src:
        sys.exit(0)
except FileNotFoundError:
    pass
open(out, "w").write(src)
PY
# Two builds: with assertions (as the fixture oracle) and with -DNDEBUG.
# The golden holds the NDEBUG output; scenes in which the asserting build
# fails an upstream COLA_ASSERT (or crashes in an assert-only check) are
# marked "#assert <message>" (the Rust test then checks them only in
# release builds, where debug_assert! is off like COLA_ASSERT under NDEBUG).
build_variant() {
  local dir="$1"; shift
  local flags="-std=c++11 -O2 -ffp-contract=off -fno-access-control -w -I$BUILD $*"
  mkdir -p "$BUILD/$dir"
  local objs=()
  for f in "$BUILD"/libavoid/*.cpp; do
    local o="$BUILD/$dir/$(basename "${f%.cpp}").o"
    objs+=("$o")
    if [ ! -f "$o" ] || [ "$f" -nt "$o" ]; then
      g++ $flags -c "$f" -o "$o" &
    fi
  done
  wait
  g++ $flags -c "$ROOT/tools/oracle/bump_alloc.cpp" -o "$BUILD/$dir/bump_alloc.o"
  g++ $flags "$ROOT/tools/oracle/nudging_oracle.cpp" "$BUILD/$dir/bump_alloc.o" "${objs[@]}" -o "$BUILD/$dir/nudging_oracle"
}
build_variant assert
build_variant ndebug -DNDEBUG
N="${1:-400}"
"$BUILD/assert/nudging_oracle" "$N" > "$BUILD/assert.txt" 2> "$BUILD/assert.stderr"
"$BUILD/ndebug/nudging_oracle" "$N" > "$BUILD/ndebug.txt" 2> "$BUILD/ndebug.stderr"
python3 - "$BUILD/assert.txt" "$BUILD/ndebug.txt" "$ROOT/tests/golden/nudging_oracle.txt" <<'PY'
import sys
def scenes(path):
    out, cur, name = {}, [], None
    order = []
    for line in open(path).read().split('\n'):
        if line.startswith('#scene '):
            name = line[7:]
            cur = [line]
            order.append(name)
        elif name is not None:
            cur.append(line)
            if line == '#end':
                out[name] = cur
                name = None
    return order, out
order, a = scenes(sys.argv[1])
order2, n = scenes(sys.argv[2])
assert order == order2
res = ['# nudging oracle (NDEBUG output; "#assert" = the asserting build fails)']
for name in order:
    sa, sn = a[name], n[name]
    i = sn.index('#out')
    assert sa[:sa.index('#out')] == sn[:i]
    body = sn[i + 1:-1]
    ia = sa.index('#out')
    abody = sa[ia + 1:-1]
    marker = []
    if abody and abody[0].startswith('#crash') and not (body and body[0].startswith('#crash')):
        marker = ['#assert' + abody[0][len('#crash'):]]
    elif not (body and body[0].startswith('#crash')):
        assert abody == body, name
    res += sn[:i + 1] + marker + body + ['#end']
open(sys.argv[3], 'w').write('\n'.join(res) + '\n')
PY
grep -c '^#scene' "$ROOT/tests/golden/nudging_oracle.txt" | sed 's/^/scenes: /'
grep -c '^#assert' "$ROOT/tests/golden/nudging_oracle.txt" | sed 's/^/assert-build failures: /' || true
grep -c '^#crash' "$ROOT/tests/golden/nudging_oracle.txt" | sed 's/^/crashed (NDEBUG): /' || true
