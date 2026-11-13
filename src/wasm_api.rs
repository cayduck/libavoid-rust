//! The low-level wasm-bindgen layer under the JavaScript API.
//!
//! `lib/adapter.cjs` implements upstream libavoid's public classes
//! (`Avoid.Router`, `Avoid.ShapeRef`, `Avoid.ConnRef`, ...) on top of the
//! functions here; nothing in this file is public JavaScript API.  Every
//! function delegates to the faithful port in [`crate::avoid`]; this layer
//! only
//!
//! * names router-owned objects by numeric **keys** (see below),
//! * converts values to and from flat numeric arrays (no JSON),
//! * checks upstream's documented preconditions (`COLA_ASSERT`s on
//!   public-API arguments) and the liveness of every object it is handed,
//!   returning a JS `Error` instead of reaching a core path that would
//!   panic (and trap the instance, which is built with `panic = "abort"`),
//! * records connector callbacks (upstream `ConnRef::setCallback`) in the
//!   order the core performs them, for the adapter to invoke after the call
//!   returns, and forwards the virtual
//!   `Router::shouldContinueTransactionWithProgress()` to JavaScript.
//!
//! # Keys
//! A key names one shape, junction, connector, cluster or connection pin
//! of one `RawRouter` for the router's lifetime: `key = (n << 3) | kind`
//! with `n` a per-router counter (keys are never reused) and `kind` one of
//! the `KIND_*` constants.  Key 0 is the null pointer.  Objects the core
//! creates itself (hyperedge rerouting / improvement, `splitAtSegment`) get
//! a key the first time they are reported.  Arena handles are
//! generation-checked, so a key of a freed object stays dead; every
//! function taking a key fails with "object has been deleted" for it.
//!
//! # Value encodings (all `Float64Array` / `Uint32Array` on the JS side)
//! * Point: `x, y, id, vn`.
//! * Polygon: `_id, n, (x, y, id, vn) * n, nts, ts[0..nts] (char codes),
//!   ncp, (index, x, y, id, vn) * ncp` (`ps`, `ts`, `checkpointsOnRoute`).
//! * Box: `min (4), max (4)`.
//! * ConnEnd: `type, x, y, id, vn, directions, pinClassId, anchorKey,
//!   activePinKey, connRefKey` (all private members of upstream's value
//!   type, so that a copy returned by `endpointConnEnds()` behaves as
//!   upstream's copy does).
//! * Checkpoint: `x, y, id, vn, arrivalDirections, departureDirections`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use wasm_bindgen::prelude::*;

use crate::avoid::arena::{ClusterIdx, ConnIdx, ObstacleIdx, PinIdx};
use crate::avoid::connectionpin::{ShapeConnectionPin, CONNECTIONPIN_UNSET};
use crate::avoid::connector::{Checkpoint, ConnRef, ConnType};
use crate::avoid::connend::{ConnEnd, ConnEndType, CONN_DIR_ALL};
use crate::avoid::geometry;
use crate::avoid::geomtypes::{Box as GeomBox, Point, Polygon, PolygonInterface, ReferencingPolygon};
use crate::avoid::hyperedge::HyperedgeNewAndDeletedObjectLists;
use crate::avoid::junction::JunctionRef;
use crate::avoid::obstacle::Obstacle;
use crate::avoid::router::{
    Router, RoutingOption, RoutingParameter, LAST_ROUTING_OPTION_MARKER, LAST_ROUTING_PARAMETER_MARKER,
    ORTHOGONAL_ROUTING, POLY_LINE_ROUTING,
};
use crate::avoid::shape::{ShapeRef, ShapeTransformationType};
use crate::avoid::viscluster::ClusterRef;

pub const KIND_SHAPE: u32 = 1;
pub const KIND_JUNCTION: u32 = 2;
pub const KIND_CONN: u32 = 3;
pub const KIND_CLUSTER: u32 = 4;
pub const KIND_PIN: u32 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Obj {
    Shape(ObstacleIdx),
    Junction(ObstacleIdx),
    Conn(ConnIdx),
    Cluster(ClusterIdx),
    Pin(PinIdx),
}

impl Obj {
    fn kind(self) -> u32 {
        match self {
            Obj::Shape(_) => KIND_SHAPE,
            Obj::Junction(_) => KIND_JUNCTION,
            Obj::Conn(_) => KIND_CONN,
            Obj::Cluster(_) => KIND_CLUSTER,
            Obj::Pin(_) => KIND_PIN,
        }
    }
}

#[wasm_bindgen]
extern "C" {
    /// `(elapsedTime, phaseNumber, totalPhases, proportion) => boolean`,
    /// the JS side of the virtual `shouldContinueTransactionWithProgress()`.
    #[wasm_bindgen(typescript_type = "(elapsedTime: number, phaseNumber: number, totalPhases: number, proportion: number) => boolean")]
    pub type ProgressFn;

    #[wasm_bindgen(method, catch, js_name = call)]
    fn call4(this: &ProgressFn, this_arg: &JsValue, elapsed: u32, phase: u32, total: u32, proportion: f64) -> Result<JsValue, JsValue>;
}

type Res<T> = Result<T, JsError>;

fn err<T>(msg: &str) -> Res<T> {
    Err(JsError::new(msg))
}

const DELETED: &str = "object has been deleted";

// ---------------------------------------------------------------------------
// Value encodings
// ---------------------------------------------------------------------------

fn u32_of(v: f64, what: &str) -> Res<u32> {
    if v.is_finite() && v.fract() == 0.0 && (0.0..=u32::MAX as f64).contains(&v) {
        Ok(v as u32)
    } else {
        err(&format!("{what} must be an unsigned 32-bit integer"))
    }
}

fn point_at(v: &[f64], i: usize) -> Res<Point> {
    let id = u32_of(v[i + 2], "Point.id")?;
    let vn = v[i + 3];
    if !(vn.is_finite() && vn.fract() == 0.0 && (0.0..=u16::MAX as f64).contains(&vn)) {
        return err("Point.vn must be an unsigned 16-bit integer");
    }
    Ok(Point { x: v[i], y: v[i + 1], id, vn: vn as u16 })
}

fn push_point(out: &mut Vec<f64>, p: &Point) {
    out.extend_from_slice(&[p.x, p.y, p.id as f64, p.vn as f64]);
}

fn enc_point(p: &Point) -> Vec<f64> {
    let mut out = Vec::with_capacity(4);
    push_point(&mut out, p);
    out
}

fn enc_poly(poly: &Polygon) -> Vec<f64> {
    let mut out = Vec::with_capacity(4 + 4 * poly.ps.len() + poly.ts.len() + 5 * poly.checkpoints_on_route.len());
    out.push(poly._id as f64);
    out.push(poly.ps.len() as f64);
    for p in &poly.ps {
        push_point(&mut out, p);
    }
    out.push(poly.ts.len() as f64);
    out.extend(poly.ts.iter().map(|&c| c as u32 as f64));
    out.push(poly.checkpoints_on_route.len() as f64);
    for (i, p) in &poly.checkpoints_on_route {
        out.push(*i as f64);
        push_point(&mut out, p);
    }
    out
}

fn dec_poly(v: &[f64]) -> Res<Polygon> {
    let bad = || JsError::new("malformed Polygon encoding");
    let mut poly = Polygon::new();
    let id = *v.first().ok_or_else(bad)?;
    if !(id.is_finite() && id.fract() == 0.0 && (i32::MIN as f64..=i32::MAX as f64).contains(&id)) {
        return err("Polygon._id must be a 32-bit integer");
    }
    poly._id = id as i32;
    let n = *v.get(1).ok_or_else(bad)? as usize;
    let mut i = 2;
    if v.len() < i + 4 * n + 1 {
        return Err(bad());
    }
    for _ in 0..n {
        poly.ps.push(point_at(v, i)?);
        i += 4;
    }
    let nts = v[i] as usize;
    i += 1;
    if v.len() < i + nts + 1 {
        return Err(bad());
    }
    for _ in 0..nts {
        poly.ts.push(char::from_u32(v[i] as u32).ok_or_else(bad)?);
        i += 1;
    }
    let ncp = v[i] as usize;
    i += 1;
    if v.len() != i + 5 * ncp {
        return Err(bad());
    }
    for _ in 0..ncp {
        let idx = v[i];
        if !(idx.is_finite() && idx.fract() == 0.0 && idx >= 0.0) {
            return err("checkpointsOnRoute index must be a non-negative integer");
        }
        poly.checkpoints_on_route.push((idx as usize, point_at(v, i + 1)?));
        i += 5;
    }
    Ok(poly)
}

fn enc_box(b: &GeomBox) -> Vec<f64> {
    let mut out = Vec::with_capacity(8);
    push_point(&mut out, &b.min);
    push_point(&mut out, &b.max);
    out
}

