//! Crashes in the (faithfully ported) core found while building the JS
//! wrapper.  They mirror upstream code paths that `COLA_ASSERT` and then
//! dereference a null pointer, so they are not fixed in `src/avoid/`; the
//! wrapper refuses the inputs that lead there (see `src/wasm_api.rs`).
//! The tests are `#[ignore]`d because they panic; run them with
//! `cargo test --test core_findings -- --ignored` to reproduce.

use libavoid_rust::avoid::connectionpin::ShapeConnectionPin;
use libavoid_rust::avoid::connend::ConnEnd;
use libavoid_rust::avoid::connector::ConnRef;
use libavoid_rust::avoid::geomtypes::{Point, Rectangle};
use libavoid_rust::avoid::router::{Router, ORTHOGONAL_ROUTING, POLY_LINE_ROUTING};
use libavoid_rust::avoid::shape::ShapeRef;

fn rect(l: f64, t: f64, r: f64, b: f64) -> libavoid_rust::avoid::geomtypes::Polygon {
    Rectangle::new(&Point::new(l, t), &Point::new(r, b))
}

/// HyperedgeRerouter with a free-point terminal: the MTST does not connect
/// the point (debug: `orig_terminals.len() == 1` assertion; release:
/// `performRerouting(): null tree root`).  Unverified whether upstream C++
/// fails the same way on this scene (upstream asserts `treeRoot` and then
/// dereferences it).
#[test]
#[ignore = "core panic (upstream null tree root); wrapper refuses point terminals"]
fn hyperedge_free_point_terminal_null_tree_root() {
    let mut r = Router::new(ORTHOGONAL_ROUTING);
    let b = ShapeRef::new(&mut r, &rect(100.0, 0.0, 120.0, 20.0), 0);
    ShapeConnectionPin::new_shape(&mut r, b, 2, 0.0, 0.5, true, 0.0, 4);
    let c = ShapeRef::new(&mut r, &rect(0.0, 100.0, 20.0, 120.0), 0);
    ShapeConnectionPin::new_shape(&mut r, c, 2, 0.5, 0.0, true, 0.0, 1);
    r.process_transaction();
    let ends = vec![
        ConnEnd::from_shape_pin(&r, b, 2),
        ConnEnd::from_shape_pin(&r, c, 2),
        ConnEnd::from_point(Point::new(50.0, 50.0)),
    ];
    r.hyperedge_rerouter().register_hyperedge_for_rerouting(ends);
    r.process_transaction();
}

/// Two-terminal hyperedge: no junction in the MTST, null tree root.
#[test]
#[ignore = "core panic (upstream null tree root); wrapper requires >= 3 terminals"]
fn hyperedge_two_terminals_null_tree_root() {
    let mut r = Router::new(ORTHOGONAL_ROUTING);
    let b = ShapeRef::new(&mut r, &rect(100.0, 0.0, 120.0, 20.0), 0);
    ShapeConnectionPin::new_shape(&mut r, b, 2, 0.0, 0.5, true, 0.0, 4);
    let c = ShapeRef::new(&mut r, &rect(0.0, 100.0, 20.0, 120.0), 0);
    ShapeConnectionPin::new_shape(&mut r, c, 2, 0.5, 0.0, true, 0.0, 1);
    r.process_transaction();
    let ends = vec![ConnEnd::from_shape_pin(&r, b, 2), ConnEnd::from_shape_pin(&r, c, 2)];
    r.hyperedge_rerouter().register_hyperedge_for_rerouting(ends);
    r.process_transaction();
}

/// Upstream `ConnRef(router, src, dst)` registers the reroute flag after
/// `setEndpoints()`; with transaction use off the connector is routed in
/// between (debug: assertion; release, poly-line: unwrap of the missing
/// flag).  The wrapper queues such constructions.
#[test]
#[ignore = "core panic mirroring upstream's missing reroute flag; wrapper works around it"]
fn conn_with_ends_without_transactions_missing_reroute_flag() {
    let mut r = Router::new(POLY_LINE_ROUTING);
    r.set_transaction_use(false);
    ConnRef::new_with_ends(
        &mut r,
        &ConnEnd::from_point(Point::new(0.0, 0.0)),
        &ConnEnd::from_point(Point::new(10.0, 10.0)),
        0,
    );
}
