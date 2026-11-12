//! Faithful port of upstream `libavoid/connend.{h,cpp}`.
//!
//! * `ConnEnd` is a value type exactly as upstream (it is copied into
//!   `ActionInfo`s, returned from `ConnRef::endpointConnEnds()`, ...).
//! * Upstream `ConnRef` owns two heap `ConnEnd`s (`m_src_connend`,
//!   `m_dst_connend`) whose *addresses* are registered in
//!   `Obstacle::m_following_conns` and `ShapeConnectionPin::m_connend_users`
//!   (both `std::set<ConnEnd *>`, i.e. pointer ordered).  Here those owned
//!   instances live inline in the `ConnRef` (`Option<ConnEnd>`), are named
//!   by a [`ConnEndRef`] (`(connector, endpoint type)`), and carry a creation
//!   `serial` assigned when upstream would `new` them.  The registration
//!   sets are `BTreeSet<PtrKey<ConnEndRef>>` ordered by that serial
//!   (`// PTR-ORDER:`).
//! * Methods that only read the ConnEnd take `&self` and `&Router`;
//!   methods that upstream only ever calls on the owned instances and that
//!   mutate registrations are associated functions taking
//!   `(&mut Router, ConnEndRef)`.

#![allow(dead_code)]
// Keep upstream statement shapes.
#![allow(
    clippy::collapsible_if,
    clippy::collapsible_else_if,
    clippy::print_with_newline,
    clippy::field_reassign_with_default,
    clippy::assign_op_pattern,
    clippy::explicit_counter_loop,
    clippy::if_same_then_else,
    clippy::question_mark,
    clippy::neg_multiply,
    clippy::needless_late_init,
    clippy::uninlined_format_args
)]
#![allow(clippy::new_ret_no_self)]

use crate::avoid::arena::{ConnIdx, ObstacleIdx, PinIdx, PtrKey, VertIdx};
use crate::avoid::connectionpin::{CONNECTIONPIN_CENTRE, CONNECTIONPIN_UNSET};
use crate::avoid::geometry::{euclidean_dist, manhattan_dist, rotational_angle};
use crate::avoid::geomtypes::{cpp_max, Point, K_UNASSIGNED_VERTEX_NUMBER};
use crate::avoid::graph::EdgeInf;
use crate::avoid::junction::JunctionRef;
use crate::avoid::obstacle::{Obstacle, ObstacleKind};
use crate::avoid::router::{Router, RoutingParameter};
use crate::avoid::shape::ShapeRef;
use crate::avoid::vertices::{VertID, VertInf};
use crate::avoid::visibility::vertex_visibility;

// upstream: connend.h enum ConnDirFlag / typedef ConnDirFlags
pub use crate::avoid::vertices::{
    ConnDirFlags, CONN_DIR_ALL, CONN_DIR_DOWN, CONN_DIR_LEFT, CONN_DIR_NONE, CONN_DIR_RIGHT,
    CONN_DIR_UP,
};

/// Types that endpoints of connectors can be.
// upstream: connend.h enum ConnEndType
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConnEndType {
    /// A ConnEnd attached to a fixed point.
    ConnEndPoint = 0,
    /// A ConnEnd attached to a shape connection pin.
    ConnEndShapePin = 1,
    /// A ConnEnd attached to a junction.
    ConnEndJunction = 2,
    /// An empty/uninitialised ConnEnd.
    ConnEndEmpty = 3,
}

/// Names one of the heap `ConnEnd`s owned by a connector (upstream
/// `ConnEnd *` equal to `conn->m_src_connend` or `conn->m_dst_connend`).
/// `end` is `VertID::SRC` or `VertID::TAR`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConnEndRef {
    pub conn: ConnIdx,
    pub end: u32,
}

impl ConnEndRef {
    /// The owned ConnEnd this names (upstream `*connEndPtr`).
    pub fn get(self, router: &Router) -> &ConnEnd {
        let c = &router.conns[self.conn];
        let ce = if self.end == VertID::TAR as u32 {
            c.m_dst_connend.as_ref()
        } else {
            c.m_src_connend.as_ref()
        };
        ce.expect("ConnEndRef: connector has no such owned ConnEnd")
    }

    pub fn get_mut(self, router: &mut Router) -> &mut ConnEnd {
        let c = &mut router.conns[self.conn];
        let ce = if self.end == VertID::TAR as u32 {
            c.m_dst_connend.as_mut()
        } else {
            c.m_src_connend.as_mut()
        };
        ce.expect("ConnEndRef: connector has no such owned ConnEnd")
    }

