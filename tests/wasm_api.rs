//! API-contract tests of the JS-facing string-id wrapper `AvoidRouter`
//! (`src/wasm_api.rs`): ids and numeric-id mapping, the error convention
//! (`false` / `null` / `undefined`, never a panic), JSON shapes,
//! transactions on/off, last-transaction connector reporting, hyperedge
//! results, and differential checks against the core API for scenes that
//! have no upstream fixture (clusters, polyline routing).
//!
//! The upstream fixture scenes themselves are driven through the wrapper
//! in `tests/wasm_api_fixtures.rs`.

#![allow(clippy::uninlined_format_args)]

#[path = "common/json.rs"]
mod json;

use json::{parse, J};
use libavoid_rust::avoid::connend::ConnEnd;
use libavoid_rust::avoid::connector::ConnRef;
use libavoid_rust::avoid::geomtypes::{Point, Polygon, Rectangle};
use libavoid_rust::avoid::router::{Router, RoutingParameter, POLY_LINE_ROUTING};
use libavoid_rust::avoid::shape::ShapeRef;
use libavoid_rust::avoid::viscluster::ClusterRef;
use libavoid_rust::{curved_polyline, AvoidRouter, CONNECTION_PIN_CENTRE, CONN_DIR_ALL, ORTHOGONAL_ROUTING};

const NONE: &str = "";

fn ortho() -> AvoidRouter {
    AvoidRouter::create(ORTHOGONAL_ROUTING).unwrap()
}

fn pts(j: &str) -> Vec<(f64, f64)> {
    parse(j).points()
}

/// `[x0, y0, x1, y1, ...]` of a rectangle polygon as upstream builds it.
fn rect(l: f64, t: f64, r: f64, b: f64) -> Vec<f64> {
    Rectangle::new(&Point::new(l, t), &Point::new(r, b)).ps.iter().flat_map(|p| [p.x, p.y]).collect()
}

fn add_point_conn(r: &mut AvoidRouter, id: &str, a: (f64, f64), b: (f64, f64)) -> bool {
    r.add_connector(id.into(), a.0, a.1, b.0, b.1, None)
}

#[test]
fn constructor_validates_flags() {
    assert!(AvoidRouter::create(0).is_none());
    assert!(AvoidRouter::create(4).is_none());
    assert!(AvoidRouter::create(7).is_none());
    for f in 1..=3 {
        assert_eq!(AvoidRouter::create(f).unwrap().router_flags(), f);
    }
}

#[test]
fn defaults_and_parameters_are_upstream() {
    let mut r = ortho();
    assert!(r.transaction_use());
    // Upstream Router() defaults.
    assert_eq!(r.routing_parameter(0.0), Some(10.0));
    assert_eq!(r.routing_parameter(3.0), Some(4000.0));
    assert_eq!(r.routing_parameter(7.0), Some(4.0));
    assert_eq!(r.routing_option(1.0), Some(true));
    assert_eq!(r.routing_option(0.0), Some(false));
    // Omitted value = chooseSensibleParamValue.
    assert!(r.set_routing_parameter(2.0, None));
    assert_eq!(r.routing_parameter(2.0), Some(200.0));
    assert!(r.set_routing_penalty(4.0, None));
    assert_eq!(r.routing_parameter(4.0), Some(110.0));
    assert!(r.set_routing_parameter(6.0, Some(12.5)));
    assert_eq!(r.routing_parameter(6.0), Some(12.5));
    // Invalid input.
    assert!(!r.set_routing_parameter(9.0, Some(1.0)));
    assert!(!r.set_routing_parameter(-1.0, Some(1.0)));
    assert!(!r.set_routing_parameter(1.5, Some(1.0)));
    assert!(!r.set_routing_parameter(f64::NAN, Some(1.0)));
    assert!(!r.set_routing_parameter(0.0, Some(f64::NAN)));
    assert!(!r.set_routing_parameter(0.0, Some(f64::INFINITY)));
    assert_eq!(r.routing_parameter(9.0), None);
    assert!(!r.set_routing_option(7.0, true));
    assert_eq!(r.routing_option(7.0), None);
    assert!(r.set_routing_option(6.0, false));
    assert_eq!(r.routing_option(6.0), Some(false));
    // validConnType
    assert_eq!(r.valid_conn_type(None), Some(2));
    assert_eq!(r.valid_conn_type(Some(1.0)), Some(2));
    assert_eq!(r.valid_conn_type(Some(3.0)), None);
    let p = AvoidRouter::create(3).unwrap();
    assert_eq!(p.valid_conn_type(None), Some(1));
    assert_eq!(p.valid_conn_type(Some(2.0)), Some(2));
}

