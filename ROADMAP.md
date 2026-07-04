# libavoid Rust/WASM Reimplementation Roadmap

This document is the development plan for completing the project goal: reimplement upstream `libavoid` in Rust and expose it to JavaScript through WASM with behavior-compatible routing.

The current repository is a useful early slice, not a complete port. The next work should prioritize upstream algorithm parity before expanding convenience APIs. Each milestone below must be backed by translated upstream tests or direct public API contract tests.

## Ground Rules

- Treat `vendor/adaptagrams/cola/libavoid` as the source of truth.
- Port from upstream files deliberately; do not replace missing behavior with new invented routing semantics.
- Keep the JavaScript API ergonomic, but preserve upstream concepts and behavior under the wrapper.
- Every new behavior must cite the upstream file or fixture it came from in code comments, test names, or commit notes.
- Prefer small compatibility fixtures over broad visual assertions.
- Do not call the port complete until upstream `libavoid/tests/*.cpp` coverage has either been translated or explicitly classified as out of JS/WASM scope.

## Current Baseline

Estimated total progress toward complete libavoid parity: about 38%.

Already useful:

- Rust/WASM package builds for Node.
- Basic router, shape, connector, junction, cluster, pin, endpoint, fixed-route, checkpoint, and transaction APIs exist.
- Basic orthogonal and polyline obstacle-avoiding routing exists.
- Rust tests and Node smoke tests pass.

Known gap pattern:

- Public API objects are represented, but upstream internals are only partially ported.
- Visibility graph, route generation, route improvement, nudging, hyperedges, VPSC, and cluster behavior are not yet complete.
- Only a small number of upstream compatibility tests have been translated.

## Milestone 0: Port Inventory And Test Harness

Goal: make progress measurable before changing core algorithms.

Tasks:

- Build a symbol inventory for upstream headers: `router.h`, `connector.h`, `connend.h`, `shape.h`, `connectionpin.h`, `junction.h`, `viscluster.h`, `geomtypes.h`, `geometry.h`, `vertices.h`, `graph.h`, `visibility.h`, `scanline.h`, `orthogonal.h`, `makepath.h`, `hyperedge.h`, `hyperedgetree.h`, `hyperedgeimprover.h`, `mtst.h`, `vpsc.h`, and `actioninfo.h`.
- Add a fixture index for every file in `vendor/adaptagrams/cola/libavoid/tests`.
- Classify each upstream fixture as: `translated`, `blocked-by-api`, `blocked-by-algorithm`, `debug-only`, or `not-applicable-to-js-wrapper`.
- Add a repeatable parity test convention for translated fixtures.
- Record accepted numeric tolerances for route equality and geometric checks.

Acceptance gate:

- A machine-readable or table-style inventory exists.
- Every upstream fixture has a status.
- `cargo test` and `npm test` remain green.

## Milestone 1: Geometry, Polygon, And Vertex Foundations

Goal: make low-level geometry match upstream before relying on it for routing.

Primary upstream files:

- `geomtypes.h`, `geomtypes.cpp`
- `geometry.h`, `geometry.cpp`
- `vertices.h`, `vertices.cpp`
- `obstacle.h`, `obstacle.cpp`

Tasks:

- Complete `Point`, `Polygon`, `PolyLine`, `Rectangle`, and bounding-box behavior needed by routing.
- Port segment intersection, containment, rotational angle, distance, total length, and polygon edge behavior with upstream-compatible border handling.
- Replace any bounding-box-only polygon routing paths with polygon-aware predicates where upstream expects real polygon geometry.
- Expand obstacle representation so rectangular and polygon shapes use the same route-blocking logic.

Acceptance gate:

- Geometry tests cover upstream documented edge cases and known fixture shapes.
- Polygon obstacles block routes by polygon boundary, not only by bounding box.
- Existing JS wrapper polygon APIs keep their current behavior.

Status: complete for the current routing foundation.  The Rust geometry layer now includes upstream-style distance, total-length, angle, projection, rotational-angle, region, corner-side, containment, segment-intersection, bounding-box, and offset-polygon helpers.  Shape routing uses polygon offsets for clearance rather than replacing non-rectangular shapes with bounding boxes. Full upstream visibility graph integration remains Milestone 2 work.

## Milestone 2: Visibility Graph And Scanline Parity

Goal: port the graph infrastructure that upstream routing depends on.

Primary upstream files:

- `graph.h`, `graph.cpp`
- `visibility.h`, `visibility.cpp`
- `scanline.h`, `scanline.cpp`
- `vertices.h`, `vertices.cpp`

Tasks:

- Port vertex identity, connection vertices, shape vertices, visibility edges, invisibility edges, and contains maps.
- Implement static visibility graph generation and invalidation behavior.
- Port scanline event ordering and obstacle interaction rules.
- Preserve incremental update behavior for shape add, delete, and move.
- Add graph-level tests before using the graph for high-level routing.

