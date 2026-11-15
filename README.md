# libavoid-rust

[![npm](https://img.shields.io/npm/v/libavoid-rust)](https://www.npmjs.com/package/libavoid-rust)
[![license](https://img.shields.io/npm/l/libavoid-rust)](LICENSE)
[![demo](https://img.shields.io/badge/demo-live-brightgreen)](https://cayduck.github.io/libavoid-rust/)
[![demo site build](https://github.com/cayduck/libavoid-rust/actions/workflows/pages.yml/badge.svg)](https://github.com/cayduck/libavoid-rust/actions/workflows/pages.yml)

`libavoid-rust` is a Rust port of the C++ connector-routing library
[libavoid](https://www.adaptagrams.org/documentation/libavoid.html) (part of
Adaptagrams), packaged for JavaScript through WebAssembly.

The Rust core in `src/avoid/` is a function-by-function port of upstream
libavoid: poly-line and orthogonal routing, connection pins, junctions,
clusters, checkpoints, nudging, and hyperedge rerouting and improvement. Its
routes are checked against the original C++ (see [Verification](#verification)).
The JavaScript API mirrors upstream's public C++ API (`namespace Avoid`):
the same classes, methods, arguments and defaults, backed by that core. It
adds a few things from the Adaptagrams fork that libavoid-js builds on:
`ConnRef.hasValidRoute()` and `ConnRef.hasCrossingObstacles()`, which tell
whether the router found a route and whether it crosses obstacles;
`Router.setRouteInsideParentShapes(true)`, which routes orthogonal
connectors inside shapes that contain other shapes; and
`Router.setRerouteAfterEdits(true)`, which reroutes connectors after
checkpoint changes and connector deletions that upstream does not reroute
for. The two settings are off by default, so routes stay upstream's.

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
const Avoid = require("libavoid-rust");

const router = new Avoid.Router(Avoid.OrthogonalRouting);
router.setRoutingParameter(Avoid.shapeBufferDistance, 10);

const box = new Avoid.ShapeRef(router, new Avoid.Rectangle(new Avoid.Point(40, 40), new Avoid.Point(120, 120)));
const pentagon = new Avoid.Polygon(5);
pentagon.ps[0] = new Avoid.Point(200, 40);
pentagon.ps[1] = new Avoid.Point(280, 40);
pentagon.ps[2] = new Avoid.Point(300, 80);
pentagon.ps[3] = new Avoid.Point(280, 120);
pentagon.ps[4] = new Avoid.Point(200, 120);
const target = new Avoid.ShapeRef(router, pentagon);

// Pin class 1 on the box's right edge, class 2 on the target's left edge.
new Avoid.ShapeConnectionPin(box, 1, Avoid.ATTACH_POS_RIGHT, Avoid.ATTACH_POS_CENTRE, true, 0, Avoid.ConnDirRight);
new Avoid.ShapeConnectionPin(target, 2, Avoid.ATTACH_POS_LEFT, Avoid.ATTACH_POS_CENTRE, true, 0, Avoid.ConnDirLeft);

const conn = new Avoid.ConnRef(router, new Avoid.ConnEnd(box, 1), new Avoid.ConnEnd(target, 2));
conn.setCallback((c) => {
  console.log(`connector ${c.id()}:`, c.displayRoute().ps.map((p) => [p.x, p.y]));
}, conn);

// As in upstream libavoid, changes are queued until the transaction is processed.
router.processTransaction(); // the callback reports the first route

router.moveShape(box, 0, 100);
router.processTransaction(); // and the new route after the move

router.delete();
```

The mapping from C++ to JavaScript follows a few rules, each stated at the
top of `index.d.ts`. `docs/JS_API_COVERAGE.md` lists every public item of
upstream's headers with its JavaScript counterpart, or the reason it has
none.

- **Names, arguments, defaults.** As upstream. An overloaded C++ function is
  one JavaScript function that dispatches on its arguments
  (`router.moveShape(shape, newPoly)` / `router.moveShape(shape, xDiff,
  yDiff)`).
- **Value types** (`Point`, `Polygon`, `Rectangle`, `ConnEnd`, `Checkpoint`,
  ...) are plain JavaScript objects. Arguments are copied when passed, and
  results are copies, also where upstream returns a reference
  (`conn.displayRoute()`, `shape.polygon()`).
- **Router-owned objects** (`ShapeRef`, `JunctionRef`, `ConnRef`,
  `ClusterRef`, `ShapeConnectionPin`) keep their identity: a getter returns
  the same object every time. They are deleted as upstream deletes them
  (`router.deleteShape(shape)`, `router.deleteConnector(conn)`, ...;
  `pin.delete()` for a pin), and a deleted object throws on every use.
  `router.delete()` is C++ `delete router`. `obj.id()` is upstream's numeric
  id; routes can depend on ids, so pass the ids of an upstream scene to
  reproduce it exactly.
- **Callbacks and virtual methods.** `conn.setCallback(cb, ptr)` is
  upstream's `ConnRef::setCallback()`: `cb(ptr)` runs for each connector whose
  route changed, before the call that ran the transaction returns. Virtual
  methods are overridden by subclassing (see below).
- **C++ idioms.** Out-parameters are arrays, `std::pair` is a two-element
  array, operators are methods (`equal`, `lessThan`, `add`, ...), and the
  `FILE *` output functions return the text.
- **Misuse.** A wrong argument type or count throws a `TypeError`; a violated
  documented precondition (upstream's `COLA_ASSERT`), a deleted object or an
  object of another router throws an `Error`. Neither changes any state.

### Failure containment

The core is a port of a C++ library that crashes on some inputs. Every such
input found so far is handled, but an input not yet found could still hit a
Rust panic. The WebAssembly is built with `panic = "abort"`, so a panic, a
failed allocation or a stack overflow stops the WebAssembly instance. The
package turns that into a contained, reportable error:

- The failing call throws an `AvoidInternalError` with the method name, the
  Rust panic message and source location (`src/...rs:line:column`) or the
  failed allocation size, a `kind`, and the original
  `WebAssembly.RuntimeError` as `cause`.
- The instance's state is unknown afterwards, so *every* object created on
  it is poisoned: each later call throws `AvoidInternalError` (`kind`
  `"poisoned"`) without running any WebAssembly, as do `new Avoid.Router()`
  and the geometry functions; `delete()` stays allowed. `internalError()`
  returns the error that poisoned the instance, or `null`.
- `reset()` (in the browser `await reset()`, or `await init()` again)
  creates a fresh instance. Objects created afterwards work normally; the
  scene has to be rebuilt.

### Long transactions

A transaction can take a long time (see "Known slow cases" in `PORTING.md`).
As in upstream, the router calls its virtual
`shouldContinueTransactionWithProgress()` during a transaction; override it
in a subclass of `Router` to watch progress or to request a stop:

```js
const Avoid = require("libavoid-rust");

class MyRouter extends Avoid.Router {
  shouldContinueTransactionWithProgress(elapsedTime, phaseNumber, totalPhases, proportion) {
    console.log(`phase ${phaseNumber} of ${totalPhases}: ${Math.round(proportion * 100)}%`);
    return elapsedTime < 1000; // milliseconds
  }
}

const router = new MyRouter(Avoid.OrthogonalRouting);
router.setRoutingParameter(Avoid.crossingPenalty, 200);
const end = (x, y) => new Avoid.ConnEnd(new Avoid.Point(x, y));
new Avoid.ConnRef(router, end(0, 0), end(100, 100));
new Avoid.ConnRef(router, end(0, 100), end(100, 0));
router.processTransaction();
router.delete();
```

The override is called with upstream's arguments at upstream's call sites
(`elapsedTime` is measured from the start of the JavaScript call that runs
the transaction). It must not use its own router; an exception it throws
counts as a stop request and is rethrown when that call returns. As upstream,
a stop request takes effect only in the crossing-improvement phases, which
run only with a non-zero `crossingPenalty` or `fixedSharedPathPenalty`;
route search, nudging and hyperedge improvement always run to completion.
The request is checked before each connector is rerouted, so the
transaction stops at most one route search late. Upstream frees the routes
of a whole group of crossing connectors before rerouting them one by one;
after a stop, the connectors of that group not yet rerouted are left
without a valid route.

### In the browser

Browsers and bundlers get an ES-module entry (`index.mjs`, via the
`browser` / `import` export conditions or `libavoid-rust/web`) with the same
API. It loads the WebAssembly asynchronously, so await `init()` before the
first `new Avoid.Router()`:

```js
import init, * as Avoid from "libavoid-rust";

await init(); // fetches pkg-web/libavoid_rust_bg.wasm next to the module
const router = new Avoid.Router(Avoid.OrthogonalRouting);
new Avoid.ShapeRef(router, new Avoid.Rectangle(new Avoid.Point(100, 0), new Avoid.Point(160, 60)));
const conn = new Avoid.ConnRef(
  router,
  new Avoid.ConnEnd(new Avoid.Point(0, 30)),
  new Avoid.ConnEnd(new Avoid.Point(260, 30)),
);
router.processTransaction();
console.log(conn.displayRoute().ps.map((p) => [p.x, p.y]));
router.delete();
```

`init(input)` also accepts the `.wasm` bytes, a `URL`, a `Response` or a
`WebAssembly.Module`; `initSync(bytesOrModule)` is the synchronous variant.
Node's `require()` and `import` use the synchronous CommonJS entry, which
needs no `init()`. Both entries share one adapter, `lib/adapter.cjs`.

## Demo

Try it at <https://cayduck.github.io/libavoid-rust/>.

`demo/` is an interactive page that runs the browser entry: drag shapes and
watch connectors re-route, switch between orthogonal and poly-line routing,
tune the routing parameters and options, and try presets for nudging, pins
and junctions, hyperedges and clusters. It is plain HTML, CSS and ES
modules with no dependencies.

```sh
npm run build:demo   # build pkg-web/ and assemble the site in demo-dist/
npm run demo         # serve demo-dist/ at http://127.0.0.1:8080/
npm run test:demo    # browser smoke test (needs Playwright; PLAYWRIGHT_BROWSER=chromium, webkit or firefox)
```

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
  not ported (dead code, declarations upstream never defines).
- **JavaScript API.** `test/fixtures-replay.cjs` replays all 83 upstream test
  programs through the JavaScript API of the built package (object for
  object, with upstream's ids) and compares the 80 that compile upstream
  with the C++ output: diagrams, exit codes, and the output printed by
  connector callbacks (one slow program runs only with
  `LIBAVOID_SLOW_FIXTURES=1`). `test/api-conformance.cjs` checks every row of
  `docs/JS_API_COVERAGE.md` (exports, methods, properties, constant values,
  required argument counts) and tests overloads, defaults, callbacks,
  subclassing, object identity and misuse.

`PORTING.md` describes how the port is organised, how the reference outputs
are produced, where the port deliberately differs from upstream, and which
inputs are known to be slow.

## Compared with libavoid-js

[libavoid-js](https://github.com/Aksem/libavoid-js) has brought libavoid to
JavaScript since 2021 by compiling the C++ with Emscripten, and it is the
routing engine of JointJS's `@joint/router-avoid`, of Sprotty and of ELK
integrations. It builds on [a fork of
Adaptagrams](https://github.com/Aksem/adaptagrams) with its own features and
fixes; this package translates upstream. In short (measured 2026-11-15
against libavoid-js 0.4.5, 0.5.0-beta.5 and its unreleased 0.5.0 work):

- **Routes.** This package matched upstream C++ exactly on 600 of 600
  random scenes. libavoid-js matched a native build of its fork on all 595
  scenes it finished, and the fork differs from upstream in 177 of the 600,
  almost all of them orthogonal.
- **Nested shapes.** The fork routes shapes nested inside other shapes,
  which upstream does not. This package does too with
  `router.setRouteInsideParentShapes(true)` (since the release after
  0.2.1), taking only the part of the fork's change that nesting needs:
  most of the fork's other differences from upstream, and its slow
  scenes, come from a part that nesting does not need.
- **Features.** This package exposes all of upstream's public API,
  including clusters, hyperedge rerouting, fixed routes and transaction
  progress, which libavoid-js does not. Both offer `hasValidRoute()` and
  `hasCrossingObstacles()` from libavoid-js's fork (here since the release
  after 0.2.1).
- **Invalid input.** In the published libavoid-js versions, input that
  violates one of upstream's assertions aborts the WebAssembly module until
  a reload; libavoid-js's unreleased 0.5.0 throws for most of it. Here the
  call throws or the input is ignored, and the module keeps working.
- **Speed.** Where both compute the same routes, libavoid-js is up to
  about 20% faster on light scenes and this package is faster on heavy
  ones. Some orthogonal scenes take libavoid-js's fork much longer (the
  slowest: 16.8 s there, 0.95 s here).
- **Size.** libavoid-js is about half the download (177 kB of gzipped
  WebAssembly against 326 kB).
- **API style.** This package mirrors the C++ API; libavoid-js 0.5.0 is
  moving to an API designed for JavaScript.

The details, the reasons behind each difference and how to reproduce them
are in [`docs/LIBAVOID_JS_COMPARISON.md`](docs/LIBAVOID_JS_COMPARISON.md).

## Not included

What upstream's public headers declare and this package does not expose
(details and reasons in `docs/JS_API_COVERAGE.md`):

- The debug-visualisation hooks: `DebugHandler`, `Router::setDebugHandler()`,
  `Router::debugHandler()`.
- The topology add-on: `TopologyAddonInterface`,
  `Router::setTopologyAddon()`.
- `Router::timers` (exists only in builds with `AVOID_PROFILE`).
- Members that take or return internal visibility-graph objects
  (`VertInf *`, `VertID`, `EdgeInf`): `Router::visGraph`, `invisGraph`,
  `visOrthogGraph`, `vertices`, `contains`, `enclosingClusters`,
  `generateContains()`; `Obstacle::firstVert()`, `lastVert()`;
  `ConnRef::src()`, `dst()`, `start()` and the `setEndpoint(type, VertID,
  Point *)` overload; `validateBendPoint()`.
- Classes that are not `AVOID_EXPORT`ed and serve the router's own code:
  `ConnRerouteFlagDelegate`, `PtOrder`, `ConnectorCrossings`,
  `splitBranchingSegments()`, `CmpConnPinPtr` / `ShapeConnectionPinSet`, and
  the contents of `graph.h`, `vertices.h`, `visibility.h`, `timer.h` and
  `debug.h`.

## License

LGPL-2.1-or-later, the same as upstream libavoid. This is a modified version
(a translation into Rust) of libavoid, Copyright (C) 2004-2015 Monash
University, written by Michael Wybrow and others; see `LICENSE` for the
notices and the licence text. The npm package contains the Rust source of
the WebAssembly modules it ships (`src/`, `Cargo.toml`, `Cargo.lock`) and
the scripts that build them.
