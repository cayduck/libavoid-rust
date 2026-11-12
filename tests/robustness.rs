//! Robustness suite: public-API misuse and degenerate input that upstream
//! libavoid leaves undefined (or crashes on).
//!
//! The release WASM build uses `panic = "abort"`, so a panic kills the
//! instance.  Required outcome for every case, in debug and release
//! builds: no panic, no infinite loop / unbounded recursion, and the router
//! remains usable afterwards (a fresh shape + connector still routes).
//! Where upstream's behaviour is defined the port matches it (checked by
//! the oracles); the behaviour chosen where upstream is undefined is
//! documented next to each case and in the `router.rs` module docs
//! ("Public-API misuse").
//!
//! Every scenario runs for all four combinations of routing type
//! (orthogonal / poly-line) and transaction mode (on / off), each in its
//! own thread with a timeout.

#![allow(clippy::uninlined_format_args)]

use std::sync::mpsc;
use std::time::Duration;

use libavoid_rust::avoid::connectionpin::{ShapeConnectionPin, ATTACH_POS_CENTRE, CONNECTIONPIN_CENTRE};
use libavoid_rust::avoid::connector::{Checkpoint, ConnRef, ConnType};
use libavoid_rust::avoid::connend::ConnEnd;
use libavoid_rust::avoid::geomtypes::{Point, Polygon, Rectangle};
use libavoid_rust::avoid::junction::JunctionRef;
use libavoid_rust::avoid::router::*;
use libavoid_rust::avoid::shape::ShapeRef;
use libavoid_rust::avoid::vertices::CONN_DIR_ALL;
use libavoid_rust::avoid::viscluster::ClusterRef;

const TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Clone, Copy, Debug)]
struct Cfg {
    flags: u32,
    transactions: bool,
}

impl Cfg {
    fn conn_type(self) -> ConnType {
        if self.flags == ORTHOGONAL_ROUTING {
            ConnType::ConnTypeOrthogonal
        } else {
            ConnType::ConnTypePolyLine
        }
    }
}

const CFGS: [Cfg; 4] = [
    Cfg { flags: ORTHOGONAL_ROUTING, transactions: true },
    Cfg { flags: ORTHOGONAL_ROUTING, transactions: false },
    Cfg { flags: POLY_LINE_ROUTING, transactions: true },
    Cfg { flags: POLY_LINE_ROUTING, transactions: false },
];

fn rect(x1: f64, y1: f64, x2: f64, y2: f64) -> Polygon {
    Rectangle::new(&Point::new(x1, y1), &Point::new(x2, y2))
}

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn conn(r: &mut Router, cfg: Cfg, a: &ConnEnd, b: &ConnEnd) -> libavoid_rust::avoid::arena::ConnIdx {
    let c = ConnRef::new_with_ends(r, a, b, 0);
    c.set_routing_type(r, cfg.conn_type());
    c
}

/// After the misuse: a fresh scene in the same router must still route.
fn assert_usable(r: &mut Router, cfg: Cfg) {
    let s = ShapeRef::new(r, &rect(10000.0, 10000.0, 10050.0, 10050.0), 0);
    let c = conn(r, cfg, &ConnEnd::from_point(pt(9900.0, 10025.0)), &ConnEnd::from_point(pt(10150.0, 10025.0)));
    r.process_transaction();
    let route = c.display_route(r).clone();
    assert!(route.ps.len() >= 2, "router unusable after misuse: route {:?}", route.ps);
    let first = route.ps[0];
    let last = route.ps[route.ps.len() - 1];
    assert_eq!((first.x, first.y), (9900.0, 10025.0));
    assert_eq!((last.x, last.y), (10150.0, 10025.0));
    r.delete_connector(c);
    r.delete_shape(s);
    r.process_transaction();
}

