// End-to-end differential fuzzer for the Rust port of libavoid
// (src/avoid/).  Generates random scenes that mix every public feature of
// the library in one Router and run long transaction sequences, executes
// them against the real C++ libavoid and dumps the observable state after
// every transaction.  The Rust test `tests/e2e_fuzz.rs` replays the same
// DSL (interpreter in `tests/e2e_fuzz_dsl/mod.rs`) and must reproduce the
// dumps byte for byte.
//
// Build & run: tools/oracle/e2e_fuzz.sh (builds an asserting, an -DNDEBUG
// and an ASan+UBSan build of this driver plus the vendored libavoid, merges
// their results and classifies every scene; see the script header).
//
// Usage:
//   e2e_fuzz gen SEED FIRST COUNT   generate scenes FIRST..FIRST+COUNT-1 of
//                                   campaign SEED, run each, print golden
//   e2e_fuzz dsl SEED FIRST COUNT   only print the generated DSL
//   e2e_fuzz run FILE               run the scenes (DSL parts) of FILE
// Environment:
//   E2E_FULL=1     never hash stage dumps (print every line)
//   E2E_INPROC=1   run scenes in-process (no fork; crashes kill the run;
//                  prints a backtrace -- for debugging)
//   E2E_TIMEOUT=s  per-scene time limit (default 20 s)
//
// Every scene runs in a forked child, so an upstream assertion failure,
// crash or timeout only marks that scene "#crash ...".
//
// Pointer order: operator new is the bump allocator of
// tools/oracle/e2e_fuzz_alloc.cpp (never reuses memory, so pointer order ==
// allocation order == the creation-serial model of the Rust port).
//
//============================================================================
// DSL (one command per line; '#' lines are comments).  Object references:
// the interpreter keeps lists of live shapes / junctions / connectors /
// clusters (creation order; objects created by the router -- hyperedge
// rerouting/improvement, splitAtSegment -- are appended when discovered,
// in router list order, after every command).  A reference R is either a
// non-negative integer (element R % size) or "L" (the last element); a
// command whose list is empty is skipped.
//
//   router FLAGS                 new Router(FLAGS)
//   param K V | option K B       setRoutingParameter / setRoutingOption
//   settings                     registerSettingsChange()
//   nowrap                       disable the immediate-mode safety wrapping
//                                (see unsafeImmediate()) for the rest of
//                                the scene
//   txn B                        setTransactionUse(B)
//   process                      processTransaction() and dump
//   markmoved                    markAllObstaclesAsMoved()
//   rect ID x1 y1 x2 y2          new ShapeRef(Rectangle(P1, P2), ID)
//   poly ID N x y ...            new ShapeRef(Polygon, ID)
//   moveby S dx dy               moveShape(shape, dx, dy)
//   moveto S FIRST N x y ...     moveShape(shape, poly, FIRST)
//   delshape S                   deleteShape
//   pin S CLS XO YO PROP INSIDE DIRS   new ShapeConnectionPin(shape, ...)
//   pinexcl S P B | pincost S P C | delpin S P
//                                (P indexes the shape's pin set, mod size)
//   transform S T                transformConnectionPinPositions(T)
//   junction ID x y FIXED        new JunctionRef (+ setPositionFixed)
//   fixj J B | movejby J dx dy | movejto J x y | deljunc J | mergej J
//                                (deljunc is skipped for a junction with a
//                                queued JunctionAdd: upstream asserts)
//   conn ID END END              new ConnRef(src, dst, ID)
//   src C END | dst C END        setSourceEndpoint / setDestEndpoint
//   type C T | hate C B | invalid C | delconn C
//   cps C N (x y ad dd)*         setRoutingCheckpoints
//   fixroute C N (x y)*          setFixedRoute
//   fixexist C                   setFixedExistingRoute (if route has >= 2 pts)
//   clearfix C                   clearFixedRoute
//   split C SEG                  splitAtSegment(SEG)
//   cluster ID N x y ...         new ClusterRef(Polygon, ID)
//   movecluster K N x y ...      deleteCluster; setNewPoly; addCluster
//   delcluster K                 deleteCluster
//   regj J                       registerHyperedgeForRerouting(junction)
//   regends N END*               registerHyperedgeForRerouting(ConnEndList)
// END := P x y DIRS | S S CLS | J J
// (S/J ends with an empty list fall back to P 0 0 15.)
//============================================================================

#include <cstdio>
#include <cstdlib>
#include <cstdint>
#include <cstring>
#include <cmath>
#include <string>
#include <vector>
#include <map>
#include <set>
#include <list>
#include <sstream>
#include <fstream>
#include <algorithm>
#include <unistd.h>
#include <sys/wait.h>
#include <poll.h>
#include <fcntl.h>
#include <execinfo.h>
#include <sys/resource.h>
#include <signal.h>
#include <ctime>

#include "libavoid/libavoid.h"
#include "libavoid/router.h"
#include "libavoid/shape.h"
#include "libavoid/junction.h"
#include "libavoid/connector.h"
#include "libavoid/connend.h"
#include "libavoid/connectionpin.h"
#include "libavoid/viscluster.h"
#include "libavoid/hyperedge.h"
#include "libavoid/hyperedgeimprover.h"
#include "libavoid/actioninfo.h"

using namespace Avoid;

// Incremented at the start of Router::rerouteAndCallbackConnectors() by the
// patched copy of router.cpp (tools/oracle/e2e_fuzz_build.sh).
namespace Avoid {
unsigned long e2eRerouteCount = 0;
}

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

static std::string ptStr(const Point& p)
{
    return g17(p.x) + "," + g17(p.y);
}

