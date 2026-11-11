//! Differential test against the C++ oracle `tools/oracle/objects_oracle.cpp`
//! (golden output in `tests/golden/objects_oracle.txt`).
//!
//! Replays the same object-layer / router-state scenarios (no routing) and
//! must reproduce the oracle's output byte for byte.

#![allow(clippy::uninlined_format_args)]

use libavoid_rust::avoid::actioninfo::ActionType;
use libavoid_rust::avoid::arena::{ConnIdx, ObstacleIdx, PinIdx, VertIdx};
use libavoid_rust::avoid::connectionpin::*;
use libavoid_rust::avoid::connector::{Checkpoint, ConnRef, ConnType};
use libavoid_rust::avoid::connend::*;
use libavoid_rust::avoid::geomtypes::{Point, Polygon, Rectangle, XDIM, YDIM};
use libavoid_rust::avoid::junction::JunctionRef;
use libavoid_rust::avoid::obstacle::Obstacle;
use libavoid_rust::avoid::router::*;
use libavoid_rust::avoid::shape::{ShapeRef, ShapeTransformationType};
use libavoid_rust::avoid::vertices::{ContainsMap, VertID};
use libavoid_rust::avoid::viscluster::ClusterRef;

use std::fmt::Write as _;

fn g17(v: f64) -> String {
    if v.is_nan() {
        return "nan".into();
    }
    format_g(v, 17)
}

fn pt(p: &Point) -> String {
    format!("({},{})", g17(p.x), g17(p.y))
}

fn pt_full(p: &Point) -> String {
    format!("({},{},{},{})", g17(p.x), g17(p.y), p.id, p.vn)
}

fn poly(p: &Polygon) -> String {
    let v: Vec<String> = p.ps.iter().map(pt_full).collect();
    format!("[{}]", v.join(" "))
}

fn vert(r: &Router, v: Option<VertIdx>) -> String {
    match v {
        None => "-".into(),
        Some(v) => {
            let v = &r.vert_infs[v];
            format!(
                "[{},{},p={}] {} vis={}",
                v.id.obj_id,
                v.id.vn,
                v.id.props,
                pt_full(&v.point),
                v.vis_directions
            )
        }
    }
}

fn pin_name(r: &Router, pin: Option<PinIdx>) -> String {
    match pin {
        None => "-".into(),
        Some(p) => {
            let pin = &r.pins[p];
            format!("{}:{}", pin.containing_object_id(&r.obstacles), pin.m_class_id)
        }
    }
}

fn conn_end(r: &Router, ce: &ConnEnd) -> String {
    let anchor = match ce.m_anchor_obj {
        Some(a) => format!("{}", r.obstacles[a].m_id),
        None => "-".into(),
    };
    let conn = match ce.m_conn_ref {
        Some(c) => format!("{}", r.conns[c].m_id),
        None => "-".into(),
    };
    format!(
        "type={} dirs={}/{} class={} anchor={} conn={} pin={} pt={} pos={}",
        ce.m_type as i32,
        ce.m_directions,
        ce.directions(r),
        ce.m_connection_pin_class_id,
        anchor,
        conn,
        pin_name(r, ce.m_active_pin),
        pt(&ce.m_point),
        pt(&ce.position(r))
    )
}

struct Out(String);

impl Out {
    fn line(&mut self, s: &str) {
        self.0.push_str(s);
        self.0.push('\n');
    }

    fn dump_pin(&mut self, r: &Router, p: PinIdx) {
        let pin = &r.pins[p];
        let s = format!(
            "    pin {} x={} y={} inside={} prop={} visdirs={} dirs={} excl={} cost={} users={} pos={} v={}",
            pin_name(r, Some(p)),
            g17(pin.m_x_offset),
            g17(pin.m_y_offset),
            g17(pin.m_inside_offset),
            pin.m_using_proportional_offsets as i32,
            pin.m_visibility_directions,
            pin.directions(),
            pin.m_exclusive as i32,
            g17(pin.m_connection_cost),
            pin.m_connend_users.len(),
            pt_full(&pin.position(r, None)),
            vert(r, pin.m_vertex)
        );
        self.line(&s);
    }

    fn dump_obstacle(&mut self, r: &Router, o: ObstacleIdx) {
        let ob = &r.obstacles[o];
        let s = format!(
            "  obstacle id={} kind={} active={} poly={} pos={}",
            ob.m_id,
            if ob.is_shape() { "shape" } else { "junction" },
            ob.is_active() as i32,
            poly(&ob.m_polygon),
            pt(&Obstacle::position(r, o))
        );
        self.line(&s);
        if let Some(j) = Obstacle::as_junction(r, o) {
            let s = format!(
                "    junction pos={} rec={} fixed={}",
                pt(&j.position(r)),
                pt(&j.recommended_position(r)),
                j.position_fixed(r) as i32
            );
            self.line(&s);
        }
        let first = ob.first_vert().unwrap();
        let mut v = first;
        loop {
            let s = format!("    ring {}", vert(r, Some(v)));
            self.line(&s);
            v = r.vert_infs[v].sh_next.unwrap();
            if v == first {
                break;
            }
        }
        let s = format!("    lastVert {}", vert(r, ob.last_vert()));
        self.line(&s);
        for &p in ob.m_connection_pins.iter() {
            self.dump_pin(r, p);
        }
        // (Pointer-ordered upstream: printed canonically sorted.)
        let mut following: Vec<(u32, u32)> = ob
            .m_following_conns
            .iter()
            .map(|k| {
                let ce = k.idx.get(r);
                (r.conns[ce.m_conn_ref.unwrap()].m_id, ConnEnd::endpoint_type(r, k.idx))
            })
            .collect();
        following.sort();
        following.dedup();
        let mut s = String::from("    following");
        for (c, t) in following {
            let _ = write!(s, " {}/{}", c, t);
        }
        self.line(&s);
        let mut attached: Vec<u32> = Obstacle::attached_connectors(r, o).iter().map(|&c| r.conns[c].m_id).collect();
        attached.sort();
        let mut s = String::from("    attached");
        for c in attached {
            let _ = write!(s, " {}", c);
        }
        self.line(&s);
    }

