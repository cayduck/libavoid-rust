//! Crashes in the core found while building the JS wrapper.  They mirror
//! upstream code paths that `COLA_ASSERT` and then dereference a null
//! pointer; the core now handles them gracefully (see "Public-API misuse"
//! in the `src/avoid/router.rs` module docs) and these tests check that it
//! does not panic and what it does instead.

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
    // Graceful: the hyperedge is skipped (no junction, no connectors).
    let lists = r.hyperedge_rerouter().new_and_deleted_object_lists(0);
    assert!(lists.new_junction_list.is_empty() && lists.new_connector_list.is_empty());
    assert!(r.conn_refs.is_empty());
}

/// Two-terminal hyperedge: no junction in the MTST, null tree root.
#[test]
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
    // Graceful: the hyperedge is skipped (no junction, no connectors).
    let lists = r.hyperedge_rerouter().new_and_deleted_object_lists(0);
    assert!(lists.new_junction_list.is_empty() && lists.new_connector_list.is_empty());
    assert!(r.conn_refs.is_empty());
}

/// Upstream `ConnRef(router, src, dst)` registers the reroute flag after
/// `setEndpoints()`; with transaction use off the connector is routed in
/// between (debug: assertion; release, poly-line: unwrap of the missing
/// flag).  The wrapper queues such constructions.
#[test]
fn conn_with_ends_without_transactions_missing_reroute_flag() {
    let mut r = Router::new(POLY_LINE_ROUTING);
    r.set_transaction_use(false);
    let c = ConnRef::new_with_ends(
        &mut r,
        &ConnEnd::from_point(Point::new(0.0, 0.0)),
        &ConnEnd::from_point(Point::new(10.0, 10.0)),
        0,
    );
    // The reroute flag is registered before the endpoints are set, so the
    // connector is routed at once.
    let route = c.display_route(&mut r).clone();
    assert_eq!(route.ps.len(), 2);
}

/// splitAtSegment on a connector whose destination is a free point:
/// upstream copies the null `m_dst_connend`.  The new connector gets a
/// point end at the old destination.
#[test]
fn split_at_segment_free_point_destination() {
    let mut r = Router::new(ORTHOGONAL_ROUTING);
    ShapeRef::new(&mut r, &rect(40.0, -10.0, 60.0, 10.0), 0);
    let c = ConnRef::new_with_ends(
        &mut r,
        &ConnEnd::from_point(Point::new(0.0, 0.0)),
        &ConnEnd::from_point(Point::new(100.0, 0.0)),
        0,
    );
    r.process_transaction();
    let (j, nc) = c.split_at_segment(&mut r, 1);
    assert!(j.is_some() && nc.is_some());
    r.process_transaction();
    let nr = nc.unwrap().display_route(&mut r).clone();
    let last = nr.ps[nr.ps.len() - 1];
    assert_eq!((last.x, last.y), (100.0, 0.0));
    assert_eq!(c.split_at_segment(&mut r, 0), (None, None));
}
