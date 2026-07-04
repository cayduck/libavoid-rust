use crate::geometry::Point;

pub(crate) const PROP_CONN_POINT: u16 = 1;
pub(crate) const PROP_ORTH_SHAPE_EDGE: u16 = 2;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub(crate) struct VertId {
    pub(crate) obj_id: u32,
    pub(crate) vn: u16,
    pub(crate) props: u16,
}

impl VertId {
    pub(crate) fn new(obj_id: u32, vn: u16, props: u16) -> VertId {
        VertId { obj_id, vn, props }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct VertInf {
    #[allow(dead_code)]
    pub(crate) id: VertId,
    pub(crate) point: Point,
}

impl VertInf {
    pub(crate) fn new(id: VertId, point: Point) -> VertInf {
        VertInf { id, point }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct VertInfList {
    vertices: Vec<VertInf>,
}

impl VertInfList {
    pub(crate) fn add_vertex(&mut self, vertex: VertInf) -> usize {
        self.vertices.push(vertex);
        self.vertices.len() - 1
    }

    pub(crate) fn get(&self, index: usize) -> Option<&VertInf> {
        self.vertices.get(index)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (usize, &VertInf)> {
        self.vertices.iter().enumerate()
    }
}
