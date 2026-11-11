//! Statement-by-statement translations of the upstream libavoid test
//! fixtures (`vendor/adaptagrams/cola/libavoid/tests/<name>.cpp`) whose
//! routers use poly-line routing only (`new Router(PolyLineRouting)`).
//!
//! Each test compares every `outputDiagram()` dump and the final router
//! state (`<name>-final.txt`, written by the oracle's patched Router
//! destructor) with the goldens produced by the real C++, plus the
//! fixture's stdout (`<name>.stdout`) and exit code (`<name>.exit`).
//!
//! Fixtures that construct `Router(PolyLineRouting | OrthogonalRouting)`
//! or `Router(OrthogonalRouting)` need the orthogonal visibility graph and
//! are not translated here.
//!
//! Connector callbacks: upstream callbacks run inside the transaction and
//! print the connector's route.  Rust callbacks cannot borrow the router,
//! so the callback only records the connector in a shared log and the
//! test prints the log (with the route, which does not change after the
//! callbacks of a transaction) right after the statement that triggered
//! the transaction.

mod common;

use std::cell::RefCell;
use std::rc::Rc;

use common::*;
use libavoid_rust::avoid::arena::ConnIdx;
use libavoid_rust::avoid::connector::ConnRef;
use libavoid_rust::avoid::connend::ConnEnd;
use libavoid_rust::avoid::geomtypes::{Point, Polygon};
use libavoid_rust::avoid::router::{Router, POLY_LINE_ROUTING};
use libavoid_rust::avoid::shape::ShapeRef;

type CallbackLog = Rc<RefCell<Vec<ConnIdx>>>;

fn recorder(log: &CallbackLog) -> Option<Box<dyn FnMut(ConnIdx)>> {
    let log = log.clone();
    Some(Box::new(move |c| log.borrow_mut().push(c)))
}

/// Upstream `connCallback()` of example.cpp / latesetup.cpp, applied to
/// the callbacks recorded since the last flush.  `display` selects
/// `displayRoute()` (example.cpp) vs `route()` (latesetup.cpp).
fn flush_callbacks(router: &mut Router, log: &CallbackLog, out: &mut String, display: bool) {
    let conns: Vec<ConnIdx> = log.borrow_mut().drain(..).collect();
    for conn_ref in conns {
        out.push_str(&format!("Connector {} needs rerouting!\n", conn_ref.id(router)));

        let route = if display {
            conn_ref.display_route(router).clone()
        } else {
            conn_ref.route(router).clone()
        };
        out.push_str("New path: ");
        for i in 0..route.ps.len() {
            out.push_str(&format!(
                "{}({}, {})",
                if i > 0 { "-" } else { "" },
                printf_f(route.ps[i].x),
                printf_f(route.ps[i].y)
            ));
        }
        out.push('\n');
    }
}

fn assert_stdout_matches(name: &str, out: &str) {
    let golden = read_golden_file(name, "stdout");
    assert_eq!(out, golden, "stdout of fixture {} differs from upstream", name);
}

// upstream: tests/example.cpp
#[test]
fn example() {
    let mut out = String::new();
    let log: CallbackLog = Rc::default();

    let mut router = Router::new(POLY_LINE_ROUTING);

    let src_pt = Point::new(1.2, 0.5);
    let dst_pt = Point::new(1.5, 4.0);
    let conn_ref = ConnRef::new_with_ends(&mut router, &ConnEnd::from_point(src_pt), &ConnEnd::from_point(dst_pt), 0);
    conn_ref.set_callback(&mut router, recorder(&log));
    // Force inital callback:
    router.process_transaction();
    flush_callbacks(&mut router, &log, &mut out, true);

    out.push_str("\nAdding a shape.\n");
    // Create the ShapeRef:
    let mut shape_poly = Polygon::with_size(3);
    shape_poly.ps[0] = Point::new(1.0, 1.0);
    shape_poly.ps[1] = Point::new(2.5, 1.5);
    shape_poly.ps[2] = Point::new(1.5, 2.5);
    let shape_ref = ShapeRef::new(&mut router, &shape_poly, 0);
    router.process_transaction();
    flush_callbacks(&mut router, &log, &mut out, true);

    out.push_str("\nShifting endpoint.\n");
    let dst_pt2 = Point::new(6.0, 4.5);
    conn_ref.set_dest_endpoint(&mut router, &ConnEnd::from_point(dst_pt2));
    // It's expected you know the connector needs rerouting, so the callback
    // isn't called.  You can force it to be called though, via:
    router.process_transaction();
    flush_callbacks(&mut router, &log, &mut out, true);

    out.push_str("\nMoving shape right by 0.5.\n");
    router.move_shape_by(shape_ref, 0.5, 0.0);
    router.process_transaction();
    flush_callbacks(&mut router, &log, &mut out, true);

    assert_matches_golden(&mut router, "example");
    // delete router;
    assert_matches_golden(&mut router, "example-final");
    router.destruct();

    assert_stdout_matches("example", &out);
    // return 0;
    assert_eq!(golden_exit_code("example"), 0);
}

