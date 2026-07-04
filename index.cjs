"use strict";

const wasm = require("./pkg");

const RouterFlag = Object.freeze({
  PolyLineRouting: 1,
  OrthogonalRouting: 2,
});

const ConnType = Object.freeze({
  None: 0,
  PolyLine: 1,
  Orthogonal: 2,
});

const ConnDir = Object.freeze({
  None: 0,
  Up: 1,
  Down: 2,
  Left: 4,
  Right: 8,
  All: 15,
});

const ConnEndType = Object.freeze({
  Point: 0,
  ShapePin: 1,
  Junction: 2,
  Empty: 3,
});

const ConnectorRelation = Object.freeze({
  RunningTo: 1,
  RunningFrom: 2,
  RunningToAndFrom: 3,
});

const RoutingParameter = Object.freeze({
  segmentPenalty: 0,
  anglePenalty: 1,
  crossingPenalty: 2,
  clusterCrossingPenalty: 3,
  fixedSharedPathPenalty: 4,
  portDirectionPenalty: 5,
  shapeBufferDistance: 6,
  idealNudgingDistance: 7,
  reverseDirectionPenalty: 8,
});

const RoutingOption = Object.freeze({
  nudgeOrthogonalSegmentsConnectedToShapes: 0,
  improveHyperedgeRoutesMovingJunctions: 1,
  penaliseOrthogonalSharedPathsAtConnEnds: 2,
  nudgeOrthogonalTouchingColinearSegments: 3,
  performUnifyingNudgingPreprocessingStep: 4,
  improveHyperedgeRoutesMovingAddingAndDeletingJunctions: 5,
  nudgeSharedPathsWithCommonEndPoint: 6,
});

const AttachPos = Object.freeze({
  Top: 0,
  Centre: 0.5,
  Bottom: 1,
  Left: 0,
  Right: 1,
  MinOffset: 0,
  MaxOffset: -1,
});

const ConnectionPin = Object.freeze({
  Unset: 2147483647,
  Centre: 2147483646,
});

const ShapeTransformation = Object.freeze({
  CW90: 0,
  CW180: 1,
  CW270: 2,
  FlipX: 3,
  FlipY: 4,
});

class AvoidRouter {
  #inner;

  static withFlags(flags) {
    return new AvoidRouter(0, wasm.AvoidRouter.withFlags(flags));
  }

  constructor(clearance = 0, inner = null) {
    this.#inner = inner ?? new wasm.AvoidRouter(clearance);
  }

  routerFlags() {
    return this.#inner.routerFlags();
  }

  clear() {
    this.#inner.clear();
  }

  setTransactionUse(transactions) {
    this.#inner.setTransactionUse(transactions);
  }

  transactionUse() {
    return this.#inner.transactionUse();
  }

  processTransaction() {
    return this.#inner.processTransaction();
  }

