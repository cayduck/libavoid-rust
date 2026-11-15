//! Faithful port of upstream `libavoid/graph.{h,cpp}`.
//!
//! * `EdgeInf *` is an [`EdgeIdx`] into `Router::edge_infs`.
//! * `EdgeInf::lstPrev/lstNext` thread the router-wide [`EdgeList`]s
//!   (`visGraph`, `invisGraph`, `visOrthogGraph`).
//! * `EdgeInf::m_pos1/m_pos2` (iterators into the endpoint vertices'
//!   `EdgeInfList`s) are the two intrusive link slots `m_pos[0]`/`m_pos[1]`;
//!   see `vertices::EdgeInfList`.
//! * `FlagList m_conns` (`std::list<bool *>`) holds [`RerouteFlagIdx`]
//!   handles into `Router::m_conn_reroute_flags`.
//! * Methods needing only the edge are `&self`/`&mut self`; methods that
//!   touch vertices or the router are associated functions taking
//!   `&mut Router`/`&Router` and the handle, e.g.
//!   `EdgeInf::set_dist(router, e, d)` for `e->setDist(d)`.
//! * `new EdgeInf(...)` is [`EdgeInf::new`]; `delete edge` is
//!   [`EdgeInf::delete`] (runs the destructor logic: `makeInactive()` if
//!   added).

#![allow(dead_code)]
// Keep upstream statement shapes (e.g. `x = x - y`, `new` returning a handle).
#![allow(
    clippy::assign_op_pattern,
    clippy::field_reassign_with_default,
    clippy::new_ret_no_self,
    clippy::while_let_loop
)]

use crate::avoid::arena::{EdgeIdx, RerouteFlagIdx, VertIdx};
use crate::avoid::geometry::{euclidean_dist, in_valid_region, segment_shape_intersect, vec_dir};
use crate::avoid::geomtypes::Point;
use crate::avoid::debug::db_printf;
use crate::avoid::router::Router;
use crate::avoid::vertices::{
    EdgeArena, EdgeEnd, EdgeInfList, EdgeListLink, EdgePos, ShapeSet, VertArena, VertID, DUMMY_ORTHOG_ID,
};

// upstream: graph.h typedef std::list<int> ShapeList
pub type ShapeList = Vec<i32>;
// upstream: graph.h typedef std::list<bool *> FlagList
///
/// Boxed, so that the (usually empty) list costs an edge one pointer
/// rather than a whole `Vec`.
#[derive(Clone, Debug, Default)]
#[allow(clippy::box_collection)] // one pointer per edge instead of a Vec
pub struct FlagList(Option<Box<Vec<RerouteFlagIdx>>>);

impl FlagList {
    pub const fn new() -> FlagList {
        FlagList(None)
    }

    pub fn push(&mut self, flag: RerouteFlagIdx) {
        self.0.get_or_insert_with(Default::default).push(flag);
    }

    /// Empties the list (and frees it).
    pub fn clear(&mut self) {
        self.0 = None;
    }

    pub fn is_empty(&self) -> bool {
        self.0.as_ref().is_none_or(|v| v.is_empty())
    }

    pub fn len(&self) -> usize {
        self.0.as_ref().map_or(0, |v| v.len())
    }

    pub fn iter(&self) -> std::slice::Iter<'_, RerouteFlagIdx> {
        self.0.as_deref().map_or(&[][..], |v| v.as_slice()).iter()
    }
}

/// Index of the *active* non-orthogonal edges (those in their vertices'
/// `visList` / `invisList`, i.e. `m_added`) by unordered vertex pair.  No upstream counterpart: upstream's
/// `EdgeInf::existingEdge()` walks a vertex's edge lists, which makes
/// building the (in)visibility graph cubic in the number of vertices; the
/// index answers the same question in constant time.
///
/// It is maintained by `makeActive()` / `makeInactive()`, the only places
/// that change list membership.  When exactly one active edge joins a pair
/// (the normal case: `existingEdge()` is what callers use to avoid making a
/// second one), that edge is what the list walk finds, whatever the list
/// order.  For a pair with several active edges the walk's answer depends
/// on the lists, so the index only records that fact and
/// `existing_edge()` falls back to the walk.
///
/// Orthogonal visibility edges are not indexed: the orthogonal graph is
/// rebuilt on every transaction and `existingEdge()` is only asked about
/// poly-line vertex pairs, so keeping them in the index cost every
/// orthogonal transaction a hash insert and remove per edge for nothing.
/// `existing_edge()` walks the (in a poly-line router, empty) orthogonal
/// list where the walk would.
#[derive(Default)]
pub struct EdgePairIndex {
    /// The pair's active edge, or `EdgeIdx::dangling()` (which names no
    /// edge; an `Option` would make every entry larger) when the pair has
    /// several, or one that is not recorded.
    map: std::collections::HashMap<u64, EdgeIdx, std::hash::BuildHasherDefault<PairHasher>>,
    /// Number of active edges of the pairs that have, or had, several.
    several: std::collections::HashMap<u64, u32, std::hash::BuildHasherDefault<PairHasher>>,
}

/// What the index knows about a pair.
enum PairLookup {
    /// No active edge.
    None,
    /// Exactly this active edge.
    One(EdgeIdx),
    /// Several active edges (or one that is not recorded): walk the lists.
    Walk,
}

/// Multiplicative hasher for the `u64` pair keys (never iterated, so the
/// map's order cannot influence anything).
#[derive(Default)]
pub struct PairHasher(u64);

impl std::hash::Hasher for PairHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0.rotate_left(5) ^ b as u64).wrapping_mul(0x517c_c1b7_2722_0a95);
        }
    }
    #[inline]
    fn write_u64(&mut self, v: u64) {
        let h = v.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        self.0 = h ^ (h >> 32);
    }
}

impl EdgePairIndex {
    /// Active edges only join live vertices and a lookup is made with live
    /// handles, so the slot numbers identify the pair.
    #[inline]
    fn key(i: VertIdx, j: VertIdx) -> u64 {
        let (a, b) = (i.slot(), j.slot());
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        ((lo as u64) << 32) | hi as u64
    }

