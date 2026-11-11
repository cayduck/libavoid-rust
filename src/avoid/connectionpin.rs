//! Faithful port of upstream `libavoid/connectionpin.{h,cpp}`.
//!
//! * `ShapeConnectionPin *` is a [`PinIdx`] into `Router::pins`; the pin
//!   carries a creation `serial` (used where upstream orders pins by
//!   pointer: `ActionInfo::operator<` for `ConnectionPinChange`).
//! * `ShapeConnectionPinSet` (`std::set<ShapeConnectionPin *,
//!   CmpConnPinPtr>`) is [`ShapeConnectionPinSet`]: a vector kept in the
//!   set's in-order sequence, with `insert`/`erase` reproducing `std::set`
//!   semantics under the *value* comparator `ShapeConnectionPin::operator<`
//!   (an "equal" pin is not inserted; `erase(pin)` erases the equal element).
//! * `m_connend_users` (`std::set<ConnEnd *>`) is a
//!   `BTreeSet<PtrKey<ConnEndRef>>` (`// PTR-ORDER:`; it is only ever
//!   tested for emptiness or drained, so its order cannot affect results).
//! * `m_router` is implicit (functions take the `Router`).

#![allow(dead_code)]
#![allow(clippy::new_ret_no_self, clippy::too_many_arguments)]

use std::collections::BTreeSet;

use crate::avoid::arena::{Arena, ObstacleIdx, PinIdx, PtrKey, VertIdx};
use crate::avoid::connend::{
    ConnDirFlags, ConnEnd, ConnEndRef, CONN_DIR_ALL, CONN_DIR_DOWN, CONN_DIR_LEFT, CONN_DIR_NONE,
    CONN_DIR_RIGHT, CONN_DIR_UP,
};
use crate::avoid::geomtypes::{Point, PolygonInterface, Polygon, K_SHAPE_CONNECTION_PIN};
use crate::avoid::junction::JunctionRef;
use crate::avoid::obstacle::{Obstacle, ObstacleArena};
use crate::avoid::router::{format_g, Router};
use crate::avoid::shape::ShapeRef;
use crate::avoid::vertices::{VertID, VertInf};
use crate::avoid::visibility::vertex_visibility;

/// Arena of all connection pins of a router.
pub type PinArena = Arena<PinIdx, ShapeConnectionPin>;

// upstream: connectionpin.h CONNECTIONPIN_UNSET / CONNECTIONPIN_CENTRE
pub const CONNECTIONPIN_UNSET: u32 = i32::MAX as u32;
pub const CONNECTIONPIN_CENTRE: u32 = (i32::MAX - 1) as u32;

// upstream: connectionpin.h typedef std::pair<unsigned int, unsigned int> ConnectionPinIds
pub type ConnectionPinIds = (u32, u32);

// upstream: connectionpin.h ATTACH_POS_* constants
pub const ATTACH_POS_TOP: f64 = 0.0;
pub const ATTACH_POS_CENTRE: f64 = 0.5;
pub const ATTACH_POS_BOTTOM: f64 = 1.0;
pub const ATTACH_POS_LEFT: f64 = ATTACH_POS_TOP;
pub const ATTACH_POS_RIGHT: f64 = ATTACH_POS_BOTTOM;
pub const ATTACH_POS_MIN_OFFSET: f64 = 0.0;
pub const ATTACH_POS_MAX_OFFSET: f64 = -1.0;

/// The ShapeConnectionPin class represents a fixed point or "pin" on a
/// shape that can be connected to.
// upstream: connectionpin.h class ShapeConnectionPin
#[derive(Clone, Debug)]
pub struct ShapeConnectionPin {
    /// Monotonic creation serial (`// PTR-ORDER:`).
    pub serial: u64,
    pub m_shape: Option<ObstacleIdx>,
    pub m_junction: Option<ObstacleIdx>,
    pub m_class_id: u32,
    pub m_x_offset: f64,
    pub m_y_offset: f64,
    pub m_inside_offset: f64,
    pub m_visibility_directions: ConnDirFlags,
    pub m_exclusive: bool,
    pub m_connection_cost: f64,
    /// PTR-ORDER: upstream `std::set<ConnEnd *>`.
    pub m_connend_users: BTreeSet<PtrKey<ConnEndRef>>,
    pub m_vertex: Option<VertIdx>,
    pub m_using_proportional_offsets: bool,
}

