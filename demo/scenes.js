// Preset scenes.  Each `build(editor)` queues objects through the editor's
// building blocks (box / junction / cluster / connect); the editor then
// processes them in one transaction.  Coordinates are router units.

const DIAMOND = [
  [0.5, 0],
  [1, 0.5],
  [0.5, 1],
  [0, 0.5],
];

export const SCENES = [
  {
    id: "basics",
    name: "Getting started",
    blurb: "Drag a box and the connectors re-route. Each box has four pins and the router picks the best side. The diamond handle is a checkpoint and the open circle a free end; drag them too.",
    mode: "orthogonal",
    build(ed) {
      const browser = ed.box("Browser", 40, 210, 120, 60);
      const gateway = ed.box("API gateway", 280, 210, 130, 60);
      const auth = ed.box("Auth", 540, 70, 110, 56);
      const orders = ed.box("Orders", 540, 212, 110, 56);
      const cache = ed.box("Cache", 555, 340, 80, 80, { polygon: DIAMOND });
      const db = ed.box("Database", 800, 210, 120, 60);
      const log = ed.box("Audit log", 800, 450, 120, 56);
      ed.connect(browser, gateway);
      ed.connect(gateway, auth);
      ed.connect(gateway, orders);
      ed.connect(gateway, cache);
      ed.connect(orders, db);
      ed.connect(auth, db);
      ed.connect(cache, db);
      ed.connect(browser, log, { checkpoints: [[420, 520]] });
      ed.connect([150, 470], log);
    },
  },
  {
    id: "nudging",
    name: "Parallel connectors",
    blurb: "Five connectors share one channel. Nudging spreads them apart and orders them to avoid crossings; try the nudging distance slider.",
    mode: "orthogonal",
    params: { idealNudgingDistance: 12 },
    build(ed) {
      const left = [];
      const right = [];
      for (let i = 0; i < 5; i++) {
        left.push(ed.box(`In ${i + 1}`, 40, 40 + i * 110, 100, 50, { pins: "sides" }));
        right.push(ed.box(`Out ${i + 1}`, 820, 40 + i * 110, 100, 50, { pins: "sides" }));
      }
      ed.box("Block A", 380, 0, 200, 230, { pins: false });
      ed.box("Block B", 380, 340, 200, 250, { pins: false });
      for (let i = 0; i < 5; i++) {
        ed.connect(`${left[i]}.right`, `${right[(i + 2) % 5]}.left`);
      }
    },
  },
  {
    id: "pins",
    name: "Pins and junctions",
    blurb: "The controller has four numbered pins on its right edge. The power net fans out through a junction; drag the junction dot.",
    mode: "orthogonal",
    build(ed) {
      const { ConnDir } = ed.api;
      const ctrl = ed.box("Controller", 60, 120, 150, 300, { pins: "sides" });
      for (let i = 0; i < 4; i++) {
        ed.router.addShapeConnectionPinWithOptions(ctrl, `io${i + 1}`, {
          classId: 10 + i,
          xOffset: 1,
          yOffset: 0.2 + i * 0.2,
          proportional: true,
          visibilityDirections: ConnDir.Right,
        });
      }
      const devices = ["Motor", "Sensor", "Display", "Speaker"].map((name, i) => ed.box(name, 720, 30 + i * 130, 120, 56, { pins: "sides" }));
      ed.connect(`${ctrl}.io1`, `${devices[2]}.left`);
      ed.connect(`${ctrl}.io2`, `${devices[0]}.left`);
      ed.connect(`${ctrl}.io3`, `${devices[1]}.left`);
      ed.connect(`${ctrl}.io4`, `${devices[3]}.left`);
      ed.box("Shield", 400, 200, 90, 120, { pins: false });
      const power = ed.box("Power supply", 330, 560, 160, 56, { pins: "sides" });
      const j = ed.junction(410, 480);
      ed.connect(`${power}.top`, j);
      ed.connect(j, `${ctrl}.bottom`);
      ed.connect(j, `${devices[3]}.bottom`);
      ed.connect(j, `${devices[2]}.bottom`);
    },
  },
  {
    id: "hyperedge",
    name: "Hyperedge",
    blurb: "Six terminals joined by one junction. Hyperedge improvement adds and moves junctions; Reroute hyperedge rebuilds the whole tree.",
    mode: "orthogonal",
    options: { improveHyperedgeRoutesMovingAddingAndDeletingJunctions: true },
    actions: [
      {
        label: "Reroute hyperedge",
        run(ed, selection) {
          const ids = ed.router.junctionIds();
          const j = ids.includes(selection) ? selection : ids[0];
          if (j) ed.rerouteHyperedge(j);
        },
      },
    ],
    build(ed) {
      const spots = [
        ["North", 400, 0],
        ["East", 820, 160],
        ["South-east", 760, 470],
        ["South", 360, 540],
        ["West", 20, 400],
        ["North-west", 60, 80],
      ];
      const j = ed.junction(470, 300);
      for (const [name, x, y] of spots) ed.connect(ed.box(name, x, y, 120, 56), j);
      ed.box("Obstacle", 500, 210, 140, 70, { pins: false });
    },
  },
  {
    id: "clusters",
    name: "Clusters (poly-line)",
    blurb: "Poly-line routing with two clusters. The cluster crossing penalty keeps connectors from cutting through clusters they do not belong to.",
    mode: "polyline",
    params: { anglePenalty: 50 },
    build(ed) {
      ed.cluster("Frontend", 20, 20, 380, 300);
      ed.cluster("Backend", 540, 20, 400, 320);
      const ui = ed.box("UI", 60, 70, 110, 56);
      const state = ed.box("State", 240, 210, 120, 56);
      const api = ed.box("API", 590, 80, 110, 56);
      const jobs = ed.box("Jobs", 780, 230, 120, 56);
      const users = ed.box("Users", 60, 470, 120, 56);
      const storage = ed.box("Storage", 760, 500, 120, 56);
      ed.connect(users, ui);
      ed.connect(ui, state);
      ed.connect(ui, api);
      ed.connect(state, jobs);
      ed.connect(api, jobs);
      ed.connect(jobs, storage);
      ed.connect(users, storage);
    },
  },
  {
    id: "stress",
    name: "Stress test",
    blurb: "30 boxes and 45 connectors. Watch the timing while you drag; poly-line routing is much slower at this size.",
    mode: "orthogonal",
    build(ed) {
      const random = mulberry32(7);
      const boxes = [];
      for (let row = 0; row < 5; row++) {
        for (let col = 0; col < 6; col++) {
          const x = col * 160 + Math.round(random() * 30);
          const y = row * 120 + Math.round(random() * 24);
          boxes.push(ed.box(`${row * 6 + col + 1}`, x, y, 80, 44));
        }
      }
      for (let i = 0; i < 45; i++) {
        const a = Math.floor(random() * boxes.length);
        let b = Math.floor(random() * boxes.length);
        if (b === a) b = (b + 1) % boxes.length;
        ed.connect(boxes[a], boxes[b]);
      }
    },
  },
  {
    id: "empty",
    name: "Empty canvas",
    blurb: "Build your own: add boxes and junctions, then drag between pins to connect them.",
    mode: "orthogonal",
    build() {},
  },
];

function mulberry32(seed) {
  return () => {
    seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
