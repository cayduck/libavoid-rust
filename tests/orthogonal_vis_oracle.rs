//! Differential test against the C++ oracle
//! `tools/oracle/orthogonal_vis_oracle.cpp` (golden output in
//! `tests/golden/orthogonal_vis_oracle.txt`).
//!
//! The golden file holds, per scene, the scene DSL (see the oracle source
//! for the command reference) and the oracle's output.  This test replays
//! every scene through the Rust port and must reproduce the output byte for
//! byte: router vertex list (ids, points, visDirections, orthogVisPropFlags,
//! orthogonal partner), the visOrthogGraph edge list in order (endpoints,
//! dist, disabled) and every vertex's orthogVisList order -- or, for large
//! stages, the FNV-1a hash of exactly those lines -- plus
//! buildOrthogonalChannelInfo limits and the checkpoint cache.
//!
//! Set `ORACLE_FULL=1` to print full dumps for every stage (compare with
//! the oracle run with the same variable) and `ORACLE_SCENE=name` to run a
//! single scene.

#![allow(clippy::uninlined_format_args)]

use std::collections::{BTreeMap, HashMap};

use libavoid_rust::avoid::arena::{ConnIdx, EdgeIdx, PinIdx, VertIdx};
use libavoid_rust::avoid::connectionpin::ShapeConnectionPin;
use libavoid_rust::avoid::connector::{Checkpoint, ConnRef, ConnType};
use libavoid_rust::avoid::connend::ConnEnd;
use libavoid_rust::avoid::geomtypes::{Point, Polygon, Rectangle};
use libavoid_rust::avoid::junction::JunctionRef;
use libavoid_rust::avoid::router::*;
use libavoid_rust::avoid::scanline::{
    build_connector_route_checkpoint_cache, build_orthogonal_channel_info,
    clear_connector_route_checkpoint_cache, ShiftSegment, ShiftSegmentBase,
};
use libavoid_rust::avoid::shape::ShapeRef;

fn g17(v: f64) -> String {
    if v.is_nan() {
        return "nan".into();
    }
    format_g(v, 17)
}

fn pt(p: &Point) -> String {
    format!("({},{})", g17(p.x), g17(p.y))
}

fn fnv(lines: &[String]) -> u64 {
    let mut h: u64 = 1469598103934665603;
    for s in lines {
        for b in s.bytes().chain(std::iter::once(b'\n')) {
            h ^= b as u64;
            h = h.wrapping_mul(1099511628211);
        }
    }
    h
}

const FULL_DUMP_LIMIT: usize = 300;

fn dump_graph(r: &Router, stage: i32, full: bool, out: &mut Vec<String>) {
    let verts: Vec<VertIdx> = r.vertices.to_vec(&r.vert_infs);
    let v_index: HashMap<VertIdx, usize> = verts.iter().enumerate().map(|(i, &v)| (v, i)).collect();
    let edges: Vec<EdgeIdx> = r.vis_orthog_graph.to_vec(&r.edge_infs);
    let e_index: HashMap<EdgeIdx, usize> = edges.iter().enumerate().map(|(i, &e)| (e, i)).collect();
    out.push(format!(
        "stage {} verts={} conns={} edges={} vis={} invis={}",
        stage,
        verts.len(),
        r.vertices.conns_size(),
        edges.len(),
        r.vis_graph.size(),
        r.invis_graph.size()
    ));
    let mut lines = Vec::new();
    for (i, &v) in verts.iter().enumerate() {
        let vi = &r.vert_infs[v];
        let partner = match vi.m_orthogonal_partner {
            Some(p) => format!("{}", v_index[&p]),
            None => "-".into(),
        };
        lines.push(format!(
            "V {} [{},{},p={}] {} vis={} f={} o={} partner={}",
            i,
            vi.id.obj_id,
            vi.id.vn,
            vi.id.props,
            pt(&vi.point),
            vi.vis_directions,
            vi.orthog_vis_prop_flags,
            vi.orthog_vis_list.len(),
            partner
        ));
    }
    for (i, &e) in edges.iter().enumerate() {
        let ei = &r.edge_infs[e];
        lines.push(format!(
            "E {} {} {} d={} dis={}",
            i,
            v_index[&ei.m_vert1],
            v_index[&ei.m_vert2],
            g17(ei.get_dist()),
            ei.is_disabled() as i32
        ));
    }
    for (i, &v) in verts.iter().enumerate() {
        let vi = &r.vert_infs[v];
        if vi.orthog_vis_list.is_empty() {
            continue;
        }
        let mut s = format!("O {}:", i);
        for e in vi.orthog_vis_list.iter(&r.edge_infs) {
            s += &format!(" {}", e_index[&e]);
        }
        lines.push(s);
    }
    if full || lines.len() <= FULL_DUMP_LIMIT {
        out.extend(lines);
    } else {
        out.push(format!("hash lines={} fnv={:016x}", lines.len(), fnv(&lines)));
    }
}