Acceptance gate:

- Visibility graph tests prove edges are added, removed, hidden, and restored according to upstream rules.
- Shape lifecycle invalidates and regenerates graph state correctly.
- Existing basic routing tests still pass on top of the new graph.

## Milestone 3: Polyline And Orthogonal Path Generation

Goal: route using upstream-compatible path search instead of the current purpose-built simplified algorithms.

Primary upstream files:

- `makepath.h`, `makepath.cpp`
- `orthogonal.h`, `orthogonal.cpp`
- `connector.h`, `connector.cpp`
- `router.h`, `router.cpp`

Tasks:

- Port upstream shortest-path behavior, including cost calculation.
- Implement routing penalties that currently exist as stored settings but do not fully affect route choice.
- Complete checkpoint routing semantics.
- Preserve `ConnType` normalization and per-connector routing type behavior.
- Make polyline routing and orthogonal routing share upstream graph state where applicable.

Acceptance gate:

- Representative upstream fixtures for valid paths, checkpoints, inline routing, obstacle avoidance, and crossing penalties pass.
- Route cost changes when relevant routing parameters change.
- JS `routeConnector`, `displayConnectorRoute`, and `routeAll` outputs remain stable enough for consumers, with documented route differences where upstream parity requires changed paths.

## Milestone 4: Transactions, Actions, Callbacks, And Object Lifecycle

Goal: make object changes behave like upstream `Router::processTransaction()`.

Primary upstream files:

- `actioninfo.h`, `actioninfo.cpp`
- `router.h`, `router.cpp`
- `connector.h`, `connector.cpp`
- `shape.h`, `shape.cpp`
- `junction.h`, `junction.cpp`
- `viscluster.h`, `viscluster.cpp`

Tasks:

- Port the full action queue model for shape, connector, junction, cluster, and pin changes.
- Implement reroute flag propagation and affected-connector selection.
- Expose callback-equivalent behavior in a JS-appropriate way if needed.
- Port transaction progress/cancellation only if a stable JS API can represent it; otherwise classify it explicitly as non-applicable.
- Ensure deletion and queued actions cannot leave stale references or stale IDs.

Acceptance gate:

- Existing transaction tests pass.
- Additional tests cover queued add, delete, move, pin update, settings change, and connector endpoint update ordering.
- Upstream lifecycle fixtures are translated or classified.

## Milestone 5: Pins, ConnEnd Semantics, Junctions, And Clusters

Goal: complete public object semantics that affect routing endpoints and containment.

Primary upstream files:

- `connend.h`, `connend.cpp`
- `connectionpin.h`, `connectionpin.cpp`
- `junction.h`, `junction.cpp`
- `shape.h`, `shape.cpp`
- `viscluster.h`, `viscluster.cpp`

Tasks:

- Complete `ConnEnd` type, position, direction, shape, junction, and pin class behavior under the JS ID wrapper.
- Complete pin visibility assignment, automatic direction calculation, inside-offset behavior, exclusivity, and cost behavior.
- Complete junction pin behavior and recommended-position semantics as far as possible before hyperedge improvement.
- Complete cluster visibility and inside/outside crossing semantics.

Acceptance gate:

- Upstream connection pin, free-floating direction, junction, and cluster fixtures pass or are explicitly classified.
- Class-based pin selection and exclusivity match upstream in multi-connector cases.
- Cluster behavior is no longer described as basic or partial in docs.

## Milestone 6: Display Route Post-Processing And Nudging

Goal: match upstream display routes, not just raw shortest paths.

Primary upstream files:

- `connector.h`, `connector.cpp`
- `orthogonal.h`, `orthogonal.cpp`
- `router.h`, `router.cpp`
- `scanline.h`, `scanline.cpp`

Tasks:

- Port route simplification, centering, overlap detection, and nudging.
- Implement shared-path ordering and endpoint shared-path options.
- Make `existsOrthogonalSegmentOverlap`, `existsOrthogonalFixedSegmentOverlap`, `existsOrthogonalTouchingPaths`, and `existsInvalidOrthogonalPaths` validate the same conditions as upstream.
- Translate fixtures with names involving inline overlap, nudging, ordering, shared paths, and final segment nudging.

Acceptance gate:

- Display route fixtures pass with route geometry tolerances.
- Current diagnostic APIs match upstream fixture expectations.
- Routing options related to nudging visibly affect output routes.

## Milestone 7: Hyperedge Routing And Improvement

Goal: complete the most advanced upstream routing behavior.

Primary upstream files:

- `hyperedge.h`, `hyperedge.cpp`
- `hyperedgetree.h`, `hyperedgetree.cpp`
- `hyperedgeimprover.h`, `hyperedgeimprover.cpp`
- `mtst.h`, `mtst.cpp`
- `junction.h`, `junction.cpp`
- `router.h`, `router.cpp`