    /// Key used in pointer-ordered `std::set<ConnEnd *>` replacements.
    pub fn key(self, router: &Router) -> PtrKey<ConnEndRef> {
        PtrKey::new(self.get(router).serial, self)
    }
}

/// The ConnEnd class represents different possible endpoints for
/// connectors.
// upstream: connend.h class ConnEnd
#[derive(Clone, Debug)]
pub struct ConnEnd {
    pub m_type: ConnEndType,
    pub m_point: Point,
    pub m_directions: ConnDirFlags,
    pub m_connection_pin_class_id: u32,
    /// The shape/junction this is attached to.
    pub m_anchor_obj: Option<ObstacleIdx>,
    /// The parent connector.
    pub m_conn_ref: Option<ConnIdx>,
    pub m_active_pin: Option<PinIdx>,
    /// Creation serial of the heap instance owned by a `ConnRef` (no
    /// upstream counterpart; stands in for the object's address).  It is
    /// copied along with the value but only meaningful for owned instances.
    pub serial: u64,
}

impl Default for ConnEnd {
    fn default() -> Self {
        ConnEnd::new()
    }
}

impl ConnEnd {
    /// Constructs an empty ConnEnd (upstream default constructor).
    // upstream: connend.cpp ConnEnd::ConnEnd()
    pub fn new() -> ConnEnd {
        ConnEnd {
            m_type: ConnEndType::ConnEndEmpty,
            m_point: Point::new(0.0, 0.0),
            m_directions: CONN_DIR_ALL,
            m_connection_pin_class_id: CONNECTIONPIN_UNSET,
            m_anchor_obj: None,
            m_conn_ref: None,
            m_active_pin: None,
            serial: 0,
        }
    }

    /// Constructs a ConnEnd from a free-floating point.
    // upstream: connend.cpp ConnEnd::ConnEnd(const Point&)
    pub fn from_point(point: Point) -> ConnEnd {
        ConnEnd {
            m_type: ConnEndType::ConnEndPoint,
            m_point: point,
            m_directions: CONN_DIR_ALL,
            m_connection_pin_class_id: CONNECTIONPIN_UNSET,
            m_anchor_obj: None,
            m_conn_ref: None,
            m_active_pin: None,
            serial: 0,
        }
    }

    /// Constructs a ConnEnd from a free-floating point with visibility
    /// directions.
    // upstream: connend.cpp ConnEnd::ConnEnd(const Point&, const ConnDirFlags)
    pub fn from_point_dirs(point: Point, vis_dirs: ConnDirFlags) -> ConnEnd {
        ConnEnd {
            m_type: ConnEndType::ConnEndPoint,
            m_point: point,
            m_directions: vis_dirs,
            m_connection_pin_class_id: CONNECTIONPIN_UNSET,
            m_anchor_obj: None,
            m_conn_ref: None,
            m_active_pin: None,
            serial: 0,
        }
    }

    /// Constructs a ConnEnd attached to a shape's connection pins of the
    /// given class.
    // upstream: connend.cpp ConnEnd::ConnEnd(ShapeRef *, const unsigned int)
    pub fn from_shape_pin(
        router: &Router,
        shape_ref: ShapeRef,
        connection_pin_class_id: u32,
    ) -> ConnEnd {
        let mut ce = ConnEnd {
            m_type: ConnEndType::ConnEndShapePin,
            m_point: Point::new(0.0, 0.0),
            m_directions: CONN_DIR_ALL,
            m_connection_pin_class_id: connection_pin_class_id,
            m_anchor_obj: Some(shape_ref.0),
            m_conn_ref: None,
            m_active_pin: None,
            serial: 0,
        };
        debug_assert!(ce.m_connection_pin_class_id > 0);

        ce.m_point = Obstacle::position(router, shape_ref.0);
        debug_assert!(ce.m_connection_pin_class_id != CONNECTIONPIN_UNSET);
        ce
    }

