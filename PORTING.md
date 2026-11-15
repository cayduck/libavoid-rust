# Porting notes: C++ libavoid to Rust

`src/avoid/` is a port of upstream libavoid from
[Adaptagrams](https://github.com/mjwybrow/adaptagrams) at commit `840ebcf`
(2025-10-29). The upstream source is vendored at
`vendor/adaptagrams/cola/libavoid` and is the source of truth: the Rust code
is ported from it function by function.

## Layout

| Path | Contents |
| --- | --- |
| `src/avoid/<name>.rs` | Port of upstream `libavoid/<name>.{h,cpp}`. Every function carries an `// upstream: file.cpp Class::method()` comment. |
| `src/avoid/arena.rs` | Arenas and `Copy` handles that stand in for upstream pointers. |
| `src/wasm_api.rs` | `RawRouter`, the low-level wasm-bindgen layer: router-owned objects named by numeric keys, values as flat numeric arrays, upstream's documented preconditions checked before the core is called. It is not public API and has no routing logic of its own. |
| `lib/adapter.cjs`, `index.cjs`, `index.mjs`, `index.d.ts` | The JavaScript API: upstream's public classes of `namespace Avoid` (`Router`, `ShapeRef`, `ConnRef`, `ConnEnd`, ...) implemented in `lib/adapter.cjs` over `RawRouter`. `index.cjs` (Node) and `index.mjs` (browsers) apply the one adapter to the `nodejs` and `web` wasm-pack builds. `index.d.ts` documents the rules that map C++ to JavaScript; `docs/JS_API_COVERAGE.md` lists every upstream public item and its JavaScript counterpart. A JavaScript subclass of `Router` overrides the virtual `shouldContinueTransactionWithProgress()`; a stop request has upstream's effect. |
| `demo/` | The interactive demo page (see the README). |
| `docs/PORTING_INVENTORY.md` | Generated function-level inventory: every upstream class, function and constant, mapped to its Rust counterpart or to the reason it is not ported. |
| `tests/` | Upstream fixtures, differential oracles, robustness tests and API tests. |
| `tools/oracle/` | C++ drivers and scripts that build upstream libavoid and record reference output. |
| `tools/fixtures/` | Converter from upstream `tests/*.cpp` to the op scripts in `tests/fixtures/`. |
| `tools/audit/` | The inventory generator. |
| `tools/wasm/` | Runs the Rust integration tests on a 32-bit WebAssembly target. |

The porting conventions and the data model are documented where they apply:
`src/avoid/mod.rs` (conventions) and the module docs of `src/avoid/router.rs`
(arenas and handles, intrusive lists, borrow pattern, dangling handles,
public-API misuse, asserts).

## How fidelity is achieved

Routes depend on details that are easy to lose in a port, so the port keeps
them deliberately:

- **Iteration order.** Upstream containers are reproduced with the same
  ordering semantics, including the intrusive vertex and edge lists,
  `std::set` comparators and `std::list` splicing.
- **Sorting and heaps.** Where the result of ties depends on the algorithm,
  the libstdc++ algorithm is ported exactly (`std::list::sort`, the
  `make_heap`/`push_heap`/`pop_heap` family, insertion sort for small
  `std::sort` ranges).
- **Floating point.** Expressions keep upstream's operation order; there is
  no fused multiply-add or algebraic simplification.
- **Pointer order.** Upstream orders some containers and breaks some ties by
  pointer address. The port uses the creation order of the objects instead;
  each such site is marked `// PTR-ORDER:`.

### Pointer order and the reference outputs

Because of that last point, upstream results depend on heap layout in a few
places. The reference outputs are therefore recorded from a build of the C++
that uses a bump allocator (`tools/oracle/bump_alloc.cpp`), which makes
pointer order equal to allocation order, the model the port implements.

For the upstream fixtures this matters in exactly one case: with the system
allocator, `hyperedge01` swaps the positions of two junctions. All other
fixture outputs are identical under both allocators. `tests/golden-bump/`
holds the bump-allocator outputs the tests compare against; `tests/golden/`
holds the system-allocator outputs for reference.

## Verification

`npm test` runs the Rust tests, builds the WASM package and runs the Node
tests. The pieces are:

| Test | What it checks |
| --- | --- |
| `tests/fixtures_ops.rs` | All 83 upstream fixtures, converted to op scripts. The 80 that compile upstream must reproduce the C++ routes, junction positions, exit code and stdout. The other 3 (`corneroverlap01`, `nudgeCrossing01`, `reallyslowrouting`) use APIs upstream has removed and run as smoke tests. Four slow fixtures only run with `LIBAVOID_SLOW_FIXTURES=1`. |
| `tests/fixtures_polyline.rs`, `fixtures_orthogonal.rs`, `fixtures_hyperedge.rs` | Hand translations of the smaller fixtures. |
| `tests/*_oracle.rs` | Per-module differential tests against recorded C++ output: geometry, objects and router state, crossings, visibility and path search, orthogonal visibility graph, nudging, hyperedges, VPSC (in `src/avoid/vpsc.rs`). |
| `tests/audit_scenes.rs` | 146 generated scenes covering router flags, every routing option and extreme parameter values, compared with the C++. |
| `tests/e2e_fuzz.rs` | End-to-end fuzz scenes that mix every feature over long transaction sequences. Of the 1,000 committed scenes, the 865 on which upstream is well defined must match the C++ exactly; the other 135 (upstream crashes, undefined behaviour or runs out of memory) and a set of regression scenes must run without panicking. Debug builds check a tenth of the scenes. |
| `test/fixtures-replay.cjs` | All 83 upstream fixtures and the audit scenes replayed through the JavaScript API of the built package (with upstream's ids), compared with the C++ output like `tests/fixtures_ops.rs`, including the stdout printed by connector callbacks. |
| `test/api-conformance.cjs`, `test/api-sync.cjs` | Every row of `docs/JS_API_COVERAGE.md` (exports, methods, properties, constant values, required argument counts); overloads, defaults, callbacks, subclassing, object identity and misuse; `index.d.ts` and the adapter in step with the modules and the wasm exports. |
| `test/node-smoke.cjs`, `test/esm-entry.mjs`, `test/failure-containment.cjs` | Hand-written fixture translations and the README examples; the ES-module entry; failure containment and `shouldContinueTransactionWithProgress()` overridden by subclassing. |
| `tests/robustness.rs`, `tests/core_findings.rs` | Misuse of the public API must not panic or hang. |
| `tests/svg_oracle.rs` | `outputInstanceToSVG()` (with its reproduction code: every `outputCode()`) and `outputDiagramSVG()` at every diagram dump of all fixtures and audit scenes: 1,838 files, byte for byte with the C++ (with `high-precision-debug`: against the C++ built with `HIGH_PRECISION_DEBUG`). |
| `tests/debug_output_files.rs` | The file-writing forms, and what no fixture reaches (`HyperedgeRerouter::outputInstanceToSVG()` with pending registrations; a scene with clusters, pins, checkpoints, a fixed route and a poly-line connector), against `tools/oracle/debug_output_oracle.cpp`. |
| `tests/fork_route_status_oracle.rs` | `ConnRef::has_valid_route()` and `has_crossing_obstacles()`, taken from the libavoid-js fork (see "Where the port differs from upstream"), after every transaction of 250 generated scenes with checkpoints, against the vendored upstream with the fork's two commits applied (`tools/oracle/fork_route_status.sh`). Debug builds check a tenth. |
| `tests/nested_shapes_oracle.rs` | `Router::set_route_inside_parent_shapes()` (see "Where the port differs from upstream") on 200 generated scenes with shapes nested in shapes: the routes after every transaction must match upstream with the option off and upstream with `tools/oracle/nested_shapes.patch` with it on (`tools/oracle/nested_shapes.sh`). Debug builds check a tenth. |
| `tests/e2e_svg_oracle.rs` | The same two SVGs after every transaction of the 865 committed fuzz scenes on which upstream is well defined, byte for byte (hashes recorded by `tools/oracle/e2e_svg_oracle.sh`). Debug builds check a tenth. |
| `tests/*_debug_oracle.rs`, `tests/debughandler_oracle.rs`, `tests/timer_oracle.rs` | The feature-gated debug output (see "Debug output") against the C++ built with the matching define, over all fixtures and audit scenes. Run with the feature named in each file. |
| `tools/audit/inventory.py --check` | Every upstream item is accounted for, and every `// upstream:` comment names something that exists. |
| `tools/wasm/run_wasm_tests.sh` | The integration tests on `wasm32-wasip1`, where `usize` is 32 bits. |

Useful commands:

```sh
tools/fixtures/run_all.sh            # status table for every fixture
tools/fixtures/verify_roundtrip.sh   # prove the op scripts against the C++
ORACLE_ALLOC=bump tools/oracle/build.sh   # rebuild upstream, regenerate tests/golden-bump
python3 tools/audit/inventory.py     # regenerate docs/PORTING_INVENTORY.md
tools/oracle/svg_oracle.sh           # re-record the SVG goldens (SVG_MODE=... for the other debug output)
tools/oracle/e2e_svg_oracle.sh       # re-record the fuzz scenes' SVG goldens
tools/oracle/e2e_fuzz.sh --seed 42 --scenes 2000   # fuzz fresh scenes against the C++
```

Beyond the committed fuzz scenes, 20,000 further scenes from eight other
seeds were run against the C++. The record of the last full run, on the
final port, is `tools/oracle/e2e_fuzz_campaign_final.log`; every entry
names the port commit it ran on. An earlier run on an older port is in
`tools/oracle/e2e_fuzz_campaign.log`.

On the final port, 17,671 of the 20,000 scenes are comparable with the C++.
17,655 match within the campaign's 20-second limit per scene. The other 16
are hyperedge improvement scenes that gain junctions on every transaction;
run to completion (19 to 40 seconds), they match too. The remaining 2,329
scenes are excluded because the C++ crashes, runs out of memory or has
undefined behaviour there, so there is nothing to compare against. On
those the port must only keep working: none of them makes it panic or
abort, and 300 take it more than 20 seconds.

Three classes of that undefined behaviour were found by the campaign
itself, because the sanitizer build had missed them, and the oracle now
detects them: erasing the `std::set` element being iterated, a double free
(detected by the bump allocator), and a connector used by the test driver
after a transaction had freed it.

The 300 slow scenes were traced on the final port (each run for up to 40
seconds, with the guard firings logged). None panics. In 296 no hyperedge
guard of the port fires at all: there the port runs upstream's own
algorithm, mostly hyperedge improvement that adds junctions on every
transaction (on an earlier port, in a random sample of 15 such scenes the
C++ finished 2, both identical, ran out of memory on 11, timed out after 5
minutes on 1, and 1 is now excluded as undefined behaviour). In the other
four a guard fires:

- **s7-942** is slow from command 83 on; the guard (rerouter, no junction)
  first fires at command 118 and the object counts do not change there.
- **s26-508** takes the improver's split-parallel-edges guard on a cyclic
  hyperedge. The growth starts at command 81, in a stretch where the guard
  does not fire, and a variant of the scene in which it never fires grows
  the same way; the C++ matches the port bit for bit on that variant (and
  on a longer one, which the ASan build finishes and the bump-allocator
  build runs out of memory on). The guard returns before changing
  anything. After command 108 the guard fires in every transaction and
  upstream itself does not return, so there is no reference for that
  stretch; the growth rate there is the one verified before it.
- **s23-1062** and **s25-1506** take a hyperedge-rerouter guard and are
  covered below, together with s25-1253, which was slow before the skip
  clean-up and no longer is.

Earlier, three scenes that kept growing because of what a guard left behind
were fixed (see "Upstream non-termination" in `src/avoid/router.rs`).

The rerouter guard scenes are s23-1062 (no junction),
s25-1253 (no junction, then a cyclic MTST tree) and s25-1506 (cyclic MTST
tree). A provenance trace (`E2E_PROV`, see `tests/e2e_fuzz.rs`) and variants
of the scenes without the skipped registration settled each case:

- **s23-1062.** Without the registration, the counts are the same as the
  original at every command. The C++ is clean on the first 145 commands of
  that variant (NDEBUG and ASan), and the port matches it byte for byte, so
  the growth is upstream's.
- **s25-1253, no-junction skip.** Without that registration the counts are
  the same as the original at every command.
- **s25-1253 and s25-1506, cyclic skip.** That skip used to leave state
  behind. In s25-1506 the hyperedge already gained 4 to 5 junctions and
  connectors per transaction before the skip (`txn 0`: every command is a
  transaction). Replacing the registration with a no-op transaction gave
  the same growth, in the same hyperedge, at the same rate. The C++ is
  clean on the first 77 commands of that variant, and the port matches it
  byte for byte. The skip, however, kept the junctions the MTST had created
  (8 and 13, attached to nothing and reported in no list). It also kept the
  `pathNext` links the MTST had written, which put s25-1253 on a different,
  faster-growing path (it ran more than 60 s; it now takes about 15 s).
  The skip now frees those junctions, restores `pathNext`, and reports
  nothing as deleted (`reg-hyperedge-cyclic-skip-clean`). After that:
  - s25-1506 has the same counts as the no-op variant after every command
    except the skipped transaction itself.
  - s25-1253 grows at about the rate of the variant without the
    registration (the C++ is clean on the first 175 commands of that
    variant, and the port matches).

The oracle scripts need `g++` and the vendored upstream source, which is not
checked in (`vendor/adaptagrams` is a checkout of the Adaptagrams repository
at the commit above).

## Debug output

Upstream's debug and SVG output is ported with the rest. What upstream
always compiles is always available: `Router::outputInstanceToSVG()`,
`outputDiagramSVG()`, `outputDiagram()`, `outputDiagramText()`,
`printInfo()`, every `outputCode()`, `HyperedgeRerouter::outputInstanceToSVG()`,
`Router::setDebugHandler()`, the vpsc printers. What upstream compiles only
under a debug define is behind a Cargo feature, all off by default, so a
default build contains none of it (and is exactly as fast as before):

| Upstream define | Feature | Effect |
| --- | --- | --- |
| `LIBAVOID_DEBUG` | `libavoid-debug` | `db_printf` prints to stdout (graph, visibility, router, connector traces) |
| `PATHDEBUG`, `ASTAR_DEBUG` | `pathdebug`, `astar-debug` | path search traces through `db_printf` (so with `libavoid-debug`) |
| `DEBUGHANDLER` | `debug-handler` | the router calls its `DebugHandler`, including `MinimumTerminalSpanningTree::drawForest()` |
| `AVOID_PROFILE` | `avoid-profile` | `Router::timers` and the `TIMER_*` instrumentation; `printInfo()` prints them |
| `HYPEREDGE_DEBUG` | `hyperedge-debug` | hyperedge improvement writes `DEBUG/hyperedges-NNNNN.svg` |
| `MAJOR_HYPEREDGE_IMPROVEMENT_DEBUG` | `major-hyperedge-improvement-debug` | hyperedge topology changes on stderr |
| `ESTIMATED_COST_DEBUG` | `estimated-cost-debug` | A* cost targets on stderr |
| `NUDGE_DEBUG`, `NUDGE_DEBUG_SVG` | `nudge-debug`, `nudge-debug-svg` | nudging regions, variables and results on stderr/stdout |
| `HIGH_PRECISION_DEBUG` | `high-precision-debug` | `PREC` is `.16` in the reproduction code |
| `SVG_OUTPUT` | `svg-output` | `outputDiagram()` also writes `outputInstanceToSVG()` |

Every one of these outputs is compared byte for byte with the C++ built
with the same define (`tools/oracle/svg_oracle.sh` records them for all
fixtures and audit scenes; the tests are listed under "Verification").
Two things are address dependent upstream and normalised on both sides:
`NUDGE_DEBUG` prints a `ConnRef` pointer (the port prints the connector's
creation serial; both are masked), and the vpsc printer shows the
uninitialised `lm` of inactive constraints (masked). The hyperedge debug
SVGs, the `DebugHandler` calls and the vpsc sets print no addresses, but
their order follows pointer order, so they are recorded with the bump
allocator like every other golden.

## Where the port differs from upstream

The port reproduces upstream wherever upstream's behaviour is defined. The
differences are confined to the following.

- **Additions from the libavoid-js fork.** These come from
  [Aksem/adaptagrams](https://github.com/Aksem/adaptagrams), the Adaptagrams
  fork that libavoid-js builds on. Each site is marked `// fork:` rather
  than `// upstream:`, so the porting inventory does not list them.
  - `ConnRef::has_valid_route()` (with the flag it reads) and
    `ConnRef::has_crossing_obstacles()` (commits `3f52e7e1`, `fe73f026`).
    They only read state and change no route.
  - `Router::set_route_inside_parent_shapes()`: orthogonal visibility
    treats a shape that encloses another as its frame, not as an obstacle
    (`Node::isShapeInShape()`, commit `1b4a2369`), so that routes can run
    between shapes inside a container. It is off by default, which keeps
    upstream's routes. The fork has it always on. The fork's other change
    in the same work, giving shape-side vertices of the orthogonal graph
    the id `dummyOrthogShapeID` (commit `11eabd8c`), is not taken: nesting
    does not need it, and it changes routes in many scenes without nesting
    and makes some several times slower (see
    `docs/LIBAVOID_JS_COMPARISON.md`).
- **Not ported.** Dead `#if 0` code and functions that upstream declares
  but never defines; the inventory lists each one with its reason. The
  debug output is ported (see "Debug output" above); of upstream's debug
  defines, only `SELECTIVE_DEBUG` and `LIBVPSC_LOGGING` have no feature,
  because their code does not compile at this upstream commit (the vpsc
  printers `LIBVPSC_LOGGING` would use are ported as string writers).
  `DEBUG_JUST_UNIFY` and `PAPER`, which change what is computed rather than
  print, are not mirrored either.
- **Debug output targets.** Upstream's `FILE *` writers take a
  `&mut dyn std::fmt::Write`; the upstream-named methods that create files
  (`Router::output_instance_to_svg`, `output_diagram_svg`, `output_diagram`,
  `output_diagram_text_file`) return the I/O error where upstream returns
  silently, and fail on targets without a file system
  (`wasm32-unknown-unknown`), where the `*_string` forms are the way in.
  The `DebugHandler` is owned by the router and receives the router in
  every call. The timers measure wall-clock time (Rust has no CPU clock);
  their tallies and counters are exact.
- **Undefined behaviour upstream.** Where upstream reads freed memory,
  dereferences null or recurses without bound, the port does something
  defined instead and never panics. The cases are listed under "Public-API
  misuse" and "Dangling handles" in `src/avoid/router.rs`. The main ones:
  - A `pathNext` link to a deleted vertex reads as null.
  - Using a handle after deleting its object is a no-op.
  - A registered hyperedge with fewer than three distinct terminals, or one
    containing a cycle, is skipped. The router is then left as if that
    hyperedge had not been registered, except that its connectors are not
    rerouted in that transaction (as on upstream's own invalid-hyperedge
    path).
  - With transactions off, a `processTransaction()` nested inside another is
    a no-op.
- **Infinite loops upstream.** Two loops in hyperedge improvement never
  finish upstream when a connector has both ends on the same junction
  (`HyperedgeTreeNode::spliceEdgesFrom` called on its own node). The port
  detects that exact condition and carries on. When such a connector already
  forms a self-loop edge in the hyperedge tree and major changes are on, the
  port skips that hyperedge, as it would a cyclic one; when the self-loop
  only appears during improvement, the port does not re-enter the junction
  through it when it writes the routes back (which would triple a route on
  every transaction). The guards cannot fire
  on a run that finishes upstream. See "Upstream non-termination" in
  `src/avoid/router.rs`.
- **Asserts.** `COLA_ASSERT` is `debug_assert!`, except for asserts that
  upstream itself violates on valid input; those are not ported, so that debug
  builds do not panic where upstream's release build works. Each site cites
  its evidence.
- **Callbacks.** In the Rust core, a connector callback is a closure that
  receives the connector handle and has no access to the router. In
  JavaScript, `ConnRef.setCallback(cb, ptr)` is upstream's: `RawRouter`
  records the callbacks the core performs, and the adapter invokes `cb(ptr)`
  for each, in the core's order, after the transaction has finished and
  before the call that ran it returns. Upstream invokes the callbacks inside
  the transaction, before its last `shouldContinueTransactionWithProgress()`
  call.
- **Uncaught exceptions.** Where upstream's VPSC solver would throw an
  exception nothing catches, the port leaves that nudging region unchanged.
- **Work skipped or reorganised, same results.** Each site is marked
  `(not upstream)` and gives its argument; debug builds check most of them
  against upstream's computation as it runs.
  - A* search: the cleanup visits only the vertices the search touched;
    replacing a pending node runs `std::make_heap`'s steps only at that
    node's position and its ancestors when the node order is a strict total
    order on the keys present (otherwise the whole `make_heap`).
  - A* cost with crossing, shared-path or cluster penalties: connectors and
    cluster boundaries whose segments cannot touch the route's last segment
    are not compared (their contribution is zero); a per-search grid finds
    the candidates.
  - The A* cost builds the candidate route's prefix once per expansion
    from per-node copies of what `constructPolygonPath` reads, and keeps
    `splitBranchingSegments`' state after that prefix per other polygon,
    splitting only the last segment per edge (without copies when it
    inserts nothing).
  - Crossing detection in `improveOrthogonalRoutes` and `improveCrossings`:
    pairs of routes that cannot touch are skipped; `splitBranchingSegments`
    skips segments that cannot touch the other polygon's bounds.
  - `checkAllBlockedEdges` takes the edges blocked by an obstacle from an
    index (by blocker, ordered by when they joined the invisibility graph)
    instead of walking the whole graph.
  - The poly-line vertex sweep builds its angular set by one sort (keeping
    the first of equivalent points, as `std::set::insert` does) and does
    not re-sort an edge list that is already in order; short vertex edge
    lists are sorted without allocating, and not relinked when unchanged.
  - Nudging regions are gathered without rescanning the segment list.
  - VPSC `satisfy()`: whether `findMinLMBetween` finds a split point, and
    the cycle check, are answered from a cached active constraint tree; the
    Lagrange multipliers are only computed when a split point exists, and
    the path of an `UnsatisfiableException` (which upstream discards) is not
    built. So after a failed split the multipliers keep their previous
    values, which only `IncSolver::print_blocks` shows.
  - `EdgeInf::existingEdge()` uses an index of the non-orthogonal edges.

## Known slow cases

The first two are upstream's behaviour, reproduced faithfully, and can look
like a hang in an interactive application.

- **Hyperedge improvement that adds junctions.** With
  `improveHyperedgeRoutesMovingAddingAndDeletingJunctions` on, some scenes gain
  a junction and a connector on every `processTransaction()`, so each
  transaction is slower than the last. On a truncated fuzz scene the C++ took
  10.5 s and the port 10.6 s, with identical output.
- **Unifying nudging stage with many free segments.** With
  `performUnifyingNudgingPreprocessingStep` on (the default) and
  `fixedSharedPathPenalty` 0, `nudgeOrthogonalRoutes` first tries to give
  pairs of free segments in a nudging region the same position. Upstream
  means to stop considering a pair once one of its segments has been merged.
  However, `PotentialSegmentConstraint& pc` refers to the list element that
  the rewrite loop changes first, so every later `rewriteIndex` call does
  nothing. As a result, all P = F(F−1)/2 pairs of the region's F free
  segments are tried. That takes exactly P + 1 VPSC solves, or P + 2 if the
  last pair is rejected, and the number of equality constraints grows to P.
  Each solve costs roughly the square of its constraint count, so a region
  costs about P³:
  - F = 26 (P = 325): 0.15 s.
  - F = 80 (P = 3,160): about 150 s.

  The relaxation loop of the actual nudging stage is cheap by comparison:
  each unsatisfied solve lowers the separation by a tenth of the ideal
  distance, so a region needs at most 11 solves.

  Measurements:
  - **Defined 80-connector scene** (80 Z-shaped
    connectors whose middle segments share one channel): C++ -O2 took 81 s
    with the system allocator and 702 s with the bump allocator. The port
    took 299 s on a loaded machine, before the VPSC change below. The output
    was identical.
  - **Driver for one unifying region** (51 free segments, 1,276 solves, both
    programs pinned to the same core, medians of 3): upstream's `IncSolver`
    took 5.1 s. The port took 10.4 s, and after moving the VPSC active-tree
    traversals onto slices it takes 6.6 s. The output is bit-identical.
  - Since `satisfy()` answers its split and cycle questions from a cached
    active tree (see "Work skipped or reorganised" above), most of these
    solves are cheap: scenario `unify` of `examples/bench_suite.rs` (k
    Z-shaped connectors sharing one channel) takes about 0.17 s per
    transaction for k = 50, where it took 5.6 s, and about 1 s for k = 80.
    The cost still grows about as P³.

  `reg-unifying-all-pairs` (24 connectors) in
  `tests/golden/e2e_fuzz_regressions.txt` checks this stage against the C++.
- **Fuzz scene s1-123.** The C++ crashes on this scene (iterator-erased in
  `HyperedgeImprover::execute`). It used to send the port into the case
  above: in every later transaction a self-loop hyperedge tree edge
  (see "Infinite loops upstream") tripled a connector's display route. The
  free segments in one region went 2, 8, 26, 80, 242, and the fifth
  transaction would have needed 29,162 solves (more than a day). The port now
  skips such hyperedges, and the scene runs in about 3 s.
  `reg-improver-junction-self-loop-rewrite` checks that 20 further empty
  transactions grow routes, connectors and junctions by the same amount each
  time. The fixed amount comes from the faithful junction-adding behaviour
  above.

Overriding `shouldContinueTransactionWithProgress()` (JavaScript API) does
not shorten these cases: upstream polls its continuation hook during nudging and
route search but acts on a stop request only during crossing improvement.
Run such scenes in a worker if the thread must stay responsive.
