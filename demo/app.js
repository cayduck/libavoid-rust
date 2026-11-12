// UI of the demo: renders the editor's router state as SVG and turns
// pointer / keyboard input into editor calls.  The router comes from the
// package's browser entry (index.mjs), copied next to this file by
// scripts/build-demo.cjs.

import { Editor, OPTIONS, PARAMETERS, PIN_ANY, SIDES } from "./editor.js";
import { SCENES } from "./scenes.js";

const SVGNS = "http://www.w3.org/2000/svg";
const $ = (id) => document.getElementById(id);

boot();

async function boot() {
  const root = document.documentElement;
  const bootEl = $("boot");
  root.dataset.state = "loading";
  let api;
  try {
    if (typeof WebAssembly !== "object") {
      throw new Error("this browser does not support WebAssembly");
    }
    api = await import("./libavoid-rust/index.mjs");
    await api.default();
  } catch (error) {
    root.dataset.state = "error";
    bootEl.classList.add("error");
    bootEl.textContent = `The router could not be loaded (${error.message}). Reload the page to try again; the demo needs WebAssembly enabled.`;
    return;
  }
  const app = new App(api);
  app.start();
  bootEl.hidden = true;
  root.dataset.state = "ready";
  // For the browser smoke test and for poking at the router from the console.
  window.demo = app;
}

function svg(tag, attrs = {}, parent) {
  const el = document.createElementNS(SVGNS, tag);
  for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, v);
  if (parent) parent.appendChild(el);
  return el;
}

function pathData(points, closed = false) {
  if (!points.length) return "";
  return points.map((p, i) => `${i ? "L" : "M"}${p.x} ${p.y}`).join("") + (closed ? "Z" : "");
}

function curvedPathData({ points, commands }) {
  let d = "";
  let i = 0;
  for (const c of commands) {
    if (c === "C") {
      const [a, b, e] = points.slice(i, i + 3);
      d += `C${a.x} ${a.y} ${b.x} ${b.y} ${e.x} ${e.y}`;
      i += 3;
    } else if (c === "Z") {
      d += "Z";
    } else {
      d += `${c}${points[i].x} ${points[i].y}`;
      i += 1;
    }
  }
  return d;
}

const fmtMs = (ms) => (ms < 10 ? `${ms.toFixed(2)} ms` : `${ms.toFixed(1)} ms`);
const SIDE_NAMES = Object.fromEntries(SIDES.map(([pin, classId]) => [classId, `${pin} pin`]));

const TOOL_HINTS = {
  move: "Drag boxes, junctions, free ends and checkpoints. Drag empty space to pan.",
  box: "Click the canvas to add a box.",
  connect: "Drag from a pin, a box, a junction or empty space, and release on another.",
  junction: "Click empty space to add a junction, or a connector to split it.",
  checkpoint: "Click a connector to add a checkpoint; click a checkpoint to remove it.",
  delete: "Click a box, connector, junction or cluster to delete it.",
};

class App {
  constructor(api) {
    this.api = api;
    this.editor = new Editor(api);
    this.svg = $("canvas");
    this.layers = {
      clusters: $("layer-clusters"),
      buffers: $("layer-buffers"),
      raw: $("layer-raw"),
      connectors: $("layer-connectors"),
      shapes: $("layer-shapes"),
      handles: $("layer-handles"),
    };
    this.els = { clusters: new Map(), shapes: new Map(), junctions: new Map(), connectors: new Map() };
    this.tool = "move";
    this.selection = null;
    this.scene = null;
    this.view = { x: 0, y: 0, w: 1000, h: 640 };
    this.display = { raw: false, buffers: true, curved: false };
    this.drag = null;
    this.pointers = new Map();
    this.frame = 0;
    this.pendingParam = null;
  }

  start() {
    this.buildDefs();
    this.buildControls();
    this.bindCanvas();
    this.bindKeyboard();
    new ResizeObserver(() => this.applyView()).observe(this.svg);
    const fromHash = new URLSearchParams(location.hash.slice(1)).get("scene");
    this.loadScene(SCENES.find((s) => s.id === fromHash) ?? SCENES[0]);
  }

  // --- scene / settings --------------------------------------------------

