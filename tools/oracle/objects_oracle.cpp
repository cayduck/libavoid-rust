// Differential oracle for the Rust port of the libavoid object layer and
// Router core (obstacle/shape/junction/connend/connectionpin/viscluster/
// actioninfo/ConnRef/router state handling).
//
// The Rust test `tests/objects_oracle.rs` replays exactly the same
// scenarios and must reproduce this program's output, saved as
// `tests/golden/objects_oracle.txt`.  No scenario performs routing: they
// use orthogonal-only routers (so no polyline visibility is computed) and
// call Router::processActions() directly instead of processTransaction()
// (which would build the orthogonal visibility graph and route), or only
// inspect queued state.
//
// Build & regenerate (from the repository root; VENDOR is the vendored
// adaptagrams checkout, which is gitignored):
//
//   VENDOR=$PWD/vendor/adaptagrams/cola
//   g++ -std=c++11 -O2 -ffp-contract=off -fno-access-control -I"$VENDOR" \
//       tools/oracle/objects_oracle.cpp "$VENDOR"/libavoid/*.cpp \
//       -o /tmp/objects_oracle
//   (cd /tmp && ./objects_oracle) > tests/golden/objects_oracle.txt
//
// (-fno-access-control lets us read private members; asserts stay enabled.
// The program writes and removes one scratch file, objects_oracle_diagram.txt,
// in the current directory.)

#include <cstdio>
#include <cstdlib>
#include <string>
#include <vector>
#include <list>

#include "libavoid/libavoid.h"
#include "libavoid/router.h"
#include "libavoid/shape.h"
#include "libavoid/junction.h"
#include "libavoid/connector.h"
#include "libavoid/connend.h"
#include "libavoid/connectionpin.h"
#include "libavoid/viscluster.h"
#include "libavoid/actioninfo.h"
#include "libavoid/obstacle.h"
#include "libavoid/vertices.h"

using namespace Avoid;

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

static std::string ptFull(const Point& p)
{
    char buf[64];
    snprintf(buf, sizeof(buf), ",%u,%u)", p.id, (unsigned) p.vn);
    return "(" + g17(p.x) + "," + g17(p.y) + buf;
}

static std::string poly(const Polygon& p)
{
    std::string s = "[";
    for (size_t i = 0; i < p.ps.size(); ++i)
    {
        if (i > 0) s += " ";
        s += ptFull(p.ps[i]);
    }
    return s + "]";
}

static std::string vert(const VertInf *v)
{
    if (v == nullptr)
    {
        return "-";
    }
    char buf[96];
    snprintf(buf, sizeof(buf), "[%u,%u,p=%u] ", v->id.objID,
            (unsigned) v->id.vn, (unsigned) v->id.props);
    char buf2[32];
    snprintf(buf2, sizeof(buf2), " vis=%u", v->visDirections);
    return buf + ptFull(v->point) + buf2;
}

static std::string pinName(const ShapeConnectionPin *pin)
{
    if (pin == nullptr)
    {
        return "-";
    }
    char buf[64];
    snprintf(buf, sizeof(buf), "%u:%u", pin->containingObjectId(),
            pin->m_class_id);
    return buf;
}

static std::string connEnd(const ConnEnd& ce)
{
    char buf[256];
    std::string anchor = "-";
    if (ce.m_anchor_obj)
    {
        snprintf(buf, sizeof(buf), "%u", ce.m_anchor_obj->id());
        anchor = buf;
    }
    std::string conn = "-";
    if (ce.m_conn_ref)
    {
        snprintf(buf, sizeof(buf), "%u", ce.m_conn_ref->id());
        conn = buf;
    }
    snprintf(buf, sizeof(buf), "type=%d dirs=%u/%u class=%u anchor=%s "
            "conn=%s pin=%s pt=", (int) ce.m_type, ce.m_directions,
            ce.directions(), ce.m_connection_pin_class_id, anchor.c_str(),
            conn.c_str(), pinName(ce.m_active_pin).c_str());
    return buf + pt(ce.m_point) + " pos=" + pt(ce.position());
}

static void dumpPin(const ShapeConnectionPin *pin)
{
    printf("    pin %s x=%s y=%s inside=%s prop=%d visdirs=%u dirs=%u "
            "excl=%d cost=%s users=%u pos=%s v=%s\n",
            pinName(pin).c_str(), g17(pin->m_x_offset).c_str(),
            g17(pin->m_y_offset).c_str(), g17(pin->m_inside_offset).c_str(),
            (int) pin->m_using_proportional_offsets,
            pin->m_visibility_directions, pin->directions(),
            (int) pin->m_exclusive, g17(pin->m_connection_cost).c_str(),
            (unsigned) pin->m_connend_users.size(),
            ptFull(pin->position()).c_str(), vert(pin->m_vertex).c_str());
}

