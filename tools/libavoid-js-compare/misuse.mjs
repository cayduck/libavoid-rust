// Feeds inputs that upstream libavoid rejects with an assertion, then checks
// whether the same instance still routes afterwards. Usage: node misuse.mjs <impl> <case>
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
import path from "node:path";
const ROOT = path.resolve(path.dirname(new URL(import.meta.url).pathname), "../..");
const WORK = process.env.WORK || path.join(ROOT, "target/libavoid-js-compare");
const [impl, kase] = process.argv.slice(2);
let A, AvoidLib;
if (impl === "rust") A = require(path.join(ROOT, "index.cjs"));
else { ({ AvoidLib } = await import(path.join(WORK, { ljs045: "npm-0.4.5/node_modules/libavoid-js/dist", ljsbeta5: "npm-0.5.0-beta.5/node_modules/libavoid-js/dist", ljsnext: "libavoid-js/dist" }[impl], "index-node.mjs"))); await AvoidLib.load(); A = AvoidLib.getInstance(); }
const next = impl === "ljsnext";
const orth = () => next ? new A.Router(A.RouterFlag.OrthogonalRouting) : new A.Router(impl === "ljsbeta5" ? A.RouterFlag.OrthogonalRouting.value : A.OrthogonalRouting);
const rect = (x, y, w, h) => new A.Rectangle(new A.Point(x, y), new A.Point(x + w, y + h));
const addShape = (r, x, y, w, h) => next ? r.addShape({ x, y, width: w, height: h }) : new A.ShapeRef(r, rect(x, y, w, h));
const addConn = (r, a, b) => next ? r.addConnector({ x: a[0], y: a[1] }, { x: b[0], y: b[1] }) : new A.ConnRef(r, new A.ConnEnd(new A.Point(...a)), new A.ConnEnd(new A.Point(...b)));
const route = (c) => { const d = c.displayRoute(); if (Array.isArray(d)) return d.map((p) => [p.x, p.y]); if (d.get_ps) { const o = []; for (let i = 0; i < d.size(); i++) { const p = d.get_ps(i); o.push([p.x, p.y]); } return o; } if (d.ps && d.ps.map) return d.ps.map((p) => [p.x, p.y]); const ps = d.ps ?? d; const out = []; const n = ps.size(); for (let i = 0; i < n; i++) { const p = ps.get ? ps.get(i) : d.get_ps(i); out.push([p.x, p.y]); } return out; };
const tri = () => { if (next) return [{ x: 0, y: 0 }, { x: 50, y: 0 }, { x: 25, y: 40 }]; const p = new A.Polygon(3); const pts = [[0, 0], [50, 0], [25, 40]]; pts.forEach(([x, y], i) => (p.setPoint ? p.setPoint(i, new A.Point(x, y)) : (p.ps[i] = new A.Point(x, y)))); return p; };
const cases = {
  // upstream: COLA_ASSERT in Router::moveShape (point count must stay the same)
  movePointCount(r) { const s = addShape(r, 100, 0, 60, 60); r.processTransaction(); if (next) r.moveShape(s, tri()); else if (r.moveShape_poly) r.moveShape_poly(s, tri(), false); else r.moveShape(s, tri()); r.processTransaction(); },
  // upstream: COLA_ASSERT, a shape cannot be deleted in the transaction that added it (transactions on)
  deleteInSameTransaction(r) { const s = addShape(r, 100, 0, 60, 60); r.deleteShape(s); r.processTransaction(); },
  // upstream: COLA_ASSERT(classId > 0) in ShapeConnectionPin
  pinClassZero(r) { const s = addShape(r, 100, 0, 60, 60); if (next) s.addPin({ classId: 0, x: 0, y: 0.5, directions: 4 }); else new A.ShapeConnectionPin(s, 0, 0, 0.5, true, 0, 4); r.processTransaction(); },
  // use a connector after deleting it
  useAfterDelete(r) { const c = addConn(r, [0, 0], [10, 10]); r.processTransaction(); r.deleteConnector(c); r.processTransaction(); route(c); },
  // NaN coordinates
  nanPoint(r) { addConn(r, [NaN, 0], [300, 30]); addShape(r, 100, 0, 60, 60); r.processTransaction(); },
};
let res = "ok (no error)";
const r = orth();
try { cases[kase](r); } catch (e) { res = `throws ${e?.constructor?.name ?? typeof e}: ${String(e?.message ?? e).split("\n")[0].slice(0, 140)}`; }
// Does the same module still route afterwards?
let after;
try { const r2 = orth(); addShape(r2, 100, 0, 60, 60); const c = addConn(r2, [0, 30], [260, 30]); r2.processTransaction(); after = JSON.stringify(route(c)); }
catch (e) { after = `throws ${e?.constructor?.name}: ${String(e?.message ?? e).split("\n")[0].slice(0, 100)}`; }
let recovered = "";
if (impl === "rust" && A.internalError && A.internalError()) { A.reset(); try { const r3 = new A.Router(A.OrthogonalRouting); new A.ShapeRef(r3, rect(100, 0, 60, 60)); const c = addConn(r3, [0, 30], [260, 30]); r3.processTransaction(); recovered = " | after reset(): " + JSON.stringify(route(c)); } catch (e) { recovered = " | reset failed " + e.message; } }
if (impl !== "rust" && after.startsWith("throws")) { try { await AvoidLib.load(); A = AvoidLib.getInstance(); const r3 = orth(); addShape(r3, 100, 0, 60, 60); const c = addConn(r3, [0, 30], [260, 30]); r3.processTransaction(); recovered = " | after AvoidLib.load(): " + JSON.stringify(route(c)); } catch (e) { recovered = " | after AvoidLib.load(): throws " + String(e.message).slice(0, 80); } }
console.log(`${impl} ${kase}: ${res}\n    next router on same module: ${after}${recovered}`);
