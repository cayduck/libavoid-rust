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
assert.deepEqual(methods(esm.AvoidRouter), methods(cjs.AvoidRouter), "AvoidRouter methods of index.mjs vs index.cjs");
assert.deepEqual(methods(esm.WasmAvoidRouter), methods(cjs.WasmAvoidRouter), "raw wasm class of pkg-web vs pkg");
for (const [name, value] of Object.entries(cjs)) {
  if (typeof value !== "function") assert.deepEqual(esm[name], value, `constant ${name}`);
}

// Node's ES-module view of the CommonJS entry: every export is a named
// import (index.cjs lists the names for cjs-module-lexer), plus `default`.
const cjsAsEsm = await import("../index.cjs");
assert.deepEqual(
  sorted(Object.keys(cjsAsEsm).filter((n) => n !== "module.exports")),
  sorted([...Object.keys(cjs), "default"]),
  "named imports of index.cjs",
);
assert.equal(cjsAsEsm.AvoidRouter, cjs.AvoidRouter);

// index.d.mts re-exports index.d.ts and declares the two initialisers.
const dmts = fs.readFileSync(path.join(root, "index.d.mts"), "utf8");
assert.match(dmts, /^export \* from "\.\/index\.js";$/m);
assert.match(dmts, /^export default function init\(/m);
assert.match(dmts, /^export function initSync\(/m);

// Before init(): a readable error instead of a wasm-bindgen TypeError.
assert.throws(() => new esm.AvoidRouter(), /await init\(\) first/);
assert.throws(() => esm.curvedPolyline([[0, 0], [1, 1]], 0.5), /await init\(\) first/);

await esm.default(fs.readFileSync(path.join(root, "pkg-web", "libavoid_rust_bg.wasm")));
await esm.default(); // idempotent: does not fetch again

// Same scene through both entries gives identical results.
function scene(api) {
  const router = new api.AvoidRouter(api.RouterFlag.OrthogonalRouting | api.RouterFlag.PolyLineRouting);
  router.setRoutingParameter(api.RoutingParameter.shapeBufferDistance, 8);
  router.setRoutingParameter(api.RoutingParameter.idealNudgingDistance, 6);
  router.addShape("a", 0, 0, 60, 40);
  router.addShape("b", 160, 80, 220, 140);
  router.addShape("mid", 90, 20, 120, 120);
  for (const [shape, x] of [["a", api.AttachPos.Right], ["b", api.AttachPos.Left]]) {
    router.addShapeConnectionPinWithOptions(shape, "p", { classId: 1, xOffset: x, yOffset: api.AttachPos.Centre, proportional: true });
  }
  router.addConnectorBetweenPins("c1", "a", "p", "b", "p");
  router.addConnectorWithConnEnds("c2", [-20, 60], [240, 20]);
  router.addJunction("j", 140, 160);
  router.addConnectorToJunction("c3", 20, 200, "j");
  router.addConnectorWithConnEnds("c4", { junctionId: "j" }, { shapeId: "b", pinId: "p" });
  router.setConnectorRoutingType("c2", api.ConnType.PolyLine);
  router.processTransaction();
  const out = {
    updated: sorted(router.lastTransactionUpdatedConnectorIds()),
    routes: router.connectorIds().map((id) => [id, router.displayConnectorRoute(id), router.routeConnector(id)]),
    diagram: router.outputDiagramText(true),
    curved: api.curvedPolyline([[0, 0], [10, 0], [10, 10]], 0.5),
  };
  router.free();
  return out;
}
const fromEsm = scene(esm);
assert.deepEqual(fromEsm, scene(cjs), "index.mjs and index.cjs route the same scene identically");
assert.equal(fromEsm.updated.length, 4);

// Failure containment (as test/failure-containment.cjs for the Node entry):
// a trap poisons the instance of this entry only; `await init()` again or
// `await reset()` replaces it.
assert.match(dmts, /^export function reset\(/m);
const unexpected = [];
process.on("uncaughtException", (error) => unexpected.push(error));
process.on("unhandledRejection", (error) => unexpected.push(error));
const { connectionPin01 } = require("./helpers.cjs");
const ARM = "libavoid-rust: I want to trap this WebAssembly instance";
const wasmClassBefore = esm.WasmAvoidRouter;
for (const [kind, expected, recover] of [
  [1, "panic", () => esm.default()],
  [2, "panic", () => esm.reset()],
  [3, "stack-overflow", () => esm.default()],
  [4, "out-of-memory", () => esm.reset()],
]) {
  const bystander = new esm.AvoidRouter();
  const victim = new esm.AvoidRouter();
  let first;
  assert.throws(
    () => victim.__testPanic(kind, ARM),
    (error) => {
      first = error;
      return error instanceof esm.AvoidInternalError && error.kind === expected && error.method === "AvoidRouter.__testPanic";
    },
  );
  if (expected === "panic") assert.match(first.location, /^src\/wasm_api\.rs:\d+:\d+$/);
  assert.equal(esm.internalError(), first);
  assert.equal(bystander.isPoisoned(), true);
  assert.throws(() => bystander.shapeIds(), /router is unusable after an internal error: libavoid-rust internal error/);
  assert.throws(() => new esm.AvoidRouter(), esm.AvoidInternalError);
  assert.throws(() => esm.initSync(), /use `await reset\(\)`/);
  // The Node entry has its own instance and is unaffected.
  assert.equal(cjs.internalError(), null);
  assert.ok(connectionPin01(cjs) > 0);

  await recover();
  assert.equal(esm.internalError(), null);
  assert.notEqual(esm.WasmAvoidRouter, wasmClassBefore, "WasmAvoidRouter is the class of the new instance");
  assert.ok(connectionPin01(esm) > 0);
  assert.deepEqual(scene(esm), fromEsm, "a fresh instance routes the scene as before");
  assert.equal(victim.isPoisoned(), true);
}
await esm.default(); // healthy again: a no-op
if (typeof global.gc === "function") {
  global.gc();
  await new Promise((resolve) => setTimeout(resolve, 20));
}
assert.deepEqual(unexpected, [], "no uncaught exceptions or unhandled rejections");

console.log("ok - index.mjs (web target) matches index.cjs and index.d.mts");