#[test]
fn ids_numeric_mapping_and_duplicates() {
    let mut r = ortho();
    assert_eq!(r.new_object_id(), 1);
    assert!(r.add_shape("a".into(), 0.0, 0.0, 10.0, 10.0, None));
    assert!(r.add_shape("b".into(), 20.0, 0.0, 30.0, 10.0, Some(10.0)));
    assert!(r.add_junction("j".into(), 50.0, 50.0, None));
    assert!(add_point_conn(&mut r, "c", (0.0, 50.0), (100.0, 50.0)));
    assert!(r.add_cluster_rectangle("cl".into(), -5.0, -5.0, 40.0, 40.0, Some(0.0)));
    // Sequential in creation order (largest assigned + 1), like upstream.
    assert_eq!(r.object_numeric_id("a".into()), Some(1));
    assert_eq!(r.object_numeric_id("b".into()), Some(10));
    assert_eq!(r.object_numeric_id("j".into()), Some(11));
    assert_eq!(r.object_numeric_id("c".into()), Some(12));
    assert_eq!(r.object_numeric_id("cl".into()), Some(13));
    assert_eq!(r.new_object_id(), 14);
    assert_eq!(parse(&r.object_id_from_numeric_id(10.0)), J::Str("b".into()));
    assert!(parse(&r.object_id_from_numeric_id(99.0)).is_null());
    assert!(parse(&r.object_id_from_numeric_id(-1.0)).is_null());
    assert_eq!(r.object_numeric_id("zz".into()), None);
    // String ids are unique across all kinds.
    assert!(!r.object_id_is_unused("a".into()));
    assert!(r.object_id_is_unused("zz".into()));
    assert!(!r.add_junction("a".into(), 1.0, 1.0, None));
    assert!(!add_point_conn(&mut r, "b", (0.0, 0.0), (1.0, 1.0)));
    // Numeric id: upstream objectIdIsUnused() decides (queued objects are
    // not yet known to the router, exactly as upstream).
    assert_eq!(r.numeric_object_id_is_unused(13.0), Some(false)); // cluster: added immediately
    assert!(!r.add_shape("x".into(), 0.0, 0.0, 1.0, 1.0, Some(13.0)));
    assert!(!r.add_shape("x".into(), 0.0, 0.0, 1.0, 1.0, Some(1.5)));
    assert!(!r.add_shape("x".into(), 0.0, 0.0, 1.0, 1.0, Some(-3.0)));
    assert!(!r.add_shape("x".into(), 0.0, 0.0, 1.0, 1.0, Some(f64::NAN)));
    assert_eq!(r.numeric_object_id_is_unused(f64::NAN), None);
    r.process_transaction();
    assert_eq!(r.numeric_object_id_is_unused(1.0), Some(false));
    assert!(!r.add_shape("x".into(), 0.0, 0.0, 1.0, 1.0, Some(1.0)));
    // Listing in creation order.
    assert_eq!(parse(&r.shape_ids()).strings(), vec!["a", "b"]);
    assert_eq!(parse(&r.junction_ids()).strings(), vec!["j"]);
    assert_eq!(parse(&r.connector_ids()).strings(), vec!["c"]);
    assert_eq!(parse(&r.cluster_ids()).strings(), vec!["cl"]);
}

#[test]
fn invalid_input_never_panics() {
    let mut r = ortho();
    let nan = f64::NAN;
    assert!(!r.add_shape("s".into(), nan, 0.0, 1.0, 1.0, None));
    assert!(!r.add_polygon_shape("s".into(), vec![0.0, 0.0, 1.0], None));
    assert!(!r.add_polygon_shape("s".into(), vec![0.0, 0.0, 1.0, 1.0], None));
    assert!(!r.add_polygon_shape("s".into(), vec![0.0, 0.0, 1.0, 1.0, nan, 0.0], None));
    assert!(!r.add_junction("j".into(), nan, 0.0, None));
    assert!(!r.add_cluster("k".into(), vec![0.0, 0.0], None));
    assert!(r.add_shape("s".into(), 0.0, 0.0, 10.0, 10.0, None));
    // Pins: class 0, bad dirs, non-finite, negative cost, unknown owner.
    assert!(!r.add_shape_connection_pin("s".into(), "p".into(), 0.0, 0.5, 0.5, Some(true), None, None));
    assert!(!r.add_shape_connection_pin("s".into(), "p".into(), 1.0, 0.5, 0.5, Some(true), None, Some(16.0)));
    assert!(!r.add_shape_connection_pin("s".into(), "p".into(), 1.0, nan, 0.5, Some(true), None, None));
    assert!(!r.add_shape_connection_pin_with_options(
        "s".into(),
        "p".into(),
        1.0,
        0.5,
        0.5,
        Some(true),
        None,
        None,
        Some(-1.0),
        None
    ));
    assert!(!r.add_shape_connection_pin("nope".into(), "p".into(), 1.0, 0.5, 0.5, Some(true), None, None));
    assert!(r.add_shape_connection_pin("s".into(), "p".into(), 1.0, 0.5, 0.5, Some(true), None, None));
    assert!(!r.add_shape_connection_pin("s".into(), "p".into(), 1.0, 0.5, 0.5, Some(true), None, None));
    assert!(!r.set_shape_connection_pin_connection_cost("s".into(), "p".into(), -1.0));
    assert!(!r.set_shape_connection_pin_connection_cost("s".into(), "p".into(), nan));
    // ConnEnds: unknown kinds/objects, class UNSET, bad directions.
    assert!(!r.add_connector_with_conn_ends(
        "c".into(), 9.0, NONE.into(), NONE.into(), 0.0, 0.0, 15.0, 0.0, NONE.into(), NONE.into(), 1.0, 1.0, 15.0, None
    ));
    assert!(!r.add_connector_with_conn_ends(
        "c".into(), 1.0, "s".into(), NONE.into(), 0.0, 0.0, 2147483647.0, 0.0, NONE.into(), NONE.into(), 1.0, 1.0, 15.0, None
    ));
    assert!(!r.add_connector_with_conn_ends(
        "c".into(), 2.0, "s".into(), NONE.into(), 0.0, 0.0, 0.0, 0.0, NONE.into(), NONE.into(), 1.0, 1.0, 15.0, None
    ));
    assert!(!r.add_connector_with_conn_ends(
        "c".into(), 3.0, "s".into(), "missing".into(), 0.0, 0.0, 0.0, 0.0, NONE.into(), NONE.into(), 1.0, 1.0, 15.0, None
    ));
    assert!(!r.add_connector_with_conn_ends(
        "c".into(), 0.0, NONE.into(), NONE.into(), 0.0, 0.0, 99.0, 0.0, NONE.into(), NONE.into(), 1.0, 1.0, 15.0, None
    ));
    assert!(!add_point_conn(&mut r, "c", (nan, 0.0), (1.0, 1.0)));
    assert!(add_point_conn(&mut r, "c", (-20.0, 5.0), (40.0, 5.0)));
    // Unknown ids everywhere.
    for id in ["zz", "s", "c"] {
        let s = id.to_string();
        let is_shape = id == "s";
        let is_conn = id == "c";
        assert_eq!(r.move_shape_by(s.clone(), 1.0, 1.0), is_shape);
        assert_eq!(r.connector_routing_type(s.clone()).is_some(), is_conn);
        assert_eq!(parse(&r.display_connector_route(s.clone())).is_null(), !is_conn);
        assert_eq!(parse(&r.shape_polygon(s.clone())).is_null(), !is_shape);
        assert!(parse(&r.junction_position(s.clone())).is_null());
        assert!(!r.remove_junction(s.clone()));
        assert!(!r.remove_cluster(s.clone()));
    }
    assert!(!r.move_shape_by("s".into(), nan, 0.0));
    // moveShape with a different point count is refused (upstream asserts).
    assert!(!r.move_polygon_shape("s".into(), vec![0.0, 0.0, 5.0, 0.0, 5.0, 5.0], None));
    assert!(r.move_polygon_shape("s".into(), rect(0.0, 0.0, 12.0, 12.0), None));
    assert!(!r.set_connector_routing_checkpoints("c".into(), vec![1.0, 2.0, 15.0]));
    assert!(!r.set_connector_routing_checkpoints("c".into(), vec![1.0, 2.0, 15.0, 16.0]));
    assert!(!r.set_connector_fixed_route("c".into(), vec![1.0, 2.0, 3.0]));
    assert!(!r.set_connector_routing_type("c".into(), 3.0));
    assert!(!r.transform_shape_connection_pin_positions("s".into(), 5.0));
    assert!(parse(&r.shape_containing_point(nan, 0.0)).is_null());
    assert!(parse(&r.connector_curved_route("c".into(), nan, None)).is_null());
    assert!(parse(&curved_polyline(vec![1.0], 1.0, None)).is_null());
    assert!(parse(&r.hyperedge_new_and_deleted_object_lists(0.0)).is_null());
    assert_eq!(r.register_hyperedge_for_rerouting_junction("s".into()), None);
    assert_eq!(r.register_hyperedge_for_rerouting(vec![0.0], vec![], vec![], vec![], vec![], vec![]), None);
    assert!(r.process_transaction());
}