static std::string polyStr(const PolyLine& p)
{
    std::string s = std::to_string(p.size()) + ":";
    for (size_t i = 0; i < p.size(); ++i)
    {
        s += " " + ptStr(p.ps[i]);
    }
    return s;
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

// Whole-scene outputs up to this many lines are stored in full; longer
// ones store one hash line per stage.
static const size_t kFullSceneLimit = 400;

static bool g_full = false;

//============================================================================
// Scene state.
//============================================================================

static std::vector<unsigned> g_callbacks;

// Set when a shape's ShapeConnectionPinSet is no longer sorted by its own
// comparator.  ShapeRef::transformConnectionPinPositions() mutates the
// keys (offsets, directions) of std::set elements in place, which violates
// the container's ordering requirement (undefined behaviour); libstdc++'s
// red-black tree then erases/finds by a descent the Rust port's sorted
// vector does not model (later pin/shape deletion erases the wrong number
// of elements).  Such scenes are excluded ("#exclude pinset-unsorted").
static bool g_pinsetUnsorted = false;

static void connCallback(void *ptr)
{
    g_callbacks.push_back((unsigned) (uintptr_t) ptr);
}

struct Scene
{
    Router *r;
    std::vector<ShapeRef *> shapes;
    std::vector<JunctionRef *> juncs;
    std::vector<ConnRef *> conns;
    std::vector<ClusterRef *> clusters;
    // Ids of the junctions/connectors that were live at the previous sync
    // (to name objects deleted since).
    std::map<const void *, unsigned> knownIds;
    // Last seen hyperedge improver / rerouter object lists.
    unsigned long rerouteCount;   // e2eRerouteCount at the previous sync
    bool noWrap;                  // "nowrap" seen
    std::vector<std::string> pending;   // H/R lines for the next dump
    int stage;
    std::vector<std::vector<std::string> > stages;
};

static bool isPendingAction(Router *r, ActionType type, const void *obj)
{
    for (ActionInfoList::const_iterator it = r->actionList.begin();
            it != r->actionList.end(); ++it)
    {
        if (it->type == type && it->objPtr == obj)
        {
            return true;
        }
    }
    return false;
}

// Whether a queued ConnChange action holds a ConnEnd attached to `o`:
// deleting `o` in the same transaction makes processActions() read the
// freed obstacle when it applies the ConnEnd (connend.cpp:128).
static bool queuedConnEndUses(Router *r, const Obstacle *o)
{
    for (ActionInfoList::const_iterator it = r->actionList.begin();
            it != r->actionList.end(); ++it)
    {
        if (it->type != ConnChange) continue;
        for (ConnUpdateList::const_iterator c = it->conns.begin(); c != it->conns.end(); ++c)
        {
            if (c->second.m_anchor_obj == o) return true;
        }
    }
    return false;
}

static bool inObstacles(Router *r, const Obstacle *o)
{
    for (ObstacleList::const_iterator it = r->m_obstacles.begin();
            it != r->m_obstacles.end(); ++it)
    {
        if (*it == o) return true;
    }
    return false;
}

static bool inConnRefs(Router *r, const ConnRef *c)
{
    for (ConnRefList::const_iterator it = r->connRefs.begin();
            it != r->connRefs.end(); ++it)
    {
        if (*it == c) return true;
    }
    return false;
}

// The driver's connector list is synchronised after transactions; a
// connector freed since (e.g. by the transaction that immediate mode runs
// before the next command) is a stale pointer there.  Using it is a
// use-after-free in the driver, which the ASan build only sees when its own
// pointer order leads to the same deletion: it is reported in every build
// (the scene is classified "#exclude crash").  The Rust interpreter treats
// such a handle as a no-op.
static void staleConnCheck(Scene& sc, ConnRef *c)
{
    if (!inConnRefs(sc.r, c) && !isPendingAction(sc.r, ConnChange, c))
    {
        fprintf(stderr, "ERROR: stale connector handle (driver use of a freed ConnRef)\n");
        abort();
    }
}

static bool junctionLive(Router *r, JunctionRef *j)
{
    if (isPendingAction(r, JunctionRemove, j))
    {
        return false;
    }
    return inObstacles(r, j) || isPendingAction(r, JunctionAdd, j);
}

// Name of an object in a "new" list: the live object's id if it is live,
// else the id it had at the previous sync, else "?".
static std::string idNew(Scene& sc, const void *obj, bool isConn)
{
    if (isConn)
    {
        for (size_t i = 0; i < sc.conns.size(); ++i)
        {
            if (sc.conns[i] == obj) return std::to_string(sc.conns[i]->id());
        }
    }
    else
    {
        for (size_t i = 0; i < sc.juncs.size(); ++i)
        {
            if (sc.juncs[i] == obj) return std::to_string(sc.juncs[i]->id());
        }
    }
    std::map<const void *, unsigned>::const_iterator k = sc.knownIds.find(obj);
    if (k != sc.knownIds.end()) return std::to_string(k->second);
    return "?";
}

// Name of an object in a "deleted" list: the id it had at the previous
// sync, else the live object's id, else "?".
static std::string idDel(Scene& sc, const void *obj, bool isConn)
{
    std::map<const void *, unsigned>::const_iterator k = sc.knownIds.find(obj);
    if (k != sc.knownIds.end()) return std::to_string(k->second);
    return idNew(sc, obj, isConn);
}

template <class L>
static std::string listStr(Scene& sc, const L& l, bool isConn, bool deleted)
{
    std::string s = "[";
    bool first = true;
    for (typename L::const_iterator it = l.begin(); it != l.end(); ++it)
    {
        if (!first) s += " ";
        first = false;
        s += deleted ? idDel(sc, *it, isConn) : idNew(sc, *it, isConn);
    }
    return s + "]";
}

// After every command: drop dead objects from the live lists, append
// objects the router created, then record regenerated hyperedge object lists
// (objects in "new" lists are named by their live id, objects in "deleted"
// lists by the id they had at the previous sync).
static void sync(Scene& sc)
{
    Router *r = sc.r;
    if (!r) return;

    // Connectors: live iff in connRefs or with a queued ConnChange.
    {
        std::vector<ConnRef *> keep;
        for (size_t i = 0; i < sc.conns.size(); ++i)
        {
            // (a connector created in a transaction only joins connRefs
            // when its queued ConnChange is processed)
            if (inConnRefs(r, sc.conns[i]) || isPendingAction(r, ConnChange, sc.conns[i]))
            {
                keep.push_back(sc.conns[i]);
            }
        }
        sc.conns = keep;
        for (ConnRefList::const_iterator it = r->connRefs.begin();
                it != r->connRefs.end(); ++it)
        {
            if (std::find(sc.conns.begin(), sc.conns.end(), *it) == sc.conns.end())
            {
                sc.conns.push_back(*it);
                (*it)->setCallback(connCallback, (void *) (uintptr_t) (*it)->id());
            }
        }
    }
    // Junctions.
    {
        std::vector<JunctionRef *> keep;
        for (size_t i = 0; i < sc.juncs.size(); ++i)
        {
            if (junctionLive(r, sc.juncs[i])) keep.push_back(sc.juncs[i]);
        }
        sc.juncs = keep;
        for (ObstacleList::const_iterator it = r->m_obstacles.begin();
                it != r->m_obstacles.end(); ++it)
        {
            JunctionRef *j = dynamic_cast<JunctionRef *> (*it);
            if (j && std::find(sc.juncs.begin(), sc.juncs.end(), j) == sc.juncs.end() &&
                    !isPendingAction(r, JunctionRemove, j))
            {
                sc.juncs.push_back(j);
            }
        }
    }
    // Hyperedge improver / rerouter object lists: regenerated by every
    // rerouteAndCallbackConnectors() (improver lists only while an
    // improvement option is on), so they are printed after commands during
    // which a reroute ran (counted by the patched copy of router.cpp).
    if (e2eRerouteCount != sc.rerouteCount)
    {
        sc.rerouteCount = e2eRerouteCount;
        if (r->routingOption(improveHyperedgeRoutesMovingJunctions) ||
                r->routingOption(improveHyperedgeRoutesMovingAddingAndDeletingJunctions))
        {
            const HyperedgeImprover& h = r->m_hyperedge_improver;
            sc.pending.push_back("H nj=" + listStr(sc, h.m_new_junctions, false, false) +
                    " nc=" + listStr(sc, h.m_new_connectors, true, false) +
                    " dj=" + listStr(sc, h.m_deleted_junctions, false, true) +
                    " dc=" + listStr(sc, h.m_deleted_connectors, true, true) +
                    " cc=" + listStr(sc, h.m_changed_connectors, true, false));
        }
        const HyperedgeRerouter& h = r->m_hyperedge_rerouter;
        for (size_t i = 0; i < h.m_new_junctions_vector.size(); ++i)
        {
            sc.pending.push_back("R " + std::to_string(i) +
                    " nj=" + listStr(sc, h.m_new_junctions_vector[i], false, false) +
                    " nc=" + listStr(sc, h.m_new_connectors_vector[i], true, false) +
                    " dj=" + listStr(sc, h.m_deleted_junctions_vector[i], false, true) +
                    " dc=" + listStr(sc, h.m_deleted_connectors_vector[i], true, true));
        }
    }
    sc.knownIds.clear();
    for (size_t i = 0; i < sc.conns.size(); ++i)
    {
        sc.knownIds[sc.conns[i]] = sc.conns[i]->id();
    }
    for (size_t i = 0; i < sc.juncs.size(); ++i)
    {
        sc.knownIds[sc.juncs[i]] = sc.juncs[i]->id();
    }
}

static const char *endTypeStr(ConnEndType t)
{
    switch (t)
    {
    case ConnEndPoint: return "P";
    case ConnEndShapePin: return "S";
    case ConnEndJunction: return "J";
    default: return "E";
    }
}

static std::string idListStr(const IntList& l)
{
    std::string s = "[";
    for (IntList::const_iterator it = l.begin(); it != l.end(); ++it)
    {
        if (it != l.begin()) s += " ";
        s += std::to_string(*it);
    }
    return s + "]";
}

static std::string readFile(const std::string& path)
{
    std::string s;
    FILE *fp = fopen(path.c_str(), "r");
    if (!fp) return s;
    char buf[65536];
    size_t n;
    while ((n = fread(buf, 1, sizeof(buf), fp)) > 0)
    {
        s.append(buf, n);
    }
    fclose(fp);
    return s;
}

static void dump(Scene& sc, bool ret)
{
    Router *r = sc.r;
    std::vector<std::string> o;
    o.push_back("T " + std::to_string(sc.stage) + " ret=" + std::to_string((int) ret));
    {
        std::string s = "cb";
        for (size_t i = 0; i < g_callbacks.size(); ++i)
        {
            s += " " + std::to_string(g_callbacks[i]);
        }
        g_callbacks.clear();
        o.push_back(s);
    }
    for (size_t i = 0; i < sc.pending.size(); ++i)
    {
        o.push_back(sc.pending[i]);
    }
    sc.pending.clear();
    // Shapes (interpreter list order) with their pins (pin-set order).
    for (size_t i = 0; i < sc.shapes.size(); ++i)
    {
        ShapeRef *s = sc.shapes[i];
        o.push_back("S " + std::to_string(s->id()) + " " + polyStr(s->polygon()));
        for (ShapeConnectionPinSet::const_iterator p = s->m_connection_pins.begin();
                p != s->m_connection_pins.end(); ++p)
        {
            ShapeConnectionPin *pin = *p;
            o.push_back("  pin c=" + std::to_string(pin->m_class_id) + " " +
                    ptStr(pin->position()) + " d=" + std::to_string(pin->directions()) +
                    " x=" + std::to_string((int) pin->m_exclusive) + " $=" +
                    g17(pin->m_connection_cost));
        }
    }
    for (size_t i = 0; i < sc.juncs.size(); ++i)
    {
        JunctionRef *j = sc.juncs[i];
        o.push_back("J " + std::to_string(j->id()) + " " + ptStr(j->position()) +
                " f=" + std::to_string((int) j->positionFixed()));
    }
    for (size_t i = 0; i < sc.clusters.size(); ++i)
    {
        ClusterRef *k = sc.clusters[i];
        o.push_back("K " + std::to_string(k->id()) + " " +
                polyStr(k->rectangularPolygon()));
    }
    // Connectors in router order.
    for (ConnRefList::const_iterator it = r->connRefs.begin();
            it != r->connRefs.end(); ++it)
    {
        ConnRef *c = *it;
        std::pair<ConnEnd, ConnEnd> ends = c->endpointConnEnds();
        o.push_back("C " + std::to_string(c->id()) + " t=" +
                std::to_string((int) c->routingType()) + " f=" +
                std::to_string((int) c->hasFixedRoute()) + " src=" +
                endTypeStr(ends.first.type()) + ptStr(ends.first.position()) + " dst=" +
                endTypeStr(ends.second.type()) + ptStr(ends.second.position()));
        o.push_back("  raw " + polyStr(c->route()));
        o.push_back("  disp " + polyStr(c->displayRoute()));
    }
    o.push_back("Q ovl=" + std::to_string((int) r->existsOrthogonalSegmentOverlap(false)) +
            std::to_string((int) r->existsOrthogonalSegmentOverlap(true)) +
            " fovl=" + std::to_string((int) r->existsOrthogonalFixedSegmentOverlap(false)) +
            std::to_string((int) r->existsOrthogonalFixedSegmentOverlap(true)) +
            " touch=" + std::to_string((int) r->existsOrthogonalTouchingPaths()) +
            " cross=" + std::to_string(r->existsCrossings(false)) + "," +
            std::to_string(r->existsCrossings(true)) +
            " invalid=" + std::to_string((int) r->existsInvalidOrthogonalPaths()));
    // attachedConns / attachedShapes for (up to) the first three shapes
    // and the first junction.
    {
        std::vector<unsigned> ids;
        for (size_t i = 0; i < sc.shapes.size() && i < 3; ++i)
        {
            ids.push_back(sc.shapes[i]->id());
        }
        if (!sc.juncs.empty()) ids.push_back(sc.juncs[0]->id());
        for (size_t i = 0; i < ids.size(); ++i)
        {
            std::string s = "A " + std::to_string(ids[i]);
            for (unsigned t = 1; t <= 3; ++t)
            {
                IntList conns, shapes;
                r->attachedConns(conns, ids[i], t);
                r->attachedShapes(shapes, ids[i], t);
                s += " " + idListStr(conns) + idListStr(shapes);
            }
            o.push_back(s);
        }
    }
    // outputDiagramText (upstream %g format), via a temporary file.
    {
        char base[128];
        snprintf(base, sizeof(base), "/tmp/e2e_fuzz_diag_%d", (int) getpid());
        r->outputDiagramText(base);
        std::string path = std::string(base) + ".txt";
        std::string text = readFile(path);
        unlink(path.c_str());
        size_t start = 0;
        while (start < text.size())
        {
            size_t nl = text.find('\n', start);
            if (nl == std::string::npos) nl = text.size();
            o.push_back("D " + text.substr(start, nl - start));
            start = nl + 1;
        }
    }
    sc.stages.push_back(o);
    sc.stage++;
}

//============================================================================
// Interpreter.
//============================================================================

static std::string word(std::istringstream& in)
{
    std::string t;
    in >> t;
    return t;
}

static double num(std::istringstream& in)
{
    std::string t = word(in);
    if (t.empty())
    {
        fprintf(stderr, "missing number\n");
        abort();
    }
    return strtod(t.c_str(), nullptr);
}

static unsigned unum(std::istringstream& in)
{
    std::string t = word(in);
    if (t.empty())
    {
        fprintf(stderr, "missing unsigned\n");
        abort();
    }
    return (unsigned) strtoul(t.c_str(), nullptr, 10);
}

// Resolves a list reference; returns false if the list is empty.
static bool ref(std::istringstream& in, size_t n, size_t& idx)
{
    std::string t = word(in);
    if (t.empty())
    {
        fprintf(stderr, "missing reference\n");
        abort();
    }
    if (n == 0) return false;
    if (t == "L")
    {
        idx = n - 1;
    }
    else
    {
        idx = (size_t) (strtoull(t.c_str(), nullptr, 10) % n);
    }
    return true;
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

static bool hasPinClass(ShapeRef *s, unsigned cls)
{
    for (ShapeConnectionPinSet::const_iterator p = s->m_connection_pins.begin();
            p != s->m_connection_pins.end(); ++p)
    {
        if ((*p)->m_class_id == cls) return true;
    }
    return false;
}

// Whether the connectors reachable from junction `root` through junctions
// (live connectors, by their endpoint ConnEnds) form a tree.
static bool hyperedgeIsTree(Scene& sc, JunctionRef *root)
{
    std::set<JunctionRef *> seenJ;
    std::set<ConnRef *> seenC;
    std::vector<JunctionRef *> stack;
    stack.push_back(root);
    seenJ.insert(root);
    while (!stack.empty())
    {
        JunctionRef *j = stack.back();
        stack.pop_back();
        for (size_t k = 0; k < sc.conns.size(); ++k)
        {
            ConnRef *c = sc.conns[k];
            std::pair<ConnEnd, ConnEnd> ends = c->endpointConnEnds();
            JunctionRef *a = ends.first.junction();
            JunctionRef *b = ends.second.junction();
            if (a != j && b != j) continue;
            if (a == b) return false;
            if (seenC.count(c)) continue;
            seenC.insert(c);
            JunctionRef *o = (a == j) ? b : a;
            if (!o) continue;
            if (seenJ.count(o)) return false;
            seenJ.insert(o);
            stack.push_back(o);
        }
    }
    return true;
}

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
        size_t i;
        bool ok = ref(in, sc.shapes.size(), i);
        unsigned cls = unum(in);
        if (!ok || !hasPinClass(sc.shapes[i], cls))
        {
            // (a ConnEnd naming a pin class the shape does not have makes
            // upstream dereference a null vertex, connend.cpp:292)
            return ConnEnd(Point(0, 0), ConnDirAll);
        }
        return ConnEnd(sc.shapes[i], cls);
    }
    else if (k == "J")
    {
        size_t i;
        if (!ref(in, sc.juncs.size(), i)) return ConnEnd(Point(0, 0), ConnDirAll);
        return ConnEnd(sc.juncs[i]);
    }
    fprintf(stderr, "bad end %s\n", k.c_str());
    abort();
}

