"use strict";

// Every upstream libavoid test program (vendor/.../libavoid/tests/*.cpp, as
// the op scripts tests/fixtures/*.ops) replayed through the JavaScript API
// of the built package -- object-style calls (new Avoid.ShapeRef(router,
// poly, id), conn.setDestEndpoint(end), router.processTransaction(), ...),
// the same numeric ids -- and compared with the C++ goldens exactly as
// tests/fixtures_ops.rs compares the core:
//
//  * every `output` op: the diagram (shapes, junction positions, connector
//    display routes, in router order) must match tests/golden-bump/NAME.txt
//    (numbers within 1e-9; the dump is rebuilt from the public API:
//    router.m_obstacles, shape.polygon().offsetBoundingBox(0),
//    junction.position(), router.connRefs, conn.displayRoute());
//  * `delete`: the final state likewise, then router.delete();
//  * the fixture's exit code (tests/golden/NAME.exit) and, if it prints
//    (connector callbacks set with conn.setCallback(cb, conn), printf), its
//    stdout (tests/golden/NAME.stdout): this checks the callbacks' order.
//
// The audit scenes (tests/audit_scenes/*.ops, extra upstream API use:
// clusters, Router flag members, fixed routes, ...) are replayed too and
// compared bit for bit with their C++ goldens, except the ones that call
// the private ShapeRef::setCentrePos() (not part of the API) and those
// without a golden (undefined behaviour upstream; run only).
//
// The interpreter mirrors tests/common/fixture_ops.rs (format: see
// tools/fixtures/convert.py).  Slow fixtures (upstream takes minutes) run
// only with LIBAVOID_SLOW_FIXTURES=1.  FIXTURE=name runs one script.

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const Avoid = require("../index.cjs");

const root = path.join(__dirname, "..");
// unsatisfiableRangeAssertion takes ~2 minutes in wasm (the others marked slow in
// tests/fixtures_ops.rs take about a second here).
const SLOW = new Set(["unsatisfiableRangeAssertion"]);
const runSlow = !!process.env.LIBAVOID_SLOW_FIXTURES && process.env.LIBAVOID_SLOW_FIXTURES !== "0";
const only = process.env.FIXTURE || null;
const TOLERANCE = 1e-9;

class ScriptError extends Error {}

// --- C printf formatting --------------------------------------------------------

/** Exact decimal expansion rounded half-to-even at `digits` fraction digits (C printf). */
function roundDecimal(v, digits) {
  const exact = Math.abs(v).toFixed(100); // exact for the magnitudes used here
  const [ip, fp] = exact.split(".");
  let keep = ip + fp.slice(0, digits);
  const rest = fp.slice(digits);
  const half = rest[0] > "5" || (rest[0] === "5" && /[1-9]/.test(rest.slice(1)));
  const tie = rest[0] === "5" && !/[1-9]/.test(rest.slice(1));
  let up = half || (tie && Number(keep[keep.length - 1]) % 2 === 1);
  if (up) {
    const d = keep.split("").map(Number);
    let i = d.length - 1;
    while (i >= 0 && d[i] === 9) d[i--] = 0;
    if (i < 0) d.unshift(1);
    else d[i] += 1;
    keep = d.join("");
  }
  const intLen = keep.length - digits;
  const out = digits > 0 ? `${keep.slice(0, intLen)}.${keep.slice(intLen)}` : keep;
  return (v < 0 || Object.is(v, -0) ? "-" : "") + out.replace(/^0+(?=\d)/, "");
}

const printfF = (v) => roundDecimal(v, 6);

/** C `printf("%.17g", v)`. */
function formatG17(v) {
  if (Number.isNaN(v)) return "nan";
  if (!Number.isFinite(v)) return v < 0 ? "-inf" : "inf";
  if (v === 0) return Object.is(v, -0) ? "-0" : "0";
  const exp = Number(v.toExponential(16).split("e")[1]);
  if (exp < -4 || exp >= 17) {
    let [m, e] = v.toExponential(16).split("e");
    m = m.includes(".") ? m.replace(/0+$/, "").replace(/\.$/, "") : m;
    const en = Number(e);
    return `${m}e${en < 0 ? "-" : "+"}${String(Math.abs(en)).padStart(2, "0")}`;
  }
  let s = roundDecimal(v, 16 - exp);
  if (s.includes(".")) s = s.replace(/0+$/, "").replace(/\.$/, "");
  return s;
}

// --- diagram text (the oracle's outputDiagramText) rebuilt from the API -----------------

function diagramText(router) {
  let out = "";
  const g = formatG17;
  const obstacles = router.m_obstacles;
  for (const o of obstacles) {
    if (!(o instanceof Avoid.ShapeRef)) continue;
    const b = o.polygon().offsetBoundingBox(0);
    out += `rect\nid=${o.id()}\nx=${g(b.min.x)}\ny=${g(b.min.y)}\nwidth=${g(b.max.x - b.min.x)}\nheight=${g(b.max.y - b.min.y)}\n\n`;
  }
  for (const o of obstacles) {
    if (!(o instanceof Avoid.JunctionRef)) continue;
    const p = o.position();
    out += `junction\nid=${o.id()}\nx=${g(p.x)}\ny=${g(p.y)}\n\n`;
  }
  for (const c of router.connRefs) {
    const route = c.displayRoute();
    if (route.empty()) continue;
    out += `path\nid=${c.id()}\n`;
    for (let i = 0; i < route.size(); i++) out += `p${i}: ${g(route.ps[i].x)} ${g(route.ps[i].y)} \n`;
    out += "\n";
  }
  return out + "\n";
}

function blocks(text) {
  const out = [];
  let cur = [];
  for (const line of text.split("\n")) {
    if (line.trim() === "") {
      if (cur.length) out.push(cur);
      cur = [];
    } else cur.push(line);
  }
  if (cur.length) out.push(cur);
  return out;
}