static void dumpObstacle(Obstacle *o)
{
    ShapeRef *shape = dynamic_cast<ShapeRef *>(o);
    JunctionRef *junction = dynamic_cast<JunctionRef *>(o);
    printf("  obstacle id=%u kind=%s active=%d poly=%s pos=%s\n", o->id(),
            shape ? "shape" : "junction", (int) o->isActive(),
            poly(o->polygon()).c_str(), pt(o->position()).c_str());
    if (junction)
    {
        printf("    junction pos=%s rec=%s fixed=%d\n",
                pt(junction->position()).c_str(),
                pt(junction->recommendedPosition()).c_str(),
                (int) junction->positionFixed());
    }
    VertInf *v = o->firstVert();
    do
    {
        printf("    ring %s\n", vert(v).c_str());
        v = v->shNext;
    }
    while (v != o->firstVert());
    printf("    lastVert %s\n", vert(o->lastVert()).c_str());
    for (ShapeConnectionPinSet::iterator it = o->m_connection_pins.begin();
            it != o->m_connection_pins.end(); ++it)
    {
        dumpPin(*it);
    }
    // m_following_conns is a std::set<ConnEnd *>: its order is malloc
    // address order, which the Rust port cannot reproduce (it uses creation
    // order).  Print it canonically sorted.
    std::set<std::pair<unsigned, unsigned> > following;
    for (std::set<ConnEnd *>::iterator it = o->m_following_conns.begin();
            it != o->m_following_conns.end(); ++it)
    {
        following.insert(std::make_pair((*it)->m_conn_ref->id(),
                (*it)->endpointType()));
    }
    printf("    following");
    for (std::set<std::pair<unsigned, unsigned> >::iterator it =
            following.begin(); it != following.end(); ++it)
    {
        printf(" %u/%u", it->first, it->second);
    }
    printf("\n");
    ConnRefList attached = o->attachedConnectors();
    std::multiset<unsigned> attachedIds;
    for (ConnRefList::iterator it = attached.begin(); it != attached.end();
            ++it)
    {
        attachedIds.insert((*it)->id());
    }
    printf("    attached");
    for (std::multiset<unsigned>::iterator it = attachedIds.begin();
            it != attachedIds.end(); ++it)
    {
        printf(" %u", *it);
    }
    printf("\n");
}

static void dumpConn(ConnRef *c)
{
    printf("  conn id=%u type=%d active=%d reroute=%d false=%d repaint=%d "
            "fixed=%d hate=%d flag=%d dist=%s\n", c->id(), (int) c->m_type,
            (int) c->m_active, (int) c->m_needs_reroute_flag,
            (int) c->m_false_path, (int) c->m_needs_repaint,
            (int) c->m_has_fixed_route, (int) c->m_hate_crossings,
            (int) *(c->m_reroute_flag_ptr), g17(c->m_route_dist).c_str());
    printf("    src %s\n", vert(c->m_src_vert).c_str());
    printf("    dst %s\n", vert(c->m_dst_vert).c_str());
    printf("    srcEnd %s\n", c->m_src_connend ?
            connEnd(*c->m_src_connend).c_str() : "-");
    printf("    dstEnd %s\n", c->m_dst_connend ?
            connEnd(*c->m_dst_connend).c_str() : "-");
    std::pair<ConnEnd, ConnEnd> ends = c->endpointConnEnds();
    printf("    endpoints %s | %s\n", connEnd(ends.first).c_str(),
            connEnd(ends.second).c_str());
    for (size_t i = 0; i < c->m_checkpoint_vertices.size(); ++i)
    {
        printf("    checkpoint %s\n", vert(c->m_checkpoint_vertices[i]).c_str());
    }
    printf("    route=%s display=%s\n", poly(c->m_route).c_str(),
            poly(c->m_display_route).c_str());
    std::vector<Point> pins = c->possibleDstPinPoints();
    printf("    dstPinPoints");
    for (size_t i = 0; i < pins.size(); ++i)
    {
        printf(" %s", pt(pins[i]).c_str());
    }
    printf("\n");
}

