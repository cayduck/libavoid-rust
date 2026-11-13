//! Rust interpreter of the end-to-end fuzzer DSL (see the command reference
//! at the top of `tools/oracle/e2e_fuzz.cpp`).  Mirrors that driver's
//! interpreter, live-object bookkeeping (`sync`) and stage dumps exactly,
//! so that the output can be compared byte for byte with the C++.
//!
//! Live-object lists hold `(handle, creation serial)`: arena slots are
//! reused, so a handle alone could silently name a newer object.

#![allow(dead_code, clippy::uninlined_format_args)]

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use libavoid_rust::avoid::actioninfo::{ActionObj, ActionType};
use libavoid_rust::avoid::arena::{ArenaIndex, ClusterIdx, ConnIdx, ObstacleIdx, PinIdx};
use libavoid_rust::avoid::connectionpin::ShapeConnectionPin;
use libavoid_rust::avoid::connector::{Checkpoint, ConnRef, ConnType};
use libavoid_rust::avoid::connend::{ConnEnd, ConnEndType, CONN_DIR_ALL};
use libavoid_rust::avoid::geomtypes::{Point, Polygon, Rectangle};
use libavoid_rust::avoid::junction::JunctionRef;
use libavoid_rust::avoid::obstacle::Obstacle;
use libavoid_rust::avoid::router::*;
use libavoid_rust::avoid::shape::{ShapeRef, ShapeTransformationType};
use libavoid_rust::avoid::viscluster::ClusterRef;

pub fn g17(v: f64) -> String {
    if v.is_nan() {
        return "nan".into();
    }
    format_g(v, 17)
}

fn pt_str(p: &Point) -> String {
    format!("{},{}", g17(p.x), g17(p.y))
}

fn poly_str(p: &Polygon) -> String {
    let mut s = format!("{}:", p.ps.len());
    for q in p.ps.iter() {
        s.push(' ');
        s.push_str(&pt_str(q));
    }
    s
}

/// Also dump the SVG debug output after every transaction (the C++
/// driver's `E2E_SVG`; set by tests/e2e_svg_oracle.rs).
#[allow(dead_code)]
pub static SVG_OUTPUT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn fnv(lines: &[String]) -> u64 {
    let mut h: u64 = 1469598103934665603;
    for s in lines {
        for b in s.bytes().chain(std::iter::once(b'\n')) {
            h ^= b as u64;
            h = h.wrapping_mul(1099511628211);
        }
    }
    h
}

/// Whole-scene outputs up to this many lines are stored in full.
pub const FULL_SCENE_LIMIT: usize = 400;

/// (slot index, generation) of a handle: generation-checked handles name
/// one object forever, so this is a stable key.
fn hk<I: ArenaIndex>(h: I) -> (usize, u32) {
    (h.index(), h.generation())
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Conn((usize, u32)),
    Junc((usize, u32)),
}

struct Scene {
    router: Option<Router>,
    shapes: Vec<ShapeRef>,
    juncs: Vec<(JunctionRef, u64)>,
    conns: Vec<(ConnIdx, u64)>,
    clusters: Vec<ClusterIdx>,
    /// handle -> (serial, id) of the objects live at the previous sync.
    known: BTreeMap<Key, (u64, u32)>,
    /// e2e_reroute_count() at the previous sync.
    reroute_count: u64,
    /// "nowrap" seen (see `run_command`).
    no_wrap: bool,
    /// Raw mode (Rust only, `run_scene_raw`): every guard against upstream
    /// API-precondition violations is off, so the port itself meets the
    /// misuse (robustness check: no panic, bounded time).
    raw: bool,
    pending: Vec<String>,
    stage: usize,
    stages: Vec<Vec<String>>,
    callbacks: Rc<RefCell<Vec<u32>>>,
}

struct Toks<'a> {
    it: std::str::SplitWhitespace<'a>,
}

impl<'a> Toks<'a> {
    fn word(&mut self) -> &'a str {
        self.it.next().expect("missing token")
    }
    fn num(&mut self) -> f64 {
        let w = self.word();
        w.parse::<f64>().unwrap_or_else(|_| panic!("bad number {}", w))
    }
    fn unum(&mut self) -> u32 {
        let w = self.word();
        w.parse::<u32>().unwrap_or_else(|_| panic!("bad unsigned {}", w))
    }
    /// List reference (index mod n, or "L" = last); None if n == 0.
    fn reference(&mut self, n: usize) -> Option<usize> {
        let w = self.word();
        if n == 0 {
            return None;
        }
        if w == "L" {
            Some(n - 1)
        } else {
            Some((w.parse::<u64>().expect("bad reference") % n as u64) as usize)
        }
    }
    fn poly(&mut self) -> Polygon {
        let n = self.unum() as usize;
        let mut p = Polygon::with_size(n);
        for i in 0..n {
            let x = self.num();
            let y = self.num();
            p.ps[i] = Point::new(x, y);
        }
        p
    }
}

fn is_pending(r: &Router, ty: ActionType, o: ObstacleIdx) -> bool {
    r.action_list.iter().any(|a| a.type_ == ty && a.obj_ptr == ActionObj::Obstacle(o))
}

/// As `queuedConnEndUses()` in the C++ driver.
fn queued_conn_end_uses(r: &Router, o: ObstacleIdx) -> bool {
    r.action_list
        .iter()
        .any(|a| a.type_ == ActionType::ConnChange && a.conns.iter().any(|(_, ce)| ce.m_anchor_obj == Some(o)))
}