    fn insert(&mut self, i: VertIdx, j: VertIdx, e: EdgeIdx) {
        let key = Self::key(i, j);
        match self.map.entry(key) {
            std::collections::hash_map::Entry::Vacant(v) => {
                v.insert(e);
            }
            std::collections::hash_map::Entry::Occupied(mut o) => {
                o.insert(EdgeIdx::dangling());
                *self.several.entry(key).or_insert(1) += 1;
            }
        }
    }

    fn remove(&mut self, i: VertIdx, j: VertIdx) {
        let key = Self::key(i, j);
        match self.several.get_mut(&key) {
            Some(count) => {
                // The remaining edge(s) stay unrecorded.
                *count -= 1;
                if *count == 0 {
                    self.several.remove(&key);
                    self.map.remove(&key);
                }
            }
            None => {
                let removed = self.map.remove(&key);
                debug_assert!(removed.is_some(), "edge pair index: removing an unknown pair");
            }
        }
    }

    #[inline]
    fn lookup(&self, i: VertIdx, j: VertIdx) -> PairLookup {
        match self.map.get(&Self::key(i, j)) {
            None => PairLookup::None,
            Some(e) if *e != EdgeIdx::dangling() => PairLookup::One(*e),
            Some(_) => PairLookup::Walk,
        }
    }
}

/// Upstream `class EdgeInf`.
// upstream: graph.h class EdgeInf
#[derive(Clone, Debug)]
pub struct EdgeInf {
    /// Monotonic creation serial; use for any upstream pointer ordering
    /// (`// PTR-ORDER:`).
    pub serial: u64,
    pub lst_prev: Option<EdgeIdx>,
    pub lst_next: Option<EdgeIdx>,
    // (upstream private members; `pub` so friend code — MTST, VertInf — can
    // reach them)
    pub m_blocker: i32,
    pub m_added: bool,
    pub m_visible: bool,
    pub m_orthogonal: bool,
    pub m_is_hyperedge_segment: bool,
    pub m_disabled: bool,
    pub m_vert1: VertIdx,
    pub m_vert2: VertIdx,
    /// `m_pos1` / `m_pos2`: intrusive links into `m_vert1`'s / `m_vert2`'s
    /// edge list.
    pub m_pos: [EdgeListLink; 2],
    pub m_conns: FlagList,
    pub m_dist: f64,
    /// Uninitialised upstream; 0.0 here.
    pub m_mtst_dist: f64,
}

// upstream: graph.cpp orthogTurnOrder() (static inline)
//
// Gives an order value between 0 and 3 for the point c, given the last
// segment was from a to b.  Returns the following value:
//    0 : Point c is directly backwards from point b.
//    1 : Point c is a left-hand 90 degree turn.
//    2 : Point c is a right-hand 90 degree turn.
//    3 : Point c is straight ahead (collinear).
//    4 : Point c is not orthogonally positioned.
#[inline]
pub fn orthog_turn_order(a: &Point, b: &Point, c: &Point) -> i32 {
    if ((c.x != b.x) && (c.y != b.y)) || ((a.x != b.x) && (a.y != b.y)) {
        // Not orthogonally positioned.
        return 4;
    }

    let direction = vec_dir(a, b, c, 0.0);

    if direction > 0 {
        // Counterclockwise := left
        return 1;
    } else if direction < 0 {
        // Clockwise := right
        return 2;
    }

    if b.x == c.x {
        if ((a.y < b.y) && (c.y < b.y)) || ((a.y > b.y) && (c.y > b.y)) {
            // Behind.
            return 0;
        }
    } else if ((a.x < b.x) && (c.x < b.x)) || ((a.x > b.x) && (c.x > b.x)) {
        // Behind.
        return 0;
    }

    // Ahead.
    3
}

impl EdgeInf {
    /// Allocates an edge (upstream `new EdgeInf(v1, v2, orthogonal)`;
    /// upstream default for `orthogonal` is false).  The edge is not added
    /// to any list until it is activated (`set_dist`/`add_blocker`).
    // upstream: graph.cpp EdgeInf::EdgeInf()
    pub fn new(router: &mut Router, v1: VertIdx, v2: VertIdx, orthogonal: bool) -> EdgeIdx {
        // (upstream asserts both vertices belong to the same router.)
        debug_assert!(router.vert_infs.contains(v1) && router.vert_infs.contains(v2));
        let edge = EdgeInf {
            serial: router.next_serial(),
            lst_prev: None,
            lst_next: None,
            m_blocker: 0,
            m_added: false,
            m_visible: false,
            m_orthogonal: orthogonal,
            m_is_hyperedge_segment: false,
            m_disabled: false,
            m_vert1: v1,
            m_vert2: v2,
            m_pos: [EdgeListLink::default(); 2],
            m_conns: FlagList::new(),
            m_dist: -1.0,
            m_mtst_dist: 0.0,
        };
        router.edge_infs.alloc(edge)
    }

    /// Upstream `delete edge`.
    // upstream: graph.cpp EdgeInf::~EdgeInf()
    pub fn delete(router: &mut Router, e: EdgeIdx) {
        if router.edge_infs[e].m_added {
            EdgeInf::make_inactive(router, e);
        }
        router.edge_infs.free(e);
    }

    // upstream: graph.h EdgeInf::getDist()
    #[inline]
    pub fn get_dist(&self) -> f64 {
        self.m_dist
    }

