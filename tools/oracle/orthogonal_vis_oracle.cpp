// Differential oracle for the Rust port of libavoid scanline.cpp and the
// first half of orthogonal.cpp (static orthogonal visibility graph:
// generateStaticOrthogonalVisGraph, plus buildOrthogonalChannelInfo and the
// connector route checkpoint cache).
//
// Scenes are written in a small line-based DSL (documented at
// runCommand() below) that both this program and the Rust test
// `tests/orthogonal_vis_oracle.rs` interpret.  The golden file
// `tests/golden/orthogonal_vis_oracle.txt` contains, for every scene, the
// DSL text followed by this program's output; the Rust test re-runs the DSL
// and must reproduce the output byte for byte.
//
// Scenes:
//  * translations of upstream fixtures (tools/oracle/fixture_scenes.py ->
//    tools/oracle/orthogonal_vis_fixtures.scenes, passed as argv[1]);
//  * deterministic random scenes generated here (LCG below, same as the
//    geometry oracle), plus "checkpoints-N" scenes whose hand-made display
//    routes pass through / bend at / just miss checkpoints.  Never put two
//    RNG calls in one C++ expression.
//
// No routing happens: `process` runs Router::processActions(), marks the
// static orthogonal graph invalidated and calls
// Router::regenerateStaticBuiltGraph() (destroyOrthogonalVisGraph() +
// generateStaticOrthogonalVisGraph()), then dumps the vertex list and the
// orthogonal visibility graph.
//
// Pointer order: upstream breaks ties by comparing pointers (CmpVertInf,
// CmpNodePos) and, in compare_events, by `(int)(ea->v - eb->v)` -- a
// difference of two Node* that do not point into one array, which GCC
// compiles as an *exact* division by sizeof(Node) (multiplication by the
// modular inverse), so its sign is garbage unless the two addresses differ
// by a multiple of sizeof(Node).  This program therefore replaces the global
// operator new/delete by a bump allocator that never reuses memory within a
// scene and rounds every allocation up to a multiple of kStride (a multiple
// of sizeof(Node) and of 16).  Pointer order is then allocation order and
// the Node difference has the right sign -- the model the Rust port uses
// (creation serials / creation index).  The arena is released after every
// scene.  Set ORACLE_REAL_MALLOC=1 to use malloc/free instead and compare
// the outputs to see where real-heap pointer order changes a scene.
//
// Build & regenerate (from the repository root; VENDOR is the vendored
// adaptagrams checkout, which is gitignored):
//
//   VENDOR=$PWD/vendor/adaptagrams/cola
//   g++ -std=c++11 -O2 -ffp-contract=off -fno-access-control -I"$VENDOR" \
//       tools/oracle/orthogonal_vis_oracle.cpp "$VENDOR"/libavoid/*.cpp \
//       -o /tmp/orthogonal_vis_oracle
//   /tmp/orthogonal_vis_oracle tools/oracle/orthogonal_vis_fixtures.scenes \
//       > tests/golden/orthogonal_vis_oracle.txt
//
// (asserts stay enabled; ORACLE_FULL=1 prints full dumps instead of hashes
// for large stages.)

#include <cstdio>
#include <cstdlib>
#include <cstdint>
#include <cstring>
#include <string>
#include <vector>
#include <map>
#include <list>
#include <sstream>
#include <fstream>
#include <new>
#include <sys/mman.h>

#include "libavoid/libavoid.h"
#include "libavoid/router.h"
#include "libavoid/shape.h"
#include "libavoid/junction.h"
#include "libavoid/connector.h"
#include "libavoid/connend.h"
#include "libavoid/connectionpin.h"
#include "libavoid/obstacle.h"
#include "libavoid/vertices.h"
#include "libavoid/graph.h"
#include "libavoid/scanline.h"
#include "libavoid/orthogonal.h"

//============================================================================
// Monotonic allocator.
//============================================================================

static char *g_arena = nullptr;
static size_t g_arena_off = 0;
static const size_t g_arena_size = (size_t) 1 << 38;
static int g_real_malloc = -1;
static const size_t kStride = (sizeof(Avoid::Node) % 16 == 0) ?
        sizeof(Avoid::Node) : 2 * sizeof(Avoid::Node);

// Mark/release of the bump arena around a scene.
static size_t arenaMark(void)
{
    return g_arena_off;
}

static void arenaRelease(size_t mark)
{
    if (g_real_malloc == 0)
    {
        g_arena_off = mark;
    }
}

static void *oracleAlloc(size_t n)
{
    if (g_real_malloc < 0)
    {
        g_real_malloc = getenv("ORACLE_REAL_MALLOC") ? 1 : 0;
    }
    if (g_real_malloc)
    {
        void *p = malloc(n ? n : 1);
        if (!p) throw std::bad_alloc();
        return p;
    }
    if (!g_arena)
    {
        g_arena = (char *) mmap(nullptr, g_arena_size, PROT_READ | PROT_WRITE,
                MAP_PRIVATE | MAP_ANONYMOUS | MAP_NORESERVE, -1, 0);
        if (g_arena == MAP_FAILED) abort();
    }
    n = ((n + kStride - 1) / kStride) * kStride;
    if (n == 0) n = kStride;
    void *p = g_arena + g_arena_off;
    g_arena_off += n;
    if (g_arena_off > g_arena_size) abort();
    return p;
}

