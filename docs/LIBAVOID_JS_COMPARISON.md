# libavoid-rust and libavoid-js

[libavoid-js](https://github.com/Aksem/libavoid-js), by Vladyslav Hnatiuk
and contributors, has brought libavoid to JavaScript since 2021. It started
as part of a research project at TU Wien, and it is the routing engine
behind JointJS's
[`@joint/router-avoid`](https://docs.jointjs.com/learn/features/automatic-layouts/standalone-link-routing/),
[sprotty-routing-libavoid](https://github.com/Aksem/sprotty-routing-libavoid)
and [elkjs-libavoid](https://github.com/MrMint/elkjs-libavoid). If you are
choosing between the two packages, this page sets out how they differ, as
measured on 2026-11-15. Every number can be reproduced with
`tools/libavoid-js-compare/run.sh`.

The two packages bring the same library to JavaScript in different ways:

- **libavoid-js** compiles C++ libavoid to WebAssembly with Emscripten. It
  builds on [Aksem/adaptagrams](https://github.com/Aksem/adaptagrams), a
  maintained fork of Adaptagrams that adds features and fixes.
- **libavoid-rust** is a translation of upstream libavoid (Adaptagrams
  commit `840ebcf`) into Rust, compiled to WebAssembly. Its goal is to give
  the same results as upstream.

## Versions compared

| Package | Version | libavoid source |
| --- | --- | --- |
| libavoid-rust | 0.2.1 | upstream `840ebcf` |
| libavoid-js | 0.4.5, the last stable release (the version `@joint/router-avoid` 4.3.3 uses) | fork v1.0.4 |
| libavoid-js | 0.5.0-beta.5, npm's `latest` tag | fork v1.0.8 |
| libavoid-js | unreleased 0.5.0 work, commit `df689b5` of branch `new-emsdk-and-embind` (2026-11-13) | fork v1.0.9 |

libavoid-js 0.5.0 is under active development and changes much of what is
compared here (see [API](#api) and [Invalid input](#invalid-input)). The
unreleased commit is included so that this page does not describe problems
that are already being fixed.

## In short

- **Routes.** libavoid-rust gives exactly the routes of upstream C++
  libavoid. libavoid-js gives exactly the routes of its fork, which differ
  from upstream in many orthogonal scenes.
- **Nested shapes.** If your diagrams put shapes inside other shapes
  (containers, groups, swimlanes), libavoid-js routes them, and upstream
  does not. Since the release after 0.2.1, libavoid-rust does too with
  `router.setRouteInsideParentShapes(true)`, giving the same routes as the
  fork's nesting change applied to upstream.
- **Features.** libavoid-rust exposes all of upstream's public API,
  including clusters, hyperedge rerouting and transaction progress.
  libavoid-js exposes the core (shapes, pins, junctions, connectors,
  checkpoints). Both offer `hasValidRoute()` and `hasCrossingObstacles()`
  from libavoid-js's fork (libavoid-rust since the release after 0.2.1).
- **API style.** libavoid-rust mirrors the C++ API, so upstream's
  documentation and examples apply as written. libavoid-js 0.5.0 is moving
  to an API designed for JavaScript (`router.addShape({x, y, width,
  height})`, plain objects, only the router needs `delete()`).
- **Invalid input.** In the published libavoid-js versions, an input that
  violates one of upstream's assertions aborts the WebAssembly module, and
  the page or process has to be reloaded. The unreleased 0.5.0 checks
  arguments first and throws in most of these cases. libavoid-rust throws
  or ignores the input, and the module keeps working.
- **Speed.** Where both compute the same routes, libavoid-js is up to
  about 20% faster on light scenes and libavoid-rust is faster on heavy ones. Some
  orthogonal scenes take the fork much longer: the slowest one took 16.8 s
  in libavoid-js and 0.95 s in libavoid-rust.
- **Size.** libavoid-js is about half the download: 177 kB of gzipped
  WebAssembly against 326 kB.
- **Ecosystem.** libavoid-js has the integrations (JointJS, Sprotty, ELK)
  and a longer track record. libavoid-rust is new.

## Routes

600 random scenes (`gen.mjs`: 3 to 25 rectangles, shape pins, point and pin
ends, moves and connector deletions over several transactions, random
routing parameters and options; 435 orthogonal, 165 poly-line) were run
through every build and through native C++ builds of upstream and of the
fork. The routes after every transaction were compared exactly.

| | Scenes with the same routes as upstream C++ |
| --- | --- |
| libavoid-rust 0.2.1 | 600 of 600 |
| fork v1.0.9, native C++ | 423 of 600 |
| libavoid-js 0.4.5, 0.5.0-beta.5, unreleased | 422 of the 595 they finish (see [Invalid input](#invalid-input) for the other 5) |

All three libavoid-js versions agree with each other and with the native
fork on all 595 scenes. So the differences come from the fork's changes to
libavoid, not from the WebAssembly build. 174 of the 177 differing scenes
are orthogonal (40% of the orthogonal scenes), and 3 are poly-line.

Bisecting the fork shows that 170 of the 177 come from one commit,
[`11eabd8c`](https://github.com/Aksem/adaptagrams/commit/11eabd8c0571a602ca6880dfd13b615f686c0d7c)
"Route child shapes in hierarchical diagram". The fork's work on nesting,
as it stands in v1.0.9, makes two changes to orthogonal routing, and each
was applied to upstream on its own:

| Applied to upstream alone | Scenes that differ from upstream | `s00041` (native) | Nested scenes below |
| --- | --- | --- | --- |
| Shape-side vertices of the orthogonal graph get the id `dummyOrthogShapeID` (`orthogonal.cpp`, `11eabd8c`) | 170 of 600 | 1.9 s → 8.5 s | not routed |
| `Node::isShapeInShape()`: a shape that encloses another limits its view only with its far sides (`scanline.cpp`, `1b4a2369`) | 1 of 600 (the one where a move puts one shape inside another) | 1.9 s → 1.9 s | routed as the fork routes them |

So the nesting itself comes from the second change, which takes effect
only where a shape's routing box lies inside another's. The first change accounts for most of the fork's differences from
upstream and for its slow scenes (see [Speed](#speed)). The differences it
makes are usually a nudged segment in a different place, and sometimes a
different path.

### Shapes nested in a parent shape

Here two children inside a parent are connected by pins, with a third child
between them (`tools/libavoid-js-compare/scenes/nested-obstacle.txt`):

| | Route |
| --- | --- |
| upstream C++, libavoid-rust | `(80,130) (320,130)`: a straight line from centre to centre, through the shape in between |
| fork, libavoid-js | `(110,130) (140,130) (140,76) (260,76) (260,130) (290,130)`: around it, pin to pin |

With a second scene (`nested-free-points.txt`), upstream takes a connector
between two points inside the parent around the outside of the parent,
while the fork keeps it inside.

libavoid-rust takes the second change above as an option, off by default
so that routes stay upstream's. With `router.setRouteInsideParentShapes(true)`,
it routes both scenes as the fork does, and on the 600 random scenes it
matches upstream with that change applied exactly (and so differs from
upstream in the one scene with nesting). `tests/nested_shapes_oracle.rs`
checks both settings on 200 scenes built around nesting.

## API

### Upstream features each package exposes

Checked at runtime (`tools/libavoid-js-compare/features.mjs`), not from the
typings.

| Feature | libavoid-rust | libavoid-js 0.4.5 | 0.5.0-beta.5 | unreleased 0.5.0 |
| --- | --- | --- | --- | --- |
| Shapes, pins, connectors, poly-line and orthogonal routing, callbacks | yes | yes | yes | yes |
| Junctions | yes | yes | no | yes |
| Routing checkpoints | yes | no | yes | yes |
| Clusters (`ClusterRef`) | yes | no | no | no |
| Hyperedge rerouting (`Router.hyperedgeRerouter()`) | yes | no | no | no |
| Transaction progress and stop (`shouldContinueTransactionWithProgress`) | yes | no | no | no |
| Fixed routes (`ConnRef.setFixedRoute`) | yes | no | no | no |
| Pin transforms (`ShapeRef.transformConnectionPinPositions`) | yes | no | no | no |
| SVG and text debug output | yes | no | no | no |
| Routing inside shapes that contain other shapes (from the fork) | opt-in (`setRouteInsideParentShapes`), since the release after 0.2.1 | always | always | always |
| `ConnRef.hasValidRoute()`, `hasCrossingObstacles()` (from the fork) | yes, since the release after 0.2.1 | no | yes | yes |
| Rerouting after checkpoint changes and connector deletions (from the fork) | opt-in (`setRerouteAfterEdits`), since the release after 0.2.1 | deletions only (no checkpoint API) | always | always |

`docs/JS_API_COVERAGE.md` lists every public item of upstream's headers
with its libavoid-rust counterpart.

### Style

libavoid-rust keeps upstream's classes, method names, arguments and
defaults, so upstream's documentation and C++ examples translate line by
line:

```js
const shape = new Avoid.ShapeRef(router, new Avoid.Rectangle(new Avoid.Point(0, 0), new Avoid.Point(100, 50)));
new Avoid.ShapeConnectionPin(shape, 1, Avoid.ATTACH_POS_RIGHT, Avoid.ATTACH_POS_CENTRE, true, 0, Avoid.ConnDirRight);
```

libavoid-js 0.4.5 and the 0.5 betas expose the Emscripten bindings
directly: objects created with `new` must be freed with `.delete()` (or
`Avoid.destroy()`), and a connector callback receives a pointer to pass to
`Avoid.wrapPointer()`. The unreleased 0.5.0 replaces this with an API
designed for JavaScript:

```js
const shape = router.addShape({ x: 0, y: 0, width: 100, height: 50 });
shape.addPin({ classId: 1, x: 1, y: 0.5, directions: Avoid.ConnDirFlag.ConnDirRight });
```

In libavoid-rust, value types (`Point`, `Polygon`, `ConnEnd`, ...) are plain
JavaScript objects and only the router needs `delete()`, as in the
unreleased libavoid-js. libavoid-rust's Node.js entry loads synchronously
(`require` needs no `init()`); libavoid-js loads asynchronously
(`await AvoidLib.load()`) in both Node.js and browsers.

## Invalid input

Upstream libavoid checks its preconditions with assertions. libavoid-js is
built with assertions enabled, so a violated precondition aborts the
WebAssembly module. In 0.4.5 and 0.5.0-beta.5, every later call then fails
with `program has already aborted!`, and `AvoidLib.load()` does not create a
new instance, so the page or process has to be reloaded. Results of
`tools/libavoid-js-compare/misuse.mjs`:

| Input | libavoid-js 0.4.5, 0.5.0-beta.5 | unreleased 0.5.0 | libavoid-rust |
| --- | --- | --- | --- |
| `moveShape` to a polygon with a different number of points | module aborts | `RangeError` | the move is ignored |
| Delete a shape in the transaction that added it | module aborts | `Error` | the shape is never added |
| Pin class id 0 | module aborts | `RangeError` | `Error` |
| `NaN` connector end | module aborts | `TypeError` | the connector gets an empty route |
| Use a connector after deleting it | 0.4.5: no error; beta.5: module aborts | `Error` | `Error` |
| Orthogonal routing with `segmentPenalty` 0 | module aborts | module aborts | routes as upstream's release build does |

After each of these inputs, the libavoid-rust module keeps working, and so
does the unreleased libavoid-js module, except after the last row. For the
first row, the unreleased libavoid-js is more helpful: it reports the
mistake, while libavoid-rust silently ignores the move.

The same `segmentPenalty` assertion is why libavoid-js finishes 595 of the
600 random scenes: the other 5 set that parameter to 0.

libavoid-rust's own failures (a Rust panic, which no test has produced) are
contained in the same way: the call throws `AvoidInternalError`, and
`reset()` creates a fresh module (see "Failure containment" in the
README).

## Speed

594 of the random scenes (the 600 above without the 6 that set
`segmentPenalty` to 0, 5 of which abort libavoid-js) were timed one build
at a time, on Node.js 24 and an AMD Ryzen 5 5600X. The time is the sum
of the `processTransaction()` calls of a scene, measured the same way in
every build (one process per build, so the WebAssembly is warmed up).
Native C++ (g++ 13, `-O2`) is given for reference.

Where upstream and the fork compute the same routes (421 scenes), both
packages do the same work. Per-scene time of libavoid-js relative to
libavoid-rust (median), by scene weight:

| Native upstream time | Scenes | unreleased 0.5.0 / libavoid-rust | 0.4.5 / libavoid-rust |
| --- | --- | --- | --- |
| under 10 ms | 209 | 0.81 | 0.87 |
| 10–50 ms | 159 | 0.89 | 0.93 |
| 50–200 ms | 45 | 1.02 | 0.99 |
| over 200 ms | 8 | 1.61 | 1.85 |

So on light scenes, which are most of them, libavoid-js is faster: by
10–20% for the unreleased 0.5.0 and by 7–13% for 0.4.5.
On heavy scenes, libavoid-rust is faster because it skips work that cannot
change the result (listed in `PORTING.md` under "Work skipped or
reorganised, same results").

Total time per build:

| | Same routes (421) | Different routes (173) | All (594) |
| --- | --- | --- | --- |
| native C++, upstream | 11.1 s | 30.7 s | 41.8 s |
| native C++, fork v1.0.9 | 18.5 s | 115 s | 134 s |
| libavoid-rust 0.2.1 | 12.2 s | 16.4 s | 28.6 s |
| libavoid-js 0.4.5 | 15.3 s | 122 s | 137 s |
| libavoid-js 0.5.0-beta.5 | 18.2 s | 154 s | 172 s |
| libavoid-js unreleased | 13.8 s | 116 s | 130 s |

Where the routes differ, the native fork is as slow as libavoid-js, so the
difference comes from the fork's routing, not from the WebAssembly build.
On the three slowest scenes checked (`s00041`, `s00338`, `s00501`),
upstream with only the fork's shape-side vertex change is as slow as the
fork (1.9 s → 8.5 s, 3.8 s → 16.8 s, 2.8 s → 11.7 s), and upstream with only
its nesting change is as fast as upstream (see [Routes](#routes)). These
totals measure only time, not which routes are better.

## Size

| | WebAssembly (raw / gzip) | JavaScript glue (raw / gzip) | npm unpacked |
| --- | --- | --- | --- |
| libavoid-rust 0.2.1 | 935 kB / 326 kB | 150 kB / 29 kB (`pkg-web/libavoid_rust.js`, `lib/adapter.cjs`, `index.mjs`) | 2.7 MB |
| libavoid-js 0.4.5 | 485 kB / 174 kB | 76 kB / 18 kB | 0.96 MB |
| libavoid-js 0.5.0-beta.5 | 492 kB / 181 kB | 44 kB / 17 kB | 0.81 MB |
| libavoid-js unreleased | 476 kB / 177 kB | 60 kB / 22 kB | — |

The libavoid-rust npm package is larger on disk because it also contains
the Rust source of its WebAssembly (see "License" in the README).

## Which to choose

- Choose **libavoid-js** if you use JointJS, Sprotty or ELK (it is
  already integrated), if download size matters most, if your scenes are
  small (it routes them a little faster), or if you prefer an API designed
  for JavaScript. Until the release after libavoid-rust 0.2.1, it is also
  the choice for diagrams that nest shapes inside other shapes.
- Choose **libavoid-rust** if you need the same routes as C++ libavoid (for
  example, to match a desktop application or upstream's documentation), if
  you need clusters, hyperedge rerouting, fixed routes or transaction
  progress, if your scenes are large enough for routing to take a
  noticeable time, or if bad input must not take the module down.

Both packages are LGPL-2.1-or-later, like libavoid.

## Reproducing

```sh
npm run build:node
tools/libavoid-js-compare/run.sh
```

See `tools/libavoid-js-compare/README.md`. The scenes are generated from
fixed seeds, so the route comparison is exact and repeatable. Timings
depend on the machine.
