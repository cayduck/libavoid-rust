// Differential oracle for the Rust port of the second half of libavoid
// connector.cpp: splitBranchingSegments, ConnectorCrossings::countForSegment
// (with crossingPoints / sharedPaths / pointOrders collection) and PtOrder.
//
// The Rust test `tests/crossings_oracle.rs` regenerates exactly the same
// inputs (same LCG, same consumption order) and must reproduce this
// program's output, saved as `tests/golden/crossings_oracle.txt`.
//
// Build & regenerate (from the repository root; VENDOR is the vendored
// adaptagrams checkout, which is gitignored):
//
//   VENDOR=$PWD/vendor/adaptagrams/cola
//   g++ -std=c++11 -O2 -ffp-contract=off -fno-access-control -I"$VENDOR" \
//       tools/oracle/crossings_oracle.cpp "$VENDOR"/libavoid/*.cpp \
//       -o /tmp/crossings_oracle
//   /tmp/crossings_oracle > tests/golden/crossings_oracle.txt 2>/dev/null
//
// (-ffp-contract=off guarantees no FMA contraction; asserts stay enabled,
// and the generators are designed so that no upstream COLA_ASSERT fires.
// -fno-access-control lets us set ConnRef::m_type / m_has_fixed_route
// directly.  stderr carries upstream warnings only.)
//
// IMPORTANT: never put two RNG calls in one C++ expression (argument
// evaluation order is unspecified); always sequence them in statements.

#include <cstdio>
#include <cstdint>
#include <cfloat>
#include <string>
#include <vector>
#include <list>
#include <limits>
#include <algorithm>

#include "libavoid/libavoid.h"
#include "libavoid/router.h"
#include "libavoid/connector.h"
#include "libavoid/connend.h"
#include "libavoid/junction.h"
#include "libavoid/geometry.h"

using namespace Avoid;

static uint64_t g_state = 0x2545f4914f6cdd1dULL;

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

static std::string g17(double v)
{
    char buf[64];
    snprintf(buf, sizeof(buf), "%.17g", v);
    return buf;
}

static std::string ptStr(const Point& p)
{
    char buf[160];
    snprintf(buf, sizeof(buf), "(%s,%s,%u,%u)", g17(p.x).c_str(),
            g17(p.y).c_str(), (unsigned) p.id, (unsigned) p.vn);
    return buf;
}

static std::string polyStr(const Polygon& p)
{
    std::string s = "[";
    for (size_t i = 0; i < p.ps.size(); ++i)
    {
        if (i > 0) s += " ";
        s += ptStr(p.ps[i]);
    }
    return s + "]";
}

// ---------------------------------------------------------------------------
// Geometry generators.

static Point gridPt(unsigned range)
{
    double x = (double) rnd(range);
    double y = (double) rnd(range);
    return Point(x, y);
}

// Orthogonal random walk of n points starting at start.
static std::vector<Point> orthWalk(unsigned n, Point start)
{
    std::vector<Point> ps;
    ps.push_back(start);
    unsigned axis = rnd(2);
    for (unsigned k = 1; k < n; ++k)
    {
        Point q = ps.back();
        if (rnd(10) < 7)
        {
            axis = 1 - axis;
        }
        double len = (double) (rnd(3) + 1);
        if (rnd(2) == 0)
        {
            len = -len;
        }
        if (axis == 0)
        {
            q.x += len;
        }
        else
        {
            q.y += len;
        }
        ps.push_back(q);
    }
    return ps;
}

// General random walk of n points starting at start (no zero-length steps).
static std::vector<Point> freeWalk(unsigned n, Point start)
{
    std::vector<Point> ps;
    ps.push_back(start);
    for (unsigned k = 1; k < n; ++k)
    {
        Point q = gridPt(6);
        if (q == ps.back())
        {
            q.x += 1;
        }
        ps.push_back(q);
    }
    return ps;
}

static std::vector<Point> walk(bool orth, unsigned n, Point start)
{
    return orth ? orthWalk(n, start) : freeWalk(n, start);
}