static void dumpActions(Router *r)
{
    printf("  actions %u\n", (unsigned) r->actionList.size());
    for (ActionInfoList::iterator it = r->actionList.begin();
            it != r->actionList.end(); ++it)
    {
        ActionInfo& a = *it;
        printf("    action type=%d", (int) a.type);
        if (a.type == ConnChange)
        {
            printf(" conn=%u", a.conn()->id());
        }
        else if (a.type == ConnectionPinChange)
        {
            printf(" pin=%s",
                    pinName((ShapeConnectionPin *) a.objPtr).c_str());
        }
        else
        {
            printf(" obstacle=%u", a.obstacle()->id());
        }
        if (a.type == ShapeMove)
        {
            printf(" first=%d poly=%s", (int) a.firstMove,
                    poly(a.newPoly).c_str());
        }
        if (a.type == JunctionMove)
        {
            printf(" newpos=%s", pt(a.newPosition).c_str());
        }
        printf("\n");
        for (ConnUpdateList::iterator c = a.conns.begin(); c != a.conns.end();
                ++c)
        {
            printf("      end %u %s\n", c->first, connEnd(c->second).c_str());
        }
    }
}

static void dumpContainsMap(const char *name, const ContainsMap& m)
{
    printf("  %s", name);
    for (ContainsMap::const_iterator it = m.begin(); it != m.end(); ++it)
    {
        printf(" [%u,%u]{", it->first.objID, (unsigned) it->first.vn);
        bool first = true;
        for (ShapeSet::const_iterator s = it->second.begin();
                s != it->second.end(); ++s)
        {
            printf(first ? "%u" : ",%u", *s);
            first = false;
        }
        printf("}");
    }
    printf("\n");
}

static void dump(Router *r, const char *label)
{
    printf("== %s\n", label);
    printf("  largest=%u next=%u settings=%d static=%d consolidate=%d\n",
            r->m_largest_assigned_id, r->newObjectId(),
            (int) r->m_settings_changes,
            (int) r->m_static_orthogonal_graph_invalidated,
            (int) r->m_consolidate_actions);
    for (ObstacleList::iterator it = r->m_obstacles.begin();
            it != r->m_obstacles.end(); ++it)
    {
        dumpObstacle(*it);
    }
    for (ConnRefList::iterator it = r->connRefs.begin();
            it != r->connRefs.end(); ++it)
    {
        dumpConn(*it);
    }
    for (ClusterRefList::iterator it = r->clusterRefs.begin();
            it != r->clusterRefs.end(); ++it)
    {
        ClusterRef *cl = *it;
        printf("  cluster id=%u active=%d rect=%s n=%u", cl->id(),
                (int) cl->m_active, poly(cl->rectangularPolygon()).c_str(),
                (unsigned) cl->polygon().size());
        for (size_t i = 0; i < cl->polygon().size(); ++i)
        {
            printf(" %s", pt(cl->polygon().at(i)).c_str());
        }
        printf("\n");
    }
    printf("  vertices conns=%u shapes=%u\n", r->vertices.connsSize(),
            r->vertices.shapesSize());
    for (VertInf *v = r->vertices.connsBegin(); v != nullptr; v = v->lstNext)
    {
        printf("    %s\n", vert(v).c_str());
    }
    dumpContainsMap("contains", r->contains);
    dumpContainsMap("clusters", r->enclosingClusters);
    dumpActions(r);
}

static Polygon rect(double x1, double y1, double x2, double y2)
{
    return Rectangle(Point(x1, y1), Point(x2, y2));
}

static void printIntList(const char *name, const IntList& l)
{
    printf("  %s", name);
    for (IntList::const_iterator it = l.begin(); it != l.end(); ++it)
    {
        printf(" %u", *it);
    }
    printf("\n");
}

static void scenarioDefaults(void)
{
    printf("#### defaults\n");
    for (unsigned flags = 1; flags <= 3; ++flags)
    {
        Router *r = new Router(flags);
        printf("flags=%u poly=%d orth=%d", flags,
                (int) r->m_allows_polyline_routing,
                (int) r->m_allows_orthogonal_routing);
        for (int t = 0; t <= 2; ++t)
        {
            printf(" valid(%d)=%d", t, (int) r->validConnType((ConnType) t));
        }
        printf(" transactions=%d\n", (int) r->transactionUse());
        printf(" params");
        for (int p = 0; p < lastRoutingParameterMarker; ++p)
        {
            printf(" %s", g17(r->routingParameter((RoutingParameter) p)).c_str());
        }
        printf("\n options");
        for (int o = 0; o < lastRoutingOptionMarker; ++o)
        {
            printf(" %d", (int) r->routingOption((RoutingOption) o));
        }
        printf("\n flagsRouter partial=%d simple=%d clustered=%d ignore=%d "
                "lees=%d invis=%d selective=%d feedback=%d rubber=%d\n",
                (int) r->PartialTime, (int) r->SimpleRouting,
                (int) r->ClusteredRouting, (int) r->IgnoreRegions,
                (int) r->UseLeesAlgorithm, (int) r->InvisibilityGrph,
                (int) r->SelectiveReroute, (int) r->PartialFeedback,
                (int) r->RubberBandRouting);
        printf(" emptyTransaction=%d\n", (int) r->processTransaction());
        delete r;
    }
    const double values[] = { -1, -0.5, 0, 7.5, 1e9 };
    for (int v = 0; v < 5; ++v)
    {
        Router *r = new Router(OrthogonalRouting);
        printf("set %s:", g17(values[v]).c_str());
        for (int p = 0; p < lastRoutingParameterMarker; ++p)
        {
            r->m_settings_changes = false;
            r->setRoutingParameter((RoutingParameter) p, values[v]);
            printf(" %s/%d", g17(r->routingParameter((RoutingParameter) p)).c_str(),
                    (int) r->m_settings_changes);
        }
        r->setRoutingPenalty(segmentPenalty);
        printf(" penaltyDefault=%s", g17(r->routingParameter(segmentPenalty)).c_str());
        r->m_settings_changes = false;
        r->setRoutingOption(nudgeSharedPathsWithCommonEndPoint, false);
        printf(" opt=%d/%d\n", (int) r->routingOption(
                    nudgeSharedPathsWithCommonEndPoint), (int) r->m_settings_changes);
        delete r;
    }
}