impl ShapeConnectionPin {
    fn alloc(router: &mut Router, pin: ShapeConnectionPin) -> PinIdx {
        router.pins.alloc(pin)
    }

    /// Constructs a ShapeConnectionPin on a shape, with explicit choice of
    /// proportional or absolute offsets.
    // upstream: connectionpin.cpp ShapeConnectionPin::ShapeConnectionPin(ShapeRef *, unsigned, double, double, bool, double, ConnDirFlags)
    pub fn new_shape(
        router: &mut Router,
        shape: ShapeRef,
        class_id: u32,
        x_portion_offset: f64,
        y_portion_offset: f64,
        proportional: bool,
        inside_offset: f64,
        vis_dirs: ConnDirFlags,
    ) -> PinIdx {
        let serial = router.next_serial();
        let pin = ShapeConnectionPin::alloc(
            router,
            ShapeConnectionPin {
                serial,
                m_shape: Some(shape.0),
                m_junction: None,
                m_class_id: class_id,
                m_x_offset: x_portion_offset,
                m_y_offset: y_portion_offset,
                m_inside_offset: inside_offset,
                m_visibility_directions: vis_dirs,
                m_exclusive: true,
                m_connection_cost: 0.0,
                m_connend_users: BTreeSet::new(),
                m_vertex: None,
                m_using_proportional_offsets: proportional,
            },
        );
        ShapeConnectionPin::common_init_for_shape_connection(router, pin);
        pin
    }

    /// Constructs a ShapeConnectionPin on a shape using proportional
    /// offsets (the six-argument upstream constructor).
    // upstream: connectionpin.cpp ShapeConnectionPin::ShapeConnectionPin(ShapeRef *, unsigned, double, double, double, ConnDirFlags)
    pub fn new_shape_proportional(
        router: &mut Router,
        shape: ShapeRef,
        class_id: u32,
        x_offset: f64,
        y_offset: f64,
        inside_offset: f64,
        vis_dirs: ConnDirFlags,
    ) -> PinIdx {
        ShapeConnectionPin::new_shape(
            router,
            shape,
            class_id,
            x_offset,
            y_offset,
            true,
            inside_offset,
            vis_dirs,
        )
    }

    // upstream: connectionpin.cpp ShapeConnectionPin::commonInitForShapeConnection()
    fn common_init_for_shape_connection(router: &mut Router, pin: PinIdx) {
        let p = router.pins[pin].clone();
        let shape = p.m_shape.expect("pin without shape");
        debug_assert!(p.m_class_id > 0);

        if p.m_using_proportional_offsets {
            // Parameter checking
            if (p.m_x_offset < 0.0) || (p.m_x_offset > 1.0) {
                eprint!(
                    "xPortionOffset value ({}) in ShapeConnectionPin constructor not between 0 and 1.\n",
                    format_g(p.m_x_offset, 6)
                );
            }
            if (p.m_y_offset < 0.0) || (p.m_y_offset > 1.0) {
                eprint!(
                    "yPortionOffset value ({}) in ShapeConnectionPin constructor not between 0 and 1.\n",
                    format_g(p.m_y_offset, 6)
                );
            }
        } else {
            let shape_box = router.obstacles[shape].m_polygon.offset_bounding_box(0.0);
            // Parameter checking
            if p.m_x_offset > shape_box.width() {
                eprint!(
                    "xOffset value ({}) in ShapeConnectionPin constructor greater than shape width ({}).\n",
                    format_g(p.m_x_offset, 6),
                    format_g(shape_box.width(), 6)
                );
            }
            if p.m_y_offset > shape_box.height() {
                eprint!(
                    "yOffset value ({}) in ShapeConnectionPin constructor greater than shape height ({}).\n",
                    format_g(p.m_y_offset, 6),
                    format_g(shape_box.height(), 6)
                );
            }
        }

        Obstacle::add_connection_pin(router, shape, pin);

        // Create a visibility vertex for this ShapeConnectionPin.
        let id = VertID::new(
            router.obstacles[shape].m_id,
            K_SHAPE_CONNECTION_PIN,
            VertID::PROP_CONN_POINT | VertID::PROP_CONNECTION_PIN,
        );
        let pos = router.pins[pin].position(router, None);
        let vertex = VertInf::new(router, id, pos, true);
        router.pins[pin].m_vertex = Some(vertex);
        router.vert_infs[vertex].vis_directions = router.pins[pin].directions();

        if router.vert_infs[vertex].vis_directions == CONN_DIR_ALL {
            // A pin with visibility in all directions is not exclusive
            // by default.
            router.pins[pin].m_exclusive = false;
        }

        if router.m_allows_polyline_routing {
            vertex_visibility(router, vertex, None, true, true);
        }
    }