static void oracleFree(void *p)
{
    if (g_real_malloc == 1)
    {
        free(p);
    }
}

void *operator new(size_t n) { return oracleAlloc(n); }
void *operator new[](size_t n) { return oracleAlloc(n); }
void operator delete(void *p) noexcept { oracleFree(p); }
void operator delete[](void *p) noexcept { oracleFree(p); }
void operator delete(void *p, size_t) noexcept { oracleFree(p); }
void operator delete[](void *p, size_t) noexcept { oracleFree(p); }

using namespace Avoid;

//============================================================================
// Output helpers.
//============================================================================

static std::string g17(double v)
{
    if (v != v)
    {
        return "nan";
    }
    char buf[64];
    snprintf(buf, sizeof(buf), "%.17g", v);
    return buf;
}

static std::string pt(const Point& p)
{
    return "(" + g17(p.x) + "," + g17(p.y) + ")";
}

static bool g_full = false;
static std::vector<std::string> g_out;

static void out(const std::string& s)
{
    g_out.push_back(s);
}

static uint64_t fnv(const std::vector<std::string>& lines, size_t from)
{
    uint64_t h = 1469598103934665603ULL;
    for (size_t i = from; i < lines.size(); ++i)
    {
        const std::string& s = lines[i];
        for (size_t k = 0; k < s.size(); ++k)
        {
            h ^= (unsigned char) s[k];
            h *= 1099511628211ULL;
        }
        h ^= (unsigned char) '\n';
        h *= 1099511628211ULL;
    }
    return h;
}

// Large stage dumps are replaced by a line count and an FNV-1a hash.
static const size_t kFullDumpLimit = 300;

static void dumpGraph(Router *r, int stage)
{
    std::vector<std::string> lines;
    std::map<const VertInf *, size_t> vIndex;
    std::vector<VertInf *> verts;
    for (VertInf *v = r->vertices.connsBegin(); v; v = v->lstNext)
    {
        vIndex[v] = verts.size();
        verts.push_back(v);
    }
    std::map<const EdgeInf *, size_t> eIndex;
    std::vector<EdgeInf *> edges;
    for (EdgeInf *e = r->visOrthogGraph.begin(); e; e = e->lstNext)
    {
        eIndex[e] = edges.size();
        edges.push_back(e);
    }
    char buf[512];
    snprintf(buf, sizeof(buf), "stage %d verts=%u conns=%u edges=%u vis=%d invis=%d",
            stage, (unsigned) verts.size(), r->vertices.connsSize(),
            (unsigned) edges.size(), r->visGraph.size(), r->invisGraph.size());
    out(buf);
    for (size_t i = 0; i < verts.size(); ++i)
    {
        VertInf *v = verts[i];
        std::string partner = "-";
        if (v->m_orthogonalPartner)
        {
            partner = std::to_string(vIndex[v->m_orthogonalPartner]);
        }
        snprintf(buf, sizeof(buf), "V %u [%u,%u,p=%u] ", (unsigned) i,
                v->id.objID, (unsigned) v->id.vn, (unsigned) v->id.props);
        std::string s = buf + pt(v->point);
        snprintf(buf, sizeof(buf), " vis=%u f=%u o=%u partner=", v->visDirections,
                v->orthogVisPropFlags, (unsigned) v->orthogVisList.size());
        lines.push_back(s + buf + partner);
    }
    for (size_t i = 0; i < edges.size(); ++i)
    {
        EdgeInf *e = edges[i];
        snprintf(buf, sizeof(buf), "E %u %u %u d=%s dis=%d", (unsigned) i,
                (unsigned) vIndex[e->m_vert1], (unsigned) vIndex[e->m_vert2],
                g17(e->getDist()).c_str(), (int) e->isDisabled());
        lines.push_back(buf);
    }
    for (size_t i = 0; i < verts.size(); ++i)
    {
        VertInf *v = verts[i];
        if (v->orthogVisList.empty())
        {
            continue;
        }
        std::string s = "O " + std::to_string(i) + ":";
        for (EdgeInfList::const_iterator it = v->orthogVisList.begin();
                it != v->orthogVisList.end(); ++it)
        {
            s += " " + std::to_string(eIndex[*it]);
        }
        lines.push_back(s);
    }
    if (g_full || lines.size() <= kFullDumpLimit)
    {
        for (size_t i = 0; i < lines.size(); ++i)
        {
            out(lines[i]);
        }
    }
    else
    {
        snprintf(buf, sizeof(buf), "hash lines=%u fnv=%016llx",
                (unsigned) lines.size(), (unsigned long long) fnv(lines, 0));
        out(buf);
    }
}

//============================================================================
// Test ShiftSegment (twin of the Rust test's TestShiftSegment).
//============================================================================

