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
| `src/wasm_api.rs`, `index.cjs`, `index.d.ts` | The JavaScript-facing `AvoidRouter`, a string-id adapter over the core with no routing logic of its own. |
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
| `tests/wasm_api_fixtures.rs`, `tests/wasm_api.rs`, `test/*.cjs` | The fixtures again through the string-id wrapper, wrapper API contracts, and the built WASM package from Node. |
| `tests/robustness.rs`, `tests/core_findings.rs` | Misuse of the public API must not panic or hang. |
| `tools/audit/inventory.py --check` | Every upstream item is accounted for, and every `// upstream:` comment names something that exists. |
| `tools/wasm/run_wasm_tests.sh` | The integration tests on `wasm32-wasip1`, where `usize` is 32 bits. |

Useful commands:

```sh
tools/fixtures/run_all.sh            # status table for every fixture
tools/fixtures/verify_roundtrip.sh   # prove the op scripts against the C++
ORACLE_ALLOC=bump tools/oracle/build.sh   # rebuild upstream, regenerate tests/golden-bump
python3 tools/audit/inventory.py     # regenerate docs/PORTING_INVENTORY.md
```

The oracle scripts need `g++` and the vendored upstream source, which is not
checked in (`vendor/adaptagrams` is a checkout of the Adaptagrams repository
at the commit above).

## Where the port differs from upstream

The port reproduces upstream wherever upstream's behaviour is defined. The
differences are confined to the following.

- **Not ported.** SVG and debug output (`outputInstanceToSVG`,
  `outputDiagramSVG`, `outputCode`, the debug handler), timers, dead `#if 0`
  code and functions that upstream declares but never defines. The inventory
  lists each one with its reason.
- **Undefined behaviour upstream.** Where upstream reads freed memory,
  dereferences null or recurses without bound, the port does something
  defined instead and never panics. The cases are listed under "Public-API
  misuse" and "Dangling handles" in `src/avoid/router.rs`. The main ones:
  - A `pathNext` link to a deleted vertex reads as null.
  - Using a handle after deleting its object is a no-op.
  - A registered hyperedge with fewer than three distinct terminals, or one
    containing a cycle, is skipped.
  - With transactions off, a `processTransaction()` nested inside another is
    a no-op.
- **Asserts.** `COLA_ASSERT` is `debug_assert!`, except for asserts that
  upstream itself violates on valid input; those are not ported, so that debug
  builds do not panic where upstream's release build works. Each site cites
  its evidence.
- **Callbacks.** A connector callback is a closure that receives the
  connector handle and has no access to the router. The JavaScript API
  reports the connectors updated by the last transaction instead.
- **Uncaught exceptions.** Where upstream's VPSC solver would throw an
  exception nothing catches, the port leaves that nudging region unchanged.
