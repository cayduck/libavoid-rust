#!/usr/bin/env python3
"""Translate upstream libavoid test fixtures into the scene DSL used by
tools/oracle/orthogonal_vis_oracle.cpp and tests/orthogonal_vis_oracle.rs.

    python3 tools/oracle/fixture_scenes.py VENDOR_TESTS_DIR NAME... \
        > tools/oracle/orthogonal_vis_fixtures.scenes

Only the statement forms that occur in the translated fixtures are
supported; anything else aborts the translation.  Every
`router->processTransaction()` becomes a `process` command (which, in the
oracle, runs processActions() and regenerates the orthogonal visibility
graph without routing).  Routers are always created orthogonal-only (the
orthogonal visibility graph does not depend on PolyLineRouting); the
original flags are kept in a comment.  Numeric literals are copied
verbatim; computed values are written with repr() (shortest round-trip).
"""
import re
import sys

CONSTS = {
    'ATTACH_POS_TOP': 0.0, 'ATTACH_POS_CENTRE': 0.5, 'ATTACH_POS_BOTTOM': 1.0,
    'ATTACH_POS_LEFT': 0.0, 'ATTACH_POS_RIGHT': 1.0,
    'ATTACH_POS_MIN_OFFSET': 0.0,
    'CONNECTIONPIN_UNSET': 2147483647, 'CONNECTIONPIN_CENTRE': 2147483646,
    'ConnDirNone': 0, 'ConnDirUp': 1, 'ConnDirDown': 2, 'ConnDirLeft': 4,
    'ConnDirRight': 8, 'ConnDirAll': 15,
    'true': 1, 'false': 0,
    'OrthogonalRouting': 2, 'PolyLineRouting': 1,
}
PARAMS = {
    'segmentPenalty': 0, 'anglePenalty': 1, 'crossingPenalty': 2,
    'clusterCrossingPenalty': 3, 'fixedSharedPathPenalty': 4,
    'portDirectionPenalty': 5, 'shapeBufferDistance': 6,
    'idealNudgingDistance': 7, 'reverseDirectionPenalty': 8,
}
OPTIONS = {
    'nudgeOrthogonalSegmentsConnectedToShapes': 0,
    'improveHyperedgeRoutesMovingJunctions': 1,
    'penaliseOrthogonalSharedPathsAtConnEnds': 2,
    'nudgeOrthogonalTouchingColinearSegments': 3,
    'performUnifyingNudgingPreprocessingStep': 4,
    'improveHyperedgeRoutesMovingAddingAndDeletingJunctions': 5,
    'nudgeSharedPathsWithCommonEndPoint': 6,
}

NUM = r'[-+]?(?:\d+\.?\d*(?:[eE][-+]?\d+)?|\.\d+(?:[eE][-+]?\d+)?)'


