//! Faithful port of upstream `libavoid/connector.{h,cpp}`.
//!
//! This file currently contains the `ConnRef` class, `Checkpoint`, and
//! `validateBendPoint`.  The second half of connector.cpp
//! (`PtOrder`, `ConnectorCrossings`, `splitBranchingSegments`, ...) is
//! declared at the bottom with `todo!` bodies for a later port.
//!
//! * `ConnRef *` is a [`ConnIdx`] into `Router::conns`.  Public upstream
//!   methods are available as methods on the handle
//!   (`conn.set_source_endpoint(&mut router, &end)`), internal ones as
//!   associated functions `ConnRef::xxx(router, conn, ..)`.
//! * `m_connrefs_pos` (iterator into `Router::connRefs`) is not stored;
//!   `Router::conn_refs` is a `Vec` and `makeInactive` erases the unique
//!   entry by value.
//! * The owned heap ConnEnds `m_src_connend`/`m_dst_connend` are stored
//!   inline as `Option<ConnEnd>` (see `connend.rs` for their identity).
//! * Callbacks (`setCallback(void (*)(void *), void *)`) are boxed
//!   `FnMut(ConnIdx)` closures ([`ConnCallback`]).  They are invoked from
//!   `Router::rerouteAndCallbackConnectors()` at the same moment as
//!   upstream, but cannot access the router (it is mutably borrowed);
//!   additionally the router records every callback in
//!   `Router::m_callback_log` (cleared at the start of each
//!   `rerouteAndCallbackConnectors`), which hosts can inspect after
//!   `process_transaction()` returns.

#![allow(dead_code)]
#![allow(clippy::new_ret_no_self, clippy::too_many_arguments)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::avoid::arena::{ConnIdx, ObstacleIdx, RerouteFlagIdx, VertIdx};
use crate::avoid::connend::{ConnDirFlags, ConnEnd, ConnEndRef, CONN_DIR_ALL, CONN_DIR_NONE};
use crate::avoid::geometry::{euclidean_dist, manhattan_dist, vec_dir};
use crate::avoid::geomtypes::{
    Point, PointOrd, PolyLine, Polygon, PolygonInterface, K_UNASSIGNED_VERTEX_NUMBER, XDIM, YDIM,
};
use crate::avoid::graph::EdgeInf;
use crate::avoid::junction::JunctionRef;
use crate::avoid::makepath::AStarPath;
use crate::avoid::router::Router;
use crate::avoid::vertices::{VertID, VertIDProps, VertInf};
use crate::avoid::visibility::vertex_visibility;

/// Describes the type of routing that is performed for each connector.
// upstream: connector.h enum ConnType
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConnType {
    ConnTypeNone = 0,
    ConnTypePolyLine = 1,
    ConnTypeOrthogonal = 2,
}

/// A checkpoint is a point that the route for a particular connector must
/// visit.  They may optionally be given an arrival/departure direction.
// upstream: connector.h class Checkpoint
#[derive(Clone, Copy, Debug)]
pub struct Checkpoint {
    pub point: Point,
    pub arrival_directions: ConnDirFlags,
    pub departure_directions: ConnDirFlags,
}

impl Default for Checkpoint {
    // upstream: connector.h Checkpoint::Checkpoint()
    fn default() -> Self {
        Checkpoint {
            point: Point::default(),
            arrival_directions: CONN_DIR_ALL,
            departure_directions: CONN_DIR_ALL,
        }
    }
}

impl Checkpoint {
    /// A checkpoint with no direction restrictions.
    // upstream: connector.h Checkpoint::Checkpoint(const Point&)
    pub fn new(p: Point) -> Checkpoint {
        Checkpoint {
            point: p,
            arrival_directions: CONN_DIR_ALL,
            departure_directions: CONN_DIR_ALL,
        }
    }

    // upstream: connector.h Checkpoint::Checkpoint(const Point&, ConnDirFlags, ConnDirFlags)
    pub fn with_directions(p: Point, ad: ConnDirFlags, dd: ConnDirFlags) -> Checkpoint {
        Checkpoint {
            point: p,
            arrival_directions: ad,
            departure_directions: dd,
        }
    }
}

/// Connector callback (upstream `void (*)(void *)` + `void *`): called with
/// the connector's handle when it has been rerouted.
pub struct ConnCallback(pub std::boxed::Box<dyn FnMut(ConnIdx)>);

impl fmt::Debug for ConnCallback {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ConnCallback(..)")
    }
}

/// The ConnRef class represents a connector object.
// upstream: connector.h class ConnRef
#[derive(Debug)]
pub struct ConnRef {
    /// Monotonic creation serial (`// PTR-ORDER:`).
    pub serial: u64,
    pub m_id: u32,
    pub m_type: ConnType,
    pub m_reroute_flag_ptr: Option<RerouteFlagIdx>,
    pub m_needs_reroute_flag: bool,
    pub m_false_path: bool,
    pub m_needs_repaint: bool,
    pub m_active: bool,
    /// Never initialised or read upstream; false here.
    pub m_initialised: bool,
    pub m_hate_crossings: bool,
    pub m_has_fixed_route: bool,
    pub m_route: PolyLine,
    pub m_display_route: Polygon,
    pub m_route_dist: f64,
    pub m_src_vert: Option<VertIdx>,
    pub m_dst_vert: Option<VertIdx>,
    /// Uninitialised by the two-ConnEnd upstream constructor; None here.
    pub m_start_vert: Option<VertIdx>,
    /// `m_callback_func` + `m_connector`.
    pub m_callback: Option<ConnCallback>,
    pub m_src_connend: Option<ConnEnd>,
    pub m_dst_connend: Option<ConnEnd>,
    pub m_checkpoints: Vec<Checkpoint>,
    pub m_checkpoint_vertices: Vec<VertIdx>,
}

// upstream: connector.cpp midpoint()
pub fn midpoint(a: Point, b: Point) -> Point {
    let mut midpoint = Point::default();

    midpoint.x = (a.x + b.x) / 2.0;
    midpoint.y = (a.y + b.y) / 2.0;

    midpoint
}

impl ConnRef {
    fn blank(router: &mut Router, m_id: u32) -> ConnRef {
        ConnRef {
            serial: router.next_serial(),
            m_id,
            m_type: router.valid_conn_type(ConnType::ConnTypeNone),
            m_reroute_flag_ptr: None,
            m_needs_reroute_flag: true,
            m_false_path: false,
            m_needs_repaint: false,
            m_active: false,
            m_initialised: false,
            m_hate_crossings: false,
            m_has_fixed_route: false,
            m_route: Polygon::new(),
            m_display_route: Polygon::new(),
            m_route_dist: 0.0,
            m_src_vert: None,
            m_dst_vert: None,
            m_start_vert: None,
            m_callback: None,
            m_src_connend: None,
            m_dst_connend: None,
            m_checkpoints: Vec::new(),
            m_checkpoint_vertices: Vec::new(),
        }
    }

