// Random scene generator for the libavoid-rust / libavoid-js comparison.
// Usage: node gen.mjs <outdir> <seed> <count> [size]
// Uses only operations that every compared build exposes: rectangles, shape
// pins, point and pin connector ends, moves, deletions, routing parameters
// and options. All coordinates are integers.
import fs from "node:fs";
const [outdir, seed0, count, sizeArg] = process.argv.slice(2);
function rng(seed) { let s = seed >>> 0; return () => { s = (s + 0x6D2B79F5) >>> 0; let t = s; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; }; }
for (let k = 0; k < +count; k++) {
  const seed = +seed0 + k; const r = rng(seed * 7919 + 13);
  const ri = (a, b) => a + Math.floor(r() * (b - a + 1));
  const pick = (xs) => xs[Math.floor(r() * xs.length)];
  const size = sizeArg ? +sizeArg : ri(3, 25);
  const L = [];
  const orth = r() < 0.7;
  L.push(`router ${orth ? "orth" : "poly"}`);
  // RoutingParameter: 0 segment 1 angle 2 crossing 3 clusterCrossing 4 fixedSharedPath 5 portDirection 6 shapeBuffer 7 idealNudging 8 reverseDirection
  L.push(`param 6 ${ri(0, 12)}`);
  L.push(`param 7 ${ri(2, 10)}`);
  if (r() < 0.3) L.push(`param 0 ${ri(0, 50)}`);
  if (r() < 0.25) L.push(`param 2 ${ri(0, 200)}`);
  if (r() < 0.15) L.push(`param 4 ${ri(0, 100)}`);
  if (r() < 0.2) L.push(`param 1 ${ri(0, 100)}`);
  // RoutingOption: 0 nudgeOrthSegsConnectedToShapes 2 penaliseOrthSharedPathsAtConnEnds 3 nudgeOrthTouchingColinear 4 performUnifying 6 nudgeSharedPathsWithCommonEndPoint
  for (const o of [0, 2, 3, 4, 6]) if (r() < 0.3) L.push(`option ${o} ${r() < 0.5 ? 1 : 0}`);
  const cols = Math.ceil(Math.sqrt(size * 1.6));
  const cells = []; for (let i = 0; i < cols * cols; i++) cells.push(i);
  const shapes = []; let nextId = 1;
  const pins = new Map();
  for (let i = 0; i < size; i++) {
    const c = cells.splice(Math.floor(r() * cells.length), 1)[0];
    const x = (c % cols) * 120 + ri(0, 40), y = Math.floor(c / cols) * 120 + ri(0, 40);
    const id = nextId++;
    L.push(`shape ${id} ${x} ${y} ${ri(20, 70)} ${ri(20, 70)}`);
    shapes.push(id);
    const ps = [];
    if (r() < 0.6) {
      const n = ri(1, 4);
      for (let p = 1; p <= n; p++) {
        const side = ri(0, 3);
        const [px, py, d] = [[0.5, 0, 1], [0.5, 1, 2], [0, 0.5, 4], [1, 0.5, 8]][side];
        const dirs = r() < 0.2 ? 15 : d;
        L.push(`pin ${id} ${p} ${px} ${py} ${dirs}`);
        ps.push(p);
      }
    }
    pins.set(id, ps);
  }
  const W = cols * 120;
  const end = () => {
    const s = pick(shapes);
    if (r() < 0.75 && shapes.length) {
      const ps = pins.get(s);
      if (ps.length && r() < 0.6) return `S ${s} ${pick(ps)}`;
    }
    return `P ${ri(-20, W)} ${ri(-20, W)}`;
  };
  const conns = [];
  const nconn = ri(Math.ceil(size / 2), size * 2);
  for (let i = 0; i < nconn; i++) { const id = nextId++; L.push(`conn ${id} ${end()} ${end()}`); conns.push(id); }
  L.push("process");
  const steps = ri(1, 6);
  for (let s = 0; s < steps; s++) {
    const nops = ri(1, 4);
    for (let o = 0; o < nops; o++) {
      const t = r();
      if (t < 0.6) L.push(`move ${pick(shapes)} ${ri(-60, 60)} ${ri(-60, 60)}`);
      else if (t < 0.75 && conns.length > 1) L.push(`delconn ${conns.splice(Math.floor(r() * conns.length), 1)[0]}`);
      else { const id = nextId++; L.push(`conn ${id} ${end()} ${end()}`); conns.push(id); }
    }
    L.push("process");
  }
  fs.writeFileSync(`${outdir}/s${String(seed).padStart(5, "0")}.txt`, L.join("\n") + "\n");
}
