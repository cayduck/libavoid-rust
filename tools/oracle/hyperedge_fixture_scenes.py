#!/usr/bin/env python3
"""Translate the upstream hyperedge/junction fixtures into the scene DSL of
tools/oracle/hyperedge_oracle.cpp / tests/hyperedge_oracle.rs.

    python3 tools/oracle/hyperedge_fixture_scenes.py VENDOR_TESTS_DIR \
        tests/golden > tools/oracle/hyperedge_fixtures.scenes

(The golden directory is only used to see which fixtures have a
`NAME-final.txt` dump, i.e. reached `delete router`.)

Builds on tools/oracle/fixture_scenes.py (same statement forms) and adds
the forms used by these fixtures.  Every fixture becomes two scenes:

  fixture-NAME       each `processTransaction()` becomes `reroute 1`
                     (Router::rerouteAndCallbackConnectors() up to, but
                     excluding, orthogonal nudging) -- checked now;
  full-fixture-NAME  each `processTransaction()` becomes `transaction`
                     and each `outputDiagram("output/X")` becomes
                     `diagram X` (compared with tests/golden/X.txt by
                     tests/fixtures_hyperedge.rs once nudging is ported).

Statements that only read results (printing, asserts, return values) are
skipped; they do not change the router.
"""
import re
import sys
import os

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import fixture_scenes as fs  # noqa: E402

FIXTURES = [
    'hyperedge01', 'hyperedge02', 'hyperedgeLoop1', 'hyperedgeRerouting01',
    'improveHyperedge01', 'improveHyperedge02', 'improveHyperedge03',
    'improveHyperedge04', 'improveHyperedge05', 'improveHyperedge06',
    'junction02', 'junction03', '2junctions', 'removeJunctions01', 'tjunct',
]

SKIP = [
    r'ConnEnd \w+', r'Polygon \w+', r'HyperedgeNewAndDeletedObjectLists \w+',
    r'(?:JunctionRefList|ConnRefList)::const_iterator \w+',
    r'std::vector<Checkpoint> \w+', r'std::ostringstream \w+',
    r'static int counter = \d+', r'os << .*', r'router->outputDiagram\(os\.str\(\)\)',
    r'void outputDiagram\(Router \* router\)', r'(?:void|int) test\(\)',
    r'extern "C" int main\(.*\)', r'test\(\)', r'printf\(.*\)', r'COLA_ASSERT\(.*\)',
    r'\w+ = router->newAndDeletedObjectListsFromHyperedgeImprovement\(\)',
    r'(?:int|bool) \w+ = router->exists\w+\(.*\)', r'\w+\+\+',
    r'\w+ = \w+\.(?:newJunctionList|newConnectorList)\.begin\(\)',
r'\w+->(?:position|attachedConnectors|endpointConnEnds)\(\)',
    r'Point \w+ = \w+->recommendedPosition\(\)', r'return\b.*',
    r'HyperedgeRerouter \*\w+ = router->hyperedgeRerouter\(\)',
    r'router->setTransactionUse\(true\)', r'ConnEndList \w+',
    r'PolyLine \w+', r'\w+ \*\w+ = nullptr', r'std::pair<ConnEnd, ConnEnd> \w+ = .*',
]


