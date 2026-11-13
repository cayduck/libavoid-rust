# libavoid-rust

`libavoid-rust` is a Rust port of the C++ connector-routing library
[libavoid](https://www.adaptagrams.org/documentation/libavoid.html) (part of
Adaptagrams), packaged for JavaScript through WebAssembly.

The Rust core in `src/avoid/` is a function-by-function port of upstream
libavoid: poly-line and orthogonal routing, connection pins, junctions,
clusters, checkpoints, nudging, and hyperedge rerouting and improvement. Its
routes are checked against the original C++ (see [Verification](#verification)).
The JavaScript `AvoidRouter` is a thin adapter over that core that names
objects by string ids.

## Install

```sh
npm install libavoid-rust
```

## Build from source

```sh
npm install
npm run build        # both targets: build:node (pkg/) and build:web (pkg-web/)
npm run build:node   # Node.js package in pkg/
npm run build:web    # browser package in pkg-web/
```

Requires Rust with the `wasm32-unknown-unknown` target and `wasm-pack`.

## JavaScript usage

```js
const {
  AttachPos,
  AvoidRouter,
  ConnDir,
  RouterFlag,
  RoutingParameter,
} = require("libavoid-rust");

const router = AvoidRouter.withFlags(RouterFlag.OrthogonalRouting);
router.setRoutingParameter(RoutingParameter.shapeBufferDistance, 10);
router.addShape("box", 40, 40, 120, 120);
router.addPolygonShape("target", [
  { x: 200, y: 40 },
  { x: 280, y: 40 },
  { x: 260, y: 120 },
]);
router.addShapeConnectionPinWithOptions("box", "out", {
  classId: 1,
  xOffset: AttachPos.Right,
  yOffset: AttachPos.Centre,
  proportional: true,
  visibilityDirections: ConnDir.Right,
});
router.addShapeConnectionPinWithOptions("target", "in", {
  classId: 2,
  xOffset: AttachPos.Left,
  yOffset: AttachPos.Centre,
  proportional: true,
  visibilityDirections: ConnDir.Left,
});
router.addConnectorBetweenPinClasses("edge", "box", 1, "target", 2);

// As in upstream libavoid, changes are queued until the transaction is processed.
router.processTransaction();
console.log(router.displayConnectorRoute("edge"));
console.log(router.shapeBoundingBox("target"));
```

Things to know, all documented in detail at the top of `index.d.ts`:

- **Transactions.** Changes are queued until `processTransaction()`, as in
  upstream. `setTransactionUse(false)` processes every change immediately.
- **Routes.** `displayConnectorRoute(id)` is upstream `ConnRef::displayRoute()`
  (after nudging); `routeConnector(id)` is the raw `ConnRef::route()`.
- **Ids.** Objects are named by string ids. Each also has the upstream numeric
  id, assigned in creation order or passed explicitly as `numericId`; routing
  results can depend on numeric ids, so pass the ids of an upstream scene to
  reproduce it exactly.
- **Errors.** Malformed arguments throw a `TypeError`; otherwise mutators
  return `false` and queries return `null` for unknown ids or invalid values.
  An internal failure (a bug) throws an `AvoidInternalError`; see below.
- **Callbacks.** Instead of per-connector callbacks,
  `lastTransactionUpdatedConnectorIds()` lists the connectors whose routes
  changed in the last transaction.

The package also exports `WasmAvoidRouter`, the raw `wasm-bindgen` class,
which returns JSON strings for structured results.

### Failure containment

The core is a port of a C++ library that crashes on some inputs. Every such
input found so far is handled, but an input not yet found could still hit a
Rust panic. The WebAssembly is built with `panic = "abort"`, so a panic, a
failed allocation or a stack overflow stops the WebAssembly instance. The
package turns that into a contained, reportable error:

- The failing call throws an `AvoidInternalError` with the method name, the
  Rust panic message and source location (`src/...rs:line:column`), or the
  failed allocation size; `cause` is the original `WebAssembly.RuntimeError`.
- The instance's state is unknown afterwards, so *every* router created on it
  is poisoned: each later call throws `AvoidInternalError` ("router is
  unusable after an internal error: ...") without running any WebAssembly.
  `router.isPoisoned()` and `internalError()` report this.
- `reset()` (in the browser `await reset()`, or `await init()` again)
  creates a fresh instance. Routers created afterwards work normally; the
  scene has to be rebuilt on them.

A transaction can also take a long time (see "Known slow cases" in
`PORTING.md`). `setTransactionTimeLimit(ms)` and
`setTransactionProgressCallback(fn)` use upstream's
`shouldContinueTransactionWithProgress()` hook. Upstream only acts on a stop
request in its crossing-improvement phases, which run only with a
`crossingPenalty` or `fixedSharedPathPenalty`. Route search, nudging and
hyperedge improvement always run to completion. After a cancelled
transaction the next `processTransaction()` finishes the remaining work.
`index.d.ts` documents both features in detail.

### In the browser

Browsers and bundlers get an ES-module entry (`index.mjs`, via the
`browser` / `import` export conditions or `libavoid-rust/web`) with the same
API. It loads the WebAssembly asynchronously, so await `init()` first:

```js
import init, { AvoidRouter, RouterFlag } from "libavoid-rust";

await init(); // fetches pkg-web/libavoid_rust_bg.wasm next to the module
const router = new AvoidRouter(RouterFlag.OrthogonalRouting);
```

Node's `require()` and `import` keep using the synchronous CommonJS entry.
Both entries share one adapter, `lib/adapter.cjs`.

## Demo

`demo/` is an interactive page that runs the browser entry: drag shapes and
watch connectors re-route, switch between orthogonal and poly-line routing,
tune the routing parameters and options, and try presets for nudging, pins
and junctions, hyperedges and clusters. It is plain HTML, CSS and ES
modules with no dependencies.

```sh
npm run build:demo   # build pkg-web/ and assemble the site in demo-dist/
npm run demo         # serve demo-dist/ at http://127.0.0.1:8080/
npm run test:demo    # browser smoke test (needs Playwright with Chromium)
```

`.github/workflows/pages.yml` builds and publishes the same site with GitHub
Pages on every push to `main`, and can also be run by hand. It needs a
one-time repository setting: **Settings → Pages → Build and deployment →
Source: GitHub Actions**. The demo is then served at
`https://<owner>.github.io/<repository>/`, for this repository
<https://cayduck.github.io/libavoid-rust/>.

## Rust usage

The core is available as `libavoid_rust::avoid`. Its API mirrors upstream:
`Router::new(flags)`, `ShapeRef::new(&mut router, &polygon, id)`,
`ConnRef::new(&mut router, id)`, and so on, with `Copy` handles in place of
pointers. `tests/fixtures_orthogonal.rs` has many worked examples.

## Verification

Run everything with `npm test` (Rust tests, WASM build, Node tests).

- **Upstream fixtures.** All 83 programs in upstream `libavoid/tests` run
  against the port. The 80 that compile upstream must reproduce the C++
  output (connector routes, junction positions, exit code); the other 3 use
  APIs upstream has since removed and run as smoke tests.
- **Differential oracles.** Each module has a test that replays generated
  scenes and compares the result with output recorded from the C++
  (`tests/*_oracle.rs`, `tests/audit_scenes.rs`).
- **End-to-end fuzzing.** `tests/e2e_fuzz.rs` replays 1,000 generated scenes
  that mix every feature over long transaction sequences. The 865 on which
  the C++ is well defined must match it exactly; the rest must run without
  panicking. A further 20,000 fuzz scenes were checked the same way on the
  final port: 17,671 are comparable and all of them match (see
  `PORTING.md`).
- **Function inventory.** `docs/PORTING_INVENTORY.md` maps every upstream
  class, function and constant to its Rust counterpart or to the reason it is
  not ported (debug/SVG output and the like).

`PORTING.md` describes how the port is organised, how the reference outputs
are produced, where the port deliberately differs from upstream, and which
inputs are known to be slow.

## Not included

- SVG and debug output (`outputInstanceToSVG`, the debug handler, timers).
- From JavaScript: the topology add-on interface, per-connector callback
  functions and the router's raw public flag members.

## License

LGPL-2.1-or-later, the same as upstream libavoid. This is a modified version
(a translation into Rust) of libavoid, Copyright (C) 2004-2015 Monash
University, written by Michael Wybrow and others; see `LICENSE` for the
notices and the licence text. The npm package contains the Rust source of
the WebAssembly modules it ships (`src/`, `Cargo.toml`, `Cargo.lock`) and
the scripts that build them.