    /// Constructs a ShapeConnectionPin on a junction.  Upstream default for
    /// `vis_dirs` is `ConnDirNone`.
    // upstream: connectionpin.cpp ShapeConnectionPin::ShapeConnectionPin(JunctionRef *, unsigned, ConnDirFlags)
    pub fn new_junction(
        router: &mut Router,
        junction: JunctionRef,
        class_id: u32,
        vis_dirs: ConnDirFlags,
    ) -> PinIdx {
        let serial = router.next_serial();
        let pin = ShapeConnectionPin::alloc(
            router,
            ShapeConnectionPin {
                serial,
                m_shape: None,
                m_junction: Some(junction.0),
                m_class_id: class_id,
                m_x_offset: 0.0,
                m_y_offset: 0.0,
                m_inside_offset: 0.0,
                m_visibility_directions: vis_dirs,
                m_exclusive: true,
                m_connection_cost: 0.0,
                m_connend_users: BTreeSet::new(),
                m_vertex: None,
                m_using_proportional_offsets: false,
            },
        );
        Obstacle::add_connection_pin(router, junction.0, pin);

        // Create a visibility vertex for this ShapeConnectionPin.
        // XXX These IDs should really be uniquely identifiable in case there
        //     are multiple pins on a shape.  I think currently this case will
        //     break rubber-band routing.
        let id = VertID::new(
            router.obstacles[junction.0].m_id,
            K_SHAPE_CONNECTION_PIN,
            VertID::PROP_CONN_POINT | VertID::PROP_CONNECTION_PIN,
        );
        let pos = Obstacle::position(router, junction.0);
        let vertex = VertInf::new(router, id, pos, true);
        router.pins[pin].m_vertex = Some(vertex);
        router.vert_infs[vertex].vis_directions = vis_dirs;

        if router.m_allows_polyline_routing {
            vertex_visibility(router, vertex, None, true, true);
        }
        pin
    }

    /// Upstream `delete pin`.
    // upstream: connectionpin.cpp ShapeConnectionPin::~ShapeConnectionPin()
    pub fn delete(router: &mut Router, pin: PinIdx) {
        let (shape, junction) = (router.pins[pin].m_shape, router.pins[pin].m_junction);
        debug_assert!(shape.is_some() || junction.is_some());
        if let Some(s) = shape {
            Obstacle::remove_connection_pin(router, s, pin);
        } else if let Some(j) = junction {
            Obstacle::remove_connection_pin(router, j, pin);
        }

        // Disconnect connend using this pin.
        while let Some(first) = router.pins[pin].m_connend_users.first().copied() {
            ConnEnd::free_active_pin(router, first.idx);
        }

        if let Some(v) = router.pins[pin].m_vertex {
            VertInf::remove_from_graph(router, v, true);
            router.vertices.remove_vertex(&mut router.vert_infs, Some(v));
            VertInf::delete(router, v);
            router.pins[pin].m_vertex = None;
        }
        router.pins.free(pin);
    }