    /// Returns a less than operation for a set exploration order for
    /// orthogonal searching.  Forward, then left, then right.  Or if there
    /// is no previous point, then the order is north, east, south, then
    /// west.  Assumes the two edges share a common point.
    // upstream: graph.cpp EdgeInf::rotationLessThan()
    pub fn rotation_less_than(
        &self,
        verts: &VertArena,
        last_v: Option<VertIdx>,
        rhs: &EdgeInf,
    ) -> bool {
        if (self.m_vert1 == rhs.m_vert1) && (self.m_vert2 == rhs.m_vert2) {
            // Effectively the same visibility edge, so they are equal.
            return false;
        }
        let (lhs_v, rhs_v, common_v);

        // Determine common Point and the comparison point on the left- and
        // the right-hand-side.
        if self.m_vert1 == rhs.m_vert1 {
            common_v = self.m_vert1;
            lhs_v = self.m_vert2;
            rhs_v = rhs.m_vert2;
        } else if self.m_vert1 == rhs.m_vert2 {
            common_v = self.m_vert1;
            lhs_v = self.m_vert2;
            rhs_v = rhs.m_vert1;
        } else if self.m_vert2 == rhs.m_vert1 {
            common_v = self.m_vert2;
            lhs_v = self.m_vert1;
            rhs_v = rhs.m_vert2;
        } else if self.m_vert2 == rhs.m_vert2 {
            common_v = self.m_vert2;
            lhs_v = self.m_vert1;
            rhs_v = rhs.m_vert1;
        } else {
            // Upstream dereferences nullptr here (precondition violation).
            panic!("EdgeInf::rotationLessThan(): edges share no vertex");
        }

        let lhs_pt = &verts[lhs_v].point;
        let rhs_pt = &verts[rhs_v].point;
        let common_pt = &verts[common_v].point;

        // If no lastPt, use one directly to the left;
        let last_pt = match last_v {
            Some(l) => verts[l].point,
            None => Point::new(common_pt.x - 10.0, common_pt.y),
        };

        let lhs_val = orthog_turn_order(&last_pt, common_pt, lhs_pt);
        let rhs_val = orthog_turn_order(&last_pt, common_pt, rhs_pt);

        lhs_val < rhs_val
    }

    // upstream: graph.cpp EdgeInf::makeActive()
    pub fn make_active(router: &mut Router, e: EdgeIdx) {
        let (orthogonal, visible, v1, v2) = {
            let edge = &router.edge_infs[e];
            debug_assert!(!edge.m_added);
            (edge.m_orthogonal, edge.m_visible, edge.m_vert1, edge.m_vert2)
        };
        let pos1 = EdgePos::new(e, EdgeEnd::First);
        let pos2 = EdgePos::new(e, EdgeEnd::Second);

        let Router {
            vert_infs,
            edge_infs,
            vis_graph,
            invis_graph,
            vis_orthog_graph,
            ..
        } = router;

        if orthogonal {
            debug_assert!(visible);
            vis_orthog_graph.add_edge(edge_infs, vert_infs, e);
            vert_infs[v1].orthog_vis_list.push_front(edge_infs, pos1);
            vert_infs[v1].orthog_vis_list_size += 1;
            vert_infs[v2].orthog_vis_list.push_front(edge_infs, pos2);
            vert_infs[v2].orthog_vis_list_size += 1;
        } else if visible {
            vis_graph.add_edge(edge_infs, vert_infs, e);
            vert_infs[v1].vis_list.push_front(edge_infs, pos1);
            vert_infs[v1].vis_list_size += 1;
            vert_infs[v2].vis_list.push_front(edge_infs, pos2);
            vert_infs[v2].vis_list_size += 1;
        } else {
            // if (invisible)
            invis_graph.add_edge(edge_infs, vert_infs, e);
            vert_infs[v1].invis_list.push_front(edge_infs, pos1);
            vert_infs[v1].invis_list_size += 1;
            vert_infs[v2].invis_list.push_front(edge_infs, pos2);
            vert_infs[v2].invis_list_size += 1;
        }
        edge_infs[e].m_added = true;
        if !orthogonal {
            router.edge_pair_index.insert(v1, v2, e);
        }
    }

    // upstream: graph.cpp EdgeInf::makeInactive()
    pub fn make_inactive(router: &mut Router, e: EdgeIdx) {
        let (orthogonal, visible, v1, v2) = {
            let edge = &router.edge_infs[e];
            debug_assert!(edge.m_added);
            (edge.m_orthogonal, edge.m_visible, edge.m_vert1, edge.m_vert2)
        };
        let pos1 = EdgePos::new(e, EdgeEnd::First);
        let pos2 = EdgePos::new(e, EdgeEnd::Second);

        let Router {
            vert_infs,
            edge_infs,
            vis_graph,
            invis_graph,
            vis_orthog_graph,
            ..
        } = router;

        if orthogonal {
            debug_assert!(visible);
            vis_orthog_graph.remove_edge(edge_infs, e);
            vert_infs[v1].orthog_vis_list.erase(edge_infs, pos1);
            vert_infs[v1].orthog_vis_list_size -= 1;
            vert_infs[v2].orthog_vis_list.erase(edge_infs, pos2);
            vert_infs[v2].orthog_vis_list_size -= 1;
        } else if visible {
            vis_graph.remove_edge(edge_infs, e);
            vert_infs[v1].vis_list.erase(edge_infs, pos1);
            vert_infs[v1].vis_list_size -= 1;
            vert_infs[v2].vis_list.erase(edge_infs, pos2);
            vert_infs[v2].vis_list_size -= 1;
        } else {
            // if (invisible)
            invis_graph.remove_edge(edge_infs, e);
            vert_infs[v1].invis_list.erase(edge_infs, pos1);
            vert_infs[v1].invis_list_size -= 1;
            vert_infs[v2].invis_list.erase(edge_infs, pos2);
            vert_infs[v2].invis_list_size -= 1;
        }
        let edge = &mut edge_infs[e];
        edge.m_blocker = 0;
        edge.m_conns.clear();
        edge.m_added = false;
        if !orthogonal {
            router.edge_pair_index.remove(v1, v2);
        }
    }

    // upstream: graph.cpp EdgeInf::setDist()
    pub fn set_dist(router: &mut Router, e: EdgeIdx, dist: f64) {
        if router.edge_infs[e].m_added && !router.edge_infs[e].m_visible {
            EdgeInf::make_inactive(router, e);
            debug_assert!(!router.edge_infs[e].m_added);
        }
        if !router.edge_infs[e].m_added {
            router.edge_infs[e].m_visible = true;
            EdgeInf::make_active(router, e);
        }
        let edge = &mut router.edge_infs[e];
        edge.m_dist = dist;
        edge.m_blocker = 0;
    }

    // upstream: graph.cpp EdgeInf::setMtstDist()
    #[inline]
    pub fn set_mtst_dist(&mut self, join_cost: f64) {
        self.m_mtst_dist = join_cost;
    }

    // upstream: graph.cpp EdgeInf::mtstDist()
    #[inline]
    pub fn mtst_dist(&self) -> f64 {
        self.m_mtst_dist
    }

