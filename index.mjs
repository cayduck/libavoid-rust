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

import wasmInit, * as wasm from "./pkg-web/libavoid_rust.js";
import createAvoidApi from "./lib/adapter.mjs";

let ready = false;

export default async function init(input) {
  if (!ready) {
    await wasmInit(input === undefined || isPlainObject(input) ? input : { module_or_path: input });
    ready = true;
  }
}

export function initSync(module) {
  if (!ready) {
    wasm.initSync(isPlainObject(module) ? module : { module });
    ready = true;
  }
}

function isPlainObject(value) {
  return value !== null && typeof value === "object" && Object.getPrototypeOf(value) === Object.prototype;
}

const api = createAvoidApi(wasm, {
  assertReady() {
    if (!ready) {
      throw new Error("libavoid-rust: the WebAssembly module is not initialised; await init() first");
    }
  },
});

export const {
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
  WasmAvoidRouter,
} = api;