static void scenarioIds(void)
{
    printf("#### ids\n");
    Router *r = new Router(OrthogonalRouting);
    Polygon p = rect(0, 0, 10, 10);
    ShapeRef *s1 = new ShapeRef(r, p);
    ShapeRef *s2 = new ShapeRef(r, p, 10);
    ConnRef *c1 = new ConnRef(r);
    JunctionRef *j1 = new JunctionRef(r, Point(5, 5), 5);
    ConnRef *c2 = new ConnRef(r);
    Polygon cp = rect(-5, -5, 50, 50);
    ClusterRef *cl = new ClusterRef(r, cp);
    ShapeRef *s3 = new ShapeRef(r, p, 3);
    ConnRef *c3 = new ConnRef(r, 100);
    JunctionRef *j2 = new JunctionRef(r, Point(1, 1));
    printf("ids %u %u %u %u %u %u %u %u %u next=%u\n", s1->id(), s2->id(),
            c1->id(), j1->id(), c2->id(), cl->id(), s3->id(), c3->id(),
            j2->id(), r->newObjectId());
    printf("unused");
    const unsigned ids[] = { 1, 2, 3, 5, 10, 11, 12, 13, 100, 101, 102 };
    for (int i = 0; i < 11; ++i)
    {
        printf(" %u:%d", ids[i], (int) r->objectIdIsUnused(ids[i]));
    }
    printf("\n");
    dump(r, "ids-queued");
    r->processActions();
    printf("unused-after");
    for (int i = 0; i < 11; ++i)
    {
        printf(" %u:%d", ids[i], (int) r->objectIdIsUnused(ids[i]));
    }
    printf("\n");
    dump(r, "ids-processed");
    delete r;
}

static void scenarioActions(void)
{
    printf("#### actions\n");
    Router *r = new Router(OrthogonalRouting);
    Polygon p1 = rect(0, 0, 10, 10);
    ShapeRef *s1 = new ShapeRef(r, p1, 1);
    Polygon p2 = rect(20, 0, 30, 10);
    ShapeRef *s2 = new ShapeRef(r, p2, 2);
    dump(r, "two-adds");
    // Moving a shape whose add is queued just changes its polygon.
    r->moveShape(s1, 1, 2);
    Polygon p1b = rect(100, 100, 120, 130);
    r->moveShape(s2, p1b, true);
    dump(r, "moves-while-add-queued");
    r->processActions();
    dump(r, "processed-adds");

    r->moveShape(s1, 5, 5);
    r->moveShape(s1, 1, 1);
    Polygon p3 = rect(40, 40, 50, 60);
    r->moveShape(s2, p3, true);
    Polygon p4 = rect(41, 41, 51, 61);
    r->moveShape(s2, p4, false);
    dump(r, "queued-moves");

    JunctionRef *j = new JunctionRef(r, Point(70, 70), 7);
    r->moveJunction(j, 1, 1);
    r->moveJunction(j, Point(80, 80));
    dump(r, "junction-add-moved");
    r->processActions();
    dump(r, "processed-moves");

    r->moveJunction(j, 2, 3);
    r->moveJunction(j, 1, 1);
    r->deleteShape(s1);
    r->moveShape(s2, 3, 3);
    dump(r, "move-then-delete");
    r->processActions();
    dump(r, "processed-delete");

    r->markAllObstaclesAsMoved();
    dump(r, "all-moved");
    r->processActions();
    dump(r, "processed-all-moved");

    r->deleteJunction(j);
    r->deleteShape(s2);
    dump(r, "delete-all");
    r->processActions();
    dump(r, "processed-delete-all");
    delete r;
}

