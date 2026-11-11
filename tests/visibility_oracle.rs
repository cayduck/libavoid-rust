//! Differential test against the C++ oracle
//! `tools/oracle/visibility_oracle.cpp` (poly-line visibility graph +
//! A* routing).  Replays exactly the same random scenes (same LCG, same
//! call order) and must reproduce the oracle's output byte for byte:
//! `tests/golden/visibility_oracle.txt` (main mode) and
//! `tests/golden/visibility_oracle_penalties.txt` (crossing / fixed shared
//! path penalties; needs `ConnectorCrossings`).

#![allow(clippy::uninlined_format_args)]

use std::fmt::Write as _;

use libavoid_rust::avoid::arena::ConnIdx;
use libavoid_rust::avoid::connectionpin::{
    ShapeConnectionPin, ATTACH_POS_BOTTOM, ATTACH_POS_CENTRE, ATTACH_POS_LEFT, ATTACH_POS_RIGHT, ATTACH_POS_TOP,
};
use libavoid_rust::avoid::connector::{Checkpoint, ConnRef};
use libavoid_rust::avoid::connend::ConnEnd;
use libavoid_rust::avoid::geomtypes::{Point, Polygon, Rectangle};
use libavoid_rust::avoid::router::{format_g, Router, RoutingParameter, POLY_LINE_ROUTING};
use libavoid_rust::avoid::shape::ShapeRef;
use libavoid_rust::avoid::vertices::{VertID, CONN_DIR_NONE};
use libavoid_rust::avoid::viscluster::ClusterRef;

fn g17(v: f64) -> String {
    format_g(v, 17)
}

// ---------------------------------------------------------------------------
// Deterministic LCG (identical to the C++ oracle).
struct Rng {
    state: u64,
}

impl Rng {
    fn next(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.state >> 33) as u32
    }

    fn rnd(&mut self, n: i32) -> i32 {
        (self.next() % (n as u32)) as i32
    }
}

// ---------------------------------------------------------------------------

struct Scene {
    router: Router,
    rng: Rng,
    penalty_mode: bool,
    rects_and_small_polys_only: bool,
    has_cluster: bool,
    shapes: Vec<Option<ShapeRef>>,
    shape_polys: Vec<Polygon>,
    shape_added_this_round: Vec<bool>,
    shape_has_pin: Vec<bool>,
    conns: Vec<Option<ConnIdx>>,
    conn_src: Vec<Point>,
    conn_dst: Vec<Point>,
    out: String,
}

const TRI_X: [f64; 3] = [1.0, 0.0, -1.0];
const TRI_Y: [f64; 3] = [-1.0, 1.0, -1.0];
const DIA_X: [f64; 4] = [1.0, 0.0, -1.0, 0.0];
const DIA_Y: [f64; 4] = [0.0, 1.0, 0.0, -1.0];
const HEX_X: [f64; 6] = [2.0, 1.0, -1.0, -2.0, -1.0, 1.0];
const HEX_Y: [f64; 6] = [0.0, 2.0, 2.0, 0.0, -2.0, -2.0];
const PEN_X: [f64; 5] = [2.0, 1.0, -1.0, -2.0, 0.0];
const PEN_Y: [f64; 5] = [0.0, 2.0, 2.0, 0.0, -2.0];

impl Scene {
    fn rnd(&mut self, n: i32) -> i32 {
        self.rng.rnd(n)
    }

    fn make_shape_poly(&mut self) -> Polygon {
        let mut kind = self.rnd(7);
        if self.rects_and_small_polys_only && (kind >= 5) {
            kind = 0;
        }
        if kind < 3 {
            let x0 = (5 * self.rnd(20)) as f64;
            let y0 = (5 * self.rnd(20)) as f64;
            let w = (5 * (1 + self.rnd(6))) as f64;
            let h = (5 * (1 + self.rnd(6))) as f64;
            return Rectangle::new(&Point::new(x0, y0), &Point::new(x0 + w, y0 + h));
        }
        let (tx, ty): (&[f64], &[f64]) = match kind {
            3 => (&TRI_X, &TRI_Y),
            4 => (&DIA_X, &DIA_Y),
            5 => (&HEX_X, &HEX_Y),
            _ => (&PEN_X, &PEN_Y),
        };
        let n = tx.len();
        let cx = (5 * self.rnd(20)) as f64;
        let cy = (5 * self.rnd(20)) as f64;
        let s = 2.5 * (1 + self.rnd(4)) as f64;
        let mut poly = Polygon::with_size(n);
        for i in 0..n {
            poly.ps[i] = Point::new(cx + s * tx[i], cy + s * ty[i]);
        }
        poly
    }

