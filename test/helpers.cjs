"use strict";

// Golden-file helpers shared by the Node tests (node-smoke.cjs,
// failure-containment.cjs, esm-entry.mjs, api-conformance.cjs).

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const root = path.join(__dirname, "..");

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

/**
 * Every connector's displayRoute() must equal the golden's path of the
 * same id (exactly), and every golden path must belong to a connector;
 * returns the number of routes compared.
 */
function assertGolden(router, name) {
  const expected = goldenPaths(golden(name));
  const conns = router.connRefs.filter((c) => !c.displayRoute().empty());
  assert.deepEqual(conns.map((c) => c.id()).sort((a, b) => a - b), [...expected.keys()].sort((a, b) => a - b), `${name}: connector ids`);
  for (const c of conns) {
    assert.deepEqual(
      c.displayRoute().ps.map((p) => [p.x, p.y]),
      expected.get(c.id()),
      `${name}: route of connector ${c.id()}`,
    );
  }
  return conns.length;
}

/**
 * Upstream fixture connectionpin01 (tests/connectionpin01.cpp), statement
 * by statement through the given API object (index.cjs or index.mjs
 * exports), checked against its goldens; returns the number of routes
 * compared.
 */
function connectionPin01(Avoid) {
  const router = new Avoid.Router(2);
  let compared = 0;
  const shapeRect1 = new Avoid.Rectangle(new Avoid.Point(0, 0), new Avoid.Point(10, 10));
  const shapeRef1 = new Avoid.ShapeRef(router, shapeRect1, 0);
  const shapeRect2 = new Avoid.Rectangle(new Avoid.Point(40, 20), new Avoid.Point(50, 30));
  const shapeRef2 = new Avoid.ShapeRef(router, shapeRect2, 0);
  new Avoid.ShapeConnectionPin(shapeRef1, 1, 0.5, 0.5, true, 0.0, 0);
  new Avoid.ShapeConnectionPin(shapeRef2, 1, 0.5, 0.5, true, 0.0, 0);
  const dstEnd = new Avoid.ConnEnd(shapeRef1, 1);
  const srcEnd = new Avoid.ConnEnd(shapeRef2, 1);
  new Avoid.ConnRef(router, srcEnd, dstEnd);
  router.processTransaction();
  compared += assertGolden(router, "connectionpin01-1");
  router.moveShape(shapeRef2, 5, 0);
  router.processTransaction();
  compared += assertGolden(router, "connectionpin01-2");
  router.moveShape(shapeRef1, 0, -10);
  router.processTransaction();
  compared += assertGolden(router, "connectionpin01-3");
  compared += assertGolden(router, "connectionpin01-final");
  router.delete();
  return compared;
}

module.exports = { assertGolden, connectionPin01, golden, goldenPaths, root };