// --- SVG output against the C++ (tests/golden-svg/manifest.txt: file sha256 bytes) -------

const svgGolden = new Map();
for (const line of fs.readFileSync(path.join(root, "tests", "golden-svg", "manifest.txt"), "utf8").split("\n")) {
  const m = /^(\S+\.svg) ([0-9a-f]{64}) \d+$/.exec(line);
  if (m) svgGolden.set(m[1], m[2]);
}
const svgStats = { compared: 0 };

/** `outputInstanceToSVG()` / `outputDiagramSVG()` at a diagram dump must be upstream's bytes. */
function compareSvg(router, goldenName) {
  for (const [file, svg] of [
    [`${goldenName}.svg`, () => router.outputInstanceToSVG()],
    [`${goldenName}-diagram.svg`, () => router.outputDiagramSVG()],
  ]) {
    const want = svgGolden.get(file);
    if (want === undefined) continue;
    const got = require("node:crypto").createHash("sha256").update(svg()).digest("hex");
    assert.equal(got, want, `${file}: SVG differs from the C++`);
    svgStats.compared++;
  }
}

/** tests/common/mod.rs compare_diagram_text(): returns max |diff| or throws. */
function compareDiagram(ours, golden, name) {
  const ob = blocks(ours);
  const gb = blocks(golden);
  let maxDiff = 0;
  const fail = (i, why) => {
    throw new Error(`${name}: diagram mismatch in block #${i + 1}: ${why}\n--- golden:\n${(gb[i] || []).join("\n")}\n--- ours:\n${(ob[i] || []).join("\n")}`);
  };
  for (let i = 0; i < Math.max(ob.length, gb.length); i++) {
    const o = ob[i];
    const g = gb[i];
    if (!o || !g) fail(i, "different number of blocks");
    if (o.length !== g.length) fail(i, `${o.length} lines vs golden ${g.length}`);
    for (let l = 0; l < o.length; l++) {
      const ot = o[l].split(/[\s=]+/).filter(Boolean);
      const gt = g[l].split(/[\s=]+/).filter(Boolean);
      if (ot.length !== gt.length) fail(i, `line ${l + 1} has a different token count`);
      for (let t = 0; t < ot.length; t++) {
        if (ot[t] === gt[t]) continue;
        const x = Number(ot[t]);
        const y = Number(gt[t]);
        if (ot[t] === "" || gt[t] === "" || Number.isNaN(x) || Number.isNaN(y)) fail(i, `line ${l + 1}: token ${ot[t]} vs golden ${gt[t]}`);
        const d = Math.abs(x - y);
        if (!(d <= TOLERANCE)) fail(i, `line ${l + 1}: value ${ot[t]} vs golden ${gt[t]} (|diff| = ${d})`);
        maxDiff = Math.max(maxDiff, d);
      }
    }
  }
  return maxDiff;
}

// --- script parsing (tests/common/fixture_ops.rs parse_script) -------------------------

function tokenize(s, err) {
  const toks = [];
  let i = 0;
  while (i < s.length) {
    const c = s[i];
    if (/\s/.test(c)) i++;
    else if (c === "(" || c === ")") toks.push(s[i++]);
    else if (c === '"') {
      let j = i + 1;
      while (j < s.length && s[j] !== '"') j += s[j] === "\\" ? 2 : 1;
      if (j >= s.length) err("unterminated string");
      toks.push(s.slice(i, j + 1));
      i = j + 1;
    } else {
      let j = i;
      while (j < s.length && !/\s/.test(s[j]) && s[j] !== "(" && s[j] !== ")") j++;
      toks.push(s.slice(i, j));
      i = j;
    }
  }
  return toks;
}

function unescapeC(lit, err) {
  const map = { n: "\n", t: "\t", r: "\r", 0: "\0", "\\": "\\", '"': '"', "'": "'" };
  return lit.slice(1, -1).replace(/\\(.)/g, (_, c) => map[c] ?? err(`unsupported escape \\${c}`));
}

function parseExpr(toks, pos, err) {
  const t = toks[pos.i++];
  if (t === undefined) err("unexpected end of expression");
  if (t === "(") {
    const f = toks[pos.i++];
    const args = [];
    for (;;) {
      if (toks[pos.i] === ")") {
        pos.i++;
        break;
      }
      if (toks[pos.i] === undefined) err("unbalanced parentheses");
      args.push(parseExpr(toks, pos, err));
    }
    return { call: f, args };
  }
  if (t === ")") err("unexpected )");
  if (t.startsWith('"')) return { str: unescapeC(t, err) };
  return { atom: t };
}

function parseFull(toks, start, err) {
  const pos = { i: start };
  const e = parseExpr(toks, pos, err);
  if (pos.i !== toks.length) err("trailing tokens");
  return e;
}

