//! Synthetic benchmark suite: deterministic scenes exercising the main
//! features of the router, with time and heap measurements.
//!
//!   cargo run --release --example bench_suite -- [scenario|all] [small|medium|large] [repeat]
//!
//! Scenarios:
//!   ortho        orthogonal routing, free-floating point ends
//!   ortho_pen    ortho with crossing and fixed shared path penalties
//!   poly         poly-line routing, point ends
//!   poly_pen     poly with a crossing penalty
//!   pins         orthogonal routing to shape connection pins
//!   hyper        junctions (hyperedges) with hyperedge improvement
//!   cluster      poly-line routing with clusters
//!   checkpoints  orthogonal routing through checkpoints
//!   churn        shapes and connectors added and deleted every step
//!   edge_ends    orthogonal, point ends on the shapes' left and right sides
//!                with visibility directions, several sharing a point, in a
//!                layered left-to-right layout
//!   unify        two columns of k shapes, k Z-shaped connectors from the
//!                left one to the right one whose middle segments share the
//!                channel between them (the unifying nudging stage's slow
//!                case, see PORTING.md); k is the number of free segments
//!
//! For each scene: the first transaction, the median and the slowest later
//! step (moves plus transaction), the total time, the peak and the final
//! heap size, the number of allocations, and a checksum of the routes.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::time::Instant;

use libavoid_rust::avoid::arena::ConnIdx;
use libavoid_rust::avoid::connectionpin::{ShapeConnectionPin, ATTACH_POS_CENTRE};
use libavoid_rust::avoid::connector::{Checkpoint, ConnRef, ConnType};
use libavoid_rust::avoid::connend::ConnEnd;
use libavoid_rust::avoid::geomtypes::{Point, Rectangle};
use libavoid_rust::avoid::junction::JunctionRef;
use libavoid_rust::avoid::router::*;
use libavoid_rust::avoid::shape::ShapeRef;
use libavoid_rust::avoid::vertices::{CONN_DIR_DOWN, CONN_DIR_LEFT, CONN_DIR_RIGHT, CONN_DIR_UP};
use libavoid_rust::avoid::viscluster::ClusterRef;

// ---------------------------------------------------------------------------
// Heap accounting

struct Counting;

static CUR: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static COUNT: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = System.alloc(l);
        if !p.is_null() {
            let c = CUR.fetch_add(l.size(), Relaxed) + l.size();
            PEAK.fetch_max(c, Relaxed);
            COUNT.fetch_add(1, Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l);
        CUR.fetch_sub(l.size(), Relaxed);
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new_size: usize) -> *mut u8 {
        let q = System.realloc(p, l, new_size);
        if !q.is_null() {
            if new_size >= l.size() {
                let c = CUR.fetch_add(new_size - l.size(), Relaxed) + (new_size - l.size());
                PEAK.fetch_max(c, Relaxed);
            } else {
                CUR.fetch_sub(l.size() - new_size, Relaxed);
            }
            COUNT.fetch_add(1, Relaxed);
        }
        q
    }
}

// (On wasm32 the library installs its own allocator; heap figures read 0.)
#[cfg(not(target_arch = "wasm32"))]
#[global_allocator]
static GLOBAL: Counting = Counting;

// ---------------------------------------------------------------------------

/// xorshift64*: deterministic across platforms.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * ((self.next() >> 11) as f64 / (1u64 << 53) as f64)
    }
}

const CELL: f64 = 220.0;

#[derive(Clone, Copy)]
struct Geo {
    cx: f64,
    cy: f64,
    w: f64,
    h: f64,
}

#[derive(Clone, Copy)]
enum End {
    /// Free point beside a shape: (shape, side 0 = right / 1 = left, offset).
    Beside(usize, u8, f64),
    /// Point on a shape's side with visibility away from the shape:
    /// (shape, side 0 = right / 1 = left, offset).
    OnSide(usize, u8, f64),
    /// Pin class of a shape.
    Pin(usize, u32),
    Junction(usize),
}

struct Scene {
    r: Router,
    geo: Vec<Option<Geo>>,
    shapes: Vec<Option<ShapeRef>>,
    junctions: Vec<JunctionRef>,
    conns: Vec<Option<(ConnIdx, End, End)>>,
    rng: Rng,
    k: usize,
    orthogonal: bool,
    pins: bool,
    edge_ends: bool,
    adjacent_only: bool,
    /// With BENCH_CPP set: the API calls as C++ for upstream libavoid
    /// (see `cpp_program()`).
    cpp: Option<String>,
}

