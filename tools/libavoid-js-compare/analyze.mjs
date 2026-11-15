// node analyze.mjs <root> impl...   -> per-impl status counts and pairwise agreement
import fs from "node:fs";
import { parse, same } from "./cmp.mjs";
const [root, ...impls] = process.argv.slice(2);
const names = fs.readdirSync(`${root}/${impls[0]}`).filter((f) => f.endsWith(".log")).map((f) => f.slice(0, -4)).sort();
const data = {};
for (const i of impls) {
  data[i] = {};
  for (const n of names) {
    let log = ""; try { log = fs.readFileSync(`${root}/${i}/${n}.log`, "utf8"); } catch { data[i][n] = { st: "missing" }; continue; }
    const exit = +(log.match(/exit (\d+)/)?.[1] ?? -1);
    let out = ""; try { out = fs.readFileSync(`${root}/${i}/${n}.out`, "utf8"); } catch {}
    let st = "ok";
    if (exit === 124) st = "timeout"; else if (exit !== 0 || / error /.test(log) || out.startsWith("ERROR")) st = "fail";
    const reason = (log.match(/Aborted\(([^)]*)\)/)?.[1] ?? log.match(/ error (.*)/)?.[1] ?? (exit > 128 ? `signal ${exit - 128}` : "")).slice(0, 120);
    data[i][n] = { st, routes: st === "ok" ? parse(out) : null, reason };
  }
}
console.log(`scenes: ${names.length}`);
for (const i of impls) {
  const c = {}; for (const n of names) c[data[i][n].st] = (c[data[i][n].st] || 0) + 1;
  console.log(i.padEnd(10), JSON.stringify(c));
}
console.log("\npairwise: same / both ok");
console.log("".padEnd(10) + impls.map((i) => i.padStart(14)).join(""));
for (const a of impls) {
  let row = a.padEnd(10);
  for (const b of impls) { let s = 0, both = 0; for (const n of names) { const x = data[a][n], y = data[b][n]; if (x.st === "ok" && y.st === "ok") { both++; if (same(x.routes, y.routes)) s++; } } row += `${s}/${both}`.padStart(14); }
  console.log(row);
}
if (process.env.LIST) {
  const [a, b] = process.env.LIST.split(",");
  for (const n of names) { const x = data[a][n], y = data[b][n]; if (x.st !== y.st || (x.st === "ok" && !same(x.routes, y.routes))) console.log(n, a, x.st, x.reason || "", "|", b, y.st, y.reason || ""); }
}