fn run_one<F>(name: &str, cfg: Cfg, f: F)
where
    F: Fn(&mut Router, Cfg) + Send + Sync + 'static + Clone,
{
    let (tx, rx) = mpsc::channel();
    let label = format!("{} [{:?}]", name, cfg);
    std::thread::Builder::new()
        .name(label.clone())
        .stack_size(64 << 20)
        .spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut r = Router::new(cfg.flags);
                r.set_transaction_use(cfg.transactions);
                f(&mut r, cfg);
                r.process_transaction();
                assert_usable(&mut r, cfg);
                // Tear-down must not panic either.
                r.destruct();
            }));
            let _ = tx.send(result.map_err(|e| {
                e.downcast_ref::<String>()
                    .cloned()
                    .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_default()
            }));
        })
        .unwrap();
    match rx.recv_timeout(TIMEOUT) {
        Ok(Ok(())) => {}
        Ok(Err(msg)) => panic!("{}: panicked: {}", label, msg),
        Err(_) => panic!("{}: did not finish within {:?} (infinite loop?)", label, TIMEOUT),
    }
}

fn run<F>(name: &str, f: F)
where
    F: Fn(&mut Router, Cfg) + Send + Sync + 'static + Clone,
{
    for cfg in CFGS {
        run_one(name, cfg, f.clone());
    }
}

macro_rules! robust {
    ($name:ident, |$r:ident, $cfg:ident| $body:block) => {
        #[test]
        fn $name() {
            run(stringify!($name), |$r: &mut Router, $cfg: Cfg| {
                let _ = &$cfg;
                $body
            });
        }
    };
}

// ---------------------------------------------------------------------------
// Degenerate but valid input.

robust!(empty_router_process_transaction, |r, _cfg| {
    r.process_transaction();
    r.process_transaction();
});

robust!(zero_size_shapes, |r, cfg| {
    ShapeRef::new(r, &rect(50.0, 50.0, 50.0, 50.0), 0);
    ShapeRef::new(r, &rect(80.0, 50.0, 80.0, 90.0), 0);
    ShapeRef::new(r, &rect(100.0, 60.0, 140.0, 60.0), 0);
    conn(r, cfg, &ConnEnd::from_point(pt(0.0, 50.0)), &ConnEnd::from_point(pt(200.0, 60.0)));
    r.process_transaction();
});

robust!(coincident_endpoints, |r, cfg| {
    ShapeRef::new(r, &rect(50.0, 50.0, 100.0, 100.0), 0);
    conn(r, cfg, &ConnEnd::from_point(pt(10.0, 10.0)), &ConnEnd::from_point(pt(10.0, 10.0)));
    conn(r, cfg, &ConnEnd::from_point(pt(75.0, 75.0)), &ConnEnd::from_point(pt(75.0, 75.0)));
    r.process_transaction();
});

robust!(coincident_and_overlapping_shapes, |r, cfg| {
    ShapeRef::new(r, &rect(50.0, 50.0, 100.0, 100.0), 0);
    ShapeRef::new(r, &rect(50.0, 50.0, 100.0, 100.0), 0);
    ShapeRef::new(r, &rect(60.0, 40.0, 90.0, 110.0), 0);
    conn(r, cfg, &ConnEnd::from_point(pt(0.0, 75.0)), &ConnEnd::from_point(pt(200.0, 75.0)));
    conn(r, cfg, &ConnEnd::from_point(pt(75.0, 75.0)), &ConnEnd::from_point(pt(200.0, 0.0)));
    r.process_transaction();
});

robust!(connector_without_endpoints, |r, cfg| {
    let c = ConnRef::new(r, 0);
    c.set_routing_type(r, cfg.conn_type());
    r.process_transaction();
    let _ = c.display_route(r).ps.len();
    // Only one end set.
    let c2 = ConnRef::new(r, 0);
    c2.set_source_endpoint(r, &ConnEnd::from_point(pt(0.0, 0.0)));
    r.process_transaction();
    r.delete_connector(c);
    r.delete_connector(c2);
});

