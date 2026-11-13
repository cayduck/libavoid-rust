// Type-level smoke test for the CommonJS entry (index.d.ts).
// Run with `npm run test:types`; it only has to compile.
import {
  AvoidRouter,
  AvoidInternalError,
  AttachPos,
  ConnDir,
  RouterFlag,
  RoutingOption,
  RoutingParameter,
  curvedPolyline,
  reset,
} from "../../index";

const router = AvoidRouter.withFlags(RouterFlag.OrthogonalRouting);
router.setRoutingParameter(RoutingParameter.shapeBufferDistance, 10);
router.setRoutingOption(RoutingOption.nudgeOrthogonalSegmentsConnectedToShapes, true);
const added: boolean = router.addShape("a", 0, 0, 10, 10);
router.addPolygonShape("b", [{ x: 20, y: 0 }, { x: 30, y: 0 }, { x: 25, y: 10 }]);
router.addShapeConnectionPinWithOptions("a", "out", {
  classId: 1,
  xOffset: AttachPos.Right,
  yOffset: AttachPos.Centre,
  proportional: true,
  visibilityDirections: ConnDir.Right,
});
router.addConnector("c", 0, 0, 50, 50);
router.setTransactionTimeLimit(100);
const processed: boolean = router.processTransaction();
const route = router.displayConnectorRoute("c");
if (route) {
  const first: { x: number; y: number } | undefined = route[0];
  void first;
  void curvedPolyline(route, 5);
}
const updated: string[] = router.lastTransactionUpdatedConnectorIds();
try {
  router.processTransaction();
} catch (e) {
  if (e instanceof AvoidInternalError) {
    const kind: string = e.kind;
    void kind;
    reset();
  }
}
void added;
void processed;
void updated;