static void scenarioPins(void)
{
    printf("#### pins\n");
    Router *r = new Router(OrthogonalRouting);
    Polygon p = rect(10, 20, 50, 80);
    ShapeRef *s = new ShapeRef(r, p, 1);
    ShapeConnectionPin *pins[12];
    pins[0] = new ShapeConnectionPin(s, 1, ATTACH_POS_LEFT, ATTACH_POS_TOP, true, 0.0, ConnDirNone);
    pins[1] = new ShapeConnectionPin(s, 1, ATTACH_POS_RIGHT, ATTACH_POS_BOTTOM, true, 2.0, ConnDirNone);
    pins[2] = new ShapeConnectionPin(s, 1, ATTACH_POS_CENTRE, ATTACH_POS_CENTRE, true, 0.0, ConnDirNone);
    pins[3] = new ShapeConnectionPin(s, 2, 0.25, 0.75, true, 1.0, ConnDirUp | ConnDirLeft);
    pins[4] = new ShapeConnectionPin(s, 2, 5.0, 10.0, false, 0.0, ConnDirDown);
    pins[5] = new ShapeConnectionPin(s, 2, ATTACH_POS_MIN_OFFSET, ATTACH_POS_MAX_OFFSET, false, 1.5, ConnDirNone);
    pins[6] = new ShapeConnectionPin(s, 3, 40.0, 60.0, false, 0.0, ConnDirRight);
    pins[7] = new ShapeConnectionPin(s, 3, 0.5, 0.0, 0.0, ConnDirAll);
    pins[8] = new ShapeConnectionPin(s, 1, ATTACH_POS_LEFT, ATTACH_POS_TOP, true, 0.0, ConnDirNone);
    pins[9] = new ShapeConnectionPin(s, 4, 1.5, -0.5, true, 0.0, ConnDirNone);
    pins[10] = new ShapeConnectionPin(s, 4, 100.0, 100.0, false, 0.0, ConnDirNone);
    pins[11] = new ShapeConnectionPin(s, 2, 0.25, 0.75, true, 1.0, ConnDirUp);
    pins[2]->setConnectionCost(3.5);
    pins[3]->setExclusive(false);
    pins[7]->setExclusive(true);
    printf("set size=%u\n", (unsigned) s->m_connection_pins.size());
    for (int i = 0; i < 12; ++i)
    {
        printf("pin%d excl=%d ids=%u,%u dirs=%u pos=%s\n", i,
                (int) pins[i]->isExclusive(), pins[i]->ids().first,
                pins[i]->ids().second, pins[i]->directions(),
                ptFull(pins[i]->position()).c_str());
    }
    printf("lt");
    for (int i = 0; i < 12; ++i)
    {
        for (int k = 0; k < 12; ++k)
        {
            printf("%d", (int) (*pins[i] < *pins[k]));
        }
        printf(" ");
    }
    printf("\neq");
    for (int i = 0; i < 12; ++i)
    {
        for (int k = 0; k < 12; ++k)
        {
            printf("%d", (int) (*pins[i] == *pins[k]));
        }
        printf(" ");
    }
    printf("\n");
    dump(r, "pins-created");
    r->processActions();
    dump(r, "pins-processed");

    // Pin positions after the shape moves.
    r->moveShape(s, 5, -5);
    r->processActions();
    dump(r, "pins-after-move");

    const ShapeTransformationType transforms[] = { TransformationType_CW90,
        TransformationType_CW180, TransformationType_CW270,
        TransformationType_FlipX, TransformationType_FlipY };
    for (int t = 0; t < 5; ++t)
    {
        Polygon tp = rect(0, 0, 40 + t, 20 + 2 * t);
        ShapeRef *ts = new ShapeRef(r, tp, 50 + t);
        // (One pin per class id: transforms mutate the set's keys in place,
        // and distinct class ids keep the set ordering valid.)
        new ShapeConnectionPin(ts, 1, ATTACH_POS_LEFT, ATTACH_POS_TOP, true, 0.0, ConnDirNone);
        new ShapeConnectionPin(ts, 2, 0.25, 0.6, true, 1.0, ConnDirUp | ConnDirRight);
        new ShapeConnectionPin(ts, 3, 10.0, 5.0, false, 0.0, ConnDirLeft);
        new ShapeConnectionPin(ts, 4, ATTACH_POS_MIN_OFFSET, ATTACH_POS_MAX_OFFSET, false, 0.0, ConnDirDown | ConnDirLeft);
        new ShapeConnectionPin(ts, 5, 0.0, 7.0, false, 0.0, ConnDirAll);
        new ShapeConnectionPin(ts, 6, 0.5, 1.0, true, 0.0, ConnDirNone);
        r->processActions();
        ts->transformConnectionPinPositions(transforms[t]);
        printf("transform %d\n", (int) transforms[t]);
        dumpObstacle(ts);
        dumpActions(r);
        ts->transformConnectionPinPositions(transforms[t]);
        printf("transform-again %d\n", (int) transforms[t]);
        dumpObstacle(ts);
        r->processActions();
    }
    delete r;
}

