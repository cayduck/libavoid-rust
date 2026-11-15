"use strict";

// Conformance of the JavaScript API with upstream libavoid's public API:
//  1. every row of docs/JS_API_COVERAGE.md: the listed export / method /
//     property exists, constants have upstream's values, and the number of
//     required arguments equals upstream's (parameters without a default);
//     every export and every public method of the JavaScript classes is
//     accounted for by a row (nothing without an upstream counterpart
//     except the documented WebAssembly infrastructure and the additions
//     from the libavoid-js fork);
//  2. unit tests for each overload set, each default argument, callbacks
//     (order, ptr, reroute after a move), virtual methods overridden by
//     subclassing, the Router's public data members, object identity and
//     invalidation (also for objects the router creates and deletes
//     itself), the C++ idiom mappings, and misuse (TypeError / Error, never
//     an AvoidInternalError, no state change).

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const Avoid = require("../index.cjs");

const root = path.join(__dirname, "..");
const { Router, ShapeRef, JunctionRef, ConnRef, ConnEnd, ShapeConnectionPin, ClusterRef, Point, Polygon, Rectangle, Checkpoint } = Avoid;

let count = 0;
function test(name, fn) {
  try {
    fn();
    count++;
    console.log(`ok - ${name}`);
  } catch (error) {
    console.error(`not ok - ${name}`);
    throw error;
  }
}

const P = (x, y) => new Point(x, y);
const rect = (x1, y1, x2, y2) => new Rectangle(P(x1, y1), P(x2, y2));
const xy = (poly) => poly.ps.map((p) => [p.x, p.y]);

// --- 1. docs/JS_API_COVERAGE.md -------------------------------------------------------

const rows = [];
let fromFork = false; // rows under "## Additions from the libavoid-js fork"
for (const line of fs.readFileSync(path.join(root, "docs", "JS_API_COVERAGE.md"), "utf8").split("\n")) {
  if (line.startsWith("## ")) fromFork = line.startsWith("## Additions from the libavoid-js fork");
  if (!line.startsWith("| `") && !line.startsWith("| —")) continue;
  const cells = line
    .trim()
    .replace(/^\|/, "")
    .replace(/\|$/, "")
    .split(/(?<!\\)\|/)
    .map((c) => c.trim());
  assert.equal(cells.length, 4, `malformed row: ${line}`);
  rows.push({ upstream: cells[0], js: cells[1], arg: cells[2], notes: cells[3], fromFork });
}

function sampleInstance(cls) {
  if (cls === Rectangle) return rect(0, 0, 1, 1);
  return new cls();
}

const mapped = new Map(); // "X" or "X#m" -> rows
test("docs/JS_API_COVERAGE.md: every row holds", () => {
  for (const row of rows) {
    if (row.js === "—") {
      assert.ok(row.notes.length > 10, `${row.upstream}: a row that is not exposed needs a reason`);
      continue;
    }
    const m = /^`Avoid\.(\w+)(?:#(\w+))?( \(property\))?`$/.exec(row.js);
    assert.ok(m, `${row.upstream}: bad JavaScript cell ${row.js}`);
    const [, name, member, isProp] = m;
    assert.ok(name in Avoid, `${row.upstream}: Avoid.${name} is not exported`);
    const key = member ? `${name}#${member}` : name;
    if (!mapped.has(key)) mapped.set(key, []);
    mapped.get(key).push(row);
    const value = Avoid[name];
    const overload = /\boverload\b/.test(row.notes);
    if (!member) {
      if (row.arg === "class") assert.equal(typeof value, "function", `${row.upstream}: Avoid.${name} is a class`);
      else if (row.arg === "object") assert.equal(typeof value, "object", `${row.upstream}: Avoid.${name} is an object`);
      else if (typeof value === "function") {
        if (!overload) assert.equal(value.length, Number(row.arg), `${row.upstream}: Avoid.${name}.length`);
      } else {
        assert.equal(value, Number(row.arg), `${row.upstream}: value of Avoid.${name}`);
      }
      continue;
    }
    if (isProp) {
      const onProto = member in value.prototype;
      const onInstance = !onProto && member in sampleInstance(value);
      assert.ok(onProto || onInstance, `${row.upstream}: Avoid.${name} has no property ${member}`);
      continue;
    }
    assert.equal(typeof value.prototype[member], "function", `${row.upstream}: Avoid.${name}#${member} is not a method`);
    if (!overload) assert.equal(value.prototype[member].length, Number(row.arg), `${row.upstream}: Avoid.${name}#${member}.length`);
  }
});

const INFRASTRUCTURE = new Set(["AvoidInternalError", "internalError", "reset"]);
const ENUM_OBJECTS = new Set([
  "RouterFlag",
  "RoutingParameter",
  "PenaltyType",
  "RoutingOption",
  "TransactionPhases",
  "ConnType",
  "ConnDirFlag",
  "ConnEndType",
  "ShapeTransformationType",
]);

test("every export and every public method has an upstream counterpart", () => {
  for (const name of Object.keys(Avoid)) {
    if (INFRASTRUCTURE.has(name)) continue;
    if (ENUM_OBJECTS.has(name)) {
      for (const [k, v] of Object.entries(Avoid[name])) assert.equal(Avoid[k], v, `${name}.${k} aliases Avoid.${k}`);
      continue;
    }
    assert.ok(mapped.has(name), `export ${name} has no row in docs/JS_API_COVERAGE.md`);
    const cls = Avoid[name];
    if (typeof cls !== "function" || !cls.prototype) continue;
    for (const member of Object.getOwnPropertyNames(cls.prototype)) {
      if (member === "constructor") continue;
      let owner = cls;
      // Inherited members are documented on the class that declares them.
      while (owner && !mapped.has(`${owner.name}#${member}`)) owner = Object.getPrototypeOf(owner);
      const valueTypeDelete = member === "delete" && !mapped.has(`${name}#delete`);
      assert.ok(owner || valueTypeDelete, `${name}#${member} has no row in docs/JS_API_COVERAGE.md`);
    }
  }
});