class TestShiftSegment : public ShiftSegment
{
    public:
        TestShiftSegment(size_t dim, const Point& low, const Point& high,
                double minLim, double maxLim)
            : ShiftSegment(dim),
              m_low(low),
              m_high(high)
        {
            minSpaceLimit = minLim;
            maxSpaceLimit = maxLim;
        }
        Point& lowPoint(void) { return m_low; }
        Point& highPoint(void) { return m_high; }
        const Point& lowPoint(void) const { return m_low; }
        const Point& highPoint(void) const { return m_high; }
        bool overlapsWith(const ShiftSegment *rhs, const size_t dim) const
        {
            (void) rhs;
            (void) dim;
            return false;
        }
        bool immovable(void) const { return false; }

        Point m_low;
        Point m_high;
};

//============================================================================
// Scene DSL interpreter.
//============================================================================

struct Scene
{
    Router *router;
    std::map<std::string, ShapeRef *> shapes;
    std::map<std::string, JunctionRef *> juncs;
    std::map<std::string, ConnRef *> conns;
    std::map<std::string, ShapeConnectionPin *> pins;
    std::vector<ShiftSegment *> segs;
    int stage;
};

static double num(std::istringstream& in)
{
    std::string t;
    in >> t;
    if (t.empty())
    {
        fprintf(stderr, "missing number\n");
        abort();
    }
    return strtod(t.c_str(), nullptr);
}

static unsigned unum(std::istringstream& in)
{
    std::string t;
    in >> t;
    return (unsigned) strtoul(t.c_str(), nullptr, 10);
}

static std::string word(std::istringstream& in)
{
    std::string t;
    in >> t;
    return t;
}

// end := P x y dirs | S shape classId | J junction
static ConnEnd readEnd(Scene& sc, std::istringstream& in)
{
    std::string k = word(in);
    if (k == "P")
    {
        double x = num(in);
        double y = num(in);
        unsigned dirs = unum(in);
        return ConnEnd(Point(x, y), (ConnDirFlags) dirs);
    }
    else if (k == "S")
    {
        std::string s = word(in);
        unsigned cls = unum(in);
        return ConnEnd(sc.shapes.at(s), cls);
    }
    else if (k == "J")
    {
        std::string j = word(in);
        return ConnEnd(sc.juncs.at(j));
    }
    fprintf(stderr, "bad end %s\n", k.c_str());
    abort();
}

