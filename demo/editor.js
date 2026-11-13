// DOM-free core of the demo: owns the Avoid.Router, applies edits as
// transactions and times them.  The router is the single source of truth
// for geometry; the editor only keeps display metadata (labels, colours,
// pin names) keyed by upstream's object ids (`obj.id()`, as strings).
// app.js renders the snapshot and turns pointer / keyboard input into calls.

/**
 * Demo boxes get one pin in the middle of each side, in one of two models:
 *  - "any": all four pins share class PIN_ANY, so a connector to any of them
 *    lets the router pick the best side (it changes side as boxes move);
 *  - "sides": each side has its own class, so a connector keeps its side.
 * The two are never combined on one box: as in upstream libavoid, pins of
 * different classes at the same position break orthogonal routing to them
 * (the connector falls back to the shape centre).
 */
export const PIN_ANY = 1;
export const SIDES = [
  // [pin name, classId in the "sides" model, xOffset, yOffset, visibility direction]
  ["top", 2, 0.5, 0, "ConnDirUp"],
  ["right", 3, 1, 0.5, "ConnDirRight"],
  ["bottom", 4, 0.5, 1, "ConnDirDown"],
  ["left", 5, 0, 0.5, "ConnDirLeft"],
];

/**
 * Routing parameters the demo exposes, in the order shown.  `modes` lists the
 * routing modes the parameter influences (the others dim it).
 */
export const PARAMETERS = [
  { key: "shapeBufferDistance", label: "Shape buffer", min: 0, max: 40, step: 1, modes: ["orthogonal", "polyline"], hint: "Clearance kept around every shape." },
  { key: "idealNudgingDistance", label: "Nudging distance", min: 0, max: 30, step: 1, modes: ["orthogonal"], hint: "Spacing between parallel segments after nudging." },
  { key: "segmentPenalty", label: "Segment penalty", min: 0, max: 200, step: 5, modes: ["orthogonal", "polyline"], hint: "Cost of each extra segment: higher means fewer bends." },
  { key: "anglePenalty", label: "Angle penalty", min: 0, max: 500, step: 10, modes: ["polyline"], hint: "Cost of sharp bends in poly-line routes." },
  { key: "crossingPenalty", label: "Crossing penalty", min: 0, max: 1000, step: 10, modes: ["orthogonal", "polyline"], hint: "Cost of crossing another connector. Slower when non-zero." },
  { key: "clusterCrossingPenalty", label: "Cluster crossing penalty", min: 0, max: 10000, step: 100, modes: ["polyline"], hint: "Cost of crossing a cluster boundary." },
  { key: "fixedSharedPathPenalty", label: "Shared path penalty", min: 0, max: 500, step: 10, modes: ["orthogonal", "polyline"], hint: "Cost of sharing a path with a fixed connector." },
  { key: "portDirectionPenalty", label: "Port direction penalty", min: 0, max: 500, step: 10, modes: ["polyline"], hint: "Cost of leaving a pin against its direction." },
];

export const OPTIONS = [
  { key: "nudgeOrthogonalSegmentsConnectedToShapes", label: "Nudge segments at shapes", modes: ["orthogonal"] },
  { key: "nudgeSharedPathsWithCommonEndPoint", label: "Nudge paths with a shared end", modes: ["orthogonal"] },
  { key: "nudgeOrthogonalTouchingColinearSegments", label: "Nudge touching colinear segments", modes: ["orthogonal"] },
  { key: "performUnifyingNudgingPreprocessingStep", label: "Unify segments before nudging", modes: ["orthogonal"] },
  { key: "penaliseOrthogonalSharedPathsAtConnEnds", label: "Penalise shared paths at ends", modes: ["orthogonal"] },
  { key: "improveHyperedgeRoutesMovingJunctions", label: "Improve hyperedges: move junctions", modes: ["orthogonal"] },
  { key: "improveHyperedgeRoutesMovingAddingAndDeletingJunctions", label: "Improve hyperedges: add and remove junctions", modes: ["orthogonal"] },
];

