"use strict";

// JavaScript API of libavoid-rust: upstream libavoid's public C++ API
// (namespace Avoid: Router, ShapeRef, ConnRef, ConnEnd, ...), implemented
// on the low-level wasm layer of src/wasm_api.rs (RawRouter: numeric
// object keys, flat numeric arrays).  index.d.ts documents the API and the
// rules mapping C++ idioms to JavaScript; docs/JS_API_COVERAGE.md lists
// every upstream public item and its JavaScript counterpart.
//
// This file is shared by both entry points: index.cjs (Node, wasm-pack
// `nodejs` target in pkg/) and index.mjs (browsers and bundlers, `web`
// target in pkg-web/).  `createAvoidApi(wasm, { assertReady, loadInstance })`
// returns the package exports for a wasm-bindgen module `wasm`;
// `assertReady`, if given, runs before the wasm instance is first used by
// a constructor or free function, so that an uninitialised web build fails
// with a readable error; `loadInstance(...args)` returns (or resolves to) a
// freshly instantiated wasm-bindgen module and implements `reset()`.
// lib/adapter.mjs is the ES-module copy generated from this file by
// scripts/prepare-pkg.cjs; edit this file only.
//
// Failure containment:
//   Every call into wasm goes through `guarded()`.  A trap (Rust panic,
//   failed allocation, stack overflow: the instance is built with
//   panic = "abort") leaves the wasm instance in an unknown state, so the
//   adapter poisons the whole *instance*: every object created on it throws
//   AvoidInternalError from then on, without calling into wasm, and the raw
//   wasm objects are detached from wasm-bindgen's FinalizationRegistry so
//   that garbage collection never calls into the dead instance either.
//   `reset()` replaces the instance by a fresh one.  The Rust side records
//   the panic message and location (or the size of the failed allocation)
//   before trapping; it is read back with takeInternalErrorMessage()
//   (src/wasm_failure.rs).

// --- upstream constants (flat names, as in namespace Avoid) -------------------

const CONSTANTS = Object.freeze({
  // geomtypes.h
  XDIM: 0,
  YDIM: 1,
  kUnassignedVertexNumber: 8,
  kShapeConnectionPin: 9,
  // geometry.h
  DONT_INTERSECT: 0,
  DO_INTERSECT: 1,
  PARALLEL: 3,
  // router.h enum RouterFlag
  PolyLineRouting: 1,
  OrthogonalRouting: 2,
  // router.h
  runningTo: 1,
  runningFrom: 2,
  runningToAndFrom: 3,
  // router.h enum RoutingParameter
  segmentPenalty: 0,
  anglePenalty: 1,
  crossingPenalty: 2,
  clusterCrossingPenalty: 3,
  fixedSharedPathPenalty: 4,
  portDirectionPenalty: 5,
  shapeBufferDistance: 6,
  idealNudgingDistance: 7,
  reverseDirectionPenalty: 8,
  lastRoutingParameterMarker: 9,
  // router.h enum RoutingOption
  nudgeOrthogonalSegmentsConnectedToShapes: 0,
  improveHyperedgeRoutesMovingJunctions: 1,
  penaliseOrthogonalSharedPathsAtConnEnds: 2,
  nudgeOrthogonalTouchingColinearSegments: 3,
  performUnifyingNudgingPreprocessingStep: 4,
  improveHyperedgeRoutesMovingAddingAndDeletingJunctions: 5,
  nudgeSharedPathsWithCommonEndPoint: 6,
  lastRoutingOptionMarker: 7,
  // router.h enum TransactionPhases
  TransactionPhaseOrthogonalVisibilityGraphScanX: 1,
  TransactionPhaseOrthogonalVisibilityGraphScanY: 2,
  TransactionPhaseRouteSearch: 3,
  TransactionPhaseCrossingDetection: 4,
  TransactionPhaseRerouteSearch: 5,
  TransactionPhaseOrthogonalNudgingX: 6,
  TransactionPhaseOrthogonalNudgingY: 7,
  TransactionPhaseCompleted: 8,
  // router.h
  zeroParamValue: 0,
  chooseSensibleParamValue: -1,
  // connector.h enum ConnType
  ConnType_None: 0,
  ConnType_PolyLine: 1,
  ConnType_Orthogonal: 2,
  // connector.h
  CROSSING_NONE: 0,
  CROSSING_TOUCHES: 1,
  CROSSING_SHARES_PATH: 2,
  CROSSING_SHARES_PATH_AT_END: 4,
  CROSSING_SHARES_FIXED_SEGMENT: 8,
  // connend.h enum ConnDirFlag
  ConnDirNone: 0,
  ConnDirUp: 1,
  ConnDirDown: 2,
  ConnDirLeft: 4,
  ConnDirRight: 8,
  ConnDirAll: 15,
  // connend.h enum ConnEndType
  ConnEndPoint: 0,
  ConnEndShapePin: 1,
  ConnEndJunction: 2,
  ConnEndEmpty: 3,
  // connectionpin.h
  CONNECTIONPIN_UNSET: 2147483647,
  CONNECTIONPIN_CENTRE: 2147483646,
  ATTACH_POS_TOP: 0,
  ATTACH_POS_CENTRE: 0.5,
  ATTACH_POS_BOTTOM: 1,
  ATTACH_POS_LEFT: 0,
  ATTACH_POS_RIGHT: 1,
  ATTACH_POS_MIN_OFFSET: 0,
  ATTACH_POS_MAX_OFFSET: -1,
  // shape.h enum ShapeTransformationType
  TransformationType_CW90: 0,
  TransformationType_CW180: 1,
  TransformationType_CW270: 2,
  TransformationType_FlipX: 3,
  TransformationType_FlipY: 4,
});

function pick(...names) {
  return Object.freeze(Object.fromEntries(names.map((n) => [n, CONSTANTS[n]])));
}

// Additional aliases: upstream's enums as objects (the flat names are the API).
const ENUMS = Object.freeze({
  RouterFlag: pick("PolyLineRouting", "OrthogonalRouting"),
  RoutingParameter: pick(
    "segmentPenalty",
    "anglePenalty",
    "crossingPenalty",
    "clusterCrossingPenalty",
    "fixedSharedPathPenalty",
    "portDirectionPenalty",
    "shapeBufferDistance",
    "idealNudgingDistance",
    "reverseDirectionPenalty",
    "lastRoutingParameterMarker",
  ),
  RoutingOption: pick(
    "nudgeOrthogonalSegmentsConnectedToShapes",
    "improveHyperedgeRoutesMovingJunctions",
    "penaliseOrthogonalSharedPathsAtConnEnds",
    "nudgeOrthogonalTouchingColinearSegments",
    "performUnifyingNudgingPreprocessingStep",
    "improveHyperedgeRoutesMovingAddingAndDeletingJunctions",
    "nudgeSharedPathsWithCommonEndPoint",
    "lastRoutingOptionMarker",
  ),
  TransactionPhases: pick(
    "TransactionPhaseOrthogonalVisibilityGraphScanX",
    "TransactionPhaseOrthogonalVisibilityGraphScanY",
    "TransactionPhaseRouteSearch",
    "TransactionPhaseCrossingDetection",
    "TransactionPhaseRerouteSearch",
    "TransactionPhaseOrthogonalNudgingX",
    "TransactionPhaseOrthogonalNudgingY",
    "TransactionPhaseCompleted",
  ),
  ConnType: pick("ConnType_None", "ConnType_PolyLine", "ConnType_Orthogonal"),
  ConnDirFlag: pick("ConnDirNone", "ConnDirUp", "ConnDirDown", "ConnDirLeft", "ConnDirRight", "ConnDirAll"),
  ConnEndType: pick("ConnEndPoint", "ConnEndShapePin", "ConnEndJunction", "ConnEndEmpty"),
  ShapeTransformationType: pick(
    "TransformationType_CW90",
    "TransformationType_CW180",
    "TransformationType_CW270",
    "TransformationType_FlipX",
    "TransformationType_FlipY",
  ),
});

// upstream: typedef enum RoutingParameter PenaltyType
const PenaltyType = ENUMS.RoutingParameter;

// Keys of src/wasm_api.rs: (n << 3) | kind.
const KIND_SHAPE = 1;
const KIND_JUNCTION = 2;
const KIND_CONN = 3;
const KIND_CLUSTER = 4;
const KIND_PIN = 5;

// Index of each public bool member of Router in RawRouter.flagMember().
const ROUTER_FLAG_MEMBERS = [
  "PartialTime",
  "SimpleRouting",
  "ClusteredRouting",
  "IgnoreRegions",
  "UseLeesAlgorithm",
  "InvisibilityGrph",
  "SelectiveReroute",
  "PartialFeedback",
  "RubberBandRouting",
];

/** Key of the test-only trap trigger (not API): `Router[TEST_PANIC](router, kind, arming)`. */
const TEST_PANIC = Symbol.for("libavoid-rust.testPanic");