impl Scene {
    /// `aligned`: layered layout, every shape of a column with the same
    /// x and width.
    fn new(k: usize, orthogonal: bool, pins: bool, aligned: bool, seed: u64) -> Scene {
        let flags = if orthogonal { ORTHOGONAL_ROUTING } else { POLY_LINE_ROUTING };
        let r = Router::new(flags);
        let mut s = Scene {
            r,
            geo: Vec::new(),
            shapes: Vec::new(),
            junctions: Vec::new(),
            conns: Vec::new(),
            rng: Rng(seed | 1),
            k,
            orthogonal,
            pins,
            edge_ends: false,
            adjacent_only: false,
            cpp: std::env::var_os("BENCH_CPP").map(|_| String::new()),
        };
        s.log(|| format!("router = new Router({flags});"));
        s.parameter(RoutingParameter::ShapeBufferDistance, 8.0);
        s.parameter(RoutingParameter::IdealNudgingDistance, 8.0);
        s.option(RoutingOption::NudgeOrthogonalSegmentsConnectedToShapes, true);
        for j in 0..k {
            for i in 0..k {
                let mut g = Geo {
                    cx: i as f64 * CELL + s.rng.range(-30.0, 30.0),
                    cy: j as f64 * CELL + s.rng.range(-30.0, 30.0),
                    w: s.rng.range(60.0, 140.0),
                    h: s.rng.range(40.0, 100.0),
                };
                if aligned {
                    g.cx = i as f64 * CELL;
                    g.w = 160.0;
                }
                s.add_shape(g);
            }
        }
        s
    }

    fn log(&mut self, f: impl FnOnce() -> String) {
        if let Some(c) = self.cpp.as_mut() {
            c.push_str("    ");
            c.push_str(&f());
            c.push('\n');
        }
    }

    fn parameter(&mut self, p: RoutingParameter, v: f64) {
        self.r.set_routing_parameter(p, v);
        self.log(|| format!("router->setRoutingParameter((RoutingParameter){}, {v:?});", p as u32));
    }

    fn option(&mut self, o: RoutingOption, v: bool) {
        self.r.set_routing_option(o, v);
        self.log(|| format!("router->setRoutingOption((RoutingOption){}, {v});", o as u32));
    }

    fn transaction(&mut self) {
        self.log(|| "T();".to_string());
        if let (Ok(path), Some(code)) = (std::env::var("BENCH_CPP"), self.cpp.as_ref()) {
            // Written before the transaction too, for scenes that hang.
            std::fs::write(&path, cpp_program("partial", self.k, &format!("{code}    mark();\n"))).expect("BENCH_CPP");
        }
        self.r.process_transaction();
    }

    fn add_shape(&mut self, g: Geo) -> usize {
        let poly = Rectangle::from_centre(&Point::new(g.cx, g.cy), g.w, g.h);
        let sh = ShapeRef::new(&mut self.r, &poly, 0);
        self.log(|| {
            format!(
                "S.push_back(new ShapeRef(router, R({:?}, {:?}, {:?}, {:?})));",
                g.cx, g.cy, g.w, g.h
            )
        });
        if self.pins {
            let c = ATTACH_POS_CENTRE;
            let sides = [
                (1, 1.0, c, CONN_DIR_RIGHT),
                (2, 0.0, c, CONN_DIR_LEFT),
                (3, c, 0.0, CONN_DIR_UP),
                (4, c, 1.0, CONN_DIR_DOWN),
            ];
            for (class, x, y, dirs) in sides {
                let pin = ShapeConnectionPin::new_shape(&mut self.r, sh, class, x, y, true, 0.0, dirs);
                pin.set_exclusive(&mut self.r, false);
                self.log(|| {
                    format!(
                        "(new ShapeConnectionPin(S.back(), {class}, {x:?}, {y:?}, true, 0.0, {dirs}))->setExclusive(false);"
                    )
                });
            }
        }
        self.geo.push(Some(g));
        self.shapes.push(Some(sh));
        self.geo.len() - 1
    }

    fn conn_end_cpp(&self, e: End) -> String {
        match e {
            End::Beside(..) => {
                let p = self.conn_end(e).position(&self.r);
                format!("ConnEnd(Point({:?}, {:?}))", p.x, p.y)
            }
            End::OnSide(_, side, _) => {
                let p = self.conn_end(e).position(&self.r);
                let dir = if side == 0 { CONN_DIR_RIGHT } else { CONN_DIR_LEFT };
                format!("ConnEnd(Point({:?}, {:?}), {dir})", p.x, p.y)
            }
            End::Pin(s, class) => format!("ConnEnd(S[{s}], {class})"),
            End::Junction(j) => format!("ConnEnd(J[{j}])"),
        }
    }

