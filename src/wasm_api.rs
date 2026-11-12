//! The wasm-bindgen `AvoidRouter`: a thin string-id adapter over
//! [`crate::avoid::router::Router`] (the faithful port of upstream
//! libavoid).
//!
//! Every method delegates to the upstream-ported function named in its doc
//! comment; the adapter itself only
//!
//! * maps JS string ids to core handles (and back),
//! * validates JS input so that nothing reaches a core precondition
//!   (`COLA_ASSERT` = `debug_assert!`, or an `expect`) that would abort the
//!   wasm instance (`panic = "abort"`),
//! * converts results to JSON strings (parsed by `index.cjs`), and
//! * keeps its id maps in sync with objects the core creates/deletes
//!   itself during a transaction (hyperedge rerouting / improvement).
//!
//! # Ids
//! Every shape, junction, connector and cluster has a caller-chosen string
//! id (unique across all four kinds) and an upstream numeric id
//! (`unsigned int`).  The numeric id is the optional `numericId` argument of
//! the creating method; when it is omitted (or 0) the core assigns
//! `Router::newObjectId()` (largest id assigned so far + 1), i.e. ids are
//! sequential in creation order exactly as upstream.  An explicit numeric
//! id is accepted iff upstream `Router::objectIdIsUnused(id)` holds (the
//! condition upstream asserts).  Objects created by the router itself (new
//! junctions/connectors of hyperedge rerouting/improvement) get the decimal
//! string of their numeric id as string id (suffixed with `#2`, `#3`, ...
//! if that string is taken).
//!
//! Connection pins are named per owner (shape or junction): the pin id is
//! a string unique among that owner's pins.  Junctions' built-in centre pin
//! is not named.
//!
//! # Error convention
//! Nothing a JS caller passes can panic: mutators return `false` and
//! queries return `null` (raw layer: `"null"` JSON / `undefined`) for
//! unknown ids, duplicate ids, non-finite numbers, out-of-range enum
//! values and violated upstream preconditions.  The constructor throws.
//! (`index.cjs` additionally throws `TypeError` for structurally malformed
//! arguments such as a non-array where an array is expected.)

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use wasm_bindgen::prelude::*;

use crate::avoid::actioninfo::{ActionObj, ActionType};
use crate::avoid::arena::{ClusterIdx, ConnIdx, ObstacleIdx, PinIdx};
use crate::avoid::connectionpin::{ShapeConnectionPin, CONNECTIONPIN_UNSET};
use crate::avoid::connector::{Checkpoint, ConnRef, ConnType};
use crate::avoid::connend::{ConnEnd, ConnEndType, CONN_DIR_ALL};
use crate::avoid::geomtypes::{Point, Polygon, PolygonInterface, Rectangle};
use crate::avoid::hyperedge::HyperedgeNewAndDeletedObjectLists;
use crate::avoid::junction::JunctionRef;
use crate::avoid::router::{
    Router, RoutingOption, RoutingParameter, CHOOSE_SENSIBLE_PARAM_VALUE, ORTHOGONAL_ROUTING,
    POLY_LINE_ROUTING, TRANSACTION_PHASE_COMPLETED,
};
use crate::avoid::shape::{ShapeRef, ShapeTransformationType};
use crate::avoid::viscluster::ClusterRef;
use crate::json;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Obj {
    Shape(ObstacleIdx),
    Junction(ObstacleIdx),
    Conn(ConnIdx),
    Cluster(ClusterIdx),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Shape,
    Junction,
    Conn,
    Cluster,
}

impl Obj {
    fn kind(self) -> Kind {
        match self {
            Obj::Shape(_) => Kind::Shape,
            Obj::Junction(_) => Kind::Junction,
            Obj::Conn(_) => Kind::Conn,
            Obj::Cluster(_) => Kind::Cluster,
        }
    }
}

#[derive(Clone, Debug)]
struct Entry {
    obj: Obj,
    serial: u64,
}

#[derive(Clone, Debug)]
struct PinEntry {
    name: String,
    pin: PinIdx,
}

/// String-id form of upstream `HyperedgeNewAndDeletedObjectLists`.
#[derive(Clone, Debug, Default)]
struct NdLists {
    new_junctions: Vec<String>,
    deleted_junctions: Vec<String>,
    new_connectors: Vec<String>,
    deleted_connectors: Vec<String>,
    changed_connectors: Vec<String>,
}

impl NdLists {
    fn to_json(&self) -> String {
        json::object(&[
            ("newJunctionIds", json::strings(&self.new_junctions)),
            ("deletedJunctionIds", json::strings(&self.deleted_junctions)),
            ("newConnectorIds", json::strings(&self.new_connectors)),
            ("deletedConnectorIds", json::strings(&self.deleted_connectors)),
            ("changedConnectorIds", json::strings(&self.changed_connectors)),
        ])
    }
}

type CallbackLog = Rc<RefCell<Vec<(ConnIdx, u64)>>>;

/// Libavoid router with string object ids (see the module docs).
#[wasm_bindgen]
pub struct AvoidRouter {
    router: Router,
    flags: u32,
    objects: HashMap<String, Entry>,
    by_obstacle: HashMap<ObstacleIdx, (u64, String)>,
    by_conn: HashMap<ConnIdx, (u64, String)>,
    by_cluster: HashMap<ClusterIdx, (u64, String)>,
    pins: HashMap<String, Vec<PinEntry>>,
    transactions: Rc<Cell<u64>>,
    callback_log: CallbackLog,
    last_updated: Vec<String>,
    hyperedge_results: Option<Vec<NdLists>>,
    improvement_results: NdLists,
}

// ---------------------------------------------------------------------------
// Input validation helpers
// ---------------------------------------------------------------------------

fn as_u32(v: f64) -> Option<u32> {
    if v.is_finite() && v.fract() == 0.0 && (0.0..=u32::MAX as f64).contains(&v) {
        Some(v as u32)
    } else {
        None
    }
}

fn as_enum(v: f64, count: usize) -> Option<usize> {
    as_u32(v).map(|i| i as usize).filter(|&i| i < count)
}

fn as_dirs(v: f64) -> Option<u32> {
    as_u32(v).filter(|&d| d <= CONN_DIR_ALL)
}

/// Pin class ids: 1..=u32::MAX (upstream asserts `classId > 0`).
fn as_class(v: f64) -> Option<u32> {
    as_u32(v).filter(|&c| c > 0)
}

fn finite(vs: &[f64]) -> bool {
    vs.iter().all(|v| v.is_finite())
}

fn point_list(coords: &[f64]) -> Option<Vec<Point>> {
    if coords.len() % 2 != 0 || !finite(coords) {
        return None;
    }
    Some(coords.chunks(2).map(|c| Point::new(c[0], c[1])).collect())
}

fn polygon_from(coords: &[f64], min_points: usize) -> Option<Polygon> {
    let ps = point_list(coords)?;
    if ps.len() < min_points {
        return None;
    }
    let mut poly = Polygon::new();
    poly.ps = ps;
    Some(poly)
}

fn rect(left: f64, top: f64, right: f64, bottom: f64) -> Option<Polygon> {
    if !finite(&[left, top, right, bottom]) {
        return None;
    }
    Some(Rectangle::new(&Point::new(left, top), &Point::new(right, bottom)))
}

fn opt_json(v: Option<String>) -> String {
    v.unwrap_or_else(|| "null".to_string())
}

const TRANSFORMS: [ShapeTransformationType; 5] = [
    ShapeTransformationType::TransformationTypeCW90,
    ShapeTransformationType::TransformationTypeCW180,
    ShapeTransformationType::TransformationTypeCW270,
    ShapeTransformationType::TransformationTypeFlipX,
    ShapeTransformationType::TransformationTypeFlipY,
];

const CONN_TYPES: [ConnType; 3] = [ConnType::ConnTypeNone, ConnType::ConnTypePolyLine, ConnType::ConnTypeOrthogonal];

/// Raw ConnEnd encoding used by the wasm layer (see `index.cjs`
/// `connEndArgs`): kind 0 = point (x, y, num = directions), 1 = shape pin
/// class (objectId = shape, num = class id), 2 = junction (objectId),
/// 3 = shape pin by name (objectId = shape, pinId).
#[derive(Clone, Copy)]
struct RawEnd<'a> {
    kind: f64,
    object_id: &'a str,
    pin_id: &'a str,
    x: f64,
    y: f64,
    num: f64,
}

// ---------------------------------------------------------------------------
// Native (non-JS) API
// ---------------------------------------------------------------------------

impl AvoidRouter {
    /// Creates a router (upstream `Router(flags)`); `None` unless `flags`
    /// is a non-empty combination of `POLY_LINE_ROUTING` and
    /// `ORTHOGONAL_ROUTING`.
    pub fn create(flags: u32) -> Option<AvoidRouter> {
        if flags == 0 || flags & !(POLY_LINE_ROUTING | ORTHOGONAL_ROUTING) != 0 {
            return None;
        }
        let transactions = Rc::new(Cell::new(0u64));
        let mut router = Router::new(flags);
        AvoidRouter::install_progress_hook(&mut router, &transactions);
        Some(AvoidRouter {
            router,
            flags,
            objects: HashMap::new(),
            by_obstacle: HashMap::new(),
            by_conn: HashMap::new(),
            by_cluster: HashMap::new(),
            pins: HashMap::new(),
            transactions,
            callback_log: Rc::new(RefCell::new(Vec::new())),
            last_updated: Vec::new(),
            hyperedge_results: None,
            improvement_results: NdLists::default(),
        })
    }

    /// Number of transactions processed so far (Rust-only diagnostic used
    /// by the wrapper tests).
    pub fn transaction_count(&self) -> u64 {
        self.transactions.get()
    }

    /// Read access to the wrapped core router (for tests and Rust users).
    pub fn core(&self) -> &Router {
        &self.router
    }

    /// Counts completed transactions through the upstream
    /// `shouldContinueTransactionWithProgress()` hook (always continues).
    fn install_progress_hook(router: &mut Router, counter: &Rc<Cell<u64>>) {
        let counter = counter.clone();
        router.m_progress_callback = Some(Box::new(move |_elapsed, phase, _total, _proportion| {
            if phase == TRANSACTION_PHASE_COMPLETED {
                counter.set(counter.get() + 1);
            }
            true
        }));
    }

    // --- transaction bookkeeping -------------------------------------------

    fn begin(&mut self) -> u64 {
        self.callback_log.borrow_mut().clear();
        self.transactions.get()
    }

    fn end(&mut self, t0: u64) {
        if self.transactions.get() != t0 {
            self.after_transaction();
        }
    }

