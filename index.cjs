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

// Never executed: the names Node's ES-module loader (cjs-module-lexer) reads
// to offer this CommonJS module's exports as named imports, so that
// `import { AvoidRouter } from "libavoid-rust"` works in Node.  (Such an
// import is a snapshot: after reset(), read WasmAvoidRouter from the default
// import.)  test/esm-entry.mjs checks the list against the real exports.
0 &&
  (module.exports = {
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
    reset,
    WasmAvoidRouter,
  });
