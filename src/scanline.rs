use std::collections::BTreeMap;

use crate::geometry::{Node, Point, Rect, XDIM, YDIM};
use crate::graph::VisibilityGraph;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScanDimension {
    X,
    Y,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[allow(dead_code)]
pub(crate) enum ScanEventType {
    Open = 1,
    SegOpen = 2,
    ConnPoint = 3,
    SegClose = 4,
    Close = 5,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub(crate) struct ScanEvent {
    pub(crate) event_type: ScanEventType,
    pub(crate) position: i64,
    pub(crate) object_index: usize,
}

impl ScanEvent {
    #[allow(dead_code)]
    pub(crate) fn new(event_type: ScanEventType, position: i64, object_index: usize) -> ScanEvent {
        ScanEvent {
            event_type,
            position,
            object_index,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ScanLine {
    pub(crate) axis: i64,
    pub(crate) vertices: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ScanObstacleLimits {
    pub(crate) lower: Option<f64>,
    pub(crate) upper: Option<f64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScanNodeKind {
    Obstacle,
    Point,
    ShiftSegment,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ScanNode {
    pub(crate) kind: ScanNodeKind,
    pub(crate) pos: f64,
    pub(crate) min: [f64; 2],
    pub(crate) max: [f64; 2],
    pub(crate) first_above: Option<usize>,
    pub(crate) first_below: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ScanPointLimits {
    pub(crate) first_above: f64,
    pub(crate) first_below: f64,
    pub(crate) last_above: f64,
    pub(crate) last_below: f64,
}

#[allow(dead_code)]
impl ScanNode {
    pub(crate) fn obstacle(rect: Rect, pos: f64) -> ScanNode {
        ScanNode {
            kind: ScanNodeKind::Obstacle,
            pos,
            min: [rect.left, rect.top],
            max: [rect.right, rect.bottom],
            first_above: None,
            first_below: None,
        }
    }

    pub(crate) fn point(point: Point, pos: f64) -> ScanNode {
        ScanNode {
            kind: ScanNodeKind::Point,
            pos,
            min: [point.x, point.y],
            max: [point.x, point.y],
            first_above: None,
            first_below: None,
        }
    }

    pub(crate) fn shift_segment(pos: f64) -> ScanNode {
        ScanNode {
            kind: ScanNodeKind::ShiftSegment,
            pos,
            min: [0.0, 0.0],
            max: [0.0, 0.0],
            first_above: None,
            first_below: None,
        }
    }

    pub(crate) fn first_obstacle_above(&self, nodes: &[ScanNode], dim: usize) -> f64 {
        let mut current = self.first_above;
        while let Some(index) = current {
            let node = &nodes[index];
            if node.kind != ScanNodeKind::ShiftSegment && node.max[dim] <= self.pos {
                return node.max[dim];
            }
            current = node.first_above;
        }
        -f64::MAX
    }

    pub(crate) fn first_obstacle_below(&self, nodes: &[ScanNode], dim: usize) -> f64 {
        let mut current = self.first_below;
        while let Some(index) = current {
            let node = &nodes[index];
            if node.kind != ScanNodeKind::ShiftSegment && node.min[dim] >= self.pos {
                return node.min[dim];
            }
            current = node.first_below;
        }
        f64::MAX
    }

    pub(crate) fn first_point_above(&self, nodes: &[ScanNode], dim: usize) -> f64 {
        let alt_dim = alternate_dimension(dim);
        let mut result = -f64::MAX;
        let mut current = self.first_above;
        while let Some(index) = current {
            let node = &nodes[index];
            let inline_with_edge =
                self.min[alt_dim] == node.min[alt_dim] || self.min[alt_dim] == node.max[alt_dim];
            if !inline_with_edge && node.max[dim] <= self.pos {
                result = result.max(node.max[dim]);
            }
            current = node.first_above;
        }
        result
    }

    pub(crate) fn first_point_below(&self, nodes: &[ScanNode], dim: usize) -> f64 {
        let alt_dim = alternate_dimension(dim);
        let mut result = f64::MAX;
        let mut current = self.first_below;
        while let Some(index) = current {
            let node = &nodes[index];
            let inline_with_edge =
                self.min[alt_dim] == node.min[alt_dim] || self.min[alt_dim] == node.max[alt_dim];
            if !inline_with_edge && node.min[dim] >= self.pos {
                result = result.min(node.min[dim]);
            }
            current = node.first_below;
        }
        result
    }

    pub(crate) fn is_inside_shape(&self, nodes: &[ScanNode], dimension: usize) -> bool {
        let mut current = self.first_below;
        while let Some(index) = current {
            let node = &nodes[index];
            if node.min[dimension] < self.pos && self.pos < node.max[dimension] {
                return true;
            }
            current = node.first_below;
        }
        current = self.first_above;
        while let Some(index) = current {
            let node = &nodes[index];
            if node.min[dimension] < self.pos && self.pos < node.max[dimension] {
                return true;
            }
            current = node.first_above;
        }
        false
    }

    pub(crate) fn find_first_point_above_and_below(
        &self,
        nodes: &[ScanNode],
        dim: usize,
        line_pos: f64,
    ) -> ScanPointLimits {
        let alt_dim = alternate_dimension(dim);
        let mut limits = ScanPointLimits {
            first_above: -f64::MAX,
            first_below: f64::MAX,
            last_above: self.max[dim],
            last_below: self.min[dim],
        };

        for above in [true, false] {
            let mut current = if above {
                self.first_above
            } else {
                self.first_below
            };
            while let Some(index) = current {
                let node = &nodes[index];
                let events_at_same_pos = (line_pos == self.max[alt_dim]
                    && line_pos == node.max[alt_dim])
                    || (line_pos == self.min[alt_dim] && line_pos == node.min[alt_dim]);

                if node.max[dim] <= self.min[dim] {
                    limits.first_above = limits.first_above.max(node.max[dim]);
                } else if node.min[dim] >= self.max[dim] {
                    limits.first_below = limits.first_below.min(node.min[dim]);
                } else if !events_at_same_pos {
                    limits.last_above = limits.last_above.min(node.min[dim]);
                    limits.last_below = limits.last_below.max(node.max[dim]);
                }

                current = if above {
                    node.first_above
                } else {
                    node.first_below
                };
            }
        }

        limits
    }
}

fn alternate_dimension(dim: usize) -> usize {
    match dim {
        XDIM => YDIM,
        YDIM => XDIM,
        _ => (dim + 1) % 2,
    }
}

#[allow(dead_code)]
pub(crate) fn order_scan_events(events: &mut [ScanEvent]) {
    events.sort_by_key(|event| (event.position, event.event_type, event.object_index));
}

#[allow(dead_code)]
pub(crate) fn nearest_obstacle_limits(
    obstacles: &[Rect],
    dimension: ScanDimension,
    line_position: f64,
    point_position: f64,
) -> ScanObstacleLimits {
    let mut lower = None;
    let mut upper = None;

    for obstacle in obstacles {
        let (line_min, line_max, scan_min, scan_max) = match dimension {
            ScanDimension::X => (obstacle.top, obstacle.bottom, obstacle.left, obstacle.right),
            ScanDimension::Y => (obstacle.left, obstacle.right, obstacle.top, obstacle.bottom),
        };
        if line_position <= line_min || line_position >= line_max {
            continue;
        }
        if scan_max <= point_position {
            lower = Some(lower.map_or(scan_max, |current: f64| current.max(scan_max)));
        } else if scan_min >= point_position {
            upper = Some(upper.map_or(scan_min, |current: f64| current.min(scan_min)));
        } else {
            lower = Some(lower.map_or(scan_min, |current: f64| current.max(scan_min)));
            upper = Some(upper.map_or(scan_max, |current: f64| current.min(scan_max)));
        }
    }

    ScanObstacleLimits { lower, upper }
}

pub(crate) fn ordered_axis_lines(
    graph: &VisibilityGraph,
    dimension: ScanDimension,
) -> Vec<ScanLine> {
    let mut lines: BTreeMap<i64, Vec<(i64, usize)>> = BTreeMap::new();

    for (index, vertex) in graph.vertices.iter() {
        let node = Node::from_point(vertex.point);
        let (axis, order) = match dimension {
            ScanDimension::X => (node.x, node.y),
            ScanDimension::Y => (node.y, node.x),
        };
        lines.entry(axis).or_default().push((order, index));
    }

    lines
        .into_iter()
        .map(|(axis, mut vertices)| {
            vertices.sort_by_key(|(order, index)| (*order, *index));
            ScanLine {
                axis,
                vertices: vertices.into_iter().map(|(_, index)| index).collect(),
            }
        })
        .collect()
}
