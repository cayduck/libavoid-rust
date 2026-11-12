"use strict";

// Minimal static file server for the assembled demo (npm run demo).
//   node scripts/serve-demo.cjs [dir=demo-dist] [--port 8080]
// Serves only files inside `dir`, with the MIME types the demo needs
// (application/wasm enables streaming compilation).  Also used by
// test/demo-browser.cjs via startServer().

const fs = require("node:fs");
const http = require("node:http");
const path = require("node:path");

const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".svg": "image/svg+xml",
  ".wasm": "application/wasm",
  ".png": "image/png",
};

function startServer(dir, port = 0, host = "127.0.0.1") {
  const base = path.resolve(dir);
  const server = http.createServer((req, res) => {
    let pathname;
    try {
      pathname = decodeURIComponent(new URL(req.url, "http://localhost").pathname);
    } catch {
      res.writeHead(400).end();
      return;
    }
    let file = path.join(base, pathname);
    if (!file.startsWith(base + path.sep) && file !== base) {
      res.writeHead(403).end();
      return;
    }
    if (fs.existsSync(file) && fs.statSync(file).isDirectory()) file = path.join(file, "index.html");
    fs.readFile(file, (err, data) => {
      if (err) {
        res.writeHead(404, { "content-type": "text/plain" }).end("not found");
        return;
      }
      res.writeHead(200, {
        "content-type": TYPES[path.extname(file)] ?? "application/octet-stream",
        "cache-control": "no-cache",
      });
      res.end(data);
    });
  });
  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, host, () => resolve(server));
  });
}

module.exports = { startServer };

if (require.main === module) {
  const args = process.argv.slice(2);
  const portFlag = args.indexOf("--port");
  const port = portFlag >= 0 ? Number(args.splice(portFlag, 2)[1]) : Number(process.env.PORT ?? 8080);
  const dir = args[0] ?? path.join(__dirname, "..", "demo-dist");
  if (!fs.existsSync(path.join(dir, "index.html"))) {
    console.error(`${dir}/index.html not found: run \`npm run build:demo\` first`);
    process.exit(1);
  }
  startServer(dir, port).then((server) => {
    console.log(`serving ${path.resolve(dir)} at http://127.0.0.1:${server.address().port}/`);
  });
}
