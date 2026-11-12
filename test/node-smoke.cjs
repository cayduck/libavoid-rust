"use strict";

// Node tests of the built WASM package through index.cjs:
//  1. upstream libavoid fixture scenes (translated statement by statement
//     from vendor/adaptagrams/cola/libavoid/tests/*.cpp, same numeric ids)
//     compared with the C++ goldens in tests/golden-bump/;
//  2. every exported method called at least once with result-shape checks;
//  3. the README usage example.

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const api = require("../index.cjs");
const {
  AttachPos,
  AvoidRouter,
  ConnDir,
  ConnectionPin,
  ConnectorRelation,
  ConnType,
  OrthogonalDimension,
  RouterFlag,
  RoutingOption,
  RoutingParameter,
  ShapeTransformation,
  WasmAvoidRouter,
  curvedPolyline,
} = api;

const root = path.join(__dirname, "..");
let checks = 0;

function test(name, fn) {
  try {
    fn();
    console.log(`ok - ${name}`);
  } catch (error) {
    console.error(`not ok - ${name}`);
    throw error;
  }
}

// --- golden helpers ----------------------------------------------------------

function golden(name) {
  return fs.readFileSync(path.join(root, "tests", "golden-bump", `${name}.txt`), "utf8");
}

/** Parses the `path` blocks of a diagram text: numeric id -> [[x, y], ...]. */
function goldenPaths(text) {
  const paths = new Map();
  const lines = text.split("\n");
  for (let i = 0; i < lines.length; i++) {
    if (lines[i] !== "path") continue;
    const id = Number(lines[++i].slice(3));
    const pts = [];
    while (i + 1 < lines.length && /^p\d+:/.test(lines[i + 1])) {
      const [, x, y] = lines[++i].trim().split(/\s+/);
      pts.push([Number(x), Number(y)]);
    }
    paths.set(id, pts);
  }
  return paths;
}

/** Diagram text and every display route must equal the golden. */
function assertGolden(router, name) {
  const expected = golden(name);
  assert.equal(router.outputDiagramText(true), expected, `diagram ${name}`);
  const byNumeric = new Map(router.connectorIds().map((id) => [router.objectNumericId(id), id]));
  for (const [nid, pts] of goldenPaths(expected)) {
    const route = router.displayConnectorRoute(byNumeric.get(nid));
    assert.deepEqual(
      route.map((p) => [p.x, p.y]),
      pts,
      `${name}: route of connector ${nid}`,
    );
    checks++;
  }
}

// --- 1. upstream fixture scenes ---------------------------------------------

test("fixture example (poly-line, callbacks)", () => {
  const router = new AvoidRouter(RouterFlag.PolyLineRouting);
  let stdout = "";
  const report = () => {
    for (const id of router.lastTransactionUpdatedConnectorIds()) {
      const p = router.displayConnectorRoute(id).map((q) => `(${q.x.toFixed(6)}, ${q.y.toFixed(6)})`);
      stdout += `Connector ${router.objectNumericId(id)} needs rerouting!\nNew path: ${p.join("-")}\n`;
    }
  };
  assert.equal(router.addConnectorWithConnEnds("connRef", [1.2, 0.5], [1.5, 4], 0), true);
  assert.equal(router.processTransaction(), true);
  report();
  stdout += "\nAdding a shape.\n";
  assert.equal(router.addPolygonShape("shapeRef", [[1, 1], [2.5, 1.5], [1.5, 2.5]], 0), true);
  router.processTransaction();
  report();
  stdout += "\nShifting endpoint.\n";
  assert.equal(router.setConnectorDestConnEnd("connRef", [6, 4.5]), true);
  router.processTransaction();
  report();
  stdout += "\nMoving shape right by 0.5.\n";
  assert.equal(router.moveShapeBy("shapeRef", 0.5, 0), true);
  router.processTransaction();
  report();
  assertGolden(router, "example");
  assert.equal(stdout, fs.readFileSync(path.join(root, "tests", "golden", "example.stdout"), "utf8"));
  router.free();
});