// Assign fresh ids / vertex numbers.  Non-orthogonal geometry never gets
// kUnassignedVertexNumber (8): midVertexNumber() asserts the split segment
// is orthogonal when it has to fall back on unassigned neighbours.
static void assignIds(std::vector<Point>& ps, bool orth)
{
    for (size_t i = 0; i < ps.size(); ++i)
    {
        ps[i].id = rnd(4);
        ps[i].vn = (unsigned short) rnd(orth ? 9 : 8);
    }
}

static Polygon toPoly(const std::vector<Point>& ps)
{
    Polygon p;
    p.ps = ps;
    return p;
}

// A pair of open polylines (conn, poly) designed to overlap a lot.
static void genConnPair(bool orth, Polygon& conn, Polygon& poly)
{
    unsigned cn = 2 + rnd(6);
    Point cs = gridPt(5);
    std::vector<Point> c = walk(orth, cn, cs);
    std::vector<Point> p;
    unsigned strategy = rnd(5);
    if (strategy == 0)
    {
        // Independent.
        unsigned pn = 2 + rnd(6);
        Point ps0 = gridPt(5);
        p = walk(orth, pn, ps0);
    }
    else if (strategy == 1 || strategy == 2)
    {
        // A slice of conn, with random extensions at either end.
        unsigned a = rnd((unsigned) c.size() - 1);
        unsigned b = a + 1 + rnd((unsigned) c.size() - 1 - a);
        std::vector<Point> slice(c.begin() + a, c.begin() + b + 1);
        unsigned preN = 1 + rnd(3);
        std::vector<Point> pre = walk(orth, preN, slice.front());
        std::reverse(pre.begin(), pre.end());
        pre.pop_back();
        unsigned sufN = 1 + rnd(3);
        std::vector<Point> suf = walk(orth, sufN, slice.back());
        p = pre;
        p.insert(p.end(), slice.begin(), slice.end());
        p.insert(p.end(), suf.begin() + 1, suf.end());
        if (rnd(2) == 0)
        {
            std::reverse(p.begin(), p.end());
        }
    }
    else if (strategy == 3)
    {
        // Fully coincident, maybe reversed.
        p = c;
        if (rnd(2) == 0)
        {
            std::reverse(p.begin(), p.end());
        }
    }
    else
    {
        // Shares an endpoint.
        Point start = (rnd(2) == 0) ? c.front() : c.back();
        unsigned pn = 2 + rnd(5);
        p = walk(orth, pn, start);
        if (rnd(2) == 0)
        {
            std::reverse(p.begin(), p.end());
        }
    }
    assignIds(c, orth);
    assignIds(p, orth);
    conn = toPoly(c);
    poly = toPoly(p);
}

// A neighbouring grid point (one of the 8 directions).
static Point offsetPt(const Point& v)
{
    static const int dx[8] = { 1, -1, 0, 0, 1, 1, -1, -1 };
    static const int dy[8] = { 0, 0, 1, -1, 1, -1, 1, -1 };
    unsigned d = rnd(8);
    return Point(v.x + dx[d], v.y + dy[d]);
}

static bool onBoundary(const std::vector<Point>& p, const Point& q)
{
    for (size_t i = 0; i < p.size(); ++i)
    {
        const Point& a = p[i];
        const Point& b = p[(i + 1) % p.size()];
        if ((q == a) || pointOnLine(a, b, q))
        {
            return true;
        }
    }
    return false;
}