    fn set_conn_callback(&mut self, c: ConnIdx) {
        let serial = self.router.conns[c].serial;
        let log = self.callback_log.clone();
        c.set_callback(
            &mut self.router,
            Some(Box::new(move |conn| log.borrow_mut().push((conn, serial)))),
        );
    }

    /// Synchronises the id maps with objects the core created or deleted
    /// during the transaction(s) just run and records the transaction's
    /// results (updated connectors, hyperedge object lists).
    fn after_transaction(&mut self) {
        // 1. Deleted objects are resolved before the maps are reconciled
        //    (their arena slots may already have been reused).
        let rr = &self.router.m_hyperedge_rerouter;
        let n = rr
            .m_new_junctions_vector
            .len()
            .min(rr.m_new_connectors_vector.len())
            .min(rr.m_deleted_junctions_vector.len())
            .min(rr.m_deleted_connectors_vector.len());
        let mut rerouter_lists: Vec<HyperedgeNewAndDeletedObjectLists> = Vec::new();
        for i in 0..n {
            rerouter_lists.push(HyperedgeNewAndDeletedObjectLists {
                new_junction_list: rr.m_new_junctions_vector[i].clone(),
                new_connector_list: rr.m_new_connectors_vector[i].clone(),
                deleted_junction_list: rr.m_deleted_junctions_vector[i].clone(),
                deleted_connector_list: rr.m_deleted_connectors_vector[i].clone(),
                changed_connector_list: Vec::new(),
            });
        }
        let improver_ran = self.router.routing_option(RoutingOption::ImproveHyperedgeRoutesMovingJunctions)
            || self
                .router
                .routing_option(RoutingOption::ImproveHyperedgeRoutesMovingAddingAndDeletingJunctions);
        let improver = if improver_ran {
            Some(self.router.new_and_deleted_object_lists_from_hyperedge_improvement())
        } else {
            None
        };
        let deleted_names = |this: &AvoidRouter, l: &HyperedgeNewAndDeletedObjectLists| {
            let js: Vec<String> = l
                .deleted_junction_list
                .iter()
                .filter_map(|j| this.by_obstacle.get(&j.0).map(|e| e.1.clone()))
                .collect();
            let cs: Vec<String> = l
                .deleted_connector_list
                .iter()
                .filter_map(|c| this.by_conn.get(c).map(|e| e.1.clone()))
                .collect();
            (js, cs)
        };
        let mut rerouter_nd: Vec<NdLists> = rerouter_lists
            .iter()
            .map(|l| {
                let (dj, dc) = deleted_names(self, l);
                NdLists { deleted_junctions: dj, deleted_connectors: dc, ..NdLists::default() }
            })
            .collect();
        let mut improver_nd = improver.as_ref().map(|l| {
            let (dj, dc) = deleted_names(self, l);
            NdLists { deleted_junctions: dj, deleted_connectors: dc, ..NdLists::default() }
        });

        // 2. Forget objects the core freed or queued for removal.
        let dead_obstacles: Vec<ObstacleIdx> = self
            .by_obstacle
            .iter()
            .filter(|(o, (serial, _))| !self.obstacle_live(**o, *serial))
            .map(|(o, _)| *o)
            .collect();
        for o in dead_obstacles {
            if let Some((_, name)) = self.by_obstacle.remove(&o) {
                self.objects.remove(&name);
                self.pins.remove(&name);
            }
        }
        let dead_conns: Vec<ConnIdx> = self
            .by_conn
            .iter()
            .filter(|(c, (serial, _))| self.router.conns.get(**c).map(|x| x.serial) != Some(*serial))
            .map(|(c, _)| *c)
            .collect();
        for c in dead_conns {
            if let Some((_, name)) = self.by_conn.remove(&c) {
                self.objects.remove(&name);
            }
        }

        // 3. Name objects the core created.
        let mut new_obstacles: Vec<(u64, ObstacleIdx)> = self
            .router
            .obstacles
            .iter_unordered()
            .filter(|(o, ob)| {
                self.by_obstacle.get(o).map(|e| e.0) != Some(ob.serial) && self.obstacle_live(*o, ob.serial)
            })
            .map(|(o, ob)| (ob.serial, o))
            .collect();
        new_obstacles.sort_by_key(|e| e.0);
        for (serial, o) in new_obstacles {
            let id = self.router.obstacles[o].m_id;
            let name = self.auto_name(id);
            let obj = if self.router.obstacles[o].is_junction() { Obj::Junction(o) } else { Obj::Shape(o) };
            self.register(name, obj, serial);
        }
        let mut new_conns: Vec<(u64, ConnIdx)> = self
            .router
            .conns
            .iter_unordered()
            .filter(|(c, conn)| self.by_conn.get(c).map(|e| e.0) != Some(conn.serial))
            .map(|(c, conn)| (conn.serial, c))
            .collect();
        new_conns.sort_by_key(|e| e.0);
        for (serial, c) in new_conns {
            let id = self.router.conns[c].m_id;
            let name = self.auto_name(id);
            self.register(name, Obj::Conn(c), serial);
            self.set_conn_callback(c);
        }

        // 4. New / changed objects are resolved against the updated maps.
        let new_names = |this: &AvoidRouter, l: &HyperedgeNewAndDeletedObjectLists, nd: &mut NdLists| {
            nd.new_junctions = l.new_junction_list.iter().filter_map(|j| this.obstacle_name(j.0)).collect();
            nd.new_connectors = l.new_connector_list.iter().filter_map(|c| this.conn_name(*c)).collect();
            nd.changed_connectors = l.changed_connector_list.iter().filter_map(|c| this.conn_name(*c)).collect();
        };
        for (l, nd) in rerouter_lists.iter().zip(rerouter_nd.iter_mut()) {
            new_names(self, l, nd);
        }
        if let (Some(l), Some(nd)) = (improver.as_ref(), improver_nd.as_mut()) {
            new_names(self, l, nd);
        }
        self.hyperedge_results = Some(rerouter_nd);
        if let Some(nd) = improver_nd {
            self.improvement_results = nd;
        }

        // 5. Connectors whose callback fired (routes changed).
        let log: Vec<(ConnIdx, u64)> = self.callback_log.borrow_mut().drain(..).collect();
        let mut seen = HashSet::new();
        self.last_updated = log
            .into_iter()
            .filter_map(|(c, serial)| match self.by_conn.get(&c) {
                Some((s, name)) if *s == serial => Some(name.clone()),
                _ => None,
            })
            .filter(|n| seen.insert(n.clone()))
            .collect();
    }

    fn obstacle_live(&self, o: ObstacleIdx, serial: u64) -> bool {
        match self.router.obstacles.get(o) {
            Some(ob) if ob.serial == serial => !self.has_pending_remove(o),
            _ => false,
        }
    }

    fn has_pending_action(&self, o: ObstacleIdx, t: ActionType) -> bool {
        self.router.action_list.iter().any(|a| a.type_ == t && a.obj_ptr == ActionObj::Obstacle(o))
    }

    fn has_pending_remove(&self, o: ObstacleIdx) -> bool {
        self.has_pending_action(o, ActionType::ShapeRemove) || self.has_pending_action(o, ActionType::JunctionRemove)
    }

    /// Obstacles referenced by hyperedges registered for rerouting in the
    /// next transaction.
    fn hyperedge_referenced(&self, o: ObstacleIdx) -> bool {
        let rr = &self.router.m_hyperedge_rerouter;
        rr.m_root_junction_vector.iter().any(|j| j.map(|j| j.0) == Some(o))
            || rr.m_terminals_vector.iter().flatten().any(|ce| ce.m_anchor_obj == Some(o))
    }

    fn auto_name(&self, id: u32) -> String {
        let base = id.to_string();
        if !self.objects.contains_key(&base) {
            return base;
        }
        let mut k = 2;
        loop {
            let name = format!("{}#{}", base, k);
            if !self.objects.contains_key(&name) {
                return name;
            }
            k += 1;
        }
    }

    fn register(&mut self, name: String, obj: Obj, serial: u64) {
        match obj {
            Obj::Shape(o) | Obj::Junction(o) => {
                self.by_obstacle.insert(o, (serial, name.clone()));
            }
            Obj::Conn(c) => {
                self.by_conn.insert(c, (serial, name.clone()));
            }
            Obj::Cluster(c) => {
                self.by_cluster.insert(c, (serial, name.clone()));
            }
        }
        self.objects.insert(name, Entry { obj, serial });
    }

    fn unregister(&mut self, name: &str) {
        if let Some(e) = self.objects.remove(name) {
            match e.obj {
                Obj::Shape(o) | Obj::Junction(o) => {
                    self.by_obstacle.remove(&o);
                }
                Obj::Conn(c) => {
                    self.by_conn.remove(&c);
                }
                Obj::Cluster(c) => {
                    self.by_cluster.remove(&c);
                }
            }
        }
        self.pins.remove(name);
    }

    fn obstacle_name(&self, o: ObstacleIdx) -> Option<String> {
        let ob = self.router.obstacles.get(o)?;
        match self.by_obstacle.get(&o) {
            Some((s, n)) if *s == ob.serial => Some(n.clone()),
            _ => None,
        }
    }

    fn conn_name(&self, c: ConnIdx) -> Option<String> {
        let conn = self.router.conns.get(c)?;
        match self.by_conn.get(&c) {
            Some((s, n)) if *s == conn.serial => Some(n.clone()),
            _ => None,
        }
    }

    fn lookup(&self, id: &str) -> Option<Obj> {
        self.objects.get(id).map(|e| e.obj)
    }

    fn shape(&self, id: &str) -> Option<ShapeRef> {
        match self.lookup(id)? {
            Obj::Shape(o) => Some(ShapeRef(o)),
            _ => None,
        }
    }

    fn junction(&self, id: &str) -> Option<JunctionRef> {
        match self.lookup(id)? {
            Obj::Junction(o) => Some(JunctionRef(o)),
            _ => None,
        }
    }

    fn conn(&self, id: &str) -> Option<ConnIdx> {
        match self.lookup(id)? {
            Obj::Conn(c) => Some(c),
            _ => None,
        }
    }

    fn cluster(&self, id: &str) -> Option<ClusterIdx> {
        match self.lookup(id)? {
            Obj::Cluster(c) => Some(c),
            _ => None,
        }
    }

    /// Validates a new string id and the optional numeric id; returns the
    /// id to pass to the core constructor (0 = assign).
    fn new_ids(&self, id: &str, numeric_id: Option<f64>) -> Option<u32> {
        if self.objects.contains_key(id) {
            return None;
        }
        match numeric_id {
            None => {
                if self.router.new_object_id() == 0 {
                    None // id space exhausted (upstream would wrap to 0)
                } else {
                    Some(0)
                }
            }
            Some(v) => {
                let n = as_u32(v)?;
                if n == 0 {
                    return if self.router.new_object_id() == 0 { None } else { Some(0) };
                }
                if self.router.object_id_is_unused(n) {
                    Some(n)
                } else {
                    None
                }
            }
        }
    }