test("fixture connectionpin01 (pins, moves)", () => {
  const router = new AvoidRouter(RouterFlag.OrthogonalRouting);
  router.addShape("shapeRef1", 0, 0, 10, 10, 0);
  router.addShape("shapeRef2", 40, 20, 50, 30, 0);
  assert.equal(router.addShapeConnectionPin("shapeRef1", "p", 1, 0.5, 0.5, true, 0, ConnDir.None), true);
  assert.equal(router.addShapeConnectionPin("shapeRef2", "p", 1, 0.5, 0.5, true, 0, ConnDir.None), true);
  router.addConnectorWithConnEnds("conn", { shapeId: "shapeRef2", classId: 1 }, { shapeId: "shapeRef1", classId: 1 }, 0);
  router.processTransaction();
  assertGolden(router, "connectionpin01-1");
  router.moveShapeBy("shapeRef2", 5, 0);
  router.processTransaction();
  assertGolden(router, "connectionpin01-2");
  router.moveShapeBy("shapeRef1", 0, -10);
  router.processTransaction();
  assertGolden(router, "connectionpin01-3");
  assertGolden(router, "connectionpin01-final");
});

test("fixture inlineoverlap01 (penalties, touching paths)", () => {
  const router = new AvoidRouter(RouterFlag.OrthogonalRouting);
  router.setRoutingPenalty(RoutingParameter.segmentPenalty, -1);
  router.setRoutingPenalty(RoutingParameter.fixedSharedPathPenalty, -1);
  router.setRoutingParameter(RoutingParameter.idealNudgingDistance, 20);
  router.addShape("a", 51140, 50190, 51510, 50885, 335855988);
  router.addShape("b", 51915, 50415, 52285, 50885, 548374163);
  router.addShape("c", 51771, 50790, 51829, 50885, 942733064);
  router.addConnectorWithConnEnds("c1", { point: [51925, 50800], directions: 4 }, { point: [51500, 50650], directions: 8 }, 100850179);
  router.addConnectorWithConnEnds("c2", { point: [51925, 50650], directions: 4 }, { point: [51800, 50800], directions: 1 }, 240732432);
  router.processTransaction();
  assertGolden(router, "inlineoverlap01");
  const exit = Number(fs.readFileSync(path.join(root, "tests", "golden", "inlineoverlap01.exit"), "utf8"));
  assert.equal(router.existsOrthogonalTouchingPaths() ? 1 : 0, exit);
  assertGolden(router, "inlineoverlap01-final");
});

function junctionScene(router) {
  router.setRoutingPenalty(0, 50);
  router.setRoutingPenalty(1, 0);
  router.setRoutingPenalty(2, 0);
  router.setRoutingPenalty(3, 4000);
  router.setRoutingPenalty(4, 105);
  router.setRoutingParameter(7, 25);
}

test("fixture junction01 (centre pins, numeric ids)", () => {
  const router = new AvoidRouter(RouterFlag.OrthogonalRouting);
  junctionScene(router);
  router.addShape("s478845150", 50695, 51070, 50705, 51080, 478845150);
  router.addShape("s92712048", 51246, 50475, 51304, 50585, 92712048);
  router.addShape("s23127921", 50365, 50766, 50485, 50834, 23127921);
  router.addShape("s159957148", 51740, 50891, 51860, 50959, 159957148);
  router.addShape("s1350214", 50375, 51015, 50625, 51135, 1350214);
  router.addShape("s247197258", 50265, 51046, 50375, 51104, 247197258);
  router.addShape("s281096654", 50700, 50390, 51000, 50610, 281096654);
  router.addShapeConnectionPin("s478845150", "centre", ConnectionPin.Centre, 0.5, 0.5, true, 0, ConnDir.None);
  router.setShapeConnectionPinExclusive("s478845150", "centre", false);
  const hub = { shapeId: "s478845150", classId: ConnectionPin.Centre };
  const pt = (x, y, d) => ({ point: [x, y], directions: d });
  router.addConnectorWithConnEnds("c1", pt(50425, 50800, 15), pt(50710, 50450, 4), 342420237);
  router.addConnectorWithConnEnds("c2", pt(50990, 50500, 8), hub, 352019675);
  router.addConnectorWithConnEnds("c3", hub, pt(50615, 51075, 8), 42699400);
  router.addConnectorWithConnEnds("c4", pt(50710, 50550, 4), hub, 94712625);
  router.addConnectorWithConnEnds("c5", pt(50990, 50450, 8), pt(51275, 50485, 1), 92802970);
  router.addConnectorWithConnEnds("c6", pt(50710, 50500, 4), pt(51800, 50925, 15), 716502036);
  router.processTransaction();
  assertGolden(router, "junction01");
  assertGolden(router, "junction01-final");
});