// Commands (one per line; '#' starts a comment line):
//   router FLAGS
//   param K V                    setRoutingParameter((RoutingParameter) K, V)
//   option K 0|1                 setRoutingOption((RoutingOption) K, b)
//   shape NAME ID N x1 y1 ...    new ShapeRef(router, Polygon, ID)
//   rect NAME ID x1 y1 x2 y2     new ShapeRef(router, Rectangle(P1, P2), ID)
//   junction NAME ID x y         new JunctionRef(router, Point, ID)
//   fixj NAME 0|1                junction->setPositionFixed(b)
//   pin NAME SHAPE CLASS XO YO PROP INSIDE DIRS
//                                new ShapeConnectionPin(shape, ...)
//   jpin NAME JUNC CLASS DIRS    new ShapeConnectionPin(junction, ...)
//   pinexcl NAME 0|1             pin->setExclusive(b)
//   conn NAME ID                 new ConnRef(router, ID)
//   connse NAME ID END END       new ConnRef(router, src, dst, ID)
//   src CONN END / dst CONN END  setSourceEndpoint / setDestEndpoint
//   type CONN T                  setRoutingType((ConnType) T)
//   checkpoints CONN N (x y ad dd)*   setRoutingCheckpoints
//   move SHAPE dx dy             router->moveShape(shape, dx, dy)
//   movej JUNC dx dy             router->moveJunction(junction, dx, dy)
//   delshape SHAPE / delconn CONN
//   process                      processActions + regenerate graph + dump
//   route CONN N x y ...         set the display route (no routing)
//   cpcache                      build/dump/clear/dump checkpoint cache
//   seg DIM lx ly hx hy MIN MAX  new TestShiftSegment (appended to list)
//   channel DIM                  buildOrthogonalChannelInfo + dump, clear list
//   obstacles                    print m_obstacles ids (debugging aid)
static void runCommand(Scene& sc, const std::string& line)
{
    std::istringstream in(line);
    std::string cmd = word(in);
    Router *r = sc.router;
    if (cmd.empty() || cmd[0] == '#')
    {
        return;
    }
    if (cmd == "router")
    {
        unsigned flags = unum(in);
        sc.router = new Router(flags);
        return;
    }
    if (cmd == "param")
    {
        unsigned k = unum(in);
        double v = num(in);
        r->setRoutingParameter((RoutingParameter) k, v);
    }
    else if (cmd == "option")
    {
        unsigned k = unum(in);
        unsigned v = unum(in);
        r->setRoutingOption((RoutingOption) k, v != 0);
    }
    else if (cmd == "shape")
    {
        std::string name = word(in);
        unsigned id = unum(in);
        unsigned n = unum(in);
        Polygon poly(n);
        for (unsigned i = 0; i < n; ++i)
        {
            double x = num(in);
            double y = num(in);
            poly.ps[i] = Point(x, y);
        }
        sc.shapes[name] = new ShapeRef(r, poly, id);
    }
    else if (cmd == "rect")
    {
        std::string name = word(in);
        unsigned id = unum(in);
        double x1 = num(in);
        double y1 = num(in);
        double x2 = num(in);
        double y2 = num(in);
        Rectangle rect(Point(x1, y1), Point(x2, y2));
        sc.shapes[name] = new ShapeRef(r, rect, id);
    }
    else if (cmd == "junction")
    {
        std::string name = word(in);
        unsigned id = unum(in);
        double x = num(in);
        double y = num(in);
        sc.juncs[name] = new JunctionRef(r, Point(x, y), id);
    }
    else if (cmd == "fixj")
    {
        std::string name = word(in);
        unsigned v = unum(in);
        sc.juncs.at(name)->setPositionFixed(v != 0);
    }
    else if (cmd == "pin")
    {
        std::string name = word(in);
        std::string shape = word(in);
        unsigned cls = unum(in);
        double xo = num(in);
        double yo = num(in);
        unsigned prop = unum(in);
        double inside = num(in);
        unsigned dirs = unum(in);
        sc.pins[name] = new ShapeConnectionPin(sc.shapes.at(shape), cls, xo, yo,
                prop != 0, inside, (ConnDirFlags) dirs);
    }
    else if (cmd == "jpin")
    {
        std::string name = word(in);
        std::string junc = word(in);
        unsigned cls = unum(in);
        unsigned dirs = unum(in);
        sc.pins[name] = new ShapeConnectionPin(sc.juncs.at(junc), cls,
                (ConnDirFlags) dirs);
    }
    else if (cmd == "pinexcl")
    {
        std::string name = word(in);
        unsigned v = unum(in);
        sc.pins.at(name)->setExclusive(v != 0);
    }
    else if (cmd == "conn")
    {
        std::string name = word(in);
        unsigned id = unum(in);
        sc.conns[name] = new ConnRef(r, id);
    }
    else if (cmd == "connse")
    {
        std::string name = word(in);
        unsigned id = unum(in);
        ConnEnd src = readEnd(sc, in);
        ConnEnd dst = readEnd(sc, in);
        sc.conns[name] = new ConnRef(r, src, dst, id);
    }
    else if (cmd == "src" || cmd == "dst")
    {
        std::string name = word(in);
        ConnEnd end = readEnd(sc, in);
        if (cmd == "src")
        {
            sc.conns.at(name)->setSourceEndpoint(end);
        }
        else
        {
            sc.conns.at(name)->setDestEndpoint(end);
        }
    }
    else if (cmd == "type")
    {
        std::string name = word(in);
        unsigned t = unum(in);
        sc.conns.at(name)->setRoutingType((ConnType) t);
    }
    else if (cmd == "checkpoints")
    {
        std::string name = word(in);
        unsigned n = unum(in);
        std::vector<Checkpoint> cps;
        for (unsigned i = 0; i < n; ++i)
        {
            double x = num(in);
            double y = num(in);
            unsigned ad = unum(in);
            unsigned dd = unum(in);
            cps.push_back(Checkpoint(Point(x, y), (ConnDirFlags) ad,
                    (ConnDirFlags) dd));
        }
        sc.conns.at(name)->setRoutingCheckpoints(cps);
    }
    else if (cmd == "move")
    {
        std::string name = word(in);
        double dx = num(in);
        double dy = num(in);
        r->moveShape(sc.shapes.at(name), dx, dy);
    }
    else if (cmd == "movej")
    {
        std::string name = word(in);
        double dx = num(in);
        double dy = num(in);
        r->moveJunction(sc.juncs.at(name), dx, dy);
    }
    else if (cmd == "delshape")
    {
        std::string name = word(in);
        r->deleteShape(sc.shapes.at(name));
        sc.shapes.erase(name);
    }
    else if (cmd == "delconn")
    {
        std::string name = word(in);
        r->deleteConnector(sc.conns.at(name));
        sc.conns.erase(name);
    }
    else if (cmd == "process")
    {
        r->processActions();
        r->m_static_orthogonal_graph_invalidated = true;
        r->regenerateStaticBuiltGraph();
        dumpGraph(r, sc.stage++);
    }
    else if (cmd == "obstacles")
    {
        std::string s = "obstacles";
        for (ObstacleList::iterator it = r->m_obstacles.begin();
                it != r->m_obstacles.end(); ++it)
        {
            s += " " + std::to_string((*it)->id());
        }
        out(s);
    }
    else if (cmd == "route")
    {
        std::string name = word(in);
        unsigned n = unum(in);
        Polygon poly(n);
        for (unsigned i = 0; i < n; ++i)
        {
            double x = num(in);
            double y = num(in);
            poly.ps[i] = Point(x, y);
        }
        sc.conns.at(name)->m_display_route = poly;
    }
    else if (cmd == "cpcache")
    {
        buildConnectorRouteCheckpointCache(r);
        for (ConnRefList::const_iterator it = r->connRefs.begin();
                it != r->connRefs.end(); ++it)
        {
            ConnRef *c = *it;
            std::string s = "C " + std::to_string(c->id()) + " n=" +
                    std::to_string(c->displayRoute().checkpointsOnRoute.size());
            for (size_t i = 0; i < c->displayRoute().checkpointsOnRoute.size(); ++i)
            {
                s += " " + std::to_string(c->displayRoute().checkpointsOnRoute[i].first) +
                        pt(c->displayRoute().checkpointsOnRoute[i].second);
            }
            out(s);
        }
        clearConnectorRouteCheckpointCache(r);
        for (ConnRefList::const_iterator it = r->connRefs.begin();
                it != r->connRefs.end(); ++it)
        {
            ConnRef *c = *it;
            out("Cleared " + std::to_string(c->id()) + " n=" +
                    std::to_string(c->displayRoute().checkpointsOnRoute.size()));
        }
    }
    else if (cmd == "seg")
    {
        unsigned dim = unum(in);
        double lx = num(in);
        double ly = num(in);
        double hx = num(in);
        double hy = num(in);
        double mn = num(in);
        double mx = num(in);
        sc.segs.push_back(new TestShiftSegment(dim, Point(lx, ly), Point(hx, hy),
                mn, mx));
    }
    else if (cmd == "channel")
    {
        unsigned dim = unum(in);
        ShiftSegmentList list(sc.segs.begin(), sc.segs.end());
        buildOrthogonalChannelInfo(r, dim, list);
        for (size_t i = 0; i < sc.segs.size(); ++i)
        {
            out("S " + std::to_string(i) + " min=" + g17(sc.segs[i]->minSpaceLimit) +
                    " max=" + g17(sc.segs[i]->maxSpaceLimit));
        }
        for (size_t i = 0; i < sc.segs.size(); ++i)
        {
            delete sc.segs[i];
        }
        sc.segs.clear();
    }
    else
    {
        fprintf(stderr, "unknown command: %s\n", line.c_str());
        abort();
    }
}