  lastTransactionUpdatedConnectorIds() {
    return JSON.parse(this.#inner.lastTransactionUpdatedConnectorIds());
  }

  setRoutingParameter(parameter, value) {
    return this.#inner.setRoutingParameter(parameter, value);
  }

  setRoutingPenalty(penaltyType, penaltyValue = -1) {
    return this.#inner.setRoutingPenalty(penaltyType, penaltyValue);
  }

  routingParameter(parameter) {
    return this.#inner.routingParameter(parameter);
  }

  setRoutingOption(option, value) {
    return this.#inner.setRoutingOption(option, value);
  }

  routingOption(option) {
    return this.#inner.routingOption(option);
  }

  validConnType(select = ConnType.None) {
    return this.#inner.validConnType(select);
  }

  objectIdIsUnused(id) {
    return this.#inner.objectIdIsUnused(id);
  }

  addCluster(id, points) {
    return this.#inner.addCluster(id, flattenPoints(points));
  }

  addClusterRectangle(id, left, top, right, bottom) {
    return this.#inner.addClusterRectangle(id, left, top, right, bottom);
  }

  setClusterPolygon(id, points) {
    return this.#inner.setClusterPolygon(id, flattenPoints(points));
  }

  setClusterRectangle(id, left, top, right, bottom) {
    return this.#inner.setClusterRectangle(id, left, top, right, bottom);
  }

  removeCluster(id) {
    return this.#inner.removeCluster(id);
  }

  clusterBoundingBox(id) {
    return JSON.parse(this.#inner.clusterBoundingBox(id));
  }

  clusterPolygon(id) {
    return JSON.parse(this.#inner.clusterPolygon(id));
  }

  clusterRectangularPolygon(id) {
    return JSON.parse(this.#inner.clusterRectangularPolygon(id));
  }

  addShape(id, left, top, right, bottom) {
    this.#inner.addShape(id, left, top, right, bottom);
  }

  addPolygonShape(id, points) {
    return this.#inner.addPolygonShape(id, flattenPoints(points));
  }

  moveShape(id, left, top, right, bottom) {
    return this.#inner.moveShape(id, left, top, right, bottom);
  }

  movePolygonShape(id, points) {
    return this.#inner.movePolygonShape(id, flattenPoints(points));
  }

  moveShapeBy(id, xDiff, yDiff) {
    return this.#inner.moveShapeBy(id, xDiff, yDiff);
  }

  shapeBoundingBox(id) {
    return JSON.parse(this.#inner.shapeBoundingBox(id));
  }

  shapePolygon(id) {
    return JSON.parse(this.#inner.shapePolygon(id));
  }

  shapePosition(id) {
    return JSON.parse(this.#inner.shapePosition(id));
  }

  shapeContainingPoint(point) {
    const [x, y] = pointTuple(point);
    return JSON.parse(this.#inner.shapeContainingPoint(x, y));
  }

  shapeAttachedConnectors(id) {
    return JSON.parse(this.#inner.shapeAttachedConnectors(id));
  }

  shapeAttachedShapes(id, type = ConnectorRelation.RunningToAndFrom) {
    return JSON.parse(this.#inner.shapeAttachedShapes(id, type));
  }

  transformShapeConnectionPinPositions(id, transform) {
    return this.#inner.transformShapeConnectionPinPositions(id, transform);
  }

  removeShape(id) {
    return this.#inner.removeShape(id);
  }

  addJunction(id, x, y) {
    this.#inner.addJunction(id, x, y);
  }

  removeJunction(id) {
    return this.#inner.removeJunction(id);
  }

  removeJunctionAndMergeConnectors(id) {
    return JSON.parse(this.#inner.removeJunctionAndMergeConnectors(id));
  }

  moveJunction(id, x, y) {
    return this.#inner.moveJunction(id, x, y);
  }

  moveJunctionBy(id, xDiff, yDiff) {
    return this.#inner.moveJunctionBy(id, xDiff, yDiff);
  }

  junctionPosition(id) {
    return JSON.parse(this.#inner.junctionPosition(id));
  }

  junctionRecommendedPosition(id) {
    return JSON.parse(this.#inner.junctionRecommendedPosition(id));
  }

  setJunctionRecommendedPosition(id, point) {
    const [x, y] = pointTuple(point);
    return this.#inner.setJunctionRecommendedPosition(id, x, y);
  }

  setJunctionPositionFixed(id, fixed) {
    return this.#inner.setJunctionPositionFixed(id, fixed);
  }

  junctionPositionFixed(id) {
    return this.#inner.junctionPositionFixed(id);
  }

  preferJunctionOrthogonalDimension(id, dimension) {
    return this.#inner.preferJunctionOrthogonalDimension(id, dimension);
  }

  junctionPreferredOrthogonalDimension(id) {
    return this.#inner.junctionPreferredOrthogonalDimension(id);
  }

  junctionAttachedConnectors(id) {
    return JSON.parse(this.#inner.junctionAttachedConnectors(id));
  }

  addShapeConnectionPin(shapeId, pinId, x, y) {
    return this.#inner.addShapeConnectionPin(shapeId, pinId, x, y);
  }

  addShapeConnectionPinWithOptions(shapeId, pinId, options = {}) {
    return this.#inner.addShapeConnectionPinWithOptions(
      shapeId,
      pinId,
      options.classId ?? ConnectionPin.Unset,
      options.xOffset,
      options.yOffset,
      options.proportional ?? false,
      options.insideOffset ?? 0,
      options.visibilityDirections ?? ConnDir.None,
      options.connectionCost ?? Number.NaN,
      options.exclusive ?? false,
    );
  }

  removeShapeConnectionPin(shapeId, pinId) {
    return this.#inner.removeShapeConnectionPin(shapeId, pinId);
  }

  setShapeConnectionPinConnectionCost(shapeId, pinId, cost) {
    return this.#inner.setShapeConnectionPinConnectionCost(shapeId, pinId, cost);
  }

  shapeConnectionPinConnectionCost(shapeId, pinId) {
    return this.#inner.shapeConnectionPinConnectionCost(shapeId, pinId);
  }

  setShapeConnectionPinExclusive(shapeId, pinId, exclusive) {
    return this.#inner.setShapeConnectionPinExclusive(shapeId, pinId, exclusive);
  }

  shapeConnectionPinExclusive(shapeId, pinId) {
    return this.#inner.shapeConnectionPinExclusive(shapeId, pinId);
  }

  shapeConnectionPinPosition(shapeId, pinId) {
    return JSON.parse(this.#inner.shapeConnectionPinPosition(shapeId, pinId));
  }

  shapeConnectionPinPositionJson(shapeId, pinId) {
    return this.#inner.shapeConnectionPinPosition(shapeId, pinId);
  }

  shapeConnectionPinDirections(shapeId, pinId) {
    return this.#inner.shapeConnectionPinDirections(shapeId, pinId);
  }

  shapeConnectionPinIds(shapeId, pinId) {
    return JSON.parse(this.#inner.shapeConnectionPinIds(shapeId, pinId));
  }

  addJunctionConnectionPin(junctionId, pinId, classId, visibilityDirections = ConnDir.All) {
    return this.#inner.addJunctionConnectionPin(junctionId, pinId, classId, visibilityDirections);
  }

  removeJunctionConnectionPin(junctionId, pinId) {
    return this.#inner.removeJunctionConnectionPin(junctionId, pinId);
  }

  junctionConnectionPinPosition(junctionId, pinId) {
    return JSON.parse(this.#inner.junctionConnectionPinPosition(junctionId, pinId));
  }

  junctionConnectionPinPositionJson(junctionId, pinId) {
    return this.#inner.junctionConnectionPinPosition(junctionId, pinId);
  }

  junctionConnectionPinDirections(junctionId, pinId) {
    return this.#inner.junctionConnectionPinDirections(junctionId, pinId);
  }

  junctionConnectionPinIds(junctionId, pinId) {
    return JSON.parse(this.#inner.junctionConnectionPinIds(junctionId, pinId));
  }

  setJunctionConnectionPinConnectionCost(junctionId, pinId, cost) {
    return this.#inner.setJunctionConnectionPinConnectionCost(junctionId, pinId, cost);
  }

  junctionConnectionPinConnectionCost(junctionId, pinId) {
    return this.#inner.junctionConnectionPinConnectionCost(junctionId, pinId);
  }

  setJunctionConnectionPinExclusive(junctionId, pinId, exclusive) {
    return this.#inner.setJunctionConnectionPinExclusive(junctionId, pinId, exclusive);
  }

  junctionConnectionPinExclusive(junctionId, pinId) {
    return this.#inner.junctionConnectionPinExclusive(junctionId, pinId);
  }

  addConnector(id, srcX, srcY, dstX, dstY) {
    this.#inner.addConnector(id, srcX, srcY, dstX, dstY);
  }

  addConnectorWithEndpointDirections(id, src, srcDirections, dst, dstDirections) {
    const [srcX, srcY] = pointTuple(src);
    const [dstX, dstY] = pointTuple(dst);
    this.#inner.addConnectorWithEndpointDirections(
      id,
      srcX,
      srcY,
      srcDirections,
      dstX,
      dstY,
      dstDirections,
    );
  }

  setConnectorEndpoints(id, src, dst) {
    const [srcX, srcY] = pointTuple(src);
    const [dstX, dstY] = pointTuple(dst);
    return this.#inner.setConnectorEndpoints(id, srcX, srcY, dstX, dstY);
  }

  setConnectorEndpointsWithDirections(id, src, srcDirections, dst, dstDirections) {
    const [srcX, srcY] = pointTuple(src);
    const [dstX, dstY] = pointTuple(dst);
    return this.#inner.setConnectorEndpointsWithDirections(
      id,
      srcX,
      srcY,
      srcDirections,
      dstX,
      dstY,
      dstDirections,
    );
  }

  setConnectorSourceEndpoint(id, point) {
    const [x, y] = pointTuple(point);
    return this.#inner.setConnectorSourceEndpoint(id, x, y);
  }

  setConnectorSourceEndpointWithDirections(id, point, directions) {
    const [x, y] = pointTuple(point);
    return this.#inner.setConnectorSourceEndpointWithDirections(id, x, y, directions);
  }

  setConnectorDestEndpoint(id, point) {
    const [x, y] = pointTuple(point);
    return this.#inner.setConnectorDestEndpoint(id, x, y);
  }

  setConnectorDestEndpointWithDirections(id, point, directions) {
    const [x, y] = pointTuple(point);
    return this.#inner.setConnectorDestEndpointWithDirections(id, x, y, directions);
  }

  setConnectorSourceJunctionEndpoint(id, junctionId) {
    return this.#inner.setConnectorSourceJunctionEndpoint(id, junctionId);
  }

  setConnectorDestJunctionEndpoint(id, junctionId) {
    return this.#inner.setConnectorDestJunctionEndpoint(id, junctionId);
  }

  setConnectorSourcePinEndpoint(id, shapeId, pinId) {
    return this.#inner.setConnectorSourcePinEndpoint(id, shapeId, pinId);
  }

  setConnectorDestPinEndpoint(id, shapeId, pinId) {
    return this.#inner.setConnectorDestPinEndpoint(id, shapeId, pinId);
  }

  setConnectorSourcePinClassEndpoint(id, shapeId, pinClassId) {
    return this.#inner.setConnectorSourcePinClassEndpoint(id, shapeId, pinClassId);
  }

  setConnectorDestPinClassEndpoint(id, shapeId, pinClassId) {
    return this.#inner.setConnectorDestPinClassEndpoint(id, shapeId, pinClassId);
  }

  setConnectorSourceJunctionPinEndpoint(id, junctionId, pinId) {
    return this.#inner.setConnectorSourceJunctionPinEndpoint(id, junctionId, pinId);
  }

  setConnectorDestJunctionPinEndpoint(id, junctionId, pinId) {
    return this.#inner.setConnectorDestJunctionPinEndpoint(id, junctionId, pinId);
  }

  setConnectorSourceJunctionPinClassEndpoint(id, junctionId, pinClassId) {
    return this.#inner.setConnectorSourceJunctionPinClassEndpoint(id, junctionId, pinClassId);
  }

  setConnectorDestJunctionPinClassEndpoint(id, junctionId, pinClassId) {
    return this.#inner.setConnectorDestJunctionPinClassEndpoint(id, junctionId, pinClassId);
  }

  addConnectorBetweenPins(id, srcShapeId, srcPinId, dstShapeId, dstPinId) {
    return this.#inner.addConnectorBetweenPins(id, srcShapeId, srcPinId, dstShapeId, dstPinId);
  }

  addConnectorFromJunction(id, srcJunctionId, dstX, dstY) {
    return this.#inner.addConnectorFromJunction(id, srcJunctionId, dstX, dstY);
  }

  addConnectorToJunction(id, srcX, srcY, dstJunctionId) {
    return this.#inner.addConnectorToJunction(id, srcX, srcY, dstJunctionId);
  }

  addConnectorBetweenJunctions(id, srcJunctionId, dstJunctionId) {
    return this.#inner.addConnectorBetweenJunctions(id, srcJunctionId, dstJunctionId);
  }

  addConnectorBetweenPinClasses(id, srcShapeId, srcPinClassId, dstShapeId, dstPinClassId) {
    return this.#inner.addConnectorBetweenPinClasses(
      id,
      srcShapeId,
      srcPinClassId,
      dstShapeId,
      dstPinClassId,
    );
  }

  addConnectorBetweenJunctionPinClasses(
    id,
    srcJunctionId,
    srcPinClassId,
    dstJunctionId,
    dstPinClassId,
  ) {
    return this.#inner.addConnectorBetweenJunctionPinClasses(
      id,
      srcJunctionId,
      srcPinClassId,
      dstJunctionId,
      dstPinClassId,
    );
  }

  removeConnector(id) {
    return this.#inner.removeConnector(id);
  }

  setConnectorRoutingType(id, routingType) {
    return this.#inner.setConnectorRoutingType(id, routingType);
  }

  setConnectorHateCrossings(id, value) {
    return this.#inner.setConnectorHateCrossings(id, value);
  }

  connectorDoesHateCrossings(id) {
    return this.#inner.connectorDoesHateCrossings(id);
  }

  connectorRoutingType(id) {
    return this.#inner.connectorRoutingType(id);
  }

  connectorEndpointConnEnds(id) {
    return JSON.parse(this.#inner.connectorEndpointConnEnds(id));
  }

  connectorEndpointConnEndsJson(id) {
    return this.#inner.connectorEndpointConnEnds(id);
  }

  connectorPossibleDstPinPoints(id) {
    return JSON.parse(this.#inner.connectorPossibleDstPinPoints(id));
  }

  connectorPossibleDstPinPointsJson(id) {
    return this.#inner.connectorPossibleDstPinPoints(id);
  }

  setConnectorRoutingCheckpoints(id, checkpoints) {
    return this.#inner.setConnectorRoutingCheckpoints(id, flattenCheckpoints(checkpoints));
  }

  connectorRoutingCheckpoints(id) {
    return JSON.parse(this.#inner.connectorRoutingCheckpoints(id));
  }

  connectorRoutingCheckpointsJson(id) {
    return this.#inner.connectorRoutingCheckpoints(id);
  }

  setConnectorFixedRoute(id, route) {
    return this.#inner.setConnectorFixedRoute(id, flattenPoints(route));
  }

  setConnectorFixedExistingRoute(id) {
    return this.#inner.setConnectorFixedExistingRoute(id);
  }

  connectorHasFixedRoute(id) {
    return this.#inner.connectorHasFixedRoute(id);
  }

  clearConnectorFixedRoute(id) {
    return this.#inner.clearConnectorFixedRoute(id);
  }

  makeConnectorPathInvalid(id) {
    return this.#inner.makeConnectorPathInvalid(id);
  }

  connectorNeedsRepaint(id) {
    return this.#inner.connectorNeedsRepaint(id);
  }

  existsInvalidOrthogonalPaths() {
    return this.#inner.existsInvalidOrthogonalPaths();
  }

  existsCrossings(optimisedForConnectorType = false) {
    return this.#inner.existsCrossings(optimisedForConnectorType);
  }

  existsOrthogonalSegmentOverlap(atEnds = false) {
    return this.#inner.existsOrthogonalSegmentOverlap(atEnds);
  }

  existsOrthogonalFixedSegmentOverlap(atEnds = false) {
    return this.#inner.existsOrthogonalFixedSegmentOverlap(atEnds);
  }

  existsOrthogonalTouchingPaths() {
    return this.#inner.existsOrthogonalTouchingPaths();
  }

  routeConnector(id) {
    return JSON.parse(this.#inner.routeConnector(id));
  }

  routeConnectorJson(id) {
    return this.#inner.routeConnector(id);
  }

  displayConnectorRoute(id) {
    return JSON.parse(this.#inner.displayConnectorRoute(id));
  }

  displayConnectorRouteJson(id) {
    return this.#inner.displayConnectorRoute(id);
  }

  splitConnectorAtSegment(id, segmentN, junctionId, connectorId) {
    return JSON.parse(this.#inner.splitConnectorAtSegment(id, segmentN, junctionId, connectorId));
  }

  routeAll() {
    return JSON.parse(this.#inner.routeAll());
  }

  routeAllJson() {
    return this.#inner.routeAll();
  }

  free() {
    this.#inner.free();
  }
}

module.exports = {
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
  WasmAvoidRouter: wasm.AvoidRouter,
};

function flattenPoints(points) {
  if (!Array.isArray(points)) {
    throw new TypeError("points must be an array");
  }

  const flat = [];
  for (const point of points) {
    if (Array.isArray(point)) {
      flat.push(point[0], point[1]);
    } else if (point && typeof point === "object") {
      flat.push(point.x, point.y);
    } else {
      throw new TypeError("each point must be [x, y] or { x, y }");
    }
  }
  return flat;
}

function pointTuple(point) {
  if (Array.isArray(point)) {
    return [point[0], point[1]];
  }
  if (point && typeof point === "object") {
    return [point.x, point.y];
  }
  throw new TypeError("point must be [x, y] or { x, y }");
}

function flattenCheckpoints(checkpoints) {
  if (!Array.isArray(checkpoints)) {
    throw new TypeError("checkpoints must be an array");
  }

  const flat = [];
  for (const checkpoint of checkpoints) {
    if (Array.isArray(checkpoint)) {
      flat.push(checkpoint[0], checkpoint[1], ConnDir.All, ConnDir.All);
    } else if (checkpoint && typeof checkpoint === "object") {
      const point = checkpoint.point ?? checkpoint;
      if (Array.isArray(point)) {
        flat.push(
          point[0],
          point[1],
          checkpoint.arrivalDirections ?? ConnDir.All,
          checkpoint.departureDirections ?? ConnDir.All,
        );
      } else if (point && typeof point === "object") {
        flat.push(
          point.x,
          point.y,
          checkpoint.arrivalDirections ?? ConnDir.All,
          checkpoint.departureDirections ?? ConnDir.All,
        );
      } else {
        throw new TypeError("checkpoint point must be [x, y] or { x, y }");
      }
    } else {
      throw new TypeError("each checkpoint must be [x, y], { x, y }, or { point }");
    }
  }
  return flat;
}