    fn pin(&self, owner: &str, pin_id: &str) -> Option<PinIdx> {
        self.pins.get(owner)?.iter().find(|p| p.name == pin_id).map(|p| p.pin)
    }

    fn shape_pin(&self, shape_id: &str, pin_id: &str) -> Option<PinIdx> {
        self.shape(shape_id)?;
        self.pin(shape_id, pin_id)
    }

    fn junction_pin(&self, junction_id: &str, pin_id: &str) -> Option<PinIdx> {
        self.junction(junction_id)?;
        self.pin(junction_id, pin_id)
    }

    fn conn_end(&self, e: RawEnd) -> Option<ConnEnd> {
        match as_enum(e.kind, 4)? {
            0 => {
                if !finite(&[e.x, e.y]) {
                    return None;
                }
                Some(ConnEnd::from_point_dirs(Point::new(e.x, e.y), as_dirs(e.num)?))
            }
            1 => {
                let s = self.shape(e.object_id)?;
                let class = as_class(e.num).filter(|&c| c != CONNECTIONPIN_UNSET)?;
                Some(ConnEnd::from_shape_pin(&self.router, s, class))
            }
            2 => Some(ConnEnd::from_junction(&self.router, self.junction(e.object_id)?)),
            _ => {
                let s = self.shape(e.object_id)?;
                let pin = self.pin(e.object_id, e.pin_id)?;
                let class = self.router.pins[pin].m_class_id;
                if class == 0 || class == CONNECTIONPIN_UNSET {
                    return None;
                }
                Some(ConnEnd::from_shape_pin(&self.router, s, class))
            }
        }
    }

    fn conn_end_json(&self, ce: &ConnEnd) -> String {
        let ty = match ce.type_() {
            ConnEndType::ConnEndPoint => "point",
            ConnEndType::ConnEndShapePin => "shapePin",
            ConnEndType::ConnEndJunction => "junction",
            ConnEndType::ConnEndEmpty => "empty",
        };
        let mut fields = vec![
            ("type", json::string(ty)),
            ("position", json::point(&ce.position(&self.router))),
            ("directions", json::number(ce.directions(&self.router) as f64)),
        ];
        match ce.type_() {
            ConnEndType::ConnEndShapePin => {
                let name = ce.m_anchor_obj.and_then(|o| self.obstacle_name(o));
                fields.push(("shapeId", json::opt_string(name.as_deref())));
                fields.push(("classId", json::number(ce.pin_class_id() as f64)));
            }
            ConnEndType::ConnEndJunction => {
                let name = ce.m_anchor_obj.and_then(|o| self.obstacle_name(o));
                fields.push(("junctionId", json::opt_string(name.as_deref())));
                fields.push(("classId", json::number(ce.pin_class_id() as f64)));
            }
            _ => {}
        }
        json::object(&fields)
    }

    fn ids_of_kind(&self, kind: Kind) -> Vec<String> {
        let mut v: Vec<(u64, &String)> = self
            .objects
            .iter()
            .filter(|(_, e)| e.obj.kind() == kind)
            .map(|(n, e)| (e.serial, n))
            .collect();
        v.sort();
        v.into_iter().map(|(_, n)| n.clone()).collect()
    }

    fn name_for_numeric(&self, n: u32) -> Option<String> {
        let mut best: Option<(u64, &String)> = None;
        for (name, e) in self.objects.iter() {
            let id = match e.obj {
                Obj::Shape(o) | Obj::Junction(o) => self.router.obstacles[o].m_id,
                Obj::Conn(c) => self.router.conns[c].m_id,
                Obj::Cluster(c) => self.router.clusters[c].m_id,
            };
            if id == n && best.is_none_or(|(s, _)| e.serial < s) {
                best = Some((e.serial, name));
            }
        }
        best.map(|(_, n)| n.clone())
    }

    fn numeric_names(&self, ids: &[u32]) -> Vec<String> {
        ids.iter()
            .map(|&n| self.name_for_numeric(n).unwrap_or_else(|| n.to_string()))
            .collect()
    }

    fn conn_names(&self, conns: &[ConnIdx]) -> Vec<String> {
        conns.iter().filter_map(|&c| self.conn_name(c)).collect()
    }

    fn add_conn_internal(&mut self, id: &str, ends: Option<(ConnEnd, ConnEnd)>, numeric_id: Option<f64>) -> bool {
        let Some(nid) = self.new_ids(id, numeric_id) else {
            return false;
        };
        let t0 = self.begin();
        // Upstream's two-ConnEnd constructor registers the connector's
        // reroute flag only after setEndpoints(); with transaction use off,
        // setEndpoints() already routes the connector and generatePath()
        // asserts on (release: dereferences) the missing flag.  The
        // wrapper therefore queues the construction and processes it as one
        // transaction afterwards.
        let immediate = !self.router.transaction_use();
        if immediate && ends.is_some() {
            self.router.set_transaction_use(true);
        }
        let c = match &ends {
            Some((s, d)) => ConnRef::new_with_ends(&mut self.router, s, d, nid),
            None => ConnRef::new(&mut self.router, nid),
        };
        let serial = self.router.conns[c].serial;
        self.register(id.to_string(), Obj::Conn(c), serial);
        self.set_conn_callback(c);
        if immediate && ends.is_some() {
            self.router.set_transaction_use(false);
            self.router.process_transaction();
        }
        self.end(t0);
        true
    }

    fn set_end_internal(&mut self, id: &str, src: Option<ConnEnd>, dst: Option<ConnEnd>) -> bool {
        let Some(c) = self.conn(id) else {
            return false;
        };
        let t0 = self.begin();
        match (src, dst) {
            (Some(s), Some(d)) => c.set_endpoints(&mut self.router, &s, &d),
            (Some(s), None) => c.set_source_endpoint(&mut self.router, &s),
            (None, Some(d)) => c.set_dest_endpoint(&mut self.router, &d),
            (None, None) => {}
        }
        self.end(t0);
        true
    }

    fn add_pin_internal(&mut self, owner: &str, pin_id: &str, make: impl FnOnce(&mut Router) -> PinIdx) -> bool {
        if self.pin(owner, pin_id).is_some() {
            return false;
        }
        let t0 = self.begin();
        let pin = make(&mut self.router);
        self.pins
            .entry(owner.to_string())
            .or_default()
            .push(PinEntry { name: pin_id.to_string(), pin });
        self.end(t0);
        true
    }

    fn remove_pin_internal(&mut self, owner: &str, pin_id: &str) -> bool {
        let Some(pin) = self.pin(owner, pin_id) else {
            return false;
        };
        if let Some(Obj::Shape(o)) | Some(Obj::Junction(o)) = self.lookup(owner) {
            // A registered hyperedge terminal needs a pin of its class when
            // the transaction runs (upstream dereferences a null vertex).
            if self.hyperedge_referenced(o) {
                return false;
            }
        }
        let t0 = self.begin();
        ShapeConnectionPin::delete(&mut self.router, pin);
        if let Some(v) = self.pins.get_mut(owner) {
            v.retain(|p| p.name != pin_id);
        }
        self.end(t0);
        true
    }

    fn pin_position_json(&self, pin: Option<PinIdx>) -> String {
        opt_json(pin.map(|p| json::point(&p.position(&self.router))))
    }

    fn pin_ids_json(&self, owner: &str, pin: Option<PinIdx>) -> String {
        opt_json(pin.map(|p| {
            let (obj, class) = p.ids(&self.router);
            json::object(&[
                ("objectId", json::string(owner)),
                ("numericObjectId", json::number(obj as f64)),
                ("classId", json::number(class as f64)),
            ])
        }))
    }

    fn set_pin_cost(&mut self, pin: Option<PinIdx>, cost: f64) -> bool {
        match pin {
            Some(p) if cost.is_finite() && cost >= 0.0 => {
                p.set_connection_cost(&mut self.router, cost);
                true
            }
            _ => false,
        }
    }

    fn set_pin_exclusive(&mut self, pin: Option<PinIdx>, exclusive: bool) -> bool {
        match pin {
            Some(p) => {
                p.set_exclusive(&mut self.router, exclusive);
                true
            }
            None => false,
        }
    }

    fn route_json(&mut self, id: &str, display: bool) -> String {
        let Some(c) = self.conn(id) else {
            return "null".into();
        };
        if display {
            json::points(&c.display_route(&mut self.router).ps)
        } else {
            json::points(&c.route(&self.router).ps)
        }
    }

    fn add_shape_poly(&mut self, id: String, poly: &Polygon, numeric_id: Option<f64>) -> bool {
        let Some(nid) = self.new_ids(&id, numeric_id) else {
            return false;
        };
        let t0 = self.begin();
        let s = ShapeRef::new(&mut self.router, poly, nid);
        let serial = self.router.obstacles[s.0].serial;
        self.register(id, Obj::Shape(s.0), serial);
        self.end(t0);
        true
    }

    fn move_shape_poly(&mut self, id: &str, poly: &Polygon, first_move: bool) -> bool {
        let Some(s) = self.shape(id) else {
            return false;
        };
        if self.router.obstacles[s.0].m_polygon.size() != poly.size() {
            return false;
        }
        let t0 = self.begin();
        self.router.move_shape(s, poly, first_move);
        self.end(t0);
        true
    }

    fn curved_json(poly: &Polygon) -> String {
        let commands: Vec<String> = poly.ts.iter().map(|c| c.to_string()).collect();
        json::object(&[("points", json::points(&poly.ps)), ("commands", json::strings(&commands))])
    }
}

// ---------------------------------------------------------------------------
// JS API
// ---------------------------------------------------------------------------

#[wasm_bindgen]
impl AvoidRouter {
    /// `new Router(flags)`.  Throws unless `flags` is 1, 2 or 3 (default:
    /// `OrthogonalRouting`).
    #[wasm_bindgen(constructor)]
    pub fn new(flags: Option<f64>) -> Result<AvoidRouter, JsError> {
        let f = flags.unwrap_or(ORTHOGONAL_ROUTING as f64);
        as_u32(f)
            .and_then(AvoidRouter::create)
            .ok_or_else(|| JsError::new("AvoidRouter: flags must be RouterFlag.PolyLineRouting, RouterFlag.OrthogonalRouting or both"))
    }