test("fixture checkpoints01 (junctions, checkpoints, junction moves)", () => {
  const router = new AvoidRouter(RouterFlag.OrthogonalRouting);
  junctionScene(router);
  router.addJunction("J", 50700, 51075, 478845150);
  router.addShape("s92712048", 51246, 50475, 51304, 50585, 92712048);
  router.addShape("s23127921", 50365, 50766, 50485, 50834, 23127921);
  router.addShape("s159957148", 51740, 50891, 51860, 50959, 159957148);
  router.addShape("s1350214", 50375, 51015, 50625, 51135, 1350214);
  router.addShape("s247197258", 50265, 51046, 50375, 51104, 247197258);
  router.addShape("s281096654", 50700, 50390, 51000, 50610, 281096654);
  const pt = (x, y, d) => ({ point: [x, y], directions: d });
  router.addConnectorWithConnEnds("c1", pt(50425, 50800, 15), pt(50710, 50450, 4), 342420237);
  router.addConnectorWithConnEnds("c2", pt(50990, 50500, 8), { junctionId: "J" }, 352019675);
  router.addConnectorWithConnEnds("c3", { junctionId: "J" }, pt(50615, 51075, 8), 42699400);
  router.addConnectorWithConnEnds("c4", pt(50710, 50550, 4), { junctionId: "J" }, 94712625);
  router.addConnectorWithConnEnds("c5", pt(50990, 50450, 8), pt(51275, 50485, 1), 92802970);
  assert.equal(
    router.setConnectorRoutingCheckpoints("c5", [[50000, 50000], { point: { x: 50200, y: 50000 } }, [50200, 50200]]),
    true,
  );
  router.addConnectorWithConnEnds("c6", pt(50710, 50500, 4), pt(51800, 50925, 15), 716502036);
  router.processTransaction();
  assertGolden(router, "checkpoints01-1");
  router.moveJunctionBy("J", 585, 0);
  router.processTransaction();
  assertGolden(router, "checkpoints01-2");
  assertGolden(router, "checkpoints01-final");
});

// --- 2. every exported method -----------------------------------------------

const isPoint = (p) => p && typeof p.x === "number" && typeof p.y === "number";
const isRoute = (r) => Array.isArray(r) && r.every(isPoint);
const isBox = (b) => ["left", "top", "right", "bottom"].every((k) => typeof b[k] === "number");
const isNd = (l) =>
  ["newJunctionIds", "deletedJunctionIds", "newConnectorIds", "deletedConnectorIds", "changedConnectorIds"].every(
    (k) => Array.isArray(l[k]),
  );

const called = new Set();
function track(router) {
  return new Proxy(router, {
    get(target, prop) {
      const value = target[prop];
      if (typeof value === "function") {
        called.add(prop);
        return value.bind(target);
      }
      return value;
    },
  });
}

