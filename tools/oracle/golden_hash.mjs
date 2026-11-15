// Per-transaction hashes of a driver's output, for the compact golden files
// of tests/fork_route_status_oracle.rs, tests/nested_shapes_oracle.rs and
// tests/reroute_after_edits_oracle.rs (computed the same way by
// `transaction_hashes()` in tests/scene_replay/mod.rs).
//
// A transaction is its `T <n>` line and the lines after it.  Its hash is the
// 64-bit FNV-1a of, per line, the first word's bytes, a 0 byte, the IEEE
// bits (little-endian) of every further word read as a number, and a
// newline byte.  Reading the numbers makes the hash independent of how they
// are printed: the C++ prints `%.17g` and Rust prints the shortest form,
// both of which read back exactly.  NaN and the infinities are canonical.
//
//   node tools/oracle/golden_hash.mjs < driver-output    # prints h <hex> <lines> per transaction
//   node tools/oracle/golden_hash.mjs --convert <golden> # rewrites the `expect` sections of a
//                                                       # full-output golden file in place
import fs from "node:fs";

const PRIME = 0x100000001b3n;
const MASK = (1n << 64n) - 1n;
const f64 = new Float64Array(1);
const bytes = new Uint8Array(f64.buffer);

function number(word) {
  const w = word.toLowerCase();
  if (w === "nan" || w === "-nan" || w === "+nan") return NaN;
  if (w === "inf" || w === "+inf" || w === "infinity") return Infinity;
  if (w === "-inf" || w === "-infinity") return -Infinity;
  const v = Number(word);
  if (Number.isNaN(v)) throw new Error(`not a number: ${word}`);
  return v;
}

/** Hash lines (one transaction) as described above. */
export function hashTransaction(lines) {
  let h = 0xcbf29ce484222325n;
  const eat = (b) => {
    h = ((h ^ BigInt(b)) * PRIME) & MASK;
  };
  for (const line of lines) {
    const words = line.split(" ");
    for (const c of Buffer.from(words[0])) eat(c);
    eat(0);
    for (const word of words.slice(1)) {
      const v = number(word);
      f64[0] = Number.isNaN(v) ? NaN : v; // JS NaN is the canonical quiet NaN
      for (const b of bytes) eat(b);
    }
    eat(10);
  }
  return h.toString(16).padStart(16, "0");
}

/** Driver output lines -> `h <hex> <line count>` lines, one per transaction. */
export function hashOutput(lines) {
  const out = [];
  let tx = null;
  const flush = () => {
    if (tx) out.push(`h ${hashTransaction(tx)} ${tx.length}`);
  };
  for (const line of lines) {
    if (line === "") continue;
    if (line.startsWith("T ")) {
      flush();
      tx = [];
    }
    if (!tx) throw new Error(`output before the first transaction: ${line}`);
    tx.push(line);
  }
  flush();
  return out;
}

if (process.argv[2] === "--convert") {
  const file = process.argv[3];
  const out = [];
  let body = null;
  for (const line of fs.readFileSync(file, "utf8").split("\n")) {
    if (body) {
      if (line === "end" || line === "expect" || line.startsWith("expect ")) {
        out.push(...hashOutput(body));
        body = line === "end" ? null : [];
        out.push(line);
        continue;
      }
      body.push(line);
      continue;
    }
    out.push(line);
    if (line === "expect" || line.startsWith("expect ")) body = [];
  }
  fs.writeFileSync(file, out.join("\n"));
} else if (process.argv[1] && process.argv[1].endsWith("golden_hash.mjs")) {
  const lines = fs.readFileSync(0, "utf8").split("\n");
  for (const l of hashOutput(lines)) console.log(l);
}