#[test]
fn transactions_on_queue_until_process() {
    let mut r = ortho();
    assert!(r.add_shape("s".into(), 10.0, -10.0, 20.0, 10.0, None));
    assert!(add_point_conn(&mut r, "c", (0.0, 0.0), (30.0, 0.0)));
    // Nothing routed before processTransaction (upstream semantics).
    assert_eq!(pts(&r.route_connector("c".into())), vec![]);
    assert_eq!(pts(&r.display_connector_route("c".into())), vec![]);
    assert_eq!(parse(&r.last_transaction_updated_connector_ids()).strings(), Vec::<String>::new());
    assert!(r.process_transaction());
    assert_eq!(parse(&r.last_transaction_updated_connector_ids()).strings(), vec!["c"]);
    let route = pts(&r.display_connector_route("c".into()));
    assert_eq!(route.first(), Some(&(0.0, 0.0)));
    assert_eq!(route.last(), Some(&(30.0, 0.0)));
    assert!(route.len() >= 4, "route avoids the shape: {:?}", route);
    assert!(!r.exists_invalid_orthogonal_paths());
    // No changes: processTransaction returns false, last list unchanged.
    assert!(!r.process_transaction());
    assert_eq!(parse(&r.last_transaction_updated_connector_ids()).strings(), vec!["c"]);
    // Moving the shape away reroutes c.
    assert!(r.move_shape("s".into(), 10.0, 20.0, 20.0, 40.0));
    assert!(r.process_transaction());
    assert_eq!(parse(&r.last_transaction_updated_connector_ids()).strings(), vec!["c"]);
    assert_eq!(pts(&r.display_connector_route("c".into())), vec![(0.0, 0.0), (30.0, 0.0)]);
    assert_eq!(pts(&r.route_connector("c".into())).first(), Some(&(0.0, 0.0)));
    // removeConnector is immediate (upstream deleteConnector()).
    assert!(r.remove_connector("c".into()));
    assert!(!r.remove_connector("c".into()));
    assert_eq!(parse(&r.last_transaction_updated_connector_ids()).strings(), Vec::<String>::new());
    // removeShape is queued; the id is released immediately.
    assert!(r.remove_shape("s".into()));
    assert!(!r.remove_shape("s".into()));
    assert!(r.object_id_is_unused("s".into()));
    assert!(r.process_transaction());
}