robust!(extreme_penalties, |r, cfg| {
    for p in RoutingParameter::ALL {
        r.set_routing_parameter(p, 0.0);
    }
    ShapeRef::new(r, &rect(50.0, 50.0, 100.0, 100.0), 0);
    let c = conn(r, cfg, &ConnEnd::from_point(pt(0.0, 75.0)), &ConnEnd::from_point(pt(200.0, 75.0)));
    r.process_transaction();
    for p in RoutingParameter::ALL {
        r.set_routing_parameter(p, CHOOSE_SENSIBLE_PARAM_VALUE);
    }
    r.process_transaction();
    for p in RoutingParameter::ALL {
        r.set_routing_parameter(p, 1e300);
    }
    c.set_hate_crossings(r, true);
    r.process_transaction();
    for p in RoutingParameter::ALL {
        r.set_routing_parameter(p, -123.0);
    }
    for o in RoutingOption::ALL {
        r.set_routing_option(o, true);
    }
    r.process_transaction();
});

// ---------------------------------------------------------------------------
// Invalid input upstream leaves undefined.

robust!(nan_and_infinite_coordinates, |r, cfg| {
    let nan = f64::NAN;
    let inf = f64::INFINITY;
    ShapeRef::new(r, &rect(50.0, 50.0, 100.0, 100.0), 0);
    let s = ShapeRef::new(r, &rect(nan, 0.0, 10.0, 10.0), 0);
    ShapeRef::new(r, &rect(-inf, 0.0, 10.0, inf), 0);
    conn(r, cfg, &ConnEnd::from_point(pt(nan, 75.0)), &ConnEnd::from_point(pt(200.0, 75.0)));
    conn(r, cfg, &ConnEnd::from_point(pt(0.0, 75.0)), &ConnEnd::from_point(pt(inf, -inf)));
    let c = conn(r, cfg, &ConnEnd::from_point(pt(0.0, 0.0)), &ConnEnd::from_point(pt(300.0, 300.0)));
    c.set_routing_checkpoints(r, &[Checkpoint::new(pt(nan, nan))]);
    r.process_transaction();
    r.move_shape_by(s, nan, inf);
    r.process_transaction();
    r.delete_shape(s);
    r.process_transaction();
});

robust!(polygons_with_fewer_than_three_points, |r, cfg| {
    let mut p0 = Polygon::new();
    let s0 = ShapeRef::new(r, &p0, 0);
    p0.ps.push(pt(10.0, 10.0));
    let s1 = ShapeRef::new(r, &p0, 0);
    p0.ps.push(pt(20.0, 30.0));
    let s2 = ShapeRef::new(r, &p0, 0);
    conn(r, cfg, &ConnEnd::from_point(pt(0.0, 0.0)), &ConnEnd::from_point(pt(40.0, 40.0)));
    r.process_transaction();
    r.move_shape_by(s1, 5.0, 5.0);
    r.move_shape(s2, &rect(0.0, 0.0, 5.0, 5.0), false);
    r.process_transaction();
    r.delete_shape(s0);
    r.delete_shape(s1);
    r.delete_shape(s2);
});

robust!(pin_with_class_id_zero, |r, cfg| {
    let s = ShapeRef::new(r, &rect(50.0, 50.0, 100.0, 100.0), 0);
    ShapeConnectionPin::new_shape(r, s, 0, ATTACH_POS_CENTRE, ATTACH_POS_CENTRE, true, 0.0, CONN_DIR_ALL);
    let ce = ConnEnd::from_shape_pin(r, s, 0);
    conn(r, cfg, &ce, &ConnEnd::from_point(pt(200.0, 75.0)));
    r.process_transaction();
});