  loadScene(scene) {
    this.scene = scene;
    this.cancelDrag();
    this.select(null);
    for (const map of Object.values(this.els)) {
      for (const el of map.values()) el.remove?.();
      map.clear();
    }
    for (const layer of Object.values(this.layers)) layer.replaceChildren();
    this.editor.load(scene);
    $("scene").value = scene.id;
    $("scene-blurb").textContent = scene.blurb;
    const actions = $("scene-actions");
    actions.replaceChildren();
    for (const action of scene.actions ?? []) {
      const b = document.createElement("button");
      b.type = "button";
      b.textContent = action.label;
      b.addEventListener("click", () => {
        action.run(this.editor, this.selection);
        this.afterEdit(true);
        this.announce(`${action.label}: done`);
      });
      actions.append(b);
    }
    this.syncSettingsControls();
    history.replaceState(null, "", `#scene=${scene.id}`);
    this.fit();
    this.afterEdit(true);
  }

  syncSettingsControls() {
    const { editor } = this;
    for (const input of document.querySelectorAll('input[name="mode"]')) input.checked = input.value === editor.mode;
    for (const p of PARAMETERS) {
      const input = $(`param-${p.key}`);
      input.value = editor.params[p.key];
      $(`param-${p.key}-value`).value = editor.params[p.key];
      input.closest(".param").classList.toggle("inactive", !p.modes.includes(editor.mode));
    }
    for (const o of OPTIONS) {
      const input = $(`option-${o.key}`);
      input.checked = editor.options[o.key];
      input.closest("label").classList.toggle("inactive", !o.modes.includes(editor.mode));
    }
  }

  buildControls() {
    const sceneSelect = $("scene");
    for (const s of SCENES) sceneSelect.append(new Option(s.name, s.id));
    sceneSelect.addEventListener("change", () => {
      this.loadScene(SCENES.find((s) => s.id === sceneSelect.value));
      this.announce(`Loaded scene ${this.scene.name}`);
    });

    for (const input of document.querySelectorAll('input[name="mode"]')) {
      input.addEventListener("change", () => {
        if (!input.checked) return;
        this.editor.setMode(input.value);
        this.syncSettingsControls();
        this.afterEdit(true);
      });
    }

    const params = $("params");
    for (const p of PARAMETERS) {
      const scope = p.modes.length === 1 ? (p.modes[0] === "orthogonal" ? "orthogonal" : "poly-line") : "";
      const row = document.createElement("div");
      row.className = "param";
      row.innerHTML = `
        <div class="param-head">
          <label for="param-${p.key}">${p.label}${scope ? ` <span class="scope">${scope}</span>` : ""}</label>
          <output id="param-${p.key}-value" for="param-${p.key}"></output>
        </div>
        <input type="range" id="param-${p.key}" min="${p.min}" max="${p.max}" step="${p.step}" aria-describedby="param-${p.key}-hint">
        <p class="param-hint" id="param-${p.key}-hint">${p.hint} <code>RoutingParameter.${p.key}</code></p>`;
      params.append(row);
      const input = row.querySelector("input");
      input.addEventListener("input", () => {
        $(`param-${p.key}-value`).value = input.value;
        this.pendingParam = [p.key, Number(input.value)];
        this.schedule();
      });
    }
    $("reset-params").addEventListener("click", () => {
      this.editor.resetSettings();
      this.syncSettingsControls();
      this.afterEdit(true);
      this.announce("Parameters reset");
    });

    const options = $("options");
    for (const o of OPTIONS) {
      const label = document.createElement("label");
      label.title = `RoutingOption.${o.key}`;
      label.innerHTML = `<input type="checkbox" id="option-${o.key}"> <span>${o.label}</span>`;
      options.append(label);
      label.querySelector("input").addEventListener("change", (e) => {
        this.editor.setOption(o.key, e.target.checked);
        this.afterEdit(true);
      });
    }

    $("show-raw").addEventListener("change", (e) => {
      this.display.raw = e.target.checked;
      this.svg.classList.toggle("raw-hidden", !this.display.raw);
      this.renderAll();
    });
    $("show-buffers").addEventListener("change", (e) => {
      this.display.buffers = e.target.checked;
      this.svg.classList.toggle("buffers-hidden", !this.display.buffers);
    });
    $("show-curved").addEventListener("change", (e) => {
      this.display.curved = e.target.checked;
      this.renderAll();
    });
    this.svg.classList.toggle("raw-hidden", !this.display.raw);

    for (const b of document.querySelectorAll("[data-tool]")) {
      b.addEventListener("click", () => this.setTool(b.dataset.tool));
    }
    for (const b of document.querySelectorAll("[data-zoom]")) {
      b.addEventListener("click", () => {
        if (b.dataset.zoom === "fit") this.fit();
        else this.zoomBy(b.dataset.zoom === "in" ? 1 / 1.25 : 1.25);
      });
    }
  }

