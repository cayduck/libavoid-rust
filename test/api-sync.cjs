"use strict";

// Fails when index.d.ts drifts from index.cjs or from the wasm exports:
// compares the method names declared for AvoidRouter / WasmAvoidRouter and
// the exported names with what the modules actually provide.

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const api = require("../index.cjs");
const wasm = require("../pkg");

const dts = fs.readFileSync(path.join(__dirname, "..", "index.d.ts"), "utf8");

function declaredMethods(className) {
  const start = dts.indexOf(`export class ${className} {`);
  assert.ok(start >= 0, `class ${className} not declared`);
  const body = dts.slice(start, dts.indexOf("\n}\n", start));
  const names = new Set();
  for (const m of body.matchAll(/^ {2}(?:static )?([A-Za-z]\w*)\(/gm)) {
    if (m[1] !== "constructor") names.add(m[1]);
  }
  return names;
}

function actualMethods(proto, statics) {
  const names = new Set(Object.getOwnPropertyNames(proto).filter((n) => n !== "constructor" && !n.startsWith("__") && typeof proto[n] === "function"));
  for (const s of statics) names.add(s);
  return names;
}

function sorted(set) {
  return [...set].sort();
}

const staticsOf = (cls) => Object.getOwnPropertyNames(cls).filter((n) => typeof cls[n] === "function");

assert.deepEqual(
  sorted(declaredMethods("AvoidRouter")),
  sorted(actualMethods(api.AvoidRouter.prototype, staticsOf(api.AvoidRouter))),
  "index.d.ts AvoidRouter vs index.cjs",
);
assert.deepEqual(
  sorted(declaredMethods("WasmAvoidRouter")),
  sorted(actualMethods(wasm.AvoidRouter.prototype, staticsOf(wasm.AvoidRouter).filter((n) => !n.startsWith("__")))),
  "index.d.ts WasmAvoidRouter vs wasm exports",
);

const declaredExports = new Set();
for (const m of dts.matchAll(/^export (?:const|class|function) (\w+)/gm)) declaredExports.add(m[1]);
assert.deepEqual(sorted(declaredExports), sorted(new Set(Object.keys(api))), "index.d.ts exports vs index.cjs");

// Every raw wasm method is used by the JS wrapper (no unreachable exports).
const adapterPath = path.join(__dirname, "..", "lib", "adapter.cjs");
const adapter = fs.readFileSync(adapterPath, "utf8");
for (const name of declaredMethods("WasmAvoidRouter")) {
  if (name === "withFlags") continue;
  assert.ok(adapter.includes(`#inner.${name}(`), `lib/adapter.cjs does not use wasm method ${name}`);
}

// The ES-module adapter used by index.mjs is the generated copy of the
// current lib/adapter.cjs (rerun `node scripts/prepare-pkg.cjs` otherwise).
const { adapterToEsm } = require("../scripts/prepare-pkg.cjs");
assert.equal(
  fs.readFileSync(path.join(__dirname, "..", "lib", "adapter.mjs"), "utf8"),
  adapterToEsm(adapter),
  "lib/adapter.mjs is stale: run `node scripts/prepare-pkg.cjs`",
);

console.log("ok - index.d.ts, index.cjs, lib/adapter.mjs and the wasm exports are in sync");
