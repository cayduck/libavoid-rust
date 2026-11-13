"use strict";

// Node smoke tests of the built WASM package through index.cjs: upstream
// libavoid fixture programs (vendor/adaptagrams/cola/libavoid/tests/*.cpp)
// written by hand against the JavaScript API, statement by statement as
// the C++ (same numeric ids), and compared with the C++ goldens in
// tests/golden-bump/ (routes), tests/golden/*.exit and *.stdout.  The
// complete set of fixtures is replayed mechanically by fixtures-replay.cjs;
// these are the readable examples.

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const Avoid = require("../index.cjs");
const helpers = require("./helpers.cjs");

const root = path.join(__dirname, "..");
let checks = 0;
let scenes = 0;

function test(name, fn) {
  try {
    fn();
    scenes++;
    console.log(`ok - ${name}`);
  } catch (error) {
    console.error(`not ok - ${name}`);
    throw error;
  }
}

function assertGolden(router, name) {
  checks += helpers.assertGolden(router, name);
}

const goldenFile = (name, ext) => fs.readFileSync(path.join(root, "tests", "golden", `${name}.${ext}`), "utf8");
const f6 = (v) => v.toFixed(6);

test("fixture example (poly-line, callbacks)", () => {
  let stdout = "";
  // upstream: static void connCallback(void *ptr)
  function connCallback(ptr) {
    const connRef = ptr;
    stdout += `Connector ${connRef.id()} needs rerouting!\n`;
    const route = connRef.displayRoute();
    stdout += "New path: ";
    for (let i = 0; i < route.ps.length; ++i) {
      stdout += `${i > 0 ? "-" : ""}(${f6(route.ps[i].x)}, ${f6(route.ps[i].y)})`;
    }
    stdout += "\n";
  }
  const router = new Avoid.Router(Avoid.PolyLineRouting);
  const srcPt = new Avoid.Point(1.2, 0.5);
  const dstPt = new Avoid.Point(1.5, 4);
  const connRef = new Avoid.ConnRef(router, new Avoid.ConnEnd(srcPt), new Avoid.ConnEnd(dstPt));
  connRef.setCallback(connCallback, connRef);
  // Force inital callback:
  router.processTransaction();
  stdout += "\nAdding a shape.\n";
  // Create the ShapeRef:
  const shapePoly = new Avoid.Polygon(3);
  shapePoly.ps[0] = new Avoid.Point(1, 1);
  shapePoly.ps[1] = new Avoid.Point(2.5, 1.5);
  shapePoly.ps[2] = new Avoid.Point(1.5, 2.5);
  const shapeRef = new Avoid.ShapeRef(router, shapePoly);
  router.processTransaction();
  stdout += "\nShifting endpoint.\n";
  const dstPt2 = new Avoid.Point(6, 4.5);
  connRef.setDestEndpoint(new Avoid.ConnEnd(dstPt2));
  // It's expected you know the connector needs rerouting, so the callback
  // isn't called.  You can force it to be called though, via:
  router.processTransaction();
  stdout += "\nMoving shape right by 0.5.\n";
  router.moveShape(shapeRef, 0.5, 0);
  router.processTransaction();
  assertGolden(router, "example");
  assertGolden(router, "example-final");
  router.delete();
  assert.equal(stdout, goldenFile("example", "stdout"));
});

test("fixture connectionpin01 (pins, moves)", () => {
  checks += helpers.connectionPin01(Avoid);
});

test("fixture inlineoverlap01 (penalties, touching paths)", () => {
  const router = new Avoid.Router(Avoid.OrthogonalRouting);
  router.setRoutingPenalty(Avoid.segmentPenalty, -1);
  router.setRoutingPenalty(Avoid.fixedSharedPathPenalty, -1);
  router.setRoutingParameter(Avoid.idealNudgingDistance, 20);
  const rect = (x1, y1, x2, y2) => new Avoid.Rectangle(new Avoid.Point(x1, y1), new Avoid.Point(x2, y2));
  new Avoid.ShapeRef(router, rect(51140, 50190, 51510, 50885), 335855988);
  new Avoid.ShapeRef(router, rect(51915, 50415, 52285, 50885), 548374163);
  new Avoid.ShapeRef(router, rect(51771, 50790, 51829, 50885), 942733064);
  const end = (x, y, dirs) => new Avoid.ConnEnd(new Avoid.Point(x, y), dirs);
  new Avoid.ConnRef(router, end(51925, 50800, 4), end(51500, 50650, 8), 100850179);
  new Avoid.ConnRef(router, end(51925, 50650, 4), end(51800, 50800, 1), 240732432);
  router.processTransaction();
  assertGolden(router, "inlineoverlap01");
  const overlap = router.existsOrthogonalTouchingPaths();
  assertGolden(router, "inlineoverlap01-final");
  router.delete();
  assert.equal(overlap ? 1 : 0, Number(goldenFile("inlineoverlap01", "exit")));
});

