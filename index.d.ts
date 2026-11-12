// Type declarations for libavoid-rust, a Rust/WASM port of upstream C++
// libavoid (adaptagrams).  `AvoidRouter` is a thin adapter over the ported
// `Avoid::Router`: every method delegates to the upstream function named in
// its comment, so routing behaviour is upstream libavoid's.
//
// Ids
// ---
// Shapes, junctions, connectors and clusters are named by caller-chosen
// string ids, unique across all four kinds.  Each object also has the
// upstream numeric id (`unsigned int`): pass `numericId` to the creating
// method to choose it (accepted iff upstream `objectIdIsUnused(id)` holds),
// otherwise the router assigns `newObjectId()` (largest id so far + 1), i.e.
// sequential in creation order exactly as upstream.  Routing results can
// depend on numeric ids, so passing the ids of an upstream scene reproduces
// it exactly.  Objects the router creates itself (hyperedge rerouting /
// improvement) are named by the decimal string of their numeric id
// (suffixed "#2", "#3", ... if taken).  Connection pins are named per
// owner shape/junction.
//
// Transactions
// ------------
// As upstream, changes are queued until `processTransaction()` while
// `transactionUse()` is true (the default); routes are only (re)computed
// then.  With `setTransactionUse(false)` every change is processed
// immediately.
//
// Errors
// ------
// * Structurally malformed arguments (a non-string id, a point that is
//   neither [x, y] nor {x, y}, a non-array where an array is expected, a
//   non-boolean flag, a non-number where a number is expected) throw a
//   TypeError.
// * Otherwise nothing throws or aborts the wasm instance: mutators return
//   `false` and queries return `null` for unknown ids, duplicate ids,
//   non-finite numbers, out-of-range enum values and violated upstream
//   preconditions (documented per method).
// * The constructor throws an Error for invalid router flags.

export interface Point {
  x: number;
  y: number;
}

export type Route = Point[];
export type PointInput = Point | readonly [number, number];

/** upstream `Checkpoint(point, arrivalDirections = All, departureDirections = All)`. */
export type CheckpointInput =
  | PointInput
  | {
      point: PointInput;
      arrivalDirections?: number;
      departureDirections?: number;
    };

export interface Checkpoint {
  point: Point;
  arrivalDirections: number;
  departureDirections: number;
}

/**
 * An upstream `ConnEnd`:
 * - `[x, y]` / `{x, y}`: `ConnEnd(Point)` (directions `ConnDir.All`);
 * - `{point, directions}`: `ConnEnd(Point, ConnDirFlags)`;
 * - `{shapeId, classId}`: `ConnEnd(ShapeRef*, connectionPinClassID)`;
 * - `{shapeId, pinId}`: convenience for `{shapeId, classId: <class of that pin>}`
 *   (upstream connects to *any* suitable pin of the class, not that pin);
 * - `{junctionId}`: `ConnEnd(JunctionRef*)`.
 * Objects returned by `connectorEndpointConnEnds()` are accepted too.
 */
export type ConnEndInput =
  | PointInput
  | { point: PointInput; directions?: number }
  | { shapeId: string; classId: number }
  | { shapeId: string; pinId: string }
  | { junctionId: string }
  | ConnEnd;

/** upstream `ConnEnd` as returned by `ConnRef::endpointConnEnds()`. */
export type ConnEnd =
  | { type: "point"; position: Point; directions: number }
  | { type: "shapePin"; position: Point; directions: number; shapeId: string | null; classId: number }
  | { type: "junction"; position: Point; directions: number; junctionId: string | null; classId: number }
  | { type: "empty"; position: Point; directions: number };

export interface ConnectorEndpointConnEnds {
  source: ConnEnd;
  destination: ConnEnd;
}

export interface ConnectorSplitResult {
  junctionId: string;
  connectorId: string;
  position: Point;
}

