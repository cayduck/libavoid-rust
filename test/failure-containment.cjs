"use strict";

// Failure containment of the Node entry (index.cjs), on the shipped wasm:
//  1. the built .wasm / glue embed no absolute path of the build machine;
//  2. every kind of internal failure (explicit panic, index out of bounds in
//     a router method, stack overflow, failed allocation), triggered with
//     the armed test-only `__testPanic`, throws AvoidInternalError with the
//     expected fields, poisons every router of the instance, and reset()
//     gives a fresh instance that routes an upstream fixture exactly;
//  3. transaction progress callback / time limit: upstream cancellation
//     semantics, the router stays usable and the next processTransaction()
//     finishes the work.
// Run with --expose-gc so that garbage collection of detached routers (whose
// finalizers must not call into a dead instance) happens during the test.

const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const api = require("../index.cjs");
const { connectionPin01, root } = require("./helpers.cjs");

const { AvoidInternalError, AvoidRouter, RouterFlag, RoutingParameter, TransactionPhase } = api;
const ARM = "libavoid-rust: I want to trap this WebAssembly instance";

const unexpected = [];
process.on("uncaughtException", (error) => unexpected.push(error));
process.on("unhandledRejection", (error) => unexpected.push(error));

async function collectGarbage() {
  if (typeof global.gc === "function") {
    global.gc();
    await new Promise((resolve) => setTimeout(resolve, 20));
    global.gc();
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
}

const tests = [];
function test(name, fn) {
  tests.push([name, fn]);
}

// --- 1. no machine paths in the built package --------------------------------

test("built files embed no home directory, user name or checkout path", () => {
  const forbidden = [os.homedir(), root];
  const user = os.userInfo().username;
  if (user.length >= 4) forbidden.push(user);
  for (const dir of ["pkg", "pkg-web"]) {
    for (const file of fs.readdirSync(path.join(root, dir))) {
      const bytes = fs.readFileSync(path.join(root, dir, file));
      for (const needle of forbidden) {
        assert.equal(bytes.indexOf(Buffer.from(needle)), -1, `${dir}/${file} contains a machine-specific path or name`);
      }
    }
  }
  // Panic locations are crate-relative.
  const wasm = fs.readFileSync(path.join(root, "pkg", "libavoid_rust_bg.wasm"));
  assert.notEqual(wasm.indexOf(Buffer.from("src/wasm_api.rs")), -1);
});

// --- 2. traps -----------------------------------------------------------------

const KINDS = [
  { kind: 1, expect: "panic", panic: /^libavoid-rust test panic \(kind 1\)$/ },
  { kind: 2, expect: "panic", panic: /^index out of bounds: the len is \d+ but the index is \d+$/ },
  { kind: 3, expect: "stack-overflow", message: /stack overflow/ },
  { kind: 4, expect: "out-of-memory", message: /out of memory: memory allocation of \d+ bytes failed/ },
];

function sceneRouter() {
  const r = new AvoidRouter(RouterFlag.OrthogonalRouting);
  r.addShape("a", 0, 0, 60, 40);
  r.addShape("b", 160, 80, 220, 140);
  r.addConnector("c", 30, 20, 190, 110);
  r.processTransaction();
  return r;
}

function assertPoisoned(fn, first, method) {
  assert.throws(fn, (error) => {
    assert.ok(error instanceof AvoidInternalError, `${method}: ${error}`);
    assert.equal(error.kind, "poisoned");
    assert.equal(error.method, method);
    assert.equal(error.cause, first);
    assert.equal(error.message, `${method}: router is unusable after an internal error: ${first.message}`);
    return true;
  });
}

for (const { kind, expect, panic, message } of KINDS) {
  test(`trap kind ${kind} (${expect}) poisons the instance; reset() recovers`, async () => {
    api.reset(); // start from a fresh instance
    assert.equal(api.internalError(), null);
    const oldWasmClass = api.WasmAvoidRouter;
    const bystander = sceneRouter();
    const victim = sceneRouter();
    const idle = new AvoidRouter();

    // Unarmed: inert.
    assert.equal(victim.__testPanic(kind, "please"), false);
    assert.equal(victim.isPoisoned(), false);

    let first;
    assert.throws(
      () => victim.__testPanic(kind, ARM),
      (error) => {
        first = error;
        assert.ok(error instanceof AvoidInternalError);
        assert.ok(error instanceof Error);
        assert.equal(error.name, "AvoidInternalError");
        assert.equal(error.kind, expect);
        assert.equal(error.method, "AvoidRouter.__testPanic");
        assert.ok(error.cause instanceof WebAssembly.RuntimeError || error.cause instanceof RangeError, String(error.cause));
        assert.ok(error.message.startsWith("libavoid-rust internal error in AvoidRouter.__testPanic: "), error.message);
        if (panic) {
          assert.match(error.panicMessage, panic);
          assert.match(error.location, /^src\/wasm_api\.rs:\d+:\d+$/);
          assert.ok(error.message.includes(`Rust panic at ${error.location}: ${error.panicMessage}`));
        } else {
          assert.equal(error.panicMessage, null);
          assert.equal(error.location, null);
          assert.match(error.message, message);
        }
        return true;
      },
    );

    // The whole instance is poisoned: the failed router, routers that were
    // never involved, new routers and curvedPolyline.
    assert.equal(api.internalError(), first);
    for (const r of [victim, bystander, idle]) assert.equal(r.isPoisoned(), true);
    assertPoisoned(() => victim.connectorIds(), first, "AvoidRouter.connectorIds");
    assertPoisoned(() => bystander.displayConnectorRoute("c"), first, "AvoidRouter.displayConnectorRoute");
    assertPoisoned(() => bystander.processTransaction(), first, "AvoidRouter.processTransaction");
    assertPoisoned(() => idle.addShape("x", 0, 0, 1, 1), first, "AvoidRouter.addShape");
    // The poison check comes before argument validation.
    assertPoisoned(() => idle.addShape(42), first, "AvoidRouter.addShape");
    assertPoisoned(() => new AvoidRouter(), first, "new AvoidRouter");
    assertPoisoned(() => api.curvedPolyline([[0, 0], [1, 1]], 0.5), first, "curvedPolyline");
    // free() is still allowed (a no-op).
    bystander.free();

    // Recovery: a fresh instance routes an upstream fixture exactly.
    api.reset();
    assert.equal(api.internalError(), null);
    assert.notEqual(api.WasmAvoidRouter, oldWasmClass);
    assert.ok(connectionPin01(api) > 0);
    // Routers of the old instance stay poisoned with the original error.
    assertPoisoned(() => victim.connectorIds(), first, "AvoidRouter.connectorIds");
    assert.equal(victim.isPoisoned(), true);

    // Garbage collection of the old instance's routers must not call into it.
    await collectGarbage();
  });
}

test("reset() of a healthy instance invalidates its routers", () => {
  const before = sceneRouter();
  api.reset();
  assert.equal(before.isPoisoned(), true);
  assert.throws(
    () => before.connectorIds(),
    (error) =>
      error instanceof AvoidInternalError &&
      error.kind === "poisoned" &&
      error.cause.kind === "reset" &&
      /router is unusable: its WebAssembly instance was replaced by reset\(\)/.test(error.message),
  );
  assert.equal(api.internalError(), null);
  assert.ok(connectionPin01(api) > 0);
});

test("ordinary errors do not poison", () => {
  const r = sceneRouter();
  assert.throws(() => r.addShape(1, 0, 0, 1, 1), TypeError);
  assert.throws(() => new AvoidRouter(7), /flags/);
  assert.equal(r.addShape("a", 0, 0, 1, 1), false);
  assert.equal(r.isPoisoned(), false);
  assert.equal(api.internalError(), null);
  r.free();
});

// --- 3. transaction progress ------------------------------------------------------

/** Eight pairwise crossing connectors and a crossing penalty, so that improveCrossings() runs. */
function crossingScene() {
  const r = new AvoidRouter(RouterFlag.OrthogonalRouting);
  r.setRoutingParameter(RoutingParameter.crossingPenalty, 200);
  r.setRoutingParameter(RoutingParameter.shapeBufferDistance, 4);
  r.addShape("m", 40, 40, 60, 60);
  for (let i = 0; i < 4; i++) {
    r.addConnector(`h${i}`, 0, 20 + 20 * i, 100, 80 - 20 * i);
    r.addConnector(`v${i}`, 20 + 20 * i, 0, 80 - 20 * i, 100);
  }
  return r;
}

const routes = (r) => Object.fromEntries(r.connectorIds().map((id) => [id, r.displayConnectorRoute(id)]));

test("progress callback sees upstream's phases", () => {
  const r = crossingScene();
  const phases = new Set();
  let lastElapsed = -1;
  r.setTransactionProgressCallback((elapsed, phase, total, proportion) => {
    assert.equal(total, TransactionPhase.Completed);
    assert.ok(proportion >= 0 && proportion <= 1);
    assert.ok(elapsed >= lastElapsed);
    lastElapsed = elapsed;
    phases.add(phase);
    return true;
  });
  r.processTransaction();
  assert.deepEqual([...phases].sort(), [1, 2, 3, 4, 5, 6, 7, 8]);
  assert.equal(r.lastTransactionCancelled(), false);
  r.free();
});

test("cancelling: only crossing improvement stops; the next processTransaction() finishes the work", () => {
  const reference = crossingScene();
  reference.processTransaction();
  const expected = routes(reference);
  reference.free();

  for (const [stopAt, skipsWork] of [
    [TransactionPhase.RouteSearch, true], // takes effect at the crossing check
    [TransactionPhase.CrossingDetection, true],
    [TransactionPhase.RerouteSearch, true],
    [TransactionPhase.OrthogonalNudgingX, false], // nudging does not act on it
  ]) {
    const r = crossingScene();
    let asked = 0;
    r.setTransactionProgressCallback((elapsed, phase) => {
      if (phase === stopAt) asked++;
      return phase !== stopAt;
    });
    assert.equal(r.processTransaction(), true);
    assert.ok(asked > 0);
    assert.equal(r.lastTransactionCancelled(), true, `phase ${stopAt}`);
    const cancelled = routes(r);
    assert.equal(JSON.stringify(cancelled) !== JSON.stringify(expected), skipsWork, `phase ${stopAt}`);
    if (stopAt === TransactionPhase.RerouteSearch) {
      // Upstream frees the routes of a crossing group before rerouting it:
      // connectors not yet rerouted when it stops are left without a route.
      assert.ok(Object.values(cancelled).some((route) => route.length === 0));
    }

    r.setTransactionProgressCallback(null);
    assert.equal(r.processTransaction(), true, "reruns although nothing new is queued");
    assert.equal(r.lastTransactionCancelled(), false);
    assert.deepEqual(routes(r), expected, `phase ${stopAt}: finished`);
    assert.equal(r.processTransaction(), false, "nothing left to do");
    r.free();
  }
});

test("transaction time limit", () => {
  const r = crossingScene();
  r.setTransactionTimeLimit(0);
  r.processTransaction();
  assert.equal(r.lastTransactionCancelled(), true);
  r.setTransactionTimeLimit(60_000);
  r.processTransaction();
  assert.equal(r.lastTransactionCancelled(), false);
  // The limit survives clear() (it is a router setting like the others).
  r.clear();
  r.setTransactionUse(false);
  r.addConnector("c", 0, 0, 50, 50);
  assert.equal(r.transactionTimeLimit(), 60_000);
  r.setTransactionTimeLimit(null);
  r.free();
});

test("a throwing progress callback cancels and its error is rethrown; the router stays usable", () => {
  const r = crossingScene();
  const boom = new Error("boom");
  r.setTransactionProgressCallback((elapsed, phase) => {
    if (phase === TransactionPhase.CrossingDetection) throw boom;
    return true;
  });
  assert.throws(() => r.processTransaction(), (error) => error === boom);
  assert.equal(r.isPoisoned(), false);
  assert.equal(r.lastTransactionCancelled(), true);
  r.setTransactionProgressCallback(null);
  assert.equal(r.processTransaction(), true);
  assert.equal(r.lastTransactionCancelled(), false);
  r.free();
});

test("a progress callback cannot call into its own router", () => {
  const r = crossingScene();
  let inner;
  r.setTransactionProgressCallback(() => {
    try {
      r.connectorIds();
    } catch (error) {
      inner = error;
      throw error;
    }
    return true;
  });
  assert.throws(() => r.processTransaction(), /cannot be used from inside its own transaction progress callback/);
  assert.ok(inner instanceof Error && !(inner instanceof AvoidInternalError));
  assert.equal(r.isPoisoned(), false);
  r.setTransactionProgressCallback(null);
  assert.equal(r.processTransaction(), true);
  r.free();
});

test("a trap inside a progress callback poisons the instance; the outer call reports it", () => {
  api.reset();
  const outer = crossingScene();
  const other = new AvoidRouter();
  outer.setTransactionProgressCallback(() => {
    other.__testPanic(1, ARM);
    return true;
  });
  assert.throws(
    () => outer.processTransaction(),
    (error) => error instanceof AvoidInternalError && error.kind === "poisoned" && error.cause.kind === "panic",
  );
  assert.equal(api.internalError().method, "AvoidRouter.__testPanic");
  api.reset();
  assert.ok(connectionPin01(api) > 0);
});

(async () => {
  for (const [name, fn] of tests) {
    try {
      await fn();
      console.log(`ok - ${name}`);
    } catch (error) {
      console.error(`not ok - ${name}`);
      throw error;
    }
  }
  await collectGarbage();
  assert.deepEqual(unexpected, [], "no uncaught exceptions or unhandled rejections");
  console.log("failure containment tests passed");
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