// A closed polygon (cluster boundary) and an open conn that runs along it.
static void genClosedPair(Polygon& conn, Polygon& poly, bool& orth)
{
    std::vector<Point> p;
    unsigned kind = rnd(3);
    orth = (kind == 0);
    if (kind == 0)
    {
        double x1 = (double) rnd(4);
        double y1 = (double) rnd(4);
        double x2 = x1 + 1 + rnd(4);
        double y2 = y1 + 1 + rnd(4);
        p.push_back(Point(x2, y1));
        p.push_back(Point(x2, y2));
        p.push_back(Point(x1, y2));
        p.push_back(Point(x1, y1));
        // Optionally add mid points on the edges.
        if (rnd(2) == 0)
        {
            std::vector<Point> q;
            for (size_t i = 0; i < p.size(); ++i)
            {
                q.push_back(p[i]);
                if (rnd(2) == 0)
                {
                    const Point& n = p[(i + 1) % p.size()];
                    Point m((p[i].x + n.x) / 2, (p[i].y + n.y) / 2);
                    q.push_back(m);
                }
            }
            p = q;
        }
    }
    else
    {
        // A convex polygon: a rectangle with some corners chamfered.
        // (Self-touching polygons can defeat splitBranchingSegments and
        // trip an upstream COLA_ASSERT in countForSegment.)
        double x1 = (double) rnd(3);
        double y1 = (double) rnd(3);
        double x2 = x1 + 3 + rnd(3);
        double y2 = y1 + 3 + rnd(3);
        Point corners[4] = { Point(x2, y1), Point(x2, y2), Point(x1, y2),
                Point(x1, y1) };
        for (int i = 0; i < 4; ++i)
        {
            const Point& prev = corners[(i + 3) % 4];
            const Point& next = corners[(i + 1) % 4];
            if (rnd(3) != 0)
            {
                // Chamfer: step 1 towards prev, then 1 towards next.
                Point a = corners[i];
                Point b = corners[i];
                if (prev.x == a.x) a.y += (prev.y > a.y) ? 1 : -1;
                else a.x += (prev.x > a.x) ? 1 : -1;
                if (next.x == b.x) b.y += (next.y > b.y) ? 1 : -1;
                else b.x += (next.x > b.x) ? 1 : -1;
                p.push_back(a);
                p.push_back(b);
            }
            else
            {
                p.push_back(corners[i]);
            }
        }
    }
    // The conn follows part of the boundary, optionally starting/ending at
    // a neighbouring grid point that is not on the boundary (so that the
    // conn never touches itself, which could defeat splitBranchingSegments
    // and trip an upstream COLA_ASSERT).
    unsigned k = rnd((unsigned) p.size());
    unsigned len = 1 + rnd((unsigned) p.size());
    bool fwd = (rnd(2) == 0);
    std::vector<Point> path;
    for (unsigned i = 0; i < len; ++i)
    {
        size_t idx = (fwd) ? ((k + i) % p.size()) :
                ((k + p.size() * 4 - i) % p.size());
        path.push_back(p[idx]);
    }
    std::vector<Point> c;
    if (rnd(4) != 0)
    {
        Point q = offsetPt(path.front());
        if (!onBoundary(p, q))
        {
            c.push_back(q);
        }
    }
    c.insert(c.end(), path.begin(), path.end());
    if ((rnd(4) != 0) || (c.size() < 2))
    {
        Point q = offsetPt(path.back());
        if (!onBoundary(p, q) || (c.size() < 2))
        {
            c.push_back(q);
        }
    }
    // Shape corners: vn 0..3 for rectangles.
    for (size_t i = 0; i < p.size(); ++i)
    {
        p[i].id = rnd(4);
        if (orth && p.size() == 4)
        {
            p[i].vn = (unsigned short) i;
            rnd(8);
        }
        else
        {
            p[i].vn = (unsigned short) rnd(8);
        }
    }
    for (size_t i = 0; i < c.size(); ++i)
    {
        c[i].id = rnd(4);
        c[i].vn = (unsigned short) rnd(8);
    }
    conn = toPoly(c);
    poly = toPoly(p);
}

// ---------------------------------------------------------------------------
// Output of ConnectorCrossings results.

static std::string connName(const ConnRef *c)
{
    if (!c) return "-";
    char buf[32];
    snprintf(buf, sizeof(buf), "%u", c->id());
    return buf;
}