const upstreamRows = rows.filter((r) => !r.fromFork);
const total = upstreamRows.length;
const exposed = upstreamRows.filter((r) => r.js !== "—").length;
console.log(
  `coverage: ${total} upstream items, ${exposed} exposed, ${total - exposed} not exposable (reason given); ` +
    `${rows.length - total} additions from the libavoid-js fork`,
);

// --- 2. unit tests ------------------------------------------------------------------------

test("constants and enum aliases", () => {
  assert.equal(Avoid.PenaltyType, Avoid.RoutingParameter);
  assert.equal(Avoid.RouterFlag.OrthogonalRouting, Avoid.OrthogonalRouting);
  assert.equal(Avoid.Vector, Point);
  assert.equal(Avoid.PolyLine, Polygon);
  assert.equal(Avoid.CONNECTIONPIN_CENTRE, Avoid.CONNECTIONPIN_UNSET - 1);
});

test("Point: constructors, data members, operators", () => {
  const o = new Point();
  assert.deepEqual([o.x, o.y, o.id, o.vn], [0, 0, 0, Avoid.kUnassignedVertexNumber]);
  const p = P(1, 2);
  assert.ok(p.equal(P(1, 2)) && !p.notEqual(P(1, 2)));
  assert.ok(p.equals(P(1.00001, 2)) && !p.equals(P(1.00001, 2), 0.000001), "equals(rhs, epsilon = 0.0001)");
  assert.ok(P(1, 2).lessThan(P(1, 3)) && P(0, 9).lessThan(P(1, 0)));
  assert.equal(p.get(Avoid.XDIM), 1);
  p.set(Avoid.YDIM, 5);
  assert.equal(p.y, 5);
  assert.deepEqual([p.add(P(1, 1)).x, p.sub(P(1, 1)).y], [2, 4]);
  assert.throws(() => p.get(2), RangeError);
  assert.throws(() => new Point(1), TypeError);
  assert.throws(() => new Point("1", 2), TypeError);
});

test("Polygon / Rectangle / PolyLine overloads and methods", () => {
  assert.equal(new Polygon().size(), 0);
  assert.equal(new Polygon(3).size(), 3);
  const r1 = rect(10, 20, 0, 0);
  assert.deepEqual(xy(r1), [[10, 0], [10, 20], [0, 20], [0, 0]]);
  const r2 = new Rectangle(P(5, 10), 10, 20);
  assert.deepEqual(xy(r2), xy(r1));
  const copy = new Polygon(r1);
  assert.deepEqual(xy(copy), xy(r1));
  copy.setPoint(0, P(11, 0));
  assert.equal(r1.at(0).x, 10, "a copy");
  assert.throws(() => copy.at(4), RangeError);
  copy.translate(1, 1);
  assert.equal(copy.at(0).x, 12);
  const line = new Avoid.PolyLine(3);
  line.ps = [P(0, 0), P(5, 0), P(10, 0)];
  assert.deepEqual(xy(line.simplify()), [[0, 0], [10, 0]]);
  const curved = new Polygon(3);
  curved.ps = [P(0, 0), P(10, 0), P(10, 10)];
  assert.deepEqual(curved.curvedPolyline(2).ts, curved.curvedPolyline(2, false).ts, "closed = false");
  assert.deepEqual(curved.curvedPolyline(2).ts, ["M", "L", "C", "C", "C", "L"]);
  assert.equal(curved.checkpointsOnSegment(0).length, curved.checkpointsOnSegment(0, 0).length, "indexModifier = 0");
  const box = r1.offsetBoundingBox(1);
  assert.deepEqual([box.min.x, box.min.y, box.max.x, box.max.y, box.width(), box.height(), box.length(Avoid.YDIM)], [-1, -1, 11, 21, 12, 22, 22]);
  assert.equal(r1.offsetPolygon(1).size(), 4);
  assert.deepEqual(xy(curved.boundingRectPolygon()), [[10, 0], [10, 10], [0, 10], [0, 0]]);
  assert.throws(() => new Avoid.PolygonInterface(), TypeError);
  assert.throws(() => new Rectangle(P(0, 0)), TypeError);
});

