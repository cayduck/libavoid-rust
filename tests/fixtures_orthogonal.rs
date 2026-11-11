//! Statement-by-statement translations of upstream libavoid test fixtures
//! (`vendor/adaptagrams/cola/libavoid/tests/<name>.cpp`) that use
//! orthogonal routing (and hence orthogonal nudging).
//!
//! Each `router->outputDiagram("output/NAME")` is checked against
//! `tests/golden-bump/NAME.txt` and the final router state against
//! `tests/golden-bump/<fixture>-final.txt` (C++ run with a bump allocator,
//! so pointer order == allocation order, the model the Rust port
//! implements); whether the glibc-malloc golden `tests/golden/` also
//! matches is reported on stderr.  The fixture's return condition is
//! asserted against the recorded upstream exit code.

#![allow(clippy::uninlined_format_args)]
#![allow(unused_variables, unused_mut)]

mod common;

use common::*;
use libavoid_rust::avoid::connector::ConnRef;
use libavoid_rust::avoid::connend::ConnEnd;
use libavoid_rust::avoid::geomtypes::{Point, Rectangle};
use libavoid_rust::avoid::router::{Router, RoutingParameter, ORTHOGONAL_ROUTING};
use libavoid_rust::avoid::shape::ShapeRef;
use libavoid_rust::avoid::vertices::{CONN_DIR_DOWN, CONN_DIR_LEFT, CONN_DIR_RIGHT, CONN_DIR_UP};

fn exit_code(name: &str) -> i32 {
    read_golden_file_in(GoldenDir::Bump, name, "exit").trim().parse().expect("exit code")
}

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn rect(router: &mut Router, x1: f64, y1: f64, x2: f64, y2: f64, id: u32) -> ShapeRef {
    let poly = Rectangle::new(&pt(x1, y1), &pt(x2, y2));
    ShapeRef::new(router, &poly, id)
}

// upstream: tests/inline.cpp
#[test]
fn inline() {
    let mut router = Router::new(ORTHOGONAL_ROUTING);

    // Create the ShapeRef:
    rect(&mut router, 0.0, 0.0, 100.0, 100.0, 0);

    let src_pt = ConnEnd::from_point(pt(200.0, 200.0));

    let mut dst_pt = ConnEnd::from_point_dirs(pt(95.0, 25.0), CONN_DIR_UP | CONN_DIR_RIGHT | CONN_DIR_DOWN);
    ConnRef::new_with_ends(&mut router, &src_pt, &dst_pt, 0);

    dst_pt = ConnEnd::from_point_dirs(pt(95.0, 75.0), CONN_DIR_UP | CONN_DIR_RIGHT | CONN_DIR_DOWN);
    ConnRef::new_with_ends(&mut router, &src_pt, &dst_pt, 0);

    // Up goes towards negative coordinates.
    dst_pt = ConnEnd::from_point_dirs(pt(25.0, 5.0), CONN_DIR_LEFT | CONN_DIR_RIGHT | CONN_DIR_UP);
    ConnRef::new_with_ends(&mut router, &src_pt, &dst_pt, 0);

    dst_pt = ConnEnd::from_point_dirs(pt(75.0, 5.0), CONN_DIR_LEFT | CONN_DIR_RIGHT | CONN_DIR_UP);
    ConnRef::new_with_ends(&mut router, &src_pt, &dst_pt, 0);

    dst_pt = ConnEnd::from_point_dirs(pt(25.0, 95.0), CONN_DIR_LEFT | CONN_DIR_RIGHT | CONN_DIR_DOWN);
    ConnRef::new_with_ends(&mut router, &src_pt, &dst_pt, 0);

    dst_pt = ConnEnd::from_point_dirs(pt(75.0, 95.0), CONN_DIR_LEFT | CONN_DIR_RIGHT | CONN_DIR_DOWN);
    ConnRef::new_with_ends(&mut router, &src_pt, &dst_pt, 0);

    router.process_transaction();
    assert_matches_golden_bump(&mut router, "inline");

    assert_matches_golden_bump(&mut router, "inline-final");
    router.destruct();
    assert_eq!(exit_code("inline"), 0);
}