  setTool(tool) {
    this.tool = tool;
    this.cancelDrag();
    for (const b of document.querySelectorAll("[data-tool]")) b.setAttribute("aria-pressed", String(b.dataset.tool === tool));
    this.svg.setAttribute("class", `tool-${tool}${this.display.raw ? "" : " raw-hidden"}${this.display.buffers ? "" : " buffers-hidden"}`);
    $("tool-hint").textContent = TOOL_HINTS[tool];
  }

  announce(text) {
    $("announce").textContent = text;
  }

  // --- view --------------------------------------------------------------

  /** world units per CSS pixel */
  unit() {
    const r = this.svg.getBoundingClientRect();
    if (!r.width || !r.height) return 1;
    return Math.max(this.view.w / r.width, this.view.h / r.height);
  }

  applyView() {
    const r = this.svg.getBoundingClientRect();
    if (r.width && r.height) {
      // Keep the centre and scale; match the element's aspect ratio.
      const u = this.unit();
      const cx = this.view.x + this.view.w / 2;
      const cy = this.view.y + this.view.h / 2;
      this.view = { x: cx - (r.width * u) / 2, y: cy - (r.height * u) / 2, w: r.width * u, h: r.height * u };
    }
    const { x, y, w, h } = this.view;
    this.svg.setAttribute("viewBox", `${x} ${y} ${w} ${h}`);
    const bg = $("backdrop");
    bg.setAttribute("x", x);
    bg.setAttribute("y", y);
    bg.setAttribute("width", w);
    bg.setAttribute("height", h);
    const u = this.unit();
    if (u !== this.lastUnit) {
      this.lastUnit = u;
      this.renderAll();
    }
  }

  fit() {
    const b = this.editor.bounds();
    const pad = 40;
    const r = this.svg.getBoundingClientRect();
    const w = b.right - b.left + 2 * pad;
    const h = b.bottom - b.top + 2 * pad;
    const u = r.width && r.height ? Math.max(w / r.width, h / r.height, 0.4) : 1;
    const cx = (b.left + b.right) / 2;
    const cy = (b.top + b.bottom) / 2;
    const vw = (r.width || w) * u;
    const vh = (r.height || h) * u;
    this.view = { x: cx - vw / 2, y: cy - vh / 2, w: vw, h: vh };
    this.applyView();
  }

  zoomBy(factor, around) {
    const { x, y, w, h } = this.view;
    const c = around ?? { x: x + w / 2, y: y + h / 2 };
    const nw = Math.min(Math.max(w * factor, 100), 20000);
    const f = nw / w;
    this.view = { x: c.x - (c.x - x) * f, y: c.y - (c.y - y) * f, w: w * f, h: h * f };
    this.applyView();
  }

  toWorld(e) {
    const p = new DOMPoint(e.clientX, e.clientY).matrixTransform(this.svg.getScreenCTM().inverse());
    return { x: p.x, y: p.y };
  }

  // --- rendering ---------------------------------------------------------

  buildDefs() {
    const defs = $("defs");
    const grid = svg("pattern", { id: "grid", width: 100, height: 100, patternUnits: "userSpaceOnUse" }, defs);
    for (let i = 20; i < 100; i += 20) {
      svg("path", { d: `M${i} 0V100M0 ${i}H100`, class: "grid-minor", "stroke-width": 1, "vector-effect": "non-scaling-stroke" }, grid);
    }
    svg("path", { d: "M0 0V100M0 0H100", class: "grid-major", "stroke-width": 1, "vector-effect": "non-scaling-stroke" }, grid);
    this.markers = [];
    for (let i = 0; i < 6; i++) {
      const m = svg("marker", { id: `arrow-${i}`, viewBox: "0 0 10 10", refX: 9, refY: 5, orient: "auto", markerUnits: "userSpaceOnUse" }, defs);
      svg("path", { d: "M0 1L10 5L0 9z", style: `fill: var(--c${i})` }, m);
      this.markers.push(m);
    }
  }

  /** Re-renders everything (structure, zoom or display changes). */
  renderAll() {
    if (!this.editor.router) return;
    this.render(null);
  }