/// Twin of the oracle's `TestShiftSegment`.
struct TestShiftSegment {
    base: ShiftSegmentBase,
    low: Point,
    high: Point,
}

impl ShiftSegment<()> for TestShiftSegment {
    fn base(&self) -> &ShiftSegmentBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut ShiftSegmentBase {
        &mut self.base
    }
    fn low_point(&self, _: &()) -> Point {
        self.low
    }
    fn high_point(&self, _: &()) -> Point {
        self.high
    }
    fn overlaps_with(&self, _rhs: &Self, _dim: usize, _: &()) -> bool {
        false
    }
    fn immovable(&self, _: &()) -> bool {
        false
    }
}

struct Scene {
    router: Option<Router>,
    shapes: BTreeMap<String, ShapeRef>,
    juncs: BTreeMap<String, JunctionRef>,
    conns: BTreeMap<String, ConnIdx>,
    pins: BTreeMap<String, PinIdx>,
    segs: Vec<TestShiftSegment>,
    stage: i32,
    full: bool,
    out: Vec<String>,
}

struct Toks<'a> {
    it: std::str::SplitWhitespace<'a>,
}

impl<'a> Toks<'a> {
    fn word(&mut self) -> &'a str {
        self.it.next().expect("missing token")
    }
    fn num(&mut self) -> f64 {
        self.word().parse::<f64>().expect("bad number")
    }
    fn unum(&mut self) -> u32 {
        self.word().parse::<u32>().expect("bad unsigned")
    }
}

fn read_end(sc: &Scene, t: &mut Toks) -> ConnEnd {
    let r = sc.router.as_ref().unwrap();
    match t.word() {
        "P" => {
            let x = t.num();
            let y = t.num();
            let dirs = t.unum();
            ConnEnd::from_point_dirs(Point::new(x, y), dirs)
        }
        "S" => {
            let s = sc.shapes[t.word()];
            let cls = t.unum();
            ConnEnd::from_shape_pin(r, s, cls)
        }
        "J" => {
            let j = sc.juncs[t.word()];
            ConnEnd::from_junction(r, j)
        }
        k => panic!("bad end {}", k),
    }
}

fn conn_type(t: u32) -> ConnType {
    match t {
        0 => ConnType::ConnTypeNone,
        1 => ConnType::ConnTypePolyLine,
        2 => ConnType::ConnTypeOrthogonal,
        _ => panic!("bad conn type"),
    }
}

