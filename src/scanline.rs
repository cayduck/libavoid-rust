use std::collections::BTreeMap;

use crate::geometry::{Node, Rect};
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