    fn conn_end(&self, e: End) -> ConnEnd {
        match e {
            End::Beside(s, side, off) => {
                let g = self.geo[s].unwrap();
                let x = if side == 0 { g.cx + g.w / 2.0 + 20.0 } else { g.cx - g.w / 2.0 - 20.0 };
                ConnEnd::from_point(Point::new(x, g.cy + off * g.h / 2.0))
            }
            End::OnSide(s, side, off) => {
                let g = self.geo[s].unwrap();
                let (x, dir) = if side == 0 { (g.cx + g.w / 2.0, CONN_DIR_RIGHT) } else { (g.cx - g.w / 2.0, CONN_DIR_LEFT) };
                ConnEnd::from_point_dirs(Point::new(x, g.cy + off * g.h / 2.0), dir)
            }
            End::Pin(s, class) => ConnEnd::from_shape_pin(&self.r, self.shapes[s].unwrap(), class),
            End::Junction(j) => ConnEnd::from_junction(&self.r, self.junctions[j]),
        }
    }

    fn add_conn(&mut self, a: End, b: End) {
        let (ea, eb) = (self.conn_end(a), self.conn_end(b));
        let c = ConnRef::new_with_ends(&mut self.r, &ea, &eb, 0);
        if self.orthogonal {
            c.set_routing_type(&mut self.r, ConnType::ConnTypeOrthogonal);
        }
        self.conns.push(Some((c, a, b)));
        if self.cpp.is_some() {
            let (ca, cb) = (self.conn_end_cpp(a), self.conn_end_cpp(b));
            let t = if self.orthogonal { "C.back()->setRoutingType(ConnType_Orthogonal);" } else { "" };
            self.log(|| format!("C.push_back(new ConnRef(router, {ca}, {cb})); {t}"));
        }
    }

    /// A live shape near `s` (within `reach` cells), other than `s`.
    fn near(&mut self, s: usize, reach: usize) -> usize {
        let k = self.k as isize;
        let (i, j) = ((s % self.k) as isize, (s / self.k) as isize);
        loop {
            let di = self.rng.below(2 * reach + 1) as isize - reach as isize;
            let dj = self.rng.below(2 * reach + 1) as isize - reach as isize;
            let (ni, nj) = (i + di, j + dj);
            if (di, dj) == (0, 0) || ni < 0 || nj < 0 || ni >= k || nj >= k {
                continue;
            }
            let t = (nj * k + ni) as usize;
            if self.shapes.get(t).is_some_and(|x| x.is_some()) {
                return t;
            }
        }
    }

    fn random_shape(&mut self) -> usize {
        loop {
            let s = self.rng.below(self.shapes.len());
            if self.shapes[s].is_some() {
                return s;
            }
        }
    }

    /// A live shape 1-3 columns right of `s`, within 3 rows (layered,
    /// left-to-right flow), if any.
    fn downstream(&mut self, s: usize) -> Option<usize> {
        let (i, j) = (s % self.k, s / self.k);
        for _ in 0..50 {
            let ni = i + 1 + if self.adjacent_only { 0 } else { self.rng.below(3) };
            let nj = j as isize + self.rng.below(7) as isize - 3;
            if ni >= self.k || nj < 0 || nj >= self.k as isize {
                continue;
            }
            let t = nj as usize * self.k + ni;
            if self.shapes[t].is_some() {
                return Some(t);
            }
        }
        None
    }

    fn add_random_conn(&mut self) {
        let (a, b) = if self.edge_ends {
            loop {
                let a = self.random_shape();
                if let Some(b) = self.downstream(a) {
                    break (a, b);
                }
            }
        } else {
            let a = self.random_shape();
            (a, self.near(a, 3))
        };
        let (ea, eb) = if self.edge_ends {
            // Three port positions per side, so ends often share a point.
            let port = |r: &mut Rng| [-0.5, 0.0, 0.5][r.below(3)];
            let (oa, ob) = (port(&mut self.rng), port(&mut self.rng));
            (End::OnSide(a, 0, oa), End::OnSide(b, 1, ob))
        } else if self.pins {
            let (ca, cb) = (1 + self.rng.below(4) as u32, 1 + self.rng.below(4) as u32);
            (End::Pin(a, ca), End::Pin(b, cb))
        } else {
            let (oa, ob) = (self.rng.range(-0.8, 0.8), self.rng.range(-0.8, 0.8));
            (End::Beside(a, 0, oa), End::Beside(b, 1, ob))
        };
        self.add_conn(ea, eb);
    }