robust!(pin_class_without_pins, |r, cfg| {
    let s = ShapeRef::new(r, &rect(50.0, 50.0, 100.0, 100.0), 0);
    let ce = ConnEnd::from_shape_pin(r, s, 7);
    let c = conn(r, cfg, &ce, &ConnEnd::from_point(pt(200.0, 75.0)));
    r.process_transaction();
    // A pin of another class added afterwards (with transactions off, its
    // registration reroutes the connector before the pin has a vertex).
    ShapeConnectionPin::new_shape(r, s, 1, ATTACH_POS_CENTRE, ATTACH_POS_CENTRE, true, 0.0, CONN_DIR_ALL);
    r.process_transaction();
    // And then one of the wanted class.
    ShapeConnectionPin::new_shape(r, s, 7, ATTACH_POS_CENTRE, ATTACH_POS_CENTRE, true, 0.0, CONN_DIR_ALL);
    r.process_transaction();
    let _ = c.display_route(r).ps.len();
});

// ---------------------------------------------------------------------------
// Deletion in every order, both transaction modes.

fn scene(r: &mut Router, cfg: Cfg) -> (ShapeRef, ShapeRef, JunctionRef, Vec<libavoid_rust::avoid::arena::ConnIdx>) {
    let s1 = ShapeRef::new(r, &rect(0.0, 0.0, 50.0, 50.0), 0);
    let s2 = ShapeRef::new(r, &rect(200.0, 0.0, 250.0, 50.0), 0);
    ShapeConnectionPin::new_shape(r, s1, 1, ATTACH_POS_CENTRE, ATTACH_POS_CENTRE, true, 0.0, CONN_DIR_ALL);
    ShapeConnectionPin::new_shape(r, s2, 1, ATTACH_POS_CENTRE, ATTACH_POS_CENTRE, true, 0.0, CONN_DIR_ALL);
    let j = JunctionRef::new(r, pt(125.0, 150.0), 0);
    let c1 = conn(r, cfg, &ConnEnd::from_shape_pin(r, s1, 1), &ConnEnd::from_junction(r, j));
    let c2 = conn(r, cfg, &ConnEnd::from_junction(r, j), &ConnEnd::from_shape_pin(r, s2, 1));
    let c3 = conn(r, cfg, &ConnEnd::from_shape_pin(r, s1, 1), &ConnEnd::from_point(pt(125.0, -100.0)));
    (s1, s2, j, vec![c1, c2, c3])
}

#[derive(Clone, Copy, Debug)]
enum Obj {
    S1,
    S2,
    J,
    C(usize),
}

fn delete_in_order(r: &mut Router, cfg: Cfg, order: &[Obj], process_first: bool, process_between: bool) {
    let (s1, s2, j, cs) = scene(r, cfg);
    if process_first {
        r.process_transaction();
    }
    for o in order {
        match *o {
            Obj::S1 => r.delete_shape(s1),
            Obj::S2 => r.delete_shape(s2),
            Obj::J => r.delete_junction(j),
            Obj::C(i) => r.delete_connector(cs[i]),
        }
        if process_between {
            r.process_transaction();
        }
    }
    r.process_transaction();
}

fn permutations(items: &[Obj]) -> Vec<Vec<Obj>> {
    if items.len() <= 1 {
        return vec![items.to_vec()];
    }
    let mut out = Vec::new();
    for i in 0..items.len() {
        let mut rest = items.to_vec();
        let x = rest.remove(i);
        for mut p in permutations(&rest) {
            p.insert(0, x);
            out.push(p);
        }
    }
    out
}

