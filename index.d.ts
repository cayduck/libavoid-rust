// Type declarations for libavoid-rust: upstream C++ libavoid (adaptagrams,
// namespace Avoid) ported to Rust/WebAssembly, with upstream's public API
// in JavaScript.  The module exports mirror namespace Avoid: same class
// names, method names, argument order, default arguments and semantics.
// Routing is done by the faithful Rust port of upstream; the JavaScript
// layer adds no behaviour of its own (test/fixtures-replay.cjs replays all
// 83 upstream test programs through this API against the C++ output).
// docs/JS_API_COVERAGE.md lists every upstream public item and its
// JavaScript counterpart (or why there is none).
//
//   const Avoid = require("libavoid-rust");      // ES modules / browsers:
//                                                 //   import init, * as Avoid from "libavoid-rust"; await init();
//   const router = new Avoid.Router(Avoid.OrthogonalRouting);
//   const shape = new Avoid.ShapeRef(router, new Avoid.Rectangle(new Avoid.Point(0, 0), new Avoid.Point(50, 50)));
//   new Avoid.ShapeConnectionPin(shape, 1, Avoid.ATTACH_POS_CENTRE, Avoid.ATTACH_POS_CENTRE, true, 0, Avoid.ConnDirNone);
//   const conn = new Avoid.ConnRef(router, new Avoid.ConnEnd(shape, 1), new Avoid.ConnEnd(new Avoid.Point(200, 20)));
//   conn.setCallback((c) => draw(c.displayRoute()), conn);
//   router.processTransaction();
//   const route = conn.displayRoute();          // Avoid.PolyLine: route.size(), route.at(i), route.ps
//   router.moveShape(shape, 10, 0);
//   router.processTransaction();
//   router.delete();
//
// How C++ maps to JavaScript (one rule each)
// ------------------------------------------
// * Names, argument order, defaults.  As upstream.  A parameter is optional
//   iff the C++ parameter has a default argument, with the same default;
//   passing `undefined` means "use the default".
// * Overloads.  One JavaScript function per C++ name, dispatching on the
//   number and types of the arguments: `router.moveShape(shape, polygon,
//   first_move = false)` / `moveShape(shape, xDiff, yDiff)`, the
//   constructors of `ConnEnd`, `ConnRef`, `ShapeConnectionPin`, `Point`,
//   `Polygon`, `Rectangle`, `Checkpoint`, `ReferencingPolygon`, ...
// * Numbers.  `double` is a number; integer types (`unsigned int`, `int`,
//   `size_t`, enums) must be integers in range, `bool` must be a boolean,
//   `ConnDirFlags` a combination of the ConnDir* bits (0..15), `char` a
//   one-character string.  Anything else throws a TypeError.
// * Value types (Point/Vector, Box, Edge, LineRep, Polygon/PolyLine,
//   Rectangle, ReferencingPolygon, ConnEnd, Checkpoint,
//   HyperedgeNewAndDeletedObjectLists) are plain JavaScript objects.  An
//   argument passed by value or `const&` is copied when it is passed; a
//   result is a fresh copy -- also where upstream returns a reference to
//   internal state (`shape.polygon()`, `conn.displayRoute()`,
//   `cluster.rectangularPolygon()`, ...): change the object through its
//   methods, not by editing the returned copy.
// * Objects owned by a router (ShapeRef, JunctionRef, ConnRef, ClusterRef,
//   ShapeConnectionPin and the router's HyperedgeRerouter) are wrappers with
//   identity: every getter returns the same wrapper for the same object,
//   including objects the router creates itself (hyperedge rerouting and
//   improvement, `splitAtSegment`).  After the object is deleted -- by you
//   (`router.deleteShape(shape)`, `pin.delete()`, ...) or by the router
//   (hyperedge improvement) -- its wrapper throws on every use, as the
//   dangling pointer would be invalid upstream.  `obj.id()` is upstream's
//   numeric id; constructors take upstream's optional `id` (0 = assign
//   `router.newObjectId()`).
// * Containers.  `std::list` / `std::vector` values are arrays; a
//   `std::pair` is a two-element array.
// * Out-parameters.  A container out-parameter is an array that is
//   appended to, as upstream appends (`router.attachedShapes(shapes,
//   shapeId, type)`); a scalar out-parameter (`bool&`, `double*`) is a
//   one-element array whose element 0 is read (in/out) and set.
// * Operators.  `operator==` is `equal(rhs)`, `!=` `notEqual(rhs)`, `<`
//   `lessThan(rhs)`, `[]` `get(dim)` / `set(dim, value)`, `+` `add(rhs)`,
//   `-` `sub(rhs)`.
// * Public data members are properties with upstream's names
//   (`point.x`, `point.id`, `poly.ps`, `poly._id`, `checkpoint.point`,
//   `router.RubberBandRouting`, `router.st_checked_edges`, ...).  The
//   router's object lists `m_obstacles`, `connRefs` and `clusterRefs` are
//   read-only (fresh arrays of the wrappers, in upstream's list order).
// * `delete obj` is `obj.delete()`: for a Router it frees the router and
//   everything it owns (all its wrappers become invalid; a Router is also
//   freed when it is garbage collected); for a ShapeConnectionPin it
//   removes the pin; for value types it does nothing.  Shapes, junctions,
//   connectors and clusters are deleted through the Router
//   (`deleteShape`, `deleteJunction`, `deleteConnector`, `deleteCluster`),
//   as upstream requires.
// * Virtual methods.  Override by subclassing: `class MyRouter extends
//   Avoid.Router { shouldContinueTransactionWithProgress(elapsedTime,
//   phaseNumber, totalPhases, proportion) { ... } }`.  It is called with
//   upstream's arguments at upstream's call sites during a transaction
//   (`elapsedTime`: milliseconds since the JavaScript call that runs the
//   transaction started).  It must not use its own router (that throws);
//   an exception it throws counts as a stop request and is rethrown when
//   the call returns.
// * Callbacks.  `conn.setCallback(cb, ptr)`: `cb(ptr)` is invoked for each
//   connector whose route changed, in the order upstream invokes the
//   callbacks, after the routing finished and before the call that ran the
//   transaction (`processTransaction()`, or any change while
//   `transactionUse()` is false) returns, so a callback may use the
//   router.  An exception thrown by a callback propagates out of that call;
//   the remaining callbacks of the transaction are not invoked.
// * `FILE*` / file-name output returns the text: `outputInstanceToSVG()`,
//   `outputDiagramSVG()`, `outputDiagramText()`, `outputDiagram()` and
//   `printInfo()` return what upstream writes.
//
// Misuse
// ------
// Upstream checks its preconditions with COLA_ASSERT (abort in debug
// builds, undefined behaviour otherwise).  Here a wrong argument type or
// count throws a TypeError; a violated documented precondition, the use of
// a deleted object, or an object of another Router throws an Error (the
// analogue of the assert).  Neither changes any state.  The preconditions
// checked: Router flags include PolyLineRouting or OrthogonalRouting; ids
// are unused (`objectIdIsUnused`); routing parameter / option < its
// `last...Marker`; a ConnEnd's pin class is > 0 and not
// CONNECTIONPIN_UNSET; a pin's class is > 0; `setConnectionCost(cost)`
// with cost >= 0; `Polygon.at` / `setPoint` index < size(); `Point.get` /
// `set` dimension XDIM or YDIM; `vecDir` maybeZero >= 0; `setNewPoly`
// keeps the point count and the obstacle is not in the visibility graph;
// `makeActive` / `makeInactive` toggle; `ReferencingPolygon` / cluster
// point ids name obstacles of the router and `vn` a point of the
// obstacle's polygon; `setEndpoint(type, end)` with type 1 (VertID::src) or
// 2 (VertID::tar).  Where upstream behaviour is undefined and the Rust
// core defines it ("Public-API misuse" in src/avoid/router.rs: e.g. shapes
// with non-finite coordinates are inert, `splitAtSegment(0)` is a no-op),
// that is what happens; where neither does, the call throws an Error
// instead of reaching undefined behaviour (a ShapeConnectionPin's offsets
// must be finite).
//
// WebAssembly infrastructure (no upstream counterpart)
// ----------------------------------------------------
// `init()` / `initSync()` (ES-module entry: load the wasm module before
// the first `new Router()`), and failure containment: the wasm module is
// built with `panic = "abort"`, so an internal failure (a bug: a Rust
// panic, a failed allocation, a stack overflow) traps the WebAssembly
// instance.  The call throws an `AvoidInternalError` (method name, Rust
// panic message and source location, `kind`, `cause`), and the instance is
// *poisoned*: every object created on it -- not only the one that failed
// -- throws `AvoidInternalError` (`kind` "poisoned") from every later call
// without entering wasm, as do `new Router()` and the wasm-backed free
// functions; `delete()` stays allowed.  `reset()` (ES modules: `await
// reset()` or `await init()`) instantiates a fresh module; objects created
// afterwards work normally.  `internalError()` returns the error that
// poisoned the current instance, or null.
//
// Additions from the libavoid-js fork (no upstream counterpart)
// -------------------------------------------------------------
// `ConnRef.hasValidRoute()` and `ConnRef.hasCrossingObstacles()` come from
// Aksem/adaptagrams, the Adaptagrams fork that libavoid-js builds on.  They
// only read state and change no route (tests/fork_route_status_oracle.rs
// checks them against the C++ of that fork).

