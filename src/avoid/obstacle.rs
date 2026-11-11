//! Faithful port of upstream `libavoid/obstacle.{h,cpp}`.
//!
//! Upstream `Obstacle` is the abstract base class of `ShapeRef` and
//! `JunctionRef`.  Here there is one [`Obstacle`] struct (living in
//! `Router::obstacles`, named by an [`ObstacleIdx`]) whose [`ObstacleKind`]
//! holds the subclass-specific data; the virtual methods (`position()`,
//! `outputCode()`, ...) dispatch on the kind, and `dynamic_cast<ShapeRef *>`
//! / `dynamic_cast<JunctionRef *>` become [`Obstacle::as_shape`] /
//! [`Obstacle::as_junction`].  `ShapeRef`/`JunctionRef` are thin `Copy`
//! handle wrappers around the `ObstacleIdx` (see `shape.rs`, `junction.rs`).
//!
//! * `m_router_obstacles_pos` (iterator into `Router::m_obstacles`) is not
//!   stored: `Router::m_obstacles` is a `Vec` and `makeInactive` erases the
//!   (unique) entry by value.
//! * `m_following_conns` (`std::set<ConnEnd *>`) is a
//!   `BTreeSet<PtrKey<ConnEndRef>>` ordered by the owned ConnEnds' creation
//!   serials (`// PTR-ORDER:`).

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

use std::collections::BTreeSet;

use crate::avoid::arena::{Arena, ConnIdx, ObstacleIdx, PinIdx, PtrKey, VertIdx};
use crate::avoid::connectionpin::{ShapeConnectionPin, ShapeConnectionPinSet};
use crate::avoid::connend::{ConnEnd, ConnEndRef};
use crate::avoid::geomtypes::{Box, Point, Polygon, PolygonInterface};
use crate::avoid::junction::{JunctionData, JunctionRef};
use crate::avoid::router::{Router, RoutingParameter};
use crate::avoid::shape::ShapeRef;
use crate::avoid::vertices::{VertID, VertInf};

/// Arena of all obstacles (shapes and junctions) of a router.
pub type ObstacleArena = Arena<ObstacleIdx, Obstacle>;

/// Subclass of an [`Obstacle`].
#[derive(Clone, Debug)]
pub enum ObstacleKind {
    /// `ShapeRef` (no extra data).
    Shape,
    /// `JunctionRef`.
    Junction(JunctionData),
}

/// Upstream `class Obstacle` (with the `ShapeRef`/`JunctionRef` subclass
/// data in `kind`).
// upstream: obstacle.h class Obstacle
#[derive(Clone, Debug)]
pub struct Obstacle {
    /// Monotonic creation serial (`// PTR-ORDER:`).
    pub serial: u64,
    pub m_id: u32,
    pub m_polygon: Polygon,
    pub m_active: bool,
    pub m_first_vert: Option<VertIdx>,
    pub m_last_vert: Option<VertIdx>,
    /// PTR-ORDER: upstream `std::set<ConnEnd *>`.
    pub m_following_conns: BTreeSet<PtrKey<ConnEndRef>>,
    pub m_connection_pins: ShapeConnectionPinSet,
    pub kind: ObstacleKind,
}