  /** Renders after a transaction: connectors in `updated` only, or all when null. */
  render(updated) {
    const ed = this.editor;
    const r = ed.router;
    const u = this.unit();
    for (const m of this.markers) {
      m.setAttribute("markerWidth", 9 * u);
      m.setAttribute("markerHeight", 9 * u);
    }

    // clusters
    this.syncKeys(this.els.clusters, r.clusterIds(), (id) => {
      const g = svg("g", { class: "cluster", "data-kind": "cluster", "data-id": id }, this.layers.clusters);
      svg("path", {}, g);
      svg("text", {}, g);
      return g;
    });
    for (const [id, g] of this.els.clusters) {
      const c = ed.clusterGeometry(id);
      g.firstChild.setAttribute("d", pathData(c.polygon, true));
      const minX = Math.min(...c.polygon.map((p) => p.x));
      const minY = Math.min(...c.polygon.map((p) => p.y));
      g.lastChild.setAttribute("x", minX + 10);
      g.lastChild.setAttribute("y", minY + 20);
      g.lastChild.textContent = c.label;
    }

    // shapes, their pins and buffers
    this.syncKeys(this.els.shapes, r.shapeIds(), (id) => {
      const g = svg("g", { class: "shape", "data-kind": "shape", "data-id": id, tabindex: 0 }, this.layers.shapes);
      svg("path", { class: "body", "vector-effect": "non-scaling-stroke" }, g);
      svg("text", { class: "label" }, g);
      svg("g", { class: "pins" }, g);
      g.buffer = svg("path", { class: "buffer", "vector-effect": "non-scaling-stroke" }, this.layers.buffers);
      g.addEventListener("focus", () => this.select(id));
      return g;
    });
    for (const [id, g] of this.els.shapes) {
      const s = ed.shapeGeometry(id);
      const [body, label, pins] = g.children;
      body.setAttribute("d", pathData(s.polygon, true));
      label.setAttribute("x", (s.box.left + s.box.right) / 2);
      label.setAttribute("y", (s.box.top + s.box.bottom) / 2);
      label.textContent = s.label;
      g.setAttribute("aria-label", `Box ${s.label}`);
      g.classList.toggle("obstacle", s.pins.length === 0);
      g.buffer.setAttribute("d", pathData(s.routingPolygon, true));
      const size = 7 * u;
      while (pins.children.length > s.pins.length) pins.lastChild.remove();
      while (pins.children.length < s.pins.length) svg("rect", { class: "pin", "data-kind": "pin", "vector-effect": "non-scaling-stroke" }, pins);
      s.pins.forEach((pin, i) => {
        const el = pins.children[i];
        el.dataset.pin = pin.id;
        el.dataset.id = id;
        el.setAttribute("x", pin.x - size / 2);
        el.setAttribute("y", pin.y - size / 2);
        el.setAttribute("width", size);
        el.setAttribute("height", size);
      });
    }

    // connectors
    const connectorIds = r.connectorIds();
    this.syncKeys(this.els.connectors, connectorIds, (id) => {
      const g = svg("g", { "data-kind": "connector", "data-id": id }, this.layers.connectors);
      svg("path", { class: "conn-hit" }, g);
      svg("path", { class: "conn", "vector-effect": "non-scaling-stroke" }, g);
      g.raw = svg("path", { class: "raw", "vector-effect": "non-scaling-stroke" }, this.layers.raw);
      g.handles = svg("g", {}, this.layers.handles);
      g.fresh = true;
      return g;
    });
    const curve = this.display.curved ? 10 : 0;
    for (const [id, g] of this.els.connectors) {
      if (updated && !updated.has(id) && !g.fresh) continue;
      g.fresh = false;
      const c = ed.connectorGeometry(id, { curve });
      const colour = `c${c.colour}`;
      g.setAttribute("class", colour);
      g.handles.setAttribute("class", colour);
      const d = c.curved ? curvedPathData(c.curved) : pathData(c.display);
      g.children[0].setAttribute("d", d);
      const line = g.children[1];
      line.setAttribute("d", d);
      line.classList.toggle("selected", this.selection === id);
      if (c.ends?.destination.type === "junction") line.removeAttribute("marker-end");
      else line.setAttribute("marker-end", `url(#arrow-${c.colour})`);
      g.raw.setAttribute("d", this.display.raw ? pathData(c.raw) : "");
      this.renderHandles(g.handles, id, c, u);
    }

    // junctions (above connectors)
    this.syncKeys(this.els.junctions, r.junctionIds(), (id) => {
      const el = svg("circle", { class: "junction", "data-kind": "junction", "data-id": id, tabindex: 0, "vector-effect": "non-scaling-stroke" }, this.layers.handles);
      el.addEventListener("focus", () => this.select(id));
      return el;
    });
    for (const [id, el] of this.els.junctions) {
      const j = ed.junctionGeometry(id);
      el.setAttribute("cx", j.x);
      el.setAttribute("cy", j.y);
      el.setAttribute("r", 5.5 * u);
      el.classList.toggle("fixed", Boolean(j.fixed));
      el.setAttribute("aria-label", `Junction ${id}${j.fixed ? ", fixed" : ""}`);
      this.layers.handles.appendChild(el); // keep junctions on top of handle groups
    }

    for (const [id, el] of this.els.shapes) el.classList.toggle("selected", this.selection === id);
    for (const [id, el] of this.els.junctions) el.classList.toggle("selected", this.selection === id);
    if (this.selection && !ed.kindOf(this.selection)) this.select(null);
    else this.updateSelectionText();
  }