/** Demo defaults, applied before each preset's own overrides. */
export const DEFAULT_PARAMS = {
  shapeBufferDistance: 8,
  idealNudgingDistance: 8,
  segmentPenalty: 50,
  anglePenalty: 0,
  crossingPenalty: 0,
  clusterCrossingPenalty: 4000,
  fixedSharedPathPenalty: 0,
  portDirectionPenalty: 100,
};
export const DEFAULT_OPTIONS = {
  nudgeOrthogonalSegmentsConnectedToShapes: true,
};

const HISTORY = 120;

const END_TYPES = ["point", "shapePin", "junction", "empty"]; // upstream ConnEndType order

export class Editor {
  constructor(api) {
    this.api = api;
    this.router = null;
    this.mode = "orthogonal";
    this.params = {};
    this.options = {};
    this.objects = new Map(); // id -> ShapeRef / JunctionRef / ConnRef / ClusterRef, in creation order
    this.meta = new Map(); // id -> { label?, colour?, pins? }
    this.hooked = new WeakSet(); // connectors whose callback is set
    this.changed = new Set(); // ids of connectors whose callback fired
    this.boxCounter = 0;
    this.colourCounter = 0;
    this.timings = [];
    this.last = null;
    // upstream: void (*cb)(void *ptr), registered with ConnRef::setCallback();
    // `ptr` is the connector's id.
    this.routeChanged = (id) => this.changed.add(id);
  }

  // --- scene lifecycle -----------------------------------------------------

  /** Replaces the router and builds `scene` (see scenes.js). */
  load(scene) {
    const { api } = this;
    if (this.router) this.router.delete();
    this.router = new api.Router(api.OrthogonalRouting | api.PolyLineRouting);
    this.objects.clear();
    this.meta.clear();
    this.hooked = new WeakSet();
    this.boxCounter = 0;
    this.colourCounter = 0;
    this.timings = [];
    this.mode = scene.mode ?? "orthogonal";
    this.params = { ...DEFAULT_PARAMS, ...scene.params };
    this.options = {};
    for (const { key } of OPTIONS) {
      this.options[key] = this.router.routingOption(api[key]);
    }
    Object.assign(this.options, DEFAULT_OPTIONS, scene.options);
    for (const [key, value] of Object.entries(this.params)) {
      this.router.setRoutingParameter(api[key], value);
    }
    for (const [key, value] of Object.entries(this.options)) {
      this.router.setRoutingOption(api[key], value);
    }
    this.sceneDefaults = { params: { ...this.params }, options: { ...this.options } };
    return this.transact(() => scene.build(this));
  }

  /** Restores the parameters and options the current scene started with, in one transaction. */
  resetSettings() {
    const { api, router } = this;
    this.params = { ...this.sceneDefaults.params };
    this.options = { ...this.sceneDefaults.options };
    return this.transact(() => {
      for (const [key, value] of Object.entries(this.params)) router.setRoutingParameter(api[key], value);
      for (const [key, value] of Object.entries(this.options)) router.setRoutingOption(api[key], value);
      router.markAllObstaclesAsMoved();
    });
  }

  /**
   * Runs `fn` (queued edits), processes the transaction and times it.  The
   * connectors whose callback fired are the ones the transaction rerouted.
   */
  transact(fn) {
    const r = this.router;
    const before = this.structureKey();
    const result = fn ? fn() : undefined;
    this.changed.clear();
    let t0 = performance.now();
    r.processTransaction();
    let ms = performance.now() - t0;
    this.sync();
    const structural = before !== this.structureKey();
    // Connectors the router created (hyperedge rerouting / improvement,
    // splitAtSegment) get its default type, poly-line under both routing
    // flags: give them the current mode and route them again (which may
    // create more of them).
    for (let round = 0; structural && round < 4; round++) {
      const type = this.connType();
      const wrong = this.ofKind("connector").filter((c) => c.routingType() !== type);
      if (!wrong.length) break;
      for (const c of wrong) c.setRoutingType(type);
      t0 = performance.now();
      r.processTransaction();
      ms += performance.now() - t0;
      this.sync();
    }
    const total = this.ofKind("connector").length;
    this.last = { ms, updated: [...this.changed].filter((id) => this.objects.has(id)), total, structural };
    this.timings.push(ms);
    if (this.timings.length > HISTORY) this.timings.shift();
    return result;
  }

