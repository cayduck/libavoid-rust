const {
  AttachPos,
  AvoidRouter,
  ConnDir,
  ConnEndType,
  ConnectionPin,
  ConnectorRelation,
  ConnType,
  RouterFlag,
  RoutingOption,
  RoutingParameter,
  ShapeTransformation,
  WasmAvoidRouter,
} = require("..");

if (
  ConnEndType.Point !== 0 ||
  ConnEndType.ShapePin !== 1 ||
  ConnEndType.Junction !== 2 ||
  ConnEndType.Empty !== 3
) {
  throw new Error(`ConnEndType constants did not match upstream values: ${JSON.stringify(ConnEndType)}`);
}

const router = new AvoidRouter(10);
router.setRoutingParameter(RoutingParameter.shapeBufferDistance, 10);
router.addShape("box", 40, 40, 120, 120);
router.addConnector("edge", 0, 80, 200, 80);
router.setConnectorRoutingType("edge", ConnType.Orthogonal);
if (router.connectorRoutingType("edge") !== ConnType.Orthogonal) {
  throw new Error("connector routing type was not preserved");
}
if (
  !router.setConnectorHateCrossings("edge", true) ||
  !router.connectorDoesHateCrossings("edge") ||
  !router.setConnectorHateCrossings("edge", false) ||
  router.connectorDoesHateCrossings("edge") ||
  router.setConnectorHateCrossings("missing", true) ||
  router.connectorDoesHateCrossings("missing")
) {
  throw new Error("connector hate-crossings flag was not preserved");
}

const route = router.routeConnector("edge");

if (route.length <= 2) {
  throw new Error(`expected obstacle-avoiding route, got ${JSON.stringify(route)}`);
}

for (const point of route) {
  if (typeof point.x !== "number" || typeof point.y !== "number") {
    throw new Error(`route point is not numeric: ${JSON.stringify(point)}`);
  }
}

console.log(JSON.stringify(route));

