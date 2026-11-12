"use strict";

// Browser smoke test of the assembled demo site (npm run test:demo; not
// part of `npm test` because it needs a headless browser).  Serves
// demo-dist/, drives it with Playwright's Chromium and checks that routing
// renders, that dragging (mouse and touch), mode switches, presets,
// parameters, connecting and hyperedge rerouting change the routes, that the
// phone layout has no horizontal scroll, and that a failed wasm download
// shows a readable error.  Fails on any console error or page error.
// Screenshots go to test-output/demo/.
//
// Playwright is not a dependency of this package: it is loaded from
// node_modules if installed, or from the directory in PLAYWRIGHT_PATH
// (e.g. a global or npx-cached `playwright-core`), with its Chromium
// installed (`npx playwright install chromium`).

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { startServer } = require("../scripts/serve-demo.cjs");

const root = path.join(__dirname, "..");
const site = path.join(root, "demo-dist");
const shots = path.join(root, "test-output", "demo");

function loadPlaywright() {
  const candidates = [process.env.PLAYWRIGHT_PATH, "playwright", "playwright-core"].filter(Boolean);
  for (const c of candidates) {
    try {
      return require(c);
    } catch {
      // try the next one
    }
  }
  console.error("Playwright not found: install it (npm i -D playwright-core && npx playwright install chromium) or set PLAYWRIGHT_PATH.");
  process.exit(2);
}

async function main() {
  if (!fs.existsSync(path.join(site, "index.html"))) {
    throw new Error("demo-dist/ is missing: run `npm run build:demo` first");
  }
  fs.mkdirSync(shots, { recursive: true });
  const { chromium } = loadPlaywright();
  const server = await startServer(site, 0);
  const base = `http://127.0.0.1:${server.address().port}/`;
  const browser = await chromium.launch();
  try {
    await desktop(browser, base);
    await phone(browser, base);
    await dark(browser, base);
    await loadFailure(browser, base);
  } finally {
    await browser.close();
    server.close();
  }
  console.log(`ok - demo smoke test passed (screenshots in ${path.relative(root, shots)})`);
}

/** New page that records console errors and page errors. */
async function openPage(browser, base, contextOptions = {}, hash = "") {
  const context = await browser.newContext(contextOptions);
  const page = await context.newPage();
  const errors = [];
  page.on("console", (msg) => {
    if (msg.type() === "error" || msg.type() === "warning") errors.push(`console.${msg.type()}: ${msg.text()}`);
  });
  page.on("pageerror", (err) => errors.push(`pageerror: ${err.message}`));
  await page.goto(base + hash);
  return { context, page, errors };
}

async function waitReady(page) {
  await page.waitForFunction(() => ["ready", "error"].includes(document.documentElement.dataset.state));
  const state = await page.evaluate(() => document.documentElement.dataset.state);
  assert.equal(state, "ready", `demo failed to start: ${await page.textContent("#boot")}`);
}

/** id -> d of every rendered connector. */
function connectorPaths(page) {
  return page.$$eval("#layer-connectors [data-kind=connector]", (gs) =>
    Object.fromEntries(gs.map((g) => [g.dataset.id, g.querySelector(".conn").getAttribute("d")])),
  );
}

function changed(before, after) {
  return Object.keys(after).filter((id) => before[id] !== after[id]);
}

async function centreOf(page, selector) {
  const box = await page.locator(selector).first().boundingBox();
  assert.ok(box, `${selector} not visible`);
  return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
}

async function mouseDrag(page, from, dx, dy) {
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  for (let i = 1; i <= 8; i++) {
    await page.mouse.move(from.x + (dx * i) / 8, from.y + (dy * i) / 8);
    await page.waitForTimeout(16);
  }
  await page.mouse.up();
  await page.waitForTimeout(50);
}