  renderHandles(group, id, c, u) {
    const items = [];
    for (const which of ["source", "destination"]) {
      const end = c.ends?.[which];
      if (end?.type === "point") items.push(["endpoint", which, end.position]);
    }
    c.checkpoints.forEach((p, i) => items.push(["checkpoint", i, p]));
    group.replaceChildren();
    for (const [kind, key, p] of items) {
      if (kind === "endpoint") {
        svg("circle", { class: "endpoint", "data-kind": "endpoint", "data-id": id, "data-which": key, cx: p.x, cy: p.y, r: 5 * u, "vector-effect": "non-scaling-stroke" }, group);
      } else {
        const s = 6 * u;
        svg("path", {
          class: "checkpoint",
          "data-kind": "checkpoint",
          "data-id": id,
          "data-index": key,
          d: `M${p.x} ${p.y - s}L${p.x + s} ${p.y}L${p.x} ${p.y + s}L${p.x - s} ${p.y}Z`,
          "vector-effect": "non-scaling-stroke",
        }, group);
      }
    }
  }

  syncKeys(map, ids, create) {
    const keep = new Set(ids);
    for (const [id, el] of map) {
      if (!keep.has(id)) {
        el.remove();
        el.buffer?.remove();
        el.raw?.remove();
        el.handles?.remove();
        map.delete(id);
      }
    }
    for (const id of ids) if (!map.has(id)) map.set(id, create(id));
  }

  /** Redraw after an editor call, and refresh the timing read-out. */
  afterEdit(full = false) {
    const last = this.editor.last;
    if (full || !last || last.structural) this.render(null);
    else this.render(new Set(last.updated));
    const stats = this.editor.stats();
    $("t-last").textContent = stats.ms === undefined ? "–" : fmtMs(stats.ms);
    $("t-median").textContent = stats.samples ? fmtMs(stats.median) : "–";
    $("t-updated").textContent = last ? `${last.updated.length} of ${last.total}` : "–";
  }

  // --- selection -----------------------------------------------------------

  select(id) {
    if (this.selection === id) return;
    this.selection = id;
    const panel = $("selection");
    const actions = $("selection-actions");
    actions.replaceChildren();
    const kind = id ? this.editor.kindOf(id) : null;
    panel.hidden = !kind;
    if (kind) {
      const button = (label, fn, cls) => {
        const b = document.createElement("button");
        b.type = "button";
        b.textContent = label;
        if (cls) b.className = cls;
        b.addEventListener("click", fn);
        actions.append(b);
      };
      if (kind === "shape" && this.editor.shapeGeometry(id).pins.length) {
        button("Rotate pins 90°", () => {
          this.editor.rotatePins(id);
          this.afterEdit(true);
        });
      }
      if (kind === "connector") {
        button("Clear checkpoints", () => {
          this.editor.transact(() => this.editor.router.setConnectorRoutingCheckpoints(id, []));
          this.afterEdit(true);
        });
      }
      if (kind === "junction") {
        const label = document.createElement("label");
        label.innerHTML = '<input type="checkbox"> Fixed position';
        const box = label.querySelector("input");
        box.checked = Boolean(this.editor.router.junctionPositionFixed(id));
        box.addEventListener("change", () => {
          this.editor.setJunctionFixed(id, box.checked);
          this.afterEdit(true);
        });
        actions.append(label);
        button("Reroute hyperedge", () => {
          this.editor.rerouteHyperedge(id);
          this.afterEdit(true);
        });
      }
      button("Delete", () => this.deleteObject(id), "danger");
    }
    this.renderSelectionClasses();
    this.updateSelectionText();
  }

  renderSelectionClasses() {
    for (const [id, el] of this.els.shapes) el.classList.toggle("selected", this.selection === id);
    for (const [id, el] of this.els.junctions) el.classList.toggle("selected", this.selection === id);
    for (const [id, g] of this.els.connectors) g.children[1].classList.toggle("selected", this.selection === id);
  }

  describeEnd(end) {
    if (!end) return "nothing";
    const r = this.editor.router;
    const p = `(${Math.round(end.position.x)}, ${Math.round(end.position.y)})`;
    if (end.type === "point") return `a free end at ${p}`;
    if (end.type === "junction") return `junction ${end.junctionId}`;
    if (end.type === "shapePin") {
      const label = this.editor.meta.get(end.shapeId)?.label ?? end.shapeId;
      const pin = end.classId === PIN_ANY ? "best side" : SIDE_NAMES[end.classId] ?? `pin class ${end.classId}`;
      return `${label} (${pin})`;
    }
    return r ? "an empty end" : "";
  }