class Tr:
    def __init__(self, name):
        self.name = name
        self.out = []
        self.consts = dict(CONSTS)
        self.polys = {}      # var -> ('poly', [pts]) | ('rect', (x1,y1,x2,y2))
        self.shapes = {}     # var -> dsl name
        self.juncs = {}
        self.conns = {}
        self.pins = {}
        self.ends = {}       # var -> end token string
        self.cpvecs = {}
        self.anon = 0

    def fresh(self, prefix):
        self.anon += 1
        return '%s%d' % (prefix, self.anon)

    def num(self, expr):
        """Return the DSL text for a numeric C++ expression."""
        e = expr.strip()
        e = re.sub(r'^\((?:ConnDirFlags|PenaltyType|ConnType|unsigned int|double|int)\)\s*', '', e)
        e = e.strip()
        if re.fullmatch(NUM, e):
            return e
        if e in self.consts:
            v = self.consts[e]
            return repr(v) if isinstance(v, float) else str(v)
        if '/' in e:
            raise SystemExit('%s: division in expression %r' % (self.name, e))
        if not re.fullmatch(r'[\w\s.+\-*|()]+', e):
            raise SystemExit('%s: bad expression %r' % (self.name, e))
        v = eval(e, {'__builtins__': {}}, self.consts)
        return repr(v) if isinstance(v, float) else str(int(v))

    def point(self, s):
        m = re.fullmatch(r'\s*Point\((.*)\)\s*', s)
        if not m:
            raise SystemExit('%s: not a point %r' % (self.name, s))
        a = split_args(m.group(1))
        return self.num(a[0]), self.num(a[1])

    def end(self, args):
        """ConnEnd constructor arguments -> DSL end tokens."""
        if args[0].strip().startswith('Point('):
            x, y = self.point(args[0])
            dirs = self.num(args[1]) if len(args) > 1 else '15'
            return 'P %s %s %s' % (x, y, dirs)
        obj = args[0].strip()
        if obj in self.shapes:
            return 'S %s %s' % (self.shapes[obj], self.num(args[1]))
        if obj in self.juncs:
            assert len(args) == 1
            return 'J %s' % self.juncs[obj]
        raise SystemExit('%s: bad ConnEnd %r' % (self.name, args))

    def end_ref(self, s):
        s = s.strip()
        if s in self.ends:
            return self.ends[s]
        m = re.fullmatch(r'ConnEnd\((.*)\)', s)
        if m:
            return self.end(split_args(m.group(1)))
        raise SystemExit('%s: unknown end %r' % (self.name, s))

    def stmt(self, s):
        s = ' '.join(s.split())
        if not s or s in ('{', '}'):
            return
        if s.startswith('#') or s.startswith('using ') or s.startswith('return'):
            return
        if re.fullmatch(r'int main\s*\(\s*(void)?\s*\)\s*\{?', s):
            return
        if s == '}' or s.startswith('delete router') or 'outputDiagram' in s:
            return
        if s.startswith('assert(') or s.startswith('bool ') or s.startswith('printf'):
            return
        m = re.fullmatch(r'(?:Router \*router = )?new Router\((.*)\)', s)
        if m:
            self.out.append('# original router flags: %s' % m.group(1))
            self.out.append('router 2')
            return
        m = re.fullmatch(r'const (?:unsigned )?int (\w+) = (.*)', s)
        if m:
            self.consts[m.group(1)] = int(self.num(m.group(2)))
            return
        m = re.fullmatch(r'router->setRouting(Penalty|Parameter)\((.*)\)', s)
        if m:
            a = split_args(m.group(2))
            k = a[0].strip()
            k = PARAMS[k] if k in PARAMS else int(self.num(k))
            v = self.num(a[1]) if len(a) > 1 else None
            if v is None:
                raise SystemExit('%s: default penalty value' % self.name)
            self.out.append('param %d %s' % (k, v))
            return
        m = re.fullmatch(r'router->setRoutingOption\((\w+), (\w+)\)', s)
        if m:
            self.out.append('option %d %s' % (OPTIONS[m.group(1)], self.num(m.group(2))))
            return
        m = re.fullmatch(r'Polygon (\w+)\((\d+)\)', s)
        if m:
            self.polys[m.group(1)] = ('poly', [None] * int(m.group(2)))
            return
        m = re.fullmatch(r'(\w+)\.ps\[(\d+)\] = (Point\(.*\))', s)
        if m:
            self.polys[m.group(1)][1][int(m.group(2))] = self.point(m.group(3))
            return
        m = re.fullmatch(r'Rectangle (\w+)\((Point\(.*?\)), (Point\(.*\))\)', s)
        if m:
            self.polys[m.group(1)] = ('rect', self.point(m.group(2)) + self.point(m.group(3)))
            return
        m = re.fullmatch(r'(?:ShapeRef \*(\w+) = )?new ShapeRef\(router, (\w+)(?:, (.*))?\)', s)
        if m:
            var = m.group(1) or self.fresh('_s')
            name = 's%d' % (len(self.shapes) + 1)
            self.shapes[var] = name
            sid = self.num(m.group(3)) if m.group(3) else '0'
            kind, data = self.polys[m.group(2)]
            if kind == 'rect':
                self.out.append('rect %s %s %s' % (name, sid, ' '.join(data)))
            else:
                pts = ' '.join('%s %s' % p for p in data)
                self.out.append('shape %s %s %d %s' % (name, sid, len(data), pts))
            return
        m = re.fullmatch(r'(?:JunctionRef \*(\w+) = )?new JunctionRef\(router, (Point\(.*?\))(?:, (.*))?\)', s)
        if m:
            var = m.group(1) or self.fresh('_j')
            name = 'j%d' % (len(self.juncs) + 1)
            self.juncs[var] = name
            jid = self.num(m.group(3)) if m.group(3) else '0'
            x, y = self.point(m.group(2))
            self.out.append('junction %s %s %s %s' % (name, jid, x, y))
            return
        m = re.fullmatch(r'(?:ShapeConnectionPin \*(\w+) = )?new ShapeConnectionPin\((.*)\)', s)
        if m:
            var = m.group(1) or self.fresh('_p')
            name = 'p%d' % (len(self.pins) + 1)
            self.pins[var] = name
            a = split_args(m.group(2))
            obj = a[0].strip()
            if obj in self.shapes:
                if len(a) == 6:
                    a.append('ConnDirNone')
                assert len(a) == 7, a
                self.out.append('pin %s %s %s %s %s %s %s %s' % (
                    name, self.shapes[obj], self.num(a[1]), self.num(a[2]),
                    self.num(a[3]), self.num(a[4]), self.num(a[5]), self.num(a[6])))
            else:
                dirs = self.num(a[2]) if len(a) > 2 else '0'
                self.out.append('jpin %s %s %s %s' % (name, self.juncs[obj], self.num(a[1]), dirs))
            return
        m = re.fullmatch(r'(\w+)->setExclusive\((\w+)\)', s)
        if m and m.group(1) in self.pins:
            self.out.append('pinexcl %s %s' % (self.pins[m.group(1)], self.num(m.group(2))))
            return
        m = re.fullmatch(r'(?:ConnEnd )?(\w+)\((.*)\)', s)
        if m and (s.startswith('ConnEnd ')):
            self.ends[m.group(1)] = self.end(split_args(m.group(2)))
            return
        m = re.fullmatch(r'(\w+) = ConnEnd\((.*)\)', s)
        if m:
            self.ends[m.group(1)] = self.end(split_args(m.group(2)))
            return
        m = re.fullmatch(r'(?:ConnRef \*(\w+) = )?new ConnRef\(router(?:, (.*))?\)', s)
        if m:
            var = m.group(1) or self.fresh('_c')
            name = 'c%d' % (len(self.conns) + 1)
            self.conns[var] = name
            a = split_args(m.group(2)) if m.group(2) else []
            if len(a) <= 1:
                cid = self.num(a[0]) if a else '0'
                self.out.append('conn %s %s' % (name, cid))
            else:
                cid = self.num(a[2]) if len(a) > 2 else '0'
                self.out.append('connse %s %s %s %s' % (name, cid, self.end_ref(a[0]), self.end_ref(a[1])))
            return
        m = re.fullmatch(r'(\w+)->set(Source|Dest)Endpoint\((.*)\)', s)
        if m:
            self.out.append('%s %s %s' % ('src' if m.group(2) == 'Source' else 'dst',
                                          self.conns[m.group(1)], self.end_ref(m.group(3))))
            return
        m = re.fullmatch(r'(\w+)->setRoutingType\((.*)\)', s)
        if m:
            t = m.group(2).strip()
            t = {'ConnType_PolyLine': '1', 'ConnType_Orthogonal': '2'}.get(t) or self.num(t)
            self.out.append('type %s %s' % (self.conns[m.group(1)], t))
            return
        m = re.fullmatch(r'std::vector<Checkpoint> (\w+)', s)
        if m:
            self.cpvecs[m.group(1)] = []
            return
        m = re.fullmatch(r'(\w+)\.push_back\((Point\(.*\))\)', s)
        if m:
            self.cpvecs[m.group(1)].append(self.point(m.group(2)))
            return
        m = re.fullmatch(r'(\w+)->setRoutingCheckpoints\((\w+)\)', s)
        if m:
            cps = self.cpvecs[m.group(2)]
            self.out.append('checkpoints %s %d %s' % (
                self.conns[m.group(1)], len(cps),
                ' '.join('%s %s 15 15' % p for p in cps)))
            return
        m = re.fullmatch(r'router->moveShape\((\w+), (.*), (.*)\)', s)
        if m:
            self.out.append('move %s %s %s' % (self.shapes[m.group(1)], self.num(m.group(2)), self.num(m.group(3))))
            return
        m = re.fullmatch(r'router->moveJunction\((\w+), (.*), (.*)\)', s)
        if m:
            self.out.append('movej %s %s %s' % (self.juncs[m.group(1)], self.num(m.group(2)), self.num(m.group(3))))
            return
        if s == 'router->processTransaction()':
            self.out.append('process')
            return
        raise SystemExit('%s: unsupported statement %r' % (self.name, s))