export interface Box {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

export interface ShapeConnectionPinOptions {
  /** Pin class id, 1..4294967295 (upstream asserts > 0). */
  classId: number;
  xOffset: number;
  yOffset: number;
  /** Default false (absolute offsets from the shape's top-left). */
  proportional?: boolean;
  /** Default 0. */
  insideOffset?: number;
  /** Default `ConnDir.None` (upstream: derive from the position). */
  visibilityDirections?: number;
  /** `setConnectionCost(cost)` (>= 0) if given. */
  connectionCost?: number;
  /** `setExclusive(exclusive)` if given (upstream default otherwise). */
  exclusive?: boolean;
}

/** upstream `ShapeConnectionPin::ids()`. */
export interface ConnectionPinIds {
  objectId: string;
  numericObjectId: number;
  classId: number;
}

/** upstream `HyperedgeNewAndDeletedObjectLists`, with string ids. */
export interface HyperedgeNewAndDeletedObjectLists {
  newJunctionIds: string[];
  deletedJunctionIds: string[];
  newConnectorIds: string[];
  deletedConnectorIds: string[];
  changedConnectorIds: string[];
}

/** upstream `Polygon::curvedPolyline()`: points plus SVG path commands ("M", "L", "C", "Z"). */
export interface CurvedPolyline {
  points: Point[];
  commands: string[];
}

export const RouterFlag: Readonly<{
  PolyLineRouting: 1;
  OrthogonalRouting: 2;
}>;

export const ConnType: Readonly<{
  None: 0;
  PolyLine: 1;
  Orthogonal: 2;
}>;

export const ConnDir: Readonly<{
  None: 0;
  Up: 1;
  Down: 2;
  Left: 4;
  Right: 8;
  All: 15;
}>;

export const ConnEndType: Readonly<{
  Point: 0;
  ShapePin: 1;
  Junction: 2;
  Empty: 3;
}>;

export const ConnectorRelation: Readonly<{
  RunningTo: 1;
  RunningFrom: 2;
  RunningToAndFrom: 3;
}>;

export const RoutingParameter: Readonly<{
  segmentPenalty: 0;
  anglePenalty: 1;
  crossingPenalty: 2;
  clusterCrossingPenalty: 3;
  fixedSharedPathPenalty: 4;
  portDirectionPenalty: 5;
  shapeBufferDistance: 6;
  idealNudgingDistance: 7;
  reverseDirectionPenalty: 8;
}>;

/** upstream `typedef enum RoutingParameter PenaltyType`. */
export const PenaltyType: typeof RoutingParameter;

/** upstream `zeroParamValue` / `chooseSensibleParamValue`. */
export const RoutingParameterValue: Readonly<{
  zero: 0;
  chooseSensible: -1;
}>;

export const RoutingOption: Readonly<{
  nudgeOrthogonalSegmentsConnectedToShapes: 0;
  improveHyperedgeRoutesMovingJunctions: 1;
  penaliseOrthogonalSharedPathsAtConnEnds: 2;
  nudgeOrthogonalTouchingColinearSegments: 3;
  performUnifyingNudgingPreprocessingStep: 4;
  improveHyperedgeRoutesMovingAddingAndDeletingJunctions: 5;
  nudgeSharedPathsWithCommonEndPoint: 6;
}>;

export const AttachPos: Readonly<{
  Top: 0;
  Centre: 0.5;
  Bottom: 1;
  Left: 0;
  Right: 1;
  MinOffset: 0;
  MaxOffset: -1;
}>;

export const ConnectionPin: Readonly<{
  Unset: 2147483647;
  Centre: 2147483646;
}>;

export const ShapeTransformation: Readonly<{
  CW90: 0;
  CW180: 1;
  CW270: 2;
  FlipX: 3;
  FlipY: 4;
}>;

/** upstream XDIM / YDIM (`JunctionRef::preferOrthogonalDimension`). */
export const OrthogonalDimension: Readonly<{
  X: 0;
  Y: 1;
}>;

/** upstream `Polygon(points).curvedPolyline(curveAmount, closed = false)`; null for invalid input. */
export function curvedPolyline(points: readonly PointInput[], curveAmount: number, closed?: boolean): CurvedPolyline | null;

export class AvoidRouter {
  /** `new Router(flags)`; flags default `RouterFlag.OrthogonalRouting`.  Throws unless flags is 1, 2 or 3. */
  constructor(flags?: number);
  /** Same as the constructor. */
  static withFlags(flags: number): AvoidRouter;
  /** The constructor's flags (convenience). */
  routerFlags(): number;
  /** Convenience: delete the router and create a new one with the same flags, routing parameters, options and transaction setting. */
  clear(): void;

  // --- transactions ---
  /** `Router::setTransactionUse()`. */
  setTransactionUse(transactions: boolean): void;
  /** `Router::transactionUse()`. */
  transactionUse(): boolean;
  /** `Router::processTransaction()`: true if anything was processed. */
  processTransaction(): boolean;
  /**
   * Connectors whose upstream callback fired (i.e. were rerouted) in the last
   * transaction (with transaction use off: in all transactions run by the
   * last call).  Built on `ConnRef::setCallback()`.
   */
  lastTransactionUpdatedConnectorIds(): string[];