    // upstream: graph.cpp EdgeInf::isHyperedgeSegment()
    #[inline]
    pub fn is_hyperedge_segment(&self) -> bool {
        self.m_is_hyperedge_segment
    }

    // upstream: graph.cpp EdgeInf::isDisabled()
    #[inline]
    pub fn is_disabled(&self) -> bool {
        self.m_disabled
    }

    // upstream: graph.cpp EdgeInf::setDisabled()
    #[inline]
    pub fn set_disabled(&mut self, disabled: bool) {
        self.m_disabled = disabled;
    }

    // upstream: graph.cpp EdgeInf::setHyperedgeSegment()
    #[inline]
    pub fn set_hyperedge_segment(&mut self, hyperedge: bool) {
        self.m_is_hyperedge_segment = hyperedge;
    }

    // upstream: graph.cpp EdgeInf::added()
    #[inline]
    pub fn added(&self) -> bool {
        self.m_added
    }

    // upstream: graph.cpp EdgeInf::blocker()
    #[inline]
    pub fn blocker(&self) -> i32 {
        self.m_blocker
    }

    // upstream: graph.cpp EdgeInf::alertConns()
    pub fn alert_conns(router: &mut Router, e: EdgeIdx) {
        let Router {
            edge_infs,
            m_conn_reroute_flags,
            ..
        } = router;
        let edge = &mut edge_infs[e];
        for &flag in edge.m_conns.iter() {
            *m_conn_reroute_flags.flag_mut(flag) = true;
        }
        edge.m_conns.clear();
    }

    // upstream: graph.cpp EdgeInf::addConn()
    #[inline]
    pub fn add_conn(&mut self, flag: RerouteFlagIdx) {
        self.m_conns.push(flag);
    }

    // upstream: graph.cpp EdgeInf::addCycleBlocker()
    pub fn add_cycle_blocker(router: &mut Router, e: EdgeIdx) {
        // Needs to be in invisibility graph.
        EdgeInf::add_blocker(router, e, -1);
    }

    // upstream: graph.cpp EdgeInf::addBlocker()
    pub fn add_blocker(router: &mut Router, e: EdgeIdx, b: i32) {
        debug_assert!(router.invisibility_grph);

        if router.edge_infs[e].m_added && router.edge_infs[e].m_visible {
            EdgeInf::make_inactive(router, e);
            debug_assert!(!router.edge_infs[e].m_added);
        }
        if !router.edge_infs[e].m_added {
            router.edge_infs[e].m_visible = false;
            EdgeInf::make_active(router, e);
        }
        let edge = &mut router.edge_infs[e];
        edge.m_dist = 0.0;
        edge.m_blocker = b;
    }

    // upstream: graph.cpp EdgeInf::ids()
    pub fn ids(&self, verts: &VertArena) -> (VertID, VertID) {
        (verts[self.m_vert1].id, verts[self.m_vert2].id)
    }

    // upstream: graph.cpp EdgeInf::points()
    pub fn points(&self, verts: &VertArena) -> (Point, Point) {
        (verts[self.m_vert1].point, verts[self.m_vert2].point)
    }

    /// Prints the edge with `db_printf` (only with the `libavoid-debug`
    /// feature, upstream's `LIBAVOID_DEBUG`).
    // upstream: graph.cpp EdgeInf::db_print()
    #[inline]
    pub fn db_print(router: &Router, e: EdgeIdx) {
        if !cfg!(feature = "libavoid-debug") {
            return;
        }
        let edge = &router.edge_infs[e];
        db_printf!("Edge(");
        router.vert_infs[edge.m_vert1].id.db_print();
        db_printf!(",");
        router.vert_infs[edge.m_vert2].id.db_print();
        db_printf!(")\n");
    }

    // upstream: graph.cpp EdgeInf::checkVis()
    pub fn check_vis(router: &mut Router, e: EdgeIdx) {
        {
            let edge = &router.edge_infs[e];
            if edge.m_added && !edge.m_visible {
                db_printf!("\tChecking visibility for existing invisibility edge...\n\t\t");
                EdgeInf::db_print(router, e);
            } else if edge.m_added && edge.m_visible {
                db_printf!("\tChecking visibility for existing visibility edge...\n\t\t");
                EdgeInf::db_print(router, e);
            }
        }

        let mut blocker = 0;
        let mut cone1 = true;
        let mut cone2 = true;

        let i = router.edge_infs[e].m_vert1;
        let j = router.edge_infs[e].m_vert2;
        let i_id = router.vert_infs[i].id;
        let j_id = router.vert_infs[j].id;
        let i_point = router.vert_infs[i].point;
        let j_point = router.vert_infs[j].point;

        router.st_checked_edges += 1;

        if !i_id.is_conn_pt() {
            let vi = &router.vert_infs[i];
            cone1 = in_valid_region(
                router.ignore_regions,
                &router.vert_infs[vi.sh_prev.unwrap()].point,
                &i_point,
                &router.vert_infs[vi.sh_next.unwrap()].point,
                &j_point,
            );
        } else if !router.ignore_regions {
            // If Ignoring regions then this case is already caught by
            // the invalid regions, so only check it when not ignoring
            // regions.
            let ss = router.contains.entry(i_id).or_default();

            if !j_id.is_conn_pt() && ss.contains(&j_id.obj_id) {
                db_printf!("1: Edge of bounding shape\n");
                // Don't even check this edge, it should be zero,
                // since a point in a shape can't see it's corners
                cone1 = false;
            }
        }

        if cone1 {
            // If outside the first cone, don't even bother checking.
            if !j_id.is_conn_pt() {
                let vj = &router.vert_infs[j];
                cone2 = in_valid_region(
                    router.ignore_regions,
                    &router.vert_infs[vj.sh_prev.unwrap()].point,
                    &j_point,
                    &router.vert_infs[vj.sh_next.unwrap()].point,
                    &i_point,
                );
            } else if !router.ignore_regions {
                // If Ignoring regions then this case is already caught by
                // the invalid regions, so only check it when not ignoring
                // regions.
                let ss = router.contains.entry(j_id).or_default();

                if !i_id.is_conn_pt() && ss.contains(&i_id.obj_id) {
                    db_printf!("2: Edge of bounding shape\n");
                    // Don't even check this edge, it should be zero,
                    // since a point in a shape can't see it's corners
                    cone2 = false;
                }
            }
        }

        if cone1 && cone2 && {
            blocker = EdgeInf::first_blocker(router, e);
            blocker == 0
        } {
            // if i and j see each other, add edge
            db_printf!("\tSetting visibility edge... \n\t\t");
            EdgeInf::db_print(router, e);

            let d = euclidean_dist(&i_point, &j_point);

            EdgeInf::set_dist(router, e, d);
        } else if router.invisibility_grph {
            // (An `#if 0` block upstream printed cone1/cone2/blocker.)

            // if i and j can't see each other, add blank edge
            db_printf!("\tSetting invisibility edge... \n\t\t");
            EdgeInf::db_print(router, e);
            EdgeInf::add_blocker(router, e, blocker);
        }
    }

