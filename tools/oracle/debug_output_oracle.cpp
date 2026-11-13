// Reference output for tests/debug_output_files.rs: the parts of the debug
// output that no fixture reaches -- HyperedgeRerouter::outputInstanceToSVG()
// with pending registrations (upstream never calls it itself, and
// processTransaction() clears the registrations), and the reproduction code
// of a scene with clusters, pins (shape and junction), a fixed route,
// checkpoints and a polyline connector.
//
//   ORACLE_LIB_ONLY=1 tools/oracle/build.sh
//   g++ -O2 -std=c++11 -w -fno-access-control -I target/oracle \
//       tools/oracle/debug_output_oracle.cpp target/oracle/libavoid.a \
//       -o target/debug_output_oracle
//   (cd target && ./debug_output_oracle)    # prints the rerouter output and
//                                            # writes debug-output-oracle.svg
//
// The scene must stay in lock-step with tests/debug_output_files.rs.
#include <cstdio>
#include "libavoid/libavoid.h"

using namespace Avoid;

int main(void)
{
    Router *router = new Router(PolyLineRouting | OrthogonalRouting);
    router->setRoutingParameter(segmentPenalty, 50);
    router->setRoutingOption(nudgeOrthogonalSegmentsConnectedToShapes, true);

    Polygon rect = Rectangle(Point(0, 0), Point(30, 20));
    ShapeRef *shape = new ShapeRef(router, rect, 7);
    new ShapeConnectionPin(shape, 1, ATTACH_POS_LEFT, ATTACH_POS_CENTRE, true, 0.0, ConnDirLeft);
    ShapeConnectionPin *pin = new ShapeConnectionPin(shape, 2, 0.25, 1.0 / 3.0, true, 2.5, ConnDirNone);
    pin->setExclusive(false);
    Polygon rect2 = Rectangle(Point(100, 50), Point(140, 80));
    ShapeRef *shape2 = new ShapeRef(router, rect2, 8);

    JunctionRef *junction = new JunctionRef(router, Point(60.5, 40.25), 9);
    junction->setPositionFixed(true);

    Polygon clusterPoly = Rectangle(Point(-10, -10), Point(150, 90));
    new ClusterRef(router, clusterPoly, 11);

    ConnRef *c1 = new ConnRef(router, ConnEnd(shape, 1), ConnEnd(junction), 20);
    c1->setRoutingType(ConnType_Orthogonal);
    ConnRef *c2 = new ConnRef(router, ConnEnd(junction), ConnEnd(shape2, CONNECTIONPIN_CENTRE), 21);
    c2->setRoutingType(ConnType_Orthogonal);
    std::vector<Checkpoint> checkpoints;
    checkpoints.push_back(Checkpoint(Point(80, 45), ConnDirUp, ConnDirDown));
    checkpoints.push_back(Checkpoint(Point(90.125, 47)));
    c2->setRoutingCheckpoints(checkpoints);
    ConnRef *c3 = new ConnRef(router, ConnEnd(Point(-20, 5), ConnDirLeft),
            ConnEnd(Point(200, 1.0 / 7.0)), 22);
    c3->setRoutingType(ConnType_PolyLine);
    ConnRef *c4 = new ConnRef(router, ConnEnd(Point(10, 100)), ConnEnd(Point(50, 120)), 23);
    PolyLine fixed;
    fixed.ps.push_back(Point(10, 100));
    fixed.ps.push_back(Point(10, 110));
    fixed.ps.push_back(Point(50, 110));
    fixed.ps.push_back(Point(50, 120));
    c4->setFixedRoute(fixed);
    router->processTransaction();
    router->outputInstanceToSVG("debug-output-oracle");

    HyperedgeRerouter *rerouter = router->hyperedgeRerouter();
    rerouter->registerHyperedgeForRerouting(junction);
    ConnEndList ends;
    ends.push_back(ConnEnd(Point(1.5, 2), ConnDirUp));
    ends.push_back(ConnEnd(shape, 2));
    ends.push_back(ConnEnd(junction));
    rerouter->registerHyperedgeForRerouting(ends);
    rerouter->outputInstanceToSVG(stdout);
    return 0;
}