    #[wasm_bindgen(js_name = withFlags)]
    pub fn with_flags(flags: f64) -> Result<AvoidRouter, JsError> {
        AvoidRouter::new(Some(flags))
    }

    /// The flags passed to the constructor.
    #[wasm_bindgen(js_name = routerFlags)]
    pub fn router_flags(&self) -> u32 {
        self.flags
    }

    /// Convenience: equivalent to deleting the router and constructing a
    /// new one with the same flags, routing parameters, routing options and
    /// transaction setting (all objects and ids are dropped).
    pub fn clear(&mut self) {
        let mut fresh = AvoidRouter::create(self.flags).expect("valid flags");
        for p in RoutingParameter::ALL {
            fresh.router.set_routing_parameter(p, self.router.routing_parameter(p));
        }
        for o in RoutingOption::ALL {
            fresh.router.set_routing_option(o, self.router.routing_option(o));
        }
        fresh.router.set_transaction_use(self.router.transaction_use());
        *self = fresh;
    }

    // --- transactions -------------------------------------------------------

    /// `Router::setTransactionUse()`.
    #[wasm_bindgen(js_name = setTransactionUse)]
    pub fn set_transaction_use(&mut self, transactions: bool) {
        self.router.set_transaction_use(transactions);
    }

    /// `Router::transactionUse()`.
    #[wasm_bindgen(js_name = transactionUse)]
    pub fn transaction_use(&self) -> bool {
        self.router.transaction_use()
    }

    /// `Router::processTransaction()`.
    #[wasm_bindgen(js_name = processTransaction)]
    pub fn process_transaction(&mut self) -> bool {
        let t0 = self.begin();
        let r = self.router.process_transaction();
        self.end(t0);
        r
    }

    /// Ids of the connectors whose upstream callback fired (route changed)
    /// in the last transaction (all transactions of the last call when
    /// transaction use is off).  JSON string array.
    #[wasm_bindgen(js_name = lastTransactionUpdatedConnectorIds)]
    pub fn last_transaction_updated_connector_ids(&self) -> String {
        json::strings(&self.last_updated)
    }

    // --- parameters ---------------------------------------------------------

    /// `Router::setRoutingParameter()`; omitted value =
    /// `chooseSensibleParamValue`.
    #[wasm_bindgen(js_name = setRoutingParameter)]
    pub fn set_routing_parameter(&mut self, parameter: f64, value: Option<f64>) -> bool {
        let Some(p) = as_enum(parameter, RoutingParameter::ALL.len()) else {
            return false;
        };
        let v = value.unwrap_or(CHOOSE_SENSIBLE_PARAM_VALUE);
        if !v.is_finite() {
            return false;
        }
        self.router.set_routing_parameter(RoutingParameter::ALL[p], v);
        true
    }

    /// `Router::setRoutingPenalty()`.
    #[wasm_bindgen(js_name = setRoutingPenalty)]
    pub fn set_routing_penalty(&mut self, penalty_type: f64, penalty_value: Option<f64>) -> bool {
        let Some(p) = as_enum(penalty_type, RoutingParameter::ALL.len()) else {
            return false;
        };
        let v = penalty_value.unwrap_or(CHOOSE_SENSIBLE_PARAM_VALUE);
        if !v.is_finite() {
            return false;
        }
        self.router.set_routing_penalty(RoutingParameter::ALL[p], v);
        true
    }

    /// `Router::routingParameter()`.
    #[wasm_bindgen(js_name = routingParameter)]
    pub fn routing_parameter(&self, parameter: f64) -> Option<f64> {
        as_enum(parameter, RoutingParameter::ALL.len()).map(|p| self.router.routing_parameter(RoutingParameter::ALL[p]))
    }

    /// `Router::setRoutingOption()`.
    #[wasm_bindgen(js_name = setRoutingOption)]
    pub fn set_routing_option(&mut self, option: f64, value: bool) -> bool {
        let Some(o) = as_enum(option, RoutingOption::ALL.len()) else {
            return false;
        };
        self.router.set_routing_option(RoutingOption::ALL[o], value);
        true
    }

    /// `Router::routingOption()`.
    #[wasm_bindgen(js_name = routingOption)]
    pub fn routing_option(&self, option: f64) -> Option<bool> {
        as_enum(option, RoutingOption::ALL.len()).map(|o| self.router.routing_option(RoutingOption::ALL[o]))
    }

    /// `Router::validConnType(select)`.
    #[wasm_bindgen(js_name = validConnType)]
    pub fn valid_conn_type(&self, select: Option<f64>) -> Option<u32> {
        let s = as_enum(select.unwrap_or(0.0), 3)?;
        Some(self.router.valid_conn_type(CONN_TYPES[s]) as u32)
    }

    // --- ids ----------------------------------------------------------------

    /// Whether a string id is free (unused by any shape, junction,
    /// connector or cluster known to this wrapper).
    #[wasm_bindgen(js_name = objectIdIsUnused)]
    pub fn object_id_is_unused(&self, id: String) -> bool {
        !self.objects.contains_key(&id)
    }

    /// `Router::objectIdIsUnused(id)` for an upstream numeric id.
    #[wasm_bindgen(js_name = numericObjectIdIsUnused)]
    pub fn numeric_object_id_is_unused(&self, id: f64) -> Option<bool> {
        as_u32(id).map(|n| self.router.object_id_is_unused(n))
    }

    /// `Router::newObjectId()`.
    #[wasm_bindgen(js_name = newObjectId)]
    pub fn new_object_id(&self) -> u32 {
        self.router.new_object_id()
    }

    /// The upstream numeric id (`id()`) of an object.
    #[wasm_bindgen(js_name = objectNumericId)]
    pub fn object_numeric_id(&self, id: String) -> Option<u32> {
        Some(match self.lookup(&id)? {
            Obj::Shape(o) | Obj::Junction(o) => self.router.obstacles[o].m_id,
            Obj::Conn(c) => self.router.conns[c].m_id,
            Obj::Cluster(c) => self.router.clusters[c].m_id,
        })
    }

    /// The (earliest created) object with the given numeric id; JSON
    /// string or `null`.
    #[wasm_bindgen(js_name = objectIdFromNumericId)]
    pub fn object_id_from_numeric_id(&self, numeric_id: f64) -> String {
        opt_json(as_u32(numeric_id).and_then(|n| self.name_for_numeric(n)).map(|n| json::string(&n)))
    }

    /// Ids of all shapes (creation order).  JSON array.
    #[wasm_bindgen(js_name = shapeIds)]
    pub fn shape_ids(&self) -> String {
        json::strings(&self.ids_of_kind(Kind::Shape))
    }

    #[wasm_bindgen(js_name = junctionIds)]
    pub fn junction_ids(&self) -> String {
        json::strings(&self.ids_of_kind(Kind::Junction))
    }

    #[wasm_bindgen(js_name = connectorIds)]
    pub fn connector_ids(&self) -> String {
        json::strings(&self.ids_of_kind(Kind::Conn))
    }

    #[wasm_bindgen(js_name = clusterIds)]
    pub fn cluster_ids(&self) -> String {
        json::strings(&self.ids_of_kind(Kind::Cluster))
    }

    // --- clusters -----------------------------------------------------------

    /// `new ClusterRef(router, poly, numericId)`.
    #[wasm_bindgen(js_name = addCluster)]
    pub fn add_cluster(&mut self, id: String, coordinates: Vec<f64>, numeric_id: Option<f64>) -> bool {
        let Some(poly) = polygon_from(&coordinates, 3) else {
            return false;
        };
        let Some(nid) = self.new_ids(&id, numeric_id) else {
            return false;
        };
        let t0 = self.begin();
        let c = ClusterRef::new(&mut self.router, &poly, nid);
        let serial = self.router.clusters[c].serial;
        self.register(id, Obj::Cluster(c), serial);
        self.end(t0);
        true
    }

    /// Convenience: `addCluster` with a `Rectangle`.
    #[wasm_bindgen(js_name = addClusterRectangle)]
    pub fn add_cluster_rectangle(
        &mut self,
        id: String,
        left: f64,
        top: f64,
        right: f64,
        bottom: f64,
        numeric_id: Option<f64>,
    ) -> bool {
        let Some(poly) = rect(left, top, right, bottom) else {
            return false;
        };
        let coords: Vec<f64> = poly.ps.iter().flat_map(|p| [p.x, p.y]).collect();
        self.add_cluster(id, coords, numeric_id)
    }

    /// `ClusterRef::setNewPoly()`.
    #[wasm_bindgen(js_name = setClusterPolygon)]
    pub fn set_cluster_polygon(&mut self, id: String, coordinates: Vec<f64>) -> bool {
        let (Some(c), Some(poly)) = (self.cluster(&id), polygon_from(&coordinates, 3)) else {
            return false;
        };
        c.set_new_poly(&mut self.router, &poly);
        true
    }

    #[wasm_bindgen(js_name = setClusterRectangle)]
    pub fn set_cluster_rectangle(&mut self, id: String, left: f64, top: f64, right: f64, bottom: f64) -> bool {
        let Some(poly) = rect(left, top, right, bottom) else {
            return false;
        };
        let coords: Vec<f64> = poly.ps.iter().flat_map(|p| [p.x, p.y]).collect();
        self.set_cluster_polygon(id, coords)
    }

    /// `Router::deleteCluster()`.
    #[wasm_bindgen(js_name = removeCluster)]
    pub fn remove_cluster(&mut self, id: String) -> bool {
        let Some(c) = self.cluster(&id) else {
            return false;
        };
        self.router.delete_cluster(c);
        self.unregister(&id);
        true
    }

    /// Convenience: bounding box of `ClusterRef::rectangularPolygon()`.
    #[wasm_bindgen(js_name = clusterBoundingBox)]
    pub fn cluster_bounding_box(&self, id: String) -> String {
        opt_json(self.cluster(&id).map(|c| json::bbox(&c.rectangular_polygon(&self.router).offset_bounding_box(0.0))))
    }

    /// `ClusterRef::polygon()`.
    #[wasm_bindgen(js_name = clusterPolygon)]
    pub fn cluster_polygon(&self, id: String) -> String {
        opt_json(self.cluster(&id).map(|c| {
            let poly = Polygon::from_interface(&self.router.clusters[c].polygon().view(&self.router));
            json::points(&poly.ps)
        }))
    }

    /// `ClusterRef::rectangularPolygon()`.
    #[wasm_bindgen(js_name = clusterRectangularPolygon)]
    pub fn cluster_rectangular_polygon(&self, id: String) -> String {
        opt_json(self.cluster(&id).map(|c| json::points(&c.rectangular_polygon(&self.router).ps)))
    }

    // --- shapes -------------------------------------------------------------