  /**
   * Brings `objects` in line with the router's lists after a transaction:
   * drops objects the router deleted, adds those it created (keeping the
   * creation order) and sets the route callback on new connectors.
   */
  sync() {
    const r = this.router;
    const live = new Map();
    for (const list of [r.m_obstacles, r.connRefs, r.clusterRefs]) {
      for (const obj of list) live.set(String(obj.id()), obj);
    }
    for (const id of [...this.objects.keys()]) {
      if (!live.has(id)) {
        this.objects.delete(id);
        this.meta.delete(id);
      }
    }
    for (const [id, obj] of live) {
      if (!this.objects.has(id)) this.objects.set(id, obj);
      if (obj instanceof this.api.ConnRef) this.hook(obj);
    }
  }

  hook(conn) {
    if (this.hooked.has(conn)) return;
    conn.setCallback(this.routeChanged, String(conn.id()));
    this.hooked.add(conn);
  }

  structureKey() {
    return [...this.objects.keys()].join(",");
  }

  stats() {
    const sorted = [...this.timings].sort((a, b) => a - b);
    return {
      ...this.last,
      median: sorted.length ? sorted[Math.floor(sorted.length / 2)] : 0,
      max: sorted.length ? sorted[sorted.length - 1] : 0,
      samples: sorted.length,
    };
  }

  connType() {
    return this.mode === "orthogonal" ? this.api.ConnType_Orthogonal : this.api.ConnType_PolyLine;
  }

  nextColour() {
    return this.colourCounter++ % 6;
  }

  /** Registers an object the editor created; returns its id. */
  add(obj, meta) {
    const id = String(obj.id());
    this.objects.set(id, obj);
    this.meta.set(id, meta);
    if (obj instanceof this.api.ConnRef) this.hook(obj);
    return id;
  }

  // --- building blocks (no transaction of their own; used by scenes) ------

  /** A rectangle (or a polygon given as unit-box fractions) with demo pins ("any", "sides" or false). */
  box(label, x, y, w, h, { polygon, pins = "any" } = {}) {
    const { api, router } = this;
    let poly;
    if (polygon) {
      poly = new api.Polygon(polygon.length);
      polygon.forEach(([fx, fy], i) => (poly.ps[i] = new api.Point(x + fx * w, y + fy * h)));
    } else {
      poly = new api.Rectangle(new api.Point(x, y), new api.Point(x + w, y + h));
    }
    const shape = new api.ShapeRef(router, poly);
    const id = this.add(shape, { label, polygon: Boolean(polygon), pins: [] });
    if (pins) {
      for (const [name, sideClass, xOffset, yOffset, dir] of SIDES) {
        const pin = this.pin(id, name, pins === "sides" ? sideClass : PIN_ANY, xOffset, yOffset, api[dir]);
        // Upstream pins are exclusive (one connector each) by default; a
        // box with more connectors than pins would leave some unrouted.
        pin.setExclusive(false);
      }
    }
    return id;
  }

  /** A proportional ShapeConnectionPin on shape `id`, named `name` for the UI. */
  pin(id, name, classId, xOffset, yOffset, visDirs) {
    const pin = new this.api.ShapeConnectionPin(this.objects.get(id), classId, xOffset, yOffset, true, 0, visDirs);
    this.meta.get(id).pins.push({ name, classId, pin });
    return pin;
  }

  junction(x, y) {
    const { api } = this;
    return this.add(new api.JunctionRef(this.router, new api.Point(x, y)), {});
  }

  cluster(label, x, y, w, h) {
    const { api } = this;
    return this.add(new api.ClusterRef(this.router, new api.Rectangle(new api.Point(x, y), new api.Point(x + w, y + h))), { label });
  }