fn dirs_of(v: f64) -> Res<u32> {
    let d = u32_of(v, "ConnDirFlags")?;
    if d > CONN_DIR_ALL {
        return err("ConnDirFlags must be a combination of ConnDirUp/Down/Left/Right (0..15)");
    }
    Ok(d)
}

/// Pin offsets must be finite: the core defines no behaviour for a pin at
/// a non-finite position (upstream: undefined; the orthogonal router would
/// index out of bounds).
fn check_offsets(x: f64, y: f64, inside: f64) -> Res<()> {
    if x.is_finite() && y.is_finite() && inside.is_finite() {
        Ok(())
    } else {
        err("xOffset, yOffset and insideOffset must be finite numbers")
    }
}

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

const TRANSFORMS: [ShapeTransformationType; 5] = [
    ShapeTransformationType::TransformationTypeCW90,
    ShapeTransformationType::TransformationTypeCW180,
    ShapeTransformationType::TransformationTypeCW270,
    ShapeTransformationType::TransformationTypeFlipX,
    ShapeTransformationType::TransformationTypeFlipY,
];

const CONN_TYPES: [ConnType; 3] = [ConnType::ConnTypeNone, ConnType::ConnTypePolyLine, ConnType::ConnTypeOrthogonal];

fn conn_type_of(t: u32) -> Res<ConnType> {
    CONN_TYPES.get(t as usize).copied().ok_or_else(|| JsError::new("ConnType must be ConnType_None, ConnType_PolyLine or ConnType_Orthogonal"))
}

fn param_of(p: u32) -> Res<RoutingParameter> {
    if (p as usize) < LAST_ROUTING_PARAMETER_MARKER {
        Ok(RoutingParameter::ALL[p as usize])
    } else {
        err("RoutingParameter out of range (COLA_ASSERT(parameter < lastRoutingParameterMarker))")
    }
}

fn option_of(o: u32) -> Res<RoutingOption> {
    if (o as usize) < LAST_ROUTING_OPTION_MARKER {
        Ok(RoutingOption::ALL[o as usize])
    } else {
        err("RoutingOption out of range (COLA_ASSERT(option < lastRoutingOptionMarker))")
    }
}

// ---------------------------------------------------------------------------
// RawRouter
// ---------------------------------------------------------------------------

type ProgressSlot = Rc<RefCell<Option<ProgressFn>>>;

/// One upstream `Router` and the key table of its objects.
#[wasm_bindgen]
pub struct RawRouter {
    router: Router,
    keys: Vec<Obj>,
    key_of: HashMap<Obj, u32>,
    callbacks: Rc<RefCell<Vec<ConnIdx>>>,
    progress: ProgressSlot,
}

impl RawRouter {
    /// Read access to the wrapped core router (for Rust users and tests).
    pub fn core(&self) -> &Router {
        &self.router
    }

    fn key(&mut self, o: Obj) -> u32 {
        if let Some(&k) = self.key_of.get(&o) {
            return k;
        }
        let k = ((self.keys.len() as u32) << 3) | o.kind();
        self.keys.push(o);
        self.key_of.insert(o, k);
        k
    }

    fn key_obstacle(&mut self, o: ObstacleIdx) -> u32 {
        if let Some(&k) = self.key_of.get(&Obj::Shape(o)).or_else(|| self.key_of.get(&Obj::Junction(o))) {
            return k;
        }
        let junction = self.router.obstacles.get(o).map(|ob| ob.is_junction()).unwrap_or(false);
        self.key(if junction { Obj::Junction(o) } else { Obj::Shape(o) })
    }

    fn opt_key_obstacle(&mut self, o: Option<ObstacleIdx>) -> u32 {
        o.map(|o| self.key_obstacle(o)).unwrap_or(0)
    }

    fn obj(&self, key: u32) -> Res<Obj> {
        match self.keys.get((key >> 3) as usize) {
            Some(&o) if o.kind() == key & 7 => Ok(o),
            _ => err("unknown object"),
        }
    }

    fn alive_obj(&self, o: Obj) -> bool {
        match o {
            Obj::Shape(x) | Obj::Junction(x) => self.router.obstacles.contains(x),
            Obj::Conn(x) => self.router.conns.contains(x),
            Obj::Cluster(x) => self.router.clusters.contains(x),
            Obj::Pin(x) => self.router.pins.contains(x),
        }
    }

    fn live(&self, key: u32) -> Res<Obj> {
        let o = self.obj(key)?;
        if !self.alive_obj(o) {
            return err(DELETED);
        }
        Ok(o)
    }

    fn shape(&self, key: u32) -> Res<ShapeRef> {
        match self.live(key)? {
            Obj::Shape(o) => Ok(ShapeRef(o)),
            _ => err("not a ShapeRef"),
        }
    }

    fn junction(&self, key: u32) -> Res<JunctionRef> {
        match self.live(key)? {
            Obj::Junction(o) => Ok(JunctionRef(o)),
            _ => err("not a JunctionRef"),
        }
    }

    fn obstacle(&self, key: u32) -> Res<ObstacleIdx> {
        match self.live(key)? {
            Obj::Shape(o) | Obj::Junction(o) => Ok(o),
            _ => err("not an Obstacle"),
        }
    }

    fn conn(&self, key: u32) -> Res<ConnIdx> {
        match self.live(key)? {
            Obj::Conn(c) => Ok(c),
            _ => err("not a ConnRef"),
        }
    }

    fn cluster(&self, key: u32) -> Res<ClusterIdx> {
        match self.live(key)? {
            Obj::Cluster(c) => Ok(c),
            _ => err("not a ClusterRef"),
        }
    }

    fn pin(&self, key: u32) -> Res<PinIdx> {
        match self.live(key)? {
            Obj::Pin(p) => Ok(p),
            _ => err("not a ShapeConnectionPin"),
        }
    }

    /// The id for a new object (upstream `Router::assignId()`), checking
    /// its `COLA_ASSERT(objectIdIsUnused(assignedId))` beforehand.
    fn check_id(&self, id: u32) -> Res<u32> {
        let assigned = if id == 0 { self.router.new_object_id() } else { id };
        if assigned == 0 {
            return err("the object id space is exhausted");
        }
        if !self.router.object_id_is_unused(assigned) {
            return err(&format!("id {assigned} is already used by another object (COLA_ASSERT(objectIdIsUnused(id)))"));
        }
        Ok(id)
    }

    fn conn_keys(&mut self, cs: &[ConnIdx]) -> Vec<u32> {
        cs.iter().map(|&c| self.key(Obj::Conn(c))).collect()
    }

    fn junction_keys(&mut self, js: &[JunctionRef]) -> Vec<u32> {
        js.iter().map(|j| self.key(Obj::Junction(j.0))).collect()
    }

    fn enc_lists(&mut self, l: &HyperedgeNewAndDeletedObjectLists) -> Vec<u32> {
        let mut out = Vec::new();
        for list in [
            self.junction_keys(&l.new_junction_list),
            self.conn_keys(&l.new_connector_list),
            self.junction_keys(&l.deleted_junction_list),
            self.conn_keys(&l.deleted_connector_list),
            self.conn_keys(&l.changed_connector_list),
        ] {
            out.push(list.len() as u32);
            out.extend(list);
        }
        out
    }

    fn enc_conn_end(&mut self, ce: &ConnEnd, out: &mut Vec<f64>) {
        out.push(ce.m_type as u32 as f64);
        push_point(out, &ce.m_point);
        out.push(ce.m_directions as f64);
        out.push(ce.m_connection_pin_class_id as f64);
        let anchor = self.opt_key_obstacle(ce.m_anchor_obj);
        out.push(anchor as f64);
        let pin = match ce.m_active_pin {
            Some(p) => self.key(Obj::Pin(p)),
            None => 0,
        };
        out.push(pin as f64);
        let conn = match ce.m_conn_ref {
            Some(c) => self.key(Obj::Conn(c)),
            None => 0,
        };
        out.push(conn as f64);
    }