    /// Constructs a connector with no endpoints specified.  `id` 0 means
    /// "assign one".
    // upstream: connector.cpp ConnRef::ConnRef(Router *, const unsigned int)
    pub fn new(router: &mut Router, id: u32) -> ConnIdx {
        // (upstream assigns the id in the constructor body, after the
        // member initialisers, which do not allocate.)
        let mut blank = ConnRef::blank(router, 0);
        blank.m_id = router.assign_id(id);
        let c = router.conns.alloc(blank);

        // TODO: Store endpoints and details.
        router.conns[c].m_route.clear();

        let flag = router.m_conn_reroute_flags.add_conn(c);
        router.conns[c].m_reroute_flag_ptr = Some(flag);
        c
    }

    /// Constructs a connector with endpoints specified.  `id` 0 means
    /// "assign one".
    // upstream: connector.cpp ConnRef::ConnRef(Router *, const ConnEnd&, const ConnEnd&, const unsigned int)
    pub fn new_with_ends(router: &mut Router, src: &ConnEnd, dst: &ConnEnd, id: u32) -> ConnIdx {
        let mut blank = ConnRef::blank(router, 0);
        blank.m_id = router.assign_id(id);
        let c = router.conns.alloc(blank);
        router.conns[c].m_route.clear();

        // Set endpoint values.
        ConnRef::set_endpoints(router, c, src, dst);

        let flag = router.m_conn_reroute_flags.add_conn(c);
        router.conns[c].m_reroute_flag_ptr = Some(flag);
        c
    }

    /// Upstream `delete connRef` (only valid from the router).
    // upstream: connector.cpp ConnRef::~ConnRef()
    pub fn delete(router: &mut Router, c: ConnIdx) {
        if !router.m_currently_calling_destructors {
            eprint!("ERROR: ConnRef::~ConnRef() shouldn't be called directly.\n");
            eprint!("       It is owned by the router.  Call Router::deleteConnector() instead.\n");
            panic!("ConnRef deleted directly instead of through the Router");
        }

        router.m_conn_reroute_flags.remove_conn(c);

        router.remove_object_from_queued_actions(crate::avoid::actioninfo::ActionObj::Conn(c));

        router.conns[c].free_routes();

        if let Some(v) = router.conns[c].m_src_vert {
            VertInf::remove_from_graph(router, v, true);
            router.vertices.remove_vertex(&mut router.vert_infs, Some(v));
            VertInf::delete(router, v);
            router.conns[c].m_src_vert = None;
        }
        if router.conns[c].m_src_connend.is_some() {
            let r = ConnEndRef { conn: c, end: VertID::SRC as u32 };
            ConnEnd::disconnect(router, r, false);
            ConnEnd::free_active_pin(router, r);
            router.conns[c].m_src_connend = None;
        }

        if let Some(v) = router.conns[c].m_dst_vert {
            VertInf::remove_from_graph(router, v, true);
            router.vertices.remove_vertex(&mut router.vert_infs, Some(v));
            VertInf::delete(router, v);
            router.conns[c].m_dst_vert = None;
        }
        if router.conns[c].m_dst_connend.is_some() {
            let r = ConnEndRef { conn: c, end: VertID::TAR as u32 };
            ConnEnd::disconnect(router, r, false);
            ConnEnd::free_active_pin(router, r);
            router.conns[c].m_dst_connend = None;
        }

        // Clear checkpoint vertices.
        let cps = std::mem::take(&mut router.conns[c].m_checkpoint_vertices);
        for v in cps {
            VertInf::remove_from_graph(router, v, true);
            router.vertices.remove_vertex(&mut router.vert_infs, Some(v));
            VertInf::delete(router, v);
        }

        if router.conns[c].m_active {
            ConnRef::make_inactive(router, c);
        }
        router.conns.free(c);
    }

    // upstream: connector.cpp ConnRef::routingType()
    #[inline]
    pub fn routing_type(&self) -> ConnType {
        self.m_type
    }

    // upstream: connector.cpp ConnRef::setRoutingType()
    pub fn set_routing_type(router: &mut Router, c: ConnIdx, type_: ConnType) {
        let type_ = router.valid_conn_type(type_);
        if router.conns[c].m_type != type_ {
            router.conns[c].m_type = type_;

            router.conns[c].make_path_invalid();

            router.modify_connector(c);
        }
    }

    // upstream: connector.cpp ConnRef::routingCheckpoints()
    pub fn routing_checkpoints(&self) -> Vec<Checkpoint> {
        self.m_checkpoints.clone()
    }

    // upstream: connector.cpp ConnRef::setRoutingCheckpoints()
    pub fn set_routing_checkpoints(router: &mut Router, c: ConnIdx, checkpoints: &[Checkpoint]) {
        router.conns[c].m_checkpoints = checkpoints.to_vec();

        // Clear previous checkpoint vertices.
        let old = std::mem::take(&mut router.conns[c].m_checkpoint_vertices);
        for v in old {
            VertInf::remove_from_graph(router, v, true);
            router.vertices.remove_vertex(&mut router.vert_infs, Some(v));
            VertInf::delete(router, v);
        }

        let m_id = router.conns[c].m_id;
        for i in 0..router.conns[c].m_checkpoints.len() {
            let pt_id = VertID::new(
                m_id,
                (2 + i) as u16,
                VertID::PROP_CONN_POINT | VertID::PROP_CONN_CHECKPOINT,
            );
            let p = router.conns[c].m_checkpoints[i].point;
            let vertex = VertInf::new(router, pt_id, p, true);
            router.vert_infs[vertex].vis_directions = CONN_DIR_ALL;

            router.conns[c].m_checkpoint_vertices.push(vertex);
        }
        if router.m_allows_polyline_routing {
            for i in 0..router.conns[c].m_checkpoints.len() {
                let v = router.conns[c].m_checkpoint_vertices[i];
                vertex_visibility(router, v, None, true, true);
            }
        }
    }

    // upstream: connector.cpp ConnRef::common_updateEndPoint()
    pub fn common_update_end_point(router: &mut Router, c: ConnIdx, type_: u32, mut conn_end: ConnEnd) {
        let point = conn_end.position(router);
        debug_assert!((type_ == VertID::SRC as u32) || (type_ == VertID::TAR as u32));

        // The connEnd is a copy of a ConnEnd that will get disconnected,
        // so don't leave it looking like it is still connected.
        conn_end.m_conn_ref = None;

        if !router.conns[c].m_active {
            ConnRef::make_active(router, c);
        }

        let altered: VertIdx;

        let mut properties: VertIDProps = VertID::PROP_CONN_POINT;
        if conn_end.is_pin_connection() {
            properties |= VertID::PROP_DUMMY_PIN_HELPER;
        }
        let pt_id = VertID::new(router.conns[c].m_id, type_ as u16, properties);
        let is_src = type_ == VertID::SRC as u32;
        let vert = if is_src {
            router.conns[c].m_src_vert
        } else {
            router.conns[c].m_dst_vert
        };
        let vert = match vert {
            Some(v) => {
                router.vert_infs[v].reset(pt_id, point);
                v
            }
            None => {
                let v = VertInf::new(router, pt_id, point, true);
                if is_src {
                    router.conns[c].m_src_vert = Some(v);
                } else {
                    router.conns[c].m_dst_vert = Some(v);
                }
                v
            }
        };
        router.vert_infs[vert].vis_directions = conn_end.directions(router);

        let this_end = ConnEndRef { conn: c, end: type_ };
        let has_owned = if is_src {
            router.conns[c].m_src_connend.is_some()
        } else {
            router.conns[c].m_dst_connend.is_some()
        };
        if has_owned {
            ConnEnd::disconnect(router, this_end, false);
            ConnEnd::free_active_pin(router, this_end);
            if is_src {
                router.conns[c].m_src_connend = None;
            } else {
                router.conns[c].m_dst_connend = None;
            }
        }
        if conn_end.is_pin_connection() {
            // `new ConnEnd(connEnd)`: a fresh heap object.
            let mut owned = conn_end.clone();
            owned.serial = router.next_serial();
            if is_src {
                router.conns[c].m_src_connend = Some(owned);
            } else {
                router.conns[c].m_dst_connend = Some(owned);
            }
            ConnEnd::connect(router, this_end, c);
            // Don't need this to have visibility since we won't
            // be connecting to it.
            router.vert_infs[vert].vis_directions = CONN_DIR_NONE;
        }

        altered = vert;

        // XXX: Seems to be faster to just remove the edges and recreate
        let is_conn = true;
        VertInf::remove_from_graph(router, altered, is_conn);

        router.conns[c].make_path_invalid();
        router.set_static_graph_invalidated(true);
    }