    // upstream: graph.cpp EdgeInf::firstBlocker()
    pub fn first_blocker(router: &mut Router, e: EdgeIdx) -> i32 {
        let mut ss = ShapeSet::new();

        let v1 = router.edge_infs[e].m_vert1;
        let v2 = router.edge_infs[e].m_vert2;
        let pti = router.vert_infs[v1].point;
        let ptj = router.vert_infs[v2].point;
        let i_id = router.vert_infs[v1].id;
        let j_id = router.vert_infs[v2].id;

        let contains = &mut router.contains;
        if i_id.is_conn_pt() {
            // `contains[iID]` default-inserts, as upstream operator[] does.
            ss.extend(contains.entry(i_id).or_default().iter().copied());
        }
        if j_id.is_conn_pt() {
            ss.extend(contains.entry(j_id).or_default().iter().copied());
        }

        let verts = &router.vert_infs;
        let last = router.vertices.end();
        let mut last_id: u32 = 0;
        let mut seen_intersection_at_endpoint = false;
        let mut k = router.vertices.shapes_begin();
        while k != last {
            let kv = k.unwrap();
            let k_id = verts[kv].id;
            if verts[kv].id == DUMMY_ORTHOG_ID {
                // Don't include orthogonal dummy vertices.
                k = verts[kv].lst_next;
                continue;
            }
            if k_id.obj_id != last_id {
                if ss.contains(&k_id.obj_id) {
                    let shape_id = k_id.obj_id;
                    db_printf!("Endpoint is inside shape {} so ignore shape edges.\n", k_id.obj_id);
                    // One of the endpoints is inside this shape so ignore it.
                    while let Some(kk) = k {
                        if verts[kk].id.obj_id != shape_id {
                            break;
                        }
                        // And skip the other vertices from this shape.
                        k = verts[kk].lst_next;
                    }
                    continue;
                }
                seen_intersection_at_endpoint = false;
                last_id = k_id.obj_id;
            }
            let k_point = &verts[kv].point;
            let k_prev_point = &verts[verts[kv].sh_prev.unwrap()].point;
            if segment_shape_intersect(
                &pti,
                &ptj,
                k_prev_point,
                k_point,
                &mut seen_intersection_at_endpoint,
            ) {
                return k_id.obj_id as i32;
            }
            k = verts[kv].lst_next;
        }
        0
    }

    // upstream: graph.cpp EdgeInf::isBetween()
    #[inline]
    pub fn is_between(&self, i: VertIdx, j: VertIdx) -> bool {
        if ((i == self.m_vert1) && (j == self.m_vert2))
            || ((i == self.m_vert2) && (j == self.m_vert1))
        {
            return true;
        }
        false
    }

    /// Returns true if this edge is a vertical or horizontal line segment.
    // upstream: graph.cpp EdgeInf::isOrthogonal()
    #[inline]
    pub fn is_orthogonal(&self, verts: &VertArena) -> bool {
        let p1 = &verts[self.m_vert1].point;
        let p2 = &verts[self.m_vert2].point;
        (p1.x == p2.x) || (p1.y == p2.y)
    }

    // upstream: graph.cpp EdgeInf::isDummyConnection()
    pub fn is_dummy_connection(&self, verts: &VertArena) -> bool {
        // This is a dummy edge from a shape centre to
        // a set of its ShapeConnectionPins.
        let id1 = &verts[self.m_vert1].id;
        let id2 = &verts[self.m_vert2].id;
        (id1.is_connection_pin() && id2.is_conn_pt()) || (id2.is_connection_pin() && id1.is_conn_pt())
    }

    // upstream: graph.cpp EdgeInf::otherVert()
    #[inline]
    pub fn other_vert(&self, vert: VertIdx) -> VertIdx {
        debug_assert!((vert == self.m_vert1) || (vert == self.m_vert2));

        if vert == self.m_vert1 {
            self.m_vert2
        } else {
            self.m_vert1
        }
    }

    /// Upstream default for `known_new` is false.
    // upstream: graph.cpp EdgeInf::checkEdgeVisibility()
    pub fn check_edge_visibility(
        router: &mut Router,
        i: VertIdx,
        j: VertIdx,
        known_new: bool,
    ) -> Option<EdgeIdx> {
        // This is for polyline routing, so check we're not
        // considering orthogonal vertices.
        debug_assert!(router.vert_infs[i].id != DUMMY_ORTHOG_ID);
        debug_assert!(router.vert_infs[j].id != DUMMY_ORTHOG_ID);

        let edge = if known_new {
            // Upstream: COLA_ASSERT(existingEdge(i, j) == nullptr).  Not
            // ported (assert policy, router.rs module docs): the asserting
            // upstream build fails it on valid input (audit scene
            // tests/audit_scenes/audit_000.ops: mixed poly-line/orthogonal
            // router, transactions off); the NDEBUG build creates the
            // duplicate edge, as here (the scene matches its golden).
            EdgeInf::new(router, i, j, false)
        } else {
            match EdgeInf::existing_edge(router, i, j) {
                Some(e) => e,
                None => EdgeInf::new(router, i, j, false),
            }
        };
        EdgeInf::check_vis(router, edge);
        if !router.edge_infs[edge].m_added && !router.invisibility_grph {
            EdgeInf::delete(router, edge);
            return None;
        }

        Some(edge)
    }

