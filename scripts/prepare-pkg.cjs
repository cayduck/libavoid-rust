"use strict";

// Post-build step of `npm run build:node` / `npm run build:web`:
//  * removes the `.gitignore` wasm-pack writes into pkg/ and pkg-web/ (it
//    would make npm leave the generated package out of the tarball);
//  * generates lib/adapter.mjs, the ES-module copy of lib/adapter.cjs used
//    by index.mjs, so that both entry points share one adapter source.

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

if (require.main === module) {
  writeEsmAdapter();
}

module.exports = { adapterToEsm };
