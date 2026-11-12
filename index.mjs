// Browser / ES-module entry of libavoid-rust (wasm-pack `web` target in
// pkg-web/).  The API is the one of index.cjs (same adapter, lib/adapter.cjs,
// compiled to lib/adapter.mjs by scripts/prepare-pkg.cjs); the only
// difference is that the wasm module is loaded asynchronously, so
// `await init()` must complete before the first `new AvoidRouter()`:
//
//   import init, { AvoidRouter, RouterFlag } from "libavoid-rust";
//   await init();                     // fetches pkg-web/libavoid_rust_bg.wasm
//   const router = new AvoidRouter(RouterFlag.OrthogonalRouting);
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
  WasmAvoidRouter = api.WasmAvoidRouter;
}

export const {
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
  internalError,
} = api;

/** The raw wasm-bindgen class of the current instance (updated by reset()). */
export let WasmAvoidRouter = api.WasmAvoidRouter;