    /// Constructs a ConnEnd attached to a junction.
    // upstream: connend.cpp ConnEnd::ConnEnd(JunctionRef *)
    pub fn from_junction(router: &Router, junction_ref: JunctionRef) -> ConnEnd {
        // (upstream leaves m_point default-constructed, then assigns it)
        let mut ce = ConnEnd {
            m_type: ConnEndType::ConnEndJunction,
            m_point: Point::default(),
            m_directions: CONN_DIR_ALL,
            m_connection_pin_class_id: CONNECTIONPIN_CENTRE,
            m_anchor_obj: Some(junction_ref.0),
            m_conn_ref: None,
            m_active_pin: None,
            serial: 0,
        };
        ce.m_point = Obstacle::position(router, junction_ref.0);
        ce
    }

    // upstream: connend.cpp ConnEnd::type()
    #[inline]
    pub fn type_(&self) -> ConnEndType {
        self.m_type
    }

    /// Returns the position of this connector endpoint.
    // upstream: connend.cpp ConnEnd::position()
    pub fn position(&self, router: &Router) -> Point {
        if let Some(pin) = self.m_active_pin {
            // Attached to a pin!
            router.pins[pin].position(router, None)
        } else if let Some(anchor) = self.m_anchor_obj {
            Obstacle::position(router, anchor)
        } else {
            self.m_point
        }
    }

    /// Returns the directions in which this connector endpoint should be
    /// given visibility.
    // upstream: connend.cpp ConnEnd::directions()
    pub fn directions(&self, router: &Router) -> ConnDirFlags {
        if let Some(pin) = self.m_active_pin {
            // Attached to a pin!
            router.pins[pin].directions()
        } else {
            self.m_directions
        }
    }

    /// Returns the shape this ConnEnd attaches to, or None.
    // upstream: connend.cpp ConnEnd::shape()
    pub fn shape(&self, router: &Router) -> Option<ShapeRef> {
        let a = self.m_anchor_obj?;
        match router.obstacles[a].kind {
            ObstacleKind::Shape => Some(ShapeRef(a)),
            ObstacleKind::Junction(_) => None,
        }
    }

    /// Returns the junction this ConnEnd attaches to, or None.
    // upstream: connend.cpp ConnEnd::junction()
    pub fn junction(&self, router: &Router) -> Option<JunctionRef> {
        let a = self.m_anchor_obj?;
        match router.obstacles[a].kind {
            ObstacleKind::Junction(_) => Some(JunctionRef(a)),
            ObstacleKind::Shape => None,
        }
    }

    // upstream: connend.cpp ConnEnd::pinClassId()
    #[inline]
    pub fn pin_class_id(&self) -> u32 {
        self.m_connection_pin_class_id
    }

    // upstream: connend.cpp ConnEnd::isPinConnection()
    #[inline]
    pub fn is_pin_connection(&self) -> bool {
        (self.m_type == ConnEndType::ConnEndShapePin) || (self.m_type == ConnEndType::ConnEndJunction)
    }

    /// `VertID::TAR` if `this` is its connector's `m_dst_connend`, else
    /// `VertID::SRC`.
    // upstream: connend.cpp ConnEnd::endpointType()
    pub fn endpoint_type(router: &Router, this: ConnEndRef) -> u32 {
        let ce = this.get(router);
        debug_assert!(ce.m_conn_ref.is_some());
        // `m_conn_ref->m_dst_connend == this`: the owned ConnEnd named by
        // `this` is its connector's destination ConnEnd.
        let dst_is_this = ce.m_conn_ref == Some(this.conn) && this.end == VertID::TAR as u32;
        if dst_is_this {
            VertID::TAR as u32
        } else {
            VertID::SRC as u32
        }
    }

    /// Marks this ConnEnd as using a particular ShapeConnectionPin.
    // upstream: connend.cpp ConnEnd::usePin()
    pub fn use_pin(router: &mut Router, this: ConnEndRef, pin: Option<PinIdx>) {
        debug_assert!(this.get(router).m_active_pin.is_none());

        this.get_mut(router).m_active_pin = pin;
        if let Some(p) = pin {
            let key = this.key(router);
            // PTR-ORDER: m_connend_users is a std::set<ConnEnd *>.
            router.pins[p].m_connend_users.insert(key);
        }
    }

    /// Marks this ConnEnd as using a particular ShapeConnectionPin's vertex.
    // upstream: connend.cpp ConnEnd::usePinVertex()
    pub fn use_pin_vertex(router: &mut Router, this: ConnEndRef, pin_vert: VertIdx) {
        debug_assert!(this.get(router).m_active_pin.is_none());

        let anchor = this
            .get(router)
            .m_anchor_obj
            .expect("ConnEnd::usePinVertex(): no anchor object");
        let mut found = None;
        for &curr_pin in router.obstacles[anchor].m_connection_pins.iter() {
            if router.pins[curr_pin].m_vertex == Some(pin_vert) {
                found = Some(curr_pin);
                break;
            }
        }
        if let Some(p) = found {
            ConnEnd::use_pin(router, this, Some(p));
        }
    }