fn run_command(sc: &mut Scene, line: &str) {
    let mut t = Toks {
        it: line.split_whitespace(),
    };
    let cmd = match t.it.next() {
        None => return,
        Some(c) => c,
    };
    if cmd.starts_with('#') {
        return;
    }
    if cmd == "router" {
        let flags = t.unum();
        sc.router = Some(Router::new(flags));
        return;
    }
    match cmd {
        "param" => {
            let k = t.unum() as usize;
            let v = t.num();
            sc.router.as_mut().unwrap().set_routing_parameter(RoutingParameter::ALL[k], v);
        }
        "option" => {
            let k = t.unum() as usize;
            let v = t.unum();
            sc.router.as_mut().unwrap().set_routing_option(RoutingOption::ALL[k], v != 0);
        }
        "shape" => {
            let name = t.word().to_string();
            let id = t.unum();
            let n = t.unum() as usize;
            let mut poly = Polygon::with_size(n);
            for i in 0..n {
                let x = t.num();
                let y = t.num();
                poly.ps[i] = Point::new(x, y);
            }
            let s = ShapeRef::new(sc.router.as_mut().unwrap(), &poly, id);
            sc.shapes.insert(name, s);
        }
        "rect" => {
            let name = t.word().to_string();
            let id = t.unum();
            let x1 = t.num();
            let y1 = t.num();
            let x2 = t.num();
            let y2 = t.num();
            let rect = Rectangle::new(&Point::new(x1, y1), &Point::new(x2, y2));
            let s = ShapeRef::new(sc.router.as_mut().unwrap(), &rect, id);
            sc.shapes.insert(name, s);
        }
        "junction" => {
            let name = t.word().to_string();
            let id = t.unum();
            let x = t.num();
            let y = t.num();
            let j = JunctionRef::new(sc.router.as_mut().unwrap(), Point::new(x, y), id);
            sc.juncs.insert(name, j);
        }
        "fixj" => {
            let j = sc.juncs[t.word()];
            let v = t.unum();
            j.set_position_fixed(sc.router.as_mut().unwrap(), v != 0);
        }
        "pin" => {
            let name = t.word().to_string();
            let s = sc.shapes[t.word()];
            let cls = t.unum();
            let xo = t.num();
            let yo = t.num();
            let prop = t.unum();
            let inside = t.num();
            let dirs = t.unum();
            let p = ShapeConnectionPin::new_shape(sc.router.as_mut().unwrap(), s, cls, xo, yo, prop != 0, inside, dirs);
            sc.pins.insert(name, p);
        }
        "jpin" => {
            let name = t.word().to_string();
            let j = sc.juncs[t.word()];
            let cls = t.unum();
            let dirs = t.unum();
            let p = ShapeConnectionPin::new_junction(sc.router.as_mut().unwrap(), j, cls, dirs);
            sc.pins.insert(name, p);
        }
        "pinexcl" => {
            let p = sc.pins[t.word()];
            let v = t.unum();
            p.set_exclusive(sc.router.as_mut().unwrap(), v != 0);
        }
        "conn" => {
            let name = t.word().to_string();
            let id = t.unum();
            let c = ConnRef::new(sc.router.as_mut().unwrap(), id);
            sc.conns.insert(name, c);
        }
        "connse" => {
            let name = t.word().to_string();
            let id = t.unum();
            let src = read_end(sc, &mut t);
            let dst = read_end(sc, &mut t);
            let c = ConnRef::new_with_ends(sc.router.as_mut().unwrap(), &src, &dst, id);
            sc.conns.insert(name, c);
        }
        "src" | "dst" => {
            let c = sc.conns[t.word()];
            let end = read_end(sc, &mut t);
            let r = sc.router.as_mut().unwrap();
            if cmd == "src" {
                c.set_source_endpoint(r, &end);
            } else {
                c.set_dest_endpoint(r, &end);
            }
        }
        "type" => {
            let c = sc.conns[t.word()];
            let ty = t.unum();
            c.set_routing_type(sc.router.as_mut().unwrap(), conn_type(ty));
        }
        "checkpoints" => {
            let c = sc.conns[t.word()];
            let n = t.unum();
            let mut cps = Vec::new();
            for _ in 0..n {
                let x = t.num();
                let y = t.num();
                let ad = t.unum();
                let dd = t.unum();
                cps.push(Checkpoint::with_directions(Point::new(x, y), ad, dd));
            }
            c.set_routing_checkpoints(sc.router.as_mut().unwrap(), &cps);
        }
        "move" => {
            let s = sc.shapes[t.word()];
            let dx = t.num();
            let dy = t.num();
            sc.router.as_mut().unwrap().move_shape_by(s, dx, dy);
        }
        "movej" => {
            let j = sc.juncs[t.word()];
            let dx = t.num();
            let dy = t.num();
            sc.router.as_mut().unwrap().move_junction_by(j, dx, dy);
        }
        "delshape" => {
            let name = t.word().to_string();
            let s = sc.shapes.remove(&name).unwrap();
            sc.router.as_mut().unwrap().delete_shape(s);
        }
        "delconn" => {
            let name = t.word().to_string();
            let c = sc.conns.remove(&name).unwrap();
            sc.router.as_mut().unwrap().delete_connector(c);
        }
        "process" => {
            let r = sc.router.as_mut().unwrap();
            r.process_actions();
            r.m_static_orthogonal_graph_invalidated = true;
            r.regenerate_static_built_graph();
            let stage = sc.stage;
            sc.stage += 1;
            dump_graph(r, stage, sc.full, &mut sc.out);
        }
        "obstacles" => {
            let r = sc.router.as_ref().unwrap();
            let mut s = String::from("obstacles");
            for &o in r.m_obstacles.iter() {
                s += &format!(" {}", r.obstacles[o].m_id);
            }
            sc.out.push(s);
        }
        "route" => {
            let c = sc.conns[t.word()];
            let n = t.unum() as usize;
            let mut poly = Polygon::with_size(n);
            for i in 0..n {
                let x = t.num();
                let y = t.num();
                poly.ps[i] = Point::new(x, y);
            }
            sc.router.as_mut().unwrap().conns[c].m_display_route = poly;
        }
        "cpcache" => {
            let r = sc.router.as_mut().unwrap();
            build_connector_route_checkpoint_cache(r);
            for c in r.conn_refs.clone() {
                let id = r.conns[c].m_id;
                let route = r.conns[c].display_route();
                let mut s = format!("C {} n={}", id, route.checkpoints_on_route.len());
                for (i, p) in route.checkpoints_on_route.iter() {
                    s += &format!(" {}{}", i, pt(p));
                }
                sc.out.push(s);
            }
            clear_connector_route_checkpoint_cache(r);
            for c in r.conn_refs.clone() {
                let id = r.conns[c].m_id;
                let n = r.conns[c].display_route().checkpoints_on_route.len();
                sc.out.push(format!("Cleared {} n={}", id, n));
            }
        }
        "seg" => {
            let dim = t.unum() as usize;
            let lx = t.num();
            let ly = t.num();
            let hx = t.num();
            let hy = t.num();
            let mn = t.num();
            let mx = t.num();
            let mut base = ShiftSegmentBase::new(sc.router.as_mut().unwrap(), dim);
            base.min_space_limit = mn;
            base.max_space_limit = mx;
            sc.segs.push(TestShiftSegment {
                base,
                low: Point::new(lx, ly),
                high: Point::new(hx, hy),
            });
        }
        "channel" => {
            let dim = t.unum() as usize;
            let r = sc.router.as_ref().unwrap();
            build_orthogonal_channel_info(r, &(), dim, &mut sc.segs);
            for (i, s) in sc.segs.iter().enumerate() {
                sc.out.push(format!(
                    "S {} min={} max={}",
                    i,
                    g17(s.base.min_space_limit),
                    g17(s.base.max_space_limit)
                ));
            }
            sc.segs.clear();
        }
        _ => panic!("unknown command: {}", line),
    }
}

