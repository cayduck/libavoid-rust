"use strict";

// Node.js entry of libavoid-rust (CommonJS, wasm-pack `nodejs` target in
// pkg/, which instantiates the wasm module synchronously on require).  The
// adapter itself lives in lib/adapter.cjs and is shared with the browser /
// ESM entry index.mjs; see index.d.ts for the API documentation.

module.exports = require("./lib/adapter.cjs")(require("./pkg"));