fn conn_live(r: &Router, c: ConnIdx, serial: u64) -> bool {
    // (a connector created in a transaction only joins connRefs when its
    // queued ConnChange is processed)
    r.conns.get(c).is_some_and(|x| x.serial == serial)
        && (r.conn_refs.contains(&c)
            || r.action_list.iter().any(|a| a.type_ == ActionType::ConnChange && a.obj_ptr == ActionObj::Conn(c)))
}

fn junction_live(r: &Router, j: JunctionRef, serial: u64) -> bool {
    if r.obstacles.get(j.0).is_none_or(|o| o.serial != serial) {
        return false;
    }
    if is_pending(r, ActionType::JunctionRemove, j.0) {
        return false;
    }
    r.m_obstacles.contains(&j.0) || is_pending(r, ActionType::JunctionAdd, j.0)
}

fn has_pin_class(r: &Router, s: ShapeRef, cls: u32) -> bool {
    s.connection_pins(r).iter().any(|&p| r.pins[p].m_class_id == cls)
}

fn install_callback(r: &mut Router, cbs: &Rc<RefCell<Vec<u32>>>, c: ConnIdx) {
    let id = r.conns[c].id();
    let log = cbs.clone();
    c.set_callback(r, Some(Box::new(move |_c: ConnIdx| log.borrow_mut().push(id))));
}

impl Scene {
    fn r(&mut self) -> &mut Router {
        self.router.as_mut().unwrap()
    }

    fn id_new(&self, k: Key) -> String {
        let r = self.router.as_ref().unwrap();
        match k {
            Key::Conn(h) => {
                if let Some(&(c, _)) = self.conns.iter().find(|(c, _)| hk(*c) == h) {
                    return r.conns[c].id().to_string();
                }
            }
            Key::Junc(h) => {
                if let Some(&(j, _)) = self.juncs.iter().find(|(j, _)| hk(j.0) == h) {
                    return j.id(r).to_string();
                }
            }
        }
        match self.known.get(&k) {
            Some(&(_, id)) => id.to_string(),
            None => "?".into(),
        }
    }

    fn id_del(&self, k: Key) -> String {
        match self.known.get(&k) {
            Some(&(_, id)) => id.to_string(),
            None => self.id_new(k),
        }
    }

    fn list_str(&self, keys: &[Key], deleted: bool) -> String {
        let v: Vec<String> =
            keys.iter().map(|&k| if deleted { self.id_del(k) } else { self.id_new(k) }).collect();
        format!("[{}]", v.join(" "))
    }

    fn sync(&mut self) {
        if self.router.is_none() {
            return;
        }
        // Connectors.
        {
            let r = self.router.as_ref().unwrap();
            self.conns.retain(|&(c, s)| conn_live(r, c, s));
            let mut added = Vec::new();
            for &c in r.conn_refs.iter() {
                let s = r.conns[c].serial;
                if !self.conns.iter().any(|&(c2, s2)| c2 == c && s2 == s) {
                    added.push((c, s));
                }
            }
            for (c, s) in added {
                self.conns.push((c, s));
                let cbs = self.callbacks.clone();
                install_callback(self.router.as_mut().unwrap(), &cbs, c);
            }
        }
        // Junctions.
        {
            let r = self.router.as_ref().unwrap();
            self.juncs.retain(|&(j, s)| junction_live(r, j, s));
            for &o in r.m_obstacles.iter() {
                if let Some(j) = Obstacle::as_junction(r, o) {
                    let s = r.obstacles[o].serial;
                    if !self.juncs.iter().any(|&(j2, s2)| j2 == j && s2 == s)
                        && !is_pending(r, ActionType::JunctionRemove, o)
                    {
                        self.juncs.push((j, s));
                    }
                }
            }
        }
        // Hyperedge improver / rerouter object lists: regenerated by every
        // rerouteAndCallbackConnectors() (improver lists only while an
        // improvement option is on), so they are printed after commands
        // during which a reroute ran.  Objects in "new" lists are named by
        // their live id, objects in "deleted" lists by the id they had at
        // the previous sync (they may have been freed since).
        let count = e2e_reroute_count();
        if count != self.reroute_count {
            self.reroute_count = count;
            let r = self.router.as_ref().unwrap();
            let keys_j = |l: &[JunctionRef]| -> Vec<Key> { l.iter().map(|j| Key::Junc(hk(j.0))).collect() };
            let keys_c = |l: &[ConnIdx]| -> Vec<Key> { l.iter().map(|c| Key::Conn(hk(*c))).collect() };
            if r.routing_option(RoutingOption::ImproveHyperedgeRoutesMovingJunctions)
                || r.routing_option(RoutingOption::ImproveHyperedgeRoutesMovingAddingAndDeletingJunctions)
            {
                let h = &r.m_hyperedge_improver;
                let line = format!(
                    "H nj={} nc={} dj={} dc={} cc={}",
                    self.list_str(&keys_j(&h.m_new_junctions), false),
                    self.list_str(&keys_c(&h.m_new_connectors), false),
                    self.list_str(&keys_j(&h.m_deleted_junctions), true),
                    self.list_str(&keys_c(&h.m_deleted_connectors), true),
                    self.list_str(&keys_c(&h.m_changed_connectors), false)
                );
                self.pending.push(line);
            }
            let h = &r.m_hyperedge_rerouter;
            for i in 0..h.m_new_junctions_vector.len() {
                let line = format!(
                    "R {} nj={} nc={} dj={} dc={}",
                    i,
                    self.list_str(&keys_j(&h.m_new_junctions_vector[i]), false),
                    self.list_str(&keys_c(&h.m_new_connectors_vector[i]), false),
                    self.list_str(&keys_j(&h.m_deleted_junctions_vector[i]), true),
                    self.list_str(&keys_c(&h.m_deleted_connectors_vector[i]), true)
                );
                self.pending.push(line);
            }
        }
        // Remember the ids of the live objects.
        let r = self.router.as_ref().unwrap();
        self.known.clear();
        for &(c, s) in self.conns.iter() {
            self.known.insert(Key::Conn(hk(c)), (s, r.conns[c].id()));
        }
        for &(j, s) in self.juncs.iter() {
            self.known.insert(Key::Junc(hk(j.0)), (s, j.id(r)));
        }
    }

