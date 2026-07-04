use crate::constants::{
    ATTACH_POS_BOTTOM, ATTACH_POS_LEFT, ATTACH_POS_MAX_OFFSET, ATTACH_POS_MIN_OFFSET,
    ATTACH_POS_RIGHT, ATTACH_POS_TOP, CONN_DIR_ALL, CONN_DIR_DOWN, CONN_DIR_LEFT, CONN_DIR_NONE,
    CONN_DIR_RIGHT, CONN_DIR_UP, EPS, TRANSFORMATION_TYPE_CW180, TRANSFORMATION_TYPE_CW270,
    TRANSFORMATION_TYPE_CW90, TRANSFORMATION_TYPE_FLIP_X, TRANSFORMATION_TYPE_FLIP_Y,
};
use crate::geometry::{Point, Polygon, Rect};

#[derive(Clone, Debug)]
pub(crate) struct Shape {
    pub(crate) id: String,
    pub(crate) polygon: Polygon,
    pub(crate) rect: Rect,
    pub(crate) pins: Vec<ShapeConnectionPin>,
}

impl Shape {
    pub(crate) fn from_rect(id: String, rect: Rect) -> Shape {
        Shape {
            id,
            polygon: Polygon::rectangle(rect.left, rect.top, rect.right, rect.bottom),
            rect,
            pins: Vec::new(),
        }
    }

    pub(crate) fn from_polygon(id: String, polygon: Polygon) -> Option<Shape> {
        let rect = polygon.bounding_box()?.to_rect();
        Some(Shape {
            id,
            polygon,
            rect,
            pins: Vec::new(),
        })
    }

    pub(crate) fn set_rect(&mut self, rect: Rect) {
        self.polygon = rect.to_polygon();
        self.rect = rect;
    }

    pub(crate) fn set_polygon(&mut self, polygon: Polygon) -> bool {
        let Some(rect) = polygon.bounding_box().map(|bbox| bbox.to_rect()) else {
            return false;
        };
        self.polygon = polygon;
        self.rect = rect;
        true
    }

    pub(crate) fn translate(&mut self, x_diff: f64, y_diff: f64) {
        self.polygon.translate(x_diff, y_diff);
        self.rect = self.rect.translate(x_diff, y_diff);
    }