#[test]
fn transactions_off_route_immediately() {
    let mut r = ortho();
    r.set_transaction_use(false);
    assert!(!r.transaction_use());
    assert!(r.add_shape("s".into(), 10.0, -10.0, 20.0, 10.0, None));
    assert!(add_point_conn(&mut r, "c", (0.0, 0.0), (30.0, 0.0)));
    assert!(pts(&r.display_connector_route("c".into())).len() >= 4);
    let n = r.transaction_count();
    assert!(r.move_shape_by("s".into(), 0.0, 30.0));
    assert!(r.transaction_count() > n);
    assert_eq!(parse(&r.last_transaction_updated_connector_ids()).strings(), vec!["c"]);
    assert_eq!(pts(&r.display_connector_route("c".into())), vec![(0.0, 0.0), (30.0, 0.0)]);
    // With immediate processing a shape can be removed right after adding.
    assert!(r.add_shape("t".into(), 100.0, 100.0, 110.0, 110.0, None));
    assert!(r.remove_shape("t".into()));
}

#[test]
fn removing_queued_additions_is_refused() {
    // Upstream asserts that a shape/junction is not deleted in the same
    // transaction it was added in.
    let mut r = ortho();
    assert!(r.add_shape("s".into(), 0.0, 0.0, 10.0, 10.0, None));
    assert!(r.add_junction("j".into(), 50.0, 50.0, None));
    assert!(!r.remove_shape("s".into()));
    assert!(!r.remove_junction("j".into()));
    assert!(parse(&r.remove_junction_and_merge_connectors("j".into())).is_null());
    r.process_transaction();
    assert!(r.remove_shape("s".into()));
    assert!(r.remove_junction("j".into()));
    r.process_transaction();
    assert_eq!(parse(&r.shape_ids()).strings(), Vec::<String>::new());
}

#[test]
fn shape_queries_and_pins() {
    let mut r = ortho();
    r.set_routing_parameter(6.0, Some(2.0));
    assert!(r.add_shape("s".into(), 0.0, 0.0, 20.0, 10.0, None));
    let bb = parse(&r.shape_bounding_box("s".into()));
    assert_eq!(bb.keys(), vec!["left", "top", "right", "bottom"]);
    assert_eq!((bb.get("left").num(), bb.get("bottom").num()), (0.0, 10.0));
    let rb = parse(&r.shape_routing_box("s".into()));
    assert_eq!((rb.get("left").num(), rb.get("bottom").num()), (-2.0, 12.0));
    assert_eq!(pts(&r.shape_polygon("s".into())), vec![(20.0, 0.0), (20.0, 10.0), (0.0, 10.0), (0.0, 0.0)]);
    assert_eq!(pts(&r.shape_routing_polygon("s".into())).len(), 4);
    let pos = parse(&r.shape_position("s".into()));
    assert_eq!((pos.get("x").num(), pos.get("y").num()), (10.0, 5.0));
    // Queued shapes are not yet in the scene (upstream).
    assert!(parse(&r.shape_containing_point(5.0, 5.0)).is_null());
    r.process_transaction();
    assert_eq!(parse(&r.shape_containing_point(5.0, 5.0)), J::Str("s".into()));
    assert!(parse(&r.shape_containing_point(50.0, 5.0)).is_null());

    assert!(r.add_shape_connection_pin_with_options(
        "s".into(),
        "out".into(),
        3.0,
        1.0,
        0.5,
        Some(true),
        None,
        Some(8.0),
        Some(2.5),
        Some(false)
    ));
    assert!(r.add_shape_connection_pin("s".into(), "in".into(), 4.0, 0.0, 5.0, Some(false), Some(0.0), Some(4.0)));
    assert_eq!(parse(&r.shape_connection_pin_id_list("s".into())).strings(), vec!["out", "in"]);
    let p = parse(&r.shape_connection_pin_position("s".into(), "out".into()));
    assert_eq!((p.get("x").num(), p.get("y").num()), (20.0, 5.0));
    let p = parse(&r.shape_connection_pin_position("s".into(), "in".into()));
    assert_eq!((p.get("x").num(), p.get("y").num()), (0.0, 5.0));
    assert_eq!(r.shape_connection_pin_directions("s".into(), "out".into()), Some(8));
    assert_eq!(r.shape_connection_pin_exclusive("s".into(), "out".into()), Some(false));
    // Upstream default: pins without visibility in all directions are exclusive.
    assert_eq!(r.shape_connection_pin_exclusive("s".into(), "in".into()), Some(true));
    assert!(r.set_shape_connection_pin_exclusive("s".into(), "in".into(), false));
    assert_eq!(r.shape_connection_pin_exclusive("s".into(), "in".into()), Some(false));
    let ids = parse(&r.shape_connection_pin_ids("s".into(), "out".into()));
    assert_eq!(ids.keys(), vec!["objectId", "numericObjectId", "classId"]);
    assert_eq!((ids.get("objectId").str(), ids.get("numericObjectId").num(), ids.get("classId").num()), ("s", 1.0, 3.0));
    assert!(r.set_shape_connection_pin_connection_cost("s".into(), "out".into(), 1.0));
    // transformConnectionPinPositions (FlipX moves the right pin left).
    assert!(r.transform_shape_connection_pin_positions("s".into(), 3.0));
    let p = parse(&r.shape_connection_pin_position("s".into(), "out".into()));
    assert_eq!(p.get("x").num(), 0.0);
    assert_eq!(r.shape_connection_pin_directions("s".into(), "out".into()), Some(4));
    assert!(r.remove_shape_connection_pin("s".into(), "in".into()));
    assert!(!r.remove_shape_connection_pin("s".into(), "in".into()));
    assert!(parse(&r.shape_connection_pin_position("s".into(), "in".into())).is_null());
}

