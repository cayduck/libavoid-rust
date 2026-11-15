# JavaScript API coverage of upstream libavoid

Every public item of upstream libavoid's public headers (Adaptagrams
commit 840ebcf: `libavoid.h` and the headers it includes) and its
JavaScript counterpart in `libavoid-rust` (`const Avoid =
require("libavoid-rust")`), or the specific reason it cannot exist in
JavaScript.  The rules that map C++ idioms to JavaScript (overloads,
out-parameters, containers, operators, `delete`, virtual methods,
callbacks, misuse) are at the top of `index.d.ts`.  Two methods taken
from the libavoid-js fork of Adaptagrams are listed separately, under
"Additions from the libavoid-js fork".

`test/api-conformance.cjs` reads the tables below and checks every row:

* **JavaScript** `Avoid.X` — the export exists; `Avoid.X#m` — `m` is a
  method of `X` (own or inherited); `Avoid.X#m (property)` — `m` is a
  property (data member) of `X` instances; `—` — not exposed (the notes
  give the reason).
* **Args / value** — for a constant its value; for a function or method
  the number of required arguments (upstream's parameters without a
  default), checked against the function's `length` unless the JavaScript
  name implements several overloads (rows marked *overload*; each
  overload is exercised by `test/api-conformance.cjs`).

The headers `graph.h`, `vertices.h`, `visibility.h`, `timer.h`, `debug.h`
and the private parts of `actioninfo.h`, `hyperedgeimprover.h`,
`hyperedgetree.h`, `mtst.h`, `makepath.h`, `orthogonal.h`, `scanline.h`,
`vpsc.h` are included by `libavoid.h` for the library's own use: their
classes are not `AVOID_EXPORT`ed and appear in no public signature except
as opaque pointers.  They are listed by class in the last table.

## geomtypes.h

| Upstream | JavaScript | Args / value | Notes |
|---|---|---|---|
| `XDIM` | `Avoid.XDIM` | 0 | |
| `YDIM` | `Avoid.YDIM` | 1 | |
| `kUnassignedVertexNumber` | `Avoid.kUnassignedVertexNumber` | 8 | |
| `kShapeConnectionPin` | `Avoid.kShapeConnectionPin` | 9 | |
| `class Point` | `Avoid.Point` | class | value type |
| `Point::Point()` | `Avoid.Point` | 0 | overload; (0, 0) (upstream leaves x, y uninitialised) |
| `Point::Point(xv, yv)` | `Avoid.Point` | 2 | overload |
| `Point::operator==` | `Avoid.Point#equal` | 1 | operator rule |
| `Point::equals(rhs, epsilon = 0.0001)` | `Avoid.Point#equals` | 1 | |
| `Point::operator!=` | `Avoid.Point#notEqual` | 1 | operator rule |
| `Point::operator<` | `Avoid.Point#lessThan` | 1 | operator rule |
| `Point::operator[](dimension)` | `Avoid.Point#set` | 2 | non-const overload (assignment through the reference) |
| `Point::operator[](dimension) const` | `Avoid.Point#get` | 1 | |
| `Point::operator+` | `Avoid.Point#add` | 1 | operator rule |
| `Point::operator-` | `Avoid.Point#sub` | 1 | operator rule |
| `Point::x` | `Avoid.Point#x (property)` | | |
| `Point::y` | `Avoid.Point#y (property)` | | |
| `Point::id` | `Avoid.Point#id (property)` | | |
| `Point::vn` | `Avoid.Point#vn (property)` | | |
| `typedef Point Vector` | `Avoid.Vector` | class | the same class as `Avoid.Point` |
| `class Box` | `Avoid.Box` | class | value type; default constructor |
| `Box::min` | `Avoid.Box#min (property)` | | |
| `Box::max` | `Avoid.Box#max (property)` | | |
| `Box::length(dimension)` | `Avoid.Box#length` | 1 | |
| `Box::width()` | `Avoid.Box#width` | 0 | |
| `Box::height()` | `Avoid.Box#height` | 0 | |
| `class PolygonInterface` | `Avoid.PolygonInterface` | class | abstract (constructing it throws) |
| `PolygonInterface::clear()` | `Avoid.PolygonInterface#clear` | 0 | pure virtual |
| `PolygonInterface::empty()` | `Avoid.PolygonInterface#empty` | 0 | pure virtual |
| `PolygonInterface::size()` | `Avoid.PolygonInterface#size` | 0 | pure virtual |
| `PolygonInterface::id()` | `Avoid.PolygonInterface#id` | 0 | pure virtual |
| `PolygonInterface::at(index)` | `Avoid.PolygonInterface#at` | 1 | pure virtual |
| `PolygonInterface::boundingRectPolygon()` | `Avoid.PolygonInterface#boundingRectPolygon` | 0 | |
| `PolygonInterface::offsetBoundingBox(offset)` | `Avoid.PolygonInterface#offsetBoundingBox` | 1 | |
| `PolygonInterface::offsetPolygon(offset)` | `Avoid.PolygonInterface#offsetPolygon` | 1 | |
| `class Edge` | `Avoid.Edge` | class | value type |
| `Edge::a` | `Avoid.Edge#a (property)` | | |
| `Edge::b` | `Avoid.Edge#b (property)` | | |
| `class Polygon` | `Avoid.Polygon` | class | value type |
| `Polygon::Polygon()` | `Avoid.Polygon` | 0 | overload |
| `Polygon::Polygon(n)` | `Avoid.Polygon` | 1 | overload |
| `Polygon::Polygon(const PolygonInterface&)` | `Avoid.Polygon` | 1 | overload; also the implicit conversion where a `Polygon` is expected |
| `Polygon::clear()` | `Avoid.Polygon#clear` | 0 | |
| `Polygon::empty()` | `Avoid.Polygon#empty` | 0 | |
| `Polygon::size()` | `Avoid.Polygon#size` | 0 | |
| `Polygon::id()` | `Avoid.Polygon#id` | 0 | |
| `Polygon::at(index)` | `Avoid.Polygon#at` | 1 | |
| `Polygon::setPoint(index, point)` | `Avoid.Polygon#setPoint` | 2 | |
| `Polygon::simplify()` | `Avoid.Polygon#simplify` | 0 | |
| `Polygon::curvedPolyline(curve_amount, closed = false)` | `Avoid.Polygon#curvedPolyline` | 1 | |
| `Polygon::translate(xDist, yDist)` | `Avoid.Polygon#translate` | 2 | |
| `Polygon::_id` | `Avoid.Polygon#_id (property)` | | |
| `Polygon::ps` | `Avoid.Polygon#ps (property)` | | array of `Point` |
| `Polygon::ts` | `Avoid.Polygon#ts (property)` | | array of one-character strings |
| `Polygon::checkpointsOnRoute` | `Avoid.Polygon#checkpointsOnRoute (property)` | | array of `[index, Point]` |
| `Polygon::checkpointsOnSegment(segmentLowerIndex, indexModifier = 0)` | `Avoid.Polygon#checkpointsOnSegment` | 1 | |
| `typedef Polygon PolyLine` | `Avoid.PolyLine` | class | the same class as `Avoid.Polygon` |
| `class ReferencingPolygon` | `Avoid.ReferencingPolygon` | class | value type |
| `ReferencingPolygon::ReferencingPolygon()` | `Avoid.ReferencingPolygon` | 0 | overload |
| `ReferencingPolygon::ReferencingPolygon(poly, router)` | `Avoid.ReferencingPolygon` | 2 | overload |
| `ReferencingPolygon::clear()` | `Avoid.ReferencingPolygon#clear` | 0 | |
| `ReferencingPolygon::empty()` | `Avoid.ReferencingPolygon#empty` | 0 | |
| `ReferencingPolygon::size()` | `Avoid.ReferencingPolygon#size` | 0 | |
| `ReferencingPolygon::id()` | `Avoid.ReferencingPolygon#id` | 0 | |
| `ReferencingPolygon::at(index)` | `Avoid.ReferencingPolygon#at` | 1 | |
| `ReferencingPolygon::_id` | `Avoid.ReferencingPolygon#_id (property)` | | |
| `ReferencingPolygon::psRef` | `Avoid.ReferencingPolygon#psRef (property)` | | `[Polygon \| null, vn]` pairs; the `const Polygon *` is a copy of the referenced obstacle's polygon |
| `ReferencingPolygon::psPoints` | `Avoid.ReferencingPolygon#psPoints (property)` | | |
| `class Rectangle` | `Avoid.Rectangle` | class | value type, extends `Polygon` |
| `Rectangle::Rectangle(topLeft, bottomRight)` | `Avoid.Rectangle` | 2 | overload |
| `Rectangle::Rectangle(centre, width, height)` | `Avoid.Rectangle` | 3 | overload |