function parseScript(name, text) {
  let fixture = null;
  let golden = null;
  let prints = false;
  const stack = [{ then: [], els: [], inElse: false, cond: null }];
  const push = (s) => {
    const top = stack[stack.length - 1];
    (top.inElse ? top.els : top.then).push(s);
  };
  text.split("\n").forEach((raw, idx) => {
    const line = idx + 1;
    const s = raw.trim();
    if (!s || s.startsWith("#")) return;
    const err = (m) => {
      throw new ScriptError(`${name}.ops:${line}: ${m}`);
    };
    const toks = tokenize(s, err);
    const kw = toks[0];
    const st = { line, text: s };
    switch (kw) {
      case "fixture":
        fixture = toks[1];
        return;
      case "mode":
        golden = toks[1] === "golden";
        return;
      case "let":
        push({ ...st, op: "let", name: toks[1], type: toks[2], expr: parseFull(toks, 3, err) });
        return;
      case "decl":
        push({ ...st, op: "decl", name: toks[1], type: toks[2] });
        return;
      case "set": {
        const pos = { i: 1 };
        const lhs = parseExpr(toks, pos, err);
        push({ ...st, op: "set", lhs, expr: parseFull(toks, pos.i, err) });
        return;
      }
      case "do":
        push({ ...st, op: "do", expr: parseFull(toks, 1, err) });
        return;
      case "output":
      case "output-unchecked":
        push({ ...st, op: "output", router: toks[1], golden: toks[2], checked: kw === "output" });
        return;
      case "delete":
        push({ ...st, op: "delete", router: toks[1], golden: toks[2] === "-" ? null : toks[2] });
        return;
      case "callback":
        prints = true;
        push({ ...st, op: "callback", conn: toks[1], kind: toks[2] });
        return;
      case "print":
        prints = true;
        push({ ...st, op: "print", text: unescapeC(toks[1], err) });
        return;
      case "assert":
        push({ ...st, op: "assert", expr: parseFull(toks, 1, err) });
        return;
      case "ret":
        push({ ...st, op: "ret", expr: parseFull(toks, 1, err) });
        return;
      case "if":
        stack.push({ then: [], els: [], inElse: false, cond: parseFull(toks, 1, err), line, text: s });
        return;
      case "else":
        stack[stack.length - 1].inElse = true;
        return;
      case "end": {
        const b = stack.pop();
        push({ line: b.line, text: b.text, op: "if", cond: b.cond, then: b.then, els: b.els });
        return;
      }
      default:
        err(`unknown op ${kw}`);
    }
  });
  if (fixture !== name) throw new ScriptError(`${name}: script is for fixture ${fixture}`);
  return { golden, prints, body: stack[0].then };
}

// --- interpreter ------------------------------------------------------------------

const V = (t, v) => ({ t, v });
const POINTER_TYPES = new Set(["router", "conn", "shape", "junction", "pin", "rerouter", "cluster"]);

function copyPoint(p) {
  const q = new Avoid.Point(p.x, p.y);
  q.id = p.id;
  q.vn = p.vn;
  return q;
}

function copyPoly(p) {
  const q = new Avoid.Polygon();
  q._id = p._id;
  q.ps = p.ps.map(copyPoint);
  q.ts = p.ts.slice();
  q.checkpointsOnRoute = p.checkpointsOnRoute.map(([i, pt]) => [i, copyPoint(pt)]);
  return q;
}

function copyCheckpoint(c) {
  return new Avoid.Checkpoint(copyPoint(c.point), c.arrivalDirections, c.departureDirections);
}

/** C++ value semantics for variables: values are copied when stored. */
function copyValue(x) {
  switch (x.t) {
    case "point":
      return V("point", copyPoint(x.v));
    case "poly":
      return V("poly", copyPoly(x.v));
    case "checkpoint":
      return V("checkpoint", copyCheckpoint(x.v));
    case "checkpoints":
      return V("checkpoints", x.v.map(copyCheckpoint));
    case "connendlist":
      return V("connendlist", x.v.slice());
    default:
      return x;
  }
}

class Interp {
  constructor(name, script, auditMode) {
    this.name = name;
    this.script = script;
    this.audit = auditMode;
    this.router = null;
    this.routerVar = null;
    this.routerDeleted = false;
    this.vars = new Map();
    this.stdout = "";
    this.ret = null;
    this.line = 0;
    this.outputs = 0;
    this.identical = 0;
    this.maxDiff = 0;
  }

  err(m) {
    throw new ScriptError(`${this.name}.ops:${this.line}: ${m}`);
  }

  r() {
    if (this.routerDeleted) this.err("use of deleted router");
    if (!this.router) this.err("no router");
    return this.router;
  }

  num(x) {
    if (x.t === "int" || x.t === "double") return x.v;
    if (x.t === "bool") return x.v ? 1 : 0;
    return this.err(`expected a number, got ${x.t}`);
  }
  int(x) {
    if (x.t === "int") return x.v;
    if (x.t === "bool") return x.v ? 1 : 0;
    return this.err(`expected an integer, got ${x.t}`);
  }
  uint(x) {
    const i = this.int(x);
    if (i < 0 || i > 0xffffffff) this.err(`integer ${i} out of range for unsigned`);
    return i;
  }
  bool(x) {
    if (x.t === "bool") return x.v;
    if (x.t === "int" || x.t === "double") return x.v !== 0;
    return this.err(`expected a bool, got ${x.t}`);
  }
  is(x, t) {
    if (x.t === "null" && POINTER_TYPES.has(t)) this.err(`null ${t} dereferenced`);
    if (x.t !== t) this.err(`expected ${t}, got ${x.t}`);
    return x.v;
  }

  coerce(ty, x) {
    if (ty === x.t && ["int", "double", "bool"].includes(ty)) return x;
    if (ty === "int" && x.t === "bool") return V("int", x.v ? 1 : 0);
    if (ty === "int" && x.t === "double") return V("int", Math.trunc(x.v));
    if (ty === "double" && (x.t === "int" || x.t === "bool")) return V("double", this.num(x));
    if (ty === "bool") return V("bool", this.bool(x));
    if (x.t === "null" && POINTER_TYPES.has(ty)) return x;
    if (ty === x.t) return copyValue(x);
    if (ty === "junctionlist" && x.t === "junctionlist") return x;
    return this.err(`cannot store ${x.t} in a variable of type ${ty}`);
  }

  defaultValue(ty) {
    switch (ty) {
      case "connend":
        return V("connend", new Avoid.ConnEnd());
      case "poly":
        return V("poly", new Avoid.Polygon());
      case "checkpoints":
        return V("checkpoints", []);
      case "connendlist":
        return V("connendlist", []);
      case "ndlists":
        return V("ndlists", new Avoid.HyperedgeNewAndDeletedObjectLists());
      case "jiter":
        return V("jiter", { list: [], i: 0 });
      case "citer":
        return V("citer", { list: [], i: 0 });
      case "point":
        return V("point", new Avoid.Point());
      default:
        return this.err(`no default value for ${ty}`);
    }
  }