impl Obstacle {
    /// Base-class constructor: allocates the obstacle and its (not yet
    /// registered) ring of routing-polygon vertices.  Upstream default for
    /// `id` is 0.
    // upstream: obstacle.cpp Obstacle::Obstacle()
    pub fn new_base(router: &mut Router, ply: Polygon, id: u32, kind: ObstacleKind) -> ObstacleIdx {
        let serial = router.next_serial();
        let m_id = router.assign_id(id);
        let o = router.obstacles.alloc(Obstacle {
            serial,
            m_id,
            m_polygon: ply,
            m_active: false,
            m_first_vert: None,
            m_last_vert: None,
            m_following_conns: BTreeSet::new(),
            m_connection_pins: ShapeConnectionPinSet::new(),
            kind,
        });

        let mut i = VertID::new(m_id, 0, 0);

        let routing_poly = Obstacle::routing_polygon(router, o);
        let add_to_router_now = false;
        let mut last: Option<VertIdx> = None;
        let mut node: Option<VertIdx> = None;
        for pt_i in 0..routing_poly.size() {
            let n = VertInf::new(router, i, routing_poly.ps[pt_i], add_to_router_now);
            node = Some(n);

            if router.obstacles[o].m_first_vert.is_none() {
                router.obstacles[o].m_first_vert = Some(n);
            } else {
                router.vert_infs[n].sh_prev = last;
                router.vert_infs[last.unwrap()].sh_next = Some(n);
            }

            last = Some(n);
            i.post_inc();
        }
        router.obstacles[o].m_last_vert = node;

        let first = router.obstacles[o].m_first_vert.expect("Obstacle with empty polygon");
        let lastv = node.unwrap();
        router.vert_infs[lastv].sh_next = Some(first);
        router.vert_infs[first].sh_prev = Some(lastv);
        o
    }

    /// Upstream `delete obstacle` (runs `~ShapeRef`/`~JunctionRef` checks,
    /// then `~Obstacle`).
    // upstream: obstacle.cpp Obstacle::~Obstacle()
    // upstream: shape.cpp ShapeRef::~ShapeRef(), junction.cpp JunctionRef::~JunctionRef()
    pub fn delete(router: &mut Router, o: ObstacleIdx) {
        if !router.m_currently_calling_destructors {
            match router.obstacles[o].kind {
                ObstacleKind::Shape => {
                    eprint!("ERROR: ShapeRef::~ShapeRef() shouldn't be called directly.\n");
                    eprint!("       It is owned by the router.  Call Router::deleteShape() instead.\n");
                }
                ObstacleKind::Junction(_) => {
                    eprint!("ERROR: JunctionRef::~JunctionRef() shouldn't be called directly.\n");
                    eprint!("       It is owned by the router.  Call Router::deleteJunction() instead.\n");
                }
            }
            panic!("Obstacle deleted directly instead of through the Router");
        }

        debug_assert!(!router.obstacles[o].m_active);
        let first = router.obstacles[o].m_first_vert.expect("Obstacle without vertices");

        let mut it = first;
        loop {
            let tmp = it;
            it = router.vert_infs[it].sh_next.unwrap();

            VertInf::delete(router, tmp);
            if it == first {
                break;
            }
        }
        router.obstacles[o].m_first_vert = None;
        router.obstacles[o].m_last_vert = None;

        // Free and clear any connection pins.
        while let Some(pin) = router.obstacles[o].m_connection_pins.first() {
            ShapeConnectionPin::delete(router, pin);
        }
        router.obstacles.free(o);
    }

    // ---------------------------------------------------------------------
    // dynamic_cast helpers
    // ---------------------------------------------------------------------

    /// `dynamic_cast<ShapeRef *>(obstacle)`.
    pub fn as_shape(router: &Router, o: ObstacleIdx) -> Option<ShapeRef> {
        match router.obstacles[o].kind {
            ObstacleKind::Shape => Some(ShapeRef(o)),
            ObstacleKind::Junction(_) => None,
        }
    }

    /// `dynamic_cast<JunctionRef *>(obstacle)`.
    pub fn as_junction(router: &Router, o: ObstacleIdx) -> Option<JunctionRef> {
        match router.obstacles[o].kind {
            ObstacleKind::Junction(_) => Some(JunctionRef(o)),
            ObstacleKind::Shape => None,
        }
    }

    #[inline]
    pub fn is_shape(&self) -> bool {
        matches!(self.kind, ObstacleKind::Shape)
    }

    #[inline]
    pub fn is_junction(&self) -> bool {
        matches!(self.kind, ObstacleKind::Junction(_))
    }

    /// Junction data (panics for shapes).
    pub fn junction_data(&self) -> &JunctionData {
        match &self.kind {
            ObstacleKind::Junction(j) => j,
            ObstacleKind::Shape => panic!("obstacle is not a junction"),
        }
    }