static void runScene(const std::string& name, const std::vector<std::string>& dsl)
{
    printf("#scene %s\n", name.c_str());
    for (size_t i = 0; i < dsl.size(); ++i)
    {
        printf("%s\n", dsl[i].c_str());
    }
    printf("#out\n");
    g_out.clear();
    Scene sc;
    sc.router = nullptr;
    sc.stage = 0;
    for (size_t i = 0; i < dsl.size(); ++i)
    {
        runCommand(sc, dsl[i]);
    }
    delete sc.router;
    for (size_t i = 0; i < g_out.size(); ++i)
    {
        printf("%s\n", g_out[i].c_str());
    }
    std::vector<std::string>().swap(g_out);
    printf("#end\n");
    fflush(stdout);
}

//============================================================================
// Random scenes.
//============================================================================

static uint64_t g_state = 0x853c49e6748fea9bULL;

static uint64_t next_u64()
{
    g_state = g_state * 6364136223846793005ULL + 1442695040888963407ULL;
    return g_state;
}

// Uniform integer in [0, n).
static unsigned rnd(unsigned n)
{
    return (unsigned) ((next_u64() >> 33) % n);
}

static double unit()
{
    return (double) (next_u64() >> 11) * (1.0 / 9007199254740992.0);
}

struct Gen
{
    bool frac;        // fractional coordinates
    double grid;      // grid step for integer scenes
    std::vector<std::string> lines;
    std::vector<std::string> shapeNames;
    std::vector<std::vector<double> > boxes;   // x1 y1 x2 y2 of shapes
    std::vector<std::string> juncNames;
    std::vector<std::vector<double> > juncPos;
    std::vector<std::string> connNames;
    std::vector<double> coordPool;   // previously used coordinates
    int pinCount;
};

static std::string f17(double v)
{
    return g17(v);
}

static double genCoord(Gen& g, double lo, double hi)
{
    unsigned mode = rnd(6);
    if (mode == 0 && !g.coordPool.empty())
    {
        // Reuse a coordinate to provoke alignment.
        return g.coordPool[rnd((unsigned) g.coordPool.size())];
    }
    double v;
    if (g.frac)
    {
        double u = unit();
        v = lo + u * (hi - lo);
        if (rnd(3) == 0)
        {
            v = (double) ((long) (v * 4)) / 4.0;
        }
    }
    else
    {
        unsigned steps = (unsigned) ((hi - lo) / g.grid);
        unsigned k = rnd(steps + 1);
        v = lo + k * g.grid;
    }
    g.coordPool.push_back(v);
    return v;
}

static std::string genEnd(Gen& g)
{
    unsigned kind = rnd(10);
    if (kind < 2 && !g.shapeNames.empty())
    {
        // Shape pin (pins are created with class ids 1..3 on every shape
        // that has pins; an unknown class makes upstream assert, so only
        // use shapes listed with pins: we always give class 1 to all).
        unsigned s = rnd((unsigned) g.shapeNames.size());
        return "S " + g.shapeNames[s] + " 1";
    }
    if (kind < 3 && !g.juncNames.empty())
    {
        unsigned j = rnd((unsigned) g.juncNames.size());
        return "J " + g.juncNames[j];
    }
    double x, y;
    unsigned where = rnd(4);
    if (where == 0 && !g.boxes.empty())
    {
        // Inside a shape.
        const std::vector<double>& b = g.boxes[rnd((unsigned) g.boxes.size())];
        double u = unit();
        x = b[0] + u * (b[2] - b[0]);
        double w = unit();
        y = b[1] + w * (b[3] - b[1]);
        if (!g.frac)
        {
            x = (double) (long) x;
            y = (double) (long) y;
        }
    }
    else if (where == 1 && !g.boxes.empty())
    {
        // On a shape edge.
        const std::vector<double>& b = g.boxes[rnd((unsigned) g.boxes.size())];
        unsigned side = rnd(4);
        double u = unit();
        if (side == 0) { x = b[0]; y = b[1] + u * (b[3] - b[1]); }
        else if (side == 1) { x = b[2]; y = b[1] + u * (b[3] - b[1]); }
        else if (side == 2) { y = b[1]; x = b[0] + u * (b[2] - b[0]); }
        else { y = b[3]; x = b[0] + u * (b[2] - b[0]); }
        if (!g.frac)
        {
            x = (double) (long) x;
            y = (double) (long) y;
        }
    }
    else
    {
        x = genCoord(g, -20, 120);
        y = genCoord(g, -20, 120);
    }
    unsigned dirs = rnd(16);
    return "P " + f17(x) + " " + f17(y) + " " + std::to_string(dirs);
}