#[test]
fn pin_connectors_attachment_and_endpoint_conn_ends() {
    let mut r = ortho();
    assert!(r.add_shape("a".into(), 0.0, 0.0, 10.0, 10.0, None));
    assert!(r.add_shape("b".into(), 50.0, 0.0, 60.0, 10.0, None));
    assert!(r.add_shape_connection_pin("a".into(), "r".into(), 1.0, 1.0, 0.5, Some(true), None, Some(8.0)));
    assert!(r.add_shape_connection_pin("b".into(), "l".into(), 2.0, 0.0, 0.5, Some(true), None, Some(4.0)));
    // Pin by name = ConnEnd(shape, classOf(pin)).
    assert!(r.add_connector_with_conn_ends(
        "c".into(), 3.0, "a".into(), "r".into(), 0.0, 0.0, 0.0, 1.0, "b".into(), NONE.into(), 0.0, 0.0, 2.0, Some(100.0)
    ));
    assert!(r.process_transaction());
    assert_eq!(pts(&r.display_connector_route("c".into())), vec![(10.0, 5.0), (50.0, 5.0)]);
    let ends = parse(&r.connector_endpoint_conn_ends("c".into()));
    let src = ends.get("source");
    assert_eq!(src.keys(), vec!["type", "position", "directions", "shapeId", "classId"]);
    assert_eq!((src.get("type").str(), src.get("shapeId").str(), src.get("classId").num()), ("shapePin", "a", 1.0));
    assert_eq!(ends.get("destination").get("shapeId").str(), "b");
    assert_eq!(parse(&r.shape_attached_connectors("a".into())).strings(), vec!["c"]);
    assert_eq!(parse(&r.shape_attached_shapes("a".into(), None)).strings(), vec!["b"]);
    assert_eq!(parse(&r.shape_attached_shapes("a".into(), Some(1.0))).strings(), Vec::<String>::new());
    assert_eq!(parse(&r.shape_attached_shapes("a".into(), Some(2.0))).strings(), vec!["b"]);
    assert_eq!(parse(&r.shape_attached_conns("b".into(), Some(1.0))).strings(), vec!["c"]);
    assert!(parse(&r.shape_attached_conns("b".into(), Some(4.0))).is_null());
    // Exclusive pin already in use: no further candidates (upstream).
    assert_eq!(pts(&r.connector_possible_dst_pin_points("c".into())), vec![]);
    // Point ends report positions and directions.
    assert!(r.set_connector_dest_conn_end("c".into(), 0.0, NONE.into(), NONE.into(), 80.0, 5.0, 4.0));
    assert!(r.process_transaction());
    let dst = parse(&r.connector_endpoint_conn_ends("c".into())).get("destination").clone();
    assert_eq!(dst.keys(), vec!["type", "position", "directions"]);
    assert_eq!(dst.get("type").str(), "point");
    // Upstream reports the endpoint vertex's current visibility directions
    // (which orthogonal routing may have widened from the requested 4).
    let d = dst.get("directions").num();
    assert!((0.0..=15.0).contains(&d) && (d as u32) & 4 != 0);
    assert_eq!(dst.get("position").get("x").num(), 80.0);
    assert_eq!(parse(&r.shape_attached_shapes("a".into(), None)).strings(), Vec::<String>::new());
}

#[test]
fn junctions_split_and_merge() {
    let mut r = ortho();
    assert!(r.add_shape("a".into(), 0.0, 0.0, 10.0, 10.0, None));
    assert!(r.add_shape_connection_pin("a".into(), "c".into(), CONNECTION_PIN_CENTRE as f64, 0.5, 0.5, Some(true), None, None));
    assert!(r.add_connector_with_conn_ends(
        "c1".into(),
        0.0,
        NONE.into(),
        NONE.into(),
        100.0,
        5.0,
        CONN_DIR_ALL as f64,
        1.0,
        "a".into(),
        NONE.into(),
        0.0,
        0.0,
        CONNECTION_PIN_CENTRE as f64,
        None
    ));
    assert!(r.process_transaction());
    let route = pts(&r.display_connector_route("c1".into()));
    assert_eq!(route.len(), 2);
    // Splitting: segment 0 / out of range / point destination refused.
    assert!(parse(&r.split_connector_at_segment("c1".into(), 0.0, "J".into(), "c2".into())).is_null());
    assert!(parse(&r.split_connector_at_segment("c1".into(), 2.0, "J".into(), "c2".into())).is_null());
    assert!(parse(&r.split_connector_at_segment("c1".into(), 1.0, "a".into(), "c2".into())).is_null());
    let split = parse(&r.split_connector_at_segment("c1".into(), 1.0, "J".into(), "c2".into()));
    assert_eq!(split.keys(), vec!["junctionId", "connectorId", "position"]);
    assert_eq!(split.get("junctionId").str(), "J");
    assert_eq!(split.get("position").get("y").num(), 5.0);
    assert!(r.process_transaction());
    let mut attached = parse(&r.junction_attached_connectors("J".into())).strings();
    attached.sort();
    assert_eq!(attached, vec!["c1", "c2"]);
    assert_eq!(r.junction_position_fixed("J".into()), Some(false));
    assert!(r.set_junction_position_fixed("J".into(), true));
    assert_eq!(r.junction_position_fixed("J".into()), Some(true));
    assert!(r.prefer_junction_orthogonal_dimension("J".into(), 0.0));
    assert!(!r.prefer_junction_orthogonal_dimension("J".into(), 2.0));
    assert!(r.move_junction("J".into(), 60.0, 5.0));
    assert!(r.move_junction_by("J".into(), 1.0, 0.0));
    assert!(r.process_transaction());
    let p = parse(&r.junction_position("J".into()));
    assert_eq!((p.get("x").num(), p.get("y").num()), (61.0, 5.0));
    assert!(!parse(&r.junction_recommended_position("J".into())).is_null());
    // Merge: c2 is deleted by the core and forgotten by the wrapper.
    let merged = parse(&r.remove_junction_and_merge_connectors("J".into()));
    let merged = merged.str().to_string();
    assert!(merged == "c1" || merged == "c2");
    assert_eq!(parse(&r.connector_ids()).strings(), vec![merged.clone()]);
    assert!(r.object_id_is_unused("J".into()));
    assert!(r.process_transaction());
    assert_eq!(pts(&r.display_connector_route(merged)).len(), 2);
    // Junction pins.
    assert!(r.add_junction("K".into(), 0.0, 50.0, None));
    assert!(r.add_junction_connection_pin("K".into(), "p".into(), 7.0, Some(1.0)));
    assert!(!r.add_junction_connection_pin("K".into(), "p".into(), 7.0, Some(1.0)));
    assert_eq!(r.junction_connection_pin_directions("K".into(), "p".into()), Some(1));
    assert_eq!(r.junction_connection_pin_exclusive("K".into(), "p".into()), Some(true));
    assert!(r.set_junction_connection_pin_exclusive("K".into(), "p".into(), false));
    assert!(r.set_junction_connection_pin_connection_cost("K".into(), "p".into(), 3.0));
    let ids = parse(&r.junction_connection_pin_ids("K".into(), "p".into()));
    assert_eq!(ids.get("classId").num(), 7.0);
    let p = parse(&r.junction_connection_pin_position("K".into(), "p".into()));
    assert_eq!((p.get("x").num(), p.get("y").num()), (0.0, 50.0));
    assert!(r.remove_junction_connection_pin("K".into(), "p".into()));
}