  // --- parameters ---
  /** `Router::setRoutingParameter()`; omitted value = `chooseSensibleParamValue`. */
  setRoutingParameter(parameter: number, value?: number): boolean;
  /** `Router::setRoutingPenalty()`; omitted value = `chooseSensibleParamValue`. */
  setRoutingPenalty(penaltyType: number, penaltyValue?: number): boolean;
  /** `Router::routingParameter()`. */
  routingParameter(parameter: number): number | null;
  /** `Router::setRoutingOption()`. */
  setRoutingOption(option: number, value: boolean): boolean;
  /** `Router::routingOption()`. */
  routingOption(option: number): boolean | null;
  /** `Router::validConnType(select = ConnType.None)`. */
  validConnType(select?: number): number | null;

  // --- ids ---
  /** string id: unused by any object of this router; number: upstream `Router::objectIdIsUnused(id)`. */
  objectIdIsUnused(id: string | number): boolean | null;
  /** `Router::newObjectId()`. */
  newObjectId(): number;
  /** The upstream numeric id (`id()`) of an object. */
  objectNumericId(id: string): number | null;
  /** The (earliest created) object with this numeric id. */
  objectIdFromNumericId(numericId: number): string | null;
  /** Ids of all shapes, in creation order. */
  shapeIds(): string[];
  junctionIds(): string[];
  connectorIds(): string[];
  clusterIds(): string[];

  // --- clusters ---
  /** `new ClusterRef(router, poly, numericId)` (at least 3 points). */
  addCluster(id: string, points: readonly PointInput[], numericId?: number): boolean;
  /** Convenience: `addCluster` with a `Rectangle`. */
  addClusterRectangle(id: string, left: number, top: number, right: number, bottom: number, numericId?: number): boolean;
  /** `ClusterRef::setNewPoly()`. */
  setClusterPolygon(id: string, points: readonly PointInput[]): boolean;
  /** Convenience: `setClusterPolygon` with a `Rectangle`. */
  setClusterRectangle(id: string, left: number, top: number, right: number, bottom: number): boolean;
  /** `Router::deleteCluster()`. */
  removeCluster(id: string): boolean;
  /** Convenience: bounding box of `rectangularPolygon()`. */
  clusterBoundingBox(id: string): Box | null;
  /** `ClusterRef::polygon()`. */
  clusterPolygon(id: string): Point[] | null;
  /** `ClusterRef::rectangularPolygon()`. */
  clusterRectangularPolygon(id: string): Point[] | null;

  // --- shapes ---
  /** `new ShapeRef(router, Rectangle(Point(left, top), Point(right, bottom)), numericId)`. */
  addShape(id: string, left: number, top: number, right: number, bottom: number, numericId?: number): boolean;
  /** `new ShapeRef(router, poly, numericId)` (at least 3 points). */
  addPolygonShape(id: string, points: readonly PointInput[], numericId?: number): boolean;
  /** `Router::moveShape(shape, Rectangle(...))` (the shape must have 4 points). */
  moveShape(id: string, left: number, top: number, right: number, bottom: number): boolean;
  /** `Router::moveShape(shape, newPoly, firstMove = false)` (same number of points, which upstream asserts). */
  movePolygonShape(id: string, points: readonly PointInput[], firstMove?: boolean): boolean;
  /** `Router::moveShape(shape, xDiff, yDiff)`. */
  moveShapeBy(id: string, xDiff: number, yDiff: number): boolean;
  /** Convenience: `polygon().offsetBoundingBox(0)`. */
  shapeBoundingBox(id: string): Box | null;
  /** `Obstacle::routingBox()` (includes shapeBufferDistance). */
  shapeRoutingBox(id: string): Box | null;
  /** `ShapeRef::polygon()`. */
  shapePolygon(id: string): Point[] | null;
  /** `Obstacle::routingPolygon()`. */
  shapeRoutingPolygon(id: string): Point[] | null;
  /** `ShapeRef::position()`. */
  shapePosition(id: string): Point | null;
  /** `Router::shapeContainingPoint()` (shapes in the scene, i.e. processed). */
  shapeContainingPoint(point: PointInput): string | null;
  /** `Obstacle::attachedConnectors()`. */
  shapeAttachedConnectors(id: string): string[] | null;
  /** `Router::attachedShapes(shapes, shapeId, type)` (type default RunningToAndFrom; junction ids accepted). */
  shapeAttachedShapes(id: string, type?: number): string[] | null;
  /** `Router::attachedConns(conns, shapeId, type)` (type default RunningToAndFrom; junction ids accepted). */
  shapeAttachedConns(id: string, type?: number): string[] | null;
  /** `ShapeRef::transformConnectionPinPositions()`. */
  transformShapeConnectionPinPositions(id: string, transform: number): boolean;
  /**
   * `Router::deleteShape()` (queued).  false while the shape's own addition is
   * still queued (upstream asserts against that; call processTransaction()
   * first) or while a hyperedge registered for rerouting references it.
   */
  removeShape(id: string): boolean;
  /** `Router::markAllObstaclesAsMoved()`. */
  markAllObstaclesAsMoved(): void;