    /// Decodes a ConnEnd (10 numbers at `v[i..]`).  The anchor must be a
    /// live shape / junction of this router; an active pin or connector
    /// that no longer exists is dropped (as the core does for copies that
    /// outlive their objects).
    fn dec_conn_end(&self, v: &[f64], i: usize) -> Res<ConnEnd> {
        if v.len() < i + 10 {
            return err("malformed ConnEnd encoding");
        }
        let ty = match u32_of(v[i], "ConnEndType")? {
            0 => ConnEndType::ConnEndPoint,
            1 => ConnEndType::ConnEndShapePin,
            2 => ConnEndType::ConnEndJunction,
            3 => ConnEndType::ConnEndEmpty,
            _ => return err("ConnEndType out of range"),
        };
        let mut ce = ConnEnd::new();
        ce.m_type = ty;
        ce.m_point = point_at(v, i + 1)?;
        ce.m_directions = dirs_of(v[i + 5])?;
        ce.m_connection_pin_class_id = u32_of(v[i + 6], "pinClassId")?;
        let anchor = u32_of(v[i + 7], "anchor")?;
        if anchor != 0 {
            ce.m_anchor_obj = Some(self.obstacle(anchor)?);
        }
        let pin = u32_of(v[i + 8], "active pin")?;
        if pin != 0 {
            if let Ok(Obj::Pin(p)) = self.obj(pin) {
                if self.router.pins.contains(p) {
                    ce.m_active_pin = Some(p);
                }
            }
        }
        let conn = u32_of(v[i + 9], "connector")?;
        if conn != 0 {
            if let Ok(Obj::Conn(c)) = self.obj(conn) {
                if self.router.conns.contains(c) {
                    ce.m_conn_ref = Some(c);
                }
            }
        }
        match ty {
            ConnEndType::ConnEndShapePin if !matches!(self.obj(anchor), Ok(Obj::Shape(_))) => {
                err("a ConnEndShapePin needs a ShapeRef")
            }
            ConnEndType::ConnEndJunction if !matches!(self.obj(anchor), Ok(Obj::Junction(_))) => {
                err("a ConnEndJunction needs a JunctionRef")
            }
            _ => Ok(ce),
        }
    }

    fn enc_checkpoints(cps: &[Checkpoint]) -> Vec<f64> {
        let mut out = Vec::with_capacity(6 * cps.len());
        for cp in cps {
            push_point(&mut out, &cp.point);
            out.push(cp.arrival_directions as f64);
            out.push(cp.departure_directions as f64);
        }
        out
    }

    fn install_progress_hook(router: &mut Router, progress: &ProgressSlot) {
        let progress = progress.clone();
        router.m_progress_callback = Some(std::boxed::Box::new(move |elapsed, phase, total, proportion| {
            match progress.borrow().as_ref() {
                None => true,
                // A JS override that throws counts as a stop request; the
                // adapter rethrows its error after the call returns.
                Some(f) => match f.call4(&JsValue::UNDEFINED, elapsed, phase, total, proportion) {
                    Ok(v) => v.as_bool().unwrap_or(false),
                    Err(_) => false,
                },
            }
        }));
    }

    fn check_obstacle_out_of_graph(&self, o: ObstacleIdx) -> Res<()> {
        let ob = &self.router.obstacles[o];
        let Some(first) = ob.first_vert() else {
            return err("the obstacle has no vertices (COLA_ASSERT(m_first_vert != nullptr))");
        };
        let mut curr = first;
        loop {
            let v = &self.router.vert_infs[curr];
            if v.vis_list_size != 0 || v.invis_list_size != 0 {
                return err("the obstacle is part of the visibility graph (COLA_ASSERT(curr->visListSize == 0)); use Router::moveShape()");
            }
            match v.sh_next {
                Some(n) if n != first => curr = n,
                _ => break,
            }
        }
        Ok(())
    }
}

#[wasm_bindgen]
impl RawRouter {
    /// upstream `Router::Router(flags)`.
    #[wasm_bindgen(constructor)]
    pub fn new(flags: u32) -> Res<RawRouter> {
        if flags & (POLY_LINE_ROUTING | ORTHOGONAL_ROUTING) == 0 {
            return err("Router: flags must include PolyLineRouting or OrthogonalRouting (COLA_ASSERT(flags & (PolyLineRouting | OrthogonalRouting)))");
        }
        let progress: ProgressSlot = Rc::new(RefCell::new(None));
        let mut router = Router::new(flags);
        RawRouter::install_progress_hook(&mut router, &progress);
        Ok(RawRouter {
            router,
            keys: Vec::new(),
            key_of: HashMap::new(),
            callbacks: Rc::new(RefCell::new(Vec::new())),
            progress,
        })
    }

    // --- infrastructure -----------------------------------------------------

    /// For each key: 1 if its object is alive, else 0.
    #[wasm_bindgen(js_name = aliveMany)]
    pub fn alive_many(&self, keys: Vec<u32>) -> Vec<u8> {
        keys.iter().map(|&k| self.live(k).is_ok() as u8).collect()
    }

    /// Sets (or removes) the JS side of `shouldContinueTransactionWithProgress`.
    #[wasm_bindgen(js_name = setProgressHook)]
    pub fn set_progress_hook(&mut self, f: Option<ProgressFn>) {
        *self.progress.borrow_mut() = f;
    }

    /// Keys of the connectors whose callback the core performed since the
    /// last call, in order (cleared).
    #[wasm_bindgen(js_name = takeCallbacks)]
    pub fn take_callbacks(&mut self) -> Vec<u32> {
        let log: Vec<ConnIdx> = self.callbacks.borrow_mut().drain(..).collect();
        log.into_iter().map(|c| self.key(Obj::Conn(c))).collect()
    }

    /// Test-only trap trigger (not API; used by the Node tests to exercise
    /// failure containment on the shipped wasm).  Does nothing and returns
    /// false unless `arming` is exactly `wasm_failure::TEST_PANIC_ARMING`;
    /// otherwise traps the wasm instance: kind 1 = explicit panic, 2 = index
    /// out of bounds after mutating this router, 3 = stack overflow, 4 =
    /// failed allocation.  Unknown kinds return false.
    #[wasm_bindgen(js_name = __testPanic, skip_typescript)]
    pub fn test_panic(&mut self, kind: u32, arming: String) -> bool {
        if arming != crate::wasm_failure::TEST_PANIC_ARMING {
            return false;
        }
        match kind {
            crate::wasm_failure::TEST_PANIC_EXPLICIT => panic!("libavoid-rust test panic (kind {kind})"),
            crate::wasm_failure::TEST_PANIC_INDEX => {
                // Half-update the router's state, then index out of bounds
                // while `&mut self` is held, as a bug in a method would.
                self.keys.push(Obj::Conn(ConnIdx::dangling()));
                let i = std::hint::black_box(self.keys.len());
                self.keys[i].kind() > 0
            }
            crate::wasm_failure::TEST_PANIC_STACK_OVERFLOW => crate::wasm_failure::recurse_forever(0) > 0,
            crate::wasm_failure::TEST_PANIC_OUT_OF_MEMORY => crate::wasm_failure::exhaust_memory(),
            _ => false,
        }
    }

    // --- Router -------------------------------------------------------------

    #[wasm_bindgen(js_name = setTransactionUse)]
    pub fn set_transaction_use(&mut self, transactions: bool) {
        self.router.set_transaction_use(transactions);
    }

    #[wasm_bindgen(js_name = transactionUse)]
    pub fn transaction_use(&self) -> bool {
        self.router.transaction_use()
    }

    #[wasm_bindgen(js_name = processTransaction)]
    pub fn process_transaction(&mut self) -> bool {
        self.router.process_transaction()
    }

    #[wasm_bindgen(js_name = deleteShape)]
    pub fn delete_shape(&mut self, shape: u32) -> Res<()> {
        let s = self.shape(shape)?;
        self.router.delete_shape(s);
        Ok(())
    }

    #[wasm_bindgen(js_name = moveShapePoly)]
    pub fn move_shape_poly(&mut self, shape: u32, poly: Vec<f64>, first_move: bool) -> Res<()> {
        let s = self.shape(shape)?;
        let p = dec_poly(&poly)?;
        self.router.move_shape(s, &p, first_move);
        Ok(())
    }

    #[wasm_bindgen(js_name = moveShapeBy)]
    pub fn move_shape_by(&mut self, shape: u32, x_diff: f64, y_diff: f64) -> Res<()> {
        let s = self.shape(shape)?;
        self.router.move_shape_by(s, x_diff, y_diff);
        Ok(())
    }

    #[wasm_bindgen(js_name = deleteJunction)]
    pub fn delete_junction(&mut self, junction: u32) -> Res<()> {
        let j = self.junction(junction)?;
        self.router.delete_junction(j);
        Ok(())
    }

    #[wasm_bindgen(js_name = deleteConnector)]
    pub fn delete_connector(&mut self, conn: u32) -> Res<()> {
        let c = self.conn(conn)?;
        self.router.delete_connector(c);
        Ok(())
    }

    #[wasm_bindgen(js_name = moveJunctionTo)]
    pub fn move_junction_to(&mut self, junction: u32, x: f64, y: f64) -> Res<()> {
        let j = self.junction(junction)?;
        self.router.move_junction(j, &pt(x, y));
        Ok(())
    }

    #[wasm_bindgen(js_name = moveJunctionBy)]
    pub fn move_junction_by(&mut self, junction: u32, x_diff: f64, y_diff: f64) -> Res<()> {
        let j = self.junction(junction)?;
        self.router.move_junction_by(j, x_diff, y_diff);
        Ok(())
    }

    #[wasm_bindgen(js_name = setRoutingParameter)]
    pub fn set_routing_parameter(&mut self, parameter: u32, value: f64) -> Res<()> {
        let p = param_of(parameter)?;
        self.router.set_routing_parameter(p, value);
        Ok(())
    }