test("every AvoidRouter method", () => {
  assert.throws(() => new AvoidRouter(0), /flags/);
  assert.throws(() => new AvoidRouter("2"), TypeError);
  called.add("constructor");
  const r = track(AvoidRouter.withFlags(RouterFlag.OrthogonalRouting));
  called.add("withFlags");
  assert.equal(r.routerFlags(), 2);
  assert.equal(r.transactionUse(), true);
  r.setTransactionUse(true);

  // parameters
  assert.equal(r.setRoutingParameter(RoutingParameter.shapeBufferDistance, 2), true);
  assert.equal(r.routingParameter(RoutingParameter.shapeBufferDistance), 2);
  assert.equal(r.setRoutingPenalty(RoutingParameter.crossingPenalty), true);
  assert.equal(r.routingParameter(RoutingParameter.crossingPenalty), 200);
  assert.equal(r.routingParameter(99), null);
  assert.equal(r.setRoutingParameter(99, 1), false);
  assert.equal(r.setRoutingOption(RoutingOption.nudgeOrthogonalSegmentsConnectedToShapes, true), true);
  assert.equal(r.routingOption(RoutingOption.nudgeOrthogonalSegmentsConnectedToShapes), true);
  assert.equal(r.routingOption(42), null);
  assert.equal(r.validConnType(), ConnType.Orthogonal);
  assert.equal(r.validConnType(9), null);

  // shapes & clusters
  assert.equal(r.addShape("a", 0, 0, 20, 20), true);
  assert.equal(r.addShape("a", 0, 0, 20, 20), false);
  assert.equal(r.addShape("nan", NaN, 0, 1, 1), false);
  assert.throws(() => r.addShape(1, 0, 0, 1, 1), TypeError);
  assert.equal(r.addPolygonShape("b", [[100, 0], { x: 120, y: 0 }, [120, 20], [100, 20]], 50), true);
  assert.throws(() => r.addPolygonShape("x", "nope"), TypeError);
  assert.equal(r.objectNumericId("b"), 50);
  assert.equal(r.objectIdFromNumericId(50), "b");
  assert.equal(r.objectIdIsUnused("b"), false);
  assert.equal(r.objectIdIsUnused(50), true); // queued, not yet in the scene (upstream)
  assert.equal(r.newObjectId(), 51);
  assert.equal(r.addCluster("k", [[-10, -10], [30, -10], [30, 30], [-10, 30]]), true);
  assert.equal(r.addClusterRectangle("k2", 90, -10, 130, 30), true);
  assert.equal(r.setClusterPolygon("k", [[-11, -11], [31, -11], [31, 31], [-11, 31]]), true);
  assert.equal(r.setClusterRectangle("k2", 89, -10, 130, 30), true);
  assert.ok(isBox(r.clusterBoundingBox("k")));
  assert.ok(isRoute(r.clusterPolygon("k")));
  assert.ok(isRoute(r.clusterRectangularPolygon("k2")));
  assert.deepEqual(r.clusterIds(), ["k", "k2"]);
  assert.equal(r.removeCluster("k2"), true);
  assert.equal(r.clusterBoundingBox("k2"), null);

  // pins
  assert.equal(r.addShapeConnectionPin("a", "right", 1, AttachPos.Right, AttachPos.Centre, true, 0, ConnDir.Right), true);
  assert.equal(
    r.addShapeConnectionPinWithOptions("b", "left", {
      classId: 2,
      xOffset: AttachPos.Left,
      yOffset: AttachPos.Centre,
      proportional: true,
      visibilityDirections: ConnDir.Left,
      connectionCost: 1,
      exclusive: true,
    }),
    true,
  );
  assert.equal(r.addShapeConnectionPin("a", "bad", 0, 0.5, 0.5, true), false); // class 0
  assert.deepEqual(r.shapeConnectionPinIdList("a"), ["right"]);
  assert.ok(isPoint(r.shapeConnectionPinPosition("a", "right")));
  assert.equal(r.shapeConnectionPinDirections("a", "right"), ConnDir.Right);
  assert.deepEqual(r.shapeConnectionPinIds("a", "right"), { objectId: "a", numericObjectId: 1, classId: 1 });
  assert.equal(r.setShapeConnectionPinConnectionCost("a", "right", 2), true);
  assert.equal(r.setShapeConnectionPinConnectionCost("a", "right", -2), false);
  assert.equal(r.setShapeConnectionPinExclusive("a", "right", true), true);
  assert.equal(r.shapeConnectionPinExclusive("a", "right"), true);
  assert.equal(r.shapeConnectionPinExclusive("a", "nope"), null);

  // connectors
  assert.equal(r.addConnectorBetweenPins("pins", "a", "right", "b", "left"), true);
  assert.equal(r.addConnectorBetweenPinClasses("classes", "a", 1, "b", 2), true);
  assert.equal(r.addConnector("free", -50, 50, 150, 50), true);
  assert.equal(r.addConnectorWithEndpointDirections("dirs", [-50, 80], ConnDir.Right, [150, 80], ConnDir.Left), true);
  assert.equal(r.addConnectorWithoutEndpoints("later"), true);
  assert.equal(r.addConnectorWithConnEnds("bad", [0, 0], { shapeId: "zz", classId: 1 }), false);
  assert.throws(() => r.addConnectorWithConnEnds("bad", 5, [0, 0]), TypeError);
  assert.equal(r.processTransaction(), true);
  const updated = r.lastTransactionUpdatedConnectorIds();
  assert.ok(updated.includes("pins") && updated.includes("free"));
  assert.ok(isRoute(r.routeConnector("pins")));
  assert.ok(r.displayConnectorRoute("pins").length >= 2);
  assert.equal(r.displayConnectorRoute("nope"), null);
  assert.deepEqual(r.displayConnectorRoute("later"), []);
  assert.equal(r.shapeContainingPoint([5, 5]), "a");
  assert.equal(r.shapeContainingPoint({ x: 500, y: 5 }), null);
  assert.ok(isBox(r.shapeBoundingBox("a")));
  assert.ok(isBox(r.shapeRoutingBox("a")));
  assert.ok(isRoute(r.shapePolygon("a")));
  assert.ok(isRoute(r.shapeRoutingPolygon("a")));
  assert.ok(isPoint(r.shapePosition("a")));
  assert.deepEqual(r.shapeAttachedConnectors("a").sort(), ["classes", "pins"]);
  assert.deepEqual(r.shapeAttachedShapes("a", ConnectorRelation.RunningFrom), ["b", "b"]);
  assert.deepEqual(r.shapeAttachedConns("b").sort(), ["classes", "pins"]);
  assert.ok(Array.isArray(r.connectorPossibleDstPinPoints("pins")));
  const ends = r.connectorEndpointConnEnds("pins");
  assert.equal(ends.source.type, "shapePin");
  assert.equal(ends.source.shapeId, "a");
  assert.equal(ends.destination.classId, 2);
  assert.equal(r.connectorRoutingType("free"), ConnType.Orthogonal);
  assert.equal(r.setConnectorRoutingType("free", ConnType.Orthogonal), true);
  assert.equal(r.setConnectorHateCrossings("free", true), true);
  assert.equal(r.connectorDoesHateCrossings("free"), true);
  assert.equal(r.connectorNeedsRepaint("free"), true);
  assert.equal(r.makeConnectorPathInvalid("free"), true);

  // endpoint setters
  assert.equal(r.setConnectorEndpoints("later", [-50, 120], [150, 120]), true);
  assert.equal(r.setConnectorEndpointsWithDirections("later", [-50, 120], ConnDir.All, [150, 120], ConnDir.All), true);
  assert.equal(r.setConnectorSourceEndpoint("later", [-50, 121]), true);
  assert.equal(r.setConnectorSourceEndpointWithDirections("later", [-50, 120], ConnDir.Right), true);
  assert.equal(r.setConnectorDestEndpoint("later", [150, 121]), true);
  assert.equal(r.setConnectorDestEndpointWithDirections("later", [150, 120], ConnDir.Left), true);
  assert.equal(r.setConnectorConnEnds("later", [-50, 120], [150, 120]), true);
  assert.equal(r.setConnectorSourceConnEnd("later", { point: [-50, 120], directions: ConnDir.All }), true);
  assert.equal(r.setConnectorDestConnEnd("later", { x: 150, y: 120 }), true);
  assert.equal(r.setConnectorSourcePinEndpoint("classes", "a", "right"), true);
  assert.equal(r.setConnectorDestPinEndpoint("classes", "b", "left"), true);
  assert.equal(r.setConnectorSourcePinClassEndpoint("classes", "a", 1), true);
  assert.equal(r.setConnectorDestPinClassEndpoint("classes", "b", 2), true);
  assert.equal(r.setConnectorDestPinClassEndpoint("classes", "b", ConnectionPin.Unset), false);

  // checkpoints, fixed routes, curves
  assert.equal(r.setConnectorRoutingCheckpoints("dirs", [{ point: [50, 100], arrivalDirections: ConnDir.All }]), true);
  assert.deepEqual(r.connectorRoutingCheckpoints("dirs"), [
    { point: { x: 50, y: 100 }, arrivalDirections: 15, departureDirections: 15 },
  ]);
  assert.equal(r.processTransaction(), true);
  assert.equal(r.setConnectorFixedExistingRoute("dirs"), true);
  assert.equal(r.connectorHasFixedRoute("dirs"), true);
  assert.equal(r.setConnectorFixedRoute("dirs", [[-50, 80], [150, 80]]), true);
  assert.equal(r.clearConnectorFixedRoute("dirs"), true);
  assert.equal(r.connectorHasFixedRoute("dirs"), false);
  const curved = r.connectorCurvedRoute("free", 4);
  assert.ok(isRoute(curved.points) && curved.commands[0] === "M");
  assert.deepEqual(curvedPolyline([[0, 0], [10, 0], [10, 10]], 2).commands, ["M", "L", "C", "C", "C", "L"]);

  // junctions
  assert.equal(r.addJunction("J", 50, 200), true);
  assert.equal(r.removeJunction("J"), false); // its addition is still queued (upstream asserts)
  assert.equal(r.addConnectorFromJunction("j1", "J", 0, 250), true);
  assert.equal(r.addConnectorToJunction("j2", 100, 250, "J"), true);
  assert.equal(r.addJunction("J2", 50, 300), true);
  assert.equal(r.addConnectorBetweenJunctions("jj", "J", "J2"), true);
  assert.equal(r.removeConnector("jj"), true);
  assert.equal(r.removeConnector("jj"), false);
  assert.equal(r.setConnectorSourceJunctionEndpoint("later", "J"), true);
  assert.equal(r.setConnectorDestJunctionEndpoint("later", "J2"), true);
  assert.equal(r.setConnectorEndpoints("later", [-50, 120], [150, 120]), true);
  assert.equal(r.processTransaction(), true);
  assert.equal(r.moveJunction("J", 55, 200), true);
  assert.equal(r.moveJunctionBy("J", -5, 0), true);
  assert.equal(r.processTransaction(), true);
  assert.deepEqual(r.junctionPosition("J"), { x: 50, y: 200 });
  assert.ok(isPoint(r.junctionRecommendedPosition("J")));
  assert.equal(r.setJunctionPositionFixed("J", true), true);
  assert.equal(r.junctionPositionFixed("J"), true);
  assert.equal(r.preferJunctionOrthogonalDimension("J", OrthogonalDimension.X), true);
  assert.equal(r.preferJunctionOrthogonalDimension("J", 7), false);
  assert.deepEqual(r.junctionAttachedConnectors("J").sort(), ["j1", "j2"]);
  assert.equal(r.addJunctionConnectionPin("J", "jp", 3, ConnDir.Up), true);
  assert.ok(isPoint(r.junctionConnectionPinPosition("J", "jp")));
  assert.equal(r.junctionConnectionPinDirections("J", "jp"), ConnDir.Up);
  assert.equal(r.junctionConnectionPinIds("J", "jp").classId, 3);
  assert.equal(r.setJunctionConnectionPinConnectionCost("J", "jp", 1), true);
  assert.equal(r.setJunctionConnectionPinExclusive("J", "jp", false), true);
  assert.equal(r.junctionConnectionPinExclusive("J", "jp"), false);
  assert.equal(r.removeJunctionConnectionPin("J", "jp"), true);
  // Upstream returns nullptr when the second connector's other end is a free
  // point (it has no owned ConnEnd); attach j2 to a pin class first.
  assert.equal(r.removeJunctionAndMergeConnectors("J"), null);
  assert.equal(r.setConnectorSourcePinClassEndpoint("j2", "b", 2), true);
  r.processTransaction();
  const merged = r.removeJunctionAndMergeConnectors("J");
  assert.ok(merged === "j1" || merged === "j2", `merged ${merged}`);
  assert.equal(r.junctionPosition("J"), null);
  r.processTransaction();

  // split
  assert.equal(r.splitConnectorAtSegment("free", 0, "S", "free2"), null);
  assert.equal(r.splitConnectorAtSegment("pins", 1, "S", "pins2").junctionId, "S");
  r.processTransaction();
  assert.deepEqual(r.junctionAttachedConnectors("S").sort(), ["pins", "pins2"]);

  // diagnostics
  assert.equal(typeof r.existsCrossings(), "number");
  assert.equal(typeof r.existsInvalidOrthogonalPaths(), "boolean");
  assert.equal(typeof r.existsOrthogonalSegmentOverlap(true), "boolean");
  assert.equal(typeof r.existsOrthogonalFixedSegmentOverlap(), "boolean");
  assert.equal(typeof r.existsOrthogonalTouchingPaths(), "boolean");
  assert.match(r.outputDiagramText(), /^rect\nid=\d+\n/);
  assert.match(r.printInfo(), /Visibility Graph info/);
  const all = r.routeAll();
  assert.ok(isRoute(all.free));

  // shape moves / transforms / removal
  assert.equal(r.moveShape("a", 0, 0, 22, 20), true);
  assert.equal(r.movePolygonShape("b", [[100, 0], [121, 0], [121, 20], [100, 20]], false), true);
  assert.equal(r.movePolygonShape("b", [[100, 0], [121, 0], [121, 20]]), false); // point count changes
  assert.equal(r.moveShapeBy("a", 1, 1), true);
  assert.equal(r.transformShapeConnectionPinPositions("a", ShapeTransformation.FlipX), true);
  r.markAllObstaclesAsMoved();
  assert.equal(r.processTransaction(), true);
  assert.equal(r.removeShapeConnectionPin("a", "right"), true);
  assert.equal(r.removeShape("a"), true);
  assert.equal(r.removeShape("a"), false);
  assert.deepEqual(r.shapeIds(), ["b"]);
  assert.ok(r.junctionIds().includes("S"));
  assert.ok(r.connectorIds().includes("free"));
  r.processTransaction();

  // hyperedges
  assert.equal(r.registerHyperedgeForReroutingJunction("nope"), null);
  assert.equal(r.registerHyperedgeForReroutingJunction("S"), 0);
  assert.equal(r.hyperedgeReroutingCount(), 1);
  assert.equal(r.registerHyperedgeForRerouting([{ shapeId: "b", classId: 99 }]), null);
  // Duplicate / fewer than 3 / free-point terminals are refused (upstream
  // would dereference a null MTST root).
  assert.equal(r.registerHyperedgeForRerouting([{ shapeId: "b", pinId: "left" }, { shapeId: "b", classId: 2 }]), null);
  assert.equal(r.registerHyperedgeForRerouting([[0, 0], { shapeId: "b", classId: 2 }, { junctionId: "S" }]), null);
  r.processTransaction();
  assert.equal(r.hyperedgeReroutingCount(), 0);
  assert.ok(isNd(r.hyperedgeNewAndDeletedObjectLists(0)));
  assert.equal(r.hyperedgeNewAndDeletedObjectLists(5), null);
  assert.ok(isNd(r.newAndDeletedObjectListsFromHyperedgeImprovement()));

  // transactions off
  r.clear();
  assert.deepEqual(r.shapeIds(), []);
  assert.equal(r.routingParameter(RoutingParameter.shapeBufferDistance), 2);
  r.setTransactionUse(false);
  assert.equal(r.transactionUse(), false);
  r.addShape("s", 10, -10, 20, 10);
  r.addConnector("c", 0, 0, 30, 0);
  assert.ok(r.displayConnectorRoute("c").length >= 4);
  r.moveShapeBy("s", 0, 50);
  assert.deepEqual(r.lastTransactionUpdatedConnectorIds(), ["c"]);
  assert.deepEqual(r.displayConnectorRoute("c"), [{ x: 0, y: 0 }, { x: 30, y: 0 }]);
  r.free();
  called.add("free");

  const methods = Object.getOwnPropertyNames(AvoidRouter.prototype);
  const missed = methods.filter((m) => !called.has(m));
  assert.deepEqual(missed, [], "methods not exercised");
});