// --- constants (upstream names) -------------------------------------------------

/** geomtypes.h */
export const XDIM: 0;
export const YDIM: 1;
export const kUnassignedVertexNumber: 8;
export const kShapeConnectionPin: 9;
/** geometry.h segmentIntersectPoint() / rayIntersectPoint() results */
export const DONT_INTERSECT: 0;
export const DO_INTERSECT: 1;
export const PARALLEL: 3;

/** router.h enum RouterFlag */
export const PolyLineRouting: 1;
export const OrthogonalRouting: 2;
export type RouterFlag = 1 | 2;
export const RouterFlag: { readonly PolyLineRouting: 1; readonly OrthogonalRouting: 2 };

/** router.h: connector relation for attachedShapes() / attachedConns() */
export const runningTo: 1;
export const runningFrom: 2;
export const runningToAndFrom: 3;

/** router.h enum RoutingParameter (typedef PenaltyType) */
export const segmentPenalty: 0;
export const anglePenalty: 1;
export const crossingPenalty: 2;
export const clusterCrossingPenalty: 3;
export const fixedSharedPathPenalty: 4;
export const portDirectionPenalty: 5;
export const shapeBufferDistance: 6;
export const idealNudgingDistance: 7;
export const reverseDirectionPenalty: 8;
export const lastRoutingParameterMarker: 9;
export type RoutingParameter = 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9;
export type PenaltyType = RoutingParameter;
export const RoutingParameter: {
  readonly segmentPenalty: 0;
  readonly anglePenalty: 1;
  readonly crossingPenalty: 2;
  readonly clusterCrossingPenalty: 3;
  readonly fixedSharedPathPenalty: 4;
  readonly portDirectionPenalty: 5;
  readonly shapeBufferDistance: 6;
  readonly idealNudgingDistance: 7;
  readonly reverseDirectionPenalty: 8;
  readonly lastRoutingParameterMarker: 9;
};
export const PenaltyType: typeof RoutingParameter;