## geometry.h

| Upstream | JavaScript | Args / value | Notes |
|---|---|---|---|
| `euclideanDist(a, b)` | `Avoid.euclideanDist` | 2 | |
| `manhattanDist(a, b)` | `Avoid.manhattanDist` | 2 | |
| `totalLength(poly)` | `Avoid.totalLength` | 1 | |
| `angle(a, b, c)` | `Avoid.angle` | 3 | |
| `segmentIntersect(a, b, c, d)` | `Avoid.segmentIntersect` | 4 | |
| `segmentShapeIntersect(e1, e2, s1, s2, bool& seenIntersectionAtEndpoint)` | `Avoid.segmentShapeIntersect` | 5 | scalar out-parameter: one-element array |
| `inPoly(poly, q, countBorder = true)` | `Avoid.inPoly` | 2 | |
| `inPolyGen(poly, q)` | `Avoid.inPolyGen` | 2 | |
| `inValidRegion(IgnoreRegions, a0, a1, a2, b)` | `Avoid.inValidRegion` | 5 | |
| `cornerSide(c1, c2, c3, p)` | `Avoid.cornerSide` | 4 | |
| `pointOnLine(a, b, c, tolerance = 0.0)` | `Avoid.pointOnLine` | 3 | |
| `colinear(a, b, c, tolerance = 0.0)` | `Avoid.colinear` | 3 | |
| `inBetween(a, b, c)` | `Avoid.inBetween` | 3 | |
| `vecDir(a, b, c, maybeZero = 0.0)` | `Avoid.vecDir` | 3 | static inline upstream |
| `projection(a, b, c)` | `Avoid.projection` | 3 | static inline upstream |
| `DONT_INTERSECT` | `Avoid.DONT_INTERSECT` | 0 | |
| `DO_INTERSECT` | `Avoid.DO_INTERSECT` | 1 | |
| `PARALLEL` | `Avoid.PARALLEL` | 3 | |
| `segmentIntersectPoint(a1, a2, b1, b2, double *x, double *y)` | `Avoid.segmentIntersectPoint` | 6 | scalar out-parameters: one-element arrays |
| `rayIntersectPoint(a1, a2, b1, b2, double *x, double *y)` | `Avoid.rayIntersectPoint` | 6 | scalar out-parameters: one-element arrays |
| `rotationalAngle(p)` | `Avoid.rotationalAngle` | 1 | |

## router.h