  variable(n) {
    if (!this.vars.has(n)) this.err(`unknown variable ${n}`);
    return this.vars.get(n);
  }

  eval(e) {
    if (e.str !== undefined) this.err("string value in expression");
    if (e.atom !== undefined) return this.atom(e.atom);
    return this.call(e.call, e.args);
  }

  atom(a) {
    if (a === "true") return V("bool", true);
    if (a === "false") return V("bool", false);
    if (a === "null") return V("null", null);
    if (/^[-.0-9]/.test(a)) {
      if (/^-?\d+$/.test(a)) return V("int", Number(a));
      const d = Number(a);
      if (Number.isNaN(d)) this.err(`bad numeric literal ${a}`);
      return V("double", d);
    }
    if (a === this.routerVar) {
      if (this.routerDeleted) this.err("use of deleted router");
      return V("router", this.router);
    }
    return this.variable(a).value;
  }

  arity(f, args, n) {
    if (args.length !== n) this.err(`${f} expects ${n} arguments, got ${args.length}`);
  }

  call(f, args) {
    if (f === "setPoint" || f === "resize" || f === "pushBack") {
      const name = args[0].atom;
      if (name === undefined) this.err(`${f} on a temporary`);
      const rest = args.slice(1).map((a) => this.eval(a));
      const target = this.variable(name).value;
      if (f === "setPoint" && target.t === "poly") target.v.setPoint(this.int(rest[0]), this.is(rest[1], "point"));
      else if (f === "resize" && target.t === "poly") {
        const n = this.int(rest[0]);
        target.v.ps = target.v.ps.slice(0, n);
        while (target.v.ps.length < n) target.v.ps.push(new Avoid.Point());
      } else if (f === "pushBack" && target.t === "checkpoints") target.v.push(copyCheckpoint(this.is(rest[0], "checkpoint")));
      else if (f === "pushBack" && target.t === "connendlist") target.v.push(this.is(rest[0], "connend"));
      else this.err(`${f} on ${target.t}`);
      return V("void");
    }
    const v = args.map((a) => this.eval(a));
    const ar = (n) => this.arity(f, args, n);
    const R = () => {
      if (v[0].t !== "router") this.err(`expected the router, got ${v[0].t}`);
      return this.r();
    };
    switch (f) {
      case "+":
      case "-":
      case "*":
      case "/":
      case "%":
      case "|":
      case "&":
      case "<":
      case ">":
      case "<=":
      case ">=":
      case "==":
      case "!=":
        ar(2);
        return this.binop(f, v[0], v[1]);
      case "and":
        ar(2);
        return V("bool", this.bool(v[0]) && this.bool(v[1]));
      case "or":
        ar(2);
        return V("bool", this.bool(v[0]) || this.bool(v[1]));
      case "not":
        ar(1);
        return V("bool", !this.bool(v[0]));
      case "neg":
        ar(1);
        if (v[0].t !== "int" && v[0].t !== "double") this.err("neg");
        return V(v[0].t, -v[0].v);
      case "?":
        ar(3);
        return this.bool(v[0]) ? v[1] : v[2];
      case "int":
      case "double":
      case "bool":
        ar(1);
        return this.coerce(f, v[0]);
      case "fabs":
        ar(1);
        return V("double", Math.abs(this.num(v[0])));
      // --- values ---
      case "Router":
        ar(1);
        if (this.router) this.err("a second Router is not supported");
        this.router = new Avoid.Router(this.uint(v[0]));
        return V("router", this.router);
      case "Point":
        ar(2);
        return V("point", new Avoid.Point(this.num(v[0]), this.num(v[1])));
      case "Polygon":
        ar(1);
        return V("poly", new Avoid.Polygon(this.int(v[0])));
      case "Rectangle":
        ar(2);
        return V("poly", new Avoid.Rectangle(this.is(v[0], "point"), this.is(v[1], "point")));
      case "ConnEnd":
        ar(0);
        return V("connend", new Avoid.ConnEnd());
      case "ConnEndPt1":
        ar(1);
        return V("connend", new Avoid.ConnEnd(this.is(v[0], "point")));
      case "ConnEndPt":
        ar(2);
        return V("connend", new Avoid.ConnEnd(this.is(v[0], "point"), this.uint(v[1])));
      case "ConnEndShape":
        ar(2);
        return V("connend", new Avoid.ConnEnd(this.is(v[0], "shape"), this.uint(v[1])));
      case "ConnEndJunction":
        ar(1);
        return V("connend", new Avoid.ConnEnd(this.is(v[0], "junction")));
      case "Checkpoint":
        ar(1);
        return V("checkpoint", new Avoid.Checkpoint(this.is(v[0], "point")));
      case "CheckpointDirs":
        ar(3);
        return V("checkpoint", new Avoid.Checkpoint(this.is(v[0], "point"), this.uint(v[1]), this.uint(v[2])));
      case "Checkpoints":
        ar(1);
        return V("checkpoints", Array.from({ length: this.int(v[0]) }, () => new Avoid.Checkpoint()));
      // --- objects ---
      case "ShapeRef":
        ar(3);
        return V("shape", new Avoid.ShapeRef(R(), this.is(v[1], "poly"), this.uint(v[2])));
      case "JunctionRef":
        ar(3);
        return V("junction", new Avoid.JunctionRef(R(), this.is(v[1], "point"), this.uint(v[2])));
      case "ConnRef":
        ar(2);
        return V("conn", new Avoid.ConnRef(R(), this.uint(v[1])));
      case "ConnRefEnds":
        ar(4);
        return V("conn", new Avoid.ConnRef(R(), this.is(v[1], "connend"), this.is(v[2], "connend"), this.uint(v[3])));
      case "Pin":
        ar(7);
        return V(
          "pin",
          new Avoid.ShapeConnectionPin(
            this.is(v[0], "shape"),
            this.uint(v[1]),
            this.num(v[2]),
            this.num(v[3]),
            this.bool(v[4]),
            this.num(v[5]),
            this.uint(v[6]),
          ),
        );
      case "PinOld":
        ar(6);
        return V(
          "pin",
          new Avoid.ShapeConnectionPin(this.is(v[0], "shape"), this.uint(v[1]), this.num(v[2]), this.num(v[3]), this.num(v[4]), this.uint(v[5])),
        );
      case "PinJunction":
        ar(3);
        return V("pin", new Avoid.ShapeConnectionPin(this.is(v[0], "junction"), this.uint(v[1]), this.uint(v[2])));
      case "ClusterRef":
        ar(3);
        return V("cluster", new Avoid.ClusterRef(R(), this.is(v[1], "poly"), this.uint(v[2])));
      case "deleteCluster":
        ar(2);
        R().deleteCluster(this.is(v[1], "cluster"));
        return V("void");
      case "clusterSetNewPoly":
        ar(2);
        this.is(v[0], "cluster").setNewPoly(this.is(v[1], "poly"));
        return V("void");
      case "setRouterFlag": {
        ar(3);
        const names = [
          "PartialTime",
          "SimpleRouting",
          "ClusteredRouting",
          "IgnoreRegions",
          "UseLeesAlgorithm",
          "InvisibilityGrph",
          "SelectiveReroute",
          "PartialFeedback",
          "RubberBandRouting",
        ];
        const n = names[this.int(v[1])];
        if (!n) this.err("bad router flag");
        R()[n] = this.bool(v[2]);
        return V("void");
      }
      case "markAllObstaclesAsMoved":
        ar(1);
        R().markAllObstaclesAsMoved();
        return V("void");
      case "setFixedExistingRoute":
        ar(1);
        this.is(v[0], "conn").setFixedExistingRoute();
        return V("void");
      case "setHateCrossings":
        ar(2);
        this.is(v[0], "conn").setHateCrossings(this.bool(v[1]));
        return V("void");
      case "setEndpoint":
        ar(3);
        this.is(v[0], "conn").setEndpoint(this.uint(v[1]), this.is(v[2], "connend"));
        return V("void");
      case "preferOrthogonalDimension":
        ar(2);
        this.is(v[0], "junction").preferOrthogonalDimension(this.uint(v[1]));
        return V("void");
      case "setConnectionCost":
        ar(2);
        this.is(v[0], "pin").setConnectionCost(this.num(v[1]));
        return V("void");
      // --- Router ---
      case "processTransaction":
        ar(1);
        return V("bool", R().processTransaction());
      case "setTransactionUse":
        ar(2);
        R().setTransactionUse(this.bool(v[1]));
        return V("void");
      case "setRoutingParameter":
        ar(3);
        R().setRoutingParameter(this.int(v[1]), this.num(v[2]));
        return V("void");
      case "setRoutingPenalty":
        ar(3);
        R().setRoutingPenalty(this.int(v[1]), this.num(v[2]));
        return V("void");
      case "setRoutingOption":
        ar(3);
        R().setRoutingOption(this.int(v[1]), this.bool(v[2]));
        return V("void");
      case "moveShape":
        ar(4);
        R().moveShape(this.is(v[1], "shape"), this.num(v[2]), this.num(v[3]));
        return V("void");
      case "moveShapePoly":
        ar(4);
        R().moveShape(this.is(v[1], "shape"), this.is(v[2], "poly"), this.bool(v[3]));
        return V("void");
      case "moveJunction":
        ar(4);
        R().moveJunction(this.is(v[1], "junction"), this.num(v[2]), this.num(v[3]));
        return V("void");
      case "moveJunctionPt":
        ar(3);
        R().moveJunction(this.is(v[1], "junction"), this.is(v[2], "point"));
        return V("void");
      case "deleteShape":
        ar(2);
        R().deleteShape(this.is(v[1], "shape"));
        return V("void");
      case "deleteConnector":
        ar(2);
        R().deleteConnector(this.is(v[1], "conn"));
        return V("void");
      case "deleteJunction":
        ar(2);
        R().deleteJunction(this.is(v[1], "junction"));
        return V("void");
      case "existsOrthogonalSegmentOverlap":
        ar(2);
        return V("bool", R().existsOrthogonalSegmentOverlap(this.bool(v[1])));
      case "existsOrthogonalFixedSegmentOverlap":
        ar(2);
        return V("bool", R().existsOrthogonalFixedSegmentOverlap(this.bool(v[1])));
      case "existsOrthogonalTouchingPaths":
        ar(1);
        return V("bool", R().existsOrthogonalTouchingPaths());
      case "existsCrossings":
        ar(2);
        return V("int", R().existsCrossings(this.bool(v[1])));
      case "existsInvalidOrthogonalPaths":
        ar(1);
        return V("bool", R().existsInvalidOrthogonalPaths());
      case "hyperedgeRerouter":
        ar(1);
        return V("rerouter", R().hyperedgeRerouter());
      case "registerHyperedgeForReroutingJunction":
        ar(2);
        return V("int", this.is(v[0], "rerouter").registerHyperedgeForRerouting(this.is(v[1], "junction")));
      case "registerHyperedgeForReroutingEnds":
        ar(2);
        return V("int", this.is(v[0], "rerouter").registerHyperedgeForRerouting(this.is(v[1], "connendlist")));
      case "newAndDeletedObjectListsFromHyperedgeImprovement":
        ar(1);
        return V("ndlists", R().newAndDeletedObjectListsFromHyperedgeImprovement());
      // --- ConnRef ---
      case "setSourceEndpoint":
        ar(2);
        this.is(v[0], "conn").setSourceEndpoint(this.is(v[1], "connend"));
        return V("void");
      case "setDestEndpoint":
        ar(2);
        this.is(v[0], "conn").setDestEndpoint(this.is(v[1], "connend"));
        return V("void");
      case "setEndpoints":
        ar(3);
        this.is(v[0], "conn").setEndpoints(this.is(v[1], "connend"), this.is(v[2], "connend"));
        return V("void");
      case "setRoutingType":
        ar(2);
        this.is(v[0], "conn").setRoutingType(this.uint(v[1]));
        return V("void");
      case "setRoutingCheckpoints":
        ar(2);
        this.is(v[0], "conn").setRoutingCheckpoints(this.is(v[1], "checkpoints"));
        return V("void");
      case "setFixedRoute":
        ar(2);
        this.is(v[0], "conn").setFixedRoute(this.is(v[1], "poly"));
        return V("void");
      case "clearFixedRoute":
        ar(1);
        this.is(v[0], "conn").clearFixedRoute();
        return V("void");
      case "makePathInvalid":
        ar(1);
        this.is(v[0], "conn").makePathInvalid();
        return V("void");
      case "displayRoute":
        ar(1);
        return V("poly", this.is(v[0], "conn").displayRoute());
      case "route":
        ar(1);
        return V("poly", this.is(v[0], "conn").route());
      case "splitAtSegment": {
        ar(2);
        const [j, c] = this.is(v[0], "conn").splitAtSegment(this.int(v[1]));
        return V("jcpair", [j, c]);
      }
      case "endpointConnEnds":
        ar(1);
        return V("endpair", this.is(v[0], "conn").endpointConnEnds());
      case "id": {
        ar(1);
        const x = v[0];
        if (!["conn", "shape", "junction"].includes(x.t)) this.err(`id() of ${x.t}`);
        return V("int", x.v.id());
      }
      // --- ShapeRef / JunctionRef / pins ---
      case "transformConnectionPinPositions":
        ar(2);
        this.is(v[0], "shape").transformConnectionPinPositions(this.uint(v[1]));
        return V("void");
      case "setPositionFixed":
        ar(2);
        this.is(v[0], "junction").setPositionFixed(this.bool(v[1]));
        return V("void");
      case "recommendedPosition":
        ar(1);
        return V("point", this.is(v[0], "junction").recommendedPosition());
      case "position":
        ar(1);
        return V("point", this.is(v[0], "junction").position());
      case "attachedConnectors":
        ar(1);
        return V("connlist", this.is(v[0], "junction").attachedConnectors());
      case "removeJunctionAndMergeConnectors": {
        ar(1);
        const c = this.is(v[0], "junction").removeJunctionAndMergeConnectors();
        return c ? V("conn", c) : V("null", null);
      }
      case "setExclusive":
        ar(2);
        this.is(v[0], "pin").setExclusive(this.bool(v[1]));
        return V("void");
      case "shape": {
        ar(1);
        const s = this.is(v[0], "connend").shape();
        return s ? V("shape", s) : V("null", null);
      }
      case "junction": {
        ar(1);
        const j = this.is(v[0], "connend").junction();
        return j ? V("junction", j) : V("null", null);
      }
      // --- plain data ---
      case "size": {
        ar(1);
        const x = v[0];
        if (x.t === "poly") return V("int", x.v.size());
        if (["connlist", "junctionlist", "checkpoints"].includes(x.t)) return V("int", x.v.length);
        return this.err(`size() of ${x.t}`);
      }
      case "ps": {
        ar(2);
        const p = this.is(v[0], "poly");
        const i = this.int(v[1]);
        if (i < 0 || i >= p.ps.length) this.err(`ps[${i}] out of range`);
        return V("point", p.ps[i]);
      }
      case "at": {
        ar(2);
        const l = this.is(v[0], "checkpoints");
        const i = this.int(v[1]);
        if (i < 0 || i >= l.length) this.err(`[${i}] out of range`);
        return V("checkpoint", l[i]);
      }
      case "x":
      case "y":
        ar(1);
        return V("double", this.is(v[0], "point")[f]);
      case "_id":
        ar(1);
        return V("int", this.is(v[0], "poly")._id);
      case "first":
      case "second": {
        ar(1);
        const k = f === "first" ? 0 : 1;
        const x = v[0];
        if (x.t === "jcpair") {
          const o = x.v[k];
          return o === null ? V("null", null) : V(k === 0 ? "junction" : "conn", o);
        }
        if (x.t === "endpair") return V("connend", x.v[k]);
        return this.err(`${f} of ${x.t}`);
      }
      case "newJunctionList":
      case "deletedJunctionList":
        ar(1);
        return V("junctionlist", this.is(v[0], "ndlists")[f]);
      case "newConnectorList":
      case "deletedConnectorList":
      case "changedConnectorList":
        ar(1);
        return V("connlist", this.is(v[0], "ndlists")[f]);
      case "begin":
      case "end": {
        ar(1);
        const x = v[0];
        const kind = x.t === "junctionlist" ? "jiter" : x.t === "connlist" ? "citer" : this.err(`${f}() of ${x.t}`);
        return V(kind, { list: x.v, i: f === "begin" ? 0 : x.v.length });
      }
      case "deref": {
        ar(1);
        const x = v[0];
        if ((x.t === "jiter" || x.t === "citer") && x.v.i < x.v.list.length) return V(x.t === "jiter" ? "junction" : "conn", x.v.list[x.v.i]);
        return this.err("dereference of past-the-end iterator");
      }
      case "next": {
        ar(1);
        const x = v[0];
        if ((x.t === "jiter" || x.t === "citer") && x.v.i < x.v.list.length) return V(x.t, { list: x.v.list, i: x.v.i + 1 });
        return this.err("increment of past-the-end iterator");
      }
      default:
        return this.err(`unknown function ${f}`);
    }
  }