/** router.h enum RoutingOption */
export const nudgeOrthogonalSegmentsConnectedToShapes: 0;
export const improveHyperedgeRoutesMovingJunctions: 1;
export const penaliseOrthogonalSharedPathsAtConnEnds: 2;
export const nudgeOrthogonalTouchingColinearSegments: 3;
export const performUnifyingNudgingPreprocessingStep: 4;
export const improveHyperedgeRoutesMovingAddingAndDeletingJunctions: 5;
export const nudgeSharedPathsWithCommonEndPoint: 6;
export const lastRoutingOptionMarker: 7;
export type RoutingOption = 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7;
export const RoutingOption: {
  readonly nudgeOrthogonalSegmentsConnectedToShapes: 0;
  readonly improveHyperedgeRoutesMovingJunctions: 1;
  readonly penaliseOrthogonalSharedPathsAtConnEnds: 2;
  readonly nudgeOrthogonalTouchingColinearSegments: 3;
  readonly performUnifyingNudgingPreprocessingStep: 4;
  readonly improveHyperedgeRoutesMovingAddingAndDeletingJunctions: 5;
  readonly nudgeSharedPathsWithCommonEndPoint: 6;
  readonly lastRoutingOptionMarker: 7;
};

/** router.h enum TransactionPhases */
export const TransactionPhaseOrthogonalVisibilityGraphScanX: 1;
export const TransactionPhaseOrthogonalVisibilityGraphScanY: 2;
export const TransactionPhaseRouteSearch: 3;
export const TransactionPhaseCrossingDetection: 4;
export const TransactionPhaseRerouteSearch: 5;
export const TransactionPhaseOrthogonalNudgingX: 6;
export const TransactionPhaseOrthogonalNudgingY: 7;
export const TransactionPhaseCompleted: 8;
export type TransactionPhases = 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8;
export const TransactionPhases: {
  readonly TransactionPhaseOrthogonalVisibilityGraphScanX: 1;
  readonly TransactionPhaseOrthogonalVisibilityGraphScanY: 2;
  readonly TransactionPhaseRouteSearch: 3;
  readonly TransactionPhaseCrossingDetection: 4;
  readonly TransactionPhaseRerouteSearch: 5;
  readonly TransactionPhaseOrthogonalNudgingX: 6;
  readonly TransactionPhaseOrthogonalNudgingY: 7;
  readonly TransactionPhaseCompleted: 8;
};

