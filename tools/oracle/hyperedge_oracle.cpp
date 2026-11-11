// Differential oracle for the Rust port of libavoid's hyperedge code:
// mtst.cpp (MinimumTerminalSpanningTree), hyperedgetree.cpp,
// hyperedge.cpp (HyperedgeRerouter) and hyperedgeimprover.cpp
// (HyperedgeImprover).
//
// Scenes are written in a small line-based DSL (the object commands are
// those of tools/oracle/orthogonal_vis_oracle.cpp; the hyperedge commands
// are documented at runCommand() below) that both this program and the
// Rust test `tests/hyperedge_oracle.rs` interpret.  The golden file
// `tests/golden/hyperedge_oracle.txt` contains, for every scene, the DSL
// text followed by this program's output; the Rust test re-runs the DSL and
// must reproduce the output byte for byte.
//
// Stages (observed *before* orthogonal nudging, which is ported
// separately):
//  (a) `mtst`: MinimumTerminalSpanningTree::constructInterleaved() /
//      constructSequential() run directly on terminal sets, dumping the
//      hyperedge tree, every vertex's sptfDist/pathNext and the edges
//      marked as hyperedge segments;
//  (b) `reroute 0`: Router::rerouteAndCallbackConnectors() replicated up
//      to and including HyperedgeRerouter::performRerouting() and
//      improveCrossings(), with registered hyperedges;
//  (c) `reroute 1` (the same plus HyperedgeImprover::execute() per the
//      routing options, on A* routes) and `improve M` (the improver alone
//      on hand-made display routes / fixed routes).
// Scenes named "full-*" end with a real processTransaction() (including
// improveOrthogonalRoutes); the Rust test for those is #[ignore]d until
// nudging is ported.
//
// Every scene runs in a forked child (so that a scene on which upstream
// asserts or crashes is simply dropped; the count is reported on stderr).
// Scenes that hit the upstream use-after-free of a stale `start->pathNext`
// in A* (see tools/oracle/build_hyperedge_oracle.sh) are dropped too
// (exit code 7), and so are the full-pipeline twins of dropped scenes.
// The child's stdout is /dev/null (libavoid prints warnings there); its
// output comes back through a pipe.
//
// Pointer order: operator new/delete are replaced by a bump allocator that
// never reuses memory and rounds allocations up to a multiple of a stride
// that is a multiple of sizeof(Avoid::Node) and of 16, so pointer order is
// allocation order -- the model the Rust port uses (creation serials).
//
// Build & regenerate (from the repository root):
//
//   VENDOR=$PWD/vendor/adaptagrams/cola
//   g++ -std=c++11 -O2 -ffp-contract=off -fno-access-control -I"$VENDOR" \
//       tools/oracle/hyperedge_oracle.cpp "$VENDOR"/libavoid/*.cpp \
//       -o /tmp/hyperedge_oracle
//   /tmp/hyperedge_oracle tools/oracle/hyperedge_fixtures.scenes \
//       > tests/golden/hyperedge_oracle.txt
// (or simply tools/oracle/build_hyperedge_oracle.sh).  The fixture scenes
// are translations of upstream fixtures by
// tools/oracle/hyperedge_fixture_scenes.py.
//
// (asserts stay enabled; ORACLE_RANDOM=n sets the number of random scenes
// per kind.)

#include <cstdio>
#include <cstdlib>
#include <cstdint>
#include <cstring>
#include <string>
#include <vector>
#include <map>
#include <set>
#include <list>
#include <sstream>
#include <fstream>
#include <new>
#include <sys/mman.h>
#include <sys/wait.h>
#include <unistd.h>
#include <fcntl.h>
#include <algorithm>
#include <csignal>

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
#include "libavoid/hyperedge.h"
#include "libavoid/hyperedgetree.h"
#include "libavoid/hyperedgeimprover.h"
#include "libavoid/mtst.h"

//============================================================================
// Monotonic allocator.
//============================================================================

static char *g_arena = nullptr;
static size_t g_arena_off = 0;
static const size_t g_arena_size = (size_t) 1 << 38;
static const size_t kStride = (sizeof(Avoid::Node) % 16 == 0) ?
        sizeof(Avoid::Node) : 2 * sizeof(Avoid::Node);

