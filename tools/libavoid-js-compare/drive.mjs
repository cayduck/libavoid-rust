// Replays comparison scenes through one JS build and prints routes in the
// same format as driver.cpp.  Usage: node drive.mjs <impl> <scene>...
// impl: rust (this package, built) | ljs045 | ljsbeta5 | ljsnext (installed by run.sh)
// OUT=<dir> writes <scene>.out files; REPS=<n> times n runs (median, min).
import fs from "node:fs";
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
import path from "node:path";
const ROOT = path.resolve(path.dirname(new URL(import.meta.url).pathname), "../..");
const WORK = process.env.WORK || path.join(ROOT, "target/libavoid-js-compare");
const [impl, ...files] = process.argv.slice(2);
const PARAMS = ["segmentPenalty", "anglePenalty", "crossingPenalty", "clusterCrossingPenalty", "fixedSharedPathPenalty", "portDirectionPenalty", "shapeBufferDistance", "idealNudgingDistance", "reverseDirectionPenalty"];
const OPTIONS = ["nudgeOrthogonalSegmentsConnectedToShapes", "improveHyperedgeRoutesMovingJunctions", "penaliseOrthogonalSharedPathsAtConnEnds", "nudgeOrthogonalTouchingColinearSegments", "performUnifyingNudgingPreprocessingStep", "improveHyperedgeRoutesMovingAddingAndDeletingJunctions", "nudgeSharedPathsWithCommonEndPoint"];

export function ljsEntry(impl) {
  const p = { ljs045: "npm-0.4.5/node_modules/libavoid-js/dist", ljsbeta5: "npm-0.5.0-beta.5/node_modules/libavoid-js/dist", ljsnext: "libavoid-js/dist" }[impl];
  if (!p) throw new Error(`unknown implementation ${impl}`);
  return path.join(WORK, p, "index-node.mjs");
}
async function load() {
  if (impl === "rust") return require(path.join(ROOT, "index.cjs"));
  const { AvoidLib } = await import(ljsEntry(impl));
  await AvoidLib.load();
  return AvoidLib.getInstance();
}
const A = await load();

// Each adapter: router(orth), param(p,v), option(o,v), shape(id,x,y,w,h), pin(shape,c,px,py,d),
// conn(id,a,b) where an end is {x,y} or {shape,pin}, move(shape,dx,dy), delconn(c), process(), route(c) -> [[x,y]...]
const adapters = {
  rust: {
    router: (o) => new A.Router(o ? A.OrthogonalRouting : A.PolyLineRouting),
    param: (r, p, v) => r.setRoutingParameter(A[PARAMS[p]], v),
    option: (r, o, v) => r.setRoutingOption(A[OPTIONS[o]], v),
    shape: (r, id, x, y, w, h) => new A.ShapeRef(r, new A.Rectangle(new A.Point(x, y), new A.Point(x + w, y + h)), id),
    pin: (s, c, px, py, d) => new A.ShapeConnectionPin(s, c, px, py, true, 0, d),
    end: (e) => e.shape ? new A.ConnEnd(e.shape, e.pin) : new A.ConnEnd(new A.Point(e.x, e.y)),
    conn(r, id, a, b) { return new A.ConnRef(r, this.end(a), this.end(b), id); },
    move: (r, s, dx, dy) => r.moveShape(s, dx, dy),
    delconn: (r, c) => r.deleteConnector(c),
    process: (r) => r.processTransaction(),
    route: (c) => c.displayRoute().ps.map((p) => [p.x, p.y]),
    free: (r) => r.delete(),
  },
  ljs045: {
    router: (o) => new A.Router(o ? A.OrthogonalRouting : A.PolyLineRouting),
    param: (r, p, v) => r.setRoutingParameter(A[PARAMS[p]], v),
    option: (r, o, v) => r.setRoutingOption(A[OPTIONS[o]], v),
    shape: (r, id, x, y, w, h) => new A.ShapeRef(r, new A.Rectangle(new A.Point(x, y), new A.Point(x + w, y + h)), id),
    pin: (s, c, px, py, d) => new A.ShapeConnectionPin(s, c, px, py, true, 0, d),
    end: (e) => e.shape ? new A.ConnEnd(e.shape, e.pin) : new A.ConnEnd(new A.Point(e.x, e.y)),
    conn(r, id, a, b) { return new A.ConnRef(r, this.end(a), this.end(b), id); },
    move: (r, s, dx, dy) => r.moveShape(s, dx, dy),
    delconn: (r, c) => r.deleteConnector(c),
    process: (r) => r.processTransaction(),
    route(c) { const pl = c.displayRoute(); const out = []; for (let i = 0; i < pl.size(); i++) { const p = pl.get_ps(i); out.push([p.x, p.y]); } return out; },
    free: (r) => A.destroy(r),
  },
  ljsbeta5: {
    router: (o) => new A.Router((o ? A.RouterFlag.OrthogonalRouting : A.RouterFlag.PolyLineRouting).value),
    param: (r, p, v) => r.setRoutingParameter(A.RoutingParameter[PARAMS[p]], v),
    option: (r, o, v) => r.setRoutingOption(A.RoutingOption[OPTIONS[o]], v),
    shape: (r, id, x, y, w, h) => new A.ShapeRef(r, new A.Rectangle(new A.Point(x, y), new A.Point(x + w, y + h)), id),
    pin: (s, c, px, py, d) => new A.ShapeConnectionPin(s, c, px, py, true, 0, d),
    end: (e) => e.shape ? new A.ConnEnd(e.shape, e.pin) : new A.ConnEnd(new A.Point(e.x, e.y)),
    conn(r, id, a, b) { return new A.ConnRef(r, this.end(a), this.end(b), id); },
    move: (r, s, dx, dy) => r.moveShape_delta(s, dx, dy),
    delconn: (r, c) => r.deleteConnector(c),
    process: (r) => r.processTransaction(),
    route(c) { const pl = c.displayRoute(); const ps = pl.ps; const out = []; for (let i = 0; i < ps.size(); i++) { const p = ps.get(i); out.push([p.x, p.y]); } return out; },
    free: (r) => r.delete(),
  },
  ljsnext: {
    router: (o) => new A.Router(o ? A.RouterFlag.OrthogonalRouting : A.RouterFlag.PolyLineRouting),
    param: (r, p, v) => r.setRoutingParameter(A.RoutingParameter[PARAMS[p]], v),
    option: (r, o, v) => r.setRoutingOption(A.RoutingOption[OPTIONS[o]], v),
    shape: (r, id, x, y, w, h) => r.addShape({ x, y, width: w, height: h }, { id }),
    pin: (s, c, px, py, d) => s.addPin({ classId: c, x: px, y: py, directions: d }),
    end: (e) => e.shape ? { shape: e.shape, pin: e.pin } : { x: e.x, y: e.y },
    conn(r, id, a, b) { return r.addConnector(this.end(a), this.end(b), { id }); },
    move: (r, s, dx, dy) => r.moveShape(s, dx, dy),
    delconn: (r, c) => r.deleteConnector(c),
    process: (r) => r.processTransaction(),
    route: (c) => c.displayRoute().map((p) => [p.x, p.y]),
    free: (r) => r.delete(),
  },
};
const ad = adapters[impl];