    // upstream: graph.cpp EdgeInf::existingEdge()
    pub fn existing_edge(router: &Router, i: VertIdx, j: VertIdx) -> Option<EdgeIdx> {
        // (not upstream: the pair index gives the walk's answer directly
        // unless several active edges join the pair; see `EdgePairIndex`)
        // The walk looks through the visList, then the orthogonal list,
        // then the invisList; the index knows the first and the last.
        let found = match router.edge_pair_index.lookup(i, j) {
            PairLookup::None => EdgeInf::existing_orthogonal_edge(router, i, j),
            PairLookup::One(e) if router.edge_infs[e].m_visible => Some(e),
            PairLookup::One(e) => EdgeInf::existing_orthogonal_edge(router, i, j).or(Some(e)),
            PairLookup::Walk => return EdgeInf::existing_edge_walk(router, i, j),
        };
        debug_assert_eq!(EdgeInf::existing_edge_walk(router, i, j), found);
        found
    }

    /// The orthogonal-list part of `existing_edge_walk()`.
    fn existing_orthogonal_edge(router: &Router, i: VertIdx, j: VertIdx) -> Option<EdgeIdx> {
        if router.vis_orthog_graph.size() == 0 {
            return None;
        }
        let (vi, vj) = (&router.vert_infs[i], &router.vert_infs[j]);
        let selected = if vi.orthog_vis_list_size <= vj.orthog_vis_list_size {
            vi
        } else {
            vj
        };
        find_in_list(&router.edge_infs, &selected.orthog_vis_list, i, j)
    }

    /// Upstream's `existingEdge()`: the walk over the edge lists.
    // upstream: graph.cpp EdgeInf::existingEdge()
    fn existing_edge_walk(router: &Router, i: VertIdx, j: VertIdx) -> Option<EdgeIdx> {
        let verts = &router.vert_infs;
        let edges = &router.edge_infs;
        let vi = &verts[i];
        let vj = &verts[j];

        let find = find_in_list;

        // Look through poly-line visibility edges.
        let selected = if vi.vis_list_size <= vj.vis_list_size { vi } else { vj };
        if let Some(e) = find(edges, &selected.vis_list, i, j) {
            return Some(e);
        }

        // Look through orthogonal visibility edges.
        let selected = if vi.orthog_vis_list_size <= vj.orthog_vis_list_size {
            vi
        } else {
            vj
        };
        if let Some(e) = find(edges, &selected.orthog_vis_list, i, j) {
            return Some(e);
        }

        // Look through poly-line invisibility edges.
        let selected = if vi.invis_list_size <= vj.invis_list_size { vi } else { vj };
        if let Some(e) = find(edges, &selected.invis_list, i, j) {
            return Some(e);
        }

        None
    }
}

/// The first edge of `list` joining `i` and `j` (one arena access per list
/// node: the node's edge gives both the test and the link to the next node).
#[inline]
fn find_in_list(edges: &EdgeArena, list: &EdgeInfList, i: VertIdx, j: VertIdx) -> Option<EdgeIdx> {
    let mut cur = list.first();
    while let Some(p) = cur {
        let edge = &edges[p.edge()];
        if edge.is_between(i, j) {
            return Some(p.edge());
        }
        cur = edge.m_pos[p.end() as usize].next;
    }
    None
}

/// `CmpVisEdgeRotation` comparator from makepath.cpp, provided here because
/// it is the user of `rotationLessThan()` and of in-place edge list sorting.
/// The non-orthogonal fallback `u < v` compares pointers upstream.
// upstream: makepath.cpp class CmpVisEdgeRotation::operator()
pub fn cmp_vis_edge_rotation(router: &Router, last_pt: Option<VertIdx>, u: EdgeIdx, v: EdgeIdx) -> bool {
    let verts = &router.vert_infs;
    let eu = &router.edge_infs[u];
    let ev = &router.edge_infs[v];
    // Dummy ShapeConnectionPin edges are not orthogonal and
    // therefore can't be compared in the same way.
    if eu.is_orthogonal(verts) && ev.is_orthogonal(verts) {
        return eu.rotation_less_than(verts, last_pt, ev);
    }
    // PTR-ORDER: upstream `u < v` on EdgeInf pointers; creation serial here.
    eu.serial < ev.serial
}

//===========================================================================

/// Identifies one of the router's three global edge lists.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EdgeListId {
    VisGraph,
    InvisGraph,
    VisOrthogGraph,
}

/// Intrusive list of edges threaded through `EdgeInf::lst_prev/lst_next`.
// upstream: graph.h class EdgeList
#[derive(Clone, Debug, Default)]
pub struct EdgeList {
    m_orthogonal: bool,
    m_first_edge: Option<EdgeIdx>,
    m_last_edge: Option<EdgeIdx>,
    m_count: u32,
}

impl EdgeList {
    // upstream: graph.cpp EdgeList::EdgeList()
    pub fn new(orthogonal: bool) -> Self {
        EdgeList {
            m_orthogonal: orthogonal,
            m_first_edge: None,
            m_last_edge: None,
            m_count: 0,
        }
    }

    /// Deletes every edge in the list (upstream `EdgeList::clear()`, also
    /// run by the destructor).
    // upstream: graph.cpp EdgeList::clear()
    pub fn clear(router: &mut Router, which: EdgeListId) {
        while let Some(first) = router.edge_list(which).m_first_edge {
            // The Edge destructor results in EdgeList::removeEdge() being
            // called for this edge and m_first_edge being updated to the
            // subsequent edge in the EdgeList.
            EdgeInf::delete(router, first);
        }
        let list = router.edge_list_mut(which);
        debug_assert!(list.m_count == 0);
        list.m_last_edge = None;
    }

    // upstream: graph.cpp EdgeList::size()
    #[inline]
    pub fn size(&self) -> i32 {
        self.m_count as i32
    }