static std::vector<std::string> genScene(unsigned index)
{
    Gen g;
    g.frac = (index % 3) == 2;
    g.grid = ((index % 5) == 0) ? 10.0 : 5.0;
    g.pinCount = 0;
    g.lines.push_back("router 2");
    if (rnd(4) == 0)
    {
        double buf = (double) rnd(3) * 2.5;
        g.lines.push_back("param 6 " + f17(buf));
    }
    if (rnd(5) == 0)
    {
        g.lines.push_back("param 7 " + f17((double) (1 + rnd(8))));
    }
    unsigned nShapes = rnd(9);
    for (unsigned i = 0; i < nShapes; ++i)
    {
        std::string name = "s" + std::to_string(i + 1);
        double x1 = genCoord(g, 0, 100);
        double y1 = genCoord(g, 0, 100);
        double w = g.frac ? (1 + unit() * 30) : (double) (g.grid * (1 + rnd(5)));
        double h = g.frac ? (1 + unit() * 30) : (double) (g.grid * (1 + rnd(5)));
        if (rnd(4) == 0 && !g.boxes.empty())
        {
            // Nested in / overlapping / touching an earlier shape.
            const std::vector<double>& b = g.boxes[rnd((unsigned) g.boxes.size())];
            unsigned rel = rnd(4);
            if (rel == 0) { x1 = b[2]; y1 = b[1]; }                 // touching right
            else if (rel == 1) { x1 = b[0]; y1 = b[3]; }            // touching below
            else if (rel == 2) { x1 = b[0] + 1; y1 = b[1] + 1; w = (b[2] - b[0]) / 2; h = (b[3] - b[1]) / 2; }
            else { x1 = b[0] + (b[2] - b[0]) / 2; y1 = b[1] + (b[3] - b[1]) / 2; } // overlap
            if (w <= 0) w = 1;
            if (h <= 0) h = 1;
        }
        double x2 = x1 + w;
        double y2 = y1 + h;
        unsigned id = rnd(3) == 0 ? 0 : 100 + i;
        if (rnd(5) == 0)
        {
            // Non-rectangular polygon (routing uses its bounding box).
            double mx = x1 + (x2 - x1) / 2;
            double my = y1 + (y2 - y1) / 2;
            g.lines.push_back("shape " + name + " " + std::to_string(id) + " 5 " +
                    f17(mx) + " " + f17(y1) + " " + f17(x2) + " " + f17(my) + " " +
                    f17(x2) + " " + f17(y2) + " " + f17(x1) + " " + f17(y2) + " " +
                    f17(x1) + " " + f17(my));
        }
        else
        {
            g.lines.push_back("rect " + name + " " + std::to_string(id) + " " +
                    f17(x1) + " " + f17(y1) + " " + f17(x2) + " " + f17(y2));
        }
        g.shapeNames.push_back(name);
        std::vector<double> b;
        b.push_back(x1); b.push_back(y1); b.push_back(x2); b.push_back(y2);
        g.boxes.push_back(b);
        // Pins: always one class-1 pin (so "S name 1" ends are valid),
        // sometimes more.
        unsigned nPins = 1 + rnd(3);
        for (unsigned p = 0; p < nPins; ++p)
        {
            std::string pname = "p" + std::to_string(++g.pinCount);
            unsigned cls = (p == 0) ? 1 : 1 + rnd(3);
            unsigned prop = rnd(3) != 0;
            double xo, yo;
            if (prop)
            {
                const double opts[] = { 0.0, 0.25, 0.5, 1.0, 0.3 };
                xo = opts[rnd(5)];
                yo = opts[rnd(5)];
            }
            else
            {
                xo = (double) rnd((unsigned) (w + 1));
                yo = (double) rnd((unsigned) (h + 1));
            }
            double inside = (rnd(3) == 0) ? (double) rnd(4) : 0.0;
            unsigned dirs = rnd(16);
            g.lines.push_back("pin " + pname + " " + name + " " + std::to_string(cls) +
                    " " + f17(xo) + " " + f17(yo) + " " + std::to_string(prop) + " " +
                    f17(inside) + " " + std::to_string(dirs));
            if (rnd(3) == 0)
            {
                g.lines.push_back("pinexcl " + pname + " " + std::to_string(rnd(2)));
            }
        }
    }
    unsigned nJuncs = rnd(3);
    for (unsigned i = 0; i < nJuncs; ++i)
    {
        std::string name = "j" + std::to_string(i + 1);
        double x = genCoord(g, 0, 100);
        double y = genCoord(g, 0, 100);
        g.lines.push_back("junction " + name + " 0 " + f17(x) + " " + f17(y));
        if (rnd(2) == 0)
        {
            g.lines.push_back("fixj " + name + " 1");
        }
        g.juncNames.push_back(name);
        std::vector<double> jp;
        jp.push_back(x);
        jp.push_back(y);
        g.juncPos.push_back(jp);
    }
    unsigned nConns = rnd(8);
    for (unsigned i = 0; i < nConns; ++i)
    {
        std::string name = "c" + std::to_string(i + 1);
        std::string src = genEnd(g);
        std::string dst = genEnd(g);
        if (rnd(2) == 0)
        {
            g.lines.push_back("connse " + name + " 0 " + src + " " + dst);
        }
        else
        {
            g.lines.push_back("conn " + name + " 0");
            g.lines.push_back("src " + name + " " + src);
            g.lines.push_back("dst " + name + " " + dst);
        }
        if (rnd(4) == 0)
        {
            unsigned n = 1 + rnd(2);
            std::string s = "checkpoints " + name + " " + std::to_string(n);
            for (unsigned k = 0; k < n; ++k)
            {
                double x = genCoord(g, 0, 100);
                double y = genCoord(g, 0, 100);
                unsigned ad = rnd(16);
                unsigned dd = rnd(16);
                s += " " + f17(x) + " " + f17(y) + " " + std::to_string(ad) + " " +
                        std::to_string(dd);
            }
            g.lines.push_back(s);
        }
        g.connNames.push_back(name);
    }
    g.lines.push_back("process");
    // Sometimes modify and regenerate.
    if (rnd(3) == 0)
    {
        if (!g.shapeNames.empty() && rnd(2) == 0)
        {
            std::string s = g.shapeNames[rnd((unsigned) g.shapeNames.size())];
            double dx = g.frac ? unit() * 10 - 5 : (double) ((int) rnd(5) - 2) * g.grid;
            double dy = g.frac ? unit() * 10 - 5 : (double) ((int) rnd(5) - 2) * g.grid;
            g.lines.push_back("move " + s + " " + f17(dx) + " " + f17(dy));
        }
        if (!g.juncNames.empty() && rnd(2) == 0)
        {
            std::string j = g.juncNames[rnd((unsigned) g.juncNames.size())];
            g.lines.push_back("movej " + j + " " + f17(g.grid) + " 0");
        }
        if (!g.connNames.empty() && rnd(3) == 0)
        {
            std::string c = g.connNames[rnd((unsigned) g.connNames.size())];
            std::string e = genEnd(g);
            g.lines.push_back("dst " + c + " " + e);
        }
        g.lines.push_back("process");
    }
    // Channel info for a few random segments.
    if (rnd(2) == 0)
    {
        unsigned dim = rnd(2);
        unsigned altDim = (dim + 1) % 2;
        unsigned nSegs = 1 + rnd(6);
        for (unsigned k = 0; k < nSegs; ++k)
        {
            double p;
            if (rnd(3) == 0 && !g.boxes.empty())
            {
                // Aligned with a shape edge or centre line.
                const std::vector<double>& b = g.boxes[rnd((unsigned) g.boxes.size())];
                unsigned which = rnd(3);
                double lo = b[dim], hi = b[dim + 2];
                p = (which == 0) ? lo : (which == 1) ? hi : lo + (hi - lo) / 2;
            }
            else if (rnd(4) == 0 && !g.juncPos.empty())
            {
                p = g.juncPos[0][dim];
            }
            else
            {
                p = genCoord(g, -10, 110);
            }
            double a = genCoord(g, -10, 110);
            double len = g.frac ? 0.5 + unit() * 60 : g.grid * (1 + rnd(8));
            Point low, high;
            low[dim] = p;
            high[dim] = p;
            low[altDim] = a;
            high[altDim] = a + len;
            double mn = (rnd(3) == 0) ? p - 5 : -CHANNEL_MAX;
            double mx = (rnd(3) == 0) ? p + 5 : CHANNEL_MAX;
            g.lines.push_back("seg " + std::to_string(dim) + " " + f17(low.x) + " " +
                    f17(low.y) + " " + f17(high.x) + " " + f17(high.y) + " " + f17(mn) +
                    " " + f17(mx));
        }
        g.lines.push_back("channel " + std::to_string(dim));
    }
    // Checkpoint cache on hand-made display routes.
    if (!g.connNames.empty() && rnd(3) == 0)
    {
        for (size_t c = 0; c < g.connNames.size(); ++c)
        {
            unsigned n = 2 + rnd(4);
            std::string s = "route " + g.connNames[c] + " " + std::to_string(n);
            double x = genCoord(g, 0, 100);
            double y = genCoord(g, 0, 100);
            for (unsigned k = 0; k < n; ++k)
            {
                s += " " + f17(x) + " " + f17(y);
                if (k % 2 == 0)
                {
                    x = genCoord(g, 0, 100);
                }
                else
                {
                    y = genCoord(g, 0, 100);
                }
            }
            g.lines.push_back(s);
        }
        g.lines.push_back("cpcache");
    }
    return g.lines;
}