test("geometry.h free functions, defaults and scalar out-parameters", () => {
  assert.equal(Avoid.euclideanDist(P(0, 0), P(3, 4)), 5);
  assert.equal(Avoid.manhattanDist(P(0, 0), P(3, 4)), 7);
  assert.equal(Avoid.totalLength(rect(0, 0, 1, 1)), 3);
  assert.equal(Avoid.inPoly(rect(0, 0, 10, 10), P(10, 5)), true, "countBorder = true");
  assert.equal(Avoid.inPoly(rect(0, 0, 10, 10), P(10, 5), false), false);
  assert.equal(Avoid.inPolyGen(rect(0, 0, 10, 10), P(5, 5)), true);
  assert.equal(Avoid.pointOnLine(P(0, 0), P(10, 0), P(5, 0.5)), Avoid.pointOnLine(P(0, 0), P(10, 0), P(5, 0.5), 0), "tolerance = 0");
  assert.equal(Avoid.pointOnLine(P(0, 0), P(10, 0), P(5, 0)), true);
  assert.equal(Avoid.colinear(P(0, 0), P(1, 1), P(2, 2.5)), Avoid.colinear(P(0, 0), P(1, 1), P(2, 2.5), 0.0), "tolerance = 0");
  assert.equal(Avoid.colinear(P(0, 0), P(1, 1), P(2, 2)), true);
  assert.equal(Avoid.inBetween(P(0, 0), P(10, 0), P(5, 0)), true);
  assert.equal(Avoid.vecDir(P(0, 0), P(1, 0), P(1, 1)), 1);
  assert.equal(Avoid.vecDir(P(0, 0), P(1, 0), P(1, 1), 0), 1, "maybeZero = 0");
  assert.throws(() => Avoid.vecDir(P(0, 0), P(1, 0), P(1, 1), -1), /maybeZero/);
  assert.deepEqual([Avoid.projection(P(0, 0), P(5, 5), P(10, 0)).x], [5]);
  assert.equal(Avoid.segmentIntersect(P(0, 0), P(2, 2), P(0, 2), P(2, 0)), true);
  const seen = [false];
  assert.equal(Avoid.segmentShapeIntersect(P(5, 2), P(10, 2), P(5, 0), P(5, 5), seen), false);
  assert.deepEqual(seen, [true], "bool& out-parameter");
  assert.equal(Avoid.segmentShapeIntersect(P(5, 2), P(10, 2), P(5, 0), P(5, 5), seen), true);
  const x = [0];
  const y = [0];
  assert.equal(Avoid.segmentIntersectPoint(P(0, 0), P(2, 2), P(0, 2), P(2, 0), x, y), Avoid.DO_INTERSECT);
  assert.deepEqual([x[0], y[0]], [1, 1], "double* out-parameters");
  assert.equal(Avoid.rayIntersectPoint(P(0, 0), P(1, 0), P(0, 1), P(1, 1), x, y), Avoid.PARALLEL);
  assert.equal(typeof Avoid.angle(P(1, 0), P(0, 0), P(0, 1)), "number");
  assert.equal(typeof Avoid.cornerSide(P(0, 0), P(1, 0), P(1, 1), P(2, 2)), "number");
  assert.equal(typeof Avoid.inValidRegion(false, P(0, 0), P(1, 0), P(1, 1), P(2, 2)), "boolean");
  assert.equal(typeof Avoid.rotationalAngle(P(1, 1)), "number");
  assert.throws(() => Avoid.segmentShapeIntersect(P(0, 0), P(1, 1), P(0, 1), P(1, 0), false), TypeError);
});

test("Router: constructor, parameters, options, defaults", () => {
  assert.throws(() => new Router(), TypeError);
  assert.throws(() => new Router(0), /flags must include/);
  const r = new Router(Avoid.OrthogonalRouting);
  r.setRoutingParameter(Avoid.shapeBufferDistance, 12);
  assert.equal(r.routingParameter(Avoid.shapeBufferDistance), 12);
  r.setRoutingParameter(Avoid.crossingPenalty);
  const sensible = r.routingParameter(Avoid.crossingPenalty);
  r.setRoutingParameter(Avoid.crossingPenalty, Avoid.chooseSensibleParamValue);
  assert.equal(r.routingParameter(Avoid.crossingPenalty), sensible, "value = chooseSensibleParamValue");
  r.setRoutingPenalty(Avoid.segmentPenalty, 5);
  assert.equal(r.routingParameter(Avoid.segmentPenalty), 5);
  r.setRoutingPenalty(Avoid.segmentPenalty);
  const sensiblePenalty = r.routingParameter(Avoid.segmentPenalty);
  r.setRoutingPenalty(Avoid.segmentPenalty, 5);
  r.setRoutingPenalty(Avoid.segmentPenalty, Avoid.chooseSensibleParamValue);
  assert.equal(r.routingParameter(Avoid.segmentPenalty), sensiblePenalty, "penVal = chooseSensibleParamValue");
  assert.notEqual(sensiblePenalty, 5);
  assert.throws(() => r.setRoutingParameter(Avoid.lastRoutingParameterMarker, 1), /lastRoutingParameterMarker/);
  r.setRoutingOption(Avoid.nudgeOrthogonalSegmentsConnectedToShapes, true);
  assert.equal(r.routingOption(Avoid.nudgeOrthogonalSegmentsConnectedToShapes), true);
  assert.throws(() => r.routingOption(Avoid.lastRoutingOptionMarker), /lastRoutingOptionMarker/);
  assert.equal(r.validConnType(), Avoid.ConnType_Orthogonal, "select = ConnType_None");
  assert.equal(r.validConnType(Avoid.ConnType_PolyLine), Avoid.ConnType_Orthogonal);
  assert.equal(r.transactionUse(), true);
  assert.equal(r.isInCrossingPenaltyReroutingStage(), false);
  assert.equal(r.newObjectId(), 1);
  assert.equal(r.objectIdIsUnused(1), true);
  assert.match(r.printInfo(), /Visibility Graph info/);
  assert.equal(r.outputDiagramText(), r.outputDiagramText("name"));
  assert.equal(r.outputDiagram(), r.outputDiagramText());
  assert.match(r.outputInstanceToSVG(), /^<\?xml[\s\S]*<svg[\s\S]*<\/svg>\s*$/);
  assert.equal(r.outputInstanceToSVG(), r.outputInstanceToSVG("name"));
  assert.match(r.outputDiagramSVG(), /^<\?xml[\s\S]*<svg[\s\S]*<\/svg>\s*$/);
  {
    const rep = new Avoid.LineRep();
    rep.begin = new Avoid.Point(1, 2);
    rep.end = new Avoid.Point(30, 40);
    assert.notEqual(r.outputDiagramSVG("", [rep]), r.outputDiagramSVG("", []));
    assert.throws(() => r.outputDiagramSVG("", [{}]), TypeError);
  }
  r.regenerateStaticBuiltGraph();
  r.destroyOrthogonalVisGraph();
  r.setStaticGraphInvalidated(true);
  r.improveOrthogonalTopology();
  r.registerSettingsChange();
  r.processActions();
  r.delete();
  assert.throws(() => r.transactionUse(), /Router has been deleted/);
  assert.throws(() => r.delete(), /Router has been deleted/);
});