  binop(op, a, b) {
    let eq = null;
    if (a.t === "null" && b.t === "null") eq = true;
    else if (a.t === "null" && POINTER_TYPES.has(b.t)) eq = false;
    else if (b.t === "null" && POINTER_TYPES.has(a.t)) eq = false;
    else if (a.t === b.t && ["conn", "shape", "junction", "pin"].includes(a.t)) eq = a.v === b.v; // wrapper identity
    else if (a.t === b.t && (a.t === "jiter" || a.t === "citer")) {
      eq = a.v.i === b.v.i && a.v.list.length === b.v.list.length && a.v.list.every((o, k) => o === b.v.list[k]);
    }
    if (eq !== null) {
      if (op === "==") return V("bool", eq);
      if (op === "!=") return V("bool", !eq);
      this.err(`operator ${op} on pointers`);
    }
    const isNum = (x) => x.t === "int" || x.t === "double" || x.t === "bool";
    if (!isNum(a) || !isNum(b)) this.err(`operator ${op} on ${a.t} and ${b.t}`);
    if (a.t !== "double" && b.t !== "double") {
      const x = this.int(a);
      const y = this.int(b);
      switch (op) {
        case "+":
          return V("int", x + y);
        case "-":
          return V("int", x - y);
        case "*":
          return V("int", x * y);
        case "/":
        case "%":
          if (y === 0) this.err("integer division by zero");
          return V("int", op === "/" ? Math.trunc(x / y) : x % y);
        case "|":
          return V("int", x | y);
        case "&":
          return V("int", x & y);
      }
    }
    const x = this.num(a);
    const y = this.num(b);
    switch (op) {
      case "+":
        return V("double", x + y);
      case "-":
        return V("double", x - y);
      case "*":
        return V("double", x * y);
      case "/":
        return V("double", x / y);
      case "<":
        return V("bool", x < y);
      case ">":
        return V("bool", x > y);
      case "<=":
        return V("bool", x <= y);
      case ">=":
        return V("bool", x >= y);
      case "==":
        return V("bool", x === y);
      case "!=":
        return V("bool", x !== y);
      default:
        return this.err(`operator ${op} on doubles`);
    }
  }

