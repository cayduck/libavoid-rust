// Type declarations for the browser / ES-module entry (index.mjs).  The API
// is the one declared in index.d.ts; this entry adds the asynchronous wasm
// initialisation that must complete before the first `new AvoidRouter()`.

export * from "./index.js";

/** What the wasm-bindgen initialiser accepts as the wasm source. */
export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

/**
 * Loads and instantiates the wasm module (by default
 * `pkg-web/libavoid_rust_bg.wasm`, resolved relative to the module URL).
 * Resolves once the API is usable; later calls are no-ops, except after an
 * internal error poisoned the instance: then they act as `reset(input)`.
 */
export default function init(input?: InitInput | Promise<InitInput> | { module_or_path: InitInput | Promise<InitInput> }): Promise<void>;

/**
 * ES-module form of `reset()`: imports a fresh copy of the wasm-bindgen
 * glue and instantiates it from `input`, else from the bytes or
 * WebAssembly.Module given to `init()`, else by fetching the default .wasm
 * again.  `init()` does the same when the current instance is poisoned.
 */
export function reset(input?: InitInput | Promise<InitInput> | { module_or_path: InitInput | Promise<InitInput> }): Promise<void>;

/** Synchronous variant of `init()` from bytes or a compiled module (throws once the instance is poisoned; use `reset()`). */
export function initSync(module: BufferSource | WebAssembly.Module | { module: BufferSource | WebAssembly.Module }): void;