static std::string pairStr(const PtConnPtrPair& pr, bool validPtrs,
        const Polygon& poly, const Polygon& conn)
{
    std::string s = connName(pr.second);
    if (validPtrs)
    {
        const Point *pt = pr.first;
        char buf[64];
        if (!poly.ps.empty() && pt >= &poly.ps[0] &&
                pt < &poly.ps[0] + poly.ps.size())
        {
            snprintf(buf, sizeof(buf), ":P%d", (int) (pt - &poly.ps[0]));
        }
        else if (!conn.ps.empty() && pt >= &conn.ps[0] &&
                pt < &conn.ps[0] + conn.ps.size())
        {
            snprintf(buf, sizeof(buf), ":C%d", (int) (pt - &conn.ps[0]));
        }
        else
        {
            snprintf(buf, sizeof(buf), ":?");
        }
        s += buf;
        s += ptStr(*pt);
    }
    return s;
}

static void dumpPointOrders(PtOrderMap& orders, bool validPtrs,
        const Polygon& poly, const Polygon& conn, ConnRef *c1, ConnRef *c2)
{
    printf("  orders %d\n", (int) orders.size());
    for (PtOrderMap::iterator it = orders.begin(); it != orders.end(); ++it)
    {
        printf("   key %s\n", ptStr(it->first).c_str());
        for (size_t dim = 0; dim < 2; ++dim)
        {
            PointRepVector v = it->second.sortedPoints(dim);
            std::string s;
            for (size_t i = 0; i < v.size(); ++i)
            {
                s += " ";
                s += pairStr(v[i], validPtrs, poly, conn);
            }
            int pos1 = it->second.positionFor(dim, c1);
            int pos2 = it->second.positionFor(dim, c2);
            int posN = it->second.positionFor(dim, nullptr);
            printf("    d%d n=%d%s | %d %d %d\n", (int) dim, (int) v.size(),
                    s.c_str(), pos1, pos2, posN);
        }
    }
}

static void dumpSegmentResult(size_t i, const ConnectorCrossings& cross)
{
    printf("  seg %d count=%u flags=%u first=%s second=%s\n", (int) i,
            cross.crossingCount, cross.crossingFlags,
            g17(cross.firstSharedPathAtEndLength).c_str(),
            g17(cross.secondSharedPathAtEndLength).c_str());
}

// Whether countForSegment would overflow its c_path/p_path arrays (UB
// upstream) for a closed poly: predicted by doing its split on copies.
static bool wouldOverflow(const Polygon& poly, bool polyIsConn,
        const Polygon& conn, bool willSplit)
{
    Polygon p2 = poly;
    Polygon c2 = conn;
    if (willSplit)
    {
        double tolerance = (!polyIsConn) ?
                std::numeric_limits<double>::epsilon() : 0.0;
        splitBranchingSegments(p2, polyIsConn, c2, tolerance);
    }
    return c2.size() > p2.size();
}

// ---------------------------------------------------------------------------
// Sections.

static void sectionSplit(unsigned n)
{
    printf("### split\n");
    for (unsigned t = 0; t < n; ++t)
    {
        Polygon conn, poly;
        bool polyIsConn = (rnd(3) != 0);
        bool orth = false;
        if (polyIsConn)
        {
            orth = (rnd(3) != 0);
            genConnPair(orth, conn, poly);
        }
        else
        {
            genClosedPair(conn, poly, orth);
        }
        double tolerance = 0;
        if (rnd(2) == 0)
        {
            tolerance = std::numeric_limits<double>::epsilon();
        }
        printf("case %u pic=%d tol=%s\n", t, (int) polyIsConn,
                g17(tolerance).c_str());
        printf("  poly %s\n", polyStr(poly).c_str());
        printf("  conn %s\n", polyStr(conn).c_str());
        splitBranchingSegments(poly, polyIsConn, conn, tolerance);
        printf("  poly' %s\n", polyStr(poly).c_str());
        printf("  conn' %s\n", polyStr(conn).c_str());
    }
}