    /// Sets both new source and destination endpoints for this connector.
    // upstream: connector.cpp ConnRef::setEndpoints()
    pub fn set_endpoints(router: &mut Router, c: ConnIdx, src_point: &ConnEnd, dst_point: &ConnEnd) {
        router.modify_connector_end(c, VertID::SRC as u32, src_point, false);
        router.modify_connector_end(c, VertID::TAR as u32, dst_point, false);
    }

    // upstream: connector.cpp ConnRef::setEndpoint(const unsigned int, const ConnEnd&)
    pub fn set_endpoint(router: &mut Router, c: ConnIdx, type_: u32, conn_end: &ConnEnd) {
        router.modify_connector_end(c, type_, conn_end, false);
    }

    // upstream: connector.cpp ConnRef::setSourceEndpoint()
    pub fn set_source_endpoint(router: &mut Router, c: ConnIdx, src_point: &ConnEnd) {
        router.modify_connector_end(c, VertID::SRC as u32, src_point, false);
    }

    // upstream: connector.cpp ConnRef::setDestEndpoint()
    pub fn set_dest_endpoint(router: &mut Router, c: ConnIdx, dst_point: &ConnEnd) {
        router.modify_connector_end(c, VertID::TAR as u32, dst_point, false);
    }

    /// Given the start or end vertex of a connector, returns the ConnEnd
    /// that can be used to reproduce that endpoint.  This is used for
    /// hyperedge routing.  Returns false (leaving `conn_end` alone) if the
    /// vertex is not an endpoint of this connector.
    // upstream: connector.cpp ConnRef::getConnEndForEndpointVertex()
    pub fn get_conn_end_for_endpoint_vertex(
        &self,
        router: &Router,
        vertex: Option<VertIdx>,
        conn_end: &mut ConnEnd,
    ) -> bool {
        let vertex = match vertex {
            None => {
                eprint!(
                    "Warning: In ConnRef::getConnEndForEndpointVertex():\n         ConnEnd for connector {} is uninitialised.  It may have been\n         set but Router::processTrancaction has not yet been called.\n",
                    self.m_id as i32
                );
                return false;
            }
            Some(v) => v,
        };

        if Some(vertex) == self.m_src_vert {
            if let Some(ce) = &self.m_src_connend {
                *conn_end = ce.clone();
            } else {
                let sv = &router.vert_infs[self.m_src_vert.unwrap()];
                *conn_end = ConnEnd::from_point_dirs(Point::new(sv.point.x, sv.point.y), sv.vis_directions);
            }
            return true;
        } else if Some(vertex) == self.m_dst_vert {
            if let Some(ce) = &self.m_dst_connend {
                *conn_end = ce.clone();
            } else {
                let dv = &router.vert_infs[self.m_dst_vert.unwrap()];
                *conn_end = ConnEnd::from_point_dirs(Point::new(dv.point.x, dv.point.y), dv.vis_directions);
            }
            return true;
        }
        false
    }

    // upstream: connector.cpp ConnRef::updateEndPoint()
    pub fn update_end_point(router: &mut Router, c: ConnIdx, type_: u32, conn_end: &ConnEnd) {
        ConnRef::common_update_end_point(router, c, type_, conn_end.clone());

        if router.conns[c].m_has_fixed_route {
            // Don't need to continue and compute visibility if route is fixed.
            return;
        }

        if router.m_allows_polyline_routing {
            let known_new = true;
            let gen_contains = true;
            let conn = &router.conns[c];
            if type_ == VertID::SRC as u32 {
                let dummy_src = conn.m_src_connend.as_ref().is_some_and(|ce| ce.is_pin_connection());
                if !dummy_src {
                    // Only generate visibility if not attached to a pin.
                    let (s, d) = (conn.m_src_vert.unwrap(), conn.m_dst_vert);
                    vertex_visibility(router, s, d, known_new, gen_contains);
                }
            } else {
                let dummy_dst = conn.m_dst_connend.as_ref().is_some_and(|ce| ce.is_pin_connection());
                if !dummy_dst {
                    // Only generate visibility if not attached to a pin.
                    let (d, s) = (conn.m_dst_vert.unwrap(), conn.m_src_vert);
                    vertex_visibility(router, d, s, known_new, gen_contains);
                }
            }
        }
    }

    // (upstream: connector.cpp ConnRef::outputCode() — debug code output,
    // not ported.)

    // upstream: connector.cpp ConnRef::endpointAnchors()
    pub fn endpoint_anchors(&self) -> (Option<ObstacleIdx>, Option<ObstacleIdx>) {
        let mut anchors = (None, None);

        if let Some(ce) = &self.m_src_connend {
            anchors.0 = ce.m_anchor_obj;
        }
        if let Some(ce) = &self.m_dst_connend {
            anchors.1 = ce.m_anchor_obj;
        }
        anchors
    }

    /// Returns ConnEnds specifying what this connector is attached to.
    // upstream: connector.cpp ConnRef::endpointConnEnds()
    pub fn endpoint_conn_ends(&self, router: &Router) -> (ConnEnd, ConnEnd) {
        let mut endpoints = (ConnEnd::new(), ConnEnd::new());
        self.get_conn_end_for_endpoint_vertex(router, self.m_src_vert, &mut endpoints.0);
        self.get_conn_end_for_endpoint_vertex(router, self.m_dst_vert, &mut endpoints.1);
        endpoints
    }

    // upstream: connector.cpp ConnRef::setEndpoint(const unsigned int, const VertID&, Point *)
    pub fn set_endpoint_vertex(
        router: &mut Router,
        c: ConnIdx,
        type_: u32,
        point_id: &VertID,
        point_suggestion: Option<&Point>,
    ) -> bool {
        let v_inf = match router.vertices.get_vertex_by_id(&router.vert_infs, point_id) {
            None => return false,
            Some(v) => v,
        };
        let point = router.vert_infs[v_inf].point;
        if let Some(sugg) = point_suggestion {
            if euclidean_dist(&point, sugg) > 0.5 {
                return false;
            }
        }

        ConnRef::common_update_end_point(router, c, type_, ConnEnd::from_point(point));

        // Give this visibility just to the point it is over.
        let from = if type_ == VertID::SRC as u32 {
            router.conns[c].m_src_vert.unwrap()
        } else {
            router.conns[c].m_dst_vert.unwrap()
        };
        let edge = EdgeInf::new(router, from, v_inf, false);
        // XXX: We should be able to set this to zero, but can't due to
        //      assumptions elsewhere in the code.
        EdgeInf::set_dist(router, edge, 0.001);

        router.process_transaction();
        true
    }

