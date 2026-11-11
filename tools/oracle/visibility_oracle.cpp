// Differential oracle for the Rust port of libavoid's poly-line routing
// (visibility.cpp: naive + Lee sweep visibility graph; makepath.cpp: A*
// search and cost function).
//
// For a few hundred deterministic random poly-line scenes (rectangles and
// convex polygons on a coarse grid -- so overlapping, touching and
// coordinate-sharing shapes are common --, connector endpoints on the grid,
// inside shapes, on shape vertices / edge midpoints or shared with other
// connectors, checkpoints, clusters, all combinations of UseLeesAlgorithm /
// InvisibilityGrph / IgnoreRegions / RubberBandRouting / SelectiveReroute,
// segment / angle / reverse-direction penalties, and shape moves, shape
// deletions/additions and connector endpoint changes between transactions)
// it dumps after every transaction:
//   * every vertex in Router::vertices order with its visList and invisList
//     (other-end vertex ids, list order),
//   * the global visGraph and invisGraph edge lists in order (vertex ids +
//     %.17g distance, resp. blocker),
//   * every connector's raw route (with point ids) and display route.
//
// The Rust test `tests/visibility_oracle.rs` replays exactly the same
// scenes (identical LCG; never two RNG calls in one C++ expression, so the
// evaluation order is fixed) and must reproduce the output byte for byte.
//
//   main mode    (`visibility_oracle`)          -> tests/golden/visibility_oracle.txt
//                crossing / cluster crossing / fixed shared path penalties 0.
//   penalty mode (`visibility_oracle penalties`) -> tests/golden/visibility_oracle_penalties.txt
//                adds crossingPenalty, fixedSharedPathPenalty and
//                clusterCrossingPenalty (needs ConnectorCrossings).
//
// Build & regenerate (from the repository root; VENDOR is the vendored
// adaptagrams checkout, which is gitignored):
//
//   VENDOR=$PWD/vendor/adaptagrams/cola
//   g++ -std=c++11 -O2 -ffp-contract=off -fno-access-control -I"$VENDOR" \
//       tools/oracle/visibility_oracle.cpp "$VENDOR"/libavoid/*.cpp \
//       -o /tmp/visibility_oracle
//   /tmp/visibility_oracle > tests/golden/visibility_oracle.txt
//   /tmp/visibility_oracle penalties > tests/golden/visibility_oracle_penalties.txt
//
// (-fno-access-control lets us read private members; asserts stay enabled.)

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>

#include "libavoid/libavoid.h"
#include "libavoid/router.h"
#include "libavoid/shape.h"
#include "libavoid/connector.h"
#include "libavoid/connend.h"
#include "libavoid/viscluster.h"
#include "libavoid/vertices.h"
#include "libavoid/graph.h"

using namespace Avoid;

static std::string g17(double v)
{
    char buf[64];
    snprintf(buf, sizeof(buf), "%.17g", v);
    return buf;
}

// ---------------------------------------------------------------------------
// Deterministic LCG (identical in tests/visibility_oracle.rs).
static unsigned long long g_state = 0;
static bool g_rectsAndSmallPolysOnly = false;

static unsigned int rngNext(void)
{
    g_state = g_state * 6364136223846793005ULL + 1442695040888963407ULL;
    return (unsigned int) (g_state >> 33);
}

static int rnd(int n)
{
    return (int) (rngNext() % (unsigned int) n);
}

// ---------------------------------------------------------------------------

struct Scene
{
    Router *router;
    bool penaltyMode;
    bool hasCluster;
    std::vector<ShapeRef *> shapes;
    std::vector<Polygon> shapePolys;
    std::vector<bool> shapeAddedThisRound;
    std::vector<ConnRef *> conns;
    std::vector<Point> connSrc;
    std::vector<Point> connDst;
};