static ShapeConnectionPin *pinAt(ShapeRef *s, std::istringstream& in)
{
    size_t i;
    if (!ref(in, s->m_connection_pins.size(), i)) return nullptr;
    ShapeConnectionPinSet::iterator it = s->m_connection_pins.begin();
    std::advance(it, i);
    return *it;
}

static void runCommandInner(Scene& sc, const std::string& line)
{
    std::istringstream in(line);
    std::string cmd = word(in);
    if (cmd.empty() || cmd[0] == '#')
    {
        return;
    }
    if (cmd == "router")
    {
        sc.r = new Router(unum(in));
        return;
    }
    Router *r = sc.r;
    size_t i;
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
    else if (cmd == "settings")
    {
        r->registerSettingsChange();
    }
    else if (cmd == "txn")
    {
        r->setTransactionUse(unum(in) != 0);
    }
    else if (cmd == "process")
    {
        bool ret = r->processTransaction();
        sync(sc);
        dump(sc, ret);
    }
    else if (cmd == "markmoved")
    {
        // (moving an obstacle with a queued removal uses freed memory)
        for (ActionInfoList::const_iterator it = r->actionList.begin();
                it != r->actionList.end(); ++it)
        {
            if (it->type == ShapeRemove || it->type == JunctionRemove) return;
        }
        r->markAllObstaclesAsMoved();
    }
    else if (cmd == "rect")
    {
        unsigned id = unum(in);
        double x1 = num(in);
        double y1 = num(in);
        double x2 = num(in);
        double y2 = num(in);
        Rectangle rect(Point(x1, y1), Point(x2, y2));
        sc.shapes.push_back(new ShapeRef(r, rect, id));
    }
    else if (cmd == "poly")
    {
        unsigned id = unum(in);
        Polygon poly = readPoly(in);
        sc.shapes.push_back(new ShapeRef(r, poly, id));
    }
    else if (cmd == "moveby")
    {
        if (!ref(in, sc.shapes.size(), i)) return;
        double dx = num(in);
        double dy = num(in);
        r->moveShape(sc.shapes[i], dx, dy);
    }
    else if (cmd == "moveto")
    {
        if (!ref(in, sc.shapes.size(), i)) return;
        bool first = unum(in) != 0;
        Polygon poly = readPoly(in);
        // (a polygon with a different number of points violates
        // Obstacle::setNewPoly()'s precondition; the port ignores such
        // moves, upstream's release build corrupts the shape's vertices)
        if (poly.size() != sc.shapes[i]->polygon().size()) return;
        r->moveShape(sc.shapes[i], poly, first);
    }
    else if (cmd == "delshape")
    {
        if (!ref(in, sc.shapes.size(), i)) return;
        ShapeRef *s = sc.shapes[i];
        // (upstream asserts / frees twice for a shape with a queued add)
        if (isPendingAction(r, ShapeAdd, s) || queuedConnEndUses(r, s)) return;
        sc.shapes.erase(sc.shapes.begin() + i);
        r->deleteShape(s);
    }
    else if (cmd == "pin")
    {
        if (!ref(in, sc.shapes.size(), i)) return;
        unsigned cls = unum(in);
        double xo = num(in);
        double yo = num(in);
        unsigned prop = unum(in);
        double inside = num(in);
        unsigned dirs = unum(in);
        new ShapeConnectionPin(sc.shapes[i], cls, xo, yo, prop != 0, inside,
                (ConnDirFlags) dirs);
    }
    else if (cmd == "pinexcl" || cmd == "pincost" || cmd == "delpin")
    {
        if (!ref(in, sc.shapes.size(), i)) return;
        ShapeConnectionPin *pin = pinAt(sc.shapes[i], in);
        if (!pin) return;
        if (cmd == "pinexcl")
        {
            pin->setExclusive(unum(in) != 0);
        }
        else if (cmd == "pincost")
        {
            pin->setConnectionCost(num(in));
        }
        else
        {
            // Keep the last pin of a class: a ConnEnd naming a class without
            // pins dereferences a null vertex upstream (connend.cpp:292).
            unsigned same = 0;
            for (ShapeConnectionPinSet::iterator p = sc.shapes[i]->m_connection_pins.begin();
                    p != sc.shapes[i]->m_connection_pins.end(); ++p)
            {
                if ((*p)->m_class_id == pin->m_class_id) ++same;
            }
            if (same < 2) return;
            delete pin;
        }
    }
    else if (cmd == "transform")
    {
        if (!ref(in, sc.shapes.size(), i)) return;
        sc.shapes[i]->transformConnectionPinPositions(
                (ShapeTransformationType) unum(in));
        const ShapeConnectionPinSet& set = sc.shapes[i]->m_connection_pins;
        ShapeConnectionPinSet::const_iterator prev = set.begin();
        for (ShapeConnectionPinSet::const_iterator it = set.begin(); it != set.end(); ++it)
        {
            if (it != prev && **it < **prev && !g_pinsetUnsorted)
            {
                g_pinsetUnsorted = true;
                fprintf(stderr, "ERROR: pinset-unsorted (transformConnectionPinPositions)\n");
            }
            prev = it;
        }
    }
    else if (cmd == "junction")
    {
        unsigned id = unum(in);
        double x = num(in);
        double y = num(in);
        unsigned fixed = unum(in);
        JunctionRef *j = new JunctionRef(r, Point(x, y), id);
        if (fixed) j->setPositionFixed(true);
        sc.juncs.push_back(j);
    }
    else if (cmd == "fixj")
    {
        if (!ref(in, sc.juncs.size(), i)) return;
        sc.juncs[i]->setPositionFixed(unum(in) != 0);
    }
    else if (cmd == "movejby")
    {
        if (!ref(in, sc.juncs.size(), i)) return;
        double dx = num(in);
        double dy = num(in);
        r->moveJunction(sc.juncs[i], dx, dy);
    }
    else if (cmd == "movejto")
    {
        if (!ref(in, sc.juncs.size(), i)) return;
        double x = num(in);
        double y = num(in);
        r->moveJunction(sc.juncs[i], Point(x, y));
    }
    else if (cmd == "deljunc")
    {
        if (!ref(in, sc.juncs.size(), i)) return;
        JunctionRef *j = sc.juncs[i];
        if (isPendingAction(r, JunctionAdd, j) || queuedConnEndUses(r, j)) return;
        sc.juncs.erase(sc.juncs.begin() + i);
        r->deleteJunction(j);
    }
    else if (cmd == "mergej")
    {
        if (!ref(in, sc.juncs.size(), i)) return;
        sc.juncs[i]->removeJunctionAndMergeConnectors();
    }
    else if (cmd == "conn")
    {
        unsigned id = unum(in);
        ConnEnd src = readEnd(sc, in);
        ConnEnd dst = readEnd(sc, in);
        ConnRef *c = new ConnRef(r, src, dst, id);
        if (std::find(sc.conns.begin(), sc.conns.end(), c) == sc.conns.end())
        {
            sc.conns.push_back(c);
            c->setCallback(connCallback, (void *) (uintptr_t) c->id());
        }
    }
    else if (cmd == "src" || cmd == "dst")
    {
        if (!ref(in, sc.conns.size(), i)) return;
        staleConnCheck(sc, sc.conns[i]);
        ConnRef *c = sc.conns[i];
        ConnEnd end = readEnd(sc, in);
        if (cmd == "src") c->setSourceEndpoint(end);
        else c->setDestEndpoint(end);
    }
    else if (cmd == "type")
    {
        if (!ref(in, sc.conns.size(), i)) return;
        staleConnCheck(sc, sc.conns[i]);
        sc.conns[i]->setRoutingType((ConnType) unum(in));
    }
    else if (cmd == "hate")
    {
        if (!ref(in, sc.conns.size(), i)) return;
        staleConnCheck(sc, sc.conns[i]);
        sc.conns[i]->setHateCrossings(unum(in) != 0);
    }
    else if (cmd == "invalid")
    {
        if (!ref(in, sc.conns.size(), i)) return;
        staleConnCheck(sc, sc.conns[i]);
        sc.conns[i]->makePathInvalid();
    }
    else if (cmd == "delconn")
    {
        if (!ref(in, sc.conns.size(), i)) return;
        staleConnCheck(sc, sc.conns[i]);
        ConnRef *c = sc.conns[i];
        sc.conns.erase(sc.conns.begin() + i);
        r->deleteConnector(c);
    }
    else if (cmd == "cps")
    {
        if (!ref(in, sc.conns.size(), i)) return;
        staleConnCheck(sc, sc.conns[i]);
        unsigned n = unum(in);
        std::vector<Checkpoint> cps;
        for (unsigned k = 0; k < n; ++k)
        {
            double x = num(in);
            double y = num(in);
            unsigned ad = unum(in);
            unsigned dd = unum(in);
            cps.push_back(Checkpoint(Point(x, y), (ConnDirFlags) ad, (ConnDirFlags) dd));
        }
        sc.conns[i]->setRoutingCheckpoints(cps);
    }
    else if (cmd == "fixroute")
    {
        if (!ref(in, sc.conns.size(), i)) return;
        staleConnCheck(sc, sc.conns[i]);
        PolyLine route = readPoly(in);
        sc.conns[i]->setFixedRoute(route);
    }
    else if (cmd == "fixexist")
    {
        if (!ref(in, sc.conns.size(), i)) return;
        staleConnCheck(sc, sc.conns[i]);
        if (sc.conns[i]->route().size() >= 2)
        {
            sc.conns[i]->setFixedExistingRoute();
        }
    }
    else if (cmd == "clearfix")
    {
        if (!ref(in, sc.conns.size(), i)) return;
        staleConnCheck(sc, sc.conns[i]);
        sc.conns[i]->clearFixedRoute();
    }
    else if (cmd == "split")
    {
        if (!ref(in, sc.conns.size(), i)) return;
        staleConnCheck(sc, sc.conns[i]);
        unsigned seg = unum(in);
        ConnRef *c = sc.conns[i];
        // Upstream dereferences m_dst_connend, which is null when the
        // destination is a free point: skip those.
        if (c->m_display_route.size() > seg && !c->m_dst_connend) return;
        c->splitAtSegment(seg);
    }
    else if (cmd == "cluster")
    {
        unsigned id = unum(in);
        Polygon poly = readPoly(in);
        sc.clusters.push_back(new ClusterRef(r, poly, id));
    }
    else if (cmd == "movecluster")
    {
        if (!ref(in, sc.clusters.size(), i)) return;
        Polygon poly = readPoly(in);
        r->deleteCluster(sc.clusters[i]);
        sc.clusters[i]->setNewPoly(poly);
        r->addCluster(sc.clusters[i]);
    }
    else if (cmd == "delcluster")
    {
        if (!ref(in, sc.clusters.size(), i)) return;
        ClusterRef *k = sc.clusters[i];
        sc.clusters.erase(sc.clusters.begin() + i);
        r->deleteCluster(k);
    }
    else if (cmd == "regj")
    {
        if (!ref(in, sc.juncs.size(), i)) return;
        // (a cyclic hyperedge -- e.g. a connector from a junction to itself
        // -- makes upstream recurse forever / use freed memory)
        if (!hyperedgeIsTree(sc, sc.juncs[i])) return;
        r->hyperedgeRerouter()->registerHyperedgeForRerouting(sc.juncs[i]);
    }
    else if (cmd == "regends")
    {
        // Terminals must be existing pin classes of distinct shapes: free
        // points get vertices without orthogonal visibility and coincident
        // terminals collapse, and upstream then dereferences a null MTST
        // root (hyperedge.cpp:337).  Otherwise the command is skipped.
        unsigned n = unum(in);
        ConnEndList ends;
        std::set<ShapeRef *> used;
        bool ok = true;
        for (unsigned k = 0; k < n; ++k)
        {
            std::string kind = word(in);
            if (kind != "S")
            {
                ok = false;
                if (kind == "P") { num(in); num(in); unum(in); }
                else word(in);
                continue;
            }
            bool have = ref(in, sc.shapes.size(), i);
            unsigned cls = unum(in);
            if (!have || !hasPinClass(sc.shapes[i], cls) || used.count(sc.shapes[i]))
            {
                ok = false;
                continue;
            }
            used.insert(sc.shapes[i]);
            ends.push_back(ConnEnd(sc.shapes[i], cls));
        }
        // (with two terminals the MTST has no junction: null root)
        if (!ok || ends.size() < 3) return;
        r->hyperedgeRerouter()->registerHyperedgeForRerouting(ends);
    }
    else
    {
        fprintf(stderr, "unknown command: %s\n", line.c_str());
        abort();
    }
    sync(sc);
}