    /// Returns a vector of the possible Points for this ConnEnd.
    // upstream: connend.cpp ConnEnd::possiblePinPoints()
    pub fn possible_pin_points(&self, router: &Router) -> Vec<Point> {
        let points = Vec::new();
        let anchor = match self.m_anchor_obj {
            Some(a) if self.m_connection_pin_class_id != CONNECTIONPIN_UNSET => a,
            _ => return points,
        };

        Obstacle::possible_pin_points(router, anchor, self.m_connection_pin_class_id)
    }

    /// Marks this ConnEnd as no longer using the active ShapeConnectionPin.
    // upstream: connend.cpp ConnEnd::freeActivePin()
    pub fn free_active_pin(router: &mut Router, this: ConnEndRef) {
        if let Some(p) = this.get(router).m_active_pin {
            let key = this.key(router);
            router.pins[p].m_connend_users.remove(&key);
        }
        this.get_mut(router).m_active_pin = None;
    }

    /// Creates the connection between a connector and a shape/junction.
    // upstream: connend.cpp ConnEnd::connect()
    pub fn connect(router: &mut Router, this: ConnEndRef, conn: ConnIdx) {
        let ce = this.get(router);
        debug_assert!(ce.is_pin_connection());
        debug_assert!(ce.m_anchor_obj.is_some());
        debug_assert!(ce.m_conn_ref.is_none());

        let anchor = ce.m_anchor_obj.unwrap();
        let key = this.key(router);
        Obstacle::add_following_conn_end(router, anchor, key);
        this.get_mut(router).m_conn_ref = Some(conn);
    }

    /// Removes the connection between a connector and a shape/junction.
    /// Upstream default for `shape_deleted` is false.
    // upstream: connend.cpp ConnEnd::disconnect()
    pub fn disconnect(router: &mut Router, this: ConnEndRef, shape_deleted: bool) {
        if this.get(router).m_conn_ref.is_none() {
            // Not connected.
            return;
        }

        let pos = this.get(router).position(router);
        this.get_mut(router).m_point = pos;
        let anchor = this.get(router).m_anchor_obj.unwrap();
        let key = this.key(router);
        Obstacle::remove_following_conn_end(router, anchor, &key);
        this.get_mut(router).m_conn_ref = None;

        if shape_deleted {
            // Turn this into a manual ConnEnd.
            let pos = this.get(router).position(router);
            let ce = this.get_mut(router);
            ce.m_point = pos;
            ce.m_anchor_obj = None;
            ce.m_type = ConnEndType::ConnEndPoint;
            ce.m_connection_pin_class_id = CONNECTIONPIN_UNSET;
        }
    }