    fn dump_conn(&mut self, r: &Router, c: ConnIdx) {
        let cr = &r.conns[c];
        let flag = r.m_conn_reroute_flags.m_mapping[cr.m_reroute_flag_ptr.unwrap().0 as usize].1;
        let s = format!(
            "  conn id={} type={} active={} reroute={} false={} repaint={} fixed={} hate={} flag={} dist={}",
            cr.m_id,
            cr.m_type as i32,
            cr.m_active as i32,
            cr.m_needs_reroute_flag as i32,
            cr.m_false_path as i32,
            cr.m_needs_repaint as i32,
            cr.m_has_fixed_route as i32,
            cr.m_hate_crossings as i32,
            flag as i32,
            g17(cr.m_route_dist)
        );
        self.line(&s);
        self.line(&format!("    src {}", vert(r, cr.m_src_vert)));
        self.line(&format!("    dst {}", vert(r, cr.m_dst_vert)));
        let se = match &cr.m_src_connend {
            Some(ce) => conn_end(r, ce),
            None => "-".into(),
        };
        self.line(&format!("    srcEnd {}", se));
        let de = match &cr.m_dst_connend {
            Some(ce) => conn_end(r, ce),
            None => "-".into(),
        };
        self.line(&format!("    dstEnd {}", de));
        let ends = cr.endpoint_conn_ends(r);
        self.line(&format!("    endpoints {} | {}", conn_end(r, &ends.0), conn_end(r, &ends.1)));
        for &v in cr.m_checkpoint_vertices.iter() {
            self.line(&format!("    checkpoint {}", vert(r, Some(v))));
        }
        self.line(&format!("    route={} display={}", poly(&cr.m_route), poly(&cr.m_display_route)));
        let mut s = String::from("    dstPinPoints");
        for p in cr.possible_dst_pin_points(r) {
            let _ = write!(s, " {}", pt(&p));
        }
        self.line(&s);
    }

    fn dump_actions(&mut self, r: &Router) {
        self.line(&format!("  actions {}", r.action_list.len()));
        for a in r.action_list.iter() {
            let mut s = format!("    action type={}", a.type_ as i32);
            match a.type_ {
                ActionType::ConnChange => {
                    let _ = write!(s, " conn={}", r.conns[a.conn()].m_id);
                }
                ActionType::ConnectionPinChange => {
                    let p = match a.obj_ptr {
                        libavoid_rust::avoid::actioninfo::ActionObj::Pin(p) => p,
                        _ => unreachable!(),
                    };
                    let _ = write!(s, " pin={}", pin_name(r, Some(p)));
                }
                _ => {
                    let _ = write!(s, " obstacle={}", r.obstacles[a.obstacle()].m_id);
                }
            }
            if a.type_ == ActionType::ShapeMove {
                let _ = write!(s, " first={} poly={}", a.first_move as i32, poly(&a.new_poly));
            }
            if a.type_ == ActionType::JunctionMove {
                let _ = write!(s, " newpos={}", pt(&a.new_position));
            }
            self.line(&s);
            for (t, ce) in a.conns.iter() {
                self.line(&format!("      end {} {}", t, conn_end(r, ce)));
            }
        }
    }

    fn dump_contains_map(&mut self, name: &str, m: &ContainsMap) {
        let mut s = format!("  {}", name);
        for (k, v) in m.iter() {
            let _ = write!(s, " [{},{}]{{", k.obj_id, k.vn);
            let items: Vec<String> = v.iter().map(|x| x.to_string()).collect();
            s.push_str(&items.join(","));
            s.push('}');
        }
        self.line(&s);
    }

    fn dump(&mut self, r: &mut Router, label: &str) {
        self.line(&format!("== {}", label));
        self.line(&format!(
            "  largest={} next={} settings={} static={} consolidate={}",
            r.m_largest_assigned_id,
            r.new_object_id(),
            r.m_settings_changes as i32,
            r.m_static_orthogonal_graph_invalidated as i32,
            r.m_consolidate_actions as i32
        ));
        for &o in r.m_obstacles.clone().iter() {
            self.dump_obstacle(r, o);
        }
        for &c in r.conn_refs.clone().iter() {
            self.dump_conn(r, c);
        }
        for &cl in r.cluster_refs.clone().iter() {
            let c = &r.clusters[cl];
            let mut s = format!(
                "  cluster id={} active={} rect={} n={}",
                c.m_id,
                c.m_active as i32,
                poly(&c.m_rectangular_polygon),
                c.m_polygon.size()
            );
            for i in 0..c.m_polygon.size() {
                let _ = write!(s, " {}", pt(&c.m_polygon.at(i, &*r)));
            }
            self.line(&s);
        }
        self.line(&format!(
            "  vertices conns={} shapes={}",
            r.vertices.conns_size(),
            r.vertices.shapes_size()
        ));
        let mut v = r.vertices.conns_begin();
        while let Some(vi) = v {
            self.line(&format!("    {}", vert(r, Some(vi))));
            v = r.vert_infs[vi].lst_next;
        }
        self.dump_contains_map("contains", &r.contains);
        self.dump_contains_map("clusters", &r.enclosing_clusters);
        self.dump_actions(r);
    }
}

fn rect(x1: f64, y1: f64, x2: f64, y2: f64) -> Polygon {
    Rectangle::new(&Point::new(x1, y1), &Point::new(x2, y2))
}

fn int_list(name: &str, l: &[u32]) -> String {
    let mut s = format!("  {}", name);
    for x in l {
        let _ = write!(s, " {}", x);
    }
    s
}

