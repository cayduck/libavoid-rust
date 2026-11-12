"use strict";

// Golden-file helpers shared by the Node tests (node-smoke.cjs,
// failure-containment.cjs, esm-entry.mjs).

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

/** Diagram text and every display route must equal the golden; returns the number of routes compared. */
function assertGolden(router, name) {
  const expected = golden(name);
  assert.equal(router.outputDiagramText(true), expected, `diagram ${name}`);
  const byNumeric = new Map(router.connectorIds().map((id) => [router.objectNumericId(id), id]));
  let compared = 0;
  for (const [nid, pts] of goldenPaths(expected)) {
    const route = router.displayConnectorRoute(byNumeric.get(nid));
    assert.deepEqual(
      route.map((p) => [p.x, p.y]),
      pts,
      `${name}: route of connector ${nid}`,
    );
    compared++;
  }
  return compared;
}

/**
 * Upstream fixture connectionpin01 (tests/connectionpin01.cpp) through the
 * given API object (index.cjs or index.mjs exports), checked against its
 * goldens; returns the number of routes compared.
 */
function connectionPin01(api) {
  const { AvoidRouter, ConnDir, RouterFlag } = api;
  const router = new AvoidRouter(RouterFlag.OrthogonalRouting);
  let compared = 0;
  router.addShape("shapeRef1", 0, 0, 10, 10, 0);
  router.addShape("shapeRef2", 40, 20, 50, 30, 0);
  assert.equal(router.addShapeConnectionPin("shapeRef1", "p", 1, 0.5, 0.5, true, 0, ConnDir.None), true);
  assert.equal(router.addShapeConnectionPin("shapeRef2", "p", 1, 0.5, 0.5, true, 0, ConnDir.None), true);
  router.addConnectorWithConnEnds("conn", { shapeId: "shapeRef2", classId: 1 }, { shapeId: "shapeRef1", classId: 1 }, 0);
  router.processTransaction();
  compared += assertGolden(router, "connectionpin01-1");
  router.moveShapeBy("shapeRef2", 5, 0);
  router.processTransaction();
  compared += assertGolden(router, "connectionpin01-2");
  router.moveShapeBy("shapeRef1", 0, -10);
  router.processTransaction();
  compared += assertGolden(router, "connectionpin01-3");
  compared += assertGolden(router, "connectionpin01-final");
  router.free();
  return compared;
}

module.exports = { assertGolden, connectionPin01, golden, goldenPaths, root };