// Orthogonal splits with vertex numbers drawn from {0..3, 8} so that every
// fallback branch of midVertexNumber() is exercised.
static void sectionSplitVn(unsigned n)
{
    printf("### splitvn\n");
    for (unsigned t = 0; t < n; ++t)
    {
        Polygon conn, poly;
        genConnPair(true, conn, poly);
        Polygon *ps[2] = { &conn, &poly };
        for (int k = 0; k < 2; ++k)
        {
            for (size_t i = 0; i < ps[k]->ps.size(); ++i)
            {
                unsigned vn = 8;
                if (rnd(2) == 0)
                {
                    vn = rnd(4);
                }
                ps[k]->ps[i].vn = (unsigned short) vn;
            }
        }
        printf("case %u\n", t);
        printf("  poly %s\n", polyStr(poly).c_str());
        printf("  conn %s\n", polyStr(conn).c_str());
        splitBranchingSegments(poly, true, conn);
        printf("  poly' %s\n", polyStr(poly).c_str());
        printf("  conn' %s\n", polyStr(conn).c_str());
    }
}

// Closed polygons with a conn starting in the middle of one of the edges
// (including the closing edge back()->front()).
static void sectionSplitClosed(unsigned n)
{
    printf("### splitclosed\n");
    for (unsigned t = 0; t < n; ++t)
    {
        Polygon conn, poly;
        bool orth;
        genClosedPair(conn, poly, orth);
        unsigned k = rnd((unsigned) poly.size());
        const Point& a = poly.ps[k];
        const Point& b = poly.ps[(k + 1) % poly.size()];
        std::vector<Point> c;
        c.push_back(Point((a.x + b.x) / 2, (a.y + b.y) / 2));
        unsigned cn = 1 + rnd(4);
        for (unsigned i = 0; i < cn; ++i)
        {
            Point q = gridPt(7);
            if (q == c.back())
            {
                q.x += 1;
            }
            c.push_back(q);
        }
        for (size_t i = 0; i < c.size(); ++i)
        {
            c[i].id = rnd(4);
            c[i].vn = (unsigned short) rnd(8);
        }
        conn = toPoly(c);
        double tolerance = 0;
        if (rnd(2) == 0)
        {
            tolerance = std::numeric_limits<double>::epsilon();
        }
        printf("case %u tol=%s\n", t, g17(tolerance).c_str());
        printf("  poly %s\n", polyStr(poly).c_str());
        printf("  conn %s\n", polyStr(conn).c_str());
        splitBranchingSegments(poly, false, conn, tolerance);
        printf("  poly' %s\n", polyStr(poly).c_str());
        printf("  conn' %s\n", polyStr(conn).c_str());
    }
}

static ConnEnd randomEnd(JunctionRef *j1, JunctionRef *j2)
{
    unsigned k = rnd(3);
    if (k == 1) return ConnEnd(j1);
    if (k == 2) return ConnEnd(j2);
    double x = (double) rnd(50);
    double y = (double) rnd(50);
    return ConnEnd(Point(x + 300, y + 300));
}

