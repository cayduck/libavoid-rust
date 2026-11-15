//! Native benchmark: replays a scene script on an orthogonal router
//! (shapeBufferDistance 12, idealNudgingDistance 10,
//! nudgeOrthogonalSegmentsConnectedToShapes) and prints the time of the
//! first transaction and of each later step, plus a checksum of the routes.
//! A step is everything from the end of one `R` line to the end of the next.
//!
//!   cargo run --release --example bench_scene -- scene.txt [repeat]
//!
//! Script lines:
//!   S <id> <cx> <cy> <w> <h>        shape (Rectangle(centre, w, h)); ids from 0
//!   C <id> <sx> <sy> <dx> <dy>      orthogonal connector between two points,
//!                                   hateCrossings; ids from 0
//!   T                               processTransaction()
//!   M <shape> <dx> <dy>             moveShape(shape, dx, dy)
//!   E <conn> <sx> <sy> <dx> <dy>    setSourceEndpoint / setDestEndpoint
//!   R                               read every displayRoute (ends a step)
//!
//! With `repeat` > 1 the whole script runs that many times (for profiling).

use std::time::Instant;

use libavoid_rust::avoid::arena::ConnIdx;
use libavoid_rust::avoid::connector::{ConnRef, ConnType};
use libavoid_rust::avoid::connend::ConnEnd;
use libavoid_rust::avoid::geomtypes::{Point, Rectangle};
use libavoid_rust::avoid::router::*;
use libavoid_rust::avoid::shape::ShapeRef;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let script = std::fs::read_to_string(&args[1]).expect("scene file");
    let repeat: usize = args.get(2).map_or(1, |s| s.parse().unwrap());
    for _ in 0..repeat {
        run(&script);
    }
}

fn run(script: &str) {
    let mut r = Router::new(ORTHOGONAL_ROUTING);
    r.set_routing_parameter(RoutingParameter::ShapeBufferDistance, 12.0);
    r.set_routing_parameter(RoutingParameter::IdealNudgingDistance, 10.0);
    r.set_routing_option(RoutingOption::NudgeOrthogonalSegmentsConnectedToShapes, true);

    let mut shapes: Vec<ShapeRef> = Vec::new();
    let mut conns: Vec<ConnIdx> = Vec::new();
    let mut times: Vec<f64> = Vec::new();
    let mut t = Instant::now();
    let mut hash: u64 = 0xcbf29ce484222325;

    for line in script.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        let n = |i: usize| -> f64 { f[i].parse().unwrap() };
        match f[0] {
            "S" => {
                let poly = Rectangle::from_centre(&Point::new(n(2), n(3)), n(4), n(5));
                shapes.push(ShapeRef::new(&mut r, &poly, 0));
            }
            "C" => {
                let c = ConnRef::new_with_ends(
                    &mut r,
                    &ConnEnd::from_point(Point::new(n(2), n(3))),
                    &ConnEnd::from_point(Point::new(n(4), n(5))),
                    0,
                );
                c.set_routing_type(&mut r, ConnType::ConnTypeOrthogonal);
                c.set_hate_crossings(&mut r, true);
                conns.push(c);
            }
            "M" => {
                r.move_shape_by(shapes[f[1].parse::<usize>().unwrap()], n(2), n(3));
            }
            "E" => {
                let c = conns[f[1].parse::<usize>().unwrap()];
                c.set_source_endpoint(&mut r, &ConnEnd::from_point(Point::new(n(2), n(3))));
                c.set_dest_endpoint(&mut r, &ConnEnd::from_point(Point::new(n(4), n(5))));
            }
            "T" => {
                r.process_transaction();
            }
            "R" => {
                for &c in &conns {
                    for p in &c.display_route(&mut r).ps {
                        for v in [p.x, p.y] {
                            hash ^= ((v * 100.0).round() as i64) as u64;
                            hash = hash.wrapping_mul(0x100000001b3);
                        }
                    }
                }
                times.push(t.elapsed().as_secs_f64() * 1000.0);
                t = Instant::now();
            }
            _ => panic!("bad line {line}"),
        }
    }
    let first = times[0];
    let mut drag = times[1..].to_vec();
    drag.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "shapes {} conns {}  first {:.1} ms  drag median {:.1} ms  p90 {:.1} ms  total {:.0} ms  hash {:016x}",
        shapes.len(),
        conns.len(),
        first,
        drag[drag.len() / 2],
        drag[drag.len() * 9 / 10],
        times.iter().sum::<f64>(),
        hash
    );
}
