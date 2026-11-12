// Type declarations for the browser / ES-module entry (index.mjs).  The API
// is the one declared in index.d.ts; this entry adds the asynchronous wasm
// initialisation that must complete before the first `new AvoidRouter()`.

export * from "./index.js";

/** What the wasm-bindgen initialiser accepts as the wasm source. */
export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

/**
 * Loads and instantiates the wasm module (by default
 * `pkg-web/libavoid_rust_bg.wasm`, resolved relative to the module URL).
 * Resolves once the API is usable; later calls are no-ops.
 */
export default function init(input?: InitInput | Promise<InitInput> | { module_or_path: InitInput | Promise<InitInput> }): Promise<void>;

/** Synchronous variant of `init()` from bytes or a compiled module. */
export function initSync(module: BufferSource | WebAssembly.Module | { module: BufferSource | WebAssembly.Module }): void;