test("Router public data members", () => {
  const r = new Router(Avoid.PolyLineRouting);
  const defaults = {
    PartialTime: false,
    SimpleRouting: false,
    ClusteredRouting: true,
    IgnoreRegions: true,
    UseLeesAlgorithm: true,
    InvisibilityGrph: true,
    SelectiveReroute: true,
    PartialFeedback: false,
    RubberBandRouting: false,
  };
  for (const [name, value] of Object.entries(defaults)) {
    assert.equal(r[name], value, `upstream default of Router::${name}`);
    r[name] = !value;
    assert.equal(r[name], !value);
    r[name] = value;
  }
  assert.throws(() => {
    r.SimpleRouting = 1;
  }, TypeError);
  assert.equal(r.st_checked_edges, 0);
  r.st_checked_edges = 3;
  assert.equal(r.st_checked_edges, 3);
  const s = new ShapeRef(r, rect(0, 0, 10, 10));
  const j = new JunctionRef(r, P(50, 50));
  const c = new ConnRef(r, new ConnEnd(P(-10, 5)), new ConnEnd(j));
  const k = new ClusterRef(r, rect(-20, -20, 30, 30));
  r.processTransaction();
  assert.deepEqual(r.m_obstacles.sort((a, b) => a.id() - b.id()), [s, j]);
  assert.deepEqual(r.connRefs, [c]);
  assert.deepEqual(r.clusterRefs, [k]);
  assert.throws(() => {
    "use strict";
    r.connRefs = [];
  }, TypeError);
  // SimpleRouting: processTransaction() does nothing (upstream).
  r.SimpleRouting = true;
  r.moveShape(s, 1, 0);
  assert.equal(r.processTransaction(), false);
  r.delete();
});

test("objects: constructors with upstream's default id, ids, getters", () => {
  const r = new Router(Avoid.OrthogonalRouting);
  const s = new ShapeRef(r, rect(0, 0, 10, 10));
  assert.equal(s.id(), 1, "id = 0: newObjectId()");
  const s2 = new ShapeRef(r, rect(20, 0, 30, 10), 0);
  assert.equal(s2.id(), 2);
  const s3 = new ShapeRef(r, rect(40, 0, 50, 10), 100);
  assert.equal(s3.id(), 100);
  assert.equal(r.newObjectId(), 101);
  // upstream objectIdIsUnused() looks at the scene: a queued shape is not in it yet
  assert.equal(r.objectIdIsUnused(100), true);
  r.processTransaction();
  assert.equal(r.objectIdIsUnused(100), false);
  assert.throws(() => new ShapeRef(r, rect(0, 0, 1, 1), 100), /already used/);
  assert.equal(r.newObjectId(), 101, "a refused constructor changes nothing");
  const j = new JunctionRef(r, P(60, 60));
  assert.equal(j.id(), 101);
  const c1 = new ConnRef(r);
  const c2 = new ConnRef(r, 500);
  const c3 = new ConnRef(r, new ConnEnd(s, Avoid.CONNECTIONPIN_CENTRE), new ConnEnd(j));
  const c4 = new ConnRef(r, new ConnEnd(P(0, 0)), new ConnEnd(P(1, 1)), 600);
  assert.deepEqual([c1.id(), c2.id(), c3.id(), c4.id()], [102, 500, 501, 600]);
  assert.equal(c1.router(), r);
  assert.equal(s.router(), r);
  assert.deepEqual(xy(s.polygon()), xy(rect(0, 0, 10, 10)));
  assert.deepEqual([s.position().x, s.position().y], [5, 5]);
  assert.deepEqual([j.position().x, j.position().y], [60, 60]);
  r.setRoutingParameter(Avoid.shapeBufferDistance, 2);
  const rb = s.routingBox();
  assert.deepEqual([rb.min.x, rb.max.x], [-2, 12]);
  assert.equal(s.routingPolygon().size(), 4);
  assert.equal(j.makeRectangle(r, P(0, 0)).size(), 4);
  assert.equal(j.positionFixed(), false);
  j.setPositionFixed(true);
  assert.equal(j.positionFixed(), true);
  j.preferOrthogonalDimension(Avoid.XDIM);
  assert.equal(typeof j.recommendedPosition().x, "number");
  r.delete();
});

test("ConnEnd overloads, accessors and endpointConnEnds()", () => {
  const r = new Router(Avoid.OrthogonalRouting);
  const s = new ShapeRef(r, rect(0, 0, 10, 10));
  new ShapeConnectionPin(s, 3, Avoid.ATTACH_POS_RIGHT, Avoid.ATTACH_POS_CENTRE, true, 0, Avoid.ConnDirRight);
  const j = new JunctionRef(r, P(50, 50));
  const empty = new ConnEnd();
  assert.equal(empty.type(), Avoid.ConnEndEmpty);
  const pt = new ConnEnd(P(1, 2));
  assert.deepEqual([pt.type(), pt.directions(), pt.position().x], [Avoid.ConnEndPoint, Avoid.ConnDirAll, 1]);
  const ptd = new ConnEnd(P(1, 2), Avoid.ConnDirUp);
  assert.equal(ptd.directions(), Avoid.ConnDirUp);
  const sh = new ConnEnd(s, 3);
  assert.deepEqual([sh.type(), sh.pinClassId(), sh.shape() === s, sh.junction()], [Avoid.ConnEndShapePin, 3, true, null]);
  const jn = new ConnEnd(j);
  assert.deepEqual([jn.type(), jn.junction() === j, jn.shape(), jn.pinClassId()], [Avoid.ConnEndJunction, true, null, Avoid.CONNECTIONPIN_CENTRE]);
  assert.throws(() => new ConnEnd(s, 0), /> 0/);
  assert.throws(() => new ConnEnd(s, Avoid.CONNECTIONPIN_UNSET), /CONNECTIONPIN_UNSET/);
  assert.throws(() => new ConnEnd(s), TypeError);
  assert.throws(() => new ConnEnd(P(0, 0), 16), RangeError);
  const c = new ConnRef(r, sh, jn);
  r.processTransaction();
  const [src, dst] = c.endpointConnEnds();
  assert.equal(src.shape(), s);
  assert.equal(dst.junction(), j);
  // Upstream's copy keeps the active pin: position() is the pin's.
  assert.deepEqual([src.position().x, src.position().y], [10, 5]);
  assert.equal(src.directions(), Avoid.ConnDirRight);
  r.delete();
});

