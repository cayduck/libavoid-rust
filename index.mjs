// Browser / ES-module entry of libavoid-rust (wasm-pack `web` target in
// pkg-web/).  The API is the one of index.cjs (same adapter, lib/adapter.cjs,
// compiled to lib/adapter.mjs by scripts/prepare-pkg.cjs); the only
// difference is that the wasm module is loaded asynchronously, so
// `await init()` must complete before the first `new Router()`:
//
//   import init, * as Avoid from "libavoid-rust";
//   await init();                     // fetches pkg-web/libavoid_rust_bg.wasm
//   const router = new Avoid.Router(Avoid.OrthogonalRouting);
//
// `init(input)` accepts what the wasm-bindgen initialiser accepts (a URL,
// Request, Response, BufferSource or WebAssembly.Module, or a promise of one,
// or `{ module_or_path }`); `initSync(bytesOrModule)` is the synchronous
// variant.  See index.d.ts / index.d.mts for the API documentation.
//
// Recovery after an internal error (see "Failure containment" in
// index.d.ts): `await reset()` -- or `await init()` again, which does the
// same once the instance is poisoned -- imports a fresh copy of the
// wasm-bindgen glue module (`pkg-web/libavoid_rust.js?instance=<n>`) and
// instantiates it from the bytes or WebAssembly.Module given to init(), or
// by fetching the default .wasm URL again.

import wasmInit, * as wasm from "./pkg-web/libavoid_rust.js";
import createAvoidApi from "./lib/adapter.mjs";

let ready = false;
let reusableSource; // bytes or WebAssembly.Module given to init() / initSync()
let generation = 0;

function isPlainObject(value) {
  return value !== null && typeof value === "object" && Object.getPrototypeOf(value) === Object.prototype;
}

function remember(input) {
  const source = isPlainObject(input) ? input.module_or_path ?? input.module : input;
  if (
    (typeof WebAssembly !== "undefined" && source instanceof WebAssembly.Module) ||
    source instanceof ArrayBuffer ||
    ArrayBuffer.isView(source)
  ) {
    reusableSource = source;
  }
}

async function loadInstance(input) {
  generation += 1;
  const glue = await import(`./pkg-web/libavoid_rust.js?instance=${generation}`);
  const source = input ?? reusableSource;
  await glue.default(source === undefined || isPlainObject(source) ? source : { module_or_path: source });
  return glue;
}

const api = createAvoidApi(wasm, {
  assertReady() {
    if (!ready) {
      throw new Error("libavoid-rust: the WebAssembly module is not initialised; await init() first");
    }
  },
  loadInstance,
});

export default async function init(input) {
  if (!ready) {
    remember(input);
    await wasmInit(input === undefined || isPlainObject(input) ? input : { module_or_path: input });
    ready = true;
  } else if (api.internalError() !== null) {
    await reset(input);
  }
}

export function initSync(module) {
  if (!ready) {
    remember(module);
    wasm.initSync(isPlainObject(module) ? module : { module });
    ready = true;
  } else if (api.internalError() !== null) {
    throw new Error("libavoid-rust: the WebAssembly instance failed; use `await reset()` to replace it");
  }
}

export async function reset(input) {
  if (!ready) {
    await init(input);
    return;
  }
  remember(input);
  await api.reset(input);
}

export const {
  angle,
  anglePenalty,
  ATTACH_POS_BOTTOM,
  ATTACH_POS_CENTRE,
  ATTACH_POS_LEFT,
  ATTACH_POS_MAX_OFFSET,
  ATTACH_POS_MIN_OFFSET,
  ATTACH_POS_RIGHT,
  ATTACH_POS_TOP,
  AvoidInternalError,
  Box,
  Checkpoint,
  chooseSensibleParamValue,
  clusterCrossingPenalty,
  ClusterRef,
  colinear,
  ConnDirAll,
  ConnDirDown,
  ConnDirFlag,
  ConnDirLeft,
  ConnDirNone,
  ConnDirRight,
  ConnDirUp,
  CONNECTIONPIN_CENTRE,
  CONNECTIONPIN_UNSET,
  ConnEnd,
  ConnEndEmpty,
  ConnEndJunction,
  ConnEndPoint,
  ConnEndShapePin,
  ConnEndType,
  ConnRef,
  ConnType,
  ConnType_None,
  ConnType_Orthogonal,
  ConnType_PolyLine,
  cornerSide,
  CROSSING_NONE,
  CROSSING_SHARES_FIXED_SEGMENT,
  CROSSING_SHARES_PATH,
  CROSSING_SHARES_PATH_AT_END,
  CROSSING_TOUCHES,
  crossingPenalty,
  DO_INTERSECT,
  DONT_INTERSECT,
  Edge,
  euclideanDist,
  fixedSharedPathPenalty,
  HyperedgeNewAndDeletedObjectLists,
  HyperedgeRerouter,
  idealNudgingDistance,
  improveHyperedgeRoutesMovingAddingAndDeletingJunctions,
  improveHyperedgeRoutesMovingJunctions,
  inBetween,
  inPoly,
  inPolyGen,
  internalError,
  inValidRegion,
  JunctionRef,
  kShapeConnectionPin,
  kUnassignedVertexNumber,
  lastRoutingOptionMarker,
  lastRoutingParameterMarker,
  LineRep,
  manhattanDist,
  nudgeOrthogonalSegmentsConnectedToShapes,
  nudgeOrthogonalTouchingColinearSegments,
  nudgeSharedPathsWithCommonEndPoint,
  Obstacle,
  OrthogonalRouting,
  PARALLEL,
  penaliseOrthogonalSharedPathsAtConnEnds,
  PenaltyType,
  performUnifyingNudgingPreprocessingStep,
  Point,
  pointOnLine,
  Polygon,
  PolygonInterface,
  PolyLine,
  PolyLineRouting,
  portDirectionPenalty,
  projection,
  rayIntersectPoint,
  Rectangle,
  ReferencingPolygon,
  reverseDirectionPenalty,
  rotationalAngle,
  Router,
  RouterFlag,
  RoutingOption,
  RoutingParameter,
  runningFrom,
  runningTo,
  runningToAndFrom,
  segmentIntersect,
  segmentIntersectPoint,
  segmentPenalty,
  segmentShapeIntersect,
  shapeBufferDistance,
  ShapeConnectionPin,
  ShapeRef,
  ShapeTransformationType,
  totalLength,
  TransactionPhaseCompleted,
  TransactionPhaseCrossingDetection,
  TransactionPhaseOrthogonalNudgingX,
  TransactionPhaseOrthogonalNudgingY,
  TransactionPhaseOrthogonalVisibilityGraphScanX,
  TransactionPhaseOrthogonalVisibilityGraphScanY,
  TransactionPhaseRerouteSearch,
  TransactionPhaseRouteSearch,
  TransactionPhases,
  TransformationType_CW180,
  TransformationType_CW270,
  TransformationType_CW90,
  TransformationType_FlipX,
  TransformationType_FlipY,
  vecDir,
  Vector,
  XDIM,
  YDIM,
  zeroParamValue,
} = api;
