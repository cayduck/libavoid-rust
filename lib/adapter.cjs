"use strict";

// JavaScript API of libavoid-rust: a string-id adapter over the faithful
// Rust port of upstream libavoid (see index.d.ts for the documentation of
// every method and of the id / error conventions).
//
// This file is shared by both entry points: index.cjs (Node, wasm-pack
// `nodejs` target in pkg/) and index.mjs (browsers and bundlers, wasm-pack
// `web` target in pkg-web/).  `createAvoidApi(wasm, { assertReady })`
// returns the package exports for a wasm-bindgen module `wasm` (its
// `AvoidRouter` class and `curvedPolyline` / `takeInternalErrorMessage`
// functions); `assertReady`, if given, runs before the wasm instance is used
// without an existing router (the AvoidRouter constructor, curvedPolyline)
// so that an uninitialised web build fails with a readable error;
// `loadInstance(...args)` returns (or resolves to) a freshly instantiated
// wasm-bindgen module and implements `reset()`.  lib/adapter.mjs is the
// ES-module copy generated from this file by scripts/prepare-pkg.cjs; edit
// this file only.
//
// Error convention:
//   * structurally malformed arguments (an id that is not a string, a point
//     that is neither [x, y] nor {x, y}, a non-array where an array is
//     expected, a non-boolean flag) throw a TypeError here;
//   * everything else is validated by the wasm module, which never aborts:
//     mutators return false and queries return null for unknown ids,
//     duplicate ids, non-finite numbers, out-of-range enum values and
//     violated upstream preconditions;
//   * the constructor throws an Error for invalid router flags;
//   * internal failures (a Rust panic, a failed allocation, a stack
//     overflow: all trap the wasm instance, which is built with
//     panic = "abort") are rethrown as AvoidInternalError, see below.
//
// Failure containment:
//   Every call into wasm goes through `guarded()`.  A trap leaves the wasm
//   instance in an unknown state (no Rust destructors ran; the shadow stack
//   pointer, wasm-bindgen's borrow flag of the object that was executing and
//   any half-updated router state stay as they were), so the adapter
//   poisons the whole *instance*: every router created on it throws
//   AvoidInternalError from then on, without calling into wasm, and the raw
//   wasm objects are detached from wasm-bindgen's FinalizationRegistry so
//   that garbage collection never calls into the dead instance either.
//   `reset()` replaces the instance by a fresh one; routers created
//   afterwards work normally.  The Rust side records the panic message and
//   location (or the size of the failed allocation) before trapping; it is
//   read back with takeInternalErrorMessage() (src/wasm_failure.rs).

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

// upstream: router.h enum TransactionPhases
const TransactionPhase = Object.freeze({
  OrthogonalVisibilityGraphScanX: 1,
  OrthogonalVisibilityGraphScanY: 2,
  RouteSearch: 3,
  CrossingDetection: 4,
  RerouteSearch: 5,
  OrthogonalNudgingX: 6,
  OrthogonalNudgingY: 7,
  Completed: 8,
});

/**
 * An internal failure of the wasm module (see the file header).  `kind` is
 * "panic", "out-of-memory", "stack-overflow", "trap" (a trap without a
 * recorded cause), "poisoned" (a call on a router whose instance failed
 * earlier; `cause` is that first error) or "reset" (the router's instance
 * was replaced by reset()).
 */
class AvoidInternalError extends Error {
  constructor(message, { method = null, kind = "trap", panicMessage = null, location = null, cause } = {}) {
    super(message, cause === undefined ? undefined : { cause });
    this.name = "AvoidInternalError";
    this.method = method;
    this.kind = kind;
    this.panicMessage = panicMessage;
    this.location = location;
  }
}

const now =
  typeof performance !== "undefined" && typeof performance.now === "function" ? () => performance.now() : () => Date.now();

function isTrap(error) {
  if (typeof WebAssembly !== "undefined" && error instanceof WebAssembly.RuntimeError) return true;
  return isStackExhaustion(error);
}

