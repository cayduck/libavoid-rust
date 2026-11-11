#!/usr/bin/env bash
# Builds the vendored upstream C++ libavoid as a reference oracle and runs every
# upstream test fixture, capturing full-precision route output as golden files.
#
#   tools/oracle/build.sh [fixture-name ...]
#
# Output: tests/golden/<fixture>.txt   (full precision outputDiagramText dump)
#         tests/golden/<fixture>-final.txt (same dump taken in Router::~Router)
#         tests/golden/<fixture>.exit  (process exit code; 0 == upstream test passed)
#         tests/golden/<fixture>.stdout
# The vendored sources are never modified: they are copied into a build
# directory and only the copy of router.cpp is patched (%g -> %.17g, plus
# junction positions).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SRC="$ROOT/vendor/adaptagrams/cola/libavoid"
BUILD="${ORACLE_BUILD_DIR:-$ROOT/target/oracle}"
GOLDEN="$ROOT/tests/golden"
TIMEOUT="${ORACLE_TIMEOUT:-600}"
mkdir -p "$BUILD/libavoid" "$BUILD/bin" "$BUILD/run/output" "$GOLDEN"

cp -u "$SRC"/*.cpp "$SRC"/*.h "$BUILD/libavoid/"
python3 - "$SRC/router.cpp" "$BUILD/libavoid/router.cpp" <<'PY'
import sys
src = open(sys.argv[1]).read()
start = src.index("void Router::outputDiagramText(std::string instanceName)")
end = src.index("HyperedgeNewAndDeletedObjectLists", start)
body = src[start:end]
assert body.count("%g") == 6, body.count("%g")
body = body.replace("%g", "%.17g")
marker = "    ConnRefList::iterator connRefIt = connRefs.begin();"
assert marker in body
junctions = r'''
    for (ObstacleList::iterator jIt = m_obstacles.begin();
            jIt != m_obstacles.end(); ++jIt)
    {
        JunctionRef *junction = dynamic_cast<JunctionRef *> (*jIt);
        if (junction)
        {
            fprintf(fp, "junction\n");
            fprintf(fp, "id=%u\n", junction->id());
            fprintf(fp, "x=%.17g\n", junction->position().x);
            fprintf(fp, "y=%.17g\n", junction->position().y);
            fprintf(fp, "\n");
        }
    }

'''
body = body.replace(marker, junctions + marker)
out = src[:start] + body + src[end:]
# Dump the final router state from the destructor so that fixtures which never
# call outputDiagram() still produce a golden file (<fixture>-final.txt).
dtor = "Router::~Router()\n{\n"
assert out.count(dtor) == 1
out = out.replace(dtor, dtor + """    if (getenv("ORACLE_FINAL"))
    {
        static int oracleRouterCount = 0;
        char oracleName[1024];
        if (oracleRouterCount == 0)
            snprintf(oracleName, sizeof(oracleName), "%s-final", getenv("ORACLE_FINAL"));
        else
            snprintf(oracleName, sizeof(oracleName), "%s-final%d", getenv("ORACLE_FINAL"), oracleRouterCount + 1);
        ++oracleRouterCount;
        outputDiagramText(oracleName);
    }
""")
try:
    if open(sys.argv[2]).read() == out:
        sys.exit(0)
except FileNotFoundError:
    pass
open(sys.argv[2], "w").write(out)
PY

CXXFLAGS="-O2 -std=c++11 -w -I$BUILD"
objs=()
for f in "$BUILD"/libavoid/*.cpp; do
  o="${f%.cpp}.o"
  objs+=("$o")
  if [ ! -f "$o" ] || [ "$f" -nt "$o" ]; then
    g++ $CXXFLAGS -c "$f" -o "$o" &
  fi
done
wait
ar rcs "$BUILD/libavoid.a" "${objs[@]}"

if [ $# -gt 0 ]; then names=("$@"); else
  names=(); for t in "$SRC"/tests/*.cpp; do names+=("$(basename "$t" .cpp)"); done
fi

run_one() {
  local n="$1"
  g++ $CXXFLAGS "$SRC/tests/$n.cpp" "$BUILD/libavoid.a" -o "$BUILD/bin/$n" || { echo "BUILD-FAIL $n"; return; }
  local code=0
  (cd "$BUILD/run" && rm -f "output/$n.txt" && ORACLE_FINAL="output/$n" timeout "$TIMEOUT" "$BUILD/bin/$n" > "$BUILD/run/$n.stdout" 2>&1) || code=$?
  echo "$code" > "$GOLDEN/$n.exit"
  cp "$BUILD/run/$n.stdout" "$GOLDEN/$n.stdout"
  # endlessLoop01 writes "1.txt" into the working directory.
  if [ "$n" = endlessLoop01 ] && [ -f "$BUILD/run/1.txt" ]; then cp "$BUILD/run/1.txt" "$GOLDEN/endlessLoop01-1.txt"; fi
  # Fixtures name their output file freely; collect whatever new .txt appeared.
  echo "$n exit=$code"
}
for n in "${names[@]}"; do run_one "$n" & 
  while [ "$(jobs -r | wc -l)" -ge 10 ]; do sleep 0.2; done
done
wait
cp "$BUILD"/run/output/*.txt "$GOLDEN/" 2>/dev/null || true