  /**
   * A connector between two ends: `[x, y]` (free point), `"shape"` (class
   * PIN_ANY), `"shape.pin"` (the class of that pin), a junction id, or
   * `{ shapeId, pinId }` / `{ junctionId }`.
   */
  connect(src, dst, { checkpoints } = {}) {
    const { api } = this;
    const conn = new api.ConnRef(this.router, this.end(src), this.end(dst));
    conn.setRoutingType(this.connType());
    if (checkpoints) conn.setRoutingCheckpoints(checkpoints.map(([x, y]) => new api.Checkpoint(new api.Point(x, y))));
    return this.add(conn, { colour: this.nextColour() });
  }

  end(spec) {
    const { api } = this;
    if (Array.isArray(spec)) return new api.ConnEnd(new api.Point(spec[0], spec[1]));
    if (typeof spec === "string") {
      if (this.kindOf(spec) === "junction") spec = { junctionId: spec };
      else {
        const [shapeId, pinId] = spec.split(".");
        spec = { shapeId, pinId };
      }
    }
    if (spec.junctionId) return new api.ConnEnd(this.objects.get(spec.junctionId));
    const pin = spec.pinId && this.meta.get(spec.shapeId)?.pins.find((p) => p.name === spec.pinId);
    return new api.ConnEnd(this.objects.get(spec.shapeId), pin ? pin.classId : PIN_ANY);
  }

  // --- interactive edits (each one transaction) -------------------------

  setMode(mode) {
    this.mode = mode;
    return this.transact(() => {
      for (const c of this.ofKind("connector")) c.setRoutingType(this.connType());
    });
  }

  setParam(key, value) {
    this.params[key] = value;
    return this.transact(() => {
      this.router.setRoutingParameter(this.api[key], value);
      this.router.markAllObstaclesAsMoved();
    });
  }

  setOption(key, value) {
    this.options[key] = value;
    return this.transact(() => {
      this.router.setRoutingOption(this.api[key], value);
      this.router.markAllObstaclesAsMoved();
    });
  }

  addBox(x, y) {
    return this.transact(() => this.box(`Box ${++this.boxCounter}`, Math.round(x - 50), Math.round(y - 30), 100, 60));
  }

  addJunction(x, y) {
    return this.transact(() => this.junction(Math.round(x), Math.round(y)));
  }

  addConnector(src, dst) {
    return this.transact(() => this.connect(src, dst));
  }

  moveShapeBy(id, dx, dy) {
    return this.transact(() => this.router.moveShape(this.objects.get(id), dx, dy));
  }

  moveJunction(id, x, y) {
    return this.transact(() => this.router.moveJunction(this.objects.get(id), new this.api.Point(x, y)));
  }

  /** Moves the free (point) source or destination of a connector. */
  moveEndpoint(id, which, x, y) {
    const conn = this.objects.get(id);
    const end = new this.api.ConnEnd(new this.api.Point(x, y));
    return this.transact(() => (which === "source" ? conn.setSourceEndpoint(end) : conn.setDestEndpoint(end)));
  }

  checkpoints(id) {
    return this.objects.get(id).routingCheckpoints().map((c) => c.point);
  }

  setCheckpoints(id, points) {
    const { api } = this;
    const conn = this.objects.get(id);
    return this.transact(() => conn.setRoutingCheckpoints(points.map((p) => new api.Checkpoint(new api.Point(p.x, p.y)))));
  }

  /** Inserts a checkpoint at `point`, ordered along the current route. */
  addCheckpoint(id, point) {
    const route = this.objects.get(id).displayRoute().ps;
    const pts = [...this.checkpoints(id), point];
    pts.sort((a, b) => arcPosition(route, a) - arcPosition(route, b));
    return this.setCheckpoints(id, pts);
  }

  moveCheckpoint(id, index, point) {
    const pts = this.checkpoints(id);
    pts[index] = point;
    return this.setCheckpoints(id, pts);
  }