/** The JS engine's own stack overflow (V8/JSC: RangeError, SpiderMonkey: InternalError). */
function isStackExhaustion(error) {
  return (
    error !== null &&
    typeof error === "object" &&
    (error.name === "RangeError" || error.name === "InternalError") &&
    /call stack|too much recursion/i.test(String(error.message))
  );
}

/** One wasm instance: its wasm-bindgen module, first internal error, live raw routers. */
function instanceState(wasm) {
  return { wasm, error: null, live: new Set(), pruneAt: 64 };
}

function track(state, raw) {
  state.live.add(typeof WeakRef === "function" ? new WeakRef(raw) : { deref: () => raw });
  if (state.live.size >= state.pruneAt) {
    for (const ref of state.live) {
      if (ref.deref() === undefined) state.live.delete(ref);
    }
    state.pruneAt = Math.max(64, 2 * state.live.size);
  }
}

/** Marks the instance unusable and detaches its raw objects from finalization. */
function poison(state, error) {
  if (state.error) return;
  state.error = error;
  for (const ref of state.live) {
    const raw = ref.deref();
    // wasm-bindgen's own method: zeroes the pointer and unregisters the
    // object from the FinalizationRegistry without calling into wasm.
    if (raw && typeof raw.__destroy_into_raw === "function") {
      try {
        raw.__destroy_into_raw();
      } catch {
        // nothing more can be done for this object
      }
    }
  }
  state.live.clear();
}

/** The error for a call on a poisoned instance. */
function unusable(state, method) {
  const first = state.error;
  const message =
    first.kind === "reset"
      ? `${method}: router is unusable: its WebAssembly instance was replaced by reset()`
      : `${method}: router is unusable after an internal error: ${first.message}`;
  return new AvoidInternalError(message, { method, kind: "poisoned", cause: first });
}

/** AvoidInternalError for a trap `error` during `method`, or null if `error` is not a trap. */
function internalError(error, method, state) {
  if (!isTrap(error)) return null;
  let record = "";
  try {
    record = String(state.wasm.takeInternalErrorMessage());
  } catch {
    // the instance cannot run even this (e.g. its stack is exhausted)
  }
  const fields = { method, cause: error };
  let detail;
  const panic = /^panicked at (\S+:\d+:\d+): ([\s\S]*)$/.exec(record);
  if (panic) {
    Object.assign(fields, { kind: "panic", location: panic[1], panicMessage: panic[2] });
    detail = `Rust panic at ${panic[1]}: ${panic[2]}`;
  } else if (record.startsWith("panicked")) {
    Object.assign(fields, { kind: "panic", panicMessage: record });
    detail = `Rust ${record}`;
  } else if (/^memory allocation of \d+ bytes failed$/.test(record)) {
    Object.assign(fields, { kind: "out-of-memory" });
    detail = `out of memory: ${record}`;
  } else if (isStackExhaustion(error)) {
    Object.assign(fields, { kind: "stack-overflow" });
    detail = `stack overflow (${error.message})`;
  } else if (/memory access out of bounds/i.test(String(error.message))) {
    // Safe Rust does not access memory out of bounds; the wasm shadow stack
    // growing below address 0 does.
    Object.assign(fields, { kind: "stack-overflow" });
    detail = `stack overflow (WebAssembly trap: ${error.message}; no Rust panic was recorded)`;
  } else {
    Object.assign(fields, { kind: "trap" });
    detail = `WebAssembly trap without a recorded Rust panic: ${error.message}`;
  }
  return new AvoidInternalError(`libavoid-rust internal error in ${method}: ${detail}`, fields);
}

/** Runs `fn` (a call into `state`'s wasm instance), converting a trap into a poisoning AvoidInternalError. */
function guarded(state, method, fn) {
  if (state.error) throw unusable(state, method);
  let result;
  try {
    result = fn();
  } catch (error) {
    if (state.error) throw unusable(state, method); // poisoned by a nested call
    const internal = internalError(error, method, state);
    if (internal) {
      poison(state, internal);
      throw internal;
    }
    throw error;
  }
  // A call made from a progress callback during `fn` may have poisoned it.
  if (state.error) throw unusable(state, method);
  return result;
}