    fn read_end(&self, t: &mut Toks) -> ConnEnd {
        let r = self.router.as_ref().unwrap();
        match t.word() {
            "P" => {
                let x = t.num();
                let y = t.num();
                let dirs = t.unum();
                ConnEnd::from_point_dirs(Point::new(x, y), dirs)
            }
            "S" => {
                let i = t.reference(self.shapes.len());
                let cls = t.unum();
                match i {
                    Some(i) if self.raw || has_pin_class(r, self.shapes[i], cls) => {
                        ConnEnd::from_shape_pin(r, self.shapes[i], cls)
                    }
                    _ => ConnEnd::from_point_dirs(Point::new(0.0, 0.0), CONN_DIR_ALL),
                }
            }
            "J" => match t.reference(self.juncs.len()) {
                None => ConnEnd::from_point_dirs(Point::new(0.0, 0.0), CONN_DIR_ALL),
                Some(i) => ConnEnd::from_junction(r, self.juncs[i].0),
            },
            k => panic!("bad end {}", k),
        }
    }

    fn dump(&mut self, ret: bool) {
        let mut o: Vec<String> = Vec::new();
        o.push(format!("T {} ret={}", self.stage, ret as i32));
        {
            let mut s = String::from("cb");
            for id in self.callbacks.borrow_mut().drain(..) {
                s.push_str(&format!(" {}", id));
            }
            o.push(s);
        }
        o.append(&mut self.pending);
        let shapes = self.shapes.clone();
        let juncs = self.juncs.clone();
        let clusters = self.clusters.clone();
        let r = self.router.as_mut().unwrap();
        for s in shapes {
            o.push(format!("S {} {}", s.id(r), poly_str(s.polygon(r))));
            for p in s.connection_pins(r) {
                let pin = &r.pins[p];
                o.push(format!(
                    "  pin c={} {} d={} x={} $={}",
                    pin.m_class_id,
                    pt_str(&pin.position(r, None)),
                    pin.directions(),
                    pin.m_exclusive as i32,
                    g17(pin.m_connection_cost)
                ));
            }
        }
        for (j, _) in juncs {
            o.push(format!("J {} {} f={}", j.id(r), pt_str(&j.position(r)), j.position_fixed(r) as i32));
        }
        for k in clusters {
            o.push(format!("K {} {}", k.id(r), poly_str(k.rectangular_polygon(r))));
        }
        let conn_refs = r.conn_refs.clone();
        for c in conn_refs {
            let (src, dst) = c.endpoint_conn_ends(r);
            let ty = |e: &ConnEnd| match e.type_() {
                ConnEndType::ConnEndPoint => "P",
                ConnEndType::ConnEndShapePin => "S",
                ConnEndType::ConnEndJunction => "J",
                _ => "E",
            };
            o.push(format!(
                "C {} t={} f={} src={}{} dst={}{}",
                r.conns[c].id(),
                r.conns[c].routing_type() as i32,
                r.conns[c].has_fixed_route() as i32,
                ty(&src),
                pt_str(&src.position(r)),
                ty(&dst),
                pt_str(&dst.position(r))
            ));
            o.push(format!("  raw {}", poly_str(r.conns[c].route())));
            o.push(format!("  disp {}", poly_str(r.conns[c].display_route())));
        }
        let q = format!(
            "Q ovl={}{} fovl={}{} touch={} cross={},{} invalid={}",
            r.exists_orthogonal_segment_overlap(false) as i32,
            r.exists_orthogonal_segment_overlap(true) as i32,
            r.exists_orthogonal_fixed_segment_overlap(false) as i32,
            r.exists_orthogonal_fixed_segment_overlap(true) as i32,
            r.exists_orthogonal_touching_paths() as i32,
            r.exists_crossings(false),
            r.exists_crossings(true),
            r.exists_invalid_orthogonal_paths() as i32
        );
        o.push(q);
        {
            let mut ids: Vec<u32> = self.shapes.iter().take(3).map(|s| s.id(r)).collect();
            if let Some(&(j, _)) = self.juncs.first() {
                ids.push(j.id(r));
            }
            let l = |v: &[u32]| format!("[{}]", v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" "));
            for id in ids {
                let mut s = format!("A {}", id);
                for t in 1..=3u32 {
                    let mut conns = Vec::new();
                    let mut shapes = Vec::new();
                    r.attached_conns(&mut conns, id, t);
                    r.attached_shapes(&mut shapes, id, t);
                    s.push_str(&format!(" {}{}", l(&conns), l(&shapes)));
                }
                o.push(s);
            }
        }
        {
            let text = r.output_diagram_text_g();
            // Like the C++ driver: split on '\n', a trailing newline does
            // not produce an empty final line.
            let mut start = 0;
            let b = text.as_bytes();
            while start < b.len() {
                let nl = text[start..].find('\n').map(|k| start + k).unwrap_or(b.len());
                o.push(format!("D {}", &text[start..nl]));
                start = nl + 1;
            }
        }
        // E2E_SVG (`SVG_OUTPUT` below): the SVG debug output too, as the C++
        // driver writes it (tests/e2e_svg_oracle.rs).
        if SVG_OUTPUT.load(std::sync::atomic::Ordering::Relaxed) {
            let r = self.router.as_mut().unwrap();
            let svg = r.output_instance_to_svg_string();
            o.push(format!("V {:016x} {}", fnv(std::slice::from_ref(&svg)), svg.len()));
            let svg = r.output_diagram_svg_string(None);
            o.push(format!("W {:016x} {}", fnv(std::slice::from_ref(&svg)), svg.len()));
        }
        self.stages.push(o);
        self.stage += 1;
    }

    /// As `hyperedgeIsTree()` in the C++ driver.
    fn hyperedge_is_tree(&self, root: JunctionRef) -> bool {
        let r = self.router.as_ref().unwrap();
        let mut seen_j = vec![root];
        let mut seen_c: Vec<ConnIdx> = Vec::new();
        let mut stack = vec![root];
        while let Some(j) = stack.pop() {
            for &(c, _) in self.conns.iter() {
                let (e1, e2) = c.endpoint_conn_ends(r);
                let a = e1.junction(r);
                let b = e2.junction(r);
                if a != Some(j) && b != Some(j) {
                    continue;
                }
                if a == b {
                    return false;
                }
                if seen_c.contains(&c) {
                    continue;
                }
                seen_c.push(c);
                let o = if a == Some(j) { b } else { a };
                if let Some(o) = o {
                    if seen_j.contains(&o) {
                        return false;
                    }
                    seen_j.push(o);
                    stack.push(o);
                }
            }
        }
        true
    }

    fn pin_at(&self, s: ShapeRef, t: &mut Toks) -> Option<PinIdx> {
        let pins = s.connection_pins(self.router.as_ref().unwrap());
        t.reference(pins.len()).map(|i| pins[i])
    }

    /// As the C++ driver: in immediate mode, commands that upstream cannot
    /// run there (see `unsafeImmediate()` in e2e_fuzz.cpp) run as a
    /// one-command transaction.
    fn run_command(&mut self, line: &str) {
        let cmd = line.split_whitespace().next().unwrap_or("");
        if cmd == "nowrap" {
            self.no_wrap = true;
            return;
        }
        if !self.no_wrap && self.router.as_ref().is_some_and(|r| !r.transaction_use()) {
            for _ in 0..3 {
                if self.router.as_ref().unwrap().action_list.is_empty() {
                    break;
                }
                self.r().set_transaction_use(true);
                self.r().process_transaction();
                self.r().set_transaction_use(false);
            }
        }
        let wrap = !self.no_wrap
            && self.router.as_ref().is_some_and(|r| !r.transaction_use())
            && !matches!(cmd, "txn" | "router" | "nowrap" | "")
            && !cmd.starts_with('#');
        if wrap {
            self.r().set_transaction_use(true);
        }
        self.run_command_inner(line);
        if wrap {
            self.r().process_transaction();
            self.r().set_transaction_use(false);
            self.sync();
        }
    }

    fn run_command_inner(&mut self, line: &str) {
        let mut t = Toks { it: line.split_whitespace() };
        let cmd = match t.it.next() {
            None => return,
            Some(c) => c,
        };
        if cmd.starts_with('#') {
            return;
        }
        if cmd == "router" {
            let flags = t.unum();
            self.router = Some(Router::new(flags));
            return;
        }
        macro_rules! refr {
            ($n:expr) => {
                match t.reference($n) {
                    None => return,
                    Some(i) => i,
                }
            };
        }
        match cmd {
            "param" => {
                let k = t.unum() as usize;
                let v = t.num();
                self.r().set_routing_parameter(RoutingParameter::ALL[k], v);
            }
            "option" => {
                let k = t.unum() as usize;
                let v = t.unum();
                self.r().set_routing_option(RoutingOption::ALL[k], v != 0);
            }
            "settings" => self.r().register_settings_change(),
            "txn" => {
                let v = t.unum();
                self.r().set_transaction_use(v != 0);
            }
            "process" => {
                let ret = self.r().process_transaction();
                self.sync();
                self.dump(ret);
            }
            "markmoved" => {
                // (as the C++ driver: not with queued removals)
                if !self.raw && self.router.as_ref().unwrap().action_list.iter().any(|a| {
                    a.type_ == ActionType::ShapeRemove || a.type_ == ActionType::JunctionRemove
                }) {
                    return;
                }
                self.r().mark_all_obstacles_as_moved()
            }
            "rect" => {
                let id = t.unum();
                let x1 = t.num();
                let y1 = t.num();
                let x2 = t.num();
                let y2 = t.num();
                let rect = Rectangle::new(&Point::new(x1, y1), &Point::new(x2, y2));
                let s = ShapeRef::new(self.r(), &rect, id);
                self.shapes.push(s);
            }
            "poly" => {
                let id = t.unum();
                let p = t.poly();
                let s = ShapeRef::new(self.r(), &p, id);
                self.shapes.push(s);
            }
            "moveby" => {
                let i = refr!(self.shapes.len());
                let dx = t.num();
                let dy = t.num();
                let s = self.shapes[i];
                self.r().move_shape_by(s, dx, dy);
            }
            "moveto" => {
                let i = refr!(self.shapes.len());
                let first = t.unum() != 0;
                let p = t.poly();
                let s = self.shapes[i];
                // (as the C++ driver: same number of points only)
                if !self.raw && p.ps.len() != s.polygon(self.router.as_ref().unwrap()).ps.len() {
                    return;
                }
                self.r().move_shape(s, &p, first);
            }
            "delshape" => {
                let i = refr!(self.shapes.len());
                let r = self.router.as_ref().unwrap();
                if !self.raw
                    && (is_pending(r, ActionType::ShapeAdd, self.shapes[i].0) || queued_conn_end_uses(r, self.shapes[i].0))
                {
                    return;
                }
                let s = self.shapes.remove(i);
                self.r().delete_shape(s);
            }
            "pin" => {
                let i = refr!(self.shapes.len());
                let cls = t.unum();
                let xo = t.num();
                let yo = t.num();
                let prop = t.unum();
                let inside = t.num();
                let dirs = t.unum();
                let s = self.shapes[i];
                ShapeConnectionPin::new_shape(self.r(), s, cls, xo, yo, prop != 0, inside, dirs);
            }
            "pinexcl" | "pincost" | "delpin" => {
                let i = refr!(self.shapes.len());
                let s = self.shapes[i];
                let p = match self.pin_at(s, &mut t) {
                    None => return,
                    Some(p) => p,
                };
                match cmd {
                    "pinexcl" => {
                        let v = t.unum();
                        p.set_exclusive(self.r(), v != 0);
                    }
                    "pincost" => {
                        let v = t.num();
                        p.set_connection_cost(self.r(), v);
                    }
                    _ => {
                        // (as the C++ driver: keep the last pin of a class)
                        let r = self.router.as_ref().unwrap();
                        let cls = r.pins[p].m_class_id;
                        let same = s.connection_pins(r).iter().filter(|&&q| r.pins[q].m_class_id == cls).count();
                        if same < 2 && !self.raw {
                            return;
                        }
                        ShapeConnectionPin::delete(self.r(), p)
                    }
                }
            }
            "transform" => {
                let i = refr!(self.shapes.len());
                let ty = match t.unum() {
                    0 => ShapeTransformationType::TransformationTypeCW90,
                    1 => ShapeTransformationType::TransformationTypeCW180,
                    2 => ShapeTransformationType::TransformationTypeCW270,
                    3 => ShapeTransformationType::TransformationTypeFlipX,
                    4 => ShapeTransformationType::TransformationTypeFlipY,
                    k => panic!("bad transform {}", k),
                };
                let s = self.shapes[i];
                s.transform_connection_pin_positions(self.r(), ty);
            }
            "junction" => {
                let id = t.unum();
                let x = t.num();
                let y = t.num();
                let fixed = t.unum();
                let j = JunctionRef::new(self.r(), Point::new(x, y), id);
                if fixed != 0 {
                    j.set_position_fixed(self.r(), true);
                }
                let s = self.router.as_ref().unwrap().obstacles[j.0].serial;
                self.juncs.push((j, s));
            }
            "fixj" => {
                let i = refr!(self.juncs.len());
                let v = t.unum();
                let j = self.juncs[i].0;
                j.set_position_fixed(self.r(), v != 0);
            }
            "movejby" => {
                let i = refr!(self.juncs.len());
                let dx = t.num();
                let dy = t.num();
                let j = self.juncs[i].0;
                self.r().move_junction_by(j, dx, dy);
            }
            "movejto" => {
                let i = refr!(self.juncs.len());
                let x = t.num();
                let y = t.num();
                let j = self.juncs[i].0;
                self.r().move_junction(j, &Point::new(x, y));
            }
            "deljunc" => {
                let i = refr!(self.juncs.len());
                let j = self.juncs[i].0;
                let r = self.router.as_ref().unwrap();
                if !self.raw && (is_pending(r, ActionType::JunctionAdd, j.0) || queued_conn_end_uses(r, j.0)) {
                    return;
                }
                self.juncs.remove(i);
                self.r().delete_junction(j);
            }
            "mergej" => {
                let i = refr!(self.juncs.len());
                let j = self.juncs[i].0;
                j.remove_junction_and_merge_connectors(self.r());
            }
            "conn" => {
                let id = t.unum();
                let src = self.read_end(&mut t);
                let dst = self.read_end(&mut t);
                let c = ConnRef::new_with_ends(self.r(), &src, &dst, id);
                let s = self.router.as_ref().unwrap().conns[c].serial;
                if !self.conns.iter().any(|&(c2, s2)| c2 == c && s2 == s) {
                    self.conns.push((c, s));
                    let cbs = self.callbacks.clone();
                    install_callback(self.r(), &cbs, c);
                }
            }
            "src" | "dst" => {
                let i = refr!(self.conns.len());
                let c = self.conns[i].0;
                let end = self.read_end(&mut t);
                if cmd == "src" {
                    c.set_source_endpoint(self.r(), &end);
                } else {
                    c.set_dest_endpoint(self.r(), &end);
                }
            }
            "type" => {
                let i = refr!(self.conns.len());
                let ty = match t.unum() {
                    0 => ConnType::ConnTypeNone,
                    1 => ConnType::ConnTypePolyLine,
                    2 => ConnType::ConnTypeOrthogonal,
                    k => panic!("bad conn type {}", k),
                };
                let c = self.conns[i].0;
                c.set_routing_type(self.r(), ty);
            }
            "hate" => {
                let i = refr!(self.conns.len());
                let v = t.unum();
                let c = self.conns[i].0;
                c.set_hate_crossings(self.r(), v != 0);
            }
            "invalid" => {
                let i = refr!(self.conns.len());
                let c = self.conns[i].0;
                // (The connector list is synchronised after transactions; a
                // connector freed since -- e.g. merged away by a split or a
                // junction merge in the same transaction -- is a dead handle
                // here, a use-after-free in the C++ driver: no-op, as for
                // the core's handle methods.)
                if !self.router.as_ref().unwrap().conns.contains(c) {
                    return;
                }
                self.r().conns[c].make_path_invalid();
            }
            "delconn" => {
                let i = refr!(self.conns.len());
                let (c, _) = self.conns.remove(i);
                self.r().delete_connector(c);
            }
            "cps" => {
                let i = refr!(self.conns.len());
                let n = t.unum();
                let mut cps = Vec::new();
                for _ in 0..n {
                    let x = t.num();
                    let y = t.num();
                    let ad = t.unum();
                    let dd = t.unum();
                    cps.push(Checkpoint::with_directions(Point::new(x, y), ad, dd));
                }
                let c = self.conns[i].0;
                c.set_routing_checkpoints(self.r(), &cps);
            }
            "fixroute" => {
                let i = refr!(self.conns.len());
                let p = t.poly();
                let c = self.conns[i].0;
                c.set_fixed_route(self.r(), &p);
            }
            "fixexist" => {
                let i = refr!(self.conns.len());
                let c = self.conns[i].0;
                // (dead handle: no-op, see "invalid")
                if self.router.as_ref().unwrap().conns.contains(c) && self.r().conns[c].route().ps.len() >= 2 {
                    c.set_fixed_existing_route(self.r());
                }
            }
            "clearfix" => {
                let i = refr!(self.conns.len());
                let c = self.conns[i].0;
                c.clear_fixed_route(self.r());
            }
            "split" => {
                let i = refr!(self.conns.len());
                let seg = t.unum() as usize;
                let c = self.conns[i].0;
                // (dead handle: no-op, see "invalid")
                if !self.router.as_ref().unwrap().conns.contains(c) {
                    return;
                }
                // (as the C++ driver: skip splits that would dereference a
                // null m_dst_connend upstream)
                let conn = &self.router.as_ref().unwrap().conns[c];
                if !self.raw && conn.m_display_route.ps.len() > seg && conn.m_dst_connend.is_none() {
                    return;
                }
                c.split_at_segment(self.r(), seg);
            }
            "cluster" => {
                let id = t.unum();
                let p = t.poly();
                let k = ClusterRef::new(self.r(), &p, id);
                self.clusters.push(k);
            }
            "movecluster" => {
                let i = refr!(self.clusters.len());
                let p = t.poly();
                let k = self.clusters[i];
                self.r().delete_cluster(k);
                k.set_new_poly(self.r(), &p);
                self.r().add_cluster(k);
            }
            "delcluster" => {
                let i = refr!(self.clusters.len());
                let k = self.clusters.remove(i);
                self.r().delete_cluster(k);
            }
            "regj" => {
                let i = refr!(self.juncs.len());
                let j = self.juncs[i].0;
                if !self.raw && !self.hyperedge_is_tree(j) {
                    return;
                }
                self.r().hyperedge_rerouter().register_hyperedge_for_rerouting_junction(j);
            }
            "regends" => {
                // (as the C++ driver: only pins of distinct shapes)
                let n = t.unum();
                let mut ends = Vec::new();
                let mut used: Vec<ShapeRef> = Vec::new();
                let mut ok = true;
                for _ in 0..n {
                    let kind = t.word();
                    if kind != "S" {
                        ok = false;
                        if kind == "P" {
                            t.num();
                            t.num();
                            t.unum();
                        } else {
                            t.word();
                        }
                        continue;
                    }
                    let i = t.reference(self.shapes.len());
                    let cls = t.unum();
                    let r = self.router.as_ref().unwrap();
                    match i {
                        Some(i)
                            if self.raw || (has_pin_class(r, self.shapes[i], cls) && !used.contains(&self.shapes[i])) =>
                        {
                            used.push(self.shapes[i]);
                            ends.push(ConnEnd::from_shape_pin(r, self.shapes[i], cls));
                        }
                        _ => ok = false,
                    }
                }
                if !self.raw && (!ok || ends.len() < 3) {
                    return;
                }
                self.r().hyperedge_rerouter().register_hyperedge_for_rerouting(ends);
            }
            _ => panic!("unknown command: {}", line),
        }
        self.sync();
    }
}