fn scenario_defaults(o: &mut Out) {
    o.line("#### defaults");
    for flags in 1..=3u32 {
        let mut r = Router::new(flags);
        let mut s = format!(
            "flags={} poly={} orth={}",
            flags, r.m_allows_polyline_routing as i32, r.m_allows_orthogonal_routing as i32
        );
        for t in [ConnType::ConnTypeNone, ConnType::ConnTypePolyLine, ConnType::ConnTypeOrthogonal] {
            let _ = write!(s, " valid({})={}", t as i32, r.valid_conn_type(t) as i32);
        }
        let _ = write!(s, " transactions={}", r.transaction_use() as i32);
        o.line(&s);
        let mut s = String::from(" params");
        for p in RoutingParameter::ALL {
            let _ = write!(s, " {}", g17(r.routing_parameter(p)));
        }
        o.line(&s);
        let mut s = String::from(" options");
        for op in RoutingOption::ALL {
            let _ = write!(s, " {}", r.routing_option(op) as i32);
        }
        o.line(&s);
        o.line(&format!(
            " flagsRouter partial={} simple={} clustered={} ignore={} lees={} invis={} selective={} feedback={} rubber={}",
            r.partial_time as i32,
            r.simple_routing as i32,
            r.clustered_routing as i32,
            r.ignore_regions as i32,
            r.use_lees_algorithm as i32,
            r.invisibility_grph as i32,
            r.selective_reroute as i32,
            r.partial_feedback as i32,
            r.rubber_band_routing as i32
        ));
        o.line(&format!(" emptyTransaction={}", r.process_transaction() as i32));
        r.destruct();
    }
    for v in [-1.0, -0.5, 0.0, 7.5, 1e9] {
        let mut r = Router::new(ORTHOGONAL_ROUTING);
        let mut s = format!("set {}:", g17(v));
        for p in RoutingParameter::ALL {
            r.m_settings_changes = false;
            r.set_routing_parameter(p, v);
            let _ = write!(s, " {}/{}", g17(r.routing_parameter(p)), r.m_settings_changes as i32);
        }
        r.set_routing_penalty(RoutingParameter::SegmentPenalty, CHOOSE_SENSIBLE_PARAM_VALUE);
        let _ = write!(s, " penaltyDefault={}", g17(r.routing_parameter(RoutingParameter::SegmentPenalty)));
        r.m_settings_changes = false;
        r.set_routing_option(RoutingOption::NudgeSharedPathsWithCommonEndPoint, false);
        let _ = write!(
            s,
            " opt={}/{}",
            r.routing_option(RoutingOption::NudgeSharedPathsWithCommonEndPoint) as i32,
            r.m_settings_changes as i32
        );
        o.line(&s);
    }
}

fn scenario_ids(o: &mut Out) {
    o.line("#### ids");
    let mut r = Router::new(ORTHOGONAL_ROUTING);
    let p = rect(0.0, 0.0, 10.0, 10.0);
    let s1 = ShapeRef::new(&mut r, &p, 0);
    let s2 = ShapeRef::new(&mut r, &p, 10);
    let c1 = ConnRef::new(&mut r, 0);
    let j1 = JunctionRef::new(&mut r, Point::new(5.0, 5.0), 5);
    let c2 = ConnRef::new(&mut r, 0);
    let cp = rect(-5.0, -5.0, 50.0, 50.0);
    let cl = ClusterRef::new(&mut r, &cp, 0);
    let s3 = ShapeRef::new(&mut r, &p, 3);
    let c3 = ConnRef::new(&mut r, 100);
    let j2 = JunctionRef::new(&mut r, Point::new(1.0, 1.0), 0);
    o.line(&format!(
        "ids {} {} {} {} {} {} {} {} {} next={}",
        s1.id(&r),
        s2.id(&r),
        c1.id(&r),
        j1.id(&r),
        c2.id(&r),
        cl.id(&r),
        s3.id(&r),
        c3.id(&r),
        j2.id(&r),
        r.new_object_id()
    ));
    let ids = [1u32, 2, 3, 5, 10, 11, 12, 13, 100, 101, 102];
    let mut s = String::from("unused");
    for id in ids {
        let _ = write!(s, " {}:{}", id, r.object_id_is_unused(id) as i32);
    }
    o.line(&s);
    o.dump(&mut r, "ids-queued");
    r.process_actions();
    let mut s = String::from("unused-after");
    for id in ids {
        let _ = write!(s, " {}:{}", id, r.object_id_is_unused(id) as i32);
    }
    o.line(&s);
    o.dump(&mut r, "ids-processed");
    r.destruct();
}

fn scenario_actions(o: &mut Out) {
    o.line("#### actions");
    let mut r = Router::new(ORTHOGONAL_ROUTING);
    let p1 = rect(0.0, 0.0, 10.0, 10.0);
    let s1 = ShapeRef::new(&mut r, &p1, 1);
    let p2 = rect(20.0, 0.0, 30.0, 10.0);
    let s2 = ShapeRef::new(&mut r, &p2, 2);
    o.dump(&mut r, "two-adds");
    r.move_shape_by(s1, 1.0, 2.0);
    let p1b = rect(100.0, 100.0, 120.0, 130.0);
    r.move_shape(s2, &p1b, true);
    o.dump(&mut r, "moves-while-add-queued");
    r.process_actions();
    o.dump(&mut r, "processed-adds");

    r.move_shape_by(s1, 5.0, 5.0);
    r.move_shape_by(s1, 1.0, 1.0);
    let p3 = rect(40.0, 40.0, 50.0, 60.0);
    r.move_shape(s2, &p3, true);
    let p4 = rect(41.0, 41.0, 51.0, 61.0);
    r.move_shape(s2, &p4, false);
    o.dump(&mut r, "queued-moves");

    let j = JunctionRef::new(&mut r, Point::new(70.0, 70.0), 7);
    r.move_junction_by(j, 1.0, 1.0);
    r.move_junction(j, &Point::new(80.0, 80.0));
    o.dump(&mut r, "junction-add-moved");
    r.process_actions();
    o.dump(&mut r, "processed-moves");

    r.move_junction_by(j, 2.0, 3.0);
    r.move_junction_by(j, 1.0, 1.0);
    r.delete_shape(s1);
    r.move_shape_by(s2, 3.0, 3.0);
    o.dump(&mut r, "move-then-delete");
    r.process_actions();
    o.dump(&mut r, "processed-delete");

    r.mark_all_obstacles_as_moved();
    o.dump(&mut r, "all-moved");
    r.process_actions();
    o.dump(&mut r, "processed-all-moved");

    r.delete_junction(j);
    r.delete_shape(s2);
    o.dump(&mut r, "delete-all");
    r.process_actions();
    o.dump(&mut r, "processed-delete-all");
    r.destruct();
}