static void sectionCrossings(unsigned n)
{
    printf("### crossings\n");
    Router *nullRouter = new Router(OrthogonalRouting);
    for (unsigned t = 0; t < n; ++t)
    {
        // kind 0: conn vs conn with ConnRefs; 1: conn vs conn, no ConnRefs;
        // 2: conn vs closed polygon (cluster boundary), no ConnRefs.
        unsigned kr = rnd(10);
        unsigned kind = (kr < 7) ? 0 : ((kr < 8) ? 1 : 2);
        Polygon conn, poly;
        bool polyIsConn = (kind != 2);
        bool orth = true;
        if (kind == 2)
        {
            genClosedPair(conn, poly, orth);
        }
        else
        {
            orth = (rnd(4) != 0);
            genConnPair(orth, conn, poly);
        }

        Router *router = nullRouter;
        ConnRef *c1 = nullptr;
        ConnRef *c2 = nullptr;
        std::string setup;
        if (kind == 0)
        {
            router = new Router(OrthogonalRouting);
            JunctionRef *j1 = new JunctionRef(router, Point(100, 100), 10);
            JunctionRef *j2 = new JunctionRef(router, Point(200, 200), 11);
            ConnEnd e1 = randomEnd(j1, j2);
            ConnEnd e2 = randomEnd(j1, j2);
            c1 = new ConnRef(router, e1, e2, 1);
            ConnEnd e3 = randomEnd(j1, j2);
            ConnEnd e4 = randomEnd(j1, j2);
            c2 = new ConnRef(router, e3, e4, 2);
            router->processActions();
            ConnRef *cs[2] = { c1, c2 };
            for (int k = 0; k < 2; ++k)
            {
                bool poly = (rnd(5) == 0);
                if (!orth && rnd(2) == 0)
                {
                    poly = true;
                }
                cs[k]->m_type = poly ? ConnType_PolyLine : ConnType_Orthogonal;
                cs[k]->m_has_fixed_route = (rnd(6) == 0);
                char buf[64];
                snprintf(buf, sizeof(buf), " c%d:%s%s[%s,%s]", k + 1,
                        poly ? "poly" : "orth",
                        cs[k]->m_has_fixed_route ? ",fixed" : "",
                        cs[k]->endpointConnEnds().first.junction() ?
                            "J" : "p",
                        cs[k]->endpointConnEnds().second.junction() ?
                            "J" : "p");
                setup += buf;
            }
        }
        bool checkBranching = (rnd(3) == 0);
        bool preSplit = polyIsConn && (rnd(2) == 0);
        bool wantPoints = (rnd(4) != 0);
        bool wantShared = (rnd(2) == 0);
        bool wantOrders = (rnd(4) != 0);

        printf("case %u kind=%u orth=%d cb=%d pre=%d%s\n", t, kind, (int) orth,
                (int) checkBranching, (int) preSplit, setup.c_str());
        printf("  poly %s\n", polyStr(poly).c_str());
        printf("  conn %s\n", polyStr(conn).c_str());
        if (preSplit)
        {
            // As ImproveOrthogonalRoutes::buildOrthogonalNudgingOrderInfo.
            splitBranchingSegments(poly, true, conn);
            splitBranchingSegments(conn, true, poly);
            printf("  pre poly %s\n", polyStr(poly).c_str());
            printf("  pre conn %s\n", polyStr(conn).c_str());
        }

        bool polyIsOrthogonal = (c1 && (c1->routingType() == ConnType_Orthogonal));
        bool connIsOrthogonal = (c2 && (c2->routingType() == ConnType_Orthogonal));
        bool willSplit = checkBranching || (c1 && c1->hasFixedRoute()) ||
                (c2 && c2->hasFixedRoute()) || !polyIsOrthogonal ||
                !connIsOrthogonal;

        PointSet crossingPoints;
        SharedPathList sharedPaths;
        PtOrderMap pointOrders;
        ConnectorCrossings cross(poly, polyIsConn, conn, c1, c2);
        cross.checkForBranchingSegments = checkBranching;
        if (wantPoints) cross.crossingPoints = &crossingPoints;
        if (wantShared) cross.sharedPaths = &sharedPaths;
        if (wantOrders) cross.pointOrders = &pointOrders;
        unsigned total = 0;
        unsigned flags = 0;
        bool skipped = false;
        for (size_t i = 1; i < conn.size(); ++i)
        {
            if (!polyIsConn && wouldOverflow(poly, polyIsConn, conn, willSplit))
            {
                printf("  skip-overflow\n");
                skipped = true;
                break;
            }
            const bool finalSegment = ((i + 1) == conn.size());
            cross.countForSegment(i, finalSegment);
            dumpSegmentResult(i, cross);
            total += cross.crossingCount;
            flags |= cross.crossingFlags;
        }
        if (!skipped)
        {
            printf("  total=%u flags=%u\n", total, flags);
            printf("  poly' %s\n", polyStr(poly).c_str());
            printf("  conn' %s\n", polyStr(conn).c_str());
            if (wantPoints)
            {
                std::string s;
                for (PointSet::iterator it = crossingPoints.begin();
                        it != crossingPoints.end(); ++it)
                {
                    s += " " + ptStr(*it);
                }
                printf("  points%s\n", s.c_str());
            }
            if (wantShared)
            {
                printf("  shared %d\n", (int) sharedPaths.size());
                for (size_t i = 0; i < sharedPaths.size(); ++i)
                {
                    std::string s;
                    for (size_t k = 0; k < sharedPaths[i].size(); ++k)
                    {
                        s += " " + ptStr(sharedPaths[i][k]);
                    }
                    printf("   path%s\n", s.c_str());
                }
            }
            if (wantOrders)
            {
                dumpPointOrders(pointOrders, !willSplit, poly, conn, c1, c2);
            }
        }
        if (router != nullRouter)
        {
            delete router;
        }
    }
    delete nullRouter;
}

