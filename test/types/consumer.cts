// Type-level smoke test for the CommonJS entry (index.d.ts).
// Run with `npm run test:types`; it only has to compile.
import * as Avoid from "../../index";
import { Router, ShapeRef, ConnRef, ConnEnd, Point, Rectangle, AvoidInternalError, reset } from "../../index";

class ProgressRouter extends Router {
  shouldContinueTransactionWithProgress(elapsedTime: number, phaseNumber: number, totalPhases: number, proportion: number): boolean {
    return elapsedTime < 1000 || phaseNumber === totalPhases || proportion < 0.5;
  }
}

const router: Router = new ProgressRouter(Avoid.OrthogonalRouting | Avoid.PolyLineRouting);
router.setRoutingParameter(Avoid.shapeBufferDistance, 10);
router.setRoutingParameter(Avoid.RoutingParameter.idealNudgingDistance);
router.setRoutingPenalty(Avoid.segmentPenalty);
router.setRoutingOption(Avoid.nudgeOrthogonalSegmentsConnectedToShapes, true);
router.RubberBandRouting = false;
const flag: boolean = router.UseLeesAlgorithm;

const shape = new ShapeRef(router, new Rectangle(new Point(0, 0), new Point(10, 10)));
const other = new ShapeRef(router, new Rectangle(new Point(50, 50), 20, 10), 77);
const poly = new Avoid.Polygon(3);
poly.ps[0] = new Point(20, 0);
poly.ps[1] = new Point(30, 0);
poly.ps[2] = new Point(25, 10);
const tri = new ShapeRef(router, poly);
const pin = new Avoid.ShapeConnectionPin(shape, 1, Avoid.ATTACH_POS_RIGHT, Avoid.ATTACH_POS_CENTRE, true, 0, Avoid.ConnDirRight);
pin.setExclusive(false);
const ids: [number, number] = pin.ids();
const junction = new Avoid.JunctionRef(router, new Point(100, 100));
new Avoid.ShapeConnectionPin(junction, 2);

const conn = new ConnRef(router, new ConnEnd(shape, 1), new ConnEnd(new Point(50, 50), Avoid.ConnDirAll));
const conn2 = new ConnRef(router);
conn2.setEndpoints(new ConnEnd(junction), new ConnEnd(other, Avoid.CONNECTIONPIN_CENTRE));
conn.setCallback((c: ConnRef) => {
  const route: Avoid.PolyLine = c.displayRoute();
  for (let i = 0; i < route.size(); i++) void route.at(i).x;
}, conn);
conn.setRoutingCheckpoints([new Avoid.Checkpoint(new Point(30, 30)), new Avoid.Checkpoint(new Point(40, 30), Avoid.ConnDirAll, Avoid.ConnDirLeft)]);
const processed: boolean = router.processTransaction();
const [src, dst]: [ConnEnd, ConnEnd] = conn.endpointConnEnds();
const s: ShapeRef | null = src.shape();
void dst.position().x;
const attached: ConnRef[] = shape.attachedConnectors();
const routed: boolean = conn.hasValidRoute() && !conn.hasCrossingObstacles();
router.setRouteInsideParentShapes(true);
const inside: boolean = router.routeInsideParentShapes();
router.setRerouteAfterEdits(true);
const rerouting: boolean = router.rerouteAfterEdits();
const shapes: number[] = [];
router.attachedShapes(shapes, shape.id(), Avoid.runningToAndFrom);
const [j, c2]: [Avoid.JunctionRef | null, ConnRef | null] = conn.splitAtSegment(1);
router.moveShape(shape, 5, 0);
router.moveShape(tri, new Rectangle(new Point(20, 0), new Point(30, 10)), true);
router.moveJunction(junction, new Point(90, 90));
router.moveJunction(junction, 1, 1);
const lists: Avoid.HyperedgeNewAndDeletedObjectLists = router.newAndDeletedObjectListsFromHyperedgeImprovement();
const index: number = router.hyperedgeRerouter().registerHyperedgeForRerouting(junction);
router.hyperedgeRerouter().registerHyperedgeForRerouting([new ConnEnd(shape, 1), new ConnEnd(other, 1)]);
const cluster = new Avoid.ClusterRef(router, new Rectangle(new Point(-10, -10), new Point(40, 40)));
const refPoly: Avoid.ReferencingPolygon = cluster.polygon();
const box: Avoid.Box = shape.routingBox();
const text: string = router.outputDiagramText();
const obstacles: Avoid.Obstacle[] = router.m_obstacles;
const seen: [boolean] = [false];
const hit: boolean = Avoid.segmentShapeIntersect(new Point(0, 0), new Point(1, 1), new Point(0, 1), new Point(1, 0), seen);
const x: [number] = [0];
const y: [number] = [0];
const kind: number = Avoid.segmentIntersectPoint(new Point(0, 0), new Point(1, 1), new Point(0, 1), new Point(1, 0), x, y);
const curved: Avoid.Polygon = poly.curvedPolyline(2);
const err: AvoidInternalError | null = Avoid.internalError();
router.deleteConnector(conn2);
router.deleteShape(other);
router.deleteCluster(cluster);
pin.delete();
router.delete();
reset();
void [flag, ids, processed, s, attached, j, c2, lists, index, refPoly, box, text, obstacles, hit, kind, curved, err];