class HTr(fs.Tr):
    def __init__(self, name):
        fs.Tr.__init__(self, name)
        self.cur_poly = None
        self.shape_poly = {}   # dsl shape name -> list of points
        self.lists = {}        # ConnEndList var -> [end tokens]
        self.diagram_counter = 0

    def poly_points(self, var):
        kind, data = self.polys[var]
        if kind == 'rect':
            x1, y1, x2, y2 = (float(v) for v in data)
            return [(x2, y1), (x2, y2), (x1, y2), (x1, y1)]
        return [(float(x), float(y)) for (x, y) in data]

    def stmt(self, s):
        s = ' '.join(s.split())
        s = re.sub(r'\b(\w+) \* (\w+) =', r'\1 *\2 =', s)
        s = re.sub(r'\brouter\d+\b', 'router', s)
        s = s.replace('(RoutingParameter)', '')
        for pat in SKIP:
            if re.fullmatch(pat, s):
                return
        m = re.fullmatch(r'(\w+) = (new ShapeConnectionPin\(.*\))', s)
        if m:
            s = 'ShapeConnectionPin *%s = %s' % (m.group(1), m.group(2))
        m = re.fullmatch(r'(?:ConnRef \*\w+ = )?(\w+)->removeJunctionAndMergeConnectors\(\)', s)
        if m:
            self.out.append('removejunc %s' % self.juncs[m.group(1)])
            return
        m = re.fullmatch(r'(JunctionRef|ConnRef) \*(\w+) = \*\w+', s)
        if m:
            # The first object of the improver's new-object list (the
            # fixtures only take begin() of these lists).
            if m.group(1) == 'JunctionRef':
                name = 'j%d' % (len(self.juncs) + 1)
                self.juncs[m.group(2)] = name
                self.out.append('bindnewj %s' % name)
            else:
                name = 'c%d' % (len(self.conns) + 1)
                self.conns[m.group(2)] = name
                self.out.append('bindnewc %s' % name)
            return
        m = re.fullmatch(r'(\w+) = Polygon\((\d+)\)', s)
        if m:
            self.polys[m.group(1)] = ('poly', [None] * int(m.group(2)))
            return
        m = re.fullmatch(r'(\w+)\.setPoint\((\d+), (Point\(.*\))\)', s)
        if m:
            self.polys[m.group(1)][1][int(m.group(2))] = self.point(m.group(3))
            return
        m = re.fullmatch(r'Router \*router = new Router\((.*)\)', s)
        if m:
            flags = m.group(1).replace('Avoid::', '')
            v = 0
            for f in flags.split('|'):
                v |= int(self.num(f.strip()))
            self.out.append('router %d' % v)
            return
        m = re.fullmatch(r'router->setRoutingPenalty\((\w+)\)', s)
        if m:
            # Default value: chooseSensibleParamValue (-1).
            self.out.append('param %d -1' % fs.PARAMS[m.group(1)])
            return
        m = re.fullmatch(r'router->setRoutingOption\(\(RoutingOption\)(\d+), (\w+)\)', s)
        if m:
            self.out.append('option %s %s' % (m.group(1), self.num(m.group(2))))
            return
        m = re.fullmatch(r'(\w+)->setPositionFixed\((\w+)\)', s)
        if m and m.group(1) in self.juncs:
            self.out.append('fixj %s %s' % (self.juncs[m.group(1)], self.num(m.group(2))))
            return
        m = re.fullmatch(r'(\w+)->makePathInvalid\(\)', s)
        if m:
            self.out.append('invalid %s' % self.conns[m.group(1)])
            return
        m = re.fullmatch(r'router->moveShape\((\w+), (\w+)\)', s)
        if m:
            name = self.shapes[m.group(1)]
            self.out.append('moveto %s %d %s' % (name, len(self.poly_points(m.group(2))),
                ' '.join('%s %s' % (repr(x), repr(y)) for x, y in self.poly_points(m.group(2)))))
            return
        m = re.fullmatch(r'router->delete(Shape|Junction|Connector)\((\w+)\)', s)
        if m:
            kind, var = m.group(1), m.group(2)
            if kind == 'Shape':
                self.out.append('delshape %s' % self.shapes[var])
            elif kind == 'Junction':
                self.out.append('deljunc %s' % self.juncs[var])
            else:
                self.out.append('delconn %s' % self.conns[var])
            return
        m = re.fullmatch(r'(\w+) = nullptr', s)
        if m:
            return
        m = re.fullmatch(r'(\w+)\.push_back\((ConnEnd\(.*\))\)', s)
        if m and m.group(1) not in self.cpvecs:
            self.lists.setdefault(m.group(1), []).append(self.end_ref(m.group(2)))
            return
        m = re.fullmatch(r"(?:\w+|router->hyperedgeRerouter\(\))->registerHyperedgeForRerouting\((\w+)\)", s)
        if m:
            v = m.group(1)
            if v in self.juncs:
                self.out.append('registerj %s' % self.juncs[v])
            else:
                ends = self.lists[v]
                self.out.append('registerends %d %s' % (len(ends), ' '.join(ends)))
            return
        m = re.fullmatch(r'router->outputDiagram\("output/(.*)"\)', s)
        if m:
            self.out.append('diagram %s' % m.group(1))
            return
        m = re.fullmatch(r'outputDiagram\(router\)', s)
        if m and not self.helper_outputs:
            return
        if m:
            self.diagram_counter += 1
            self.out.append('diagram %s-%d' % (self.name, self.diagram_counter))
            return
        m = re.fullmatch(r'(\w+)->setRoutingCheckpoints\((\w+)\)', s)
        if m and m.group(1) in self.conns and m.group(2) in self.cpvecs:
            cps = self.cpvecs[m.group(2)]
            self.out.append('checkpoints %s %d %s' % (
                self.conns[m.group(1)], len(cps), ' '.join(cps)))
            return
        m = re.fullmatch(r'(\w+)\.push_back\(Checkpoint\((.*)\)\)', s)
        if m and m.group(1) in self.cpvecs:
            a = fs.split_args(m.group(2))
            x, y = self.point(a[0])
            ad = self.num(a[1]) if len(a) > 1 else '15'
            dd = self.num(a[2]) if len(a) > 2 else '15'
            self.cpvecs[m.group(1)].append('%s %s %s %s' % (x, y, ad, dd))
            return
        if s.startswith('router->outputDiagram'):
            raise SystemExit('%s: unsupported %r' % (self.name, s))
        fs.Tr.stmt(self, s)