    /// Moves shape `s` and re-sets the point ends that follow it.
    fn move_shape(&mut self, s: usize, dx: f64, dy: f64) {
        let g = self.geo[s].as_mut().unwrap();
        g.cx += dx;
        g.cy += dy;
        self.r.move_shape_by(self.shapes[s].unwrap(), dx, dy);
        self.log(|| format!("router->moveShape(S[{s}], {dx:?}, {dy:?});"));
        for i in 0..self.conns.len() {
            if let Some((c, a, b)) = self.conns[i] {
                if matches!(a, End::Beside(x, ..) | End::OnSide(x, ..) if x == s) {
                    let e = self.conn_end(a);
                    c.set_source_endpoint(&mut self.r, &e);
                    let ce = self.conn_end_cpp(a);
                    self.log(|| format!("C[{i}]->setSourceEndpoint({ce});"));
                }
                if matches!(b, End::Beside(x, ..) | End::OnSide(x, ..) if x == s) {
                    let e = self.conn_end(b);
                    c.set_dest_endpoint(&mut self.r, &e);
                    let ce = self.conn_end_cpp(b);
                    self.log(|| format!("C[{i}]->setDestEndpoint({ce});"));
                }
            }
        }
    }

    fn delete_conn(&mut self, i: usize) {
        if let Some((c, ..)) = self.conns[i].take() {
            self.r.delete_connector(c);
            self.log(|| format!("router->deleteConnector(C[{i}]); C[{i}] = nullptr;"));
        }
    }

    fn delete_shape(&mut self, s: usize) {
        for i in 0..self.conns.len() {
            if let Some((_, a, b)) = self.conns[i] {
                let uses = |e: End| matches!(e, End::Beside(x, ..) | End::OnSide(x, ..) | End::Pin(x, _) if x == s);
                if uses(a) || uses(b) {
                    self.delete_conn(i);
                }
            }
        }
        let sh = self.shapes[s].take().unwrap();
        self.geo[s] = None;
        self.r.delete_shape(sh);
        self.log(|| format!("router->deleteShape(S[{s}]); S[{s}] = nullptr;"));
    }

    fn checksum(&mut self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for i in 0..self.conns.len() {
            if let Some((c, ..)) = self.conns[i] {
                for p in &c.display_route(&mut self.r).ps {
                    for v in [p.x, p.y] {
                        h ^= ((v * 100.0).round() as i64) as u64;
                        h = h.wrapping_mul(0x100_0000_01b3);
                    }
                }
            }
        }
        h
    }
}