  // --- junctions ---
  /** `new JunctionRef(router, Point(x, y), numericId)`. */
  addJunction(id: string, x: number, y: number, numericId?: number): boolean;
  /** `Router::deleteJunction()` (same restrictions as removeShape). */
  removeJunction(id: string): boolean;
  /** `JunctionRef::removeJunctionAndMergeConnectors()`: the merged connector's id or null. */
  removeJunctionAndMergeConnectors(id: string): string | null;
  /** `Router::moveJunction(junction, Point)`. */
  moveJunction(id: string, x: number, y: number): boolean;
  /** `Router::moveJunction(junction, xDiff, yDiff)`. */
  moveJunctionBy(id: string, xDiff: number, yDiff: number): boolean;
  /** `JunctionRef::position()`. */
  junctionPosition(id: string): Point | null;
  /** `JunctionRef::recommendedPosition()`. */
  junctionRecommendedPosition(id: string): Point | null;
  /** `JunctionRef::setPositionFixed()`. */
  setJunctionPositionFixed(id: string, fixed: boolean): boolean;
  /** `JunctionRef::positionFixed()`. */
  junctionPositionFixed(id: string): boolean | null;
  /** `JunctionRef::preferOrthogonalDimension(dim)` (OrthogonalDimension.X / .Y). */
  preferJunctionOrthogonalDimension(id: string, dimension: number): boolean;
  /** `Obstacle::attachedConnectors()`. */
  junctionAttachedConnectors(id: string): string[] | null;

  // --- connection pins ---
  /**
   * `new ShapeConnectionPin(shape, classId, xOffset, yOffset, proportional = false,
   * insideOffset = 0, visDirs = ConnDir.None)`, named `pinId` on the shape.
   */
  addShapeConnectionPin(
    shapeId: string,
    pinId: string,
    classId: number,
    xOffset: number,
    yOffset: number,
    proportional?: boolean,
    insideOffset?: number,
    visibilityDirections?: number,
  ): boolean;
  /** As addShapeConnectionPin, then `setConnectionCost()` / `setExclusive()` if given. */
  addShapeConnectionPinWithOptions(shapeId: string, pinId: string, options: ShapeConnectionPinOptions): boolean;
  /** `delete pin` (false while a registered hyperedge references the shape). */
  removeShapeConnectionPin(shapeId: string, pinId: string): boolean;
  /** Names of the shape's pins, in creation order. */
  shapeConnectionPinIdList(shapeId: string): string[] | null;
  /** `ShapeConnectionPin::setConnectionCost()` (cost >= 0). */
  setShapeConnectionPinConnectionCost(shapeId: string, pinId: string, cost: number): boolean;
  /** `ShapeConnectionPin::setExclusive()`. */
  setShapeConnectionPinExclusive(shapeId: string, pinId: string, exclusive: boolean): boolean;
  /** `ShapeConnectionPin::isExclusive()`. */
  shapeConnectionPinExclusive(shapeId: string, pinId: string): boolean | null;
  /** `ShapeConnectionPin::position()`. */
  shapeConnectionPinPosition(shapeId: string, pinId: string): Point | null;
  /** `ShapeConnectionPin::directions()`. */
  shapeConnectionPinDirections(shapeId: string, pinId: string): number | null;
  /** `ShapeConnectionPin::ids()`. */
  shapeConnectionPinIds(shapeId: string, pinId: string): ConnectionPinIds | null;
  /** `new ShapeConnectionPin(junction, classId, visDirs = ConnDir.None)` (classId > 0). */
  addJunctionConnectionPin(junctionId: string, pinId: string, classId: number, visibilityDirections?: number): boolean;
  /** `delete pin`. */
  removeJunctionConnectionPin(junctionId: string, pinId: string): boolean;
  junctionConnectionPinPosition(junctionId: string, pinId: string): Point | null;
  junctionConnectionPinDirections(junctionId: string, pinId: string): number | null;
  junctionConnectionPinIds(junctionId: string, pinId: string): ConnectionPinIds | null;
  setJunctionConnectionPinConnectionCost(junctionId: string, pinId: string, cost: number): boolean;
  setJunctionConnectionPinExclusive(junctionId: string, pinId: string, exclusive: boolean): boolean;
  junctionConnectionPinExclusive(junctionId: string, pinId: string): boolean | null;