static const double triX[] = { 1, 0, -1 };
static const double triY[] = { -1, 1, -1 };
static const double diaX[] = { 1, 0, -1, 0 };
static const double diaY[] = { 0, 1, 0, -1 };
static const double hexX[] = { 2, 1, -1, -2, -1, 1 };
static const double hexY[] = { 0, 2, 2, 0, -2, -2 };
static const double penX[] = { 2, 1, -1, -2, 0 };
static const double penY[] = { 0, 2, 2, 0, -2 };

static Polygon makeShapePoly(void)
{
    int kind = rnd(7);
    if (g_rectsAndSmallPolysOnly && (kind >= 5))
    {
        // Penalty mode: upstream splitBranchingSegments() (midVertexNumber)
        // asserts on diagonal segments between vertices with vn >= 4, so
        // only use shapes with at most four vertices there.
        kind = 0;
    }
    if (kind < 3)
    {
        double x0 = 5 * rnd(20);
        double y0 = 5 * rnd(20);
        double w = 5 * (1 + rnd(6));
        double h = 5 * (1 + rnd(6));
        return Rectangle(Point(x0, y0), Point(x0 + w, y0 + h));
    }
    const double *tx;
    const double *ty;
    int n;
    if (kind == 3)
    {
        tx = triX; ty = triY; n = 3;
    }
    else if (kind == 4)
    {
        tx = diaX; ty = diaY; n = 4;
    }
    else if (kind == 5)
    {
        tx = hexX; ty = hexY; n = 6;
    }
    else
    {
        tx = penX; ty = penY; n = 5;
    }
    double cx = 5 * rnd(20);
    double cy = 5 * rnd(20);
    double s = 2.5 * (1 + rnd(4));
    Polygon poly(n);
    for (int i = 0; i < n; ++i)
    {
        poly.ps[i] = Point(cx + s * tx[i], cy + s * ty[i]);
    }
    return poly;
}

static Point gridPoint(void)
{
    double x = 5 * rnd(21);
    double y = 5 * rnd(21);
    return Point(x, y);
}

// Shapes that actions may touch (in penalty mode with a cluster, shape 0
// defines the cluster boundary and is left alone).
static std::vector<int> liveShapes(Scene& sc, bool forDeletion)
{
    std::vector<int> live;
    for (size_t i = 0; i < sc.shapes.size(); ++i)
    {
        if (sc.shapes[i] == nullptr)
        {
            continue;
        }
        if (sc.penaltyMode && sc.hasCluster && (i == 0))
        {
            continue;
        }
        if (forDeletion && sc.shapeAddedThisRound[i])
        {
            continue;
        }
        live.push_back((int) i);
    }
    return live;
}

static std::vector<int> liveConns(Scene& sc)
{
    std::vector<int> live;
    for (size_t i = 0; i < sc.conns.size(); ++i)
    {
        if (sc.conns[i] != nullptr)
        {
            live.push_back((int) i);
        }
    }
    return live;
}

static Point randPoint(Scene& sc)
{
    int kind = rnd(8);
    std::vector<int> live;
    for (size_t i = 0; i < sc.shapes.size(); ++i)
    {
        if (sc.shapes[i] != nullptr)
        {
            live.push_back((int) i);
        }
    }
    if ((kind <= 3) || live.empty())
    {
        return gridPoint();
    }
    if (kind == 7)
    {
        std::vector<int> lc = liveConns(sc);
        if (lc.empty())
        {
            return gridPoint();
        }
        int c = lc[rnd((int) lc.size())];
        int which = rnd(2);
        return (which == 0) ? sc.connSrc[c] : sc.connDst[c];
    }
    int s = live[rnd((int) live.size())];
    const Polygon& poly = sc.shapePolys[s];
    size_t n = poly.size();
    if (kind == 4)
    {
        // Centre of the bounding box (inside the shape).
        double minX = poly.ps[0].x, maxX = poly.ps[0].x;
        double minY = poly.ps[0].y, maxY = poly.ps[0].y;
        for (size_t i = 1; i < n; ++i)
        {
            if (poly.ps[i].x < minX) minX = poly.ps[i].x;
            if (poly.ps[i].x > maxX) maxX = poly.ps[i].x;
            if (poly.ps[i].y < minY) minY = poly.ps[i].y;
            if (poly.ps[i].y > maxY) maxY = poly.ps[i].y;
        }
        return Point((minX + maxX) / 2, (minY + maxY) / 2);
    }
    int v = rnd((int) n);
    if (kind == 5)
    {
        // A shape vertex.
        return Point(poly.ps[v].x, poly.ps[v].y);
    }
    // kind == 6: an edge midpoint.
    const Point& a = poly.ps[v];
    const Point& b = poly.ps[(v + 1) % n];
    return Point((a.x + b.x) / 2, (a.y + b.y) / 2);
}