#[test]
fn checkpoints_fixed_routes_and_curves() {
    let mut r = ortho();
    assert!(add_point_conn(&mut r, "c", (0.0, 0.0), (100.0, 100.0)));
    assert!(r.set_connector_routing_checkpoints("c".into(), vec![50.0, 20.0, 15.0, 8.0]));
    let cps = parse(&r.connector_routing_checkpoints("c".into()));
    let cp = &cps.arr()[0];
    assert_eq!(cp.keys(), vec!["point", "arrivalDirections", "departureDirections"]);
    assert_eq!((cp.get("point").get("x").num(), cp.get("departureDirections").num()), (50.0, 8.0));
    assert!(r.process_transaction());
    assert!(pts(&r.display_connector_route("c".into())).contains(&(50.0, 20.0)));
    assert_eq!(r.connector_has_fixed_route("c".into()), Some(false));
    assert!(r.set_connector_fixed_existing_route("c".into()));
    assert_eq!(r.connector_has_fixed_route("c".into()), Some(true));
    assert!(r.set_connector_fixed_route("c".into(), vec![0.0, 0.0, 0.0, 100.0, 100.0, 100.0]));
    assert_eq!(pts(&r.display_connector_route("c".into())), vec![(0.0, 0.0), (0.0, 100.0), (100.0, 100.0)]);
    assert!(r.process_transaction());
    assert_eq!(pts(&r.display_connector_route("c".into())), vec![(0.0, 0.0), (0.0, 100.0), (100.0, 100.0)]);
    assert!(r.clear_connector_fixed_route("c".into()));
    assert!(r.make_connector_path_invalid("c".into()));
    assert!(r.process_transaction());
    assert_eq!(r.connector_needs_repaint("c".into()), Some(true));
    // A connector without a route cannot be fixed (upstream asserts).
    assert!(r.add_connector_without_endpoints("d".into(), None));
    assert!(!r.set_connector_fixed_existing_route("d".into()));
    // Curves: same as Polygon::curvedPolyline on the display route.
    let curved = parse(&r.connector_curved_route("c".into(), 5.0, None));
    assert_eq!(curved.keys(), vec!["points", "commands"]);
    assert_eq!(curved.get("commands").arr().len(), curved.get("points").arr().len());
    let direct = parse(&curved_polyline(vec![0.0, 0.0, 0.0, 100.0, 100.0, 100.0], 5.0, Some(false)));
    assert_eq!(direct.get("commands").strings(), vec!["M", "L", "C", "C", "C", "L"]);
    // Routing type / hate crossings.
    assert_eq!(r.connector_routing_type("c".into()), Some(2));
    assert!(r.set_connector_routing_type("c".into(), 1.0));
    assert_eq!(r.connector_routing_type("c".into()), Some(2)); // validConnType
    assert_eq!(r.connector_does_hate_crossings("c".into()), Some(false));
    assert!(r.set_connector_hate_crossings("c".into(), true));
    assert_eq!(r.connector_does_hate_crossings("c".into()), Some(true));
    // routeAll = processTransaction + display routes.
    let all = parse(&r.route_all());
    assert_eq!(all.keys(), vec!["c", "d"]);
    assert_eq!(all.get("d").arr().len(), 0);
    assert_eq!(r.exists_crossings(None), 0);
    assert!(!r.exists_orthogonal_segment_overlap(None));
    assert!(!r.exists_orthogonal_fixed_segment_overlap(Some(true)));
    assert!(!r.exists_orthogonal_touching_paths());
    assert!(r.output_diagram_text(None).contains("path\nid=1\n"));
    assert!(r.print_info().contains("Visibility Graph info"));
}