struct GoldenScene {
    name: String,
    dsl: Vec<String>,
    out: Vec<String>,
}

fn parse_golden(text: &str) -> Vec<GoldenScene> {
    let mut scenes = Vec::new();
    let mut cur: Option<GoldenScene> = None;
    let mut in_out = false;
    for line in text.lines() {
        if let Some(name) = line.strip_prefix("#scene ") {
            cur = Some(GoldenScene {
                name: name.to_string(),
                dsl: Vec::new(),
                out: Vec::new(),
            });
            in_out = false;
        } else if line == "#out" {
            in_out = true;
        } else if line == "#end" {
            scenes.push(cur.take().unwrap());
        } else if let Some(c) = cur.as_mut() {
            if in_out {
                c.out.push(line.to_string());
            } else {
                c.dsl.push(line.to_string());
            }
        }
    }
    scenes
}

fn run_scene(dsl: &[String], full: bool) -> Vec<String> {
    let mut sc = Scene {
        router: None,
        shapes: BTreeMap::new(),
        juncs: BTreeMap::new(),
        conns: BTreeMap::new(),
        pins: BTreeMap::new(),
        segs: Vec::new(),
        stage: 0,
        full,
        out: Vec::new(),
    };
    for line in dsl {
        run_command(&mut sc, line);
    }
    sc.out
}

#[test]
fn orthogonal_vis_graph_matches_cpp_oracle() {
    let path = std::env::var("ORACLE_GOLDEN")
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/orthogonal_vis_oracle.txt").to_string());
    let text = std::fs::read_to_string(&path).expect("golden file");
    let scenes = parse_golden(&text);
    assert!(std::env::var("ORACLE_GOLDEN").is_ok() || scenes.len() > 100, "golden file looks truncated");
    let only = std::env::var("ORACLE_SCENE").ok();
    let full = std::env::var("ORACLE_FULL").is_ok();
    let mut failures = Vec::new();
    let mut stages = 0;
    for scene in scenes.iter() {
        if let Some(o) = only.as_ref() {
            if &scene.name != o {
                continue;
            }
        }
        let out = run_scene(&scene.dsl, full);
        if full {
            println!("#scene {}", scene.name);
            for l in out.iter() {
                println!("{}", l);
            }
            continue;
        }
        stages += out.iter().filter(|l| l.starts_with("stage ")).count();
        if out != scene.out {
            let first = out
                .iter()
                .zip(scene.out.iter())
                .position(|(a, b)| a != b)
                .unwrap_or(out.len().min(scene.out.len()));
            failures.push(format!(
                "{}: first difference at output line {}:\n  rust:   {}\n  oracle: {}",
                scene.name,
                first,
                out.get(first).map(|s| s.as_str()).unwrap_or("<none>"),
                scene.out.get(first).map(|s| s.as_str()).unwrap_or("<none>")
            ));
        }
    }
    if !full {
        println!("{} scenes, {} graph stages checked", scenes.len(), stages);
    }
    assert!(failures.is_empty(), "{} scene(s) differ:\n{}", failures.len(), failures.join("\n"));
}