    // upstream: connector.cpp ConnRef::makeActive()
    pub fn make_active(router: &mut Router, c: ConnIdx) {
        debug_assert!(!router.conns[c].m_active);

        // Add to connRefs list.
        router.conn_refs.insert(0, c);
        router.conns[c].m_active = true;
    }

    // upstream: connector.cpp ConnRef::freeActivePins()
    pub fn free_active_pins(router: &mut Router, c: ConnIdx) {
        if router.conns[c].m_src_connend.is_some() {
            ConnEnd::free_active_pin(router, ConnEndRef { conn: c, end: VertID::SRC as u32 });
        }
        if router.conns[c].m_dst_connend.is_some() {
            ConnEnd::free_active_pin(router, ConnEndRef { conn: c, end: VertID::TAR as u32 });
        }
    }

    // upstream: connector.cpp ConnRef::makeInactive()
    pub fn make_inactive(router: &mut Router, c: ConnIdx) {
        debug_assert!(router.conns[c].m_active);

        // Remove from connRefs list.
        let pos = router
            .conn_refs
            .iter()
            .position(|&x| x == c)
            .expect("active connector missing from connRefs");
        router.conn_refs.remove(pos);
        router.conns[c].m_active = false;
    }

    // upstream: connector.cpp ConnRef::freeRoutes()
    pub fn free_routes(&mut self) {
        self.m_route.clear();
        self.m_display_route.clear();
    }

    /// Returns a reference to the current raw "debug" route for the
    /// connector.
    // upstream: connector.cpp ConnRef::route()
    #[inline]
    pub fn route(&self) -> &PolyLine {
        &self.m_route
    }

    // upstream: connector.cpp ConnRef::routeRef()
    #[inline]
    pub fn route_ref(&mut self) -> &mut PolyLine {
        &mut self.m_route
    }

    // upstream: connector.cpp ConnRef::set_route()
    pub fn set_route(&mut self, route: &PolyLine) {
        // (upstream rejects `route` aliasing m_display_route; impossible
        // here.)
        self.m_display_route.ps = route.ps.clone();
    }

    // upstream: connector.cpp ConnRef::setFixedExistingRoute()
    pub fn set_fixed_existing_route(router: &mut Router, c: ConnIdx) {
        debug_assert!(router.conns[c].m_route.size() >= 2);
        router.conns[c].m_has_fixed_route = true;
        router.register_settings_change();
    }

    // upstream: connector.cpp ConnRef::setFixedRoute()
    pub fn set_fixed_route(router: &mut Router, c: ConnIdx, route: &PolyLine) {
        if route.size() >= 2 {
            // Set endpoints based on the fixed route incase the
            // fixed route is later cleared.
            let s = ConnEnd::from_point(route.ps[0]);
            let d = ConnEnd::from_point(route.ps[route.size() - 1]);
            ConnRef::set_endpoints(router, c, &s, &d);
        }
        let conn = &mut router.conns[c];
        conn.m_has_fixed_route = true;
        conn.m_route = route.clone();
        conn.m_display_route = conn.m_route.simplify();
        router.register_settings_change();
    }

    // upstream: connector.cpp ConnRef::hasFixedRoute()
    #[inline]
    pub fn has_fixed_route(&self) -> bool {
        self.m_has_fixed_route
    }

    // upstream: connector.cpp ConnRef::clearFixedRoute()
    pub fn clear_fixed_route(router: &mut Router, c: ConnIdx) {
        router.conns[c].m_has_fixed_route = false;
        router.conns[c].make_path_invalid();
        router.register_settings_change();
    }

    /// Returns a reference to the current display version of the route
    /// for the connector (computing it from the raw route if unset).
    // upstream: connector.cpp ConnRef::displayRoute()
    pub fn display_route(&mut self) -> &Polygon {
        if self.m_display_route.empty() {
            // No displayRoute is set.  Simplify the current route to get it.
            self.m_display_route = self.m_route.simplify();
        }
        &self.m_display_route
    }

    // upstream: connector.cpp ConnRef::calcRouteDist()
    pub fn calc_route_dist(&mut self) {
        let dist: fn(&Point, &Point) -> f64 = if self.m_type == ConnType::ConnTypePolyLine {
            euclidean_dist
        } else {
            manhattan_dist
        };

        self.m_route_dist = 0.0;
        for i in 1..self.m_route.size() {
            self.m_route_dist += dist(&self.m_route.at(i), &self.m_route.at(i - 1));
        }
    }

    // upstream: connector.cpp ConnRef::needsRepaint()
    #[inline]
    pub fn needs_repaint(&self) -> bool {
        self.m_needs_repaint
    }

    // upstream: connector.cpp ConnRef::id()
    #[inline]
    pub fn id(&self) -> u32 {
        self.m_id
    }

    /// Splits a connector in the centre of the segment-th segment and
    /// creates a junction point there as well as a second connector.
    // upstream: connector.cpp ConnRef::splitAtSegment()
    pub fn split_at_segment(
        router: &mut Router,
        c: ConnIdx,
        segment_n: usize,
    ) -> (Option<JunctionRef>, Option<ConnIdx>) {
        let mut new_conn = None;
        let mut new_junction = None;

        if router.conns[c].m_display_route.size() > segment_n {
            // Position the junction at the midpoint of the desired segment.
            let (a, b) = {
                let dr = &router.conns[c].m_display_route;
                (dr.at(segment_n - 1), dr.at(segment_n))
            };
            let junction_pos = midpoint(a, b);

            // Create the new junction.
            let nj = JunctionRef::new(router, junction_pos, 0);
            router.add_junction(nj);
            nj.prefer_orthogonal_dimension(router, if a.x == b.x { YDIM } else { XDIM });
            new_junction = Some(nj);

            // Create a new connection routing from the junction to the original
            // connector's endpoint.
            let new_conn_src = ConnEnd::from_junction(router, nj);
            let new_conn_dst = router.conns[c]
                .m_dst_connend
                .clone()
                .expect("ConnRef::splitAtSegment(): destination is not a pin connection");
            new_conn = Some(ConnRef::new_with_ends(router, &new_conn_src, &new_conn_dst, 0));

            // Reroute the endpoint of the original connector to attach to the
            // new junction.
            let old_conn_dst = ConnEnd::from_junction(router, nj);
            ConnRef::set_dest_endpoint(router, c, &old_conn_dst);
        }

        (new_junction, new_conn)
    }

    // upstream: connector.cpp ConnRef::src()
    #[inline]
    pub fn src(&self) -> Option<VertIdx> {
        self.m_src_vert
    }

    // upstream: connector.cpp ConnRef::dst()
    #[inline]
    pub fn dst(&self) -> Option<VertIdx> {
        self.m_dst_vert
    }

