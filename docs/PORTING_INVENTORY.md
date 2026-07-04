# libavoid Porting Inventory

This inventory satisfies the Milestone 0 roadmap gate.  Status values are the roadmap values: `translated`, `blocked-by-api`, `blocked-by-algorithm`, `debug-only`, and `not-applicable-to-js-wrapper`.

Upstream source of truth: `vendor/adaptagrams/cola/libavoid`.

## Parity Test Convention

- Rust unit tests live in `src/tests.rs`.
- Node/WASM smoke and API contract tests live in `test/node-smoke.cjs`.
- Translated fixture tests should cite the upstream fixture name in the Rust or Node test name, or in the assertion comment when a fixture is split across several focused API tests.
- Route equality accepts exact endpoint equality and segment-level geometric predicates for obstacle avoidance unless the upstream fixture asserts exact checkpoint or display-route coordinates.
- Numeric tolerance for geometry and route predicates is `EPS = 1e-9` in Rust unless a fixture records a broader upstream-compatible tolerance.

## Public Header Inventory

| Header | Upstream public surface | Current Rust/WASM status |
| --- | --- | --- |
| `router.h` | Router flags, parameters, options, transactions, object lifecycle, clusters, topology diagnostics. | Partially translated: flags/options/parameters including upstream constructor defaults and negative-value sensible activation values, object IDs, queued shape/junction/connector/cluster/pin-deletion lifecycle actions, clusters, diagnostics, settings-change route invalidation, penalty-aware sequential transaction rerouting with `hateCrossings` reroute ordering, and JS-style updated-connector transaction reporting exist. Full action queue, visibility graph lifecycle, progress/cancellation, topology addon, and complete reroute selection remain incomplete. |
| `connector.h` | `ConnType`, `Checkpoint`, `ConnRef` endpoints, routing type, route/display route, callbacks, checkpoints, fixed routes, splitting, pin-point introspection, crossing utilities. | Partially translated: routing type, checkpoints, fixed routes, display route, repaint state, updated-connector transaction reporting, penalty-aware `routeAll` ordering, splitting, hate-crossings, and pin-point introspection exist. Native-style callbacks and full upstream route generation/post-processing remain incomplete. |
| `connend.h` | Direction flags, `ConnEndType`, point/shape-pin/junction endpoints, active pin assignment, possible pin points. | Partially translated: JS ID wrapper exposes direction and `ConnEndType` constants, point direction flags, shape-pin endpoints, pin-class endpoint construction and mutation with upstream shape class-ID validation, valid missing classes retained as anchor-position class endpoints, junction endpoints, junction-pin-class endpoint construction and mutation, resolved endpoint positions, endpoint directions, possible destination pin points for shape pins, junction pins, and plain junction centre pins, class endpoint reassignment when an active pin is removed, shape/junction pins are added/replaced, or pin cost/exclusivity changes, and shape/junction deletion detachment to manual point endpoints. Full upstream active-pin lifecycle remains simplified. |
| `shape.h` | `ShapeRef`, polygon, position, pin transforms, attached connector movement. | Partially translated: rectangular and polygon shapes, position, movement, deletion, delete-time ConnEnd detachment, pin transforms, and polygon-aware clearance obstacles exist. Full upstream visibility integration remains incomplete. |
| `connectionpin.h` | `ShapeConnectionPin`, class IDs, proportional/absolute offsets, inside offset, visibility, cost, exclusivity, containing object. | Partially translated: shape and junction pin position/direction/cost/exclusivity/ids/class selection exists, absolute offsets are shape-relative, boundary `insideOffset` handling and automatic directions follow upstream stored-offset rules, invalid shape-pin coordinate inputs are rejected without replacing existing pins, class `0` is rejected for shape pins, string-specific convenience pins use `CONNECTIONPIN_UNSET` outside class-based `ConnEnd` construction, class candidate enumeration filters occupied exclusive pins, class-based endpoint setters and shape/junction pin add/replace choose by cost/exclusivity with `portDirectionPenalty` using the upstream `assignPinVisibilityTo()` rotational-angle ranges, and pin cost/exclusivity mutations queue class endpoint reselection and invalidation transactionally. Full visibility graph assignment remains incomplete. |
| `junction.h` | `JunctionRef`, position, fixed/recommended position, preferred orthogonal dimension, attached connector movement. | Partially translated: junction endpoints, default non-exclusive centre connection pins, junction connection pins with cost/exclusivity selection, fixed/recommended position, preferred dimension including direction-based pin cost updates, split/merge helpers, movement, and delete-time ConnEnd detachment exist. Hyperedge tree improvement remains incomplete. |
| `viscluster.h` | `ClusterRef`, polygon, rectangular polygon, lifecycle. | Milestone 5 translated: cluster lifecycle, polygon/rectangle construction and update, polygon/rectangular-polygon/bounding-box APIs, outside-to-outside avoidance, inside-to-inside containment, explicit inside/outside boundary crossing, and `clusterCrossingPenalty` enable/disable behavior exist. Full upstream visibility graph internals remain tracked with the broader routing algorithm work. |
| `geomtypes.h` | `Point`, `Box`, `PolygonInterface`, `Edge`, `Polygon`, `PolyLine`, `ReferencingPolygon`, `Rectangle`. | Milestone 1 translated for routing foundations: core point, rect, polygon/polyline storage, bounding box, offset bounding box, offset polygon, translation, containment, and edge iteration behavior exist. Curved polylines and full referencing polygon semantics remain outside the current routing foundation. |
| `geometry.h` | Distance, total length, angles, segment intersection, containment, region checks, projection, rotational angle. | Milestone 1 translated for routing foundations: Euclidean/Manhattan distance, total length, angle, projection, rotational angle, colinearity, point-on-line, segment intersection, shape-boundary intersection, containment, valid-region, corner-side, and route predicates exist. Further edge cases should be added as upstream graph/path fixtures are translated. |
| `vertices.h` | `VertID`, `VertInf`, visibility direction helpers, vertex lists, contains maps. | Milestone 2 translated for routing foundations: upstream-style `VertID` identity/order/property semantics, connection vertices, shape vertices, vertex records, visibility-direction storage and filtering, vertex-list category counts/lookups, graph-level contains maps, and visibility graph tests exist. Tree-root/path-linked-list internals remain tracked with later path-generation and hyperedge parity. |
| `graph.h` | `EdgeInf`, `EdgeList`, blocker/visibility metadata, connector alerts. | Milestone 2 translated for routing foundations: visibility graph edges, visible/invisible state transitions, blocker metadata, disabled edges, removal, adjacency filtering, upstream-style edge state transitions, edge ID/point accessors, other-vertex lookup, existing-edge lookup, and dummy connection detection exist. Hyperedge metadata, MTST distance, linked `EdgeList`, and connector alert behavior remain tracked with later routing parity. |
| `visibility.h` | Dynamic vertex visibility routines. | Milestone 2 translated for routing foundations: static visibility graph generation, invisible blocked edges, containment tracking, and router cache invalidation/regeneration tests exist for shape and connector lifecycle changes. Full upstream Lee sweep optimisation remains tracked with later routing parity. |
| `scanline.h` | Scanline events, shift segments, orthogonal channel info, checkpoint cache. | Milestone 2 translated for routing foundations: event ordering, nearest obstacle limit rules, upstream-style scanline node neighbour queries (`firstObstacleAbove/Below`, `firstPointAbove/Below`, `findFirstPointAboveAndBelow`, and `isInsideShape`), inline-edge visibility behavior, and scanline tests exist for graph support. Full `buildOrthogonalChannelInfo`, shift-segment mutation, and checkpoint cache behavior remain tracked with display-route and checkpoint routing work. |
| `orthogonal.h` | Static orthogonal visibility graph generation and route improvement. | Partially translated: orthogonal obstacle-avoiding search exists with segment, crossing, fixed-shared-path, reverse-direction, cluster-crossing, and shape-buffer route-cost inputs, with upstream-style bend and double-back segment costing, upstream orthogonal neighbour rotation ordering, upstream A* timestamp tie-breaking, free endpoint direction visibility from containing shapes to their first boundary, and exact fractional endpoint coordinates preserved in generated routes. Full upstream connection-pin dummy helper graph semantics, route improvement, and nudging remain incomplete. |
| `makepath.h` | A* path search over visibility graph. | Partially translated: simplified shortest-path search exists, orthogonal costs use stored routing parameters including crossing, fixed-shared-path, and reverse-direction penalties, shape pin-class selection uses a focused port-direction penalty slice, and polyline routing now uses the shared static visibility graph foundation with endpoint direction filtering plus a focused segment/angle/crossing/fixed-shared/reverse transition-cost slice, including upstream-style scaled polyline angle cost and double-back segment cost. Orthogonal search also copies upstream A* epsilon/timestamp tie-breaking and `rotationLessThan()` turn ordering. Full upstream cost model, reroute selection, and rubber-band behavior remain incomplete. |
| `hyperedge.h` | `HyperedgeRerouter` and hyperedge registration/rerouting. | `blocked-by-algorithm`: hyperedge routing is Milestone 7. |
| `hyperedgetree.h` | Hyperedge tree nodes/edges, junction/connector rewriting. | `blocked-by-algorithm`: hyperedge routing is Milestone 7. |
| `hyperedgeimprover.h` | Hyperedge improvement, segment shifting, junction creation/deletion. | `blocked-by-algorithm`: hyperedge routing is Milestone 7. |
| `mtst.h` | Minimum terminal spanning tree support for hyperedges. | `blocked-by-algorithm`: hyperedge routing is Milestone 7. |
| `vpsc.h` | Variable placement with separation constraints. | `blocked-by-algorithm`: VPSC is Milestone 8. |
| `actioninfo.h` | Action queue records and queued connector endpoint updates. | Partially translated: queued movement/deletion/endpoint/settings actions, pin deletion, coalesced pin-change invalidation, and last-updated connector reporting exist. Full upstream action queue and progress/cancellation behavior remain incomplete. |