#[test]
fn delete_objects_in_every_order() {
    let all = [Obj::S1, Obj::S2, Obj::J, Obj::C(0), Obj::C(1)];
    for order in permutations(&all) {
        for (pf, pb) in [(false, false), (true, false), (true, true)] {
            let o = order.clone();
            run(&format!("delete {:?} process_first={} between={}", order, pf, pb), move |r, cfg| {
                delete_in_order(r, cfg, &o, pf, pb);
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Handles used after their object was deleted.

robust!(double_delete, |r, cfg| {
    let (s1, _s2, j, cs) = scene(r, cfg);
    r.process_transaction();
    r.delete_shape(s1);
    r.delete_shape(s1);
    r.delete_junction(j);
    r.delete_junction(j);
    r.delete_connector(cs[2]);
    r.delete_connector(cs[2]);
    r.process_transaction();
    r.delete_shape(s1);
    r.delete_junction(j);
    r.delete_connector(cs[2]);
    r.process_transaction();
});

robust!(handles_after_delete, |r, cfg| {
    let (s1, s2, j, cs) = scene(r, cfg);
    r.process_transaction();
    r.delete_shape(s2);
    r.delete_junction(j);
    r.delete_connector(cs[0]);
    r.process_transaction();
    // Router-level operations on dead handles are ignored.
    r.move_shape_by(s2, 1.0, 1.0);
    r.move_shape(s2, &rect(0.0, 0.0, 1.0, 1.0), false);
    r.move_junction_by(j, 1.0, 1.0);
    r.move_junction(j, &pt(3.0, 3.0));
    r.delete_shape(s2);
    // Handle methods on dead handles are no-ops / return defaults.
    let c = cs[0];
    c.set_source_endpoint(r, &ConnEnd::from_point(pt(0.0, 0.0)));
    c.set_endpoints(r, &ConnEnd::from_point(pt(0.0, 0.0)), &ConnEnd::from_point(pt(9.0, 9.0)));
    c.set_routing_type(r, ConnType::ConnTypePolyLine);
    c.set_routing_checkpoints(r, &[Checkpoint::new(pt(1.0, 1.0))]);
    c.set_hate_crossings(r, true);
    c.set_fixed_existing_route(r);
    c.clear_fixed_route(r);
    let _ = c.display_route(r).ps.len();
    let _ = c.route(r).ps.len();
    let _ = c.id(r);
    let _ = c.split_at_segment(r, 1);
    let _ = s2.polygon(r).ps.len();
    let _ = s2.position(r);
    s2.set_centre_pos(r, &pt(5.0, 5.0));
    let _ = j.position(r);
    j.set_position_fixed(r, true);
    let _ = j.remove_junction_and_merge_connectors(r);
    // Endpoints referring to deleted objects.
    let _ = ConnEnd::from_shape_pin(r, s2, 1);
    let _ = ConnEnd::from_junction(r, j);
    cs[1].set_dest_endpoint(r, &ConnEnd::from_point(pt(300.0, 300.0)));
    r.process_transaction();
    let _ = s1;
});

robust!(endpoint_attached_to_shape_deleted_in_same_transaction, |r, cfg| {
    let s = ShapeRef::new(r, &rect(0.0, 0.0, 50.0, 50.0), 0);
    ShapeConnectionPin::new_shape(r, s, 1, ATTACH_POS_CENTRE, ATTACH_POS_CENTRE, true, 0.0, CONN_DIR_ALL);
    r.process_transaction();
    let ce = ConnEnd::from_shape_pin(r, s, 1);
    let c = conn(r, cfg, &ce, &ConnEnd::from_point(pt(200.0, 75.0)));
    r.delete_shape(s);
    r.process_transaction();
    // A ConnEnd copy referring to the deleted shape, set later.
    c.set_source_endpoint(r, &ce);
    r.process_transaction();
});

robust!(add_and_delete_in_same_transaction, |r, cfg| {
    let s = ShapeRef::new(r, &rect(0.0, 0.0, 50.0, 50.0), 0);
    r.delete_shape(s);
    let j = JunctionRef::new(r, pt(10.0, 10.0), 0);
    r.delete_junction(j);
    let s2 = ShapeRef::new(r, &rect(0.0, 0.0, 50.0, 50.0), 0);
    r.move_shape_by(s2, 10.0, 0.0);
    r.delete_shape(s2);
    r.move_shape_by(s2, 10.0, 0.0);
    let c = conn(r, cfg, &ConnEnd::from_point(pt(0.0, 0.0)), &ConnEnd::from_point(pt(1.0, 1.0)));
    r.delete_connector(c);
    r.process_transaction();
});

robust!(move_shape_and_junction_with_attached_connectors, |r, cfg| {
    let (s1, _s2, j, _cs) = scene(r, cfg);
    r.process_transaction();
    r.move_shape_by(s1, 10.0, 10.0);
    r.move_junction_by(j, -10.0, 5.0);
    r.process_transaction();
    r.move_shape(s1, &rect(0.0, 300.0, 50.0, 350.0), true);
    r.mark_all_obstacles_as_moved();
    r.process_transaction();
});

robust!(junction_with_both_ends_of_one_connector, |r, cfg| {
    let j = JunctionRef::new(r, pt(100.0, 100.0), 0);
    let ce = ConnEnd::from_junction(r, j);
    conn(r, cfg, &ce, &ce);
    r.process_transaction();
    let _ = j.remove_junction_and_merge_connectors(r);
    r.process_transaction();
});

robust!(split_at_segment_edge_cases, |r, cfg| {
    ShapeRef::new(r, &rect(50.0, 50.0, 100.0, 100.0), 0);
    let c = conn(r, cfg, &ConnEnd::from_point(pt(0.0, 75.0)), &ConnEnd::from_point(pt(200.0, 80.0)));
    r.process_transaction();
    let _ = c.split_at_segment(r, 0);
    let _ = c.split_at_segment(r, 1);
    let _ = c.split_at_segment(r, 1000);
    r.process_transaction();
});

robust!(fixed_routes, |r, cfg| {
    let c = conn(r, cfg, &ConnEnd::from_point(pt(0.0, 75.0)), &ConnEnd::from_point(pt(200.0, 80.0)));
    // Before any route exists.
    c.set_fixed_existing_route(r);
    r.process_transaction();
    let mut route = Polygon::new();
    c.set_fixed_route(r, &route);
    r.process_transaction();
    route.ps.push(pt(1.0, 1.0));
    c.set_fixed_route(r, &route);
    r.process_transaction();
    c.clear_fixed_route(r);
    c.clear_fixed_route(r);
    r.process_transaction();
});

robust!(clusters, |r, cfg| {
    r.clustered_routing = true;
    let cl = ClusterRef::new(r, &rect(0.0, 0.0, 300.0, 300.0), 0);
    let _ = ClusterRef::new(r, &Polygon::new(), 0);
    ShapeRef::new(r, &rect(50.0, 50.0, 100.0, 100.0), 0);
    conn(r, cfg, &ConnEnd::from_point(pt(10.0, 75.0)), &ConnEnd::from_point(pt(400.0, 75.0)));
    r.process_transaction();
    cl.set_new_poly(r, &rect(0.0, 0.0, 150.0, 150.0));
    r.process_transaction();
    r.delete_cluster(cl);
    r.delete_cluster(cl);
    r.process_transaction();
});

// ---------------------------------------------------------------------------
// Hyperedges.

robust!(hyperedge_with_too_few_terminals, |r, cfg| {
    let s = ShapeRef::new(r, &rect(50.0, 50.0, 100.0, 100.0), 0);
    ShapeConnectionPin::new_shape(r, s, 1, ATTACH_POS_CENTRE, ATTACH_POS_CENTRE, true, 0.0, CONN_DIR_ALL);
    r.hyperedge_rerouter().register_hyperedge_for_rerouting(Vec::new());
    let one = vec![ConnEnd::from_shape_pin(r, s, 1)];
    r.hyperedge_rerouter().register_hyperedge_for_rerouting(one);
    let j = JunctionRef::new(r, pt(200.0, 200.0), 0);
    r.hyperedge_rerouter().register_hyperedge_for_rerouting_junction(j);
    r.process_transaction();
    let _ = cfg;
});

robust!(hyperedge_with_coincident_terminals, |r, cfg| {
    let t = vec![
        ConnEnd::from_point(pt(10.0, 10.0)),
        ConnEnd::from_point(pt(10.0, 10.0)),
        ConnEnd::from_point(pt(10.0, 10.0)),
    ];
    r.hyperedge_rerouter().register_hyperedge_for_rerouting(t);
    r.process_transaction();
    let j = JunctionRef::new(r, pt(100.0, 100.0), 0);
    conn(r, cfg, &ConnEnd::from_junction(r, j), &ConnEnd::from_point(pt(100.0, 100.0)));
    conn(r, cfg, &ConnEnd::from_junction(r, j), &ConnEnd::from_point(pt(100.0, 100.0)));
    r.process_transaction();
    r.hyperedge_rerouter().register_hyperedge_for_rerouting_junction(j);
    r.process_transaction();
});

robust!(cyclic_registered_hyperedge, |r, cfg| {
    let j1 = JunctionRef::new(r, pt(100.0, 100.0), 0);
    let j2 = JunctionRef::new(r, pt(200.0, 100.0), 0);
    conn(r, cfg, &ConnEnd::from_junction(r, j1), &ConnEnd::from_junction(r, j2));
    conn(r, cfg, &ConnEnd::from_junction(r, j1), &ConnEnd::from_junction(r, j2));
    conn(r, cfg, &ConnEnd::from_junction(r, j1), &ConnEnd::from_point(pt(0.0, 0.0)));
    r.process_transaction();
    r.hyperedge_rerouter().register_hyperedge_for_rerouting_junction(j1);
    r.process_transaction();
});

robust!(hyperedge_improvement_options, |r, cfg| {
    r.set_routing_option(RoutingOption::ImproveHyperedgeRoutesMovingJunctions, true);
    r.set_routing_option(RoutingOption::ImproveHyperedgeRoutesMovingAddingAndDeletingJunctions, true);
    let (_s1, _s2, j, _cs) = scene(r, cfg);
    r.process_transaction();
    r.hyperedge_rerouter().register_hyperedge_for_rerouting_junction(j);
    r.process_transaction();
});

// ---------------------------------------------------------------------------
// Misc API.

robust!(router_destruct_then_reuse, |r, cfg| {
    let _ = scene(r, cfg);
    r.process_transaction();
    r.destruct();
    r.process_transaction();
    r.destruct();
});

robust!(checkpoints_degenerate, |r, cfg| {
    ShapeRef::new(r, &rect(50.0, 50.0, 100.0, 100.0), 0);
    let c = conn(r, cfg, &ConnEnd::from_point(pt(0.0, 75.0)), &ConnEnd::from_point(pt(200.0, 75.0)));
    // Inside a shape, on an endpoint, duplicated.
    c.set_routing_checkpoints(
        r,
        &[
            Checkpoint::new(pt(75.0, 75.0)),
            Checkpoint::new(pt(0.0, 75.0)),
            Checkpoint::new(pt(0.0, 75.0)),
            Checkpoint::new(pt(200.0, 75.0)),
        ],
    );
    r.process_transaction();
    c.set_routing_checkpoints(r, &[]);
    r.process_transaction();
});

robust!(rubber_band_routing, |r, cfg| {
    if cfg.flags != POLY_LINE_ROUTING {
        return;
    }
    r.rubber_band_routing = true;
    r.ignore_regions = true;
    ShapeRef::new(r, &rect(50.0, 50.0, 100.0, 100.0), 0);
    let c = conn(r, cfg, &ConnEnd::from_point(pt(0.0, 75.0)), &ConnEnd::from_point(pt(200.0, 80.0)));
    r.process_transaction();
    for k in 0..10 {
        c.set_dest_endpoint(r, &ConnEnd::from_point(pt(200.0 + k as f64 * 7.0, 80.0 - k as f64 * 13.0)));
        r.process_transaction();
    }
});

robust!(junction_pin_and_centre_class, |r, cfg| {
    let j = JunctionRef::new(r, pt(100.0, 100.0), 0);
    ShapeConnectionPin::new_junction(r, j, CONNECTIONPIN_CENTRE, CONN_DIR_ALL);
    conn(r, cfg, &ConnEnd::from_junction(r, j), &ConnEnd::from_point(pt(0.0, 0.0)));
    r.process_transaction();
    j.prefer_orthogonal_dimension(r, 0);
    j.prefer_orthogonal_dimension(r, 1);
    r.process_transaction();
});
