export interface Point {
  x: number;
  y: number;
}

export type Route = Point[];
export type PointInput = Point | readonly [number, number];
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

export type ConnectorEndpoint =
  | { type: "point"; point: Point; position: Point; directions: number }
  | {
      type: "shapePin";
      shapeId: string;
      pinId?: string;
      classId?: number;
      position: Point;
      directions: number;
    }
  | {
      type: "junctionPin";
      junctionId: string;
      pinId?: string;
      classId?: number;
      position: Point;
      directions: number;
    }
  | { type: "junction"; junctionId: string; position: Point; directions: number };

export interface ConnectorEndpointConnEnds {
  source: ConnectorEndpoint;
  destination: ConnectorEndpoint;
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
  classId?: number;
  xOffset: number;
  yOffset: number;
  proportional?: boolean;
  insideOffset?: number;
  visibilityDirections?: number;
  connectionCost?: number;
  exclusive?: boolean;
}

export interface ConnectionPinIds {
  objectId: string;
  classId: number;
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

export class AvoidRouter {
  constructor(clearance?: number);
  static withFlags(flags: number): AvoidRouter;
  routerFlags(): number;
  clear(): void;
  setTransactionUse(transactions: boolean): void;
  transactionUse(): boolean;
  processTransaction(): boolean;
  lastTransactionUpdatedConnectorIds(): string[];
  setRoutingParameter(parameter: number, value: number): boolean;
  setRoutingPenalty(penaltyType: number, penaltyValue?: number): boolean;
  routingParameter(parameter: number): number;
  setRoutingOption(option: number, value: boolean): boolean;
  routingOption(option: number): boolean;
  validConnType(select?: number): number;
  objectIdIsUnused(id: string): boolean;
  addCluster(id: string, points: readonly PointInput[]): boolean;
  addClusterRectangle(id: string, left: number, top: number, right: number, bottom: number): boolean;
  setClusterPolygon(id: string, points: readonly PointInput[]): boolean;
  setClusterRectangle(id: string, left: number, top: number, right: number, bottom: number): boolean;
  removeCluster(id: string): boolean;
  clusterBoundingBox(id: string): Box | null;
  clusterPolygon(id: string): Point[] | null;
  clusterRectangularPolygon(id: string): Point[] | null;
  addShape(id: string, left: number, top: number, right: number, bottom: number): void;
  addPolygonShape(id: string, points: readonly PointInput[]): boolean;
  moveShape(id: string, left: number, top: number, right: number, bottom: number): boolean;
  movePolygonShape(id: string, points: readonly PointInput[]): boolean;
  moveShapeBy(id: string, xDiff: number, yDiff: number): boolean;
  shapeBoundingBox(id: string): Box | null;
  shapePolygon(id: string): Point[] | null;
  shapePosition(id: string): Point | null;
  shapeContainingPoint(point: PointInput): string | null;
  shapeAttachedConnectors(id: string): string[] | null;
  shapeAttachedShapes(id: string, type?: number): string[] | null;
  transformShapeConnectionPinPositions(id: string, transform: number): boolean;
  removeShape(id: string): boolean;
  addJunction(id: string, x: number, y: number): void;
  removeJunction(id: string): boolean;
  removeJunctionAndMergeConnectors(id: string): string | null;
  moveJunction(id: string, x: number, y: number): boolean;
  moveJunctionBy(id: string, xDiff: number, yDiff: number): boolean;
  junctionPosition(id: string): Point | null;
  junctionRecommendedPosition(id: string): Point | null;
  setJunctionRecommendedPosition(id: string, point: PointInput): boolean;
  setJunctionPositionFixed(id: string, fixed: boolean): boolean;
  junctionPositionFixed(id: string): boolean;
  preferJunctionOrthogonalDimension(id: string, dimension: number): boolean;
  junctionPreferredOrthogonalDimension(id: string): number;
  junctionAttachedConnectors(id: string): string[] | null;
  addShapeConnectionPin(shapeId: string, pinId: string, x: number, y: number): boolean;
  addShapeConnectionPinWithOptions(
    shapeId: string,
    pinId: string,
    options: ShapeConnectionPinOptions,
  ): boolean;
  removeShapeConnectionPin(shapeId: string, pinId: string): boolean;
  setShapeConnectionPinConnectionCost(shapeId: string, pinId: string, cost: number): boolean;
  shapeConnectionPinConnectionCost(shapeId: string, pinId: string): number;
  setShapeConnectionPinExclusive(shapeId: string, pinId: string, exclusive: boolean): boolean;
  shapeConnectionPinExclusive(shapeId: string, pinId: string): boolean;
  shapeConnectionPinPosition(shapeId: string, pinId: string): Point | null;
  shapeConnectionPinPositionJson(shapeId: string, pinId: string): string;
  shapeConnectionPinDirections(shapeId: string, pinId: string): number;
  shapeConnectionPinIds(shapeId: string, pinId: string): ConnectionPinIds | null;
  addJunctionConnectionPin(
    junctionId: string,
    pinId: string,
    classId: number,
    visibilityDirections?: number,
  ): boolean;
  removeJunctionConnectionPin(junctionId: string, pinId: string): boolean;
  junctionConnectionPinPosition(junctionId: string, pinId: string): Point | null;
  junctionConnectionPinPositionJson(junctionId: string, pinId: string): string;
  junctionConnectionPinDirections(junctionId: string, pinId: string): number;
  junctionConnectionPinIds(junctionId: string, pinId: string): ConnectionPinIds | null;
  setJunctionConnectionPinConnectionCost(junctionId: string, pinId: string, cost: number): boolean;
  junctionConnectionPinConnectionCost(junctionId: string, pinId: string): number;
  setJunctionConnectionPinExclusive(
    junctionId: string,
    pinId: string,
    exclusive: boolean,
  ): boolean;
  junctionConnectionPinExclusive(junctionId: string, pinId: string): boolean;
  addConnector(id: string, srcX: number, srcY: number, dstX: number, dstY: number): void;
  addConnectorWithEndpointDirections(
    id: string,
    src: PointInput,
    srcDirections: number,
    dst: PointInput,
    dstDirections: number,
  ): void;
  setConnectorEndpoints(id: string, src: PointInput, dst: PointInput): boolean;
  setConnectorEndpointsWithDirections(
    id: string,
    src: PointInput,
    srcDirections: number,
    dst: PointInput,
    dstDirections: number,
  ): boolean;
  setConnectorSourceEndpoint(id: string, point: PointInput): boolean;
  setConnectorSourceEndpointWithDirections(
    id: string,
    point: PointInput,
    directions: number,
  ): boolean;
  setConnectorDestEndpoint(id: string, point: PointInput): boolean;
  setConnectorDestEndpointWithDirections(
    id: string,
    point: PointInput,
    directions: number,
  ): boolean;
  setConnectorSourceJunctionEndpoint(id: string, junctionId: string): boolean;
  setConnectorDestJunctionEndpoint(id: string, junctionId: string): boolean;
  setConnectorSourcePinEndpoint(id: string, shapeId: string, pinId: string): boolean;
  setConnectorDestPinEndpoint(id: string, shapeId: string, pinId: string): boolean;
  setConnectorSourcePinClassEndpoint(id: string, shapeId: string, pinClassId: number): boolean;
  setConnectorDestPinClassEndpoint(id: string, shapeId: string, pinClassId: number): boolean;
  setConnectorSourceJunctionPinEndpoint(
    id: string,
    junctionId: string,
    pinId: string,
  ): boolean;
  setConnectorDestJunctionPinEndpoint(id: string, junctionId: string, pinId: string): boolean;
  setConnectorSourceJunctionPinClassEndpoint(
    id: string,
    junctionId: string,
    pinClassId: number,
  ): boolean;
  setConnectorDestJunctionPinClassEndpoint(
    id: string,
    junctionId: string,
    pinClassId: number,
  ): boolean;
  addConnectorBetweenPins(
    id: string,
    srcShapeId: string,
    srcPinId: string,
    dstShapeId: string,
    dstPinId: string,
  ): boolean;
  addConnectorFromJunction(id: string, srcJunctionId: string, dstX: number, dstY: number): boolean;
  addConnectorToJunction(id: string, srcX: number, srcY: number, dstJunctionId: string): boolean;
  addConnectorBetweenJunctions(
    id: string,
    srcJunctionId: string,
    dstJunctionId: string,
  ): boolean;
  addConnectorBetweenPinClasses(
    id: string,
    srcShapeId: string,
    srcPinClassId: number,
    dstShapeId: string,
    dstPinClassId: number,
  ): boolean;
  addConnectorBetweenJunctionPinClasses(
    id: string,
    srcJunctionId: string,
    srcPinClassId: number,
    dstJunctionId: string,
    dstPinClassId: number,
  ): boolean;
  removeConnector(id: string): boolean;
  setConnectorRoutingType(id: string, routingType: number): boolean;
  setConnectorHateCrossings(id: string, value: boolean): boolean;
  connectorDoesHateCrossings(id: string): boolean;
  connectorRoutingType(id: string): number;
  connectorEndpointConnEnds(id: string): ConnectorEndpointConnEnds | null;
  connectorEndpointConnEndsJson(id: string): string;
  connectorPossibleDstPinPoints(id: string): Point[] | null;
  connectorPossibleDstPinPointsJson(id: string): string;
  setConnectorRoutingCheckpoints(id: string, checkpoints: readonly CheckpointInput[]): boolean;
  connectorRoutingCheckpoints(id: string): Checkpoint[] | null;
  connectorRoutingCheckpointsJson(id: string): string;
  setConnectorFixedRoute(id: string, route: readonly PointInput[]): boolean;
  setConnectorFixedExistingRoute(id: string): boolean;
  connectorHasFixedRoute(id: string): boolean;
  clearConnectorFixedRoute(id: string): boolean;
  makeConnectorPathInvalid(id: string): boolean;
  connectorNeedsRepaint(id: string): boolean;
  existsInvalidOrthogonalPaths(): boolean;
  existsCrossings(optimisedForConnectorType?: boolean): number;
  existsOrthogonalSegmentOverlap(atEnds?: boolean): boolean;
  existsOrthogonalFixedSegmentOverlap(atEnds?: boolean): boolean;
  existsOrthogonalTouchingPaths(): boolean;
  routeConnector(id: string): Route | null;
  routeConnectorJson(id: string): string;
  displayConnectorRoute(id: string): Route | null;
  displayConnectorRouteJson(id: string): string;
  splitConnectorAtSegment(
    id: string,
    segmentN: number,
    junctionId: string,
    connectorId: string,
  ): ConnectorSplitResult | null;
  routeAll(): Record<string, Route>;
  routeAllJson(): string;
  free(): void;
}

export class WasmAvoidRouter {
  constructor(clearance: number);
  static withFlags(flags: number): WasmAvoidRouter;
  routerFlags(): number;
  clear(): void;
  setTransactionUse(transactions: boolean): void;
  transactionUse(): boolean;
  processTransaction(): boolean;
  lastTransactionUpdatedConnectorIds(): string;
  setRoutingParameter(parameter: number, value: number): boolean;
  setRoutingPenalty(penalty_type: number, penalty_value: number): boolean;
  routingParameter(parameter: number): number;
  setRoutingOption(option: number, value: boolean): boolean;
  routingOption(option: number): boolean;
  validConnType(select: number): number;
  objectIdIsUnused(id: string): boolean;
  addCluster(id: string, coordinates: number[]): boolean;
  addClusterRectangle(id: string, left: number, top: number, right: number, bottom: number): boolean;
  setClusterPolygon(id: string, coordinates: number[]): boolean;
  setClusterRectangle(id: string, left: number, top: number, right: number, bottom: number): boolean;
  removeCluster(id: string): boolean;
  clusterBoundingBox(id: string): string;
  clusterPolygon(id: string): string;
  clusterRectangularPolygon(id: string): string;
  addShape(id: string, left: number, top: number, right: number, bottom: number): void;
  addPolygonShape(id: string, coordinates: number[]): boolean;
  moveShape(id: string, left: number, top: number, right: number, bottom: number): boolean;
  movePolygonShape(id: string, coordinates: number[]): boolean;
  moveShapeBy(id: string, x_diff: number, y_diff: number): boolean;
  shapeBoundingBox(id: string): string;
  shapePolygon(id: string): string;
  shapePosition(id: string): string;
  shapeContainingPoint(x: number, y: number): string;
  shapeAttachedConnectors(id: string): string;
  shapeAttachedShapes(id: string, type: number): string;
  transformShapeConnectionPinPositions(id: string, transform: number): boolean;
  removeShape(id: string): boolean;
  addJunction(id: string, x: number, y: number): void;
  removeJunction(id: string): boolean;
  removeJunctionAndMergeConnectors(id: string): string;
  moveJunction(id: string, x: number, y: number): boolean;
  moveJunctionBy(id: string, x_diff: number, y_diff: number): boolean;
  junctionPosition(id: string): string;
  junctionRecommendedPosition(id: string): string;
  setJunctionRecommendedPosition(id: string, x: number, y: number): boolean;
  setJunctionPositionFixed(id: string, fixed: boolean): boolean;
  junctionPositionFixed(id: string): boolean;
  preferJunctionOrthogonalDimension(id: string, dimension: number): boolean;
  junctionPreferredOrthogonalDimension(id: string): number;
  junctionAttachedConnectors(id: string): string;
  addShapeConnectionPin(shape_id: string, pin_id: string, x: number, y: number): boolean;
  addShapeConnectionPinWithOptions(
    shape_id: string,
    pin_id: string,
    class_id: number,
    x_offset: number,
    y_offset: number,
    proportional: boolean,
    inside_offset: number,
    visibility_directions: number,
    connection_cost: number,
    exclusive: boolean,
  ): boolean;
  removeShapeConnectionPin(shape_id: string, pin_id: string): boolean;
  setShapeConnectionPinConnectionCost(shape_id: string, pin_id: string, cost: number): boolean;
  shapeConnectionPinConnectionCost(shape_id: string, pin_id: string): number;
  setShapeConnectionPinExclusive(shape_id: string, pin_id: string, exclusive: boolean): boolean;
  shapeConnectionPinExclusive(shape_id: string, pin_id: string): boolean;
  shapeConnectionPinPosition(shape_id: string, pin_id: string): string;
  shapeConnectionPinDirections(shape_id: string, pin_id: string): number;
  shapeConnectionPinIds(shape_id: string, pin_id: string): string;
  addJunctionConnectionPin(
    junction_id: string,
    pin_id: string,
    class_id: number,
    visibility_directions: number,
  ): boolean;
  removeJunctionConnectionPin(junction_id: string, pin_id: string): boolean;
  junctionConnectionPinPosition(junction_id: string, pin_id: string): string;
  junctionConnectionPinDirections(junction_id: string, pin_id: string): number;
  junctionConnectionPinIds(junction_id: string, pin_id: string): string;
  setJunctionConnectionPinConnectionCost(
    junction_id: string,
    pin_id: string,
    cost: number,
  ): boolean;
  junctionConnectionPinConnectionCost(junction_id: string, pin_id: string): number;
  setJunctionConnectionPinExclusive(
    junction_id: string,
    pin_id: string,
    exclusive: boolean,
  ): boolean;
  junctionConnectionPinExclusive(junction_id: string, pin_id: string): boolean;
  addConnector(id: string, src_x: number, src_y: number, dst_x: number, dst_y: number): void;
  addConnectorWithEndpointDirections(
    id: string,
    src_x: number,
    src_y: number,
    src_directions: number,
    dst_x: number,
    dst_y: number,
    dst_directions: number,
  ): void;
  setConnectorEndpoints(
    id: string,
    src_x: number,
    src_y: number,
    dst_x: number,
    dst_y: number,
  ): boolean;
  setConnectorEndpointsWithDirections(
    id: string,
    src_x: number,
    src_y: number,
    src_directions: number,
    dst_x: number,
    dst_y: number,
    dst_directions: number,
  ): boolean;
  setConnectorSourceEndpoint(id: string, x: number, y: number): boolean;
  setConnectorSourceEndpointWithDirections(
    id: string,
    x: number,
    y: number,
    directions: number,
  ): boolean;
  setConnectorDestEndpoint(id: string, x: number, y: number): boolean;
  setConnectorDestEndpointWithDirections(
    id: string,
    x: number,
    y: number,
    directions: number,
  ): boolean;
  setConnectorSourceJunctionEndpoint(id: string, junction_id: string): boolean;
  setConnectorDestJunctionEndpoint(id: string, junction_id: string): boolean;
  setConnectorSourcePinEndpoint(id: string, shape_id: string, pin_id: string): boolean;
  setConnectorDestPinEndpoint(id: string, shape_id: string, pin_id: string): boolean;
  setConnectorSourcePinClassEndpoint(id: string, shape_id: string, pin_class_id: number): boolean;
  setConnectorDestPinClassEndpoint(id: string, shape_id: string, pin_class_id: number): boolean;
  setConnectorSourceJunctionPinEndpoint(
    id: string,
    junction_id: string,
    pin_id: string,
  ): boolean;
  setConnectorDestJunctionPinEndpoint(id: string, junction_id: string, pin_id: string): boolean;
  setConnectorSourceJunctionPinClassEndpoint(
    id: string,
    junction_id: string,
    pin_class_id: number,
  ): boolean;
  setConnectorDestJunctionPinClassEndpoint(
    id: string,
    junction_id: string,
    pin_class_id: number,
  ): boolean;
  addConnectorBetweenPins(
    id: string,
    src_shape_id: string,
    src_pin_id: string,
    dst_shape_id: string,
    dst_pin_id: string,
  ): boolean;
  addConnectorFromJunction(
    id: string,
    src_junction_id: string,
    dst_x: number,
    dst_y: number,
  ): boolean;
  addConnectorToJunction(
    id: string,
    src_x: number,
    src_y: number,
    dst_junction_id: string,
  ): boolean;
  addConnectorBetweenJunctions(
    id: string,
    src_junction_id: string,
    dst_junction_id: string,
  ): boolean;
  addConnectorBetweenPinClasses(
    id: string,
    src_shape_id: string,
    src_pin_class_id: number,
    dst_shape_id: string,
    dst_pin_class_id: number,
  ): boolean;
  addConnectorBetweenJunctionPinClasses(
    id: string,
    src_junction_id: string,
    src_pin_class_id: number,
    dst_junction_id: string,
    dst_pin_class_id: number,
  ): boolean;
  removeConnector(id: string): boolean;
  setConnectorRoutingType(id: string, routing_type: number): boolean;
  setConnectorHateCrossings(id: string, value: boolean): boolean;
  connectorDoesHateCrossings(id: string): boolean;
  connectorRoutingType(id: string): number;
  connectorEndpointConnEnds(id: string): string;
  connectorPossibleDstPinPoints(id: string): string;
  setConnectorRoutingCheckpoints(id: string, checkpoints: number[]): boolean;
  connectorRoutingCheckpoints(id: string): string;
  setConnectorFixedRoute(id: string, route: number[]): boolean;
  setConnectorFixedExistingRoute(id: string): boolean;
  connectorHasFixedRoute(id: string): boolean;
  clearConnectorFixedRoute(id: string): boolean;
  makeConnectorPathInvalid(id: string): boolean;
  connectorNeedsRepaint(id: string): boolean;
  existsInvalidOrthogonalPaths(): boolean;
  existsCrossings(optimised_for_connector_type: boolean): number;
  existsOrthogonalSegmentOverlap(at_ends: boolean): boolean;
  existsOrthogonalFixedSegmentOverlap(at_ends: boolean): boolean;
  existsOrthogonalTouchingPaths(): boolean;
  routeConnector(id: string): string;
  displayConnectorRoute(id: string): string;
  splitConnectorAtSegment(
    id: string,
    segment_n: number,
    junction_id: string,
    connector_id: string,
  ): string;
  routeAll(): string;
  free(): void;
}
