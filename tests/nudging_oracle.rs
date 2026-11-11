//! Differential test against the C++ oracle `tools/oracle/nudging_oracle.cpp`
//! (golden output in `tests/golden/nudging_oracle.txt`, regenerated with
//! `tools/oracle/nudging_oracle.sh`).
//!
//! The golden file holds, per random scene, the scene DSL (see the oracle
//! source for the command reference) and the oracle's output.  This test
//! replays every scene through the Rust port and must reproduce the output
//! byte for byte: after every `processTransaction()` the nudging trace
//! (every region of `nudgeOrthogonalRoutes` with its sorted segment list
//! and limits, then the VPSC variables/constraints and the final
//! separation distance -- or, for long traces, the FNV-1a hash of exactly
//! those lines) and every connector's raw and display route (`%.17g`).
//!
//! The golden is the output of the C++ built with `-DNDEBUG`.  Scenes in
//! which the asserting C++ build fails an upstream `COLA_ASSERT` start with
//! an `#assert` line; they are checked only in release builds (`cargo test
//! --release`), where `debug_assert!` is off just like `COLA_ASSERT` under
//! NDEBUG, and skipped in debug builds.  Scenes the NDEBUG C++ could not
//! finish (crash / >20 s) would be marked `#crash` and skipped.  Set `ORACLE_SCENE=name` to run a single scene and
//! `ORACLE_FULL=1` to print full traces (compare with the oracle run with
//! the same variable).

#![allow(clippy::uninlined_format_args)]

use std::collections::BTreeMap;

use libavoid_rust::avoid::arena::{ConnIdx, PinIdx};
use libavoid_rust::avoid::connectionpin::ShapeConnectionPin;
use libavoid_rust::avoid::connector::{Checkpoint, ConnRef, ConnType};
use libavoid_rust::avoid::connend::ConnEnd;
use libavoid_rust::avoid::geomtypes::{Point, Polygon, Rectangle};
use libavoid_rust::avoid::junction::JunctionRef;
use libavoid_rust::avoid::orthogonal::{nudge_oracle_trace_enable, nudge_oracle_trace_take};
use libavoid_rust::avoid::router::*;
use libavoid_rust::avoid::shape::ShapeRef;

fn g17(v: f64) -> String {
    if v.is_nan() {
        return "nan".into();
    }
    format_g(v, 17)
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

const FULL_TRACE_LIMIT: usize = 200;

fn poly_str(p: &Polygon) -> String {
    let mut s = format!("{}:", p.ps.len());
    for q in p.ps.iter() {
        s += &format!(" {},{}", g17(q.x), g17(q.y));
    }
    s
}

struct Scene {
    router: Option<Router>,
    shapes: BTreeMap<String, ShapeRef>,
    juncs: BTreeMap<String, JunctionRef>,
    conns: BTreeMap<String, ConnIdx>,
    pins: BTreeMap<String, PinIdx>,
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

fn dump_routes(sc: &mut Scene, trace: Vec<String>) {
    let r = sc.router.as_mut().unwrap();
    sc.out.push(format!("stage {}", sc.stage));
    sc.stage += 1;
    if sc.full || trace.len() <= FULL_TRACE_LIMIT {
        sc.out.extend(trace);
    } else {
        sc.out.push(format!("trace lines={} fnv={:016x}", trace.len(), fnv(&trace)));
    }
    let conn_refs = r.conn_refs.clone();
    for c in conn_refs {
        let raw = poly_str(r.conns[c].route());
        sc.out.push(format!(
            "C {} t={} raw {}",
            r.conns[c].id(),
            r.conns[c].routing_type() as i32,
            raw
        ));
        let display = poly_str(r.conns[c].display_route());
        sc.out.push(format!("  display {}", display));
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
        "pinexcl" => {
            let p = sc.pins[t.word()];
            let v = t.unum();
            p.set_exclusive(sc.router.as_mut().unwrap(), v != 0);
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
            let _ = nudge_oracle_trace_take();
            sc.router.as_mut().unwrap().process_transaction();
            let trace = nudge_oracle_trace_take();
            dump_routes(sc, trace);
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
        stage: 0,
        full,
        out: Vec::new(),
    };
    nudge_oracle_trace_enable(true);
    for line in dsl {
        run_command(&mut sc, line);
    }
    nudge_oracle_trace_enable(false);
    if let Some(mut r) = sc.router.take() {
        r.destruct();
    }
    sc.out
}

#[test]
fn nudging_matches_cpp_oracle() {
    let path = std::env::var("ORACLE_GOLDEN")
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/nudging_oracle.txt").to_string());
    let text = std::fs::read_to_string(&path).expect("golden file");
    let scenes = parse_golden(&text);
    assert!(std::env::var("ORACLE_GOLDEN").is_ok() || scenes.len() >= 400, "golden file looks truncated");
    let only = std::env::var("ORACLE_SCENE").ok();
    let full = std::env::var("ORACLE_FULL").is_ok();
    let mut failures = Vec::new();
    let mut stages = 0;
    let mut regions = 0;
    let mut skipped = 0;
    let mut checked = 0;
    let mut skipped_assert = 0;
    let mut assert_scenes = 0;
    for scene in scenes.iter() {
        if let Some(o) = only.as_ref() {
            if &scene.name != o {
                continue;
            }
        }
        if scene.out.first().map(|l| l.starts_with("#crash")).unwrap_or(false) {
            skipped += 1;
            continue;
        }
        let mut expected: &[String] = &scene.out;
        if scene.out.first().map(|l| l.starts_with("#assert")).unwrap_or(false) {
            // The asserting C++ build fails a COLA_ASSERT here; the
            // expected output is the NDEBUG build's.  Check only when
            // debug_assert! is off, like COLA_ASSERT under NDEBUG.
            if cfg!(debug_assertions) {
                skipped_assert += 1;
                continue;
            }
            assert_scenes += 1;
            expected = &scene.out[1..];
        }
        let out = run_scene(&scene.dsl, full);
        if full {
            println!("#scene {}", scene.name);
            for l in out.iter() {
                println!("{}", l);
            }
            continue;
        }
        checked += 1;
        stages += out.iter().filter(|l| l.starts_with("stage ")).count();
        regions += out.iter().filter(|l| l.starts_with("R ")).count();
        if out != expected {
            let first = out
                .iter()
                .zip(expected.iter())
                .position(|(a, b)| a != b)
                .unwrap_or(out.len().min(expected.len()));
            failures.push(format!(
                "{}: first difference at output line {}:\n  rust:   {}\n  oracle: {}",
                scene.name,
                first,
                out.get(first).map(|s| s.as_str()).unwrap_or("<none>"),
                expected.get(first).map(|s| s.as_str()).unwrap_or("<none>")
            ));
        }
    }
    if !full {
        println!(
            "{} scenes ({} checked, of which {} fail a C++ assertion; {} #assert scenes skipped in this debug build; {} skipped as #crash), {} process stages, {} fully printed nudging regions",
            scenes.len(),
            checked,
            assert_scenes,
            skipped_assert,
            skipped,
            stages,
            regions
        );
    }
    assert!(failures.is_empty(), "{} scene(s) differ:\n{}", failures.len(), failures.join("\n"));
}