    fn grid_point(&mut self) -> Point {
        let x = (5 * self.rnd(21)) as f64;
        let y = (5 * self.rnd(21)) as f64;
        Point::new(x, y)
    }

    fn live_shapes(&self, for_deletion: bool) -> Vec<usize> {
        let mut live = Vec::new();
        for i in 0..self.shapes.len() {
            if self.shapes[i].is_none() {
                continue;
            }
            if self.penalty_mode && self.has_cluster && (i == 0) {
                continue;
            }
            // (Shapes with pins are never deleted: connectors attached to their
            // pins would be left referring to the deleted shape upstream.)
            if for_deletion && (self.shape_added_this_round[i] || self.shape_has_pin[i]) {
                continue;
            }
            live.push(i);
        }
        live
    }

    fn live_conns(&self) -> Vec<usize> {
        (0..self.conns.len()).filter(|&i| self.conns[i].is_some()).collect()
    }

    fn rand_point(&mut self) -> Point {
        let kind = self.rnd(8);
        let live: Vec<usize> = (0..self.shapes.len()).filter(|&i| self.shapes[i].is_some()).collect();
        if (kind <= 3) || live.is_empty() {
            return self.grid_point();
        }
        if kind == 7 {
            let lc = self.live_conns();
            if lc.is_empty() {
                return self.grid_point();
            }
            let c = lc[self.rnd(lc.len() as i32) as usize];
            let which = self.rnd(2);
            return if which == 0 { self.conn_src[c] } else { self.conn_dst[c] };
        }
        let s = live[self.rnd(live.len() as i32) as usize];
        let poly = self.shape_polys[s].clone();
        let n = poly.ps.len();
        if kind == 4 {
            // Centre of the bounding box (inside the shape).
            let (mut min_x, mut max_x) = (poly.ps[0].x, poly.ps[0].x);
            let (mut min_y, mut max_y) = (poly.ps[0].y, poly.ps[0].y);
            for i in 1..n {
                if poly.ps[i].x < min_x {
                    min_x = poly.ps[i].x;
                }
                if poly.ps[i].x > max_x {
                    max_x = poly.ps[i].x;
                }
                if poly.ps[i].y < min_y {
                    min_y = poly.ps[i].y;
                }
                if poly.ps[i].y > max_y {
                    max_y = poly.ps[i].y;
                }
            }
            return Point::new((min_x + max_x) / 2.0, (min_y + max_y) / 2.0);
        }
        let v = self.rnd(n as i32) as usize;
        if kind == 5 {
            // A shape vertex.
            return Point::new(poly.ps[v].x, poly.ps[v].y);
        }
        // kind == 6: an edge midpoint.
        let a = poly.ps[v];
        let b = poly.ps[(v + 1) % n];
        Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)
    }

    fn add_shape(&mut self) {
        let poly = self.make_shape_poly();
        let shape = ShapeRef::new(&mut self.router, &poly, 0);
        self.shapes.push(Some(shape));
        self.shape_polys.push(poly);
        self.shape_added_this_round.push(true);
        self.shape_has_pin.push(false);
        if self.rnd(4) == 0 {
            // A connection pin (class 1) at a proportional position on the
            // shape's bounding box.
            let px = self.rnd(3);
            let py = self.rnd(3);
            let xs = if px == 0 {
                ATTACH_POS_LEFT
            } else if px == 1 {
                ATTACH_POS_CENTRE
            } else {
                ATTACH_POS_RIGHT
            };
            let ys = if py == 0 {
                ATTACH_POS_TOP
            } else if py == 1 {
                ATTACH_POS_CENTRE
            } else {
                ATTACH_POS_BOTTOM
            };
            ShapeConnectionPin::new_shape(&mut self.router, shape, 1, xs, ys, true, 0.0, CONN_DIR_NONE);
            *self.shape_has_pin.last_mut().unwrap() = true;
        }
    }

    fn add_conn(&mut self) {
        let src = self.rand_point();
        let mut dst = self.rand_point();
        if src == dst {
            dst.x += 5.0;
        }
        let mut src_end = ConnEnd::from_point(src);
        let mut dst_end = ConnEnd::from_point(dst);
        let pin_shapes: Vec<usize> = (0..self.shapes.len())
            .filter(|&i| self.shapes[i].is_some() && self.shape_has_pin[i])
            .collect();
        if !pin_shapes.is_empty() {
            let mut src_pin_shape: Option<usize> = None;
            if self.rnd(4) == 0 {
                let s = pin_shapes[self.rnd(pin_shapes.len() as i32) as usize];
                src_pin_shape = Some(s);
                src_end = ConnEnd::from_shape_pin(&self.router, self.shapes[s].unwrap(), 1);
            }
            if self.rnd(4) == 0 {
                let s = pin_shapes[self.rnd(pin_shapes.len() as i32) as usize];
                if Some(s) != src_pin_shape {
                    dst_end = ConnEnd::from_shape_pin(&self.router, self.shapes[s].unwrap(), 1);
                }
            }
        }
        let conn = ConnRef::new_with_ends(&mut self.router, &src_end, &dst_end, 0);
        self.conns.push(Some(conn));
        self.conn_src.push(src);
        self.conn_dst.push(dst);
        if self.rnd(5) == 0 {
            let cp = self.grid_point();
            conn.set_routing_checkpoints(&mut self.router, &[Checkpoint::new(cp)]);
        }
    }

    fn line(&mut self, s: &str) {
        self.out.push_str(s);
        self.out.push('\n');
    }

    fn dump(&mut self, round: i32, transaction_result: bool) {
        fn id(v: &VertID) -> String {
            format!("{}.{}", v.obj_id, v.vn)
        }
        fn poly(label: &str, route: &Polygon) -> String {
            let mut s = format!(" {}[", label);
            for p in route.ps.iter() {
                let _ = write!(s, " ({},{},{},{})", g17(p.x), g17(p.y), p.id, p.vn);
            }
            s.push_str(" ]");
            s
        }
        let mut o = String::new();
        let r = &mut self.router;
        let _ = writeln!(o, "round {} transaction={}", round, transaction_result as i32);
        let end = r.vertices.end();
        let mut k = r.vertices.conns_begin();
        while k != end {
            let v = k.unwrap();
            let vi = &r.vert_infs[v];
            let _ = write!(o, "v {} p{} ({},{}) vis[", id(&vi.id), vi.id.props, g17(vi.point.x), g17(vi.point.y));
            for e in vi.vis_list.iter(&r.edge_infs) {
                let other = r.edge_infs[e].other_vert(v);
                let _ = write!(o, " {}", id(&r.vert_infs[other].id));
            }
            o.push_str(" ] inv[");
            for e in vi.invis_list.iter(&r.edge_infs) {
                let other = r.edge_infs[e].other_vert(v);
                let _ = write!(o, " {}", id(&r.vert_infs[other].id));
            }
            o.push_str(" ]\n");
            k = vi.lst_next;
        }
        let _ = writeln!(o, "vis {}", r.vis_graph.size());
        for e in r.vis_graph.to_vec(&r.edge_infs) {
            let ed = &r.edge_infs[e];
            let _ = writeln!(
                o,
                "{}-{} {}",
                id(&r.vert_infs[ed.m_vert1].id),
                id(&r.vert_infs[ed.m_vert2].id),
                g17(ed.m_dist)
            );
        }
        let _ = writeln!(o, "invis {}", r.invis_graph.size());
        for e in r.invis_graph.to_vec(&r.edge_infs) {
            let ed = &r.edge_infs[e];
            let _ = writeln!(
                o,
                "{}-{} b{} {}",
                id(&r.vert_infs[ed.m_vert1].id),
                id(&r.vert_infs[ed.m_vert2].id),
                ed.m_blocker,
                g17(ed.m_dist)
            );
        }
        for c in r.conn_refs.clone() {
            let _ = write!(o, "conn {} reroute={}", c.id(r), r.conns[c].m_needs_reroute_flag as i32);
            let route = c.route(r).clone();
            o.push_str(&poly("route", &route));
            let display = c.display_route(r).clone();
            o.push_str(&poly("display", &display));
            o.push('\n');
        }
        self.out.push_str(&o);
    }
}