    // upstream: graph.cpp EdgeList::addEdge()
    pub(crate) fn add_edge(&mut self, edges: &mut EdgeArena, verts: &VertArena, edge: EdgeIdx) {
        // Dummy connections for ShapeConnectionPins won't be orthogonal,
        // even in the orthogonal visibility graph.
        debug_assert!(
            !self.m_orthogonal
                || edges[edge].is_orthogonal(verts)
                || edges[edge].is_dummy_connection(verts)
        );
        let _ = verts;

        if self.m_first_edge.is_none() {
            debug_assert!(self.m_last_edge.is_none());

            self.m_last_edge = Some(edge);
            self.m_first_edge = Some(edge);

            edges[edge].lst_prev = None;
            edges[edge].lst_next = None;
        } else {
            let last = self
                .m_last_edge
                .expect("EdgeList::addEdge(): m_last_edge is null");

            edges[last].lst_next = Some(edge);
            edges[edge].lst_prev = Some(last);

            self.m_last_edge = Some(edge);

            edges[edge].lst_next = None;
        }
        self.m_count += 1;
    }

    // upstream: graph.cpp EdgeList::removeEdge()
    pub(crate) fn remove_edge(&mut self, edges: &mut EdgeArena, edge: EdgeIdx) {
        let prev = edges[edge].lst_prev;
        let next = edges[edge].lst_next;
        if let Some(p) = prev {
            edges[p].lst_next = next;
        }
        if let Some(n) = next {
            edges[n].lst_prev = prev;
        }
        if Some(edge) == self.m_last_edge {
            self.m_last_edge = prev;
            if Some(edge) == self.m_first_edge {
                self.m_first_edge = None;
            }
        } else if Some(edge) == self.m_first_edge {
            self.m_first_edge = next;
        }

        edges[edge].lst_prev = None;
        edges[edge].lst_next = None;

        self.m_count -= 1;
    }

    // upstream: graph.cpp EdgeList::begin()
    #[inline]
    pub fn begin(&self) -> Option<EdgeIdx> {
        self.m_first_edge
    }

    // upstream: graph.cpp EdgeList::end()
    #[inline]
    pub fn end(&self) -> Option<EdgeIdx> {
        None
    }

