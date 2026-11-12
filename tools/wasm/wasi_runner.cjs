#!/usr/bin/env node
// Cargo test runner for wasm32-wasip1 under Node's WASI (32-bit `usize`):
//
//   CARGO_TARGET_WASM32_WASIP1_RUNNER="node tools/wasm/wasi_runner.cjs" \
//     cargo test --release --target wasm32-wasip1 --test fixtures_ops
//
// (tools/wasm/run_wasm_tests.sh does exactly that for the core's
// differential tests.)  The repository root is preopened at its absolute
// path, so the CARGO_MANIFEST_DIR paths compiled into the tests resolve.
"use strict";
const fs = require("fs");
const path = require("path");
const { WASI } = require("wasi");

const [wasmPath, ...args] = process.argv.slice(2);
const root = path.resolve(__dirname, "..", "..");
const wasi = new WASI({
  version: "preview1",
  args: [path.basename(wasmPath), ...args],
  env: process.env,
  preopens: { [root]: root, "/tmp": "/tmp" },
  returnOnExit: true,
});
const wasmModule = new WebAssembly.Module(fs.readFileSync(wasmPath));
const instance = new WebAssembly.Instance(wasmModule, wasi.getImportObject());
const code = wasi.start(instance);
process.exit(code);