    pub(crate) fn transform_connection_pin_positions(&mut self, transform: u32) -> bool {
        if !matches!(
            transform,
            TRANSFORMATION_TYPE_CW90
                | TRANSFORMATION_TYPE_CW180
                | TRANSFORMATION_TYPE_CW270
                | TRANSFORMATION_TYPE_FLIP_X
                | TRANSFORMATION_TYPE_FLIP_Y
        ) {
            return false;
        }
        for pin in &mut self.pins {
            pin.transform_position(transform, self.rect);
        }
        true
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ShapeConnectionPin {
    pub(crate) id: String,
    pub(crate) class_id: u32,
    pub(crate) offset: Point,
    pub(crate) proportional: bool,
    pub(crate) inside_offset: f64,
    pub(crate) visibility_directions: u32,
    pub(crate) exclusive: bool,
    pub(crate) connection_cost: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct Junction {
    pub(crate) id: String,
    pub(crate) position: Point,
    pub(crate) recommended_position: Point,
    pub(crate) fixed: bool,
    pub(crate) preferred_orthogonal_dimension: Option<usize>,
    pub(crate) pins: Vec<ShapeConnectionPin>,
}

#[derive(Clone, Debug)]
pub(crate) struct Cluster {
    pub(crate) id: String,
    pub(crate) polygon: Polygon,
    pub(crate) rect: Rect,
}

#[derive(Clone, Debug)]
pub(crate) struct Connector {
    pub(crate) id: String,
    pub(crate) src: Endpoint,
    pub(crate) dst: Endpoint,
    pub(crate) routing_type: u32,
    pub(crate) hate_crossings: bool,
    pub(crate) checkpoints: Vec<Checkpoint>,
    pub(crate) fixed_route: Option<Vec<Point>>,
    pub(crate) display_route: Vec<Point>,
    pub(crate) needs_reroute: bool,
    pub(crate) needs_repaint: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct Checkpoint {
    pub(crate) point: Point,
    pub(crate) arrival_directions: u32,
    pub(crate) departure_directions: u32,
}

#[derive(Clone, Debug)]
pub(crate) enum Endpoint {
    Point {
        point: Point,
        directions: u32,
    },
    ShapePin {
        shape_id: String,
        pin_id: String,
        pin_class_id: Option<u32>,
    },
    ShapePinClass {
        shape_id: String,
        pin_class_id: u32,
    },
    Junction {
        junction_id: String,
    },
    JunctionPin {
        junction_id: String,
        pin_id: String,
        pin_class_id: Option<u32>,
    },
    JunctionPinClass {
        junction_id: String,
        pin_class_id: u32,
    },
}

impl Cluster {
    pub(crate) fn from_polygon(id: String, polygon: Polygon) -> Option<Cluster> {
        let rect = polygon.bounding_box()?.to_rect();
        Some(Cluster { id, polygon, rect })
    }

    pub(crate) fn set_polygon(&mut self, polygon: Polygon) -> bool {
        let Some(rect) = polygon.bounding_box().map(|bbox| bbox.to_rect()) else {
            return false;
        };
        self.polygon = polygon;
        self.rect = rect;
        true
    }
}

impl ShapeConnectionPin {
    pub(crate) fn resolve_position(&self, rect: Rect) -> Point {
        let mut point = if self.proportional {
            Point {
                x: rect.left + (rect.right - rect.left) * self.offset.x,
                y: rect.top + (rect.bottom - rect.top) * self.offset.y,
            }
        } else {
            Point {
                x: rect.left + absolute_offset(self.offset.x, rect.right - rect.left),
                y: rect.top + absolute_offset(self.offset.y, rect.bottom - rect.top),
            }
        };

        if self.inside_offset > 0.0 {
            if (point.x - rect.left).abs() < EPS {
                point.x += self.inside_offset;
            } else if (point.x - rect.right).abs() < EPS {
                point.x -= self.inside_offset;
            }
            if (point.y - rect.top).abs() < EPS {
                point.y += self.inside_offset;
            } else if (point.y - rect.bottom).abs() < EPS {
                point.y -= self.inside_offset;
            }
        }

        point
    }

    fn transform_position(&mut self, transform: u32, rect: Rect) {
        let mut rotation_n = 0;
        if self.proportional {
            self.offset.x -= 0.5;
            self.offset.y -= 0.5;
            match transform {
                TRANSFORMATION_TYPE_CW90 => {
                    rotation_n = 3;
                    let old_y = self.offset.y;
                    self.offset.y = self.offset.x;
                    self.offset.x = -old_y;
                }
                TRANSFORMATION_TYPE_CW180 => {
                    rotation_n = 2;
                    self.offset.x = -self.offset.x;
                    self.offset.y = -self.offset.y;
                }
                TRANSFORMATION_TYPE_CW270 => {
                    rotation_n = 1;
                    let old_y = self.offset.y;
                    self.offset.y = -self.offset.x;
                    self.offset.x = old_y;
                }
                TRANSFORMATION_TYPE_FLIP_X => {
                    self.offset.x = -self.offset.x;
                }
                TRANSFORMATION_TYPE_FLIP_Y => {
                    self.offset.y = -self.offset.y;
                }
                _ => {}
            }
            self.offset.x += 0.5;
            self.offset.y += 0.5;
        } else {
            match transform {
                TRANSFORMATION_TYPE_CW90 => {
                    rotation_n = 3;
                    let old_y = self.offset.y;
                    self.offset.y = self.offset.x;
                    self.offset.x = absolute_offset_inverse(old_y, rect.right - rect.left);
                }
                TRANSFORMATION_TYPE_CW180 => {
                    rotation_n = 2;
                    self.offset.x = absolute_offset_inverse(self.offset.x, rect.right - rect.left);
                    self.offset.y = absolute_offset_inverse(self.offset.y, rect.bottom - rect.top);
                }
                TRANSFORMATION_TYPE_CW270 => {
                    rotation_n = 1;
                    let old_y = self.offset.y;
                    self.offset.y = absolute_offset_inverse(self.offset.x, rect.bottom - rect.top);
                    self.offset.x = old_y;
                }
                TRANSFORMATION_TYPE_FLIP_X => {
                    self.offset.x = absolute_offset_inverse(self.offset.x, rect.right - rect.left);
                }
                TRANSFORMATION_TYPE_FLIP_Y => {
                    self.offset.y = absolute_offset_inverse(self.offset.y, rect.bottom - rect.top);
                }
                _ => {}
            }
        }
        self.visibility_directions =
            transform_directions(self.visibility_directions, transform, rotation_n);
    }
}

impl Connector {
    pub(crate) fn new(id: String, src: Endpoint, dst: Endpoint, routing_type: u32) -> Connector {
        Connector {
            id,
            src,
            dst,
            routing_type,
            hate_crossings: false,
            checkpoints: Vec::new(),
            fixed_route: None,
            display_route: Vec::new(),
            needs_reroute: true,
            needs_repaint: false,
        }
    }

    pub(crate) fn make_path_invalid(&mut self) {
        self.display_route.clear();
        self.needs_reroute = true;
    }

    pub(crate) fn set_display_route(&mut self, route: Vec<Point>) {
        self.display_route = route;
        self.needs_reroute = false;
        self.needs_repaint = true;
    }

    pub(crate) fn references_shape(&self, shape_id: &str) -> bool {
        self.src.references_shape(shape_id) || self.dst.references_shape(shape_id)
    }

    pub(crate) fn references_pin(&self, shape_id: &str, pin_id: &str) -> bool {
        self.src.references_pin(shape_id, pin_id) || self.dst.references_pin(shape_id, pin_id)
    }

    pub(crate) fn endpoint_shape_ids(&self) -> Vec<&String> {
        let mut ids = Vec::new();
        self.src.push_shape_id(&mut ids);
        self.dst.push_shape_id(&mut ids);
        ids
    }

    pub(crate) fn references_junction(&self, junction_id: &str) -> bool {
        self.src.references_junction(junction_id) || self.dst.references_junction(junction_id)
    }

    pub(crate) fn references_junction_pin(&self, junction_id: &str, pin_id: &str) -> bool {
        self.src.references_junction_pin(junction_id, pin_id)
            || self.dst.references_junction_pin(junction_id, pin_id)
    }

    pub(crate) fn endpoint_junction_ids(&self) -> Vec<&String> {
        let mut ids = Vec::new();
        self.src.push_junction_id(&mut ids);
        self.dst.push_junction_id(&mut ids);
        ids
    }
}

impl Endpoint {
    pub(crate) fn shape_id(&self) -> Option<&str> {
        match self {
            Endpoint::ShapePin { shape_id, .. } | Endpoint::ShapePinClass { shape_id, .. } => {
                Some(shape_id)
            }
            Endpoint::Point { .. }
            | Endpoint::Junction { .. }
            | Endpoint::JunctionPin { .. }
            | Endpoint::JunctionPinClass { .. } => None,
        }
    }

    pub(crate) fn references_shape(&self, candidate: &str) -> bool {
        match self {
            Endpoint::Point { .. } => false,
            Endpoint::ShapePin { shape_id, .. } | Endpoint::ShapePinClass { shape_id, .. } => {
                shape_id == candidate
            }
            Endpoint::Junction { .. }
            | Endpoint::JunctionPin { .. }
            | Endpoint::JunctionPinClass { .. } => false,
        }
    }

    pub(crate) fn references_pin(&self, candidate_shape: &str, candidate_pin: &str) -> bool {
        match self {
            Endpoint::Point { .. } => false,
            Endpoint::ShapePin {
                shape_id, pin_id, ..
            } => shape_id == candidate_shape && pin_id == candidate_pin,
            Endpoint::ShapePinClass { .. }
            | Endpoint::Junction { .. }
            | Endpoint::JunctionPin { .. }
            | Endpoint::JunctionPinClass { .. } => false,
        }
    }

    pub(crate) fn references_junction(&self, candidate: &str) -> bool {
        match self {
            Endpoint::Junction { junction_id }
            | Endpoint::JunctionPin { junction_id, .. }
            | Endpoint::JunctionPinClass { junction_id, .. } => junction_id == candidate,
            Endpoint::Point { .. } | Endpoint::ShapePin { .. } | Endpoint::ShapePinClass { .. } => {
                false
            }
        }
    }

    pub(crate) fn references_junction_pin(
        &self,
        candidate_junction: &str,
        candidate_pin: &str,
    ) -> bool {
        match self {
            Endpoint::JunctionPin {
                junction_id,
                pin_id,
                ..
            } => junction_id == candidate_junction && pin_id == candidate_pin,
            Endpoint::Point { .. }
            | Endpoint::ShapePin { .. }
            | Endpoint::ShapePinClass { .. }
            | Endpoint::Junction { .. }
            | Endpoint::JunctionPinClass { .. } => false,
        }
    }

    fn push_shape_id<'a>(&'a self, ids: &mut Vec<&'a String>) {
        if let Endpoint::ShapePin { shape_id, .. } | Endpoint::ShapePinClass { shape_id, .. } = self
        {
            if !ids.iter().any(|id| *id == shape_id) {
                ids.push(shape_id);
            }
        }
    }

    fn push_junction_id<'a>(&'a self, ids: &mut Vec<&'a String>) {
        if let Endpoint::Junction { junction_id }
        | Endpoint::JunctionPin { junction_id, .. }
        | Endpoint::JunctionPinClass { junction_id, .. } = self
        {
            if !ids.iter().any(|id| *id == junction_id) {
                ids.push(junction_id);
            }
        }
    }
}

pub(crate) fn absolute_offset(offset: f64, size: f64) -> f64 {
    if (offset - ATTACH_POS_MAX_OFFSET).abs() < EPS {
        size
    } else {
        offset
    }
}

fn absolute_offset_inverse(offset: f64, size: f64) -> f64 {
    if (offset - ATTACH_POS_MIN_OFFSET).abs() < EPS {
        ATTACH_POS_MAX_OFFSET
    } else if (offset - ATTACH_POS_MAX_OFFSET).abs() < EPS {
        ATTACH_POS_MIN_OFFSET
    } else {
        size - offset
    }
}

fn transform_directions(directions: u32, transform: u32, rotation_n: usize) -> u32 {
    if directions & CONN_DIR_ALL == 0 || directions == CONN_DIR_ALL {
        return directions;
    }

    let mut visible = [
        directions & CONN_DIR_UP != 0,
        directions & CONN_DIR_RIGHT != 0,
        directions & CONN_DIR_DOWN != 0,
        directions & CONN_DIR_LEFT != 0,
    ];
    if transform == TRANSFORMATION_TYPE_FLIP_Y {
        visible.swap(0, 2);
    } else if transform == TRANSFORMATION_TYPE_FLIP_X {
        visible.swap(1, 3);
    }

    let mut transformed = CONN_DIR_NONE;
    if visible[(rotation_n) % 4] {
        transformed |= CONN_DIR_UP;
    }
    if visible[(rotation_n + 1) % 4] {
        transformed |= CONN_DIR_RIGHT;
    }
    if visible[(rotation_n + 2) % 4] {
        transformed |= CONN_DIR_DOWN;
    }
    if visible[(rotation_n + 3) % 4] {
        transformed |= CONN_DIR_LEFT;
    }
    transformed
}

pub(crate) fn pin_visibility(x_offset: f64, y_offset: f64, requested: u32) -> u32 {
    if requested != CONN_DIR_NONE {
        return requested;
    }

    let mut directions = CONN_DIR_NONE;
    if (y_offset - ATTACH_POS_TOP).abs() < EPS {
        directions |= CONN_DIR_UP;
    }
    if (y_offset - ATTACH_POS_BOTTOM).abs() < EPS {
        directions |= CONN_DIR_DOWN;
    }
    if (x_offset - ATTACH_POS_LEFT).abs() < EPS {
        directions |= CONN_DIR_LEFT;
    }
    if (x_offset - ATTACH_POS_RIGHT).abs() < EPS {
        directions |= CONN_DIR_RIGHT;
    }
    if directions == CONN_DIR_NONE {
        CONN_DIR_ALL
    } else {
        directions
    }
}