    // upstream: connectionpin.cpp ShapeConnectionPin::updatePositionAndVisibility()
    pub fn update_position_and_visibility(router: &mut Router, pin: PinIdx) {
        let v = router.pins[pin].m_vertex.unwrap();
        let pos = router.pins[pin].position(router, None);
        router.vert_infs[v].reset_point(pos);
        router.vert_infs[v].vis_directions = router.pins[pin].directions();
        ShapeConnectionPin::update_visibility(router, pin);
    }

    // upstream: connectionpin.cpp ShapeConnectionPin::updateVisibility()
    pub fn update_visibility(router: &mut Router, pin: PinIdx) {
        let v = router.pins[pin].m_vertex.unwrap();
        VertInf::remove_from_graph(router, v, true);
        if router.m_allows_polyline_routing {
            vertex_visibility(router, v, None, true, true);
        }
    }

    /// Sets a cost used when selecting whether connectors should be
    /// routed to this connection pin.
    // upstream: connectionpin.cpp ShapeConnectionPin::setConnectionCost()
    pub fn set_connection_cost(&mut self, cost: f64) {
        debug_assert!(cost >= 0.0);

        self.m_connection_cost = cost;
    }

    // upstream: connectionpin.cpp ShapeConnectionPin::setExclusive()
    pub fn set_exclusive(&mut self, exclusive: bool) {
        self.m_exclusive = exclusive;
    }

    // upstream: connectionpin.cpp ShapeConnectionPin::isExclusive()
    pub fn is_exclusive(&self) -> bool {
        self.m_exclusive
    }

    // upstream: connectionpin.cpp ShapeConnectionPin::updatePosition(const Point&)
    pub fn update_position_point(router: &mut Router, pin: PinIdx, new_position: Point) {
        let v = router.pins[pin].m_vertex.unwrap();
        router.vert_infs[v].reset_point(new_position);
    }

    // upstream: connectionpin.cpp ShapeConnectionPin::updatePosition(const Polygon&)
    pub fn update_position_poly(router: &mut Router, pin: PinIdx, new_poly: &Polygon) {
        let v = router.pins[pin].m_vertex.unwrap();
        let pos = router.pins[pin].position(router, Some(new_poly));
        router.vert_infs[v].reset_point(pos);
    }

    /// Returns the position of this connection pin.  `new_poly` is the
    /// upstream optional polygon argument (default `Polygon()`, i.e. use the
    /// shape's current polygon); an empty polygon behaves like `None`.
    // upstream: connectionpin.cpp ShapeConnectionPin::position()
    pub fn position(&self, router: &Router, new_poly: Option<&Polygon>) -> Point {
        if let Some(j) = self.m_junction {
            return Obstacle::position(router, j);
        }

        let shape = self.m_shape.expect("pin without shape or junction");
        let poly: &Polygon = match new_poly {
            Some(p) if !p.empty() => p,
            _ => &router.obstacles[shape].m_polygon,
        };
        let shape_box = poly.offset_bounding_box(0.0);

        let mut point = Point::default();

        if self.m_using_proportional_offsets {
            // We want to place connection points exactly on the edges of shapes,
            // or possibly slightly inside them (if m_insideOfset is set).
            if self.m_x_offset == ATTACH_POS_LEFT {
                point.x = shape_box.min.x + self.m_inside_offset;
                point.vn = 6;
            } else if self.m_x_offset == ATTACH_POS_RIGHT {
                point.x = shape_box.max.x - self.m_inside_offset;
                point.vn = 4;
            } else {
                point.x = shape_box.min.x + (self.m_x_offset * shape_box.width());
            }

            if self.m_y_offset == ATTACH_POS_TOP {
                point.y = shape_box.min.y + self.m_inside_offset;
                point.vn = 5;
            } else if self.m_y_offset == ATTACH_POS_BOTTOM {
                point.y = shape_box.max.y - self.m_inside_offset;
                point.vn = 7;
            } else {
                point.y = shape_box.min.y + (self.m_y_offset * shape_box.height());
            }
        } else {
            // Using absolute offsets for connection pin position.
            if self.m_x_offset == ATTACH_POS_MIN_OFFSET {
                point.x = shape_box.min.x + self.m_inside_offset;
                point.vn = 6;
            } else if (self.m_x_offset == ATTACH_POS_MAX_OFFSET)
                || (self.m_x_offset == shape_box.width())
            {
                point.x = shape_box.max.x - self.m_inside_offset;
                point.vn = 4;
            } else {
                point.x = shape_box.min.x + self.m_x_offset;
            }

            if self.m_y_offset == ATTACH_POS_MIN_OFFSET {
                point.y = shape_box.min.y + self.m_inside_offset;
                point.vn = 5;
            } else if (self.m_y_offset == ATTACH_POS_MAX_OFFSET)
                || (self.m_y_offset == shape_box.height())
            {
                point.y = shape_box.max.y - self.m_inside_offset;
                point.vn = 7;
            } else {
                point.y = shape_box.min.y + self.m_y_offset;
            }
        }

        point
    }