fn build(name: &str, k: usize) -> Scene {
    let seed = 0x9e37_79b9_7f4a_7c15 ^ k as u64;
    let n_conns = k * k * 3 / 2;
    let orthogonal = !matches!(name, "poly" | "poly_pen" | "cluster");
    let pins = matches!(name, "pins" | "hyper");
    let layered = matches!(name, "edge_ends" | "unify");
    let mut s = Scene::new(k, orthogonal, pins, layered, seed);
    s.edge_ends = layered;
    s.adjacent_only = name == "unify";
    match name {
        "ortho_pen" => {
            s.parameter(RoutingParameter::CrossingPenalty, 200.0);
            s.parameter(RoutingParameter::FixedSharedPathPenalty, 110.0);
        }
        "poly_pen" => s.parameter(RoutingParameter::CrossingPenalty, 200.0),
        "cluster" => s.parameter(RoutingParameter::ClusterCrossingPenalty, 4000.0),
        "hyper" => s.option(RoutingOption::ImproveHyperedgeRoutesMovingAddingAndDeletingJunctions, true),
        _ => {}
    }
    if name == "unify" {
        // Only the first two columns (the shapes of the others are deleted
        // before the first transaction).
        for s_idx in 0..s.shapes.len() {
            if s_idx % k >= 2 {
                let sh = s.shapes[s_idx].take().unwrap();
                s.geo[s_idx] = None;
                s.r.delete_shape(sh);
                s.log(|| format!("router->deleteShape(S[{s_idx}]); S[{s_idx}] = nullptr;"));
            }
        }
        for row in 0..k {
            let (a, b) = (row * k, ((row + k / 2) % k) * k + 1);
            let port = |r: &mut Rng| [-0.5, 0.0, 0.5][r.below(3)];
            let (oa, ob) = (port(&mut s.rng), port(&mut s.rng));
            s.add_conn(End::OnSide(a, 0, oa), End::OnSide(b, 1, ob));
        }
    } else if name == "hyper" {
        // Hyperedges: a junction between a shape and 2-4 of its neighbours.
        for _ in 0..k * k / 3 {
            let a = s.random_shape();
            let g = s.geo[a].unwrap();
            let p = Point::new(g.cx + CELL / 2.0, g.cy + CELL / 2.0);
            let j = JunctionRef::new(&mut s.r, p, 0);
            s.junctions.push(j);
            s.log(|| format!("J.push_back(new JunctionRef(router, Point({:?}, {:?})));", p.x, p.y));
            let ji = s.junctions.len() - 1;
            s.add_conn(End::Pin(a, 1), End::Junction(ji));
            for _ in 0..2 + s.rng.below(3) {
                let b = s.near(a, 2);
                let class = 1 + s.rng.below(4) as u32;
                s.add_conn(End::Junction(ji), End::Pin(b, class));
            }
        }
    } else {
        for _ in 0..n_conns {
            s.add_random_conn();
        }
    }
    if name == "cluster" {
        // A cluster around each 2x2 block of cells.
        for bj in (0..k - 1).step_by(3) {
            for bi in (0..k - 1).step_by(3) {
                let lo = Point::new(bi as f64 * CELL - 100.0, bj as f64 * CELL - 90.0);
                let hi = Point::new((bi + 1) as f64 * CELL + 100.0, (bj + 1) as f64 * CELL + 90.0);
                ClusterRef::new(&mut s.r, &Rectangle::new(&lo, &hi), 0);
                s.log(|| {
                    format!(
                        "{{ Polygon p = Rectangle(Point({:?}, {:?}), Point({:?}, {:?})); new ClusterRef(router, p); }}",
                        lo.x, lo.y, hi.x, hi.y
                    )
                });
            }
        }
    }
    if name == "checkpoints" {
        for i in (0..s.conns.len()).step_by(3) {
            let (c, a, _) = s.conns[i].unwrap();
            let End::Beside(sa, ..) = a else { unreachable!() };
            let g = s.geo[sa].unwrap();
            let cp = Checkpoint::new(Point::new(g.cx + CELL / 2.0, g.cy + CELL / 2.0));
            c.set_routing_checkpoints(&mut s.r, &[cp]);
            s.log(|| {
                format!(
                    "C[{i}]->setRoutingCheckpoints(std::vector<Checkpoint>{{Checkpoint(Point({:?}, {:?}))}});",
                    cp.point.x, cp.point.y
                )
            });
        }
    }
    s
}

fn step(s: &mut Scene, name: &str) {
    if name == "churn" {
        for _ in 0..s.k {
            let live: Vec<usize> = (0..s.conns.len()).filter(|&i| s.conns[i].is_some()).collect();
            let i = live[s.rng.below(live.len())];
            s.delete_conn(i);
        }
        for _ in 0..2 {
            let v = s.random_shape();
            let g = s.geo[v].unwrap();
            s.delete_shape(v);
            s.add_shape(g);
        }
        while s.conns.iter().filter(|c| c.is_some()).count() < s.k * s.k * 3 / 2 {
            s.add_random_conn();
        }
    } else {
        for _ in 0..2 {
            let v = s.random_shape();
            let (dx, dy) = (s.rng.range(-12.0, 12.0).round(), s.rng.range(-12.0, 12.0).round());
            s.move_shape(v, dx, dy);
        }
    }
    s.transaction();
}