    // upstream: connector.cpp ConnRef::start()
    #[inline]
    pub fn start(&self) -> Option<VertIdx> {
        self.m_start_vert
    }

    // upstream: connector.cpp ConnRef::isInitialised()
    #[inline]
    pub fn is_initialised(&self) -> bool {
        self.m_active
    }

    // upstream: connector.cpp ConnRef::unInitialise()
    pub fn un_initialise(router: &mut Router, c: ConnIdx) {
        let (s, d) = (router.conns[c].m_src_vert, router.conns[c].m_dst_vert);
        router.vertices.remove_vertex(&mut router.vert_infs, s);
        router.vertices.remove_vertex(&mut router.vert_infs, d);
        ConnRef::make_inactive(router, c);
    }

    // upstream: connector.cpp ConnRef::removeFromGraph()
    pub fn remove_from_graph(router: &mut Router, c: ConnIdx) {
        if let Some(v) = router.conns[c].m_src_vert {
            VertInf::remove_from_graph(router, v, true);
        }

        if let Some(v) = router.conns[c].m_dst_vert {
            VertInf::remove_from_graph(router, v, true);
        }
    }

    // upstream: connector.cpp ConnRef::setCallback()
    pub fn set_callback(&mut self, cb: Option<ConnCallback>) {
        self.m_callback = cb;
    }

    /// Invokes the callback (if any) with this connector's handle.
    // upstream: connector.cpp ConnRef::performCallback()
    pub fn perform_callback(router: &mut Router, c: ConnIdx) {
        if let Some(cb) = router.conns[c].m_callback.as_mut() {
            (cb.0)(c);
            router.m_callback_log.push(c);
        }
    }

    // upstream: connector.cpp ConnRef::makePathInvalid()
    #[inline]
    pub fn make_path_invalid(&mut self) {
        self.m_needs_reroute_flag = true;
    }

    // upstream: connector.cpp ConnRef::assignConnectionPinVisibility()
    pub fn assign_connection_pin_visibility(router: &mut Router, c: ConnIdx, connect: bool) -> (bool, bool) {
        // XXX This is kind of a hack for connection pins.  Probably we want to
        //     not use m_src_vert and m_dst_vert.  For the moment we will clear
        //     their visibility and give them visibility to the pins.
        let dummy_src = router.conns[c].m_src_connend.as_ref().is_some_and(|ce| ce.is_pin_connection());
        if dummy_src {
            let sv = router.conns[c].m_src_vert.unwrap();
            VertInf::remove_from_graph(router, sv, true);
            if connect {
                let ce = router.conns[c].m_src_connend.clone().unwrap();
                let dv = router.conns[c].m_dst_vert.expect("no dst vertex");
                ce.assign_pin_visibility_to(router, sv, dv);
            }
        }
        let dummy_dst = router.conns[c].m_dst_connend.as_ref().is_some_and(|ce| ce.is_pin_connection());
        if dummy_dst {
            let dv = router.conns[c].m_dst_vert.unwrap();
            VertInf::remove_from_graph(router, dv, true);
            if connect {
                let ce = router.conns[c].m_dst_connend.clone().unwrap();
                let sv = router.conns[c].m_src_vert.expect("no src vertex");
                ce.assign_pin_visibility_to(router, dv, sv);
            }
        }

        (dummy_src, dummy_dst)
    }

    // upstream: connector.cpp ConnRef::generatePath()
    pub fn generate_path(router: &mut Router, c: ConnIdx) -> bool {
        // XXX Currently rubber-band routing only works when dragging the
        //     destination point of a connector, but not the source.  The code
        //     needs to be reworked to work in both directions.

        {
            let conn = &router.conns[c];
            if !conn.m_false_path && !conn.m_needs_reroute_flag {
                // This connector is up to date.
                return false;
            }

            if conn.m_dst_vert.is_none() || conn.m_src_vert.is_none() {
                // Connector is not fully initialised.
                return false;
            }
        }

        {
            let conn = &mut router.conns[c];
            conn.m_false_path = false;
            conn.m_needs_reroute_flag = false;

            conn.m_start_vert = conn.m_src_vert;
        }

        // Some connectors may attach to connection pins, which means they route
        // to the closest of multiple pins on a shape.  How we handle this is to
        // add a dummy vertex as the source or target vertex.  This is then given
        // visibility to each of the possible pins and tiny distance.  Here we
        // assign this visibility by adding edges to the visibility graph that we
        // later remove.
        let is_dummy_at_end = ConnRef::assign_connection_pin_visibility(router, c, true);

        if router.rubber_band_routing && !router.conns[c].route().ps.is_empty() {
            if is_dummy_at_end.0 {
                let sv = router.conns[c].m_src_vert.unwrap();
                let mut first_point = router.vert_infs[sv].point;
                first_point.id = router.vert_infs[sv].id.obj_id;
                first_point.vn = router.vert_infs[sv].id.vn;
                let existing_route = router.conns[c].route_ref();
                existing_route.ps.insert(0, first_point);
            }
        }

        let mut path: Vec<Point> = Vec::new();
        let mut vertices: Vec<VertIdx> = Vec::new();
        if router.conns[c].m_checkpoints.is_empty() {
            ConnRef::generate_standard_path(router, c, &mut path, &mut vertices);
        } else {
            ConnRef::generate_checkpoints_path(router, c, &mut path, &mut vertices);
        }

        debug_assert!(vertices.len() >= 2);
        debug_assert!(Some(vertices[0]) == router.conns[c].src());
        debug_assert!(Some(vertices[vertices.len() - 1]) == router.conns[c].dst());
        debug_assert!(router.conns[c].m_reroute_flag_ptr.is_some());

        for i in 1..vertices.len() {
            if router.invisibility_grph && (router.conns[c].m_type == ConnType::ConnTypePolyLine) {
                // TODO: Again, we could know this edge without searching.
                if let Some(edge) = EdgeInf::existing_edge(router, vertices[i - 1], vertices[i]) {
                    let flag = router.conns[c].m_reroute_flag_ptr.unwrap();
                    router.edge_infs[edge].add_conn(flag);
                }
            } else {
                router.conns[c].m_false_path = true;
            }

            let vertex = vertices[i];
            if let Some(pn) = router.vert_infs[vertex].path_next {
                if router.vert_infs[pn].point == router.vert_infs[vertex].point {
                    if !(router.vert_infs[pn].id.is_conn_pt()) && !(router.vert_infs[vertex].id.is_conn_pt()) {
                        // Check for consecutive points on opposite
                        // corners of two touching shapes.
                        debug_assert!(
                            (router.vert_infs[pn].id.vn as i32 - router.vert_infs[vertex].id.vn as i32).abs()
                                != 2
                        );
                    }
                }
            }
        }

        // Get rid of dummy ShapeConnectionPin bridging points at beginning
        // and end of path.
        let mut path_begin = 0usize;
        let mut path_end = path.len();
        if path.len() > 2 && is_dummy_at_end.0 {
            path_begin += 1;
            ConnEnd::use_pin_vertex(router, ConnEndRef { conn: c, end: VertID::SRC as u32 }, vertices[1]);
        }
        if path.len() > 2 && is_dummy_at_end.1 {
            path_end -= 1;
            ConnEnd::use_pin_vertex(
                router,
                ConnEndRef { conn: c, end: VertID::TAR as u32 },
                vertices[vertices.len() - 2],
            );
        }
        let clipped_path: Vec<Point> = path[path_begin..path_end].to_vec();

        // Clear visibility edges added for connection pins dummy vertices.
        ConnRef::assign_connection_pin_visibility(router, c, false);

        let conn = &mut router.conns[c];
        conn.free_routes();
        conn.m_route.ps = clipped_path;

        true
    }