test("ConnRef: setEndpoint(s), routing type, checkpoints, fixed routes, misc", () => {
  const r = new Router(Avoid.OrthogonalRouting | Avoid.PolyLineRouting);
  const c = new ConnRef(r);
  assert.equal(c.isInitialised(), false);
  c.setEndpoints(new ConnEnd(P(0, 0)), new ConnEnd(P(100, 50)));
  c.setSourceEndpoint(new ConnEnd(P(0, 10)));
  c.setDestEndpoint(new ConnEnd(P(100, 60)));
  c.setEndpoint(1, new ConnEnd(P(0, 20)));
  assert.throws(() => c.setEndpoint(3, new ConnEnd(P(0, 0))), /VertID/);
  assert.equal(c.routingType(), Avoid.ConnType_PolyLine, "the router's first valid type");
  c.setRoutingType(Avoid.ConnType_Orthogonal);
  assert.equal(c.routingType(), Avoid.ConnType_Orthogonal);
  c.setRoutingCheckpoints([new Checkpoint(P(50, 0)), new Checkpoint(P(60, 0), Avoid.ConnDirAll, Avoid.ConnDirDown), new Checkpoint()]);
  const cps = c.routingCheckpoints();
  assert.deepEqual(
    cps.map((cp) => [cp.point.x, cp.arrivalDirections, cp.departureDirections]),
    [[50, 15, 15], [60, 15, 2], [0, 15, 15]],
  );
  c.setRoutingCheckpoints([]);
  c.setHateCrossings(true);
  assert.equal(c.doesHateCrossings(), true);
  assert.equal(r.processTransaction(), true);
  assert.equal(c.isInitialised(), true);
  assert.equal(c.needsRepaint(), true);
  const route = xy(c.displayRoute());
  assert.deepEqual([route[0], route.at(-1)], [[0, 20], [100, 60]]);
  assert.ok(route.slice(1).every(([x, y], i) => x === route[i][0] || y === route[i][1]), "orthogonal");
  assert.ok(c.route().size() >= 2);
  c.setFixedExistingRoute();
  assert.equal(c.hasFixedRoute(), true);
  c.clearFixedRoute();
  assert.equal(c.hasFixedRoute(), false);
  const fixed = new Avoid.PolyLine(2);
  fixed.ps = [P(0, 0), P(10, 0)];
  c.setFixedRoute(fixed);
  assert.deepEqual(xy(c.displayRoute()), [[0, 0], [10, 0]]);
  c.set_route(fixed);
  c.calcRouteDist();
  c.makePathInvalid();
  assert.deepEqual(c.possibleDstPinPoints(), []);
  c.makeInactive();
  assert.throws(() => c.makeInactive(), /not active/);
  c.makeActive();
  assert.throws(() => c.makeActive(), /already active/);
  c.removeFromGraph();
  r.delete();
});

test("ConnRef: hasValidRoute, hasCrossingObstacles (from the libavoid-js fork)", () => {
  // Two scenes whose expected values come from the fork's C++
  // (tools/oracle/fork_route_status.cpp, with the same objects and ids).
  const status = (c) => [c.hasValidRoute(), c.hasCrossingObstacles()];
  let r = new Router(Avoid.OrthogonalRouting);
  new ShapeRef(r, rect(100, 0, 160, 60), 1);
  const around = new ConnRef(r, new ConnEnd(P(0, 30)), new ConnEnd(P(260, 30)), 10);
  const fromInside = new ConnRef(r, new ConnEnd(P(130, 30)), new ConnEnd(P(300, 30)), 11);
  r.processTransaction();
  assert.deepEqual(status(around), [true, false]);
  assert.equal(around.route().size(), 7);
  assert.deepEqual(status(fromInside), [true, true]);
  r.deleteConnector(around);
  assert.throws(() => around.hasValidRoute(), /deleted/);
  r.delete();

  // A pin on the right side that only lets connectors leave to the left,
  // into the shape: no path, so the route falls back to a straight line.
  r = new Router(Avoid.OrthogonalRouting);
  const shape = new ShapeRef(r, rect(100, 0, 160, 60), 1);
  new ShapeConnectionPin(shape, 1, Avoid.ATTACH_POS_RIGHT, Avoid.ATTACH_POS_CENTRE, true, 0, Avoid.ConnDirLeft);
  const blocked = new ConnRef(r, new ConnEnd(shape, 1), new ConnEnd(P(300, 30)), 10);
  r.processTransaction();
  assert.deepEqual(status(blocked), [false, false]);
  assert.equal(blocked.route().size(), 2);
  r.delete();
});

