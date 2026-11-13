"use strict";

// Fails when index.d.ts drifts from index.cjs or the adapter from the wasm
// exports: compares the declared exports, classes, methods and properties
// with what the modules actually provide, checks that every raw wasm
// function is used by lib/adapter.cjs, and that lib/adapter.mjs is the
// generated copy of lib/adapter.cjs.

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const api = require("../index.cjs");
const wasm = require("../pkg");

const root = path.join(__dirname, "..");
const dts = fs.readFileSync(path.join(root, "index.d.ts"), "utf8");
const sorted = (xs) => [...new Set(xs)].sort();

// --- exports ---
const declaredValues = [];
for (const m of dts.matchAll(/^export (?:const|(?:abstract )?class|function) (\w+)/gm)) declaredValues.push(m[1]);
assert.deepEqual(sorted(declaredValues), sorted(Object.keys(api)), "index.d.ts value exports vs index.cjs");

// --- classes: methods and properties ---
function declaredMembers(className) {
  const start = dts.search(new RegExp(`^export (?:abstract )?class ${className}\\b[^{]*\\{`, "m"));
  assert.ok(start >= 0, `class ${className} not declared`);
  const body = dts.slice(start, dts.indexOf("\n}\n", start));
  const methods = new Set();
  const props = new Set();
  for (const m of body.matchAll(/^ {2}(?:abstract |protected )?(?:readonly )?([A-Za-z_]\w*)(<[^>]*>)?(\(|\??:)/gm)) {
    if (m[1] === "constructor") continue;
    (m[3] === "(" ? methods : props).add(m[1]);
  }
  return { methods: sorted(methods), props: sorted(props) };
}

function actualMembers(cls) {
  const methods = [];
  const props = [];
  for (const name of Object.getOwnPropertyNames(cls.prototype)) {
    if (name === "constructor") continue;
    const d = Object.getOwnPropertyDescriptor(cls.prototype, name);
    if (typeof d.value === "function") methods.push(name);
    else if (d.get) props.push(name);
  }
  // Data members set by the constructor (value types).
  let instance = null;
  try {
    if (cls === api.Rectangle) instance = new cls(new api.Point(0, 0), new api.Point(1, 1));
    else if (![api.Router, api.ShapeRef, api.JunctionRef, api.ConnRef, api.ShapeConnectionPin, api.ClusterRef, api.Obstacle, api.PolygonInterface].includes(cls)) instance = new cls();
  } catch {
    instance = null;
  }
  const parent = Object.getPrototypeOf(cls);
  const inherited = parent && parent !== Function.prototype ? actualMembers(parent).props : [];
  if (instance) for (const name of Object.keys(instance)) if (!inherited.includes(name)) props.push(name);
  return { methods: sorted(methods), props: sorted(props) };
}

let classes = 0;
for (const [name, value] of Object.entries(api)) {
  if (typeof value !== "function" || !/^export (?:abstract )?class/m.test(dts.match(new RegExp(`^export (?:abstract )?class ${name}\\b.*$`, "m"))?.[0] ?? "")) continue;
  if (name === "AvoidInternalError") continue;
  classes++;
  const declared = declaredMembers(name);
  const actual = actualMembers(value);
  assert.deepEqual(declared.methods, actual.methods, `index.d.ts methods of ${name} vs index.cjs`);
  assert.deepEqual(declared.props, actual.props, `index.d.ts properties of ${name} vs index.cjs`);
}
assert.ok(classes >= 18, `checked ${classes} classes`);

// --- every raw wasm function is used by the adapter ---
const adapterPath = path.join(root, "lib", "adapter.cjs");
const adapter = fs.readFileSync(adapterPath, "utf8");
const rawMethods = Object.getOwnPropertyNames(wasm.RawRouter.prototype).filter(
  (n) => n !== "constructor" && n !== "free" && !n.startsWith("__") && typeof wasm.RawRouter.prototype[n] === "function",
);
for (const name of rawMethods) {
  assert.ok(adapter.includes(`raw.${name}(`), `lib/adapter.cjs does not use RawRouter.${name}`);
}
const rawFunctions = Object.keys(wasm).filter((n) => typeof wasm[n] === "function" && /^[a-z]/.test(n) && !["initSync", "takeInternalErrorMessage"].includes(n));
for (const name of rawFunctions) {
  assert.ok(adapter.includes(`w.${name}(`), `lib/adapter.cjs does not use wasm function ${name}`);
}

// The ES-module adapter used by index.mjs is the generated copy of the
// current lib/adapter.cjs (rerun `node scripts/prepare-pkg.cjs` otherwise).
const { adapterToEsm } = require("../scripts/prepare-pkg.cjs");
assert.equal(
  fs.readFileSync(path.join(root, "lib", "adapter.mjs"), "utf8"),
  adapterToEsm(adapter),
  "lib/adapter.mjs is stale: run `node scripts/prepare-pkg.cjs`",
);

console.log(`ok - index.d.ts (${classes} classes, ${declaredValues.length} exports), index.cjs, lib/adapter.mjs and the wasm exports are in sync`);
