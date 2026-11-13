//! The file-writing debug output methods and the parts of the debug output
//! that no fixture reaches, against the C++ (`tools/oracle/debug_output_oracle.cpp`,
//! whose output is committed as `tests/golden-svg/full/debug-output-oracle*`):
//!
//! * `HyperedgeRerouter::outputInstanceToSVG()` with pending registrations
//!   (a junction, and a `ConnEndList` with point, shape-pin and junction
//!   ends);
//! * `Router::outputInstanceToSVG()` of a scene with a cluster, shape and
//!   junction pins, checkpoints, a fixed route and a poly-line connector,
//!   written to a file;
//! * `outputDiagramSVG()`, `outputDiagramText()` and `outputDiagram()`
//!   (`.svg` too with the `svg-output` feature), written to files, equal
//!   their string forms; a file that cannot be created is an error.

#![allow(clippy::uninlined_format_args)]

use libavoid_rust::avoid::connectionpin::{ShapeConnectionPin, ATTACH_POS_CENTRE, ATTACH_POS_LEFT, CONNECTIONPIN_CENTRE};
use libavoid_rust::avoid::connector::{Checkpoint, ConnRef, ConnType};
use libavoid_rust::avoid::connend::{ConnEnd, CONN_DIR_DOWN, CONN_DIR_LEFT, CONN_DIR_NONE, CONN_DIR_UP};
use libavoid_rust::avoid::geomtypes::{Point, PolyLine, Rectangle};
use libavoid_rust::avoid::junction::JunctionRef;
use libavoid_rust::avoid::router::{
    Router, RoutingOption, RoutingParameter, ORTHOGONAL_ROUTING, POLY_LINE_ROUTING,
};
use libavoid_rust::avoid::shape::ShapeRef;
use libavoid_rust::avoid::viscluster::ClusterRef;

