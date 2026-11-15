//! Replays scenes in the text format of `tools/libavoid-js-compare/gen.mjs`
//! (plus `checkpoints <conn id> <n> <x> <y> ...`), as the C++ drivers
//! `tools/libavoid-js-compare/driver.cpp` and
//! `tools/oracle/fork_route_status.cpp` do, for the oracle tests built on
//! those drivers.

#![allow(dead_code)]

use std::collections::BTreeMap;

use libavoid_rust::avoid::arena::ConnIdx;
use libavoid_rust::avoid::connectionpin::ShapeConnectionPin;
use libavoid_rust::avoid::connector::{Checkpoint, ConnRef};
use libavoid_rust::avoid::connend::ConnEnd;
use libavoid_rust::avoid::geomtypes::{Point, Rectangle};
use libavoid_rust::avoid::router::{
    Router, RoutingOption, RoutingParameter, ORTHOGONAL_ROUTING, POLY_LINE_ROUTING,
};
use libavoid_rust::avoid::shape::ShapeRef;

fn parameter(n: u32) -> RoutingParameter {
    use RoutingParameter::*;
    [
        SegmentPenalty,
        AnglePenalty,
        CrossingPenalty,
        ClusterCrossingPenalty,
        FixedSharedPathPenalty,
        PortDirectionPenalty,
        ShapeBufferDistance,
        IdealNudgingDistance,
        ReverseDirectionPenalty,
    ][n as usize]
}

fn option(n: u32) -> RoutingOption {
    use RoutingOption::*;
    [
        NudgeOrthogonalSegmentsConnectedToShapes,
        ImproveHyperedgeRoutesMovingJunctions,
        PenaliseOrthogonalSharedPathsAtConnEnds,
        NudgeOrthogonalTouchingColinearSegments,
        PerformUnifyingNudgingPreprocessingStep,
        ImproveHyperedgeRoutesMovingAddingAndDeletingJunctions,
        NudgeSharedPathsWithCommonEndPoint,
    ][n as usize]
}

/// Replays `scene`. `setup` runs right after the router is created;
/// `on_process` runs after every `processTransaction()` with the live
/// connectors by id.
pub fn replay(
    scene: &[&str],
    setup: impl Fn(&mut Router),
    mut on_process: impl FnMut(&mut Router, &BTreeMap<u32, ConnIdx>),
) {
    let mut router: Option<Router> = None;
    let mut shapes: BTreeMap<u32, ShapeRef> = BTreeMap::new();
    let mut conns: BTreeMap<u32, ConnIdx> = BTreeMap::new();
    for line in scene {
        let w: Vec<&str> = line.split(' ').collect();
        let num = |i: usize| -> f64 { w[i].parse().unwrap_or_else(|_| panic!("bad number in {line}")) };
        let int = |i: usize| -> u32 { w[i].parse().unwrap_or_else(|_| panic!("bad integer in {line}")) };
        if w[0] == "router" {
            let mut r = Router::new(if w[1] == "orth" { ORTHOGONAL_ROUTING } else { POLY_LINE_ROUTING });
            setup(&mut r);
            router = Some(r);
            continue;
        }
        let r = router.as_mut().expect("router first");
        let end = |r: &Router, i: usize| -> (ConnEnd, usize) {
            if w[i] == "P" {
                (ConnEnd::from_point(Point::new(num(i + 1), num(i + 2))), i + 3)
            } else {
                (ConnEnd::from_shape_pin(r, shapes[&int(i + 1)], int(i + 2)), i + 3)
            }
        };
        match w[0] {
            "param" => r.set_routing_parameter(parameter(int(1)), num(2)),
            "option" => r.set_routing_option(option(int(1)), int(2) != 0),
            "shape" => {
                let (x, y) = (num(2), num(3));
                let poly = Rectangle::new(&Point::new(x, y), &Point::new(x + num(4), y + num(5)));
                shapes.insert(int(1), ShapeRef::new(r, &poly, int(1)));
            }
            "pin" => {
                ShapeConnectionPin::new_shape(r, shapes[&int(1)], int(2), num(3), num(4), true, 0.0, int(5));
            }
            "conn" => {
                let (a, i) = end(r, 2);
                let (b, _) = end(r, i);
                conns.insert(int(1), ConnRef::new_with_ends(r, &a, &b, int(1)));
            }
            "checkpoints" => {
                let n = int(2) as usize;
                let cps: Vec<Checkpoint> =
                    (0..n).map(|k| Checkpoint::new(Point::new(num(3 + 2 * k), num(4 + 2 * k)))).collect();
                conns[&int(1)].set_routing_checkpoints(r, &cps);
            }
            "move" => r.move_shape_by(shapes[&int(1)], num(2), num(3)),
            "delconn" => {
                let c = conns.remove(&int(1)).expect("live connector");
                r.delete_connector(c);
            }
            "process" => {
                r.process_transaction();
                on_process(r, &conns);
            }
            other => panic!("unknown op {other}"),
        }
    }
}

/// The scenes of a golden file: `scene <name>`, the scene lines, then one or
/// more `expect[ <label>]` sections, then `end`. Lines starting with `#`
/// are comments.
pub struct GoldenScene<'a> {
    pub name: &'a str,
    pub scene: Vec<&'a str>,
    /// (label, expected lines); the label is "" for a bare `expect`.
    pub expected: Vec<(&'a str, Vec<&'a str>)>,
}

