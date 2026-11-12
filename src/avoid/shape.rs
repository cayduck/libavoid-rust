//! Faithful port of upstream `libavoid/shape.{h,cpp}`.
//!
//! `ShapeRef *` is the `Copy` handle [`ShapeRef`] wrapping the
//! [`ObstacleIdx`] of an [`Obstacle`] whose kind is `ObstacleKind::Shape`.

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
use crate::avoid::connectionpin::{ShapeConnectionPin, ATTACH_POS_MAX_OFFSET, ATTACH_POS_MIN_OFFSET};
use crate::avoid::connend::{
    ConnEnd, CONN_DIR_ALL, CONN_DIR_DOWN, CONN_DIR_LEFT, CONN_DIR_NONE, CONN_DIR_RIGHT,
    CONN_DIR_UP,
};
use crate::avoid::geomtypes::{Box, Point, Polygon, PolygonInterface, XDIM, YDIM};
use crate::avoid::obstacle::{Obstacle, ObstacleKind};
use crate::avoid::router::Router;

/// Describes the type of transformation that has been applied to a shape
/// having its transformConnectionPinPositions() method called.
// upstream: shape.h enum ShapeTransformationType
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeTransformationType {
    /// The shape is rotated clockwise by 90 degrees.
    TransformationTypeCW90 = 0,
    /// The shape is rotated clockwise by 180 degrees.
    TransformationTypeCW180 = 1,
    /// The shape is rotated clockwise by 270 degrees.
    TransformationTypeCW270 = 2,
    /// The shape is flipped horizontally in the X-dimension.
    TransformationTypeFlipX = 3,
    /// The shape is flipped vertically in the Y-dimension.
    TransformationTypeFlipY = 4,
}

/// Handle of a shape obstacle (upstream `ShapeRef *`).
// upstream: shape.h class ShapeRef
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ShapeRef(pub ObstacleIdx);

impl From<ShapeRef> for ObstacleIdx {
    fn from(s: ShapeRef) -> ObstacleIdx {
        s.0
    }
}

impl ShapeRef {
    /// Shape reference constructor: creates a shape obstacle and queues it
    /// for addition to the router scene.  `id` 0 means "assign one".
    // upstream: shape.cpp ShapeRef::ShapeRef()
    pub fn new(router: &mut Router, poly: &Polygon, id: u32) -> ShapeRef {
        let o = Obstacle::new_base(router, poly.clone(), id, ObstacleKind::Shape);
        let shape = ShapeRef(o);
        // MISUSE (port addition): a shape with an empty polygon is "inert":
        // it is never added to the scene (upstream dereferences null while
        // constructing it).  It can be deleted; moves are ignored.
        // Likewise for a polygon with non-finite coordinates.
        if router.obstacles[o].m_first_vert.is_none() || !poly.ps.iter().all(|p| p.is_finite()) {
            return shape;
        }
        router.add_shape(shape);
        shape
    }

    /// The underlying obstacle handle.
    #[inline]
    pub fn obstacle(self) -> ObstacleIdx {
        self.0
    }

    // upstream: obstacle.cpp Obstacle::id()
    pub fn id(self, router: &Router) -> u32 {
        // MISUSE: a handle of a freed object is a no-op (router.rs
        // module docs, "Public-API misuse").
        if !router.obstacles.contains(self.0) {
            return 0;
        }
        router.obstacles[self.0].m_id
    }

    /// Returns a reference to the polygon boundary of this shape.
    // upstream: shape.cpp ShapeRef::polygon()
    pub fn polygon(self, router: &Router) -> &Polygon {
        // MISUSE: a handle of a freed object is a no-op (router.rs
        // module docs, "Public-API misuse").
        if !router.obstacles.contains(self.0) {
            return &crate::avoid::geomtypes::EMPTY_POLYGON;
        }
        &router.obstacles[self.0].m_polygon
    }

    /// Returns the centre of the shape's routing box.
    // upstream: shape.cpp ShapeRef::position()
    pub fn position(self, router: &Router) -> Point {
        // MISUSE: a handle of a freed object is a no-op (router.rs
        // module docs, "Public-API misuse").
        if !router.obstacles.contains(self.0) {
            return Point::default();
        }
        let b_box = Obstacle::routing_box(router, self.0);

        let mut centre = Point::default();

        centre.x = b_box.min.x + (0.5 * (b_box.max.x - b_box.min.x));
        centre.y = b_box.min.y + (0.5 * (b_box.max.y - b_box.min.y));

        centre
    }

