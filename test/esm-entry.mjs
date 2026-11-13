// Tests of the browser / ES-module entry index.mjs (web target in pkg-web/)
// under Node: it must export exactly what index.cjs exports plus the
// initialisers, be declared consistently in index.d.mts, refuse to work
// before init(), route exactly like the Node entry, and contain internal
// failures like it (poisoning, recovery with init() / reset()).

import assert from "node:assert/strict";
import fs from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const require = createRequire(import.meta.url);
const cjs = require("../index.cjs");
const esm = await import("../index.mjs");

const sorted = (xs) => [...xs].sort();
const methods = (cls) =>
  sorted([
    ...Object.getOwnPropertyNames(cls.prototype).filter((n) => n !== "constructor"),
    ...Object.getOwnPropertyNames(cls).filter((n) => typeof cls[n] === "function"),
  ]);

// Same exports as the Node entry, plus `default` (init) and `initSync`.
assert.deepEqual(sorted(Object.keys(esm)), sorted([...Object.keys(cjs), "default", "initSync"]), "index.mjs exports vs index.cjs");
for (const [name, value] of Object.entries(cjs)) {
  if (typeof value === "function" && value.prototype && name !== "reset") {
    assert.deepEqual(methods(esm[name]), methods(value), `methods of ${name} in index.mjs vs index.cjs`);
  } else if (typeof value !== "function") {
    assert.deepEqual(esm[name], value, `constant ${name}`);
  }
}

// Node's ES-module view of the CommonJS entry: every export is a named
// import (index.cjs lists the names for cjs-module-lexer), plus `default`.
const cjsAsEsm = await import("../index.cjs");
assert.deepEqual(
  sorted(Object.keys(cjsAsEsm).filter((n) => n !== "module.exports")),
  sorted([...Object.keys(cjs), "default"]),
  "named imports of index.cjs",
);
assert.equal(cjsAsEsm.Router, cjs.Router);