    // upstream: connector.cpp ConnRef::generateCheckpointsPath()
    pub fn generate_checkpoints_path(
        router: &mut Router,
        c: ConnIdx,
        path: &mut Vec<Point>,
        vertices: &mut Vec<VertIdx>,
    ) {
        let src = router.conns[c].src().unwrap();
        let dst = router.conns[c].dst().unwrap();
        let mut checkpoints: Vec<VertIdx> = router.conns[c].m_checkpoint_vertices.clone();
        checkpoints.insert(0, src);
        checkpoints.push(dst);

        path.clear();
        vertices.clear();
        path.push(router.vert_infs[src].point);
        vertices.push(src);

        let m_id = router.conns[c].m_id;
        let mut last_successful_index: usize = 0;
        for i in 1..checkpoints.len() {
            let start = checkpoints[last_successful_index];
            let end = checkpoints[i];

            // Handle checkpoint directions by disabling some visibility edges.
            if last_successful_index > 0 {
                let src_cp = router.conns[c].m_checkpoints[last_successful_index - 1];
                if src_cp.departure_directions != CONN_DIR_ALL {
                    VertInf::set_visible_directions(router, start, src_cp.departure_directions);
                }
            }
            if (i + 1) < checkpoints.len() {
                let dst_cp = router.conns[c].m_checkpoints[i - 1];
                if dst_cp.arrival_directions != CONN_DIR_ALL {
                    VertInf::set_visible_directions(router, end, dst_cp.arrival_directions);
                }
            }

            let mut a_star = AStarPath::new();
            // Route the connector
            a_star.search(router, c, start, end, None);

            // Restore changes made for checkpoint visibility directions.
            if last_successful_index > 0 {
                VertInf::set_visible_directions(router, start, CONN_DIR_ALL);
            }
            if (i + 1) < checkpoints.len() {
                VertInf::set_visible_directions(router, end, CONN_DIR_ALL);
            }

            // Process the path.
            let pathlen = VertInf::path_leads_back_to(&router.vert_infs, end, Some(start)) as i32;
            if pathlen >= 2 {
                let prev_path_size = path.len();
                let new_len = prev_path_size + (pathlen as usize - 1);
                path.resize(new_len, Point::default());
                vertices.resize(new_len, src);
                let mut vert_inf = Some(end);
                let mut index = path.len() - 1;
                while index >= prev_path_size {
                    let vi = vert_inf.unwrap();
                    let v = &router.vert_infs[vi];
                    path[index] = v.point;
                    if v.id.is_conn_pt() {
                        path[index].id = m_id;
                        path[index].vn = K_UNASSIGNED_VERTEX_NUMBER;
                    } else {
                        path[index].id = v.id.obj_id;
                        path[index].vn = v.id.vn;
                    }
                    vertices[index] = vi;
                    vert_inf = v.path_next;
                    if index == 0 {
                        break;
                    }
                    index -= 1;
                }
                last_successful_index = i;
            } else if i + 1 == checkpoints.len() {
                // There is no valid path.
                router.conns[c].m_needs_reroute_flag = true;

                path.push(router.vert_infs[dst].point);
                vertices.push(dst);

                debug_assert!(path.len() >= 2);
            } else {
                let p = router.vert_infs[checkpoints[i]].point;
                eprint!(
                    "Warning: skipping checkpoint for connector {} at ({}, {}).\n",
                    m_id as i32,
                    crate::avoid::router::format_g(p.x, 6),
                    crate::avoid::router::format_g(p.y, 6)
                );
            }
        }
        // Use topbit to differentiate between start and end point of connector.
        // They need unique IDs for nudging.
        let topbit: u32 = 1u32 << 31;
        let last = path.len() - 1;
        path[last].id = m_id | topbit;
        path[last].vn = K_UNASSIGNED_VERTEX_NUMBER;
    }

    // upstream: connector.cpp ConnRef::generateStandardPath()
    pub fn generate_standard_path(
        router: &mut Router,
        c: ConnIdx,
        path: &mut Vec<Point>,
        vertices: &mut Vec<VertIdx>,
    ) {
        let tar = router.conns[c].m_dst_vert.unwrap();
        let src_vert = router.conns[c].m_src_vert.unwrap();
        let mut existing_path_start: usize = 0;
        let curr_route: PolyLine = router.conns[c].route().clone();
        if router.rubber_band_routing {
            debug_assert!(router.ignore_regions);

            if curr_route.size() > 2 {
                if router.vert_infs[src_vert].point == curr_route.ps[0] {
                    existing_path_start = curr_route.size() - 2;
                    debug_assert!(existing_path_start != 0);
                    let pnt = curr_route.at(existing_path_start);
                    let v_id = VertID::new(pnt.id, pnt.vn, 0);

                    router.conns[c].m_start_vert = router.vertices.get_vertex_by_id(&router.vert_infs, &v_id);
                    debug_assert!(router.conns[c].m_start_vert.is_some());
                }
            }
        }
        let mut pathlen: u32 = 0;
        while pathlen == 0 {
            let mut a_star = AStarPath::new();
            let start = router.conns[c].start();
            a_star.search(router, c, src_vert, tar, start);
            pathlen = VertInf::path_leads_back_to(&router.vert_infs, tar, Some(src_vert));
            if pathlen < 2 {
                if existing_path_start == 0 {
                    break;
                }
                existing_path_start -= 1;
                let pnt = curr_route.at(existing_path_start);
                let props: VertIDProps = if existing_path_start > 0 { 0 } else { VertID::PROP_CONN_POINT };
                let v_id = VertID::new(pnt.id, pnt.vn, props);

                router.conns[c].m_start_vert = router.vertices.get_vertex_by_id(&router.vert_infs, &v_id);
                debug_assert!(router.conns[c].m_start_vert.is_some());
            } else if router.rubber_band_routing {
                // found.
                let mut unwind = false;

                let mut prior: Option<VertIdx> = None;
                let stop = router.vert_infs[router.conns[c].m_start_vert.unwrap()].path_next;
                let mut curr = Some(tar);
                while curr != stop {
                    let cv = curr.unwrap();
                    let pn = router.vert_infs[cv].path_next;
                    if !validate_bend_point(router, pn, cv, prior) {
                        unwind = true;
                        break;
                    }
                    prior = Some(cv);
                    curr = pn;
                }
                if unwind {
                    if existing_path_start == 0 {
                        break;
                    }
                    existing_path_start -= 1;
                    let pnt = curr_route.at(existing_path_start);
                    let props: VertIDProps = if existing_path_start > 0 { 0 } else { VertID::PROP_CONN_POINT };
                    let v_id = VertID::new(pnt.id, pnt.vn, props);

                    router.conns[c].m_start_vert = router.vertices.get_vertex_by_id(&router.vert_infs, &v_id);
                    debug_assert!(router.conns[c].m_start_vert.is_some());

                    pathlen = 0;
                }
            }
        }

        if pathlen < 2 {
            // There is no valid path.
            router.conns[c].m_needs_reroute_flag = true;
            pathlen = 2;
            router.vert_infs[tar].path_next = Some(src_vert);
            if (router.conns[c].m_type == ConnType::ConnTypePolyLine) && router.invisibility_grph {
                // TODO:  Could we know this edge already?
            }
        }
        path.clear();
        path.resize(pathlen as usize, Point::default());
        vertices.clear();
        vertices.resize(pathlen as usize, src_vert);

        let mut j = pathlen as usize - 1;
        let mut i = tar;
        while i != src_vert {
            let v = &router.vert_infs[i];
            path[j] = v.point;
            vertices[j] = i;
            path[j].id = v.id.obj_id;
            path[j].vn = v.id.vn;

            j = j.wrapping_sub(1);
            i = v.path_next.expect("ConnRef::generateStandardPath(): broken path");
        }
        vertices[0] = src_vert;
        let sv = &router.vert_infs[src_vert];
        path[0] = sv.point;
        path[0].id = sv.id.obj_id;
        path[0].vn = sv.id.vn;
    }

