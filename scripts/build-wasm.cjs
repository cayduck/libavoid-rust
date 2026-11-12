"use strict";

// `wasm-pack build` for one target, with source paths remapped so that the
// .wasm embeds no absolute path of the build machine.
//
//   node scripts/build-wasm.cjs nodejs pkg
//   node scripts/build-wasm.cjs web pkg-web
//
// Panic messages (and therefore AvoidInternalError messages) carry the
// source location of the panic.  rustc embeds the paths it was given: this
// crate's own files are crate-relative ("src/avoid/router.rs"), but
// dependencies from the cargo registry and the standard library (when the
// rust-src component is installed, rustc maps std's "/rustc/<hash>" paths
// back to the local copy) would carry the home directory.  The prefixes are
// remapped with `--remap-path-prefix`; they are computed here rather than
// written into .cargo/config.toml because they are machine-specific.
// test/failure-containment.cjs checks the built files.

const { execFileSync } = require("node:child_process");
const os = require("node:os");
const path = require("node:path");

const [target, outDir] = process.argv.slice(2);
if (!target || !outDir) {
  throw new Error("usage: node scripts/build-wasm.cjs <nodejs|web> <out-dir>");
}

const root = path.join(__dirname, "..");
const cargoHome = process.env.CARGO_HOME || path.join(os.homedir(), ".cargo");
const sysroot = execFileSync("rustc", ["--print", "sysroot"], { encoding: "utf8" }).trim();

const remaps = [
  [path.join(sysroot, "lib", "rustlib", "src", "rust"), "/rustc/src"],
  [sysroot, "/rustc/sysroot"],
  [path.join(cargoHome, "registry", "src"), "/cargo/registry"],
  [path.join(cargoHome, "git", "checkouts"), "/cargo/git"],
  [cargoHome, "/cargo"],
  [root, "."],
];
if (process.env.RUSTUP_HOME) remaps.push([process.env.RUSTUP_HOME, "/rustup"]);
remaps.push([os.homedir(), "~"]);

// Later --remap-path-prefix flags take precedence, so the most specific
// prefixes go last.
const flags = remaps.reverse().map(([from, to]) => `--remap-path-prefix=${from}=${to}`);
const env = { ...process.env };
// CARGO_ENCODED_RUSTFLAGS (0x1f-separated) keeps paths with spaces intact
// and takes precedence over RUSTFLAGS; keep flags the caller already set.
const existing = env.CARGO_ENCODED_RUSTFLAGS
  ? env.CARGO_ENCODED_RUSTFLAGS.split("\x1f")
  : (env.RUSTFLAGS || "").split(/\s+/).filter(Boolean);
env.CARGO_ENCODED_RUSTFLAGS = [...existing, ...flags].join("\x1f");
delete env.RUSTFLAGS;

execFileSync("wasm-pack", ["build", "--target", target, "--out-dir", outDir], {
  cwd: root,
  env,
  stdio: "inherit",
});