// index.d.mts re-exports index.d.ts and declares the two initialisers.
const dmts = fs.readFileSync(path.join(root, "index.d.mts"), "utf8");
assert.match(dmts, /^export \* from "\.\/index\.js";$/m);
assert.match(dmts, /^export default function init\(/m);
assert.match(dmts, /^export function initSync\(/m);

// Before init(): a readable error instead of a wasm-bindgen TypeError.
// Plain value types work without the wasm module.
assert.throws(() => new esm.Router(esm.OrthogonalRouting), /await init\(\) first/);
assert.throws(() => esm.euclideanDist(new esm.Point(0, 0), new esm.Point(1, 1)), /await init\(\) first/);
const preInit = new esm.Rectangle(new esm.Point(0, 0), new esm.Point(2, 1));
assert.equal(preInit.size(), 4);
assert.throws(() => preInit.simplify(), /await init\(\) first/);

await esm.default(fs.readFileSync(path.join(root, "pkg-web", "libavoid_rust_bg.wasm")));
await esm.default(); // idempotent: does not fetch again

// Same scene through both entries gives identical results.
function scene(Avoid) {
  const router = new Avoid.Router(Avoid.OrthogonalRouting | Avoid.PolyLineRouting);
  router.setRoutingParameter(Avoid.shapeBufferDistance, 8);
  router.setRoutingParameter(Avoid.idealNudgingDistance, 6);
  const rect = (x1, y1, x2, y2) => new Avoid.Rectangle(new Avoid.Point(x1, y1), new Avoid.Point(x2, y2));
  const a = new Avoid.ShapeRef(router, rect(0, 0, 60, 40));
  const b = new Avoid.ShapeRef(router, rect(160, 80, 220, 140));
  new Avoid.ShapeRef(router, rect(90, 20, 120, 120));
  new Avoid.ShapeConnectionPin(a, 1, Avoid.ATTACH_POS_RIGHT, Avoid.ATTACH_POS_CENTRE, true, 0, Avoid.ConnDirNone);
  new Avoid.ShapeConnectionPin(b, 1, Avoid.ATTACH_POS_LEFT, Avoid.ATTACH_POS_CENTRE, true, 0, Avoid.ConnDirNone);
  const j = new Avoid.JunctionRef(router, new Avoid.Point(140, 160));
  const conns = [
    new Avoid.ConnRef(router, new Avoid.ConnEnd(a, 1), new Avoid.ConnEnd(b, 1)),
    new Avoid.ConnRef(router, new Avoid.ConnEnd(new Avoid.Point(-20, 60)), new Avoid.ConnEnd(new Avoid.Point(240, 20))),
    new Avoid.ConnRef(router, new Avoid.ConnEnd(new Avoid.Point(20, 200)), new Avoid.ConnEnd(j)),
    new Avoid.ConnRef(router, new Avoid.ConnEnd(j), new Avoid.ConnEnd(b, 1)),
  ];
  conns[1].setRoutingType(Avoid.ConnType_PolyLine);
  const fired = [];
  for (const c of conns) c.setCallback((ptr) => fired.push(ptr.id()), c);
  router.processTransaction();
  const out = {
    fired,
    routes: conns.map((c) => [c.id(), c.displayRoute().ps.map((p) => [p.x, p.y]), c.route().ps.map((p) => [p.x, p.y])]),
    diagram: router.outputDiagramText(),
    curved: rect(0, 0, 10, 10).curvedPolyline(0.5).ps.map((p) => [p.x, p.y]),
  };
  router.delete();
  return out;
}
const fromEsm = scene(esm);
assert.deepEqual(fromEsm, scene(cjs), "index.mjs and index.cjs route the same scene identically");
assert.equal(fromEsm.fired.length, 4);

// Failure containment (as test/failure-containment.cjs for the Node entry):
// a trap poisons the instance of this entry only; `await init()` again or
// `await reset()` replaces it.
assert.match(dmts, /^export function reset\(/m);
const unexpected = [];
process.on("uncaughtException", (error) => unexpected.push(error));
process.on("unhandledRejection", (error) => unexpected.push(error));
const { connectionPin01 } = require("./helpers.cjs");
const ARM = "libavoid-rust: I want to trap this WebAssembly instance";
const testPanic = (router, kind) => esm.Router[Symbol.for("libavoid-rust.testPanic")](router, kind, ARM);
for (const [kind, expected, recover] of [
  [1, "panic", () => esm.default()],
  [2, "panic", () => esm.reset()],
  [3, "stack-overflow", () => esm.default()],
  [4, "out-of-memory", () => esm.reset()],
]) {
  const bystander = new esm.Router(esm.OrthogonalRouting);
  const victim = new esm.Router(esm.OrthogonalRouting);
  let first;
  assert.throws(
    () => testPanic(victim, kind),
    (error) => {
      first = error;
      return error instanceof esm.AvoidInternalError && error.kind === expected && error.method === "Router.__testPanic";
    },
  );
  if (expected === "panic") assert.match(first.location, /^src\/wasm_api\.rs:\d+:\d+$/);
  assert.equal(esm.internalError(), first);
  assert.throws(() => bystander.m_obstacles, /object is unusable after an internal error: libavoid-rust internal error/);
  assert.throws(() => new esm.Router(esm.OrthogonalRouting), esm.AvoidInternalError);
  assert.throws(() => esm.initSync(), /use `await reset\(\)`/);
  // The Node entry has its own instance and is unaffected.
  assert.equal(cjs.internalError(), null);
  assert.ok(connectionPin01(cjs) > 0);

  await recover();
  assert.equal(esm.internalError(), null);
  assert.ok(connectionPin01(esm) > 0);
  assert.deepEqual(scene(esm), fromEsm, "a fresh instance routes the scene as before");
  assert.throws(() => victim.m_obstacles, esm.AvoidInternalError);
}
await esm.default(); // healthy again: a no-op
if (typeof global.gc === "function") {
  global.gc();
  await new Promise((resolve) => setTimeout(resolve, 20));
}
assert.deepEqual(unexpected, [], "no uncaught exceptions or unhandled rejections");

console.log("ok - index.mjs (web target) matches index.cjs and index.d.mts");