/** router.h */
export const zeroParamValue: 0;
export const chooseSensibleParamValue: -1;

/** connector.h enum ConnType */
export const ConnType_None: 0;
export const ConnType_PolyLine: 1;
export const ConnType_Orthogonal: 2;
export type ConnType = 0 | 1 | 2;
export const ConnType: { readonly ConnType_None: 0; readonly ConnType_PolyLine: 1; readonly ConnType_Orthogonal: 2 };

/** connector.h crossing flags */
export const CROSSING_NONE: 0;
export const CROSSING_TOUCHES: 1;
export const CROSSING_SHARES_PATH: 2;
export const CROSSING_SHARES_PATH_AT_END: 4;
export const CROSSING_SHARES_FIXED_SEGMENT: 8;

/** connend.h enum ConnDirFlag (ConnDirFlags: a combination, 0..15) */
export const ConnDirNone: 0;
export const ConnDirUp: 1;
export const ConnDirDown: 2;
export const ConnDirLeft: 4;
export const ConnDirRight: 8;
export const ConnDirAll: 15;
export type ConnDirFlags = number;
export const ConnDirFlag: {
  readonly ConnDirNone: 0;
  readonly ConnDirUp: 1;
  readonly ConnDirDown: 2;
  readonly ConnDirLeft: 4;
  readonly ConnDirRight: 8;
  readonly ConnDirAll: 15;
};

/** connend.h enum ConnEndType */
export const ConnEndPoint: 0;
export const ConnEndShapePin: 1;
export const ConnEndJunction: 2;
export const ConnEndEmpty: 3;
export type ConnEndType = 0 | 1 | 2 | 3;
export const ConnEndType: { readonly ConnEndPoint: 0; readonly ConnEndShapePin: 1; readonly ConnEndJunction: 2; readonly ConnEndEmpty: 3 };

/** connectionpin.h */
export const CONNECTIONPIN_UNSET: 2147483647;
export const CONNECTIONPIN_CENTRE: 2147483646;
export const ATTACH_POS_TOP: 0;
export const ATTACH_POS_CENTRE: 0.5;
export const ATTACH_POS_BOTTOM: 1;
export const ATTACH_POS_LEFT: 0;
export const ATTACH_POS_RIGHT: 1;
export const ATTACH_POS_MIN_OFFSET: 0;
export const ATTACH_POS_MAX_OFFSET: -1;

/** shape.h enum ShapeTransformationType */
export const TransformationType_CW90: 0;
export const TransformationType_CW180: 1;
export const TransformationType_CW270: 2;
export const TransformationType_FlipX: 3;
export const TransformationType_FlipY: 4;
export type ShapeTransformationType = 0 | 1 | 2 | 3 | 4;
export const ShapeTransformationType: {
  readonly TransformationType_CW90: 0;
  readonly TransformationType_CW180: 1;
  readonly TransformationType_CW270: 2;
  readonly TransformationType_FlipX: 3;
  readonly TransformationType_FlipY: 4;
};

// --- geomtypes.h ------------------------------------------------------------------