test("connectors created by hyperedge rerouting are reported as updated", () => {
  const r = new AvoidRouter(RouterFlag.OrthogonalRouting);
  r.addJunction("j", 150, 150);
  for (const [id, x, y] of [["a", 0, 0], ["b", 300, 20], ["c", 40, 260], ["d", 320, 280]]) {
    r.addShape(id, x, y, x + 60, y + 40);
    r.addShapeConnectionPin(id, "p", 1, 0.5, 0.5, true, 0, ConnDir.None);
    r.addConnectorWithConnEnds(`to-${id}`, { shapeId: id, pinId: "p" }, { junctionId: "j" });
  }
  r.processTransaction();
  assert.deepEqual(r.lastTransactionUpdatedConnectorIds().sort(), ["to-a", "to-b", "to-c", "to-d"]);
  assert.equal(r.registerHyperedgeForReroutingJunction("j"), 0);
  r.processTransaction();
  const { newConnectorIds } = r.hyperedgeNewAndDeletedObjectLists(0);
  assert.ok(newConnectorIds.length >= 4);
  // Upstream creates them without a callback; the wrapper still reports them.
  assert.deepEqual(r.lastTransactionUpdatedConnectorIds().sort(), [...newConnectorIds].sort());
  assert.deepEqual(r.connectorIds().sort(), [...newConnectorIds].sort());
  r.free();
});