    pub fn junction_data_mut(&mut self) -> &mut JunctionData {
        match &mut self.kind {
            ObstacleKind::Junction(j) => j,
            ObstacleKind::Shape => panic!("obstacle is not a junction"),
        }
    }

    // ---------------------------------------------------------------------
    // virtual methods
    // ---------------------------------------------------------------------

    /// Virtual `position()`: `ShapeRef::position()` (centre of the routing
    /// box) or `JunctionRef::position()`.
    // upstream: shape.cpp ShapeRef::position(), junction.cpp JunctionRef::position()
    pub fn position(router: &Router, o: ObstacleIdx) -> Point {
        match &router.obstacles[o].kind {
            ObstacleKind::Shape => ShapeRef(o).position(router),
            ObstacleKind::Junction(j) => j.m_position,
        }
    }

    // ---------------------------------------------------------------------
    // non-virtual methods
    // ---------------------------------------------------------------------

    // upstream: obstacle.cpp Obstacle::setNewPoly()
    pub fn set_new_poly(router: &mut Router, o: ObstacleIdx, poly: &Polygon) {
        debug_assert!(router.obstacles[o].m_first_vert.is_some());
        debug_assert!(router.obstacles[o].m_polygon.size() == poly.size());

        router.obstacles[o].m_polygon = poly.clone();
        let routing_poly = Obstacle::routing_polygon(router, o);

        let first = router.obstacles[o].m_first_vert.unwrap();
        let mut curr = first;
        for pt_i in 0..routing_poly.size() {
            debug_assert!(router.vert_infs[curr].vis_list_size == 0);
            debug_assert!(router.vert_infs[curr].invis_list_size == 0);

            // Reset with the new polygon point.
            router.vert_infs[curr].reset_point(routing_poly.ps[pt_i]);
            router.vert_infs[curr].path_next = None;

            curr = router.vert_infs[curr].sh_next.unwrap();
        }
        debug_assert!(curr == first);

        // It may be that the polygon for the obstacle has been updated after
        // creating the shape.  These events may have been combined for a single
        // transaction, so update pin positions.
        let pins: Vec<PinIdx> = router.obstacles[o].m_connection_pins.iter().copied().collect();
        let polygon = router.obstacles[o].m_polygon.clone();
        for pin in pins {
            ShapeConnectionPin::update_position_poly(router, pin, &polygon);
        }
    }

    // upstream: obstacle.cpp Obstacle::makeActive()
    pub fn make_active(router: &mut Router, o: ObstacleIdx) {
        debug_assert!(!router.obstacles[o].m_active);

        // Add to shapeRefs list.
        router.m_obstacles.insert(0, o);

        // Add points to vertex list.
        let first = router.obstacles[o].m_first_vert.unwrap();
        let mut it = first;
        loop {
            let tmp = it;
            it = router.vert_infs[it].sh_next.unwrap();

            router.vertices.add_vertex(&mut router.vert_infs, tmp);
            if it == first {
                break;
            }
        }

        router.obstacles[o].m_active = true;
    }

    // upstream: obstacle.cpp Obstacle::makeInactive()
    pub fn make_inactive(router: &mut Router, o: ObstacleIdx) {
        debug_assert!(router.obstacles[o].m_active);

        // Remove from shapeRefs list.
        let pos = router
            .m_obstacles
            .iter()
            .position(|&x| x == o)
            .expect("active obstacle missing from m_obstacles");
        router.m_obstacles.remove(pos);

        // Remove points from vertex list.
        let first = router.obstacles[o].m_first_vert.unwrap();
        let mut it = first;
        loop {
            let tmp = it;
            it = router.vert_infs[it].sh_next.unwrap();

            router.vertices.remove_vertex(&mut router.vert_infs, Some(tmp));
            if it == first {
                break;
            }
        }

        router.obstacles[o].m_active = false;

        // Turn attached ConnEnds into manual points.
        let deleted_shape = true;
        // PTR-ORDER: iterates std::set<ConnEnd *> from begin().
        while let Some(first_ce) = router.obstacles[o].m_following_conns.first().copied() {
            ConnEnd::disconnect(router, first_ce.idx, deleted_shape);
        }
    }

