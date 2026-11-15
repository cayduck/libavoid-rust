// Scenes with shapes nested inside other shapes, for the nested_shapes
// oracle (tools/oracle/nested_shapes.sh). Same format as
// tools/libavoid-js-compare/gen.mjs: containers with children (and
// sometimes grandchildren) inside, pins on the children, connectors between
// children of one container, across containers and to free points inside
// and outside them, then moves that keep children inside, push them out, or
// move containers.
//
//   node tools/oracle/nested_shapes_scenes.mjs <outdir> <first seed> <count>
import fs from "node:fs";
import path from "node:path";

const [outdir, first, count] = process.argv.slice(2);
fs.mkdirSync(outdir, { recursive: true });

function rng(seed) {
  let s = seed >>> 0;
  return () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

for (let k = 0; k < +count; k++) {
  const seed = +first + k;
  const r = rng(seed * 6151 + 29);
  const ri = (a, b) => a + Math.floor(r() * (b - a + 1));
  const pick = (xs) => xs[Math.floor(r() * xs.length)];
  const L = [];
  L.push(`router ${r() < 0.85 ? "orth" : "poly"}`);
  L.push(`param 6 ${ri(0, 8)}`);
  L.push(`param 7 ${ri(2, 8)}`);
  if (r() < 0.3) L.push(`param 0 ${ri(1, 50)}`);
  if (r() < 0.2) L.push(`param 2 ${ri(0, 200)}`);
  for (const o of [0, 2, 3, 4, 6]) if (r() < 0.2) L.push(`option ${o} ${r() < 0.5 ? 1 : 0}`);

  let nextId = 1;
  const shapes = []; // {id, x, y, w, h, pins: [], parent}
  const addShape = (x, y, w, h, parent) => {
    const s = { id: nextId++, x, y, w, h, pins: [], parent };
    L.push(`shape ${s.id} ${x} ${y} ${w} ${h}`);
    shapes.push(s);
    return s;
  };
  const addPins = (s) => {
    const n = ri(0, 3);
    for (let p = 1; p <= n; p++) {
      const [px, py, d] = pick([[0.5, 0, 1], [0.5, 1, 2], [0, 0.5, 4], [1, 0.5, 8]]);
      L.push(`pin ${s.id} ${p} ${px} ${py} ${r() < 0.2 ? 15 : d}`);
      s.pins.push(p);
    }
  };

  const containers = [];
  const nc = ri(1, 3);
  for (let c = 0; c < nc; c++) {
    const cw = ri(220, 420), ch = ri(160, 320);
    const parent = addShape(c * 480 + ri(0, 40), ri(0, 60), cw, ch, null);
    containers.push(parent);
    // Children in a grid of cells inside the container.
    const cols = ri(2, 3), rows = ri(1, 3);
    const cellW = Math.floor((cw - 20) / cols), cellH = Math.floor((ch - 20) / rows);
    for (let i = 0; i < cols; i++) {
      for (let j = 0; j < rows; j++) {
        if (r() < 0.25) continue;
        const w = ri(20, Math.max(21, cellW - 30)), h = ri(20, Math.max(21, cellH - 30));
        const x = parent.x + 10 + i * cellW + ri(0, Math.max(0, cellW - w - 10));
        const y = parent.y + 10 + j * cellH + ri(0, Math.max(0, cellH - h - 10));
        const child = addShape(x, y, w, h, parent);
        addPins(child);
        if (w > 60 && h > 60 && r() < 0.3) {
          const g = addShape(x + 15, y + 15, ri(15, w - 30), ri(15, h - 30), child);
          addPins(g);
        }
      }
    }
  }
  const outside = addShape(nc * 480 + 20, ri(0, 200), ri(30, 80), ri(30, 80), null);
  addPins(outside);

  const inner = shapes.filter((s) => s.parent);
  const pointIn = (s) => `P ${ri(s.x + 2, s.x + s.w - 2)} ${ri(s.y + 2, s.y + s.h - 2)}`;
  const end = () => {
    const t = r();
    if (t < 0.55 && inner.length) {
      const s = pick(inner);
      return s.pins.length && r() < 0.7 ? `S ${s.id} ${pick(s.pins)}` : pointIn(s.parent);
    }
    if (t < 0.8) return pointIn(pick(containers));
    if (outside.pins.length && r() < 0.5) return `S ${outside.id} ${pick(outside.pins)}`;
    return `P ${ri(-30, nc * 480 + 150)} ${ri(-30, 420)}`;
  };
  const conns = [];
  const nconn = ri(3, 12);
  for (let i = 0; i < nconn; i++) {
    const id = nextId++;
    L.push(`conn ${id} ${end()} ${end()}`);
    conns.push(id);
  }
  L.push("process");
  const steps = ri(1, 4);
  for (let s = 0; s < steps; s++) {
    for (let o = ri(1, 3); o > 0; o--) {
      const t = r();
      if (t < 0.5 && inner.length) L.push(`move ${pick(inner).id} ${ri(-25, 25)} ${ri(-25, 25)}`);
      else if (t < 0.65) L.push(`move ${pick(shapes).id} ${ri(-120, 120)} ${ri(-120, 120)}`);
      else if (t < 0.75 && conns.length > 1) L.push(`delconn ${conns.splice(Math.floor(r() * conns.length), 1)[0]}`);
      else {
        const id = nextId++;
        L.push(`conn ${id} ${end()} ${end()}`);
        conns.push(id);
      }
    }
    L.push("process");
  }
  fs.writeFileSync(path.join(outdir, `n${String(seed).padStart(5, "0")}.txt`), L.join("\n") + "\n");
}