  removeCheckpoint(id, index) {
    const pts = this.checkpoints(id);
    pts.splice(index, 1);
    return this.setCheckpoints(id, pts);
  }

  /**
   * Splits a connector at the display-route segment nearest `point` with a
   * new junction (ConnRef::splitAtSegment); null if libavoid did not split.
   */
  splitConnector(id, point) {
    const conn = this.objects.get(id);
    const segment = nearestSegment(conn.displayRoute().ps, point);
    let result = null;
    this.transact(() => {
      const [junction, newConn] = conn.splitAtSegment(segment);
      if (junction) {
        const junctionId = this.add(junction, {});
        this.add(newConn, { colour: this.meta.get(id)?.colour ?? this.nextColour() });
        result = { junctionId };
      }
    });
    return result;
  }

  /** Removes a shape (with its connectors), junction (merging if possible), connector or cluster. */
  remove(id) {
    const r = this.router;
    const obj = this.objects.get(id);
    const kind = this.kindOf(id);
    return this.transact(() => {
      if (kind === "shape") {
        for (const c of new Set(obj.attachedConnectors())) r.deleteConnector(c);
        r.deleteShape(obj);
      } else if (kind === "junction") {
        const attached = new Set(obj.attachedConnectors());
        if (attached.size === 2 && obj.removeJunctionAndMergeConnectors() !== null) return;
        for (const c of attached) r.deleteConnector(c);
        r.deleteJunction(obj);
      } else if (kind === "connector") {
        r.deleteConnector(obj);
      } else if (kind === "cluster") {
        r.deleteCluster(obj);
      }
    });
  }

  rotatePins(id) {
    return this.transact(() => this.objects.get(id).transformConnectionPinPositions(this.api.TransformationType_CW90));
  }

  setJunctionFixed(id, fixed) {
    return this.transact(() => this.objects.get(id).setPositionFixed(fixed));
  }

  /**
   * Re-routes the whole hyperedge around `junctionId` as an optimal tree.
   * The connectors the router creates for it are routed again (nudged
   * against each other) by the following transaction, so run one more.
   */
  rerouteHyperedge(junctionId) {
    const result = this.transact(() => this.router.hyperedgeRerouter().registerHyperedgeForRerouting(this.objects.get(junctionId)));
    const first = this.last;
    this.transact();
    this.last = { ...this.last, ms: first.ms + this.last.ms, structural: true, updated: [...new Set([...first.updated, ...this.last.updated])] };
    return result;
  }

  // --- geometry for rendering -------------------------------------------

  kindOf(id) {
    const obj = this.objects.get(id);
    const { api } = this;
    if (obj instanceof api.ShapeRef) return "shape";
    if (obj instanceof api.JunctionRef) return "junction";
    if (obj instanceof api.ConnRef) return "connector";
    if (obj instanceof api.ClusterRef) return "cluster";
    return null;
  }

  /** Ids of the objects of one kind ("shape", "junction", "connector", "cluster"). */
  ids(kind) {
    return [...this.objects.keys()].filter((id) => this.kindOf(id) === kind);
  }

  ofKind(kind) {
    return this.ids(kind).map((id) => this.objects.get(id));
  }

  get(id) {
    return this.objects.get(id);
  }

  shapeGeometry(id) {
    const shape = this.objects.get(id);
    const meta = this.meta.get(id) ?? {};
    const polygon = shape.polygon().ps;
    return {
      id,
      label: meta.label ?? id,
      polygon,
      box: boundsOf(polygon),
      routingPolygon: shape.routingPolygon().ps,
      pins: (meta.pins ?? []).map(({ name, pin }) => {
        const p = pin.position();
        return { id: name, x: p.x, y: p.y };
      }),
    };
  }

  pinPosition(shapeId, name) {
    return this.meta.get(shapeId).pins.find((p) => p.name === name).pin.position();
  }

  /**
   * Where the junction is drawn: its recommendedPosition(), which is where
   * the routes meet.  Hyperedge improvement moves junctions only there
   * (upstream leaves position() as the caller set it).
   */
  junctionGeometry(id) {
    const junction = this.objects.get(id);
    const p = junction.recommendedPosition();
    return { id, x: p.x, y: p.y, fixed: junction.positionFixed() };
  }