static void addShape(Scene& sc)
{
    Polygon poly = makeShapePoly();
    sc.shapes.push_back(new ShapeRef(sc.router, poly));
    sc.shapePolys.push_back(poly);
    sc.shapeAddedThisRound.push_back(true);
}

static void addConn(Scene& sc)
{
    Point src = randPoint(sc);
    Point dst = randPoint(sc);
    if (src == dst)
    {
        dst.x += 5;
    }
    ConnRef *conn = new ConnRef(sc.router, ConnEnd(src), ConnEnd(dst));
    sc.conns.push_back(conn);
    sc.connSrc.push_back(src);
    sc.connDst.push_back(dst);
    if (rnd(5) == 0)
    {
        Point cp = gridPoint();
        std::vector<Checkpoint> cps;
        cps.push_back(Checkpoint(cp));
        conn->setRoutingCheckpoints(cps);
    }
}

static void printId(const VertID& id)
{
    printf("%u.%u", id.objID, (unsigned) id.vn);
}

static void printPoly(const char *label, const PolyLine& route)
{
    printf(" %s[", label);
    for (size_t i = 0; i < route.ps.size(); ++i)
    {
        printf(" (%s,%s,%u,%u)", g17(route.ps[i].x).c_str(),
                g17(route.ps[i].y).c_str(), route.ps[i].id,
                (unsigned) route.ps[i].vn);
    }
    printf(" ]");
}

static void dump(Scene& sc, int round, bool transactionResult)
{
    Router *r = sc.router;
    printf("round %d transaction=%d\n", round, (int) transactionResult);
    for (VertInf *v = r->vertices.connsBegin(); v != r->vertices.end();
            v = v->lstNext)
    {
        printf("v ");
        printId(v->id);
        printf(" p%u (%s,%s) vis[", (unsigned) v->id.props,
                g17(v->point.x).c_str(), g17(v->point.y).c_str());
        for (EdgeInfList::iterator e = v->visList.begin();
                e != v->visList.end(); ++e)
        {
            printf(" ");
            printId((*e)->otherVert(v)->id);
        }
        printf(" ] inv[");
        for (EdgeInfList::iterator e = v->invisList.begin();
                e != v->invisList.end(); ++e)
        {
            printf(" ");
            printId((*e)->otherVert(v)->id);
        }
        printf(" ]\n");
    }
    printf("vis %d\n", r->visGraph.size());
    for (EdgeInf *e = r->visGraph.begin(); e != r->visGraph.end();
            e = e->lstNext)
    {
        printId(e->m_vert1->id);
        printf("-");
        printId(e->m_vert2->id);
        printf(" %s\n", g17(e->m_dist).c_str());
    }
    printf("invis %d\n", r->invisGraph.size());
    for (EdgeInf *e = r->invisGraph.begin(); e != r->invisGraph.end();
            e = e->lstNext)
    {
        printId(e->m_vert1->id);
        printf("-");
        printId(e->m_vert2->id);
        printf(" b%d %s\n", e->m_blocker, g17(e->m_dist).c_str());
    }
    for (ConnRefList::iterator it = r->connRefs.begin();
            it != r->connRefs.end(); ++it)
    {
        ConnRef *c = *it;
        printf("conn %u reroute=%d", c->id(), (int) c->m_needs_reroute_flag);
        printPoly("route", c->route());
        printPoly("display", c->displayRoute());
        printf("\n");
    }
}

