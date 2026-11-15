# Comparison with libavoid-js

Scripts behind `docs/LIBAVOID_JS_COMPARISON.md`. They run the same scenes
through this package, through
[libavoid-js](https://github.com/Aksem/libavoid-js) and through native C++
builds of upstream libavoid and of the
[Adaptagrams fork](https://github.com/Aksem/adaptagrams) that libavoid-js
compiles, so that every difference can be traced to its source.

```sh
npm run build:node
tools/libavoid-js-compare/run.sh          # setup, routes, misuse, bench
tools/libavoid-js-compare/run.sh routes   # or one part: setup, routes, features, misuse, bench
```

Everything is downloaded and built under `target/libavoid-js-compare/`
(override with `WORK=`). Needs node, npm, git and g++.

| File | Purpose |
| --- | --- |
| `gen.mjs` | Random scenes using only what every compared build exposes: rectangles, shape pins, point and pin ends, moves, connector deletion, routing parameters and options. Integer coordinates. |
| `driver.cpp` | Replays a scene through C++ libavoid (`ConnRef(router, src, dst, id)` etc.; also `checkpoints`, which the random scenes do not use), printing each connector's display route after every `processTransaction()`. Also used by `tools/oracle/nested_shapes.sh` and `reroute_after_edits.sh`. |
| `drive.mjs` | The same for the JavaScript builds: `rust` (this package), `rustnested` (the same with `setRouteInsideParentShapes(true)`), `ljs045`, `ljsbeta5` (npm) and `ljsnext` (a commit of libavoid-js's unreleased 0.5.0 work, `LJS_NEXT`). |
| `cmp.mjs`, `analyze.mjs` | Exact numeric comparison of the route dumps; status counts and pairwise agreement. |
| `features.mjs` | Which upstream features each build exposes, checked on the loaded module rather than in its typings. |
| `misuse.mjs` | Inputs that upstream rejects with an assertion, and whether the module still works afterwards. |
| `bench-summary.mjs` | Totals and per-scene ratios of the sequential timings, split by whether upstream and the fork compute the same routes. |
| `scenes/nested-*.txt` | Shapes inside a parent shape, which the fork routes and upstream does not. |

Objects get the same ids in every build, because routes can depend on ids.
The C++ is built with `-DNDEBUG` (a release build); libavoid-js builds keep
assertions enabled.
