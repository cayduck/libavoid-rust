// Scenes for the reroute_after_edits oracle: those of
// tools/libavoid-js-compare/gen.mjs, with edits before later transactions:
// in odd-numbered scenes, checkpoints set (or cleared) on connectors that
// have already been routed; in even-numbered ones, extra connector
// deletions.  Every fourth scene is replaced by one built for the deletion
// case: more connectors than free pins on a few shapes (the rest fall back
// to straight lines), then deletions that free pins.  The two are kept apart because rerouting for a checkpoint
// change also repairs most of what rerouting after a deletion would, which
// would leave the second untested.
//
//   node tools/oracle/reroute_after_edits_scenes.mjs <outdir> <first seed> <count>
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

const [outdir, first, count] = process.argv.slice(2);
const here = path.dirname(new URL(import.meta.url).pathname);
fs.mkdirSync(outdir, { recursive: true });
execFileSync("node", [path.join(here, "../libavoid-js-compare/gen.mjs"), outdir, first, count]);

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

function contentionScene(r) {
  const ri = (a, b) => a + Math.floor(r() * (b - a + 1));
  const L = [`router ${r() < 0.8 ? "orth" : "poly"}`, `param 6 ${ri(0, 8)}`, `param 0 ${ri(1, 50)}`];
  let nextId = 1;
  const holders = [];
  for (let k = ri(1, 3); k > 0; k--) {
    const id = nextId++;
    L.push(`shape ${id} ${ri(0, 500)} ${ri(0, 500)} ${ri(30, 80)} ${ri(30, 80)}`);
    const n = ri(1, 2);
    for (let p = 0; p < n; p++) {
      const [px, py, d] = [[0.5, 0, 1], [0.5, 1, 2], [0, 0.5, 4], [1, 0.5, 8]][ri(0, 3)];
      L.push(`pin ${id} 1 ${px} ${py} ${r() < 0.3 ? 15 : d}`);
    }
    for (let c = n + ri(1, 3); c > 0; c--) holders.push([id, nextId++]);
  }
  for (let k = ri(0, 4); k > 0; k--) L.push(`shape ${nextId++} ${ri(0, 500)} ${ri(0, 500)} ${ri(20, 60)} ${ri(20, 60)}`);
  for (const [shape, id] of holders) L.push(`conn ${id} S ${shape} 1 P ${ri(-50, 650)} ${ri(-50, 650)}`);
  L.push("process");
  const live = holders.map(([, id]) => id);
  for (let t = ri(1, 3); t > 0; t--) {
    for (let k = ri(1, 2); k > 0 && live.length > 1; k--) L.push(`delconn ${live.splice(ri(0, live.length - 1), 1)[0]}`);
    L.push("process");
  }
  return L;
}

for (const f of fs.readdirSync(outdir).filter((f) => f.endsWith(".txt")).sort()) {
  const lines = fs.readFileSync(path.join(outdir, f), "utf8").trim().split("\n");
  const seed = Number(f.replace(/\D/g, ""));
  const r = rng(seed * 7907 + 3);
  if (seed % 4 === 0) {
    fs.writeFileSync(path.join(outdir, f), contentionScene(r).join("\n") + "\n");
    continue;
  }
  const checkpointEdits = seed % 2 === 1;
  let w = 0;
  for (const l of lines) {
    const p = l.split(" ");
    if (p[0] === "shape") w = Math.max(w, +p[2] + +p[4], +p[3] + +p[5]);
  }
  // Shape pins (exclusive, as every pin here), for extra connectors that
  // compete for them in the deletion scenes.
  const pins = [];
  let nextId = 1;
  for (const l of lines) {
    const p = l.split(" ");
    if (p[0] === "pin") pins.push(`${p[1]} ${p[2]}`);
    if (p[0] === "shape" || p[0] === "conn") nextId = Math.max(nextId, +p[1] + 1);
  }
  const live = [];
  const onPins = new Set(); // connectors with an end on a shape pin
  let routed = false;
  const out = [];
  for (const l of lines) {
    const p = l.split(" ");
    if (p[0] === "process" && !routed && !checkpointEdits && pins.length) {
      for (let k = 2 + Math.floor(r() * 5); k > 0; k--) {
        const id = String(nextId++);
        out.push(`conn ${id} S ${pins[Math.floor(r() * pins.length)]} P ${Math.floor(r() * w)} ${Math.floor(r() * w)}`);
        live.push(id);
        onPins.add(id);
      }
    }
    if (p[0] === "process" && routed) {
      // Edits after the connectors have been routed once.
      if (checkpointEdits) {
        for (let k = Math.floor(r() * 3); k > 0 && live.length; k--) {
          const id = live[Math.floor(r() * live.length)];
          const n = Math.floor(r() * 3);
          const pts = [];
          for (let i = 0; i < n; i++) pts.push(Math.floor(r() * w), Math.floor(r() * w));
          out.push(`checkpoints ${id} ${n}${n ? " " + pts.join(" ") : ""}`);
        }
      } else {
        // Mostly connectors that hold a pin, which may free it for another.
        for (let k = 1 + Math.floor(r() * 2); k > 0 && live.length > 2; k--) {
          const held = live.filter((id) => onPins.has(id));
          const from = held.length && r() < 0.8 ? held : live;
          const id = from[Math.floor(r() * from.length)];
          live.splice(live.indexOf(id), 1);
          out.push(`delconn ${id}`);
        }
      }
    }
    if (p[0] === "delconn") {
      // An inserted deletion may already have removed this connector.
      if (!live.includes(p[1])) continue;
      live.splice(live.indexOf(p[1]), 1);
    }
    out.push(l);
    if (p[0] === "conn") {
      live.push(p[1]);
      if (p.includes("S")) onPins.add(p[1]);
    }
    if (p[0] === "process") routed = true;
  }
  fs.writeFileSync(path.join(outdir, f), out.join("\n") + "\n");
}
