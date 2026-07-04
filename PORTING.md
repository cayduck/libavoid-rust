# libavoid C++ to Rust/WASM Porting Map

This repository vendors the upstream Adaptagrams source at:

- `vendor/adaptagrams/cola/libavoid`

The Rust implementation must be ported from that source, not invented from memory.

## Public API Parity Targets

- `libavoid.h`: umbrella include, public surface inventory
- `router.h` / `router.cpp`: `Router`, flags, transactions, routing parameters/options, object lifecycle
- `shape.h` / `shape.cpp`: `ShapeRef`, polygon obstacles, shape movement, pin transforms
- `connector.h` / `connector.cpp`: `ConnRef`, connector type, endpoints, checkpoints, fixed routes, repaint state
- `connend.h` / `connend.cpp`: `ConnEnd`, point/shape-pin/junction endpoints, direction flags
- `connectionpin.h` / `connectionpin.cpp`: `ShapeConnectionPin`, proportional/absolute offsets, visibility, exclusivity, cost
- `junction.h` / `junction.cpp`: `JunctionRef` endpoints and hyperedge support
- `geometry.h`, `geomtypes.h`, `vertices.h`: `Point`, `Polygon`, `Box`, vertices, segment utilities
- `visibility.*`, `scanline.*`, `graph.*`: visibility graph maintenance
- `orthogonal.*`, `makepath.*`: orthogonal route generation and post-processing
- `hyperedge.*`, `hyperedgetree.*`, `hyperedgeimprover.*`, `mtst.*`: hyperedge routing

## Current Rust Status

Implemented as an initial slice:

- `AvoidRouter` exported through `wasm-bindgen`
- router object-id, connector-type, and shape-containment queries
- rectangular shape obstacles
- shape and junction obstacle add/remove invalidates connector routes
- cluster objects matching `ClusterRef` lifecycle, polygon/rectangle
  construction and update, polygon access, outside-to-outside boundary
  avoidance, inside-to-inside containment, and explicit inside/outside
  boundary crossing, with `clusterCrossingPenalty` controlling whether cluster
  boundaries participate in routing
- free-point connectors, including `ConnEnd(Point, ConnDirFlags)`-style endpoint direction flags
- shape connection pins, including shape-relative absolute offsets, `ids()`-style
  containing-object/class metadata, and boundary `insideOffset` handling for
  automatic visibility
- shape connection pin position/direction introspection plus connection-cost and
  exclusivity mutation with queued, pin-specific connector invalidation
- shape connection pin transforms matching `ShapeRef::transformConnectionPinPositions()`
- pin-to-pin c
- onnectors and source/destination endpoint mutation to shape pins
  and shape pin classes
- class-based shape/junction pin endpoints can reassign to another available
  pin in the same class when their active pin is removed or pin
  cost/exclusivity changes
- transaction-queued shape/junction movement, connector endpoint updates, and
  shape/junction/connector/pin deletion matching the `processTransaction()`
  model
- shape/junction deletion detaches attached `ConnEnd`s to manual point
  endpoints at their last resolved positions, matching
  `Obstacle::makeInactive()` / `ConnEnd::disconnect()`
- basic junction objects and junction-attached connector endpoints
- junction connection pins with class-based connector endpoints, class-based
  source/destination endpoint mutation, resolved
  position/direction/cost/exclusivity introspection, mutation with queued,
  pin-specific connector invalidation, movement, default non-exclusive centre
  pins, and dependent connector deletion
- shape/junction attached connector introspection
- shape-to-shape attachment introspection matching `Router::attachedShapes()`
  with `runningTo`, `runningFrom`, and `runningToAndFrom` filters
- connector endpoint introspection matching `ConnRef::endpointConnEnds()`,
  including resolved position and direction metadata for point, shape-pin, and
  junction endpoints
- connector routing type getter/setter with `Router::validConnType()` normalization
  and `ConnRef::setRoutingType()` transaction/immediate reroute behavior
- connector hate-crossing flag matching `ConnRef::setHateCrossings()` /
  `doesHateCrossings()`
- connector route invalidation, repaint state, and display-route retrieval matching
  `ConnRef::makePathInvalid()`, `needsRepaint()`, and `displayRoute()`
- JS callback-equivalent transaction reporting via last updated connector IDs
  after `Router::processTransaction()` reroutes invalid paths
- destination shape-pin point introspection for `ConnRef::possibleDstPinPoints()`,
  including class-based candidate point enumeration for class endpoints with
  upstream-style exclusive-pin availability filtering
- invalid orthogonal display-route diagnostics matching
  `Router::existsInvalidOrthogonalPaths()`
- strict display-route crossing count for `Router::existsCrossings()`
- shared orthogonal segment diagnostics matching
  `Router::existsOrthogonalSegmentOverlap()`
- fixed shared segment diagnostics matching
  `Router::existsOrthogonalFixedSegmentOverlap()`
- touching/crossing orthogonal path diagnostics matching
  `Router::existsOrthogonalTouchingPaths()`
- connector splitting matching `ConnRef::splitAtSegment()` with explicit JS string IDs
- two-connector junction removal/merge matching `JunctionRef::removeJunctionAndMergeConnectors()`
- junction preferred orthogonal dimension matching `JunctionRef::preferOrthogonalDimension()`
  including direction-based junction pin connection-cost updates
- junction fixed-position and recommended-position state matching
  `JunctionRef::setPositionFixed()`, `positionFixed()`,
  `recommendedPosition()`, and `setRecommendedPosition()`
- connector checkpoints with arrival/departure direction constraints
- fixed connector routes
- `Router::registerSettingsChange()`-style transaction reporting for routing
  parameters/options, fixed-route state, and junction fixed-position state
- basic polyline visibility routing around obstacle vertices
- upstream routing option indices, defaults, and getters/setters
- upstream routing parameter getters/setters plus `Router::setRoutingPenalty()` alias
- transaction-style shape movement, junction movement, connector endpoint updates,
  cluster polygon updates/removal, and `Router::setTransactionUse()`
  queue-preserving toggle behavior
- orthogonal obstacle-avoiding route search
- generated Node/WASM package and smoke tests

Not complete:

- upstream visibility graph algorithm
- full upstream polyline routing behavior, including rubber-band routing and
  invisibility graph maintenance
- full polygon obstacles
- full upstream cluster visibility graph integration
- hyperedge routing
- full junction/hyperedge tree improvement
- complete transactions, including the full upstream action queue, progress
  reporting/cancellation, and callback hooks beyond the current updated-connector
  transaction report
- nudging and shared-path ordering
- behavior for all upstream routing parameters/options
- compatibility assertions against upstream test fixtures

## Porting Rule

Each new Rust behavior should name the corresponding upstream source files in commits or notes and add tests that are either translated from `vendor/adaptagrams/cola/libavoid/tests` or directly assert documented public API behavior from the headers.
