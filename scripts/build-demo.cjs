"use strict";

// Assembles the static demo site into demo-dist/ (or the directory given as
// the first argument): the files of demo/ plus the package's browser entry
// (index.mjs, lib/adapter.mjs, pkg-web/) under libavoid-rust/.  Requires
// `npm run build:web` first (npm run build:demo does both).  The same
// script assembles the GitHub Pages site in .github/workflows/pages.yml.

const fs = require("node:fs");
const path = require("node:path");

const root = path.join(__dirname, "..");
const out = path.resolve(process.argv[2] ?? path.join(root, "demo-dist"));
const pkg = require(path.join(root, "package.json"));

/** https://github.com/owner/repo from package.json `repository`. */
function repositoryUrl() {
  const repo = typeof pkg.repository === "string" ? pkg.repository : pkg.repository?.url;
  if (!repo) throw new Error("package.json has no repository");
  return repo.replace(/^git\+/, "").replace(/\.git$/, "").replace(/^git:\/\//, "https://");
}

const packageFiles = [
  "index.mjs",
  "lib/adapter.mjs",
  "pkg-web/libavoid_rust.js",
  "pkg-web/libavoid_rust_bg.wasm",
  "pkg-web/package.json", // "type": "module" for tools that look at it; harmless on the web
];

for (const file of packageFiles) {
  if (!fs.existsSync(path.join(root, file))) {
    console.error(`missing ${file}: run \`npm run build:web\` first (or \`npm run build:demo\`)`);
    process.exit(1);
  }
}

fs.rmSync(out, { recursive: true, force: true });
fs.mkdirSync(out, { recursive: true });

for (const entry of fs.readdirSync(path.join(root, "demo"))) {
  const src = path.join(root, "demo", entry);
  let content = fs.readFileSync(src);
  if (entry === "index.html") {
    content = content
      .toString("utf8")
      .replaceAll("{{repository}}", repositoryUrl())
      .replaceAll("{{version}}", pkg.version);
  }
  fs.writeFileSync(path.join(out, entry), content);
}

for (const file of packageFiles) {
  const dest = path.join(out, "libavoid-rust", file);
  fs.mkdirSync(path.dirname(dest), { recursive: true });
  fs.copyFileSync(path.join(root, file), dest);
}

// GitHub Pages runs Jekyll unless told not to; the site is plain static files.
fs.writeFileSync(path.join(out, ".nojekyll"), "");

const size = fs.statSync(path.join(out, "libavoid-rust", "pkg-web", "libavoid_rust_bg.wasm")).size;
console.log(`demo site assembled in ${path.relative(process.cwd(), out) || "."} (wasm ${(size / 1024).toFixed(0)} KiB)`);