function junctionPenalties(router) {
  router.setRoutingPenalty(Avoid.segmentPenalty, 50);
  router.setRoutingPenalty(Avoid.anglePenalty, 0);
  router.setRoutingPenalty(Avoid.crossingPenalty, 0);
  router.setRoutingPenalty(Avoid.clusterCrossingPenalty, 4000);
  router.setRoutingPenalty(Avoid.fixedSharedPathPenalty, 105);
  router.setRoutingParameter(Avoid.idealNudgingDistance, 25);
}

const SHAPES = [
  [51246, 50475, 51304, 50585, 92712048],
  [50365, 50766, 50485, 50834, 23127921],
  [51740, 50891, 51860, 50959, 159957148],
  [50375, 51015, 50625, 51135, 1350214],
  [50265, 51046, 50375, 51104, 247197258],
  [50700, 50390, 51000, 50610, 281096654],
];

function addShapes(router) {
  for (const [x1, y1, x2, y2, id] of SHAPES) {
    new Avoid.ShapeRef(router, new Avoid.Rectangle(new Avoid.Point(x1, y1), new Avoid.Point(x2, y2)), id);
  }
}

test("fixture junction01 (centre pins, numeric ids)", () => {
  const router = new Avoid.Router(Avoid.OrthogonalRouting);
  junctionPenalties(router);
  const rect478845150 = new Avoid.Rectangle(new Avoid.Point(50695, 51070), new Avoid.Point(50705, 51080));
  const shapeRef478845150 = new Avoid.ShapeRef(router, rect478845150, 478845150);
  const pin = new Avoid.ShapeConnectionPin(shapeRef478845150, Avoid.CONNECTIONPIN_CENTRE, Avoid.ATTACH_POS_CENTRE, Avoid.ATTACH_POS_CENTRE, true, 0.0, Avoid.ConnDirNone);
  pin.setExclusive(false);
  addShapes(router);
  const pt = (x, y, d) => new Avoid.ConnEnd(new Avoid.Point(x, y), d);
  const hub = () => new Avoid.ConnEnd(shapeRef478845150, Avoid.CONNECTIONPIN_CENTRE);
  new Avoid.ConnRef(router, pt(50425, 50800, 15), pt(50710, 50450, 4), 342420237);
  new Avoid.ConnRef(router, pt(50990, 50500, 8), hub(), 352019675);
  new Avoid.ConnRef(router, hub(), pt(50615, 51075, 8), 42699400);
  new Avoid.ConnRef(router, pt(50710, 50550, 4), hub(), 94712625);
  new Avoid.ConnRef(router, pt(50990, 50450, 8), pt(51275, 50485, 1), 92802970);
  new Avoid.ConnRef(router, pt(50710, 50500, 4), pt(51800, 50925, 15), 716502036);
  router.processTransaction();
  assertGolden(router, "junction01");
  assertGolden(router, "junction01-final");
  router.delete();
});

test("fixture checkpoints01 (junctions, checkpoints, junction moves)", () => {
  const router = new Avoid.Router(Avoid.OrthogonalRouting);
  junctionPenalties(router);
  const junctionRef478845150 = new Avoid.JunctionRef(router, new Avoid.Point(50700, 51075), 478845150);
  addShapes(router);
  const pt = (x, y, d) => new Avoid.ConnEnd(new Avoid.Point(x, y), d);
  const j = () => new Avoid.ConnEnd(junctionRef478845150);
  new Avoid.ConnRef(router, pt(50425, 50800, 15), pt(50710, 50450, 4), 342420237);
  new Avoid.ConnRef(router, pt(50990, 50500, 8), j(), 352019675);
  new Avoid.ConnRef(router, j(), pt(50615, 51075, 8), 42699400);
  new Avoid.ConnRef(router, pt(50710, 50550, 4), j(), 94712625);
  const conn = new Avoid.ConnRef(router, pt(50990, 50450, 8), pt(51275, 50485, 1), 92802970);
  const checkpoints = [];
  checkpoints.push(new Avoid.Checkpoint(new Avoid.Point(50000, 50000)));
  checkpoints.push(new Avoid.Checkpoint(new Avoid.Point(50200, 50000)));
  checkpoints.push(new Avoid.Checkpoint(new Avoid.Point(50200, 50200)));
  conn.setRoutingCheckpoints(checkpoints);
  new Avoid.ConnRef(router, pt(50710, 50500, 4), pt(51800, 50925, 15), 716502036);
  router.processTransaction();
  assertGolden(router, "checkpoints01-1");
  router.moveJunction(junctionRef478845150, 585, 0);
  router.processTransaction();
  assertGolden(router, "checkpoints01-2");
  assertGolden(router, "checkpoints01-final");
  router.delete();
});

console.log(`node smoke tests passed (${scenes} upstream fixtures, ${checks} golden routes compared)`);