    /// Assign visibility to a dummy vertex representing all the possible
    /// pins for this pinClassId.  (`self` is a snapshot of the owned
    /// ConnEnd; the function only reads it.)
    // upstream: connend.cpp ConnEnd::assignPinVisibilityTo()
    pub fn assign_pin_visibility_to(
        &self,
        router: &mut Router,
        dummy_connection_vert: VertIdx,
        target_vert: VertIdx,
    ) {
        let mut valid_pin_count: u32 = 0;

        debug_assert!(self.m_anchor_obj.is_some());
        debug_assert!(self.m_connection_pin_class_id != CONNECTIONPIN_UNSET);
        let anchor = self.m_anchor_obj.unwrap();

        let pins: Vec<PinIdx> = router.obstacles[anchor].m_connection_pins.iter().copied().collect();
        for curr_pin in pins {
            let (class_id, exclusive, users_empty, cost, pin_vertex, pin_dirs) = {
                let p = &router.pins[curr_pin];
                (
                    p.m_class_id,
                    p.m_exclusive,
                    p.m_connend_users.is_empty(),
                    p.m_connection_cost,
                    // (only dereferenced for matching pins, as upstream: a
                    // pin of another class may not have its vertex yet when
                    // its own addConnectionPin() triggers a transaction)
                    p.m_vertex,
                    p.directions(),
                )
            };
            if (class_id == self.m_connection_pin_class_id) && (!exclusive || users_empty) {
                let pin_vertex = pin_vertex.expect("pin without vertex");
                let mut routing_cost = cost;
                let adj_target_pt =
                    router.vert_infs[target_vert].point - router.vert_infs[pin_vertex].point;
                let angle = rotational_angle(&adj_target_pt);
                let mut in_visibility_range = false;

                if angle <= 45.0 || angle >= 315.0 {
                    if pin_dirs & CONN_DIR_RIGHT != 0 {
                        in_visibility_range = true;
                    }
                }
                if (45.0..=135.0).contains(&angle) {
                    if pin_dirs & CONN_DIR_DOWN != 0 {
                        in_visibility_range = true;
                    }
                }
                if (135.0..=225.0).contains(&angle) {
                    if pin_dirs & CONN_DIR_LEFT != 0 {
                        in_visibility_range = true;
                    }
                }
                if (225.0..=315.0).contains(&angle) {
                    if pin_dirs & CONN_DIR_UP != 0 {
                        in_visibility_range = true;
                    }
                }
                if !in_visibility_range {
                    routing_cost += router.routing_parameter(RoutingParameter::PortDirectionPenalty);
                }

                if router.m_allows_orthogonal_routing {
                    // This has same ID and is either unconnected or not
                    // exclusive, so give it visibility.
                    let edge = EdgeInf::new(router, dummy_connection_vert, pin_vertex, true);
                    // XXX Can't use a zero cost due to assumptions
                    //     elsewhere in code.
                    let d = manhattan_dist(
                        &router.vert_infs[dummy_connection_vert].point,
                        &router.vert_infs[pin_vertex].point,
                    ) + cpp_max(0.001, routing_cost);
                    EdgeInf::set_dist(router, edge, d);
                }

                if router.m_allows_polyline_routing {
                    // This has same ID and is either unconnected or not
                    // exclusive, so give it visibility.
                    let edge = EdgeInf::new(router, dummy_connection_vert, pin_vertex, false);
                    // XXX Can't use a zero cost due to assumptions
                    //     elsewhere in code.
                    let d = euclidean_dist(
                        &router.vert_infs[dummy_connection_vert].point,
                        &router.vert_infs[pin_vertex].point,
                    ) + cpp_max(0.001, routing_cost);
                    EdgeInf::set_dist(router, edge, d);
                }

                // Increment the number of valid pins for this ConnEnd connection.
                valid_pin_count += 1;
            }
        }

        if valid_pin_count == 0 {
            // There should be at least one pin, otherwise we will have
            // problems finding connector routes.
            let conn_id = self.m_conn_ref.map(|c| router.conns[c].m_id as i32).unwrap_or(0);
            eprint!(
                "Warning: In ConnEnd::assignPinVisibilityTo():\n         ConnEnd for connector {} can't connect to shape {}\n         since it has no pins with class id of {}.\n",
                conn_id,
                router.obstacles[anchor].m_id as i32,
                self.m_connection_pin_class_id
            );
        }
    }

    /// Returns `(added_vertex, vertex)`.
    // upstream: connend.cpp ConnEnd::getHyperedgeVertex()
    pub fn get_hyperedge_vertex(&self, router: &mut Router) -> (bool, VertIdx) {
        let mut added_vertex = false;
        let mut vertex: Option<VertIdx> = None;

        if let Some(anchor) = self.m_anchor_obj {
            for &curr_pin in router.obstacles[anchor].m_connection_pins.iter() {
                let p = &router.pins[curr_pin];
                if (p.m_class_id == self.m_connection_pin_class_id)
                    && (!p.m_exclusive || p.m_connend_users.is_empty())
                {
                    vertex = p.m_vertex;
                }
            }
            debug_assert!(vertex.is_some());
        } else {
            let id = VertID::new(0, K_UNASSIGNED_VERTEX_NUMBER, VertID::PROP_CONN_POINT);
            let v = VertInf::new(router, id, self.m_point, true);
            router.vert_infs[v].vis_directions = self.m_directions;
            added_vertex = true;

            if router.m_allows_polyline_routing {
                vertex_visibility(router, v, None, true, true);
            }
            vertex = Some(v);
        }

        (added_vertex, vertex.expect("ConnEnd::getHyperedgeVertex(): no vertex"))
    }

    // (upstream: connend.cpp ConnEnd::outputCode() — debug code output, not
    // ported.)
}