static void runScene(int index, bool penaltyMode)
{
    g_state = 0x9E3779B97F4A7C15ULL ^ ((unsigned long long) index * 1000003ULL);
    rngNext();
    rngNext();

    Scene sc;
    sc.penaltyMode = penaltyMode;
    g_rectsAndSmallPolysOnly = penaltyMode;
    sc.hasCluster = false;
    sc.router = new Router(PolyLineRouting);
    Router *router = sc.router;

    router->UseLeesAlgorithm = (rnd(2) == 1);
    router->InvisibilityGrph = (rnd(2) == 1);
    router->IgnoreRegions = (rnd(4) != 0);
    router->RubberBandRouting = false;
    if (router->IgnoreRegions)
    {
        router->RubberBandRouting = (rnd(4) == 0);
    }
    router->SelectiveReroute = (rnd(3) != 0);

    int sp = rnd(3);
    router->setRoutingParameter(segmentPenalty,
            (sp == 0) ? 0 : ((sp == 1) ? 10 : 50));
    int ap = rnd(3);
    router->setRoutingParameter(anglePenalty,
            (ap == 0) ? 0 : ((ap == 1) ? 20 : 200));
    int rp = rnd(3);
    router->setRoutingParameter(reverseDirectionPenalty,
            (rp == 2) ? 30 : 0);
    if (penaltyMode)
    {
        int cp = rnd(2);
        router->setRoutingParameter(crossingPenalty, (cp == 0) ? 0 : 200);
        int fp = rnd(2);
        router->setRoutingParameter(fixedSharedPathPenalty,
                (fp == 0) ? 0 : 110);
    }
    else
    {
        router->setRoutingParameter(clusterCrossingPenalty, 0);
    }

    printf("scene %d lees=%d invis=%d ignoreRegions=%d rubberBand=%d "
            "selective=%d seg=%s angle=%s reverse=%s crossing=%s "
            "shared=%s cluster=%s\n", index,
            (int) router->UseLeesAlgorithm, (int) router->InvisibilityGrph,
            (int) router->IgnoreRegions, (int) router->RubberBandRouting,
            (int) router->SelectiveReroute,
            g17(router->routingParameter(segmentPenalty)).c_str(),
            g17(router->routingParameter(anglePenalty)).c_str(),
            g17(router->routingParameter(reverseDirectionPenalty)).c_str(),
            g17(router->routingParameter(crossingPenalty)).c_str(),
            g17(router->routingParameter(fixedSharedPathPenalty)).c_str(),
            g17(router->routingParameter(clusterCrossingPenalty)).c_str());

    int nShapes = 1 + rnd(4);
    for (int i = 0; i < nShapes; ++i)
    {
        addShape(sc);
    }
    // Clusters only in main mode (clusterCrossingPenalty 0 there): with a
    // nonzero clusterCrossingPenalty, upstream
    // ConnectorCrossings::countForSegment() overflows a heap buffer
    // (connector.cpp, c_path/p_path) when a route shares a path with a
    // (closed) cluster boundary, so that combination is undefined
    // behaviour and cannot serve as an oracle.
    if (!penaltyMode && (rnd(4) == 0))
    {
        double x0 = 5 * rnd(15);
        double y0 = 5 * rnd(15);
        double w = 5 * (2 + rnd(6));
        double h = 5 * (2 + rnd(6));
        Polygon cpoly = Rectangle(Point(x0, y0), Point(x0 + w, y0 + h));
        new ClusterRef(router, cpoly);
        sc.hasCluster = true;
    }
    int nConns = 1 + rnd(4);
    for (int i = 0; i < nConns; ++i)
    {
        addConn(sc);
    }
    bool result = router->processTransaction();
    dump(sc, 0, result);
    fflush(stdout);

    for (int round = 1; round <= 3; ++round)
    {
        for (size_t i = 0; i < sc.shapeAddedThisRound.size(); ++i)
        {
            sc.shapeAddedThisRound[i] = false;
        }
        int nActions = 1 + rnd(3);
        for (int a = 0; a < nActions; ++a)
        {
            int action = rnd(7);
            if (action == 0)
            {
                std::vector<int> live = liveShapes(sc, false);
                if (live.empty()) continue;
                int s = live[rnd((int) live.size())];
                double dx = 5 * (rnd(5) - 2);
                double dy = 5 * (rnd(5) - 2);
                printf("action move-by %d %s %s\n", s, g17(dx).c_str(),
                        g17(dy).c_str());
                router->moveShape(sc.shapes[s], dx, dy);
                sc.shapePolys[s].translate(dx, dy);
            }
            else if (action == 1)
            {
                std::vector<int> live = liveShapes(sc, false);
                if (live.empty()) continue;
                int s = live[rnd((int) live.size())];
                Polygon poly = makeShapePoly();
                if (poly.size() != sc.shapePolys[s].size())
                {
                    // Moves must keep the vertex count, so translate the
                    // current polygon instead.
                    poly = sc.shapePolys[s];
                    double dx = 5 * (rnd(9) - 4);
                    double dy = 5 * (rnd(9) - 4);
                    poly.translate(dx, dy);
                }
                printf("action move-to %d\n", s);
                router->moveShape(sc.shapes[s], poly);
                sc.shapePolys[s] = poly;
            }
            else if (action == 2)
            {
                if (router->RubberBandRouting)
                {
                    // Upstream rubber-band routing restarts from the
                    // vertices of the previous route and asserts if they
                    // no longer exist, so never delete shapes then.
                    continue;
                }
                std::vector<int> live = liveShapes(sc, true);
                if (live.empty()) continue;
                int s = live[rnd((int) live.size())];
                printf("action delete-shape %d\n", s);
                router->deleteShape(sc.shapes[s]);
                sc.shapes[s] = nullptr;
            }
            else if (action == 3)
            {
                printf("action add-shape\n");
                addShape(sc);
            }
            else if (action == 4)
            {
                std::vector<int> lc = liveConns(sc);
                if (lc.empty()) continue;
                int c = lc[rnd((int) lc.size())];
                Point p = randPoint(sc);
                int which = rnd(2);
                if (which == 0)
                {
                    if (p == sc.connDst[c])
                    {
                        p.x += 5;
                    }
                    printf("action set-source %d\n", c);
                    sc.conns[c]->setSourceEndpoint(ConnEnd(p));
                    sc.connSrc[c] = p;
                }
                else
                {
                    if (p == sc.connSrc[c])
                    {
                        p.x += 5;
                    }
                    printf("action set-dest %d\n", c);
                    sc.conns[c]->setDestEndpoint(ConnEnd(p));
                    sc.connDst[c] = p;
                }
            }
            else if (action == 5)
            {
                std::vector<int> lc = liveConns(sc);
                if (lc.empty()) continue;
                int c = lc[rnd((int) lc.size())];
                printf("action delete-conn %d\n", c);
                router->deleteConnector(sc.conns[c]);
                sc.conns[c] = nullptr;
            }
            else
            {
                printf("action add-conn\n");
                addConn(sc);
            }
        }
        result = router->processTransaction();
        dump(sc, round, result);
        fflush(stdout);
    }

    delete router;
}

int main(int argc, char **argv)
{
    bool penaltyMode = (argc > 1) && (strcmp(argv[1], "penalties") == 0);
    int nScenes = penaltyMode ? 100 : 300;
    for (int i = 0; i < nScenes; ++i)
    {
        runScene(i, penaltyMode);
    }
    return 0;
}