#[test]
fn hyperedge_rerouting_results_use_string_ids() {
    let mut r = ortho();
    assert!(r.add_shape("a".into(), 0.0, 0.0, 10.0, 10.0, None));
    assert!(r.add_shape("b".into(), 100.0, 0.0, 110.0, 10.0, None));
    assert!(r.add_shape("c".into(), 50.0, 100.0, 60.0, 110.0, None));
    for (s, x, y, d) in [("a", 1.0, 0.5, 8.0), ("b", 0.0, 0.5, 4.0), ("c", 0.5, 0.0, 1.0)] {
        assert!(r.add_shape_connection_pin(s.into(), "p".into(), 1.0, x, y, Some(true), None, Some(d)));
    }
    assert!(r.add_junction("J".into(), 55.0, 5.0, None));
    for (cid, s) in [("ca", "a"), ("cb", "b"), ("cc", "c")] {
        assert!(r.add_connector_with_conn_ends(
            cid.into(), 2.0, "J".into(), NONE.into(), 0.0, 0.0, 0.0, 1.0, s.into(), NONE.into(), 0.0, 0.0, 1.0, None
        ));
    }
    assert!(r.process_transaction());
    assert_eq!(r.register_hyperedge_for_rerouting_junction("J".into()), Some(0));
    assert_eq!(r.hyperedge_rerouting_count(), 1);
    // Objects referenced by a pending registration cannot be removed.
    assert!(!r.remove_junction("J".into()));
    assert!(r.process_transaction());
    assert_eq!(r.hyperedge_rerouting_count(), 0);
    let nd = parse(&r.hyperedge_new_and_deleted_object_lists(0.0));
    assert_eq!(
        nd.keys(),
        vec!["newJunctionIds", "deletedJunctionIds", "newConnectorIds", "deletedConnectorIds", "changedConnectorIds"]
    );
    assert_eq!(nd.get("deletedJunctionIds").strings(), vec!["J"]);
    let mut deleted = nd.get("deletedConnectorIds").strings();
    deleted.sort();
    assert_eq!(deleted, vec!["ca", "cb", "cc"]);
    let new_conns = nd.get("newConnectorIds").strings();
    let new_junctions = nd.get("newJunctionIds").strings();
    assert_eq!(new_conns.len(), 3);
    assert_eq!(new_junctions.len(), 1);
    // Core-created objects are named by their numeric id.
    for id in new_conns.iter().chain(new_junctions.iter()) {
        assert_eq!(r.object_numeric_id(id.clone()).unwrap().to_string(), *id);
    }
    assert!(r.object_id_is_unused("J".into()));
    assert!(r.object_id_is_unused("ca".into()));
    let mut listed = parse(&r.connector_ids()).strings();
    listed.sort();
    let mut expected = new_conns.clone();
    expected.sort();
    assert_eq!(listed, expected);
    assert!(parse(&r.hyperedge_new_and_deleted_object_lists(1.0)).is_null());
    // The new connectors are routed and reported in later transactions.
    assert!(r.process_transaction());
    for id in &new_conns {
        assert!(pts(&r.display_connector_route(id.clone())).len() >= 2);
    }
    assert!(r.move_shape_by("c".into(), 5.0, 0.0));
    assert!(r.process_transaction());
    assert!(!parse(&r.last_transaction_updated_connector_ids()).strings().is_empty());
    let imp = parse(&r.new_and_deleted_object_lists_from_hyperedge_improvement());
    assert_eq!(imp.keys().len(), 5);

    // ConnEnd-list registration.
    let mut r2 = ortho();
    for (s, x, px, d) in [("a", 0.0, 1.0, 8.0), ("b", 100.0, 0.0, 4.0)] {
        assert!(r2.add_shape(s.into(), x, 0.0, x + 10.0, 10.0, None));
        assert!(r2.add_shape_connection_pin(s.into(), "p".into(), 1.0, px, 0.5, Some(true), None, Some(d)));
    }
    assert!(r2.add_shape("c".into(), 50.0, 100.0, 60.0, 110.0, None));
    assert!(r2.add_shape_connection_pin("c".into(), "p".into(), 1.0, 0.5, 0.0, Some(true), None, Some(1.0)));
    assert!(r2.process_transaction());
    // A shape terminal needs a pin of that class.
    assert_eq!(
        r2.register_hyperedge_for_rerouting(
            vec![1.0, 1.0],
            vec!["a".into(), "b".into()],
            vec![String::new(), String::new()],
            vec![0.0, 0.0],
            vec![0.0, 0.0],
            vec![1.0, 9.0]
        ),
        None
    );
    // Free points, duplicates and fewer than three terminals are refused.
    let refused = |r2: &mut AvoidRouter, kinds: Vec<f64>, objs: Vec<&str>| {
        let n = kinds.len();
        r2.register_hyperedge_for_rerouting(
            kinds,
            objs.into_iter().map(String::from).collect(),
            vec![String::new(); n],
            vec![0.0; n],
            vec![0.0; n],
            vec![1.0; n],
        )
    };
    assert_eq!(refused(&mut r2, vec![1.0, 1.0, 0.0], vec!["a", "b", ""]), None);
    assert_eq!(refused(&mut r2, vec![1.0, 1.0, 1.0], vec!["a", "b", "b"]), None);
    assert_eq!(refused(&mut r2, vec![1.0, 1.0], vec!["a", "b"]), None);
    let idx = r2.register_hyperedge_for_rerouting(
        vec![1.0, 3.0, 1.0],
        vec!["a".into(), "b".into(), "c".into()],
        vec![String::new(), "p".into(), String::new()],
        vec![0.0, 0.0, 0.0],
        vec![0.0, 0.0, 0.0],
        vec![1.0, 0.0, 1.0],
    );
    assert_eq!(idx, Some(0));
    assert!(!r2.remove_shape("a".into()));
    assert!(r2.process_transaction());
    let nd = parse(&r2.hyperedge_new_and_deleted_object_lists(0.0));
    assert_eq!(nd.get("newConnectorIds").arr().len(), 3);
    assert_eq!(nd.get("newJunctionIds").arr().len(), 1);
    assert!(r2.remove_shape("a".into()));
}