    /// `new ShapeRef(router, Rectangle(topLeft, bottomRight), numericId)`.
    #[wasm_bindgen(js_name = addShape)]
    pub fn add_shape(
        &mut self,
        id: String,
        left: f64,
        top: f64,
        right: f64,
        bottom: f64,
        numeric_id: Option<f64>,
    ) -> bool {
        let Some(poly) = rect(left, top, right, bottom) else {
            return false;
        };
        self.add_shape_poly(id, &poly, numeric_id)
    }

    /// `new ShapeRef(router, poly, numericId)` (at least 3 points).
    #[wasm_bindgen(js_name = addPolygonShape)]
    pub fn add_polygon_shape(&mut self, id: String, coordinates: Vec<f64>, numeric_id: Option<f64>) -> bool {
        let Some(poly) = polygon_from(&coordinates, 3) else {
            return false;
        };
        self.add_shape_poly(id, &poly, numeric_id)
    }


    /// `Router::moveShape(shape, Rectangle(..))` (shape must have 4 points).
    #[wasm_bindgen(js_name = moveShape)]
    pub fn move_shape(&mut self, id: String, left: f64, top: f64, right: f64, bottom: f64) -> bool {
        let Some(poly) = rect(left, top, right, bottom) else {
            return false;
        };
        self.move_shape_poly(&id, &poly, false)
    }

    /// `Router::moveShape(shape, newPoly, firstMove)` (same point count).
    #[wasm_bindgen(js_name = movePolygonShape)]
    pub fn move_polygon_shape(&mut self, id: String, coordinates: Vec<f64>, first_move: Option<bool>) -> bool {
        let Some(poly) = polygon_from(&coordinates, 3) else {
            return false;
        };
        self.move_shape_poly(&id, &poly, first_move.unwrap_or(false))
    }


    /// `Router::moveShape(shape, xDiff, yDiff)`.
    #[wasm_bindgen(js_name = moveShapeBy)]
    pub fn move_shape_by(&mut self, id: String, x_diff: f64, y_diff: f64) -> bool {
        let Some(s) = self.shape(&id) else {
            return false;
        };
        if !finite(&[x_diff, y_diff]) {
            return false;
        }
        let t0 = self.begin();
        self.router.move_shape_by(s, x_diff, y_diff);
        self.end(t0);
        true
    }

    /// Convenience: `polygon().offsetBoundingBox(0)`.
    #[wasm_bindgen(js_name = shapeBoundingBox)]
    pub fn shape_bounding_box(&self, id: String) -> String {
        opt_json(self.shape(&id).map(|s| json::bbox(&s.polygon(&self.router).offset_bounding_box(0.0))))
    }

    /// `Obstacle::routingBox()` (includes `shapeBufferDistance`).
    #[wasm_bindgen(js_name = shapeRoutingBox)]
    pub fn shape_routing_box(&self, id: String) -> String {
        opt_json(self.shape(&id).map(|s| json::bbox(&s.routing_box(&self.router))))
    }

    /// `ShapeRef::polygon()`.
    #[wasm_bindgen(js_name = shapePolygon)]
    pub fn shape_polygon(&self, id: String) -> String {
        opt_json(self.shape(&id).map(|s| json::points(&s.polygon(&self.router).ps)))
    }

    /// `Obstacle::routingPolygon()`.
    #[wasm_bindgen(js_name = shapeRoutingPolygon)]
    pub fn shape_routing_polygon(&self, id: String) -> String {
        opt_json(self.shape(&id).map(|s| json::points(&s.routing_polygon(&self.router).ps)))
    }

    /// `ShapeRef::position()`.
    #[wasm_bindgen(js_name = shapePosition)]
    pub fn shape_position(&self, id: String) -> String {
        opt_json(self.shape(&id).map(|s| json::point(&s.position(&self.router))))
    }

    /// `Router::shapeContainingPoint()`; JSON string id or `null`.
    #[wasm_bindgen(js_name = shapeContainingPoint)]
    pub fn shape_containing_point(&self, x: f64, y: f64) -> String {
        if !finite(&[x, y]) {
            return "null".into();
        }
        opt_json(
            self.router
                .shape_containing_point(&Point::new(x, y))
                .and_then(|s| self.obstacle_name(s.0))
                .map(|n| json::string(&n)),
        )
    }

    /// `Obstacle::attachedConnectors()` of a shape.
    #[wasm_bindgen(js_name = shapeAttachedConnectors)]
    pub fn shape_attached_connectors(&self, id: String) -> String {
        opt_json(self.shape(&id).map(|s| json::strings(&self.conn_names(&s.attached_connectors(&self.router)))))
    }

    /// `Router::attachedShapes(shapes, shapeId, type)` (also accepts a
    /// junction id).
    #[wasm_bindgen(js_name = shapeAttachedShapes)]
    pub fn shape_attached_shapes(&self, id: String, relation: Option<f64>) -> String {
        let (Some(Obj::Shape(o)) | Some(Obj::Junction(o))) = self.lookup(&id) else {
            return "null".into();
        };
        let Some(t) = as_u32(relation.unwrap_or(3.0)).filter(|t| *t <= 3) else {
            return "null".into();
        };
        let mut out = Vec::new();
        self.router.attached_shapes(&mut out, self.router.obstacles[o].m_id, t);
        json::strings(&self.numeric_names(&out))
    }

    /// `Router::attachedConns(conns, shapeId, type)` (also accepts a
    /// junction id).
    #[wasm_bindgen(js_name = shapeAttachedConns)]
    pub fn shape_attached_conns(&self, id: String, relation: Option<f64>) -> String {
        let (Some(Obj::Shape(o)) | Some(Obj::Junction(o))) = self.lookup(&id) else {
            return "null".into();
        };
        let Some(t) = as_u32(relation.unwrap_or(3.0)).filter(|t| *t <= 3) else {
            return "null".into();
        };
        let mut out = Vec::new();
        self.router.attached_conns(&mut out, self.router.obstacles[o].m_id, t);
        json::strings(&self.numeric_names(&out))
    }

    /// `ShapeRef::transformConnectionPinPositions()`.
    #[wasm_bindgen(js_name = transformShapeConnectionPinPositions)]
    pub fn transform_shape_connection_pin_positions(&mut self, id: String, transform: f64) -> bool {
        let (Some(s), Some(t)) = (self.shape(&id), as_enum(transform, TRANSFORMS.len())) else {
            return false;
        };
        let t0 = self.begin();
        s.transform_connection_pin_positions(&mut self.router, TRANSFORMS[t]);
        self.end(t0);
        true
    }

    /// `Router::deleteShape()`.  Refused (false) while the shape's own
    /// addition is still queued (upstream asserts against that) or while a
    /// hyperedge registered for rerouting references it.
    #[wasm_bindgen(js_name = removeShape)]
    pub fn remove_shape(&mut self, id: String) -> bool {
        let Some(s) = self.shape(&id) else {
            return false;
        };
        if self.has_pending_action(s.0, ActionType::ShapeAdd) || self.hyperedge_referenced(s.0) {
            return false;
        }
        let t0 = self.begin();
        self.unregister(&id);
        self.router.delete_shape(s);
        self.end(t0);
        true
    }

    /// `Router::markAllObstaclesAsMoved()`.
    #[wasm_bindgen(js_name = markAllObstaclesAsMoved)]
    pub fn mark_all_obstacles_as_moved(&mut self) {
        let t0 = self.begin();
        self.router.mark_all_obstacles_as_moved();
        self.end(t0);
    }

    // --- junctions ----------------------------------------------------------

    /// `new JunctionRef(router, Point(x, y), numericId)`.
    #[wasm_bindgen(js_name = addJunction)]
    pub fn add_junction(&mut self, id: String, x: f64, y: f64, numeric_id: Option<f64>) -> bool {
        if !finite(&[x, y]) {
            return false;
        }
        let Some(nid) = self.new_ids(&id, numeric_id) else {
            return false;
        };
        let t0 = self.begin();
        let j = JunctionRef::new(&mut self.router, Point::new(x, y), nid);
        let serial = self.router.obstacles[j.0].serial;
        self.register(id, Obj::Junction(j.0), serial);
        self.end(t0);
        true
    }

    /// `Router::deleteJunction()` (same restrictions as `removeShape`).
    #[wasm_bindgen(js_name = removeJunction)]
    pub fn remove_junction(&mut self, id: String) -> bool {
        let Some(j) = self.junction(&id) else {
            return false;
        };
        if self.has_pending_action(j.0, ActionType::JunctionAdd) || self.hyperedge_referenced(j.0) {
            return false;
        }
        let t0 = self.begin();
        self.unregister(&id);
        self.router.delete_junction(j);
        self.end(t0);
        true
    }

    /// `JunctionRef::removeJunctionAndMergeConnectors()`; returns the
    /// merged connector's id (JSON string) or `null`.
    #[wasm_bindgen(js_name = removeJunctionAndMergeConnectors)]
    pub fn remove_junction_and_merge_connectors(&mut self, id: String) -> String {
        let Some(j) = self.junction(&id) else {
            return "null".into();
        };
        if self.has_pending_action(j.0, ActionType::JunctionAdd) || self.hyperedge_referenced(j.0) {
            return "null".into();
        }
        let before: Vec<(ConnIdx, String)> = self.by_conn.iter().map(|(c, (_, n))| (*c, n.clone())).collect();
        let t0 = self.begin();
        let merged = j.remove_junction_and_merge_connectors(&mut self.router);
        if merged.is_some() {
            self.unregister(&id);
            // Forget the connector the core deleted.
            for (c, name) in before {
                let alive = self.router.conns.get(c).map(|x| x.serial) == self.by_conn.get(&c).map(|e| e.0);
                if !alive {
                    self.unregister(&name);
                }
            }
        }
        let name = merged.and_then(|c| self.conn_name(c));
        self.end(t0);
        opt_json(name.map(|n| json::string(&n)))
    }

    /// `Router::moveJunction(junction, Point)`.
    #[wasm_bindgen(js_name = moveJunction)]
    pub fn move_junction(&mut self, id: String, x: f64, y: f64) -> bool {
        let Some(j) = self.junction(&id) else {
            return false;
        };
        if !finite(&[x, y]) {
            return false;
        }
        let t0 = self.begin();
        self.router.move_junction(j, &Point::new(x, y));
        self.end(t0);
        true
    }

    /// `Router::moveJunction(junction, xDiff, yDiff)`.
    #[wasm_bindgen(js_name = moveJunctionBy)]
    pub fn move_junction_by(&mut self, id: String, x_diff: f64, y_diff: f64) -> bool {
        let Some(j) = self.junction(&id) else {
            return false;
        };
        if !finite(&[x_diff, y_diff]) {
            return false;
        }
        let t0 = self.begin();
        self.router.move_junction_by(j, x_diff, y_diff);
        self.end(t0);
        true
    }