static void *oracleAlloc(size_t n)
{
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

void *operator new(size_t n) { return oracleAlloc(n); }
void *operator new[](size_t n) { return oracleAlloc(n); }
void operator delete(void *) noexcept { }
void operator delete[](void *) noexcept { }
void operator delete(void *, size_t) noexcept { }
void operator delete[](void *, size_t) noexcept { }

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

static std::vector<std::string> g_out;

static void out(const std::string& s)
{
    g_out.push_back(s);
}

static uint64_t fnv(const std::vector<std::string>& lines)
{
    uint64_t h = 1469598103934665603ULL;
    for (size_t i = 0; i < lines.size(); ++i)
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

static const size_t kFullDumpLimit = 200;

static void outLines(const std::vector<std::string>& lines)
{
    if (lines.size() <= kFullDumpLimit)
    {
        for (size_t i = 0; i < lines.size(); ++i)
        {
            out(lines[i]);
        }
    }
    else
    {
        char buf[128];
        snprintf(buf, sizeof(buf), "hash lines=%u fnv=%016llx",
                (unsigned) lines.size(), (unsigned long long) fnv(lines));
        out(buf);
    }
}

static std::string polyStr(const Polygon& poly)
{
    std::string s = "[";
    for (size_t i = 0; i < poly.ps.size(); ++i)
    {
        if (i) s += " ";
        s += pt(poly.ps[i]);
    }
    return s + "]";
}

static std::string vidStr(const VertID& id)
{
    return std::to_string(id.objID) + "," + std::to_string(id.vn) + ",p=" +
            std::to_string((unsigned) id.props);
}

static std::string endStr(const ConnEnd& e)
{
    switch (e.type())
    {
        case ConnEndPoint:
            return "P" + pt(e.position()) + "d" + std::to_string(e.directions());
        case ConnEndShapePin:
            return "S" + std::to_string(e.shape()->id()) + ":" +
                    std::to_string(e.pinClassId());
        case ConnEndJunction:
            return "J" + std::to_string(e.junction()->id());
        default:
            return "E";
    }
}

typedef std::map<const ConnRef *, unsigned> ConnIdMap;

static ConnIdMap connIdMap(Router *r)
{
    ConnIdMap m;
    for (ConnRefList::iterator it = r->connRefs.begin(); it != r->connRefs.end(); ++it)
    {
        m[*it] = (*it)->id();
    }
    return m;
}

// Connector ids of a list: from `m` (ids of connectors that existed at the
// start of the stage, for lists of deleted connectors), else from the live
// connector, else "dead" (a connector created and deleted in the stage).
static std::string connListStr(Router *r, const ConnRefList& l, const ConnIdMap& m)
{
    std::string s;
    for (ConnRefList::const_iterator it = l.begin(); it != l.end(); ++it)
    {
        ConnIdMap::const_iterator f = m.find(*it);
        if (f != m.end())
        {
            s += " " + std::to_string(f->second);
        }
        else if (std::find(r->connRefs.begin(), r->connRefs.end(), *it) != r->connRefs.end())
        {
            s += " " + std::to_string((*it)->id());
        }
        else
        {
            s += " dead";
        }
    }
    return s;
}

static std::string juncListStr(const JunctionRefList& l)
{
    std::string s;
    for (JunctionRefList::const_iterator it = l.begin(); it != l.end(); ++it)
    {
        s += " " + std::to_string((*it)->id());
    }
    return s;
}

static void dumpConnsAndJunctions(Router *r)
{
    std::vector<std::string> lines;
    for (ConnRefList::iterator it = r->connRefs.begin(); it != r->connRefs.end(); ++it)
    {
        ConnRef *c = *it;
        std::pair<ConnEnd, ConnEnd> ends = c->endpointConnEnds();
        lines.push_back("C " + std::to_string(c->id()) + " " + endStr(ends.first) +
                " " + endStr(ends.second) + " fixed=" +
                std::to_string((int) c->hasFixedRoute()) + " route=" +
                polyStr(c->route()) + " disp=" + polyStr(c->m_display_route));
    }
    for (ObstacleList::iterator it = r->m_obstacles.begin(); it != r->m_obstacles.end(); ++it)
    {
        JunctionRef *j = dynamic_cast<JunctionRef *> (*it);
        if (j)
        {
            lines.push_back("J " + std::to_string(j->id()) + " pos=" + pt(j->position()) +
                    " rec=" + pt(j->recommendedPosition()) + " fixed=" +
                    std::to_string((int) j->positionFixed()) + " conns=" +
                    std::to_string(j->attachedConnectors().size()));
        }
    }
    outLines(lines);
}

static void dumpRerouterLists(Router *r, const ConnIdMap& pre)
{
    HyperedgeRerouter& h = r->m_hyperedge_rerouter;
    for (size_t i = 0; i < h.m_new_junctions_vector.size(); ++i)
    {
        out("RL " + std::to_string(i) + " newJ" + juncListStr(h.m_new_junctions_vector[i]) +
                " | newC" + connListStr(r, h.m_new_connectors_vector[i], ConnIdMap()) +
                " | delJ" + juncListStr(h.m_deleted_junctions_vector[i]) +
                " | delC" + connListStr(r, h.m_deleted_connectors_vector[i], pre));
    }
}

static void dumpImproverLists(Router *r, const ConnIdMap& pre)
{
    HyperedgeImprover& h = r->m_hyperedge_improver;
    out("IL newJ" + juncListStr(h.m_new_junctions) +
            " | newC" + connListStr(r, h.m_new_connectors, ConnIdMap()) +
            " | delJ" + juncListStr(h.m_deleted_junctions) +
            " | delC" + connListStr(r, h.m_deleted_connectors, pre) +
            " | chgC" + connListStr(r, h.m_changed_connectors, ConnIdMap()));
}

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

static Polygon readPoly(std::istringstream& in)
{
    unsigned n = unum(in);
    Polygon poly(n);
    for (unsigned i = 0; i < n; ++i)
    {
        double x = num(in);
        double y = num(in);
        poly.ps[i] = Point(x, y);
    }
    return poly;
}

// Stage (a) dump of a MinimumTerminalSpanningTree.
static void dumpTreeNode(HyperedgeTreeNode *node, HyperedgeTreeEdge *parent,
        int depth, std::set<HyperedgeTreeNode *>& seen,
        std::map<const VertInf *, size_t>& vIndex, std::vector<std::string>& lines)
{
    if (seen.count(node))
    {
        lines.push_back(std::string(depth, ' ') + "N revisit " + pt(node->point));
        return;
    }
    seen.insert(node);
    std::string fv = "-";
    if (node->finalVertex)
    {
        std::map<const VertInf *, size_t>::iterator f = vIndex.find(node->finalVertex);
        fv = (f != vIndex.end()) ? std::to_string(f->second) : std::string("x");
        fv += "[" + vidStr(node->finalVertex->id) + "]";
    }
    lines.push_back(std::string(depth, ' ') + "N " + pt(node->point) + " j=" +
            (node->junction ? std::to_string(node->junction->id()) : std::string("-")) +
            " fv=" + fv + " cs=" + std::to_string((int) node->isConnectorSource) +
            " pd=" + std::to_string((int) node->isPinDummyEndpoint) + " e=" +
            std::to_string(node->edges.size()));
    for (std::list<HyperedgeTreeEdge *>::iterator it = node->edges.begin();
            it != node->edges.end(); ++it)
    {
        if (*it == parent)
        {
            continue;
        }
        dumpTreeNode((*it)->followFrom(node), *it, depth + 1, seen, vIndex, lines);
    }
}

// Replicates Router::rerouteAndCallbackConnectors() up to (and excluding)
// improveOrthogonalRoutes().  withImprover: also run the hyperedge improver
// as the routing options say.
static void rerouteStage(Router *r, bool withImprover)
{
    r->processActions();
    r->m_static_orthogonal_graph_invalidated = true;
    // (Connectors become active in processActions(); the rerouter's deleted
    // connectors are freed, so their ids are looked up here.)
    ConnIdMap pre = connIdMap(r);

    r->m_conn_reroute_flags.alertConns();
    r->regenerateStaticBuiltGraph();
    for (ConnRefList::const_iterator i = r->connRefs.begin(); i != r->connRefs.end(); ++i)
    {
        (*i)->freeActivePins();
    }
    ConnRefSet hyperedgeConns = r->m_hyperedge_rerouter.calcHyperedgeConnectors();
    for (ConnRefList::const_iterator i = r->connRefs.begin(); i != r->connRefs.end(); ++i)
    {
        ConnRef *connector = *i;
        if (hyperedgeConns.find(connector) != hyperedgeConns.end())
        {
            continue;
        }
        if (connector->hasFixedRoute())
        {
            continue;
        }
        connector->m_needs_repaint = false;
        connector->generatePath();
    }
    ConnIdMap pre2 = connIdMap(r);
    r->m_hyperedge_rerouter.performRerouting();
    r->improveCrossings();
    out("after rerouting");
    dumpRerouterLists(r, pre);
    if (withImprover)
    {
        ConnIdMap pre3 = connIdMap(r);
        bool withMinor = r->routingOption(improveHyperedgeRoutesMovingJunctions);
        bool withMajor = r->routingOption(
                improveHyperedgeRoutesMovingAddingAndDeletingJunctions);
        if (withMinor || withMajor)
        {
            r->m_hyperedge_improver.clear();
            r->m_hyperedge_improver.execute(withMajor);
        }
        out("after improver");
        dumpImproverLists(r, pre3);
    }
    (void) pre2;
    dumpConnsAndJunctions(r);
}

// Commands (one per line; '#' starts a comment line):
//   router FLAGS / param K V / option K 0|1
//   shape NAME ID N x1 y1 ... / rect NAME ID x1 y1 x2 y2
//   junction NAME ID x y / fixj NAME 0|1
//   pin NAME SHAPE CLASS XO YO PROP INSIDE DIRS / pinexcl NAME 0|1
//   conn NAME ID / connse NAME ID END END / src CONN END / dst CONN END
//   move SHAPE dx dy / movej JUNC dx dy / delconn CONN
//   prep                     processActions + regenerate orthogonal graph
//   route CONN N x y ...     set the display route (no routing)
//   fixroute CONN N x y ...  setFixedRoute
//   mtst int|seq N END*      MTST on the hyperedge vertices of the ends
//   registerj JUNC           registerHyperedgeForRerouting(junction)
//   registerends N END*      registerHyperedgeForRerouting(ConnEndList)
//   reroute 0|1              see rerouteStage()
//   improve 0|1              m_hyperedge_improver.clear()/execute(b)
//   transaction              processTransaction() (full pipeline)
//   type CONN T / jpin NAME JUNC CLASS DIRS / invalid CONN (makePathInvalid)
//   moveto SHAPE N x y ...   moveShape(shape, polygon)
//   delshape SHAPE / deljunc JUNC / removejunc JUNC
//                            (removeJunctionAndMergeConnectors())
//   bindnewj NAME / bindnewc NAME   name the first new junction / connector
//                            of the hyperedge improver's last run
//   diagram NAME             outputDiagram("output/NAME") (no-op here)
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
        Polygon poly = readPoly(in);
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
    else if (cmd == "type")
    {
        std::string name = word(in);
        unsigned t = unum(in);
        sc.conns.at(name)->setRoutingType((ConnType) t);
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
    else if (cmd == "invalid")
    {
        std::string name = word(in);
        sc.conns.at(name)->makePathInvalid();
    }
    else if (cmd == "moveto")
    {
        std::string name = word(in);
        Polygon poly = readPoly(in);
        r->moveShape(sc.shapes.at(name), poly);
    }
    else if (cmd == "delshape")
    {
        std::string name = word(in);
        r->deleteShape(sc.shapes.at(name));
        sc.shapes.erase(name);
    }
    else if (cmd == "deljunc")
    {
        std::string name = word(in);
        r->deleteJunction(sc.juncs.at(name));
        sc.juncs.erase(name);
    }
    else if (cmd == "removejunc")
    {
        std::string name = word(in);
        sc.juncs.at(name)->removeJunctionAndMergeConnectors();
    }
    else if (cmd == "bindnewj")
    {
        std::string name = word(in);
        sc.juncs[name] = r->m_hyperedge_improver.m_new_junctions.front();
    }
    else if (cmd == "bindnewc")
    {
        std::string name = word(in);
        sc.conns[name] = r->m_hyperedge_improver.m_new_connectors.front();
    }
    else if (cmd == "diagram")
    {
        // outputDiagram("output/NAME"): compared with tests/golden/NAME.txt
        // by tests/fixtures_hyperedge.rs (full-fixture-* scenes); nothing
        // to do here.
    }
    else if (cmd == "delconn")
    {
        std::string name = word(in);
        r->deleteConnector(sc.conns.at(name));
        sc.conns.erase(name);
    }
    else if (cmd == "prep")
    {
        r->processActions();
        r->m_static_orthogonal_graph_invalidated = true;
        r->regenerateStaticBuiltGraph();
        out("prep verts=" + std::to_string(r->vertices.connsSize()) + "+" +
                std::to_string(r->vertices.shapesSize()) + " edges=" +
                std::to_string(r->visOrthogGraph.size()));
    }
    else if (cmd == "route")
    {
        std::string name = word(in);
        Polygon poly = readPoly(in);
        sc.conns.at(name)->m_display_route = poly;
    }
    else if (cmd == "fixroute")
    {
        std::string name = word(in);
        Polygon poly = readPoly(in);
        sc.conns.at(name)->setFixedRoute(poly);
    }
    else if (cmd == "mtst")
    {
        std::string mode = word(in);
        unsigned n = unum(in);
        std::set<VertInf *> terminals;
        for (unsigned i = 0; i < n; ++i)
        {
            ConnEnd e = readEnd(sc, in);
            std::pair<bool, VertInf *> v = e.getHyperedgeVertex(r);
            terminals.insert(v.second);
        }
        JunctionHyperedgeTreeNodeMap juncMap;
        MinimumTerminalSpanningTree *mtst =
                new MinimumTerminalSpanningTree(r, terminals, &juncMap);
        if (mode == "seq")
        {
            mtst->constructSequential();
        }
        else
        {
            mtst->constructInterleaved();
        }
        std::vector<std::string> lines;
        std::map<const VertInf *, size_t> vIndex;
        std::vector<VertInf *> verts;
        for (VertInf *v = r->vertices.connsBegin(); v; v = v->lstNext)
        {
            vIndex[v] = verts.size();
            verts.push_back(v);
        }
        out("mtst " + mode + " terminals=" + std::to_string(terminals.size()) +
                " root=" + std::to_string((int) (mtst->rootJunction() != nullptr)));
        if (mtst->rootJunction())
        {
            std::set<HyperedgeTreeNode *> seen;
            dumpTreeNode(mtst->rootJunction(), nullptr, 0, seen, vIndex, lines);
        }
        outLines(lines);
        lines.clear();
        for (size_t i = 0; i < verts.size(); ++i)
        {
            VertInf *v = verts[i];
            std::string pn = "-";
            if (v->pathNext)
            {
                std::map<const VertInf *, size_t>::iterator f = vIndex.find(v->pathNext);
                pn = (f != vIndex.end()) ? std::to_string(f->second) : std::string("x");
            }
            lines.push_back("V " + std::to_string(i) + " d=" + g17(v->sptfDist) + " pn=" + pn);
        }
        size_t k = 0;
        std::string hs = "H";
        for (EdgeInf *e = r->visOrthogGraph.begin(); e; e = e->lstNext, ++k)
        {
            if (e->isHyperedgeSegment())
            {
                hs += " " + std::to_string(k);
            }
        }
        lines.push_back(hs);
        outLines(lines);
        if (mtst->rootJunction())
        {
            delete mtst;
        }
        dumpConnsAndJunctions(r);
    }
    else if (cmd == "registerj")
    {
        std::string name = word(in);
        r->hyperedgeRerouter()->registerHyperedgeForRerouting(sc.juncs.at(name));
    }
    else if (cmd == "registerends")
    {
        unsigned n = unum(in);
        ConnEndList ends;
        for (unsigned i = 0; i < n; ++i)
        {
            ends.push_back(readEnd(sc, in));
        }
        r->hyperedgeRerouter()->registerHyperedgeForRerouting(ends);
    }
    else if (cmd == "reroute")
    {
        unsigned v = unum(in);
        out("stage " + std::to_string(sc.stage++) + " reroute " + std::to_string(v));
        rerouteStage(r, v != 0);
    }
    else if (cmd == "improve")
    {
        unsigned v = unum(in);
        out("stage " + std::to_string(sc.stage++) + " improve " + std::to_string(v));
        ConnIdMap pre = connIdMap(r);
        r->m_hyperedge_improver.clear();
        r->m_hyperedge_improver.execute(v != 0);
        dumpImproverLists(r, pre);
        dumpConnsAndJunctions(r);
    }
    else if (cmd == "transaction")
    {
        out("stage " + std::to_string(sc.stage++) + " transaction");
        r->processTransaction();
        HyperedgeImprover& h = r->m_hyperedge_improver;
        out("IL counts " + std::to_string(h.m_new_junctions.size()) + " " +
                std::to_string(h.m_new_connectors.size()) + " " +
                std::to_string(h.m_deleted_junctions.size()) + " " +
                std::to_string(h.m_deleted_connectors.size()) + " " +
                std::to_string(h.m_changed_connectors.size()));
        dumpConnsAndJunctions(r);
    }
    else
    {
        fprintf(stderr, "unknown command: %s\n", line.c_str());
        abort();
    }
}

static void crashHandler(int sig)
{
    _exit(100 + sig);
}

static void runSceneInChild(const std::vector<std::string>& dsl, int fd)
{
    // Exit quietly on assertion failures and crashes (no core dumps).
    signal(SIGABRT, crashHandler);
    signal(SIGSEGV, crashHandler);
    signal(SIGBUS, crashHandler);
    signal(SIGFPE, crashHandler);
    signal(SIGILL, crashHandler);
    alarm(120);
    int devnull = open("/dev/null", O_WRONLY);
    dup2(devnull, 1);
    g_out.clear();
    Scene sc;
    sc.router = nullptr;
    sc.stage = 0;
    for (size_t i = 0; i < dsl.size(); ++i)
    {
        runCommand(sc, dsl[i]);
    }
    std::string all;
    for (size_t i = 0; i < g_out.size(); ++i)
    {
        all += g_out[i] + "\n";
    }
    size_t off = 0;
    while (off < all.size())
    {
        ssize_t w = write(fd, all.data() + off, all.size() - off);
        if (w <= 0) _exit(3);
        off += (size_t) w;
    }
    // Skip the router destructor: it is not part of what is compared.
    _exit(0);
}

static unsigned g_dropped = 0;

static bool runScene(const std::string& name, const std::vector<std::string>& dsl)
{
    int fds[2];
    if (pipe(fds) != 0) abort();
    fflush(stdout);
    pid_t pid = fork();
    if (pid == 0)
    {
        close(fds[0]);
        runSceneInChild(dsl, fds[1]);
    }
    close(fds[1]);
    std::string data;
    char buf[65536];
    for (;;)
    {
        ssize_t n = read(fds[0], buf, sizeof(buf));
        if (n <= 0) break;
        data.append(buf, (size_t) n);
    }
    close(fds[0]);
    int status = 0;
    waitpid(pid, &status, 0);
    if (!WIFEXITED(status) || WEXITSTATUS(status) != 0)
    {
        ++g_dropped;
        if (WIFEXITED(status) && WEXITSTATUS(status) == 7)
        {
            fprintf(stderr, "dropped scene %s (dangling start->pathNext)\n", name.c_str());
        }
        else
        {
            fprintf(stderr, "dropped scene %s (status %d)\n", name.c_str(), status);
        }
        return false;
    }
    printf("#scene %s\n", name.c_str());
    for (size_t i = 0; i < dsl.size(); ++i)
    {
        printf("%s\n", dsl[i].c_str());
    }
    printf("#out\n%s#end\n", data.c_str());
    fflush(stdout);
    return true;
}

//============================================================================
// Random scenes.
//============================================================================

static uint64_t g_state = 0x9e3779b97f4a7c15ULL;

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

static std::string f17(double v)
{
    return g17(v);
}

struct Gen
{
    std::vector<std::string> lines;
    std::vector<std::string> shapeNames;
    std::vector<std::vector<double> > boxes;
    std::vector<unsigned> shapeClasses;    // highest pin class per shape
    std::vector<std::string> juncNames;
    std::vector<std::vector<double> > juncPos;
    std::vector<std::string> connNames;
    int pinCount;
    int connCount;
};

static bool overlapsBox(const Gen& g, double x1, double y1, double x2, double y2)
{
    for (size_t i = 0; i < g.boxes.size(); ++i)
    {
        const std::vector<double>& b = g.boxes[i];
        if (x1 < b[2] + 5 && b[0] < x2 + 5 && y1 < b[3] + 5 && b[1] < y2 + 5)
        {
            return true;
        }
    }
    return false;
}

// Shapes on a 10-grid in [0, 200]^2 (mostly non-overlapping), each with a
// class-1 centre pin and some side pins of classes 2..4.
static void genShapes(Gen& g, unsigned nShapes)
{
    for (unsigned i = 0; i < nShapes; ++i)
    {
        double x1 = 0, y1 = 0, w = 0, h = 0;
        for (int tries = 0; tries < 20; ++tries)
        {
            x1 = 10.0 * rnd(19);
            y1 = 10.0 * rnd(19);
            w = 10.0 * (1 + rnd(4));
            h = 10.0 * (1 + rnd(4));
            if (!overlapsBox(g, x1, y1, x1 + w, y1 + h) || rnd(8) == 0)
            {
                break;
            }
        }
        std::string name = "s" + std::to_string(i + 1);
        g.lines.push_back("rect " + name + " " + std::to_string(100 + i) + " " +
                f17(x1) + " " + f17(y1) + " " + f17(x1 + w) + " " + f17(y1 + h));
        g.shapeNames.push_back(name);
        std::vector<double> b;
        b.push_back(x1); b.push_back(y1); b.push_back(x1 + w); b.push_back(y1 + h);
        g.boxes.push_back(b);
        // Centre pin, class 1, all directions.
        std::string pname = "p" + std::to_string(++g.pinCount);
        g.lines.push_back("pin " + pname + " " + name + " 1 0.5 0.5 1 0 " +
                std::to_string(rnd(3) == 0 ? 15u : 0u));
        if (rnd(2) == 0)
        {
            g.lines.push_back("pinexcl " + pname + " 0");
        }
        unsigned maxClass = 1;
        unsigned nSide = rnd(4);
        for (unsigned p = 0; p < nSide; ++p)
        {
            unsigned cls = 2 + rnd(3);
            if (cls > maxClass) maxClass = cls;
            pname = "p" + std::to_string(++g.pinCount);
            unsigned side = rnd(4);
            const char *pos[4] = { "0 0.5 1 0 4", "1 0.5 1 0 8", "0.5 0 1 0 1", "0.5 1 1 0 2" };
            g.lines.push_back("pin " + pname + " " + name + " " + std::to_string(cls) +
                    " " + pos[side]);
            if (rnd(2) == 0)
            {
                g.lines.push_back("pinexcl " + pname + " 0");
            }
        }
        g.shapeClasses.push_back(maxClass);
    }
}

static std::string shapeEnd(Gen& g)
{
    unsigned s = rnd((unsigned) g.shapeNames.size());
    // Class 1 always exists; other classes may not, so only use those
    // that are guaranteed: class 1, or the shape's highest class.
    unsigned cls = (rnd(3) == 0) ? g.shapeClasses[s] : 1;
    return "S " + g.shapeNames[s] + " " + std::to_string(cls);
}

static std::string freePoint(Gen& g)
{
    (void) g;
    double x = 5.0 * rnd(41);
    double y = 5.0 * rnd(41);
    return "P " + f17(x) + " " + f17(y) + " " + std::to_string(rnd(16));
}

static void addJunction(Gen& g, double x, double y)
{
    std::string name = "j" + std::to_string(g.juncNames.size() + 1);
    g.lines.push_back("junction " + name + " 0 " + f17(x) + " " + f17(y));
    if (rnd(5) == 0)
    {
        g.lines.push_back("fixj " + name + " 1");
    }
    g.juncNames.push_back(name);
    std::vector<double> p;
    p.push_back(x);
    p.push_back(y);
    g.juncPos.push_back(p);
}

static std::string newConn(Gen& g, const std::string& src, const std::string& dst)
{
    std::string name = "c" + std::to_string(++g.connCount);
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
    g.connNames.push_back(name);
    return name;
}

static double junctionCoord()
{
    return 10.0 * rnd(21) + ((rnd(4) == 0) ? 5.0 : 0.0);
}

// Builds junction trees: nTrees junction-rooted hyperedges, each with 1-3
// junctions connected by connectors and 3-6 terminals (shape pins / free
// points).
// cycles: sometimes add a second connector between two junctions of a
// tree (a cyclic hyperedge, which the improver skips; the rerouter would
// recurse forever on it, so only for unregistered scenes).
static void genJunctionTrees(Gen& g, unsigned nTrees, bool freePoints, bool cycles)
{
    for (unsigned t = 0; t < nTrees; ++t)
    {
        unsigned nJ = 1 + rnd(3);
        size_t first = g.juncNames.size();
        for (unsigned k = 0; k < nJ; ++k)
        {
            addJunction(g, junctionCoord(), junctionCoord());
            if (k > 0)
            {
                // Connect to an earlier junction of this tree.
                size_t other = first + rnd(k);
                std::string a = "J " + g.juncNames[first + k];
                std::string b = "J " + g.juncNames[other];
                if (rnd(2)) newConn(g, a, b); else newConn(g, b, a);
            }
        }
        if (cycles && nJ >= 2 && rnd(5) == 0)
        {
            newConn(g, "J " + g.juncNames[first], "J " + g.juncNames[first + nJ - 1]);
        }
        unsigned nTerm = (nJ == 1) ? 3 + rnd(3) : 2 + rnd(4);
        for (unsigned k = 0; k < nTerm || k < nJ + 1; ++k)
        {
            std::string j = "J " + g.juncNames[first + (k < nJ ? k : rnd(nJ))];
            std::string e = (freePoints && rnd(4) == 0) ? freePoint(g) : shapeEnd(g);
            if (rnd(2)) newConn(g, j, e); else newConn(g, e, j);
        }
    }
}

static void genOptions(Gen& g, bool improver)
{
    g.lines.push_back("router 2");
    if (rnd(3) == 0)
    {
        g.lines.push_back("param 0 " + f17((double) (10 + rnd(3) * 45)));
    }
    if (rnd(4) == 0)
    {
        g.lines.push_back("param 6 " + f17((double) (rnd(3) * 4)));
    }
    if (improver)
    {
        unsigned o = rnd(3);
        if (o == 0 || o == 2) g.lines.push_back("option 1 1");
        if (o == 1 || o == 2) g.lines.push_back("option 5 1");
    }
}

// (a) MTST on terminal sets.
static std::vector<std::string> genMtstScene(void)
{
    Gen g;
    g.pinCount = 0;
    g.connCount = 0;
    genOptions(g, false);
    genShapes(g, 3 + rnd(6));
    unsigned nJ = rnd(3);
    for (unsigned k = 0; k < nJ; ++k)
    {
        addJunction(g, junctionCoord(), junctionCoord());
    }
    // Some ordinary connectors (their endpoints are graph vertices).
    unsigned nC = rnd(4);
    for (unsigned k = 0; k < nC; ++k)
    {
        newConn(g, rnd(2) ? shapeEnd(g) : freePoint(g), shapeEnd(g));
    }
    g.lines.push_back("prep");
    unsigned nRuns = 1 + rnd(3);
    for (unsigned k = 0; k < nRuns; ++k)
    {
        unsigned nT = 2 + rnd(5);
        std::string s;
        for (unsigned t = 0; t < nT; ++t)
        {
            if (!g.juncNames.empty() && rnd(4) == 0)
            {
                s += " J " + g.juncNames[rnd((unsigned) g.juncNames.size())];
            }
            else
            {
                s += " " + shapeEnd(g);
            }
        }
        std::string mode = (rnd(4) == 0) ? "seq" : "int";
        g.lines.push_back("mtst " + mode + " " + std::to_string(nT) + s);
    }
    return g.lines;
}

// (b)/(c) Registered hyperedges and/or improver on A* routes.
static std::vector<std::string> genRerouteScene(bool improver, bool registered,
        bool full)
{
    Gen g;
    g.pinCount = 0;
    g.connCount = 0;
    genOptions(g, improver);
    genShapes(g, 2 + rnd(6));
    genJunctionTrees(g, 1 + rnd(2), true, improver && !registered);
    // Some unrelated connectors.
    unsigned nC = rnd(3);
    for (unsigned k = 0; k < nC; ++k)
    {
        newConn(g, freePoint(g), rnd(2) ? shapeEnd(g) : freePoint(g));
    }
    if (registered)
    {
        if (rnd(3) != 0 || g.juncNames.empty())
        {
            g.lines.push_back("registerj " + g.juncNames[rnd((unsigned) g.juncNames.size())]);
        }
        if (rnd(3) == 0)
        {
            unsigned nT = 3 + rnd(3);
            std::string s;
            for (unsigned t = 0; t < nT; ++t)
            {
                if (rnd(5) == 0)
                {
                    s += " J " + g.juncNames[rnd((unsigned) g.juncNames.size())];
                }
                else
                {
                    s += " " + shapeEnd(g);
                }
            }
            g.lines.push_back("registerends " + std::to_string(nT) + s);
        }
    }
    std::string stage = full ? "transaction" : (improver ? "reroute 1" : "reroute 0");
    g.lines.push_back(stage);
    // Sometimes a second round after moving something.
    if (rnd(3) == 0)
    {
        if (rnd(2) == 0)
        {
            std::string s = g.shapeNames[rnd((unsigned) g.shapeNames.size())];
            g.lines.push_back("move " + s + " " + f17(10.0 * ((int) rnd(5) - 2)) + " " +
                    f17(10.0 * ((int) rnd(5) - 2)));
        }
        else
        {
            g.lines.push_back("movej " + g.juncNames[rnd((unsigned) g.juncNames.size())] +
                    " 10 0");
        }
        if (registered && rnd(2) == 0)
        {
            // Junctions may have been replaced by the first round, so
            // register via terminal ends only.
            unsigned nT = 3 + rnd(2);
            std::string s;
            for (unsigned t = 0; t < nT; ++t)
            {
                s += " " + shapeEnd(g);
            }
            g.lines.push_back("registerends " + std::to_string(nT) + s);
        }
        g.lines.push_back(stage);
    }
    return g.lines;
}

// (c) Improver on hand-made display routes.
static std::vector<std::string> genHandRouteScene(void)
{
    Gen g;
    g.pinCount = 0;
    g.connCount = 0;
    genOptions(g, false);
    genShapes(g, rnd(5));
    std::vector<std::string> routes;
    unsigned nTrees = 1 + rnd(2);
    for (unsigned t = 0; t < nTrees; ++t)
    {
        double jx = junctionCoord();
        double jy = junctionCoord();
        addJunction(g, jx, jy);
        std::string jn = g.juncNames.back();
        unsigned nArms = 2 + rnd(4);
        // Shared first legs: direction and length.
        int prevDir = -1;
        double prevLen = 0;
        for (unsigned a = 0; a < nArms; ++a)
        {
            int dir = (prevDir >= 0 && rnd(2) == 0) ? prevDir : (int) rnd(4);
            double len = (dir == prevDir && rnd(2) == 0) ? prevLen : 5.0 * (1 + rnd(8));
            if (rnd(8) == 0) len = 0;
            prevDir = dir;
            prevLen = len;
            const double dx[4] = { 1, -1, 0, 0 };
            const double dy[4] = { 0, 0, 1, -1 };
            std::vector<Point> route;
            route.push_back(Point(jx, jy));
            Point p(jx + dx[dir] * len, jy + dy[dir] * len);
            route.push_back(p);
            unsigned nLegs = rnd(4);
            bool horiz = (dir < 2);
            for (unsigned l = 0; l < nLegs; ++l)
            {
                double d = 5.0 * ((int) rnd(13) - 6);
                if (horiz) p.y += d; else p.x += d;
                horiz = !horiz;
                route.push_back(p);
            }
            std::string endS;
            bool toJunction = (a + 1 == nArms) && rnd(3) == 0;
            if (toJunction)
            {
                // A second junction at the end of this arm (sometimes at
                // the same position as the first: a zero-length edge).
                if (rnd(3) == 0)
                {
                    route.resize(1);
                    route.push_back(Point(jx, jy));
                    p = Point(jx, jy);
                }
                addJunction(g, p.x, p.y);
                endS = "J " + g.juncNames.back();
            }
            else
            {
                endS = "P " + f17(p.x) + " " + f17(p.y) + " 15";
            }
            bool reversed = rnd(2) == 0;
            std::string c = reversed ? newConn(g, endS, "J " + jn) : newConn(g, "J " + jn, endS);
            if (reversed)
            {
                std::vector<Point> rev(route.rbegin(), route.rend());
                route = rev;
            }
            std::string s = std::to_string(route.size());
            for (size_t k = 0; k < route.size(); ++k)
            {
                s += " " + f17(route[k].x) + " " + f17(route[k].y);
            }
            if (rnd(10) == 0)
            {
                routes.push_back("fixroute " + c + " " + s);
                // Re-attach (setFixedRoute resets the endpoints to points).
                std::string first = reversed ? endS : "J " + jn;
                std::string second = reversed ? "J " + jn : endS;
                routes.push_back("src " + c + " " + first);
                routes.push_back("dst " + c + " " + second);
            }
            else
            {
                routes.push_back("route " + c + " " + s);
            }
        }
    }
    g.lines.push_back("prep");
    g.lines.insert(g.lines.end(), routes.begin(), routes.end());
    g.lines.push_back("improve " + std::to_string(rnd(2)));
    return g.lines;
}

//============================================================================

int main(int argc, char **argv)
{
    unsigned total = 0;
    if (argc > 1)
    {
        // Scenes from a file (tools/oracle/hyperedge_fixtures.scenes).
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
                runScene(name, dsl);
                ++total;
            }
            else
            {
                dsl.push_back(line);
            }
        }
    }
    unsigned nRandom = 300;
    if (getenv("ORACLE_RANDOM"))
    {
        nRandom = (unsigned) atoi(getenv("ORACLE_RANDOM"));
    }
    for (unsigned i = 0; i < nRandom; ++i)
    {
        runScene("mtst-" + std::to_string(i), genMtstScene());
        ++total;
    }
    for (unsigned i = 0; i < nRandom; ++i)
    {
        uint64_t saved = g_state;
        std::vector<std::string> dsl = genRerouteScene(false, true, false);
        bool ok = runScene("reroute-" + std::to_string(i), dsl);
        g_state = saved;
        dsl = genRerouteScene(false, true, true);
        if (ok)
        {
            runScene("full-reroute-" + std::to_string(i), dsl);
        }
        total += 2;
    }
    for (unsigned i = 0; i < nRandom; ++i)
    {
        uint64_t saved = g_state;
        bool registered = rnd(3) == 0;
        std::vector<std::string> dsl = genRerouteScene(true, registered, false);
        bool ok = runScene("improve-astar-" + std::to_string(i), dsl);
        g_state = saved;
        registered = rnd(3) == 0;
        dsl = genRerouteScene(true, registered, true);
        if (ok)
        {
            runScene("full-improve-astar-" + std::to_string(i), dsl);
        }
        total += 2;
    }
    for (unsigned i = 0; i < nRandom; ++i)
    {
        runScene("improve-hand-" + std::to_string(i), genHandRouteScene());
        ++total;
    }
    fprintf(stderr, "%u scenes, %u dropped\n", total, g_dropped);
    return 0;
}