  updateSelectionText() {
    const id = this.selection;
    const text = $("selection-text");
    if (!id) return;
    const ed = this.editor;
    const r = ed.router;
    const kind = ed.kindOf(id);
    if (kind === "shape") {
      const s = ed.shapeGeometry(id);
      text.textContent = `${s.label}: ${Math.round(s.box.right - s.box.left)} × ${Math.round(s.box.bottom - s.box.top)} at (${Math.round(s.box.left)}, ${Math.round(s.box.top)}), ${r.shapeAttachedConnectors(id).length} connectors. Arrow keys move it.`;
    } else if (kind === "connector") {
      const c = ed.connectorGeometry(id);
      const segs = Math.max(c.display.length - 1, 0);
      text.textContent = `Connector from ${this.describeEnd(c.ends?.source)} to ${this.describeEnd(c.ends?.destination)}: ${segs} segment${segs === 1 ? "" : "s"}, ${c.checkpoints.length} checkpoint${c.checkpoints.length === 1 ? "" : "s"}.`;
    } else if (kind === "junction") {
      const n = r.junctionAttachedConnectors(id).length;
      text.textContent = `Junction ${id} joining ${n} connector${n === 1 ? "" : "s"}. Arrow keys move it.`;
    } else if (kind === "cluster") {
      text.textContent = `Cluster ${ed.meta.get(id)?.label ?? id}.`;
    }
  }

  deleteObject(id) {
    const kind = this.editor.kindOf(id);
    if (!kind) return;
    this.editor.remove(id);
    if (this.selection === id) this.select(null);
    this.afterEdit(true);
    this.announce(`Deleted ${kind}`);
  }

  // --- pointer input -------------------------------------------------------

  bindCanvas() {
    const el = this.svg;
    el.addEventListener("pointerdown", (e) => this.onDown(e));
    el.addEventListener("pointermove", (e) => this.onMove(e));
    el.addEventListener("pointerup", (e) => this.onUp(e));
    el.addEventListener("pointercancel", (e) => this.onUp(e, true));
    el.addEventListener(
      "wheel",
      (e) => {
        e.preventDefault();
        this.zoomBy(Math.exp(e.deltaY * 0.0015), this.toWorld(e));
      },
      { passive: false },
    );
  }

  hitOf(target) {
    const el = target?.closest?.("[data-kind]");
    if (!el || !this.svg.contains(el)) return { kind: "canvas" };
    return { kind: el.dataset.kind, id: el.dataset.id, el };
  }

  endSpecFor(hit, p) {
    if (hit.kind === "pin") return { shapeId: hit.id, pinId: hit.el.dataset.pin };
    if (hit.kind === "shape" && !hit.el.classList.contains("obstacle")) {
      // The box body stands for its nearest pin (that is, any pin of an "any" box).
      const pins = this.editor.shapeGeometry(hit.id).pins;
      const dist = (q) => Math.hypot(q.x - p.x, q.y - p.y);
      return { shapeId: hit.id, pinId: pins.reduce((a, b) => (dist(a) <= dist(b) ? a : b)).id };
    }
    if (hit.kind === "junction") return { junctionId: hit.id };
    return [Math.round(p.x), Math.round(p.y)];
  }