  assign(lhs, rhs) {
    if (lhs.atom !== undefined) {
      const slot = this.variable(lhs.atom);
      slot.value = this.coerce(slot.type, rhs);
      return;
    }
    if (lhs.call === undefined) this.err("cannot assign to a string");
    const name = lhs.args[0] && lhs.args[0].atom;
    if (name === undefined) this.err("unsupported assignment target");
    const idx = lhs.args.length > 1 ? this.int(this.eval(lhs.args[1])) : null;
    const target = this.variable(name).value;
    const f = lhs.call;
    if (f === "ps" && target.t === "poly" && rhs.t === "point") {
      if (idx < 0 || idx >= target.v.ps.length) this.err(`ps[${idx}] out of range`);
      target.v.ps[idx] = copyPoint(rhs.v);
    } else if (f === "at" && target.t === "checkpoints" && rhs.t === "checkpoint") {
      if (idx < 0 || idx >= target.v.length) this.err(`[${idx}] out of range`);
      target.v[idx] = copyCheckpoint(rhs.v);
    } else if ((f === "x" || f === "y") && target.t === "point") target.v[f] = this.num(rhs);
    else if (f === "_id" && target.t === "poly" && rhs.t === "int") target.v._id = rhs.v;
    else this.err(`cannot assign ${rhs.t} to ${f} of ${target.t}`);
  }

