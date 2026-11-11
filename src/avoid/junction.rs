//! Faithful port of upstream `libavoid/junction.{h,cpp}`.
//!
//! `JunctionRef *` is the `Copy` handle [`JunctionRef`] wrapping the
//! [`ObstacleIdx`] of an [`Obstacle`] whose kind is
//! `ObstacleKind::Junction(JunctionData)`.

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

use crate::avoid::arena::{ConnIdx, ObstacleIdx, PinIdx};
use crate::avoid::connectionpin::{ShapeConnectionPin, CONNECTIONPIN_CENTRE};
use crate::avoid::connend::{ConnEnd, CONN_DIR_ALL, CONN_DIR_DOWN, CONN_DIR_LEFT, CONN_DIR_RIGHT, CONN_DIR_UP};
use crate::avoid::geomtypes::{cpp_min, Point, Polygon, Rectangle, XDIM, YDIM};
use crate::avoid::obstacle::{Obstacle, ObstacleKind};
use crate::avoid::router::{Router, RoutingParameter};
use crate::avoid::vertices::VertID;

/// The `JunctionRef`-specific members.
// upstream: junction.h class JunctionRef (private members)
#[derive(Clone, Debug)]
pub struct JunctionData {
    pub m_position: Point,
    pub m_recommended_position: Point,
    pub m_position_fixed: bool,
}

/// Handle of a junction obstacle (upstream `JunctionRef *`).
// upstream: junction.h class JunctionRef
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct JunctionRef(pub ObstacleIdx);

impl From<JunctionRef> for ObstacleIdx {
    fn from(j: JunctionRef) -> ObstacleIdx {
        j.0
    }
}

impl JunctionRef {
    /// Junction reference constructor: creates a junction and queues it
    /// for addition to the router scene.  `id` 0 means "assign one".
    // upstream: junction.cpp JunctionRef::JunctionRef()
    pub fn new(router: &mut Router, position: Point, id: u32) -> JunctionRef {
        let rect = JunctionRef::make_rectangle(router, &position);
        let o = Obstacle::new_base(
            router,
            rect,
            id,
            ObstacleKind::Junction(JunctionData {
                m_position: position,
                m_recommended_position: position,
                m_position_fixed: false,
            }),
        );
        let junction = JunctionRef(o);

        // For Junctions we use a single non-exclusive pin.
        let pin = ShapeConnectionPin::new_junction(router, junction, CONNECTIONPIN_CENTRE, CONN_DIR_ALL);
        router.pins[pin].set_exclusive(false);
        {
            // m_connection_pins.insert(pin) (a no-op: already inserted by
            // the pin constructor).
            let Router {
                obstacles, pins, ..
            } = router;
            let mut set = std::mem::take(&mut obstacles[o].m_connection_pins);
            set.insert(pins, obstacles, pin);
            obstacles[o].m_connection_pins = set;
        }

        router.add_junction(junction);
        junction
    }

    /// The underlying obstacle handle.
    #[inline]
    pub fn obstacle(self) -> ObstacleIdx {
        self.0
    }

    // upstream: obstacle.cpp Obstacle::id()
    pub fn id(self, router: &Router) -> u32 {
        router.obstacles[self.0].m_id
    }

    // upstream: obstacle.cpp Obstacle::polygon()
    pub fn polygon(self, router: &Router) -> &Polygon {
        &router.obstacles[self.0].m_polygon
    }

    // upstream: junction.cpp JunctionRef::makeRectangle()
    pub fn make_rectangle(router: &Router, position: &Point) -> Polygon {
        // A suitably small value.
        let mut nudge_dist = router.routing_parameter(RoutingParameter::IdealNudgingDistance);
        nudge_dist = cpp_min(1.0, nudge_dist);

        let mut low = *position;
        low.x -= nudge_dist;
        low.y -= nudge_dist;

        let mut high = *position;
        high.x += nudge_dist;
        high.y += nudge_dist;

        Rectangle::new(&low, &high)
    }

    /// Sets whether the junction has a fixed position.
    // upstream: junction.cpp JunctionRef::setPositionFixed()
    pub fn set_position_fixed(self, router: &mut Router, fixed: bool) {
        router.obstacles[self.0].junction_data_mut().m_position_fixed = fixed;
        router.register_settings_change();
    }

    // upstream: junction.cpp JunctionRef::positionFixed()
    pub fn position_fixed(self, router: &Router) -> bool {
        router.obstacles[self.0].junction_data().m_position_fixed
    }

    // upstream: junction.cpp JunctionRef::preferOrthogonalDimension()
    pub fn prefer_orthogonal_dimension(self, router: &mut Router, dim: usize) {
        let small_penalty = 1.0;
        let pins: Vec<PinIdx> = router.obstacles[self.0].m_connection_pins.iter().copied().collect();
        for pin in pins {
            let p = &mut router.pins[pin];
            if dim == YDIM {
                if p.directions() & (CONN_DIR_LEFT | CONN_DIR_RIGHT) != 0 {
                    p.set_connection_cost(small_penalty);
                }
            } else if dim == XDIM {
                if p.directions() & (CONN_DIR_UP | CONN_DIR_DOWN) != 0 {
                    p.set_connection_cost(small_penalty);
                }
            }
        }
    }

