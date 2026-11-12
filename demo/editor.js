// DOM-free core of the demo: owns the AvoidRouter, applies edits as
// transactions and times them.  The router is the single source of truth
// for geometry; the editor only keeps display metadata (labels, colours).
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
  // [pin id, classId in the "sides" model, xOffset, yOffset, direction name]
  ["top", 2, 0.5, 0, "Up"],
  ["right", 3, 1, 0.5, "Right"],
  ["bottom", 4, 0.5, 1, "Down"],
  ["left", 5, 0, 0.5, "Left"],
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

export class Editor {
  constructor(api) {
    this.api = api;
    this.router = null;
    this.mode = "orthogonal";
    this.params = {};
    this.options = {};
    this.meta = new Map(); // id -> { label?, colour? }
    this.counter = 0;
    this.colourCounter = 0;
    this.timings = [];
    this.last = null;
  }

  // --- scene lifecycle -----------------------------------------------------

  /** Replaces the router and builds `scene` (see scenes.js). */
  load(scene) {
    const { api } = this;
    if (this.router) this.router.free();
    this.router = new api.AvoidRouter(api.RouterFlag.OrthogonalRouting | api.RouterFlag.PolyLineRouting);
    this.meta.clear();
    this.counter = 0;
    this.colourCounter = 0;
    this.timings = [];
    this.mode = scene.mode ?? "orthogonal";
    this.params = { ...DEFAULT_PARAMS, ...scene.params };
    this.options = {};
    for (const { key } of OPTIONS) {
      this.options[key] = this.router.routingOption(api.RoutingOption[key]);
    }
    Object.assign(this.options, DEFAULT_OPTIONS, scene.options);
    for (const [key, value] of Object.entries(this.params)) {
      this.router.setRoutingParameter(api.RoutingParameter[key], value);
    }
    for (const [key, value] of Object.entries(this.options)) {
      this.router.setRoutingOption(api.RoutingOption[key], value);
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
      for (const [key, value] of Object.entries(this.params)) router.setRoutingParameter(api.RoutingParameter[key], value);
      for (const [key, value] of Object.entries(this.options)) router.setRoutingOption(api.RoutingOption[key], value);
      router.markAllObstaclesAsMoved();
    });
  }

  /** Runs `fn` (queued edits), processes the transaction and times it. */
  transact(fn) {
    const before = this.structureKey();
    const result = fn ? fn() : undefined;
    const t0 = performance.now();
    this.router.processTransaction();
    const ms = performance.now() - t0;
    const updated = this.router.lastTransactionUpdatedConnectorIds();
    const total = this.router.connectorIds().length;
    this.last = { ms, updated, total, structural: before !== this.structureKey() };
    this.timings.push(ms);
    if (this.timings.length > HISTORY) this.timings.shift();
    return result;
  }

  structureKey() {
    const r = this.router;
    return [r.shapeIds(), r.junctionIds(), r.connectorIds(), r.clusterIds()].map((ids) => ids.join(",")).join("|");
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

  freshId(prefix) {
    let id;
    do id = `${prefix}${++this.counter}`;
    while (!this.router.objectIdIsUnused(id));
    return id;
  }

  connType() {
    return this.mode === "orthogonal" ? this.api.ConnType.Orthogonal : this.api.ConnType.PolyLine;
  }

  nextColour() {
    return this.colourCounter++ % 6;
  }

  // --- building blocks (no transaction of their own; used by scenes) ------

  /** A rectangle (or a polygon given as unit-box fractions) with demo pins ("any", "sides" or false). */
  box(label, x, y, w, h, { id = this.freshId("box"), polygon, pins = "any" } = {}) {
    const { api, router } = this;
    if (polygon) {
      router.addPolygonShape(id, polygon.map(([fx, fy]) => [x + fx * w, y + fy * h]));
    } else {
      router.addShape(id, x, y, x + w, y + h);
    }
    if (pins) {
      for (const [pin, sideClass, xOffset, yOffset, dir] of SIDES) {
        router.addShapeConnectionPinWithOptions(id, pin, {
          classId: pins === "sides" ? sideClass : PIN_ANY,
          xOffset,
          yOffset,
          proportional: true,
          visibilityDirections: api.ConnDir[dir],
          // Upstream pins are exclusive (one connector each) by default; a
          // box with more connectors than pins would leave some unrouted.
          exclusive: false,
        });
      }
    }
    this.meta.set(id, { label, polygon: Boolean(polygon) });
    return id;
  }

  junction(x, y, { id = this.freshId("j") } = {}) {
    this.router.addJunction(id, x, y);
    this.meta.set(id, {});
    return id;
  }

  cluster(label, x, y, w, h) {
    const id = this.freshId("cluster");
    this.router.addClusterRectangle(id, x, y, x + w, y + h);
    this.meta.set(id, { label });
    return id;
  }

  /**
   * A connector between two ends: `[x, y]` (free point), `"shape"` (class
   * PIN_ANY), `"shape.pin"` (the class of that pin) or `{ junctionId }` / any ConnEnd.
   */
  connect(src, dst, { checkpoints, id = this.freshId("c") } = {}) {
    const { router } = this;
    router.addConnectorWithConnEnds(id, this.end(src), this.end(dst));
    router.setConnectorRoutingType(id, this.connType());
    if (checkpoints) router.setConnectorRoutingCheckpoints(id, checkpoints);
    this.meta.set(id, { colour: this.nextColour() });
    return id;
  }

  end(spec) {
    if (typeof spec !== "string") return spec;
    if (this.router.junctionIds().includes(spec)) return { junctionId: spec };
    const [shapeId, side] = spec.split(".");
    return side ? { shapeId, pinId: side } : { shapeId, classId: PIN_ANY };
  }

  // --- interactive edits (each one transaction) -------------------------

  setMode(mode) {
    this.mode = mode;
    return this.transact(() => {
      for (const id of this.router.connectorIds()) this.router.setConnectorRoutingType(id, this.connType());
    });
  }

  setParam(key, value) {
    this.params[key] = value;
    return this.transact(() => {
      this.router.setRoutingParameter(this.api.RoutingParameter[key], value);
      this.router.markAllObstaclesAsMoved();
    });
  }

  setOption(key, value) {
    this.options[key] = value;
    return this.transact(() => {
      this.router.setRoutingOption(this.api.RoutingOption[key], value);
      this.router.markAllObstaclesAsMoved();
    });
  }

  addBox(x, y) {
    const id = this.freshId("box");
    return this.transact(() => this.box(`Box ${id.slice(3)}`, Math.round(x - 50), Math.round(y - 30), 100, 60, { id }));
  }

  addJunction(x, y) {
    return this.transact(() => this.junction(Math.round(x), Math.round(y)));
  }

  addConnector(src, dst) {
    return this.transact(() => this.connect(src, dst));
  }

  moveShapeBy(id, dx, dy) {
    return this.transact(() => this.router.moveShapeBy(id, dx, dy));
  }

  moveJunction(id, x, y) {
    return this.transact(() => this.router.moveJunction(id, x, y));
  }

  /** Moves the free (point) source or destination of a connector. */
  moveEndpoint(id, which, x, y) {
    return this.transact(() =>
      which === "source" ? this.router.setConnectorSourceEndpoint(id, [x, y]) : this.router.setConnectorDestEndpoint(id, [x, y]),
    );
  }

  checkpoints(id) {
    return (this.router.connectorRoutingCheckpoints(id) ?? []).map((c) => c.point);
  }

  /** Inserts a checkpoint at `point`, ordered along the current route. */
  addCheckpoint(id, point) {
    const route = this.router.displayConnectorRoute(id) ?? [];
    const pts = [...this.checkpoints(id), point];
    pts.sort((a, b) => arcPosition(route, a) - arcPosition(route, b));
    return this.transact(() => this.router.setConnectorRoutingCheckpoints(id, pts));
  }

  moveCheckpoint(id, index, point) {
    const pts = this.checkpoints(id);
    pts[index] = point;
    return this.transact(() => this.router.setConnectorRoutingCheckpoints(id, pts));
  }

  removeCheckpoint(id, index) {
    const pts = this.checkpoints(id);
    pts.splice(index, 1);
    return this.transact(() => this.router.setConnectorRoutingCheckpoints(id, pts));
  }

  /** Splits a connector at the display-route segment nearest `point` with a new junction. */
  splitConnector(id, point) {
    const route = this.router.displayConnectorRoute(id) ?? [];
    const segment = nearestSegment(route, point);
    const junctionId = this.freshId("j");
    const connectorId = this.freshId("c");
    let result = null;
    this.transact(() => {
      result = this.router.splitConnectorAtSegment(id, segment, junctionId, connectorId);
      if (result) {
        this.meta.set(junctionId, {});
        this.meta.set(connectorId, { colour: this.meta.get(id)?.colour ?? this.nextColour() });
      }
    });
    return result;
  }

  /** Removes a shape (with its connectors), junction (merging if possible), connector or cluster. */
  remove(id) {
    const r = this.router;
    return this.transact(() => {
      if (r.shapeIds().includes(id)) {
        for (const c of r.shapeAttachedConnectors(id) ?? []) r.removeConnector(c);
        r.removeShape(id);
      } else if (r.junctionIds().includes(id)) {
        const attached = r.junctionAttachedConnectors(id) ?? [];
        if (attached.length === 2 && r.removeJunctionAndMergeConnectors(id) !== null) return;
        for (const c of attached) r.removeConnector(c);
        r.removeJunction(id);
      } else if (r.connectorIds().includes(id)) {
        r.removeConnector(id);
      } else if (r.clusterIds().includes(id)) {
        r.removeCluster(id);
      }
      this.meta.delete(id);
    });
  }

  rotatePins(id) {
    return this.transact(() => this.router.transformShapeConnectionPinPositions(id, this.api.ShapeTransformation.CW90));
  }

  setJunctionFixed(id, fixed) {
    return this.transact(() => this.router.setJunctionPositionFixed(id, fixed));
  }

  /**
   * Re-routes the whole hyperedge around `junctionId` as an optimal tree.
   * The connectors the router creates for it are routed again (nudged
   * against each other) by the following transaction, so run one more.
   */
  rerouteHyperedge(junctionId) {
    const result = this.transact(() => this.router.registerHyperedgeForReroutingJunction(junctionId));
    this.transact();
    return result;
  }

  // --- geometry for rendering -------------------------------------------

  kindOf(id) {
    const r = this.router;
    if (r.shapeIds().includes(id)) return "shape";
    if (r.junctionIds().includes(id)) return "junction";
    if (r.connectorIds().includes(id)) return "connector";
    if (r.clusterIds().includes(id)) return "cluster";
    return null;
  }

  shapeGeometry(id) {
    const r = this.router;
    const pins = [];
    for (const pin of r.shapeConnectionPinIdList(id) ?? []) {
      const pos = r.shapeConnectionPinPosition(id, pin);
      if (pos) pins.push({ id: pin, ...pos });
    }
    return {
      id,
      label: this.meta.get(id)?.label ?? id,
      polygon: r.shapePolygon(id),
      box: r.shapeBoundingBox(id),
      routingPolygon: r.shapeRoutingPolygon(id),
      pins,
    };
  }

  junctionGeometry(id) {
    return { id, ...this.router.junctionPosition(id), fixed: this.router.junctionPositionFixed(id) };
  }

  clusterGeometry(id) {
    return { id, label: this.meta.get(id)?.label ?? id, polygon: this.router.clusterPolygon(id) };
  }

  connectorGeometry(id, { curve = 0 } = {}) {
    const r = this.router;
    const meta = this.meta.get(id) ?? this.adopt(id);
    const ends = r.connectorEndpointConnEnds(id);
    return {
      id,
      colour: meta.colour,
      display: r.displayConnectorRoute(id) ?? [],
      raw: r.routeConnector(id) ?? [],
      curved: curve > 0 ? r.connectorCurvedRoute(id, curve) : null,
      checkpoints: this.checkpoints(id),
      ends,
    };
  }

  /** Metadata for objects the router created itself (hyperedge rerouting). */
  adopt(id) {
    const meta = { colour: this.nextColour() };
    this.meta.set(id, meta);
    return meta;
  }

  snapshot(options) {
    const r = this.router;
    return {
      clusters: r.clusterIds().map((id) => this.clusterGeometry(id)),
      shapes: r.shapeIds().map((id) => this.shapeGeometry(id)),
      junctions: r.junctionIds().map((id) => this.junctionGeometry(id)),
      connectors: r.connectorIds().map((id) => this.connectorGeometry(id, options)),
    };
  }

  /** Bounding box of everything in the scene. */
  bounds() {
    const s = this.snapshot();
    const xs = [];
    const ys = [];
    const add = (p) => {
      xs.push(p.x);
      ys.push(p.y);
    };
    s.clusters.forEach((c) => c.polygon.forEach(add));
    s.shapes.forEach((sh) => sh.polygon.forEach(add));
    s.junctions.forEach(add);
    s.connectors.forEach((c) => c.display.forEach(add));
    if (!xs.length) return { left: 0, top: 0, right: 800, bottom: 500 };
    return { left: Math.min(...xs), top: Math.min(...ys), right: Math.max(...xs), bottom: Math.max(...ys) };
  }
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