fn run_scene(index: i32, penalty_mode: bool, out: &mut String) {
    let mut rng = Rng {
        state: 0x9E3779B97F4A7C15u64 ^ (index as u64).wrapping_mul(1000003),
    };
    rng.next();
    rng.next();

    let mut sc = Scene {
        router: Router::new(POLY_LINE_ROUTING),
        rng,
        penalty_mode,
        rects_and_small_polys_only: penalty_mode,
        has_cluster: false,
        shapes: Vec::new(),
        shape_polys: Vec::new(),
        shape_added_this_round: Vec::new(),
        shape_has_pin: Vec::new(),
        conns: Vec::new(),
        conn_src: Vec::new(),
        conn_dst: Vec::new(),
        out: std::mem::take(out),
    };

    sc.router.use_lees_algorithm = sc.rnd(2) == 1;
    sc.router.invisibility_grph = sc.rnd(2) == 1;
    sc.router.ignore_regions = sc.rnd(4) != 0;
    sc.router.rubber_band_routing = false;
    if sc.router.ignore_regions {
        sc.router.rubber_band_routing = sc.rnd(4) == 0;
    }
    sc.router.selective_reroute = sc.rnd(3) != 0;

    let sp = sc.rnd(3);
    sc.router.set_routing_parameter(
        RoutingParameter::SegmentPenalty,
        if sp == 0 {
            0.0
        } else if sp == 1 {
            10.0
        } else {
            50.0
        },
    );
    let ap = sc.rnd(3);
    sc.router.set_routing_parameter(
        RoutingParameter::AnglePenalty,
        if ap == 0 {
            0.0
        } else if ap == 1 {
            20.0
        } else {
            200.0
        },
    );
    let rp = sc.rnd(3);
    sc.router
        .set_routing_parameter(RoutingParameter::ReverseDirectionPenalty, if rp == 2 { 30.0 } else { 0.0 });
    if penalty_mode {
        let cp = sc.rnd(2);
        sc.router
            .set_routing_parameter(RoutingParameter::CrossingPenalty, if cp == 0 { 0.0 } else { 200.0 });
        let fp = sc.rnd(2);
        sc.router
            .set_routing_parameter(RoutingParameter::FixedSharedPathPenalty, if fp == 0 { 0.0 } else { 110.0 });
    } else {
        sc.router
            .set_routing_parameter(RoutingParameter::ClusterCrossingPenalty, 0.0);
    }

    let header = {
        let r = &sc.router;
        format!(
            "scene {} lees={} invis={} ignoreRegions={} rubberBand={} selective={} seg={} angle={} reverse={} crossing={} shared={} cluster={}",
            index,
            r.use_lees_algorithm as i32,
            r.invisibility_grph as i32,
            r.ignore_regions as i32,
            r.rubber_band_routing as i32,
            r.selective_reroute as i32,
            g17(r.routing_parameter(RoutingParameter::SegmentPenalty)),
            g17(r.routing_parameter(RoutingParameter::AnglePenalty)),
            g17(r.routing_parameter(RoutingParameter::ReverseDirectionPenalty)),
            g17(r.routing_parameter(RoutingParameter::CrossingPenalty)),
            g17(r.routing_parameter(RoutingParameter::FixedSharedPathPenalty)),
            g17(r.routing_parameter(RoutingParameter::ClusterCrossingPenalty))
        )
    };
    sc.line(&header);

    let n_shapes = 1 + sc.rnd(4);
    for _ in 0..n_shapes {
        sc.add_shape();
    }
    if !penalty_mode && (sc.rnd(4) == 0) {
        let x0 = (5 * sc.rnd(15)) as f64;
        let y0 = (5 * sc.rnd(15)) as f64;
        let w = (5 * (2 + sc.rnd(6))) as f64;
        let h = (5 * (2 + sc.rnd(6))) as f64;
        let cpoly = Rectangle::new(&Point::new(x0, y0), &Point::new(x0 + w, y0 + h));
        ClusterRef::new(&mut sc.router, &cpoly, 0);
        sc.has_cluster = true;
    }
    let n_conns = 1 + sc.rnd(4);
    for _ in 0..n_conns {
        sc.add_conn();
    }
    let result = sc.router.process_transaction();
    sc.dump(0, result);

    for round in 1..=3 {
        for f in sc.shape_added_this_round.iter_mut() {
            *f = false;
        }
        let n_actions = 1 + sc.rnd(3);
        for _ in 0..n_actions {
            let action = sc.rnd(7);
            if action == 0 {
                let live = sc.live_shapes(false);
                if live.is_empty() {
                    continue;
                }
                let s = live[sc.rnd(live.len() as i32) as usize];
                let dx = (5 * (sc.rnd(5) - 2)) as f64;
                let dy = (5 * (sc.rnd(5) - 2)) as f64;
                sc.line(&format!("action move-by {} {} {}", s, g17(dx), g17(dy)));
                let shape = sc.shapes[s].unwrap();
                sc.router.move_shape_by(shape, dx, dy);
                sc.shape_polys[s].translate(dx, dy);
            } else if action == 1 {
                let live = sc.live_shapes(false);
                if live.is_empty() {
                    continue;
                }
                let s = live[sc.rnd(live.len() as i32) as usize];
                let mut poly = sc.make_shape_poly();
                if poly.ps.len() != sc.shape_polys[s].ps.len() {
                    // Moves must keep the vertex count, so translate the
                    // current polygon instead.
                    poly = sc.shape_polys[s].clone();
                    let dx = (5 * (sc.rnd(9) - 4)) as f64;
                    let dy = (5 * (sc.rnd(9) - 4)) as f64;
                    poly.translate(dx, dy);
                }
                sc.line(&format!("action move-to {}", s));
                let shape = sc.shapes[s].unwrap();
                sc.router.move_shape(shape, &poly, false);
                sc.shape_polys[s] = poly;
            } else if action == 2 {
                if sc.router.rubber_band_routing {
                    continue;
                }
                let live = sc.live_shapes(true);
                if live.is_empty() {
                    continue;
                }
                let s = live[sc.rnd(live.len() as i32) as usize];
                sc.line(&format!("action delete-shape {}", s));
                let shape = sc.shapes[s].unwrap();
                sc.router.delete_shape(shape);
                sc.shapes[s] = None;
            } else if action == 3 {
                sc.line("action add-shape");
                sc.add_shape();
            } else if action == 4 {
                let lc = sc.live_conns();
                if lc.is_empty() {
                    continue;
                }
                let c = lc[sc.rnd(lc.len() as i32) as usize];
                let mut p = sc.rand_point();
                let which = sc.rnd(2);
                let conn = sc.conns[c].unwrap();
                if which == 0 {
                    if p == sc.conn_dst[c] {
                        p.x += 5.0;
                    }
                    sc.line(&format!("action set-source {}", c));
                    conn.set_source_endpoint(&mut sc.router, &ConnEnd::from_point(p));
                    sc.conn_src[c] = p;
                } else {
                    if p == sc.conn_src[c] {
                        p.x += 5.0;
                    }
                    sc.line(&format!("action set-dest {}", c));
                    conn.set_dest_endpoint(&mut sc.router, &ConnEnd::from_point(p));
                    sc.conn_dst[c] = p;
                }
            } else if action == 5 {
                let lc = sc.live_conns();
                if lc.is_empty() {
                    continue;
                }
                let c = lc[sc.rnd(lc.len() as i32) as usize];
                sc.line(&format!("action delete-conn {}", c));
                let conn = sc.conns[c].unwrap();
                sc.router.delete_connector(conn);
                sc.conns[c] = None;
            } else {
                sc.line("action add-conn");
                sc.add_conn();
            }
        }
        let result = sc.router.process_transaction();
        sc.dump(round, result);
    }

    sc.router.destruct();
    *out = sc.out;
}