/** upstream: class Point (typedef Vector).  `new Point()` is (0, 0). */
export class Point {
  constructor();
  constructor(xv: number, yv: number);
  x: number;
  y: number;
  /** unsigned int */
  id: number;
  /** unsigned short */
  vn: number;
  equals(rhs: Point, epsilon?: number): boolean;
  /** operator== (compares x and y) */
  equal(rhs: Point): boolean;
  /** operator!= */
  notEqual(rhs: Point): boolean;
  /** operator< */
  lessThan(rhs: Point): boolean;
  /** operator[] (read; dimension XDIM or YDIM) */
  get(dimension: number): number;
  /** operator[] (write) */
  set(dimension: number, value: number): void;
  /** operator+ */
  add(rhs: Point): Point;
  /** operator- */
  sub(rhs: Point): Point;
  delete(): void;
}
export type Vector = Point;
export const Vector: typeof Point;

/** upstream: class Box */
export class Box {
  constructor();
  min: Point;
  max: Point;
  length(dimension: number): number;
  width(): number;
  height(): number;
  delete(): void;
}

/** upstream: class Edge */
export class Edge {
  constructor();
  a: Point;
  b: Point;
  delete(): void;
}

/** upstream: router.h struct LineRep */
export class LineRep {
  constructor();
  begin: Point;
  end: Point;
  delete(): void;
}

/** upstream: abstract class PolygonInterface. */
export abstract class PolygonInterface {
  protected constructor();
  abstract clear(): void;
  abstract empty(): boolean;
  abstract size(): number;
  abstract id(): number;
  abstract at(index: number): Point;
  boundingRectPolygon(): Polygon;
  offsetBoundingBox(offset: number): Box;
  offsetPolygon(offset: number): Polygon;
  delete(): void;
}

/** upstream: class Polygon (typedef PolyLine). */
export class Polygon extends PolygonInterface {
  constructor();
  /** `n` default points */
  constructor(n: number);
  /** copies id() and the points */
  constructor(poly: PolygonInterface);
  clear(): void;
  empty(): boolean;
  size(): number;
  id(): number;
  /** the point object in `ps` */
  at(index: number): Point;
  setPoint(index: number, point: Point): void;
  simplify(): Polygon;
  curvedPolyline(curve_amount: number, closed?: boolean): Polygon;
  translate(xDist: number, yDist: number): void;
  checkpointsOnSegment(segmentLowerIndex: number, indexModifier?: number): Point[];
  _id: number;
  ps: Point[];
  /** std::vector<char>: one-character strings ("M", "L", "C", "Z" for curvedPolyline()) */
  ts: string[];
  /** std::vector<std::pair<size_t, Point>> */
  checkpointsOnRoute: [number, Point][];
}
export type PolyLine = Polygon;
export const PolyLine: typeof Polygon;

/**
 * upstream: class ReferencingPolygon.  `psRef[i]` is `[polygon, vn]`:
 * a copy of the referenced obstacle's polygon (taken when this object was
 * made) and the index of the point in it, or `[null, vn]` for a point
 * stored in `psPoints[i]`.
 */
export class ReferencingPolygon extends PolygonInterface {
  constructor();
  constructor(poly: Polygon, router: Router);
  clear(): void;
  empty(): boolean;
  size(): number;
  id(): number;
  at(index: number): Point;
  _id: number;
  psRef: [Polygon | null, number][];
  psPoints: Point[];
}

/** upstream: class Rectangle */
export class Rectangle extends Polygon {
  constructor(topLeft: Point, bottomRight: Point);
  constructor(centre: Point, width: number, height: number);
}

// --- router.h ----------------------------------------------------------------------

/** upstream: class Router.  Subclass to override shouldContinueTransactionWithProgress(). */
export class Router {
  /** `flags`: PolyLineRouting, OrthogonalRouting or both (`|`). */
  constructor(flags: number);

  // public data members
  readonly m_obstacles: Obstacle[];
  readonly connRefs: ConnRef[];
  readonly clusterRefs: ClusterRef[];
  PartialTime: boolean;
  SimpleRouting: boolean;
  ClusteredRouting: boolean;
  IgnoreRegions: boolean;
  UseLeesAlgorithm: boolean;
  InvisibilityGrph: boolean;
  SelectiveReroute: boolean;
  PartialFeedback: boolean;
  RubberBandRouting: boolean;
  st_checked_edges: number;

