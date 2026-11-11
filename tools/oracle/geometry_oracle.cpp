// Differential oracle for the Rust port of libavoid geomtypes/geometry
// (plus EdgeInf::rotationLessThan and std::list::sort of orthogVisList).
//
// The Rust test `tests/geometry_oracle.rs` regenerates exactly the same
// inputs (same LCG, same consumption order) and must reproduce this
// program's output, saved as `tests/golden/geometry_oracle.txt`.
//
// Build & regenerate (from the repository root; VENDOR is the vendored
// adaptagrams checkout, which is gitignored):
//
//   VENDOR=$PWD/vendor/adaptagrams/cola
//   g++ -std=c++11 -O2 -ffp-contract=off -I"$VENDOR" \
//       tools/oracle/geometry_oracle.cpp "$VENDOR"/libavoid/*.cpp \
//       -o /tmp/geometry_oracle
//   /tmp/geometry_oracle > tests/golden/geometry_oracle.txt
//
// (-ffp-contract=off guarantees no FMA contraction; asserts stay enabled.)
//
// IMPORTANT: never put two RNG calls in one C++ expression (argument
// evaluation order is unspecified); always sequence them in statements.

#include <cstdio>
#include <cstdint>
#include <string>
#include <vector>
#include <list>
#include <limits>

#include "libavoid/geomtypes.h"
#include "libavoid/geometry.h"
#include "libavoid/vertices.h"
#include "libavoid/graph.h"
#include "libavoid/router.h"

using namespace Avoid;

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

// A coordinate biased towards degenerate configurations.
static double coord()
{
    unsigned mode = rnd(4);
    if (mode == 0)
    {
        return (double) rnd(5);
    }
    else if (mode == 1)
    {
        return (double) rnd(9) / 2.0;
    }
    else if (mode == 2)
    {
        return unit() * 20.0 - 10.0;
    }
    return (double) rnd(7) - 3.0;
}

static Point point()
{
    double x = coord();
    double y = coord();
    return Point(x, y);
}

// NaNs are printed as "nan" regardless of sign bit: the sign of a NaN
// produced by x86 SSE depends on operand order chosen by the compiler.
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

static void pd(double v)
{
    printf(" %s", g17(v).c_str());
}

static void pp(const Point& p)
{
    printf(" (%s,%s)", g17(p.x).c_str(), g17(p.y).c_str());
}

static void ppFull(const Point& p)
{
    printf(" (%s,%s,%u,%u)", g17(p.x).c_str(), g17(p.y).c_str(), p.id,
            (unsigned) p.vn);
}

static void printPoly(const Polygon& poly)
{
    printf(" id=%d n=%u", poly._id, (unsigned) poly.size());
    for (size_t i = 0; i < poly.ps.size(); ++i)
    {
        ppFull(poly.ps[i]);
    }
    printf(" ts=");
    for (size_t i = 0; i < poly.ts.size(); ++i)
    {
        printf("%c", poly.ts[i]);
    }
    printf(" cps=%u", (unsigned) poly.checkpointsOnRoute.size());
    for (size_t i = 0; i < poly.checkpointsOnRoute.size(); ++i)
    {
        printf(" [%u", (unsigned) poly.checkpointsOnRoute[i].first);
        pp(poly.checkpointsOnRoute[i].second);
        printf("]");
    }
}

static Polygon randomPoly(unsigned minN, unsigned maxN)
{
    unsigned n = minN + rnd(maxN - minN + 1);
    Polygon poly;
    poly._id = (int) rnd(100);
    for (unsigned i = 0; i < n; ++i)
    {
        Point p = point();
        poly.ps.push_back(p);
    }
    return poly;
}

static const int N = 500;