    /// `JunctionRef::position()`.
    #[wasm_bindgen(js_name = junctionPosition)]
    pub fn junction_position(&self, id: String) -> String {
        opt_json(self.junction(&id).map(|j| json::point(&j.position(&self.router))))
    }

    /// `JunctionRef::recommendedPosition()`.
    #[wasm_bindgen(js_name = junctionRecommendedPosition)]
    pub fn junction_recommended_position(&self, id: String) -> String {
        opt_json(self.junction(&id).map(|j| json::point(&j.recommended_position(&self.router))))
    }

    /// `JunctionRef::setPositionFixed()`.
    #[wasm_bindgen(js_name = setJunctionPositionFixed)]
    pub fn set_junction_position_fixed(&mut self, id: String, fixed: bool) -> bool {
        let Some(j) = self.junction(&id) else {
            return false;
        };
        j.set_position_fixed(&mut self.router, fixed);
        true
    }

    /// `JunctionRef::positionFixed()`.
    #[wasm_bindgen(js_name = junctionPositionFixed)]
    pub fn junction_position_fixed(&self, id: String) -> Option<bool> {
        self.junction(&id).map(|j| j.position_fixed(&self.router))
    }

    /// `JunctionRef::preferOrthogonalDimension(dim)` (0 = XDIM, 1 = YDIM).
    #[wasm_bindgen(js_name = preferJunctionOrthogonalDimension)]
    pub fn prefer_junction_orthogonal_dimension(&mut self, id: String, dimension: f64) -> bool {
        let (Some(j), Some(d)) = (self.junction(&id), as_enum(dimension, 2)) else {
            return false;
        };
        j.prefer_orthogonal_dimension(&mut self.router, d);
        true
    }

    /// `Obstacle::attachedConnectors()` of a junction.
    #[wasm_bindgen(js_name = junctionAttachedConnectors)]
    pub fn junction_attached_connectors(&self, id: String) -> String {
        opt_json(self.junction(&id).map(|j| json::strings(&self.conn_names(&j.attached_connectors(&self.router)))))
    }

    // --- connection pins ----------------------------------------------------

    /// `new ShapeConnectionPin(shape, classId, xOffset, yOffset,
    /// proportional, insideOffset, visDirs)`, named `pinId` on the shape.
    #[wasm_bindgen(js_name = addShapeConnectionPin)]
    #[allow(clippy::too_many_arguments)]
    pub fn add_shape_connection_pin(
        &mut self,
        shape_id: String,
        pin_id: String,
        class_id: f64,
        x_offset: f64,
        y_offset: f64,
        proportional: Option<bool>,
        inside_offset: Option<f64>,
        visibility_directions: Option<f64>,
    ) -> bool {
        self.add_shape_connection_pin_with_options(
            shape_id,
            pin_id,
            class_id,
            x_offset,
            y_offset,
            proportional,
            inside_offset,
            visibility_directions,
            None,
            None,
        )
    }

    /// As `addShapeConnectionPin`, then optionally `setConnectionCost()` /
    /// `setExclusive()` (omitted = upstream defaults).
    #[wasm_bindgen(js_name = addShapeConnectionPinWithOptions)]
    #[allow(clippy::too_many_arguments)]
    pub fn add_shape_connection_pin_with_options(
        &mut self,
        shape_id: String,
        pin_id: String,
        class_id: f64,
        x_offset: f64,
        y_offset: f64,
        proportional: Option<bool>,
        inside_offset: Option<f64>,
        visibility_directions: Option<f64>,
        connection_cost: Option<f64>,
        exclusive: Option<bool>,
    ) -> bool {
        let Some(s) = self.shape(&shape_id) else {
            return false;
        };
        let inside = inside_offset.unwrap_or(0.0);
        let (Some(class), Some(dirs)) = (as_class(class_id), as_dirs(visibility_directions.unwrap_or(0.0))) else {
            return false;
        };
        if !finite(&[x_offset, y_offset, inside]) {
            return false;
        }
        if let Some(c) = connection_cost {
            if !(c.is_finite() && c >= 0.0) {
                return false;
            }
        }
        let proportional = proportional.unwrap_or(false);
        let ok = self.add_pin_internal(&shape_id, &pin_id, |r| {
            let p = ShapeConnectionPin::new_shape(r, s, class, x_offset, y_offset, proportional, inside, dirs);
            if let Some(c) = connection_cost {
                p.set_connection_cost(r, c);
            }
            if let Some(e) = exclusive {
                p.set_exclusive(r, e);
            }
            p
        });
        ok
    }

    /// `delete pin`.
    #[wasm_bindgen(js_name = removeShapeConnectionPin)]
    pub fn remove_shape_connection_pin(&mut self, shape_id: String, pin_id: String) -> bool {
        if self.shape(&shape_id).is_none() {
            return false;
        }
        self.remove_pin_internal(&shape_id, &pin_id)
    }

    /// Ids of the named pins of a shape (creation order).
    #[wasm_bindgen(js_name = shapeConnectionPinIdList)]
    pub fn shape_connection_pin_id_list(&self, shape_id: String) -> String {
        if self.shape(&shape_id).is_none() {
            return "null".into();
        }
        let names: Vec<&str> = self.pins.get(&shape_id).map(|v| v.iter().map(|p| p.name.as_str()).collect()).unwrap_or_default();
        json::strings(&names)
    }

    /// `ShapeConnectionPin::setConnectionCost()` (cost >= 0).
    #[wasm_bindgen(js_name = setShapeConnectionPinConnectionCost)]
    pub fn set_shape_connection_pin_connection_cost(&mut self, shape_id: String, pin_id: String, cost: f64) -> bool {
        let pin = self.shape_pin(&shape_id, &pin_id);
        self.set_pin_cost(pin, cost)
    }

    /// `ShapeConnectionPin::setExclusive()`.
    #[wasm_bindgen(js_name = setShapeConnectionPinExclusive)]
    pub fn set_shape_connection_pin_exclusive(&mut self, shape_id: String, pin_id: String, exclusive: bool) -> bool {
        let pin = self.shape_pin(&shape_id, &pin_id);
        self.set_pin_exclusive(pin, exclusive)
    }

    /// `ShapeConnectionPin::isExclusive()`.
    #[wasm_bindgen(js_name = shapeConnectionPinExclusive)]
    pub fn shape_connection_pin_exclusive(&self, shape_id: String, pin_id: String) -> Option<bool> {
        self.shape_pin(&shape_id, &pin_id).map(|p| p.is_exclusive(&self.router))
    }

    /// `ShapeConnectionPin::position()`.
    #[wasm_bindgen(js_name = shapeConnectionPinPosition)]
    pub fn shape_connection_pin_position(&self, shape_id: String, pin_id: String) -> String {
        self.pin_position_json(self.shape_pin(&shape_id, &pin_id))
    }

    /// `ShapeConnectionPin::directions()`.
    #[wasm_bindgen(js_name = shapeConnectionPinDirections)]
    pub fn shape_connection_pin_directions(&self, shape_id: String, pin_id: String) -> Option<u32> {
        self.shape_pin(&shape_id, &pin_id).map(|p| p.directions(&self.router))
    }

    /// `ShapeConnectionPin::ids()`.
    #[wasm_bindgen(js_name = shapeConnectionPinIds)]
    pub fn shape_connection_pin_ids(&self, shape_id: String, pin_id: String) -> String {
        self.pin_ids_json(&shape_id, self.shape_pin(&shape_id, &pin_id))
    }

    /// `new ShapeConnectionPin(junction, classId, visDirs)`.
    #[wasm_bindgen(js_name = addJunctionConnectionPin)]
    pub fn add_junction_connection_pin(
        &mut self,
        junction_id: String,
        pin_id: String,
        class_id: f64,
        visibility_directions: Option<f64>,
    ) -> bool {
        let Some(j) = self.junction(&junction_id) else {
            return false;
        };
        let (Some(class), Some(dirs)) = (as_class(class_id), as_dirs(visibility_directions.unwrap_or(0.0))) else {
            return false;
        };
        self.add_pin_internal(&junction_id, &pin_id, |r| ShapeConnectionPin::new_junction(r, j, class, dirs))
    }

    /// `delete pin`.
    #[wasm_bindgen(js_name = removeJunctionConnectionPin)]
    pub fn remove_junction_connection_pin(&mut self, junction_id: String, pin_id: String) -> bool {
        if self.junction(&junction_id).is_none() {
            return false;
        }
        self.remove_pin_internal(&junction_id, &pin_id)
    }

    #[wasm_bindgen(js_name = junctionConnectionPinPosition)]
    pub fn junction_connection_pin_position(&self, junction_id: String, pin_id: String) -> String {
        self.pin_position_json(self.junction_pin(&junction_id, &pin_id))
    }

    #[wasm_bindgen(js_name = junctionConnectionPinDirections)]
    pub fn junction_connection_pin_directions(&self, junction_id: String, pin_id: String) -> Option<u32> {
        self.junction_pin(&junction_id, &pin_id).map(|p| p.directions(&self.router))
    }

    #[wasm_bindgen(js_name = junctionConnectionPinIds)]
    pub fn junction_connection_pin_ids(&self, junction_id: String, pin_id: String) -> String {
        self.pin_ids_json(&junction_id, self.junction_pin(&junction_id, &pin_id))
    }

    #[wasm_bindgen(js_name = setJunctionConnectionPinConnectionCost)]
    pub fn set_junction_connection_pin_connection_cost(&mut self, junction_id: String, pin_id: String, cost: f64) -> bool {
        let pin = self.junction_pin(&junction_id, &pin_id);
        self.set_pin_cost(pin, cost)
    }

    #[wasm_bindgen(js_name = setJunctionConnectionPinExclusive)]
    pub fn set_junction_connection_pin_exclusive(&mut self, junction_id: String, pin_id: String, exclusive: bool) -> bool {
        let pin = self.junction_pin(&junction_id, &pin_id);
        self.set_pin_exclusive(pin, exclusive)
    }

    #[wasm_bindgen(js_name = junctionConnectionPinExclusive)]
    pub fn junction_connection_pin_exclusive(&self, junction_id: String, pin_id: String) -> Option<bool> {
        self.junction_pin(&junction_id, &pin_id).map(|p| p.is_exclusive(&self.router))
    }

    // --- connectors: creation -----------------------------------------------

