use std::collections::{BTreeSet, HashMap};

use crate::geometry::Point;
use crate::orthogonal::Direction;
use crate::vertices::{VertId, VertInf, VertInfList};

#[derive(Clone, Debug)]
pub(crate) struct EdgeInf {
    pub(crate) from: usize,
    pub(crate) to: usize,
    pub(crate) dist: f64,
    pub(crate) orthogonal: bool,
    pub(crate) direction: Direction,
    pub(crate) visible: bool,
    pub(crate) blocker: Option<i32>,
    pub(crate) disabled: bool,
    active: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct VisibilityGraph {
    pub(crate) vertices: VertInfList,
    pub(crate) edges: Vec<EdgeInf>,
    adjacency: HashMap<usize, Vec<usize>>,
    #[allow(dead_code)]
    contains: HashMap<VertId, BTreeSet<u32>>,
}

impl VisibilityGraph {
    pub(crate) fn add_vertex(&mut self, id: VertId, point: Point) -> usize {
        self.vertices.add_vertex(VertInf::new(id, point))
    }

    #[allow(dead_code)]
    pub(crate) fn add_visibility_edge(&mut self, from: usize, to: usize) {
        let Some((a, b)) = self.edge_points(from, to) else {
            return;
        };
        self.activate_edge(from, to, euclidean(a, b), false, Direction::None);
    }

    pub(crate) fn add_orthogonal_edge(&mut self, from: usize, to: usize, direction: Direction) {
        let Some((a, b)) = self.edge_points(from, to) else {
            return;
        };
        let aligned = match direction {
            Direction::Horizontal => (a.y - b.y).abs() < crate::constants::EPS,
            Direction::Vertical => (a.x - b.x).abs() < crate::constants::EPS,
            Direction::None => false,
        };
        if !aligned {
            return;
        }
        self.activate_edge(from, to, manhattan(a, b), true, direction);
    }

    pub(crate) fn neighbours(&self, vertex: usize) -> impl Iterator<Item = (usize, &EdgeInf)> {
        self.adjacency
            .get(&vertex)
            .into_iter()
            .flatten()
            .filter_map(move |edge_index| {
                let edge = self.edges.get(*edge_index)?;
                if !edge.active || !edge.visible || edge.disabled {
                    return None;
                }
                let other = if edge.from == vertex {
                    edge.to
                } else {
                    edge.from
                };
                Some((other, edge))
            })
    }

    #[allow(dead_code)]
    pub(crate) fn visible_edges(&self) -> impl Iterator<Item = &EdgeInf> {
        self.edges
            .iter()
            .filter(|edge| edge.active && edge.visible && !edge.disabled)
    }

    #[allow(dead_code)]
    pub(crate) fn invisible_edges(&self) -> impl Iterator<Item = &EdgeInf> {
        self.edges
            .iter()
            .filter(|edge| edge.active && !edge.visible)
    }

    #[allow(dead_code)]
    pub(crate) fn edge_between(&self, from: usize, to: usize) -> Option<&EdgeInf> {
        self.find_edge_index(from, to)
            .and_then(|index| self.edges.get(index))
            .filter(|edge| edge.active)
    }

    #[allow(dead_code)]
    pub(crate) fn hide_edge(&mut self, from: usize, to: usize, blocker: i32) -> bool {
        let Some(index) = self.find_edge_index(from, to) else {
            return false;
        };
        let edge = &mut self.edges[index];
        edge.visible = false;
        edge.blocker = Some(blocker);
        edge.dist = 0.0;
        true
    }

    #[allow(dead_code)]
    pub(crate) fn restore_edge(&mut self, from: usize, to: usize) -> bool {
        let Some(index) = self.find_edge_index(from, to) else {
            return false;
        };
        let Some((a, b)) = self.edge_points(from, to) else {
            return false;
        };
        let edge = &mut self.edges[index];
        edge.visible = true;
        edge.blocker = None;
        edge.dist = if edge.orthogonal {
            manhattan(a, b)
        } else {
            euclidean(a, b)
        };
        true
    }

    #[allow(dead_code)]
    pub(crate) fn remove_edge(&mut self, from: usize, to: usize) -> bool {
        let Some(index) = self.find_edge_index(from, to) else {
            return false;
        };
        let edge = &mut self.edges[index];
        edge.active = false;
        edge.visible = false;
        edge.blocker = None;
        true
    }

    #[allow(dead_code)]
    pub(crate) fn set_edge_disabled(&mut self, from: usize, to: usize, disabled: bool) -> bool {
        let Some(index) = self.find_edge_index(from, to) else {
            return false;
        };
        self.edges[index].disabled = disabled;
        true
    }

    #[allow(dead_code)]
    pub(crate) fn set_contains(&mut self, vertex: usize, shape_id: u32, contains: bool) -> bool {
        let Some(id) = self.vertices.get(vertex).map(|vertex| vertex.id) else {
            return false;
        };
        let entry = self.contains.entry(id).or_default();
        if contains {
            entry.insert(shape_id);
        } else {
            entry.remove(&shape_id);
            if entry.is_empty() {
                self.contains.remove(&id);
            }
        }
        true
    }

    #[allow(dead_code)]
    pub(crate) fn vertex_contains_shape(&self, vertex: usize, shape_id: u32) -> bool {
        self.vertices
            .get(vertex)
            .and_then(|vertex| self.contains.get(&vertex.id))
            .is_some_and(|shapes| shapes.contains(&shape_id))
    }

    #[allow(dead_code)]
    pub(crate) fn contained_shapes(&self, vertex: usize) -> Vec<u32> {
        self.vertices
            .get(vertex)
            .and_then(|vertex| self.contains.get(&vertex.id))
            .map(|shapes| shapes.iter().copied().collect())
            .unwrap_or_default()
    }

    pub(crate) fn edge_points(&self, from: usize, to: usize) -> Option<(Point, Point)> {
        Some((self.vertices.get(from)?.point, self.vertices.get(to)?.point))
    }

    fn activate_edge(
        &mut self,
        from: usize,
        to: usize,
        dist: f64,
        orthogonal: bool,
        direction: Direction,
    ) {
        if from == to {
            return;
        }
        if let Some(index) = self.find_edge_index(from, to) {
            let edge = &mut self.edges[index];
            edge.dist = dist;
            edge.orthogonal = orthogonal;
            edge.direction = direction;
            edge.visible = true;
            edge.blocker = None;
            edge.disabled = false;
            edge.active = true;
            return;
        }
        let edge_index = self.edges.len();
        self.edges.push(EdgeInf {
            from,
            to,
            dist,
            orthogonal,
            direction,
            visible: true,
            blocker: None,
            disabled: false,
            active: true,
        });
        self.adjacency.entry(from).or_default().push(edge_index);
        self.adjacency.entry(to).or_default().push(edge_index);
    }

    fn find_edge_index(&self, from: usize, to: usize) -> Option<usize> {
        self.adjacency.get(&from)?.iter().copied().find(|index| {
            let Some(edge) = self.edges.get(*index) else {
                return false;
            };
            edge.active
                && ((edge.from == from && edge.to == to) || (edge.from == to && edge.to == from))
        })
    }
}

fn manhattan(a: Point, b: Point) -> f64 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
}

#[allow(dead_code)]
fn euclidean(a: Point, b: Point) -> f64 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}