// Upstream's immediate mode (setTransactionUse(false)) is unusable for
// these commands:
//  * a ConnRef created with ends routes before m_reroute_flag_ptr is set,
//    so a later EdgeInf::alertConns() writes through a null pointer (conn,
//    split, and the connector merging of mergej);
//  * moving/deleting an obstacle with attached connectors calls
//    modifyConnector() from inside processActions(), which re-enters
//    processTransaction() on the same action list until the stack
//    overflows (moveby, moveto, movejby, movejto, delshape, deljunc,
//    markmoved);
//  * Obstacle::addConnectionPin() processes the transaction before the new
//    pin has its vertex, and assignPinVisibilityTo() then dereferences the
//    null vertex if a connector uses that pin class (pin);
//  * actions left queued by a transaction (hyperedge rerouting queues the
//    removal of the junctions it replaces) and processed in immediate mode
//    re-enter processTransaction() from ~Obstacle (pin removal) on the
//    same action list; so before any command in immediate mode, queued
//    actions are first processed in a transaction (no dump);
//  * any transaction in which hyperedge improvement runs: the improver
//    creates JunctionRefs, whose pin creation (addConnectionPin ->
//    modifyConnectionPin) or deletion runs a nested processTransaction()
//    inside rerouteAndCallbackConnectors(), which improves again, ... --
//    upstream recurses until memory or stack are exhausted (seen as
//    "#exclude memory" / stack overflows before this rule).
// In immediate mode the interpreter therefore runs every command as a
// one-command transaction (setTransactionUse(true); command;
// processTransaction(); setTransactionUse(false)), which is what immediate
// mode means; "nowrap" scenes keep upstream's raw behaviour.
static bool unsafeImmediate(const std::string& cmd)
{
    return cmd != "txn" && cmd != "router" && cmd != "nowrap" && !cmd.empty() &&
            cmd[0] != '#';
}

static void runCommand(Scene& sc, const std::string& line)
{
    std::istringstream in(line);
    std::string cmd = word(in);
    if (sc.r && !sc.noWrap && !sc.r->transactionUse())
    {
        for (int k = 0; k < 3 && !sc.r->actionList.empty(); ++k)
        {
            sc.r->setTransactionUse(true);
            sc.r->processTransaction();
            sc.r->setTransactionUse(false);
        }
    }
    if (cmd == "nowrap")
    {
        // Disables the immediate-mode wrapping below for the rest of the
        // scene (regression scenes that need upstream's raw behaviour).
        sc.noWrap = true;
        return;
    }
    bool wrap = sc.r && !sc.noWrap && !sc.r->transactionUse() && unsafeImmediate(cmd);
    if (wrap) sc.r->setTransactionUse(true);
    runCommandInner(sc, line);
    if (wrap)
    {
        sc.r->processTransaction();
        sc.r->setTransactionUse(false);
        sync(sc);
    }
}