    // upstream: obstacle.cpp Obstacle::updatePinPolyLineVisibility()
    pub fn update_pin_poly_line_visibility(router: &mut Router, o: ObstacleIdx) {
        let pins: Vec<PinIdx> = router.obstacles[o].m_connection_pins.iter().copied().collect();
        for pin in pins {
            ShapeConnectionPin::update_visibility(router, pin);
        }
    }

    // upstream: obstacle.cpp Obstacle::possiblePinPoints()
    pub fn possible_pin_points(router: &Router, o: ObstacleIdx, pin_class_id: u32) -> Vec<Point> {
        let mut points = Vec::new();
        for &curr in router.obstacles[o].m_connection_pins.iter() {
            let curr_pin = &router.pins[curr];
            if (curr_pin.m_class_id == pin_class_id)
                && (!curr_pin.m_exclusive || curr_pin.m_connend_users.is_empty())
            {
                points.push(router.vert_infs[curr_pin.m_vertex.unwrap()].point);
            }
        }
        points
    }

    // upstream: obstacle.cpp Obstacle::addConnectionPin()
    pub fn add_connection_pin(router: &mut Router, o: ObstacleIdx, pin: PinIdx) -> usize {
        {
            let Router {
                obstacles, pins, ..
            } = router;
            let mut set = std::mem::take(&mut obstacles[o].m_connection_pins);
            set.insert(pins, obstacles, pin);
            obstacles[o].m_connection_pins = set;
        }
        router.modify_connection_pin(pin);

        router.obstacles[o].m_connection_pins.len()
    }

    // upstream: obstacle.cpp Obstacle::removeConnectionPin()
    pub fn remove_connection_pin(router: &mut Router, o: ObstacleIdx, pin: PinIdx) {
        {
            let Router {
                obstacles, pins, ..
            } = router;
            let mut set = std::mem::take(&mut obstacles[o].m_connection_pins);
            set.erase(pins, obstacles, pin);
            obstacles[o].m_connection_pins = set;
        }
        router.modify_connection_pin(pin);
    }

    // upstream: obstacle.cpp Obstacle::isActive()
    #[inline]
    pub fn is_active(&self) -> bool {
        self.m_active
    }

    // upstream: obstacle.cpp Obstacle::firstVert()
    #[inline]
    pub fn first_vert(&self) -> Option<VertIdx> {
        self.m_first_vert
    }

    // upstream: obstacle.cpp Obstacle::lastVert()
    #[inline]
    pub fn last_vert(&self) -> Option<VertIdx> {
        self.m_last_vert
    }

    // upstream: obstacle.cpp Obstacle::id()
    #[inline]
    pub fn id(&self) -> u32 {
        self.m_id
    }

    // upstream: obstacle.cpp Obstacle::polygon()
    #[inline]
    pub fn polygon(&self) -> &Polygon {
        &self.m_polygon
    }

    // upstream: obstacle.cpp Obstacle::routingBox()
    pub fn routing_box(router: &Router, o: ObstacleIdx) -> Box {
        let poly = &router.obstacles[o].m_polygon;
        debug_assert!(!poly.empty());

        let buffer_space = router.routing_parameter(RoutingParameter::ShapeBufferDistance);
        poly.offset_bounding_box(buffer_space)
    }

    // upstream: obstacle.cpp Obstacle::routingPolygon()
    pub fn routing_polygon(router: &Router, o: ObstacleIdx) -> Polygon {
        let poly = &router.obstacles[o].m_polygon;
        debug_assert!(!poly.empty());

        let buffer_space = router.routing_parameter(RoutingParameter::ShapeBufferDistance);
        poly.offset_polygon(buffer_space)
    }