static void scenarioJunctions(void)
{
    printf("#### junctions\n");
    Router *r = new Router(OrthogonalRouting);
    r->setRoutingParameter(idealNudgingDistance, 0.25);
    JunctionRef *j1 = new JunctionRef(r, Point(10, 10), 1);
    r->setRoutingParameter(idealNudgingDistance, 4.0);
    JunctionRef *j2 = new JunctionRef(r, Point(20, 30), 2);
    new ShapeConnectionPin(j2, 5, ConnDirLeft);
    new ShapeConnectionPin(j2, 6, ConnDirUp | ConnDirDown);
    new ShapeConnectionPin(j2, 7);
    JunctionRef *j3 = new JunctionRef(r, Point(-5, 0), 3);
    new ShapeConnectionPin(j3, 5, ConnDirRight);
    new ShapeConnectionPin(j3, 6, ConnDirUp);
    j1->setPositionFixed(true);
    dump(r, "junctions-created");
    j2->preferOrthogonalDimension(XDIM);
    j3->preferOrthogonalDimension(YDIM);
    r->processActions();
    dump(r, "junctions-preferred");
    j1->setRecommendedPosition(Point(11, 12));
    r->moveJunction(j2, Point(25, 35));
    r->processActions();
    dump(r, "junctions-moved");
    delete r;
}

static void scenarioConnectors(void)
{
    printf("#### connectors\n");
    Router *r = new Router(OrthogonalRouting);
    Polygon pa = rect(0, 0, 20, 20);
    ShapeRef *a = new ShapeRef(r, pa, 1);
    Polygon pb = rect(100, 0, 120, 20);
    ShapeRef *b = new ShapeRef(r, pb, 2);
    new ShapeConnectionPin(a, 1, ATTACH_POS_RIGHT, ATTACH_POS_CENTRE, true, 0.0, ConnDirRight);
    new ShapeConnectionPin(a, 1, ATTACH_POS_CENTRE, ATTACH_POS_BOTTOM, true, 0.0, ConnDirDown);
    new ShapeConnectionPin(b, 1, ATTACH_POS_LEFT, ATTACH_POS_CENTRE, true, 0.0, ConnDirLeft);
    new ShapeConnectionPin(b, 2, ATTACH_POS_CENTRE, ATTACH_POS_CENTRE, true, 0.0, ConnDirAll);
    JunctionRef *j = new JunctionRef(r, Point(60, 50), 3);

    ConnRef *c1 = new ConnRef(r, 10);
    c1->setSourceEndpoint(ConnEnd(Point(-10, -10)));
    c1->setSourceEndpoint(ConnEnd(a, 1));
    c1->setDestEndpoint(ConnEnd(b, 1));
    ConnRef *c2 = new ConnRef(r, ConnEnd(Point(50, 100), ConnDirUp), ConnEnd(j), 11);
    ConnRef *c3 = new ConnRef(r, ConnEnd(j), ConnEnd(b, 2), 12);
    ConnRef *c4 = new ConnRef(r, 13);
    c4->setEndpoints(ConnEnd(Point(1, 2)), ConnEnd(Point(3, 4), ConnDirLeft | ConnDirDown));
    c4->setRoutingType(ConnType_PolyLine);
    c4->setRoutingType(ConnType_Orthogonal);
    c4->setHateCrossings(true);
    std::vector<Checkpoint> cps;
    cps.push_back(Checkpoint(Point(5, 5)));
    cps.push_back(Checkpoint(Point(6, 7), ConnDirUp, ConnDirDown));
    c4->setRoutingCheckpoints(cps);
    dump(r, "conns-queued");
    // Pin-move updates must not override user updates.
    r->modifyConnector(c1, VertID::src, ConnEnd(Point(7, 7)), true);
    r->modifyConnector(c1, VertID::tar, ConnEnd(Point(8, 8)), false);
    r->modifyConnector(c1, VertID::tar, ConnEnd(b, 1), false);
    r->modifyConnector(c3);
    dumpActions(r);
    r->processActions();
    dump(r, "conns-processed");

    IntList l;
    for (unsigned id = 1; id <= 3; ++id)
    {
        for (unsigned type = 1; type <= 3; ++type)
        {
            l.clear();
            r->attachedConns(l, id, type);
            printf("attachedConns(%u,%u)", id, type);
            printIntList("", l);
            l.clear();
            r->attachedShapes(l, id, type);
            printf("attachedShapes(%u,%u)", id, type);
            printIntList("", l);
        }
    }
    printf("possible a/1:");
    std::vector<Point> pp = a->possiblePinPoints(1);
    for (size_t i = 0; i < pp.size(); ++i) printf(" %s", pt(pp[i]).c_str());
    printf("\n");

    // Shape move propagates to attached connector ends.
    r->moveShape(a, 0, 5);
    r->moveJunction(j, 5, 0);
    dumpActions(r);
    r->processActions();
    dump(r, "after-attached-move");

    // Deleting a shape turns attached ends into free points.
    r->deleteShape(b);
    r->processActions();
    dump(r, "after-shape-delete");

    // Fixed routes and display routes.
    Polygon route;
    route.ps.push_back(Point(0, 50));
    route.ps.push_back(Point(30, 50));
    route.ps.push_back(Point(60, 50));
    route.ps.push_back(Point(60, 80));
    c4->setFixedRoute(route);
    c4->calcRouteDist();
    dump(r, "fixed-route-queued");
    r->processActions();
    c4->calcRouteDist();
    dump(r, "fixed-route-processed");
    c4->clearFixedRoute();

    // Split a connector at a display-route segment.
    Polygon r2;
    r2.ps.push_back(Point(20, 10));
    r2.ps.push_back(Point(40, 10));
    r2.ps.push_back(Point(40, 30));
    r2.ps.push_back(Point(80, 30));
    c3->set_route(r2);
    std::pair<JunctionRef *, ConnRef *> split = c3->splitAtSegment(2);
    printf("split junction=%u conn=%u\n", split.first->id(), split.second->id());
    dump(r, "split-queued");
    r->processActions();
    dump(r, "split-processed");

    r->deleteConnector(c2);
    dump(r, "deleted-c2");
    ConnRef *merged = j->removeJunctionAndMergeConnectors();
    printf("merged=%d\n", merged ? (int) merged->id() : -1);
    dump(r, "merge-queued");
    r->processActions();
    dump(r, "merge-processed");
    delete r;
}