    /// `new ConnRef(router, numericId)` (no endpoints yet).
    #[wasm_bindgen(js_name = addConnectorWithoutEndpoints)]
    pub fn add_connector_without_endpoints(&mut self, id: String, numeric_id: Option<f64>) -> bool {
        self.add_conn_internal(&id, None, numeric_id)
    }

    /// `new ConnRef(router, src, dst, numericId)` with raw ConnEnds.
    #[wasm_bindgen(js_name = addConnectorWithConnEnds)]
    #[allow(clippy::too_many_arguments)]
    pub fn add_connector_with_conn_ends(
        &mut self,
        id: String,
        src_kind: f64,
        src_object: String,
        src_pin: String,
        src_x: f64,
        src_y: f64,
        src_num: f64,
        dst_kind: f64,
        dst_object: String,
        dst_pin: String,
        dst_x: f64,
        dst_y: f64,
        dst_num: f64,
        numeric_id: Option<f64>,
    ) -> bool {
        let s = self.conn_end(RawEnd { kind: src_kind, object_id: &src_object, pin_id: &src_pin, x: src_x, y: src_y, num: src_num });
        let d = self.conn_end(RawEnd { kind: dst_kind, object_id: &dst_object, pin_id: &dst_pin, x: dst_x, y: dst_y, num: dst_num });
        match (s, d) {
            (Some(s), Some(d)) => self.add_conn_internal(&id, Some((s, d)), numeric_id),
            _ => false,
        }
    }

    /// Convenience: `ConnRef(router, ConnEnd(Point), ConnEnd(Point))`.
    #[wasm_bindgen(js_name = addConnector)]
    pub fn add_connector(&mut self, id: String, src_x: f64, src_y: f64, dst_x: f64, dst_y: f64, numeric_id: Option<f64>) -> bool {
        self.add_connector_with_conn_ends(
            id, 0.0, String::new(), String::new(), src_x, src_y, CONN_DIR_ALL as f64, 0.0, String::new(), String::new(), dst_x, dst_y,
            CONN_DIR_ALL as f64, numeric_id,
        )
    }

    // --- connectors: endpoints ----------------------------------------------

    /// `ConnRef::setEndpoints(src, dst)` with raw ConnEnds.
    #[wasm_bindgen(js_name = setConnectorConnEnds)]
    #[allow(clippy::too_many_arguments)]
    pub fn set_connector_conn_ends(
        &mut self,
        id: String,
        src_kind: f64,
        src_object: String,
        src_pin: String,
        src_x: f64,
        src_y: f64,
        src_num: f64,
        dst_kind: f64,
        dst_object: String,
        dst_pin: String,
        dst_x: f64,
        dst_y: f64,
        dst_num: f64,
    ) -> bool {
        let s = self.conn_end(RawEnd { kind: src_kind, object_id: &src_object, pin_id: &src_pin, x: src_x, y: src_y, num: src_num });
        let d = self.conn_end(RawEnd { kind: dst_kind, object_id: &dst_object, pin_id: &dst_pin, x: dst_x, y: dst_y, num: dst_num });
        match (s, d) {
            (Some(s), Some(d)) => self.set_end_internal(&id, Some(s), Some(d)),
            _ => false,
        }
    }

    /// `ConnRef::setSourceEndpoint(end)` with a raw ConnEnd.
    #[wasm_bindgen(js_name = setConnectorSourceConnEnd)]
    pub fn set_connector_source_conn_end(&mut self, id: String, kind: f64, object_id: String, pin_id: String, x: f64, y: f64, num: f64) -> bool {
        match self.conn_end(RawEnd { kind, object_id: &object_id, pin_id: &pin_id, x, y, num }) {
            Some(e) => self.set_end_internal(&id, Some(e), None),
            None => false,
        }
    }

    /// `ConnRef::setDestEndpoint(end)` with a raw ConnEnd.
    #[wasm_bindgen(js_name = setConnectorDestConnEnd)]
    pub fn set_connector_dest_conn_end(&mut self, id: String, kind: f64, object_id: String, pin_id: String, x: f64, y: f64, num: f64) -> bool {
        match self.conn_end(RawEnd { kind, object_id: &object_id, pin_id: &pin_id, x, y, num }) {
            Some(e) => self.set_end_internal(&id, None, Some(e)),
            None => false,
        }
    }

    /// `Router::deleteConnector()`.
    #[wasm_bindgen(js_name = removeConnector)]
    pub fn remove_connector(&mut self, id: String) -> bool {
        let Some(c) = self.conn(&id) else {
            return false;
        };
        let t0 = self.begin();
        self.unregister(&id);
        self.router.delete_connector(c);
        self.last_updated.retain(|n| *n != id);
        self.end(t0);
        true
    }

    // --- connectors: properties ---------------------------------------------

    /// `ConnRef::setRoutingType()`.
    #[wasm_bindgen(js_name = setConnectorRoutingType)]
    pub fn set_connector_routing_type(&mut self, id: String, routing_type: f64) -> bool {
        let (Some(c), Some(t)) = (self.conn(&id), as_enum(routing_type, 3)) else {
            return false;
        };
        let t0 = self.begin();
        c.set_routing_type(&mut self.router, CONN_TYPES[t]);
        self.end(t0);
        true
    }

    /// `ConnRef::routingType()`.
    #[wasm_bindgen(js_name = connectorRoutingType)]
    pub fn connector_routing_type(&self, id: String) -> Option<u32> {
        self.conn(&id).map(|c| c.routing_type(&self.router) as u32)
    }

    /// `ConnRef::setHateCrossings()`.
    #[wasm_bindgen(js_name = setConnectorHateCrossings)]
    pub fn set_connector_hate_crossings(&mut self, id: String, value: bool) -> bool {
        let Some(c) = self.conn(&id) else {
            return false;
        };
        c.set_hate_crossings(&mut self.router, value);
        true
    }

    /// `ConnRef::doesHateCrossings()`.
    #[wasm_bindgen(js_name = connectorDoesHateCrossings)]
    pub fn connector_does_hate_crossings(&self, id: String) -> Option<bool> {
        self.conn(&id).map(|c| c.does_hate_crossings(&self.router))
    }

    /// `ConnRef::endpointConnEnds()`; `{source, destination}`.
    #[wasm_bindgen(js_name = connectorEndpointConnEnds)]
    pub fn connector_endpoint_conn_ends(&self, id: String) -> String {
        opt_json(self.conn(&id).map(|c| {
            let (s, d) = c.endpoint_conn_ends(&self.router);
            json::object(&[("source", self.conn_end_json(&s)), ("destination", self.conn_end_json(&d))])
        }))
    }

    /// `ConnRef::possibleDstPinPoints()`.
    #[wasm_bindgen(js_name = connectorPossibleDstPinPoints)]
    pub fn connector_possible_dst_pin_points(&self, id: String) -> String {
        opt_json(self.conn(&id).map(|c| json::points(&c.possible_dst_pin_points(&self.router))))
    }

    /// `ConnRef::setRoutingCheckpoints()`; flat `[x, y, arrivalDirs,
    /// departureDirs, ...]`.
    #[wasm_bindgen(js_name = setConnectorRoutingCheckpoints)]
    pub fn set_connector_routing_checkpoints(&mut self, id: String, checkpoints: Vec<f64>) -> bool {
        let Some(c) = self.conn(&id) else {
            return false;
        };
        if checkpoints.len() % 4 != 0 {
            return false;
        }
        let mut cps = Vec::new();
        for q in checkpoints.chunks(4) {
            let (Some(ad), Some(dd)) = (as_dirs(q[2]), as_dirs(q[3])) else {
                return false;
            };
            if !finite(&q[0..2]) {
                return false;
            }
            cps.push(Checkpoint::with_directions(Point::new(q[0], q[1]), ad, dd));
        }
        let t0 = self.begin();
        c.set_routing_checkpoints(&mut self.router, &cps);
        self.end(t0);
        true
    }

    /// `ConnRef::routingCheckpoints()`.
    #[wasm_bindgen(js_name = connectorRoutingCheckpoints)]
    pub fn connector_routing_checkpoints(&self, id: String) -> String {
        opt_json(self.conn(&id).map(|c| {
            let items: Vec<String> = c
                .routing_checkpoints(&self.router)
                .iter()
                .map(|cp| {
                    json::object(&[
                        ("point", json::point(&cp.point)),
                        ("arrivalDirections", json::number(cp.arrival_directions as f64)),
                        ("departureDirections", json::number(cp.departure_directions as f64)),
                    ])
                })
                .collect();
            format!("[{}]", items.join(","))
        }))
    }

    /// `ConnRef::setFixedRoute()`.
    #[wasm_bindgen(js_name = setConnectorFixedRoute)]
    pub fn set_connector_fixed_route(&mut self, id: String, route: Vec<f64>) -> bool {
        let (Some(c), Some(ps)) = (self.conn(&id), point_list(&route)) else {
            return false;
        };
        let mut poly = Polygon::new();
        poly.ps = ps;
        let t0 = self.begin();
        c.set_fixed_route(&mut self.router, &poly);
        self.end(t0);
        true
    }

    /// `ConnRef::setFixedExistingRoute()` (false if the connector has no
    /// route of at least 2 points, which upstream asserts).
    #[wasm_bindgen(js_name = setConnectorFixedExistingRoute)]
    pub fn set_connector_fixed_existing_route(&mut self, id: String) -> bool {
        let Some(c) = self.conn(&id) else {
            return false;
        };
        if c.route(&self.router).size() < 2 {
            return false;
        }
        c.set_fixed_existing_route(&mut self.router);
        true
    }

    /// `ConnRef::hasFixedRoute()`.
    #[wasm_bindgen(js_name = connectorHasFixedRoute)]
    pub fn connector_has_fixed_route(&self, id: String) -> Option<bool> {
        self.conn(&id).map(|c| c.has_fixed_route(&self.router))
    }

    /// `ConnRef::clearFixedRoute()`.
    #[wasm_bindgen(js_name = clearConnectorFixedRoute)]
    pub fn clear_connector_fixed_route(&mut self, id: String) -> bool {
        let Some(c) = self.conn(&id) else {
            return false;
        };
        c.clear_fixed_route(&mut self.router);
        true
    }

    /// `ConnRef::makePathInvalid()`.
    #[wasm_bindgen(js_name = makeConnectorPathInvalid)]
    pub fn make_connector_path_invalid(&mut self, id: String) -> bool {
        let Some(c) = self.conn(&id) else {
            return false;
        };
        self.router.conns[c].make_path_invalid();
        true
    }

    /// `ConnRef::needsRepaint()`.
    #[wasm_bindgen(js_name = connectorNeedsRepaint)]
    pub fn connector_needs_repaint(&self, id: String) -> Option<bool> {
        self.conn(&id).map(|c| c.needs_repaint(&self.router))
    }