  onDown(e) {
    if (e.pointerType === "mouse" && e.button !== 0) return;
    this.svg.setPointerCapture(e.pointerId);
    this.pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    if (this.pointers.size === 2) {
      // Two fingers: pinch-zoom / pan, abandoning a pan or connect gesture.
      if (!this.drag || this.drag.type === "pan" || this.drag.type === "connect") {
        this.cancelDrag();
        this.drag = { type: "pinch", ...this.pinchState() };
      }
      return;
    }
    if (this.pointers.size > 2) return;
    const p = this.toWorld(e);
    const hit = this.hitOf(e.target);
    const ed = this.editor;
    switch (this.tool) {
      case "move":
        if (hit.kind === "shape" || hit.kind === "pin") {
          this.select(hit.id);
          this.drag = { type: "shape", id: hit.id, last: p };
        } else if (hit.kind === "junction") {
          this.select(hit.id);
          const j = ed.router.junctionPosition(hit.id);
          this.drag = { type: "junction", id: hit.id, offset: { x: j.x - p.x, y: j.y - p.y } };
        } else if (hit.kind === "endpoint") {
          this.select(hit.id);
          this.drag = { type: "endpoint", id: hit.id, which: hit.el.dataset.which };
        } else if (hit.kind === "checkpoint") {
          this.select(hit.id);
          this.drag = { type: "checkpoint", id: hit.id, index: Number(hit.el.dataset.index) };
        } else if (hit.kind === "connector" || hit.kind === "cluster") {
          this.select(hit.id);
        } else {
          this.select(null);
          this.drag = { type: "pan", client: { x: e.clientX, y: e.clientY }, view: { ...this.view }, u: this.unit() };
          this.svg.classList.add("panning");
        }
        break;
      case "box": {
        const id = ed.addBox(p.x, p.y);
        this.afterEdit(true);
        this.setTool("move");
        this.select(id);
        this.announce("Added a box");
        break;
      }
      case "connect":
        this.drag = { type: "connect", src: this.endSpecFor(hit, p), from: this.anchorOf(hit, p) };
        break;
      case "junction":
        if (hit.kind === "connector") {
          const result = ed.splitConnector(hit.id, p);
          this.afterEdit(true);
          if (result) {
            this.select(result.junctionId);
            this.announce("Split the connector with a junction");
          } else {
            this.announce("This connector cannot be split here");
            $("tool-hint").textContent = "That connector cannot be split there (libavoid needs a segment between two bends, and no free destination end).";
          }
        } else if (hit.kind === "canvas") {
          const id = ed.addJunction(p.x, p.y);
          this.afterEdit(true);
          this.select(id);
          this.announce("Added a junction");
        }
        this.setTool("move");
        break;
      case "checkpoint":
        if (hit.kind === "connector") {
          ed.addCheckpoint(hit.id, { x: Math.round(p.x), y: Math.round(p.y) });
          this.select(hit.id);
          this.afterEdit(true);
          this.announce("Added a checkpoint");
        } else if (hit.kind === "checkpoint") {
          ed.removeCheckpoint(hit.id, Number(hit.el.dataset.index));
          this.afterEdit(true);
          this.announce("Removed a checkpoint");
        }
        break;
      case "delete":
        if (hit.kind === "checkpoint") {
          ed.removeCheckpoint(hit.id, Number(hit.el.dataset.index));
          this.afterEdit(true);
        } else if (hit.kind === "pin") {
          this.deleteObject(hit.id);
        } else if (hit.id) {
          this.deleteObject(hit.id);
        }
        break;
    }
  }

  anchorOf(hit, p) {
    const r = this.editor.router;
    if (hit.kind === "pin") return r.shapeConnectionPinPosition(hit.id, hit.el.dataset.pin);
    if (hit.kind === "junction") return r.junctionPosition(hit.id);
    if (hit.kind === "shape") {
      const b = r.shapeBoundingBox(hit.id);
      return { x: (b.left + b.right) / 2, y: (b.top + b.bottom) / 2 };
    }
    return p;
  }

  pinchState() {
    const [a, b] = [...this.pointers.values()];
    const mid = { clientX: (a.x + b.x) / 2, clientY: (a.y + b.y) / 2 };
    return { dist: Math.hypot(a.x - b.x, a.y - b.y), mid: this.toWorld(mid), midClient: mid, view: { ...this.view }, u: this.unit() };
  }