const clusterRouter = new AvoidRouter(0);
if (!clusterRouter.addClusterRectangle("cluster", 70, 70, 30, 30)) {
  throw new Error("rectangular cluster was not created");
}
if (JSON.stringify(clusterRouter.clusterPolygon("cluster")) !== JSON.stringify([
  { x: 70, y: 30 },
  { x: 70, y: 70 },
  { x: 30, y: 70 },
  { x: 30, y: 30 },
])) {
  throw new Error(`unexpected rectangular cluster polygon: ${JSON.stringify(clusterRouter.clusterPolygon("cluster"))}`);
}
if (JSON.stringify(clusterRouter.clusterRectangularPolygon("cluster")) !== JSON.stringify([
  { x: 70, y: 30 },
  { x: 70, y: 70 },
  { x: 30, y: 70 },
  { x: 30, y: 30 },
])) {
  throw new Error(`unexpected rectangular cluster boundary: ${JSON.stringify(clusterRouter.clusterRectangularPolygon("cluster"))}`);
}
if (clusterRouter.clusterRectangularPolygon("missing") !== null) {
  throw new Error("missing cluster rectangular boundary did not return null");
}
clusterRouter.addConnector("outside", 0, 50, 100, 50);
const clusterRoute = clusterRouter.routeConnector("outside");
if (clusterRoute.length <= 2) {
  throw new Error(`outside-to-outside route entered cluster boundary: ${JSON.stringify(clusterRoute)}`);
}
if (!clusterRouter.setClusterRectangle("cluster", 60, 60, 40, 40)) {
  throw new Error("rectangular cluster update was rejected");
}
if (JSON.stringify(clusterRouter.clusterBoundingBox("cluster")) !== JSON.stringify({ left: 30, top: 30, right: 70, bottom: 70 })) {
  throw new Error(`rectangular cluster update was not deferred: ${JSON.stringify(clusterRouter.clusterBoundingBox("cluster"))}`);
}
if (clusterRouter.connectorNeedsRepaint("outside")) {
  throw new Error("queued cluster polygon update repainted connector before transaction processing");
}
clusterRouter.processTransaction();
if (JSON.stringify(clusterRouter.clusterBoundingBox("cluster")) !== JSON.stringify({ left: 40, top: 40, right: 60, bottom: 60 })) {
  throw new Error(`unexpected rectangular cluster update: ${JSON.stringify(clusterRouter.clusterBoundingBox("cluster"))}`);
}
if (!clusterRouter.connectorNeedsRepaint("outside")) {
  throw new Error("cluster polygon update did not invalidate attached route after transaction processing");
}
const updatedClusterRoute = clusterRouter.displayConnectorRoute("outside");
if (updatedClusterRoute.length <= 2) {
  throw new Error(`updated cluster route did not avoid new boundary: ${JSON.stringify(updatedClusterRoute)}`);
}
if (!clusterRouter.removeCluster("cluster")) {
  throw new Error("cluster removal was rejected");
}
if (clusterRouter.objectIdIsUnused("cluster")) {
  throw new Error("cluster removal was applied before transaction processing");
}
clusterRouter.processTransaction();
const unclusteredRoute = clusterRouter.routeConnector("outside");
if (unclusteredRoute.length !== 2) {
  throw new Error(`route did not simplify after cluster removal: ${JSON.stringify(unclusteredRoute)}`);
}
const penaltyClusterRouter = new AvoidRouter(0);
penaltyClusterRouter.addClusterRectangle("cluster", 40, 40, 80, 80);
penaltyClusterRouter.addConnector("edge", 0, 60, 120, 60);
const penaltyAvoidingRoute = penaltyClusterRouter.routeConnector("edge");
if (penaltyAvoidingRoute.length <= 2) {
  throw new Error(`positive cluster crossing penalty did not avoid boundary: ${JSON.stringify(penaltyAvoidingRoute)}`);
}
penaltyClusterRouter.setRoutingParameter(RoutingParameter.clusterCrossingPenalty, 0);
const penaltyCrossingRoute = penaltyClusterRouter.routeConnector("edge");
if (JSON.stringify(penaltyCrossingRoute) !== JSON.stringify([{ x: 0, y: 60 }, { x: 120, y: 60 }])) {
  throw new Error(`zero cluster crossing penalty did not allow direct crossing: ${JSON.stringify(penaltyCrossingRoute)}`);
}
const concaveClusterRouter = new AvoidRouter(0);
concaveClusterRouter.addCluster("concave", [
  [0, 0],
  [100, 0],
  [100, 30],
  [30, 30],
  [30, 70],
  [100, 70],
  [100, 100],
  [0, 100],
]);
if (JSON.stringify(concaveClusterRouter.clusterRectangularPolygon("concave")) !== JSON.stringify([
  { x: 100, y: 0 },
  { x: 100, y: 100 },
  { x: 0, y: 100 },
  { x: 0, y: 0 },
])) {
  throw new Error(`unexpected concave cluster rectangular boundary: ${JSON.stringify(concaveClusterRouter.clusterRectangularPolygon("concave"))}`);
}
concaveClusterRouter.addConnector("inside", 80, 20, 80, 80);
const insideClusterRoute = concaveClusterRouter.routeConnector("inside");
if (insideClusterRoute.length <= 2) {
  throw new Error(`inside-to-inside route left concave cluster: ${JSON.stringify(insideClusterRoute)}`);
}
for (let index = 1; index < insideClusterRoute.length; index += 1) {
  const a = insideClusterRoute[index - 1];
  const b = insideClusterRoute[index];
  const midpoint = { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
  if (midpoint.x > 30 && midpoint.x < 100 && midpoint.y > 30 && midpoint.y < 70) {
    throw new Error(`inside route segment crossed cluster exterior: ${JSON.stringify(insideClusterRoute)}`);
  }
}

const polylineRouter = AvoidRouter.withFlags(RouterFlag.PolyLineRouting | RouterFlag.OrthogonalRouting);
polylineRouter.setRoutingParameter(RoutingParameter.shapeBufferDistance, 10);
polylineRouter.addShape("box", 40, 40, 120, 120);
polylineRouter.addConnector("poly", 0, 80, 200, 80);
polylineRouter.setConnectorRoutingType("poly", ConnType.PolyLine);
const polylineRoute = polylineRouter.routeConnector("poly");
if (polylineRoute.length <= 2) {
  throw new Error(`expected polyline obstacle-avoiding route, got ${JSON.stringify(polylineRoute)}`);
}
if (!polylineRoute.some((point, index) => index > 0 && point.x !== polylineRoute[index - 1].x && point.y !== polylineRoute[index - 1].y)) {
  throw new Error(`expected polyline route to contain diagonal visibility segments, got ${JSON.stringify(polylineRoute)}`);
}
for (const point of polylineRoute) {
  if (point.x > 30 && point.x < 130 && point.y > 30 && point.y < 130) {
    throw new Error(`polyline route entered buffered obstacle: ${JSON.stringify(polylineRoute)}`);
  }
}

const raw = new WasmAvoidRouter(0);
raw.addConnector("straight", 0, 0, 10, 0);
if (raw.routeConnector("straight") !== "[{\"x\":0,\"y\":0},{\"x\":10,\"y\":0}]") {
  throw new Error("raw WASM binding did not return the expected JSON route");
}
if (raw.connectorNeedsRepaint("straight")) {
  throw new Error("raw connector unexpectedly needed repaint before transaction processing");
}
if (!raw.processTransaction() || !raw.connectorNeedsRepaint("straight")) {
  throw new Error("raw connector repaint state was not set by transaction processing");
}
if (raw.displayConnectorRoute("straight") !== "[{\"x\":0,\"y\":0},{\"x\":10,\"y\":0}]") {
  throw new Error("raw display route did not return the expected JSON route");
}
if (raw.connectorNeedsRepaint("straight")) {
  throw new Error("raw display route did not clear repaint state");
}
if (!raw.addPolygonShape("raw-triangle", [1, 1, 4, 2, 2, 6])) {
  throw new Error("raw WASM polygon shape was not created");
}
if (raw.shapeBoundingBox("raw-triangle") !== "{\"left\":1,\"top\":1,\"right\":4,\"bottom\":6}") {
  throw new Error(`unexpected raw polygon bounding box: ${raw.shapeBoundingBox("raw-triangle")}`);
}
if (!raw.addCluster("raw-cluster", [30, 30, 70, 30, 70, 70, 30, 70])) {
  throw new Error("raw WASM cluster was not created");
}
if (raw.objectIdIsUnused("raw-cluster")) {
  throw new Error("cluster id was not registered as used");
}
if (raw.clusterBoundingBox("raw-cluster") !== "{\"left\":30,\"top\":30,\"right\":70,\"bottom\":70}") {
  throw new Error(`unexpected raw cluster bounding box: ${raw.clusterBoundingBox("raw-cluster")}`);
}
if (!raw.setClusterPolygon("raw-cluster", [40, 40, 60, 40, 60, 60, 40, 60])) {
  throw new Error("raw WASM cluster polygon was not updated");
}
if (raw.clusterBoundingBox("raw-cluster") !== "{\"left\":30,\"top\":30,\"right\":70,\"bottom\":70}") {
  throw new Error(`raw cluster polygon update was not deferred: ${raw.clusterBoundingBox("raw-cluster")}`);
}
raw.processTransaction();
if (raw.clusterBoundingBox("raw-cluster") !== "{\"left\":40,\"top\":40,\"right\":60,\"bottom\":60}") {
  throw new Error(`unexpected updated cluster bounding box: ${raw.clusterBoundingBox("raw-cluster")}`);
}
if (!raw.removeCluster("raw-cluster") || raw.objectIdIsUnused("raw-cluster")) {
  throw new Error("raw WASM cluster removal was not deferred");
}
raw.processTransaction();
if (!raw.objectIdIsUnused("raw-cluster")) {
  throw new Error("raw WASM cluster was not removed");
}
if (!raw.addClusterRectangle("raw-rect-cluster", 90, 90, 10, 10)) {
  throw new Error("raw WASM rectangular cluster was not created");
}
if (raw.clusterBoundingBox("raw-rect-cluster") !== "{\"left\":10,\"top\":10,\"right\":90,\"bottom\":90}") {
  throw new Error(`unexpected raw rectangular cluster bounding box: ${raw.clusterBoundingBox("raw-rect-cluster")}`);
}
if (raw.addClusterRectangle("raw-invalid-cluster", Number.NaN, 0, 1, 1)) {
  throw new Error("raw WASM rectangular cluster accepted NaN");
}
if (!raw.setClusterRectangle("raw-rect-cluster", 80, 80, 20, 20)) {
  throw new Error("raw WASM rectangular cluster was not updated");
}
if (raw.clusterBoundingBox("raw-rect-cluster") !== "{\"left\":10,\"top\":10,\"right\":90,\"bottom\":90}") {
  throw new Error(`raw rectangular cluster update was not deferred: ${raw.clusterBoundingBox("raw-rect-cluster")}`);
}
raw.processTransaction();
if (raw.clusterBoundingBox("raw-rect-cluster") !== "{\"left\":20,\"top\":20,\"right\":80,\"bottom\":80}") {
  throw new Error(`unexpected raw rectangular cluster update: ${raw.clusterBoundingBox("raw-rect-cluster")}`);
}

const flagged = AvoidRouter.withFlags(RouterFlag.OrthogonalRouting);
if (flagged.routerFlags() !== RouterFlag.OrthogonalRouting) {
  throw new Error("router flags were not preserved");
}
if (!flagged.routingOption(RoutingOption.improveHyperedgeRoutesMovingJunctions)) {
  throw new Error("default routing option was not preserved");
}
if (
  flagged.routingParameter(RoutingParameter.segmentPenalty) !== 10 ||
  flagged.routingParameter(RoutingParameter.anglePenalty) !== 0 ||
  flagged.routingParameter(RoutingParameter.crossingPenalty) !== 0 ||
  flagged.routingParameter(RoutingParameter.clusterCrossingPenalty) !== 4000 ||
  flagged.routingParameter(RoutingParameter.fixedSharedPathPenalty) !== 0 ||
  flagged.routingParameter(RoutingParameter.portDirectionPenalty) !== 0 ||
  flagged.routingParameter(RoutingParameter.idealNudgingDistance) !== 4
) {
  throw new Error("default routing parameters did not match upstream Router constructor");
}
if (!flagged.setRoutingOption(RoutingOption.nudgeOrthogonalSegmentsConnectedToShapes, true)) {
  throw new Error("routing option could not be set");
}
if (!flagged.routingOption(RoutingOption.nudgeOrthogonalSegmentsConnectedToShapes)) {
  throw new Error("routing option value was not preserved");
}
if (flagged.setRoutingOption(99, true)) {
  throw new Error("invalid routing option was accepted");
}
if (!flagged.processTransaction() || flagged.processTransaction()) {
  throw new Error("routing option settings change was not reported once");
}
if (
  !flagged.setRoutingPenalty(RoutingParameter.crossingPenalty, 250) ||
  flagged.routingParameter(RoutingParameter.crossingPenalty) !== 250 ||
  !flagged.setRoutingPenalty(RoutingParameter.portDirectionPenalty) ||
  flagged.routingParameter(RoutingParameter.portDirectionPenalty) !== 100 ||
  flagged.setRoutingPenalty(99, 1)
) {
  throw new Error("routing penalty alias did not match routing parameter behavior");
}
if (!flagged.processTransaction() || flagged.processTransaction()) {
  throw new Error("routing parameter settings change was not reported once");
}
if (
  flagged.validConnType(ConnType.Orthogonal) !== ConnType.Orthogonal ||
  flagged.validConnType(ConnType.PolyLine) !== ConnType.Orthogonal ||
  flagged.validConnType() !== ConnType.Orthogonal
) {
  throw new Error("valid connector type did not follow router flags");
}
flagged.addConnector("type-normalized", 0, 0, 10, 0);
if (
  !flagged.setConnectorRoutingType("type-normalized", ConnType.PolyLine) ||
  flagged.connectorRoutingType("type-normalized") !== ConnType.Orthogonal ||
  !flagged.setConnectorRoutingType("type-normalized", ConnType.None) ||
  flagged.connectorRoutingType("type-normalized") !== ConnType.Orthogonal ||
  flagged.setConnectorRoutingType("type-normalized", 99)
) {
  throw new Error("connector routing type did not normalize through router flags");
}
const typeTransactionRouter = AvoidRouter.withFlags(RouterFlag.PolyLineRouting | RouterFlag.OrthogonalRouting);
typeTransactionRouter.addConnector("type-edge", 0, 0, 10, 0);
typeTransactionRouter.processTransaction();
typeTransactionRouter.displayConnectorRoute("type-edge");
if (!typeTransactionRouter.setConnectorRoutingType("type-edge", ConnType.PolyLine)) {
  throw new Error("routing type change was rejected");
}
if (typeTransactionRouter.connectorNeedsRepaint("type-edge")) {
  throw new Error("routing type change was repainted before processTransaction");
}
if (!typeTransactionRouter.processTransaction() || !typeTransactionRouter.connectorNeedsRepaint("type-edge")) {
  throw new Error("routing type change was not processed by transaction");
}
typeTransactionRouter.displayConnectorRoute("type-edge");
if (!typeTransactionRouter.setConnectorRoutingType("type-edge", ConnType.PolyLine)) {
  throw new Error("same routing type update was rejected");
}
if (typeTransactionRouter.processTransaction()) {
  throw new Error("same routing type update reported transaction work");
}
typeTransactionRouter.setTransactionUse(false);
if (
  !typeTransactionRouter.setConnectorRoutingType("type-edge", ConnType.Orthogonal) ||
  !typeTransactionRouter.connectorNeedsRepaint("type-edge")
) {
  throw new Error("routing type change did not process immediately with transactions disabled");
}

flagged.addShape("left", 0, 0, 40, 40);
flagged.addShape("right", 100, 0, 140, 40);
if (
  flagged.objectIdIsUnused("left") ||
  !flagged.objectIdIsUnused("not-used") ||
  flagged.shapePosition("left").x !== 20 ||
  flagged.shapePosition("left").y !== 20 ||
  flagged.shapeContainingPoint([0, 20]) !== "left" ||
  flagged.shapeContainingPoint({ x: 80, y: 20 }) !== null
) {
  throw new Error("shape/object query metadata was wrong");
}
flagged.addShapeConnectionPinWithOptions("left", "out-a", {
  classId: 1,
  xOffset: AttachPos.Right,
  yOffset: 0.25,
  proportional: true,
  visibilityDirections: ConnDir.Right,
  connectionCost: 10,
  exclusive: true,
});
flagged.addShapeConnectionPinWithOptions("left", "out-b", {
  classId: 1,
  xOffset: AttachPos.Right,
  yOffset: 0.75,
  proportional: true,
  visibilityDirections: ConnDir.Right,
  connectionCost: 1,
  exclusive: true,
});
flagged.addShapeConnectionPinWithOptions("right", "in-a", {
  classId: 2,
  xOffset: AttachPos.Left,
  yOffset: 0.25,
  proportional: true,
  visibilityDirections: ConnDir.Left,
  exclusive: true,
});
flagged.addShapeConnectionPinWithOptions("right", "in-b", {
  classId: 2,
  xOffset: AttachPos.Left,
  yOffset: 0.75,
  proportional: true,
  visibilityDirections: ConnDir.Left,
  exclusive: true,
});
flagged.addShapeConnectionPinWithOptions("left", "auto-corner", {
  classId: 9,
  xOffset: AttachPos.Left,
  yOffset: AttachPos.Top,
  proportional: true,
  insideOffset: 2,
  visibilityDirections: ConnDir.None,
});
flagged.addShapeConnectionPinWithOptions("right", "absolute-offset", {
  classId: 10,
  xOffset: 0,
  yOffset: 10,
  proportional: false,
  visibilityDirections: ConnDir.None,
});
flagged.addShapeConnectionPinWithOptions("right", "absolute-max", {
  classId: 11,
  xOffset: AttachPos.MaxOffset,
  yOffset: AttachPos.MaxOffset,
  proportional: false,
  insideOffset: 2,
  visibilityDirections: ConnDir.None,
});

const pinPosition = flagged.shapeConnectionPinPosition("left", "out-a");
if (
  pinPosition.x !== 40 ||
  pinPosition.y !== 10 ||
  flagged.shapeConnectionPinDirections("left", "out-a") !== ConnDir.Right ||
  JSON.stringify(flagged.shapeConnectionPinIds("left", "out-a")) !==
    JSON.stringify({ objectId: "left", classId: 1 }) ||
  flagged.shapeConnectionPinConnectionCost("left", "out-a") !== 10 ||
  !flagged.shapeConnectionPinExclusive("left", "out-a")
) {
  throw new Error(`shape connection pin metadata was wrong: ${JSON.stringify(pinPosition)}`);
}
const autoCornerPosition = flagged.shapeConnectionPinPosition("left", "auto-corner");
if (
  autoCornerPosition.x !== 2 ||
  autoCornerPosition.y !== 2 ||
  flagged.shapeConnectionPinDirections("left", "auto-corner") !== (ConnDir.Left | ConnDir.Up)
) {
  throw new Error(`automatic corner pin metadata was wrong: ${JSON.stringify(autoCornerPosition)}`);
}
const absoluteOffsetPosition = flagged.shapeConnectionPinPosition("right", "absolute-offset");
const absoluteMaxPosition = flagged.shapeConnectionPinPosition("right", "absolute-max");
if (
  absoluteOffsetPosition.x !== 100 ||
  absoluteOffsetPosition.y !== 10 ||
  flagged.shapeConnectionPinDirections("right", "absolute-offset") !== ConnDir.Left ||
  absoluteMaxPosition.x !== 138 ||
  absoluteMaxPosition.y !== 38 ||
  flagged.shapeConnectionPinDirections("right", "absolute-max") !== ConnDir.All
) {
  throw new Error(
    `absolute pin metadata was wrong: ${JSON.stringify({ absoluteOffsetPosition, absoluteMaxPosition })}`,
  );
}
if (
  !flagged.setShapeConnectionPinConnectionCost("left", "out-a", 0.5) ||
  flagged.shapeConnectionPinConnectionCost("left", "out-a") !== 0.5 ||
  flagged.setShapeConnectionPinConnectionCost("left", "out-a", -1) ||
  !flagged.setShapeConnectionPinExclusive("left", "out-a", false) ||
  flagged.shapeConnectionPinExclusive("left", "out-a") ||
  !flagged.addShapeConnectionPin("left", "specific", 30, 20) ||
  flagged.shapeConnectionPinIds("left", "specific").classId !== ConnectionPin.Unset ||
  flagged.addShapeConnectionPinWithOptions("left", "out-a", {
    classId: 0,
    xOffset: AttachPos.Right,
    yOffset: AttachPos.Centre,
    proportional: true,
    visibilityDirections: ConnDir.Right,
  }) ||
  flagged.shapeConnectionPinIds("left", "out-a").classId !== 1 ||
  flagged.addShapeConnectionPin("left", "nan-point", Number.NaN, 20) ||
  flagged.shapeConnectionPinPosition("left", "nan-point") !== null ||
  flagged.addShapeConnectionPinWithOptions("left", "out-a", {
    classId: 1,
    xOffset: Number.NaN,
    yOffset: AttachPos.Centre,
    proportional: true,
    visibilityDirections: ConnDir.Right,
  }) ||
  flagged.shapeConnectionPinPosition("left", "out-a").x !== 40 ||
  flagged.shapeConnectionPinPosition("left", "out-a").y !== 10 ||
  flagged.shapeConnectionPinPosition("left", "missing") !== null ||
  flagged.shapeConnectionPinIds("left", "missing") !== null
) {
  throw new Error("shape connection pin mutation failed");
}
if (
  !flagged.transformShapeConnectionPinPositions("left", ShapeTransformation.CW90) ||
  flagged.shapeConnectionPinPosition("left", "out-a").x !== 30 ||
  flagged.shapeConnectionPinPosition("left", "out-a").y !== 40 ||
  flagged.shapeConnectionPinDirections("left", "out-a") !== ConnDir.Down ||
  flagged.transformShapeConnectionPinPositions("left", 99)
) {
  throw new Error("shape connection pin transformation failed");
}

if (!flagged.addConnectorBetweenPinClasses("pin-edge", "left", 1, "right", 2)) {
  throw new Error("pin class connector was not created");
}
const pinRoute = flagged.routeConnector("pin-edge");
if (pinRoute[0].x !== 30 || pinRoute[0].y !== 40 || pinRoute.at(-1).x !== 100) {
  throw new Error(`unexpected pin class route: ${JSON.stringify(pinRoute)}`);
}
flagged.processTransaction();
flagged.displayConnectorRoute("pin-edge");
if (
  flagged.connectorNeedsRepaint("pin-edge") ||
  !flagged.setShapeConnectionPinConnectionCost("left", "out-a", 0.75) ||
  flagged.connectorNeedsRepaint("pin-edge") ||
  !flagged.processTransaction() ||
  !flagged.connectorNeedsRepaint("pin-edge")
) {
  throw new Error("shape connection pin mutation did not invalidate connector transactionally");
}
flagged.displayConnectorRoute("pin-edge");
const possibleDstPins = flagged.connectorPossibleDstPinPoints("pin-edge");
if (
  possibleDstPins.length !== 1 ||
  possibleDstPins[0].x !== 100 ||
  possibleDstPins[0].y !== 30
) {
  throw new Error(`unexpected possible destination pin points: ${JSON.stringify(possibleDstPins)}`);
}
if (flagged.connectorPossibleDstPinPoints("missing") !== null) {
  throw new Error("missing connector possible destination pins should be null");
}
flagged.addConnector("point-edge", 0, 0, 10, 0);
if (flagged.connectorPossibleDstPinPoints("point-edge").length !== 0) {
  throw new Error("point destination connector should not report destination pin points");
}

if (!flagged.addConnectorBetweenPinClasses("pin-edge-reuse", "left", 1, "right", 2)) {
  throw new Error("non-exclusive pin class connector was not reused");
}
const reusedPinEnds = flagged.connectorEndpointConnEnds("pin-edge-reuse");
if (
  reusedPinEnds.source.type !== "shapePin" ||
  reusedPinEnds.source.pinId !== "out-a" ||
  reusedPinEnds.source.classId !== 1 ||
  reusedPinEnds.source.position.x !== 30 ||
  reusedPinEnds.source.position.y !== 40 ||
  reusedPinEnds.source.directions !== ConnDir.Down
) {
  throw new Error(`pin exclusivity mutation was not used: ${JSON.stringify(reusedPinEnds)}`);
}

const pinLifecycleRouter = new AvoidRouter(0);
pinLifecycleRouter.addShape("left", 0, 0, 40, 40);
pinLifecycleRouter.addShape("right", 100, 0, 140, 40);
pinLifecycleRouter.addShapeConnectionPinWithOptions("left", "first", {
  classId: 1,
  xOffset: 40,
  yOffset: 10,
  visibilityDirections: ConnDir.Right,
  connectionCost: 0,
  exclusive: true,
});
pinLifecycleRouter.addShapeConnectionPinWithOptions("left", "fallback", {
  classId: 1,
  xOffset: 40,
  yOffset: 30,
  visibilityDirections: ConnDir.Right,
  connectionCost: 5,
  exclusive: true,
});
pinLifecycleRouter.addShapeConnectionPinWithOptions("right", "in", {
  classId: 2,
  xOffset: 100,
  yOffset: 20,
  visibilityDirections: ConnDir.Left,
  connectionCost: 0,
  exclusive: true,
});
if (!pinLifecycleRouter.addConnectorBetweenPinClasses("edge", "left", 1, "right", 2)) {
  throw new Error("pin lifecycle class connector was not created");
}
if (pinLifecycleRouter.connectorEndpointConnEnds("edge").source.pinId !== "first") {
  throw new Error("pin lifecycle connector did not use first pin");
}
if (
  !pinLifecycleRouter.removeShapeConnectionPin("left", "first") ||
  pinLifecycleRouter.connectorEndpointConnEnds("edge").source.pinId !== "first" ||
  !pinLifecycleRouter.processTransaction() ||
  pinLifecycleRouter.connectorEndpointConnEnds("edge").source.pinId !== "fallback"
) {
  throw new Error("class pin endpoint was not reassigned after active pin removal");
}
if (
  !pinLifecycleRouter.addShapeConnectionPinWithOptions("left", "cheaper", {
    classId: 1,
    xOffset: 40,
    yOffset: 15,
    visibilityDirections: ConnDir.Right,
    connectionCost: 10,
    exclusive: true,
  }) ||
  !pinLifecycleRouter.setShapeConnectionPinConnectionCost("left", "cheaper", 0.25) ||
  !pinLifecycleRouter.processTransaction() ||
  pinLifecycleRouter.connectorEndpointConnEnds("edge").source.pinId !== "cheaper"
) {
  throw new Error("class pin endpoint was not reselected after pin cost change");
}

const leftAttachedConnectors = flagged.shapeAttachedConnectors("left");
if (
  !leftAttachedConnectors.includes("pin-edge") ||
  !leftAttachedConnectors.includes("pin-edge-reuse") ||
  flagged.shapeAttachedConnectors("missing") !== null
) {
  throw new Error(`shape attached connectors were wrong: ${JSON.stringify(leftAttachedConnectors)}`);
}
const leftAttachedShapes = flagged.shapeAttachedShapes("left");
if (!leftAttachedShapes.includes("right") || flagged.shapeAttachedShapes("missing") !== null) {
  throw new Error(`shape attached shapes were wrong: ${JSON.stringify(leftAttachedShapes)}`);
}
const rightRunningToShapes = flagged.shapeAttachedShapes("right", ConnectorRelation.RunningTo);
if (!rightRunningToShapes.includes("left")) {
  throw new Error(`running-to attached shapes were wrong: ${JSON.stringify(rightRunningToShapes)}`);
}
const leftRunningToShapes = flagged.shapeAttachedShapes("left", ConnectorRelation.RunningTo);
if (leftRunningToShapes.includes("right")) {
  throw new Error(`running-to filter returned a source-side shape: ${JSON.stringify(leftRunningToShapes)}`);
}

flagged.moveShapeBy("left", 5, -10);
flagged.processTransaction();
const movedPinRoute = flagged.routeConnector("pin-edge");
if (movedPinRoute[0].x !== 35 || movedPinRoute[0].y !== 30) {
  throw new Error(`pin did not follow relative shape move: ${JSON.stringify(movedPinRoute)}`);
}

const polygonRouter = new AvoidRouter();
if (
  !polygonRouter.addPolygonShape("triangle", [
    { x: 1, y: 1 },
    { x: 4, y: 2 },
    { x: 2, y: 6 },
  ])
) {
  throw new Error("wrapper polygon shape was not created");
}
const bbox = polygonRouter.shapeBoundingBox("triangle");
if (bbox.left !== 1 || bbox.top !== 1 || bbox.right !== 4 || bbox.bottom !== 6) {
  throw new Error(`unexpected wrapper polygon bounding box: ${JSON.stringify(bbox)}`);
}
polygonRouter.moveShapeBy("triangle", 10, -1);
polygonRouter.processTransaction();
const movedPolygon = polygonRouter.shapePolygon("triangle");
if (movedPolygon[0].x !== 11 || movedPolygon[0].y !== 0) {
  throw new Error(`polygon points did not move: ${JSON.stringify(movedPolygon)}`);
}

const polygonRouteRouter = new AvoidRouter(0);
polygonRouteRouter.addPolygonShape("triangle", [
  [40, 0],
  [80, 40],
  [40, 80],
]);
polygonRouteRouter.addConnector("clear", 55, 70, 100, 70);
const clearPolygonRoute = polygonRouteRouter.routeConnector("clear");
if (clearPolygonRoute.length !== 2) {
  throw new Error(`expected polygon-aware straight route: ${JSON.stringify(clearPolygonRoute)}`);
}

const checkpointRouter = new AvoidRouter(0);
checkpointRouter.addShape("blocked", 40, 40, 80, 80);
checkpointRouter.addConnector("via", 0, 0, 120, 0);
if (
  !checkpointRouter.setConnectorRoutingCheckpoints("via", [
    [20, 0],
    { x: 60, y: 60 },
    { point: { x: 100, y: 0 } },
  ])
) {
  throw new Error("checkpoint configuration was rejected");
}
const checkpoints = checkpointRouter.connectorRoutingCheckpoints("via");
if (checkpoints.length !== 3 || checkpoints[2].arrivalDirections !== ConnDir.All) {
  throw new Error(`unexpected checkpoint metadata: ${JSON.stringify(checkpoints)}`);
}
const checkpointRoute = checkpointRouter.routeConnector("via");
if (
  checkpointRoute.length !== 4 ||
  checkpointRoute[1].x !== 20 ||
  checkpointRoute[2].x !== 100 ||
  checkpointRoute.some((point) => point.x === 60 && point.y === 60)
) {
  throw new Error(`unexpected checkpoint route: ${JSON.stringify(checkpointRoute)}`);
}

const directedCheckpointRouter = new AvoidRouter(0);
directedCheckpointRouter.addShape("blocked", 40, 40, 80, 80);
directedCheckpointRouter.addConnector("via", 0, 0, 120, 0);
directedCheckpointRouter.setConnectorRoutingCheckpoints("via", [
  { point: { x: 100, y: 0 }, arrivalDirections: ConnDir.Up },
]);
const directedCheckpointRoute = directedCheckpointRouter.routeConnector("via");
const directedCheckpointIndex = directedCheckpointRoute.findIndex(
  (point) => point.x === 100 && point.y === 0,
);
if (
  directedCheckpointIndex <= 0 ||
  directedCheckpointRoute[directedCheckpointIndex - 1].y <= 0 ||
  directedCheckpointRoute.at(-1).x !== 120
) {
  throw new Error(`checkpoint direction was not enforced: ${JSON.stringify(directedCheckpointRoute)}`);
}

const directedEndpointRouter = new AvoidRouter(0);
directedEndpointRouter.addShape("blocked", 40, 40, 80, 80);
directedEndpointRouter.addConnectorWithEndpointDirections(
  "endpoint-dirs",
  [0, 0],
  ConnDir.Up,
  { x: 120, y: 0 },
  ConnDir.Up,
);
const directedEndpointEnds = directedEndpointRouter.connectorEndpointConnEnds("endpoint-dirs");
if (
  directedEndpointEnds.source.type !== "point" ||
  directedEndpointEnds.source.position.x !== 0 ||
  directedEndpointEnds.source.position.y !== 0 ||
  directedEndpointEnds.source.directions !== ConnDir.Up ||
  directedEndpointEnds.destination.type !== "point" ||
  directedEndpointEnds.destination.position.x !== 120 ||
  directedEndpointEnds.destination.position.y !== 0 ||
  directedEndpointEnds.destination.directions !== ConnDir.Up
) {
  throw new Error(`endpoint directions were not reported: ${JSON.stringify(directedEndpointEnds)}`);
}
const directedEndpointRoute = directedEndpointRouter.routeConnector("endpoint-dirs");
if (
  directedEndpointRoute.length <= 2 ||
  directedEndpointRoute[1].y <= 0 ||
  directedEndpointRoute.at(-2).y <= 0
) {
  throw new Error(`endpoint directions were not enforced: ${JSON.stringify(directedEndpointRoute)}`);
}
if (
  !directedEndpointRouter.setConnectorSourceEndpointWithDirections(
    "endpoint-dirs",
    { x: 0, y: 10 },
    ConnDir.Right,
  ) ||
  !directedEndpointRouter.processTransaction() ||
  directedEndpointRouter.connectorEndpointConnEnds("endpoint-dirs").source.directions !== ConnDir.Right
) {
  throw new Error("source endpoint direction update failed");
}
directedEndpointRouter.addShape("pin-left", 0, 100, 40, 140);
directedEndpointRouter.addShape("pin-right", 100, 100, 140, 140);
directedEndpointRouter.addShapeConnectionPin("pin-left", "east", 40, 120);
directedEndpointRouter.addShapeConnectionPin("pin-right", "west", 100, 120);
directedEndpointRouter.addConnector("pin-update", 0, 0, 10, 0);
if (
  !directedEndpointRouter.setConnectorSourcePinEndpoint("pin-update", "pin-left", "east") ||
  !directedEndpointRouter.setConnectorDestPinEndpoint("pin-update", "pin-right", "west")
) {
  throw new Error("shape pin endpoint update was rejected");
}
let pinUpdateEnds = directedEndpointRouter.connectorEndpointConnEnds("pin-update");
if (pinUpdateEnds.source.type !== "point" || pinUpdateEnds.destination.type !== "point") {
  throw new Error(`shape pin endpoint update was not deferred: ${JSON.stringify(pinUpdateEnds)}`);
}
directedEndpointRouter.processTransaction();
pinUpdateEnds = directedEndpointRouter.connectorEndpointConnEnds("pin-update");
if (
  pinUpdateEnds.source.type !== "shapePin" ||
  pinUpdateEnds.source.pinId !== "east" ||
  pinUpdateEnds.source.position.x !== 40 ||
  pinUpdateEnds.source.position.y !== 120 ||
  pinUpdateEnds.source.directions !== ConnDir.All ||
  pinUpdateEnds.destination.type !== "shapePin" ||
  pinUpdateEnds.destination.pinId !== "west" ||
  pinUpdateEnds.destination.position.x !== 100 ||
  pinUpdateEnds.destination.position.y !== 120 ||
  pinUpdateEnds.destination.directions !== ConnDir.Left
) {
  throw new Error(`shape pin endpoint update failed: ${JSON.stringify(pinUpdateEnds)}`);
}
directedEndpointRouter.addShapeConnectionPinWithOptions("pin-left", "class-expensive", {
  classId: 4,
  xOffset: AttachPos.Right,
  yOffset: 0.25,
  proportional: true,
  visibilityDirections: ConnDir.Right,
  connectionCost: 10,
  exclusive: true,
});
directedEndpointRouter.addShapeConnectionPinWithOptions("pin-left", "class-cheap", {
  classId: 4,
  xOffset: AttachPos.Right,
  yOffset: 0.75,
  proportional: true,
  visibilityDirections: ConnDir.Right,
  connectionCost: 1,
  exclusive: true,
});
directedEndpointRouter.addShapeConnectionPinWithOptions("pin-right", "class-target", {
  classId: 5,
  xOffset: AttachPos.Left,
  yOffset: AttachPos.Centre,
  proportional: true,
  visibilityDirections: ConnDir.Left,
  exclusive: true,
});
directedEndpointRouter.addConnector("pin-class-update", 0, 0, 10, 0);
if (
  !directedEndpointRouter.setConnectorSourcePinClassEndpoint("pin-class-update", "pin-left", 4) ||
  !directedEndpointRouter.setConnectorDestPinClassEndpoint("pin-class-update", "pin-right", 5)
) {
  throw new Error("shape pin class endpoint update was rejected");
}
let pinClassUpdateEnds = directedEndpointRouter.connectorEndpointConnEnds("pin-class-update");
if (pinClassUpdateEnds.source.type !== "point" || pinClassUpdateEnds.destination.type !== "point") {
  throw new Error(`shape pin class endpoint update was not deferred: ${JSON.stringify(pinClassUpdateEnds)}`);
}
directedEndpointRouter.processTransaction();
pinClassUpdateEnds = directedEndpointRouter.connectorEndpointConnEnds("pin-class-update");
if (
  pinClassUpdateEnds.source.type !== "shapePin" ||
  pinClassUpdateEnds.source.pinId !== "class-cheap" ||
  pinClassUpdateEnds.source.classId !== 4 ||
  pinClassUpdateEnds.destination.type !== "shapePin" ||
  pinClassUpdateEnds.destination.pinId !== "class-target" ||
  pinClassUpdateEnds.destination.classId !== 5 ||
  directedEndpointRouter.setConnectorSourcePinClassEndpoint("pin-class-update", "pin-left", 0) ||
  directedEndpointRouter.setConnectorDestPinClassEndpoint(
    "pin-class-update",
    "pin-right",
    ConnectionPin.Unset,
  ) ||
  directedEndpointRouter.addConnectorBetweenPinClasses("invalid-zero-class", "pin-left", 0, "pin-right", 5) ||
  directedEndpointRouter.addConnectorBetweenPinClasses(
    "invalid-unset-class",
    "pin-left",
    ConnectionPin.Unset,
    "pin-right",
    5,
  )
) {
  throw new Error(`shape pin class endpoint update failed: ${JSON.stringify(pinClassUpdateEnds)}`);
}
if (!directedEndpointRouter.setConnectorDestPinClassEndpoint("pin-class-update", "pin-right", 99)) {
  throw new Error("missing but valid shape pin class endpoint was rejected");
}
directedEndpointRouter.processTransaction();
pinClassUpdateEnds = directedEndpointRouter.connectorEndpointConnEnds("pin-class-update");
if (
  pinClassUpdateEnds.destination.type !== "shapePin" ||
  pinClassUpdateEnds.destination.pinId !== undefined ||
  pinClassUpdateEnds.destination.classId !== 99 ||
  pinClassUpdateEnds.destination.position.x !== 120 ||
  pinClassUpdateEnds.destination.position.y !== 120
) {
  throw new Error(`missing shape pin class did not fall back to shape position: ${JSON.stringify(pinClassUpdateEnds)}`);
}
if (!directedEndpointRouter.addConnectorBetweenPinClasses("missing-class-edge", "pin-left", 77, "pin-right", 88)) {
  throw new Error("missing but valid shape pin class connector was rejected");
}
const missingShapeClassEnds = directedEndpointRouter.connectorEndpointConnEnds("missing-class-edge");
if (
  missingShapeClassEnds.source.pinId !== undefined ||
  missingShapeClassEnds.source.classId !== 77 ||
  missingShapeClassEnds.source.position.x !== 20 ||
  missingShapeClassEnds.source.position.y !== 120 ||
  missingShapeClassEnds.destination.pinId !== undefined ||
  missingShapeClassEnds.destination.classId !== 88 ||
  missingShapeClassEnds.destination.position.x !== 120 ||
  missingShapeClassEnds.destination.position.y !== 120
) {
  throw new Error(`missing shape pin class connector did not use anchor positions: ${JSON.stringify(missingShapeClassEnds)}`);
}
const detachShapeRouter = new AvoidRouter(0);
detachShapeRouter.addShape("left", 0, 0, 40, 40);
detachShapeRouter.addShape("right", 100, 0, 140, 40);
detachShapeRouter.addShapeConnectionPinWithOptions("left", "out", {
  classId: 1,
  xOffset: AttachPos.Right,
  yOffset: AttachPos.Centre,
  proportional: true,
  visibilityDirections: ConnDir.Right,
  exclusive: true,
});
detachShapeRouter.addShapeConnectionPinWithOptions("right", "in", {
  classId: 2,
  xOffset: AttachPos.Left,
  yOffset: AttachPos.Centre,
  proportional: true,
  visibilityDirections: ConnDir.Left,
  exclusive: true,
});
if (!detachShapeRouter.addConnectorBetweenPins("edge", "left", "out", "right", "in")) {
  throw new Error("shape pin connector for deletion detach was rejected");
}
if (!detachShapeRouter.removeShape("left") || detachShapeRouter.routeConnector("edge") === null) {
  throw new Error("shape deletion detach was applied too early");
}
detachShapeRouter.processTransaction();
let detachedShapeEnds = detachShapeRouter.connectorEndpointConnEnds("edge");
if (
  detachedShapeEnds.source.type !== "point" ||
  detachedShapeEnds.source.position.x !== 40 ||
  detachedShapeEnds.source.position.y !== 20 ||
  detachedShapeEnds.destination.type !== "shapePin"
) {
  throw new Error(`shape deletion did not detach source endpoint: ${JSON.stringify(detachedShapeEnds)}`);
}
detachShapeRouter.setTransactionUse(false);
if (!detachShapeRouter.removeShape("right")) {
  throw new Error("immediate shape deletion detach was rejected");
}
detachedShapeEnds = detachShapeRouter.connectorEndpointConnEnds("edge");
if (
  detachedShapeEnds.destination.type !== "point" ||
  detachedShapeEnds.destination.position.x !== 100 ||
  detachedShapeEnds.destination.position.y !== 20
) {
  throw new Error(`immediate shape deletion did not detach destination endpoint: ${JSON.stringify(detachedShapeEnds)}`);
}

const fixedRouteRouter = new AvoidRouter(0);
fixedRouteRouter.addShape("blocked", 40, 40, 80, 80);
fixedRouteRouter.addConnector("fixed", 0, 60, 120, 60);
if (
  !fixedRouteRouter.setConnectorFixedRoute("fixed", [
    [0, 60],
    { x: 60, y: 60 },
    [120, 60],
  ])
) {
  throw new Error("fixed route was rejected");
}
if (!fixedRouteRouter.processTransaction() || fixedRouteRouter.processTransaction()) {
  throw new Error("fixed route settings change was not reported once");
}
if (!fixedRouteRouter.connectorHasFixedRoute("fixed")) {
  throw new Error("fixed route flag was not set");
}
const fixedRoute = fixedRouteRouter.routeConnector("fixed");
if (fixedRoute.length !== 3 || fixedRoute[1].x !== 60 || fixedRoute[1].y !== 60) {
  throw new Error(`fixed route was not returned verbatim: ${JSON.stringify(fixedRoute)}`);
}
if (!fixedRouteRouter.clearConnectorFixedRoute("fixed")) {
  throw new Error("fixed route was not cleared");
}
if (!fixedRouteRouter.processTransaction()) {
  throw new Error("cleared fixed route settings change was not reported");
}
const unfixedRoute = fixedRouteRouter.routeConnector("fixed");
if (unfixedRoute.length <= 2) {
  throw new Error(`automatic routing did not resume after clearing fixed route: ${JSON.stringify(unfixedRoute)}`);
}

const invalidPathRouter = AvoidRouter.withFlags(RouterFlag.PolyLineRouting | RouterFlag.OrthogonalRouting);
invalidPathRouter.addConnector("diag", 0, 0, 10, 0);
if (invalidPathRouter.existsInvalidOrthogonalPaths()) {
  throw new Error("straight orthogonal route was reported invalid");
}
invalidPathRouter.setConnectorFixedRoute("diag", [
  [0, 0],
  [10, 10],
]);
if (!invalidPathRouter.existsInvalidOrthogonalPaths()) {
  throw new Error("diagonal orthogonal display route was not reported invalid");
}
invalidPathRouter.setConnectorRoutingType("diag", ConnType.PolyLine);
if (invalidPathRouter.existsInvalidOrthogonalPaths()) {
  throw new Error("polyline connector was included in orthogonal diagnostic");
}

const crossingRouter = new AvoidRouter(0);
crossingRouter.addConnector("a", 0, 0, 10, 10);
crossingRouter.addConnector("b", 0, 10, 10, 0);
crossingRouter.setConnectorFixedRoute("a", [
  [0, 0],
  [10, 10],
]);
crossingRouter.setConnectorFixedRoute("b", [
  [0, 10],
  [10, 0],
]);
if (crossingRouter.existsCrossings() !== 1 || crossingRouter.existsCrossings(true) !== 1) {
  throw new Error("strict route crossing was not counted");
}
crossingRouter.setConnectorFixedRoute("b", [
  [0, 10],
  [10, 10],
]);
if (crossingRouter.existsCrossings() !== 0) {
  throw new Error("non-crossing routes were counted as crossings");
}

const overlapRouter = new AvoidRouter(0);
overlapRouter.addConnector("a", 0, 0, 10, 0);
overlapRouter.addConnector("b", 5, 0, 15, 0);
overlapRouter.setConnectorFixedRoute("a", [
  [0, 0],
  [10, 0],
]);
overlapRouter.setConnectorFixedRoute("b", [
  [5, 0],
  [15, 0],
]);
if (overlapRouter.existsOrthogonalSegmentOverlap() || !overlapRouter.existsOrthogonalSegmentOverlap(true)) {
  throw new Error("terminal segment overlap did not honour atEnds");
}
if (
  overlapRouter.existsOrthogonalFixedSegmentOverlap() ||
  !overlapRouter.existsOrthogonalFixedSegmentOverlap(true)
) {
  throw new Error("terminal fixed segment overlap did not honour atEnds");
}
overlapRouter.setConnectorFixedRoute("a", [
  [0, 10],
  [0, 0],
  [10, 0],
  [10, 10],
]);
overlapRouter.setConnectorFixedRoute("b", [
  [5, -10],
  [5, 0],
  [15, 0],
  [15, -10],
]);
if (!overlapRouter.existsOrthogonalSegmentOverlap()) {
  throw new Error("internal shared orthogonal segment was not reported");
}
if (!overlapRouter.existsOrthogonalFixedSegmentOverlap()) {
  throw new Error("internal fixed shared orthogonal segment was not reported");
}
const automaticOverlapRouter = new AvoidRouter(0);
automaticOverlapRouter.addConnector("a", 0, 0, 10, 0);
automaticOverlapRouter.addConnector("b", 5, 0, 15, 0);
automaticOverlapRouter.processTransaction();
if (
  !automaticOverlapRouter.existsOrthogonalSegmentOverlap(true) ||
  automaticOverlapRouter.existsOrthogonalFixedSegmentOverlap(true)
) {
  throw new Error("non-fixed overlap was misclassified as fixed");
}
const touchingRouter = new AvoidRouter(0);
touchingRouter.addConnector("a", 0, 0, 10, 0);
touchingRouter.addConnector("b", 10, 0, 10, 10);
touchingRouter.setConnectorFixedRoute("a", [
  [0, 0],
  [10, 0],
]);
touchingRouter.setConnectorFixedRoute("b", [
  [10, 0],
  [10, 10],
]);
if (!touchingRouter.existsOrthogonalTouchingPaths() || touchingRouter.existsOrthogonalSegmentOverlap(true)) {
  throw new Error("endpoint-only touching route contact was not reported correctly");
}
touchingRouter.setConnectorFixedRoute("b", [
  [20, 0],
  [20, 10],
]);
if (touchingRouter.existsOrthogonalTouchingPaths()) {
  throw new Error("separated routes were reported as touching");
}

const fixedExistingRouter = new AvoidRouter(0);
fixedExistingRouter.addShape("blocked", 40, 40, 80, 80);
fixedExistingRouter.addConnector("existing", 0, 60, 120, 60);
const existingRoute = fixedExistingRouter.routeConnector("existing");
if (!fixedExistingRouter.setConnectorFixedExistingRoute("existing")) {
  throw new Error("fixed existing route was rejected");
}
fixedExistingRouter.removeShape("blocked");
const frozenExistingRoute = fixedExistingRouter.routeConnector("existing");
if (JSON.stringify(frozenExistingRoute) !== JSON.stringify(existingRoute)) {
  throw new Error(`existing fixed route was not frozen: ${JSON.stringify(frozenExistingRoute)}`);
}

const endpointRouter = new AvoidRouter(0);
endpointRouter.setTransactionUse(false);
endpointRouter.addConnector("move", 0, 0, 10, 0);
if (!endpointRouter.setConnectorSourceEndpoint("move", { x: 5, y: 5 })) {
  throw new Error("source endpoint update was rejected");
}
if (!endpointRouter.setConnectorDestEndpoint("move", [20, 5])) {
  throw new Error("destination endpoint update was rejected");
}
let movedEndpointRoute = endpointRouter.routeConnector("move");
if (
  movedEndpointRoute[0].x !== 5 ||
  movedEndpointRoute[0].y !== 5 ||
  movedEndpointRoute.at(-1).x !== 20 ||
  movedEndpointRoute.at(-1).y !== 5
) {
  throw new Error(`endpoint updates were not used: ${JSON.stringify(movedEndpointRoute)}`);
}
if (!endpointRouter.setConnectorEndpoints("move", [1, 2], { x: 3, y: 4 })) {
  throw new Error("endpoint pair update was rejected");
}
movedEndpointRoute = endpointRouter.routeConnector("move");
if (movedEndpointRoute[0].x !== 1 || movedEndpointRoute.at(-1).x !== 3) {
  throw new Error(`endpoint pair update was not used: ${JSON.stringify(movedEndpointRoute)}`);
}
if (!endpointRouter.processTransaction() || !endpointRouter.connectorNeedsRepaint("move")) {
  throw new Error("wrapper connector repaint state was not set after endpoint update");
}
const endpointDisplayRoute = endpointRouter.displayConnectorRoute("move");
if (
  endpointDisplayRoute[0].x !== 1 ||
  endpointDisplayRoute.at(-1).x !== 3 ||
  endpointRouter.connectorNeedsRepaint("move")
) {
  throw new Error(`wrapper display route/repaint state failed: ${JSON.stringify(endpointDisplayRoute)}`);
}
if (!endpointRouter.makeConnectorPathInvalid("move") || !endpointRouter.processTransaction()) {
  throw new Error("wrapper explicit path invalidation was not processed");
}

const splitRouter = new AvoidRouter(0);
splitRouter.addConnector("edge", 0, 0, 100, 0);
splitRouter.processTransaction();
const splitResult = splitRouter.splitConnectorAtSegment("edge", 1, "junction", "edge-tail");
if (
  !splitResult ||
  splitResult.junctionId !== "junction" ||
  splitResult.connectorId !== "edge-tail" ||
  splitResult.position.x !== 50 ||
  splitResult.position.y !== 0 ||
  splitRouter.junctionPreferredOrthogonalDimension("junction") !== 0
) {
  throw new Error(`unexpected connector split result: ${JSON.stringify(splitResult)}`);
}
splitRouter.processTransaction();
const splitHead = splitRouter.routeConnector("edge");
const splitTail = splitRouter.routeConnector("edge-tail");
if (
  splitHead.at(-1).x !== 50 ||
  splitTail[0].x !== 50 ||
  splitTail.at(-1).x !== 100 ||
  splitRouter.splitConnectorAtSegment("edge", 0, "bad-junction", "bad-edge") !== null
) {
  throw new Error(`connector split routes were wrong: ${JSON.stringify({ splitHead, splitTail })}`);
}

endpointRouter.setConnectorFixedRoute("move", [
  [1, 2],
  [9, 9],
  [3, 4],
]);
endpointRouter.setConnectorEndpoints("move", [20, 20], [40, 20]);
if (endpointRouter.routeConnector("move")[1].x !== 9) {
  throw new Error("fixed route did not remain active after endpoint update");
}
endpointRouter.clearConnectorFixedRoute("move");
movedEndpointRoute = endpointRouter.routeConnector("move");
if (movedEndpointRoute[0].x !== 20 || movedEndpointRoute.at(-1).x !== 40) {
  throw new Error(`updated endpoints were not used after clearing fixed route: ${JSON.stringify(movedEndpointRoute)}`);
}

const junctionRouter = new AvoidRouter(0);
junctionRouter.addJunction("j", 10, 20);
let junctionPosition = junctionRouter.junctionPosition("j");
if (junctionPosition.x !== 10 || junctionPosition.y !== 20) {
  throw new Error(`unexpected junction position: ${JSON.stringify(junctionPosition)}`);
}
const defaultJunctionPin = junctionRouter.junctionConnectionPinIds("j", "centre");
if (
  !defaultJunctionPin ||
  defaultJunctionPin.objectId !== "j" ||
  defaultJunctionPin.classId !== ConnectionPin.Centre ||
  junctionRouter.junctionConnectionPinDirections("j", "centre") !== ConnDir.All ||
  junctionRouter.junctionConnectionPinExclusive("j", "centre")
) {
  throw new Error(`default junction centre pin metadata was wrong: ${JSON.stringify(defaultJunctionPin)}`);
}
let recommendedPosition = junctionRouter.junctionRecommendedPosition("j");
if (recommendedPosition.x !== 10 || recommendedPosition.y !== 20) {
  throw new Error(`unexpected initial junction recommended position: ${JSON.stringify(recommendedPosition)}`);
}
if (!junctionRouter.setJunctionRecommendedPosition("j", [30, 40])) {
  throw new Error("junction recommended position update was rejected");
}
recommendedPosition = junctionRouter.junctionRecommendedPosition("j");
if (recommendedPosition.x !== 30 || recommendedPosition.y !== 40) {
  throw new Error(`junction recommended position was not preserved: ${JSON.stringify(recommendedPosition)}`);
}
if (!junctionRouter.setJunctionPositionFixed("j", true) || !junctionRouter.junctionPositionFixed("j")) {
  throw new Error("junction fixed-position flag was not preserved");
}
if (
  junctionRouter.junctionPreferredOrthogonalDimension("j") !== -1 ||
  !junctionRouter.preferJunctionOrthogonalDimension("j", 1) ||
  junctionRouter.junctionPreferredOrthogonalDimension("j") !== 1 ||
  junctionRouter.preferJunctionOrthogonalDimension("j", 2)
) {
  throw new Error("junction preferred orthogonal dimension was not preserved");
}
junctionRouter.addJunction("default-pin-peer", 80, 20);
if (
  !junctionRouter.addConnectorBetweenJunctionPinClasses(
    "default-centre-pin-edge",
    "j",
    ConnectionPin.Centre,
    "default-pin-peer",
    ConnectionPin.Centre,
  )
) {
  throw new Error("default junction centre pin connector was rejected");
}
const defaultCentrePinEnds = junctionRouter.connectorEndpointConnEnds("default-centre-pin-edge");
if (
  defaultCentrePinEnds.source.pinId !== "centre" ||
  defaultCentrePinEnds.source.classId !== ConnectionPin.Centre ||
  defaultCentrePinEnds.destination.pinId !== "centre" ||
  defaultCentrePinEnds.destination.classId !== ConnectionPin.Centre
) {
  throw new Error(`default junction centre pin connector metadata was wrong: ${JSON.stringify(defaultCentrePinEnds)}`);
}
const preferredPinRouter = new AvoidRouter(0);
preferredPinRouter.addJunction("left", 0, 0);
preferredPinRouter.addJunction("right", 100, 0);
preferredPinRouter.addJunctionConnectionPin("left", "east", 7, ConnDir.Right);
preferredPinRouter.addJunctionConnectionPin("left", "north", 7, ConnDir.Up);
preferredPinRouter.addJunctionConnectionPin("right", "target", 8, ConnDir.Left);
if (!preferredPinRouter.addConnectorBetweenJunctionPinClasses("edge", "left", 7, "right", 8)) {
  throw new Error("preferred-dimension junction pin connector was rejected");
}
if (
  preferredPinRouter.connectorEndpointConnEnds("edge").source.pinId !== "east" ||
  !preferredPinRouter.preferJunctionOrthogonalDimension("left", 1) ||
  preferredPinRouter.junctionConnectionPinConnectionCost("left", "east") !== 1 ||
  preferredPinRouter.junctionConnectionPinConnectionCost("left", "north") !== 0 ||
  preferredPinRouter.connectorEndpointConnEnds("edge").source.pinId !== "east" ||
  !preferredPinRouter.processTransaction() ||
  preferredPinRouter.connectorEndpointConnEnds("edge").source.pinId !== "north"
) {
  throw new Error("junction preferred dimension did not penalise matching pin directions");
}
if (!junctionRouter.addConnectorFromJunction("junction-edge", "j", 50, 20)) {
  throw new Error("junction connector was rejected");
}
if (!junctionRouter.addConnectorToJunction("junction-dst-edge", 0, 20, "j")) {
  throw new Error("destination junction connector was rejected");
}
const possibleJunctionDstPins = junctionRouter.connectorPossibleDstPinPoints("junction-dst-edge");
if (
  possibleJunctionDstPins.length !== 1 ||
  possibleJunctionDstPins[0].x !== 10 ||
  possibleJunctionDstPins[0].y !== 20
) {
  throw new Error(`plain junction destination pin points were wrong: ${JSON.stringify(possibleJunctionDstPins)}`);
}
const expectedJunctionAttached = ["default-centre-pin-edge", "junction-edge", "junction-dst-edge"];
const actualJunctionAttached = junctionRouter.junctionAttachedConnectors("j");
if (JSON.stringify(actualJunctionAttached) !== JSON.stringify(expectedJunctionAttached)) {
  throw new Error(`junction attached connectors were wrong: ${JSON.stringify(actualJunctionAttached)}`);
}
let junctionRoute = junctionRouter.routeConnector("junction-edge");
if (junctionRoute[0].x !== 10 || junctionRoute.at(-1).x !== 50) {
  throw new Error(`junction endpoint was not used: ${JSON.stringify(junctionRoute)}`);
}
junctionRouter.moveJunctionBy("j", 5, -10);
junctionRouter.processTransaction();
junctionPosition = junctionRouter.junctionPosition("j");
if (junctionPosition.x !== 15 || junctionPosition.y !== 10) {
  throw new Error(`junction move was not applied: ${JSON.stringify(junctionPosition)}`);
}
recommendedPosition = junctionRouter.junctionRecommendedPosition("j");
if (recommendedPosition.x !== 15 || recommendedPosition.y !== 10) {
  throw new Error(`junction move did not reset recommended position: ${JSON.stringify(recommendedPosition)}`);
}
junctionRoute = junctionRouter.routeConnector("junction-edge");
if (junctionRoute[0].x !== 15 || junctionRoute[0].y !== 10) {
  throw new Error(`connector did not follow moved junction: ${JSON.stringify(junctionRoute)}`);
}

junctionRouter.addJunction("k", 100, 10);
if (!junctionRouter.setConnectorDestJunctionEndpoint("junction-edge", "k")) {
  throw new Error("junction destination endpoint update was rejected");
}
junctionRouter.processTransaction();
junctionRoute = junctionRouter.routeConnector("junction-edge");
if (junctionRoute.at(-1).x !== 100 || junctionRoute.at(-1).y !== 10) {
  throw new Error(`junction destination endpoint was not used: ${JSON.stringify(junctionRoute)}`);
}
let junctionEnds = junctionRouter.connectorEndpointConnEnds("junction-edge");
if (
  junctionEnds.source.type !== "junction" ||
  junctionEnds.source.junctionId !== "j" ||
  junctionEnds.source.position.x !== 15 ||
  junctionEnds.source.position.y !== 10 ||
  junctionEnds.source.directions !== ConnDir.All ||
  junctionEnds.destination.type !== "junction" ||
  junctionEnds.destination.junctionId !== "k" ||
  junctionEnds.destination.position.x !== 100 ||
  junctionEnds.destination.position.y !== 10 ||
  junctionEnds.destination.directions !== ConnDir.All
) {
  throw new Error(`unexpected endpoint conn ends: ${JSON.stringify(junctionEnds)}`);
}

junctionRouter.addJunction("pin-a", 200, 20);
junctionRouter.addJunction("pin-b", 260, 20);
if (
  !junctionRouter.addJunctionConnectionPin("pin-a", "out", 7, ConnDir.Right) ||
  !junctionRouter.addJunctionConnectionPin("pin-b", "in-a", 8, ConnDir.Left) ||
  !junctionRouter.addJunctionConnectionPin("pin-b", "in-b", 8, ConnDir.Left)
) {
  throw new Error("junction connection pin creation failed");
}
const junctionPinPosition = junctionRouter.junctionConnectionPinPosition("pin-a", "out");
if (
  junctionPinPosition.x !== 200 ||
  junctionPinPosition.y !== 20 ||
  junctionRouter.junctionConnectionPinDirections("pin-a", "out") !== ConnDir.Right ||
  JSON.stringify(junctionRouter.junctionConnectionPinIds("pin-a", "out")) !==
    JSON.stringify({ objectId: "pin-a", classId: 7 }) ||
  junctionRouter.junctionConnectionPinIds("pin-a", "missing") !== null
) {
  throw new Error(`junction connection pin metadata was wrong: ${JSON.stringify(junctionPinPosition)}`);
}
if (
  junctionRouter.junctionConnectionPinConnectionCost("pin-b", "in-a") !== 0 ||
  !junctionRouter.junctionConnectionPinExclusive("pin-b", "in-a") ||
  !junctionRouter.setJunctionConnectionPinExclusive("pin-a", "out", false) ||
  junctionRouter.junctionConnectionPinExclusive("pin-a", "out") ||
  !junctionRouter.setJunctionConnectionPinConnectionCost("pin-b", "in-a", 0.5) ||
  junctionRouter.junctionConnectionPinConnectionCost("pin-b", "in-a") !== 0.5 ||
  !junctionRouter.setJunctionConnectionPinConnectionCost("pin-b", "in-b", 1.5) ||
  junctionRouter.setJunctionConnectionPinConnectionCost("pin-b", "in-b", -1) ||
  !Number.isNaN(junctionRouter.junctionConnectionPinConnectionCost("pin-b", "missing"))
) {
  throw new Error("junction connection pin cost/exclusivity metadata was wrong");
}
if (!junctionRouter.addConnectorBetweenJunctionPinClasses("junction-pin-edge", "pin-a", 7, "pin-b", 8)) {
  throw new Error("junction pin class connector was rejected");
}
const junctionPinEnds = junctionRouter.connectorEndpointConnEnds("junction-pin-edge");
if (
  junctionPinEnds.source.type !== "junctionPin" ||
  junctionPinEnds.source.junctionId !== "pin-a" ||
  junctionPinEnds.source.pinId !== "out" ||
  junctionPinEnds.source.position.x !== 200 ||
  junctionPinEnds.source.directions !== ConnDir.Right ||
  junctionPinEnds.destination.type !== "junctionPin" ||
  junctionPinEnds.destination.junctionId !== "pin-b" ||
  junctionPinEnds.destination.pinId !== "in-a" ||
  junctionPinEnds.destination.position.x !== 260 ||
  junctionPinEnds.destination.directions !== ConnDir.Left
) {
  throw new Error(`junction pin endpoint conn ends were wrong: ${JSON.stringify(junctionPinEnds)}`);
}
const junctionPinCandidates = junctionRouter.connectorPossibleDstPinPoints("junction-pin-edge");
if (junctionPinCandidates.length !== 1 || junctionPinCandidates[0].x !== 260 || junctionPinCandidates[0].y !== 20) {
  throw new Error(`junction pin candidates were wrong: ${JSON.stringify(junctionPinCandidates)}`);
}
junctionRouter.processTransaction();
junctionRouter.displayConnectorRoute("junction-pin-edge");
if (junctionRouter.connectorNeedsRepaint("junction-pin-edge")) {
  throw new Error("junction pin route repaint state was not cleared");
}
junctionRouter.addConnector("junction-pin-class-update", 0, 0, 10, 0);
if (
  !junctionRouter.setConnectorSourceJunctionPinClassEndpoint("junction-pin-class-update", "pin-a", 7) ||
  !junctionRouter.setConnectorDestJunctionPinClassEndpoint("junction-pin-class-update", "pin-b", 8)
) {
  throw new Error("junction pin class endpoint update was rejected");
}
junctionRouter.processTransaction();
const junctionPinClassUpdateEnds = junctionRouter.connectorEndpointConnEnds("junction-pin-class-update");
if (
  junctionPinClassUpdateEnds.source.type !== "junctionPin" ||
  junctionPinClassUpdateEnds.source.pinId !== "out" ||
  junctionPinClassUpdateEnds.source.classId !== 7 ||
  junctionPinClassUpdateEnds.destination.type !== "junctionPin" ||
  junctionPinClassUpdateEnds.destination.pinId !== "in-b" ||
  junctionPinClassUpdateEnds.destination.classId !== 8
) {
  throw new Error(`junction pin class endpoint update failed: ${JSON.stringify(junctionPinClassUpdateEnds)}`);
}
if (!junctionRouter.setConnectorDestJunctionPinClassEndpoint("junction-pin-class-update", "pin-b", 99)) {
  throw new Error("missing but valid junction pin class endpoint was rejected");
}
junctionRouter.processTransaction();
const missingJunctionPinClassEnds = junctionRouter.connectorEndpointConnEnds("junction-pin-class-update");
if (
  missingJunctionPinClassEnds.destination.type !== "junctionPin" ||
  missingJunctionPinClassEnds.destination.pinId !== undefined ||
  missingJunctionPinClassEnds.destination.classId !== 99 ||
  missingJunctionPinClassEnds.destination.position.x !== 260 ||
  missingJunctionPinClassEnds.destination.position.y !== 20
) {
  throw new Error(`missing junction pin class did not fall back to junction position: ${JSON.stringify(missingJunctionPinClassEnds)}`);
}
if (!junctionRouter.setJunctionConnectionPinConnectionCost("pin-b", "in-a", 0.75)) {
  throw new Error("junction pin connection cost mutation was rejected");
}
if (junctionRouter.connectorNeedsRepaint("junction-pin-edge")) {
  throw new Error("junction pin mutation should invalidate before repainting");
}
junctionRouter.processTransaction();
if (!junctionRouter.connectorNeedsRepaint("junction-pin-edge")) {
  throw new Error("junction pin mutation did not invalidate attached connector");
}
if (
  !junctionRouter.setJunctionConnectionPinConnectionCost("pin-b", "in-b", 0.25) ||
  !junctionRouter.setJunctionConnectionPinExclusive("pin-b", "in-b", false)
) {
  throw new Error("junction pin cost/exclusivity mutation failed");
}
if (!junctionRouter.addConnectorBetweenJunctionPinClasses("junction-pin-edge-2", "pin-a", 7, "pin-b", 8)) {
  throw new Error("second junction pin class connector was rejected");
}
if (junctionRouter.connectorEndpointConnEnds("junction-pin-edge-2").destination.pinId !== "in-b") {
  throw new Error("junction pin class selection did not prefer lower cost pin");
}
if (!junctionRouter.addConnectorBetweenJunctionPinClasses("junction-pin-edge-3", "pin-a", 7, "pin-b", 8)) {
  throw new Error("third junction pin class connector was rejected");
}
if (junctionRouter.connectorEndpointConnEnds("junction-pin-edge-3").destination.pinId !== "in-b") {
  throw new Error("nonexclusive junction pin was not reusable");
}
if (!junctionRouter.removeJunctionConnectionPin("pin-b", "in-a") || junctionRouter.routeConnector("junction-pin-edge") === null) {
  throw new Error("junction pin removal was not deferred");
}
if (
  !junctionRouter.processTransaction() ||
  junctionRouter.routeConnector("junction-pin-edge") === null ||
  junctionRouter.connectorEndpointConnEnds("junction-pin-edge").destination.pinId !== "in-b"
) {
  throw new Error("junction pin removal did not reassign class endpoint");
}

if (!junctionRouter.removeJunction("k") || junctionRouter.routeConnector("junction-edge") === null) {
  throw new Error("junction removal was not deferred");
}
if (!junctionRouter.processTransaction() || junctionRouter.routeConnector("junction-edge") === null) {
  throw new Error("removing a junction removed a detached connector");
}
junctionEnds = junctionRouter.connectorEndpointConnEnds("junction-edge");
if (
  junctionEnds.destination.type !== "point" ||
  junctionEnds.destination.position.x !== 100 ||
  junctionEnds.destination.position.y !== 10
) {
  throw new Error(`removing a junction did not detach connector endpoint: ${JSON.stringify(junctionEnds)}`);
}

const mergeRouter = new AvoidRouter(0);
mergeRouter.addJunction("j", 50, 0);
if (!mergeRouter.addConnectorToJunction("left", 0, 0, "j")) {
  throw new Error("left junction connector was rejected");
}
if (!mergeRouter.addConnectorFromJunction("right", "j", 100, 0)) {
  throw new Error("right junction connector was rejected");
}
const mergedConnectorId = mergeRouter.removeJunctionAndMergeConnectors("j");
if (mergedConnectorId !== "left") {
  throw new Error(`unexpected merged connector id: ${mergedConnectorId}`);
}
const mergedRoute = mergeRouter.routeConnector("left");
if (
  mergedRoute.length !== 2 ||
  mergedRoute[0].x !== 0 ||
  mergedRoute.at(-1).x !== 100 ||
  mergeRouter.routeConnector("right") !== null ||
  mergeRouter.junctionPosition("j") !== null
) {
  throw new Error(`junction merge failed: ${JSON.stringify(mergedRoute)}`);
}

const junctionObstacleRouter = new AvoidRouter(0);
junctionObstacleRouter.addJunction("mid", 50, 0);
junctionObstacleRouter.addConnector("through", 0, 0, 100, 0);
const junctionObstacleRoute = junctionObstacleRouter.routeConnector("through");
if (junctionObstacleRoute.length <= 2) {
  throw new Error(`unattached junction did not act as an obstacle: ${JSON.stringify(junctionObstacleRoute)}`);
}

const obstacleLifecycleRouter = new AvoidRouter(0);
obstacleLifecycleRouter.addConnector("edge", 0, 60, 120, 60);
obstacleLifecycleRouter.processTransaction();
let lifecycleDisplayRoute = obstacleLifecycleRouter.displayConnectorRoute("edge");
if (lifecycleDisplayRoute.length !== 2 || obstacleLifecycleRouter.connectorNeedsRepaint("edge")) {
  throw new Error(`initial display route state was wrong: ${JSON.stringify(lifecycleDisplayRoute)}`);
}
obstacleLifecycleRouter.addShape("box", 40, 40, 80, 80);
if (obstacleLifecycleRouter.connectorNeedsRepaint("edge")) {
  throw new Error("obstacle add should invalidate before repainting");
}
obstacleLifecycleRouter.processTransaction();
lifecycleDisplayRoute = obstacleLifecycleRouter.displayConnectorRoute("edge");
if (lifecycleDisplayRoute.length <= 2) {
  throw new Error(`shape add did not invalidate route: ${JSON.stringify(lifecycleDisplayRoute)}`);
}
obstacleLifecycleRouter.removeShape("box");
obstacleLifecycleRouter.processTransaction();
lifecycleDisplayRoute = obstacleLifecycleRouter.displayConnectorRoute("edge");
if (lifecycleDisplayRoute.length !== 2) {
  throw new Error(`shape remove did not invalidate route: ${JSON.stringify(lifecycleDisplayRoute)}`);
}
obstacleLifecycleRouter.addJunction("mid", 60, 60);
obstacleLifecycleRouter.processTransaction();
lifecycleDisplayRoute = obstacleLifecycleRouter.displayConnectorRoute("edge");
if (lifecycleDisplayRoute.length <= 2) {
  throw new Error(`junction add did not invalidate route: ${JSON.stringify(lifecycleDisplayRoute)}`);
}

const transactionRouter = new AvoidRouter(0);
if (!transactionRouter.transactionUse()) {
  throw new Error("router should use transactions by default");
}
transactionRouter.addShape("box", 0, 0, 10, 10);
transactionRouter.addJunction("j", 100, 0);
transactionRouter.addConnector("edge", 0, 0, 10, 0);
transactionRouter.moveShapeBy("box", 20, 0);
transactionRouter.setConnectorEndpoints("edge", [20, 0], [30, 0]);
transactionRouter.setConnectorDestJunctionEndpoint("edge", "j");
let transactionBox = transactionRouter.shapeBoundingBox("box");
if (transactionBox.left !== 0) {
  throw new Error(`shape move was not deferred: ${JSON.stringify(transactionBox)}`);
}
let transactionRoute = transactionRouter.routeConnector("edge");
if (transactionRoute[0].x !== 0 || transactionRoute.at(-1).x !== 10) {
  throw new Error(`connector endpoint update was not deferred: ${JSON.stringify(transactionRoute)}`);
}
if (!transactionRouter.processTransaction()) {
  throw new Error("queued transaction was not processed");
}
let updatedConnectorIds = transactionRouter.lastTransactionUpdatedConnectorIds();
if (JSON.stringify(updatedConnectorIds) !== JSON.stringify(["edge"])) {
  throw new Error(`updated connector callback-equivalent list was wrong: ${JSON.stringify(updatedConnectorIds)}`);
}
transactionBox = transactionRouter.shapeBoundingBox("box");
if (transactionBox.left !== 20) {
  throw new Error(`processed shape move was not applied: ${JSON.stringify(transactionBox)}`);
}
transactionRoute = transactionRouter.routeConnector("edge");
if (transactionRoute[0].x !== 20 || transactionRoute.at(-1).x !== 100) {
  throw new Error(`processed connector endpoint update was not applied: ${JSON.stringify(transactionRoute)}`);
}
if (transactionRouter.processTransaction()) {
  throw new Error("empty transaction reported work");
}
updatedConnectorIds = transactionRouter.lastTransactionUpdatedConnectorIds();
if (updatedConnectorIds.length !== 0) {
  throw new Error(`empty transaction did not clear updated connector list: ${JSON.stringify(updatedConnectorIds)}`);
}
transactionRouter.moveShapeBy("box", 5, 0);
transactionRouter.setTransactionUse(false);
transactionBox = transactionRouter.shapeBoundingBox("box");
if (transactionBox.left !== 20) {
  throw new Error(`setTransactionUse(false) unexpectedly processed queue: ${JSON.stringify(transactionBox)}`);
}
if (!transactionRouter.processTransaction()) {
  throw new Error("queued transaction was lost after disabling transactions");
}
transactionBox = transactionRouter.shapeBoundingBox("box");
if (transactionBox.left !== 25) {
  throw new Error(`queued transaction was not applied after explicit processing: ${JSON.stringify(transactionBox)}`);
}
transactionRouter.moveShapeBy("box", 5, 0);
transactionBox = transactionRouter.shapeBoundingBox("box");
if (transactionBox.left !== 30) {
  throw new Error(`immediate transaction mode did not apply move: ${JSON.stringify(transactionBox)}`);
}
transactionRouter.setTransactionUse(true);
if (!transactionRouter.removeShape("box")) {
  throw new Error("queued shape delete was rejected");
}
if (transactionRouter.shapeBoundingBox("box") === null || transactionRouter.routeConnector("edge") === null) {
  throw new Error("queued shape delete was applied before processTransaction");
}
if (!transactionRouter.processTransaction()) {
  throw new Error("queued shape delete was not processed");
}
if (transactionRouter.shapeBoundingBox("box") !== null || transactionRouter.routeConnector("edge") === null) {
  throw new Error("queued shape delete was not applied correctly");
}
transactionRouter.addConnector("delete-edge", 0, 0, 10, 0);
if (!transactionRouter.removeConnector("delete-edge")) {
  throw new Error("queued connector delete was rejected");
}
if (transactionRouter.routeConnector("delete-edge") === null) {
  throw new Error("queued connector delete was applied before processTransaction");
}
if (!transactionRouter.processTransaction() || transactionRouter.routeConnector("delete-edge") !== null) {
  throw new Error("queued connector delete did not remove connector");
}
transactionRouter.setTransactionUse(false);
transactionRouter.addJunction("delete-j", 0, 0);
if (!transactionRouter.removeJunction("delete-j") || transactionRouter.junctionPosition("delete-j") !== null) {
  throw new Error("immediate transaction mode did not apply junction delete");
}