#[test]
fn clear_keeps_settings_and_drops_objects() {
    let mut r = ortho();
    r.set_routing_parameter(6.0, Some(7.0));
    r.set_routing_option(0.0, true);
    r.set_transaction_use(false);
    assert!(r.add_shape("s".into(), 0.0, 0.0, 1.0, 1.0, Some(50.0)));
    r.clear();
    assert_eq!(r.routing_parameter(6.0), Some(7.0));
    assert_eq!(r.routing_option(0.0), Some(true));
    assert!(!r.transaction_use());
    assert!(r.object_id_is_unused("s".into()));
    assert_eq!(r.new_object_id(), 1);
}

#[test]
fn clusters_wrapper_equals_core() {
    // No upstream fixture uses clusters: compare a polyline cluster scene
    // driven through the wrapper with the same scene on the core API.
    let mut w = AvoidRouter::create(POLY_LINE_ROUTING).unwrap();
    let mut core = Router::new(POLY_LINE_ROUTING);
    w.set_routing_parameter(3.0, Some(500.0));
    core.set_routing_parameter(RoutingParameter::ClusterCrossingPenalty, 500.0);
    let shapes = [(0.0, 0.0, 20.0, 20.0), (60.0, 10.0, 80.0, 40.0), (30.0, 50.0, 50.0, 70.0)];
    for (i, s) in shapes.iter().enumerate() {
        assert!(w.add_shape(format!("s{}", i), s.0, s.1, s.2, s.3, None));
        ShapeRef::new(&mut core, &Rectangle::new(&Point::new(s.0, s.1), &Point::new(s.2, s.3)), 0);
    }
    // Cluster corners on shape corners (upstream asserts that clustered
    // poly-line routing finds a vertex at every cluster corner).
    let cl = rect(60.0, 10.0, 80.0, 40.0);
    assert!(w.add_cluster("k".into(), cl.to_vec(), None));
    let mut cp = Polygon::new();
    cp.ps = cl.chunks(2).map(|c| Point::new(c[0], c[1])).collect();
    let k = ClusterRef::new(&mut core, &cp, 0);
    let conns = [((-10.0, 30.0), (100.0, 25.0)), ((40.0, -10.0), (40.0, 90.0)), ((10.0, 80.0), (95.0, -5.0))];
    for (i, (a, b)) in conns.iter().enumerate() {
        assert!(add_point_conn(&mut w, &format!("c{}", i), *a, *b));
        ConnRef::new_with_ends(&mut core, &ConnEnd::from_point(Point::new(a.0, a.1)), &ConnEnd::from_point(Point::new(b.0, b.1)), 0);
    }
    assert!(w.process_transaction());
    assert!(core.process_transaction());
    assert_eq!(w.output_diagram_text(Some(true)), core.output_diagram_text());
    assert_eq!(pts(&w.cluster_polygon("k".into())), cl.chunks(2).map(|c| (c[0], c[1])).collect::<Vec<_>>());
    assert_eq!(pts(&w.cluster_rectangular_polygon("k".into())).len(), 4);
    let bb = parse(&w.cluster_bounding_box("k".into()));
    assert_eq!((bb.get("left").num(), bb.get("right").num()), (60.0, 80.0));
    // setNewPoly and deleteCluster, then moves, on both.
    assert!(w.set_cluster_rectangle("k".into(), 0.0, 0.0, 20.0, 20.0));
    k.set_new_poly(&mut core, &Rectangle::new(&Point::new(0.0, 0.0), &Point::new(20.0, 20.0)));
    assert!(w.move_shape_by("s2".into(), 5.0, 5.0));
    let s2 = ShapeRef(core.m_obstacles.iter().copied().find(|&o| core.obstacles[o].m_id == 3).unwrap());
    core.move_shape_by(s2, 5.0, 5.0);
    assert!(w.process_transaction());
    core.process_transaction();
    assert_eq!(w.output_diagram_text(Some(true)), core.output_diagram_text());
    assert!(w.remove_cluster("k".into()));
    assert!(!w.remove_cluster("k".into()));
    core.delete_cluster(k);
    assert!(w.move_shape_by("s2".into(), -5.0, 0.0));
    core.move_shape_by(s2, -5.0, 0.0);
    assert!(w.process_transaction());
    core.process_transaction();
    assert_eq!(w.output_diagram_text(Some(true)), core.output_diagram_text());
    assert_eq!(w.output_diagram_text(None), core.output_diagram_text_g());
    assert!(w.exists_crossings(Some(false)) >= 0);
}

#[test]
fn json_strings_are_escaped() {
    let mut r = ortho();
    let id = "we\"ird\\id\n\u{1}";
    assert!(r.add_shape(id.into(), 0.0, 0.0, 1.0, 1.0, None));
    assert_eq!(parse(&r.shape_ids()).strings(), vec![id.to_string()]);
}