test("ShapeConnectionPin overloads, defaults, accessors, operators, delete()", () => {
  const r = new Router(Avoid.OrthogonalRouting);
  const s = new ShapeRef(r, rect(0, 0, 10, 20));
  const a = new ShapeConnectionPin(s, 1, Avoid.ATTACH_POS_LEFT, Avoid.ATTACH_POS_TOP, true, 0, Avoid.ConnDirNone);
  const b = new ShapeConnectionPin(s, 2, 1, 0.25, 0, Avoid.ConnDirRight); // six-argument overload (proportional offsets)
  const j = new JunctionRef(r, P(50, 50));
  const jp = new ShapeConnectionPin(j, 7);
  assert.equal(jp.directions(), new ShapeConnectionPin(j, 8, Avoid.ConnDirNone).directions(), "visDirs = ConnDirNone");
  assert.deepEqual([a.position().x, a.position().y], [0, 0]);
  assert.deepEqual([b.position().x, b.position().y], [10, 5]);
  assert.deepEqual([a.position(rect(100, 100, 110, 110)).x], [100], "position(newPoly)");
  assert.deepEqual([a.position(new Polygon()).x], [0], "newPoly = Polygon()");
  assert.deepEqual(a.ids(), [s.id(), 1]);
  assert.equal(b.directions(), Avoid.ConnDirRight);
  a.setExclusive(false);
  assert.equal(a.isExclusive(), false);
  a.setConnectionCost(3);
  assert.throws(() => a.setConnectionCost(-1), /cost >= 0/);
  assert.equal(a.equal(a), true);
  assert.equal(a.lessThan(b), true);
  assert.throws(() => new ShapeConnectionPin(s, 0, 0, 0, true, 0, 0), /classId must be > 0/);
  assert.throws(() => new ShapeConnectionPin(s, 1, 0, 0, true, 0), TypeError);
  assert.throws(() => new ShapeConnectionPin(P(0, 0), 1), TypeError);
  b.delete();
  assert.throws(() => b.ids(), /ShapeConnectionPin has been deleted/);
  r.delete();
});

test("Router overloads: moveShape and moveJunction", () => {
  const r = new Router(Avoid.OrthogonalRouting);
  const s = new ShapeRef(r, rect(0, 0, 10, 10));
  const j = new JunctionRef(r, P(50, 50));
  r.processTransaction();
  r.moveShape(s, 5, 0);
  r.processTransaction();
  assert.equal(s.position().x, 10);
  r.moveShape(s, rect(0, 0, 20, 20));
  r.processTransaction();
  assert.equal(s.position().x, 10);
  r.moveShape(s, rect(0, 0, 30, 30), false);
  r.processTransaction();
  assert.equal(s.position().x, 15);
  r.moveJunction(j, P(70, 70));
  r.processTransaction();
  assert.equal(j.position().x, 70);
  r.moveJunction(j, 1, 2);
  r.processTransaction();
  assert.deepEqual([j.position().x, j.position().y], [71, 72]);
  assert.throws(() => r.moveShape(s, 1), TypeError);
  assert.throws(() => r.moveShape(j, 1, 1), TypeError);
  r.delete();
});

test("callbacks: upstream order, ptr, before processTransaction() returns, reroute after a move", () => {
  const r = new Router(Avoid.OrthogonalRouting);
  const s = new ShapeRef(r, rect(40, -20, 60, 20));
  const log = [];
  const conns = [];
  for (let i = 0; i < 3; i++) {
    const c = new ConnRef(r, new ConnEnd(P(0, 10 * i)), new ConnEnd(P(100, 10 * i)));
    const ptr = { tag: `c${i}` };
    c.setCallback((p) => {
      log.push(p.tag);
      // a callback may use the router
      assert.equal(r.connRefs.length, 3);
    }, ptr);
    conns.push(c);
  }
  r.processTransaction();
  // upstream calls back in connRefs order (most recently added first)
  assert.deepEqual(log, r.connRefs.map((c) => `c${conns.indexOf(c)}`));
  log.length = 0;
  assert.equal(r.processTransaction(), false);
  assert.deepEqual(log, [], "nothing changed, no callback");
  r.moveShape(s, 0, 100);
  r.processTransaction();
  assert.deepEqual(log.sort(), ["c0", "c1", "c2"], "rerouted after the move");
  log.length = 0;
  conns[0].setCallback(null, null);
  r.moveShape(s, 0, -100);
  r.processTransaction();
  assert.deepEqual(log.sort(), ["c1", "c2"]);
  // With transactions off every change runs a transaction and calls back.
  log.length = 0;
  r.setTransactionUse(false);
  r.moveShape(s, 0, 100);
  assert.deepEqual(log.sort(), ["c1", "c2"]);
  // An exception thrown by a callback propagates out of the call.
  conns[1].setCallback(() => {
    throw new Error("from callback");
  }, null);
  assert.throws(() => r.moveShape(s, 0, -100), /from callback/);
  assert.equal(Avoid.internalError(), null);
  r.delete();
});

test("virtual newObjectId(): an override supplies ids", () => {
  class IdRouter extends Router {
    constructor(flags) {
      super(flags);
      this.next = 1000;
    }
    newObjectId() {
      return this.next++;
    }
  }
  const r = new IdRouter(Avoid.OrthogonalRouting);
  const s = new ShapeRef(r, rect(0, 0, 1, 1));
  const c = new ConnRef(r);
  const k = new ClusterRef(r, rect(0, 0, 5, 5));
  const j = new JunctionRef(r, P(9, 9), 7);
  assert.deepEqual([s.id(), c.id(), k.id(), j.id()], [1000, 1001, 1002, 7]);
  r.delete();
});