function createAvoidApi(wasm, { assertReady = () => {}, loadInstance } = {}) {
  let current = instanceState(wasm);

  class AvoidRouter {
    #inner;
    #state;
    #busy = false;
    #callStart = 0;
    #progressCallback = null;
    #timeLimit = null;
    #progressError = undefined;
    #hasProgressError = false;

    static withFlags(flags) {
      return new AvoidRouter(flags);
    }

    constructor(flags = RouterFlag.OrthogonalRouting) {
      if (typeof flags !== "number") {
        throw new TypeError("AvoidRouter: flags must be a number (RouterFlag)");
      }
      assertReady();
      const state = current;
      this.#state = state;
      this.#inner = guarded(state, "new AvoidRouter", () => new state.wasm.AvoidRouter(flags));
      track(state, this.#inner);
    }

    /** A function calling raw method `method` under the failure guard. */
    #raw(method) {
      const state = this.#state;
      const label = `AvoidRouter.${method}`;
      if (state.error) throw unusable(state, label);
      if (this.#busy) {
        throw new Error(`${label}: a router cannot be used from inside its own transaction progress callback`);
      }
      const inner = this.#inner;
      return (...args) => {
        this.#busy = true;
        this.#callStart = this.#progressInstalled() ? now() : 0;
        let result;
        try {
          result = guarded(state, label, () => inner[method](...args));
        } finally {
          this.#busy = false;
        }
        if (this.#hasProgressError) {
          const error = this.#progressError;
          this.#hasProgressError = false;
          this.#progressError = undefined;
          throw error;
        }
        return result;
      };
    }

    // --- failure containment --------------------------------------------------

    isPoisoned() {
      return this.#state.error !== null;
    }

    /** Test only: traps the wasm instance (see src/wasm_api.rs `__testPanic`). */
    __testPanic(kind, arming) {
      return this.#raw("__testPanic")(num(kind, "kind"), str(arming, "arming"));
    }

    // --- transaction progress -------------------------------------------------

    #progressInstalled() {
      return this.#progressCallback !== null || this.#timeLimit !== null;
    }

    // upstream shouldContinueTransactionWithProgress().  The core's own
    // elapsed time is 0 in wasm (no clock), so time is measured here, from
    // the start of the API call that runs the transaction.
    #onProgress = (_coreElapsed, phase, totalPhases, proportion) => {
      if (this.#state.error) return false;
      const elapsed = now() - this.#callStart;
      let proceed = true;
      if (this.#timeLimit !== null && elapsed >= this.#timeLimit) proceed = false;
      if (this.#progressCallback !== null) {
        try {
          if (this.#progressCallback(elapsed, phase, totalPhases, proportion) === false) proceed = false;
        } catch (error) {
          if (!this.#hasProgressError) {
            this.#hasProgressError = true;
            this.#progressError = error;
          }
          proceed = false;
        }
      }
      return proceed;
    };

    #updateProgressHook() {
      this.#raw("setProgressCallback")(this.#progressInstalled() ? this.#onProgress : undefined);
    }

    setTransactionProgressCallback(callback) {
      if (callback !== null && callback !== undefined && typeof callback !== "function") {
        throw new TypeError("callback must be a function or null");
      }
      this.#progressCallback = callback ?? null;
      this.#updateProgressHook();
    }

    setTransactionTimeLimit(milliseconds) {
      if (milliseconds !== null && milliseconds !== undefined && (typeof milliseconds !== "number" || !(milliseconds >= 0))) {
        throw new TypeError("milliseconds must be a non-negative number or null");
      }
      this.#timeLimit = milliseconds ?? null;
      this.#updateProgressHook();
    }

    transactionTimeLimit() {
      return this.#timeLimit;
    }

    lastTransactionCancelled() {
      return this.#raw("lastTransactionCancelled")();
    }

    // --- router ---------------------------------------------------------------

    routerFlags() {
      return this.#raw("routerFlags")();
    }

    clear() {
      this.#raw("clear")();
    }

    setTransactionUse(transactions) {
      this.#raw("setTransactionUse")(bool(transactions, "transactions"));
    }

    transactionUse() {
      return this.#raw("transactionUse")();
    }

    processTransaction() {
      return this.#raw("processTransaction")();
    }

    lastTransactionUpdatedConnectorIds() {
      return JSON.parse(this.#raw("lastTransactionUpdatedConnectorIds")());
    }

    setRoutingParameter(parameter, value) {
      return this.#raw("setRoutingParameter")(num(parameter, "parameter"), optNum(value, "value"));
    }

    setRoutingPenalty(penaltyType, penaltyValue) {
      return this.#raw("setRoutingPenalty")(num(penaltyType, "penaltyType"), optNum(penaltyValue, "penaltyValue"));
    }

    routingParameter(parameter) {
      return nullable(this.#raw("routingParameter")(num(parameter, "parameter")));
    }

    setRoutingOption(option, value) {
      return this.#raw("setRoutingOption")(num(option, "option"), bool(value, "value"));
    }

    routingOption(option) {
      return nullable(this.#raw("routingOption")(num(option, "option")));
    }

    validConnType(select = ConnType.None) {
      return nullable(this.#raw("validConnType")(num(select, "select")));
    }

    // --- ids ------------------------------------------------------------------

    objectIdIsUnused(id) {
      if (typeof id === "number") {
        return nullable(this.#raw("numericObjectIdIsUnused")(id));
      }
      return this.#raw("objectIdIsUnused")(str(id, "id"));
    }

    newObjectId() {
      return this.#raw("newObjectId")();
    }

    objectNumericId(id) {
      return nullable(this.#raw("objectNumericId")(str(id, "id")));
    }

    objectIdFromNumericId(numericId) {
      return JSON.parse(this.#raw("objectIdFromNumericId")(num(numericId, "numericId")));
    }

    shapeIds() {
      return JSON.parse(this.#raw("shapeIds")());
    }

    junctionIds() {
      return JSON.parse(this.#raw("junctionIds")());
    }

    connectorIds() {
      return JSON.parse(this.#raw("connectorIds")());
    }

    clusterIds() {
      return JSON.parse(this.#raw("clusterIds")());
    }

    // --- clusters -------------------------------------------------------------

    addCluster(id, points, numericId) {
      return this.#raw("addCluster")(str(id, "id"), flattenPoints(points), optNum(numericId, "numericId"));
    }

    addClusterRectangle(id, left, top, right, bottom, numericId) {
      return this.#raw("addClusterRectangle")(
        str(id, "id"),
        num(left, "left"),
        num(top, "top"),
        num(right, "right"),
        num(bottom, "bottom"),
        optNum(numericId, "numericId"),
      );
    }

    setClusterPolygon(id, points) {
      return this.#raw("setClusterPolygon")(str(id, "id"), flattenPoints(points));
    }

    setClusterRectangle(id, left, top, right, bottom) {
      return this.#raw("setClusterRectangle")(
        str(id, "id"),
        num(left, "left"),
        num(top, "top"),
        num(right, "right"),
        num(bottom, "bottom"),
      );
    }

    removeCluster(id) {
      return this.#raw("removeCluster")(str(id, "id"));
    }

    clusterBoundingBox(id) {
      return JSON.parse(this.#raw("clusterBoundingBox")(str(id, "id")));
    }

    clusterPolygon(id) {
      return JSON.parse(this.#raw("clusterPolygon")(str(id, "id")));
    }

    clusterRectangularPolygon(id) {
      return JSON.parse(this.#raw("clusterRectangularPolygon")(str(id, "id")));
    }

    // --- shapes ---------------------------------------------------------------

    addShape(id, left, top, right, bottom, numericId) {
      return this.#raw("addShape")(
        str(id, "id"),
        num(left, "left"),
        num(top, "top"),
        num(right, "right"),
        num(bottom, "bottom"),
        optNum(numericId, "numericId"),
      );
    }

    addPolygonShape(id, points, numericId) {
      return this.#raw("addPolygonShape")(str(id, "id"), flattenPoints(points), optNum(numericId, "numericId"));
    }

    moveShape(id, left, top, right, bottom) {
      return this.#raw("moveShape")(str(id, "id"), num(left, "left"), num(top, "top"), num(right, "right"), num(bottom, "bottom"));
    }

    movePolygonShape(id, points, firstMove) {
      return this.#raw("movePolygonShape")(str(id, "id"), flattenPoints(points), optBool(firstMove, "firstMove"));
    }

    moveShapeBy(id, xDiff, yDiff) {
      return this.#raw("moveShapeBy")(str(id, "id"), num(xDiff, "xDiff"), num(yDiff, "yDiff"));
    }

    shapeBoundingBox(id) {
      return JSON.parse(this.#raw("shapeBoundingBox")(str(id, "id")));
    }

    shapeRoutingBox(id) {
      return JSON.parse(this.#raw("shapeRoutingBox")(str(id, "id")));
    }

    shapePolygon(id) {
      return JSON.parse(this.#raw("shapePolygon")(str(id, "id")));
    }

    shapeRoutingPolygon(id) {
      return JSON.parse(this.#raw("shapeRoutingPolygon")(str(id, "id")));
    }

    shapePosition(id) {
      return JSON.parse(this.#raw("shapePosition")(str(id, "id")));
    }

    shapeContainingPoint(point) {
      const [x, y] = pointTuple(point);
      return JSON.parse(this.#raw("shapeContainingPoint")(x, y));
    }

    shapeAttachedConnectors(id) {
      return JSON.parse(this.#raw("shapeAttachedConnectors")(str(id, "id")));
    }

    shapeAttachedShapes(id, type = ConnectorRelation.RunningToAndFrom) {
      return JSON.parse(this.#raw("shapeAttachedShapes")(str(id, "id"), num(type, "type")));
    }

    shapeAttachedConns(id, type = ConnectorRelation.RunningToAndFrom) {
      return JSON.parse(this.#raw("shapeAttachedConns")(str(id, "id"), num(type, "type")));
    }

    transformShapeConnectionPinPositions(id, transform) {
      return this.#raw("transformShapeConnectionPinPositions")(str(id, "id"), num(transform, "transform"));
    }

    removeShape(id) {
      return this.#raw("removeShape")(str(id, "id"));
    }

    markAllObstaclesAsMoved() {
      this.#raw("markAllObstaclesAsMoved")();
    }

    // --- junctions ------------------------------------------------------------

    addJunction(id, x, y, numericId) {
      return this.#raw("addJunction")(str(id, "id"), num(x, "x"), num(y, "y"), optNum(numericId, "numericId"));
    }

    removeJunction(id) {
      return this.#raw("removeJunction")(str(id, "id"));
    }

    removeJunctionAndMergeConnectors(id) {
      return JSON.parse(this.#raw("removeJunctionAndMergeConnectors")(str(id, "id")));
    }

    moveJunction(id, x, y) {
      return this.#raw("moveJunction")(str(id, "id"), num(x, "x"), num(y, "y"));
    }

    moveJunctionBy(id, xDiff, yDiff) {
      return this.#raw("moveJunctionBy")(str(id, "id"), num(xDiff, "xDiff"), num(yDiff, "yDiff"));
    }

    junctionPosition(id) {
      return JSON.parse(this.#raw("junctionPosition")(str(id, "id")));
    }

    junctionRecommendedPosition(id) {
      return JSON.parse(this.#raw("junctionRecommendedPosition")(str(id, "id")));
    }

    setJunctionPositionFixed(id, fixed) {
      return this.#raw("setJunctionPositionFixed")(str(id, "id"), bool(fixed, "fixed"));
    }

    junctionPositionFixed(id) {
      return nullable(this.#raw("junctionPositionFixed")(str(id, "id")));
    }

    preferJunctionOrthogonalDimension(id, dimension) {
      return this.#raw("preferJunctionOrthogonalDimension")(str(id, "id"), num(dimension, "dimension"));
    }

    junctionAttachedConnectors(id) {
      return JSON.parse(this.#raw("junctionAttachedConnectors")(str(id, "id")));
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
      return this.#raw("addShapeConnectionPin")(
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
      return this.#raw("addShapeConnectionPinWithOptions")(
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
      return this.#raw("removeShapeConnectionPin")(str(shapeId, "shapeId"), str(pinId, "pinId"));
    }

    shapeConnectionPinIdList(shapeId) {
      return JSON.parse(this.#raw("shapeConnectionPinIdList")(str(shapeId, "shapeId")));
    }

    setShapeConnectionPinConnectionCost(shapeId, pinId, cost) {
      return this.#raw("setShapeConnectionPinConnectionCost")(str(shapeId, "shapeId"), str(pinId, "pinId"), num(cost, "cost"));
    }

    setShapeConnectionPinExclusive(shapeId, pinId, exclusive) {
      return this.#raw("setShapeConnectionPinExclusive")(
        str(shapeId, "shapeId"),
        str(pinId, "pinId"),
        bool(exclusive, "exclusive"),
      );
    }

    shapeConnectionPinExclusive(shapeId, pinId) {
      return nullable(this.#raw("shapeConnectionPinExclusive")(str(shapeId, "shapeId"), str(pinId, "pinId")));
    }

    shapeConnectionPinPosition(shapeId, pinId) {
      return JSON.parse(this.#raw("shapeConnectionPinPosition")(str(shapeId, "shapeId"), str(pinId, "pinId")));
    }

    shapeConnectionPinDirections(shapeId, pinId) {
      return nullable(this.#raw("shapeConnectionPinDirections")(str(shapeId, "shapeId"), str(pinId, "pinId")));
    }

    shapeConnectionPinIds(shapeId, pinId) {
      return JSON.parse(this.#raw("shapeConnectionPinIds")(str(shapeId, "shapeId"), str(pinId, "pinId")));
    }

    addJunctionConnectionPin(junctionId, pinId, classId, visibilityDirections) {
      return this.#raw("addJunctionConnectionPin")(
        str(junctionId, "junctionId"),
        str(pinId, "pinId"),
        num(classId, "classId"),
        optNum(visibilityDirections, "visibilityDirections"),
      );
    }

    removeJunctionConnectionPin(junctionId, pinId) {
      return this.#raw("removeJunctionConnectionPin")(str(junctionId, "junctionId"), str(pinId, "pinId"));
    }

    junctionConnectionPinPosition(junctionId, pinId) {
      return JSON.parse(this.#raw("junctionConnectionPinPosition")(str(junctionId, "junctionId"), str(pinId, "pinId")));
    }

    junctionConnectionPinDirections(junctionId, pinId) {
      return nullable(this.#raw("junctionConnectionPinDirections")(str(junctionId, "junctionId"), str(pinId, "pinId")));
    }

    junctionConnectionPinIds(junctionId, pinId) {
      return JSON.parse(this.#raw("junctionConnectionPinIds")(str(junctionId, "junctionId"), str(pinId, "pinId")));
    }

    setJunctionConnectionPinConnectionCost(junctionId, pinId, cost) {
      return this.#raw("setJunctionConnectionPinConnectionCost")(
        str(junctionId, "junctionId"),
        str(pinId, "pinId"),
        num(cost, "cost"),
      );
    }

    setJunctionConnectionPinExclusive(junctionId, pinId, exclusive) {
      return this.#raw("setJunctionConnectionPinExclusive")(
        str(junctionId, "junctionId"),
        str(pinId, "pinId"),
        bool(exclusive, "exclusive"),
      );
    }

    junctionConnectionPinExclusive(junctionId, pinId) {
      return nullable(this.#raw("junctionConnectionPinExclusive")(str(junctionId, "junctionId"), str(pinId, "pinId")));
    }

    // --- connectors: creation -------------------------------------------------

    addConnectorWithoutEndpoints(id, numericId) {
      return this.#raw("addConnectorWithoutEndpoints")(str(id, "id"), optNum(numericId, "numericId"));
    }

    addConnectorWithConnEnds(id, src, dst, numericId) {
      return this.#raw("addConnectorWithConnEnds")(
        str(id, "id"),
        ...connEndArgs(src),
        ...connEndArgs(dst),
        optNum(numericId, "numericId"),
      );
    }

    addConnector(id, srcX, srcY, dstX, dstY, numericId) {
      return this.#raw("addConnector")(
        str(id, "id"),
        num(srcX, "srcX"),
        num(srcY, "srcY"),
        num(dstX, "dstX"),
        num(dstY, "dstY"),
        optNum(numericId, "numericId"),
      );
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
      return this.#raw("setConnectorConnEnds")(str(id, "id"), ...connEndArgs(src), ...connEndArgs(dst));
    }

    setConnectorSourceConnEnd(id, end) {
      return this.#raw("setConnectorSourceConnEnd")(str(id, "id"), ...connEndArgs(end));
    }

    setConnectorDestConnEnd(id, end) {
      return this.#raw("setConnectorDestConnEnd")(str(id, "id"), ...connEndArgs(end));
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
      return this.#raw("removeConnector")(str(id, "id"));
    }

    // --- connectors: properties -----------------------------------------------

    setConnectorRoutingType(id, routingType) {
      return this.#raw("setConnectorRoutingType")(str(id, "id"), num(routingType, "routingType"));
    }

    connectorRoutingType(id) {
      return nullable(this.#raw("connectorRoutingType")(str(id, "id")));
    }

    setConnectorHateCrossings(id, value) {
      return this.#raw("setConnectorHateCrossings")(str(id, "id"), bool(value, "value"));
    }

    connectorDoesHateCrossings(id) {
      return nullable(this.#raw("connectorDoesHateCrossings")(str(id, "id")));
    }

    connectorEndpointConnEnds(id) {
      return JSON.parse(this.#raw("connectorEndpointConnEnds")(str(id, "id")));
    }

    connectorPossibleDstPinPoints(id) {
      return JSON.parse(this.#raw("connectorPossibleDstPinPoints")(str(id, "id")));
    }

    setConnectorRoutingCheckpoints(id, checkpoints) {
      return this.#raw("setConnectorRoutingCheckpoints")(str(id, "id"), flattenCheckpoints(checkpoints));
    }

    connectorRoutingCheckpoints(id) {
      return JSON.parse(this.#raw("connectorRoutingCheckpoints")(str(id, "id")));
    }

    setConnectorFixedRoute(id, route) {
      return this.#raw("setConnectorFixedRoute")(str(id, "id"), flattenPoints(route));
    }

    setConnectorFixedExistingRoute(id) {
      return this.#raw("setConnectorFixedExistingRoute")(str(id, "id"));
    }

    connectorHasFixedRoute(id) {
      return nullable(this.#raw("connectorHasFixedRoute")(str(id, "id")));
    }

    clearConnectorFixedRoute(id) {
      return this.#raw("clearConnectorFixedRoute")(str(id, "id"));
    }

    makeConnectorPathInvalid(id) {
      return this.#raw("makeConnectorPathInvalid")(str(id, "id"));
    }

    connectorNeedsRepaint(id) {
      return nullable(this.#raw("connectorNeedsRepaint")(str(id, "id")));
    }

    routeConnector(id) {
      return JSON.parse(this.#raw("routeConnector")(str(id, "id")));
    }

    displayConnectorRoute(id) {
      return JSON.parse(this.#raw("displayConnectorRoute")(str(id, "id")));
    }

    connectorCurvedRoute(id, curveAmount, closed) {
      return JSON.parse(this.#raw("connectorCurvedRoute")(str(id, "id"), num(curveAmount, "curveAmount"), optBool(closed, "closed")));
    }

    splitConnectorAtSegment(id, segmentN, junctionId, connectorId) {
      return JSON.parse(
        this.#raw("splitConnectorAtSegment")(
          str(id, "id"),
          num(segmentN, "segmentN"),
          str(junctionId, "junctionId"),
          str(connectorId, "connectorId"),
        ),
      );
    }

    routeAll() {
      return JSON.parse(this.#raw("routeAll")());
    }

    // --- diagnostics ----------------------------------------------------------

    existsInvalidOrthogonalPaths() {
      return this.#raw("existsInvalidOrthogonalPaths")();
    }

    existsCrossings(optimisedForConnectorType = false) {
      return this.#raw("existsCrossings")(bool(optimisedForConnectorType, "optimisedForConnectorType"));
    }

    existsOrthogonalSegmentOverlap(atEnds = false) {
      return this.#raw("existsOrthogonalSegmentOverlap")(bool(atEnds, "atEnds"));
    }

    existsOrthogonalFixedSegmentOverlap(atEnds = false) {
      return this.#raw("existsOrthogonalFixedSegmentOverlap")(bool(atEnds, "atEnds"));
    }

    existsOrthogonalTouchingPaths() {
      return this.#raw("existsOrthogonalTouchingPaths")();
    }

    outputDiagramText(fullPrecision = false) {
      return this.#raw("outputDiagramText")(bool(fullPrecision, "fullPrecision"));
    }

    printInfo() {
      return this.#raw("printInfo")();
    }

    // --- hyperedges -----------------------------------------------------------

    registerHyperedgeForReroutingJunction(junctionId) {
      return nullable(this.#raw("registerHyperedgeForReroutingJunction")(str(junctionId, "junctionId")));
    }

    registerHyperedgeForRerouting(terminals) {
      if (!Array.isArray(terminals)) {
        throw new TypeError("terminals must be an array of ConnEnds");
      }
      const cols = [[], [], [], [], [], []];
      for (const end of terminals) {
        connEndArgs(end).forEach((v, i) => cols[i].push(v));
      }
      return nullable(this.#raw("registerHyperedgeForRerouting")(...cols));
    }

    hyperedgeReroutingCount() {
      return this.#raw("hyperedgeReroutingCount")();
    }

    hyperedgeNewAndDeletedObjectLists(index) {
      return JSON.parse(this.#raw("hyperedgeNewAndDeletedObjectLists")(num(index, "index")));
    }

    newAndDeletedObjectListsFromHyperedgeImprovement() {
      return JSON.parse(this.#raw("newAndDeletedObjectListsFromHyperedgeImprovement")());
    }

    free() {
      if (this.#state.error) {
        return; // the instance is gone and its objects are already detached
      }
      this.#raw("free")();
    }
  }

  function curvedPolyline(points, curveAmount, closed) {
    assertReady();
    const state = current;
    const coordinates = flattenPoints(points);
    const amount = num(curveAmount, "curveAmount");
    const close = optBool(closed, "closed");
    return JSON.parse(guarded(state, "curvedPolyline", () => state.wasm.curvedPolyline(coordinates, amount, close)));
  }

  function adopt(fresh) {
    if (!fresh || typeof fresh.AvoidRouter !== "function") {
      throw new Error("libavoid-rust: reset() did not produce a wasm-bindgen module");
    }
    poison(current, new AvoidInternalError("the WebAssembly instance was replaced by reset()", { method: "reset", kind: "reset" }));
    current = instanceState(fresh);
  }

  function reset(...args) {
    if (typeof loadInstance !== "function") {
      throw new Error("libavoid-rust: reset() is not available for this entry point");
    }
    const fresh = loadInstance(...args);
    if (fresh && typeof fresh.then === "function") {
      return fresh.then(adopt);
    }
    adopt(fresh);
    return undefined;
  }

  return {
    AttachPos,
    AvoidInternalError,
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
    TransactionPhase,
    curvedPolyline,
    internalError: () => current.error,
    reset,
    get WasmAvoidRouter() {
      return current.wasm.AvoidRouter;
    },
  };
}

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

module.exports = createAvoidApi;