async function desktop(browser, base) {
  const { context, page, errors } = await openPage(browser, base, { viewport: { width: 1360, height: 860 } });
  await waitReady(page);

  // Initial scene renders routes with real geometry.
  let paths = await connectorPaths(page);
  const ids = Object.keys(paths);
  assert.ok(ids.length >= 8, `expected the basics scene's connectors, got ${ids.length}`);
  for (const id of ids) assert.match(paths[id], /^M[-\d.]+ [-\d.]+(L|C)/, `connector ${id} has no route`);
  assert.match(await page.textContent("#t-last"), /\d ms$/);
  await page.screenshot({ path: path.join(shots, "desktop-basics.png") });

  // Drag a box: connectors re-route.
  const gateway = await page.$eval("#layer-shapes .shape:nth-child(2)", (g) => g.dataset.id);
  await mouseDrag(page, await centreOf(page, `.shape[data-id="${gateway}"] .body`), 40, 160);
  let after = await connectorPaths(page);
  assert.ok(changed(paths, after).length >= 3, "dragging the gateway should re-route its connectors");
  assert.match(await page.textContent("#t-updated"), /^\d+ of \d+$/);
  assert.equal(await page.isVisible("#selection"), true, "dragged box is selected");
  paths = after;

  // Switch to poly-line routing.
  await page.check("input[name=mode][value=polyline]");
  assert.equal(await page.evaluate(() => window.demo.editor.mode), "polyline");
  after = await connectorPaths(page);
  assert.ok(changed(paths, after).length >= ids.length - 1, "poly-line routes differ from orthogonal ones");
  await page.screenshot({ path: path.join(shots, "desktop-polyline.png") });
  await page.check("input[name=mode][value=orthogonal]");

  // Nudging preset; changing the nudging distance moves the parallel connectors.
  await page.selectOption("#scene", "nudging");
  paths = await connectorPaths(page);
  assert.equal(Object.keys(paths).length, 5);
  assert.match(await page.evaluate(() => location.hash), /scene=nudging/);
  await page.$eval("#param-idealNudgingDistance", (input) => {
    input.value = "24";
    input.dispatchEvent(new Event("input", { bubbles: true }));
  });
  await page.waitForTimeout(100);
  after = await connectorPaths(page);
  assert.ok(changed(paths, after).length >= 3, "nudging distance spreads the bundle");
  assert.equal(await page.evaluate(() => window.demo.editor.router.routingParameter(window.demo.api.RoutingParameter.idealNudgingDistance)), 24);
  await page.check("#show-raw");
  assert.ok((await page.$$eval("#layer-raw path", (ps) => ps.filter((p) => p.getAttribute("d")).length)) === 5, "raw routes drawn");
  await page.screenshot({ path: path.join(shots, "desktop-nudging-raw.png") });
  await page.uncheck("#show-raw");

  // Connect tool: drag from one pin to another creates a connector.
  await page.selectOption("#scene", "pins");
  const before = Object.keys(await connectorPaths(page)).length;
  await page.keyboard.press("c");
  const pinA = await centreOf(page, '.shape:nth-child(2) .pin[data-pin="top"]');
  const pinB = await centreOf(page, '.shape:nth-child(3) .pin[data-pin="top"]');
  await mouseDrag(page, pinA, pinB.x - pinA.x, pinB.y - pinA.y);
  assert.equal(Object.keys(await connectorPaths(page)).length, before + 1, "connect tool adds a connector");
  // Junction drag re-routes the net.
  await page.keyboard.press("v");
  paths = await connectorPaths(page);
  await mouseDrag(page, await centreOf(page, ".junction"), -60, 20);
  assert.ok(changed(paths, await connectorPaths(page)).length >= 2, "dragging the junction re-routes its connectors");
  await page.screenshot({ path: path.join(shots, "desktop-pins.png") });

  // Hyperedge preset: rerouting replaces the junction tree.
  await page.selectOption("#scene", "hyperedge");
  const junctionsBefore = await page.$$eval(".junction", (js) => js.map((j) => j.dataset.id).join());
  await page.click("#scene-actions button");
  const junctionsAfter = await page.$$eval(".junction", (js) => js.map((j) => j.dataset.id).join());
  assert.notEqual(junctionsAfter, junctionsBefore, "hyperedge rerouting creates new junctions");
  for (const d of Object.values(await connectorPaths(page))) assert.ok(d.length > 0);
  await page.screenshot({ path: path.join(shots, "desktop-hyperedge.png") });

  // Clusters render in poly-line mode.
  await page.selectOption("#scene", "clusters");
  assert.equal(await page.$$eval(".cluster", (cs) => cs.length), 2);
  await page.screenshot({ path: path.join(shots, "desktop-clusters.png") });

  // Stress scene: a drag re-routes every connector within a sane time.
  await page.selectOption("#scene", "stress");
  paths = await connectorPaths(page);
  assert.equal(Object.keys(paths).length, 45);
  await mouseDrag(page, await centreOf(page, "#layer-shapes .shape:nth-child(8) .body"), 30, 30);
  assert.ok(changed(paths, await connectorPaths(page)).length >= 1, "stress drag re-routes");
  const median = await page.evaluate(() => window.demo.editor.stats().median);
  console.log(`  stress scene: median transaction ${median.toFixed(1)} ms`);
  assert.ok(median < 1000, `stress transactions take ${median} ms`);

  // Rounded corners draw curves.
  await page.check("#show-curved");
  assert.ok(Object.values(await connectorPaths(page)).some((d) => d.includes("C")), "rounded corners use curves");
  await page.screenshot({ path: path.join(shots, "desktop-stress-curved.png") });
  await page.uncheck("#show-curved");

  // Keyboard: focus a box and move it with the arrow keys; Delete removes it.
  await page.selectOption("#scene", "basics");
  paths = await connectorPaths(page);
  await page.focus("#layer-shapes .shape");
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("ArrowDown");
  assert.ok(changed(paths, await connectorPaths(page)).length >= 1, "arrow keys move the focused box");
  const shapes = await page.$$eval(".shape", (s) => s.length);
  await page.keyboard.press("Delete");
  assert.equal(await page.$$eval(".shape", (s) => s.length), shapes - 1, "Delete removes the focused box");

  assert.deepEqual(errors, [], "no console errors on desktop");
  await context.close();
}

