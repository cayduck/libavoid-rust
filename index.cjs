"use strict";

// Node.js entry of libavoid-rust (CommonJS, wasm-pack `nodejs` target in
// pkg/, which instantiates the wasm module synchronously on require).  The
// adapter itself lives in lib/adapter.cjs and is shared with the browser /
// ESM entry index.mjs; see index.d.ts for the API documentation.
//
// reset() loads a fresh copy of the wasm-bindgen glue module, which
// instantiates a new wasm instance (see "Failure containment" in
// index.d.ts).

function loadInstance() {
  const glue = require.resolve("./pkg");
  delete require.cache[glue];
  return require(glue);
}

module.exports = require("./lib/adapter.cjs")(require("./pkg"), { loadInstance });