Tasks:

- Port hyperedge registration and terminal handling.
- Port minimum terminal spanning tree behavior.
- Port hyperedge tree construction and improvement.
- Implement junction movement, creation, and deletion during hyperedge improvement.
- Expose new/deleted object lists through a JS-safe API.

Acceptance gate:

- Upstream hyperedge and improve-hyperedge fixtures pass.
- `JunctionRef` recommended positions and fixed-position rules behave like upstream.
- Hyperedge-related routing options are no longer no-op settings.

## Milestone 8: VPSC And Topology-Adjacent Behavior

Goal: port the upstream constraint machinery used by libavoid route improvement.

Primary upstream files:

- `vpsc.h`, `vpsc.cpp`
- `orthogonal.h`, `orthogonal.cpp`
- `hyperedgeimprover.h`, `hyperedgeimprover.cpp`

Tasks:

- Port the incremental solver pieces that libavoid uses.
- Add focused tests for variable, constraint, block, and solver behavior.
- Integrate VPSC where upstream uses it for shifting and overlap resolution.

Acceptance gate:

- Solver unit tests pass.
- Nudging and hyperedge improvement use the ported solver rather than custom approximations.

## Milestone 9: JS/WASM API Completion And Compatibility Review

Goal: make the JS package the production-facing form of the completed port.

Tasks:

- Compare `index.d.ts`, `index.cjs`, and `src/router.rs` against the upstream public API inventory.
- Decide and document which upstream pointer/subclass/debug APIs are exposed, adapted, or out of scope for JS/WASM.
- Stabilize error behavior for missing IDs, invalid geometry, invalid enums, and malformed route inputs.
- Ensure package build output is reproducible.
- Add browser-target smoke tests if browser WASM support is part of release scope.

Acceptance gate:

- API inventory has no unexplained gaps.
- TypeScript declarations match runtime behavior.
- `npm test` and package smoke tests pass from a clean checkout.

## Milestone 10: Release Readiness

Goal: make a defensible claim of complete reimplementation.

Tasks:

- Translate or classify all upstream libavoid fixtures.
- Run the full Rust, Node, and WASM build/test suite.
- Update `README.md` and `PORTING.md` to remove early-slice language only when compatibility evidence supports it.
- Add license completeness checks before publishing.
- Prepare a compatibility report listing unsupported upstream APIs, if any.

Acceptance gate:

- All translated fixtures pass.
- All non-translated fixtures have written rationale.
- Documentation accurately describes completed and unsupported behavior.
- The package can be built, tested, packed, and consumed through JS.

## Working Order For The Next Development Sessions

1. Create the upstream API and fixture inventory from Milestone 0.
2. Translate the smallest representative fixture for each current gap category.
3. Finish geometry and polygon parity before touching high-level routing.
4. Replace the simplified visibility and scanline implementation with upstream graph behavior.
5. Replace the simplified route search with upstream path generation.
6. Only then work on nudging, hyperedges, and VPSC, because they depend on the earlier graph and path correctness.

## Progress Accounting

Use these rough weights when reporting progress:

- API and JS/WASM package: 15%
- Geometry, polygons, obstacles, vertices: 15%
- Visibility graph and scanline: 15%
- Polyline and orthogonal path generation: 20%
- Transactions and lifecycle: 10%
- Pins, junctions, clusters: 10%
- Nudging and display-route post-processing: 7%
- Hyperedges and VPSC: 8%

Current weighted estimate:

| Area | Weight | Estimated completion | Contribution |
| --- | ---: | ---: | ---: |
| API and JS/WASM package | 15% | 70% | 10.5% |
| Geometry, polygons, obstacles, vertices | 15% | 45% | 6.75% |
| Visibility graph and scanline | 15% | 15% | 2.25% |
| Polyline and orthogonal path generation | 20% | 25% | 5% |
| Transactions and lifecycle | 10% | 55% | 5.5% |
| Pins, junctions, clusters | 10% | 70% | 7% |
| Nudging and display-route post-processing | 7% | 20% | 1.4% |
| Hyperedges and VPSC | 8% | 0% | 0% |
| **Total** | **100%** |  | **38.4%** |

The current implementation has meaningful progress in API/package, transactions/lifecycle, pins/junctions/clusters, geometry, and simplified routing, but very limited progress in the upstream graph, path, nudging, hyperedge, and VPSC categories. As a cross-check, the fixture inventory currently marks 19 of 80 non-debug upstream fixtures as translated, or about 24%, which is lower because many JS/API compatibility tests are not one-to-one fixture translations. The weighted progress estimate is therefore about 38%, with fixture parity still closer to 24%.