    // upstream: connector.cpp ConnRef::setHateCrossings()
    #[inline]
    pub fn set_hate_crossings(&mut self, value: bool) {
        self.m_hate_crossings = value;
    }

    // upstream: connector.cpp ConnRef::doesHateCrossings()
    #[inline]
    pub fn does_hate_crossings(&self) -> bool {
        self.m_hate_crossings
    }

    // upstream: connector.cpp ConnRef::possibleDstPinPoints()
    pub fn possible_dst_pin_points(&self, router: &Router) -> Vec<Point> {
        let mut points = Vec::new();
        if let Some(ce) = &self.m_dst_connend {
            points = ce.possible_pin_points(router);
        }
        points
    }
}

/// Validates a bend point on a path to check it does not form a zigzag
/// corner.  a, b, c are consecutive points on the path.  d and e are b's
/// neighbours, forming the shape corner d-b-e.
// upstream: connector.cpp validateBendPoint()
pub fn validate_bend_point(
    router: &Router,
    a_inf: Option<VertIdx>,
    b_inf: VertIdx,
    c_inf: Option<VertIdx>,
) -> bool {
    let verts = &router.vert_infs;
    let b_v = &verts[b_inf];
    if b_v.id.is_connection_pin() || b_v.id.is_conn_checkpoint() {
        // We shouldn't check connection pins or checkpoints.
        return true;
    }
    let mut bend_okay = true;

    let (a_inf, c_inf) = match (a_inf, c_inf) {
        (Some(a), Some(c)) => (a, c),
        // Not a bendpoint, i.e., the end of the connector, so don't test.
        _ => return bend_okay,
    };

    let d_inf = b_v.sh_prev.expect("validateBendPoint: no shPrev");
    let e_inf = b_v.sh_next.expect("validateBendPoint: no shNext");

    let a = &verts[a_inf].point;
    let b = &b_v.point;
    let c = &verts[c_inf].point;
    let d = &verts[d_inf].point;
    let e = &verts[e_inf].point;

    if (a == b) || (b == c) {
        return bend_okay;
    }

    // Check angle:
    let abc = vec_dir(a, b, c, 0.0);

    if abc == 0 {
        // The three consecutive point on the path are in a line.
        // There should always be an equally short path that skips this
        // bend point, but this check is used during rubber-band routing
        // so we allow this case.
        bend_okay = true;
    } else {
        // (abc != 0)
        debug_assert!(vec_dir(d, b, e, 0.0) > 0);
        let abe = vec_dir(a, b, e, 0.0);
        let abd = vec_dir(a, b, d, 0.0);
        let bce = vec_dir(b, c, e, 0.0);
        let bcd = vec_dir(b, c, d, 0.0);

        bend_okay = false;
        if abe > 0 {
            if (abc > 0) && (abd >= 0) && (bce >= 0) {
                bend_okay = true;
            }
        } else if abd < 0 {
            if (abc < 0) && (abe <= 0) && (bcd <= 0) {
                bend_okay = true;
            }
        }
    }
    bend_okay
}

/// Public-API methods on connector handles (upstream `ConnRef` public
/// methods).
impl ConnIdx {
    // upstream: connector.cpp ConnRef::setEndpoints()
    pub fn set_endpoints(self, router: &mut Router, src_point: &ConnEnd, dst_point: &ConnEnd) {
        ConnRef::set_endpoints(router, self, src_point, dst_point)
    }
    // upstream: connector.cpp ConnRef::setSourceEndpoint()
    pub fn set_source_endpoint(self, router: &mut Router, src_point: &ConnEnd) {
        ConnRef::set_source_endpoint(router, self, src_point)
    }
    // upstream: connector.cpp ConnRef::setDestEndpoint()
    pub fn set_dest_endpoint(self, router: &mut Router, dst_point: &ConnEnd) {
        ConnRef::set_dest_endpoint(router, self, dst_point)
    }
    // upstream: connector.cpp ConnRef::setEndpoint(const unsigned int, const ConnEnd&)
    pub fn set_endpoint(self, router: &mut Router, type_: u32, conn_end: &ConnEnd) {
        ConnRef::set_endpoint(router, self, type_, conn_end)
    }
    // upstream: connector.cpp ConnRef::id()
    pub fn id(self, router: &Router) -> u32 {
        router.conns[self].m_id
    }
    // upstream: connector.cpp ConnRef::needsRepaint()
    pub fn needs_repaint(self, router: &Router) -> bool {
        router.conns[self].needs_repaint()
    }
    // upstream: connector.cpp ConnRef::route()
    pub fn route(self, router: &Router) -> &PolyLine {
        router.conns[self].route()
    }
    // upstream: connector.cpp ConnRef::displayRoute()
    pub fn display_route(self, router: &mut Router) -> &Polygon {
        router.conns[self].display_route()
    }
    // upstream: connector.cpp ConnRef::setCallback()
    pub fn set_callback(self, router: &mut Router, cb: Option<std::boxed::Box<dyn FnMut(ConnIdx)>>) {
        router.conns[self].set_callback(cb.map(ConnCallback))
    }
    // upstream: connector.cpp ConnRef::routingType()
    pub fn routing_type(self, router: &Router) -> ConnType {
        router.conns[self].routing_type()
    }
    // upstream: connector.cpp ConnRef::setRoutingType()
    pub fn set_routing_type(self, router: &mut Router, type_: ConnType) {
        ConnRef::set_routing_type(router, self, type_)
    }
    // upstream: connector.cpp ConnRef::splitAtSegment()
    pub fn split_at_segment(self, router: &mut Router, segment_n: usize) -> (Option<JunctionRef>, Option<ConnIdx>) {
        ConnRef::split_at_segment(router, self, segment_n)
    }
    // upstream: connector.cpp ConnRef::setRoutingCheckpoints()
    pub fn set_routing_checkpoints(self, router: &mut Router, checkpoints: &[Checkpoint]) {
        ConnRef::set_routing_checkpoints(router, self, checkpoints)
    }
    // upstream: connector.cpp ConnRef::routingCheckpoints()
    pub fn routing_checkpoints(self, router: &Router) -> Vec<Checkpoint> {
        router.conns[self].routing_checkpoints()
    }
    // upstream: connector.cpp ConnRef::endpointConnEnds()
    pub fn endpoint_conn_ends(self, router: &Router) -> (ConnEnd, ConnEnd) {
        router.conns[self].endpoint_conn_ends(router)
    }
    // upstream: connector.cpp ConnRef::src()
    pub fn src(self, router: &Router) -> Option<VertIdx> {
        router.conns[self].src()
    }
    // upstream: connector.cpp ConnRef::dst()
    pub fn dst(self, router: &Router) -> Option<VertIdx> {
        router.conns[self].dst()
    }
    // upstream: connector.cpp ConnRef::setFixedRoute()
    pub fn set_fixed_route(self, router: &mut Router, route: &PolyLine) {
        ConnRef::set_fixed_route(router, self, route)
    }
    // upstream: connector.cpp ConnRef::setFixedExistingRoute()
    pub fn set_fixed_existing_route(self, router: &mut Router) {
        ConnRef::set_fixed_existing_route(router, self)
    }
    // upstream: connector.cpp ConnRef::hasFixedRoute()
    pub fn has_fixed_route(self, router: &Router) -> bool {
        router.conns[self].has_fixed_route()
    }
    // upstream: connector.cpp ConnRef::clearFixedRoute()
    pub fn clear_fixed_route(self, router: &mut Router) {
        ConnRef::clear_fixed_route(router, self)
    }
    // upstream: connector.cpp ConnRef::setHateCrossings()
    pub fn set_hate_crossings(self, router: &mut Router, value: bool) {
        router.conns[self].set_hate_crossings(value)
    }
    // upstream: connector.cpp ConnRef::doesHateCrossings()
    pub fn does_hate_crossings(self, router: &Router) -> bool {
        router.conns[self].does_hate_crossings()
    }
    // upstream: connector.cpp ConnRef::possibleDstPinPoints()
    pub fn possible_dst_pin_points(self, router: &Router) -> Vec<Point> {
        router.conns[self].possible_dst_pin_points(router)
    }
    // upstream: connector.cpp ConnRef::setEndpoint(const unsigned int, const VertID&, Point *)
    pub fn set_endpoint_vertex(self, router: &mut Router, type_: u32, point_id: &VertID, sugg: Option<&Point>) -> bool {
        ConnRef::set_endpoint_vertex(router, self, type_, point_id, sugg)
    }
}