| Upstream | JavaScript | Args / value | Notes |
|---|---|---|---|
| `struct LineRep` | `Avoid.LineRep` | class | value type |
| `LineRep::begin` | `Avoid.LineRep#begin (property)` | | |
| `LineRep::end` | `Avoid.LineRep#end (property)` | | |
| `typedef std::list<LineRep> LineReps` | — | | container rule: an array of `LineRep` |
| `typedef std::list<unsigned int> IntList` | — | | container rule: an array of numbers |
| `typedef std::list<ClusterRef *> ClusterRefList` | — | | container rule: an array of `ClusterRef` |
| `typedef std::list<Obstacle *> ObstacleList` | — | | container rule: an array of `Obstacle` |
| `PolyLineRouting` | `Avoid.PolyLineRouting` | 1 | `enum RouterFlag`; also `Avoid.RouterFlag.PolyLineRouting` |
| `OrthogonalRouting` | `Avoid.OrthogonalRouting` | 2 | |
| `runningTo` | `Avoid.runningTo` | 1 | |
| `runningFrom` | `Avoid.runningFrom` | 2 | |
| `runningToAndFrom` | `Avoid.runningToAndFrom` | 3 | |
| `segmentPenalty` | `Avoid.segmentPenalty` | 0 | `enum RoutingParameter`; also `Avoid.RoutingParameter.*` |
| `anglePenalty` | `Avoid.anglePenalty` | 1 | |
| `crossingPenalty` | `Avoid.crossingPenalty` | 2 | |
| `clusterCrossingPenalty` | `Avoid.clusterCrossingPenalty` | 3 | |
| `fixedSharedPathPenalty` | `Avoid.fixedSharedPathPenalty` | 4 | |
| `portDirectionPenalty` | `Avoid.portDirectionPenalty` | 5 | |
| `shapeBufferDistance` | `Avoid.shapeBufferDistance` | 6 | |
| `idealNudgingDistance` | `Avoid.idealNudgingDistance` | 7 | |
| `reverseDirectionPenalty` | `Avoid.reverseDirectionPenalty` | 8 | |
| `lastRoutingParameterMarker` | `Avoid.lastRoutingParameterMarker` | 9 | |
| `typedef enum RoutingParameter PenaltyType` | `Avoid.PenaltyType` | object | the same object as `Avoid.RoutingParameter` |
| `nudgeOrthogonalSegmentsConnectedToShapes` | `Avoid.nudgeOrthogonalSegmentsConnectedToShapes` | 0 | `enum RoutingOption`; also `Avoid.RoutingOption.*` |
| `improveHyperedgeRoutesMovingJunctions` | `Avoid.improveHyperedgeRoutesMovingJunctions` | 1 | |
| `penaliseOrthogonalSharedPathsAtConnEnds` | `Avoid.penaliseOrthogonalSharedPathsAtConnEnds` | 2 | |
| `nudgeOrthogonalTouchingColinearSegments` | `Avoid.nudgeOrthogonalTouchingColinearSegments` | 3 | |
| `performUnifyingNudgingPreprocessingStep` | `Avoid.performUnifyingNudgingPreprocessingStep` | 4 | |
| `improveHyperedgeRoutesMovingAddingAndDeletingJunctions` | `Avoid.improveHyperedgeRoutesMovingAddingAndDeletingJunctions` | 5 | |
| `nudgeSharedPathsWithCommonEndPoint` | `Avoid.nudgeSharedPathsWithCommonEndPoint` | 6 | |
| `lastRoutingOptionMarker` | `Avoid.lastRoutingOptionMarker` | 7 | |
| `TransactionPhaseOrthogonalVisibilityGraphScanX` | `Avoid.TransactionPhaseOrthogonalVisibilityGraphScanX` | 1 | `enum TransactionPhases`; also `Avoid.TransactionPhases.*` |
| `TransactionPhaseOrthogonalVisibilityGraphScanY` | `Avoid.TransactionPhaseOrthogonalVisibilityGraphScanY` | 2 | |
| `TransactionPhaseRouteSearch` | `Avoid.TransactionPhaseRouteSearch` | 3 | |
| `TransactionPhaseCrossingDetection` | `Avoid.TransactionPhaseCrossingDetection` | 4 | |
| `TransactionPhaseRerouteSearch` | `Avoid.TransactionPhaseRerouteSearch` | 5 | |
| `TransactionPhaseOrthogonalNudgingX` | `Avoid.TransactionPhaseOrthogonalNudgingX` | 6 | |
| `TransactionPhaseOrthogonalNudgingY` | `Avoid.TransactionPhaseOrthogonalNudgingY` | 7 | |
| `TransactionPhaseCompleted` | `Avoid.TransactionPhaseCompleted` | 8 | |
| `class ConnRerouteFlagDelegate` | — | | not `AVOID_EXPORT`ed; the router's private bookkeeping of `bool *` reroute flags inside connectors (`addConn` returns a `bool *`); only the Router itself may use it (`m_conn_reroute_flags` is private) |
| `ConnRerouteFlagDelegate::addConn(conn)` | — | | see `ConnRerouteFlagDelegate` |
| `ConnRerouteFlagDelegate::removeConn(conn)` | — | | see `ConnRerouteFlagDelegate` |
| `ConnRerouteFlagDelegate::alertConns()` | — | | see `ConnRerouteFlagDelegate` |
| `zeroParamValue` | `Avoid.zeroParamValue` | 0 | |
| `chooseSensibleParamValue` | `Avoid.chooseSensibleParamValue` | -1 | |
| `class TopologyAddonInterface` | — | | extension point for libtopology (C++): `improveOrthogonalTopology(Router *)` runs inside a transaction and must use the router re-entrantly, which a JavaScript object cannot do while the wasm call holds the router; its other members write C++ code to a `FILE *` |
| `TopologyAddonInterface::clone()` | — | | see `TopologyAddonInterface` |
| `TopologyAddonInterface::improveOrthogonalTopology(router)` | — | | see `TopologyAddonInterface` |
| `TopologyAddonInterface::outputCode(FILE *)` | — | | see `TopologyAddonInterface` |
| `TopologyAddonInterface::outputDeletionCode(FILE *)` | — | | see `TopologyAddonInterface` |
| `class Router` | `Avoid.Router` | class | subclass to override virtual methods |
| `Router::Router(flags)` | `Avoid.Router` | 1 | |
| `Router::~Router()` | `Avoid.Router#delete` | 0 | `delete` rule |
| `Router::m_obstacles` | `Avoid.Router#m_obstacles (property)` | | read-only array of `ShapeRef`/`JunctionRef` |
| `Router::connRefs` | `Avoid.Router#connRefs (property)` | | read-only array |
| `Router::clusterRefs` | `Avoid.Router#clusterRefs (property)` | | read-only array |
| `Router::visGraph` | — | | `EdgeList` of internal `EdgeInf` objects (visibility graph); not `AVOID_EXPORT`ed and only meaningful to the router's own code |
| `Router::invisGraph` | — | | see `Router::visGraph` |
| `Router::visOrthogGraph` | — | | see `Router::visGraph` |
| `Router::contains` | — | | `ContainsMap` keyed by internal `VertID`s (vertex-in-shape bookkeeping) |
| `Router::vertices` | — | | `VertInfList` of internal `VertInf` objects |
| `Router::enclosingClusters` | — | | see `Router::contains` |
| `Router::PartialTime` | `Avoid.Router#PartialTime (property)` | | |
| `Router::SimpleRouting` | `Avoid.Router#SimpleRouting (property)` | | |
| `Router::ClusteredRouting` | `Avoid.Router#ClusteredRouting (property)` | | |
| `Router::IgnoreRegions` | `Avoid.Router#IgnoreRegions (property)` | | |
| `Router::UseLeesAlgorithm` | `Avoid.Router#UseLeesAlgorithm (property)` | | |
| `Router::InvisibilityGrph` | `Avoid.Router#InvisibilityGrph (property)` | | |
| `Router::SelectiveReroute` | `Avoid.Router#SelectiveReroute (property)` | | |
| `Router::PartialFeedback` | `Avoid.Router#PartialFeedback (property)` | | |
| `Router::RubberBandRouting` | `Avoid.Router#RubberBandRouting (property)` | | |
| `Router::timers` | — | | exists only when compiled with `AVOID_PROFILE` (not defined by upstream's build or this port) |
| `Router::st_checked_edges` | `Avoid.Router#st_checked_edges (property)` | | |
| `Router::setTransactionUse(transactions)` | `Avoid.Router#setTransactionUse` | 1 | |
| `Router::transactionUse()` | `Avoid.Router#transactionUse` | 0 | |
| `Router::processTransaction()` | `Avoid.Router#processTransaction` | 0 | |
| `Router::deleteShape(shape)` | `Avoid.Router#deleteShape` | 1 | |
| `Router::moveShape(shape, newPoly, first_move = false)` | `Avoid.Router#moveShape` | 2 | overload |
| `Router::moveShape(shape, xDiff, yDiff)` | `Avoid.Router#moveShape` | 3 | overload |
| `Router::deleteJunction(junction)` | `Avoid.Router#deleteJunction` | 1 | |
| `Router::deleteConnector(connector)` | `Avoid.Router#deleteConnector` | 1 | |
| `Router::moveJunction(junction, newPosition)` | `Avoid.Router#moveJunction` | 2 | overload |
| `Router::moveJunction(junction, xDiff, yDiff)` | `Avoid.Router#moveJunction` | 3 | overload |
| `Router::setRoutingParameter(parameter, value = chooseSensibleParamValue)` | `Avoid.Router#setRoutingParameter` | 1 | |
| `Router::routingParameter(parameter)` | `Avoid.Router#routingParameter` | 1 | |
| `Router::setRoutingOption(option, value)` | `Avoid.Router#setRoutingOption` | 2 | |
| `Router::routingOption(option)` | `Avoid.Router#routingOption` | 1 | |
| `Router::setRoutingPenalty(penType, penVal = chooseSensibleParamValue)` | `Avoid.Router#setRoutingPenalty` | 1 | |
| `Router::hyperedgeRerouter()` | `Avoid.Router#hyperedgeRerouter` | 0 | the same object every time |
| `Router::outputInstanceToSVG(filename = std::string())` | `Avoid.Router#outputInstanceToSVG` | 0 | returns the SVG upstream writes to `<filename>.svg` |
| `virtual Router::newObjectId()` | `Avoid.Router#newObjectId` | 0 | virtual: an override supplies the ids of objects constructed through the API with id 0; objects the router creates itself get the built-in id |
| `Router::objectIdIsUnused(id)` | `Avoid.Router#objectIdIsUnused` | 1 | |
| `virtual Router::shouldContinueTransactionWithProgress(elapsedTime, phaseNumber, totalPhases, proportion)` | `Avoid.Router#shouldContinueTransactionWithProgress` | 4 | virtual: override by subclassing |
| `Router::newAndDeletedObjectListsFromHyperedgeImprovement()` | `Avoid.Router#newAndDeletedObjectListsFromHyperedgeImprovement` | 0 | |
| `Router::setDebugHandler(DebugHandler *)` | — | | the debug-visualisation hooks (`DebugHandler`) are not ported by the core; its callbacks receive internal `VertInf *` |
| `Router::debugHandler()` | — | | see `Router::setDebugHandler` |
| `Router::processActions()` | `Avoid.Router#processActions` | 0 | |
| `Router::deleteCluster(cluster)` | `Avoid.Router#deleteCluster` | 1 | |
| `Router::attachedShapes(IntList& shapes, shapeId, type)` | `Avoid.Router#attachedShapes` | 3 | container out-parameter: an array that is appended to |
| `Router::attachedConns(IntList& conns, shapeId, type)` | `Avoid.Router#attachedConns` | 3 | container out-parameter |
| `Router::markPolylineConnectorsNeedingReroutingForDeletedObstacle(obstacle)` | `Avoid.Router#markPolylineConnectorsNeedingReroutingForDeletedObstacle` | 1 | |
| `Router::generateContains(VertInf *pt)` | — | | takes an internal `VertInf *` (visibility-graph vertex) |
| `Router::printInfo()` | `Avoid.Router#printInfo` | 0 | returns the text upstream prints |
| `Router::regenerateStaticBuiltGraph()` | `Avoid.Router#regenerateStaticBuiltGraph` | 0 | |
| `Router::destroyOrthogonalVisGraph()` | `Avoid.Router#destroyOrthogonalVisGraph` | 0 | |
| `Router::setStaticGraphInvalidated(invalidated)` | `Avoid.Router#setStaticGraphInvalidated` | 1 | |
| `Router::validConnType(select = ConnType_None)` | `Avoid.Router#validConnType` | 0 | |
| `Router::isInCrossingPenaltyReroutingStage()` | `Avoid.Router#isInCrossingPenaltyReroutingStage` | 0 | |
| `Router::markAllObstaclesAsMoved()` | `Avoid.Router#markAllObstaclesAsMoved` | 0 | |
| `Router::shapeContainingPoint(point)` | `Avoid.Router#shapeContainingPoint` | 1 | |
| `Router::performContinuationCheck(phaseNumber, stepNumber, totalSteps)` | `Avoid.Router#performContinuationCheck` | 3 | |
| `Router::registerSettingsChange()` | `Avoid.Router#registerSettingsChange` | 0 | |
| `Router::setTopologyAddon(TopologyAddonInterface *)` | — | | see `TopologyAddonInterface` |
| `Router::improveOrthogonalTopology()` | `Avoid.Router#improveOrthogonalTopology` | 0 | runs the default (no-op) addon |
| `Router::existsOrthogonalSegmentOverlap(atEnds = false)` | `Avoid.Router#existsOrthogonalSegmentOverlap` | 0 | |
| `Router::existsOrthogonalFixedSegmentOverlap(atEnds = false)` | `Avoid.Router#existsOrthogonalFixedSegmentOverlap` | 0 | |
| `Router::existsOrthogonalTouchingPaths()` | `Avoid.Router#existsOrthogonalTouchingPaths` | 0 | |
| `Router::existsCrossings(optimisedForConnectorType = false)` | `Avoid.Router#existsCrossings` | 0 | |
| `Router::existsInvalidOrthogonalPaths()` | `Avoid.Router#existsInvalidOrthogonalPaths` | 0 | |
| `Router::outputDiagramSVG(instanceName = std::string(), LineReps *lineReps = nullptr)` | `Avoid.Router#outputDiagramSVG` | 0 | returns the SVG upstream writes to `<instanceName>.svg` |
| `Router::outputDiagramText(instanceName = std::string())` | `Avoid.Router#outputDiagramText` | 0 | returns the text upstream writes to `<instanceName>.txt` |
| `Router::outputDiagram(instanceName = std::string())` | `Avoid.Router#outputDiagram` | 0 | returns the text diagram; upstream adds `outputInstanceToSVG()` only in builds with the debug define `SVG_OUTPUT` |

## obstacle.h, shape.h, junction.h

| Upstream | JavaScript | Args / value | Notes |
|---|---|---|---|
| `typedef std::list<ConnRef *> ConnRefList` | — | | container rule: an array of `ConnRef` |
| `class Obstacle` | `Avoid.Obstacle` | class | abstract base of `ShapeRef` / `JunctionRef` (constructing it throws) |
| `Obstacle::Obstacle(router, poly, id = 0)` | — | | abstract (pure virtual `position()`); construct a `ShapeRef` or `JunctionRef` |
| `Obstacle::~Obstacle()` | — | | owned by the router: `Router#deleteShape` / `deleteJunction` (upstream aborts on a direct `delete`) |
| `Obstacle::id()` | `Avoid.Obstacle#id` | 0 | |
| `Obstacle::polygon()` | `Avoid.Obstacle#polygon` | 0 | a copy |
| `Obstacle::router()` | `Avoid.Obstacle#router` | 0 | |
| `Obstacle::position()` | `Avoid.Obstacle#position` | 0 | pure virtual upstream |
| `Obstacle::setNewPoly(poly)` | `Avoid.Obstacle#setNewPoly` | 1 | |
| `Obstacle::firstVert()` | — | | returns an internal `VertInf *` |
| `Obstacle::lastVert()` | — | | returns an internal `VertInf *` |
| `Obstacle::routingBox()` | `Avoid.Obstacle#routingBox` | 0 | |
| `Obstacle::routingPolygon()` | `Avoid.Obstacle#routingPolygon` | 0 | |
| `Obstacle::attachedConnectors()` | `Avoid.Obstacle#attachedConnectors` | 0 | |
| `typedef std::list<ShapeRef *> ShapeRefList` | — | | container rule: an array of `ShapeRef` |
| `TransformationType_CW90` | `Avoid.TransformationType_CW90` | 0 | `enum ShapeTransformationType`; also `Avoid.ShapeTransformationType.*` |
| `TransformationType_CW180` | `Avoid.TransformationType_CW180` | 1 | |
| `TransformationType_CW270` | `Avoid.TransformationType_CW270` | 2 | |
| `TransformationType_FlipX` | `Avoid.TransformationType_FlipX` | 3 | |
| `TransformationType_FlipY` | `Avoid.TransformationType_FlipY` | 4 | |
| `class ShapeRef` | `Avoid.ShapeRef` | class | extends `Obstacle` |
| `ShapeRef::ShapeRef(router, poly, id = 0)` | `Avoid.ShapeRef` | 2 | |
| `ShapeRef::~ShapeRef()` | `Avoid.Router#deleteShape` | 1 | upstream aborts on a direct `delete`; the router deletes it |
| `ShapeRef::polygon()` | `Avoid.ShapeRef#polygon` | 0 | a copy |
| `ShapeRef::transformConnectionPinPositions(transform)` | `Avoid.ShapeRef#transformConnectionPinPositions` | 1 | |
| `ShapeRef::position()` | `Avoid.ShapeRef#position` | 0 | |
| `typedef std::list<JunctionRef *> JunctionRefList` | — | | container rule: an array of `JunctionRef` |
| `class JunctionRef` | `Avoid.JunctionRef` | class | extends `Obstacle` |
| `JunctionRef::JunctionRef(router, position, id = 0)` | `Avoid.JunctionRef` | 2 | |
| `JunctionRef::~JunctionRef()` | `Avoid.Router#deleteJunction` | 1 | upstream aborts on a direct `delete`; the router deletes it |
| `JunctionRef::removeJunctionAndMergeConnectors()` | `Avoid.JunctionRef#removeJunctionAndMergeConnectors` | 0 | |
| `JunctionRef::position()` | `Avoid.JunctionRef#position` | 0 | |
| `JunctionRef::setPositionFixed(fixed)` | `Avoid.JunctionRef#setPositionFixed` | 1 | |
| `JunctionRef::positionFixed()` | `Avoid.JunctionRef#positionFixed` | 0 | |
| `JunctionRef::recommendedPosition()` | `Avoid.JunctionRef#recommendedPosition` | 0 | |
| `JunctionRef::makeRectangle(router, position)` | `Avoid.JunctionRef#makeRectangle` | 2 | |
| `JunctionRef::preferOrthogonalDimension(dim)` | `Avoid.JunctionRef#preferOrthogonalDimension` | 1 | |

## connend.h, connector.h

| Upstream | JavaScript | Args / value | Notes |
|---|---|---|---|
| `ConnDirNone` | `Avoid.ConnDirNone` | 0 | `enum ConnDirFlag`; also `Avoid.ConnDirFlag.*` |
| `ConnDirUp` | `Avoid.ConnDirUp` | 1 | |
| `ConnDirDown` | `Avoid.ConnDirDown` | 2 | |
| `ConnDirLeft` | `Avoid.ConnDirLeft` | 4 | |
| `ConnDirRight` | `Avoid.ConnDirRight` | 8 | |
| `ConnDirAll` | `Avoid.ConnDirAll` | 15 | |
| `typedef unsigned int ConnDirFlags` | — | | a number (combination of the `ConnDir*` bits) |
| `ConnEndPoint` | `Avoid.ConnEndPoint` | 0 | `enum ConnEndType`; also `Avoid.ConnEndType.*` |
| `ConnEndShapePin` | `Avoid.ConnEndShapePin` | 1 | |
| `ConnEndJunction` | `Avoid.ConnEndJunction` | 2 | |
| `ConnEndEmpty` | `Avoid.ConnEndEmpty` | 3 | |
| `class ConnEnd` | `Avoid.ConnEnd` | class | value type |
| `ConnEnd::ConnEnd(point)` | `Avoid.ConnEnd` | 1 | overload |
| `ConnEnd::ConnEnd(point, visDirs)` | `Avoid.ConnEnd` | 2 | overload |
| `ConnEnd::ConnEnd(shapeRef, connectionPinClassID)` | `Avoid.ConnEnd` | 2 | overload |
| `ConnEnd::ConnEnd(junctionRef)` | `Avoid.ConnEnd` | 1 | overload |
| `ConnEnd::ConnEnd()` | `Avoid.ConnEnd` | 0 | overload |
| `ConnEnd::~ConnEnd()` | `Avoid.ConnEnd#delete` | 0 | `delete` rule (nothing to free) |
| `ConnEnd::type()` | `Avoid.ConnEnd#type` | 0 | |
| `ConnEnd::position()` | `Avoid.ConnEnd#position` | 0 | |
| `ConnEnd::directions()` | `Avoid.ConnEnd#directions` | 0 | |
| `ConnEnd::shape()` | `Avoid.ConnEnd#shape` | 0 | |
| `ConnEnd::junction()` | `Avoid.ConnEnd#junction` | 0 | |
| `ConnEnd::pinClassId()` | `Avoid.ConnEnd#pinClassId` | 0 | |
| `ConnType_None` | `Avoid.ConnType_None` | 0 | `enum ConnType`; also `Avoid.ConnType.*` |
| `ConnType_PolyLine` | `Avoid.ConnType_PolyLine` | 1 | |
| `ConnType_Orthogonal` | `Avoid.ConnType_Orthogonal` | 2 | |
| `class Checkpoint` | `Avoid.Checkpoint` | class | value type |
| `Checkpoint::Checkpoint(p)` | `Avoid.Checkpoint` | 1 | overload |
| `Checkpoint::Checkpoint(p, ad, dd)` | `Avoid.Checkpoint` | 3 | overload |
| `Checkpoint::Checkpoint()` | `Avoid.Checkpoint` | 0 | overload |
| `Checkpoint::point` | `Avoid.Checkpoint#point (property)` | | |
| `Checkpoint::arrivalDirections` | `Avoid.Checkpoint#arrivalDirections (property)` | | |
| `Checkpoint::departureDirections` | `Avoid.Checkpoint#departureDirections (property)` | | |
| `class ConnRef` | `Avoid.ConnRef` | class | |
| `ConnRef::ConnRef(router, id = 0)` | `Avoid.ConnRef` | 1 | overload |
| `ConnRef::ConnRef(router, src, dst, id = 0)` | `Avoid.ConnRef` | 3 | overload |
| `ConnRef::~ConnRef()` | `Avoid.Router#deleteConnector` | 1 | upstream aborts on a direct `delete`; the router deletes it |
| `ConnRef::setEndpoints(srcPoint, dstPoint)` | `Avoid.ConnRef#setEndpoints` | 2 | |
| `ConnRef::setSourceEndpoint(srcPoint)` | `Avoid.ConnRef#setSourceEndpoint` | 1 | |
| `ConnRef::setDestEndpoint(dstPoint)` | `Avoid.ConnRef#setDestEndpoint` | 1 | |
| `ConnRef::id()` | `Avoid.ConnRef#id` | 0 | |
| `ConnRef::router()` | `Avoid.ConnRef#router` | 0 | |
| `ConnRef::needsRepaint()` | `Avoid.ConnRef#needsRepaint` | 0 | |
| `ConnRef::route()` | `Avoid.ConnRef#route` | 0 | a copy |
| `ConnRef::displayRoute()` | `Avoid.ConnRef#displayRoute` | 0 | a copy |
| `ConnRef::setCallback(void (*cb)(void *), void *ptr)` | `Avoid.ConnRef#setCallback` | 2 | callback rule |
| `ConnRef::routingType()` | `Avoid.ConnRef#routingType` | 0 | |
| `ConnRef::setRoutingType(type)` | `Avoid.ConnRef#setRoutingType` | 1 | |
| `ConnRef::splitAtSegment(segmentN)` | `Avoid.ConnRef#splitAtSegment` | 1 | `std::pair`: a two-element array |
| `ConnRef::setRoutingCheckpoints(checkpoints)` | `Avoid.ConnRef#setRoutingCheckpoints` | 1 | |
| `ConnRef::routingCheckpoints()` | `Avoid.ConnRef#routingCheckpoints` | 0 | |
| `ConnRef::endpointConnEnds()` | `Avoid.ConnRef#endpointConnEnds` | 0 | `std::pair`: a two-element array |
| `ConnRef::src()` | — | | returns an internal `VertInf *` |
| `ConnRef::dst()` | — | | returns an internal `VertInf *` |
| `ConnRef::setFixedRoute(route)` | `Avoid.ConnRef#setFixedRoute` | 1 | |
| `ConnRef::setFixedExistingRoute()` | `Avoid.ConnRef#setFixedExistingRoute` | 0 | |
| `ConnRef::hasFixedRoute()` | `Avoid.ConnRef#hasFixedRoute` | 0 | |
| `ConnRef::clearFixedRoute()` | `Avoid.ConnRef#clearFixedRoute` | 0 | |
| `ConnRef::set_route(route)` | `Avoid.ConnRef#set_route` | 1 | |
| `ConnRef::calcRouteDist()` | `Avoid.ConnRef#calcRouteDist` | 0 | |
| `ConnRef::makeActive()` | `Avoid.ConnRef#makeActive` | 0 | |
| `ConnRef::makeInactive()` | `Avoid.ConnRef#makeInactive` | 0 | |
| `ConnRef::start()` | — | | returns an internal `VertInf *` |
| `ConnRef::removeFromGraph()` | `Avoid.ConnRef#removeFromGraph` | 0 | |
| `ConnRef::isInitialised()` | `Avoid.ConnRef#isInitialised` | 0 | |
| `ConnRef::makePathInvalid()` | `Avoid.ConnRef#makePathInvalid` | 0 | |
| `ConnRef::setHateCrossings(value)` | `Avoid.ConnRef#setHateCrossings` | 1 | |
| `ConnRef::doesHateCrossings()` | `Avoid.ConnRef#doesHateCrossings` | 0 | |
| `ConnRef::setEndpoint(type, connEnd)` | `Avoid.ConnRef#setEndpoint` | 2 | `type`: 1 (`VertID::src`) or 2 (`VertID::tar`) |
| `ConnRef::setEndpoint(type, const VertID& pointID, Point *pointSuggestion = nullptr)` | — | | takes an internal `VertID` (visibility-graph vertex id) |
| `ConnRef::possibleDstPinPoints()` | `Avoid.ConnRef#possibleDstPinPoints` | 0 | |
| `typedef std::pair<Point *, ConnRef *> PtConnPtrPair` | — | | internal (crossing detection); see `PtOrder` |
| `typedef std::vector<PtConnPtrPair> PointRepVector` | — | | internal; see `PtOrder` |
| `typedef std::list<std::pair<size_t, size_t>> NodeIndexPairLinkList` | — | | internal; see `PtOrder` |
| `class PtOrder` | — | | not `AVOID_EXPORT`ed; the nudging code's ordering of shared-path points, built from pointers into the router's routes |
| `typedef std::map<Point, PtOrder> PtOrderMap` | — | | internal; see `PtOrder` |
| `typedef std::set<Point> PointSet` | — | | internal; see `ConnectorCrossings` |
| `CROSSING_NONE` | `Avoid.CROSSING_NONE` | 0 | |
| `CROSSING_TOUCHES` | `Avoid.CROSSING_TOUCHES` | 1 | |
| `CROSSING_SHARES_PATH` | `Avoid.CROSSING_SHARES_PATH` | 2 | |
| `CROSSING_SHARES_PATH_AT_END` | `Avoid.CROSSING_SHARES_PATH_AT_END` | 4 | |
| `CROSSING_SHARES_FIXED_SEGMENT` | `Avoid.CROSSING_SHARES_FIXED_SEGMENT` | 8 | |
| `typedef std::pair<int, unsigned int> CrossingsInfoPair` | — | | internal; see `ConnectorCrossings` |
| `typedef std::vector<Point> PointList` | — | | container rule: an array of `Point` |
| `typedef std::vector<PointList> SharedPathList` | — | | internal; see `ConnectorCrossings` |
| `class ConnectorCrossings` | — | | not `AVOID_EXPORT`ed; the router's crossing counter: holds non-owning references (`Polygon&`) into routes and output pointers (`PointSet *`, `PtOrderMap *`, `SharedPathList *`) to internal structures; use `Router#existsCrossings` |
| `splitBranchingSegments(poly, polyIsConn, conn, tolerance = 0)` | — | | internal helper of `ConnectorCrossings` (modifies routes in place) |
| `validateBendPoint(VertInf *, VertInf *, VertInf *)` | — | | takes internal `VertInf *` |

## connectionpin.h

| Upstream | JavaScript | Args / value | Notes |
|---|---|---|---|
| `CONNECTIONPIN_UNSET` | `Avoid.CONNECTIONPIN_UNSET` | 2147483647 | |
| `CONNECTIONPIN_CENTRE` | `Avoid.CONNECTIONPIN_CENTRE` | 2147483646 | |
| `typedef std::pair<unsigned int, unsigned int> ConnectionPinIds` | — | | `std::pair`: a two-element array (`ShapeConnectionPin#ids`) |
| `ATTACH_POS_TOP` | `Avoid.ATTACH_POS_TOP` | 0 | |
| `ATTACH_POS_CENTRE` | `Avoid.ATTACH_POS_CENTRE` | 0.5 | |
| `ATTACH_POS_BOTTOM` | `Avoid.ATTACH_POS_BOTTOM` | 1 | |
| `ATTACH_POS_LEFT` | `Avoid.ATTACH_POS_LEFT` | 0 | |
| `ATTACH_POS_RIGHT` | `Avoid.ATTACH_POS_RIGHT` | 1 | |
| `ATTACH_POS_MIN_OFFSET` | `Avoid.ATTACH_POS_MIN_OFFSET` | 0 | |
| `ATTACH_POS_MAX_OFFSET` | `Avoid.ATTACH_POS_MAX_OFFSET` | -1 | |
| `class ShapeConnectionPin` | `Avoid.ShapeConnectionPin` | class | |
| `ShapeConnectionPin::ShapeConnectionPin(shape, classId, xOffset, yOffset, proportional, insideOffset, visDirs)` | `Avoid.ShapeConnectionPin` | 7 | overload |
| `ShapeConnectionPin::ShapeConnectionPin(shape, classId, xOffset, yOffset, insideOffset, visDirs)` | `Avoid.ShapeConnectionPin` | 6 | overload |
| `ShapeConnectionPin::ShapeConnectionPin(junction, classId, visDirs = ConnDirNone)` | `Avoid.ShapeConnectionPin` | 2 | overload |
| `ShapeConnectionPin::~ShapeConnectionPin()` | `Avoid.ShapeConnectionPin#delete` | 0 | `delete` rule: removes the pin |
| `ShapeConnectionPin::setConnectionCost(cost)` | `Avoid.ShapeConnectionPin#setConnectionCost` | 1 | |
| `ShapeConnectionPin::position(newPoly = Polygon())` | `Avoid.ShapeConnectionPin#position` | 0 | |
| `ShapeConnectionPin::directions()` | `Avoid.ShapeConnectionPin#directions` | 0 | |
| `ShapeConnectionPin::setExclusive(exclusive)` | `Avoid.ShapeConnectionPin#setExclusive` | 1 | |
| `ShapeConnectionPin::isExclusive()` | `Avoid.ShapeConnectionPin#isExclusive` | 0 | |
| `ShapeConnectionPin::ids()` | `Avoid.ShapeConnectionPin#ids` | 0 | |
| `ShapeConnectionPin::operator==` | `Avoid.ShapeConnectionPin#equal` | 1 | operator rule |
| `ShapeConnectionPin::operator<` | `Avoid.ShapeConnectionPin#lessThan` | 1 | operator rule |
| `class CmpConnPinPtr` | — | | comparison functor for `ShapeConnectionPinSet`; in JavaScript `(a, b) => a.lessThan(b)` |
| `typedef std::set<ShapeConnectionPin *, CmpConnPinPtr> ShapeConnectionPinSet` | — | | internal (an obstacle's private pin set) |

## viscluster.h

| Upstream | JavaScript | Args / value | Notes |
|---|---|---|---|
| `class ClusterRef` | `Avoid.ClusterRef` | class | |
| `ClusterRef::ClusterRef(router, poly, id = 0)` | `Avoid.ClusterRef` | 2 | |
| `ClusterRef::~ClusterRef()` | `Avoid.Router#deleteCluster` | 1 | upstream aborts on a direct `delete`; the router deletes it |
| `ClusterRef::setNewPoly(poly)` | `Avoid.ClusterRef#setNewPoly` | 1 | |
| `ClusterRef::id()` | `Avoid.ClusterRef#id` | 0 | |
| `ClusterRef::polygon()` | `Avoid.ClusterRef#polygon` | 0 | a copy |
| `ClusterRef::rectangularPolygon()` | `Avoid.ClusterRef#rectangularPolygon` | 0 | a copy |
| `ClusterRef::router()` | `Avoid.ClusterRef#router` | 0 | |
| `ClusterRef::makeActive()` | `Avoid.ClusterRef#makeActive` | 0 | |
| `ClusterRef::makeInactive()` | `Avoid.ClusterRef#makeInactive` | 0 | |

## hyperedge.h

| Upstream | JavaScript | Args / value | Notes |
|---|---|---|---|
| `typedef std::list<ConnEnd> ConnEndList` | — | | container rule: an array of `ConnEnd` |
| `typedef std::set<ConnRef *> ConnRefSet` | — | | internal (private members only) |
| `typedef std::set<VertInf *> VertexSet`, `VertexList`, `VertexSetVector` | — | | internal: sets of `VertInf *` |
| `typedef std::vector<...> JunctionRefVector`, `ConnEndListVector`, `ConnRefListVector`, `JunctionRefListVector` | — | | internal (private members only) |
| `struct HyperedgeNewAndDeletedObjectLists` | `Avoid.HyperedgeNewAndDeletedObjectLists` | class | value type |
| `HyperedgeNewAndDeletedObjectLists::newJunctionList` | `Avoid.HyperedgeNewAndDeletedObjectLists#newJunctionList (property)` | | |
| `HyperedgeNewAndDeletedObjectLists::newConnectorList` | `Avoid.HyperedgeNewAndDeletedObjectLists#newConnectorList (property)` | | |
| `HyperedgeNewAndDeletedObjectLists::deletedJunctionList` | `Avoid.HyperedgeNewAndDeletedObjectLists#deletedJunctionList (property)` | | wrappers of deleted junctions (invalid) |
| `HyperedgeNewAndDeletedObjectLists::deletedConnectorList` | `Avoid.HyperedgeNewAndDeletedObjectLists#deletedConnectorList (property)` | | wrappers of deleted connectors (invalid) |
| `HyperedgeNewAndDeletedObjectLists::changedConnectorList` | `Avoid.HyperedgeNewAndDeletedObjectLists#changedConnectorList (property)` | | |
| `class HyperedgeRerouter` | `Avoid.HyperedgeRerouter` | class | |
| `HyperedgeRerouter::HyperedgeRerouter()` | `Avoid.HyperedgeRerouter` | 0 | belongs to no router (upstream's `setRouter` is private): records registrations only; use `Router#hyperedgeRerouter` |
| `HyperedgeRerouter::registerHyperedgeForRerouting(ConnEndList terminals)` | `Avoid.HyperedgeRerouter#registerHyperedgeForRerouting` | 1 | overload |
| `HyperedgeRerouter::registerHyperedgeForRerouting(JunctionRef *junction)` | `Avoid.HyperedgeRerouter#registerHyperedgeForRerouting` | 1 | overload |
| `HyperedgeRerouter::newAndDeletedObjectLists(index)` | `Avoid.HyperedgeRerouter#newAndDeletedObjectLists` | 1 | |
| `HyperedgeRerouter::count()` | `Avoid.HyperedgeRerouter#count` | 0 | |

## debughandler.h, dllexport.h

| Upstream | JavaScript | Args / value | Notes |
|---|---|---|---|
| `class DebugHandler` | — | | debug-visualisation hooks (Adaptagrams' internal debugging GUI); not ported by the core, and the hooks receive internal `VertInf *` |
| `DebugHandler::updateObstacleBoxes(obstacles)` | — | | see `DebugHandler` |
| `DebugHandler::updateConnectorRoute(conn, index1, index2)` | — | | see `DebugHandler` |
| `DebugHandler::beginningSearchWithEndpoints(src, tar)` | — | | see `DebugHandler` |
| `DebugHandler::updateCurrentSearchPath(currentPath)` | — | | see `DebugHandler` |
| `DebugHandler::beginningHyperedgeReroutingWithEndpoints(endpoints)` | — | | see `DebugHandler` |
| `DebugHandler::mtstGrowForestWithEdge(u, v, shouldWait)` | — | | see `DebugHandler` |
| `DebugHandler::mtstPotentialBridgingEdge(u, v)` | — | | see `DebugHandler` |
| `DebugHandler::mtstCommitToEdge(u, v, isBridge)` | — | | see `DebugHandler` |
| `AVOID_EXPORT` | — | | a C++ linkage macro |

## Internal headers included by libavoid.h

| Upstream | JavaScript | Args / value | Notes |
|---|---|---|---|
| `graph.h`: `class EdgeInf`, `class EdgeList`, `typedef EdgeInfList` | — | | visibility-graph edges: internal, not `AVOID_EXPORT`ed |
| `vertices.h`: `class VertID`, `class VertInf`, `class VertInfList`, `ContainsMap`, `IdSet` | — | | visibility-graph vertices: internal, not `AVOID_EXPORT`ed |
| `visibility.h`: `vertexVisibility()`, `vertexSweep()` | — | | visibility-graph construction: internal |
| `timer.h`: `class Timer` | — | | profiling (only with `AVOID_PROFILE`) |
| `debug.h`: `db_printf()` | — | | debug printing (only with `LIBAVOID_DEBUG`) |

## Additions from the libavoid-js fork

Methods of [Aksem/adaptagrams](https://github.com/Aksem/adaptagrams), the
Adaptagrams fork that libavoid-js builds on, that upstream does not have.
They only read state and change no route; `tests/fork_route_status_oracle.rs`
checks them against the C++ of that fork. The first column names the fork's
C++ declaration.

| Fork | JavaScript | Args / value | Notes |
|---|---|---|---|
| `ConnRef::hasValidRoute()` | `Avoid.ConnRef#hasValidRoute` | 0 | fork commit 3f52e7e1 |
| `ConnRef::hasCrossingObstacles()` | `Avoid.ConnRef#hasCrossingObstacles` | 0 | fork commit fe73f026 |

## JavaScript items without an upstream counterpart

WebAssembly infrastructure, kept outside the classes above:

* `init()`, `initSync()` (ES-module entry only): instantiate the wasm module.
* `AvoidInternalError`, `internalError()`, `reset()`: failure containment
  (see `index.d.ts`).
* `Router[Symbol.for("libavoid-rust.testPanic")]`: test-only trap
  trigger, inert unless given a fixed arming string; a symbol-keyed static
  so it is not part of the visible API.
* `delete()` on value types (`Point`, `Box`, `Edge`, `LineRep`,
  `PolygonInterface` and subclasses, `ConnEnd`, `Checkpoint`,
  `HyperedgeNewAndDeletedObjectLists`): the `delete` rule (nothing to
  free in JavaScript).
* The enum objects `RouterFlag`, `RoutingParameter`, `PenaltyType`,
  `RoutingOption`, `TransactionPhases`, `ConnType`, `ConnDirFlag`,
  `ConnEndType`, `ShapeTransformationType`: aliases grouping the flat
  constants under upstream's enum names.
