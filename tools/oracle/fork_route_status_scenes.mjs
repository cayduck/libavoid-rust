// Scenes for the fork_route_status oracle: those of
// tools/libavoid-js-compare/gen.mjs, with routing checkpoints added to about
// a fifth of the connectors (some inside shapes, so that the checkpoint path
// search fails too).
//
//   node tools/oracle/fork_route_status_scenes.mjs <outdir> <first seed> <count>
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

for (const f of fs.readdirSync(outdir).filter((f) => f.endsWith(".txt")).sort()) {
  const lines = fs.readFileSync(path.join(outdir, f), "utf8").trim().split("\n");
  const r = rng(Number(f.replace(/\D/g, "")) * 104729 + 7);
  let w = 0;
  for (const l of lines) {
    const p = l.split(" ");
    if (p[0] === "shape") w = Math.max(w, +p[2] + +p[4], +p[3] + +p[5]);
  }
  const out = [];
  for (const l of lines) {
    out.push(l);
    const p = l.split(" ");
    if (p[0] === "conn" && r() < 0.2) {
      const n = 1 + Math.floor(r() * 2);
      const pts = [];
      for (let i = 0; i < n; i++) pts.push(Math.floor(r() * w), Math.floor(r() * w));
      out.push(`checkpoints ${p[1]} ${n} ${pts.join(" ")}`);
    }
  }
  fs.writeFileSync(path.join(outdir, f), out.join("\n") + "\n");
}
