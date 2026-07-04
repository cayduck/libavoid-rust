use crate::geometry::Point;
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

#[allow(dead_code)]
pub(crate) const VERT_ID_SRC: u16 = 1;
#[allow(dead_code)]
pub(crate) const VERT_ID_TAR: u16 = 2;
pub(crate) const PROP_CONN_POINT: u16 = 1;
pub(crate) const PROP_ORTH_SHAPE_EDGE: u16 = 2;
#[allow(dead_code)]
pub(crate) const PROP_CONNECTION_PIN: u16 = 4;
#[allow(dead_code)]
pub(crate) const PROP_CONN_CHECKPOINT: u16 = 8;
#[allow(dead_code)]
pub(crate) const PROP_DUMMY_PIN_HELPER: u16 = 16;

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct VertId {
    pub(crate) obj_id: u32,
    pub(crate) vn: u16,
    pub(crate) props: u16,
}

#[allow(dead_code)]
impl VertId {
    pub(crate) fn new(obj_id: u32, vn: u16, props: u16) -> VertId {
        VertId { obj_id, vn, props }
    }

    pub(crate) fn is_orth_shape_edge(&self) -> bool {
        (self.props & PROP_ORTH_SHAPE_EDGE) != 0
    }

    pub(crate) fn is_conn_point(&self) -> bool {
        (self.props & PROP_CONN_POINT) != 0
    }

    pub(crate) fn is_connection_pin(&self) -> bool {
        (self.props & PROP_CONNECTION_PIN) != 0
    }

    pub(crate) fn is_conn_checkpoint(&self) -> bool {
        (self.props & PROP_CONN_CHECKPOINT) != 0
    }

    pub(crate) fn is_dummy_pin_helper(&self) -> bool {
        (self.props & PROP_DUMMY_PIN_HELPER) != 0
    }

    pub(crate) fn offset(self, rhs: i16) -> VertId {
        VertId {
            obj_id: self.obj_id,
            vn: self.vn.wrapping_add_signed(rhs),
            props: self.props,
        }
    }
}

impl PartialEq for VertId {
    fn eq(&self, other: &Self) -> bool {
        self.obj_id == other.obj_id && self.vn == other.vn
    }
}

impl Eq for VertId {}

impl PartialOrd for VertId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for VertId {
    fn cmp(&self, other: &Self) -> Ordering {
        self.obj_id
            .cmp(&other.obj_id)
            .then_with(|| self.vn.cmp(&other.vn))
    }
}

impl Hash for VertId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.obj_id.hash(state);
        self.vn.hash(state);
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