static void scenarioMerge(void)
{
    printf("#### merge\n");
    Router *r = new Router(OrthogonalRouting);
    JunctionRef *j = new JunctionRef(r, Point(50, 50), 1);
    Polygon ps = rect(90, 40, 110, 60);
    ShapeRef *s = new ShapeRef(r, ps, 4);
    new ShapeConnectionPin(s, 1, ATTACH_POS_LEFT, ATTACH_POS_CENTRE, true, 0.0, ConnDirLeft);
    Polygon pt6 = rect(-10, 40, 10, 60);
    ShapeRef *t = new ShapeRef(r, pt6, 6);
    new ShapeConnectionPin(t, 1, ATTACH_POS_RIGHT, ATTACH_POS_CENTRE, true, 0.0, ConnDirRight);
    ConnRef *c1 = new ConnRef(r, ConnEnd(t, 1), ConnEnd(j), 2);
    ConnRef *c2 = new ConnRef(r, ConnEnd(j), ConnEnd(s, 1), 3);
    ConnRef *c3 = new ConnRef(r, ConnEnd(j), ConnEnd(Point(50, 0)), 5);
    r->processActions();
    dump(r, "merge-setup");
    // Three attached connectors: nothing to merge.
    printf("merged3=%d\n", j->removeJunctionAndMergeConnectors() ? 1 : 0);
    r->deleteConnector(c3);
    // Which of c1/c2 survives depends on the (pointer) order of the
    // junction's m_following_conns, so only order-independent facts are
    // printed from here on.
    ConnRef *merged = j->removeJunctionAndMergeConnectors();
    printf("merged=%d\n", merged ? 1 : 0);
    r->processActions();
    printf("after merge: conns=%u obstacles=%u verts=%u+%u\n",
            (unsigned) r->connRefs.size(), (unsigned) r->m_obstacles.size(),
            r->vertices.connsSize(), r->vertices.shapesSize());
    std::pair<ConnEnd, ConnEnd> ends = merged->endpointConnEnds();
    printf("  merged ends %u:%u -> %u:%u\n", ends.first.m_anchor_obj->id(),
            ends.first.m_connection_pin_class_id,
            ends.second.m_anchor_obj->id(),
            ends.second.m_connection_pin_class_id);
    (void) c1;
    (void) c2;
    delete r;
}