static void sectionPtOrder(unsigned n)
{
    printf("### ptorder\n");
    Router *router = new Router(OrthogonalRouting);
    ConnRef *conns[8];
    for (int i = 0; i < 8; ++i)
    {
        conns[i] = new ConnRef(router, 20 + i);
    }
    Point pts[16];
    Polygon dummy;
    for (unsigned t = 0; t < n; ++t)
    {
        PtOrder ord;
        unsigned nconn = 2 + rnd(6);
        unsigned ops = 1 + rnd(14);
        printf("case %u nconn=%u ops=%u\n", t, nconn, ops);
        for (unsigned o = 0; o < ops; ++o)
        {
            unsigned kind = rnd(12);
            size_t dim = rnd(2);
            if (kind < 9)
            {
                unsigned pi = rnd(16);
                unsigned pj = (pi + 1 + rnd(15)) % 16;
                unsigned ci = rnd(nconn + 1);
                unsigned cj = rnd(nconn + 1);
                ConnRef *a = (ci == nconn) ? nullptr : conns[ci];
                ConnRef *b = (cj == nconn) ? nullptr : conns[cj];
                if (kind < 2)
                {
                    printf("  add %d P%u:%s P%u:%s\n", (int) dim, pi,
                            connName(a).c_str(), pj, connName(b).c_str());
                    ord.addPoints(dim, std::make_pair(&pts[pi], a),
                            std::make_pair(&pts[pj], b));
                }
                else
                {
                    bool swapped = (rnd(2) == 0);
                    printf("  ord %d P%u:%s P%u:%s %d\n", (int) dim, pi,
                            connName(a).c_str(), pj, connName(b).c_str(),
                            (int) swapped);
                    ord.addOrderedPoints(dim, std::make_pair(&pts[pi], a),
                            std::make_pair(&pts[pj], b), swapped);
                }
            }
            else if (kind < 11)
            {
                PointRepVector v = ord.sortedPoints(dim);
                std::string s;
                for (size_t i = 0; i < v.size(); ++i)
                {
                    char buf[32];
                    snprintf(buf, sizeof(buf), " P%d:",
                            (int) (v[i].first - &pts[0]));
                    s += buf + connName(v[i].second);
                }
                printf("  sorted %d n=%d%s\n", (int) dim, (int) v.size(),
                        s.c_str());
            }
            else
            {
                unsigned ci = rnd(nconn + 1);
                ConnRef *a = (ci == nconn) ? nullptr : conns[ci];
                printf("  pos %d %s = %d\n", (int) dim, connName(a).c_str(),
                        ord.positionFor(dim, a));
            }
        }
        for (size_t dim = 0; dim < 2; ++dim)
        {
            PointRepVector v = ord.sortedPoints(dim);
            std::string s;
            for (size_t i = 0; i < v.size(); ++i)
            {
                char buf[32];
                snprintf(buf, sizeof(buf), " P%d:",
                        (int) (v[i].first - &pts[0]));
                s += buf + connName(v[i].second);
            }
            std::string p;
            for (unsigned ci = 0; ci <= nconn; ++ci)
            {
                ConnRef *a = (ci == nconn) ? nullptr : conns[ci];
                char buf[32];
                snprintf(buf, sizeof(buf), " %d", ord.positionFor(dim, a));
                p += buf;
            }
            printf("  final %d n=%d%s |%s\n", (int) dim, (int) v.size(),
                    s.c_str(), p.c_str());
        }
    }
    delete router;
}

int main(int argc, char **argv)
{
    sectionSplit(1500);
    sectionSplitVn(1500);
    sectionSplitClosed(1500);
    sectionCrossings(6000);
    sectionPtOrder(1500);
    return 0;
}