  // --- connectors: creation ---
  /** `new ConnRef(router, numericId)` (endpoints set later). */
  addConnectorWithoutEndpoints(id: string, numericId?: number): boolean;
  /**
   * `new ConnRef(router, src, dst, numericId)`.  With transaction use off the
   * construction is processed as one transaction after the connector is
   * complete (upstream's constructor would route it before registering its
   * reroute flag and assert).
   */
  addConnectorWithConnEnds(id: string, src: ConnEndInput, dst: ConnEndInput, numericId?: number): boolean;
  /** Convenience: addConnectorWithConnEnds with two free points. */
  addConnector(id: string, srcX: number, srcY: number, dstX: number, dstY: number, numericId?: number): boolean;
  /** Convenience: `ConnEnd(Point, dirs)` at both ends. */
  addConnectorWithEndpointDirections(
    id: string,
    src: PointInput,
    srcDirections: number,
    dst: PointInput,
    dstDirections: number,
    numericId?: number,
  ): boolean;
  /** Convenience: `ConnEnd(shape, classOf(pin))` at both ends. */
  addConnectorBetweenPins(
    id: string,
    srcShapeId: string,
    srcPinId: string,
    dstShapeId: string,
    dstPinId: string,
    numericId?: number,
  ): boolean;
  /** Convenience: `ConnEnd(shape, classId)` at both ends. */
  addConnectorBetweenPinClasses(
    id: string,
    srcShapeId: string,
    srcPinClassId: number,
    dstShapeId: string,
    dstPinClassId: number,
    numericId?: number,
  ): boolean;
  /** Convenience: `ConnEnd(junction)` -> `ConnEnd(Point)`. */
  addConnectorFromJunction(id: string, srcJunctionId: string, dstX: number, dstY: number, numericId?: number): boolean;
  /** Convenience: `ConnEnd(Point)` -> `ConnEnd(junction)`. */
  addConnectorToJunction(id: string, srcX: number, srcY: number, dstJunctionId: string, numericId?: number): boolean;
  /** Convenience: `ConnEnd(junction)` at both ends. */
  addConnectorBetweenJunctions(id: string, srcJunctionId: string, dstJunctionId: string, numericId?: number): boolean;

  // --- connectors: endpoints ---
  /** `ConnRef::setEndpoints(src, dst)`. */
  setConnectorConnEnds(id: string, src: ConnEndInput, dst: ConnEndInput): boolean;
  /** `ConnRef::setSourceEndpoint(end)`. */
  setConnectorSourceConnEnd(id: string, end: ConnEndInput): boolean;
  /** `ConnRef::setDestEndpoint(end)`. */
  setConnectorDestConnEnd(id: string, end: ConnEndInput): boolean;
  /** Convenience: setConnectorConnEnds with free points. */
  setConnectorEndpoints(id: string, src: PointInput, dst: PointInput): boolean;
  setConnectorEndpointsWithDirections(
    id: string,
    src: PointInput,
    srcDirections: number,
    dst: PointInput,
    dstDirections: number,
  ): boolean;
  setConnectorSourceEndpoint(id: string, point: PointInput): boolean;
  setConnectorSourceEndpointWithDirections(id: string, point: PointInput, directions: number): boolean;
  setConnectorDestEndpoint(id: string, point: PointInput): boolean;
  setConnectorDestEndpointWithDirections(id: string, point: PointInput, directions: number): boolean;
  setConnectorSourceJunctionEndpoint(id: string, junctionId: string): boolean;
  setConnectorDestJunctionEndpoint(id: string, junctionId: string): boolean;
  /** Convenience: `ConnEnd(shape, classOf(pin))`. */
  setConnectorSourcePinEndpoint(id: string, shapeId: string, pinId: string): boolean;
  setConnectorDestPinEndpoint(id: string, shapeId: string, pinId: string): boolean;
  setConnectorSourcePinClassEndpoint(id: string, shapeId: string, pinClassId: number): boolean;
  setConnectorDestPinClassEndpoint(id: string, shapeId: string, pinClassId: number): boolean;
  /** `Router::deleteConnector()` (immediate). */
  removeConnector(id: string): boolean;