async function phone(browser, base) {
  const { context, page, errors } = await openPage(
    browser,
    base,
    { viewport: { width: 390, height: 844 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true },
    "#scene=basics",
  );
  await waitReady(page);
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
  assert.ok(overflow <= 0, `phone layout scrolls horizontally by ${overflow}px`);
  await page.screenshot({ path: path.join(shots, "phone.png") });
  await page.screenshot({ path: path.join(shots, "phone-full.png"), fullPage: true });

  // Touch drag of a box via CDP touch events.
  const paths = await connectorPaths(page);
  const from = await centreOf(page, "#layer-shapes .shape:nth-child(2) .body");
  const cdp = await context.newCDPSession(page);
  const touch = (type, x, y) => cdp.send("Input.dispatchTouchEvent", { type, touchPoints: type === "touchEnd" ? [] : [{ x, y }] });
  await touch("touchStart", from.x, from.y);
  for (let i = 1; i <= 6; i++) {
    await touch("touchMove", from.x + i * 5, from.y + i * 12);
    await page.waitForTimeout(20);
  }
  await touch("touchEnd");
  await page.waitForTimeout(50);
  assert.ok(changed(paths, await connectorPaths(page)).length >= 2, "touch drag re-routes connectors");
  assert.deepEqual(errors, [], "no console errors on phone");
  await context.close();
}

async function dark(browser, base) {
  const { context, page, errors } = await openPage(browser, base, { viewport: { width: 1360, height: 860 }, colorScheme: "dark" }, "#scene=pins");
  await waitReady(page);
  const bg = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);
  assert.notEqual(bg, "rgb(244, 247, 246)", "dark mode changes the background");
  await page.screenshot({ path: path.join(shots, "desktop-dark.png") });
  await context.close();
  const phoneDark = await openPage(
    browser,
    base,
    { viewport: { width: 390, height: 844 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true, colorScheme: "dark" },
    "#scene=nudging",
  );
  await waitReady(phoneDark.page);
  await phoneDark.page.screenshot({ path: path.join(shots, "phone-dark.png") });
  assert.deepEqual([...errors, ...phoneDark.errors], [], "no console errors in dark mode");
  await phoneDark.context.close();
}

async function loadFailure(browser, base) {
  const context = await browser.newContext({ viewport: { width: 900, height: 600 } });
  const page = await context.newPage();
  await page.route("**/*.wasm", (route) => route.abort());
  await page.goto(base);
  await page.waitForFunction(() => document.documentElement.dataset.state === "error");
  const text = await page.textContent("#boot");
  assert.match(text, /could not load the router/i);
  assert.equal(await page.$eval(".inspector", (el) => el.inert), true, "controls stay inert without a router");
  assert.equal(await page.isVisible("#boot"), true);
  await page.screenshot({ path: path.join(shots, "load-failure.png") });
  await context.close();
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
