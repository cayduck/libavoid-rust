"use strict";

// Post-build step of `npm run build:node` / `npm run build:web`:
//  * removes the `.gitignore` wasm-pack writes into pkg/ and pkg-web/ (it
//    would make npm leave the generated package out of the tarball);
//  * generates lib/adapter.mjs, the ES-module copy of lib/adapter.cjs used
//    by index.mjs, so that both entry points share one adapter source;
//  * once both packages are built, keeps a single copy of the WebAssembly:
//    the `nodejs` and `web` targets emit the same .wasm, so pkg/'s copy is
//    removed and pkg/'s glue reads pkg-web/'s (which the browser glue and
//    bundlers find next to pkg-web/libavoid_rust.js).

const fs = require("node:fs");
const path = require("node:path");

const root = path.join(__dirname, "..");

for (const dir of ["pkg", "pkg-web"]) {
  const generatedIgnore = path.join(root, dir, ".gitignore");
  if (fs.existsSync(generatedIgnore)) {
    fs.unlinkSync(generatedIgnore);
  }
}

/** lib/adapter.cjs source -> ES-module source (exported for test/api-sync.cjs). */
function adapterToEsm(source) {
  const cjsExport = "\nmodule.exports = createAvoidApi;\n";
  if (source.split(cjsExport).length !== 2 || !source.endsWith(cjsExport)) {
    throw new Error("lib/adapter.cjs must end with exactly one `module.exports = createAvoidApi;`");
  }
  if (/\brequire\(/.test(source)) {
    throw new Error("lib/adapter.cjs must not require() anything (it is also loaded as an ES module)");
  }
  return (
    "// GENERATED from lib/adapter.cjs by scripts/prepare-pkg.cjs; do not edit.\n" +
    source.slice(0, -cjsExport.length).replace(/^"use strict";\n/, "") +
    "\nexport default createAvoidApi;\n"
  );
}

function writeEsmAdapter() {
  const source = fs.readFileSync(path.join(root, "lib", "adapter.cjs"), "utf8");
  fs.writeFileSync(path.join(root, "lib", "adapter.mjs"), adapterToEsm(source));
}

const WASM = "libavoid_rust_bg.wasm";
const NODE_WASM_LINE = "const wasmPath = `${__dirname}/libavoid_rust_bg.wasm`;";
const SHARED_WASM_LINE = "const wasmPath = require('path').join(__dirname, '..', 'pkg-web', 'libavoid_rust_bg.wasm');";

/** Points pkg/'s glue at pkg-web/'s .wasm and removes pkg/'s copy, when the
 *  two copies are identical (both built from the same source). */
function shareWasm() {
  const nodeWasm = path.join(root, "pkg", WASM);
  const webWasm = path.join(root, "pkg-web", WASM);
  if (!fs.existsSync(nodeWasm) || !fs.existsSync(webWasm)) {
    return;
  }
  if (!fs.readFileSync(nodeWasm).equals(fs.readFileSync(webWasm))) {
    // One package was rebuilt alone: keep both until the other catches up.
    return;
  }
  const gluePath = path.join(root, "pkg", "libavoid_rust.js");
  const glue = fs.readFileSync(gluePath, "utf8");
  if (glue.split(NODE_WASM_LINE).length !== 2) {
    throw new Error(`pkg/libavoid_rust.js: expected exactly one \`${NODE_WASM_LINE}\` (wasm-bindgen output changed?)`);
  }
  fs.writeFileSync(gluePath, glue.replace(NODE_WASM_LINE, SHARED_WASM_LINE));
  fs.unlinkSync(nodeWasm);
}

if (require.main === module) {
  writeEsmAdapter();
  shareWasm();
}

module.exports = { adapterToEsm };
