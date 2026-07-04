use crate::constants::{CONN_DIR_DOWN, CONN_DIR_LEFT, CONN_DIR_NONE, CONN_DIR_RIGHT, CONN_DIR_UP};
use crate::geometry::Point;
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

#[allow(dead_code)]
pub(crate) const VERT_ID_SRC: u16 = 1;
#[allow(dead_code)]
pub(crate) const VERT_ID_TAR: u16 = 2;
#[allow(dead_code)]
pub(crate) const VERT_ID_UNASSIGNED: u16 = 8;
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
    #[allow(dead_code)]
    pub(crate) vis_directions: u32,
}

impl VertInf {
    pub(crate) fn new(id: VertId, point: Point) -> VertInf {
        VertInf {
            id,
            point,
            vis_directions: CONN_DIR_NONE,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn reset(&mut self, id: VertId, point: Point) {
        self.id = id;
        self.point = point;
    }

    #[allow(dead_code)]
    pub(crate) fn reset_point(&mut self, point: Point) {
        self.point = point;
    }

    #[allow(dead_code)]
    pub(crate) fn direction_from(&self, other: &VertInf) -> u32 {
        direction_between(other.point, self.point)
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

    pub(crate) fn get_mut(&mut self, index: usize) -> Option<&mut VertInf> {
        self.vertices.get_mut(index)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (usize, &VertInf)> {
        self.vertices.iter().enumerate()
    }

    #[allow(dead_code)]
    pub(crate) fn get_vertex_by_id(&self, id: VertId) -> Option<(usize, &VertInf)> {
        let search_id = normalize_search_id(id);
        self.vertices
            .iter()
            .enumerate()
            .find(|(_, vertex)| vertex.id == search_id)
    }

    #[allow(dead_code)]
    pub(crate) fn get_shape_vertex_by_pos(&self, point: Point) -> Option<(usize, &VertInf)> {
        self.shapes_iter().find(|(_, vertex)| vertex.point == point)
    }

    #[allow(dead_code)]
    pub(crate) fn conns_iter(&self) -> impl Iterator<Item = (usize, &VertInf)> {
        self.vertices
            .iter()
            .enumerate()
            .filter(|(_, vertex)| vertex.id.is_conn_point())
    }

    #[allow(dead_code)]
    pub(crate) fn shapes_iter(&self) -> impl Iterator<Item = (usize, &VertInf)> {
        self.vertices
            .iter()
            .enumerate()
            .filter(|(_, vertex)| !vertex.id.is_conn_point())
    }

    #[allow(dead_code)]
    pub(crate) fn conns_size(&self) -> usize {
        self.conns_iter().count()
    }

    #[allow(dead_code)]
    pub(crate) fn shapes_size(&self) -> usize {
        self.shapes_iter().count()
    }
}

fn normalize_search_id(id: VertId) -> VertId {
    if id.vn != VERT_ID_UNASSIGNED {
        return id;
    }
    let top_bit = 1_u32 << 31;
    if (id.obj_id & top_bit) != 0 {
        VertId::new(id.obj_id & !top_bit, VERT_ID_SRC, id.props)
    } else {
        VertId::new(id.obj_id, VERT_ID_TAR, id.props)
    }
}

#[allow(dead_code)]
pub(crate) fn direction_between(from: Point, to: Point) -> u32 {
    let epsilon = 0.000001;
    let mut directions = CONN_DIR_NONE;
    let dx = to.x - from.x;
    let dy = to.y - from.y;

    if dy > epsilon {
        directions |= CONN_DIR_UP;
    }
    if dy < -epsilon {
        directions |= CONN_DIR_DOWN;
    }
    if dx > epsilon {
        directions |= CONN_DIR_RIGHT;
    }
    if dx < -epsilon {
        directions |= CONN_DIR_LEFT;
    }
    directions
}