fn scenario_pins(o: &mut Out) {
    o.line("#### pins");
    let mut r = Router::new(ORTHOGONAL_ROUTING);
    let p = rect(10.0, 20.0, 50.0, 80.0);
    let s = ShapeRef::new(&mut r, &p, 1);
    let r_ = &mut r;
    let pins: Vec<PinIdx> = vec![
        ShapeConnectionPin::new_shape(r_, s, 1, ATTACH_POS_LEFT, ATTACH_POS_TOP, true, 0.0, CONN_DIR_NONE),
        ShapeConnectionPin::new_shape(r_, s, 1, ATTACH_POS_RIGHT, ATTACH_POS_BOTTOM, true, 2.0, CONN_DIR_NONE),
        ShapeConnectionPin::new_shape(r_, s, 1, ATTACH_POS_CENTRE, ATTACH_POS_CENTRE, true, 0.0, CONN_DIR_NONE),
        ShapeConnectionPin::new_shape(r_, s, 2, 0.25, 0.75, true, 1.0, CONN_DIR_UP | CONN_DIR_LEFT),
        ShapeConnectionPin::new_shape(r_, s, 2, 5.0, 10.0, false, 0.0, CONN_DIR_DOWN),
        ShapeConnectionPin::new_shape(r_, s, 2, ATTACH_POS_MIN_OFFSET, ATTACH_POS_MAX_OFFSET, false, 1.5, CONN_DIR_NONE),
        ShapeConnectionPin::new_shape(r_, s, 3, 40.0, 60.0, false, 0.0, CONN_DIR_RIGHT),
        ShapeConnectionPin::new_shape_proportional(r_, s, 3, 0.5, 0.0, 0.0, CONN_DIR_ALL),
        ShapeConnectionPin::new_shape(r_, s, 1, ATTACH_POS_LEFT, ATTACH_POS_TOP, true, 0.0, CONN_DIR_NONE),
        ShapeConnectionPin::new_shape(r_, s, 4, 1.5, -0.5, true, 0.0, CONN_DIR_NONE),
        ShapeConnectionPin::new_shape(r_, s, 4, 100.0, 100.0, false, 0.0, CONN_DIR_NONE),
        ShapeConnectionPin::new_shape(r_, s, 2, 0.25, 0.75, true, 1.0, CONN_DIR_UP),
    ];
    pins[2].set_connection_cost(&mut r, 3.5);
    pins[3].set_exclusive(&mut r, false);
    pins[7].set_exclusive(&mut r, true);
    o.line(&format!("set size={}", r.obstacles[s.0].m_connection_pins.len()));
    for (i, &pin) in pins.iter().enumerate() {
        let ids = pin.ids(&r);
        o.line(&format!(
            "pin{} excl={} ids={},{} dirs={} pos={}",
            i,
            pin.is_exclusive(&r) as i32,
            ids.0,
            ids.1,
            pin.directions(&r),
            pt_full(&pin.position(&r))
        ));
    }
    let mut s_lt = String::from("lt");
    let mut s_eq = String::from("eq");
    for &a in pins.iter() {
        for &b in pins.iter() {
            let _ = write!(s_lt, "{}", r.pins[a].less_than(&r.pins[b], &r.obstacles) as i32);
            let _ = write!(s_eq, "{}", r.pins[a].equals(&r.pins[b], &r.obstacles) as i32);
        }
        s_lt.push(' ');
        s_eq.push(' ');
    }
    o.line(&s_lt);
    o.line(&s_eq);
    o.dump(&mut r, "pins-created");
    r.process_actions();
    o.dump(&mut r, "pins-processed");

    r.move_shape_by(s, 5.0, -5.0);
    r.process_actions();
    o.dump(&mut r, "pins-after-move");

    let transforms = [
        ShapeTransformationType::TransformationTypeCW90,
        ShapeTransformationType::TransformationTypeCW180,
        ShapeTransformationType::TransformationTypeCW270,
        ShapeTransformationType::TransformationTypeFlipX,
        ShapeTransformationType::TransformationTypeFlipY,
    ];
    for (t, &tr) in transforms.iter().enumerate() {
        let tf = t as f64;
        let tp = rect(0.0, 0.0, 40.0 + tf, 20.0 + 2.0 * tf);
        let ts = ShapeRef::new(&mut r, &tp, 50 + t as u32);
        let r_ = &mut r;
        ShapeConnectionPin::new_shape(r_, ts, 1, ATTACH_POS_LEFT, ATTACH_POS_TOP, true, 0.0, CONN_DIR_NONE);
        ShapeConnectionPin::new_shape(r_, ts, 2, 0.25, 0.6, true, 1.0, CONN_DIR_UP | CONN_DIR_RIGHT);
        ShapeConnectionPin::new_shape(r_, ts, 3, 10.0, 5.0, false, 0.0, CONN_DIR_LEFT);
        ShapeConnectionPin::new_shape(
            r_,
            ts,
            4,
            ATTACH_POS_MIN_OFFSET,
            ATTACH_POS_MAX_OFFSET,
            false,
            0.0,
            CONN_DIR_DOWN | CONN_DIR_LEFT,
        );
        ShapeConnectionPin::new_shape(r_, ts, 5, 0.0, 7.0, false, 0.0, CONN_DIR_ALL);
        ShapeConnectionPin::new_shape(r_, ts, 6, 0.5, 1.0, true, 0.0, CONN_DIR_NONE);
        r.process_actions();
        ts.transform_connection_pin_positions(&mut r, tr);
        o.line(&format!("transform {}", tr as i32));
        o.dump_obstacle(&r, ts.0);
        o.dump_actions(&r);
        ts.transform_connection_pin_positions(&mut r, tr);
        o.line(&format!("transform-again {}", tr as i32));
        o.dump_obstacle(&r, ts.0);
        r.process_actions();
    }
    r.destruct();
}