/// Runs a scene; returns its output lines (stage dumps hashed per stage if
/// the scene's output is large and `full` is false).
pub fn run_scene(dsl: &[String], full: bool) -> Vec<String> {
    run_scene_mode(dsl, full, false)
}

/// Runs a scene with all interpreter guards off (see `Scene::raw`); the
/// output is not comparable with the C++.
pub fn run_scene_raw(dsl: &[String]) -> Vec<String> {
    run_scene_mode(dsl, false, true)
}

fn run_scene_mode(dsl: &[String], full: bool, raw: bool) -> Vec<String> {
    let mut sc = Scene {
        router: None,
        shapes: Vec::new(),
        juncs: Vec::new(),
        conns: Vec::new(),
        clusters: Vec::new(),
        known: BTreeMap::new(),
        reroute_count: e2e_reroute_count(),
        no_wrap: raw,
        raw,
        pending: Vec::new(),
        stage: 0,
        stages: Vec::new(),
        callbacks: Rc::new(RefCell::new(Vec::new())),
    };
    // E2E_TRACE=1 (triage of slow scenes): one stderr line per command with
    // its run time and the router's object counts.
    let trace = std::env::var("E2E_TRACE").is_ok();
    let mut guards_before = libavoid_rust::avoid::router::e2e_guard_counts();
    let prov_on = std::env::var("E2E_PROV").is_ok();
    let mut prov = Prov { reroute_count: e2e_reroute_count(), ..Prov::default() };
    if prov_on {
        libavoid_rust::avoid::router::e2e_object_log_enable();
    }
    // E2E_PROV_TAINT=id,id,..: junction ids tainted from the start (to
    // follow the same hyperedge in a variant of the scene without the
    // skipped registration).
    if let Ok(v) = std::env::var("E2E_PROV_TAINT") {
        prov.tainted.extend(v.split(',').filter_map(|s| s.trim().parse::<u32>().ok()));
    }
    for (i, line) in dsl.iter().enumerate() {
        let t0 = std::time::Instant::now();
        sc.run_command(line);
        if prov_on {
            if let Some(r) = sc.router.as_ref() {
                prov_step(&mut prov, r, i, line.split_whitespace().next().unwrap_or(""));
            }
        }
        if trace {
            let g = libavoid_rust::avoid::router::e2e_guard_counts();
            if g != guards_before {
                let d: Vec<String> = g
                    .iter()
                    .filter(|(k, v)| guards_before.get(*k) != Some(*v))
                    .map(|(k, v)| format!("{}+{}", k, v - guards_before.get(k).copied().unwrap_or(0)))
                    .collect();
                eprintln!("guard {} {}", i, d.join(","));
                guards_before = g;
            }
            let (conns, obstacles) =
                sc.router.as_ref().map(|r| (r.conn_refs.len(), r.obstacles.len())).unwrap_or((0, 0));
            let cmd = line.split_whitespace().next().unwrap_or("");
            eprintln!("trace {} {} ms {} conns={} obstacles={}", i, t0.elapsed().as_millis(), cmd, conns, obstacles);
        }
    }
    if let Some(r) = sc.router.as_mut() {
        // (as the C++ driver: process queued deletions before ~Router)
        r.set_transaction_use(true);
        r.process_transaction();
    }
    if let Some(mut r) = sc.router.take() {
        r.destruct();
    }
    let total: usize = sc.stages.iter().map(|s| s.len()).sum();
    let mut res = Vec::new();
    for st in sc.stages.iter() {
        if full || total <= FULL_SCENE_LIMIT {
            res.extend(st.iter().cloned());
        } else {
            res.push(format!("{} lines={} fnv={:016x}", st[0], st.len(), fnv(st)));
        }
    }
    res
}