    #[wasm_bindgen(js_name = routingParameter)]
    pub fn routing_parameter(&self, parameter: u32) -> Res<f64> {
        Ok(self.router.routing_parameter(param_of(parameter)?))
    }

    #[wasm_bindgen(js_name = setRoutingOption)]
    pub fn set_routing_option(&mut self, option: u32, value: bool) -> Res<()> {
        let o = option_of(option)?;
        self.router.set_routing_option(o, value);
        Ok(())
    }

    #[wasm_bindgen(js_name = routingOption)]
    pub fn routing_option(&self, option: u32) -> Res<bool> {
        Ok(self.router.routing_option(option_of(option)?))
    }

    #[wasm_bindgen(js_name = setRoutingPenalty)]
    pub fn set_routing_penalty(&mut self, pen_type: u32, pen_val: f64) -> Res<()> {
        let p = param_of(pen_type)?;
        self.router.set_routing_penalty(p, pen_val);
        Ok(())
    }

    #[wasm_bindgen(js_name = newObjectId)]
    pub fn new_object_id(&self) -> u32 {
        self.router.new_object_id()
    }

    #[wasm_bindgen(js_name = objectIdIsUnused)]
    pub fn object_id_is_unused(&self, id: u32) -> bool {
        self.router.object_id_is_unused(id)
    }

    /// `newAndDeletedObjectListsFromHyperedgeImprovement()`: five key lists,
    /// each as `length, keys...`.
    #[wasm_bindgen(js_name = newAndDeletedObjectListsFromHyperedgeImprovement)]
    pub fn new_and_deleted_object_lists_from_hyperedge_improvement(&mut self) -> Vec<u32> {
        let l = self.router.new_and_deleted_object_lists_from_hyperedge_improvement();
        self.enc_lists(&l)
    }

    #[wasm_bindgen(js_name = processActions)]
    pub fn process_actions(&mut self) {
        self.router.process_actions();
    }

    #[wasm_bindgen(js_name = deleteCluster)]
    pub fn delete_cluster(&mut self, cluster: u32) -> Res<()> {
        let c = self.cluster(cluster)?;
        self.router.delete_cluster(c);
        Ok(())
    }

    #[wasm_bindgen(js_name = attachedShapes)]
    pub fn attached_shapes(&self, shape_id: u32, type_: u32) -> Vec<u32> {
        let mut out = Vec::new();
        self.router.attached_shapes(&mut out, shape_id, type_);
        out
    }

    #[wasm_bindgen(js_name = attachedConns)]
    pub fn attached_conns(&self, shape_id: u32, type_: u32) -> Vec<u32> {
        let mut out = Vec::new();
        self.router.attached_conns(&mut out, shape_id, type_);
        out
    }

    #[wasm_bindgen(js_name = markPolylineConnectorsNeedingReroutingForDeletedObstacle)]
    pub fn mark_polyline_connectors_needing_rerouting_for_deleted_obstacle(&mut self, obstacle: u32) -> Res<()> {
        let o = self.obstacle(obstacle)?;
        self.router.mark_polyline_connectors_needing_rerouting_for_deleted_obstacle(o);
        Ok(())
    }

    #[wasm_bindgen(js_name = printInfo)]
    pub fn print_info(&self) -> String {
        self.router.print_info()
    }

    #[wasm_bindgen(js_name = regenerateStaticBuiltGraph)]
    pub fn regenerate_static_built_graph(&mut self) {
        self.router.regenerate_static_built_graph();
    }

    #[wasm_bindgen(js_name = destroyOrthogonalVisGraph)]
    pub fn destroy_orthogonal_vis_graph(&mut self) {
        self.router.destroy_orthogonal_vis_graph();
    }

    #[wasm_bindgen(js_name = setStaticGraphInvalidated)]
    pub fn set_static_graph_invalidated(&mut self, invalidated: bool) {
        self.router.set_static_graph_invalidated(invalidated);
    }

    #[wasm_bindgen(js_name = validConnType)]
    pub fn valid_conn_type(&self, select: u32) -> Res<u32> {
        Ok(self.router.valid_conn_type(conn_type_of(select)?) as u32)
    }

    #[wasm_bindgen(js_name = isInCrossingPenaltyReroutingStage)]
    pub fn is_in_crossing_penalty_rerouting_stage(&self) -> bool {
        self.router.is_in_crossing_penalty_rerouting_stage()
    }

    #[wasm_bindgen(js_name = markAllObstaclesAsMoved)]
    pub fn mark_all_obstacles_as_moved(&mut self) {
        self.router.mark_all_obstacles_as_moved();
    }

    /// Key of the shape, or 0.
    #[wasm_bindgen(js_name = shapeContainingPoint)]
    pub fn shape_containing_point(&mut self, x: f64, y: f64) -> u32 {
        match self.router.shape_containing_point(&pt(x, y)) {
            Some(s) => self.key(Obj::Shape(s.0)),
            None => 0,
        }
    }

    #[wasm_bindgen(js_name = performContinuationCheck)]
    pub fn perform_continuation_check(&mut self, phase_number: u32, step_number: f64, total_steps: f64) {
        self.router.perform_continuation_check(phase_number, step_number as usize, total_steps as usize);
    }

    #[wasm_bindgen(js_name = registerSettingsChange)]
    pub fn register_settings_change(&mut self) {
        self.router.register_settings_change();
    }

    #[wasm_bindgen(js_name = improveOrthogonalTopology)]
    pub fn improve_orthogonal_topology(&mut self) {
        self.router.improve_orthogonal_topology();
    }

    #[wasm_bindgen(js_name = existsOrthogonalSegmentOverlap)]
    pub fn exists_orthogonal_segment_overlap(&mut self, at_ends: bool) -> bool {
        self.router.exists_orthogonal_segment_overlap(at_ends)
    }

    #[wasm_bindgen(js_name = existsOrthogonalFixedSegmentOverlap)]
    pub fn exists_orthogonal_fixed_segment_overlap(&mut self, at_ends: bool) -> bool {
        self.router.exists_orthogonal_fixed_segment_overlap(at_ends)
    }

    #[wasm_bindgen(js_name = existsOrthogonalTouchingPaths)]
    pub fn exists_orthogonal_touching_paths(&mut self) -> bool {
        self.router.exists_orthogonal_touching_paths()
    }

    #[wasm_bindgen(js_name = existsCrossings)]
    pub fn exists_crossings(&mut self, optimised_for_connector_type: bool) -> i32 {
        self.router.exists_crossings(optimised_for_connector_type)
    }

    #[wasm_bindgen(js_name = existsInvalidOrthogonalPaths)]
    pub fn exists_invalid_orthogonal_paths(&mut self) -> bool {
        self.router.exists_invalid_orthogonal_paths()
    }

    /// The text upstream's `outputDiagramText()` writes (`%g` format).
    #[wasm_bindgen(js_name = outputDiagramText)]
    pub fn output_diagram_text(&mut self) -> String {
        self.router.output_diagram_text_g()
    }

    /// Public `bool` members of upstream `Router`, by index: PartialTime,
    /// SimpleRouting, ClusteredRouting, IgnoreRegions, UseLeesAlgorithm,
    /// InvisibilityGrph, SelectiveReroute, PartialFeedback,
    /// RubberBandRouting.
    #[wasm_bindgen(js_name = flagMember)]
    pub fn flag_member(&self, index: u32) -> bool {
        let r = &self.router;
        match index {
            0 => r.partial_time,
            1 => r.simple_routing,
            2 => r.clustered_routing,
            3 => r.ignore_regions,
            4 => r.use_lees_algorithm,
            5 => r.invisibility_grph,
            6 => r.selective_reroute,
            7 => r.partial_feedback,
            _ => r.rubber_band_routing,
        }
    }

    #[wasm_bindgen(js_name = setFlagMember)]
    pub fn set_flag_member(&mut self, index: u32, value: bool) {
        let r = &mut self.router;
        match index {
            0 => r.partial_time = value,
            1 => r.simple_routing = value,
            2 => r.clustered_routing = value,
            3 => r.ignore_regions = value,
            4 => r.use_lees_algorithm = value,
            5 => r.invisibility_grph = value,
            6 => r.selective_reroute = value,
            7 => r.partial_feedback = value,
            _ => r.rubber_band_routing = value,
        }
    }

    #[wasm_bindgen(js_name = stCheckedEdges)]
    pub fn st_checked_edges(&self) -> i32 {
        self.router.st_checked_edges
    }

    #[wasm_bindgen(js_name = setStCheckedEdges)]
    pub fn set_st_checked_edges(&mut self, value: i32) {
        self.router.st_checked_edges = value;
    }

    /// Keys of `m_obstacles` (list order).
    pub fn obstacles(&mut self) -> Vec<u32> {
        let list = self.router.m_obstacles.clone();
        list.into_iter().map(|o| self.key_obstacle(o)).collect()
    }

