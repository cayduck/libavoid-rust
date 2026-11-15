"use strict";

// Failure containment of the Node entry (index.cjs), on the shipped wasm:
//  1. the built .wasm / glue embed no absolute path of the build machine;
//  2. every kind of internal failure (explicit panic, index out of bounds in
//     a router method, stack overflow, failed allocation), triggered with
//     the armed test-only trap trigger, throws AvoidInternalError with the
//     expected fields, poisons every object of the instance, and reset()
//     gives a fresh instance that routes an upstream fixture exactly;
//  3. Router::shouldContinueTransactionWithProgress() overridden by
//     subclassing: upstream's arguments and stop semantics, and the router
//     stays usable (errors thrown by the override, re-entrant use, a trap
//     inside the override).
// Run with --expose-gc so that garbage collection of detached routers (whose
// finalizers must not call into a dead instance) happens during the test.

const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const Avoid = require("../index.cjs");
const { connectionPin01, root } = require("./helpers.cjs");

const { AvoidInternalError, Router, ShapeRef, ConnRef, ConnEnd, Point, Rectangle } = Avoid;
const ARM = "libavoid-rust: I want to trap this WebAssembly instance";
const testPanic = (router, kind, arming) => Router[Symbol.for("libavoid-rust.testPanic")](router, kind, arming);

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

const rect = (x1, y1, x2, y2) => new Rectangle(new Point(x1, y1), new Point(x2, y2));
const ptEnd = (x, y) => new ConnEnd(new Point(x, y));

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
  const wasm = fs.readFileSync(path.join(root, "pkg-web", "libavoid_rust_bg.wasm"));
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
  const r = new Router(Avoid.OrthogonalRouting);
  new ShapeRef(r, rect(0, 0, 60, 40));
  new ShapeRef(r, rect(160, 80, 220, 140));
  const c = new ConnRef(r, ptEnd(30, 20), ptEnd(190, 110));
  r.processTransaction();
  return { r, c };
}

function assertPoisoned(fn, first, method) {
  assert.throws(fn, (error) => {
    assert.ok(error instanceof AvoidInternalError, `${method}: ${error}`);
    assert.equal(error.kind, "poisoned");
    assert.equal(error.method, method);
    assert.equal(error.cause, first);
    assert.equal(error.message, `${method}: object is unusable after an internal error: ${first.message}`);
    return true;
  });
}