    // upstream: junction.cpp JunctionRef::position()
    pub fn position(self, router: &Router) -> Point {
        router.obstacles[self.0].junction_data().m_position
    }

    // upstream: junction.cpp JunctionRef::setPosition()
    pub fn set_position(self, router: &mut Router, position: &Point) {
        {
            let jd = router.obstacles[self.0].junction_data_mut();
            jd.m_position = *position;
            jd.m_recommended_position = *position;
        }
        let rect = JunctionRef::make_rectangle(router, position);
        router.obstacles[self.0].m_polygon = rect.clone();
        Obstacle::set_new_poly(router, self.0, &rect);
    }

    // upstream: junction.cpp JunctionRef::recommendedPosition()
    pub fn recommended_position(self, router: &Router) -> Point {
        router.obstacles[self.0].junction_data().m_recommended_position
    }

    // upstream: junction.cpp JunctionRef::setRecommendedPosition()
    pub fn set_recommended_position(self, router: &mut Router, position: &Point) {
        router.obstacles[self.0].junction_data_mut().m_recommended_position = *position;
    }

    // (upstream: junction.cpp JunctionRef::outputCode() — debug code output,
    // not ported.)

    // upstream: junction.cpp JunctionRef::moveAttachedConns()
    pub fn move_attached_conns(self, router: &mut Router, new_position: &Point) {
        // Update positions of attached connector ends.
        // PTR-ORDER: iterates std::set<ConnEnd *>; the order determines the
        // order in which ConnChange actions are appended to the action list.
        let following: Vec<_> = router.obstacles[self.0].m_following_conns.iter().copied().collect();
        for curr in following {
            let conn_end = curr.idx;
            let ce = conn_end.get(router);
            debug_assert!(ce.m_conn_ref.is_some());
            let conn = ce.m_conn_ref.unwrap();
            let ce_copy = ce.clone();
            let ty = ConnEnd::endpoint_type(router, conn_end);
            router.modify_connector_end(conn, ty, &ce_copy, false);
        }
        let pins: Vec<PinIdx> = router.obstacles[self.0].m_connection_pins.iter().copied().collect();
        for pin in pins {
            ShapeConnectionPin::update_position_point(router, pin, *new_position);
        }
    }

    /// Removes a junction that has only two connectors attached to it and
    /// merges them into a single connector.  Returns the merged connector.
    // upstream: junction.cpp JunctionRef::removeJunctionAndMergeConnectors()
    pub fn remove_junction_and_merge_connectors(self, router: &mut Router) -> Option<ConnIdx> {
        if router.obstacles[self.0].m_following_conns.len() != 2 {
            return None;
        }

        // PTR-ORDER: the first/second ConnEnd of std::set<ConnEnd *> decides
        // which connector survives (malloc order upstream, creation order
        // here).
        let mut curr = router.obstacles[self.0].m_following_conns.iter();
        let conn_end1 = curr.next().unwrap().idx;
        let conn_end2 = curr.next().unwrap().idx;
        debug_assert!(conn_end2.get(router).m_conn_ref.is_some());
        debug_assert!(conn_end1.get(router).m_conn_ref.is_some());

        // The second conn will be the one we will delete.
        let conn2 = conn_end2.get(router).m_conn_ref.unwrap();
        // Determine its endpoint that is not attached to the junction.
        let conn2_ref = &router.conns[conn2];
        let conn_end2_is_src = conn_end2.conn == conn2 && conn_end2.end == VertID::SRC as u32
            && conn2_ref.m_src_connend.is_some();
        let conn_end2_other = if conn_end2_is_src {
            conn2_ref.m_dst_connend.clone()
        } else {
            conn2_ref.m_src_connend.clone()
        };
        let conn_end2_other = match conn_end2_other {
            // If it doesn't have a valid other endpoint, then ignore.
            None => return None,
            Some(ce) => ce,
        };
        // Modify the first connector's junction endpoint to connect to the
        // other end of the second connector.
        let conn1 = conn_end1.get(router).m_conn_ref.unwrap();
        let ty1 = ConnEnd::endpoint_type(router, conn_end1);
        router.modify_connector_end(conn1, ty1, &conn_end2_other, false);

        // Delete the second connector.
        router.delete_connector(conn2);

        // Remove the junction from the router scene.  It should get deleted later.
        router.delete_junction(self);

        // Return the first (i.e. merged) connector.
        conn_end1.get(router).m_conn_ref
    }

    // upstream: obstacle.cpp Obstacle::attachedConnectors()
    pub fn attached_connectors(self, router: &Router) -> Vec<ConnIdx> {
        Obstacle::attached_connectors(router, self.0)
    }

    /// The junction's connection pins in `ShapeConnectionPinSet` order.
    pub fn connection_pins(self, router: &Router) -> Vec<PinIdx> {
        router.obstacles[self.0].m_connection_pins.iter().copied().collect()
    }
}