    /// Keys of `connRefs` (list order).
    #[wasm_bindgen(js_name = connRefs)]
    pub fn conn_refs(&mut self) -> Vec<u32> {
        let list = self.router.conn_refs.clone();
        list.into_iter().map(|c| self.key(Obj::Conn(c))).collect()
    }

    /// Keys of `clusterRefs` (list order).
    #[wasm_bindgen(js_name = clusterRefs)]
    pub fn cluster_refs(&mut self) -> Vec<u32> {
        let list = self.router.cluster_refs.clone();
        list.into_iter().map(|c| self.key(Obj::Cluster(c))).collect()
    }

    /// `ReferencingPolygon(poly, router)`: per point `anchorKey, vn, x, y,
    /// id, vn` (see `clusterPolygon`), preceded by `_id`.
    #[wasm_bindgen(js_name = referencingPolygon)]
    pub fn referencing_polygon(&mut self, poly: Vec<f64>) -> Res<Vec<f64>> {
        let p = dec_poly(&poly)?;
        self.check_referencing(&p)?;
        let rp = ReferencingPolygon::from_polygon(&p, self.router.obstacle_id_list());
        Ok(self.enc_referencing(&rp))
    }

    // --- hyperedge rerouter -----------------------------------------------------

    #[wasm_bindgen(js_name = registerHyperedgeForReroutingTerminals)]
    pub fn register_hyperedge_for_rerouting_terminals(&mut self, ends: Vec<f64>) -> Res<u32> {
        if ends.len() % 10 != 0 {
            return err("malformed ConnEnd list");
        }
        let mut list = Vec::with_capacity(ends.len() / 10);
        for i in (0..ends.len()).step_by(10) {
            list.push(self.dec_conn_end(&ends, i)?);
        }
        Ok(self.router.hyperedge_rerouter().register_hyperedge_for_rerouting(list) as u32)
    }

    #[wasm_bindgen(js_name = registerHyperedgeForReroutingJunction)]
    pub fn register_hyperedge_for_rerouting_junction(&mut self, junction: u32) -> Res<u32> {
        let j = self.junction(junction)?;
        Ok(self.router.hyperedge_rerouter().register_hyperedge_for_rerouting_junction(j) as u32)
    }

    #[wasm_bindgen(js_name = hyperedgeCount)]
    pub fn hyperedge_count(&self) -> u32 {
        self.router.m_hyperedge_rerouter.count() as u32
    }

    #[wasm_bindgen(js_name = hyperedgeNewAndDeletedObjectLists)]
    pub fn hyperedge_new_and_deleted_object_lists(&mut self, index: f64) -> Vec<u32> {
        let l = self.router.m_hyperedge_rerouter.new_and_deleted_object_lists(index as usize);
        self.enc_lists(&l)
    }

    // --- Obstacle / ShapeRef / JunctionRef --------------------------------------

    #[wasm_bindgen(js_name = shapeNew)]
    pub fn shape_new(&mut self, poly: Vec<f64>, id: u32) -> Res<u32> {
        let p = dec_poly(&poly)?;
        let id = self.check_id(id)?;
        let s = ShapeRef::new(&mut self.router, &p, id);
        Ok(self.key(Obj::Shape(s.0)))
    }

    #[wasm_bindgen(js_name = obstacleId)]
    pub fn obstacle_id(&self, key: u32) -> Res<u32> {
        let o = self.obstacle(key)?;
        Ok(self.router.obstacles[o].m_id)
    }

    #[wasm_bindgen(js_name = obstaclePolygon)]
    pub fn obstacle_polygon(&self, key: u32) -> Res<Vec<f64>> {
        let o = self.obstacle(key)?;
        Ok(enc_poly(&self.router.obstacles[o].m_polygon))
    }

    #[wasm_bindgen(js_name = obstaclePosition)]
    pub fn obstacle_position(&self, key: u32) -> Res<Vec<f64>> {
        let o = self.obstacle(key)?;
        Ok(enc_point(&Obstacle::position(&self.router, o)))
    }

    #[wasm_bindgen(js_name = obstacleSetNewPoly)]
    pub fn obstacle_set_new_poly(&mut self, key: u32, poly: Vec<f64>) -> Res<()> {
        let o = self.obstacle(key)?;
        let p = dec_poly(&poly)?;
        if self.router.obstacles[o].m_polygon.size() != p.size() {
            return err("the new polygon must have as many points as the old one (COLA_ASSERT(m_polygon.size() == poly.size()))");
        }
        self.check_obstacle_out_of_graph(o)?;
        Obstacle::set_new_poly(&mut self.router, o, &p);
        Ok(())
    }

    #[wasm_bindgen(js_name = obstacleRoutingBox)]
    pub fn obstacle_routing_box(&self, key: u32) -> Res<Vec<f64>> {
        let o = self.obstacle(key)?;
        Ok(enc_box(&Obstacle::routing_box(&self.router, o)))
    }

    #[wasm_bindgen(js_name = obstacleRoutingPolygon)]
    pub fn obstacle_routing_polygon(&self, key: u32) -> Res<Vec<f64>> {
        let o = self.obstacle(key)?;
        Ok(enc_poly(&Obstacle::routing_polygon(&self.router, o)))
    }

    #[wasm_bindgen(js_name = obstacleAttachedConnectors)]
    pub fn obstacle_attached_connectors(&mut self, key: u32) -> Res<Vec<u32>> {
        let o = self.obstacle(key)?;
        let cs = Obstacle::attached_connectors(&self.router, o);
        Ok(self.conn_keys(&cs))
    }

    #[wasm_bindgen(js_name = shapeTransformConnectionPinPositions)]
    pub fn shape_transform_connection_pin_positions(&mut self, key: u32, transform: u32) -> Res<()> {
        let s = self.shape(key)?;
        let t = *TRANSFORMS
            .get(transform as usize)
            .ok_or_else(|| JsError::new("ShapeTransformationType out of range"))?;
        s.transform_connection_pin_positions(&mut self.router, t);
        Ok(())
    }

    #[wasm_bindgen(js_name = junctionNew)]
    pub fn junction_new(&mut self, x: f64, y: f64, id: u32) -> Res<u32> {
        let id = self.check_id(id)?;
        let j = JunctionRef::new(&mut self.router, pt(x, y), id);
        Ok(self.key(Obj::Junction(j.0)))
    }

    /// Key of the merged connector, or 0.
    #[wasm_bindgen(js_name = junctionRemoveJunctionAndMergeConnectors)]
    pub fn junction_remove_junction_and_merge_connectors(&mut self, key: u32) -> Res<u32> {
        let j = self.junction(key)?;
        Ok(match j.remove_junction_and_merge_connectors(&mut self.router) {
            Some(c) => self.key(Obj::Conn(c)),
            None => 0,
        })
    }

    #[wasm_bindgen(js_name = junctionSetPositionFixed)]
    pub fn junction_set_position_fixed(&mut self, key: u32, fixed: bool) -> Res<()> {
        let j = self.junction(key)?;
        j.set_position_fixed(&mut self.router, fixed);
        Ok(())
    }

    #[wasm_bindgen(js_name = junctionPositionFixed)]
    pub fn junction_position_fixed(&self, key: u32) -> Res<bool> {
        Ok(self.junction(key)?.position_fixed(&self.router))
    }

    #[wasm_bindgen(js_name = junctionRecommendedPosition)]
    pub fn junction_recommended_position(&self, key: u32) -> Res<Vec<f64>> {
        Ok(enc_point(&self.junction(key)?.recommended_position(&self.router)))
    }

    /// `JunctionRef::makeRectangle(this router, position)`.
    #[wasm_bindgen(js_name = junctionMakeRectangle)]
    pub fn junction_make_rectangle(&self, x: f64, y: f64, id: u32, vn: u32) -> Vec<f64> {
        let p = Point { x, y, id, vn: vn as u16 };
        enc_poly(&JunctionRef::make_rectangle(&self.router, &p))
    }

    #[wasm_bindgen(js_name = junctionPreferOrthogonalDimension)]
    pub fn junction_prefer_orthogonal_dimension(&mut self, key: u32, dim: f64) -> Res<()> {
        let j = self.junction(key)?;
        j.prefer_orthogonal_dimension(&mut self.router, dim as usize);
        Ok(())
    }

    // --- ConnRef ------------------------------------------------------------------

    #[wasm_bindgen(js_name = connNew)]
    pub fn conn_new(&mut self, id: u32) -> Res<u32> {
        let id = self.check_id(id)?;
        let c = ConnRef::new(&mut self.router, id);
        Ok(self.key(Obj::Conn(c)))
    }