  execBlock(stmts) {
    for (const s of stmts) {
      if (this.ret !== null) this.err("statement after ret");
      this.line = s.line;
      try {
        this.exec(s);
      } catch (error) {
        if (!error.opLine) {
          error.opLine = true;
          error.message = `${this.name}.ops:${s.line} \`${s.text}\`: ${error.message}`;
        }
        throw error;
      }
    }
  }

  exec(s) {
    switch (s.op) {
      case "let": {
        if (this.vars.has(s.name)) this.err(`redeclaration of ${s.name}`);
        const x = this.eval(s.expr);
        if (s.type === "router") {
          if (x.t !== "router") this.err("router variable must be initialised with (Router ..)");
          this.routerVar = s.name;
          return;
        }
        this.vars.set(s.name, { type: s.type, value: this.coerce(s.type, x) });
        return;
      }
      case "decl":
        if (this.vars.has(s.name)) this.err(`redeclaration of ${s.name}`);
        this.vars.set(s.name, { type: s.type, value: this.defaultValue(s.type) });
        return;
      case "set":
        this.assign(s.lhs, this.eval(s.expr));
        return;
      case "do":
        this.eval(s.expr);
        return;
      case "output":
        this.checkRouterVar(s.router);
        if (this.script.golden && s.checked) this.compare(s.golden);
        else diagramText(this.r()); // displayRoute() caches routes: same side effects as upstream
        return;
      case "delete":
        this.checkRouterVar(s.router);
        if (s.golden && this.script.golden) this.compare(s.golden);
        else diagramText(this.r());
        this.r().delete();
        this.routerDeleted = true;
        return;
      case "callback": {
        const c = this.variable(s.conn).value;
        if (c.t !== "conn") this.err(`callback on ${c.t}`);
        const display = s.kind === "displayRoute";
        // upstream: conn->setCallback(cb, conn); cb prints the route.
        c.v.setCallback((conn) => {
          const route = display ? conn.displayRoute() : conn.route();
          this.stdout += `Connector ${conn.id()} needs rerouting!\nNew path: `;
          this.stdout += route.ps.map((p, i) => `${i > 0 ? "-" : ""}(${printfF(p.x)}, ${printfF(p.y)})`).join("");
          this.stdout += "\n";
        }, c.v);
        return;
      }
      case "print":
        this.stdout += s.text;
        return;
      case "assert":
        if (!this.bool(this.eval(s.expr))) throw new Error(`${this.name}: assertion from upstream fixture failed: ${s.text}`);
        return;
      case "if":
        this.execBlock(this.bool(this.eval(s.cond)) ? s.then : s.els);
        return;
      case "ret":
        this.ret = this.int(this.eval(s.expr));
        return;
      default:
        this.err(`unknown op ${s.op}`);
    }
  }