    // upstream: obstacle.cpp Obstacle::shapeCentre()
    pub fn shape_centre(router: &Router, o: ObstacleIdx) -> Point {
        let bb = Obstacle::routing_box(router, o);

        let mut centre = Point::default();
        centre.x = bb.min.x + (0.5 * (bb.max.x - bb.min.x));
        centre.y = bb.min.y + (0.5 * (bb.max.y - bb.min.y));
        centre
    }

    // upstream: obstacle.cpp Obstacle::removeFromGraph()
    pub fn remove_from_graph(router: &mut Router, o: ObstacleIdx) {
        let is_conn_pt = false;
        let first = router.obstacles[o].m_first_vert;
        let end = router.vert_infs[router.obstacles[o].m_last_vert.unwrap()].lst_next;
        let mut iter = first;
        while iter != end {
            let tmp = iter.unwrap();
            iter = router.vert_infs[tmp].lst_next;

            VertInf::remove_from_graph(router, tmp, is_conn_pt);
        }
    }

    // upstream: obstacle.cpp Obstacle::getPointVertex()
    pub fn get_point_vertex(router: &Router, o: ObstacleIdx, point: &Point) -> Option<VertIdx> {
        let first = router.obstacles[o].m_first_vert.unwrap();
        let mut curr = first;
        loop {
            if router.vert_infs[curr].point == *point {
                return Some(curr);
            }
            curr = router.vert_infs[curr].sh_next.unwrap();
            if curr == first {
                break;
            }
        }
        None
    }

    // upstream: obstacle.cpp Obstacle::addFollowingConnEnd()
    pub fn add_following_conn_end(router: &mut Router, o: ObstacleIdx, conn_end: PtrKey<ConnEndRef>) {
        router.obstacles[o].m_following_conns.insert(conn_end);
    }

    // upstream: obstacle.cpp Obstacle::removeFollowingConnEnd()
    pub fn remove_following_conn_end(router: &mut Router, o: ObstacleIdx, conn_end: &PtrKey<ConnEndRef>) {
        router.obstacles[o].m_following_conns.remove(conn_end);
    }

    /// Returns a list of ConnRefs attached to the obstacle.
    // upstream: obstacle.cpp Obstacle::attachedConnectors()
    pub fn attached_connectors(router: &Router, o: ObstacleIdx) -> Vec<ConnIdx> {
        let mut attached_conns = Vec::new();
        // PTR-ORDER: iterates std::set<ConnEnd *>; the result order is
        // ConnEnd creation order here, malloc address order upstream.
        for conn_end in router.obstacles[o].m_following_conns.iter() {
            let ce = conn_end.idx.get(router);
            debug_assert!(ce.m_conn_ref.is_some());
            attached_conns.push(ce.m_conn_ref.unwrap());
        }
        attached_conns
    }

    // (upstream: Obstacle::computeVisibilityNaive()/computeVisibilitySweep()
    // live in visibility.cpp: see `crate::avoid::visibility`.)
    // (upstream: Obstacle::outputCode() — debug code output, not ported.)
}

/// Public-API convenience methods on obstacle handles.
impl ObstacleIdx {
    // upstream: obstacle.cpp Obstacle::id()
    pub fn id(self, router: &Router) -> u32 {
        router.obstacles[self].m_id
    }
    // upstream: obstacle.cpp Obstacle::polygon()
    pub fn polygon(self, router: &Router) -> &Polygon {
        &router.obstacles[self].m_polygon
    }
    // upstream: Obstacle::position() (virtual)
    pub fn position(self, router: &Router) -> Point {
        Obstacle::position(router, self)
    }
    // upstream: obstacle.cpp Obstacle::routingBox()
    pub fn routing_box(self, router: &Router) -> Box {
        Obstacle::routing_box(router, self)
    }
    // upstream: obstacle.cpp Obstacle::routingPolygon()
    pub fn routing_polygon(self, router: &Router) -> Polygon {
        Obstacle::routing_polygon(router, self)
    }
    // upstream: obstacle.cpp Obstacle::attachedConnectors()
    pub fn attached_connectors(self, router: &Router) -> Vec<ConnIdx> {
        Obstacle::attached_connectors(router, self)
    }
}