    #[wasm_bindgen(js_name = connNewWithEnds)]
    pub fn conn_new_with_ends(&mut self, ends: Vec<f64>, id: u32) -> Res<u32> {
        let s = self.dec_conn_end(&ends, 0)?;
        let d = self.dec_conn_end(&ends, 10)?;
        let id = self.check_id(id)?;
        let c = ConnRef::new_with_ends(&mut self.router, &s, &d, id);
        Ok(self.key(Obj::Conn(c)))
    }

    /// `which`: 0 = setEndpoints(src, dst), 1 = setSourceEndpoint,
    /// 2 = setDestEndpoint, 3 = setEndpoint(VertID::src, end), 4 =
    /// setEndpoint(VertID::tar, end).
    #[wasm_bindgen(js_name = connSetEnds)]
    pub fn conn_set_ends(&mut self, key: u32, which: u32, ends: Vec<f64>) -> Res<()> {
        let c = self.conn(key)?;
        let a = self.dec_conn_end(&ends, 0)?;
        match which {
            0 => {
                let b = self.dec_conn_end(&ends, 10)?;
                c.set_endpoints(&mut self.router, &a, &b);
            }
            1 => c.set_source_endpoint(&mut self.router, &a),
            2 => c.set_dest_endpoint(&mut self.router, &a),
            3 => c.set_endpoint(&mut self.router, 1, &a),
            _ => c.set_endpoint(&mut self.router, 2, &a),
        }
        Ok(())
    }

    #[wasm_bindgen(js_name = connId)]
    pub fn conn_id(&self, key: u32) -> Res<u32> {
        Ok(self.conn(key)?.id(&self.router))
    }

    #[wasm_bindgen(js_name = connNeedsRepaint)]
    pub fn conn_needs_repaint(&self, key: u32) -> Res<bool> {
        Ok(self.conn(key)?.needs_repaint(&self.router))
    }

    #[wasm_bindgen(js_name = connRoute)]
    pub fn conn_route(&self, key: u32) -> Res<Vec<f64>> {
        Ok(enc_poly(self.conn(key)?.route(&self.router)))
    }

    #[wasm_bindgen(js_name = connDisplayRoute)]
    pub fn conn_display_route(&mut self, key: u32) -> Res<Vec<f64>> {
        let c = self.conn(key)?;
        Ok(enc_poly(c.display_route(&mut self.router)))
    }

    /// Installs (`on`) or removes the core callback that records this
    /// connector for `takeCallbacks()`.
    #[wasm_bindgen(js_name = connSetCallback)]
    pub fn conn_set_callback(&mut self, key: u32, on: bool) -> Res<()> {
        let c = self.conn(key)?;
        if on {
            let log = self.callbacks.clone();
            c.set_callback(&mut self.router, Some(std::boxed::Box::new(move |conn| log.borrow_mut().push(conn))));
        } else {
            c.set_callback(&mut self.router, None);
        }
        Ok(())
    }

    #[wasm_bindgen(js_name = connRoutingType)]
    pub fn conn_routing_type(&self, key: u32) -> Res<u32> {
        Ok(self.conn(key)?.routing_type(&self.router) as u32)
    }

    #[wasm_bindgen(js_name = connSetRoutingType)]
    pub fn conn_set_routing_type(&mut self, key: u32, type_: u32) -> Res<()> {
        let c = self.conn(key)?;
        let t = conn_type_of(type_)?;
        c.set_routing_type(&mut self.router, t);
        Ok(())
    }

    /// `[junctionKey, connKey]` (0 = nullptr).
    #[wasm_bindgen(js_name = connSplitAtSegment)]
    pub fn conn_split_at_segment(&mut self, key: u32, segment_n: f64) -> Res<Vec<u32>> {
        let c = self.conn(key)?;
        let (j, nc) = c.split_at_segment(&mut self.router, segment_n as usize);
        let jk = match j {
            Some(j) => self.key(Obj::Junction(j.0)),
            None => 0,
        };
        let ck = match nc {
            Some(nc) => self.key(Obj::Conn(nc)),
            None => 0,
        };
        Ok(vec![jk, ck])
    }

    #[wasm_bindgen(js_name = connSetRoutingCheckpoints)]
    pub fn conn_set_routing_checkpoints(&mut self, key: u32, cps: Vec<f64>) -> Res<()> {
        let c = self.conn(key)?;
        if cps.len() % 6 != 0 {
            return err("malformed Checkpoint list");
        }
        let mut list = Vec::with_capacity(cps.len() / 6);
        for i in (0..cps.len()).step_by(6) {
            let p = point_at(&cps, i)?;
            list.push(Checkpoint::with_directions(p, dirs_of(cps[i + 4])?, dirs_of(cps[i + 5])?));
        }
        c.set_routing_checkpoints(&mut self.router, &list);
        Ok(())
    }

    #[wasm_bindgen(js_name = connRoutingCheckpoints)]
    pub fn conn_routing_checkpoints(&self, key: u32) -> Res<Vec<f64>> {
        Ok(RawRouter::enc_checkpoints(&self.conn(key)?.routing_checkpoints(&self.router)))
    }

    #[wasm_bindgen(js_name = connEndpointConnEnds)]
    pub fn conn_endpoint_conn_ends(&mut self, key: u32) -> Res<Vec<f64>> {
        let c = self.conn(key)?;
        let (s, d) = c.endpoint_conn_ends(&self.router);
        let mut out = Vec::with_capacity(20);
        self.enc_conn_end(&s, &mut out);
        self.enc_conn_end(&d, &mut out);
        Ok(out)
    }

    #[wasm_bindgen(js_name = connSetFixedRoute)]
    pub fn conn_set_fixed_route(&mut self, key: u32, route: Vec<f64>) -> Res<()> {
        let c = self.conn(key)?;
        let p = dec_poly(&route)?;
        c.set_fixed_route(&mut self.router, &p);
        Ok(())
    }

    #[wasm_bindgen(js_name = connSetFixedExistingRoute)]
    pub fn conn_set_fixed_existing_route(&mut self, key: u32) -> Res<()> {
        let c = self.conn(key)?;
        c.set_fixed_existing_route(&mut self.router);
        Ok(())
    }

    #[wasm_bindgen(js_name = connHasFixedRoute)]
    pub fn conn_has_fixed_route(&self, key: u32) -> Res<bool> {
        Ok(self.conn(key)?.has_fixed_route(&self.router))
    }

    #[wasm_bindgen(js_name = connClearFixedRoute)]
    pub fn conn_clear_fixed_route(&mut self, key: u32) -> Res<()> {
        let c = self.conn(key)?;
        c.clear_fixed_route(&mut self.router);
        Ok(())
    }

    /// `ConnRef::set_route()`.
    #[wasm_bindgen(js_name = connSetRoute)]
    pub fn conn_set_route(&mut self, key: u32, route: Vec<f64>) -> Res<()> {
        let c = self.conn(key)?;
        let p = dec_poly(&route)?;
        self.router.conns[c].set_route(&p);
        Ok(())
    }

    #[wasm_bindgen(js_name = connCalcRouteDist)]
    pub fn conn_calc_route_dist(&mut self, key: u32) -> Res<()> {
        let c = self.conn(key)?;
        self.router.conns[c].calc_route_dist();
        Ok(())
    }

    #[wasm_bindgen(js_name = connMakeActive)]
    pub fn conn_make_active(&mut self, key: u32) -> Res<()> {
        let c = self.conn(key)?;
        if self.router.conns[c].m_active {
            return err("the connector is already active (COLA_ASSERT(!m_active))");
        }
        ConnRef::make_active(&mut self.router, c);
        Ok(())
    }

    #[wasm_bindgen(js_name = connMakeInactive)]
    pub fn conn_make_inactive(&mut self, key: u32) -> Res<()> {
        let c = self.conn(key)?;
        if !self.router.conns[c].m_active {
            return err("the connector is not active (COLA_ASSERT(m_active))");
        }
        ConnRef::make_inactive(&mut self.router, c);
        Ok(())
    }

    #[wasm_bindgen(js_name = connRemoveFromGraph)]
    pub fn conn_remove_from_graph(&mut self, key: u32) -> Res<()> {
        let c = self.conn(key)?;
        ConnRef::remove_from_graph(&mut self.router, c);
        Ok(())
    }

    #[wasm_bindgen(js_name = connIsInitialised)]
    pub fn conn_is_initialised(&self, key: u32) -> Res<bool> {
        let c = self.conn(key)?;
        Ok(self.router.conns[c].is_initialised())
    }

    #[wasm_bindgen(js_name = connMakePathInvalid)]
    pub fn conn_make_path_invalid(&mut self, key: u32) -> Res<()> {
        let c = self.conn(key)?;
        self.router.conns[c].make_path_invalid();
        Ok(())
    }

    #[wasm_bindgen(js_name = connSetHateCrossings)]
    pub fn conn_set_hate_crossings(&mut self, key: u32, value: bool) -> Res<()> {
        let c = self.conn(key)?;
        c.set_hate_crossings(&mut self.router, value);
        Ok(())
    }

