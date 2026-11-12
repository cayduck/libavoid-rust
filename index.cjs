"use strict";

// JavaScript API of libavoid-rust: a string-id adapter over the faithful
// Rust port of upstream libavoid (see index.d.ts for the documentation of
// every method and of the id / error conventions).
//
// Error convention:
//   * structurally malformed arguments (an id that is not a string, a point
//     that is neither [x, y] nor {x, y}, a non-array where an array is
//     expected, a non-boolean flag) throw a TypeError here;
//   * everything else is validated by the wasm module, which never aborts:
//     mutators return false and queries return null for unknown ids,
//     duplicate ids, non-finite numbers, out-of-range enum values and
//     violated upstream preconditions;
//   * the constructor throws an Error for invalid router flags.

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

// upstream: typedef enum RoutingParameter PenaltyType
const PenaltyType = RoutingParameter;

const RoutingParameterValue = Object.freeze({
  zero: 0,
  chooseSensible: -1,
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

const OrthogonalDimension = Object.freeze({
  X: 0,
  Y: 1,
});

class AvoidRouter {
  #inner;

  static withFlags(flags) {
    return new AvoidRouter(flags);
  }

  constructor(flags = RouterFlag.OrthogonalRouting) {
    if (typeof flags !== "number") {
      throw new TypeError("AvoidRouter: flags must be a number (RouterFlag)");
    }
    this.#inner = new wasm.AvoidRouter(flags);
  }

  // --- router ---------------------------------------------------------------

  routerFlags() {
    return this.#inner.routerFlags();
  }

  clear() {
    this.#inner.clear();
  }

  setTransactionUse(transactions) {
    this.#inner.setTransactionUse(bool(transactions, "transactions"));
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
    return this.#inner.setRoutingParameter(num(parameter, "parameter"), optNum(value, "value"));
  }

  setRoutingPenalty(penaltyType, penaltyValue) {
    return this.#inner.setRoutingPenalty(num(penaltyType, "penaltyType"), optNum(penaltyValue, "penaltyValue"));
  }

  routingParameter(parameter) {
    return nullable(this.#inner.routingParameter(num(parameter, "parameter")));
  }

  setRoutingOption(option, value) {
    return this.#inner.setRoutingOption(num(option, "option"), bool(value, "value"));
  }

  routingOption(option) {
    return nullable(this.#inner.routingOption(num(option, "option")));
  }

  validConnType(select = ConnType.None) {
    return nullable(this.#inner.validConnType(num(select, "select")));
  }

  // --- ids ------------------------------------------------------------------

  objectIdIsUnused(id) {
    if (typeof id === "number") {
      return nullable(this.#inner.numericObjectIdIsUnused(id));
    }
    return this.#inner.objectIdIsUnused(str(id, "id"));
  }

  newObjectId() {
    return this.#inner.newObjectId();
  }

  objectNumericId(id) {
    return nullable(this.#inner.objectNumericId(str(id, "id")));
  }

  objectIdFromNumericId(numericId) {
    return JSON.parse(this.#inner.objectIdFromNumericId(num(numericId, "numericId")));
  }

  shapeIds() {
    return JSON.parse(this.#inner.shapeIds());
  }

  junctionIds() {
    return JSON.parse(this.#inner.junctionIds());
  }

  connectorIds() {
    return JSON.parse(this.#inner.connectorIds());
  }

  clusterIds() {
    return JSON.parse(this.#inner.clusterIds());
  }

  // --- clusters -------------------------------------------------------------

  addCluster(id, points, numericId) {
    return this.#inner.addCluster(str(id, "id"), flattenPoints(points), optNum(numericId, "numericId"));
  }

  addClusterRectangle(id, left, top, right, bottom, numericId) {
    return this.#inner.addClusterRectangle(
      str(id, "id"),
      num(left, "left"),
      num(top, "top"),
      num(right, "right"),
      num(bottom, "bottom"),
      optNum(numericId, "numericId"),
    );
  }

  setClusterPolygon(id, points) {
    return this.#inner.setClusterPolygon(str(id, "id"), flattenPoints(points));
  }

  setClusterRectangle(id, left, top, right, bottom) {
    return this.#inner.setClusterRectangle(
      str(id, "id"),
      num(left, "left"),
      num(top, "top"),
      num(right, "right"),
      num(bottom, "bottom"),
    );
  }

  removeCluster(id) {
    return this.#inner.removeCluster(str(id, "id"));
  }

  clusterBoundingBox(id) {
    return JSON.parse(this.#inner.clusterBoundingBox(str(id, "id")));
  }

  clusterPolygon(id) {
    return JSON.parse(this.#inner.clusterPolygon(str(id, "id")));
  }

  clusterRectangularPolygon(id) {
    return JSON.parse(this.#inner.clusterRectangularPolygon(str(id, "id")));
  }

  // --- shapes ---------------------------------------------------------------

  addShape(id, left, top, right, bottom, numericId) {
    return this.#inner.addShape(
      str(id, "id"),
      num(left, "left"),
      num(top, "top"),
      num(right, "right"),
      num(bottom, "bottom"),
      optNum(numericId, "numericId"),
    );
  }

  addPolygonShape(id, points, numericId) {
    return this.#inner.addPolygonShape(str(id, "id"), flattenPoints(points), optNum(numericId, "numericId"));
  }

  moveShape(id, left, top, right, bottom) {
    return this.#inner.moveShape(str(id, "id"), num(left, "left"), num(top, "top"), num(right, "right"), num(bottom, "bottom"));
  }

  movePolygonShape(id, points, firstMove) {
    return this.#inner.movePolygonShape(str(id, "id"), flattenPoints(points), optBool(firstMove, "firstMove"));
  }

  moveShapeBy(id, xDiff, yDiff) {
    return this.#inner.moveShapeBy(str(id, "id"), num(xDiff, "xDiff"), num(yDiff, "yDiff"));
  }

  shapeBoundingBox(id) {
    return JSON.parse(this.#inner.shapeBoundingBox(str(id, "id")));
  }

  shapeRoutingBox(id) {
    return JSON.parse(this.#inner.shapeRoutingBox(str(id, "id")));
  }

  shapePolygon(id) {
    return JSON.parse(this.#inner.shapePolygon(str(id, "id")));
  }

  shapeRoutingPolygon(id) {
    return JSON.parse(this.#inner.shapeRoutingPolygon(str(id, "id")));
  }

  shapePosition(id) {
    return JSON.parse(this.#inner.shapePosition(str(id, "id")));
  }

  shapeContainingPoint(point) {
    const [x, y] = pointTuple(point);
    return JSON.parse(this.#inner.shapeContainingPoint(x, y));
  }

  shapeAttachedConnectors(id) {
    return JSON.parse(this.#inner.shapeAttachedConnectors(str(id, "id")));
  }

  shapeAttachedShapes(id, type = ConnectorRelation.RunningToAndFrom) {
    return JSON.parse(this.#inner.shapeAttachedShapes(str(id, "id"), num(type, "type")));
  }

  shapeAttachedConns(id, type = ConnectorRelation.RunningToAndFrom) {
    return JSON.parse(this.#inner.shapeAttachedConns(str(id, "id"), num(type, "type")));
  }

  transformShapeConnectionPinPositions(id, transform) {
    return this.#inner.transformShapeConnectionPinPositions(str(id, "id"), num(transform, "transform"));
  }

  removeShape(id) {
    return this.#inner.removeShape(str(id, "id"));
  }

  markAllObstaclesAsMoved() {
    this.#inner.markAllObstaclesAsMoved();
  }

  // --- junctions ------------------------------------------------------------

  addJunction(id, x, y, numericId) {
    return this.#inner.addJunction(str(id, "id"), num(x, "x"), num(y, "y"), optNum(numericId, "numericId"));
  }

  removeJunction(id) {
    return this.#inner.removeJunction(str(id, "id"));
  }

  removeJunctionAndMergeConnectors(id) {
    return JSON.parse(this.#inner.removeJunctionAndMergeConnectors(str(id, "id")));
  }

  moveJunction(id, x, y) {
    return this.#inner.moveJunction(str(id, "id"), num(x, "x"), num(y, "y"));
  }

  moveJunctionBy(id, xDiff, yDiff) {
    return this.#inner.moveJunctionBy(str(id, "id"), num(xDiff, "xDiff"), num(yDiff, "yDiff"));
  }

  junctionPosition(id) {
    return JSON.parse(this.#inner.junctionPosition(str(id, "id")));
  }

  junctionRecommendedPosition(id) {
    return JSON.parse(this.#inner.junctionRecommendedPosition(str(id, "id")));
  }

  setJunctionPositionFixed(id, fixed) {
    return this.#inner.setJunctionPositionFixed(str(id, "id"), bool(fixed, "fixed"));
  }

  junctionPositionFixed(id) {
    return nullable(this.#inner.junctionPositionFixed(str(id, "id")));
  }

  preferJunctionOrthogonalDimension(id, dimension) {
    return this.#inner.preferJunctionOrthogonalDimension(str(id, "id"), num(dimension, "dimension"));
  }

  junctionAttachedConnectors(id) {
    return JSON.parse(this.#inner.junctionAttachedConnectors(str(id, "id")));
  }

  // --- connection pins ------------------------------------------------------

  addShapeConnectionPin(
    shapeId,
    pinId,
    classId,
    xOffset,
    yOffset,
    proportional,
    insideOffset,
    visibilityDirections,
  ) {
    return this.#inner.addShapeConnectionPin(
      str(shapeId, "shapeId"),
      str(pinId, "pinId"),
      num(classId, "classId"),
      num(xOffset, "xOffset"),
      num(yOffset, "yOffset"),
      optBool(proportional, "proportional"),
      optNum(insideOffset, "insideOffset"),
      optNum(visibilityDirections, "visibilityDirections"),
    );
  }

  addShapeConnectionPinWithOptions(shapeId, pinId, options) {
    if (!options || typeof options !== "object") {
      throw new TypeError("options must be an object");
    }
    return this.#inner.addShapeConnectionPinWithOptions(
      str(shapeId, "shapeId"),
      str(pinId, "pinId"),
      num(options.classId, "options.classId"),
      num(options.xOffset, "options.xOffset"),
      num(options.yOffset, "options.yOffset"),
      optBool(options.proportional, "options.proportional"),
      optNum(options.insideOffset, "options.insideOffset"),
      optNum(options.visibilityDirections, "options.visibilityDirections"),
      optNum(options.connectionCost, "options.connectionCost"),
      optBool(options.exclusive, "options.exclusive"),
    );
  }

  removeShapeConnectionPin(shapeId, pinId) {
    return this.#inner.removeShapeConnectionPin(str(shapeId, "shapeId"), str(pinId, "pinId"));
  }

  shapeConnectionPinIdList(shapeId) {
    return JSON.parse(this.#inner.shapeConnectionPinIdList(str(shapeId, "shapeId")));
  }

  setShapeConnectionPinConnectionCost(shapeId, pinId, cost) {
    return this.#inner.setShapeConnectionPinConnectionCost(str(shapeId, "shapeId"), str(pinId, "pinId"), num(cost, "cost"));
  }

  setShapeConnectionPinExclusive(shapeId, pinId, exclusive) {
    return this.#inner.setShapeConnectionPinExclusive(
      str(shapeId, "shapeId"),
      str(pinId, "pinId"),
      bool(exclusive, "exclusive"),
    );
  }

  shapeConnectionPinExclusive(shapeId, pinId) {
    return nullable(this.#inner.shapeConnectionPinExclusive(str(shapeId, "shapeId"), str(pinId, "pinId")));
  }

  shapeConnectionPinPosition(shapeId, pinId) {
    return JSON.parse(this.#inner.shapeConnectionPinPosition(str(shapeId, "shapeId"), str(pinId, "pinId")));
  }

  shapeConnectionPinDirections(shapeId, pinId) {
    return nullable(this.#inner.shapeConnectionPinDirections(str(shapeId, "shapeId"), str(pinId, "pinId")));
  }

  shapeConnectionPinIds(shapeId, pinId) {
    return JSON.parse(this.#inner.shapeConnectionPinIds(str(shapeId, "shapeId"), str(pinId, "pinId")));
  }

  addJunctionConnectionPin(junctionId, pinId, classId, visibilityDirections) {
    return this.#inner.addJunctionConnectionPin(
      str(junctionId, "junctionId"),
      str(pinId, "pinId"),
      num(classId, "classId"),
      optNum(visibilityDirections, "visibilityDirections"),
    );
  }

  removeJunctionConnectionPin(junctionId, pinId) {
    return this.#inner.removeJunctionConnectionPin(str(junctionId, "junctionId"), str(pinId, "pinId"));
  }

  junctionConnectionPinPosition(junctionId, pinId) {
    return JSON.parse(this.#inner.junctionConnectionPinPosition(str(junctionId, "junctionId"), str(pinId, "pinId")));
  }

  junctionConnectionPinDirections(junctionId, pinId) {
    return nullable(this.#inner.junctionConnectionPinDirections(str(junctionId, "junctionId"), str(pinId, "pinId")));
  }

  junctionConnectionPinIds(junctionId, pinId) {
    return JSON.parse(this.#inner.junctionConnectionPinIds(str(junctionId, "junctionId"), str(pinId, "pinId")));
  }

  setJunctionConnectionPinConnectionCost(junctionId, pinId, cost) {
    return this.#inner.setJunctionConnectionPinConnectionCost(
      str(junctionId, "junctionId"),
      str(pinId, "pinId"),
      num(cost, "cost"),
    );
  }

  setJunctionConnectionPinExclusive(junctionId, pinId, exclusive) {
    return this.#inner.setJunctionConnectionPinExclusive(
      str(junctionId, "junctionId"),
      str(pinId, "pinId"),
      bool(exclusive, "exclusive"),
    );
  }

  junctionConnectionPinExclusive(junctionId, pinId) {
    return nullable(this.#inner.junctionConnectionPinExclusive(str(junctionId, "junctionId"), str(pinId, "pinId")));
  }

  // --- connectors: creation -------------------------------------------------

  addConnectorWithoutEndpoints(id, numericId) {
    return this.#inner.addConnectorWithoutEndpoints(str(id, "id"), optNum(numericId, "numericId"));
  }

  addConnectorWithConnEnds(id, src, dst, numericId) {
    return this.#inner.addConnectorWithConnEnds(
      str(id, "id"),
      ...connEndArgs(src),
      ...connEndArgs(dst),
      optNum(numericId, "numericId"),
    );
  }

  addConnector(id, srcX, srcY, dstX, dstY, numericId) {
    return this.addConnectorWithConnEnds(id, [srcX, srcY], [dstX, dstY], numericId);
  }

  addConnectorWithEndpointDirections(id, src, srcDirections, dst, dstDirections, numericId) {
    return this.addConnectorWithConnEnds(
      id,
      { point: src, directions: srcDirections },
      { point: dst, directions: dstDirections },
      numericId,
    );
  }

  addConnectorBetweenPins(id, srcShapeId, srcPinId, dstShapeId, dstPinId, numericId) {
    return this.addConnectorWithConnEnds(
      id,
      { shapeId: srcShapeId, pinId: srcPinId },
      { shapeId: dstShapeId, pinId: dstPinId },
      numericId,
    );
  }

  addConnectorBetweenPinClasses(id, srcShapeId, srcPinClassId, dstShapeId, dstPinClassId, numericId) {
    return this.addConnectorWithConnEnds(
      id,
      { shapeId: srcShapeId, classId: srcPinClassId },
      { shapeId: dstShapeId, classId: dstPinClassId },
      numericId,
    );
  }

  addConnectorFromJunction(id, srcJunctionId, dstX, dstY, numericId) {
    return this.addConnectorWithConnEnds(id, { junctionId: srcJunctionId }, [dstX, dstY], numericId);
  }

  addConnectorToJunction(id, srcX, srcY, dstJunctionId, numericId) {
    return this.addConnectorWithConnEnds(id, [srcX, srcY], { junctionId: dstJunctionId }, numericId);
  }

  addConnectorBetweenJunctions(id, srcJunctionId, dstJunctionId, numericId) {
    return this.addConnectorWithConnEnds(id, { junctionId: srcJunctionId }, { junctionId: dstJunctionId }, numericId);
  }

  // --- connectors: endpoints ------------------------------------------------

  setConnectorConnEnds(id, src, dst) {
    return this.#inner.setConnectorConnEnds(str(id, "id"), ...connEndArgs(src), ...connEndArgs(dst));
  }

  setConnectorSourceConnEnd(id, end) {
    return this.#inner.setConnectorSourceConnEnd(str(id, "id"), ...connEndArgs(end));
  }

  setConnectorDestConnEnd(id, end) {
    return this.#inner.setConnectorDestConnEnd(str(id, "id"), ...connEndArgs(end));
  }

  setConnectorEndpoints(id, src, dst) {
    return this.setConnectorConnEnds(id, pointEnd(src), pointEnd(dst));
  }

  setConnectorEndpointsWithDirections(id, src, srcDirections, dst, dstDirections) {
    return this.setConnectorConnEnds(
      id,
      { point: src, directions: srcDirections },
      { point: dst, directions: dstDirections },
    );
  }

  setConnectorSourceEndpoint(id, point) {
    return this.setConnectorSourceConnEnd(id, pointEnd(point));
  }

  setConnectorSourceEndpointWithDirections(id, point, directions) {
    return this.setConnectorSourceConnEnd(id, { point, directions });
  }

  setConnectorDestEndpoint(id, point) {
    return this.setConnectorDestConnEnd(id, pointEnd(point));
  }

  setConnectorDestEndpointWithDirections(id, point, directions) {
    return this.setConnectorDestConnEnd(id, { point, directions });
  }

  setConnectorSourceJunctionEndpoint(id, junctionId) {
    return this.setConnectorSourceConnEnd(id, { junctionId });
  }

  setConnectorDestJunctionEndpoint(id, junctionId) {
    return this.setConnectorDestConnEnd(id, { junctionId });
  }

  setConnectorSourcePinEndpoint(id, shapeId, pinId) {
    return this.setConnectorSourceConnEnd(id, { shapeId, pinId });
  }

  setConnectorDestPinEndpoint(id, shapeId, pinId) {
    return this.setConnectorDestConnEnd(id, { shapeId, pinId });
  }

  setConnectorSourcePinClassEndpoint(id, shapeId, pinClassId) {
    return this.setConnectorSourceConnEnd(id, { shapeId, classId: pinClassId });
  }

  setConnectorDestPinClassEndpoint(id, shapeId, pinClassId) {
    return this.setConnectorDestConnEnd(id, { shapeId, classId: pinClassId });
  }

  removeConnector(id) {
    return this.#inner.removeConnector(str(id, "id"));
  }

  // --- connectors: properties -----------------------------------------------

  setConnectorRoutingType(id, routingType) {
    return this.#inner.setConnectorRoutingType(str(id, "id"), num(routingType, "routingType"));
  }

  connectorRoutingType(id) {
    return nullable(this.#inner.connectorRoutingType(str(id, "id")));
  }

  setConnectorHateCrossings(id, value) {
    return this.#inner.setConnectorHateCrossings(str(id, "id"), bool(value, "value"));
  }

  connectorDoesHateCrossings(id) {
    return nullable(this.#inner.connectorDoesHateCrossings(str(id, "id")));
  }

  connectorEndpointConnEnds(id) {
    return JSON.parse(this.#inner.connectorEndpointConnEnds(str(id, "id")));
  }

  connectorPossibleDstPinPoints(id) {
    return JSON.parse(this.#inner.connectorPossibleDstPinPoints(str(id, "id")));
  }

  setConnectorRoutingCheckpoints(id, checkpoints) {
    return this.#inner.setConnectorRoutingCheckpoints(str(id, "id"), flattenCheckpoints(checkpoints));
  }

  connectorRoutingCheckpoints(id) {
    return JSON.parse(this.#inner.connectorRoutingCheckpoints(str(id, "id")));
  }

  setConnectorFixedRoute(id, route) {
    return this.#inner.setConnectorFixedRoute(str(id, "id"), flattenPoints(route));
  }

  setConnectorFixedExistingRoute(id) {
    return this.#inner.setConnectorFixedExistingRoute(str(id, "id"));
  }

  connectorHasFixedRoute(id) {
    return nullable(this.#inner.connectorHasFixedRoute(str(id, "id")));
  }

  clearConnectorFixedRoute(id) {
    return this.#inner.clearConnectorFixedRoute(str(id, "id"));
  }

  makeConnectorPathInvalid(id) {
    return this.#inner.makeConnectorPathInvalid(str(id, "id"));
  }

  connectorNeedsRepaint(id) {
    return nullable(this.#inner.connectorNeedsRepaint(str(id, "id")));
  }

  routeConnector(id) {
    return JSON.parse(this.#inner.routeConnector(str(id, "id")));
  }

  displayConnectorRoute(id) {
    return JSON.parse(this.#inner.displayConnectorRoute(str(id, "id")));
  }

  connectorCurvedRoute(id, curveAmount, closed) {
    return JSON.parse(this.#inner.connectorCurvedRoute(str(id, "id"), num(curveAmount, "curveAmount"), optBool(closed, "closed")));
  }

  splitConnectorAtSegment(id, segmentN, junctionId, connectorId) {
    return JSON.parse(
      this.#inner.splitConnectorAtSegment(
        str(id, "id"),
        num(segmentN, "segmentN"),
        str(junctionId, "junctionId"),
        str(connectorId, "connectorId"),
      ),
    );
  }

  routeAll() {
    return JSON.parse(this.#inner.routeAll());
  }

  // --- diagnostics ----------------------------------------------------------

  existsInvalidOrthogonalPaths() {
    return this.#inner.existsInvalidOrthogonalPaths();
  }

  existsCrossings(optimisedForConnectorType = false) {
    return this.#inner.existsCrossings(bool(optimisedForConnectorType, "optimisedForConnectorType"));
  }

  existsOrthogonalSegmentOverlap(atEnds = false) {
    return this.#inner.existsOrthogonalSegmentOverlap(bool(atEnds, "atEnds"));
  }

  existsOrthogonalFixedSegmentOverlap(atEnds = false) {
    return this.#inner.existsOrthogonalFixedSegmentOverlap(bool(atEnds, "atEnds"));
  }

  existsOrthogonalTouchingPaths() {
    return this.#inner.existsOrthogonalTouchingPaths();
  }

  outputDiagramText(fullPrecision = false) {
    return this.#inner.outputDiagramText(bool(fullPrecision, "fullPrecision"));
  }

  printInfo() {
    return this.#inner.printInfo();
  }

  // --- hyperedges -----------------------------------------------------------

  registerHyperedgeForReroutingJunction(junctionId) {
    return nullable(this.#inner.registerHyperedgeForReroutingJunction(str(junctionId, "junctionId")));
  }

  registerHyperedgeForRerouting(terminals) {
    if (!Array.isArray(terminals)) {
      throw new TypeError("terminals must be an array of ConnEnds");
    }
    const cols = [[], [], [], [], [], []];
    for (const end of terminals) {
      connEndArgs(end).forEach((v, i) => cols[i].push(v));
    }
    return nullable(this.#inner.registerHyperedgeForRerouting(...cols));
  }

  hyperedgeReroutingCount() {
    return this.#inner.hyperedgeReroutingCount();
  }

  hyperedgeNewAndDeletedObjectLists(index) {
    return JSON.parse(this.#inner.hyperedgeNewAndDeletedObjectLists(num(index, "index")));
  }

  newAndDeletedObjectListsFromHyperedgeImprovement() {
    return JSON.parse(this.#inner.newAndDeletedObjectListsFromHyperedgeImprovement());
  }

  free() {
    this.#inner.free();
  }
}

function curvedPolyline(points, curveAmount, closed) {
  return JSON.parse(wasm.curvedPolyline(flattenPoints(points), num(curveAmount, "curveAmount"), optBool(closed, "closed")));
}

module.exports = {
  AttachPos,
  AvoidRouter,
  ConnDir,
  ConnEndType,
  ConnectionPin,
  ConnectorRelation,
  ConnType,
  OrthogonalDimension,
  PenaltyType,
  RouterFlag,
  RoutingOption,
  RoutingParameter,
  RoutingParameterValue,
  ShapeTransformation,
  curvedPolyline,
  WasmAvoidRouter: wasm.AvoidRouter,
};

// --- argument helpers --------------------------------------------------------

function str(value, name) {
  if (typeof value !== "string") {
    throw new TypeError(`${name} must be a string`);
  }
  return value;
}

function num(value, name) {
  if (typeof value !== "number") {
    throw new TypeError(`${name} must be a number`);
  }
  return value;
}

function optNum(value, name) {
  return value === undefined || value === null ? undefined : num(value, name);
}

function bool(value, name) {
  if (typeof value !== "boolean") {
    throw new TypeError(`${name} must be a boolean`);
  }
  return value;
}

function optBool(value, name) {
  return value === undefined || value === null ? undefined : bool(value, name);
}

function nullable(value) {
  return value === undefined ? null : value;
}

function pointTuple(point) {
  if (Array.isArray(point) && point.length === 2) {
    return [num(point[0], "point[0]"), num(point[1], "point[1]")];
  }
  if (point && typeof point === "object") {
    return [num(point.x, "point.x"), num(point.y, "point.y")];
  }
  throw new TypeError("point must be [x, y] or { x, y }");
}

function flattenPoints(points) {
  if (!Array.isArray(points)) {
    throw new TypeError("points must be an array");
  }
  const flat = [];
  for (const point of points) {
    flat.push(...pointTuple(point));
  }
  return flat;
}

function flattenCheckpoints(checkpoints) {
  if (!Array.isArray(checkpoints)) {
    throw new TypeError("checkpoints must be an array");
  }
  const flat = [];
  for (const checkpoint of checkpoints) {
    if (checkpoint && typeof checkpoint === "object" && !Array.isArray(checkpoint) && "point" in checkpoint) {
      flat.push(
        ...pointTuple(checkpoint.point),
        num(checkpoint.arrivalDirections ?? ConnDir.All, "arrivalDirections"),
        num(checkpoint.departureDirections ?? ConnDir.All, "departureDirections"),
      );
    } else {
      flat.push(...pointTuple(checkpoint), ConnDir.All, ConnDir.All);
    }
  }
  return flat;
}

function pointEnd(point) {
  return { point, directions: ConnDir.All };
}

// ConnEnd input -> raw (kind, objectId, pinId, x, y, num) arguments.
function connEndArgs(end) {
  if (Array.isArray(end)) {
    const [x, y] = pointTuple(end);
    return [0, "", "", x, y, ConnDir.All];
  }
  if (!end || typeof end !== "object") {
    throw new TypeError("ConnEnd must be [x, y], { x, y }, { point, directions }, { shapeId, classId }, { shapeId, pinId } or { junctionId }");
  }
  if (end.type === "empty") {
    throw new TypeError("an empty ConnEnd cannot be used as an endpoint");
  }
  if (end.junctionId !== undefined) {
    return [2, str(end.junctionId, "junctionId"), "", 0, 0, 0];
  }
  if (end.shapeId !== undefined) {
    if (end.pinId !== undefined) {
      return [3, str(end.shapeId, "shapeId"), str(end.pinId, "pinId"), 0, 0, 0];
    }
    return [1, str(end.shapeId, "shapeId"), "", 0, 0, num(end.classId, "classId")];
  }
  const point = end.point ?? end.position ?? end;
  const [x, y] = pointTuple(point);
  const directions = end.point || end.position ? end.directions ?? ConnDir.All : ConnDir.All;
  return [0, "", "", x, y, num(directions, "directions")];
}
