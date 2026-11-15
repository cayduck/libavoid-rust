// Summarises sequential timings: node bench-summary.mjs <timesdir> <routesdir>
// <timesdir>/<impl>.txt holds "<scene> <ms>" lines (ms = summed processTransaction() time).
// <routesdir> is the route comparison output (see run.sh routes), used to
// split the scenes into those where upstream and the fork compute the same
// routes and those where they do not.
import fs from "node:fs";
import { parse, same } from "./cmp.mjs";
const [timesDir, routesDir] = process.argv.slice(2);
const impls = fs.readdirSync(timesDir).filter((f) => f.endsWith(".txt")).map((f) => f.slice(0, -4));
const t = {};
for (const i of impls) {
  t[i] = new Map();
  for (const l of fs.readFileSync(`${timesDir}/${i}.txt`, "utf8").trim().split("\n")) {
    const [n, ms] = l.split(" ");
    if (ms !== undefined && ms !== "ERR" && ms !== "") t[i].set(n, +ms);
  }
}
const read = (impl, n) => { try { return parse(fs.readFileSync(`${routesDir}/${impl}/${n}.out`, "utf8")); } catch { return null; } };
const scenes = [...t["native-upstream"].keys()].filter((n) => impls.every((i) => t[i].has(n)));
const sameWork = new Set(scenes.filter((n) => same(read("upstream", n), read("fork", n))));
const median = (xs) => { const s = [...xs].sort((a, b) => a - b); return s[Math.floor(s.length / 2)]; };
const fmt = (x) => (x >= 100 ? x.toFixed(0) : x.toFixed(1));
for (const [label, set] of [["all scenes", scenes], ["scenes where upstream and the fork give the same routes", scenes.filter((n) => sameWork.has(n))], ["scenes where they differ", scenes.filter((n) => !sameWork.has(n))]]) {
  console.log(`\n${label} (${set.length})`);
  console.log("impl".padEnd(18) + "total s".padStart(10) + "  median ratio to native upstream".padStart(34));
  for (const i of impls) {
    const total = set.reduce((s, n) => s + t[i].get(n), 0) / 1000;
    const ratios = set.filter((n) => t["native-upstream"].get(n) > 1).map((n) => t[i].get(n) / t["native-upstream"].get(n));
    console.log(i.padEnd(18) + fmt(total).padStart(10) + (ratios.length ? median(ratios).toFixed(2) : "-").padStart(34));
  }
}