  // --- connectors: properties ---
  /** `ConnRef::setRoutingType()` (normalised by `validConnType`). */
  setConnectorRoutingType(id: string, routingType: number): boolean;
  connectorRoutingType(id: string): number | null;
  setConnectorHateCrossings(id: string, value: boolean): boolean;
  connectorDoesHateCrossings(id: string): boolean | null;
  /** `ConnRef::endpointConnEnds()` (reflects processed endpoint changes). */
  connectorEndpointConnEnds(id: string): ConnectorEndpointConnEnds | null;
  /** `ConnRef::possibleDstPinPoints()`. */
  connectorPossibleDstPinPoints(id: string): Point[] | null;
  /** `ConnRef::setRoutingCheckpoints()`. */
  setConnectorRoutingCheckpoints(id: string, checkpoints: readonly CheckpointInput[]): boolean;
  /** `ConnRef::routingCheckpoints()`. */
  connectorRoutingCheckpoints(id: string): Checkpoint[] | null;
  /** `ConnRef::setFixedRoute()`. */
  setConnectorFixedRoute(id: string, route: readonly PointInput[]): boolean;
  /** `ConnRef::setFixedExistingRoute()` (false without a route of >= 2 points, which upstream asserts). */
  setConnectorFixedExistingRoute(id: string): boolean;
  connectorHasFixedRoute(id: string): boolean | null;
  clearConnectorFixedRoute(id: string): boolean;
  /** `ConnRef::makePathInvalid()`. */
  makeConnectorPathInvalid(id: string): boolean;
  /** `ConnRef::needsRepaint()`. */
  connectorNeedsRepaint(id: string): boolean | null;
  /** `ConnRef::route()`: the raw route of the last transaction ([] before routing). */
  routeConnector(id: string): Route | null;
  /** `ConnRef::displayRoute()`: the simplified, nudged route to draw. */
  displayConnectorRoute(id: string): Route | null;
  /** `displayRoute().curvedPolyline(curveAmount, closed = false)`. */
  connectorCurvedRoute(id: string, curveAmount: number, closed?: boolean): CurvedPolyline | null;
  /**
   * `ConnRef::splitAtSegment(segmentN)`; the new junction and connector get the
   * given ids.  null for segmentN 0, segmentN >= displayRoute().length, a
   * free-point destination (upstream dereferences a null ConnEnd) or taken ids.
   */
  splitConnectorAtSegment(id: string, segmentN: number, junctionId: string, connectorId: string): ConnectorSplitResult | null;
  /** Convenience: `processTransaction()`, then the display route of every connector. */
  routeAll(): Record<string, Route>;

  // --- diagnostics ---
  existsInvalidOrthogonalPaths(): boolean;
  existsCrossings(optimisedForConnectorType?: boolean): number;
  existsOrthogonalSegmentOverlap(atEnds?: boolean): boolean;
  existsOrthogonalFixedSegmentOverlap(atEnds?: boolean): boolean;
  existsOrthogonalTouchingPaths(): boolean;
  /**
   * `Router::outputDiagramText()` returned as a string (no file is written).
   * fullPrecision: the `%.17g` + junction-block format of the test goldens.
   */
  outputDiagramText(fullPrecision?: boolean): string;
  /** `Router::printInfo()` returned as a string. */
  printInfo(): string;

  // --- hyperedges ---
  /** `hyperedgeRerouter()->registerHyperedgeForRerouting(JunctionRef*)`: the index. */
  registerHyperedgeForReroutingJunction(junctionId: string): number | null;
  /**
   * `hyperedgeRerouter()->registerHyperedgeForRerouting(ConnEndList)`: the index.
   * Shape-pin terminals need an existing pin of their class.
   */
  registerHyperedgeForRerouting(terminals: readonly ConnEndInput[]): number | null;
  /** `hyperedgeRerouter()->count()` (registrations pending for the next transaction). */
  hyperedgeReroutingCount(): number;
  /** `hyperedgeRerouter()->newAndDeletedObjectLists(index)` of the last transaction. */
  hyperedgeNewAndDeletedObjectLists(index: number): HyperedgeNewAndDeletedObjectLists | null;
  /** `Router::newAndDeletedObjectListsFromHyperedgeImprovement()`. */
  newAndDeletedObjectListsFromHyperedgeImprovement(): HyperedgeNewAndDeletedObjectLists;