test("object identity and invalidation, including objects the router creates and deletes", () => {
  const r = new Router(Avoid.OrthogonalRouting);
  const j = new JunctionRef(r, P(150, 150));
  const shapes = [];
  const conns = [];
  for (const [x, y] of [[0, 0], [300, 20], [40, 260], [320, 280]]) {
    const s = new ShapeRef(r, rect(x, y, x + 60, y + 40));
    new ShapeConnectionPin(s, 1, Avoid.ATTACH_POS_CENTRE, Avoid.ATTACH_POS_CENTRE, true, 0, Avoid.ConnDirNone);
    shapes.push(s);
    conns.push(new ConnRef(r, new ConnEnd(s, 1), new ConnEnd(j)));
  }
  r.processTransaction();
  assert.equal(shapes[0].attachedConnectors()[0], conns[0]);
  assert.equal(r.shapeContainingPoint(P(10, 10)), shapes[0]);
  assert.equal(r.shapeContainingPoint(P(-10, -10)), null);
  assert.deepEqual(new Set(j.attachedConnectors()), new Set(conns));
  const rerouter = r.hyperedgeRerouter();
  assert.equal(r.hyperedgeRerouter(), rerouter, "the same HyperedgeRerouter every time");
  assert.equal(rerouter.registerHyperedgeForRerouting(j), 0);
  assert.equal(rerouter.count(), 1);
  r.processTransaction();
  assert.equal(rerouter.count(), 0);
  const lists = rerouter.newAndDeletedObjectLists(0);
  assert.ok(lists instanceof Avoid.HyperedgeNewAndDeletedObjectLists);
  // The router deleted the old hyperedge: the user's own wrappers are reported and invalid.
  assert.deepEqual(new Set(lists.deletedConnectorList), new Set(conns));
  assert.deepEqual(lists.deletedJunctionList, [j]);
  assert.throws(() => conns[0].displayRoute(), /deleted/);
  // Upstream deletes the junction with Router::deleteJunction(): with
  // transactions it is freed by the next transaction.
  assert.equal(j.position().x, 150);
  // The objects it created have wrappers of the right class, identical everywhere.
  assert.ok(lists.newConnectorList.length >= 4 && lists.newConnectorList.every((c) => c instanceof ConnRef));
  assert.ok(lists.newJunctionList.every((x) => x instanceof JunctionRef));
  assert.deepEqual(new Set(r.connRefs), new Set(lists.newConnectorList));
  const again = rerouter.newAndDeletedObjectLists(0);
  assert.notEqual(again, lists, "the struct is a value");
  assert.deepEqual(again.newConnectorList, lists.newConnectorList, "with the same wrappers");
  for (const c of lists.newConnectorList) {
    const [a, b] = c.endpointConnEnds();
    for (const end of [a, b]) {
      const owner = end.shape() ?? end.junction();
      assert.ok(shapes.includes(owner) || lists.newJunctionList.includes(owner));
    }
    assert.ok(c.displayRoute() instanceof Avoid.PolyLine);
  }
  // splitAtSegment and removeJunctionAndMergeConnectors return wrappers too.
  const c0 = lists.newConnectorList[0];
  const n = c0.displayRoute().size();
  const [nj, nc] = c0.splitAtSegment(1);
  if (n > 1) {
    assert.ok(nj instanceof JunctionRef && nc instanceof ConnRef);
    r.processTransaction();
    assert.ok(r.connRefs.includes(nc));
    const merged = nj.removeJunctionAndMergeConnectors();
    assert.ok(merged === c0 || merged === nc);
  }
  assert.deepEqual(c0.splitAtSegment(0), [null, null], "splitAtSegment(0): the core's no-op");
  r.processTransaction();
  assert.throws(() => j.position(), /deleted/);
  const improver = r.newAndDeletedObjectListsFromHyperedgeImprovement();
  assert.ok(Array.isArray(improver.newJunctionList));
  // A detached HyperedgeRerouter only records registrations.
  const detached = new Avoid.HyperedgeRerouter();
  assert.equal(detached.registerHyperedgeForRerouting([new ConnEnd(P(0, 0))]), 0);
  assert.equal(detached.count(), 1);
  assert.deepEqual(detached.newAndDeletedObjectLists(0).newConnectorList, []);
  r.delete();
  assert.throws(() => c0.id(), /Router has been deleted/);
});

test("attachedShapes / attachedConns: container out-parameters are appended to", () => {
  const r = new Router(Avoid.OrthogonalRouting);
  const a = new ShapeRef(r, rect(0, 0, 10, 10));
  const b = new ShapeRef(r, rect(50, 0, 60, 10));
  new ShapeConnectionPin(a, 1, 0.5, 0.5, true, 0, 0);
  new ShapeConnectionPin(b, 1, 0.5, 0.5, true, 0, 0);
  const c = new ConnRef(r, new ConnEnd(a, 1), new ConnEnd(b, 1));
  r.processTransaction();
  const shapes = [-1];
  r.attachedShapes(shapes, a.id(), Avoid.runningFrom);
  assert.deepEqual(shapes, [-1, b.id()]);
  const conns = [];
  r.attachedConns(conns, b.id(), Avoid.runningTo);
  assert.deepEqual(conns, [c.id()]);
  assert.throws(() => r.attachedConns(null, b.id(), Avoid.runningTo), TypeError);
  r.delete();
});

test("clusters and ReferencingPolygon", () => {
  const r = new Router(Avoid.OrthogonalRouting);
  const s = new ShapeRef(r, rect(0, 0, 10, 10));
  r.processTransaction(); // upstream: a queued shape cannot be referenced yet
  const poly = rect(-5, -5, 20, 20);
  poly.ps[0].id = s.id();
  poly.ps[0].vn = 2;
  const k = new ClusterRef(r, poly);
  const rp = k.polygon();
  assert.ok(rp instanceof Avoid.ReferencingPolygon);
  assert.deepEqual([rp.at(0).x, rp.at(0).y], [0, 10], "point 2 of the shape");
  assert.deepEqual([rp.at(1).x, rp.at(1).y], [20, 20]);
  const direct = new Avoid.ReferencingPolygon(poly, r);
  assert.deepEqual([direct.size(), direct.id(), direct.at(0).x], [4, 0, 0]);
  assert.equal(new Avoid.ReferencingPolygon().size(), 0);
  assert.equal(k.rectangularPolygon().size(), 4);
  assert.equal(k.router(), r);
  k.setNewPoly(rect(-6, -6, 21, 21));
  assert.equal(k.polygon().at(0).x, 21);
  k.makeInactive();
  assert.deepEqual(r.clusterRefs, []);
  assert.throws(() => k.makeInactive(), /not active/);
  k.makeActive();
  const bad = rect(0, 0, 1, 1);
  bad.ps[0].id = 999;
  assert.throws(() => new ClusterRef(r, bad), /names no obstacle/);
  assert.throws(() => new Avoid.ReferencingPolygon(bad, r), /names no obstacle/);
  r.deleteCluster(k);
  assert.throws(() => k.id(), /ClusterRef has been deleted/);
  r.delete();
});

