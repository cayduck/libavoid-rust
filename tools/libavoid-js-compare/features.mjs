// Which upstream features each build exposes at runtime (not from its typings).
// Usage: node features.mjs <impl>
import path from "node:path";
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const ROOT = path.resolve(path.dirname(new URL(import.meta.url).pathname), "../..");
const WORK = process.env.WORK || path.join(ROOT, "target/libavoid-js-compare");
const impl = process.argv[2];
let A;
if (impl === "rust") A = require(path.join(ROOT, "index.cjs"));
else { const { AvoidLib } = await import(path.join(WORK, { ljs045: "npm-0.4.5/node_modules/libavoid-js/dist", ljsbeta5: "npm-0.5.0-beta.5/node_modules/libavoid-js/dist", ljsnext: "libavoid-js/dist" }[impl], "index-node.mjs")); await AvoidLib.load(); A = AvoidLib.getInstance(); }
const has = (cls, m) => typeof A[cls] === "function" && typeof A[cls].prototype[m] === "function";
const f = {
  "junctions": typeof A.JunctionRef === "function" || has("Router", "addJunction"),
  "routing checkpoints": has("ConnRef", "setRoutingCheckpoints"),
  "clusters (ClusterRef)": typeof A.ClusterRef === "function",
  "hyperedge rerouting (Router.hyperedgeRerouter)": has("Router", "hyperedgeRerouter"),
  "transaction progress / stop (shouldContinueTransactionWithProgress)": has("Router", "shouldContinueTransactionWithProgress"),
  "fixed routes (ConnRef.setFixedRoute)": has("ConnRef", "setFixedRoute"),
  "shape pin transforms (ShapeRef.transformConnectionPinPositions)": has("ShapeRef", "transformConnectionPinPositions"),
  "SVG debug output (Router.outputInstanceToSVG)": has("Router", "outputInstanceToSVG"),
  "routing inside parent shapes (always on in libavoid-js; Router.setRouteInsideParentShapes here)":
    impl !== "rust" || has("Router", "setRouteInsideParentShapes"),
  "rerouting after edits (always on in libavoid-js; Router.setRerouteAfterEdits here)":
    impl !== "rust" || has("Router", "setRerouteAfterEdits"),
  "route status (ConnRef.hasValidRoute, hasCrossingObstacles, from the fork)": has("ConnRef", "hasValidRoute") && has("ConnRef", "hasCrossingObstacles"),
};
for (const [k, v] of Object.entries(f)) console.log(`${impl}\t${v ? "yes" : "no"}\t${k}`);