// upstream: tests/infinity.cpp
#[test]
fn infinity() {
    let mut router = Router::new(ORTHOGONAL_ROUTING);
    router.set_routing_penalty(RoutingParameter::SegmentPenalty, 50.0);
    router.set_routing_parameter(RoutingParameter::IdealNudgingDistance, 10.0);
    rect(&mut router, 51145.0, 50195.0, 51145.0 + 360.0, 50195.0 + 685.0, 0);
    rect(&mut router, 51920.0, 49770.0, 51920.0 + 360.0, 49770.0 + 310.0, 0);
    rect(&mut router, 51151.0, 49770.0, 51151.0 + 360.0, 49770.0 + 310.0, 0);
    let src_pt53 = ConnEnd::from_point_dirs(pt(51150.0, 49850.0), 4);
    let dst_pt53 = ConnEnd::from_point_dirs(pt(51500.0, 50350.0), 8);
    ConnRef::new_with_ends(&mut router, &src_pt53, &dst_pt53, 0);
    let src_pt52 = ConnEnd::from_point_dirs(pt(52275.0, 49850.0), 8);
    let dst_pt52 = ConnEnd::from_point_dirs(pt(51150.0, 49925.0), 4);
    ConnRef::new_with_ends(&mut router, &src_pt52, &dst_pt52, 0);
    let src_pt6 = ConnEnd::from_point_dirs(pt(51150.0, 50500.0), 4);
    let dst_pt6 = ConnEnd::from_point_dirs(pt(51925.0, 50000.0), 4);
    ConnRef::new_with_ends(&mut router, &src_pt6, &dst_pt6, 6);
    router.process_transaction();
    assert_matches_golden_bump(&mut router, "infinity");
    assert_matches_golden_bump(&mut router, "infinity-final");
    router.destruct();
    assert_eq!(exit_code("infinity"), 0);
}

// upstream: tests/nudgeintobug.cpp
#[test]
fn nudgeintobug() {
    let mut router = Router::new(ORTHOGONAL_ROUTING);
    router.set_routing_penalty(RoutingParameter::ALL[0], 50.0);
    router.set_routing_penalty(RoutingParameter::ALL[1], 0.0);
    router.set_routing_penalty(RoutingParameter::ALL[2], 200.0);
    router.set_routing_penalty(RoutingParameter::ALL[3], 4000.0);
    router.set_routing_penalty(RoutingParameter::ALL[4], 110.0);
    router.set_routing_parameter(RoutingParameter::IdealNudgingDistance, 25.0);
    rect(&mut router, 51900.0, 50400.0, 52300.0, 50900.0, 548374163);
    rect(&mut router, 51131.0, 49750.0, 51531.0, 50100.0, 901116812);
    rect(&mut router, 51125.0, 50175.0, 51525.0, 50900.0, 335855988);
    rect(&mut router, 52375.0, 50750.0, 52575.0, 50900.0, 448725420);
    rect(&mut router, 51900.0, 49750.0, 52300.0, 50100.0, 74263573);
    let src_pt463223880 = ConnEnd::from_point_dirs(pt(51500.0, 50275.0), 8);
    let dst_pt463223880 = ConnEnd::from_point_dirs(pt(51150.0, 50275.0), 4);
    let src_pt144520410 = ConnEnd::from_point_dirs(pt(51150.0, 49850.0), 4);
    let dst_pt144520410 = ConnEnd::from_point_dirs(pt(51500.0, 50350.0), 8);
    let src_pt45398340 = ConnEnd::from_point_dirs(pt(52400.0, 50825.0), 4);
    let dst_pt45398340 = ConnEnd::from_point_dirs(pt(51500.0, 49925.0), 8);
    let src_pt29344262 = ConnEnd::from_point_dirs(pt(51150.0, 50500.0), 4);
    let dst_pt29344262 = ConnEnd::from_point_dirs(pt(51925.0, 50000.0), 4);
    ConnRef::new_with_ends(&mut router, &src_pt29344262, &dst_pt29344262, 29344262);
    ConnRef::new_with_ends(&mut router, &src_pt45398340, &dst_pt45398340, 45398340);
    ConnRef::new_with_ends(&mut router, &src_pt144520410, &dst_pt144520410, 144520410);
    ConnRef::new_with_ends(&mut router, &src_pt463223880, &dst_pt463223880, 463223880);
    router.process_transaction();
    assert_matches_golden_bump(&mut router, "nudgeintobug");
    let overlap = router.exists_orthogonal_fixed_segment_overlap(false);
    let touching = router.exists_orthogonal_touching_paths();
    assert_matches_golden_bump(&mut router, "nudgeintobug-final");
    router.destruct();
    assert_eq!(if overlap || touching { 1 } else { 0 }, exit_code("nudgeintobug"));
}