    /// `ConnRef::route()` (raw route).
    #[wasm_bindgen(js_name = routeConnector)]
    pub fn route_connector(&mut self, id: String) -> String {
        self.route_json(&id, false)
    }

    /// `ConnRef::displayRoute()`.
    #[wasm_bindgen(js_name = displayConnectorRoute)]
    pub fn display_connector_route(&mut self, id: String) -> String {
        self.route_json(&id, true)
    }

    /// `displayRoute().curvedPolyline(curveAmount, closed)`;
    /// `{points, commands}`.
    #[wasm_bindgen(js_name = connectorCurvedRoute)]
    pub fn connector_curved_route(&mut self, id: String, curve_amount: f64, closed: Option<bool>) -> String {
        let Some(c) = self.conn(&id) else {
            return "null".into();
        };
        if !curve_amount.is_finite() {
            return "null".into();
        }
        let curved = c.display_route(&mut self.router).curved_polyline(curve_amount, closed.unwrap_or(false));
        AvoidRouter::curved_json(&curved)
    }

    /// `ConnRef::splitAtSegment(segmentN)`; the new junction and connector
    /// get the given string ids.  `null` if upstream would create nothing
    /// (`segmentN >= displayRoute().size()`), for `segmentN == 0`, or if the
    /// destination is a free point (upstream dereferences a null ConnEnd).
    #[wasm_bindgen(js_name = splitConnectorAtSegment)]
    pub fn split_connector_at_segment(&mut self, id: String, segment_n: f64, junction_id: String, connector_id: String) -> String {
        let (Some(c), Some(n)) = (self.conn(&id), as_u32(segment_n)) else {
            return "null".into();
        };
        let n = n as usize;
        if n == 0
            || junction_id == connector_id
            || self.objects.contains_key(&junction_id)
            || self.objects.contains_key(&connector_id)
            || self.router.conns[c].m_display_route.size() <= n
            || self.router.conns[c].m_dst_connend.is_none()
            || self.router.new_object_id() == 0
        {
            return "null".into();
        }
        let t0 = self.begin();
        // splitAtSegment() uses the two-ConnEnd ConnRef constructor: queue
        // it with transaction use off (see `add_conn_internal`).
        let immediate = !self.router.transaction_use();
        if immediate {
            self.router.set_transaction_use(true);
        }
        let (j, nc) = c.split_at_segment(&mut self.router, n);
        let mut out = "null".to_string();
        if let (Some(j), Some(nc)) = (j, nc) {
            let js = self.router.obstacles[j.0].serial;
            self.register(junction_id.clone(), Obj::Junction(j.0), js);
            let cs = self.router.conns[nc].serial;
            self.register(connector_id.clone(), Obj::Conn(nc), cs);
            self.set_conn_callback(nc);
            out = json::object(&[
                ("junctionId", json::string(&junction_id)),
                ("connectorId", json::string(&connector_id)),
                ("position", json::point(&j.position(&self.router))),
            ]);
        }
        if immediate {
            self.router.set_transaction_use(false);
            self.router.process_transaction();
        }
        self.end(t0);
        out
    }

    /// Convenience: `processTransaction()`, then `{id: displayRoute()}` for
    /// every connector.
    #[wasm_bindgen(js_name = routeAll)]
    pub fn route_all(&mut self) -> String {
        self.process_transaction();
        let ids = self.ids_of_kind(Kind::Conn);
        let fields: Vec<(String, String)> = ids.iter().map(|id| (id.clone(), self.route_json(id, true))).collect();
        let refs: Vec<(&str, String)> = fields.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        json::object(&refs)
    }

    // --- diagnostics --------------------------------------------------------

    /// `Router::existsInvalidOrthogonalPaths()`.
    #[wasm_bindgen(js_name = existsInvalidOrthogonalPaths)]
    pub fn exists_invalid_orthogonal_paths(&mut self) -> bool {
        self.router.exists_invalid_orthogonal_paths()
    }

    /// `Router::existsCrossings(optimisedForConnectorType)`.
    #[wasm_bindgen(js_name = existsCrossings)]
    pub fn exists_crossings(&mut self, optimised_for_connector_type: Option<bool>) -> i32 {
        self.router.exists_crossings(optimised_for_connector_type.unwrap_or(false))
    }

    /// `Router::existsOrthogonalSegmentOverlap(atEnds)`.
    #[wasm_bindgen(js_name = existsOrthogonalSegmentOverlap)]
    pub fn exists_orthogonal_segment_overlap(&mut self, at_ends: Option<bool>) -> bool {
        self.router.exists_orthogonal_segment_overlap(at_ends.unwrap_or(false))
    }

    /// `Router::existsOrthogonalFixedSegmentOverlap(atEnds)`.
    #[wasm_bindgen(js_name = existsOrthogonalFixedSegmentOverlap)]
    pub fn exists_orthogonal_fixed_segment_overlap(&mut self, at_ends: Option<bool>) -> bool {
        self.router.exists_orthogonal_fixed_segment_overlap(at_ends.unwrap_or(false))
    }

    /// `Router::existsOrthogonalTouchingPaths()`.
    #[wasm_bindgen(js_name = existsOrthogonalTouchingPaths)]
    pub fn exists_orthogonal_touching_paths(&mut self) -> bool {
        self.router.exists_orthogonal_touching_paths()
    }

    /// `Router::outputDiagramText()` as a string (no file is written).
    /// `fullPrecision` selects the oracle format used by the golden files
    /// (`%.17g` and junction blocks) instead of upstream's `%g`.
    #[wasm_bindgen(js_name = outputDiagramText)]
    pub fn output_diagram_text(&mut self, full_precision: Option<bool>) -> String {
        if full_precision.unwrap_or(false) {
            self.router.output_diagram_text()
        } else {
            self.router.output_diagram_text_g()
        }
    }

    /// `Router::printInfo()` as a string.
    #[wasm_bindgen(js_name = printInfo)]
    pub fn print_info(&self) -> String {
        self.router.print_info()
    }

    // --- hyperedges ---------------------------------------------------------

    /// `hyperedgeRerouter()->registerHyperedgeForRerouting(junction)`;
    /// returns the index or `undefined`.
    #[wasm_bindgen(js_name = registerHyperedgeForReroutingJunction)]
    pub fn register_hyperedge_for_rerouting_junction(&mut self, junction_id: String) -> Option<u32> {
        let j = self.junction(&junction_id)?;
        Some(self.router.hyperedge_rerouter().register_hyperedge_for_rerouting_junction(j) as u32)
    }

    /// `hyperedgeRerouter()->registerHyperedgeForRerouting(terminals)` with
    /// raw ConnEnds in parallel arrays.  Refused (undefined) unless there are
    /// at least 3 distinct shape-pin / junction terminals and every shape-pin
    /// terminal has a pin of its class: otherwise upstream's rerouting
    /// dereferences a null vertex / tree root (abort).
    #[wasm_bindgen(js_name = registerHyperedgeForRerouting)]
    pub fn register_hyperedge_for_rerouting(
        &mut self,
        kinds: Vec<f64>,
        object_ids: Vec<String>,
        pin_ids: Vec<String>,
        xs: Vec<f64>,
        ys: Vec<f64>,
        nums: Vec<f64>,
    ) -> Option<u32> {
        let n = kinds.len();
        if [object_ids.len(), pin_ids.len(), xs.len(), ys.len(), nums.len()].iter().any(|&l| l != n) {
            return None;
        }
        let mut ends = Vec::with_capacity(n);
        for i in 0..n {
            let e = self.conn_end(RawEnd { kind: kinds[i], object_id: &object_ids[i], pin_id: &pin_ids[i], x: xs[i], y: ys[i], num: nums[i] })?;
            // Free-point terminals can leave the minimum terminal spanning
            // tree disconnected (null tree root, which upstream asserts and
            // then dereferences): refused.
            if e.type_() == ConnEndType::ConnEndPoint {
                return None;
            }
            // Duplicate terminals collapse onto one vertex (same failure).
            if ends.iter().any(|x: &ConnEnd| x.m_anchor_obj == e.m_anchor_obj && x.pin_class_id() == e.pin_class_id()) {
                return None;
            }
            if e.type_() == ConnEndType::ConnEndShapePin {
                let o = e.m_anchor_obj?;
                let has_pin = self.router.obstacles[o]
                    .m_connection_pins
                    .iter()
                    .any(|&p| self.router.pins[p].m_class_id == e.pin_class_id());
                if !has_pin {
                    return None;
                }
            }
            ends.push(e);
        }
        // Fewer than three terminals need no junction: upstream's MTST then
        // has no root junction and performRerouting() dereferences null.
        if ends.len() < 3 {
            return None;
        }
        Some(self.router.hyperedge_rerouter().register_hyperedge_for_rerouting(ends) as u32)
    }

    /// `hyperedgeRerouter()->count()` (registrations pending for the next
    /// transaction).
    #[wasm_bindgen(js_name = hyperedgeReroutingCount)]
    pub fn hyperedge_rerouting_count(&self) -> u32 {
        self.router.m_hyperedge_rerouter.count() as u32
    }

    /// `hyperedgeRerouter()->newAndDeletedObjectLists(index)` as recorded at
    /// the end of the last transaction; `null` for an unknown index.
    #[wasm_bindgen(js_name = hyperedgeNewAndDeletedObjectLists)]
    pub fn hyperedge_new_and_deleted_object_lists(&self, index: f64) -> String {
        let Some(results) = &self.hyperedge_results else {
            return "null".into();
        };
        opt_json(as_u32(index).and_then(|i| results.get(i as usize)).map(|l| l.to_json()))
    }

    /// `Router::newAndDeletedObjectListsFromHyperedgeImprovement()` (as of
    /// the last transaction that ran the improver).
    #[wasm_bindgen(js_name = newAndDeletedObjectListsFromHyperedgeImprovement)]
    pub fn new_and_deleted_object_lists_from_hyperedge_improvement(&self) -> String {
        self.improvement_results.to_json()
    }
}

/// `Polygon(points).curvedPolyline(curveAmount, closed)`; `{points,
/// commands}` JSON or `null` for invalid input.
#[wasm_bindgen(js_name = curvedPolyline)]
pub fn curved_polyline(coordinates: Vec<f64>, curve_amount: f64, closed: Option<bool>) -> String {
    let Some(ps) = point_list(&coordinates) else {
        return "null".into();
    };
    if !curve_amount.is_finite() {
        return "null".into();
    }
    let mut poly = Polygon::new();
    poly.ps = ps;
    AvoidRouter::curved_json(&poly.curved_polyline(curve_amount, closed.unwrap_or(false)))
}