  clusterGeometry(id) {
    const poly = this.objects.get(id).polygon();
    const polygon = [];
    for (let i = 0; i < poly.size(); i++) polygon.push(poly.at(i));
    return { id, label: this.meta.get(id)?.label ?? id, polygon };
  }

  connectorGeometry(id, { curve = 0 } = {}) {
    const conn = this.objects.get(id);
    const meta = this.meta.get(id) ?? this.adopt(id);
    const display = conn.displayRoute();
    const [source, destination] = conn.endpointConnEnds();
    let curved = null;
    if (curve > 0) {
      const c = display.curvedPolyline(curve);
      curved = { points: c.ps, commands: c.ts };
    }
    return {
      id,
      colour: meta.colour,
      display: display.ps,
      raw: conn.route().ps,
      curved,
      checkpoints: this.checkpoints(id),
      ends: { source: this.endInfo(source), destination: this.endInfo(destination) },
    };
  }

  /** A ConnEnd as plain data for the UI. */
  endInfo(end) {
    const type = END_TYPES[end.type()];
    const info = { type, position: end.position() };
    if (type === "shapePin") {
      info.shapeId = String(end.shape().id());
      info.classId = end.pinClassId();
    } else if (type === "junction") {
      info.junctionId = String(end.junction().id());
    }
    return info;
  }

  /** Metadata for objects the router created itself (hyperedge rerouting). */
  adopt(id) {
    const meta = { colour: this.nextColour() };
    this.meta.set(id, meta);
    return meta;
  }

  snapshot(options) {
    return {
      clusters: this.ids("cluster").map((id) => this.clusterGeometry(id)),
      shapes: this.ids("shape").map((id) => this.shapeGeometry(id)),
      junctions: this.ids("junction").map((id) => this.junctionGeometry(id)),
      connectors: this.ids("connector").map((id) => this.connectorGeometry(id, options)),
    };
  }

  /** Bounding box of everything in the scene. */
  bounds() {
    const s = this.snapshot();
    const points = [];
    s.clusters.forEach((c) => points.push(...c.polygon));
    s.shapes.forEach((sh) => points.push(...sh.polygon));
    points.push(...s.junctions);
    s.connectors.forEach((c) => points.push(...c.display));
    if (!points.length) return { left: 0, top: 0, right: 800, bottom: 500 };
    return boundsOf(points);
  }
}

function boundsOf(points) {
  const xs = points.map((p) => p.x);
  const ys = points.map((p) => p.y);
  return { left: Math.min(...xs), top: Math.min(...ys), right: Math.max(...xs), bottom: Math.max(...ys) };
}

// --- route geometry helpers ------------------------------------------------

function projectOnSegment(a, b, p) {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const len2 = dx * dx + dy * dy;
  const t = len2 === 0 ? 0 : Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / len2));
  const x = a.x + t * dx;
  const y = a.y + t * dy;
  return { t, dist: Math.hypot(p.x - x, p.y - y), len: Math.sqrt(len2) };
}

/** Index i (1-based, as splitAtSegment expects) of the segment route[i-1]..route[i] nearest p. */
export function nearestSegment(route, p) {
  let best = 1;
  let bestDist = Infinity;
  for (let i = 1; i < route.length; i++) {
    const { dist } = projectOnSegment(route[i - 1], route[i], p);
    if (dist < bestDist) {
      bestDist = dist;
      best = i;
    }
  }
  return best;
}

/** Distance along the route of the projection of p (orders checkpoints). */
export function arcPosition(route, p) {
  let best = 0;
  let bestDist = Infinity;
  let walked = 0;
  for (let i = 1; i < route.length; i++) {
    const { t, dist, len } = projectOnSegment(route[i - 1], route[i], p);
    if (dist < bestDist) {
      bestDist = dist;
      best = walked + t * len;
    }
    walked += len;
  }
  return best;
}