    // upstream: obstacle.cpp Obstacle::routingBox()
    pub fn routing_box(self, router: &Router) -> Box {
        // MISUSE: a handle of a freed object is a no-op (router.rs
        // module docs, "Public-API misuse").
        if !router.obstacles.contains(self.0) {
            return Box::default();
        }
        Obstacle::routing_box(router, self.0)
    }

    // upstream: obstacle.cpp Obstacle::routingPolygon()
    pub fn routing_polygon(self, router: &Router) -> Polygon {
        // MISUSE: a handle of a freed object is a no-op (router.rs
        // module docs, "Public-API misuse").
        if !router.obstacles.contains(self.0) {
            return Polygon::new();
        }
        Obstacle::routing_polygon(router, self.0)
    }

    // upstream: obstacle.cpp Obstacle::attachedConnectors()
    pub fn attached_connectors(self, router: &Router) -> Vec<ConnIdx> {
        // MISUSE: a handle of a freed object is a no-op (router.rs
        // module docs, "Public-API misuse").
        if !router.obstacles.contains(self.0) {
            return Vec::new();
        }
        Obstacle::attached_connectors(router, self.0)
    }

    /// The shape's connection pins in `ShapeConnectionPinSet` order.
    pub fn connection_pins(self, router: &Router) -> Vec<PinIdx> {
        // MISUSE: a handle of a freed object is a no-op (router.rs
        // module docs, "Public-API misuse").
        if !router.obstacles.contains(self.0) {
            return Vec::new();
        }
        router.obstacles[self.0].m_connection_pins.iter().copied().collect()
    }