  checkRouterVar(n) {
    if (n !== this.routerVar) this.err(`${n} is not the router`);
  }

  compare(goldenName) {
    const text = diagramText(this.r());
    compareSvg(this.r(), goldenName);
    const dir = this.audit ? path.join(root, "tests", "audit_scenes", "golden") : path.join(root, "tests", "golden-bump");
    const golden = fs.readFileSync(path.join(dir, `${goldenName}.txt`), "utf8");
    this.outputs++;
    if (this.audit) {
      // audit scenes: bit for bit (as tests/common/fixture_ops.rs)
      assert.equal(text, golden, `audit scene ${this.name}: diagram ${goldenName} differs from the C++`);
      this.identical++;
      return;
    }
    const d = compareDiagram(text, golden, goldenName);
    this.maxDiff = Math.max(this.maxDiff, d);
    if (text === golden) this.identical++;
  }
}

function runScript(name, file, goldenDir, audit) {
  const script = parseScript(name, fs.readFileSync(file, "utf8"));
  const it = new Interp(name, script, audit);
  try {
    it.execBlock(script.body);
  } finally {
    if (it.router && !it.routerDeleted) it.router.delete(); // improveHyperedge05 leaks it upstream
  }
  if (it.ret === null) throw new ScriptError(`${name}: script ended without ret`);
  if (script.golden) {
    const expected = Number(fs.readFileSync(path.join(goldenDir, `${name}.exit`), "utf8").trim());
    assert.equal(it.ret & 0xff, expected, `${name}: main() returned ${it.ret} but upstream exited with ${expected}`);
    if (script.prints) {
      const golden = fs.readFileSync(path.join(goldenDir, `${name}.stdout`), "utf8");
      assert.equal(it.stdout, golden, `${name}: stdout (connector callbacks / printf) differs from upstream`);
    }
  }
  return { golden: script.golden, outputs: it.outputs, identical: it.identical, maxDiff: it.maxDiff, prints: script.prints };
}

function main() {
  const failures = [];
  const totals = { golden: 0, smoke: 0, skipped: [], outputs: 0, identical: 0, maxDiff: 0, stdout: 0 };
  const fixtureDir = path.join(root, "tests", "fixtures");
  const names = fs
    .readdirSync(fixtureDir)
    .filter((f) => f.endsWith(".ops"))
    .map((f) => f.slice(0, -4))
    .sort();
  assert.equal(names.length, 83, "83 upstream fixture scripts");
  for (const name of names) {
    if (only && only !== name) continue;
    if (SLOW.has(name) && !runSlow && !only) {
      totals.skipped.push(name);
      continue;
    }
    const t0 = Date.now();
    try {
      const r = runScript(name, path.join(fixtureDir, `${name}.ops`), path.join(root, "tests", "golden"), false);
      if (r.golden) {
        assert.ok(r.outputs > 0, `${name}: nothing compared`);
        totals.golden++;
        totals.outputs += r.outputs;
        totals.identical += r.identical;
        totals.maxDiff = Math.max(totals.maxDiff, r.maxDiff);
        if (r.prints) totals.stdout++;
      } else totals.smoke++;
      console.log(`ok - ${name} (${r.golden ? `${r.outputs} diagrams, ${r.identical} bit-identical` : "smoke: no upstream golden"}, ${Date.now() - t0} ms)`);
    } catch (error) {
      console.error(`not ok - ${name}: ${error.stack || error}`);
      failures.push(name);
    }
  }

  const audit = { compared: 0, outputs: 0, run: 0, setCentrePos: 0 };
  const auditDir = path.join(root, "tests", "audit_scenes");
  for (const name of fs.readdirSync(auditDir).filter((f) => f.endsWith(".ops")).map((f) => f.slice(0, -4)).sort()) {
    if (only && only !== name) continue;
    const file = path.join(auditDir, `${name}.ops`);
    if (/\(setCentrePos /.test(fs.readFileSync(file, "utf8"))) {
      audit.setCentrePos++; // private upstream (ShapeRef::setCentrePos): not API
      continue;
    }
    const hasGolden = fs.existsSync(path.join(auditDir, "golden", `${name}.exit`));
    try {
      if (hasGolden) {
        const r = runScript(name, file, path.join(auditDir, "golden"), true);
        audit.compared++;
        audit.outputs += r.outputs;
      } else {
        // Undefined behaviour upstream: no golden; must run without failing.
        const script = parseScript(name, fs.readFileSync(file, "utf8"));
        script.golden = false;
        const it = new Interp(name, script, true);
        try {
          it.execBlock(script.body);
        } finally {
          if (it.router && !it.routerDeleted) it.router.delete();
        }
        audit.run++;
      }
    } catch (error) {
      console.error(`not ok - ${name}: ${error.stack || error}`);
      failures.push(name);
    }
  }

  console.log(
    `fixtures: ${totals.golden} compared with C++ goldens (${totals.outputs} diagrams, ${totals.identical} byte-identical, ` +
      `max |diff| ${totals.maxDiff}; ${totals.stdout} with callback/printf stdout), ${totals.smoke} smoke (no upstream golden)` +
      (totals.skipped.length ? `, ${totals.skipped.length} slow skipped (${totals.skipped.join(", ")}; set LIBAVOID_SLOW_FIXTURES=1)` : ""),
  );
  console.log(`SVG output: ${svgStats.compared} files identical to the C++ (outputInstanceToSVG, outputDiagramSVG)`);
  console.log(
    `audit scenes: ${audit.compared} compared bit for bit (${audit.outputs} diagrams), ${audit.run} run without golden, ` +
      `${audit.setCentrePos} skipped (private ShapeRef::setCentrePos)`,
  );
  if (failures.length) {
    console.error(`FAILED: ${failures.join(", ")}`);
    process.exitCode = 1;
  } else {
    console.log("fixture replay through the JavaScript API passed");
  }
}

main();