fn scenario_junctions(o: &mut Out) {
    o.line("#### junctions");
    let mut r = Router::new(ORTHOGONAL_ROUTING);
    r.set_routing_parameter(RoutingParameter::IdealNudgingDistance, 0.25);
    let j1 = JunctionRef::new(&mut r, Point::new(10.0, 10.0), 1);
    r.set_routing_parameter(RoutingParameter::IdealNudgingDistance, 4.0);
    let j2 = JunctionRef::new(&mut r, Point::new(20.0, 30.0), 2);
    ShapeConnectionPin::new_junction(&mut r, j2, 5, CONN_DIR_LEFT);
    ShapeConnectionPin::new_junction(&mut r, j2, 6, CONN_DIR_UP | CONN_DIR_DOWN);
    ShapeConnectionPin::new_junction(&mut r, j2, 7, CONN_DIR_NONE);
    let j3 = JunctionRef::new(&mut r, Point::new(-5.0, 0.0), 3);
    ShapeConnectionPin::new_junction(&mut r, j3, 5, CONN_DIR_RIGHT);
    ShapeConnectionPin::new_junction(&mut r, j3, 6, CONN_DIR_UP);
    j1.set_position_fixed(&mut r, true);
    o.dump(&mut r, "junctions-created");
    j2.prefer_orthogonal_dimension(&mut r, XDIM);
    j3.prefer_orthogonal_dimension(&mut r, YDIM);
    r.process_actions();
    o.dump(&mut r, "junctions-preferred");
    j1.set_recommended_position(&mut r, &Point::new(11.0, 12.0));
    r.move_junction(j2, &Point::new(25.0, 35.0));
    r.process_actions();
    o.dump(&mut r, "junctions-moved");
    r.destruct();
}