int main()
{
    // ---------------- hand-picked degenerate points ----------------
    std::vector<Point> special;
    special.push_back(Point(0, 0));
    special.push_back(Point(1, 0));
    special.push_back(Point(0, 1));
    special.push_back(Point(1, 1));
    special.push_back(Point(2, 2));
    special.push_back(Point(-1, -1));
    special.push_back(Point(2, 0));
    special.push_back(Point(0.5, 0.5));
    special.push_back(Point(1e-300, 0));
    special.push_back(Point(1e300, -1e300));
    special.push_back(Point(-0.0, 0.0));
    special.push_back(Point(0.1, 0.2));
    special.push_back(Point(0.3, 0.6));
    const size_t S = special.size();

    // vecDir / projection / colinear / inBetween / pointOnLine /
    // cornerSide / angle over all special triples.
    for (size_t i = 0; i < S; ++i)
    for (size_t j = 0; j < S; ++j)
    for (size_t k = 0; k < S; ++k)
    {
        const Point& a = special[i];
        const Point& b = special[j];
        const Point& c = special[k];
        printf("S3 %u %u %u", (unsigned) i, (unsigned) j, (unsigned) k);
        printf(" vd=%d vdz=%d", vecDir(a, b, c), vecDir(a, b, c, 0.5));
        printf(" col=%d colt=%d", (int) colinear(a, b, c),
                (int) colinear(a, b, c, 0.5));
        printf(" pol=%d", (int) pointOnLine(a, b, c));
        if (vecDir(a, b, c, std::numeric_limits<double>::epsilon()) == 0)
        {
            printf(" ib=%d", (int) inBetween(a, b, c));
        }
        printf(" cs=%d", cornerSide(a, b, c, special[(i + j + k) % S]));
        printf(" proj");
        pp(projection(a, b, c));
        printf(" ang");
        pd(angle(a, b, c));
        printf("\n");
    }

    // ---------------- random point-level functions ----------------
    for (int it = 0; it < N; ++it)
    {
        Point a = point();
        Point b = point();
        Point c = point();
        Point d = point();
        printf("P %d", it);
        pp(a); pp(b); pp(c); pp(d);
        printf(" vd=%d vdz=%d", vecDir(a, b, c), vecDir(a, b, c, 0.25));
        printf(" col=%d colt=%d", (int) colinear(a, b, c),
                (int) colinear(a, b, c, 0.25));
        printf(" pol=%d", (int) pointOnLine(a, b, c));
        const double eps = std::numeric_limits<double>::epsilon();
        bool generalCase = (a.x != b.x) && (a.y != b.y);
        if (!generalCase || (vecDir(a, b, c, 0.25) != 0) ||
                (vecDir(a, b, c, eps) == 0))
        {
            printf(" polt=%d", (int) pointOnLine(a, b, c, 0.25));
        }
        if (vecDir(a, b, c, eps) == 0)
        {
            printf(" ib=%d", (int) inBetween(a, b, c));
        }
        printf(" si=%d", (int) segmentIntersect(a, b, c, d));
        bool seen = false;
        bool r1 = segmentShapeIntersect(a, b, c, d, seen);
        printf(" ssi=%d,%d", (int) r1, (int) seen);
        seen = true;
        bool r2 = segmentShapeIntersect(a, b, c, d, seen);
        printf(" ssi2=%d,%d", (int) r2, (int) seen);
        printf(" ivr=%d,%d", (int) inValidRegion(true, a, b, c, d),
                (int) inValidRegion(false, a, b, c, d));
        printf(" cs=%d", cornerSide(a, b, c, d));
        printf(" ed"); pd(euclideanDist(a, d));
        printf(" md"); pd(manhattanDist(a, d));
        printf(" ang"); pd(angle(a, b, c));
        printf(" proj"); pp(projection(a, b, c));
        double x = -12345.0, y = -12345.0;
        int sip = segmentIntersectPoint(a, b, c, d, &x, &y);
        printf(" sip=%d", sip); pd(x); pd(y);
        x = -12345.0; y = -12345.0;
        int rip = rayIntersectPoint(a, b, c, d, &x, &y);
        printf(" rip=%d", rip); pd(x); pd(y);
        printf(" ra"); pd(rotationalAngle(a - d));
        printf(" peq=%d,%d,%d,%d,%d", (int) (a == b), (int) (a != b),
                (int) (a < b), (int) a.equals(b), (int) a.equals(b, 0.5));
        Point sum = a + b;
        Point diff = a - b;
        printf(" sum"); ppFull(sum);
        printf(" diff"); ppFull(diff);
        printf(" idx"); pd(a[0]); pd(a[1]);
        printf("\n");
    }

    // ---------------- polygon-level functions ----------------
    for (int it = 0; it < N; ++it)
    {
        Polygon poly = randomPoly(0, 7);
        unsigned ncp = rnd(4);
        for (unsigned i = 0; i < ncp; ++i)
        {
            size_t maxVal = (poly.size() > 0) ? (2 * poly.size() - 1) : 1;
            size_t v = rnd((unsigned) maxVal);
            Point cp = point();
            poly.checkpointsOnRoute.push_back(std::make_pair(v, cp));
        }
        Point q = point();
        double offset = coord();
        double curve = unit() * 3.0;

        printf("G %d", it);
        printPoly(poly);
        printf(" q"); pp(q);
        printf(" off"); pd(offset);
        printf(" curve"); pd(curve);
        printf("\n");

        printf("  tl"); pd(totalLength(poly)); printf("\n");
        printf("  ipg=%d", (int) inPolyGen(poly, q));
        if (poly.size() > 0)
        {
            printf(" ip=%d,%d", (int) inPoly(poly, q, true),
                    (int) inPoly(poly, q, false));
        }
        printf("\n");
        if (poly.size() > 0)
        {
            Box bb = poly.offsetBoundingBox(offset);
            printf("  obb"); pp(bb.min); pp(bb.max);
            pd(bb.width()); pd(bb.height()); pd(bb.length(0)); pd(bb.length(1));
            printf("\n");
            printf("  brp"); printPoly(poly.boundingRectPolygon()); printf("\n");
        }
        if (poly.size() > 0)
        {
            printf("  op"); printPoly(poly.offsetPolygon(offset)); printf("\n");
            printf("  op0"); printPoly(poly.offsetPolygon(0)); printf("\n");
        }
        printf("  simp"); printPoly(poly.simplify()); printf("\n");
        if (poly.size() > 0)
        {
            printf("  cpl"); printPoly(poly.curvedPolyline(curve)); printf("\n");
        }
        if (poly.size() > 2)
        {
            printf("  cplc"); printPoly(poly.curvedPolyline(curve, true));
            printf("\n");
        }
        for (size_t seg = 0; seg + 1 < poly.size(); ++seg)
        {
            for (int mod = -1; mod <= 1; ++mod)
            {
                std::vector<Point> cps = poly.checkpointsOnSegment(seg, mod);
                printf("  cos %u %d n=%u", (unsigned) seg, mod,
                        (unsigned) cps.size());
                for (size_t i = 0; i < cps.size(); ++i)
                {
                    pp(cps[i]);
                }
                printf("\n");
            }
        }
        Polygon copy(poly);
        copy.translate(offset, curve);
        printf("  tr"); printPoly(copy); printf("\n");
        Polygon fromIface((const PolygonInterface&) poly);
        printf("  fi"); printPoly(fromIface); printf("\n");

        Point p1 = point();
        Point p2 = point();
        double w = coord();
        double h = coord();
        printf("  rect"); printPoly(Rectangle(p1, p2)); printf("\n");
        printf("  rectc"); printPoly(Rectangle(p1, w, h)); printf("\n");
    }

    // ---------------- EdgeInf::rotationLessThan + list sort ----------------
    {
        Router router(OrthogonalRouting);
        for (int it = 0; it < 300; ++it)
        {
            double cx = (double) rnd(5);
            double cy = (double) rnd(5);
            VertInf *centre = new VertInf(&router, VertID(1, 0), Point(cx, cy),
                    false);
            unsigned n = 2 + rnd(7);
            std::vector<VertInf *> others;
            std::vector<EdgeInf *> edges;
            for (unsigned i = 0; i < n; ++i)
            {
                unsigned dir = rnd(5);
                double dist = (double) (1 + rnd(3));
                Point p(cx, cy);
                if (dir == 0) p.x += dist;
                else if (dir == 1) p.x -= dist;
                else if (dir == 2) p.y += dist;
                else if (dir == 3) p.y -= dist;
                else
                {
                    // Not orthogonally positioned.
                    p.x += dist;
                    p.y += 1;
                }
                VertInf *o = new VertInf(&router, VertID(2, (unsigned short) i),
                        p, false);
                others.push_back(o);
                bool swapEnds = (rnd(2) == 1);
                EdgeInf *e = (swapEnds) ? new EdgeInf(o, centre, true) :
                        new EdgeInf(centre, o, true);
                edges.push_back(e);
            }
            // Previous vertex: none, or an arbitrary point.
            VertInf *last = nullptr;
            unsigned lastMode = rnd(3);
            if (lastMode != 0)
            {
                Point lp = point();
                if (lastMode == 1)
                {
                    // Orthogonally aligned with the centre.
                    if (rnd(2) == 0) lp.y = cy; else lp.x = cx;
                }
                last = new VertInf(&router, VertID(3, 0), lp, false);
            }
            printf("R %d", it);
            ppFull(centre->point);
            for (unsigned i = 0; i < n; ++i)
            {
                pp(others[i]->point);
            }
            if (last) pp(last->point); else printf(" null");
            printf("\n  ");
            for (unsigned i = 0; i < n; ++i)
            {
                for (unsigned j = 0; j < n; ++j)
                {
                    printf("%d", (int) edges[i]->rotationLessThan(last,
                                edges[j]));
                }
                printf(" ");
            }
            printf("\n");

            // Activate the orthogonal edges and sort the centre's
            // orthogVisList the way makepath.cpp does.  Only orthogonal
            // edges take part, so the comparator never compares pointers.
            std::vector<unsigned> active;
            for (unsigned i = 0; i < n; ++i)
            {
                if (edges[i]->isOrthogonal())
                {
                    edges[i]->setDist(1.0 + i);
                    active.push_back(i);
                }
            }
            printf("  pre");
            for (EdgeInfList::iterator e = centre->orthogVisList.begin();
                    e != centre->orthogVisList.end(); ++e)
            {
                for (unsigned i = 0; i < n; ++i)
                {
                    if (edges[i] == *e) printf(" %u", i);
                }
            }
            struct Cmp
            {
                const VertInf *lastPt;
                bool operator()(const EdgeInf *u, const EdgeInf *v) const
                {
                    return u->rotationLessThan(lastPt, v);
                }
            } cmp = { last };
            centre->orthogVisList.sort(cmp);
            printf(" sorted");
            for (EdgeInfList::iterator e = centre->orthogVisList.begin();
                    e != centre->orthogVisList.end(); ++e)
            {
                for (unsigned i = 0; i < n; ++i)
                {
                    if (edges[i] == *e) printf(" %u", i);
                }
            }
            printf(" graph=%d\n", router.visOrthogGraph.size());

            for (unsigned i = 0; i < n; ++i)
            {
                delete edges[i];
            }
            for (unsigned i = 0; i < n; ++i)
            {
                delete others[i];
            }
            delete centre;
            delete last;
        }
    }

    // ------- poly-line visibility: checkEdgeVisibility / directVis -------
    for (int it = 0; it < 80; ++it)
    {
        Router router(PolyLineRouting);
        router.IgnoreRegions = (rnd(2) == 0);
        router.InvisibilityGrph = (rnd(3) != 0);
        unsigned nShapes = 1 + rnd(4);
        std::vector<VertInf *> all;
        std::vector<Polygon> rects;
        printf("V %d ir=%d ig=%d", it, (int) router.IgnoreRegions,
                (int) router.InvisibilityGrph);
        for (unsigned s = 0; s < nShapes; ++s)
        {
            Point p1 = point();
            Point p2 = point();
            Polygon r = Rectangle(p1, p2);
            rects.push_back(r);
            unsigned id = 10 + s;
            VertInf *first = nullptr;
            VertInf *last = nullptr;
            for (size_t k = 0; k < r.size(); ++k)
            {
                VertInf *v = new VertInf(&router,
                        VertID(id, (unsigned short) k), r.ps[k], true);
                all.push_back(v);
                if (!first)
                {
                    first = v;
                }
                else
                {
                    v->shPrev = last;
                    last->shNext = v;
                }
                last = v;
            }
            last->shNext = first;
            first->shPrev = last;
            printf(" shape%u", id);
            pp(r.ps[0]);
            pp(r.ps[2]);
        }
        unsigned nConns = 1 + rnd(3);
        for (unsigned c = 0; c < nConns; ++c)
        {
            Point p = point();
            VertID vid(100 + c, VertID::src, VertID::PROP_ConnPoint);
            VertInf *v = new VertInf(&router, vid, p, true);
            all.push_back(v);
            for (unsigned s = 0; s < nShapes; ++s)
            {
                if (inPoly(rects[s], p, false))
                {
                    router.contains[vid].insert(10 + s);
                }
            }
            printf(" conn%u", 100 + c);
            pp(p);
        }
        printf("\n");

        for (int pass = 0; pass < 2; ++pass)
        {
            for (VertInf *i = router.vertices.connsBegin(); i; i = i->lstNext)
            {
                for (VertInf *j = i->lstNext; j; j = j->lstNext)
                {
                    EdgeInf *e = EdgeInf::checkEdgeVisibility(i, j);
                    printf("  ce%d %u:%u %u:%u", pass, i->id.objID,
                            (unsigned) i->id.vn, j->id.objID,
                            (unsigned) j->id.vn);
                    if (e)
                    {
                        printf(" added=%d blocker=%d dist", (int) e->added(),
                                e->blocker());
                        pd(e->getDist());
                    }
                    else
                    {
                        printf(" null");
                    }
                    printf(" dv=%d\n", (int) directVis(i, j));
                }
            }
        }
        printf("  checked=%d vis=%d invis=%d\n", router.st_checked_edges,
                router.visGraph.size(), router.invisGraph.size());
        for (VertInf *i = router.vertices.connsBegin(); i; i = i->lstNext)
        {
            printf("  vl %u:%u n=%u,%u:", i->id.objID, (unsigned) i->id.vn,
                    i->visListSize, i->invisListSize);
            for (EdgeInfList::iterator e = i->visList.begin();
                    e != i->visList.end(); ++e)
            {
                VertInf *o = (*e)->otherVert(i);
                printf(" %u:%u", o->id.objID, (unsigned) o->id.vn);
            }
            printf(" |");
            for (EdgeInfList::iterator e = i->invisList.begin();
                    e != i->invisList.end(); ++e)
            {
                VertInf *o = (*e)->otherVert(i);
                printf(" %u:%u", o->id.objID, (unsigned) o->id.vn);
            }
            printf("\n");
        }
        printf("  vg");
        for (EdgeInf *e = router.visGraph.begin(); e; e = e->lstNext)
        {
            std::pair<VertID, VertID> ids = e->ids();
            printf(" %u:%u-%u:%u", ids.first.objID, (unsigned) ids.first.vn,
                    ids.second.objID, (unsigned) ids.second.vn);
        }
        printf("\n  ig");
        for (EdgeInf *e = router.invisGraph.begin(); e; e = e->lstNext)
        {
            std::pair<VertID, VertID> ids = e->ids();
            printf(" %u:%u-%u:%u/%d", ids.first.objID, (unsigned) ids.first.vn,
                    ids.second.objID, (unsigned) ids.second.vn, e->blocker());
        }
        printf("\n");

        // Remove the first connector vertex's edges, orphan a shape vertex.
        VertInf *firstConn = router.vertices.connsBegin();
        firstConn->removeFromGraph();
        VertInf *firstShape = router.vertices.shapesBegin();
        firstShape->orphan();
        printf("  after vis=%d invis=%d orphaned=%d,%d\n",
                router.visGraph.size(), router.invisGraph.size(),
                (int) firstConn->orphaned(), (int) firstShape->orphaned());

        router.visGraph.clear();
        router.invisGraph.clear();
        for (size_t k = 0; k < all.size(); ++k)
        {
            router.vertices.removeVertex(all[k]);
            delete all[k];
        }
    }
    return 0;
}