fn run(name: &str, k: usize, steps: usize) {
    let heap0 = CUR.load(Relaxed);
    PEAK.store(heap0, Relaxed);
    let count0 = COUNT.load(Relaxed);
    let t0 = Instant::now();
    let mut s = build(name, k);
    if let Ok(path) = std::env::var("BENCH_DUMP_SVG") {
        // outputInstanceToSVG(): the SVG carries C++ code that rebuilds the
        // scene (for running it through upstream).
        std::fs::write(&path, s.r.output_instance_to_svg_string()).expect("BENCH_DUMP_SVG");
    }
    s.transaction();
    let first = t0.elapsed().as_secs_f64() * 1e3;
    let mut times = Vec::new();
    let mut hash = s.checksum();
    s.log(|| "H = CK(); mark();".to_string());
    for _ in 0..steps {
        let t = Instant::now();
        step(&mut s, name);
        times.push(t.elapsed().as_secs_f64() * 1e3);
        hash = hash.rotate_left(7) ^ s.checksum();
        s.log(|| "H = ((H << 7) | (H >> 57)) ^ CK(); mark();".to_string());
    }
    if let (Ok(path), Some(code)) = (std::env::var("BENCH_CPP"), s.cpp.take()) {
        std::fs::write(&path, cpp_program(name, k, &code)).expect("BENCH_CPP");
    }
    let total = t0.elapsed().as_secs_f64() * 1e3;
    let live = CUR.load(Relaxed) - heap0;
    let (n_shapes, n_conns) = (
        s.shapes.iter().filter(|x| x.is_some()).count(),
        s.conns.iter().filter(|x| x.is_some()).count(),
    );
    drop(s);
    let leaked = CUR.load(Relaxed) as isize - heap0 as isize;
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "{:<12} k={:<3} shapes {:>4} conns {:>5} | first {:>9.1} ms  step med {:>8.1} max {:>8.1}  total {:>9.0} ms | heap peak {:>7.1} MB live {:>6.1} MB after-drop {:>+6} B | allocs {:>9} | {:016x}",
        name,
        k,
        n_shapes,
        n_conns,
        first,
        times[times.len() / 2],
        times[times.len() - 1],
        total,
        (PEAK.load(Relaxed) - heap0) as f64 / 1048576.0,
        live as f64 / 1048576.0,
        leaked,
        COUNT.load(Relaxed) - count0,
        hash
    );
}

/// A C++ program for upstream libavoid replaying the recorded calls and
/// printing the same figures (time only) and checksum.
fn cpp_program(name: &str, k: usize, body: &str) -> String {
    format!(
        r#"#include "libavoid/libavoid.h"
#include <chrono>
#include <cmath>
#include <cstdio>
#include <algorithm>
#include <vector>
using namespace Avoid;
static Router *router;
static std::vector<ShapeRef *> S;
static std::vector<ConnRef *> C;
static std::vector<JunctionRef *> J;
static unsigned long long H;
static std::vector<double> marks;
static auto t0 = std::chrono::steady_clock::now();
static double now() {{ return std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - t0).count(); }}
static void T() {{ router->processTransaction(); }}
static Polygon poly;
static Polygon &R(double x, double y, double w, double h) {{ poly = Rectangle(Point(x, y), w, h); return poly; }}
static void mark() {{ marks.push_back(now()); }}
static unsigned long long CK() {{
    unsigned long long h = 0xcbf29ce484222325ULL;
    for (ConnRef *c : C) {{
        if (!c) continue;
        const PolyLine &r = c->displayRoute();
        for (const Point &p : r.ps) for (double v : {{p.x, p.y}}) {{
            h ^= (unsigned long long)(long long)std::round(v * 100.0);
            h *= 0x100000001b3ULL;
        }}
    }}
    return h;
}}
int main() {{
{body}    std::vector<double> steps;
    for (size_t i = 1; i < marks.size(); ++i) steps.push_back(marks[i] - marks[i - 1]);
    std::sort(steps.begin(), steps.end());
    if (steps.empty()) steps.push_back(0.0);
    printf("{name:<12} k={k:<3} (C++) first %9.1f ms  step med %8.1f max %8.1f  total %9.0f ms | %016llx\n",
        marks[0], steps[steps.size() / 2], steps.back(), marks.back(), H);
    return 0;
}}
"#
    )
}

const ALL: [&str; 11] = [
    "ortho",
    "ortho_pen",
    "poly",
    "poly_pen",
    "pins",
    "hyper",
    "cluster",
    "checkpoints",
    "churn",
    "edge_ends",
    "unify",
];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let which = args.get(1).map_or("all", |s| s.as_str());
    let k = match args.get(2).map_or("small", |s| s.as_str()) {
        "small" => 8,
        "medium" => 14,
        "large" => 22,
        n => n.parse().expect("size"),
    };
    let repeat: usize = args.get(3).map_or(1, |s| s.parse().unwrap());
    let names: Vec<&str> = if which == "all" { ALL.to_vec() } else { which.split(',').collect() };
    for _ in 0..repeat {
        for name in &names {
            run(name, k, 12);
        }
    }
}