fn scenario_connectors(o: &mut Out) {
    o.line("#### connectors");
    let mut r = Router::new(ORTHOGONAL_ROUTING);
    let pa = rect(0.0, 0.0, 20.0, 20.0);
    let a = ShapeRef::new(&mut r, &pa, 1);
    let pb = rect(100.0, 0.0, 120.0, 20.0);
    let b = ShapeRef::new(&mut r, &pb, 2);
    ShapeConnectionPin::new_shape(&mut r, a, 1, ATTACH_POS_RIGHT, ATTACH_POS_CENTRE, true, 0.0, CONN_DIR_RIGHT);
    ShapeConnectionPin::new_shape(&mut r, a, 1, ATTACH_POS_CENTRE, ATTACH_POS_BOTTOM, true, 0.0, CONN_DIR_DOWN);
    ShapeConnectionPin::new_shape(&mut r, b, 1, ATTACH_POS_LEFT, ATTACH_POS_CENTRE, true, 0.0, CONN_DIR_LEFT);
    ShapeConnectionPin::new_shape(&mut r, b, 2, ATTACH_POS_CENTRE, ATTACH_POS_CENTRE, true, 0.0, CONN_DIR_ALL);
    let j = JunctionRef::new(&mut r, Point::new(60.0, 50.0), 3);

    let c1 = ConnRef::new(&mut r, 10);
    c1.set_source_endpoint(&mut r, &ConnEnd::from_point(Point::new(-10.0, -10.0)));
    let e = ConnEnd::from_shape_pin(&r, a, 1);
    c1.set_source_endpoint(&mut r, &e);
    let e = ConnEnd::from_shape_pin(&r, b, 1);
    c1.set_dest_endpoint(&mut r, &e);
    let e1 = ConnEnd::from_point_dirs(Point::new(50.0, 100.0), CONN_DIR_UP);
    let e2 = ConnEnd::from_junction(&r, j);
    let c2 = ConnRef::new_with_ends(&mut r, &e1, &e2, 11);
    let e1 = ConnEnd::from_junction(&r, j);
    let e2 = ConnEnd::from_shape_pin(&r, b, 2);
    let c3 = ConnRef::new_with_ends(&mut r, &e1, &e2, 12);
    let c4 = ConnRef::new(&mut r, 13);
    c4.set_endpoints(
        &mut r,
        &ConnEnd::from_point(Point::new(1.0, 2.0)),
        &ConnEnd::from_point_dirs(Point::new(3.0, 4.0), CONN_DIR_LEFT | CONN_DIR_DOWN),
    );
    c4.set_routing_type(&mut r, ConnType::ConnTypePolyLine);
    c4.set_routing_type(&mut r, ConnType::ConnTypeOrthogonal);
    c4.set_hate_crossings(&mut r, true);
    let cps = vec![
        Checkpoint::new(Point::new(5.0, 5.0)),
        Checkpoint::with_directions(Point::new(6.0, 7.0), CONN_DIR_UP, CONN_DIR_DOWN),
    ];
    c4.set_routing_checkpoints(&mut r, &cps);
    o.dump(&mut r, "conns-queued");
    let src = VertID::SRC as u32;
    let tar = VertID::TAR as u32;
    r.modify_connector_end(c1, src, &ConnEnd::from_point(Point::new(7.0, 7.0)), true);
    r.modify_connector_end(c1, tar, &ConnEnd::from_point(Point::new(8.0, 8.0)), false);
    let e = ConnEnd::from_shape_pin(&r, b, 1);
    r.modify_connector_end(c1, tar, &e, false);
    r.modify_connector(c3);
    o.dump_actions(&r);
    r.process_actions();
    o.dump(&mut r, "conns-processed");

    for id in 1..=3u32 {
        for ty in 1..=3u32 {
            let mut l = Vec::new();
            r.attached_conns(&mut l, id, ty);
            o.line(&format!("attachedConns({},{}){}", id, ty, int_list("", &l)));
            let mut l = Vec::new();
            r.attached_shapes(&mut l, id, ty);
            o.line(&format!("attachedShapes({},{}){}", id, ty, int_list("", &l)));
        }
    }
    let mut s = String::from("possible a/1:");
    for p in Obstacle::possible_pin_points(&r, a.0, 1) {
        let _ = write!(s, " {}", pt(&p));
    }
    o.line(&s);

    r.move_shape_by(a, 0.0, 5.0);
    r.move_junction_by(j, 5.0, 0.0);
    o.dump_actions(&r);
    r.process_actions();
    o.dump(&mut r, "after-attached-move");

    r.delete_shape(b);
    r.process_actions();
    o.dump(&mut r, "after-shape-delete");

    let mut route = Polygon::new();
    route.ps.push(Point::new(0.0, 50.0));
    route.ps.push(Point::new(30.0, 50.0));
    route.ps.push(Point::new(60.0, 50.0));
    route.ps.push(Point::new(60.0, 80.0));
    c4.set_fixed_route(&mut r, &route);
    r.conns[c4].calc_route_dist();
    o.dump(&mut r, "fixed-route-queued");
    r.process_actions();
    r.conns[c4].calc_route_dist();
    o.dump(&mut r, "fixed-route-processed");
    c4.clear_fixed_route(&mut r);

    let mut r2 = Polygon::new();
    r2.ps.push(Point::new(20.0, 10.0));
    r2.ps.push(Point::new(40.0, 10.0));
    r2.ps.push(Point::new(40.0, 30.0));
    r2.ps.push(Point::new(80.0, 30.0));
    r.conns[c3].set_route(&r2);
    let split = c3.split_at_segment(&mut r, 2);
    o.line(&format!(
        "split junction={} conn={}",
        split.0.unwrap().id(&r),
        split.1.unwrap().id(&r)
    ));
    o.dump(&mut r, "split-queued");
    r.process_actions();
    o.dump(&mut r, "split-processed");

    r.delete_connector(c2);
    o.dump(&mut r, "deleted-c2");
    let merged = j.remove_junction_and_merge_connectors(&mut r);
    o.line(&format!("merged={}", merged.map(|m| r.conns[m].m_id as i32).unwrap_or(-1)));
    o.dump(&mut r, "merge-queued");
    r.process_actions();
    o.dump(&mut r, "merge-processed");
    r.destruct();
}

fn scenario_merge(o: &mut Out) {
    o.line("#### merge");
    let mut r = Router::new(ORTHOGONAL_ROUTING);
    let j = JunctionRef::new(&mut r, Point::new(50.0, 50.0), 1);
    let ps = rect(90.0, 40.0, 110.0, 60.0);
    let s = ShapeRef::new(&mut r, &ps, 4);
    ShapeConnectionPin::new_shape(&mut r, s, 1, ATTACH_POS_LEFT, ATTACH_POS_CENTRE, true, 0.0, CONN_DIR_LEFT);
    let pt6 = rect(-10.0, 40.0, 10.0, 60.0);
    let t = ShapeRef::new(&mut r, &pt6, 6);
    ShapeConnectionPin::new_shape(&mut r, t, 1, ATTACH_POS_RIGHT, ATTACH_POS_CENTRE, true, 0.0, CONN_DIR_RIGHT);
    let e1 = ConnEnd::from_shape_pin(&r, t, 1);
    let e2 = ConnEnd::from_junction(&r, j);
    ConnRef::new_with_ends(&mut r, &e1, &e2, 2);
    let e1 = ConnEnd::from_junction(&r, j);
    let e2 = ConnEnd::from_shape_pin(&r, s, 1);
    ConnRef::new_with_ends(&mut r, &e1, &e2, 3);
    let e1 = ConnEnd::from_junction(&r, j);
    let e2 = ConnEnd::from_point(Point::new(50.0, 0.0));
    let c3 = ConnRef::new_with_ends(&mut r, &e1, &e2, 5);
    r.process_actions();
    o.dump(&mut r, "merge-setup");
    let m3 = j.remove_junction_and_merge_connectors(&mut r);
    o.line(&format!("merged3={}", m3.is_some() as i32));
    r.delete_connector(c3);
    // (Which of c1/c2 survives depends on pointer order upstream; only
    // order-independent facts are compared.)
    let merged = j.remove_junction_and_merge_connectors(&mut r);
    o.line(&format!("merged={}", merged.is_some() as i32));
    r.process_actions();
    o.line(&format!(
        "after merge: conns={} obstacles={} verts={}+{}",
        r.conn_refs.len(),
        r.m_obstacles.len(),
        r.vertices.conns_size(),
        r.vertices.shapes_size()
    ));
    let ends = merged.unwrap().endpoint_conn_ends(&r);
    o.line(&format!(
        "  merged ends {}:{} -> {}:{}",
        r.obstacles[ends.0.m_anchor_obj.unwrap()].m_id,
        ends.0.m_connection_pin_class_id,
        r.obstacles[ends.1.m_anchor_obj.unwrap()].m_id,
        ends.1.m_connection_pin_class_id
    ));
    r.destruct();
}