  setTransactionUse(transactions: boolean): void;
  transactionUse(): boolean;
  processTransaction(): boolean;
  deleteShape(shape: ShapeRef): void;
  moveShape(shape: ShapeRef, newPoly: PolygonInterface, first_move?: boolean): void;
  moveShape(shape: ShapeRef, xDiff: number, yDiff: number): void;
  deleteJunction(junction: JunctionRef): void;
  deleteConnector(connector: ConnRef): void;
  moveJunction(junction: JunctionRef, newPosition: Point): void;
  moveJunction(junction: JunctionRef, xDiff: number, yDiff: number): void;
  setRoutingParameter(parameter: RoutingParameter, value?: number): void;
  routingParameter(parameter: RoutingParameter): number;
  setRoutingOption(option: RoutingOption, value: boolean): void;
  routingOption(option: RoutingOption): boolean;
  setRoutingPenalty(penType: PenaltyType, penVal?: number): void;
  /** The router's HyperedgeRerouter (the same object every time). */
  hyperedgeRerouter(): HyperedgeRerouter;
  /** Not yet available: throws (SVG output is not ported yet). */
  /** Returns the SVG upstream writes to `<filename>.svg`. */
  outputInstanceToSVG(filename?: string): string;
  newObjectId(): number;
  objectIdIsUnused(id: number): boolean;
  /** Virtual: override in a subclass.  Default: true. */
  shouldContinueTransactionWithProgress(elapsedTime: number, phaseNumber: number, totalPhases: number, proportion: number): boolean;
  newAndDeletedObjectListsFromHyperedgeImprovement(): HyperedgeNewAndDeletedObjectLists;
  processActions(): void;
  deleteCluster(cluster: ClusterRef): void;
  /** Appends to `shapes` (an IntList out-parameter). */
  attachedShapes(shapes: number[], shapeId: number, type: number): void;
  /** Appends to `conns` (an IntList out-parameter). */
  attachedConns(conns: number[], shapeId: number, type: number): void;
  markPolylineConnectorsNeedingReroutingForDeletedObstacle(obstacle: Obstacle): void;
  /** Returns the text upstream prints to stdout. */
  printInfo(): string;
  regenerateStaticBuiltGraph(): void;
  destroyOrthogonalVisGraph(): void;
  setStaticGraphInvalidated(invalidated: boolean): void;
  validConnType(select?: ConnType): ConnType;
  isInCrossingPenaltyReroutingStage(): boolean;
  markAllObstaclesAsMoved(): void;
  shapeContainingPoint(point: Point): ShapeRef | null;
  performContinuationCheck(phaseNumber: number, stepNumber: number, totalSteps: number): void;
  registerSettingsChange(): void;
  improveOrthogonalTopology(): void;
  existsOrthogonalSegmentOverlap(atEnds?: boolean): boolean;
  existsOrthogonalFixedSegmentOverlap(atEnds?: boolean): boolean;
  existsOrthogonalTouchingPaths(): boolean;
  existsCrossings(optimisedForConnectorType?: boolean): number;
  existsInvalidOrthogonalPaths(): boolean;
  /** Not yet available: throws (SVG output is not ported yet). */
  /** Returns the SVG upstream writes to `<instanceName>.svg`. */
  outputDiagramSVG(instanceName?: string, lineReps?: LineRep[] | null): string;
  /** Returns the text upstream writes to `<instanceName>.txt`. */
  outputDiagramText(instanceName?: string): string;
  /** Returns the text upstream writes (outputDiagramText; the SVG part exists only in SVG_OUTPUT debug builds). */
  outputDiagram(instanceName?: string): string;
  /** C++ `delete router`. */
  delete(): void;
}

// --- obstacle.h, shape.h, junction.h -------------------------------------------------

/** upstream: class Obstacle (abstract base of ShapeRef and JunctionRef). */
export abstract class Obstacle {
  protected constructor();
  id(): number;
  polygon(): Polygon;
  router(): Router;
  position(): Point;
  setNewPoly(poly: PolygonInterface): void;
  routingBox(): Box;
  routingPolygon(): Polygon;
  attachedConnectors(): ConnRef[];
}

/** upstream: class ShapeRef */
export class ShapeRef extends Obstacle {
  constructor(router: Router, poly: PolygonInterface, id?: number);
  transformConnectionPinPositions(transform: ShapeTransformationType): void;
}

/** upstream: class JunctionRef */
export class JunctionRef extends Obstacle {
  constructor(router: Router, position: Point, id?: number);
  removeJunctionAndMergeConnectors(): ConnRef | null;
  setPositionFixed(fixed: boolean): void;
  positionFixed(): boolean;
  recommendedPosition(): Point;
  makeRectangle(router: Router, position: Point): Rectangle;
  preferOrthogonalDimension(dim: number): void;
}