  free(): void;
}

/**
 * The raw wasm-bindgen class behind `AvoidRouter`: numbers instead of
 * objects, JSON strings for structured results, `undefined` instead of null,
 * ConnEnds as (kind, objectId, pinId, x, y, num) with kind 0 = point
 * (num = directions), 1 = shape pin class (num = class id), 2 = junction,
 * 3 = shape pin by name.  Same validation as AvoidRouter (never aborts).
 */
export class WasmAvoidRouter {
  constructor(flags?: number);
  static withFlags(flags: number): WasmAvoidRouter;
  routerFlags(): number;
  clear(): void;
  setTransactionUse(transactions: boolean): void;
  transactionUse(): boolean;
  processTransaction(): boolean;
  lastTransactionUpdatedConnectorIds(): string;
  setRoutingParameter(parameter: number, value?: number): boolean;
  setRoutingPenalty(penalty_type: number, penalty_value?: number): boolean;
  routingParameter(parameter: number): number | undefined;
  setRoutingOption(option: number, value: boolean): boolean;
  routingOption(option: number): boolean | undefined;
  validConnType(select?: number): number | undefined;
  objectIdIsUnused(id: string): boolean;
  numericObjectIdIsUnused(id: number): boolean | undefined;
  newObjectId(): number;
  objectNumericId(id: string): number | undefined;
  objectIdFromNumericId(numeric_id: number): string;
  shapeIds(): string;
  junctionIds(): string;
  connectorIds(): string;
  clusterIds(): string;
  addCluster(id: string, coordinates: Float64Array | number[], numeric_id?: number): boolean;
  addClusterRectangle(id: string, left: number, top: number, right: number, bottom: number, numeric_id?: number): boolean;
  setClusterPolygon(id: string, coordinates: Float64Array | number[]): boolean;
  setClusterRectangle(id: string, left: number, top: number, right: number, bottom: number): boolean;
  removeCluster(id: string): boolean;
  clusterBoundingBox(id: string): string;
  clusterPolygon(id: string): string;
  clusterRectangularPolygon(id: string): string;
  addShape(id: string, left: number, top: number, right: number, bottom: number, numeric_id?: number): boolean;
  addPolygonShape(id: string, coordinates: Float64Array | number[], numeric_id?: number): boolean;
  moveShape(id: string, left: number, top: number, right: number, bottom: number): boolean;
  movePolygonShape(id: string, coordinates: Float64Array | number[], first_move?: boolean): boolean;
  moveShapeBy(id: string, x_diff: number, y_diff: number): boolean;
  shapeBoundingBox(id: string): string;
  shapeRoutingBox(id: string): string;
  shapePolygon(id: string): string;
  shapeRoutingPolygon(id: string): string;
  shapePosition(id: string): string;
  shapeContainingPoint(x: number, y: number): string;
  shapeAttachedConnectors(id: string): string;
  shapeAttachedShapes(id: string, relation?: number): string;
  shapeAttachedConns(id: string, relation?: number): string;
  transformShapeConnectionPinPositions(id: string, transform: number): boolean;
  removeShape(id: string): boolean;
  markAllObstaclesAsMoved(): void;
  addJunction(id: string, x: number, y: number, numeric_id?: number): boolean;
  removeJunction(id: string): boolean;
  removeJunctionAndMergeConnectors(id: string): string;
  moveJunction(id: string, x: number, y: number): boolean;
  moveJunctionBy(id: string, x_diff: number, y_diff: number): boolean;
  junctionPosition(id: string): string;
  junctionRecommendedPosition(id: string): string;
  setJunctionPositionFixed(id: string, fixed: boolean): boolean;
  junctionPositionFixed(id: string): boolean | undefined;
  preferJunctionOrthogonalDimension(id: string, dimension: number): boolean;
  junctionAttachedConnectors(id: string): string;
  addShapeConnectionPin(
    shape_id: string,
    pin_id: string,
    class_id: number,
    x_offset: number,
    y_offset: number,
    proportional?: boolean,
    inside_offset?: number,
    visibility_directions?: number,
  ): boolean;
  addShapeConnectionPinWithOptions(
    shape_id: string,
    pin_id: string,
    class_id: number,
    x_offset: number,
    y_offset: number,
    proportional?: boolean,
    inside_offset?: number,
    visibility_directions?: number,
    connection_cost?: number,
    exclusive?: boolean,
  ): boolean;
  removeShapeConnectionPin(shape_id: string, pin_id: string): boolean;
  shapeConnectionPinIdList(shape_id: string): string;
  setShapeConnectionPinConnectionCost(shape_id: string, pin_id: string, cost: number): boolean;
  setShapeConnectionPinExclusive(shape_id: string, pin_id: string, exclusive: boolean): boolean;
  shapeConnectionPinExclusive(shape_id: string, pin_id: string): boolean | undefined;
  shapeConnectionPinPosition(shape_id: string, pin_id: string): string;
  shapeConnectionPinDirections(shape_id: string, pin_id: string): number | undefined;
  shapeConnectionPinIds(shape_id: string, pin_id: string): string;
  addJunctionConnectionPin(junction_id: string, pin_id: string, class_id: number, visibility_directions?: number): boolean;
  removeJunctionConnectionPin(junction_id: string, pin_id: string): boolean;
  junctionConnectionPinPosition(junction_id: string, pin_id: string): string;
  junctionConnectionPinDirections(junction_id: string, pin_id: string): number | undefined;
  junctionConnectionPinIds(junction_id: string, pin_id: string): string;
  setJunctionConnectionPinConnectionCost(junction_id: string, pin_id: string, cost: number): boolean;
  setJunctionConnectionPinExclusive(junction_id: string, pin_id: string, exclusive: boolean): boolean;
  junctionConnectionPinExclusive(junction_id: string, pin_id: string): boolean | undefined;
  addConnectorWithoutEndpoints(id: string, numeric_id?: number): boolean;
  addConnectorWithConnEnds(
    id: string,
    src_kind: number,
    src_object: string,
    src_pin: string,
    src_x: number,
    src_y: number,
    src_num: number,
    dst_kind: number,
    dst_object: string,
    dst_pin: string,
    dst_x: number,
    dst_y: number,
    dst_num: number,
    numeric_id?: number,
  ): boolean;
  addConnector(id: string, src_x: number, src_y: number, dst_x: number, dst_y: number, numeric_id?: number): boolean;
  setConnectorConnEnds(
    id: string,
    src_kind: number,
    src_object: string,
    src_pin: string,
    src_x: number,
    src_y: number,
    src_num: number,
    dst_kind: number,
    dst_object: string,
    dst_pin: string,
    dst_x: number,
    dst_y: number,
    dst_num: number,
  ): boolean;
  setConnectorSourceConnEnd(id: string, kind: number, object_id: string, pin_id: string, x: number, y: number, num: number): boolean;
  setConnectorDestConnEnd(id: string, kind: number, object_id: string, pin_id: string, x: number, y: number, num: number): boolean;
  removeConnector(id: string): boolean;
  setConnectorRoutingType(id: string, routing_type: number): boolean;
  connectorRoutingType(id: string): number | undefined;
  setConnectorHateCrossings(id: string, value: boolean): boolean;
  connectorDoesHateCrossings(id: string): boolean | undefined;
  connectorEndpointConnEnds(id: string): string;
  connectorPossibleDstPinPoints(id: string): string;
  setConnectorRoutingCheckpoints(id: string, checkpoints: Float64Array | number[]): boolean;
  connectorRoutingCheckpoints(id: string): string;
  setConnectorFixedRoute(id: string, route: Float64Array | number[]): boolean;
  setConnectorFixedExistingRoute(id: string): boolean;
  connectorHasFixedRoute(id: string): boolean | undefined;
  clearConnectorFixedRoute(id: string): boolean;
  makeConnectorPathInvalid(id: string): boolean;
  connectorNeedsRepaint(id: string): boolean | undefined;
  routeConnector(id: string): string;
  displayConnectorRoute(id: string): string;
  connectorCurvedRoute(id: string, curve_amount: number, closed?: boolean): string;
  splitConnectorAtSegment(id: string, segment_n: number, junction_id: string, connector_id: string): string;
  routeAll(): string;
  existsInvalidOrthogonalPaths(): boolean;
  existsCrossings(optimised_for_connector_type?: boolean): number;
  existsOrthogonalSegmentOverlap(at_ends?: boolean): boolean;
  existsOrthogonalFixedSegmentOverlap(at_ends?: boolean): boolean;
  existsOrthogonalTouchingPaths(): boolean;
  outputDiagramText(full_precision?: boolean): string;
  printInfo(): string;
  registerHyperedgeForReroutingJunction(junction_id: string): number | undefined;
  registerHyperedgeForRerouting(
    kinds: Float64Array | number[],
    object_ids: string[],
    pin_ids: string[],
    xs: Float64Array | number[],
    ys: Float64Array | number[],
    nums: Float64Array | number[],
  ): number | undefined;
  hyperedgeReroutingCount(): number;
  hyperedgeNewAndDeletedObjectLists(index: number): string;
  newAndDeletedObjectListsFromHyperedgeImprovement(): string;
  free(): void;
}