// Runs a scene; returns the output lines (stage dumps, hashed if large).
static std::vector<std::string> execScene(const std::vector<std::string>& dsl)
{
    Scene sc;
    sc.r = nullptr;
    sc.stage = 0;
    sc.rerouteCount = e2eRerouteCount;
    sc.noWrap = false;
    g_callbacks.clear();
    g_pinsetUnsorted = false;
    for (size_t i = 0; i < dsl.size(); ++i)
    {
        runCommand(sc, dsl[i]);
    }
    if (sc.r)
    {
        // Process queued deletions (e.g. junctions removed by hyperedge
        // rerouting) before ~Router: upstream's destructor otherwise calls
        // removeFromGraph() on such inactive junctions.
        sc.r->setTransactionUse(true);
        sc.r->processTransaction();
    }
    delete sc.r;
    size_t total = 0;
    for (size_t i = 0; i < sc.stages.size(); ++i)
    {
        total += sc.stages[i].size();
    }
    std::vector<std::string> res;
    if (g_pinsetUnsorted)
    {
        res.push_back("#pinset-unsorted");
    }
    for (size_t i = 0; i < sc.stages.size(); ++i)
    {
        const std::vector<std::string>& st = sc.stages[i];
        if (g_full || total <= kFullSceneLimit)
        {
            res.insert(res.end(), st.begin(), st.end());
        }
        else
        {
            char buf[128];
            snprintf(buf, sizeof(buf), " lines=%u fnv=%016llx", (unsigned) st.size(),
                    (unsigned long long) fnv(st));
            res.push_back(st[0] + buf);
        }
    }
    return res;
}

//============================================================================
// Scene execution in a forked child.
//============================================================================

static void crashHandler(int sig)
{
    if (getenv("E2E_INPROC"))
    {
        void *frames[64];
        int n = backtrace(frames, 64);
        backtrace_symbols_fd(frames, n, 2);
    }
    _exit(128 + sig);
}

static void installCrashHandlers(void)
{
    struct rlimit noCore;
    noCore.rlim_cur = 0;
    noCore.rlim_max = 0;
    setrlimit(RLIMIT_CORE, &noCore);
#ifndef E2E_SANITIZE
    signal(SIGABRT, crashHandler);
    signal(SIGSEGV, crashHandler);
    signal(SIGBUS, crashHandler);
    signal(SIGFPE, crashHandler);
#endif
}

static std::string runSceneChild(const std::vector<std::string>& dsl)
{
    if (getenv("E2E_INPROC"))
    {
        installCrashHandlers();
        std::vector<std::string> res = execScene(dsl);
        std::string all;
        for (size_t i = 0; i < res.size(); ++i) all += res[i] + "\n";
        return all;
    }
    int timeout = getenv("E2E_TIMEOUT") ? atoi(getenv("E2E_TIMEOUT")) : 20;
    int fds[2];
    if (pipe(fds) != 0)
    {
        abort();
    }
    FILE *errFile = tmpfile();
    fflush(stdout);
    fflush(stderr);
    pid_t pid = fork();
    if (pid == 0)
    {
        close(fds[0]);
        dup2(fileno(errFile), 2);
        // libavoid prints warnings on stdout.
        int devnull = open("/dev/null", 1);
        if (devnull >= 0) dup2(devnull, 1);
        alarm(timeout);
        installCrashHandlers();
        std::vector<std::string> res = execScene(dsl);
        std::string all;
        for (size_t i = 0; i < res.size(); ++i) all += res[i] + "\n";
        size_t off = 0;
        while (off < all.size())
        {
            ssize_t w = write(fds[1], all.data() + off, all.size() - off);
            if (w <= 0) _exit(3);
            off += (size_t) w;
        }
        close(fds[1]);
        _exit(0);
    }
    close(fds[1]);
    std::string result;
    char buf[65536];
    bool memoryKilled = false;
    time_t start = time(nullptr);
    for (;;)
    {
        struct pollfd pfd;
        pfd.fd = fds[0];
        pfd.events = POLLIN;
        int pr = poll(&pfd, 1, 200);
        if (pr > 0)
        {
            ssize_t n = read(fds[0], buf, sizeof(buf));
            if (n <= 0) break;
            result.append(buf, (size_t) n);
        }
        if (time(nullptr) - start > timeout + 5)
        {
            kill(pid, SIGKILL);
        }
        // RSS watchdog (all builds; the bump builds are also capped by
        // their arena size): a child above E2E_RSS_MB (default 4096) is
        // killed and the scene reported as "#crash memory".
        {
            char path[64];
            snprintf(path, sizeof(path), "/proc/%d/statm", (int) pid);
            FILE *fp = fopen(path, "r");
            if (fp)
            {
                unsigned long sizePages = 0, rssPages = 0;
                if (fscanf(fp, "%lu %lu", &sizePages, &rssPages) == 2)
                {
                    unsigned long capMb = getenv("E2E_RSS_MB") ? atol(getenv("E2E_RSS_MB")) : 4096;
                    if (rssPages * (unsigned long) sysconf(_SC_PAGESIZE) > (capMb << 20))
                    {
                        memoryKilled = true;
                        kill(pid, SIGKILL);
                    }
                }
                fclose(fp);
            }
        }
    }
    close(fds[0]);
    int status = 0;
    waitpid(pid, &status, 0);
    std::string lastErr;
    rewind(errFile);
    char line[4096];
    std::string frame;
    while (fgets(line, sizeof(line), errFile))
    {
        if (lastErr.empty() && (strstr(line, "Assertion") || strstr(line, "terminate") ||
                strstr(line, "what()") || strstr(line, "ERROR: AddressSanitizer") ||
                strstr(line, "runtime error") || strstr(line, "ERROR: ")))
        {
            lastErr = line;
        }
        else if (!lastErr.empty() && frame.empty() && strstr(line, " in ") &&
                strstr(line, "libavoid/") && !strstr(line, "e2e_fuzz.cpp"))
        {
            // First sanitizer stack frame inside libavoid: "fn file:line".
            const char *in = strstr(line, " in ") + 4;
            const char *lib = strstr(line, "libavoid/");
            std::string fn(in);
            size_t sp = fn.find(" /");
            if (sp != std::string::npos) fn = fn.substr(0, sp);
            size_t paren = fn.find('(');
            if (paren != std::string::npos) fn = fn.substr(0, paren);
            std::string loc(lib);
            while (!loc.empty() && (loc.back() == '\n' || loc.back() == '\r')) loc.pop_back();
            frame = " @ " + fn + " " + loc;
        }
    }
    fclose(errFile);
    while (!lastErr.empty() && (lastErr[lastErr.size() - 1] == '\n' ||
            lastErr[lastErr.size() - 1] == '\r'))
    {
        lastErr.erase(lastErr.size() - 1);
    }
    size_t slash = lastErr.find("libavoid/");
    if (slash != std::string::npos)
    {
        lastErr = lastErr.substr(slash);
    }
    // Strip addresses / pids so that messages are stable.
    {
        std::string s;
        for (size_t k = 0; k < lastErr.size(); ++k)
        {
            if (lastErr.compare(k, 2, "0x") == 0)
            {
                s += "0x";
                k += 2;
                while (k < lastErr.size() && isxdigit((unsigned char) lastErr[k])) ++k;
                --k;
                continue;
            }
            s += lastErr[k];
        }
        lastErr = s;
        size_t eq = lastErr.find("==");
        if (eq != std::string::npos)
        {
            size_t eq2 = lastErr.find("==", eq + 2);
            if (eq2 != std::string::npos) lastErr.erase(eq, eq2 + 2 - eq);
        }
    }
    lastErr += frame;
    if (!WIFEXITED(status) || WEXITSTATUS(status) != 0)
    {
        char b[128];
        if (memoryKilled || (WIFEXITED(status) && WEXITSTATUS(status) == 9))
        {
            return "#crash memory\n";
        }
        if (WIFEXITED(status))
        {
            snprintf(b, sizeof(b), "#crash exit=%d", WEXITSTATUS(status));
        }
        else if (WTERMSIG(status) == SIGALRM || WTERMSIG(status) == SIGKILL)
        {
            snprintf(b, sizeof(b), "#crash timeout");
        }
        else
        {
            snprintf(b, sizeof(b), "#crash signal=%d", WTERMSIG(status));
        }
        return std::string(b) + (lastErr.empty() ? "" : " " + lastErr) + "\n";
    }
    return result;
}

static void runScene(const std::string& name, const std::vector<std::string>& dsl)
{
    printf("#scene %s\n", name.c_str());
    for (size_t i = 0; i < dsl.size(); ++i)
    {
        printf("%s\n", dsl[i].c_str());
    }
    printf("#out\n");
    fflush(stdout);
    printf("%s", runSceneChild(dsl).c_str());
    printf("#end\n");
    fflush(stdout);
}

//============================================================================
// Scene generator.
//============================================================================

static uint64_t g_state = 0;

static uint64_t next_u64()
{
    g_state = g_state * 6364136223846793005ULL + 1442695040888963407ULL;
    return g_state;
}

// Uniform integer in [0, n).
static unsigned rnd(unsigned n)
{
    if (n == 0) return 0;
    return (unsigned) ((next_u64() >> 33) % n);
}

static bool chance(unsigned percent)
{
    return rnd(100) < percent;
}

static std::string f(double v)
{
    return g17(v);
}

struct GShape
{
    double x1, y1, x2, y2;          // bounding box
    std::vector<double> xs, ys;     // polygon
    std::vector<unsigned> classes;  // pin classes (may be stale after delpin)
    bool addedInBatch;
};

struct Gen
{
    std::vector<std::string> L;
    std::vector<GShape> shapes;
    unsigned nJ;          // approximate number of junctions
    unsigned nC;          // approximate number of connectors
    unsigned nK;          // clusters (exact)
    unsigned flags;
    bool grid;
    double unit;
    double W, H;
    unsigned nextId;
    bool txn;
    bool inBatch;
    bool allowPoly;
    bool hyperOpts;
    bool hyper;           // scene registers hyperedges for rerouting
};