// --- connend.h, connector.h --------------------------------------------------------

/** upstream: class ConnEnd (a value type) */
export class ConnEnd {
  constructor();
  constructor(point: Point);
  constructor(point: Point, visDirs: ConnDirFlags);
  constructor(shapeRef: ShapeRef, connectionPinClassID: number);
  constructor(junctionRef: JunctionRef);
  type(): ConnEndType;
  position(): Point;
  directions(): ConnDirFlags;
  shape(): ShapeRef | null;
  junction(): JunctionRef | null;
  pinClassId(): number;
  delete(): void;
}

/** upstream: class Checkpoint */
export class Checkpoint {
  constructor();
  constructor(p: Point);
  constructor(p: Point, ad: ConnDirFlags, dd: ConnDirFlags);
  point: Point;
  arrivalDirections: ConnDirFlags;
  departureDirections: ConnDirFlags;
  delete(): void;
}

/** upstream: class ConnRef */
export class ConnRef {
  constructor(router: Router, id?: number);
  constructor(router: Router, src: ConnEnd, dst: ConnEnd, id?: number);
  setEndpoints(srcPoint: ConnEnd, dstPoint: ConnEnd): void;
  setSourceEndpoint(srcPoint: ConnEnd): void;
  setDestEndpoint(dstPoint: ConnEnd): void;
  id(): number;
  router(): Router;
  needsRepaint(): boolean;
  route(): PolyLine;
  displayRoute(): PolyLine;
  /** `cb(ptr)` is invoked when the route changed (see "Callbacks" above); `null` removes it. */
  setCallback<T>(cb: ((ptr: T) => void) | null, ptr: T): void;
  routingType(): ConnType;
  setRoutingType(type: ConnType): void;
  /** std::pair<JunctionRef *, ConnRef *> */
  splitAtSegment(segmentN: number): [JunctionRef | null, ConnRef | null];
  setRoutingCheckpoints(checkpoints: Checkpoint[]): void;
  routingCheckpoints(): Checkpoint[];
  /** std::pair<ConnEnd, ConnEnd> */
  endpointConnEnds(): [ConnEnd, ConnEnd];
  setFixedRoute(route: PolyLine): void;
  setFixedExistingRoute(): void;
  hasFixedRoute(): boolean;
  /**
   * From the libavoid-js fork (no upstream counterpart): false if the last
   * route search found no path and the route is a straight line between
   * the connector's ends.  The route may still cross obstacles; see
   * `hasCrossingObstacles()`.  Meaningful after `processTransaction()`.
   */
  hasValidRoute(): boolean;
  /**
   * From the libavoid-js fork (no upstream counterpart): whether the route
   * crosses, or has a point inside, an obstacle (with its buffer) other
   * than the ones the connector's ends are attached to.  Meaningful after
   * `processTransaction()`.
   */
  hasCrossingObstacles(): boolean;
  clearFixedRoute(): void;
  set_route(route: PolyLine): void;
  calcRouteDist(): void;
  makeActive(): void;
  makeInactive(): void;
  removeFromGraph(): void;
  isInitialised(): boolean;
  makePathInvalid(): void;
  setHateCrossings(value: boolean): void;
  doesHateCrossings(): boolean;
  /** `type`: 1 (VertID::src) or 2 (VertID::tar). */
  setEndpoint(type: 1 | 2, connEnd: ConnEnd): void;
  possibleDstPinPoints(): Point[];
}

// --- connectionpin.h -------------------------------------------------------------

/** upstream: class ShapeConnectionPin */
export class ShapeConnectionPin {
  constructor(shape: ShapeRef, classId: number, xOffset: number, yOffset: number, proportional: boolean, insideOffset: number, visDirs: ConnDirFlags);
  constructor(shape: ShapeRef, classId: number, xOffset: number, yOffset: number, insideOffset: number, visDirs: ConnDirFlags);
  constructor(junction: JunctionRef, classId: number, visDirs?: ConnDirFlags);
  /** C++ `delete pin`. */
  delete(): void;
  setConnectionCost(cost: number): void;
  position(newPoly?: PolygonInterface): Point;
  directions(): ConnDirFlags;
  setExclusive(exclusive: boolean): void;
  isExclusive(): boolean;
  /** ConnectionPinIds: [containing object id, class id] */
  ids(): [number, number];
  /** operator== */
  equal(rhs: ShapeConnectionPin): boolean;
  /** operator< */
  lessThan(rhs: ShapeConnectionPin): boolean;
}