function run(file) {
  const lines = fs.readFileSync(file, "utf8").split("\n").filter(Boolean);
  let r; const shapes = new Map(), conns = new Map(); const out = []; let t = 0, ms = 0;
  const end = (w, i) => w[i] === "P" ? [{ x: +w[i + 1], y: +w[i + 2] }, i + 3] : [{ shape: shapes.get(+w[i + 1]), pin: +w[i + 2] }, i + 3];
  for (const line of lines) {
    const w = line.split(" ");
    switch (w[0]) {
      case "router": r = ad.router(w[1] === "orth"); break;
      case "param": ad.param(r, +w[1], +w[2]); break;
      case "option": ad.option(r, +w[1], w[2] === "1"); break;
      case "shape": shapes.set(+w[1], ad.shape(r, +w[1], +w[2], +w[3], +w[4], +w[5])); break;
      case "pin": ad.pin(shapes.get(+w[1]), +w[2], +w[3], +w[4], +w[5]); break;
      case "conn": { const [a, i] = end(w, 2); const [b] = end(w, i); conns.set(+w[1], ad.conn(r, +w[1], a, b)); break; }
      case "move": ad.move(r, shapes.get(+w[1]), +w[2], +w[3]); break;
      case "delconn": ad.delconn(r, conns.get(+w[1])); conns.delete(+w[1]); break;
      case "process": {
        const t0 = performance.now(); ad.process(r); ms += performance.now() - t0;
        out.push(`T ${t++}`);
        for (const id of [...conns.keys()].sort((a, b) => a - b)) { const pts = ad.route(conns.get(id)); out.push(`c ${id} ${pts.length} ${pts.flat().join(" ")}`); }
        break;
      }
    }
  }
  ad.free(r);
  return { out: out.join("\n") + "\n", ms };
}
const outdir = process.env.OUT;
for (const f of files) {
  const name = f.split("/").pop().replace(/\.txt$/, "");
  try {
    const reps = +(process.env.REPS || 1); const times = [];
    let res; for (let k = 0; k < reps; k++) { res = run(f); times.push(res.ms); }
    const { out } = res; times.sort((a, b) => a - b);
    if (outdir) fs.writeFileSync(`${outdir}/${name}.out`, out); else if (!process.env.REPS) process.stdout.write(out);
    console.error(`${name} ok ${times[Math.floor(times.length / 2)].toFixed(3)}${reps > 1 ? ` min ${times[0].toFixed(3)} reps ${reps}` : ""}`);
  } catch (e) {
    console.error(`${name} error ${String(e && e.message || e).split("\n")[0].slice(0, 200)}`);
    if (outdir) fs.writeFileSync(`${outdir}/${name}.out`, "ERROR\n");
    if (process.env.STOP_ON_ERROR) break;
  }
}