fn scenario_clusters(o: &mut Out) {
    o.line("#### clusters");
    let mut r = Router::new(ORTHOGONAL_ROUTING);
    let pa = rect(0.0, 0.0, 20.0, 20.0);
    let a = ShapeRef::new(&mut r, &pa, 1);
    let c1 = ConnRef::new_with_ends(
        &mut r,
        &ConnEnd::from_point(Point::new(5.0, 5.0)),
        &ConnEnd::from_point(Point::new(50.0, 50.0)),
        2,
    );
    let c2 = ConnRef::new_with_ends(
        &mut r,
        &ConnEnd::from_point(Point::new(30.0, 30.0)),
        &ConnEnd::from_point(Point::new(-50.0, 5.0)),
        3,
    );
    r.process_actions();
    let mut cp = Polygon::new();
    let mut q1 = a.polygon(&r).ps[0];
    q1.id = 1;
    q1.vn = 0;
    cp.ps.push(q1);
    cp.ps.push(Point::new(60.0, 0.0));
    cp.ps.push(Point::new(60.0, 60.0));
    cp.ps.push(Point::new(0.0, 60.0));
    let cl1 = ClusterRef::new(&mut r, &cp, 10);
    let cp2 = rect(-60.0, -10.0, 10.0, 10.0);
    let cl2 = ClusterRef::new(&mut r, &cp2, 0);
    o.dump(&mut r, "clusters-added");
    let v = c1.src(&r).unwrap();
    r.generate_contains(v);
    let v = c2.dst(&r).unwrap();
    r.generate_contains(v);
    o.dump(&mut r, "contains-generated");
    let cp3 = rect(-100.0, -100.0, 100.0, 100.0);
    cl1.set_new_poly(&mut r, &cp3);
    o.line(&format!("setNewPoly rect={}", poly(cl1.rectangular_polygon(&r))));
    r.delete_cluster(cl2);
    o.dump(&mut r, "cluster-deleted");
    let f = |s: Option<ShapeRef>, r: &Router| s.map(|s| s.id(r) as i32).unwrap_or(-1);
    let hit = r.shape_containing_point(&Point::new(10.0, 10.0));
    let miss = r.shape_containing_point(&Point::new(25.0, 10.0));
    let border = r.shape_containing_point(&Point::new(20.0, 10.0));
    o.line(&format!("containing {} {} {}", f(hit, &r), f(miss, &r), f(border, &r)));
    r.delete_cluster(cl1);
    r.destruct();
}

fn scenario_polyline(o: &mut Out) {
    o.line("#### polyline-queue");
    let mut r = Router::new(POLY_LINE_ROUTING);
    let pa = rect(0.0, 0.0, 20.0, 20.0);
    let a = ShapeRef::new(&mut r, &pa, 4);
    let b = ShapeRef::new(&mut r, &pa, 0);
    r.move_shape_by(a, 1.0, 1.0);
    let pb = rect(5.0, 5.0, 6.0, 7.0);
    r.move_shape(b, &pb, false);
    let c = ConnRef::new(&mut r, 0);
    c.set_routing_type(&mut r, ConnType::ConnTypeOrthogonal);
    o.line(&format!("type={}", c.routing_type(&r) as i32));
    o.dump(&mut r, "polyline-queued");
    r.action_list.clear();
    r.delete_connector(c);
    o.dump(&mut r, "polyline-cleared");
}

fn scenario_diagram(o: &mut Out) {
    o.line("#### diagram");
    let mut r = Router::new(ORTHOGONAL_ROUTING);
    let pa = rect(0.5, 0.0, 20.25, 20.0);
    ShapeRef::new(&mut r, &pa, 1);
    let pb = rect(100.0, 1e-7, 123456789.0, 20.0);
    ShapeRef::new(&mut r, &pb, 2);
    JunctionRef::new(&mut r, Point::new(3.0, 4.0), 3);
    let p1 = ConnEnd::from_point(Point::new(1.0, 1.0));
    let p2 = ConnEnd::from_point(Point::new(2.0, 2.0));
    let c = ConnRef::new_with_ends(&mut r, &p1, &p2, 4);
    ConnRef::new_with_ends(&mut r, &p1, &p2, 5);
    r.process_actions();
    let mut route = Polygon::new();
    route.ps.push(Point::new(1.0, 1.0));
    route.ps.push(Point::new(1.0 / 3.0, 1.0));
    route.ps.push(Point::new(1.0 / 3.0, 2.0));
    route.ps.push(Point::new(2.0, 2.0));
    c.set_fixed_route(&mut r, &route);
    r.process_actions();
    let text = r.output_diagram_text_g();
    o.0.push_str(&text);
    o.line("#### end diagram");
    r.destruct();
}

fn generate(o: &mut Out) {
    scenario_defaults(o);
    scenario_ids(o);
    scenario_actions(o);
    scenario_pins(o);
    scenario_junctions(o);
    scenario_connectors(o);
    scenario_merge(o);
    scenario_clusters(o);
    scenario_polyline(o);
    scenario_diagram(o);
}

#[test]
fn objects_oracle_matches_golden() {
    let golden = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/objects_oracle.txt"))
        .expect("golden file");
    let mut o = Out(String::new());
    // Compare whatever was produced even if a scenario panics, so the first
    // divergence is reported rather than the panic.
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| generate(&mut o)));
    let ours = o.0;
    if ours != golden {
        let gl: Vec<&str> = golden.lines().collect();
        let ol: Vec<&str> = ours.lines().collect();
        for i in 0..gl.len().max(ol.len()) {
            let g = gl.get(i).copied().unwrap_or("<EOF>");
            let m = ol.get(i).copied().unwrap_or("<EOF>");
            if g != m && !(m == "<EOF>" && res.is_err()) {
                panic!(
                    "objects oracle mismatch at line {}:\n  golden: {}\n  ours:   {}",
                    i + 1,
                    g,
                    m
                );
            }
        }
        if res.is_ok() {
            panic!("objects oracle mismatch (trailing newline?)");
        }
    }
    if let Err(e) = res {
        std::panic::resume_unwind(e);
    }
}