def translate(path, name):
    src = open(path).read()
    src = re.sub(r'/\*.*?\*/', '', src, flags=re.S)
    src = re.sub(r'//[^\n]*', '', src)
    # Keep the `#if 1` branch of `#if 1 ... #else ... #endif`.
    src = re.sub(r'#else\b.*?#endif', '', src, flags=re.S)
    src = re.sub(r'#[^\n]*', '', src)
    src = src.replace('Avoid::', '')
    # Drop the bodies of the helper outputDiagram(Router *) functions.
    src = re.sub(r'void outputDiagram\(Router \* router\)\s*\{.*?\n\}', '', src, flags=re.S)
    tr = HTr(name)
    # Does the fixture's outputDiagram(Router *) helper really write a
    # diagram (it is commented out in some fixtures)?
    tr.helper_outputs = re.search(
        r'\n\s*router->outputDiagram\(os\.str\(\)\)', open(path).read()) is not None
    for st in src.split(';'):
        st = st.strip()
        while st.startswith('}'):
            st = st[1:].strip()
        st = re.sub(r'^(?:extern "C" )?int main\s*\(.*?\)\s*\{', '', st).strip()
        st = re.sub(r'^(?:void|int) test\s*\(\s*\)\s*\{', '', st).strip()
        st = st.lstrip('{').strip()
        while st.endswith('}'):
            st = st[:-1].strip()
        while st.startswith('}'):
            st = st[1:].strip()
        if st.startswith('for (') or st.startswith('if ('):
            raise SystemExit('%s: control flow %r' % (name, st))
        tr.stmt(st)
    return tr.out


def main():
    tests = sys.argv[1]
    golden = sys.argv[2] if len(sys.argv) > 2 else None
    for name in FIXTURES:
        lines = translate('%s/%s.cpp' % (tests, name), name)
        # `delete router`: the oracle's patched destructor writes NAME-final.
        if golden and os.path.exists('%s/%s-final.txt' % (golden, name)):
            lines.append('diagram %s-final' % name)
        print('#scene fixture-%s' % name)
        for l in lines:
            if l == 'process':
                print('reroute 1')
            elif l.startswith('diagram '):
                continue
            else:
                print(l)
        print('#endscene')
        print('#scene full-fixture-%s' % name)
        for l in lines:
            print('transaction' if l == 'process' else l)
        print('#endscene')


if __name__ == '__main__':
    main()