// Scenes focused on buildConnectorRouteCheckpointCache: display routes are
// built to pass through (or bend at, or just miss) the checkpoints.
static std::vector<std::string> genCheckpointScene(void)
{
    std::vector<std::string> lines;
    std::vector<std::string> routes;
    lines.push_back("router 2");
    unsigned nConns = 1 + rnd(4);
    for (unsigned c = 0; c < nConns; ++c)
    {
        std::string name = "c" + std::to_string(c + 1);
        double sx = 5.0 * rnd(21);
        double sy = 5.0 * rnd(21);
        double tx = 5.0 * rnd(21);
        double ty = 5.0 * rnd(21);
        lines.push_back("connse " + name + " 0 P " + f17(sx) + " " + f17(sy) +
                " 15 P " + f17(tx) + " " + f17(ty) + " 15");
        unsigned nCps = rnd(4);
        std::vector<Point> cps;
        std::string s = "checkpoints " + name + " " + std::to_string(nCps);
        for (unsigned k = 0; k < nCps; ++k)
        {
            double x = 5.0 * rnd(21);
            double y = 5.0 * rnd(21);
            cps.push_back(Point(x, y));
            s += " " + f17(x) + " " + f17(y) + " 15 15";
        }
        lines.push_back(s);
        routes.push_back(std::string());
        std::string& r = routes.back();
        // Build an orthogonal route visiting the checkpoints.
        std::vector<Point> route;
        Point curr(sx, sy);
        route.push_back(curr);
        for (unsigned k = 0; k < nCps; ++k)
        {
            const Point& cp = cps[k];
            unsigned how = rnd(5);
            if (how == 0)
            {
                // Bend exactly at the checkpoint.
                route.push_back(Point(cp.x, curr.y));
                route.push_back(cp);
                curr = cp;
            }
            else if (how == 1)
            {
                // Pass through the checkpoint on a vertical segment.
                double d = 5.0 * (1 + rnd(3));
                route.push_back(Point(cp.x, curr.y));
                route.push_back(Point(cp.x, cp.y - d));
                route.push_back(Point(cp.x, cp.y + d));
                curr = Point(cp.x, cp.y + d);
            }
            else if (how == 2)
            {
                // Pass through on a horizontal segment.
                double d = 5.0 * (1 + rnd(3));
                route.push_back(Point(curr.x, cp.y));
                route.push_back(Point(cp.x - d, cp.y));
                route.push_back(Point(cp.x + d, cp.y));
                curr = Point(cp.x + d, cp.y);
            }
            else if (how == 3)
            {
                // Bend within / just outside Point::equals() epsilon.
                double eps = (rnd(2) == 0) ? 0.00005 : 0.0002;
                route.push_back(Point(cp.x + eps, curr.y));
                route.push_back(Point(cp.x + eps, cp.y));
                curr = Point(cp.x + eps, cp.y);
            }
            // how == 4: the route misses this checkpoint.
        }
        route.push_back(Point(tx, curr.y));
        route.push_back(Point(tx, ty));
        r = "route " + name + " " + std::to_string(route.size());
        for (size_t k = 0; k < route.size(); ++k)
        {
            r += " " + f17(route[k].x) + " " + f17(route[k].y);
        }
    }
    // (Connectors join Router::connRefs when processActions() makes them
    // active.)
    lines.push_back("process");
    lines.insert(lines.end(), routes.begin(), routes.end());
    lines.push_back("cpcache");
    return lines;
}