static double coord(Gen& g, double lo, double hi)
{
    if (hi < lo) std::swap(lo, hi);
    if (g.grid)
    {
        unsigned steps = (unsigned) ((hi - lo) / g.unit);
        return lo + g.unit * rnd(steps + 1);
    }
    double v = lo + (hi - lo) * (rnd(1000001) / 1000000.0);
    if (chance(20)) v = std::floor(v) + rnd(3) / 3.0;
    return v;
}

static std::string newId(Gen& g)
{
    if (chance(40)) return "0";
    g.nextId += 1000 + rnd(1000);
    return std::to_string(g.nextId);
}

static unsigned randDirs()
{
    // ConnDirNone on a free point gives it no visibility at all; rare,
    // because hyperedge rerouting with such an (unreachable) terminal
    // dereferences a null MTST root upstream (hyperedge.cpp:337).
    if (rnd(50) == 0) return 0;
    unsigned k = rnd(10);
    if (k < 4) return 15;
    if (k < 8) return 1u << rnd(4);
    return rnd(16);
}

static std::string ref1000()
{
    if (chance(15)) return "L";
    return std::to_string(rnd(1000));
}

// A convex polygon (points in the same rotational order as Rectangle()).
static void genPolygon(Gen& g, double cx, double cy, double rx, double ry,
        std::vector<double>& xs, std::vector<double>& ys)
{
    unsigned n = 3 + rnd(6);
    double phase = rnd(360) * M_PI / 180.0;
    if (g.grid && chance(50)) phase = 0;
    for (unsigned k = 0; k < n; ++k)
    {
        double a = phase + 2 * M_PI * k / n;
        double x = cx + rx * cos(a);
        double y = cy + ry * sin(a);
        if (g.grid)
        {
            x = std::round(x / g.unit) * g.unit;
            y = std::round(y / g.unit) * g.unit;
        }
        if (!xs.empty() && x == xs.back() && y == ys.back()) continue;
        if (!xs.empty() && x == xs[0] && y == ys[0]) continue;
        xs.push_back(x);
        ys.push_back(y);
    }
    if (xs.size() < 3)
    {
        xs.clear(); ys.clear();
        xs.push_back(cx + rx); ys.push_back(cy - ry);
        xs.push_back(cx + rx); ys.push_back(cy + ry);
        xs.push_back(cx - rx); ys.push_back(cy + ry);
    }
}

static std::string polyText(const std::vector<double>& xs, const std::vector<double>& ys)
{
    std::string s = std::to_string(xs.size());
    for (size_t k = 0; k < xs.size(); ++k)
    {
        s += " " + f(xs[k]) + " " + f(ys[k]);
    }
    return s;
}

// Picks a box for a new shape: random, touching or nested in an existing
// one, or overlapping.
static void genBox(Gen& g, double& x1, double& y1, double& x2, double& y2)
{
    double w = coord(g, 10, 90);
    double h = coord(g, 10, 70);
    if (w < 4) w = 4;
    if (h < 4) h = 4;
    unsigned mode = rnd(10);
    if (!g.shapes.empty() && mode < 2)
    {
        // Touching an existing shape.
        const GShape& o = g.shapes[rnd((unsigned) g.shapes.size())];
        if (chance(50))
        {
            x1 = o.x2;
            y1 = coord(g, o.y1 - h, o.y2);
        }
        else
        {
            y1 = o.y2;
            x1 = coord(g, o.x1 - w, o.x2);
        }
    }
    else if (!g.shapes.empty() && mode < 3)
    {
        // Nested inside an existing shape (if big enough).
        const GShape& o = g.shapes[rnd((unsigned) g.shapes.size())];
        w = std::max(2.0, (o.x2 - o.x1) / 3);
        h = std::max(2.0, (o.y2 - o.y1) / 3);
        if (g.grid)
        {
            w = std::max(g.unit, std::round(w / g.unit) * g.unit);
            h = std::max(g.unit, std::round(h / g.unit) * g.unit);
        }
        x1 = o.x1 + w;
        y1 = o.y1 + h;
    }
    else if (!g.shapes.empty() && mode < 4)
    {
        // Overlapping an existing shape.
        const GShape& o = g.shapes[rnd((unsigned) g.shapes.size())];
        x1 = coord(g, o.x1 - w / 2, o.x2 - w / 2);
        y1 = coord(g, o.y1 - h / 2, o.y2 - h / 2);
    }
    else
    {
        x1 = coord(g, 0, g.W);
        y1 = coord(g, 0, g.H);
    }
    x2 = x1 + w;
    y2 = y1 + h;
}

static void genPin(Gen& g, unsigned si)
{
    GShape& s = g.shapes[si];
    unsigned cls = 1 + rnd(4);
    bool prop = chance(75);
    double xo, yo;
    static const double attach[] = { 0, 0.5, 1, 0.25, 0.75 };
    if (prop)
    {
        xo = attach[rnd(5)];
        yo = attach[rnd(5)];
        if (chance(15)) xo = rnd(1001) / 1000.0;
        if (chance(15)) yo = rnd(1001) / 1000.0;
    }
    else
    {
        xo = coord(g, 0, s.x2 - s.x1);
        yo = coord(g, 0, s.y2 - s.y1);
        if (chance(30)) xo = 0;
        if (chance(30)) yo = s.y2 - s.y1;
    }
    double inside = chance(60) ? 0 : (chance(50) ? 5 : rnd(11));
    unsigned dirs = chance(40) ? 0 : randDirs();
    g.L.push_back("pin " + std::to_string(si) + " " + std::to_string(cls) + " " + f(xo) +
            " " + f(yo) + " " + std::to_string((int) prop) + " " + f(inside) + " " +
            std::to_string(dirs));
    if (std::find(s.classes.begin(), s.classes.end(), cls) == s.classes.end())
    {
        s.classes.push_back(cls);
    }
    if (chance(30))
    {
        g.L.push_back("pinexcl " + std::to_string(si) + " " + std::to_string(rnd(100)) +
                " " + std::to_string(rnd(2)));
    }
    if (chance(20))
    {
        g.L.push_back("pincost " + std::to_string(si) + " " + std::to_string(rnd(100)) +
                " " + f(chance(50) ? 50 : rnd(400)));
    }
}

static void genShape(Gen& g)
{
    double x1, y1, x2, y2;
    genBox(g, x1, y1, x2, y2);
    GShape s;
    s.addedInBatch = g.inBatch;
    if (g.allowPoly && chance(25))
    {
        std::vector<double> xs, ys;
        genPolygon(g, (x1 + x2) / 2, (y1 + y2) / 2, (x2 - x1) / 2, (y2 - y1) / 2, xs, ys);
        g.L.push_back("poly " + newId(g) + " " + polyText(xs, ys));
        s.xs = xs;
        s.ys = ys;
        s.x1 = *std::min_element(xs.begin(), xs.end());
        s.x2 = *std::max_element(xs.begin(), xs.end());
        s.y1 = *std::min_element(ys.begin(), ys.end());
        s.y2 = *std::max_element(ys.begin(), ys.end());
    }
    else
    {
        g.L.push_back("rect " + newId(g) + " " + f(x1) + " " + f(y1) + " " + f(x2) + " " +
                f(y2));
        s.x1 = x1; s.y1 = y1; s.x2 = x2; s.y2 = y2;
        double rx[] = { x2, x2, x1, x1 }, ry[] = { y1, y2, y2, y1 };
        s.xs.assign(rx, rx + 4);
        s.ys.assign(ry, ry + 4);
    }
    g.shapes.push_back(s);
    unsigned nPins = chance(50) ? rnd(5) : 0;
    for (unsigned p = 0; p < nPins; ++p)
    {
        genPin(g, (unsigned) g.shapes.size() - 1);
    }
}

static std::string genPointEnd(Gen& g)
{
    double x, y;
    unsigned k = rnd(4);
    if (!g.shapes.empty() && k == 0)
    {
        // Inside a shape.
        const GShape& s = g.shapes[rnd((unsigned) g.shapes.size())];
        x = coord(g, s.x1, s.x2);
        y = coord(g, s.y1, s.y2);
    }
    else if (!g.shapes.empty() && k == 1)
    {
        // On a shape boundary.
        const GShape& s = g.shapes[rnd((unsigned) g.shapes.size())];
        if (chance(50))
        {
            x = chance(50) ? s.x1 : s.x2;
            y = coord(g, s.y1, s.y2);
        }
        else
        {
            y = chance(50) ? s.y1 : s.y2;
            x = coord(g, s.x1, s.x2);
        }
    }
    else
    {
        x = coord(g, -20, g.W + 40);
        y = coord(g, -20, g.H + 40);
    }
    return "P " + f(x) + " " + f(y) + " " + std::to_string(randDirs());
}

static std::string genEnd(Gen& g, bool allowJunction = true)
{
    unsigned k = rnd(10);
    if (k < 4 && !g.shapes.empty())
    {
        unsigned si = rnd((unsigned) g.shapes.size());
        const GShape& s = g.shapes[si];
        if (!s.classes.empty())
        {
            return "S " + std::to_string(si) + " " +
                    std::to_string(s.classes[rnd((unsigned) s.classes.size())]);
        }
        // Prefer a shape with pins.
        for (unsigned t = 0; t < g.shapes.size(); ++t)
        {
            unsigned sj = (si + t) % g.shapes.size();
            if (!g.shapes[sj].classes.empty())
            {
                const GShape& s2 = g.shapes[sj];
                return "S " + std::to_string(sj) + " " +
                        std::to_string(s2.classes[rnd((unsigned) s2.classes.size())]);
            }
        }
    }
    if (k < 6 && allowJunction && g.nJ > 0)
    {
        return "J " + ref1000();
    }
    return genPointEnd(g);
}