/// `E2E_PROV=1` (triage of growing scenes, Rust only): provenance of every
/// junction and connector.  After each command one stderr line
/// `prov CMD_INDEX CMD +J[..] -J[..] +C[..] -C[..]` lists the objects
/// created / deleted by that command (ids), each new one tagged with its
/// creator (`h`: hyperedge improver new-object list, `r`: rerouter list,
/// `u`: anything else, i.e. the DSL command) and with `*` if it lies in a
/// *tainted* hyperedge.  Hyperedges are the connected components of the
/// junction/connector graph; a component is tainted if it contains a
/// junction listed as deleted by a rerouter entry that created nothing
/// (a hyperedge the port's rerouter guard skipped: the port leaves its old
/// objects in `m_deleted_*_vector`) or a junction created in a tainted
/// component.  `comp` lines then give (junctions, connectors) of the
/// tainted and the other components.
#[derive(Default)]
struct Prov {
    conns: BTreeMap<(usize, u32), u32>,
    juncs: BTreeMap<(usize, u32), u32>,
    tainted: std::collections::BTreeSet<u32>,
    reroute_count: u64,
}

fn prov_step(p: &mut Prov, r: &Router, i: usize, cmd: &str) {
    use std::collections::{BTreeMap as M, BTreeSet};
    let conns: M<(usize, u32), u32> = r.conn_refs.iter().map(|&c| (hk(c), r.conns[c].id())).collect();
    let mut juncs: M<(usize, u32), u32> = M::new();
    let mut jhandles = Vec::new();
    for &o in r.m_obstacles.iter() {
        if let Some(j) = Obstacle::as_junction(r, o) {
            juncs.insert(hk(j.0), j.id(r));
            jhandles.push(j);
        }
    }
    // Creator lists of this command's reroute (if one ran).
    let mut by_h: BTreeSet<u32> = BTreeSet::new();
    let mut by_r: BTreeSet<u32> = BTreeSet::new();
    for (tag, id) in libavoid_rust::avoid::router::e2e_object_log_take() {
        if tag == 'h' {
            by_h.insert(id);
        } else {
            by_r.insert(id);
        }
    }
    let ran = e2e_reroute_count() != p.reroute_count;
    p.reroute_count = e2e_reroute_count();
    if ran {
        let rr = &r.m_hyperedge_rerouter;
        for k in 0..rr.m_new_junctions_vector.len() {
            if rr.m_new_junctions_vector[k].is_empty() && !rr.m_deleted_junctions_vector[k].is_empty() {
                let ids: Vec<u32> = rr.m_deleted_junctions_vector[k]
                    .iter()
                    .filter_map(|j| juncs.get(&hk(j.0)).or_else(|| p.juncs.get(&hk(j.0))).copied())
                    .collect();
                eprintln!("prov-skip {} rerouter entry {} left {} junctions: {:?}", i, k, ids.len(), ids);
                p.tainted.extend(ids);
            }
        }
    }
    // Components (union-find over junction ids through connectors).
    let mut parent: M<u32, u32> = juncs.values().map(|&id| (id, id)).collect();
    fn find(p: &mut M<u32, u32>, x: u32) -> u32 {
        let mut x = x;
        while p[&x] != x {
            let g = p[&p[&x]];
            p.insert(x, g);
            x = g;
        }
        x
    }
    let mut conn_j: M<u32, u32> = M::new();
    for &j in jhandles.iter() {
        let jid = j.id(r);
        for c in j.attached_connectors(r) {
            let cid = r.conns[c].id();
            if let Some(&other) = conn_j.get(&cid) {
                let (a, b) = (find(&mut parent, jid), find(&mut parent, other));
                if a != b {
                    parent.insert(a, b);
                }
            } else {
                conn_j.insert(cid, jid);
            }
        }
    }
    // Taint propagation: whole components.
    let ids: Vec<u32> = juncs.values().copied().collect();
    let mut tainted_roots: BTreeSet<u32> = BTreeSet::new();
    for &id in ids.iter() {
        if p.tainted.contains(&id) {
            tainted_roots.insert(find(&mut parent, id));
        }
    }
    for &id in ids.iter() {
        if tainted_roots.contains(&find(&mut parent, id)) {
            p.tainted.insert(id);
        }
    }
    let conn_tainted = |cid: u32, parent: &mut M<u32, u32>| -> bool {
        conn_j.get(&cid).is_some_and(|&j| tainted_roots.contains(&find(parent, j)))
    };
    let tag = |id: u32| -> &'static str {
        if by_h.contains(&id) {
            "h"
        } else if by_r.contains(&id) {
            "r"
        } else {
            "u"
        }
    };
    let mut add_j = Vec::new();
    for (k, &id) in juncs.iter() {
        if p.juncs.get(k) != Some(&id) {
            add_j.push(format!("{}{}{}", id, tag(id), if p.tainted.contains(&id) { "*" } else { "" }));
        }
    }
    let del_j: Vec<String> =
        p.juncs.iter().filter(|(k, id)| juncs.get(*k) != Some(*id)).map(|(_, id)| id.to_string()).collect();
    let mut add_c = Vec::new();
    for (k, &id) in conns.iter() {
        if p.conns.get(k) != Some(&id) {
            let t = conn_tainted(id, &mut parent);
            add_c.push(format!("{}{}{}", id, tag(id), if t { "*" } else { "" }));
        }
    }
    let del_c: Vec<String> =
        p.conns.iter().filter(|(k, id)| conns.get(*k) != Some(*id)).map(|(_, id)| id.to_string()).collect();
    let (mut tj, mut tc, mut oj, mut oc) = (0, 0, 0, 0);
    for &id in ids.iter() {
        if p.tainted.contains(&id) {
            tj += 1;
        } else {
            oj += 1;
        }
    }
    for &cid in conns.values() {
        if conn_tainted(cid, &mut parent) {
            tc += 1;
        } else {
            oc += 1;
        }
    }
    if !(add_j.is_empty() && del_j.is_empty() && add_c.is_empty() && del_c.is_empty()) {
        eprintln!(
            "prov {} {} +J[{}] -J[{}] +C[{}] -C[{}]",
            i,
            cmd,
            add_j.join(" "),
            del_j.join(" "),
            add_c.join(" "),
            del_c.join(" ")
        );
        eprintln!("comp {} tainted={}/{} other={}/{}", i, tj, tc, oj, oc);
        // Every component with more than one junction or any connector:
        // smallest junction id, junctions, connectors, tainted.
        let mut comps: M<u32, (u32, usize, usize)> = M::new();
        for &id in ids.iter() {
            let root = find(&mut parent, id);
            let e = comps.entry(root).or_insert((id, 0, 0));
            e.0 = e.0.min(id);
            e.1 += 1;
        }
        for &j in conn_j.values() {
            let root = find(&mut parent, j);
            comps.get_mut(&root).unwrap().2 += 1;
        }
        let v: Vec<String> = comps
            .iter()
            .filter(|(_, c)| c.1 > 1 || c.2 > 0)
            .map(|(root, c)| format!("{}:{}/{}{}", c.0, c.1, c.2, if tainted_roots.contains(root) { "*" } else { "" }))
            .collect();
        eprintln!("comps {} {}", i, v.join(" "));
    }
    p.conns = conns;
    p.juncs = juncs;
}

pub struct GoldenScene {
    pub name: String,
    pub dsl: Vec<String>,
    pub out: Vec<String>,
}

pub fn parse_golden(text: &str) -> Vec<GoldenScene> {
    let mut scenes = Vec::new();
    let mut cur: Option<GoldenScene> = None;
    let mut in_out = false;
    for line in text.lines() {
        if let Some(name) = line.strip_prefix("#scene ") {
            cur = Some(GoldenScene { name: name.to_string(), dsl: Vec::new(), out: Vec::new() });
            in_out = false;
        } else if line == "#out" {
            in_out = true;
        } else if line == "#end" {
            if let Some(c) = cur.take() {
                scenes.push(c);
            }
        } else if let Some(c) = cur.as_mut() {
            if in_out {
                c.out.push(line.to_string());
            } else {
                c.dsl.push(line.to_string());
            }
        }
    }
    scenes
}