test("misuse throws TypeError / Error and changes nothing", () => {
  const r = new Router(Avoid.OrthogonalRouting);
  const other = new Router(Avoid.OrthogonalRouting);
  const s = new ShapeRef(r, rect(0, 0, 10, 10));
  const foreign = new ShapeRef(other, rect(0, 0, 10, 10));
  const c = new ConnRef(r, new ConnEnd(P(-10, 5)), new ConnEnd(P(30, 5)));
  r.processTransaction();
  const before = r.outputDiagramText();
  const cases = [
    [() => new ShapeRef(r, [P(0, 0)]), TypeError],
    [() => new ShapeRef({}, rect(0, 0, 1, 1)), TypeError],
    [() => new ShapeRef(r, rect(0, 0, 1, 1), -1), TypeError],
    [() => new ShapeRef(r, rect(0, 0, 1, 1), 1.5), TypeError],
    [() => r.deleteShape(c), TypeError],
    [() => r.deleteShape(foreign), /another Router/],
    [() => c.setDestEndpoint(new ConnEnd(foreign, 1)), /another Router/],
    [() => c.setEndpoints(new ConnEnd(P(0, 0))), TypeError],
    [() => r.setTransactionUse(1), TypeError],
    [() => r.processTransaction(1), TypeError],
    [() => c.setRoutingType(7), /ConnType/],
    [() => s.transformConnectionPinPositions(9), /ShapeTransformationType/],
    [() => s.setNewPoly(new Polygon(3)), /as many points/],
    [() => new Avoid.Obstacle(), TypeError],
    [() => Avoid.Router.prototype.processTransaction.call({}), TypeError],
    [() => new ConnEnd(P(0, 0), 1.5), TypeError],
    [() => new ShapeConnectionPin(s, 1, NaN, 0.5, true, 0, 0), /must be finite/],
    [() => new ShapeConnectionPin(s, 1, 0.5, 0.5, Infinity, 0), /must be finite/],
    [
      () => {
        const poly = rect(0, 0, 1, 1);
        poly.ps[0].id = s.id();
        poly.ps[0].vn = 9;
        return new ClusterRef(r, poly);
      },
      /poly_index < poly.size\(\)/,
    ],
  ];
  for (const [i, [fn, expected]] of cases.entries()) {
    assert.throws(fn, (error) => {
      assert.ok(!(error instanceof Avoid.AvoidInternalError), String(error));
      if (expected === TypeError) assert.ok(error instanceof TypeError, String(error));
      else assert.match(error.message, expected);
      return true;
    }, `misuse case ${i}`);
  }
  // Obstacle::setNewPoly() asserts that the obstacle is out of the
  // visibility graph (the router's own use); a poly-line scene has one.
  const pl = new Router(Avoid.PolyLineRouting);
  const ps = new ShapeRef(pl, rect(0, 0, 10, 10));
  new ShapeRef(pl, rect(30, 0, 40, 10));
  pl.processTransaction();
  assert.throws(() => ps.setNewPoly(rect(0, 0, 2, 2)), /visibility graph/);
  s.setNewPoly(rect(0, 0, 10, 10)); // orthogonal routing keeps no graph on obstacles: allowed
  pl.delete();
  assert.equal(r.outputDiagramText(), before);
  assert.equal(r.processTransaction(), false);
  assert.equal(Avoid.internalError(), null);
  r.deleteShape(s);
  assert.throws(() => s.position(), /ShapeRef has been deleted/);
  assert.throws(() => r.deleteShape(s), /ShapeRef has been deleted/);
  assert.throws(() => r.moveShape(s, 1, 1), /ShapeRef has been deleted/);
  r.processTransaction();
  r.delete();
  other.delete();
});

test("transformConnectionPinPositions, markAllObstaclesAsMoved, diagnostics", () => {
  const r = new Router(Avoid.OrthogonalRouting);
  const s = new ShapeRef(r, rect(0, 0, 10, 20));
  const pin = new ShapeConnectionPin(s, 1, Avoid.ATTACH_POS_LEFT, Avoid.ATTACH_POS_CENTRE, true, 0, Avoid.ConnDirLeft);
  s.transformConnectionPinPositions(Avoid.TransformationType_FlipX);
  assert.equal(pin.position().x, 10);
  r.markAllObstaclesAsMoved();
  r.processTransaction();
  assert.equal(typeof r.existsCrossings(), "number");
  assert.equal(r.existsCrossings(), r.existsCrossings(false), "optimisedForConnectorType = false");
  assert.equal(r.existsOrthogonalSegmentOverlap(), r.existsOrthogonalSegmentOverlap(false), "atEnds = false");
  assert.equal(r.existsOrthogonalFixedSegmentOverlap(), r.existsOrthogonalFixedSegmentOverlap(false));
  assert.equal(typeof r.existsOrthogonalTouchingPaths(), "boolean");
  assert.equal(typeof r.existsInvalidOrthogonalPaths(), "boolean");
  r.markPolylineConnectorsNeedingReroutingForDeletedObstacle(s);
  r.performContinuationCheck(Avoid.TransactionPhaseRouteSearch, 1, 2);
  r.delete();
});

test("delete() of value types does nothing", () => {
  for (const v of [P(0, 0), new Avoid.Box(), new Avoid.Edge(), new Avoid.LineRep(), new Polygon(), new ConnEnd(), new Checkpoint(), new Avoid.HyperedgeNewAndDeletedObjectLists()]) {
    v.delete();
  }
});

console.log(`api conformance passed (${count} tests; ${total} upstream items: ${exposed} exposed, ${total - exposed} not exposable)`);