// ===========================================================================
// Second half of connector.cpp (PtOrder, ConnectorCrossings,
// splitBranchingSegments, ...): NOT YET PORTED.  Declarations below are the
// interface the router code calls; bodies are `todo!`.
// ===========================================================================

// upstream: connector.h CROSSING_* flags
pub const CROSSING_NONE: u32 = 0;
pub const CROSSING_TOUCHES: u32 = 1;
pub const CROSSING_SHARES_PATH: u32 = 2;
pub const CROSSING_SHARES_PATH_AT_END: u32 = 4;
pub const CROSSING_SHARES_FIXED_SEGMENT: u32 = 8;

// upstream: connector.h typedef std::pair<int, unsigned int> CrossingsInfoPair
pub type CrossingsInfoPair = (i32, u32);
// upstream: connector.h typedef std::vector<Avoid::Point> PointList
pub type PointList = Vec<Point>;
// upstream: connector.h typedef std::vector<PointList> SharedPathList
pub type SharedPathList = Vec<PointList>;
// upstream: connector.h typedef std::set<Avoid::Point> PointSet
pub type PointSet = BTreeSet<PointOrd>;
// upstream: connector.h typedef std::map<Avoid::Point,PtOrder> PtOrderMap
pub type PtOrderMap = BTreeMap<PointOrd, PtOrder>;

/// Upstream `class PtOrder` — NOT YET PORTED.  Its members are to be
/// defined by the agent porting the second half of connector.cpp.
// upstream: connector.h class PtOrder
#[derive(Clone, Debug, Default)]
pub struct PtOrder {}

/// Upstream `class ConnectorCrossings`.
///
/// The upstream `Polygon& poly` / `Polygon& conn` members are passed to
/// [`ConnectorCrossings::count_for_segment`] instead (so that callers can
/// hand in connector routes temporarily moved out of the `ConnRef`s; the
/// implementation must therefore not read the routes of
/// `poly_conn_ref`/`conn_conn_ref` through the router).  The optional
/// output pointers are `Option<&mut ..>`.
// upstream: connector.h class ConnectorCrossings
#[derive(Debug)]
pub struct ConnectorCrossings<'a> {
    pub poly_is_conn: bool,
    pub check_for_branching_segments: bool,
    pub poly_conn_ref: Option<ConnIdx>,
    pub conn_conn_ref: Option<ConnIdx>,

    /// Uninitialised until `clear()` upstream; 0 here.
    pub crossing_count: u32,
    pub crossing_flags: u32,
    pub crossing_points: Option<&'a mut PointSet>,
    pub point_orders: Option<&'a mut PtOrderMap>,
    pub shared_paths: Option<&'a mut SharedPathList>,

    pub first_shared_path_at_end_length: f64,
    pub second_shared_path_at_end_length: f64,
}

impl<'a> ConnectorCrossings<'a> {
    // upstream: connector.cpp ConnectorCrossings::ConnectorCrossings()
    pub fn new(
        poly_is_conn: bool,
        poly_conn_ref: Option<ConnIdx>,
        conn_conn_ref: Option<ConnIdx>,
    ) -> ConnectorCrossings<'a> {
        ConnectorCrossings {
            poly_is_conn,
            check_for_branching_segments: false,
            poly_conn_ref,
            conn_conn_ref,
            crossing_count: 0,
            crossing_flags: CROSSING_NONE,
            crossing_points: None,
            point_orders: None,
            shared_paths: None,
            first_shared_path_at_end_length: 0.0,
            second_shared_path_at_end_length: 0.0,
        }
    }

    // upstream: connector.cpp ConnectorCrossings::clear()
    pub fn clear(&mut self) {
        todo!("upstream: connector.cpp ConnectorCrossings::clear — not yet ported")
    }

    /// Works out if the segment `conn[c_index-1]--conn[c_index]` really
    /// crosses `poly`.
    // upstream: connector.cpp ConnectorCrossings::countForSegment()
    pub fn count_for_segment(
        &mut self,
        router: &Router,
        poly: &mut Polygon,
        conn: &mut Polygon,
        c_index: usize,
        final_segment: bool,
    ) {
        let _ = (router, poly, conn, c_index, final_segment);
        todo!("upstream: connector.cpp ConnectorCrossings::countForSegment — not yet ported")
    }
}

// upstream: connector.cpp splitBranchingSegments()
pub fn split_branching_segments(poly: &mut Polygon, poly_is_conn: bool, conn: &mut Polygon, tolerance: f64) {
    let _ = (poly, poly_is_conn, conn, tolerance);
    todo!("upstream: connector.cpp splitBranchingSegments — not yet ported")
}