## Upstream Fixture Index

| Fixture | Status | Rationale / next evidence |
| --- | --- | --- |
| `2junctions.cpp` | translated | Covered by junction endpoint and merge tests in `src/tests.rs` and `test/node-smoke.cjs`. |
| `buildOrthogonalChannelInfo1.cpp` | blocked-by-algorithm | Needs full upstream scanline channel construction and nudging pipeline. |
| `checkpointNudging1.cpp` | blocked-by-algorithm | Depends on display-route nudging, Milestone 6. |
| `checkpointNudging2.cpp` | blocked-by-algorithm | Depends on display-route nudging, Milestone 6. |
| `checkpointNudging3.cpp` | blocked-by-algorithm | Depends on display-route nudging, Milestone 6. |
| `checkpoints01.cpp` | translated | Checkpoint APIs and route-through-checkpoint behavior are covered by Rust/Node tests. |
| `checkpoints02.cpp` | translated | Checkpoint APIs and direction constraints are covered by Rust/Node tests. |
| `checkpoints03.cpp` | translated | Checkpoint APIs and invalid input handling are covered by Rust/Node tests. |
| `complex.cpp` | blocked-by-algorithm | Broad route parity fixture; depends on complete graph/path/nudging parity. |
| `connectionpin01.cpp` | translated | Pin position, visibility, inside offset, cost, exclusivity, and class selection are covered by Rust/Node tests. |
| `connectionpin02.cpp` | translated | Pin transform and connector endpoint semantics are covered by Rust/Node tests. |
| `connectionpin03.cpp` | translated | Multi-pin class and exclusivity behavior is covered by Rust/Node tests. |
| `connendmove.cpp` | translated | Transactional shape/junction/endpoint movement is covered by Rust/Node tests. |
| `corneroverlap01.cpp` | blocked-by-algorithm | Depends on display-route overlap/nudging parity, Milestone 6. |
| `endlessLoop01.cpp` | blocked-by-algorithm | Regression requires upstream search/nudging parity to be meaningful. |
| `example.cpp` | translated | Basic shapes, connectors, routing parameters, and route output are covered by Rust/Node smoke tests. |
| `finalSegmentNudging1.cpp` | blocked-by-algorithm | Depends on final-segment nudging, Milestone 6. |
| `finalSegmentNudging2.cpp` | blocked-by-algorithm | Depends on final-segment nudging, Milestone 6. |
| `finalSegmentNudging3.cpp` | blocked-by-algorithm | Depends on final-segment nudging, Milestone 6. |
| `forwardFlowingConnectors01.cpp` | translated | Covered by `upstream_forward_flowing_connectors01_keeps_connector6_short`, including the upstream shape-pin class scene, crossing and reverse-direction penalty settings, and connector 6 display-route segment-count assertion. |
| `freeFloatingDirection01.cpp` | translated | `ConnEnd(Point, ConnDirFlags)`-style free endpoint direction constraints are covered by Rust/Node tests. |
| `hola01.cpp` | blocked-by-algorithm | Depends on complete orthogonal route search and post-processing parity. |
| `hyperedge01.cpp` | blocked-by-algorithm | Hyperedge routing is Milestone 7. |
| `hyperedge02.cpp` | blocked-by-algorithm | Hyperedge routing is Milestone 7. |
| `hyperedgeLoop1.cpp` | blocked-by-algorithm | Hyperedge routing is Milestone 7. |
| `hyperedgeRerouting01.cpp` | blocked-by-algorithm | Hyperedge routing is Milestone 7. |
| `improveHyperedge01.cpp` | blocked-by-algorithm | Hyperedge improvement is Milestone 7. |
| `improveHyperedge02.cpp` | blocked-by-algorithm | Hyperedge improvement is Milestone 7. |
| `improveHyperedge03.cpp` | blocked-by-algorithm | Hyperedge improvement is Milestone 7. |
| `improveHyperedge04.cpp` | blocked-by-algorithm | Hyperedge improvement is Milestone 7. |
| `improveHyperedge05.cpp` | blocked-by-algorithm | Hyperedge improvement is Milestone 7. |
| `improveHyperedge06.cpp` | blocked-by-algorithm | Hyperedge improvement is Milestone 7. |
| `infinity.cpp` | blocked-by-algorithm | Requires full route search robustness parity. |
| `inline.cpp` | translated | Covered by `upstream_inline_point_direction_fixture_routes_orthogonally`, including free point endpoints inside a shape, endpoint direction flags, and orthogonal visibility from containing shapes to their first boundary. |
| `inlineOverlap09.cpp` | blocked-by-algorithm | Depends on display-route overlap/nudging parity, Milestone 6. |
| `inlineOverlap10.cpp` | blocked-by-algorithm | Depends on display-route overlap/nudging parity, Milestone 6. |
| `inlineOverlap11.cpp` | blocked-by-algorithm | Depends on display-route overlap/nudging parity, Milestone 6. |
| `inlineShapes.cpp` | blocked-by-algorithm | Depends on full polygon/visibility graph path generation. |
| `inlineoverlap01.cpp` | blocked-by-algorithm | Depends on display-route overlap/nudging parity, Milestone 6. |
| `inlineoverlap02.cpp` | blocked-by-algorithm | Depends on display-route overlap/nudging parity, Milestone 6. |
| `inlineoverlap03.cpp` | blocked-by-algorithm | Depends on display-route overlap/nudging parity, Milestone 6. |
| `inlineoverlap04.cpp` | blocked-by-algorithm | Depends on display-route overlap/nudging parity, Milestone 6. |
| `inlineoverlap05.cpp` | blocked-by-algorithm | Depends on display-route overlap/nudging parity, Milestone 6. |
| `inlineoverlap06.cpp` | blocked-by-algorithm | Depends on display-route overlap/nudging parity, Milestone 6. |
| `inlineoverlap07.cpp` | blocked-by-algorithm | Depends on display-route overlap/nudging parity, Milestone 6. |
| `inlineoverlap08.cpp` | blocked-by-algorithm | Depends on display-route overlap/nudging parity, Milestone 6. |
| `junction01.cpp` | translated | Junction endpoint, movement, attached connector, fixed/recommended-position behavior is covered by Rust/Node tests. |
| `junction02.cpp` | translated | Junction endpoint and lifecycle behavior is covered by Rust/Node tests. |
| `junction03.cpp` | translated | Junction merge/remove behavior is covered by Rust/Node tests. |
| `junction04.cpp` | translated | Junction preferred orthogonal dimension and recommended position behavior is covered by Rust/Node tests. |
| `latesetup.cpp` | blocked-by-algorithm | Depends on complete graph invalidation and late object setup parity. |
| `lineSegWrapperCrash1.cpp` | blocked-by-algorithm | Regression requires upstream segment wrapper/path parity. |
| `lineSegWrapperCrash2.cpp` | blocked-by-algorithm | Regression requires upstream segment wrapper/path parity. |
| `lineSegWrapperCrash3.cpp` | blocked-by-algorithm | Regression requires upstream segment wrapper/path parity. |
| `lineSegWrapperCrash4.cpp` | blocked-by-algorithm | Regression requires upstream segment wrapper/path parity. |
| `lineSegWrapperCrash5.cpp` | blocked-by-algorithm | Regression requires upstream segment wrapper/path parity. |
| `lineSegWrapperCrash6.cpp` | blocked-by-algorithm | Regression requires upstream segment wrapper/path parity. |
| `lineSegWrapperCrash7.cpp` | blocked-by-algorithm | Regression requires upstream segment wrapper/path parity. |
| `lineSegWrapperCrash8.cpp` | blocked-by-algorithm | Regression requires upstream segment wrapper/path parity. |
| `multiconnact.cpp` | translated | Multi-connector action/lifecycle behavior is covered by Rust/Node transaction tests. |
| `node1.cpp` | blocked-by-algorithm | Depends on complete route search parity for generated graph cases. |
| `nudgeCrossing01.cpp` | blocked-by-algorithm | Depends on nudging and crossing-penalty route improvement, Milestone 6. |
| `nudgeintobug.cpp` | blocked-by-algorithm | Depends on nudging, Milestone 6. |
| `nudgeold.cpp` | blocked-by-algorithm | Depends on nudging, Milestone 6. |
| `nudgingSkipsCheckpoint01.cpp` | blocked-by-algorithm | Depends on nudging, Milestone 6. |
| `nudgingSkipsCheckpoint02.cpp` | blocked-by-algorithm | Depends on nudging, Milestone 6. |
| `orderassertion.cpp` | blocked-by-algorithm | Depends on upstream orthogonal ordering and route improvement. |
| `orthordering01.cpp` | blocked-by-algorithm | Depends on orthogonal route ordering/nudging parity, Milestone 6. |
| `orthordering02.cpp` | blocked-by-algorithm | Depends on orthogonal route ordering/nudging parity, Milestone 6. |
| `overlappingRects.cpp` | translated | Rectangular obstacle overlap and route avoidance are covered by Rust tests. |
| `penaltyRerouting01.cpp` | translated | Covered by `upstream_penalty_rerouting01_has_no_crossings`, including the upstream fixed-route scene, crossing penalty settings, automatic orthogonal rerouting for the two unfixed connectors, and `existsCrossings()` parity for the fixture outcome. Broader crossing-set candidate selection remains tracked under full reroute parity. |
| `performance01.cpp` | debug-only | Performance benchmark, not a JS/WASM compatibility contract. |
| `reallyslowrouting.cpp` | debug-only | Performance/stress benchmark, not a JS/WASM compatibility contract. |
| `removeJunctions01.cpp` | translated | `removeJunctionAndMergeConnectors` is covered by Rust/Node tests. |
| `restrictedNudging.cpp` | blocked-by-algorithm | Depends on restricted nudging, Milestone 6. |
| `slowrouting.cpp` | debug-only | Performance/stress benchmark, not a JS/WASM compatibility contract. |
| `tjunct.cpp` | blocked-by-algorithm | Depends on full hyperedge/junction topology behavior. |
| `treeRootCrash01.cpp` | blocked-by-algorithm | Depends on upstream graph/tree-root route search parity. |
| `treeRootCrash02.cpp` | blocked-by-algorithm | Depends on upstream graph/tree-root route search parity. |
| `unsatisfiableRangeAssertion.cpp` | blocked-by-algorithm | Depends on complete route search, VPSC, and assertion parity. |
| `validPaths01.cpp` | translated | Valid orthogonal paths and obstacle avoidance are covered by Rust/Node route tests. |
| `validPaths02.cpp` | translated | Covered by `upstream_valid_paths02_has_no_invalid_orthogonal_paths`, including proportional shape pins, endpoint direction metadata, exact fractional endpoint preservation, and invalid orthogonal path diagnostics. Full connection-pin dummy helper graph parity remains tracked under broader path-generation work. |
| `vertlineassertion.cpp` | blocked-by-algorithm | Regression requires upstream scanline/path parity. |