// upstream: tests/latesetup.cpp
#[test]
fn latesetup() {
    let mut out = String::new();
    let log: CallbackLog = Rc::default();

    let mut router = Router::new(POLY_LINE_ROUTING);
    router.set_transaction_use(false);

    out.push_str("\nCreate connector.\n");
    let src_pt = Point::new(1.2, 0.5);
    let dst_pt = Point::new(1.5, 4.0);
    let conn_ref = ConnRef::new(&mut router, 0);
    conn_ref.set_callback(&mut router, recorder(&log));
    // Force inital callback:
    router.process_transaction();
    flush_callbacks(&mut router, &log, &mut out, false);

    let conn_ref2 = ConnRef::new(&mut router, 0);
    conn_ref2.set_callback(&mut router, recorder(&log));
    router.process_transaction();
    flush_callbacks(&mut router, &log, &mut out, false);

    router.delete_connector(conn_ref2);
    router.process_transaction();
    flush_callbacks(&mut router, &log, &mut out, false);

    out.push_str("\nAdd endpoints.\n");
    conn_ref.set_endpoints(&mut router, &ConnEnd::from_point(src_pt), &ConnEnd::from_point(dst_pt));
    flush_callbacks(&mut router, &log, &mut out, false);
    router.process_transaction();
    flush_callbacks(&mut router, &log, &mut out, false);

    out.push_str("\nAdding a shape.\n");
    // Create the ShapeRef:
    let mut shape_poly = Polygon::with_size(3);
    shape_poly.ps[0] = Point::new(1.0, 1.0);
    shape_poly.ps[1] = Point::new(2.5, 1.5);
    shape_poly.ps[2] = Point::new(1.5, 2.5);
    let shape_ref = ShapeRef::new(&mut router, &shape_poly, 0);
    flush_callbacks(&mut router, &log, &mut out, false);
    router.process_transaction();
    flush_callbacks(&mut router, &log, &mut out, false);

    out.push_str("\nShifting endpoint.\n");
    let dst_pt2 = Point::new(6.0, 4.5);
    conn_ref.set_dest_endpoint(&mut router, &ConnEnd::from_point(dst_pt2));
    flush_callbacks(&mut router, &log, &mut out, false);
    // It's expected you know the connector needs rerouting, so the callback
    // isn't called.  You can force it to be called though, via:
    router.process_transaction();
    flush_callbacks(&mut router, &log, &mut out, false);

    out.push_str("\nMoving shape right by 0.5.\n");
    router.move_shape_by(shape_ref, 0.5, 0.0);
    flush_callbacks(&mut router, &log, &mut out, false);
    router.process_transaction();
    flush_callbacks(&mut router, &log, &mut out, false);

    // delete router;
    assert_matches_golden(&mut router, "latesetup-final");
    router.destruct();

    assert_stdout_matches("latesetup", &out);
    // return 0;
    assert_eq!(golden_exit_code("latesetup"), 0);
}

/// Self-test of the golden comparison helper.
#[test]
fn golden_harness_compares_numerically() {
    let golden = "path\nid=1\np0: 1.2 0.5 \n\n";
    let close = "path\nid=1\np0: 1.2000000000001 0.5 \n\n";
    let far = "path\nid=1\np0: 1.21 0.5 \n\n";
    let c = compare_diagram_text(golden, golden).unwrap();
    assert!(c.bit_identical && c.max_abs_diff == 0.0);
    let c = compare_diagram_text(close, golden).unwrap();
    assert!(!c.bit_identical && c.max_abs_diff > 0.0 && c.max_abs_diff < 1e-9);
    assert!(compare_diagram_text(far, golden).is_err());
    assert!(compare_diagram_text("path\nid=2\n", "path\nid=1\n").is_err());
}