// --- viscluster.h ---------------------------------------------------------------------

/** upstream: class ClusterRef */
export class ClusterRef {
  constructor(router: Router, poly: PolygonInterface, id?: number);
  setNewPoly(poly: PolygonInterface): void;
  id(): number;
  polygon(): ReferencingPolygon;
  rectangularPolygon(): Polygon;
  router(): Router;
  makeActive(): void;
  makeInactive(): void;
}

// --- hyperedge.h -------------------------------------------------------------------------

/** upstream: struct HyperedgeNewAndDeletedObjectLists */
export class HyperedgeNewAndDeletedObjectLists {
  constructor();
  newJunctionList: JunctionRef[];
  newConnectorList: ConnRef[];
  deletedJunctionList: JunctionRef[];
  deletedConnectorList: ConnRef[];
  changedConnectorList: ConnRef[];
  delete(): void;
}

/**
 * upstream: class HyperedgeRerouter.  Use `router.hyperedgeRerouter()`; one
 * made with `new HyperedgeRerouter()` belongs to no router (upstream sets
 * the router privately) and only records registrations.
 */
export class HyperedgeRerouter {
  constructor();
  registerHyperedgeForRerouting(terminals: ConnEnd[]): number;
  registerHyperedgeForRerouting(junction: JunctionRef): number;
  newAndDeletedObjectLists(index: number): HyperedgeNewAndDeletedObjectLists;
  count(): number;
}

// --- geometry.h --------------------------------------------------------------------------

export function euclideanDist(a: Point, b: Point): number;
export function manhattanDist(a: Point, b: Point): number;
export function totalLength(poly: PolygonInterface): number;
export function angle(a: Point, b: Point, c: Point): number;
export function segmentIntersect(a: Point, b: Point, c: Point, d: Point): boolean;
/** `seenIntersectionAtEndpoint`: a one-element array (bool& in/out). */
export function segmentShapeIntersect(e1: Point, e2: Point, s1: Point, s2: Point, seenIntersectionAtEndpoint: [boolean]): boolean;
export function inPoly(poly: PolygonInterface, q: Point, countBorder?: boolean): boolean;
export function inPolyGen(poly: PolygonInterface, q: Point): boolean;
export function inValidRegion(IgnoreRegions: boolean, a0: Point, a1: Point, a2: Point, b: Point): boolean;
export function cornerSide(c1: Point, c2: Point, c3: Point, p: Point): number;
export function pointOnLine(a: Point, b: Point, c: Point, tolerance?: number): boolean;
export function colinear(a: Point, b: Point, c: Point, tolerance?: number): boolean;
export function inBetween(a: Point, b: Point, c: Point): boolean;
export function vecDir(a: Point, b: Point, c: Point, maybeZero?: number): number;
export function projection(a: Point, b: Point, c: Point): Point;
/** `x`, `y`: one-element arrays (double* out-parameters).  Returns DONT_INTERSECT, DO_INTERSECT or PARALLEL. */
export function segmentIntersectPoint(a1: Point, a2: Point, b1: Point, b2: Point, x: [number], y: [number]): number;
/** `x`, `y`: one-element arrays (double* out-parameters).  Returns DONT_INTERSECT, DO_INTERSECT or PARALLEL. */
export function rayIntersectPoint(a1: Point, a2: Point, b1: Point, b2: Point, x: [number], y: [number]): number;
export function rotationalAngle(p: Point): number;

// --- WebAssembly infrastructure (no upstream counterpart) ------------------------------

/** An internal failure of the wasm module (see "WebAssembly infrastructure" above). */
export class AvoidInternalError extends Error {
  /** The API method that failed, e.g. "Router.processTransaction". */
  readonly method: string | null;
  readonly kind: "panic" | "out-of-memory" | "stack-overflow" | "trap" | "poisoned" | "reset";
  readonly panicMessage: string | null;
  readonly location: string | null;
}

/** The error that poisoned the current wasm instance, or null. */
export function internalError(): AvoidInternalError | null;

/** Replaces the wasm instance by a fresh one (Node entry: synchronous). */
export function reset(): void;