static void scenarioClusters(void)
{
    printf("#### clusters\n");
    Router *r = new Router(OrthogonalRouting);
    Polygon pa = rect(0, 0, 20, 20);
    ShapeRef *a = new ShapeRef(r, pa, 1);
    ConnRef *c1 = new ConnRef(r, ConnEnd(Point(5, 5)), ConnEnd(Point(50, 50)), 2);
    ConnRef *c2 = new ConnRef(r, ConnEnd(Point(30, 30)), ConnEnd(Point(-50, 5)), 3);
    r->processActions();
    Polygon cp;
    Point q1 = a->polygon().ps[0];
    q1.id = 1;
    q1.vn = 0;
    cp.ps.push_back(q1);
    cp.ps.push_back(Point(60, 0));
    cp.ps.push_back(Point(60, 60));
    cp.ps.push_back(Point(0, 60));
    ClusterRef *cl1 = new ClusterRef(r, cp, 10);
    Polygon cp2 = rect(-60, -10, 10, 10);
    ClusterRef *cl2 = new ClusterRef(r, cp2);
    dump(r, "clusters-added");
    r->generateContains(c1->src());
    r->generateContains(c2->dst());
    dump(r, "contains-generated");
    Polygon cp3 = rect(-100, -100, 100, 100);
    cl1->setNewPoly(cp3);
    printf("setNewPoly rect=%s\n", poly(cl1->rectangularPolygon()).c_str());
    r->deleteCluster(cl2);
    dump(r, "cluster-deleted");
    ShapeRef *hit = r->shapeContainingPoint(Point(10, 10));
    ShapeRef *miss = r->shapeContainingPoint(Point(25, 10));
    ShapeRef *border = r->shapeContainingPoint(Point(20, 10));
    printf("containing %d %d %d\n", hit ? (int) hit->id() : -1,
            miss ? (int) miss->id() : -1, border ? (int) border->id() : -1);
    r->deleteCluster(cl1);
    delete r;
}

static void scenarioPolyline(void)
{
    printf("#### polyline-queue\n");
    Router *r = new Router(PolyLineRouting);
    Polygon pa = rect(0, 0, 20, 20);
    ShapeRef *a = new ShapeRef(r, pa, 4);
    ShapeRef *b = new ShapeRef(r, pa);
    r->moveShape(a, 1, 1);
    Polygon pb = rect(5, 5, 6, 7);
    r->moveShape(b, pb);
    ConnRef *c = new ConnRef(r);
    c->setRoutingType(ConnType_Orthogonal);
    printf("type=%d\n", (int) c->routingType());
    dump(r, "polyline-queued");
    // (not processed: polyline visibility is not yet ported in Rust)
    r->actionList.clear();
    r->deleteConnector(c);
    dump(r, "polyline-cleared");
    r->m_obstacles.clear();
    // Leak the router: its shapes were never added to the router lists.
}

static void scenarioDiagram(void)
{
    printf("#### diagram\n");
    Router *r = new Router(OrthogonalRouting);
    Polygon pa = rect(0.5, 0, 20.25, 20);
    new ShapeRef(r, pa, 1);
    Polygon pb = rect(100, 1e-7, 123456789, 20);
    new ShapeRef(r, pb, 2);
    new JunctionRef(r, Point(3, 4), 3);
    ConnRef *c = new ConnRef(r, ConnEnd(Point(1, 1)), ConnEnd(Point(2, 2)), 4);
    new ConnRef(r, ConnEnd(Point(1, 1)), ConnEnd(Point(2, 2)), 5);
    r->processActions();
    Polygon route;
    route.ps.push_back(Point(1, 1));
    route.ps.push_back(Point(1.0 / 3.0, 1));
    route.ps.push_back(Point(1.0 / 3.0, 2));
    route.ps.push_back(Point(2, 2));
    c->setFixedRoute(route);
    r->processActions();
    r->outputDiagramText("objects_oracle_diagram");
    FILE *fp = fopen("objects_oracle_diagram.txt", "r");
    int ch;
    while ((ch = fgetc(fp)) != EOF)
    {
        putchar(ch);
    }
    fclose(fp);
    remove("objects_oracle_diagram.txt");
    printf("#### end diagram\n");
    delete r;
}

int main(void)
{
    scenarioDefaults();
    scenarioIds();
    scenarioActions();
    scenarioPins();
    scenarioJunctions();
    scenarioConnectors();
    scenarioMerge();
    scenarioClusters();
    scenarioPolyline();
    scenarioDiagram();
    return 0;
}