static void genConn(Gen& g)
{
    std::string src = genEnd(g);
    std::string dst = genEnd(g);
    g.L.push_back("conn " + newId(g) + " " + src + " " + dst);
    g.nC++;
    if (g.flags == 3 && chance(50))
    {
        g.L.push_back("type L " + std::to_string(1 + rnd(2)));
    }
    else if (chance(5))
    {
        g.L.push_back("type L " + std::to_string(1 + rnd(2)));
    }
    if (chance(10)) g.L.push_back("hate L 1");
}

static std::string genCheckpoints(Gen& g)
{
    unsigned n = chance(20) ? 0 : 1 + rnd(3);
    std::string s = "cps " + ref1000() + " " + std::to_string(n);
    for (unsigned k = 0; k < n; ++k)
    {
        double x = coord(g, 0, g.W);
        double y = coord(g, 0, g.H);
        unsigned ad = chance(60) ? 0 : randDirs();
        unsigned dd = chance(60) ? 0 : randDirs();
        s += " " + f(x) + " " + f(y) + " " + std::to_string(ad) + " " + std::to_string(dd);
    }
    return s;
}

static std::string genFixedRoute(Gen& g)
{
    unsigned n = 2 + rnd(4);
    bool orth = (g.flags == 2) || chance(70);
    double x = coord(g, 0, g.W);
    double y = coord(g, 0, g.H);
    std::string s = "fixroute " + ref1000() + " " + std::to_string(n);
    bool horiz = chance(50);
    for (unsigned k = 0; k < n; ++k)
    {
        s += " " + f(x) + " " + f(y);
        if (orth)
        {
            if (horiz) x = coord(g, 0, g.W);
            else y = coord(g, 0, g.H);
            horiz = !horiz;
        }
        else
        {
            x = coord(g, 0, g.W);
            y = coord(g, 0, g.H);
        }
    }
    return s;
}

static void genJunction(Gen& g)
{
    double x = coord(g, 0, g.W);
    double y = coord(g, 0, g.H);
    g.L.push_back("junction " + newId(g) + " " + f(x) + " " + f(y) + " " +
            std::to_string(chance(25) ? 1 : 0));
    g.nJ++;
    unsigned k = 2 + rnd(5);
    for (unsigned c = 0; c < k; ++c)
    {
        // (no junction ends here: a connector from a junction to itself
        // makes upstream hyperedge rerouting use freed memory)
        std::string other = genEnd(g, false);
        if (chance(50))
        {
            g.L.push_back("conn " + newId(g) + " J L " + other);
        }
        else
        {
            g.L.push_back("conn " + newId(g) + " " + other + " J L");
        }
        g.nC++;
        if (g.flags == 3 && chance(30))
        {
            g.L.push_back("type L " + std::to_string(1 + rnd(2)));
        }
    }
}

static std::string genClusterPoly(Gen& g)
{
    std::vector<double> xs, ys;
    if (false)
    {
        // Boundary through shape corners (upstream expects polyline
        // cluster boundary points to be shape vertices).
        const GShape& s = g.shapes[rnd((unsigned) g.shapes.size())];
        xs.push_back(s.x2); ys.push_back(s.y1);
        xs.push_back(s.x2); ys.push_back(s.y2);
        xs.push_back(s.x1); ys.push_back(s.y2);
        xs.push_back(s.x1); ys.push_back(s.y1);
        return polyText(xs, ys);
    }
    double x1 = coord(g, -10, g.W);
    double y1 = coord(g, -10, g.H);
    double w = coord(g, 40, 250);
    double h = coord(g, 40, 200);
    if (chance(50))
    {
        xs.push_back(x1 + w); ys.push_back(y1);
        xs.push_back(x1 + w); ys.push_back(y1 + h);
        xs.push_back(x1); ys.push_back(y1 + h);
        xs.push_back(x1); ys.push_back(y1);
    }
    else
    {
        genPolygon(g, x1 + w / 2, y1 + h / 2, w / 2, h / 2, xs, ys);
    }
    return polyText(xs, ys);
}

static void genParam(Gen& g)
{
    unsigned k = rnd(9);
    double v;
    unsigned m = rnd(10);
    if (m < 2) v = -1;   // chooseSensibleParamValue
    else if (m < 4) v = 0;
    else
    {
        switch (k)
        {
        case 6: v = rnd(3) * 4 + (chance(30) ? 0.5 : 0); break;          // buffer
        case 7: v = (double) (1 + rnd(25)) / (chance(30) ? 2 : 1); break; // nudging
        case 3: v = chance(50) ? 4000 : rnd(500); break;                  // cluster
        default: v = chance(50) ? 50 * (1 + rnd(4)) : rnd(10000) / 10.0; break;
        }
    }
    g.L.push_back("param " + std::to_string(k) + " " + f(v));
}

static void genOption(Gen& g)
{
    unsigned k = rnd(7);
    g.L.push_back("option " + std::to_string(k) + " " + std::to_string(rnd(2)));
}

// One operation of a transaction.
static void genOp(Gen& g)
{
    unsigned nS = (unsigned) g.shapes.size();
    unsigned w = rnd(100);
    if (w < 12 && nS > 0)
    {
        unsigned si = rnd(nS);
        double dx = g.grid ? g.unit * ((int) rnd(9) - 4) : coord(g, -30, 30);
        double dy = g.grid ? g.unit * ((int) rnd(9) - 4) : coord(g, -30, 30);
        g.L.push_back("moveby " + std::to_string(si) + " " + f(dx) + " " + f(dy));
        GShape& s = g.shapes[si];
        s.x1 += dx; s.x2 += dx; s.y1 += dy; s.y2 += dy;
        for (size_t k = 0; k < s.xs.size(); ++k)
        {
            s.xs[k] += dx;
            s.ys[k] += dy;
        }
    }
    else if (w < 18 && nS > 0)
    {
        // Move / resize via a new polygon.
        unsigned si = rnd(nS);
        GShape& s = g.shapes[si];
        double x1, y1, x2, y2;
        genBox(g, x1, y1, x2, y2);
        if (chance(50))
        {
            // Resize in place.
            double w = x2 - x1, h = y2 - y1;
            x1 = s.x1; y1 = s.y1;
            x2 = x1 + w; y2 = y1 + h;
        }
        std::vector<double> xs, ys;
        if (chance(85))
        {
            // Same number of points (upstream requires it: Obstacle::
            // setNewPoly() asserts): the old polygon scaled into the box.
            double ow = std::max(1e-9, s.x2 - s.x1), oh = std::max(1e-9, s.y2 - s.y1);
            for (size_t k = 0; k < s.xs.size(); ++k)
            {
                double x = x1 + (s.xs[k] - s.x1) * ((x2 - x1) / ow);
                double y = y1 + (s.ys[k] - s.y1) * ((y2 - y1) / oh);
                if (g.grid)
                {
                    x = std::round(x / g.unit) * g.unit;
                    y = std::round(y / g.unit) * g.unit;
                }
                xs.push_back(x);
                ys.push_back(y);
            }
        }
        else if (g.allowPoly && chance(30))
        {
            genPolygon(g, (x1 + x2) / 2, (y1 + y2) / 2, (x2 - x1) / 2, (y2 - y1) / 2, xs, ys);
        }
        else
        {
            xs.push_back(x2); ys.push_back(y1);
            xs.push_back(x2); ys.push_back(y2);
            xs.push_back(x1); ys.push_back(y2);
            xs.push_back(x1); ys.push_back(y1);
        }
        s.xs = xs;
        s.ys = ys;
        g.L.push_back("moveto " + std::to_string(si) + " " + std::to_string(rnd(2)) + " " +
                polyText(xs, ys));
        s.x1 = *std::min_element(xs.begin(), xs.end());
        s.x2 = *std::max_element(xs.begin(), xs.end());
        s.y1 = *std::min_element(ys.begin(), ys.end());
        s.y2 = *std::max_element(ys.begin(), ys.end());
    }
    else if (w < 21 && nS > 1)
    {
        unsigned si = rnd(nS);
        if (g.shapes[si].addedInBatch) return;   // upstream asserts
        g.L.push_back("delshape " + std::to_string(si));
        g.shapes.erase(g.shapes.begin() + si);
    }
    else if (w < 26)
    {
        genShape(g);
    }
    else if (w < 28)
    {
        genJunction(g);
    }
    else if (w < 32 && g.nJ > 0)
    {
        if (chance(50))
        {
            g.L.push_back("movejby " + ref1000() + " " + f(coord(g, -20, 20)) + " " +
                    f(coord(g, -20, 20)));
        }
        else
        {
            g.L.push_back("movejto " + ref1000() + " " + f(coord(g, 0, g.W)) + " " +
                    f(coord(g, 0, g.H)));
        }
    }
    else if (w < 33 && g.nJ > 0)
    {
        g.L.push_back("fixj " + ref1000() + " " + std::to_string(rnd(2)));
    }
    else if (w < 34 && g.nJ > 0)
    {
        g.L.push_back("deljunc " + ref1000());
        g.nJ--;
    }
    else if (w < 40 && g.nC > 0)
    {
        g.L.push_back(std::string(chance(50) ? "src " : "dst ") + ref1000() + " " + genEnd(g));
    }
    else if (w < 42 && g.nC > 0)
    {
        g.L.push_back("type " + ref1000() + " " + std::to_string(1 + rnd(2)));
    }
    else if (w < 46 && g.nC > 0)
    {
        g.L.push_back(genCheckpoints(g));
    }
    else if (w < 48 && g.nC > 0)
    {
        g.L.push_back(genFixedRoute(g));
    }
    else if (w < 49 && g.nC > 0)
    {
        g.L.push_back("fixexist " + ref1000());
    }
    else if (w < 51 && g.nC > 0)
    {
        g.L.push_back("clearfix " + ref1000());
    }
    else if (w < 53 && g.nC > 0)
    {
        g.L.push_back("invalid " + ref1000());
    }
    else if (w < 55 && g.nC > 0)
    {
        g.L.push_back("split " + ref1000() + " " + std::to_string(1 + rnd(4)));
        g.nC++;
        g.nJ++;
    }
    else if (w < 57 && g.nJ > 0)
    {
        g.L.push_back("mergej " + ref1000());
    }
    else if (w < 60 && nS > 0)
    {
        genPin(g, rnd(nS));
    }
    else if (w < 61 && nS > 0)
    {
        g.L.push_back("pinexcl " + std::to_string(rnd(nS)) + " " + std::to_string(rnd(100)) +
                " " + std::to_string(rnd(2)));
    }
    else if (w < 62 && nS > 0)
    {
        g.L.push_back("pincost " + std::to_string(rnd(nS)) + " " + std::to_string(rnd(100)) +
                " " + f(rnd(300)));
    }
    else if (w < 63 && nS > 0)
    {
        g.L.push_back("delpin " + std::to_string(rnd(nS)) + " " + std::to_string(rnd(100)));
    }
    else if (w < 65 && nS > 0)
    {
        g.L.push_back("transform " + std::to_string(rnd(nS)) + " " + std::to_string(rnd(5)));
    }
    else if (w < 67 && (g.flags & 1))
    {
        g.L.push_back("cluster " + newId(g) + " " + genClusterPoly(g));
        g.nK++;
    }
    else if (w < 68 && g.nK > 0)
    {
        g.L.push_back("movecluster " + std::to_string(rnd(g.nK)) + " " + genClusterPoly(g));
    }
    else if (w < 69 && g.nK > 0)
    {
        g.L.push_back("delcluster " + std::to_string(rnd(g.nK)));
        g.nK--;
    }
    else if (w < 72 && g.nC > 1)
    {
        g.L.push_back("delconn " + ref1000());
        g.nC--;
    }
    else if (w < 79)
    {
        genConn(g);
    }
    else if (w < 83)
    {
        genParam(g);
    }
    else if (w < 87)
    {
        genOption(g);
    }
    else if (w < 88)
    {
        g.L.push_back("settings");
    }
    else if (w < 91 && g.nJ > 0 && g.hyper)
    {
        g.L.push_back("regj " + ref1000());
    }
    else if (w < 93 && g.hyper)
    {
        // Hyperedge rerouting is orthogonal only (MTST over the orthogonal
        // visibility graph).
        // Terminals are pins of distinct shapes, as upstream uses this API
        // (hyperedge02.cpp): free-point terminals get a vertex without
        // orthogonal visibility (added after the graph is built), the MTST
        // cannot reach them and upstream dereferences a null tree root
        // (hyperedge.cpp:337); coincident terminals do the same.
        std::vector<unsigned> withPins;
        for (unsigned k = 0; k < g.shapes.size(); ++k)
        {
            if (!g.shapes[k].classes.empty()) withPins.push_back(k);
        }
        if (withPins.size() < 2) return;
        if (withPins.size() < 3) return;
        unsigned n = std::min((unsigned) withPins.size(), 3 + rnd(3));
        std::string s = "regends " + std::to_string(n);
        for (unsigned k = 0; k < n; ++k)
        {
            unsigned pick = k + rnd((unsigned) withPins.size() - k);
            std::swap(withPins[k], withPins[pick]);
            const GShape& sh = g.shapes[withPins[k]];
            s += " S " + std::to_string(withPins[k]) + " " +
                    std::to_string(sh.classes[rnd((unsigned) sh.classes.size())]);
        }
        g.L.push_back(s);
    }
    else if (w < 95 && g.nC > 0)
    {
        g.L.push_back("hate " + ref1000() + " " + std::to_string(rnd(2)));
    }
    else if (w < 96)
    {
        g.L.push_back("markmoved");
    }
    else
    {
        genConn(g);
    }
}