    /// Returns the directions in which this connection pin has visibility.
    // upstream: connectionpin.cpp ShapeConnectionPin::directions()
    pub fn directions(&self) -> ConnDirFlags {
        let mut vis_dir = self.m_visibility_directions;
        if self.m_visibility_directions == CONN_DIR_NONE {
            // None is set, use the defaults:
            if self.m_x_offset == ATTACH_POS_LEFT {
                vis_dir |= CONN_DIR_LEFT;
            } else if self.m_x_offset == ATTACH_POS_RIGHT {
                vis_dir |= CONN_DIR_RIGHT;
            }

            if self.m_y_offset == ATTACH_POS_TOP {
                vis_dir |= CONN_DIR_UP;
            } else if self.m_y_offset == ATTACH_POS_BOTTOM {
                vis_dir |= CONN_DIR_DOWN;
            }

            if vis_dir == CONN_DIR_NONE {
                vis_dir = CONN_DIR_ALL;
            }
        }
        vis_dir
    }

    // (upstream: connectionpin.cpp ShapeConnectionPin::outputCode() — debug
    // code output, not ported.)

    // upstream: connectionpin.cpp ShapeConnectionPin::ids()
    pub fn ids(&self, obstacles: &ObstacleArena) -> ConnectionPinIds {
        (self.containing_object_id(obstacles), self.m_class_id)
    }

    // upstream: connectionpin.cpp ShapeConnectionPin::containingObjectId()
    pub fn containing_object_id(&self, obstacles: &ObstacleArena) -> u32 {
        debug_assert!(self.m_shape.is_some() || self.m_junction.is_some());
        match self.m_shape {
            Some(s) => obstacles[s].m_id,
            None => obstacles[self.m_junction.unwrap()].m_id,
        }
    }

    // upstream: connectionpin.cpp ShapeConnectionPin::operator==()
    pub fn equals(&self, rhs: &ShapeConnectionPin, obstacles: &ObstacleArena) -> bool {
        if self.containing_object_id(obstacles) != rhs.containing_object_id(obstacles) {
            return false;
        }

        // The shape/junction is equal, so examine the unique members.
        if self.m_class_id != rhs.m_class_id {
            return false;
        }
        if self.m_visibility_directions != rhs.m_visibility_directions {
            return false;
        }
        if self.m_x_offset != rhs.m_x_offset {
            return false;
        }
        if self.m_y_offset != rhs.m_y_offset {
            return false;
        }
        if self.m_inside_offset != rhs.m_inside_offset {
            return false;
        }
        true
    }

    // upstream: connectionpin.cpp ShapeConnectionPin::operator<()
    pub fn less_than(&self, rhs: &ShapeConnectionPin, obstacles: &ObstacleArena) -> bool {
        let lid = self.containing_object_id(obstacles);
        let rid = rhs.containing_object_id(obstacles);
        if lid != rid {
            return lid < rid;
        }
        // Note: operator< is used for set ordering within each shape or junction,
        // so the m_shape/m_junction values should match and we needn't perform the
        // above test in most cases.

        if self.m_class_id != rhs.m_class_id {
            return self.m_class_id < rhs.m_class_id;
        }
        if self.m_visibility_directions != rhs.m_visibility_directions {
            return self.m_visibility_directions < rhs.m_visibility_directions;
        }
        if self.m_x_offset != rhs.m_x_offset {
            return self.m_x_offset < rhs.m_x_offset;
        }
        if self.m_y_offset != rhs.m_y_offset {
            return self.m_y_offset < rhs.m_y_offset;
        }
        if self.m_inside_offset != rhs.m_inside_offset {
            return self.m_inside_offset < rhs.m_inside_offset;
        }

        // Otherwise, they are considered the same.
        false
    }
}