    #[wasm_bindgen(js_name = connDoesHateCrossings)]
    pub fn conn_does_hate_crossings(&self, key: u32) -> Res<bool> {
        Ok(self.conn(key)?.does_hate_crossings(&self.router))
    }

    /// `x, y, id, vn` per point.
    #[wasm_bindgen(js_name = connPossibleDstPinPoints)]
    pub fn conn_possible_dst_pin_points(&self, key: u32) -> Res<Vec<f64>> {
        let ps = self.conn(key)?.possible_dst_pin_points(&self.router);
        let mut out = Vec::with_capacity(4 * ps.len());
        for p in &ps {
            push_point(&mut out, p);
        }
        Ok(out)
    }

    // --- ConnEnd ------------------------------------------------------------------

    /// `ConnEnd(shapeRef, classId)` (the position is the shape's now).
    #[wasm_bindgen(js_name = connEndShape)]
    pub fn conn_end_shape(&mut self, shape: u32, class_id: u32) -> Res<Vec<f64>> {
        let s = self.shape(shape)?;
        if class_id == 0 || class_id == CONNECTIONPIN_UNSET {
            return err("connectionPinClassID must be > 0 and != CONNECTIONPIN_UNSET (COLA_ASSERT)");
        }
        let ce = ConnEnd::from_shape_pin(&self.router, s, class_id);
        let mut out = Vec::with_capacity(10);
        self.enc_conn_end(&ce, &mut out);
        Ok(out)
    }

    #[wasm_bindgen(js_name = connEndJunction)]
    pub fn conn_end_junction(&mut self, junction: u32) -> Res<Vec<f64>> {
        let j = self.junction(junction)?;
        let ce = ConnEnd::from_junction(&self.router, j);
        let mut out = Vec::with_capacity(10);
        self.enc_conn_end(&ce, &mut out);
        Ok(out)
    }

    /// `ConnEnd::position()` as `x, y, id, vn`, then `directions()`.
    #[wasm_bindgen(js_name = connEndQuery)]
    pub fn conn_end_query(&self, end: Vec<f64>) -> Res<Vec<f64>> {
        let ce = self.dec_conn_end(&end, 0)?;
        let mut out = enc_point(&ce.position(&self.router));
        out.push(ce.directions(&self.router) as f64);
        Ok(out)
    }

    // --- ShapeConnectionPin ---------------------------------------------------------

    #[wasm_bindgen(js_name = pinNewShape)]
    #[allow(clippy::too_many_arguments)]
    pub fn pin_new_shape(
        &mut self,
        shape: u32,
        class_id: u32,
        x_offset: f64,
        y_offset: f64,
        proportional: bool,
        inside_offset: f64,
        vis_dirs: f64,
    ) -> Res<u32> {
        let s = self.shape(shape)?;
        if class_id == 0 {
            return err("classId must be > 0 (COLA_ASSERT(m_class_id > 0))");
        }
        check_offsets(x_offset, y_offset, inside_offset)?;
        let d = dirs_of(vis_dirs)?;
        let p = ShapeConnectionPin::new_shape(&mut self.router, s, class_id, x_offset, y_offset, proportional, inside_offset, d);
        Ok(self.key(Obj::Pin(p)))
    }

    /// The six-argument constructor (no `proportional` argument).
    #[wasm_bindgen(js_name = pinNewShapeNoProportional)]
    pub fn pin_new_shape_no_proportional(
        &mut self,
        shape: u32,
        class_id: u32,
        x_offset: f64,
        y_offset: f64,
        inside_offset: f64,
        vis_dirs: f64,
    ) -> Res<u32> {
        let s = self.shape(shape)?;
        if class_id == 0 {
            return err("classId must be > 0 (COLA_ASSERT(m_class_id > 0))");
        }
        check_offsets(x_offset, y_offset, inside_offset)?;
        let d = dirs_of(vis_dirs)?;
        let p = ShapeConnectionPin::new_shape_proportional(&mut self.router, s, class_id, x_offset, y_offset, inside_offset, d);
        Ok(self.key(Obj::Pin(p)))
    }

    #[wasm_bindgen(js_name = pinNewJunction)]
    pub fn pin_new_junction(&mut self, junction: u32, class_id: u32, vis_dirs: f64) -> Res<u32> {
        let j = self.junction(junction)?;
        let d = dirs_of(vis_dirs)?;
        let p = ShapeConnectionPin::new_junction(&mut self.router, j, class_id, d);
        Ok(self.key(Obj::Pin(p)))
    }

    /// `delete pin`.
    #[wasm_bindgen(js_name = pinDelete)]
    pub fn pin_delete(&mut self, key: u32) -> Res<()> {
        let p = self.pin(key)?;
        ShapeConnectionPin::delete(&mut self.router, p);
        Ok(())
    }

    #[wasm_bindgen(js_name = pinSetConnectionCost)]
    pub fn pin_set_connection_cost(&mut self, key: u32, cost: f64) -> Res<()> {
        let p = self.pin(key)?;
        if cost.is_nan() || cost < 0.0 {
            return err("cost must be >= 0 (COLA_ASSERT(cost >= 0))");
        }
        p.set_connection_cost(&mut self.router, cost);
        Ok(())
    }

    /// `position(newPoly)`; an empty `newPoly` means the default argument.
    #[wasm_bindgen(js_name = pinPosition)]
    pub fn pin_position(&self, key: u32, new_poly: Vec<f64>) -> Res<Vec<f64>> {
        let p = self.pin(key)?;
        let poly = dec_poly(&new_poly)?;
        Ok(enc_point(&self.router.pins[p].position(&self.router, Some(&poly))))
    }

    #[wasm_bindgen(js_name = pinDirections)]
    pub fn pin_directions(&self, key: u32) -> Res<u32> {
        Ok(self.pin(key)?.directions(&self.router))
    }

    #[wasm_bindgen(js_name = pinSetExclusive)]
    pub fn pin_set_exclusive(&mut self, key: u32, exclusive: bool) -> Res<()> {
        let p = self.pin(key)?;
        p.set_exclusive(&mut self.router, exclusive);
        Ok(())
    }

    #[wasm_bindgen(js_name = pinIsExclusive)]
    pub fn pin_is_exclusive(&self, key: u32) -> Res<bool> {
        Ok(self.pin(key)?.is_exclusive(&self.router))
    }

    #[wasm_bindgen(js_name = pinIds)]
    pub fn pin_ids(&self, key: u32) -> Res<Vec<u32>> {
        let (a, b) = self.pin(key)?.ids(&self.router);
        Ok(vec![a, b])
    }

    /// `0`: `operator==`, `1`: `operator<`.
    #[wasm_bindgen(js_name = pinCompare)]
    pub fn pin_compare(&self, lhs: u32, rhs: u32, less: bool) -> Res<bool> {
        let (a, b) = (self.pin(lhs)?, self.pin(rhs)?);
        let pins = &self.router.pins;
        Ok(if less {
            pins[a].less_than(&pins[b], &self.router.obstacles)
        } else {
            pins[a].equals(&pins[b], &self.router.obstacles)
        })
    }

    // --- ClusterRef -------------------------------------------------------------------

    #[wasm_bindgen(js_name = clusterNew)]
    pub fn cluster_new(&mut self, poly: Vec<f64>, id: u32) -> Res<u32> {
        let p = dec_poly(&poly)?;
        self.check_referencing(&p)?;
        let id = self.check_id(id)?;
        let c = ClusterRef::new(&mut self.router, &p, id);
        Ok(self.key(Obj::Cluster(c)))
    }

    #[wasm_bindgen(js_name = clusterSetNewPoly)]
    pub fn cluster_set_new_poly(&mut self, key: u32, poly: Vec<f64>) -> Res<()> {
        let c = self.cluster(key)?;
        let p = dec_poly(&poly)?;
        self.check_referencing(&p)?;
        c.set_new_poly(&mut self.router, &p);
        Ok(())
    }

    #[wasm_bindgen(js_name = clusterId)]
    pub fn cluster_id(&self, key: u32) -> Res<u32> {
        Ok(self.cluster(key)?.id(&self.router))
    }

    /// `polygon()` as `_id, n, (anchorKey, vn, x, y, id, vn) * n`: the
    /// `psRef` pair and the `psPoints` entry of each point.
    #[wasm_bindgen(js_name = clusterPolygon)]
    pub fn cluster_polygon(&mut self, key: u32) -> Res<Vec<f64>> {
        let c = self.cluster(key)?;
        let rp = self.router.clusters[c].polygon().clone();
        Ok(self.enc_referencing(&rp))
    }

    #[wasm_bindgen(js_name = clusterRectangularPolygon)]
    pub fn cluster_rectangular_polygon(&self, key: u32) -> Res<Vec<f64>> {
        Ok(enc_poly(self.cluster(key)?.rectangular_polygon(&self.router)))
    }

