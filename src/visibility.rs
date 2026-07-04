use crate::constants::EPS;
use crate::geometry::{Point, Polygon};
use crate::graph::VisibilityGraph;
use crate::orthogonal::{clear_segment, Direction, OrthogonalObstacle};
use crate::vertices::{VertId, PROP_CONN_POINT};

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub(crate) struct StaticVisibilityGraph {
    graph: VisibilityGraph,
    shapes: Vec<Polygon>,
    connection_points: Vec<Point>,
    dirty: bool,
}

#[allow(dead_code)]
impl StaticVisibilityGraph {
    pub(crate) fn new(
        shapes: Vec<Polygon>,
        connection_points: Vec<Point>,
    ) -> StaticVisibilityGraph {
        StaticVisibilityGraph {
            graph: VisibilityGraph::default(),
            shapes,
            connection_points,
            dirty: true,
        }
    }

    pub(crate) fn graph(&mut self) -> &VisibilityGraph {
        self.regenerate_if_dirty();
        &self.graph
    }

    pub(crate) fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub(crate) fn set_shape(&mut self, index: usize, shape: Polygon) -> bool {
        let Some(slot) = self.shapes.get_mut(index) else {
            return false;
        };
        *slot = shape;
        self.dirty = true;
        true
    }

    pub(crate) fn add_shape(&mut self, shape: Polygon) {
        self.shapes.push(shape);
        self.dirty = true;
    }

    pub(crate) fn remove_shape(&mut self, index: usize) -> bool {
        if index >= self.shapes.len() {
            return false;
        }
        self.shapes.remove(index);
        self.dirty = true;
        true
    }

    fn regenerate_if_dirty(&mut self) {
        if !self.dirty {
            return;
        }
        self.graph = build_static_visibility_graph(&self.shapes, &self.connection_points);
        self.dirty = false;
    }
}

#[allow(dead_code)]
pub(crate) fn build_static_visibility_graph(
    shapes: &[Polygon],
    connection_points: &[Point],
) -> VisibilityGraph {
    let mut graph = VisibilityGraph::default();
    let mut vertices = Vec::new();

    for (index, point) in connection_points.iter().copied().enumerate() {
        let vertex = graph.add_vertex(VertId::new((index + 1) as u32, 0, PROP_CONN_POINT), point);
        for (shape_index, shape) in shapes.iter().enumerate() {
            if shape.contains_point(point, true) {
                graph.set_contains(vertex, (shape_index + 1) as u32, true);
            }
        }
        vertices.push(vertex);
    }

    for (shape_index, shape) in shapes.iter().enumerate() {
        for (point_index, point) in shape.points.iter().copied().enumerate() {
            vertices.push(graph.add_vertex(
                VertId::new((shape_index + 1) as u32, point_index as u16, 0),
                point,
            ));
        }
    }

    for left in 0..vertices.len() {
        for right in (left + 1)..vertices.len() {
            let from = vertices[left];
            let to = vertices[right];
            let Some((a, b)) = graph.edge_points(from, to) else {
                continue;
            };
            graph.add_visibility_edge(from, to);
            if let Some(blocker) = first_visibility_blocker(a, b, shapes) {
                graph.hide_edge(from, to, blocker);
            }
        }
    }

    graph
}

pub(crate) fn add_axis_visibility_edges(
    graph: &mut VisibilityGraph,
    line: &[usize],
    direction: Direction,
    obstacles: &[OrthogonalObstacle],
) {
    for pair in line.windows(2) {
        let a = pair[0];
        let b = pair[1];
        let Some((pa, pb)) = graph.edge_points(a, b) else {
            continue;
        };
        if same_axis(pa, pb) == Some(direction) && clear_segment(pa, pb, obstacles) {
            graph.add_orthogonal_edge(a, b, direction);
        }
    }
}

pub(crate) fn same_axis(a: Point, b: Point) -> Option<Direction> {
    if (a.y - b.y).abs() < EPS {
        Some(Direction::Horizontal)
    } else if (a.x - b.x).abs() < EPS {
        Some(Direction::Vertical)
    } else {
        None
    }
}

fn first_visibility_blocker(a: Point, b: Point, shapes: &[Polygon]) -> Option<i32> {
    for (shape_index, shape) in shapes.iter().enumerate() {
        if segment_is_polygon_boundary(a, b, shape) {
            continue;
        }
        if shape.contains_point(a, false) || shape.contains_point(b, false) {
            continue;
        }
        if shape.intersects_segment(a, b) {
            return Some((shape_index + 1) as i32);
        }
    }
    None
}

fn segment_is_polygon_boundary(a: Point, b: Point, polygon: &Polygon) -> bool {
    polygon
        .edges()
        .any(|(start, end)| (start == a && end == b) || (start == b && end == a))
}
