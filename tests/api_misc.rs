//! Unit tests for public API with no C++-visible output of its own (so not
//! covered by the differential oracles): the topology-addon hook, the
//! progress/continuation hook, transaction-use getter, `printInfo()`.

use std::cell::Cell;
use std::rc::Rc;

use libavoid_rust::avoid::connector::ConnRef;
use libavoid_rust::avoid::connend::ConnEnd;
use libavoid_rust::avoid::geomtypes::{Point, Rectangle};
use libavoid_rust::avoid::router::*;
use libavoid_rust::avoid::shape::ShapeRef;

#[derive(Clone)]
struct CountingAddon(Rc<Cell<u32>>);

impl TopologyAddonInterface for CountingAddon {
    fn clone_box(&self) -> Box<dyn TopologyAddonInterface> {
        Box::new(self.clone())
    }
    fn improve_orthogonal_topology(&mut self, router: &mut Router) {
        // The router is usable from the hook.
        let _ = router.conn_refs.len();
        self.0.set(self.0.get() + 1);
    }
}

fn scene(r: &mut Router) {
    ShapeRef::new(r, &Rectangle::new(&Point::new(50.0, 50.0), &Point::new(100.0, 100.0)), 0);
    ConnRef::new_with_ends(
        r,
        &ConnEnd::from_point(Point::new(0.0, 75.0)),
        &ConnEnd::from_point(Point::new(200.0, 75.0)),
        0,
    );
}

// upstream: router.cpp Router::setTopologyAddon() / improveOrthogonalTopology()
#[test]
fn topology_addon_is_called_once_per_orthogonal_improvement() {
    let count = Rc::new(Cell::new(0));
    let mut r = Router::new(ORTHOGONAL_ROUTING);
    r.set_topology_addon(Some(&CountingAddon(count.clone())));
    // setTopologyAddon registers a settings change: the next transaction
    // runs even without actions.
    assert!(r.process_transaction());
    let after_settings = count.get();
    scene(&mut r);
    r.process_transaction();
    assert!(count.get() > after_settings);
    // Resetting to the default addon.
    r.set_topology_addon(None);
    let before = count.get();
    r.process_transaction();
    assert_eq!(count.get(), before);
}

// upstream: router.h Router::shouldContinueTransactionWithProgress() (virtual)
#[test]
fn progress_callback_can_abort_the_transaction() {
    let mut r = Router::new(ORTHOGONAL_ROUTING);
    let calls = Rc::new(Cell::new(0u32));
    let c2 = calls.clone();
    r.m_progress_callback = Some(Box::new(move |_ms, phase, total, proportion| {
        assert!(phase >= 1 && phase <= total);
        assert!((0.0..=1.0).contains(&proportion));
        c2.set(c2.get() + 1);
        false
    }));
    scene(&mut r);
    r.process_transaction();
    assert!(calls.get() > 0);
    assert!(r.m_abort_transaction);
}

#[test]
fn transaction_use_and_print_info() {
    let mut r = Router::new(POLY_LINE_ROUTING);
    assert!(r.transaction_use());
    r.set_transaction_use(false);
    assert!(!r.transaction_use());
    r.set_transaction_use(true);
    scene(&mut r);
    r.process_transaction();
    let info = r.print_info();
    assert!(info.contains("Number of shapes: 1"), "{}", info);
    assert!(info.contains("Number of vertices: 6 (4 real, 2 endpoints)"), "{}", info);
}