fn run_oracle(penalty_mode: bool) -> String {
    let n_scenes = if penalty_mode { 100 } else { 300 };
    let mut out = String::new();
    for i in 0..n_scenes {
        run_scene(i, penalty_mode, &mut out);
    }
    out
}

fn compare_with_golden(ours: &str, golden_name: &str) {
    let path = format!("{}/tests/golden/{}", env!("CARGO_MANIFEST_DIR"), golden_name);
    let golden = std::fs::read_to_string(&path).expect("golden file");
    if ours == golden {
        return;
    }
    let gl: Vec<&str> = golden.lines().collect();
    let ol: Vec<&str> = ours.lines().collect();
    for i in 0..gl.len().max(ol.len()) {
        let g = gl.get(i).copied().unwrap_or("<EOF>");
        let m = ol.get(i).copied().unwrap_or("<EOF>");
        if g != m {
            // Find the enclosing scene header for context.
            let scene = gl[..i.min(gl.len())]
                .iter()
                .rev()
                .find(|l| l.starts_with("scene "))
                .copied()
                .unwrap_or("<none>");
            let round = gl[..i.min(gl.len())]
                .iter()
                .rev()
                .find(|l| l.starts_with("round "))
                .copied()
                .unwrap_or("<none>");
            panic!(
                "{} mismatch at line {}\n  in: {}\n  at: {}\n  golden: {}\n  ours:   {}",
                golden_name,
                i + 1,
                scene,
                round,
                g,
                m
            );
        }
    }
    panic!("{} mismatch (trailing newline?)", golden_name);
}

#[test]
fn visibility_oracle_matches_golden() {
    let ours = run_oracle(false);
    compare_with_golden(&ours, "visibility_oracle.txt");
}

#[test]
#[ignore = "needs ConnectorCrossings (connector.rs second half, todo! stub until merged)"]
fn visibility_oracle_penalties_matches_golden() {
    let ours = run_oracle(true);
    compare_with_golden(&ours, "visibility_oracle_penalties.txt");
}