test("raw WasmAvoidRouter never aborts on bad input", () => {
  const w = new WasmAvoidRouter(2);
  assert.equal(w.addShape("s", NaN, 0, 1, 1), false);
  assert.equal(w.addPolygonShape("s", [1, 2, 3]), false);
  assert.equal(w.setRoutingParameter(-1, 1), false);
  assert.equal(w.routingParameter(1e9), undefined);
  assert.equal(w.displayConnectorRoute("x"), "null");
  assert.equal(w.addConnectorWithConnEnds("c", 7, "", "", 0, 0, 0, 0, "", "", 1, 1, 15), false);
  assert.equal(w.registerHyperedgeForRerouting([0], [], [], [], [], []), undefined);
  assert.equal(w.processTransaction(), false);
  w.free();
});

// --- 3. README example -------------------------------------------------------

test("README JavaScript usage example runs", () => {
  const readme = fs.readFileSync(path.join(root, "README.md"), "utf8");
  const match = readme.match(/## JavaScript usage\s+```js\n([\s\S]*?)```/);
  assert.ok(match, "README example not found");
  const code = match[1].replace('require("libavoid-rust")', 'require("../index.cjs")');
  const logs = [];
  const fakeConsole = { log: (...args) => logs.push(args) };
  new Function("require", "console", code)(require, fakeConsole);
  assert.ok(logs.length > 0);
  assert.ok(logs.some((args) => isRoute(args[0]) && args[0].length >= 2), "README example prints a route");
});

console.log(`node tests passed (${checks} golden routes compared)`);