/**
 * An internal failure of the wasm module (see the file header).  `kind` is
 * "panic", "out-of-memory", "stack-overflow", "trap" (a trap without a
 * recorded cause), "poisoned" (a call on an object whose instance failed
 * earlier; `cause` is that first error) or "reset" (the object's instance
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
      ? `${method}: object is unusable: its WebAssembly instance was replaced by reset()`
      : `${method}: object is unusable after an internal error: ${first.message}`;
  return new AvoidInternalError(message, { method, kind: "poisoned", cause: first });
}

/** AvoidInternalError for a trap `error` during `method`, or null if `error` is not a trap. */
function internalErrorFor(error, method, state) {
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

/**
 * Runs `fn` (a call into `state`'s wasm instance).  A trap becomes a
 * poisoning AvoidInternalError; an Error the wasm layer returned (a
 * violated precondition, a deleted object) is rethrown as an Error naming
 * `method`.
 */
function guarded(state, method, fn) {
  if (state.error) throw unusable(state, method);
  let result;
  try {
    result = fn();
  } catch (error) {
    if (state.error) throw unusable(state, method); // poisoned by a nested call
    const internal = internalErrorFor(error, method, state);
    if (internal) {
      poison(state, internal);
      throw internal;
    }
    if (error instanceof Error && error.constructor === Error) {
      throw new Error(`${method}: ${error.message}`);
    }
    throw error;
  }
  // A call made from a progress callback during `fn` may have poisoned it.
  if (state.error) throw unusable(state, method);
  return result;
}

// --- argument checks ------------------------------------------------------------

function arity(args, min, max, label) {
  if (args.length < min || args.length > max) {
    const want = min === max ? `${min}` : `${min} to ${max}`;
    throw new TypeError(`${label}: expected ${want} argument${max === 1 ? "" : "s"}, got ${args.length}`);
  }
}

function num(v, name) {
  if (typeof v !== "number") throw new TypeError(`${name} must be a number`);
  return v;
}

function uint(v, name) {
  if (typeof v !== "number" || !Number.isInteger(v) || v < 0 || v > 0xffffffff) {
    throw new TypeError(`${name} must be an unsigned 32-bit integer`);
  }
  return v;
}

function int32(v, name) {
  if (typeof v !== "number" || !Number.isInteger(v) || v < -0x80000000 || v > 0x7fffffff) {
    throw new TypeError(`${name} must be a 32-bit integer`);
  }
  return v;
}

function size(v, name) {
  if (typeof v !== "number" || !Number.isSafeInteger(v) || v < 0) {
    throw new TypeError(`${name} must be a non-negative integer (size_t)`);
  }
  return v;
}

function bool(v, name) {
  if (typeof v !== "boolean") throw new TypeError(`${name} must be a boolean`);
  return v;
}

function dirs(v, name) {
  uint(v, name);
  if (v > 15) throw new RangeError(`${name} must be a combination of ConnDirUp/Down/Left/Right (0..15)`);
  return v;
}

function array(v, name) {
  if (!Array.isArray(v)) throw new TypeError(`${name} must be an array`);
  return v;
}

function dflt(v, d) {
  return v === undefined ? d : v;
}

// std::min / std::max (NaN-propagation exactly as C++: the first argument
// wins unless the comparison holds).
function cppMin(a, b) {
  return b < a ? b : a;
}
function cppMax(a, b) {
  return a < b ? b : a;
}

function createAvoidApi(wasm, { assertReady = () => {}, loadInstance } = {}) {
  let current = instanceState(wasm);

  // Router -> its context; router object wrapper -> { ctx, key, deleted, cb, ptr }.
  const ctxOf = new WeakMap();
  const info = new WeakMap();
  const CONSTRUCT = Symbol("construct");

  /** Guarded call into the wasm instance for code without a router. */
  function free(label, fn) {
    assertReady();
    const state = current;
    return guarded(state, label, () => fn(state.wasm));
  }

  // --- value encodings (see src/wasm_api.rs) ------------------------------------

  function checkPoint(p, name) {
    if (!(p instanceof Point)) throw new TypeError(`${name} must be an Avoid.Point`);
    return p;
  }

  function pushPoint(out, p, name) {
    checkPoint(p, name);
    num(p.x, `${name}.x`);
    num(p.y, `${name}.y`);
    uint(p.id, `${name}.id`);
    if (!Number.isInteger(p.vn) || p.vn < 0 || p.vn > 0xffff) throw new TypeError(`${name}.vn must be an unsigned 16-bit integer`);
    out.push(p.x, p.y, p.id, p.vn);
  }

  function decPoint(v, i) {
    const p = new Point(v[i], v[i + 1]);
    p.id = v[i + 2];
    p.vn = v[i + 3];
    return p;
  }

  function asPolygon(poly, name) {
    if (poly instanceof Polygon) return poly;
    if (poly instanceof PolygonInterface) return new Polygon(poly); // C++ implicit conversion
    throw new TypeError(`${name} must be an Avoid.Polygon`);
  }

  function encPoly(poly, name) {
    poly = asPolygon(poly, name);
    int32(poly._id, `${name}._id`);
    array(poly.ps, `${name}.ps`);
    array(poly.ts, `${name}.ts`);
    array(poly.checkpointsOnRoute, `${name}.checkpointsOnRoute`);
    const out = [poly._id, poly.ps.length];
    for (let i = 0; i < poly.ps.length; i++) pushPoint(out, poly.ps[i], `${name}.ps[${i}]`);
    out.push(poly.ts.length);
    for (const t of poly.ts) {
      if (typeof t !== "string" || t.length !== 1) throw new TypeError(`${name}.ts must contain one-character strings`);
      out.push(t.charCodeAt(0));
    }
    out.push(poly.checkpointsOnRoute.length);
    for (const pair of poly.checkpointsOnRoute) {
      if (!Array.isArray(pair) || pair.length !== 2) throw new TypeError(`${name}.checkpointsOnRoute must contain [index, Point] pairs`);
      out.push(size(pair[0], `${name}.checkpointsOnRoute index`));
      pushPoint(out, pair[1], `${name}.checkpointsOnRoute point`);
    }
    return out;
  }

  function decPoly(v, into = new Polygon()) {
    into._id = v[0];
    const n = v[1];
    let i = 2;
    const ps = new Array(n);
    for (let k = 0; k < n; k++, i += 4) ps[k] = decPoint(v, i);
    into.ps = ps;
    const nts = v[i++];
    const ts = new Array(nts);
    for (let k = 0; k < nts; k++) ts[k] = String.fromCharCode(v[i++]);
    into.ts = ts;
    const ncp = v[i++];
    const cps = new Array(ncp);
    for (let k = 0; k < ncp; k++, i += 5) cps[k] = [v[i], decPoint(v, i + 1)];
    into.checkpointsOnRoute = cps;
    return into;
  }

  function decBox(v) {
    const b = new Box();
    b.min = decPoint(v, 0);
    b.max = decPoint(v, 4);
    return b;
  }

  function decPoints(v) {
    const out = [];
    for (let i = 0; i < v.length; i += 4) out.push(decPoint(v, i));
    return out;
  }

  // --- routers and their objects --------------------------------------------------

  function routerCtx(router, label) {
    const ctx = router instanceof Router ? ctxOf.get(router) : undefined;
    if (!ctx) throw new TypeError(`${label}: router must be an Avoid.Router`);
    return ctx;
  }

  function checkUsable(ctx, label) {
    if (ctx.state.error) throw unusable(ctx.state, label);
    if (ctx.deleted) throw new Error(`${label}: the Router has been deleted`);
    if (ctx.busy) {
      throw new Error(`${label}: a Router cannot be used from inside its own shouldContinueTransactionWithProgress()`);
    }
  }

  /**
   * upstream Router::assignId(id) calls the virtual newObjectId() for id 0:
   * a JavaScript override of newObjectId() supplies the id of objects
   * constructed through this API (the router's own objects -- hyperedge
   * rerouting / improvement, splitAtSegment -- get the built-in id).
   */
  function assignedId(ctx, id) {
    if (id !== 0 || ctx.router.newObjectId === Router.prototype.newObjectId) return id;
    return uint(ctx.router.newObjectId(), "newObjectId()");
  }

  /** A call that cannot run a transaction. */
  function read(ctx, label, fn) {
    checkUsable(ctx, label);
    return guarded(ctx.state, label, () => fn(ctx.raw));
  }

  /**
   * A call that may run a transaction (always with transactions off):
   * keeps the shouldContinueTransactionWithProgress() hook in sync with
   * the Router's method, then invokes the connector callbacks the core
   * performed, in its order, and rethrows an error thrown by the hook.
   */
  function mutate(ctx, label, fn) {
    checkUsable(ctx, label);
    const overridden = ctx.router.shouldContinueTransactionWithProgress !== Router.prototype.shouldContinueTransactionWithProgress;
    if (overridden !== ctx.hookInstalled) {
      guarded(ctx.state, label, () => ctx.raw.setProgressHook(overridden ? ctx.hook : undefined));
      ctx.hookInstalled = overridden;
    }
    if (ctx.objects.size > ctx.pruneAt) prune(ctx, label);
    ctx.busy = true;
    ctx.start = now();
    let result;
    try {
      result = guarded(ctx.state, label, () => fn(ctx.raw));
    } finally {
      ctx.busy = false;
    }
    const fired = guarded(ctx.state, label, () => ctx.raw.takeCallbacks());
    for (const key of fired) {
      const conn = ctx.objects.get(key);
      const entry = conn && info.get(conn);
      if (entry && typeof entry.cb === "function") entry.cb(entry.ptr);
    }
    if (ctx.hasHookError) {
      const error = ctx.hookError;
      ctx.hasHookError = false;
      ctx.hookError = undefined;
      throw error;
    }
    return result;
  }

  /** Forgets wrappers of objects that were already dead at the previous prune. */
  function prune(ctx, label) {
    const keys = [...ctx.objects.keys()];
    const alive = guarded(ctx.state, label, () => ctx.raw.aliveMany(Uint32Array.from(keys)));
    keys.forEach((key, i) => {
      if (alive[i]) return;
      const entry = info.get(ctx.objects.get(key));
      if (entry.dead) ctx.objects.delete(key);
      else entry.dead = true;
    });
    ctx.pruneAt = Math.max(1024, 2 * ctx.objects.size);
  }

  const CLASS_OF_KIND = () => ({
    [KIND_SHAPE]: ShapeRef,
    [KIND_JUNCTION]: JunctionRef,
    [KIND_CONN]: ConnRef,
    [KIND_CLUSTER]: ClusterRef,
    [KIND_PIN]: ShapeConnectionPin,
  });
  let classOfKind;

  function register(ctx, key, obj) {
    info.set(obj, { ctx, key, deleted: false, dead: false, cb: null, ptr: undefined });
    ctx.objects.set(key, obj);
    return obj;
  }

  /** The wrapper of `key` (0 = null), created for objects the router made itself. */
  function wrap(ctx, key) {
    if (key === 0) return null;
    let obj = ctx.objects.get(key);
    if (obj === undefined) {
      classOfKind ??= CLASS_OF_KIND();
      obj = register(ctx, key, Object.create(classOfKind[key & 7].prototype));
    }
    return obj;
  }

  function wrapAll(ctx, keys) {
    return Array.from(keys, (k) => wrap(ctx, k));
  }

  /** Info of a router object argument of class `Cls`, checked for use. */
  function objInfo(obj, Cls, label, name = "argument") {
    const entry = obj instanceof Cls ? info.get(obj) : undefined;
    if (!entry) throw new TypeError(`${label}: ${name} must be an Avoid.${Cls.name}`);
    if (entry.deleted) throw new Error(`${label}: the ${obj.constructor.name} has been deleted`);
    return entry;
  }

  /** Key of `obj` (a `Cls`) for use with router context `ctx`. */
  function keyIn(ctx, obj, Cls, label, name) {
    const entry = objInfo(obj, Cls, label, name);
    if (entry.ctx !== ctx) throw new Error(`${label}: the ${Cls.name} belongs to another Router`);
    return entry.key;
  }

  function self(obj, Cls, label) {
    const entry = objInfo(obj, Cls, label, "this");
    checkUsable(entry.ctx, label);
    return entry;
  }

  function decLists(ctx, v) {
    const lists = new HyperedgeNewAndDeletedObjectLists();
    let i = 0;
    const next = () => {
      const n = v[i++];
      const keys = v.subarray(i, i + n);
      i += n;
      return wrapAll(ctx, keys);
    };
    lists.newJunctionList = next();
    lists.newConnectorList = next();
    lists.deletedJunctionList = next();
    lists.deletedConnectorList = next();
    lists.changedConnectorList = next();
    return lists;
  }

  // --- geometry value types ---------------------------------------------------------

  // upstream: geomtypes.h class Point
  class Point {
    constructor(xv, yv) {
      arity(arguments, 0, 2, "Point");
      if (arguments.length === 1) throw new TypeError("Point: expected 0 or 2 arguments, got 1");
      this.x = arguments.length === 0 ? 0 : num(xv, "Point: xv");
      this.y = arguments.length === 0 ? 0 : num(yv, "Point: yv");
      this.id = 0;
      this.vn = CONSTANTS.kUnassignedVertexNumber;
    }
    equals(rhs, epsilon = 0.0001) {
      arity(arguments, 1, 2, "Point.equals");
      checkPoint(rhs, "Point.equals: rhs");
      num(epsilon, "Point.equals: epsilon");
      return Math.abs(this.x - rhs.x) < epsilon && Math.abs(this.y - rhs.y) < epsilon;
    }
    /** operator== */
    equal(rhs) {
      arity(arguments, 1, 1, "Point.equal");
      checkPoint(rhs, "Point.equal: rhs");
      return this.x === rhs.x && this.y === rhs.y;
    }
    /** operator!= */
    notEqual(rhs) {
      arity(arguments, 1, 1, "Point.notEqual");
      return !this.equal(rhs);
    }
    /** operator< */
    lessThan(rhs) {
      arity(arguments, 1, 1, "Point.lessThan");
      checkPoint(rhs, "Point.lessThan: rhs");
      return this.x === rhs.x ? this.y < rhs.y : this.x < rhs.x;
    }
    /** operator[] (read) */
    get(dimension) {
      arity(arguments, 1, 1, "Point.get");
      return size(dimension, "Point.get: dimension") === 0 ? this.x : checkDim(dimension, "Point.get") && this.y;
    }
    /** operator[] (write) */
    set(dimension, value) {
      arity(arguments, 2, 2, "Point.set");
      size(dimension, "Point.set: dimension");
      num(value, "Point.set: value");
      if (dimension === 0) this.x = value;
      else if (checkDim(dimension, "Point.set")) this.y = value;
    }
    /** operator+ */
    add(rhs) {
      arity(arguments, 1, 1, "Point.add");
      checkPoint(rhs, "Point.add: rhs");
      return new Point(this.x + rhs.x, this.y + rhs.y);
    }
    /** operator- */
    sub(rhs) {
      arity(arguments, 1, 1, "Point.sub");
      checkPoint(rhs, "Point.sub: rhs");
      return new Point(this.x - rhs.x, this.y - rhs.y);
    }
    /** C++ `delete` of a heap-allocated value: nothing to free in JavaScript. */
    delete() {}
  }

  function checkDim(dimension, label) {
    if (dimension !== 1) throw new RangeError(`${label}: dimension must be XDIM or YDIM (COLA_ASSERT)`);
    return true;
  }

  // upstream: geomtypes.h class Box
  class Box {
    constructor() {
      arity(arguments, 0, 0, "Box");
      this.min = new Point();
      this.max = new Point();
    }
    length(dimension) {
      arity(arguments, 1, 1, "Box.length");
      size(dimension, "Box.length: dimension");
      if (dimension === 0) return this.max.x - this.min.x;
      checkDim(dimension, "Box.length");
      return this.max.y - this.min.y;
    }
    width() {
      arity(arguments, 0, 0, "Box.width");
      return this.max.x - this.min.x;
    }
    height() {
      arity(arguments, 0, 0, "Box.height");
      return this.max.y - this.min.y;
    }
    delete() {}
  }

  // upstream: geomtypes.h class Edge
  class Edge {
    constructor() {
      arity(arguments, 0, 0, "Edge");
      this.a = new Point();
      this.b = new Point();
    }
    delete() {}
  }

  // upstream: router.h struct LineRep
  class LineRep {
    constructor() {
      arity(arguments, 0, 0, "LineRep");
      this.begin = new Point();
      this.end = new Point();
    }
    delete() {}
  }

  // upstream: geomtypes.h class PolygonInterface (abstract)
  class PolygonInterface {
    constructor() {
      if (new.target === PolygonInterface) throw new TypeError("PolygonInterface is abstract");
    }
    // pure virtual upstream
    clear() {
      throw new TypeError("PolygonInterface.clear is pure virtual");
    }
    empty() {
      throw new TypeError("PolygonInterface.empty is pure virtual");
    }
    size() {
      throw new TypeError("PolygonInterface.size is pure virtual");
    }
    id() {
      throw new TypeError("PolygonInterface.id is pure virtual");
    }
    at(index) {
      throw new TypeError("PolygonInterface.at is pure virtual");
    }
    boundingRectPolygon() {
      arity(arguments, 0, 0, "PolygonInterface.boundingRectPolygon");
      const enc = encPoly(this, "this");
      return decPoly(free("PolygonInterface.boundingRectPolygon", (w) => w.polyBoundingRectPolygon(enc)));
    }
    offsetBoundingBox(offset) {
      arity(arguments, 1, 1, "PolygonInterface.offsetBoundingBox");
      num(offset, "offset");
      const enc = encPoly(this, "this");
      return decBox(free("PolygonInterface.offsetBoundingBox", (w) => w.polyOffsetBoundingBox(enc, offset)));
    }
    offsetPolygon(offset) {
      arity(arguments, 1, 1, "PolygonInterface.offsetPolygon");
      num(offset, "offset");
      const enc = encPoly(this, "this");
      return decPoly(free("PolygonInterface.offsetPolygon", (w) => w.polyOffsetPolygon(enc, offset)));
    }
    delete() {}
  }

  // upstream: geomtypes.h class Polygon (typedef PolyLine)
  class Polygon extends PolygonInterface {
    constructor(arg) {
      super();
      arity(arguments, 0, 1, "Polygon");
      this._id = 0;
      this.ps = [];
      this.ts = [];
      this.checkpointsOnRoute = [];
      if (arguments.length === 0) return;
      if (typeof arg === "number") {
        if (!Number.isInteger(arg) || arg < 0) throw new RangeError("Polygon: n must be a non-negative integer");
        for (let i = 0; i < arg; i++) this.ps.push(new Point());
      } else if (arg instanceof PolygonInterface) {
        this._id = arg.id();
        const n = arg.size();
        for (let i = 0; i < n; i++) {
          const p = arg.at(i);
          const q = new Point(p.x, p.y);
          q.id = p.id;
          q.vn = p.vn;
          this.ps.push(q);
        }
      } else {
        throw new TypeError("Polygon: argument must be a point count or an Avoid.PolygonInterface");
      }
    }
    clear() {
      arity(arguments, 0, 0, "Polygon.clear");
      this.ps = [];
      this.ts = [];
    }
    empty() {
      arity(arguments, 0, 0, "Polygon.empty");
      return this.ps.length === 0;
    }
    size() {
      arity(arguments, 0, 0, "Polygon.size");
      return this.ps.length;
    }
    id() {
      arity(arguments, 0, 0, "Polygon.id");
      return this._id;
    }
    at(index) {
      arity(arguments, 1, 1, "Polygon.at");
      size(index, "Polygon.at: index");
      if (index >= this.ps.length) throw new RangeError("Polygon.at: index out of range (COLA_ASSERT(index < size()))");
      return this.ps[index];
    }
    setPoint(index, point) {
      arity(arguments, 2, 2, "Polygon.setPoint");
      size(index, "Polygon.setPoint: index");
      checkPoint(point, "Polygon.setPoint: point");
      if (index >= this.ps.length) throw new RangeError("Polygon.setPoint: index out of range (COLA_ASSERT(index < size()))");
      const q = new Point(point.x, point.y);
      q.id = point.id;
      q.vn = point.vn;
      this.ps[index] = q;
    }
    simplify() {
      arity(arguments, 0, 0, "Polygon.simplify");
      const enc = encPoly(this, "this");
      return decPoly(free("Polygon.simplify", (w) => w.polySimplify(enc)));
    }
    curvedPolyline(curve_amount, closed = false) {
      arity(arguments, 1, 2, "Polygon.curvedPolyline");
      num(curve_amount, "curve_amount");
      bool(closed, "closed");
      const enc = encPoly(this, "this");
      return decPoly(free("Polygon.curvedPolyline", (w) => w.polyCurvedPolyline(enc, curve_amount, closed)));
    }
    translate(xDist, yDist) {
      arity(arguments, 2, 2, "Polygon.translate");
      num(xDist, "xDist");
      num(yDist, "yDist");
      for (const p of this.ps) {
        p.x += xDist;
        p.y += yDist;
      }
    }
    checkpointsOnSegment(segmentLowerIndex, indexModifier = 0) {
      arity(arguments, 1, 2, "Polygon.checkpointsOnSegment");
      size(segmentLowerIndex, "segmentLowerIndex");
      int32(indexModifier, "indexModifier");
      const enc = encPoly(this, "this");
      return decPoints(free("Polygon.checkpointsOnSegment", (w) => w.polyCheckpointsOnSegment(enc, segmentLowerIndex, indexModifier)));
    }
  }

  // upstream: geomtypes.h class ReferencingPolygon
  class ReferencingPolygon extends PolygonInterface {
    constructor(poly, router) {
      super();
      this._id = 0;
      this.psRef = [];
      this.psPoints = [];
      if (arguments.length === 0) return;
      arity(arguments, 2, 2, "ReferencingPolygon");
      const enc = encPoly(poly, "ReferencingPolygon: poly");
      const ctx = routerCtx(router, "ReferencingPolygon");
      decReferencing(ctx, read(ctx, "ReferencingPolygon", (raw) => raw.referencingPolygon(enc)), this);
    }
    clear() {
      arity(arguments, 0, 0, "ReferencingPolygon.clear");
      this.psRef = [];
      this.psPoints = [];
    }
    empty() {
      arity(arguments, 0, 0, "ReferencingPolygon.empty");
      return this.psRef.length === 0;
    }
    size() {
      arity(arguments, 0, 0, "ReferencingPolygon.size");
      return this.psRef.length;
    }
    id() {
      arity(arguments, 0, 0, "ReferencingPolygon.id");
      return this._id;
    }
    at(index) {
      arity(arguments, 1, 1, "ReferencingPolygon.at");
      size(index, "ReferencingPolygon.at: index");
      if (index >= this.psRef.length) throw new RangeError("ReferencingPolygon.at: index out of range (COLA_ASSERT(index < size()))");
      const [poly, vn] = this.psRef[index];
      if (poly) {
        if (vn >= poly.ps.length) throw new RangeError("ReferencingPolygon.at: referenced index out of range (COLA_ASSERT)");
        return poly.ps[vn];
      }
      return this.psPoints[index];
    }
  }

  /** psRef pairs refer to snapshots of the referenced obstacles' polygons. */
  function decReferencing(ctx, v, into = new ReferencingPolygon()) {
    into._id = v[0];
    const n = v[1];
    const polys = new Map();
    into.psRef = [];
    into.psPoints = [];
    for (let k = 0, i = 2; k < n; k++, i += 6) {
      const anchor = v[i];
      let poly = null;
      if (anchor !== 0) {
        if (!polys.has(anchor)) {
          let p = null;
          try {
            p = decPoly(read(ctx, "ReferencingPolygon", (raw) => raw.obstaclePolygon(anchor)));
          } catch (error) {
            if (error instanceof AvoidInternalError) throw error;
          }
          polys.set(anchor, p);
        }
        poly = polys.get(anchor);
      }
      into.psRef.push([poly, v[i + 1]]);
      into.psPoints.push(decPoint(v, i + 2));
    }
    return into;
  }

  // upstream: geomtypes.h class Rectangle
  class Rectangle extends Polygon {
    constructor(a, b, c) {
      super(4);
      if (arguments.length === 2) {
        checkPoint(a, "Rectangle: topLeft");
        checkPoint(b, "Rectangle: bottomRight");
        const minX = cppMin(a.x, b.x);
        const maxX = cppMax(a.x, b.x);
        const minY = cppMin(a.y, b.y);
        const maxY = cppMax(a.y, b.y);
        this.ps = [new Point(maxX, minY), new Point(maxX, maxY), new Point(minX, maxY), new Point(minX, minY)];
      } else if (arguments.length === 3) {
        checkPoint(a, "Rectangle: centre");
        num(b, "Rectangle: width");
        num(c, "Rectangle: height");
        const halfWidth = b / 2.0;
        const halfHeight = c / 2.0;
        const minX = a.x - halfWidth;
        const maxX = a.x + halfWidth;
        const minY = a.y - halfHeight;
        const maxY = a.y + halfHeight;
        this.ps = [new Point(maxX, minY), new Point(maxX, maxY), new Point(minX, maxY), new Point(minX, minY)];
      } else {
        throw new TypeError(`Rectangle: expected (topLeft, bottomRight) or (centre, width, height), got ${arguments.length} arguments`);
      }
    }
  }

  // --- Router ---------------------------------------------------------------------------

  // upstream: router.h class Router
  class Router {
    constructor(flags) {
      arity(arguments, 1, 1, "Router");
      uint(flags, "Router: flags");
      assertReady();
      const state = current;
      const raw = guarded(state, "Router", () => new state.wasm.RawRouter(flags));
      track(state, raw);
      const ctx = {
        raw,
        state,
        router: this,
        objects: new Map(),
        busy: false,
        deleted: false,
        start: 0,
        hookInstalled: false,
        hasHookError: false,
        hookError: undefined,
        pruneAt: 1024,
        rerouter: null,
        hook: null,
      };
      // upstream calls the virtual shouldContinueTransactionWithProgress()
      // here; the JS override is called with upstream's arguments.  A
      // throwing override counts as a stop request; its error is rethrown
      // when the call that ran the transaction returns.
      ctx.hook = (_coreElapsed, phase, total, proportion) => {
        if (ctx.state.error) return false;
        const elapsed = Math.min(0xffffffff, Math.max(0, Math.floor(now() - ctx.start)));
        let proceed;
        const busy = ctx.busy;
        try {
          proceed = Boolean(this.shouldContinueTransactionWithProgress(elapsed, phase, total, proportion));
        } catch (error) {
          if (!ctx.hasHookError) {
            ctx.hasHookError = true;
            ctx.hookError = error;
          }
          proceed = false;
        } finally {
          ctx.busy = busy;
        }
        return proceed;
      };
      ctxOf.set(this, ctx);
    }

    /** upstream: virtual Router::shouldContinueTransactionWithProgress(); override by subclassing. */
    shouldContinueTransactionWithProgress(elapsedTime, phaseNumber, totalPhases, proportion) {
      return true;
    }

    setTransactionUse(transactions) {
      arity(arguments, 1, 1, "Router.setTransactionUse");
      bool(transactions, "transactions");
      read(this.#ctx("setTransactionUse"), "Router.setTransactionUse", (raw) => raw.setTransactionUse(transactions));
    }
    transactionUse() {
      arity(arguments, 0, 0, "Router.transactionUse");
      return read(this.#ctx("transactionUse"), "Router.transactionUse", (raw) => raw.transactionUse());
    }
    processTransaction() {
      arity(arguments, 0, 0, "Router.processTransaction");
      return mutate(this.#ctx("processTransaction"), "Router.processTransaction", (raw) => raw.processTransaction());
    }
    deleteShape(shape) {
      arity(arguments, 1, 1, "Router.deleteShape");
      const ctx = this.#ctx("deleteShape");
      const key = keyIn(ctx, shape, ShapeRef, "Router.deleteShape", "shape");
      mutate(ctx, "Router.deleteShape", (raw) => raw.deleteShape(key));
      info.get(shape).deleted = true;
    }
    moveShape(shape, a, b) {
      const label = "Router.moveShape";
      const ctx = this.#ctx("moveShape");
      if (typeof a === "number") {
        arity(arguments, 3, 3, label);
        num(b, "yDiff");
        const key = keyIn(ctx, shape, ShapeRef, label, "shape");
        mutate(ctx, label, (raw) => raw.moveShapeBy(key, a, b));
      } else {
        arity(arguments, 2, 3, label);
        const firstMove = dflt(b, false);
        bool(firstMove, "first_move");
        const key = keyIn(ctx, shape, ShapeRef, label, "shape");
        const enc = encPoly(a, "newPoly");
        mutate(ctx, label, (raw) => raw.moveShapePoly(key, enc, firstMove));
      }
    }
    deleteJunction(junction) {
      arity(arguments, 1, 1, "Router.deleteJunction");
      const ctx = this.#ctx("deleteJunction");
      const key = keyIn(ctx, junction, JunctionRef, "Router.deleteJunction", "junction");
      mutate(ctx, "Router.deleteJunction", (raw) => raw.deleteJunction(key));
      info.get(junction).deleted = true;
    }
    deleteConnector(connector) {
      arity(arguments, 1, 1, "Router.deleteConnector");
      const ctx = this.#ctx("deleteConnector");
      const key = keyIn(ctx, connector, ConnRef, "Router.deleteConnector", "connector");
      mutate(ctx, "Router.deleteConnector", (raw) => raw.deleteConnector(key));
      info.get(connector).deleted = true;
    }
    moveJunction(junction, a, b) {
      const label = "Router.moveJunction";
      const ctx = this.#ctx("moveJunction");
      if (typeof a === "number") {
        arity(arguments, 3, 3, label);
        num(b, "yDiff");
        const key = keyIn(ctx, junction, JunctionRef, label, "junction");
        mutate(ctx, label, (raw) => raw.moveJunctionBy(key, a, b));
      } else {
        arity(arguments, 2, 2, label);
        checkPoint(a, `${label}: newPosition`);
        const key = keyIn(ctx, junction, JunctionRef, label, "junction");
        mutate(ctx, label, (raw) => raw.moveJunctionTo(key, num(a.x, "newPosition.x"), num(a.y, "newPosition.y")));
      }
    }
    setRoutingParameter(parameter, value = CONSTANTS.chooseSensibleParamValue) {
      arity(arguments, 1, 2, "Router.setRoutingParameter");
      uint(parameter, "parameter");
      num(value, "value");
      mutate(this.#ctx("setRoutingParameter"), "Router.setRoutingParameter", (raw) => raw.setRoutingParameter(parameter, value));
    }
    routingParameter(parameter) {
      arity(arguments, 1, 1, "Router.routingParameter");
      uint(parameter, "parameter");
      return read(this.#ctx("routingParameter"), "Router.routingParameter", (raw) => raw.routingParameter(parameter));
    }
    setRoutingOption(option, value) {
      arity(arguments, 2, 2, "Router.setRoutingOption");
      uint(option, "option");
      bool(value, "value");
      mutate(this.#ctx("setRoutingOption"), "Router.setRoutingOption", (raw) => raw.setRoutingOption(option, value));
    }
    routingOption(option) {
      arity(arguments, 1, 1, "Router.routingOption");
      uint(option, "option");
      return read(this.#ctx("routingOption"), "Router.routingOption", (raw) => raw.routingOption(option));
    }
    // From the libavoid-js fork of Adaptagrams (no upstream counterpart).
    setRouteInsideParentShapes(value) {
      arity(arguments, 1, 1, "Router.setRouteInsideParentShapes");
      bool(value, "value");
      mutate(this.#ctx("setRouteInsideParentShapes"), "Router.setRouteInsideParentShapes", (raw) =>
        raw.setRouteInsideParentShapes(value),
      );
    }
    routeInsideParentShapes() {
      arity(arguments, 0, 0, "Router.routeInsideParentShapes");
      return read(this.#ctx("routeInsideParentShapes"), "Router.routeInsideParentShapes", (raw) =>
        raw.routeInsideParentShapes(),
      );
    }
    setRoutingPenalty(penType, penVal = CONSTANTS.chooseSensibleParamValue) {
      arity(arguments, 1, 2, "Router.setRoutingPenalty");
      uint(penType, "penType");
      num(penVal, "penVal");
      mutate(this.#ctx("setRoutingPenalty"), "Router.setRoutingPenalty", (raw) => raw.setRoutingPenalty(penType, penVal));
    }
    hyperedgeRerouter() {
      arity(arguments, 0, 0, "Router.hyperedgeRerouter");
      const ctx = this.#ctx("hyperedgeRerouter");
      checkUsable(ctx, "Router.hyperedgeRerouter");
      if (!ctx.rerouter) {
        ctx.rerouter = Object.create(HyperedgeRerouter.prototype);
        rerouterCtx.set(ctx.rerouter, ctx);
      }
      return ctx.rerouter;
    }
    /** Returns the SVG upstream writes to `<filename>.svg` (default name "libavoid-debug"). */
    outputInstanceToSVG(filename = "") {
      arity(arguments, 0, 1, "Router.outputInstanceToSVG");
      if (typeof filename !== "string") throw new TypeError("filename must be a string");
      return read(this.#ctx("outputInstanceToSVG"), "Router.outputInstanceToSVG", (raw) => raw.outputInstanceToSVG());
    }
    /** upstream: virtual Router::newObjectId(); may be overridden by subclassing (see assignedId()). */
    newObjectId() {
      arity(arguments, 0, 0, "Router.newObjectId");
      return read(this.#ctx("newObjectId"), "Router.newObjectId", (raw) => raw.newObjectId());
    }
    objectIdIsUnused(id) {
      arity(arguments, 1, 1, "Router.objectIdIsUnused");
      uint(id, "id");
      return read(this.#ctx("objectIdIsUnused"), "Router.objectIdIsUnused", (raw) => raw.objectIdIsUnused(id));
    }
    newAndDeletedObjectListsFromHyperedgeImprovement() {
      arity(arguments, 0, 0, "Router.newAndDeletedObjectListsFromHyperedgeImprovement");
      const ctx = this.#ctx("newAndDeletedObjectListsFromHyperedgeImprovement");
      return decLists(ctx, read(ctx, "Router.newAndDeletedObjectListsFromHyperedgeImprovement", (raw) => raw.newAndDeletedObjectListsFromHyperedgeImprovement()));
    }
    processActions() {
      arity(arguments, 0, 0, "Router.processActions");
      mutate(this.#ctx("processActions"), "Router.processActions", (raw) => raw.processActions());
    }
    deleteCluster(cluster) {
      arity(arguments, 1, 1, "Router.deleteCluster");
      const ctx = this.#ctx("deleteCluster");
      const key = keyIn(ctx, cluster, ClusterRef, "Router.deleteCluster", "cluster");
      mutate(ctx, "Router.deleteCluster", (raw) => raw.deleteCluster(key));
      info.get(cluster).deleted = true;
    }
    attachedShapes(shapes, shapeId, type) {
      arity(arguments, 3, 3, "Router.attachedShapes");
      array(shapes, "shapes");
      uint(shapeId, "shapeId");
      uint(type, "type");
      const ids = read(this.#ctx("attachedShapes"), "Router.attachedShapes", (raw) => raw.attachedShapes(shapeId, type));
      for (const id of ids) shapes.push(id);
    }
    attachedConns(conns, shapeId, type) {
      arity(arguments, 3, 3, "Router.attachedConns");
      array(conns, "conns");
      uint(shapeId, "shapeId");
      uint(type, "type");
      const ids = read(this.#ctx("attachedConns"), "Router.attachedConns", (raw) => raw.attachedConns(shapeId, type));
      for (const id of ids) conns.push(id);
    }
    markPolylineConnectorsNeedingReroutingForDeletedObstacle(obstacle) {
      arity(arguments, 1, 1, "Router.markPolylineConnectorsNeedingReroutingForDeletedObstacle");
      const ctx = this.#ctx("markPolylineConnectorsNeedingReroutingForDeletedObstacle");
      const key = keyIn(ctx, obstacle, Obstacle, "Router.markPolylineConnectorsNeedingReroutingForDeletedObstacle", "obstacle");
      mutate(ctx, "Router.markPolylineConnectorsNeedingReroutingForDeletedObstacle", (raw) =>
        raw.markPolylineConnectorsNeedingReroutingForDeletedObstacle(key),
      );
    }
    /** Returns the text upstream prints to stdout. */
    printInfo() {
      arity(arguments, 0, 0, "Router.printInfo");
      return read(this.#ctx("printInfo"), "Router.printInfo", (raw) => raw.printInfo());
    }
    regenerateStaticBuiltGraph() {
      arity(arguments, 0, 0, "Router.regenerateStaticBuiltGraph");
      mutate(this.#ctx("regenerateStaticBuiltGraph"), "Router.regenerateStaticBuiltGraph", (raw) => raw.regenerateStaticBuiltGraph());
    }
    destroyOrthogonalVisGraph() {
      arity(arguments, 0, 0, "Router.destroyOrthogonalVisGraph");
      mutate(this.#ctx("destroyOrthogonalVisGraph"), "Router.destroyOrthogonalVisGraph", (raw) => raw.destroyOrthogonalVisGraph());
    }
    setStaticGraphInvalidated(invalidated) {
      arity(arguments, 1, 1, "Router.setStaticGraphInvalidated");
      bool(invalidated, "invalidated");
      read(this.#ctx("setStaticGraphInvalidated"), "Router.setStaticGraphInvalidated", (raw) => raw.setStaticGraphInvalidated(invalidated));
    }
    validConnType(select = CONSTANTS.ConnType_None) {
      arity(arguments, 0, 1, "Router.validConnType");
      uint(select, "select");
      return read(this.#ctx("validConnType"), "Router.validConnType", (raw) => raw.validConnType(select));
    }
    isInCrossingPenaltyReroutingStage() {
      arity(arguments, 0, 0, "Router.isInCrossingPenaltyReroutingStage");
      return read(this.#ctx("isInCrossingPenaltyReroutingStage"), "Router.isInCrossingPenaltyReroutingStage", (raw) =>
        raw.isInCrossingPenaltyReroutingStage(),
      );
    }
    markAllObstaclesAsMoved() {
      arity(arguments, 0, 0, "Router.markAllObstaclesAsMoved");
      mutate(this.#ctx("markAllObstaclesAsMoved"), "Router.markAllObstaclesAsMoved", (raw) => raw.markAllObstaclesAsMoved());
    }
    shapeContainingPoint(point) {
      arity(arguments, 1, 1, "Router.shapeContainingPoint");
      checkPoint(point, "Router.shapeContainingPoint: point");
      const ctx = this.#ctx("shapeContainingPoint");
      return wrap(ctx, read(ctx, "Router.shapeContainingPoint", (raw) => raw.shapeContainingPoint(num(point.x, "point.x"), num(point.y, "point.y"))));
    }
    performContinuationCheck(phaseNumber, stepNumber, totalSteps) {
      arity(arguments, 3, 3, "Router.performContinuationCheck");
      uint(phaseNumber, "phaseNumber");
      size(stepNumber, "stepNumber");
      size(totalSteps, "totalSteps");
      mutate(this.#ctx("performContinuationCheck"), "Router.performContinuationCheck", (raw) =>
        raw.performContinuationCheck(phaseNumber, stepNumber, totalSteps),
      );
    }
    registerSettingsChange() {
      arity(arguments, 0, 0, "Router.registerSettingsChange");
      read(this.#ctx("registerSettingsChange"), "Router.registerSettingsChange", (raw) => raw.registerSettingsChange());
    }
    improveOrthogonalTopology() {
      arity(arguments, 0, 0, "Router.improveOrthogonalTopology");
      mutate(this.#ctx("improveOrthogonalTopology"), "Router.improveOrthogonalTopology", (raw) => raw.improveOrthogonalTopology());
    }
    existsOrthogonalSegmentOverlap(atEnds = false) {
      arity(arguments, 0, 1, "Router.existsOrthogonalSegmentOverlap");
      bool(atEnds, "atEnds");
      return read(this.#ctx("existsOrthogonalSegmentOverlap"), "Router.existsOrthogonalSegmentOverlap", (raw) => raw.existsOrthogonalSegmentOverlap(atEnds));
    }
    existsOrthogonalFixedSegmentOverlap(atEnds = false) {
      arity(arguments, 0, 1, "Router.existsOrthogonalFixedSegmentOverlap");
      bool(atEnds, "atEnds");
      return read(this.#ctx("existsOrthogonalFixedSegmentOverlap"), "Router.existsOrthogonalFixedSegmentOverlap", (raw) =>
        raw.existsOrthogonalFixedSegmentOverlap(atEnds),
      );
    }
    existsOrthogonalTouchingPaths() {
      arity(arguments, 0, 0, "Router.existsOrthogonalTouchingPaths");
      return read(this.#ctx("existsOrthogonalTouchingPaths"), "Router.existsOrthogonalTouchingPaths", (raw) => raw.existsOrthogonalTouchingPaths());
    }
    existsCrossings(optimisedForConnectorType = false) {
      arity(arguments, 0, 1, "Router.existsCrossings");
      bool(optimisedForConnectorType, "optimisedForConnectorType");
      return read(this.#ctx("existsCrossings"), "Router.existsCrossings", (raw) => raw.existsCrossings(optimisedForConnectorType));
    }
    existsInvalidOrthogonalPaths() {
      arity(arguments, 0, 0, "Router.existsInvalidOrthogonalPaths");
      return read(this.#ctx("existsInvalidOrthogonalPaths"), "Router.existsInvalidOrthogonalPaths", (raw) => raw.existsInvalidOrthogonalPaths());
    }
    /** Returns the SVG upstream writes to `<instanceName>.svg` (default name "libavoid-diagram"). */
    outputDiagramSVG(instanceName = "", lineReps = null) {
      arity(arguments, 0, 2, "Router.outputDiagramSVG");
      if (typeof instanceName !== "string") throw new TypeError("instanceName must be a string");
      let flat;
      if (lineReps !== null && lineReps !== undefined) {
        if (!Array.isArray(lineReps)) throw new TypeError("lineReps must be an array of LineRep or null");
        flat = new Float64Array(lineReps.length * 4);
        lineReps.forEach((rep, i) => {
          if (!(rep instanceof LineRep)) throw new TypeError("lineReps must contain LineRep objects");
          flat.set([rep.begin.x, rep.begin.y, rep.end.x, rep.end.y], i * 4);
        });
      }
      return read(this.#ctx("outputDiagramSVG"), "Router.outputDiagramSVG", (raw) => raw.outputDiagramSVG(flat));
    }
    /** Returns the text upstream writes to `<instanceName>.txt` (default name "libavoid-diagram"). */
    outputDiagramText(instanceName = "") {
      arity(arguments, 0, 1, "Router.outputDiagramText");
      if (typeof instanceName !== "string") throw new TypeError("instanceName must be a string");
      return read(this.#ctx("outputDiagramText"), "Router.outputDiagramText", (raw) => raw.outputDiagramText());
    }
    /**
     * Upstream writes the text diagram (and, only in builds with the debug
     * define SVG_OUTPUT, the SVG instance): returns that text.
     */
    outputDiagram(instanceName = "") {
      arity(arguments, 0, 1, "Router.outputDiagram");
      if (typeof instanceName !== "string") throw new TypeError("instanceName must be a string");
      return read(this.#ctx("outputDiagram"), "Router.outputDiagram", (raw) => raw.outputDiagramText());
    }

    // public data members
    get m_obstacles() {
      const ctx = this.#ctx("m_obstacles");
      return wrapAll(ctx, read(ctx, "Router.m_obstacles", (raw) => raw.obstacles()));
    }
    get connRefs() {
      const ctx = this.#ctx("connRefs");
      return wrapAll(ctx, read(ctx, "Router.connRefs", (raw) => raw.connRefs()));
    }
    get clusterRefs() {
      const ctx = this.#ctx("clusterRefs");
      return wrapAll(ctx, read(ctx, "Router.clusterRefs", (raw) => raw.clusterRefs()));
    }
    get st_checked_edges() {
      return read(this.#ctx("st_checked_edges"), "Router.st_checked_edges", (raw) => raw.stCheckedEdges());
    }
    set st_checked_edges(value) {
      int32(value, "st_checked_edges");
      read(this.#ctx("st_checked_edges"), "Router.st_checked_edges", (raw) => raw.setStCheckedEdges(value));
    }

    /** C++ `delete router`: frees the router and everything it owns. */
    delete() {
      arity(arguments, 0, 0, "Router.delete");
      const ctx = ctxOf.get(this);
      if (!ctx || ctx.deleted) throw new Error("Router.delete: the Router has been deleted");
      if (ctx.busy) throw new Error("Router.delete: a Router cannot be deleted from inside its own shouldContinueTransactionWithProgress()");
      ctx.deleted = true;
      if (ctx.state.error) return; // the instance is gone and its objects are already detached
      guarded(ctx.state, "Router.delete", () => ctx.raw.free());
    }

    #ctx(method) {
      const ctx = ctxOf.get(this);
      if (!ctx) throw new TypeError(`Router.${method}: this is not an Avoid.Router`);
      return ctx;
    }

    static [TEST_PANIC](router, kind, arming) {
      const ctx = routerCtx(router, "__testPanic");
      return mutate(ctx, "Router.__testPanic", (raw) => raw.__testPanic(kind, arming));
    }
  }

  ROUTER_FLAG_MEMBERS.forEach((name, index) => {
    Object.defineProperty(Router.prototype, name, {
      configurable: true,
      enumerable: false,
      get() {
        return read(routerCtx(this, `Router.${name}`), `Router.${name}`, (raw) => raw.flagMember(index));
      },
      set(value) {
        bool(value, `Router.${name}`);
        read(routerCtx(this, `Router.${name}`), `Router.${name}`, (raw) => raw.setFlagMember(index, value));
      },
    });
  });

  // --- Obstacle, ShapeRef, JunctionRef -----------------------------------------------------

  // upstream: obstacle.h class Obstacle (abstract base of ShapeRef / JunctionRef)
  class Obstacle {
    constructor(token) {
      if (token !== CONSTRUCT) throw new TypeError("Obstacle is abstract: create a ShapeRef or JunctionRef");
    }
    id() {
      arity(arguments, 0, 0, "Obstacle.id");
      const e = self(this, Obstacle, "Obstacle.id");
      return read(e.ctx, "Obstacle.id", (raw) => raw.obstacleId(e.key));
    }
    polygon() {
      arity(arguments, 0, 0, "Obstacle.polygon");
      const e = self(this, Obstacle, "Obstacle.polygon");
      return decPoly(read(e.ctx, "Obstacle.polygon", (raw) => raw.obstaclePolygon(e.key)));
    }
    router() {
      arity(arguments, 0, 0, "Obstacle.router");
      return self(this, Obstacle, "Obstacle.router").ctx.router;
    }
    position() {
      arity(arguments, 0, 0, "Obstacle.position");
      const e = self(this, Obstacle, "Obstacle.position");
      return decPoint(read(e.ctx, "Obstacle.position", (raw) => raw.obstaclePosition(e.key)), 0);
    }
    setNewPoly(poly) {
      arity(arguments, 1, 1, "Obstacle.setNewPoly");
      const e = self(this, Obstacle, "Obstacle.setNewPoly");
      const enc = encPoly(poly, "poly");
      mutate(e.ctx, "Obstacle.setNewPoly", (raw) => raw.obstacleSetNewPoly(e.key, enc));
    }
    routingBox() {
      arity(arguments, 0, 0, "Obstacle.routingBox");
      const e = self(this, Obstacle, "Obstacle.routingBox");
      return decBox(read(e.ctx, "Obstacle.routingBox", (raw) => raw.obstacleRoutingBox(e.key)));
    }
    routingPolygon() {
      arity(arguments, 0, 0, "Obstacle.routingPolygon");
      const e = self(this, Obstacle, "Obstacle.routingPolygon");
      return decPoly(read(e.ctx, "Obstacle.routingPolygon", (raw) => raw.obstacleRoutingPolygon(e.key)));
    }
    attachedConnectors() {
      arity(arguments, 0, 0, "Obstacle.attachedConnectors");
      const e = self(this, Obstacle, "Obstacle.attachedConnectors");
      return wrapAll(e.ctx, read(e.ctx, "Obstacle.attachedConnectors", (raw) => raw.obstacleAttachedConnectors(e.key)));
    }
  }

  // upstream: shape.h class ShapeRef
  class ShapeRef extends Obstacle {
    constructor(router, poly, id = 0) {
      super(CONSTRUCT);
      arity(arguments, 2, 3, "ShapeRef");
      const ctx = routerCtx(router, "ShapeRef");
      const enc = encPoly(poly, "ShapeRef: poly");
      uint(id, "ShapeRef: id");
      const assigned = assignedId(ctx, id);
      register(ctx, mutate(ctx, "ShapeRef", (raw) => raw.shapeNew(enc, assigned)), this);
    }
    transformConnectionPinPositions(transform) {
      arity(arguments, 1, 1, "ShapeRef.transformConnectionPinPositions");
      uint(transform, "transform");
      const e = self(this, ShapeRef, "ShapeRef.transformConnectionPinPositions");
      mutate(e.ctx, "ShapeRef.transformConnectionPinPositions", (raw) => raw.shapeTransformConnectionPinPositions(e.key, transform));
    }
  }

  // upstream: junction.h class JunctionRef
  class JunctionRef extends Obstacle {
    constructor(router, position, id = 0) {
      super(CONSTRUCT);
      arity(arguments, 2, 3, "JunctionRef");
      const ctx = routerCtx(router, "JunctionRef");
      checkPoint(position, "JunctionRef: position");
      num(position.x, "position.x");
      num(position.y, "position.y");
      uint(id, "JunctionRef: id");
      const assigned = assignedId(ctx, id);
      register(ctx, mutate(ctx, "JunctionRef", (raw) => raw.junctionNew(position.x, position.y, assigned)), this);
    }
    removeJunctionAndMergeConnectors() {
      arity(arguments, 0, 0, "JunctionRef.removeJunctionAndMergeConnectors");
      const e = self(this, JunctionRef, "JunctionRef.removeJunctionAndMergeConnectors");
      const key = mutate(e.ctx, "JunctionRef.removeJunctionAndMergeConnectors", (raw) => raw.junctionRemoveJunctionAndMergeConnectors(e.key));
      return wrap(e.ctx, key);
    }
    setPositionFixed(fixed) {
      arity(arguments, 1, 1, "JunctionRef.setPositionFixed");
      bool(fixed, "fixed");
      const e = self(this, JunctionRef, "JunctionRef.setPositionFixed");
      mutate(e.ctx, "JunctionRef.setPositionFixed", (raw) => raw.junctionSetPositionFixed(e.key, fixed));
    }
    positionFixed() {
      arity(arguments, 0, 0, "JunctionRef.positionFixed");
      const e = self(this, JunctionRef, "JunctionRef.positionFixed");
      return read(e.ctx, "JunctionRef.positionFixed", (raw) => raw.junctionPositionFixed(e.key));
    }
    recommendedPosition() {
      arity(arguments, 0, 0, "JunctionRef.recommendedPosition");
      const e = self(this, JunctionRef, "JunctionRef.recommendedPosition");
      return decPoint(read(e.ctx, "JunctionRef.recommendedPosition", (raw) => raw.junctionRecommendedPosition(e.key)), 0);
    }
    makeRectangle(router, position) {
      arity(arguments, 2, 2, "JunctionRef.makeRectangle");
      self(this, JunctionRef, "JunctionRef.makeRectangle");
      const ctx = routerCtx(router, "JunctionRef.makeRectangle");
      const enc = [];
      pushPoint(enc, position, "position");
      return decPoly(read(ctx, "JunctionRef.makeRectangle", (raw) => raw.junctionMakeRectangle(enc[0], enc[1], enc[2], enc[3])));
    }
    preferOrthogonalDimension(dim) {
      arity(arguments, 1, 1, "JunctionRef.preferOrthogonalDimension");
      size(dim, "dim");
      const e = self(this, JunctionRef, "JunctionRef.preferOrthogonalDimension");
      mutate(e.ctx, "JunctionRef.preferOrthogonalDimension", (raw) => raw.junctionPreferOrthogonalDimension(e.key, dim));
    }
  }

  // --- ConnEnd, Checkpoint, ConnRef ---------------------------------------------------------

  const connEndState = new WeakMap();

  // upstream: connend.h class ConnEnd (a value type)
  class ConnEnd {
    constructor(a, b) {
      arity(arguments, 0, 2, "ConnEnd");
      const s = {
        type: CONSTANTS.ConnEndEmpty,
        point: new Point(0, 0),
        directions: CONSTANTS.ConnDirAll,
        classId: CONSTANTS.CONNECTIONPIN_UNSET,
        anchor: null,
        activePin: null,
        connRef: null,
      };
      connEndState.set(this, s);
      if (arguments.length === 0) return;
      if (a instanceof Point) {
        num(a.x, "point.x");
        num(a.y, "point.y");
        s.type = CONSTANTS.ConnEndPoint;
        s.point = copyPoint(a);
        if (arguments.length === 2) s.directions = dirs(b, "ConnEnd: visDirs");
      } else if (a instanceof ShapeRef) {
        arity(arguments, 2, 2, "ConnEnd(shapeRef, connectionPinClassID)");
        uint(b, "ConnEnd: connectionPinClassID");
        const e = self(a, ShapeRef, "ConnEnd");
        setConnEnd(e.ctx, s, read(e.ctx, "ConnEnd", (raw) => raw.connEndShape(e.key, b)), 0);
      } else if (a instanceof JunctionRef) {
        arity(arguments, 1, 1, "ConnEnd(junctionRef)");
        const e = self(a, JunctionRef, "ConnEnd");
        setConnEnd(e.ctx, s, read(e.ctx, "ConnEnd", (raw) => raw.connEndJunction(e.key)), 0);
      } else {
        throw new TypeError("ConnEnd: expected (Point), (Point, visDirs), (ShapeRef, connectionPinClassID) or (JunctionRef)");
      }
    }
    type() {
      arity(arguments, 0, 0, "ConnEnd.type");
      return connEndOf(this, "ConnEnd.type").type;
    }
    position() {
      arity(arguments, 0, 0, "ConnEnd.position");
      const s = connEndOf(this, "ConnEnd.position");
      const ctx = connEndCtx(s);
      if (!ctx) return copyPoint(s.point);
      const enc = encConnEnd(ctx, this, "ConnEnd.position");
      return decPoint(read(ctx, "ConnEnd.position", (raw) => raw.connEndQuery(enc)), 0);
    }
    directions() {
      arity(arguments, 0, 0, "ConnEnd.directions");
      const s = connEndOf(this, "ConnEnd.directions");
      const ctx = connEndCtx(s);
      if (!ctx || !s.activePin) return s.directions;
      const enc = encConnEnd(ctx, this, "ConnEnd.directions");
      return read(ctx, "ConnEnd.directions", (raw) => raw.connEndQuery(enc))[4];
    }
    shape() {
      arity(arguments, 0, 0, "ConnEnd.shape");
      const a = connEndOf(this, "ConnEnd.shape").anchor;
      return a instanceof ShapeRef ? a : null;
    }
    junction() {
      arity(arguments, 0, 0, "ConnEnd.junction");
      const a = connEndOf(this, "ConnEnd.junction").anchor;
      return a instanceof JunctionRef ? a : null;
    }
    pinClassId() {
      arity(arguments, 0, 0, "ConnEnd.pinClassId");
      return connEndOf(this, "ConnEnd.pinClassId").classId;
    }
    delete() {}
  }

  function copyPoint(p) {
    const q = new Point(p.x, p.y);
    q.id = p.id;
    q.vn = p.vn;
    return q;
  }

  function connEndOf(end, label) {
    const s = end instanceof ConnEnd ? connEndState.get(end) : undefined;
    if (!s) throw new TypeError(`${label}: expected an Avoid.ConnEnd`);
    return s;
  }

  function connEndCtx(s) {
    const obj = s.anchor ?? s.activePin ?? s.connRef;
    return obj ? info.get(obj).ctx : null;
  }

  function setConnEnd(ctx, s, v, i) {
    s.type = v[i];
    s.point = decPoint(v, i + 1);
    s.directions = v[i + 5];
    s.classId = v[i + 6];
    s.anchor = wrap(ctx, v[i + 7]);
    s.activePin = wrap(ctx, v[i + 8]);
    s.connRef = wrap(ctx, v[i + 9]);
  }

  function decConnEnd(ctx, v, i) {
    const end = new ConnEnd();
    setConnEnd(ctx, connEndState.get(end), v, i);
    return end;
  }

  /** Appends the encoding of `end` for use in router `ctx` to `out`. */
  function encConnEnd(ctx, end, label, out = []) {
    const s = connEndOf(end, label);
    const objKey = (obj, Cls) => {
      if (obj === null) return 0;
      const e = info.get(obj);
      if (e.ctx !== ctx) throw new Error(`${label}: the ConnEnd refers to an object of another Router`);
      if (Cls === Obstacle && e.deleted) throw new Error(`${label}: the ConnEnd refers to a deleted ${obj.constructor.name}`);
      return e.key;
    };
    out.push(s.type);
    pushPoint(out, s.point, `${label}: ConnEnd point`);
    out.push(s.directions, s.classId, objKey(s.anchor, Obstacle), objKey(s.activePin, ShapeConnectionPin), objKey(s.connRef, ConnRef));
    return out;
  }

  // upstream: connector.h class Checkpoint
  class Checkpoint {
    constructor(p, ad, dd) {
      if (arguments.length === 0) {
        this.point = new Point();
        this.arrivalDirections = CONSTANTS.ConnDirAll;
        this.departureDirections = CONSTANTS.ConnDirAll;
      } else if (arguments.length === 1 || arguments.length === 3) {
        checkPoint(p, "Checkpoint: p");
        this.point = copyPoint(p);
        this.arrivalDirections = arguments.length === 3 ? dirs(ad, "Checkpoint: ad") : CONSTANTS.ConnDirAll;
        this.departureDirections = arguments.length === 3 ? dirs(dd, "Checkpoint: dd") : CONSTANTS.ConnDirAll;
      } else {
        throw new TypeError(`Checkpoint: expected (), (p) or (p, ad, dd), got ${arguments.length} arguments`);
      }
    }
    delete() {}
  }

  // upstream: connector.h class ConnRef
  class ConnRef {
    constructor(router, a, b, c) {
      const label = "ConnRef";
      const ctx = routerCtx(router, label);
      let key;
      if (a instanceof ConnEnd) {
        arity(arguments, 3, 4, label);
        const id = dflt(c, 0);
        uint(id, "ConnRef: id");
        const enc = encConnEnd(ctx, a, "ConnRef: src");
        encConnEnd(ctx, b, "ConnRef: dst", enc);
        const assigned = assignedId(ctx, id);
        key = mutate(ctx, label, (raw) => raw.connNewWithEnds(enc, assigned));
      } else {
        arity(arguments, 1, 2, label);
        const id = dflt(a, 0);
        uint(id, "ConnRef: id");
        const assigned = assignedId(ctx, id);
        key = mutate(ctx, label, (raw) => raw.connNew(assigned));
      }
      register(ctx, key, this);
    }
    setEndpoints(srcPoint, dstPoint) {
      arity(arguments, 2, 2, "ConnRef.setEndpoints");
      const e = self(this, ConnRef, "ConnRef.setEndpoints");
      const enc = encConnEnd(e.ctx, srcPoint, "ConnRef.setEndpoints: srcPoint");
      encConnEnd(e.ctx, dstPoint, "ConnRef.setEndpoints: dstPoint", enc);
      mutate(e.ctx, "ConnRef.setEndpoints", (raw) => raw.connSetEnds(e.key, 0, enc));
    }
    setSourceEndpoint(srcPoint) {
      arity(arguments, 1, 1, "ConnRef.setSourceEndpoint");
      const e = self(this, ConnRef, "ConnRef.setSourceEndpoint");
      const enc = encConnEnd(e.ctx, srcPoint, "ConnRef.setSourceEndpoint: srcPoint");
      mutate(e.ctx, "ConnRef.setSourceEndpoint", (raw) => raw.connSetEnds(e.key, 1, enc));
    }
    setDestEndpoint(dstPoint) {
      arity(arguments, 1, 1, "ConnRef.setDestEndpoint");
      const e = self(this, ConnRef, "ConnRef.setDestEndpoint");
      const enc = encConnEnd(e.ctx, dstPoint, "ConnRef.setDestEndpoint: dstPoint");
      mutate(e.ctx, "ConnRef.setDestEndpoint", (raw) => raw.connSetEnds(e.key, 2, enc));
    }
    /** setEndpoint(type, connEnd) with type 1 (VertID::src) or 2 (VertID::tar). */
    setEndpoint(type, connEnd) {
      arity(arguments, 2, 2, "ConnRef.setEndpoint");
      uint(type, "type");
      if (type !== 1 && type !== 2) throw new RangeError("ConnRef.setEndpoint: type must be 1 (VertID::src) or 2 (VertID::tar) (COLA_ASSERT)");
      const e = self(this, ConnRef, "ConnRef.setEndpoint");
      if (!(connEnd instanceof ConnEnd)) {
        throw new TypeError("ConnRef.setEndpoint: the (type, VertID, Point*) overload is internal; pass an Avoid.ConnEnd");
      }
      const enc = encConnEnd(e.ctx, connEnd, "ConnRef.setEndpoint: connEnd");
      mutate(e.ctx, "ConnRef.setEndpoint", (raw) => raw.connSetEnds(e.key, type + 2, enc));
    }
    id() {
      arity(arguments, 0, 0, "ConnRef.id");
      const e = self(this, ConnRef, "ConnRef.id");
      return read(e.ctx, "ConnRef.id", (raw) => raw.connId(e.key));
    }
    router() {
      arity(arguments, 0, 0, "ConnRef.router");
      return self(this, ConnRef, "ConnRef.router").ctx.router;
    }
    needsRepaint() {
      arity(arguments, 0, 0, "ConnRef.needsRepaint");
      const e = self(this, ConnRef, "ConnRef.needsRepaint");
      return read(e.ctx, "ConnRef.needsRepaint", (raw) => raw.connNeedsRepaint(e.key));
    }
    route() {
      arity(arguments, 0, 0, "ConnRef.route");
      const e = self(this, ConnRef, "ConnRef.route");
      return decPoly(read(e.ctx, "ConnRef.route", (raw) => raw.connRoute(e.key)));
    }
    displayRoute() {
      arity(arguments, 0, 0, "ConnRef.displayRoute");
      const e = self(this, ConnRef, "ConnRef.displayRoute");
      return decPoly(read(e.ctx, "ConnRef.displayRoute", (raw) => raw.connDisplayRoute(e.key)));
    }
    /** upstream setCallback(cb, ptr): `cb(ptr)` is invoked whenever the route changed. */
    setCallback(cb, ptr) {
      arity(arguments, 2, 2, "ConnRef.setCallback");
      if (cb !== null && typeof cb !== "function") throw new TypeError("ConnRef.setCallback: cb must be a function or null");
      const e = self(this, ConnRef, "ConnRef.setCallback");
      read(e.ctx, "ConnRef.setCallback", (raw) => raw.connSetCallback(e.key, cb !== null));
      e.cb = cb;
      e.ptr = ptr;
    }
    routingType() {
      arity(arguments, 0, 0, "ConnRef.routingType");
      const e = self(this, ConnRef, "ConnRef.routingType");
      return read(e.ctx, "ConnRef.routingType", (raw) => raw.connRoutingType(e.key));
    }
    setRoutingType(type) {
      arity(arguments, 1, 1, "ConnRef.setRoutingType");
      uint(type, "type");
      const e = self(this, ConnRef, "ConnRef.setRoutingType");
      mutate(e.ctx, "ConnRef.setRoutingType", (raw) => raw.connSetRoutingType(e.key, type));
    }
    splitAtSegment(segmentN) {
      arity(arguments, 1, 1, "ConnRef.splitAtSegment");
      size(segmentN, "segmentN");
      const e = self(this, ConnRef, "ConnRef.splitAtSegment");
      const [j, c] = mutate(e.ctx, "ConnRef.splitAtSegment", (raw) => raw.connSplitAtSegment(e.key, segmentN));
      return [wrap(e.ctx, j), wrap(e.ctx, c)];
    }
    setRoutingCheckpoints(checkpoints) {
      arity(arguments, 1, 1, "ConnRef.setRoutingCheckpoints");
      array(checkpoints, "checkpoints");
      const enc = [];
      checkpoints.forEach((cp, i) => {
        if (!(cp instanceof Checkpoint)) throw new TypeError(`checkpoints[${i}] must be an Avoid.Checkpoint`);
        pushPoint(enc, cp.point, `checkpoints[${i}].point`);
        enc.push(dirs(cp.arrivalDirections, `checkpoints[${i}].arrivalDirections`), dirs(cp.departureDirections, `checkpoints[${i}].departureDirections`));
      });
      const e = self(this, ConnRef, "ConnRef.setRoutingCheckpoints");
      mutate(e.ctx, "ConnRef.setRoutingCheckpoints", (raw) => raw.connSetRoutingCheckpoints(e.key, enc));
    }
    routingCheckpoints() {
      arity(arguments, 0, 0, "ConnRef.routingCheckpoints");
      const e = self(this, ConnRef, "ConnRef.routingCheckpoints");
      const v = read(e.ctx, "ConnRef.routingCheckpoints", (raw) => raw.connRoutingCheckpoints(e.key));
      const out = [];
      for (let i = 0; i < v.length; i += 6) out.push(new Checkpoint(decPoint(v, i), v[i + 4], v[i + 5]));
      return out;
    }
    endpointConnEnds() {
      arity(arguments, 0, 0, "ConnRef.endpointConnEnds");
      const e = self(this, ConnRef, "ConnRef.endpointConnEnds");
      const v = read(e.ctx, "ConnRef.endpointConnEnds", (raw) => raw.connEndpointConnEnds(e.key));
      return [decConnEnd(e.ctx, v, 0), decConnEnd(e.ctx, v, 10)];
    }
    setFixedRoute(route) {
      arity(arguments, 1, 1, "ConnRef.setFixedRoute");
      const e = self(this, ConnRef, "ConnRef.setFixedRoute");
      const enc = encPoly(route, "route");
      mutate(e.ctx, "ConnRef.setFixedRoute", (raw) => raw.connSetFixedRoute(e.key, enc));
    }
    setFixedExistingRoute() {
      arity(arguments, 0, 0, "ConnRef.setFixedExistingRoute");
      const e = self(this, ConnRef, "ConnRef.setFixedExistingRoute");
      mutate(e.ctx, "ConnRef.setFixedExistingRoute", (raw) => raw.connSetFixedExistingRoute(e.key));
    }
    hasFixedRoute() {
      arity(arguments, 0, 0, "ConnRef.hasFixedRoute");
      const e = self(this, ConnRef, "ConnRef.hasFixedRoute");
      return read(e.ctx, "ConnRef.hasFixedRoute", (raw) => raw.connHasFixedRoute(e.key));
    }
    // From the libavoid-js fork of Adaptagrams (no upstream counterpart).
    hasValidRoute() {
      arity(arguments, 0, 0, "ConnRef.hasValidRoute");
      const e = self(this, ConnRef, "ConnRef.hasValidRoute");
      return read(e.ctx, "ConnRef.hasValidRoute", (raw) => raw.connHasValidRoute(e.key));
    }
    hasCrossingObstacles() {
      arity(arguments, 0, 0, "ConnRef.hasCrossingObstacles");
      const e = self(this, ConnRef, "ConnRef.hasCrossingObstacles");
      return read(e.ctx, "ConnRef.hasCrossingObstacles", (raw) => raw.connHasCrossingObstacles(e.key));
    }
    clearFixedRoute() {
      arity(arguments, 0, 0, "ConnRef.clearFixedRoute");
      const e = self(this, ConnRef, "ConnRef.clearFixedRoute");
      mutate(e.ctx, "ConnRef.clearFixedRoute", (raw) => raw.connClearFixedRoute(e.key));
    }
    set_route(route) {
      arity(arguments, 1, 1, "ConnRef.set_route");
      const e = self(this, ConnRef, "ConnRef.set_route");
      const enc = encPoly(route, "route");
      read(e.ctx, "ConnRef.set_route", (raw) => raw.connSetRoute(e.key, enc));
    }
    calcRouteDist() {
      arity(arguments, 0, 0, "ConnRef.calcRouteDist");
      const e = self(this, ConnRef, "ConnRef.calcRouteDist");
      read(e.ctx, "ConnRef.calcRouteDist", (raw) => raw.connCalcRouteDist(e.key));
    }
    makeActive() {
      arity(arguments, 0, 0, "ConnRef.makeActive");
      const e = self(this, ConnRef, "ConnRef.makeActive");
      read(e.ctx, "ConnRef.makeActive", (raw) => raw.connMakeActive(e.key));
    }
    makeInactive() {
      arity(arguments, 0, 0, "ConnRef.makeInactive");
      const e = self(this, ConnRef, "ConnRef.makeInactive");
      read(e.ctx, "ConnRef.makeInactive", (raw) => raw.connMakeInactive(e.key));
    }
    removeFromGraph() {
      arity(arguments, 0, 0, "ConnRef.removeFromGraph");
      const e = self(this, ConnRef, "ConnRef.removeFromGraph");
      read(e.ctx, "ConnRef.removeFromGraph", (raw) => raw.connRemoveFromGraph(e.key));
    }
    isInitialised() {
      arity(arguments, 0, 0, "ConnRef.isInitialised");
      const e = self(this, ConnRef, "ConnRef.isInitialised");
      return read(e.ctx, "ConnRef.isInitialised", (raw) => raw.connIsInitialised(e.key));
    }
    makePathInvalid() {
      arity(arguments, 0, 0, "ConnRef.makePathInvalid");
      const e = self(this, ConnRef, "ConnRef.makePathInvalid");
      read(e.ctx, "ConnRef.makePathInvalid", (raw) => raw.connMakePathInvalid(e.key));
    }
    setHateCrossings(value) {
      arity(arguments, 1, 1, "ConnRef.setHateCrossings");
      bool(value, "value");
      const e = self(this, ConnRef, "ConnRef.setHateCrossings");
      mutate(e.ctx, "ConnRef.setHateCrossings", (raw) => raw.connSetHateCrossings(e.key, value));
    }
    doesHateCrossings() {
      arity(arguments, 0, 0, "ConnRef.doesHateCrossings");
      const e = self(this, ConnRef, "ConnRef.doesHateCrossings");
      return read(e.ctx, "ConnRef.doesHateCrossings", (raw) => raw.connDoesHateCrossings(e.key));
    }
    possibleDstPinPoints() {
      arity(arguments, 0, 0, "ConnRef.possibleDstPinPoints");
      const e = self(this, ConnRef, "ConnRef.possibleDstPinPoints");
      return decPoints(read(e.ctx, "ConnRef.possibleDstPinPoints", (raw) => raw.connPossibleDstPinPoints(e.key)));
    }
  }

  // --- ShapeConnectionPin -----------------------------------------------------------------

  // upstream: connectionpin.h class ShapeConnectionPin
  class ShapeConnectionPin {
    constructor(owner, classId, a, b, c, d, f) {
      const label = "ShapeConnectionPin";
      let ctx;
      let key;
      if (owner instanceof ShapeRef) {
        arity(arguments, 6, 7, label);
        const e = self(owner, ShapeRef, label);
        ctx = e.ctx;
        uint(classId, "classId");
        num(a, "xOffset");
        num(b, "yOffset");
        if (arguments.length === 7) {
          bool(c, "proportional");
          num(d, "insideOffset");
          dirs(f, "visDirs");
          key = mutate(ctx, label, (raw) => raw.pinNewShape(e.key, classId, a, b, c, d, f));
        } else {
          num(c, "insideOffset");
          dirs(d, "visDirs");
          key = mutate(ctx, label, (raw) => raw.pinNewShapeNoProportional(e.key, classId, a, b, c, d));
        }
      } else if (owner instanceof JunctionRef) {
        arity(arguments, 2, 3, label);
        const e = self(owner, JunctionRef, label);
        ctx = e.ctx;
        uint(classId, "classId");
        const visDirs = dirs(dflt(a, CONSTANTS.ConnDirNone), "visDirs");
        key = mutate(ctx, label, (raw) => raw.pinNewJunction(e.key, classId, visDirs));
      } else {
        throw new TypeError("ShapeConnectionPin: the first argument must be an Avoid.ShapeRef or Avoid.JunctionRef");
      }
      register(ctx, key, this);
    }
    /** C++ `delete pin`: removes the pin from its shape / junction. */
    delete() {
      arity(arguments, 0, 0, "ShapeConnectionPin.delete");
      const e = self(this, ShapeConnectionPin, "ShapeConnectionPin.delete");
      mutate(e.ctx, "ShapeConnectionPin.delete", (raw) => raw.pinDelete(e.key));
      e.deleted = true;
    }
    setConnectionCost(cost) {
      arity(arguments, 1, 1, "ShapeConnectionPin.setConnectionCost");
      num(cost, "cost");
      const e = self(this, ShapeConnectionPin, "ShapeConnectionPin.setConnectionCost");
      read(e.ctx, "ShapeConnectionPin.setConnectionCost", (raw) => raw.pinSetConnectionCost(e.key, cost));
    }
    position(newPoly = undefined) {
      arity(arguments, 0, 1, "ShapeConnectionPin.position");
      const e = self(this, ShapeConnectionPin, "ShapeConnectionPin.position");
      const enc = encPoly(newPoly === undefined ? new Polygon() : newPoly, "newPoly");
      return decPoint(read(e.ctx, "ShapeConnectionPin.position", (raw) => raw.pinPosition(e.key, enc)), 0);
    }
    directions() {
      arity(arguments, 0, 0, "ShapeConnectionPin.directions");
      const e = self(this, ShapeConnectionPin, "ShapeConnectionPin.directions");
      return read(e.ctx, "ShapeConnectionPin.directions", (raw) => raw.pinDirections(e.key));
    }
    setExclusive(exclusive) {
      arity(arguments, 1, 1, "ShapeConnectionPin.setExclusive");
      bool(exclusive, "exclusive");
      const e = self(this, ShapeConnectionPin, "ShapeConnectionPin.setExclusive");
      read(e.ctx, "ShapeConnectionPin.setExclusive", (raw) => raw.pinSetExclusive(e.key, exclusive));
    }
    isExclusive() {
      arity(arguments, 0, 0, "ShapeConnectionPin.isExclusive");
      const e = self(this, ShapeConnectionPin, "ShapeConnectionPin.isExclusive");
      return read(e.ctx, "ShapeConnectionPin.isExclusive", (raw) => raw.pinIsExclusive(e.key));
    }
    /** ConnectionPinIds (std::pair): [containing object id, class id]. */
    ids() {
      arity(arguments, 0, 0, "ShapeConnectionPin.ids");
      const e = self(this, ShapeConnectionPin, "ShapeConnectionPin.ids");
      return Array.from(read(e.ctx, "ShapeConnectionPin.ids", (raw) => raw.pinIds(e.key)));
    }
    /** operator== */
    equal(rhs) {
      return this.#compare(rhs, false, "ShapeConnectionPin.equal", arguments);
    }
    /** operator< */
    lessThan(rhs) {
      return this.#compare(rhs, true, "ShapeConnectionPin.lessThan", arguments);
    }
    #compare(rhs, less, label, args) {
      arity(args, 1, 1, label);
      const e = self(this, ShapeConnectionPin, label);
      const other = keyIn(e.ctx, rhs, ShapeConnectionPin, label, "rhs");
      return read(e.ctx, label, (raw) => raw.pinCompare(e.key, other, less));
    }
  }

  // --- ClusterRef ------------------------------------------------------------------------------

  // upstream: viscluster.h class ClusterRef
  class ClusterRef {
    constructor(router, poly, id = 0) {
      arity(arguments, 2, 3, "ClusterRef");
      const ctx = routerCtx(router, "ClusterRef");
      const enc = encPoly(poly, "ClusterRef: poly");
      uint(id, "ClusterRef: id");
      const assigned = assignedId(ctx, id);
      register(ctx, mutate(ctx, "ClusterRef", (raw) => raw.clusterNew(enc, assigned)), this);
    }
    setNewPoly(poly) {
      arity(arguments, 1, 1, "ClusterRef.setNewPoly");
      const e = self(this, ClusterRef, "ClusterRef.setNewPoly");
      const enc = encPoly(poly, "poly");
      mutate(e.ctx, "ClusterRef.setNewPoly", (raw) => raw.clusterSetNewPoly(e.key, enc));
    }
    id() {
      arity(arguments, 0, 0, "ClusterRef.id");
      const e = self(this, ClusterRef, "ClusterRef.id");
      return read(e.ctx, "ClusterRef.id", (raw) => raw.clusterId(e.key));
    }
    polygon() {
      arity(arguments, 0, 0, "ClusterRef.polygon");
      const e = self(this, ClusterRef, "ClusterRef.polygon");
      return decReferencing(e.ctx, read(e.ctx, "ClusterRef.polygon", (raw) => raw.clusterPolygon(e.key)));
    }
    rectangularPolygon() {
      arity(arguments, 0, 0, "ClusterRef.rectangularPolygon");
      const e = self(this, ClusterRef, "ClusterRef.rectangularPolygon");
      return decPoly(read(e.ctx, "ClusterRef.rectangularPolygon", (raw) => raw.clusterRectangularPolygon(e.key)));
    }
    router() {
      arity(arguments, 0, 0, "ClusterRef.router");
      return self(this, ClusterRef, "ClusterRef.router").ctx.router;
    }
    makeActive() {
      arity(arguments, 0, 0, "ClusterRef.makeActive");
      const e = self(this, ClusterRef, "ClusterRef.makeActive");
      read(e.ctx, "ClusterRef.makeActive", (raw) => raw.clusterMakeActive(e.key));
    }
    makeInactive() {
      arity(arguments, 0, 0, "ClusterRef.makeInactive");
      const e = self(this, ClusterRef, "ClusterRef.makeInactive");
      read(e.ctx, "ClusterRef.makeInactive", (raw) => raw.clusterMakeInactive(e.key));
    }
  }

  // --- hyperedges --------------------------------------------------------------------------------

  // upstream: hyperedge.h struct HyperedgeNewAndDeletedObjectLists
  class HyperedgeNewAndDeletedObjectLists {
    constructor() {
      arity(arguments, 0, 0, "HyperedgeNewAndDeletedObjectLists");
      this.newJunctionList = [];
      this.newConnectorList = [];
      this.deletedJunctionList = [];
      this.deletedConnectorList = [];
      this.changedConnectorList = [];
    }
    delete() {}
  }

  // A rerouter made by `new HyperedgeRerouter()` belongs to no router (its
  // router is set only by the Router that owns it, privately upstream): it
  // records registrations but never reroutes.
  const rerouterCtx = new WeakMap();
  const detachedRegistrations = new WeakMap();

  // upstream: hyperedge.h class HyperedgeRerouter
  class HyperedgeRerouter {
    constructor() {
      arity(arguments, 0, 0, "HyperedgeRerouter");
      detachedRegistrations.set(this, []);
    }
    registerHyperedgeForRerouting(arg) {
      const label = "HyperedgeRerouter.registerHyperedgeForRerouting";
      arity(arguments, 1, 1, label);
      const ctx = rerouterCtx.get(this);
      if (!ctx) {
        const list = detachedRegistrations.get(this);
        if (!list) throw new TypeError(`${label}: this is not an Avoid.HyperedgeRerouter`);
        if (arg instanceof JunctionRef) objInfo(arg, JunctionRef, label, "junction");
        else array(arg, "terminals").forEach((t) => connEndOf(t, label));
        list.push(arg);
        return list.length - 1;
      }
      if (arg instanceof JunctionRef) {
        const key = keyIn(ctx, arg, JunctionRef, label, "junction");
        return mutate(ctx, label, (raw) => raw.registerHyperedgeForReroutingJunction(key));
      }
      array(arg, "terminals");
      const enc = [];
      arg.forEach((end, i) => encConnEnd(ctx, end, `${label}: terminals[${i}]`, enc));
      return mutate(ctx, label, (raw) => raw.registerHyperedgeForReroutingTerminals(enc));
    }
    newAndDeletedObjectLists(index) {
      const label = "HyperedgeRerouter.newAndDeletedObjectLists";
      arity(arguments, 1, 1, label);
      size(index, "index");
      const ctx = rerouterCtx.get(this);
      if (!ctx) {
        if (!detachedRegistrations.has(this)) throw new TypeError(`${label}: this is not an Avoid.HyperedgeRerouter`);
        return new HyperedgeNewAndDeletedObjectLists();
      }
      return decLists(ctx, read(ctx, label, (raw) => raw.hyperedgeNewAndDeletedObjectLists(index)));
    }
    count() {
      const label = "HyperedgeRerouter.count";
      arity(arguments, 0, 0, label);
      const ctx = rerouterCtx.get(this);
      if (!ctx) {
        const list = detachedRegistrations.get(this);
        if (!list) throw new TypeError(`${label}: this is not an Avoid.HyperedgeRerouter`);
        return list.length;
      }
      return read(ctx, label, (raw) => raw.hyperedgeCount());
    }
  }

  // --- geometry.h free functions --------------------------------------------------------------

  function geo(label, which, points, a = 0, b = 0) {
    const ps = [];
    points.forEach((p, i) => {
      checkPoint(p, `${label}: point ${i + 1}`);
      ps.push(num(p.x, `${label}: x`), num(p.y, `${label}: y`));
    });
    return free(label, (w) => w.geometry(which, ps, a, b));
  }

  function polyPoints(poly, label) {
    if (!(poly instanceof PolygonInterface)) throw new TypeError(`${label}: poly must be an Avoid.Polygon`);
    const pts = [];
    const n = poly.size();
    for (let i = 0; i < n; i++) pts.push(poly.at(i));
    return pts;
  }

  function scalarOut(v, name) {
    if (!Array.isArray(v) || v.length < 1) throw new TypeError(`${name} must be a one-element array (out-parameter)`);
    return v;
  }

  const geometry = {
    euclideanDist(a, b) {
      arity(arguments, 2, 2, "euclideanDist");
      return geo("euclideanDist", 0, [a, b])[0];
    },
    manhattanDist(a, b) {
      arity(arguments, 2, 2, "manhattanDist");
      return geo("manhattanDist", 1, [a, b])[0];
    },
    totalLength(poly) {
      arity(arguments, 1, 1, "totalLength");
      return geo("totalLength", 2, polyPoints(poly, "totalLength"))[0];
    },
    angle(a, b, c) {
      arity(arguments, 3, 3, "angle");
      return geo("angle", 3, [a, b, c])[0];
    },
    segmentIntersect(a, b, c, d) {
      arity(arguments, 4, 4, "segmentIntersect");
      return geo("segmentIntersect", 4, [a, b, c, d])[0] !== 0;
    },
    /** `seenIntersectionAtEndpoint` is a one-element array (bool& in/out). */
    segmentShapeIntersect(e1, e2, s1, s2, seenIntersectionAtEndpoint) {
      arity(arguments, 5, 5, "segmentShapeIntersect");
      const seen = scalarOut(seenIntersectionAtEndpoint, "seenIntersectionAtEndpoint");
      const r = geo("segmentShapeIntersect", 5, [e1, e2, s1, s2], bool(seen[0], "seenIntersectionAtEndpoint[0]") ? 1 : 0);
      seen[0] = r[1] !== 0;
      return r[0] !== 0;
    },
    inPoly(poly, q, countBorder = true) {
      arity(arguments, 2, 3, "inPoly");
      bool(countBorder, "countBorder");
      return geo("inPoly", 6, [...polyPoints(poly, "inPoly"), q], countBorder ? 1 : 0)[0] !== 0;
    },
    inPolyGen(poly, q) {
      arity(arguments, 2, 2, "inPolyGen");
      return geo("inPolyGen", 7, [...polyPoints(poly, "inPolyGen"), q])[0] !== 0;
    },
    inValidRegion(IgnoreRegions, a0, a1, a2, b) {
      arity(arguments, 5, 5, "inValidRegion");
      bool(IgnoreRegions, "IgnoreRegions");
      return geo("inValidRegion", 8, [a0, a1, a2, b], IgnoreRegions ? 1 : 0)[0] !== 0;
    },
    cornerSide(c1, c2, c3, p) {
      arity(arguments, 4, 4, "cornerSide");
      return geo("cornerSide", 9, [c1, c2, c3, p])[0];
    },
    pointOnLine(a, b, c, tolerance = 0.0) {
      arity(arguments, 3, 4, "pointOnLine");
      return geo("pointOnLine", 10, [a, b, c], num(tolerance, "tolerance"))[0] !== 0;
    },
    colinear(a, b, c, tolerance = 0.0) {
      arity(arguments, 3, 4, "colinear");
      return geo("colinear", 11, [a, b, c], num(tolerance, "tolerance"))[0] !== 0;
    },
    inBetween(a, b, c) {
      arity(arguments, 3, 3, "inBetween");
      return geo("inBetween", 12, [a, b, c])[0] !== 0;
    },
    vecDir(a, b, c, maybeZero = 0.0) {
      arity(arguments, 3, 4, "vecDir");
      return geo("vecDir", 13, [a, b, c], num(maybeZero, "maybeZero"))[0];
    },
    projection(a, b, c) {
      arity(arguments, 3, 3, "projection");
      const r = geo("projection", 14, [a, b, c]);
      return new Point(r[0], r[1]);
    },
    /** `x` and `y` are one-element arrays (double* out-parameters). */
    segmentIntersectPoint(a1, a2, b1, b2, x, y) {
      arity(arguments, 6, 6, "segmentIntersectPoint");
      return intersectPoint("segmentIntersectPoint", 15, [a1, a2, b1, b2], x, y);
    },
    /** `x` and `y` are one-element arrays (double* out-parameters). */
    rayIntersectPoint(a1, a2, b1, b2, x, y) {
      arity(arguments, 6, 6, "rayIntersectPoint");
      return intersectPoint("rayIntersectPoint", 16, [a1, a2, b1, b2], x, y);
    },
    rotationalAngle(p) {
      arity(arguments, 1, 1, "rotationalAngle");
      return geo("rotationalAngle", 17, [p])[0];
    },
  };

  function intersectPoint(label, which, points, x, y) {
    scalarOut(x, "x");
    scalarOut(y, "y");
    const r = geo(label, which, points, typeof x[0] === "number" ? x[0] : 0, typeof y[0] === "number" ? y[0] : 0);
    x[0] = r[1];
    y[0] = r[2];
    return r[0];
  }

  // --- WebAssembly infrastructure (not upstream API) ---------------------------------------------

  function adopt(fresh) {
    if (!fresh || typeof fresh.RawRouter !== "function") {
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
    ...CONSTANTS,
    ...ENUMS,
    PenaltyType,
    Point,
    Vector: Point,
    Box,
    Edge,
    LineRep,
    PolygonInterface,
    Polygon,
    PolyLine: Polygon,
    ReferencingPolygon,
    Rectangle,
    Router,
    Obstacle,
    ShapeRef,
    JunctionRef,
    ConnEnd,
    Checkpoint,
    ConnRef,
    ShapeConnectionPin,
    ClusterRef,
    HyperedgeRerouter,
    HyperedgeNewAndDeletedObjectLists,
    ...geometry,
    AvoidInternalError,
    internalError: () => current.error,
    reset,
  };
}

module.exports = createAvoidApi;