    /// Snapshot of the edges in list order.
    pub fn to_vec(&self, edges: &EdgeArena) -> Vec<EdgeIdx> {
        let mut out = Vec::with_capacity(self.m_count as usize);
        let mut cur = self.m_first_edge;
        while let Some(e) = cur {
            out.push(e);
            cur = edges[e].lst_next;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avoid::router::{Router, POLY_LINE_ROUTING};
    use crate::avoid::vertices::{VertInf, CONN_DIR_ALL, CONN_DIR_RIGHT};

    fn v(router: &mut Router, id: u32, x: f64, y: f64) -> VertIdx {
        VertInf::new(router, VertID::new(id, 0, 0), Point::new(x, y), false)
    }

    #[test]
    fn activation_moves_between_lists() {
        let mut r = Router::new(POLY_LINE_ROUTING);
        let a = v(&mut r, 1, 0.0, 0.0);
        let b = v(&mut r, 2, 3.0, 4.0);
        let c = v(&mut r, 3, 0.0, 4.0);
        let e1 = EdgeInf::new(&mut r, a, b, false);
        assert!(!r.edge_infs[e1].added());
        assert_eq!(r.edge_infs[e1].get_dist(), -1.0);
        EdgeInf::set_dist(&mut r, e1, 5.0);
        assert!(r.edge_infs[e1].added());
        assert_eq!(r.vis_graph.size(), 1);
        assert_eq!(r.vert_infs[a].vis_list_size, 1);
        assert_eq!(r.vert_infs[b].vis_list.front(), Some(e1));

        let e2 = EdgeInf::new(&mut r, a, c, false);
        EdgeInf::set_dist(&mut r, e2, 4.0);
        // Per-vertex lists insert at the front; global list appends.
        assert_eq!(r.vert_infs[a].vis_list.to_vec(&r.edge_infs), vec![e2, e1]);
        assert_eq!(r.vis_graph.to_vec(&r.edge_infs), vec![e1, e2]);

        // Blocking moves e1 to the invisibility graph.
        EdgeInf::add_blocker(&mut r, e1, 7);
        assert_eq!(r.vis_graph.to_vec(&r.edge_infs), vec![e2]);
        assert_eq!(r.invis_graph.to_vec(&r.edge_infs), vec![e1]);
        assert_eq!(r.vert_infs[a].vis_list.to_vec(&r.edge_infs), vec![e2]);
        assert_eq!(r.vert_infs[a].invis_list.to_vec(&r.edge_infs), vec![e1]);
        assert_eq!(
            (r.vert_infs[a].vis_list_size, r.vert_infs[a].invis_list_size),
            (1, 1)
        );
        assert_eq!(r.edge_infs[e1].blocker(), 7);
        assert_eq!(r.edge_infs[e1].get_dist(), 0.0);

        // Visible again.
        EdgeInf::set_dist(&mut r, e1, 5.0);
        assert_eq!(r.vis_graph.to_vec(&r.edge_infs), vec![e2, e1]);
        assert_eq!(r.vert_infs[a].vis_list.to_vec(&r.edge_infs), vec![e1, e2]);
        assert_eq!(r.edge_infs[e1].blocker(), 0);
        assert_eq!(r.invis_graph.size(), 0);

        // existingEdge / otherVert / isBetween
        assert_eq!(EdgeInf::existing_edge(&r, b, a), Some(e1));
        assert_eq!(EdgeInf::existing_edge(&r, a, c), Some(e2));
        assert_eq!(EdgeInf::existing_edge(&r, b, c), None);
        assert_eq!(r.edge_infs[e1].other_vert(a), b);
        assert_eq!(r.edge_infs[e1].other_vert(b), a);
        assert!(r.edge_infs[e2].is_orthogonal(&r.vert_infs));
        assert!(!r.edge_infs[e1].is_orthogonal(&r.vert_infs));
        assert_eq!(VertInf::has_neighbour(&r, a, c, false), Some(e2));
        assert_eq!(VertInf::has_neighbour(&r, a, c, true), None);

        // Orthogonal edges go to the orthogonal lists.
        let e3 = EdgeInf::new(&mut r, b, c, true);
        EdgeInf::set_dist(&mut r, e3, 3.0);
        assert_eq!(r.vis_orthog_graph.to_vec(&r.edge_infs), vec![e3]);
        assert_eq!(r.vert_infs[c].orthog_vis_list.to_vec(&r.edge_infs), vec![e3]);
        assert_eq!(EdgeInf::existing_edge(&r, c, b), Some(e3));

        // setVisibleDirections disables edges by direction.
        VertInf::set_visible_directions(&mut r, c, CONN_DIR_RIGHT);
        assert!(!r.edge_infs[e3].is_disabled()); // b is to the right of c
        assert!(r.edge_infs[e2].is_disabled()); // a is below c (y smaller)
        VertInf::set_visible_directions(&mut r, c, CONN_DIR_ALL);
        assert!(!r.edge_infs[e2].is_disabled());

        // Reroute flags are raised by alertConns.
        let flag = r
            .m_conn_reroute_flags
            .add_conn(crate::avoid::arena::ConnIdx::dangling());
        r.edge_infs[e2].add_conn(flag);
        EdgeInf::alert_conns(&mut r, e2);
        assert!(*r.m_conn_reroute_flags.flag_mut(flag));
        assert!(r.edge_infs[e2].m_conns.is_empty());

        // orphan() deactivates without deleting; delete removes from lists.
        VertInf::orphan(&mut r, c);
        assert!(r.vert_infs[c].orphaned());
        assert!(!r.edge_infs[e3].added());
        assert_eq!(r.vis_orthog_graph.size(), 0);
        EdgeInf::delete(&mut r, e3);
        EdgeInf::delete(&mut r, e1);
        // e2 (a-c) was made inactive by orphan(c) too; re-add then clear.
        assert_eq!(r.vis_graph.size(), 0);
        assert!(!r.edge_infs[e2].added());
        EdgeInf::set_dist(&mut r, e2, 4.0);
        EdgeList::clear(&mut r, EdgeListId::VisGraph);
        assert_eq!(r.vis_graph.size(), 0);
        assert!(r.edge_infs.is_empty());
        assert!(r.vert_infs[a].orphaned());
    }

    #[test]
    fn global_edge_list_removal_positions() {
        let mut r = Router::new(POLY_LINE_ROUTING);
        let a = v(&mut r, 1, 0.0, 0.0);
        let mut es = Vec::new();
        for i in 0..5 {
            let b = v(&mut r, 2 + i, 1.0 + i as f64, 0.0);
            let e = EdgeInf::new(&mut r, a, b, false);
            EdgeInf::set_dist(&mut r, e, 1.0);
            es.push(e);
        }
        EdgeInf::delete(&mut r, es[2]); // middle
        EdgeInf::delete(&mut r, es[0]); // first
        EdgeInf::delete(&mut r, es[4]); // last
        assert_eq!(r.vis_graph.to_vec(&r.edge_infs), vec![es[1], es[3]]);
        assert_eq!(r.vis_graph.begin(), Some(es[1]));
        assert_eq!(
            r.vert_infs[a].vis_list.to_vec(&r.edge_infs),
            vec![es[3], es[1]]
        );
        assert_eq!(r.vert_infs[a].vis_list_size, 2);
        // Slot reuse does not disturb order.
        let b = v(&mut r, 9, 9.0, 0.0);
        let e = EdgeInf::new(&mut r, b, a, false);
        EdgeInf::set_dist(&mut r, e, 1.0);
        assert_eq!(r.vis_graph.to_vec(&r.edge_infs), vec![es[1], es[3], e]);
        assert_eq!(
            r.vert_infs[a].vis_list.to_vec(&r.edge_infs),
            vec![e, es[3], es[1]]
        );
    }

    #[test]
    fn remove_from_graph_deletes_edges() {
        let mut r = Router::new(POLY_LINE_ROUTING);
        let c = VertInf::new(
            &mut r,
            VertID::new(1, VertID::SRC, VertID::PROP_CONN_POINT),
            Point::new(0.0, 0.0),
            false,
        );
        let a = v(&mut r, 2, 1.0, 0.0);
        let e1 = EdgeInf::new(&mut r, c, a, false);
        EdgeInf::set_dist(&mut r, e1, 1.0);
        let e2 = EdgeInf::new(&mut r, a, c, false);
        EdgeInf::add_cycle_blocker(&mut r, e2);
        assert_eq!(r.edge_infs[e2].blocker(), -1);
        VertInf::remove_from_graph(&mut r, c, true);
        assert!(r.vert_infs[c].orphaned());
        assert!(r.vert_infs[a].orphaned());
        assert!(r.edge_infs.is_empty());
        VertInf::delete(&mut r, c);
        VertInf::delete(&mut r, a);
    }

    #[test]
    fn orthog_turn_orders() {
        let p = Point::new;
        let a = p(0.0, 0.0);
        let b = p(1.0, 0.0);
        assert_eq!(orthog_turn_order(&a, &b, &p(0.5, 0.0)), 0); // behind
        assert_eq!(orthog_turn_order(&a, &b, &p(1.0, 1.0)), 1); // left (ccw)
        assert_eq!(orthog_turn_order(&a, &b, &p(1.0, -1.0)), 2); // right
        assert_eq!(orthog_turn_order(&a, &b, &p(2.0, 0.0)), 3); // ahead
        assert_eq!(orthog_turn_order(&a, &b, &p(2.0, 1.0)), 4);
    }

    #[test]
    fn rotation_sort_falls_back_to_creation_order() {
        // A non-orthogonal (dummy pin) edge is compared by creation serial,
        // never by arena slot.
        let mut r = Router::new(POLY_LINE_ROUTING);
        let centre = v(&mut r, 1, 0.0, 0.0);
        let filler = v(&mut r, 50, 0.0, 0.0);
        let filler_e = EdgeInf::new(&mut r, centre, filler, false);
        let n1 = v(&mut r, 2, 1.0, 1.0);
        let n2 = v(&mut r, 3, 2.0, 3.0);
        let e_old = EdgeInf::new(&mut r, centre, n2, true); // slot 1, older
        EdgeInf::delete(&mut r, filler_e); // frees slot 0
        let e_new = EdgeInf::new(&mut r, centre, n1, true); // slot 0, newer
        assert!(e_new.slot() < e_old.slot());
        assert!(r.edge_infs[e_old].serial < r.edge_infs[e_new].serial);
        assert!(cmp_vis_edge_rotation(&r, None, e_old, e_new));
        assert!(!cmp_vis_edge_rotation(&r, None, e_new, e_old));
    }
}