/// `CmpConnPinPtr`.
// upstream: connectionpin.h class CmpConnPinPtr
#[inline]
pub fn cmp_conn_pin_ptr(pins: &PinArena, obstacles: &ObstacleArena, lhs: PinIdx, rhs: PinIdx) -> bool {
    pins[lhs].less_than(&pins[rhs], obstacles)
}

/// Upstream `std::set<ShapeConnectionPin *, CmpConnPinPtr>`.
///
/// `items` is the set's in-order sequence.  `insert`/`erase` do a binary
/// search with the upstream comparator.  Upstream mutates pins' keys in
/// place (`ShapeRef::transformConnectionPinPositions`) without re-sorting;
/// like the red-black tree, this keeps the old element order.  (After such
/// a mutation a later insert/erase could in principle probe differently
/// from the tree's descent; see the port notes.)
// upstream: connectionpin.h typedef ShapeConnectionPinSet
#[derive(Clone, Debug, Default)]
pub struct ShapeConnectionPinSet {
    pub items: Vec<PinIdx>,
}

impl ShapeConnectionPinSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// `lower_bound(pin)`.
    fn lower_bound(&self, pins: &PinArena, obstacles: &ObstacleArena, pin: PinIdx) -> usize {
        self.items
            .partition_point(|&x| cmp_conn_pin_ptr(pins, obstacles, x, pin))
    }

    /// `insert(pin)`; returns whether it was inserted.
    pub fn insert(&mut self, pins: &PinArena, obstacles: &ObstacleArena, pin: PinIdx) -> bool {
        let pos = self.lower_bound(pins, obstacles, pin);
        if pos < self.items.len() && !cmp_conn_pin_ptr(pins, obstacles, pin, self.items[pos]) {
            // An equivalent element already exists.
            return false;
        }
        self.items.insert(pos, pin);
        true
    }

    /// `erase(pin)` (by key); returns the number of erased elements.
    pub fn erase(&mut self, pins: &PinArena, obstacles: &ObstacleArena, pin: PinIdx) -> usize {
        let pos = self.lower_bound(pins, obstacles, pin);
        if pos < self.items.len() && !cmp_conn_pin_ptr(pins, obstacles, pin, self.items[pos]) {
            self.items.remove(pos);
            return 1;
        }
        0
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, PinIdx> {
        self.items.iter()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    #[inline]
    pub fn first(&self) -> Option<PinIdx> {
        self.items.first().copied()
    }
}

/// Public-API convenience methods on pin handles.
impl PinIdx {
    // upstream: connectionpin.cpp ShapeConnectionPin::setConnectionCost()
    pub fn set_connection_cost(self, router: &mut Router, cost: f64) {
        router.pins[self].set_connection_cost(cost);
    }
    // upstream: connectionpin.cpp ShapeConnectionPin::setExclusive()
    pub fn set_exclusive(self, router: &mut Router, exclusive: bool) {
        router.pins[self].set_exclusive(exclusive);
    }
    // upstream: connectionpin.cpp ShapeConnectionPin::isExclusive()
    pub fn is_exclusive(self, router: &Router) -> bool {
        router.pins[self].is_exclusive()
    }
    // upstream: connectionpin.cpp ShapeConnectionPin::position()
    pub fn position(self, router: &Router) -> Point {
        router.pins[self].position(router, None)
    }
    // upstream: connectionpin.cpp ShapeConnectionPin::directions()
    pub fn directions(self, router: &Router) -> ConnDirFlags {
        router.pins[self].directions()
    }
    // upstream: connectionpin.cpp ShapeConnectionPin::ids()
    pub fn ids(self, router: &Router) -> ConnectionPinIds {
        router.pins[self].ids(&router.obstacles)
    }
}