fn scenario_routed(o: &mut Out) {
    use std::cell::RefCell;
    use std::rc::Rc;

    o.line("#### routed");
    let log = Rc::new(RefCell::new(String::new()));
    let cb = |id: u32, log: &Rc<RefCell<String>>| -> Box<dyn FnMut(ConnIdx)> {
        let log = Rc::clone(log);
        Box::new(move |_c: ConnIdx| {
            log.borrow_mut().push_str(&format!("  callback conn={}\n", id));
        })
    };
    let flush = |o: &mut Out, log: &Rc<RefCell<String>>| {
        o.0.push_str(&log.borrow());
        log.borrow_mut().clear();
    };
    let mut r = Router::new(ORTHOGONAL_ROUTING);
    r.set_routing_parameter(RoutingParameter::SegmentPenalty, 50.0);
    let pa = rect(0.0, 0.0, 40.0, 40.0);
    let a = ShapeRef::new(&mut r, &pa, 1);
    let pb = rect(200.0, 100.0, 240.0, 140.0);
    let b = ShapeRef::new(&mut r, &pb, 2);
    ShapeConnectionPin::new_shape(&mut r, a, 1, ATTACH_POS_RIGHT, ATTACH_POS_CENTRE, true, 0.0, CONN_DIR_RIGHT);
    ShapeConnectionPin::new_shape(&mut r, a, 1, ATTACH_POS_CENTRE, ATTACH_POS_BOTTOM, true, 0.0, CONN_DIR_DOWN);
    ShapeConnectionPin::new_shape(&mut r, b, 1, ATTACH_POS_LEFT, ATTACH_POS_CENTRE, true, 0.0, CONN_DIR_LEFT);
    ShapeConnectionPin::new_shape(&mut r, b, 1, ATTACH_POS_CENTRE, ATTACH_POS_TOP, true, 0.0, CONN_DIR_UP);
    let (ea, eb) = (ConnEnd::from_shape_pin(&r, a, 1), ConnEnd::from_shape_pin(&r, b, 1));
    let c1 = ConnRef::new_with_ends(&mut r, &ea, &eb, 10);
    c1.set_callback(&mut r, Some(cb(10, &log)));
    let c2 = ConnRef::new_with_ends(
        &mut r,
        &ConnEnd::from_point(Point::new(100.0, -50.0)),
        &ConnEnd::from_point(Point::new(100.0, 200.0)),
        11,
    );
    c2.set_callback(&mut r, Some(cb(11, &log)));
    let j = JunctionRef::new(&mut r, Point::new(120.0, 250.0), 3);
    let ej = ConnEnd::from_junction(&r, j);
    let c3 = ConnRef::new_with_ends(&mut r, &ea, &ej, 12);
    let c4 = ConnRef::new_with_ends(&mut r, &ej, &eb, 13);
    c3.set_callback(&mut r, Some(cb(12, &log)));
    c4.set_callback(&mut r, Some(cb(13, &log)));
    let mut callbacks = 0usize;
    let t = r.process_transaction();
    callbacks += r.m_callback_log.len();
    flush(o, &log);
    o.line(&format!("transaction={}", t as i32));
    o.line(&format!("callbacks={}", callbacks));
    o.dump(&mut r, "routed");
    r.move_shape_by(b, 0.0, 30.0);
    let t = r.process_transaction();
    callbacks += r.m_callback_log.len();
    flush(o, &log);
    o.line(&format!("transaction={}", t as i32));
    o.line(&format!("callbacks={}", callbacks));
    o.dump(&mut r, "routed-moved");
    o.line(&format!("transaction-empty={}", r.process_transaction() as i32));
    r.set_routing_parameter(RoutingParameter::CrossingPenalty, CHOOSE_SENSIBLE_PARAM_VALUE);
    let t = r.process_transaction();
    callbacks += r.m_callback_log.len();
    flush(o, &log);
    o.line(&format!("transaction-settings={}", t as i32));
    o.line(&format!("callbacks={}", callbacks));
    let line = format!(
        "overlap={} fixedOverlap={} touching={} crossings={}/{} invalid={}",
        r.exists_orthogonal_segment_overlap(true) as i32,
        r.exists_orthogonal_fixed_segment_overlap(true) as i32,
        r.exists_orthogonal_touching_paths() as i32,
        r.exists_crossings(false),
        r.exists_crossings(true),
        r.exists_invalid_orthogonal_paths() as i32
    );
    o.line(&line);
    o.dump(&mut r, "routed-crossing-penalty");
    let text = r.output_diagram_text_g();
    o.0.push_str(&text);
    r.destruct();
}

#[test]
fn objects_oracle_routed_matches_golden() {
    let golden =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/objects_oracle_routed.txt"))
            .expect("golden file");
    let mut o = Out(String::new());
    scenario_routed(&mut o);
    let ours = o.0;
    if ours != golden {
        let gl: Vec<&str> = golden.lines().collect();
        let ol: Vec<&str> = ours.lines().collect();
        for i in 0..gl.len().max(ol.len()) {
            let g = gl.get(i).copied().unwrap_or("<EOF>");
            let m = ol.get(i).copied().unwrap_or("<EOF>");
            assert_eq!(g, m, "routed oracle mismatch at line {}", i + 1);
        }
        panic!("routed oracle mismatch (trailing newline?)");
    }
}