    #[wasm_bindgen(js_name = clusterMakeActive)]
    pub fn cluster_make_active(&mut self, key: u32) -> Res<()> {
        let c = self.cluster(key)?;
        if self.router.clusters[c].m_active {
            return err("the cluster is already active (COLA_ASSERT(!m_active))");
        }
        ClusterRef::make_active(&mut self.router, c);
        Ok(())
    }

    #[wasm_bindgen(js_name = clusterMakeInactive)]
    pub fn cluster_make_inactive(&mut self, key: u32) -> Res<()> {
        let c = self.cluster(key)?;
        if !self.router.clusters[c].m_active {
            return err("the cluster is not active (COLA_ASSERT(m_active))");
        }
        ClusterRef::make_inactive(&mut self.router, c);
        Ok(())
    }
}

impl RawRouter {
    /// `ReferencingPolygon(poly, router)` preconditions: every point with a
    /// non-zero id names an obstacle of the router (`COLA_ASSERT(polyPtr !=
    /// nullptr)`) whose polygon has a point `vn` (`at()`: `COLA_ASSERT(
    /// poly_index < poly.size())`).
    fn check_referencing(&self, p: &Polygon) -> Res<()> {
        for q in &p.ps {
            if q.id == 0 {
                continue;
            }
            let Some(&o) = self.router.m_obstacles.iter().find(|&&o| self.router.obstacles[o].m_id == q.id) else {
                return err(&format!("point id {} names no obstacle of the router (COLA_ASSERT(polyPtr != nullptr))", q.id));
            };
            if q.vn as usize >= self.router.obstacles[o].m_polygon.size() {
                return err(&format!(
                    "point vn {} is not a point of obstacle {} (COLA_ASSERT(poly_index < poly.size()))",
                    q.vn, q.id
                ));
            }
        }
        Ok(())
    }

    fn enc_referencing(&mut self, rp: &ReferencingPolygon) -> Vec<f64> {
        let mut out = Vec::with_capacity(2 + 6 * rp.ps_ref.len());
        out.push(rp._id as f64);
        out.push(rp.ps_ref.len() as f64);
        for (i, (anchor, vn)) in rp.ps_ref.iter().enumerate() {
            let k = self.opt_key_obstacle(*anchor);
            out.push(k as f64);
            out.push(*vn as f64);
            push_point(&mut out, &rp.ps_points[i]);
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Free functions (geometry, polygon methods)
// ---------------------------------------------------------------------------

#[wasm_bindgen(js_name = polySimplify)]
pub fn poly_simplify(poly: Vec<f64>) -> Res<Vec<f64>> {
    Ok(enc_poly(&dec_poly(&poly)?.simplify()))
}

#[wasm_bindgen(js_name = polyCurvedPolyline)]
pub fn poly_curved_polyline(poly: Vec<f64>, curve_amount: f64, closed: bool) -> Res<Vec<f64>> {
    Ok(enc_poly(&dec_poly(&poly)?.curved_polyline(curve_amount, closed)))
}

/// `x, y, id, vn` per point.
#[wasm_bindgen(js_name = polyCheckpointsOnSegment)]
pub fn poly_checkpoints_on_segment(poly: Vec<f64>, segment_lower_index: f64, index_modifier: i32) -> Res<Vec<f64>> {
    let ps = dec_poly(&poly)?.checkpoints_on_segment(segment_lower_index as usize, index_modifier);
    let mut out = Vec::with_capacity(4 * ps.len());
    for p in &ps {
        push_point(&mut out, p);
    }
    Ok(out)
}

#[wasm_bindgen(js_name = polyBoundingRectPolygon)]
pub fn poly_bounding_rect_polygon(poly: Vec<f64>) -> Res<Vec<f64>> {
    Ok(enc_poly(&dec_poly(&poly)?.bounding_rect_polygon()))
}

#[wasm_bindgen(js_name = polyOffsetBoundingBox)]
pub fn poly_offset_bounding_box(poly: Vec<f64>, offset: f64) -> Res<Vec<f64>> {
    Ok(enc_box(&dec_poly(&poly)?.offset_bounding_box(offset)))
}

#[wasm_bindgen(js_name = polyOffsetPolygon)]
pub fn poly_offset_polygon(poly: Vec<f64>, offset: f64) -> Res<Vec<f64>> {
    Ok(enc_poly(&dec_poly(&poly)?.offset_polygon(offset)))
}

/// Points of the geometry functions: `x, y` pairs in `ps`.
fn gp(ps: &[f64], i: usize) -> Point {
    pt(ps[2 * i], ps[2 * i + 1])
}

/// The free functions of upstream `geometry.h` (`which` selects one; the
/// adapter documents each).  Returns `[result, out1, out2]`.
#[wasm_bindgen(js_name = geometry)]
pub fn geometry_fn(which: u32, ps: Vec<f64>, a: f64, b: f64) -> Res<Vec<f64>> {
    let need = |n: usize| -> Res<()> {
        if ps.len() < 2 * n {
            err("geometry: too few points")
        } else {
            Ok(())
        }
    };
    let bool_ = |v: bool| vec![v as u8 as f64];
    Ok(match which {
        0 => {
            need(2)?;
            vec![geometry::euclidean_dist(&gp(&ps, 0), &gp(&ps, 1))]
        }
        1 => {
            need(2)?;
            vec![geometry::manhattan_dist(&gp(&ps, 0), &gp(&ps, 1))]
        }
        2 => {
            let mut poly = Polygon::new();
            poly.ps = (0..ps.len() / 2).map(|i| gp(&ps, i)).collect();
            vec![geometry::total_length(&poly)]
        }
        3 => {
            need(3)?;
            vec![geometry::angle(&gp(&ps, 0), &gp(&ps, 1), &gp(&ps, 2))]
        }
        4 => {
            need(4)?;
            bool_(geometry::segment_intersect(&gp(&ps, 0), &gp(&ps, 1), &gp(&ps, 2), &gp(&ps, 3)))
        }
        5 => {
            need(4)?;
            let mut seen = a != 0.0;
            let r = geometry::segment_shape_intersect(&gp(&ps, 0), &gp(&ps, 1), &gp(&ps, 2), &gp(&ps, 3), &mut seen);
            vec![r as u8 as f64, seen as u8 as f64]
        }
        6 => {
            // inPoly(poly, q, countBorder): the last point is q.
            need(1)?;
            let n = ps.len() / 2 - 1;
            let mut poly = Polygon::new();
            poly.ps = (0..n).map(|i| gp(&ps, i)).collect();
            bool_(geometry::in_poly(&poly, &gp(&ps, n), a != 0.0))
        }
        7 => {
            need(1)?;
            let n = ps.len() / 2 - 1;
            let mut poly = Polygon::new();
            poly.ps = (0..n).map(|i| gp(&ps, i)).collect();
            bool_(geometry::in_poly_gen(&poly, &gp(&ps, n)))
        }
        8 => {
            need(4)?;
            bool_(geometry::in_valid_region(a != 0.0, &gp(&ps, 0), &gp(&ps, 1), &gp(&ps, 2), &gp(&ps, 3)))
        }
        9 => {
            need(4)?;
            vec![geometry::corner_side(&gp(&ps, 0), &gp(&ps, 1), &gp(&ps, 2), &gp(&ps, 3)) as f64]
        }
        10 => {
            need(3)?;
            bool_(geometry::point_on_line(&gp(&ps, 0), &gp(&ps, 1), &gp(&ps, 2), a))
        }
        11 => {
            need(3)?;
            bool_(geometry::colinear(&gp(&ps, 0), &gp(&ps, 1), &gp(&ps, 2), a))
        }
        12 => {
            need(3)?;
            bool_(geometry::in_between(&gp(&ps, 0), &gp(&ps, 1), &gp(&ps, 2)))
        }
        13 => {
            need(3)?;
            if a.is_nan() || a < 0.0 {
                return err("maybeZero must be >= 0 (COLA_ASSERT(maybeZero >= 0))");
            }
            vec![geometry::vec_dir(&gp(&ps, 0), &gp(&ps, 1), &gp(&ps, 2), a) as f64]
        }
        14 => {
            need(3)?;
            let p = geometry::projection(&gp(&ps, 0), &gp(&ps, 1), &gp(&ps, 2));
            vec![p.x, p.y]
        }
        15 | 16 => {
            need(4)?;
            let (mut x, mut y) = (a, b);
            let f = if which == 15 { geometry::segment_intersect_point } else { geometry::ray_intersect_point };
            let r = f(&gp(&ps, 0), &gp(&ps, 1), &gp(&ps, 2), &gp(&ps, 3), &mut x, &mut y);
            vec![r as f64, x, y]
        }
        17 => {
            need(1)?;
            vec![geometry::rotational_angle(&gp(&ps, 0))]
        }
        _ => return err("geometry: unknown function"),
    })
}
