# libavoid-rust

`libavoid-rust` is a Rust and WebAssembly connector-routing package for JavaScript. The goal is a complete Rust/WASM conversion of the original C++ libavoid API and behavior.

Current status: early implementation. The upstream C++ source is vendored at `vendor/adaptagrams/cola/libavoid` and the porting map is tracked in `PORTING.md`. The Rust code currently provides a WASM-backed orthogonal router with rectangular and polygon obstacle handling, cluster boundary and containment support, graph-backed polyline visibility routing with endpoint direction filtering and focused transition-cost penalties, router object/type/shape-containment queries, shape and junction attached-connector introspection, shape-to-shape attachment introspection, free endpoints with `ConnEnd(Point, ConnDirFlags)`-style direction constraints, shape connection pins with position/direction introspection plus cost/exclusivity mutation and shape-transform support, shape and junction pin-class endpoint mutation, junction endpoints, default junction centre pins, junction fixed/recommended-position state, junction orthogonal-dimension preferences, connector endpoint introspection, destination pin-point introspection for shape pins, junction pins, plain junction centre pins, and class-based candidate enumeration, connector repaint/display-route state, connector routing type normalization and hate-crossing flags, connector splitting, two-connector junction merging, connector checkpoints, fixed connector routes, obstacle lifecycle route invalidation, invalid orthogonal display-route, strict crossing, shared orthogonal segment, fixed shared segment, and orthogonal touching-path diagnostics, upstream-style direction/routing constants, options, parameters, and penalty setters that affect focused orthogonal route choice, transaction-based shape/junction movement, connector endpoint updates including shape-pin endpoints, shape/junction deletion with ConnEnd detachment, connector deletion, and native Rust plus Node tests.

## Install

```sh
npm install libavoid-rust
```

## Build from source

```sh
npm install
npm run build:node
```

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

console.log(router.routeConnector("edge"));
console.log(router.shapeBoundingBox("target"));
```

The package also exports `WasmAvoidRouter`, the raw `wasm-bindgen` class. That lower-level class returns JSON strings from `routeConnector()` and `routeAll()`.

## Scope

This package is not yet a behavior-complete port of upstream libavoid. The next conversion milestones are:

- route generation on top of the translated visibility graph and scanline foundations
- full polygon obstacle routing beyond the current bounding-box fallback
- hyperedge routing and nudging
- compatibility fixtures translated from `vendor/adaptagrams/cola/libavoid/tests`