def split_args(s):
    out, depth, cur = [], 0, ''
    for ch in s:
        if ch == ',' and depth == 0:
            out.append(cur)
            cur = ''
            continue
        if ch == '(':
            depth += 1
        elif ch == ')':
            depth -= 1
        cur += ch
    if cur.strip():
        out.append(cur)
    return [a.strip() for a in out]


def translate(path, name):
    src = open(path).read()
    src = re.sub(r'/\*.*?\*/', '', src, flags=re.S)
    src = re.sub(r'//[^\n]*', '', src)
    src = re.sub(r'#[^\n]*', '', src)
    src = src.replace('Avoid::', '')
    tr = Tr(name)
    for st in src.split(';'):
        st = st.strip()
        # Strip a leading "int main(void) {" glued to the first statement.
        st = re.sub(r'^int main\s*\(\s*(void)?\s*\)\s*\{', '', st).strip()
        st = st.lstrip('{').strip()
        while st.endswith('}'):
            st = st[:-1].strip()
        while st.startswith('}'):
            st = st[1:].strip()
        tr.stmt(st)
    return tr.out


def main():
    tests = sys.argv[1]
    for name in sys.argv[2:]:
        lines = translate('%s/%s.cpp' % (tests, name), name)
        print('#scene fixture-%s' % name)
        for l in lines:
            print(l)
        print('#endscene')


if __name__ == '__main__':
    main()