static uint64_t sceneSeed(uint64_t seed, uint64_t index)
{
    uint64_t z = seed * 0x9E3779B97F4A7C15ULL + index + 0x632BE59BD9B4E019ULL;
    z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9ULL;
    z = (z ^ (z >> 27)) * 0x94D049BB133111EBULL;
    return z ^ (z >> 31);
}

static std::vector<std::string> genScene(uint64_t seed, uint64_t index)
{
    g_state = sceneSeed(seed, index);
    Gen g;
    g.nJ = 0;
    g.nC = 0;
    g.nK = 0;
    g.nextId = 0;
    g.txn = true;    // Router's default (m_consolidate_actions = true)
    g.inBatch = false;
    unsigned fk = rnd(10);
    g.flags = (fk < 4) ? 2 : (fk < 6 ? 1 : 3);
    // Hyperedge rerouting (orthogonal only) in some scenes: upstream has
    // several crash/UB paths there (see e2e_fuzz.sh), which exclude the
    // whole scene from the comparison.
    g.hyper = (g.flags & 2) && chance(40);
    g.grid = chance(60);
    g.unit = g.grid ? (chance(50) ? 10 : 5) : 1;
    unsigned size = rnd(10);   // 0-5 small, 6-8 medium, 9 large
    g.W = (size < 6) ? 250 : (size < 9 ? 450 : 700);
    g.H = g.W * 3 / 4;
    g.allowPoly = chance(50);
    std::vector<std::string>& L = g.L;
    L.push_back("router " + std::to_string(g.flags));

    // Initial parameters / options.
    unsigned nParams = rnd(6);
    for (unsigned k = 0; k < nParams; ++k) genParam(g);
    unsigned nOpts = rnd(5);
    for (unsigned k = 0; k < nOpts; ++k) genOption(g);

    // Initial scene, in a transaction or immediate mode.
    bool initTxn = chance(70);
    if (!initTxn)
    {
        L.push_back("txn 0");
        g.txn = false;
    }
    g.inBatch = g.txn;
    unsigned nShapes = (size < 6) ? 1 + rnd(5) : (size < 9 ? 4 + rnd(8) : 10 + rnd(6));
    unsigned nConns = (size < 6) ? 1 + rnd(5) : (size < 9 ? 3 + rnd(10) : 8 + rnd(8));
    unsigned nJuncs = chance(40) ? 1 + rnd(size < 6 ? 2 : 3) : 0;
    unsigned nClusters = (g.flags & 1) && chance(30) ? 1 + rnd(2) : 0;
    for (unsigned k = 0; k < nShapes; ++k) genShape(g);
    for (unsigned k = 0; k < nJuncs; ++k) genJunction(g);
    for (unsigned k = 0; k < nConns; ++k) genConn(g);
    for (unsigned k = 0; k < nClusters; ++k)
    {
        L.push_back("cluster " + newId(g) + " " + genClusterPoly(g));
        g.nK++;
    }
    if (g.nC > 0 && chance(30)) L.push_back(genCheckpoints(g));
    if (g.nJ > 0 && g.hyper && chance(30)) L.push_back("regj " + ref1000());
    L.push_back("process");
    for (size_t k = 0; k < g.shapes.size(); ++k) g.shapes[k].addedInBatch = false;
    g.inBatch = false;

    // Transactions.
    // (the bump allocator never frees: memory grows with the number of
    // transactions, so very long sequences are kept for small scenes)
    unsigned steps = (size < 6) ? 10 + rnd(31) : 10 + rnd(16);
    if (size == 9) steps = 10 + rnd(6);
    for (unsigned s = 0; s < steps; ++s)
    {
        if (chance(40))
        {
            if (!g.txn)
            {
                L.push_back("txn 1");
                g.txn = true;
            }
            g.inBatch = true;
            unsigned n = 1 + rnd(6);
            for (unsigned k = 0; k < n; ++k) genOp(g);
            L.push_back("process");
            for (size_t k = 0; k < g.shapes.size(); ++k) g.shapes[k].addedInBatch = false;
            g.inBatch = false;
            if (chance(40))
            {
                L.push_back("txn 0");
                g.txn = false;
            }
        }
        else
        {
            if (g.txn)
            {
                L.push_back("txn 0");
                g.txn = false;
            }
            genOp(g);
            L.push_back("process");
        }
    }
    return L;
}

//============================================================================
// Main.
//============================================================================

struct FileScene
{
    std::string name;
    std::vector<std::string> dsl;
};

static std::vector<FileScene> readScenes(const char *path)
{
    std::vector<FileScene> res;
    std::ifstream in(path);
    std::string line;
    bool inScene = false, inOut = false;
    while (std::getline(in, line))
    {
        if (line.compare(0, 7, "#scene ") == 0)
        {
            FileScene s;
            s.name = line.substr(7);
            res.push_back(s);
            inScene = true;
            inOut = false;
        }
        else if (line == "#out")
        {
            inOut = true;
        }
        else if (line == "#end")
        {
            inScene = false;
        }
        else if (inScene && !inOut)
        {
            res.back().dsl.push_back(line);
        }
    }
    return res;
}

int main(int argc, char **argv)
{
    g_full = getenv("E2E_FULL") != nullptr;
    std::string mode = (argc > 1) ? argv[1] : "";
    if ((mode == "gen" || mode == "dsl") && argc == 5)
    {
        uint64_t seed = strtoull(argv[2], nullptr, 10);
        uint64_t first = strtoull(argv[3], nullptr, 10);
        uint64_t count = strtoull(argv[4], nullptr, 10);
        for (uint64_t i = first; i < first + count; ++i)
        {
            std::vector<std::string> dsl = genScene(seed, i);
            std::string name = "s" + std::to_string(seed) + "-" + std::to_string(i);
            if (mode == "dsl")
            {
                printf("#scene %s\n", name.c_str());
                for (size_t k = 0; k < dsl.size(); ++k) printf("%s\n", dsl[k].c_str());
                printf("#end\n");
            }
            else
            {
                runScene(name, dsl);
            }
        }
        return 0;
    }
    if (mode == "run" && argc == 3)
    {
        std::vector<FileScene> scenes = readScenes(argv[2]);
        for (size_t i = 0; i < scenes.size(); ++i)
        {
            runScene(scenes[i].name, scenes[i].dsl);
        }
        return 0;
    }
    fprintf(stderr, "usage: e2e_fuzz gen|dsl SEED FIRST COUNT | run FILE\n");
    return 2;
}