    // upstream: shape.cpp ShapeRef::moveAttachedConns()
    pub fn move_attached_conns(self, router: &mut Router, new_poly: &Polygon) {
        // MISUSE: a handle of a freed object is a no-op (router.rs
        // module docs, "Public-API misuse").
        if !router.obstacles.contains(self.0) {
            return;
        }
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
            let conn_pin_update = true;
            let ty = ConnEnd::endpoint_type(router, conn_end);
            router.modify_connector_end(conn, ty, &ce_copy, conn_pin_update);
        }
        let pins: Vec<PinIdx> = router.obstacles[self.0].m_connection_pins.iter().copied().collect();
        for pin in pins {
            ShapeConnectionPin::update_position_poly(router, pin, new_poly);
        }
    }

    /// Adjusts all of the shape's connection pin positions and visibility
    /// directions for a given transformation type.
    // upstream: shape.cpp ShapeRef::transformConnectionPinPositions()
    pub fn transform_connection_pin_positions(self, router: &mut Router, transform: ShapeTransformationType) {
        // MISUSE: a handle of a freed object is a no-op (router.rs
        // module docs, "Public-API misuse").
        if !router.obstacles.contains(self.0) {
            return;
        }
        use ShapeTransformationType::*;
        let pins: Vec<PinIdx> = router.obstacles[self.0].m_connection_pins.iter().copied().collect();
        for pin in pins {
            // (upstream stores this bool in a double.)
            let using_proportional_offsets = router.pins[pin].m_using_proportional_offsets;
            let shape_of_pin = router.pins[pin].m_shape;
            let mut x_offset = router.pins[pin].m_x_offset;
            let mut y_offset = router.pins[pin].m_y_offset;
            let mut vis_dirs = router.pins[pin].m_visibility_directions;
            let tmp_offset;

            // Number of clockwise 90 degree rotations;
            let mut rotation_n: u32 = 0;

            if using_proportional_offsets {
                // Translate to Origin.
                x_offset -= 0.5;
                y_offset -= 0.5;

                match transform {
                    TransformationTypeCW90 => {
                        rotation_n = 3;
                        // Y <- inverse X, X <- inverse Y
                        tmp_offset = y_offset;
                        y_offset = x_offset;
                        x_offset = -tmp_offset;
                    }
                    TransformationTypeCW180 => {
                        rotation_n = 2;
                        // Y <- inverse Y, X <- inverse X
                        y_offset = -y_offset;
                        x_offset = -x_offset;
                    }
                    TransformationTypeCW270 => {
                        rotation_n = 1;
                        // Y <- X, X <- Y
                        tmp_offset = y_offset;
                        y_offset = -x_offset;
                        x_offset = tmp_offset;
                    }
                    TransformationTypeFlipX => {
                        // Y <- Y, X <- inverse X
                        x_offset = -x_offset;
                    }
                    TransformationTypeFlipY => {
                        // X <- inverse X, Y <- Y
                        y_offset = -y_offset;
                    }
                }
                // Translate back.
                x_offset += 0.5;
                y_offset += 0.5;
            } else {
                // Using absolute offsets for pin.

                let shape_box = router.obstacles[shape_of_pin.expect("pin without shape")]
                    .m_polygon
                    .offset_bounding_box(0.0);
                match transform {
                    TransformationTypeCW90 => {
                        rotation_n = 3;
                        // Y <- inverse X, X <- inverse Y
                        tmp_offset = y_offset;
                        y_offset = x_offset;
                        x_offset = absolute_offset_inverse(tmp_offset, &shape_box, XDIM);
                    }
                    TransformationTypeCW180 => {
                        rotation_n = 2;
                        // Y <- inverse Y, X <- inverse X
                        y_offset = absolute_offset_inverse(y_offset, &shape_box, YDIM);
                        x_offset = absolute_offset_inverse(x_offset, &shape_box, XDIM);
                    }
                    TransformationTypeCW270 => {
                        rotation_n = 1;
                        // Y <- X, X <- Y
                        tmp_offset = y_offset;
                        y_offset = absolute_offset_inverse(x_offset, &shape_box, YDIM);
                        x_offset = tmp_offset;
                    }
                    TransformationTypeFlipX => {
                        // Y <- Y, X <- inverse X
                        x_offset = absolute_offset_inverse(x_offset, &shape_box, XDIM);
                    }
                    TransformationTypeFlipY => {
                        // X <- inverse X, Y <- Y
                        y_offset = absolute_offset_inverse(y_offset, &shape_box, YDIM);
                    }
                }
            }

            if (vis_dirs & CONN_DIR_ALL) != 0 && (vis_dirs != CONN_DIR_ALL) {
                // Visibility is set, but not in all directions.

                const DIR_U: usize = 0;
                const DIR_R: usize = 1;
                const DIR_D: usize = 2;
                const DIR_L: usize = 3;

                let mut vis_in_dir = [false; 4];
                if vis_dirs & CONN_DIR_UP != 0 {
                    vis_in_dir[DIR_U] = true;
                }
                if vis_dirs & CONN_DIR_RIGHT != 0 {
                    vis_in_dir[DIR_R] = true;
                }
                if vis_dirs & CONN_DIR_DOWN != 0 {
                    vis_in_dir[DIR_D] = true;
                }
                if vis_dirs & CONN_DIR_LEFT != 0 {
                    vis_in_dir[DIR_L] = true;
                }

                if transform == TransformationTypeFlipY {
                    vis_in_dir.swap(DIR_U, DIR_D);
                } else if transform == TransformationTypeFlipX {
                    vis_in_dir.swap(DIR_L, DIR_R);
                }

                let rn = rotation_n as usize;
                vis_dirs = CONN_DIR_NONE;
                if vis_in_dir[(rn + DIR_U) % 4] {
                    vis_dirs |= CONN_DIR_UP;
                }
                if vis_in_dir[(rn + DIR_R) % 4] {
                    vis_dirs |= CONN_DIR_RIGHT;
                }
                if vis_in_dir[(rn + DIR_D) % 4] {
                    vis_dirs |= CONN_DIR_DOWN;
                }
                if vis_in_dir[(rn + DIR_L) % 4] {
                    vis_dirs |= CONN_DIR_LEFT;
                }
            }
            {
                // Write back the (in-place upstream) modifications.  Like
                // upstream, the pin's position in the pin set is not
                // updated.
                let p = &mut router.pins[pin];
                p.m_x_offset = x_offset;
                p.m_y_offset = y_offset;
                p.m_visibility_directions = vis_dirs;
            }
            ShapeConnectionPin::update_position_and_visibility(router, pin);
            router.modify_connection_pin(pin);
        }
    }

    // upstream: shape.cpp ShapeRef::setCentrePos()
    pub fn set_centre_pos(self, router: &mut Router, new_centre: &Point) {
        // MISUSE: a handle of a freed object is a no-op (router.rs
        // module docs, "Public-API misuse").
        if !router.obstacles.contains(self.0) {
            return;
        }
        let diff = *new_centre - self.position(router);
        router.obstacles[self.0].m_polygon.translate(diff.x, diff.y);
    }

    // (upstream: shape.cpp ShapeRef::outputCode() — debug code output, not
    // ported.)
    // (upstream: ShapeRef::assignPinVisibilityTo() is declared but never
    // defined upstream.)
}

// upstream: shape.cpp absoluteOffsetInverse() (static)
fn absolute_offset_inverse(offset: f64, shape_box: &Box, to_dim: usize) -> f64 {
    if offset == ATTACH_POS_MIN_OFFSET {
        return ATTACH_POS_MAX_OFFSET;
    }

    if offset == ATTACH_POS_MAX_OFFSET {
        return ATTACH_POS_MIN_OFFSET;
    }

    shape_box.length(to_dim) - offset
}