  onMove(e) {
    if (this.pointers.has(e.pointerId)) this.pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    if (!this.drag) return;
    const d = this.drag;
    if (d.type === "pinch") {
      if (this.pointers.size < 2) return;
      const [a, b] = [...this.pointers.values()];
      const dist = Math.hypot(a.x - b.x, a.y - b.y);
      const f = d.dist / Math.max(dist, 1);
      const midClient = { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
      const w = Math.min(Math.max(d.view.w * f, 100), 20000);
      const k = w / d.view.w;
      // Keep the world point that was under the initial midpoint under the fingers.
      const rect = this.svg.getBoundingClientRect();
      const u = d.u * k;
      this.view = {
        x: d.mid.x - (midClient.x - rect.left) * u,
        y: d.mid.y - (midClient.y - rect.top) * u,
        w: d.view.w * k,
        h: d.view.h * k,
      };
      this.applyView();
      return;
    }
    if (d.type === "pan") {
      this.view = {
        ...this.view,
        x: d.view.x - (e.clientX - d.client.x) * d.u,
        y: d.view.y - (e.clientY - d.client.y) * d.u,
      };
      this.applyView();
      return;
    }
    d.pointer = this.toWorld(e);
    if (d.type === "connect") {
      $("preview").setAttribute("d", `M${d.from.x} ${d.from.y}L${d.pointer.x} ${d.pointer.y}`);
      return;
    }
    this.schedule();
  }

  schedule() {
    if (!this.frame) this.frame = requestAnimationFrame(() => this.step());
  }

  /** Applies the latest drag position / slider value as one transaction. */
  step() {
    this.frame = 0;
    const ed = this.editor;
    if (this.pendingParam) {
      const [key, value] = this.pendingParam;
      this.pendingParam = null;
      ed.setParam(key, value);
      this.afterEdit();
    }
    const d = this.drag;
    if (!d || !d.pointer) return;
    const p = d.pointer;
    d.pointer = null;
    if (d.type === "shape") {
      const dx = Math.round(p.x - d.last.x);
      const dy = Math.round(p.y - d.last.y);
      if (!dx && !dy) return;
      d.last = { x: d.last.x + dx, y: d.last.y + dy };
      ed.moveShapeBy(d.id, dx, dy);
    } else if (d.type === "junction") {
      ed.moveJunction(d.id, Math.round(p.x + d.offset.x), Math.round(p.y + d.offset.y));
    } else if (d.type === "endpoint") {
      ed.moveEndpoint(d.id, d.which, Math.round(p.x), Math.round(p.y));
    } else if (d.type === "checkpoint") {
      ed.moveCheckpoint(d.id, d.index, { x: Math.round(p.x), y: Math.round(p.y) });
    } else {
      return;
    }
    this.afterEdit();
  }

  onUp(e, cancelled = false) {
    this.pointers.delete(e.pointerId);
    if (this.svg.hasPointerCapture(e.pointerId)) this.svg.releasePointerCapture(e.pointerId);
    const d = this.drag;
    if (!d) return;
    if (d.type === "pinch") {
      if (this.pointers.size === 0) this.drag = null;
      return;
    }
    if (d.pointer && !cancelled) {
      cancelAnimationFrame(this.frame);
      this.step();
    }
    if (d.type === "connect" && !cancelled) {
      const p = this.toWorld(e);
      const hit = this.hitOf(document.elementFromPoint(e.clientX, e.clientY));
      const dst = this.endSpecFor(hit, p);
      const same = JSON.stringify(dst) === JSON.stringify(d.src);
      const tooShort = Array.isArray(d.src) && Array.isArray(dst) && Math.hypot(dst[0] - d.src[0], dst[1] - d.src[1]) < 10 * this.unit();
      if (!same && !tooShort) {
        const id = this.editor.addConnector(d.src, dst);
        this.afterEdit(true);
        this.select(id);
        this.announce("Added a connector");
      }
    }
    this.cancelDrag();
  }

  cancelDrag() {
    this.drag = null;
    this.svg.classList.remove("panning");
    $("preview").setAttribute("d", "");
  }

  // --- keyboard ------------------------------------------------------------

  bindKeyboard() {
    const tools = { v: "move", b: "box", c: "connect", j: "junction", k: "checkpoint", x: "delete" };
    document.addEventListener("keydown", (e) => {
      const t = e.target;
      // Letter shortcuts work everywhere except where keys type or choose text.
      const inForm = t instanceof HTMLElement && t.closest('input:not([type="checkbox"]):not([type="radio"]):not([type="range"]), select, textarea');
      if (e.ctrlKey || e.metaKey || e.altKey) return;
      if (e.key === "Escape") {
        this.cancelDrag();
        this.setTool("move");
        this.select(null);
        return;
      }
      if (inForm) return;
      const onCanvasObject = t instanceof SVGElement && t.dataset.id;
      const arrows = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
      if (arrows[e.key] && onCanvasObject) {
        e.preventDefault();
        const step = e.shiftKey ? 1 : 10;
        const [dx, dy] = arrows[e.key].map((v) => v * step);
        const id = t.dataset.id;
        if (this.editor.kindOf(id) === "shape") this.editor.moveShapeBy(id, dx, dy);
        else {
          const j = this.editor.router.junctionPosition(id);
          this.editor.moveJunction(id, j.x + dx, j.y + dy);
        }
        this.afterEdit();
        return;
      }
      if ((e.key === "Delete" || e.key === "Backspace") && this.selection) {
        e.preventDefault();
        const wasFocused = onCanvasObject;
        this.deleteObject(this.selection);
        if (wasFocused) this.svg.focus();
        return;
      }
      if (e.key === "+" || e.key === "=") return this.zoomBy(1 / 1.25);
      if (e.key === "-") return this.zoomBy(1.25);
      if (e.key === "0") return this.fit();
      const tool = tools[e.key.toLowerCase()];
      if (tool) this.setTool(tool);
    });
  }
}