//============================================================================

int main(int argc, char **argv)
{
    g_full = getenv("ORACLE_FULL") != nullptr;
    if (argc > 1)
    {
        std::ifstream f(argv[1]);
        if (!f)
        {
            fprintf(stderr, "cannot open %s\n", argv[1]);
            return 1;
        }
        std::string line;
        std::string name;
        std::vector<std::string> dsl;
        while (std::getline(f, line))
        {
            if (line.compare(0, 7, "#scene ") == 0)
            {
                name = line.substr(7);
                dsl.clear();
            }
            else if (line == "#endscene")
            {
                size_t mark = arenaMark();
                runScene(name, dsl);
                arenaRelease(mark);
            }
            else
            {
                dsl.push_back(line);
            }
        }
    }
    unsigned nRandom = 600;
    if (getenv("ORACLE_RANDOM"))
    {
        nRandom = (unsigned) atoi(getenv("ORACLE_RANDOM"));
    }
    for (unsigned i = 0; i < nRandom; ++i)
    {
        std::vector<std::string> dsl = genScene(i);
        std::string name = "random-" + std::to_string(i);
        size_t mark = arenaMark();
        runScene(name, dsl);
        arenaRelease(mark);
    }
    for (unsigned i = 0; i < nRandom / 5; ++i)
    {
        std::vector<std::string> dsl = genCheckpointScene();
        std::string name = "checkpoints-" + std::to_string(i);
        size_t mark = arenaMark();
        runScene(name, dsl);
        arenaRelease(mark);
    }
    return 0;
}