fn golden(name: &str) -> String {
    let path = format!("{}/tests/golden-svg/full/{}", env!("CARGO_MANIFEST_DIR"), name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {}", path, e))
}

fn out_dir(test: &str) -> String {
    let dir = format!("{}/debug_output_files/{}", env!("CARGO_TARGET_TMPDIR"), test);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The scene of tools/oracle/debug_output_oracle.cpp.
fn scene() -> (Router, ShapeRef, JunctionRef) {
    let mut router = Router::new(POLY_LINE_ROUTING | ORTHOGONAL_ROUTING);
    router.set_routing_parameter(RoutingParameter::SegmentPenalty, 50.0);
    router.set_routing_option(RoutingOption::NudgeOrthogonalSegmentsConnectedToShapes, true);

    let rect = Rectangle::new(&Point::new(0.0, 0.0), &Point::new(30.0, 20.0));
    let shape = ShapeRef::new(&mut router, &rect, 7);
    ShapeConnectionPin::new_shape(&mut router, shape, 1, ATTACH_POS_LEFT, ATTACH_POS_CENTRE, true, 0.0, CONN_DIR_LEFT);
    let pin = ShapeConnectionPin::new_shape(&mut router, shape, 2, 0.25, 1.0 / 3.0, true, 2.5, CONN_DIR_NONE);
    pin.set_exclusive(&mut router, false);
    let rect2 = Rectangle::new(&Point::new(100.0, 50.0), &Point::new(140.0, 80.0));
    let shape2 = ShapeRef::new(&mut router, &rect2, 8);

    let junction = JunctionRef::new(&mut router, Point::new(60.5, 40.25), 9);
    junction.set_position_fixed(&mut router, true);

    let cluster_poly = Rectangle::new(&Point::new(-10.0, -10.0), &Point::new(150.0, 90.0));
    ClusterRef::new(&mut router, &cluster_poly, 11);

    let s1 = ConnEnd::from_shape_pin(&router, shape, 1);
    let d1 = ConnEnd::from_junction(&router, junction);
    let c1 = ConnRef::new_with_ends(&mut router, &s1, &d1, 20);
    c1.set_routing_type(&mut router, ConnType::ConnTypeOrthogonal);
    let s2 = ConnEnd::from_junction(&router, junction);
    let d2 = ConnEnd::from_shape_pin(&router, shape2, CONNECTIONPIN_CENTRE);
    let c2 = ConnRef::new_with_ends(&mut router, &s2, &d2, 21);
    c2.set_routing_type(&mut router, ConnType::ConnTypeOrthogonal);
    let checkpoints = vec![
        Checkpoint::with_directions(Point::new(80.0, 45.0), CONN_DIR_UP, CONN_DIR_DOWN),
        Checkpoint::new(Point::new(90.125, 47.0)),
    ];
    c2.set_routing_checkpoints(&mut router, &checkpoints);
    let s3 = ConnEnd::from_point_dirs(Point::new(-20.0, 5.0), CONN_DIR_LEFT);
    let d3 = ConnEnd::from_point(Point::new(200.0, 1.0 / 7.0));
    let c3 = ConnRef::new_with_ends(&mut router, &s3, &d3, 22);
    c3.set_routing_type(&mut router, ConnType::ConnTypePolyLine);
    let s4 = ConnEnd::from_point(Point::new(10.0, 100.0));
    let d4 = ConnEnd::from_point(Point::new(50.0, 120.0));
    let c4 = ConnRef::new_with_ends(&mut router, &s4, &d4, 23);
    let mut fixed = PolyLine::new();
    for (x, y) in [(10.0, 100.0), (10.0, 110.0), (50.0, 110.0), (50.0, 120.0)] {
        fixed.ps.push(Point::new(x, y));
    }
    c4.set_fixed_route(&mut router, &fixed);
    router.process_transaction();
    (router, shape, junction)
}

#[test]
fn instance_svg_file_matches_cpp() {
    let (mut router, _, _) = scene();
    let dir = out_dir("instance");
    router.output_instance_to_svg(&format!("{}/debug-output-oracle", dir)).unwrap();
    let written = std::fs::read_to_string(format!("{}/debug-output-oracle.svg", dir)).unwrap();
    let expected = golden("debug-output-oracle.svg");
    if let Some((i, (a, b))) = written.lines().zip(expected.lines()).enumerate().find(|(_, (a, b))| a != b) {
        panic!("line {} differs:\n  rust: {}\n  c++:  {}", i + 1, a, b);
    }
    assert_eq!(written, expected);
    assert_eq!(router.output_instance_to_svg_string(), written);
}

#[test]
fn hyperedge_rerouter_output_matches_cpp() {
    let (mut router, shape, junction) = scene();
    router.hyperedge_rerouter().register_hyperedge_for_rerouting_junction(junction);
    let ends = vec![
        ConnEnd::from_point_dirs(Point::new(1.5, 2.0), CONN_DIR_UP),
        ConnEnd::from_shape_pin(&router, shape, 2),
        ConnEnd::from_junction(&router, junction),
    ];
    router.hyperedge_rerouter().register_hyperedge_for_rerouting(ends);
    let mut out = String::new();
    router.m_hyperedge_rerouter.output_instance_to_svg(&router, &mut out).unwrap();
    assert_eq!(out, golden("debug-output-oracle-rerouter.txt"));

    // Nothing pending: nothing written.
    let empty = libavoid_rust::avoid::hyperedge::HyperedgeRerouter::new();
    let mut out = String::new();
    empty.output_instance_to_svg(&router, &mut out).unwrap();
    assert_eq!(out, "");
}

#[test]
fn file_forms_equal_string_forms() {
    let (mut router, _, _) = scene();
    let dir = out_dir("forms");

    router.output_diagram_svg(&format!("{}/diagram", dir), None).unwrap();
    let svg = std::fs::read_to_string(format!("{}/diagram.svg", dir)).unwrap();
    assert_eq!(svg, router.output_diagram_svg_string(None));

    router.output_diagram_text_file(&format!("{}/text", dir)).unwrap();
    let text = std::fs::read_to_string(format!("{}/text.txt", dir)).unwrap();
    assert_eq!(text, router.output_diagram_text_g());

    let svg_path = format!("{}/both.svg", dir);
    let _ = std::fs::remove_file(&svg_path);
    router.output_diagram(&format!("{}/both", dir)).unwrap();
    assert_eq!(std::fs::read_to_string(format!("{}/both.txt", dir)).unwrap(), text);
    // SVG_OUTPUT upstream: outputDiagram() also writes outputInstanceToSVG().
    assert_eq!(std::path::Path::new(&svg_path).exists(), cfg!(feature = "svg-output"));
    if cfg!(feature = "svg-output") {
        assert_eq!(std::fs::read_to_string(&svg_path).unwrap(), router.output_instance_to_svg_string());
    }

    // Upstream returns silently when fopen() fails; the port reports it.
    let missing = format!("{}/no-such-dir/x", dir);
    assert!(router.output_instance_to_svg(&missing).is_err());
    assert!(router.output_diagram_svg(&missing, None).is_err());
    assert!(router.output_diagram_text_file(&missing).is_err());
}
