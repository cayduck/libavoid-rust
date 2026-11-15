// Numeric comparison of two route dumps. Prints "same", "error", or "diff T<k>".
import fs from "node:fs";
export function parse(text) {
  if (text.startsWith("ERROR") || text === "") return null;
  return text.trim().split("\n").map((l) => l.split(" ").map((x, i) => (i === 0 ? x : Number(x))));
}
export function same(a, b) {
  if (!a || !b) return false;
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) { const x = a[i], y = b[i]; if (x.length !== y.length) return false; for (let j = 0; j < x.length; j++) if (!Object.is(x[j], y[j]) && x[j] !== y[j]) return false; }
  return true;
}
if (process.argv[1]?.endsWith("cmp.mjs")) {
  const [a, b] = process.argv.slice(2).map((f) => parse(fs.readFileSync(f, "utf8")));
  console.log(!a || !b ? "error" : same(a, b) ? "same" : "diff");
}