for (const { kind, expect, panic, message } of KINDS) {
  test(`trap kind ${kind} (${expect}) poisons the instance; reset() recovers`, async () => {
    Avoid.reset(); // start from a fresh instance
    assert.equal(Avoid.internalError(), null);
    const bystander = sceneRouter();
    const victim = sceneRouter();
    const idle = new Router(Avoid.PolyLineRouting);

    // Unarmed: inert.
    assert.equal(testPanic(victim.r, kind, "please"), false);
    assert.equal(Avoid.internalError(), null);

    let first;
    assert.throws(
      () => testPanic(victim.r, kind, ARM),
      (error) => {
        first = error;
        assert.ok(error instanceof AvoidInternalError);
        assert.ok(error instanceof Error);
        assert.equal(error.name, "AvoidInternalError");
        assert.equal(error.kind, expect);
        assert.equal(error.method, "Router.__testPanic");
        assert.ok(error.cause instanceof WebAssembly.RuntimeError || error.cause instanceof RangeError, String(error.cause));
        assert.ok(error.message.startsWith("libavoid-rust internal error in Router.__testPanic: "), error.message);
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
    // never involved, their objects, new routers and free functions.
    assert.equal(Avoid.internalError(), first);
    assertPoisoned(() => victim.r.connRefs, first, "Router.connRefs");
    assertPoisoned(() => bystander.c.displayRoute(), first, "ConnRef.displayRoute");
    assertPoisoned(() => bystander.r.processTransaction(), first, "Router.processTransaction");
    assertPoisoned(() => new ShapeRef(idle, rect(0, 0, 1, 1)), first, "ShapeRef");
    assertPoisoned(() => idle.setRoutingParameter(Avoid.segmentPenalty, 10), first, "Router.setRoutingParameter");
    assertPoisoned(() => new Router(Avoid.OrthogonalRouting), first, "Router");
    assertPoisoned(() => rect(0, 0, 10, 10).simplify(), first, "Polygon.simplify");
    assertPoisoned(() => Avoid.euclideanDist(new Point(0, 0), new Point(1, 1)), first, "euclideanDist");
    // delete() is still allowed (a no-op).
    bystander.r.delete();

    // Recovery: a fresh instance routes an upstream fixture exactly.
    Avoid.reset();
    assert.equal(Avoid.internalError(), null);
    assert.ok(connectionPin01(Avoid) > 0);
    // Objects of the old instance stay poisoned with the original error.
    assertPoisoned(() => victim.r.connRefs, first, "Router.connRefs");
    assertPoisoned(() => victim.c.id(), first, "ConnRef.id");

    // Garbage collection of the old instance's routers must not call into it.
    await collectGarbage();
  });
}

test("reset() of a healthy instance invalidates its objects", () => {
  const before = sceneRouter();
  Avoid.reset();
  assert.throws(
    () => before.r.connRefs,
    (error) =>
      error instanceof AvoidInternalError &&
      error.kind === "poisoned" &&
      error.cause.kind === "reset" &&
      /object is unusable: its WebAssembly instance was replaced by reset\(\)/.test(error.message),
  );
  assert.equal(Avoid.internalError(), null);
  assert.ok(connectionPin01(Avoid) > 0);
});

test("ordinary errors do not poison", () => {
  const { r, c } = sceneRouter();
  assert.throws(() => new ShapeRef(r, "not a polygon"), TypeError);
  assert.throws(() => new Router(4), /flags must include PolyLineRouting or OrthogonalRouting/);
  assert.throws(() => new ShapeRef(r, rect(0, 0, 1, 1), c.id()), /already used/);
  r.deleteConnector(c);
  assert.throws(() => c.displayRoute(), /ConnRef has been deleted/);
  assert.equal(Avoid.internalError(), null);
  new ConnRef(r, ptEnd(0, 0), ptEnd(10, 10));
  assert.equal(r.processTransaction(), true);
  r.delete();
});

// --- 3. shouldContinueTransactionWithProgress() ---------------------------------------

/** A Router whose shouldContinueTransactionWithProgress() calls `fn` (upstream: subclass and override). */
class ProgressRouter extends Router {
  constructor(flags, fn) {
    super(flags);
    this.fn = fn;
  }
  shouldContinueTransactionWithProgress(elapsedTime, phaseNumber, totalPhases, proportion) {
    return this.fn ? this.fn(elapsedTime, phaseNumber, totalPhases, proportion) : true;
  }
}

/** Eight pairwise crossing connectors and a crossing penalty, so that improveCrossings() runs. */
function crossingScene(r) {
  r.setRoutingParameter(Avoid.crossingPenalty, 200);
  r.setRoutingParameter(Avoid.shapeBufferDistance, 4);
  new ShapeRef(r, rect(40, 40, 60, 60));
  for (let i = 0; i < 4; i++) {
    new ConnRef(r, ptEnd(0, 20 + 20 * i), ptEnd(100, 80 - 20 * i));
    new ConnRef(r, ptEnd(20 + 20 * i, 0), ptEnd(80 - 20 * i, 100));
  }
  return r;
}

const routes = (r) => Object.fromEntries(r.connRefs.map((c) => [c.id(), c.displayRoute().ps.map((p) => [p.x, p.y])]));

test("the override sees upstream's arguments for every phase", () => {
  const phases = new Set();
  let lastElapsed = -1;
  const r = new ProgressRouter(Avoid.OrthogonalRouting, (elapsed, phase, total, proportion) => {
    assert.equal(total, Avoid.TransactionPhaseCompleted);
    assert.ok(proportion >= 0 && proportion <= 1);
    assert.ok(Number.isInteger(elapsed) && elapsed >= lastElapsed);
    lastElapsed = elapsed;
    phases.add(phase);
    return true;
  });
  crossingScene(r);
  assert.equal(r.processTransaction(), true);
  assert.deepEqual([...phases].sort(), [1, 2, 3, 4, 5, 6, 7, 8]);
  r.delete();
});

test("a stop request: upstream semantics (only crossing improvement stops; nothing reruns by itself)", () => {
  const reference = crossingScene(new Router(Avoid.OrthogonalRouting));
  reference.processTransaction();
  const expected = routes(reference);
  reference.delete();

  for (const [stopAt, skipsWork] of [
    [Avoid.TransactionPhaseRouteSearch, true], // takes effect at the crossing check
    [Avoid.TransactionPhaseCrossingDetection, true],
    [Avoid.TransactionPhaseRerouteSearch, true],
    [Avoid.TransactionPhaseOrthogonalNudgingX, false], // nudging does not act on it
  ]) {
    let asked = 0;
    const r = crossingScene(
      new ProgressRouter(Avoid.OrthogonalRouting, (elapsed, phase) => {
        if (phase === stopAt) asked++;
        return phase !== stopAt;
      }),
    );
    assert.equal(r.processTransaction(), true);
    assert.ok(asked > 0);
    const stopped = routes(r);
    assert.equal(JSON.stringify(stopped) !== JSON.stringify(expected), skipsWork, `phase ${stopAt}`);
    if (stopAt === Avoid.TransactionPhaseRerouteSearch) {
      // Upstream frees the routes of a crossing group before rerouting it:
      // connectors not yet rerouted when it stops are left without a route.
      assert.ok(Object.values(stopped).some((route) => route.length === 0));
    }
    r.fn = null;
    // Upstream: nothing is queued, so processTransaction() does nothing.
    assert.equal(r.processTransaction(), false);
    assert.deepEqual(routes(r), stopped);
    // Upstream's way to rerun the routing: registerSettingsChange().
    r.registerSettingsChange();
    assert.equal(r.processTransaction(), true);
    assert.deepEqual(routes(r), expected, `phase ${stopAt}: finished`);
    r.delete();
  }
});

test("a time limit implemented in the override (elapsedTime in ms)", () => {
  const r = crossingScene(new ProgressRouter(Avoid.OrthogonalRouting, (elapsed) => elapsed < 0));
  r.processTransaction();
  assert.ok(Object.values(routes(r)).length === 8);
  r.delete();
});

test("an error thrown by the override stops the transaction and is rethrown; the router stays usable", () => {
  const boom = new Error("boom");
  const r = crossingScene(
    new ProgressRouter(Avoid.OrthogonalRouting, (elapsed, phase) => {
      if (phase === Avoid.TransactionPhaseCrossingDetection) throw boom;
      return true;
    }),
  );
  assert.throws(() => r.processTransaction(), (error) => error === boom);
  assert.equal(Avoid.internalError(), null);
  r.fn = null;
  r.registerSettingsChange();
  assert.equal(r.processTransaction(), true);
  r.delete();
});

test("the override cannot call into its own router", () => {
  let inner;
  const r = new ProgressRouter(Avoid.OrthogonalRouting, null);
  crossingScene(r);
  r.fn = () => {
    try {
      void r.connRefs;
    } catch (error) {
      inner = error;
      throw error;
    }
    return true;
  };
  assert.throws(() => r.processTransaction(), /cannot be used from inside its own shouldContinueTransactionWithProgress/);
  assert.ok(inner instanceof Error && !(inner instanceof AvoidInternalError));
  r.fn = null;
  r.registerSettingsChange();
  assert.equal(r.processTransaction(), true);
  r.delete();
});

test("a trap inside the override poisons the instance; the outer call reports it", () => {
  Avoid.reset();
  const other = new Router(Avoid.OrthogonalRouting);
  const outer = crossingScene(
    new ProgressRouter(Avoid.OrthogonalRouting, () => {
      testPanic(other, 1, ARM);
      return true;
    }),
  );
  assert.throws(
    () => outer.processTransaction(),
    (error) => error instanceof AvoidInternalError && error.kind === "poisoned" && error.cause.kind === "panic",
  );
  assert.equal(Avoid.internalError().method, "Router.__testPanic");
  Avoid.reset();
  assert.ok(connectionPin01(Avoid) > 0);
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
  console.log(`failure containment tests passed (${tests.length})`);
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