pub fn golden_scenes(golden: &str) -> Vec<GoldenScene<'_>> {
    let mut out = Vec::new();
    let mut lines = golden.lines().filter(|l| !l.starts_with('#')).peekable();
    while let Some(header) = lines.next() {
        let name = header.strip_prefix("scene ").expect("scene header");
        let mut scene = Vec::new();
        while let Some(l) = lines.peek() {
            if *l == "expect" || l.starts_with("expect ") {
                break;
            }
            scene.push(lines.next().unwrap());
        }
        let mut expected = Vec::new();
        loop {
            let l = lines.next().expect("expect or end");
            if l == "end" {
                break;
            }
            let label = l.strip_prefix("expect").expect("expect section").trim_start();
            let mut body = Vec::new();
            while let Some(l) = lines.peek() {
                if *l == "end" || *l == "expect" || l.starts_with("expect ") {
                    break;
                }
                body.push(lines.next().unwrap());
            }
            expected.push((label, body));
        }
        out.push(GoldenScene { name, scene, expected });
    }
    out
}

/// Per-transaction hashes of output lines, as `tools/oracle/golden_hash.mjs`
/// computes them for the golden files: `h <hex> <line count>` per `T <n>`
/// line and the lines after it.  The hash is the 64-bit FNV-1a of, per line,
/// the first word's bytes, a 0 byte, the IEEE bits (little-endian) of every
/// further word read as a number, and a newline byte; reading the numbers
/// makes it independent of how they are printed (NaN is canonical).
pub fn transaction_hashes(lines: &[String]) -> Vec<String> {
    fn hash(tx: &[String]) -> String {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |b: u8| h = (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3);
        for line in tx {
            let mut words = line.split(' ');
            for &b in words.next().unwrap_or("").as_bytes() {
                eat(b);
            }
            eat(0);
            for w in words {
                let v: f64 = w.parse().unwrap_or_else(|_| panic!("not a number: {w} in {line}"));
                let v = if v.is_nan() { f64::NAN } else { v };
                for b in v.to_bits().to_le_bytes() {
                    eat(b);
                }
            }
            eat(b'\n');
        }
        format!("{h:016x}")
    }
    let mut out = Vec::new();
    let mut start = 0;
    for i in 1..=lines.len() {
        if i == lines.len() || lines[i].starts_with("T ") {
            if i > start {
                out.push(format!("h {} {}", hash(&lines[start..i]), i - start));
            }
            start = i;
        }
    }
    out
}

/// Compares `ours` (output lines) with `expected` (per-transaction hashes);
/// on a difference, names the transaction and gives our output for it (run
/// the generating script with FULL=1 for the C++'s).
pub fn hashed_difference(ours: &[String], expected: &[&str]) -> Option<String> {
    let hashes = transaction_hashes(ours);
    let t = (0..hashes.len().max(expected.len())).find(|&t| hashes.get(t).map(String::as_str) != expected.get(t).copied())?;
    let ours_tx: Vec<&str> = ours
        .split(|l| l.starts_with("T "))
        .skip(1)
        .nth(t)
        .map(|tx| tx.iter().map(String::as_str).collect())
        .unwrap_or_default();
    Some(format!(
        "transaction {t}: ours {:?}, C++ {:?}; our output:\n  T {t}\n  {}",
        hashes.get(t),
        expected.get(t),
        ours_tx.join("\n  ")
    ))
}

/// The display routes after every transaction, in the format of
/// `tools/libavoid-js-compare/driver.cpp` (numbers as Rust prints them).
pub fn display_routes(scene: &[&str], setup: impl Fn(&mut Router)) -> Vec<String> {
    let mut out = Vec::new();
    let mut t = 0;
    replay(scene, setup, |r, conns| {
        out.push(format!("T {t}"));
        t += 1;
        for (id, &c) in conns {
            let ps = &c.display_route(r).ps;
            let mut line = format!("c {} {}", id, ps.len());
            for p in ps {
                line.push_str(&format!(" {} {}", p.x, p.y));
            }
            out.push(line);
        }
    });
    out
}

/// Checks a golden file with `expect off` and `expect on` sections: the
/// display routes with `set(router, false)` and `set(router, true)` must
/// match.  Debug builds check every 10th scene.  Returns
/// (scenes checked, scenes where the two settings give different routes).
pub fn check_off_on(golden: &str, set: impl Fn(&mut Router, bool)) -> (usize, usize) {
    let scenes = golden_scenes(golden);
    assert!(scenes.len() >= 100, "golden has only {} scenes", scenes.len());
    let step = if cfg!(debug_assertions) { 10 } else { 1 };
    let (mut checked, mut affected) = (0, 0);
    let mut failures = Vec::new();
    for s in scenes.iter().step_by(step) {
        assert_eq!(s.expected.len(), 2, "{}: expected `expect off` and `expect on`", s.name);
        for (label, expected) in &s.expected {
            let on = match *label {
                "off" => false,
                "on" => true,
                other => panic!("{}: unknown section {other}", s.name),
            };
            if let Some(d) = hashed_difference(&display_routes(&s.scene, |r| set(r, on)), expected) {
                failures.push(format!("{} ({label}): {d}", s.name));
            }
        }
        checked += 1;
        if s.expected[0].1 != s.expected[1].1 {
            affected += 1;
        }
    }
    assert!(failures.is_empty(), "{} failures in {checked} scenes:\n{}", failures.len(), failures.join("\n"));
    (checked, affected)
}
