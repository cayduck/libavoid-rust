use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use crate::constants::{
    CONN_DIR_ALL, CONN_DIR_DOWN, CONN_DIR_LEFT, CONN_DIR_NONE, CONN_DIR_RIGHT, CONN_DIR_UP, EPS,
};
use crate::geometry::{point_on_line, segment_intersect, vec_dir, Node, Point, Polygon, Rect};
use crate::graph::VisibilityGraph;
use crate::scanline::{ordered_axis_lines, ScanDimension};
use crate::vertices::{VertId, PROP_CONN_POINT, PROP_ORTH_SHAPE_EDGE};
use crate::visibility::add_axis_visibility_edges;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Direction {
    Horizontal,
    Vertical,
    None,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum StepDirection {
    Right,
    Left,
    Up,
    Down,
    None,
}

impl StepDirection {
    fn opposite(self) -> StepDirection {
        match self {
            StepDirection::Right => StepDirection::Left,
            StepDirection::Left => StepDirection::Right,
            StepDirection::Up => StepDirection::Down,
            StepDirection::Down => StepDirection::Up,
            StepDirection::None => StepDirection::None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct QueueState {
    cost: f64,
    node: usize,
    direction: StepDirection,
    timestamp: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct OrthogonalObstacle {
    pub(crate) polygon: Polygon,
    pub(crate) bounds: Rect,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RoutingCostSettings {
    pub(crate) segment_penalty: f64,
    pub(crate) angle_penalty: f64,
    pub(crate) crossing_penalty: f64,
    pub(crate) fixed_shared_path_penalty: f64,
    pub(crate) reverse_direction_penalty: f64,
}

pub(crate) fn route_orthogonal_with_directions(
    src: Point,
    dst: Point,
    obstacles: &[OrthogonalObstacle],
    crossing_routes: &[Vec<Point>],
    clearance: f64,
    src_directions: u32,
    dst_directions: u32,
    cost_settings: RoutingCostSettings,
) -> Option<Vec<Point>> {
    let mut xs = vec![src.x, dst.x];
    let mut ys = vec![src.y, dst.y];
    let offset = clearance.max(1.0);

    for obstacle in obstacles {
        let bounds = obstacle.bounds;
        xs.extend([
            bounds.left - offset,
            bounds.left,
            bounds.right,
            bounds.right + offset,
        ]);
        ys.extend([
            bounds.top - offset,
            bounds.top,
            bounds.bottom,
            bounds.bottom + offset,
        ]);
    }
    for route in crossing_routes {
        for point in route {
            xs.push(point.x);
            ys.push(point.y);
        }
    }
    let endpoint_visibility_points = endpoint_visibility_points(
        src,
        normalized_directions(src_directions),
        dst,
        normalized_directions(dst_directions),
        obstacles,
    );
    for point in &endpoint_visibility_points {
        xs.push(point.x);
        ys.push(point.y);
    }

    sort_dedup(&mut xs);
    sort_dedup(&mut ys);

    let src_node = Node::from_point(src);
    let dst_node = Node::from_point(dst);
    let mut graph = VisibilityGraph::default();
    let mut node_to_vertex = HashMap::new();
    for &x in &xs {
        for &y in &ys {
            let point = Point { x, y };
            if point == src
                || point == dst
                || !obstacles
                    .iter()
                    .any(|obstacle| obstacle.polygon.contains_point(point, false))
            {
                let node = Node::from_point(point);
                let vertex = graph.add_vertex(VertId::new(0, 0, PROP_ORTH_SHAPE_EDGE), point);
                node_to_vertex.insert(node, vertex);
            }
        }
    }

    let src_vertex = *node_to_vertex
        .entry(src_node)
        .or_insert_with(|| graph.add_vertex(VertId::new(0, 0, PROP_CONN_POINT), src));
    let dst_vertex = *node_to_vertex
        .entry(dst_node)
        .or_insert_with(|| graph.add_vertex(VertId::new(0, 1, PROP_CONN_POINT), dst));

    for row in ordered_axis_lines(&graph, ScanDimension::Y) {
        debug_assert!(line_matches_axis(
            &graph,
            &row.vertices,
            row.axis,
            ScanDimension::Y
        ));
        add_axis_visibility_edges(&mut graph, &row.vertices, Direction::Horizontal, obstacles);
    }
    for column in ordered_axis_lines(&graph, ScanDimension::X) {
        debug_assert!(line_matches_axis(
            &graph,
            &column.vertices,
            column.axis,
            ScanDimension::X
        ));
        add_axis_visibility_edges(&mut graph, &column.vertices, Direction::Vertical, obstacles);
    }
    add_endpoint_visibility_edges(
        &mut graph,
        &node_to_vertex,
        src,
        normalized_directions(src_directions),
        dst,
        normalized_directions(dst_directions),
        obstacles,
    );

    shortest_path(
        src_vertex,
        dst_vertex,
        &graph,
        normalized_directions(src_directions),
        normalized_directions(dst_directions),
        crossing_routes,
        cost_settings,
    )
    .map(simplify_path)
}

fn endpoint_visibility_points(
    src: Point,
    src_directions: u32,
    dst: Point,
    dst_directions: u32,
    obstacles: &[OrthogonalObstacle],
) -> Vec<Point> {
    let mut points = Vec::new();
    points.extend(endpoint_boundary_points(src, src_directions, obstacles));
    points.extend(endpoint_boundary_points(dst, dst_directions, obstacles));
    points
}

fn add_endpoint_visibility_edges(
    graph: &mut VisibilityGraph,
    node_to_vertex: &HashMap<Node, usize>,
    src: Point,
    src_directions: u32,
    dst: Point,
    dst_directions: u32,
    obstacles: &[OrthogonalObstacle],
) {
    add_endpoint_visibility_edges_for_point(graph, node_to_vertex, src, src_directions, obstacles);
    add_endpoint_visibility_edges_for_point(graph, node_to_vertex, dst, dst_directions, obstacles);
}

fn add_endpoint_visibility_edges_for_point(
    graph: &mut VisibilityGraph,
    node_to_vertex: &HashMap<Node, usize>,
    endpoint: Point,
    directions: u32,
    obstacles: &[OrthogonalObstacle],
) {
    let Some(&endpoint_vertex) = node_to_vertex.get(&Node::from_point(endpoint)) else {
        return;
    };
    for boundary in endpoint_boundary_points(endpoint, directions, obstacles) {
        let Some(&boundary_vertex) = node_to_vertex.get(&Node::from_point(boundary)) else {
            continue;
        };
        let direction = if (endpoint.y - boundary.y).abs() < EPS {
            Direction::Horizontal
        } else {
            Direction::Vertical
        };
        graph.add_orthogonal_edge(endpoint_vertex, boundary_vertex, direction);
    }
}

fn endpoint_boundary_points(
    endpoint: Point,
    directions: u32,
    obstacles: &[OrthogonalObstacle],
) -> Vec<Point> {
    let mut points = Vec::new();
    if directions & CONN_DIR_LEFT != 0 {
        if let Some(point) = nearest_boundary_point(endpoint, StepDirection::Left, obstacles) {
            points.push(point);
        }
    }
    if directions & CONN_DIR_RIGHT != 0 {
        if let Some(point) = nearest_boundary_point(endpoint, StepDirection::Right, obstacles) {
            points.push(point);
        }
    }
    if directions & CONN_DIR_UP != 0 {
        if let Some(point) = nearest_boundary_point(endpoint, StepDirection::Up, obstacles) {
            points.push(point);
        }
    }
    if directions & CONN_DIR_DOWN != 0 {
        if let Some(point) = nearest_boundary_point(endpoint, StepDirection::Down, obstacles) {
            points.push(point);
        }
    }
    points
}

fn nearest_boundary_point(
    endpoint: Point,
    direction: StepDirection,
    obstacles: &[OrthogonalObstacle],
) -> Option<Point> {
    let mut best = None;
    let mut best_distance = f64::INFINITY;
    for obstacle in obstacles {
        if !obstacle.polygon.contains_point(endpoint, false) {
            continue;
        }
        for (a, b) in obstacle.polygon.edges() {
            if let Some(candidate) = axis_ray_edge_intersection(endpoint, direction, a, b) {
                let distance = (candidate.x - endpoint.x).abs() + (candidate.y - endpoint.y).abs();
                if distance > EPS && distance + EPS < best_distance {
                    best = Some(candidate);
                    best_distance = distance;
                }
            }
        }
    }
    best
}

fn axis_ray_edge_intersection(
    endpoint: Point,
    direction: StepDirection,
    a: Point,
    b: Point,
) -> Option<Point> {
    match direction {
        StepDirection::Left | StepDirection::Right => {
            if (a.x - b.x).abs() >= EPS {
                return None;
            }
            if endpoint.y + EPS < a.y.min(b.y) || endpoint.y - EPS > a.y.max(b.y) {
                return None;
            }
            let distance = a.x - endpoint.x;
            if (direction == StepDirection::Left && distance < -EPS)
                || (direction == StepDirection::Right && distance > EPS)
            {
                Some(Point {
                    x: a.x,
                    y: endpoint.y,
                })
            } else {
                None
            }
        }
        StepDirection::Up | StepDirection::Down => {
            if (a.y - b.y).abs() >= EPS {
                return None;
            }
            if endpoint.x + EPS < a.x.min(b.x) || endpoint.x - EPS > a.x.max(b.x) {
                return None;
            }
            let distance = a.y - endpoint.y;
            if (direction == StepDirection::Down && distance < -EPS)
                || (direction == StepDirection::Up && distance > EPS)
            {
                Some(Point {
                    x: endpoint.x,
                    y: a.y,
                })
            } else {
                None
            }
        }
        StepDirection::None => None,
    }
}

fn shortest_path(
    src: usize,
    dst: usize,
    graph: &VisibilityGraph,
    src_directions: u32,
    dst_directions: u32,
    crossing_routes: &[Vec<Point>],
    cost_settings: RoutingCostSettings,
) -> Option<Vec<Point>> {
    let mut heap = BinaryHeap::new();
    let mut best: HashMap<(usize, StepDirection), f64> = HashMap::new();
    let mut previous: HashMap<(usize, StepDirection), (usize, StepDirection)> = HashMap::new();
    let src_point = graph.vertices.get(src)?.point;
    let dst_point = graph.vertices.get(dst)?.point;

    let mut timestamp = 1;
    heap.push(QueueState {
        cost: 0.0,
        node: src,
        direction: StepDirection::None,
        timestamp,
    });
    timestamp += 1;
    best.insert((src, StepDirection::None), 0.0);

    let mut final_state = None;
    while let Some(state) = heap.pop() {
        let key = (state.node, state.direction);
        if state.cost > *best.get(&key).unwrap_or(&f64::INFINITY) + EPS {
            continue;
        }
        if state.node == dst {
            final_state = Some(key);
            break;
        }
        for (next, edge) in ordered_orthogonal_neighbours(graph, state.node, state.direction) {
            if !edge.orthogonal {
                continue;
            }
            if !edge_satisfies_direction_constraints(
                state.node,
                next,
                src,
                dst,
                graph,
                src_directions,
                dst_directions,
            ) {
                continue;
            }
            let Some((from_point, to_point)) = graph.edge_points(state.node, next) else {
                continue;
            };
            let next_direction = step_direction(from_point, to_point);
            let next_cost = state.cost
                + orthogonal_transition_cost(
                    state.direction,
                    next_direction,
                    edge.dist,
                    from_point,
                    to_point,
                    src_point,
                    dst_point,
                    crossing_routes,
                    cost_settings,
                );
            let next_key = (next, next_direction);
            let next_timestamp = timestamp;
            timestamp += 1;
            if next_cost + EPS < *best.get(&next_key).unwrap_or(&f64::INFINITY) {
                best.insert(next_key, next_cost);
                previous.insert(next_key, key);
                heap.push(QueueState {
                    cost: next_cost,
                    node: next,
                    direction: next_direction,
                    timestamp: next_timestamp,
                });
            }
        }
    }

    let mut key = final_state?;
    let mut path = vec![graph.vertices.get(key.0)?.point];
    while key.0 != src {
        key = *previous.get(&key)?;
        path.push(graph.vertices.get(key.0)?.point);
    }
    path.reverse();
    Some(path)
}

fn ordered_orthogonal_neighbours(
    graph: &VisibilityGraph,
    vertex: usize,
    incoming_direction: StepDirection,
) -> Vec<(usize, &crate::graph::EdgeInf)> {
    let Some(common) = graph.vertices.get(vertex).map(|vertex| vertex.point) else {
        return Vec::new();
    };
    let last = previous_point_for_turn_order(common, incoming_direction);
    let mut neighbours = graph.neighbours(vertex).collect::<Vec<_>>();
    neighbours.sort_by_key(|(next, _)| {
        graph
            .vertices
            .get(*next)
            .map(|vertex| orthog_turn_order(last, common, vertex.point))
            .unwrap_or(4)
    });
    neighbours
}

fn previous_point_for_turn_order(common: Point, incoming_direction: StepDirection) -> Point {
    match incoming_direction {
        StepDirection::Right => Point {
            x: common.x - 10.0,
            y: common.y,
        },
        StepDirection::Left => Point {
            x: common.x + 10.0,
            y: common.y,
        },
        StepDirection::Up => Point {
            x: common.x,
            y: common.y - 10.0,
        },
        StepDirection::Down => Point {
            x: common.x,
            y: common.y + 10.0,
        },
        StepDirection::None => Point {
            x: common.x - 10.0,
            y: common.y,
        },
    }
}

fn orthog_turn_order(a: Point, b: Point, c: Point) -> i32 {
    if ((c.x - b.x).abs() >= EPS && (c.y - b.y).abs() >= EPS)
        || ((a.x - b.x).abs() >= EPS && (a.y - b.y).abs() >= EPS)
    {
        return 4;
    }

    match vec_dir(a, b, c, 0.0) {
        direction if direction > 0 => 1,
        direction if direction < 0 => 2,
        _ if b.x == c.x => {
            if ((a.y < b.y) && (c.y < b.y)) || ((a.y > b.y) && (c.y > b.y)) {
                0
            } else {
                3
            }
        }
        _ => {
            if ((a.x < b.x) && (c.x < b.x)) || ((a.x > b.x) && (c.x > b.x)) {
                0
            } else {
                3
            }
        }
    }
}

pub(crate) fn segment_satisfies_directions(
    src: Point,
    dst: Point,
    src_directions: u32,
    dst_directions: u32,
) -> bool {
    direction_allowed(
        direction_flag(src, dst),
        normalized_directions(src_directions),
    ) && direction_allowed(
        direction_flag(dst, src),
        normalized_directions(dst_directions),
    )
}

fn simplify_path(path: Vec<Point>) -> Vec<Point> {
    let mut simplified = Vec::new();
    for point in path {
        simplified.push(point);
        while simplified.len() >= 3 {
            let len = simplified.len();
            let a = simplified[len - 3];
            let b = simplified[len - 2];
            let c = simplified[len - 1];
            if ((a.x - b.x).abs() < EPS && (b.x - c.x).abs() < EPS)
                || ((a.y - b.y).abs() < EPS && (b.y - c.y).abs() < EPS)
            {
                simplified.remove(len - 2);
            } else {
                break;
            }
        }
    }
    simplified
}

pub(crate) fn clear_segment(a: Point, b: Point, obstacles: &[OrthogonalObstacle]) -> bool {
    if (a.x - b.x).abs() >= EPS && (a.y - b.y).abs() >= EPS {
        return false;
    }
    !obstacles
        .iter()
        .any(|obstacle| obstacle.polygon.intersects_axis_segment(a, b))
}

fn sort_dedup(values: &mut Vec<f64>) {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    values.dedup_by(|a, b| (*a - *b).abs() < EPS);
}

fn step_direction(from: Point, to: Point) -> StepDirection {
    if to.x - from.x > EPS {
        StepDirection::Right
    } else if from.x - to.x > EPS {
        StepDirection::Left
    } else if to.y - from.y > EPS {
        StepDirection::Up
    } else if from.y - to.y > EPS {
        StepDirection::Down
    } else {
        StepDirection::None
    }
}

fn orthogonal_transition_cost(
    previous_direction: StepDirection,
    next_direction: StepDirection,
    edge_distance: f64,
    from: Point,
    to: Point,
    src: Point,
    dst: Point,
    crossing_routes: &[Vec<Point>],
    cost_settings: RoutingCostSettings,
) -> f64 {
    let mut cost = edge_distance;
    if previous_direction != StepDirection::None {
        if previous_direction.opposite() == next_direction {
            cost += 2.0 * cost_settings.segment_penalty;
        } else if previous_direction != next_direction {
            cost += cost_settings.segment_penalty;
        }
    }
    cost += crossing_count(from, to, crossing_routes) as f64 * cost_settings.crossing_penalty;
    cost += shared_path_count(from, to, crossing_routes) as f64
        * cost_settings.fixed_shared_path_penalty;
    if segment_reverses_direction(from, to, src, dst) {
        cost += cost_settings.reverse_direction_penalty;
    }
    cost
}

fn edge_satisfies_direction_constraints(
    from: usize,
    to: usize,
    src: usize,
    dst: usize,
    graph: &VisibilityGraph,
    src_directions: u32,
    dst_directions: u32,
) -> bool {
    let Some((from_point, to_point)) = graph.edge_points(from, to) else {
        return false;
    };
    if from == src && !direction_allowed(direction_flag(from_point, to_point), src_directions) {
        return false;
    }
    if to == dst && !direction_allowed(direction_flag(to_point, from_point), dst_directions) {
        return false;
    }
    true
}

pub(crate) fn route_crossing_count(a: Point, b: Point, routes: &[Vec<Point>]) -> usize {
    crossing_count(a, b, routes)
}

pub(crate) fn route_shared_path_count(a: Point, b: Point, routes: &[Vec<Point>]) -> usize {
    shared_path_count(a, b, routes)
}

fn crossing_count(a: Point, b: Point, routes: &[Vec<Point>]) -> usize {
    routes
        .iter()
        .map(|route| {
            route
                .windows(2)
                .filter(|segment| {
                    route_segment_crosses_or_touches_interior(a, b, segment[0], segment[1])
                })
                .count()
        })
        .sum()
}

fn route_segment_crosses_or_touches_interior(a: Point, b: Point, c: Point, d: Point) -> bool {
    segment_intersect(a, b, c, d) || point_on_line(c, d, a, EPS) || point_on_line(c, d, b, EPS)
}

fn shared_path_count(a: Point, b: Point, routes: &[Vec<Point>]) -> usize {
    routes
        .iter()
        .map(|route| {
            route
                .windows(2)
                .filter(|segment| orthogonal_segments_overlap(a, b, segment[0], segment[1]))
                .count()
        })
        .sum()
}

fn orthogonal_segments_overlap(a1: Point, a2: Point, b1: Point, b2: Point) -> bool {
    if (a1.x - a2.x).abs() < EPS && (b1.x - b2.x).abs() < EPS && (a1.x - b1.x).abs() < EPS {
        return ranges_overlap_strict(a1.y, a2.y, b1.y, b2.y);
    }
    if (a1.y - a2.y).abs() < EPS && (b1.y - b2.y).abs() < EPS && (a1.y - b1.y).abs() < EPS {
        return ranges_overlap_strict(a1.x, a2.x, b1.x, b2.x);
    }
    false
}

fn ranges_overlap_strict(a1: f64, a2: f64, b1: f64, b2: f64) -> bool {
    let a_min = a1.min(a2);
    let a_max = a1.max(a2);
    let b_min = b1.min(b2);
    let b_max = b1.max(b2);
    a_min.max(b_min) + EPS < a_max.min(b_max)
}

pub(crate) fn segment_reverses_direction(from: Point, to: Point, src: Point, dst: Point) -> bool {
    let x_dir = dim_direction(dst.x - src.x);
    let y_dir = dim_direction(dst.y - src.y);
    ((x_dir != 0) && (-x_dir == dim_direction(to.x - from.x)))
        || ((y_dir != 0) && (-y_dir == dim_direction(to.y - from.y)))
}

fn dim_direction(value: f64) -> i32 {
    if value > EPS {
        1
    } else if value < -EPS {
        -1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::{
        endpoint_boundary_points, orthog_turn_order, orthogonal_transition_cost,
        segment_reverses_direction, step_direction, OrthogonalObstacle, QueueState,
        RoutingCostSettings, StepDirection,
    };
    use crate::constants::{CONN_DIR_RIGHT, CONN_DIR_UP};
    use crate::geometry::{Point, Polygon};
    use std::collections::BinaryHeap;

    #[test]
    fn reverse_direction_penalty_predicate_matches_source_to_destination_axes() {
        let src = Point { x: 0.0, y: 0.0 };
        let dst = Point { x: 100.0, y: 50.0 };

        assert!(segment_reverses_direction(
            Point { x: 20.0, y: 20.0 },
            Point { x: 10.0, y: 20.0 },
            src,
            dst,
        ));
        assert!(segment_reverses_direction(
            Point { x: 20.0, y: 20.0 },
            Point { x: 20.0, y: 10.0 },
            src,
            dst,
        ));
        assert!(!segment_reverses_direction(
            Point { x: 20.0, y: 20.0 },
            Point { x: 30.0, y: 20.0 },
            src,
            dst,
        ));
        assert!(!segment_reverses_direction(
            Point { x: 20.0, y: 20.0 },
            Point { x: 20.0, y: 30.0 },
            src,
            dst,
        ));
    }

    #[test]
    fn orthogonal_transition_cost_uses_segment_not_angle_penalty_for_bends() {
        let settings = RoutingCostSettings {
            segment_penalty: 7.0,
            angle_penalty: 101.0,
            crossing_penalty: 0.0,
            fixed_shared_path_penalty: 0.0,
            reverse_direction_penalty: 19.0,
        };
        let src = Point { x: 0.0, y: 0.0 };
        let dst = Point { x: 100.0, y: 100.0 };

        assert_eq!(
            orthogonal_transition_cost(
                StepDirection::None,
                StepDirection::Right,
                10.0,
                Point { x: 0.0, y: 0.0 },
                Point { x: 10.0, y: 0.0 },
                src,
                dst,
                &[],
                settings,
            ),
            10.0
        );
        assert_eq!(
            orthogonal_transition_cost(
                StepDirection::Right,
                StepDirection::Right,
                10.0,
                Point { x: 10.0, y: 0.0 },
                Point { x: 20.0, y: 0.0 },
                src,
                dst,
                &[],
                settings,
            ),
            10.0
        );
        assert_eq!(
            orthogonal_transition_cost(
                StepDirection::Right,
                StepDirection::Up,
                10.0,
                Point { x: 10.0, y: 0.0 },
                Point { x: 10.0, y: 10.0 },
                src,
                dst,
                &[],
                settings,
            ),
            17.0
        );
        assert_eq!(
            orthogonal_transition_cost(
                StepDirection::Right,
                StepDirection::Left,
                10.0,
                Point { x: 10.0, y: 0.0 },
                Point { x: 0.0, y: 0.0 },
                Point { x: 0.0, y: 0.0 },
                Point { x: 0.0, y: 100.0 },
                &[],
                settings,
            ),
            24.0
        );
        assert_eq!(
            orthogonal_transition_cost(
                StepDirection::None,
                step_direction(Point { x: 10.0, y: 0.0 }, Point { x: 0.0, y: 0.0 }),
                10.0,
                Point { x: 10.0, y: 0.0 },
                Point { x: 0.0, y: 0.0 },
                Point { x: 0.0, y: 0.0 },
                Point { x: 100.0, y: 0.0 },
                &[],
                settings,
            ),
            29.0
        );
    }

    #[test]
    fn queue_state_tiebreaker_matches_upstream_timestamp_ordering() {
        let mut heap = BinaryHeap::new();
        heap.push(QueueState {
            cost: 100.0,
            node: 1,
            direction: StepDirection::Right,
            timestamp: 1,
        });
        heap.push(QueueState {
            cost: 100.0 + 0.00000001,
            node: 2,
            direction: StepDirection::Left,
            timestamp: 3,
        });

        let first = heap.pop().unwrap();
        assert_eq!(first.node, 2);
        assert_eq!(first.timestamp, 3);
    }

    #[test]
    fn orthogonal_turn_order_matches_upstream_rotation_order() {
        let common = Point { x: 0.0, y: 0.0 };
        let last = Point { x: -10.0, y: 0.0 };

        assert_eq!(
            orthog_turn_order(last, common, Point { x: -1.0, y: 0.0 }),
            0
        );
        assert_eq!(orthog_turn_order(last, common, Point { x: 0.0, y: 1.0 }), 1);
        assert_eq!(
            orthog_turn_order(last, common, Point { x: 0.0, y: -1.0 }),
            2
        );
        assert_eq!(orthog_turn_order(last, common, Point { x: 1.0, y: 0.0 }), 3);
        assert_eq!(orthog_turn_order(last, common, Point { x: 1.0, y: 1.0 }), 4);
    }

    #[test]
    fn endpoint_visibility_points_reach_first_containing_shape_boundary() {
        let obstacle = OrthogonalObstacle {
            polygon: Polygon::rectangle(0.0, 0.0, 100.0, 100.0),
            bounds: crate::geometry::Rect::new(0.0, 0.0, 100.0, 100.0),
        };

        assert_eq!(
            endpoint_boundary_points(
                Point { x: 95.0, y: 25.0 },
                CONN_DIR_RIGHT,
                &[obstacle.clone()],
            ),
            vec![Point { x: 100.0, y: 25.0 }]
        );
        assert_eq!(
            endpoint_boundary_points(Point { x: 25.0, y: 5.0 }, CONN_DIR_UP, &[obstacle]),
            vec![Point { x: 25.0, y: 100.0 }]
        );
    }
}

fn direction_flag(from: Point, to: Point) -> u32 {
    let mut directions = CONN_DIR_NONE;
    if to.y - from.y > EPS {
        directions |= CONN_DIR_UP;
    }
    if from.y - to.y > EPS {
        directions |= CONN_DIR_DOWN;
    }
    if from.x - to.x > EPS {
        directions |= CONN_DIR_LEFT;
    }
    if to.x - from.x > EPS {
        directions |= CONN_DIR_RIGHT;
    }
    directions
}

fn direction_allowed(direction: u32, allowed: u32) -> bool {
    allowed == CONN_DIR_ALL || direction == CONN_DIR_NONE || direction & allowed != 0
}

fn normalized_directions(directions: u32) -> u32 {
    let directions = directions & CONN_DIR_ALL;
    if directions == CONN_DIR_NONE {
        CONN_DIR_ALL
    } else {
        directions
    }
}

fn line_matches_axis(
    graph: &VisibilityGraph,
    vertices: &[usize],
    axis: i64,
    dimension: ScanDimension,
) -> bool {
    vertices.iter().all(|vertex| {
        graph
            .vertices
            .get(*vertex)
            .map(|vertex| {
                let node = Node::from_point(vertex.point);
                match dimension {
                    ScanDimension::X => node.x == axis,
                    ScanDimension::Y => node.y == axis,
                }
            })
            .unwrap_or(false)
    })
}

impl Ord for QueueState {
    fn cmp(&self, other: &Self) -> Ordering {
        if (self.cost - other.cost).abs() > 0.0000001 {
            return other
                .cost
                .partial_cmp(&self.cost)
                .unwrap_or(Ordering::Equal);
        }
        self.timestamp.cmp(&other.timestamp)
    }
}

impl PartialOrd for QueueState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for QueueState {
    fn eq(&self, other: &Self) -> bool {
        self.cost == other.cost
            && self.node == other.node
            && self.direction == other.direction
            && self.timestamp == other.timestamp
    }
}

impl Eq for QueueState {}
