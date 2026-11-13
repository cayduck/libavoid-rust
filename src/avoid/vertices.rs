//! Faithful port of upstream `libavoid/vertices.{h,cpp}`.
//!
//! * `VertInf *` is a [`VertIdx`] into `Router::vert_infs`.
//! * `VertInf::lstPrev/lstNext` (the router-wide [`VertInfList`]) and
//!   `shPrev/shNext` (the obstacle ring) are `Option<VertIdx>` fields.
//! * The per-vertex `EdgeInfList`s (`visList`, `orthogVisList`,
//!   `invisList`; upstream `std::list<EdgeInf *>` with each `EdgeInf`
//!   remembering its two iterators `m_pos1`/`m_pos2`) are intrusive doubly
//!   linked lists threaded through the edges: every `EdgeInf` carries two
//!   link slots (`m_pos[0]` for its membership in `m_vert1`'s list,
//!   `m_pos[1]` for `m_vert2`'s), and an [`EdgePos`] names one of those
//!   slots.  This gives O(1) insert-at-front and erase exactly like the
//!   upstream iterator scheme, and preserves list order.
//! * `VertInf **m_treeRoot` (a malloc'd cell shared between vertices, or
//!   reused as a plain `VertInf *` by `setSPTFRoot`) is a [`TreeRootField`]
//!   holding either a [`TreeRootPtr`] cell handle (cells live in
//!   `Router::tree_root_cells`) or an SPTF root vertex.
//! * Methods that touch only one vertex are `&self`/`&mut self` methods.
//!   Methods that touch other objects are associated functions taking
//!   `&mut Router` (or `&Router`) plus the handle, e.g.
//!   `VertInf::remove_from_graph(router, v, true)` for
//!   `v->removeFromGraph(true)`.

#![allow(dead_code)]
// Keep upstream statement shapes (e.g. `x = x - y`, `new` returning a handle).
#![allow(
    clippy::assign_op_pattern,
    clippy::field_reassign_with_default,
    clippy::new_ret_no_self,
    clippy::while_let_loop
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::avoid::arena::{ANodeIdx, Arena, EdgeIdx, TreeRootPtr, VertIdx};
use crate::avoid::geometry::segment_intersect;
use crate::avoid::geomtypes::{Point, K_UNASSIGNED_VERTEX_NUMBER};
use crate::avoid::graph::EdgeInf;
use crate::avoid::router::Router;

/// Arena of all `VertInf`s of a router.
pub type VertArena = Arena<VertIdx, VertInf>;
/// Arena of all `EdgeInf`s of a router.
pub type EdgeArena = Arena<EdgeIdx, EdgeInf>;

// upstream: vertices.h typedef std::pair<VertInf *, VertInf *> VertexPair
pub type VertexPair = (VertIdx, VertIdx);

// upstream: vertices.h typedef unsigned int ConnDirFlags
pub type ConnDirFlags = u32;
// upstream: vertices.h typedef unsigned short VertIDProps
pub type VertIDProps = u16;

// upstream: connend.h enum ConnDirFlag (needed by VertInf; connend.rs should
// re-export these rather than redefine them).
pub const CONN_DIR_NONE: ConnDirFlags = 0;
pub const CONN_DIR_UP: ConnDirFlags = 1;
pub const CONN_DIR_DOWN: ConnDirFlags = 2;
pub const CONN_DIR_LEFT: ConnDirFlags = 4;
pub const CONN_DIR_RIGHT: ConnDirFlags = 8;
pub const CONN_DIR_ALL: ConnDirFlags = 15;

/// Vertex identifier.  Equality and ordering use `(obj_id, vn)` only; the
/// `props` flags are ignored, exactly as upstream.
// upstream: vertices.h class VertID
#[derive(Clone, Copy, Debug, Default)]
pub struct VertID {
    pub obj_id: u32,
    pub vn: u16,
    /// Properties.
    pub props: VertIDProps,
}

impl VertID {
    // upstream: vertices.cpp VertID::src / VertID::tar
    pub const SRC: u16 = 1;
    pub const TAR: u16 = 2;

    // upstream: vertices.cpp VertID::PROP_* property flags
    pub const PROP_CONN_POINT: VertIDProps = 1;
    pub const PROP_ORTH_SHAPE_EDGE: VertIDProps = 2;
    pub const PROP_CONNECTION_PIN: VertIDProps = 4;
    pub const PROP_CONN_CHECKPOINT: VertIDProps = 8;
    pub const PROP_DUMMY_PIN_HELPER: VertIDProps = 16;

    // upstream: vertices.cpp VertID::VertID(unsigned int, unsigned short, VertIDProps)
    #[inline]
    pub const fn new(id: u32, n: u16, p: VertIDProps) -> VertID {
        VertID {
            obj_id: id,
            vn: n,
            props: p,
        }
    }

    // upstream: vertices.cpp VertID::operator+()
    #[allow(clippy::should_implement_trait)]
    pub fn add(&self, rhs: i32) -> VertID {
        VertID::new(self.obj_id, (self.vn as i32).wrapping_add(rhs) as u16, self.props)
    }

    // upstream: vertices.cpp VertID::operator-()
    #[allow(clippy::should_implement_trait)]
    pub fn sub(&self, rhs: i32) -> VertID {
        VertID::new(self.obj_id, (self.vn as i32).wrapping_sub(rhs) as u16, self.props)
    }

    /// Postfix `operator++(int)`; upstream returns a reference to `*this`
    /// *after* the increment.
    // upstream: vertices.cpp VertID::operator++(int)
    pub fn post_inc(&mut self) -> &mut VertID {
        self.vn = self.vn.wrapping_add(1);
        self
    }

    /// Formatted like upstream `VertID::print()`: `[%u,%d, p=%u]`.
    // upstream: vertices.cpp VertID::print()
    pub fn print_string(&self) -> String {
        format!("[{},{}, p={}]", self.obj_id, self.vn, self.props)
    }

    /// `print()` through `db_printf` (only with the `libavoid-debug`
    /// feature, upstream's `LIBAVOID_DEBUG`).
    // upstream: vertices.cpp VertID::db_print()
    pub fn db_print(&self) {
        crate::avoid::debug::db_printf!("[{},{}, p={}]", self.obj_id, self.vn, self.props);
    }

    // upstream: vertices.h VertID::isOrthShapeEdge()
    #[inline]
    pub fn is_orth_shape_edge(&self) -> bool {
        (self.props & Self::PROP_ORTH_SHAPE_EDGE) != 0
    }
    // upstream: vertices.h VertID::isConnPt()
    #[inline]
    pub fn is_conn_pt(&self) -> bool {
        (self.props & Self::PROP_CONN_POINT) != 0
    }
    // upstream: vertices.h VertID::isConnectionPin()
    #[inline]
    pub fn is_connection_pin(&self) -> bool {
        (self.props & Self::PROP_CONNECTION_PIN) != 0
    }
    // upstream: vertices.h VertID::isConnCheckpoint()
    #[inline]
    pub fn is_conn_checkpoint(&self) -> bool {
        (self.props & Self::PROP_CONN_CHECKPOINT) != 0
    }
    // upstream: vertices.h VertID::isDummyPinHelper()
    #[inline]
    pub fn is_dummy_pin_helper(&self) -> bool {
        (self.props & Self::PROP_DUMMY_PIN_HELPER) != 0
    }
}

impl PartialEq for VertID {
    // upstream: vertices.cpp VertID::operator==() / operator!=()
    #[inline]
    fn eq(&self, rhs: &VertID) -> bool {
        if (self.obj_id != rhs.obj_id) || (self.vn != rhs.vn) {
            return false;
        }
        true
    }
}
impl Eq for VertID {}

impl std::hash::Hash for VertID {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.obj_id.hash(state);
        self.vn.hash(state);
    }
}

impl PartialOrd for VertID {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for VertID {
    // upstream: vertices.cpp VertID::operator<()
    fn cmp(&self, rhs: &Self) -> std::cmp::Ordering {
        (self.obj_id, self.vn).cmp(&(rhs.obj_id, rhs.vn))
    }
}

impl fmt::Display for VertID {
    // upstream: vertices.cpp operator<<(ostream&, const VertID&)
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{},{}]", self.obj_id, self.vn)
    }
}

// An ID given to all dummy vertices inserted to allow creation of the
// orthogonal visibility graph since the vertices in the orthogonal graph
// mostly do not correspond to shape corners or connector endpoints.
// upstream: vertices.h dummyOrthogID / dummyOrthogShapeID
pub const DUMMY_ORTHOG_ID: VertID = VertID::new(0, 0, 0);
pub const DUMMY_ORTHOG_SHAPE_ID: VertID = VertID::new(0, 0, VertID::PROP_ORTH_SHAPE_EDGE);

// Orthogonal visibility property flags
// upstream: vertices.h XL_EDGE ... YH_CONN
pub const XL_EDGE: u32 = 1;
pub const XL_CONN: u32 = 2;
pub const XH_EDGE: u32 = 4;
pub const XH_CONN: u32 = 8;
pub const YL_EDGE: u32 = 16;
pub const YL_CONN: u32 = 32;
pub const YH_EDGE: u32 = 64;
pub const YH_CONN: u32 = 128;

// upstream: vertices.h typedef std::set<unsigned int> ShapeSet
pub type ShapeSet = BTreeSet<u32>;
// upstream: vertices.h typedef std::map<VertID, ShapeSet> ContainsMap
pub type ContainsMap = BTreeMap<VertID, ShapeSet>;

// ---------------------------------------------------------------------------
// Per-vertex intrusive edge lists (upstream `EdgeInfList` = std::list<EdgeInf*>)
// ---------------------------------------------------------------------------

/// A position in a per-vertex [`EdgeInfList`]: link slot `end` (0 = the
/// edge's `m_pos1`, i.e. membership in `m_vert1`'s list; 1 = `m_pos2`) of
/// `edge`.  Upstream equivalent: `EdgeInfList::iterator`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct EdgePos {
    pub edge: EdgeIdx,
    pub end: EdgeEnd,
}

/// Which end of an edge an [`EdgePos`] names.  (An enum rather than a
/// number so that `Option<EdgePos>` needs no extra tag: every edge holds
/// four of them.)
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EdgeEnd {
    /// `m_pos1`: membership in `m_vert1`'s list.
    First = 0,
    /// `m_pos2`: membership in `m_vert2`'s list.
    Second = 1,
}

/// Prev/next links of one list membership of an `EdgeInf`.
#[derive(Clone, Copy, Debug, Default)]
pub struct EdgeListLink {
    pub prev: Option<EdgePos>,
    pub next: Option<EdgePos>,
}

/// Which of a vertex's three edge lists.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VertEdgeList {
    /// `visList`
    Vis,
    /// `orthogVisList`
    OrthogVis,
    /// `invisList`
    Invis,
}

/// Intrusive replacement for upstream `EdgeInfList` (`std::list<EdgeInf *>`)
/// as used for `VertInf::visList/orthogVisList/invisList`.
///
/// Iteration (forward) follows insertion semantics exactly: upstream only
/// ever inserts at `begin()`, erases by stored iterator, and sorts in place
/// (`std::list::sort`, reproduced by [`EdgeInfList::sort_by`]).
///
/// Mutating while iterating: use the cursor API and read the next position
/// *after* processing the current element (as `++it` does in C++):
/// ```ignore
/// let mut pos = router.vert_infs[v].vis_list.first();
/// while let Some(p) = pos {
///     /* ... may mutate other lists / other elements ... */
///     pos = EdgeInfList::next(&router.edge_infs, p);
/// }
/// ```
/// Erasing the *current* element is as invalid as it is upstream.
#[derive(Clone, Copy, Debug, Default)]
pub struct EdgeInfList {
    head: Option<EdgePos>,
    tail: Option<EdgePos>,
    len: u32,
}

impl EdgeInfList {
    pub const fn new() -> Self {
        EdgeInfList {
            head: None,
            tail: None,
            len: 0,
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    /// `std::list::size()`.
    #[inline]
    pub fn len(&self) -> usize {
        self.len as usize
    }

    /// `begin()` (None == `end()`).
    #[inline]
    pub fn first(&self) -> Option<EdgePos> {
        self.head
    }

    /// The edge at `begin()`.
    #[inline]
    pub fn front(&self) -> Option<EdgeIdx> {
        self.head.map(|p| p.edge)
    }

    /// The edge at `--end()`.
    #[inline]
    pub fn back(&self) -> Option<EdgeIdx> {
        self.tail.map(|p| p.edge)
    }

    /// `++it`.
    #[inline]
    pub fn next(edges: &EdgeArena, pos: EdgePos) -> Option<EdgePos> {
        edges[pos.edge].m_pos[pos.end as usize].next
    }

    /// `--it` (None if `pos` is `begin()`).
    #[inline]
    pub fn prev(edges: &EdgeArena, pos: EdgePos) -> Option<EdgePos> {
        edges[pos.edge].m_pos[pos.end as usize].prev
    }

    /// Iterates the edges in list order.  Borrows the edge arena, so the
    /// graph cannot be mutated during the iteration (use the cursor API for
    /// that).
    pub fn iter<'a>(&self, edges: &'a EdgeArena) -> EdgeInfListIter<'a> {
        EdgeInfListIter {
            edges,
            pos: self.head,
        }
    }

    /// Snapshot of the edges in list order.
    pub fn to_vec(&self, edges: &EdgeArena) -> Vec<EdgeIdx> {
        self.iter(edges).collect()
    }

    /// `insert(begin(), edge)`; `pos` is the link slot to use.
    pub fn push_front(&mut self, edges: &mut EdgeArena, pos: EdgePos) {
        let old_head = self.head;
        {
            let link = &mut edges[pos.edge].m_pos[pos.end as usize];
            link.prev = None;
            link.next = old_head;
        }
        match old_head {
            Some(h) => edges[h.edge].m_pos[h.end as usize].prev = Some(pos),
            None => self.tail = Some(pos),
        }
        self.head = Some(pos);
        self.len += 1;
    }

    /// `erase(pos)`.
    pub fn erase(&mut self, edges: &mut EdgeArena, pos: EdgePos) {
        let link = edges[pos.edge].m_pos[pos.end as usize];
        match link.prev {
            Some(p) => edges[p.edge].m_pos[p.end as usize].next = link.next,
            None => {
                debug_assert_eq!(self.head, Some(pos));
                self.head = link.next;
            }
        }
        match link.next {
            Some(n) => edges[n.edge].m_pos[n.end as usize].prev = link.prev,
            None => {
                debug_assert_eq!(self.tail, Some(pos));
                self.tail = link.prev;
            }
        }
        edges[pos.edge].m_pos[pos.end as usize] = EdgeListLink::default();
        self.len -= 1;
    }

    /// Positions in list order.
    pub fn positions(&self, edges: &EdgeArena) -> Vec<EdgePos> {
        let mut out = Vec::with_capacity(self.len as usize);
        let mut cur = self.head;
        while let Some(p) = cur {
            out.push(p);
            cur = Self::next(edges, p);
        }
        out
    }

    /// Phase 1 of an in-place sort: computes the order libstdc++'s
    /// `std::list::sort(comp)` would produce, without mutating anything.
    /// `less(a, b)` is the upstream comparator `comp(a, b)`.
    pub fn sorted_positions<F>(&self, edges: &EdgeArena, mut less: F) -> Vec<EdgePos>
    where
        F: FnMut(EdgeIdx, EdgeIdx) -> bool,
    {
        let items = self.positions(edges);
        libstdcxx_list_sort(items, &mut |a: &EdgePos, b: &EdgePos| less(a.edge, b.edge))
    }

    /// Phase 2 of an in-place sort: relinks the list into `order`, which
    /// must be a permutation of the current positions.
    pub fn relink(&mut self, edges: &mut EdgeArena, order: &[EdgePos]) {
        debug_assert_eq!(order.len(), self.len as usize);
        for (i, &p) in order.iter().enumerate() {
            let link = &mut edges[p.edge].m_pos[p.end as usize];
            link.prev = if i > 0 { Some(order[i - 1]) } else { None };
            link.next = order.get(i + 1).copied();
        }
        self.head = order.first().copied();
        self.tail = order.last().copied();
    }

    /// `std::list::sort(comp)` (libstdc++ algorithm, so the resulting order
    /// matches upstream even for comparators that are not strict weak
    /// orderings).  The comparator may only look at the edge arena; when it
    /// needs other router state use `sorted_positions` + `relink`, or
    /// `VertInf::sort_edge_list`.
    pub fn sort_by<F>(&mut self, edges: &mut EdgeArena, less: F)
    where
        F: FnMut(&EdgeArena, EdgeIdx, EdgeIdx) -> bool,
    {
        let mut less = less;
        let order = {
            let e: &EdgeArena = edges;
            self.sorted_positions(e, |a, b| less(e, a, b))
        };
        self.relink(edges, &order);
    }
}

/// Iterator over an [`EdgeInfList`].
pub struct EdgeInfListIter<'a> {
    edges: &'a EdgeArena,
    pos: Option<EdgePos>,
}

impl Iterator for EdgeInfListIter<'_> {
    type Item = EdgeIdx;
    #[inline]
    fn next(&mut self) -> Option<EdgeIdx> {
        let p = self.pos?;
        self.pos = EdgeInfList::next(self.edges, p);
        Some(p.edge)
    }
}

/// End-of-list marker of the index-linked lists below.
const LIST_NIL: u32 = u32::MAX;

/// libstdc++ `std::list<T>::merge(list& x, Comp comp)`: merges the list
/// headed by `x` into the one headed by `this`.  The lists are singly
/// linked through `next` (indices into `items`), so, as upstream's splice,
/// a merge allocates nothing; the comparisons and their order are exactly
/// those of libstdc++.
#[inline]
fn libstdcxx_list_merge<T, F>(items: &[T], next: &mut [u32], this: &mut u32, x: &mut u32, comp: &mut F)
where
    F: FnMut(&T, &T) -> bool,
{
    let mut first2 = std::mem::replace(x, LIST_NIL);
    if first2 == LIST_NIL {
        return;
    }
    let mut first1 = *this;
    // The node before first1 (LIST_NIL while first1 is the head).
    let mut prev = LIST_NIL;
    // while (first1 != last1 && first2 != last2)
    //     if (comp(*first2, *first1)) transfer first2 before first1
    //     else ++first1;
    while first1 != LIST_NIL && first2 != LIST_NIL {
        if comp(&items[first2 as usize], &items[first1 as usize]) {
            let after = next[first2 as usize];
            if prev == LIST_NIL {
                *this = first2;
            } else {
                next[prev as usize] = first2;
            }
            next[first2 as usize] = first1;
            prev = first2;
            first2 = after;
        } else {
            prev = first1;
            first1 = next[first1 as usize];
        }
    }
    // if (first2 != last2) transfer the rest of x to the end.
    if first2 != LIST_NIL {
        if prev == LIST_NIL {
            *this = first2;
        } else {
            next[prev as usize] = first2;
        }
    }
}

/// libstdc++ `std::list<T>::sort(Comp comp)` (bottom-up merge sort using
/// up to 64 buckets), reproduced step by step so the result is identical
/// for any comparator.
pub fn libstdcxx_list_sort<T, F>(items: Vec<T>, comp: &mut F) -> Vec<T>
where
    F: FnMut(&T, &T) -> bool,
{
    // Do nothing if the list has length 0 or 1.
    let n = items.len();
    if n < 2 {
        return items;
    }
    assert!(n < LIST_NIL as usize);
    let mut next: Vec<u32> = vec![LIST_NIL; n];
    let mut tmp: [u32; 64] = [LIST_NIL; 64];
    let mut fill: usize = 0;
    for i in 0..n as u32 {
        // __carry.splice(__carry.begin(), *this, begin());
        // (carry is empty here: it was swapped with an empty bucket.)
        let mut carry: u32 = i;
        let mut counter = 0;
        while counter != fill && tmp[counter] != LIST_NIL {
            // __counter->merge(__carry, __comp);
            libstdcxx_list_merge(&items, &mut next, &mut tmp[counter], &mut carry, comp);
            // __carry.swap(*__counter);
            std::mem::swap(&mut carry, &mut tmp[counter]);
            counter += 1;
        }
        // __carry.swap(*__counter);
        std::mem::swap(&mut carry, &mut tmp[counter]);
        if counter == fill {
            fill += 1;
        }
    }
    for counter in 1..fill {
        // __counter->merge(*(__counter - 1), __comp);
        let (lo, hi) = tmp.split_at_mut(counter);
        libstdcxx_list_merge(&items, &mut next, &mut hi[0], &mut lo[counter - 1], comp);
    }
    // Already in order (common for the sweep's edge list, which is sorted
    // again after every event): nothing to move.
    let mut cur = tmp[fill - 1];
    let mut k: u32 = 0;
    while cur == k {
        cur = next[cur as usize];
        k += 1;
    }
    if k as usize == n {
        return items;
    }
    let mut slots: Vec<Option<T>> = items.into_iter().map(Some).collect();
    let mut out = Vec::with_capacity(n);
    let mut cur = tmp[fill - 1];
    while cur != LIST_NIL {
        out.push(slots[cur as usize].take().expect("list node visited once"));
        cur = next[cur as usize];
    }
    debug_assert_eq!(out.len(), n);
    out
}

// ---------------------------------------------------------------------------
// VertInf
// ---------------------------------------------------------------------------

/// Upstream `VertInf **m_treeRoot`, which is either null, a pointer to a
/// shared malloc'd root cell (`makeTreeRootPointer`/`setTreeRootPointer`),
/// or (abused via a cast) a plain `VertInf *` (`setSPTFRoot`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TreeRootField {
    #[default]
    Null,
    Pointer(TreeRootPtr),
    SptfRoot(VertIdx),
}

/// Upstream `class VertInf`.
// upstream: vertices.h class VertInf
#[derive(Clone, Debug)]
pub struct VertInf {
    /// Monotonic creation serial; use for any upstream pointer ordering
    /// (`// PTR-ORDER:`).
    pub serial: u64,
    pub id: VertID,
    pub point: Point,
    pub lst_prev: Option<VertIdx>,
    pub lst_next: Option<VertIdx>,
    pub sh_prev: Option<VertIdx>,
    pub sh_next: Option<VertIdx>,
    pub vis_list: EdgeInfList,
    pub vis_list_size: u32,
    pub orthog_vis_list: EdgeInfList,
    pub orthog_vis_list_size: u32,
    pub invis_list: EdgeInfList,
    pub invis_list_size: u32,
    pub path_next: Option<VertIdx>,

    // The tree root and distance value used when computing MTSTs.
    pub m_orthogonal_partner: Option<VertIdx>,
    pub m_tree_root: TreeRootField,
    /// Uninitialised upstream; 0.0 here.
    pub sptf_dist: f64,

    pub vis_directions: ConnDirFlags,
    /// Upstream `std::list<ANode *>`; push_back/erase/iterate preserve
    /// order in a `Vec`.
    pub a_star_done_nodes: Vec<ANodeIdx>,
    pub a_star_pending_nodes: Vec<ANodeIdx>,
    /// Flags for orthogonal visibility properties, i.e., whether the line
    /// points to a shape edge, connection point or an obstacle.
    pub orthog_vis_prop_flags: u32,
}

impl VertInf {
    /// Allocates a vertex (upstream `new VertInf(router, vid, vpoint,
    /// addToRouter)`; upstream default for `add_to_router` is true).
    // upstream: vertices.cpp VertInf::VertInf()
    pub fn new(router: &mut Router, vid: VertID, vpoint: Point, add_to_router: bool) -> VertIdx {
        let mut point = vpoint;
        point.id = vid.obj_id;
        point.vn = vid.vn;
        let vert = VertInf {
            serial: router.next_serial(),
            id: vid,
            point,
            lst_prev: None,
            lst_next: None,
            sh_prev: None,
            sh_next: None,
            vis_list: EdgeInfList::new(),
            vis_list_size: 0,
            orthog_vis_list: EdgeInfList::new(),
            orthog_vis_list_size: 0,
            invis_list: EdgeInfList::new(),
            invis_list_size: 0,
            path_next: None,
            m_orthogonal_partner: None,
            m_tree_root: TreeRootField::Null,
            sptf_dist: 0.0,
            vis_directions: CONN_DIR_NONE,
            a_star_done_nodes: Vec::new(),
            a_star_pending_nodes: Vec::new(),
            orthog_vis_prop_flags: 0,
        };
        let idx = router.vert_infs.alloc(vert);

        if add_to_router {
            router.vertices.add_vertex(&mut router.vert_infs, idx);
        }
        idx
    }

    /// `pathNext` of `v` if it names a live vertex, else `None`.
    ///
    /// `pathNext` is a weak link: upstream never clears it when the vertex
    /// it points at is deleted, and reads it again at the start of the next
    /// A* search from `v` (makepath.cpp, `if (start->pathNext)`), a
    /// use-after-free.  See "Stale `pathNext`" in the `router.rs` module
    /// docs: a link to a deleted vertex reads as null.  Links to live
    /// vertices are returned unchanged, so behaviour is bit-identical with
    /// upstream wherever upstream's behaviour is defined.
    #[inline]
    pub fn live_path_next(router: &Router, v: VertIdx) -> Option<VertIdx> {
        router.vert_infs[v].path_next.filter(|&n| router.vert_infs.contains(n))
    }

    /// Frees the vertex (upstream `delete vert`).  Like upstream, this does
    /// not unlink it from `Router::vertices`; callers do that first.
    // upstream: vertices.cpp VertInf::~VertInf()
    pub fn delete(router: &mut Router, v: VertIdx) {
        debug_assert!(router.vert_infs[v].orphaned());
        router.vert_infs.free(v);
    }

    /// Returns the edge to `target` if it is a visibility neighbour.
    // upstream: vertices.cpp VertInf::hasNeighbour()
    pub fn has_neighbour(
        router: &Router,
        v: VertIdx,
        target: VertIdx,
        orthogonal: bool,
    ) -> Option<EdgeIdx> {
        let vert = &router.vert_infs[v];
        let vis_edge_list = if orthogonal {
            &vert.orthog_vis_list
        } else {
            &vert.vis_list
        };
        vis_edge_list
            .iter(&router.edge_infs)
            .find(|&edge| router.edge_infs[edge].other_vert(v) == target)
    }

    // upstream: vertices.cpp VertInf::Reset(const VertID&, const Point&)
    pub fn reset(&mut self, vid: VertID, vpoint: Point) {
        self.id = vid;
        self.point = vpoint;
        self.point.id = self.id.obj_id;
        self.point.vn = self.id.vn;
    }

    // upstream: vertices.cpp VertInf::Reset(const Point&)
    pub fn reset_point(&mut self, vpoint: Point) {
        self.point = vpoint;
        self.point.id = self.id.obj_id;
        self.point.vn = self.id.vn;
    }

    /// Returns true if this vertex is not involved in any (in)visibility
    /// graphs.
    // upstream: vertices.cpp VertInf::orphaned()
    #[inline]
    pub fn orphaned(&self) -> bool {
        self.vis_list.is_empty() && self.invis_list.is_empty() && self.orthog_vis_list.is_empty()
    }

    /// Upstream default for `is_conn_vert` is true.
    // upstream: vertices.cpp VertInf::removeFromGraph()
    pub fn remove_from_graph(router: &mut Router, v: VertIdx, is_conn_vert: bool) {
        if is_conn_vert {
            debug_assert!(router.vert_infs[v].id.is_conn_pt());
        }

        // For each vertex.
        while let Some(edge) = router.vert_infs[v].vis_list.front() {
            // Remove each visibility edge
            EdgeInf::alert_conns(router, edge);
            EdgeInf::delete(router, edge);
        }

        while let Some(edge) = router.vert_infs[v].orthog_vis_list.front() {
            // Remove each orthogonal visibility edge.
            EdgeInf::alert_conns(router, edge);
            EdgeInf::delete(router, edge);
        }

        while let Some(edge) = router.vert_infs[v].invis_list.front() {
            // Remove each invisibility edge
            EdgeInf::delete(router, edge);
        }
    }

    // upstream: vertices.cpp VertInf::orphan()
    pub fn orphan(router: &mut Router, v: VertIdx) {
        // For each vertex.
        while let Some(edge) = router.vert_infs[v].vis_list.front() {
            // Remove each visibility edge
            EdgeInf::make_inactive(router, edge);
        }

        while let Some(edge) = router.vert_infs[v].orthog_vis_list.front() {
            // Remove each orthogonal visibility edge.
            EdgeInf::make_inactive(router, edge);
        }

        while let Some(edge) = router.vert_infs[v].invis_list.front() {
            // Remove each invisibility edge
            EdgeInf::make_inactive(router, edge);
        }
    }

    /// Returns the direction of this vertex relative to the other specified
    /// vertex.
    // upstream: vertices.cpp VertInf::directionFrom()
    pub fn direction_from(&self, other: &VertInf) -> ConnDirFlags {
        let epsilon = 0.000001;
        let this_point = self.point;
        let other_point = other.point;
        let diff = this_point - other_point;

        let mut directions = CONN_DIR_NONE;
        if diff.y > epsilon {
            directions |= CONN_DIR_UP;
        }
        if diff.y < -epsilon {
            directions |= CONN_DIR_DOWN;
        }
        if diff.x > epsilon {
            directions |= CONN_DIR_RIGHT;
        }
        if diff.x < -epsilon {
            directions |= CONN_DIR_LEFT;
        }
        directions
    }

    /// Given a set of directions, mark visibility edges in all other
    /// directions as being invalid so they get ignored during the search.
    // upstream: vertices.cpp VertInf::setVisibleDirections()
    pub fn set_visible_directions(router: &mut Router, v: VertIdx, directions: ConnDirFlags) {
        for which in [VertEdgeList::Vis, VertEdgeList::OrthogVis] {
            let mut pos = router.vert_infs[v].edge_list(which).first();
            while let Some(p) = pos {
                let edge = p.edge;
                if directions == CONN_DIR_ALL {
                    router.edge_infs[edge].set_disabled(false);
                } else {
                    let other_vert = router.edge_infs[edge].other_vert(v);
                    let direction =
                        router.vert_infs[other_vert].direction_from(&router.vert_infs[v]);
                    let visible = (direction & directions) != 0;
                    router.edge_infs[edge].set_disabled(!visible);
                }
                pos = EdgeInfList::next(&router.edge_infs, p);
            }
        }
    }

    /// Number of points in path from end back to start, or zero if no path
    /// exists.
    // upstream: vertices.cpp VertInf::pathLeadsBackTo()
    pub fn path_leads_back_to(verts: &VertArena, this: VertIdx, start: Option<VertIdx>) -> u32 {
        let mut pathlen: u32 = 1;
        let mut i = Some(this);
        while i != start {
            if (pathlen > 1) && (i == Some(this)) {
                // We have a circular path, so path not found.
                return 0;
            }

            pathlen += 1;
            let cur = match i {
                None => {
                    // Path not found.
                    return 0;
                }
                Some(cur) => cur,
            };

            // Check we don't have an apparent infinite connector path.
            debug_assert!(pathlen < 20000);
            i = verts[cur].path_next;
        }
        pathlen
    }

    /// Allocates a new shared root cell holding `root` and points this
    /// vertex at it.
    // upstream: vertices.cpp VertInf::makeTreeRootPointer()
    pub fn make_tree_root_pointer(
        router: &mut Router,
        v: VertIdx,
        root: Option<VertIdx>,
    ) -> TreeRootPtr {
        let ptr = router.tree_root_cells.alloc(root);
        router.vert_infs[v].m_tree_root = TreeRootField::Pointer(ptr);
        ptr
    }

    /// Releases a cell from `make_tree_root_pointer` (upstream `free()` in
    /// mtst.cpp).
    pub fn free_tree_root_pointer(router: &mut Router, ptr: TreeRootPtr) {
        router.tree_root_cells.free(ptr);
    }

    // upstream: vertices.cpp VertInf::treeRoot()
    pub fn tree_root(router: &Router, v: VertIdx) -> Option<VertIdx> {
        match router.vert_infs[v].m_tree_root {
            TreeRootField::Null => None,
            TreeRootField::Pointer(p) => router.tree_root_cells[p],
            TreeRootField::SptfRoot(_) => {
                panic!("VertInf::treeRoot() called on a vertex holding an SPTF root")
            }
        }
    }

    // upstream: vertices.cpp VertInf::treeRootPointer()
    pub fn tree_root_pointer(&self) -> Option<TreeRootPtr> {
        match self.m_tree_root {
            TreeRootField::Null => None,
            TreeRootField::Pointer(p) => Some(p),
            TreeRootField::SptfRoot(_) => {
                panic!("VertInf::treeRootPointer() called on a vertex holding an SPTF root")
            }
        }
    }

    // upstream: vertices.cpp VertInf::clearTreeRootPointer()
    pub fn clear_tree_root_pointer(&mut self) {
        self.m_tree_root = TreeRootField::Null;
    }

    // upstream: vertices.cpp VertInf::setTreeRootPointer()
    pub fn set_tree_root_pointer(&mut self, pointer: Option<TreeRootPtr>) {
        self.m_tree_root = match pointer {
            Some(p) => TreeRootField::Pointer(p),
            None => TreeRootField::Null,
        };
    }

    // upstream: vertices.cpp VertInf::setSPTFRoot()
    pub fn set_sptf_root(&mut self, root: Option<VertIdx>) {
        // Use the m_treeRoot instance var, but as just a normal VertInf pointer.
        self.m_tree_root = match root {
            Some(r) => TreeRootField::SptfRoot(r),
            None => TreeRootField::Null,
        };
    }

    // upstream: vertices.cpp VertInf::sptfRoot()
    pub fn sptf_root(&self) -> Option<VertIdx> {
        match self.m_tree_root {
            TreeRootField::Null => None,
            TreeRootField::SptfRoot(r) => Some(r),
            TreeRootField::Pointer(_) => {
                panic!("VertInf::sptfRoot() called on a vertex holding a tree root pointer")
            }
        }
    }

    /// Accessor for one of the three per-vertex edge lists.
    #[inline]
    pub fn edge_list(&self, which: VertEdgeList) -> &EdgeInfList {
        match which {
            VertEdgeList::Vis => &self.vis_list,
            VertEdgeList::OrthogVis => &self.orthog_vis_list,
            VertEdgeList::Invis => &self.invis_list,
        }
    }

    #[inline]
    pub fn edge_list_mut(&mut self, which: VertEdgeList) -> &mut EdgeInfList {
        match which {
            VertEdgeList::Vis => &mut self.vis_list,
            VertEdgeList::OrthogVis => &mut self.orthog_vis_list,
            VertEdgeList::Invis => &mut self.invis_list,
        }
    }

    /// In-place `std::list::sort(comp)` of one of `v`'s edge lists, with a
    /// comparator that may read the whole router (e.g. makepath.cpp's
    /// `visList.sort(CmpVisEdgeRotation(prevInf))`).
    pub fn sort_edge_list<F>(router: &mut Router, v: VertIdx, which: VertEdgeList, mut less: F)
    where
        F: FnMut(&Router, EdgeIdx, EdgeIdx) -> bool,
    {
        let order = {
            let r: &Router = router;
            r.vert_infs[v]
                .edge_list(which)
                .sorted_positions(&r.edge_infs, |a, b| less(r, a, b))
        };
        let Router {
            vert_infs,
            edge_infs,
            ..
        } = router;
        vert_infs[v].edge_list_mut(which).relink(edge_infs, &order);
    }
}

// upstream: vertices.cpp directVis()
pub fn direct_vis(router: &mut Router, src: VertIdx, dst: VertIdx) -> bool {
    let mut ss = ShapeSet::new();

    let p = router.vert_infs[src].point;
    let q = router.vert_infs[dst].point;

    let p_id = router.vert_infs[src].id;
    let q_id = router.vert_infs[dst].id;

    // (upstream asserts both vertices belong to the same router.)

    let contains = &mut router.contains;
    if p_id.is_conn_pt() {
        // `contains[pID]` default-inserts, as upstream operator[] does.
        ss.extend(contains.entry(p_id).or_default().iter().copied());
    }
    if q_id.is_conn_pt() {
        ss.extend(contains.entry(q_id).or_default().iter().copied());
    }

    // The "beginning" should be the first shape vertex, rather
    // than an endpoint, which are also stored in "vertices".
    let end_vert = router.vertices.end();
    let mut k = router.vertices.shapes_begin();
    while k != end_vert {
        let kv = k.unwrap();
        let k_inf = &router.vert_infs[kv];
        if !ss.contains(&k_inf.id.obj_id) {
            let k_next = k_inf.sh_next.expect("shape vertex without shNext");
            if segment_intersect(&p, &q, &k_inf.point, &router.vert_infs[k_next].point) {
                return false;
            }
        }
        k = k_inf.lst_next;
    }
    true
}

// ---------------------------------------------------------------------------
// VertInfList
// ---------------------------------------------------------------------------

/// A linked list of all the vertices in the router instance.  All the
/// connector endpoints are listed first, then all the shape vertices.
/// Dummy vertices inserted for orthogonal routing are classed as shape
/// vertices but have VertID(0, 0).
///
/// Links live in `VertInf::lst_prev/lst_next`; methods take the vertex
/// arena explicitly (`router.vertices.add_vertex(&mut router.vert_infs, v)`).
/// Iterate with
/// `let mut k = router.vertices.conns_begin(); while let Some(v) = k { ...; k = router.vert_infs[v].lst_next; }`.
// upstream: vertices.h class VertInfList
#[derive(Clone, Debug, Default)]
pub struct VertInfList {
    _first_shape_vert: Option<VertIdx>,
    _first_conn_vert: Option<VertIdx>,
    _last_shape_vert: Option<VertIdx>,
    _last_conn_vert: Option<VertIdx>,
    _shape_vertices: u32,
    _conn_vertices: u32,
}

impl VertInfList {
    // upstream: vertices.cpp VertInfList::VertInfList()
    pub fn new() -> Self {
        Self::default()
    }

    // upstream: vertices.cpp checkVertInfListConditions() (macro)
    fn check_vert_inf_list_conditions(&self, verts: &VertArena) {
        if !cfg!(debug_assertions) {
            return;
        }
        debug_assert!(
            (self._first_conn_vert.is_none() && (self._conn_vertices == 0))
                || ((verts[self._first_conn_vert.unwrap()].lst_prev.is_none())
                    && (self._conn_vertices > 0))
        );
        debug_assert!(
            (self._first_shape_vert.is_none() && (self._shape_vertices == 0))
                || ((verts[self._first_shape_vert.unwrap()].lst_prev.is_none())
                    && (self._shape_vertices > 0))
        );
        debug_assert!(self
            ._last_shape_vert
            .is_none_or(|v| verts[v].lst_next.is_none()));
        debug_assert!(self
            ._last_conn_vert
            .is_none_or(|v| verts[v].lst_next == self._first_shape_vert));
        debug_assert!(self._first_conn_vert.is_some() == self._last_conn_vert.is_some());
        debug_assert!(self._first_shape_vert.is_some() == self._last_shape_vert.is_some());
        debug_assert!(self
            ._first_shape_vert
            .is_none_or(|v| !verts[v].id.is_conn_pt()));
        debug_assert!(self
            ._last_shape_vert
            .is_none_or(|v| !verts[v].id.is_conn_pt()));
        debug_assert!(self
            ._first_conn_vert
            .is_none_or(|v| verts[v].id.is_conn_pt()));
        debug_assert!(self
            ._last_conn_vert
            .is_none_or(|v| verts[v].id.is_conn_pt()));
    }

    // upstream: vertices.cpp VertInfList::addVertex()
    pub fn add_vertex(&mut self, verts: &mut VertArena, vert: VertIdx) {
        self.check_vert_inf_list_conditions(verts);
        debug_assert!(verts[vert].lst_prev.is_none());
        debug_assert!(verts[vert].lst_next.is_none());

        if verts[vert].id.is_conn_pt() {
            // A Connector vertex
            if let Some(first_conn) = self._first_conn_vert {
                // Join with previous front
                verts[vert].lst_next = Some(first_conn);
                verts[first_conn].lst_prev = Some(vert);

                // Make front
                self._first_conn_vert = Some(vert);
            } else {
                // Make front and back
                self._first_conn_vert = Some(vert);
                self._last_conn_vert = Some(vert);

                // Link to front of shapes list
                verts[vert].lst_next = self._first_shape_vert;
            }
            self._conn_vertices += 1;
        } else {
            // A Shape vertex
            if let Some(last_shape) = self._last_shape_vert {
                // Join with previous back
                verts[vert].lst_prev = Some(last_shape);
                verts[last_shape].lst_next = Some(vert);

                // Make back
                self._last_shape_vert = Some(vert);
            } else {
                // Make first and last
                self._first_shape_vert = Some(vert);
                self._last_shape_vert = Some(vert);

                // Join with conns list
                if let Some(last_conn) = self._last_conn_vert {
                    debug_assert!(verts[last_conn].lst_next.is_none());

                    verts[last_conn].lst_next = Some(vert);
                }
            }
            self._shape_vertices += 1;
        }
        self.check_vert_inf_list_conditions(verts);
    }

    /// Removes a vertex from the list and returns the vertex following the
    /// removed one.
    // upstream: vertices.cpp VertInfList::removeVertex()
    pub fn remove_vertex(&mut self, verts: &mut VertArena, vert: Option<VertIdx>) -> Option<VertIdx> {
        let vert = vert?;
        // Conditions for correct data structure
        self.check_vert_inf_list_conditions(verts);

        let following = verts[vert].lst_next;

        if verts[vert].id.is_conn_pt() {
            // A Connector vertex
            if Some(vert) == self._first_conn_vert {
                if Some(vert) == self._last_conn_vert {
                    self._first_conn_vert = None;
                    self._last_conn_vert = None;
                } else {
                    // Set new first
                    self._first_conn_vert = verts[self._first_conn_vert.unwrap()].lst_next;

                    if let Some(first_conn) = self._first_conn_vert {
                        // Set previous
                        verts[first_conn].lst_prev = None;
                    }
                }
            } else if Some(vert) == self._last_conn_vert {
                // Set new last
                self._last_conn_vert = verts[self._last_conn_vert.unwrap()].lst_prev;

                // Make last point to shapes list
                let last_conn = self._last_conn_vert.unwrap();
                verts[last_conn].lst_next = self._first_shape_vert;
            } else {
                let next = verts[vert].lst_next.unwrap();
                let prev = verts[vert].lst_prev.unwrap();
                verts[next].lst_prev = verts[vert].lst_prev;
                verts[prev].lst_next = verts[vert].lst_next;
            }
            self._conn_vertices -= 1;
        } else {
            // A Shape vertex
            if Some(vert) == self._last_shape_vert {
                // Set new last
                self._last_shape_vert = verts[self._last_shape_vert.unwrap()].lst_prev;

                if Some(vert) == self._first_shape_vert {
                    self._first_shape_vert = None;
                    if let Some(last_conn) = self._last_conn_vert {
                        verts[last_conn].lst_next = None;
                    }
                }

                if let Some(last_shape) = self._last_shape_vert {
                    verts[last_shape].lst_next = None;
                }
            } else if Some(vert) == self._first_shape_vert {
                // Set new first
                self._first_shape_vert = verts[self._first_shape_vert.unwrap()].lst_next;

                // Correct the last conn vertex
                if let Some(last_conn) = self._last_conn_vert {
                    verts[last_conn].lst_next = self._first_shape_vert;
                }

                if let Some(first_shape) = self._first_shape_vert {
                    verts[first_shape].lst_prev = None;
                }
            } else {
                let next = verts[vert].lst_next.unwrap();
                let prev = verts[vert].lst_prev.unwrap();
                verts[next].lst_prev = verts[vert].lst_prev;
                verts[prev].lst_next = verts[vert].lst_next;
            }
            self._shape_vertices -= 1;
        }
        verts[vert].lst_prev = None;
        verts[vert].lst_next = None;

        self.check_vert_inf_list_conditions(verts);

        following
    }

    // upstream: vertices.cpp VertInfList::getVertexByID()
    pub fn get_vertex_by_id(&self, verts: &VertArena, id: &VertID) -> Option<VertIdx> {
        let mut search_id = *id;
        if search_id.vn == K_UNASSIGNED_VERTEX_NUMBER {
            let topbit: u32 = 1u32 << 31;
            if (search_id.obj_id & topbit) != 0 {
                search_id.obj_id &= !topbit;
                search_id.vn = VertID::SRC;
            } else {
                search_id.vn = VertID::TAR;
            }
        }
        let last = self.end();
        let mut curr = self.conns_begin();
        while curr != last {
            let c = curr.unwrap();
            if verts[c].id == search_id {
                return Some(c);
            }
            curr = verts[c].lst_next;
        }
        None
    }

    // upstream: vertices.cpp VertInfList::getVertexByPos()
    pub fn get_vertex_by_pos(&self, verts: &VertArena, p: &Point) -> Option<VertIdx> {
        let last = self.end();
        let mut curr = self.shapes_begin();
        while curr != last {
            let c = curr.unwrap();
            if verts[c].point == *p {
                return Some(c);
            }
            curr = verts[c].lst_next;
        }
        None
    }

    // upstream: vertices.cpp VertInfList::shapesBegin()
    #[inline]
    pub fn shapes_begin(&self) -> Option<VertIdx> {
        self._first_shape_vert
    }

    // upstream: vertices.cpp VertInfList::connsBegin()
    #[inline]
    pub fn conns_begin(&self) -> Option<VertIdx> {
        if self._first_conn_vert.is_some() {
            return self._first_conn_vert;
        }
        // No connector vertices
        self._first_shape_vert
    }

    // upstream: vertices.cpp VertInfList::end()
    #[inline]
    pub fn end(&self) -> Option<VertIdx> {
        None
    }

    // upstream: vertices.cpp VertInfList::connsSize()
    #[inline]
    pub fn conns_size(&self) -> u32 {
        self._conn_vertices
    }

    // upstream: vertices.cpp VertInfList::shapesSize()
    #[inline]
    pub fn shapes_size(&self) -> u32 {
        self._shape_vertices
    }

    /// Convenience: all vertices from `conns_begin()` to `end()` in list
    /// order (snapshot).
    pub fn to_vec(&self, verts: &VertArena) -> Vec<VertIdx> {
        let mut out = Vec::new();
        let mut curr = self.conns_begin();
        while let Some(c) = curr {
            out.push(c);
            curr = verts[c].lst_next;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avoid::router::{Router, POLY_LINE_ROUTING};

    fn conn(router: &mut Router, id: u32) -> VertIdx {
        VertInf::new(
            router,
            VertID::new(id, VertID::SRC, VertID::PROP_CONN_POINT),
            Point::new(id as f64, 0.0),
            true,
        )
    }

    fn shape(router: &mut Router, id: u32, vn: u16) -> VertIdx {
        VertInf::new(router, VertID::new(id, vn, 0), Point::new(id as f64, vn as f64), true)
    }

    fn order(router: &Router) -> Vec<VertIdx> {
        router.vertices.to_vec(&router.vert_infs)
    }

    #[test]
    fn vert_id_equality_and_ordering_ignore_props() {
        let a = VertID::new(3, 1, 0);
        let b = VertID::new(3, 1, VertID::PROP_CONN_POINT);
        assert_eq!(a, b);
        assert!(VertID::new(2, 9, 0) < VertID::new(3, 0, 0));
        assert!(VertID::new(3, 0, 0) < VertID::new(3, 1, 0));
        assert_eq!(DUMMY_ORTHOG_ID, DUMMY_ORTHOG_SHAPE_ID);
        assert!(DUMMY_ORTHOG_SHAPE_ID.is_orth_shape_edge());
        assert!(b.is_conn_pt() && !b.is_connection_pin());
        let c = VertID::new(1, 0, VertID::PROP_DUMMY_PIN_HELPER);
        assert_eq!(c.add(5).vn, 5);
        assert_eq!(c.add(5).props, VertID::PROP_DUMMY_PIN_HELPER);
        assert_eq!(c.sub(1).vn, u16::MAX);
        let mut d = c;
        assert_eq!(d.post_inc().vn, 1);
        assert_eq!(format!("{}", VertID::new(4, 2, 1)), "[4,2]");
        assert_eq!(VertID::new(4, 2, 1).print_string(), "[4,2, p=1]");
        // ContainsMap keyed by (objID, vn) only.
        let mut m = ContainsMap::new();
        m.entry(a).or_default().insert(7);
        assert!(m[&b].contains(&7));
    }

    #[test]
    fn vertex_point_carries_id() {
        let mut router = Router::new(POLY_LINE_ROUTING);
        let v = VertInf::new(&mut router, VertID::new(9, 3, 0), Point::new(1.0, 2.0), false);
        assert_eq!(router.vert_infs[v].point.id, 9);
        assert_eq!(router.vert_infs[v].point.vn, 3);
        router.vert_infs[v].reset_point(Point::new(5.0, 6.0));
        assert_eq!(router.vert_infs[v].point.id, 9);
        router.vert_infs[v].reset(VertID::new(1, 1, 0), Point::new(0.0, 0.0));
        assert_eq!(router.vert_infs[v].point.vn, 1);
        // Not added to the router list.
        assert_eq!(router.vertices.conns_begin(), None);
        assert!(router.vert_infs[v].serial < router.next_serial());
    }

    #[test]
    fn list_layout_conns_first_shapes_after() {
        let mut router = Router::new(POLY_LINE_ROUTING);
        let s1 = shape(&mut router, 10, 0);
        let c1 = conn(&mut router, 1);
        let s2 = shape(&mut router, 10, 1);
        let c2 = conn(&mut router, 2);
        let s3 = shape(&mut router, 11, 0);
        let c3 = conn(&mut router, 3);
        // Connectors are pushed to the front, shapes appended at the back.
        assert_eq!(order(&router), vec![c3, c2, c1, s1, s2, s3]);
        assert_eq!(router.vertices.conns_begin(), Some(c3));
        assert_eq!(router.vertices.shapes_begin(), Some(s1));
        assert_eq!(router.vertices.conns_size(), 3);
        assert_eq!(router.vertices.shapes_size(), 3);
        // Upstream quirk: first shape vertex keeps lstPrev == nullptr even
        // though the last connector links forward to it.
        assert_eq!(router.vert_infs[s1].lst_prev, None);
        assert_eq!(router.vert_infs[c1].lst_next, Some(s1));

        // Remove: middle conn, last conn, first conn, middle/first/last shape.
        let f = router.vertices.remove_vertex(&mut router.vert_infs, Some(c2));
        assert_eq!(f, Some(c1));
        assert_eq!(order(&router), vec![c3, c1, s1, s2, s3]);
        let f = router.vertices.remove_vertex(&mut router.vert_infs, Some(c1));
        assert_eq!(f, Some(s1));
        assert_eq!(order(&router), vec![c3, s1, s2, s3]);
        let f = router.vertices.remove_vertex(&mut router.vert_infs, Some(s1));
        assert_eq!(f, Some(s2));
        assert_eq!(order(&router), vec![c3, s2, s3]);
        assert_eq!(router.vert_infs[c3].lst_next, Some(s2));
        let f = router.vertices.remove_vertex(&mut router.vert_infs, Some(s3));
        assert_eq!(f, None);
        assert_eq!(order(&router), vec![c3, s2]);
        let f = router.vertices.remove_vertex(&mut router.vert_infs, Some(s2));
        assert_eq!(f, None);
        assert_eq!(order(&router), vec![c3]);
        assert_eq!(router.vert_infs[c3].lst_next, None);
        assert_eq!(router.vertices.shapes_begin(), None);
        // Re-adding a shape relinks after the connectors.
        let s4 = shape(&mut router, 12, 0);
        assert_eq!(order(&router), vec![c3, s4]);
        router.vertices.remove_vertex(&mut router.vert_infs, Some(c3));
        assert_eq!(order(&router), vec![s4]);
        assert_eq!(router.vertices.conns_begin(), Some(s4));
        assert_eq!(router.vertices.remove_vertex(&mut router.vert_infs, None), None);
    }

    #[test]
    fn lookup_by_id_and_pos() {
        let mut router = Router::new(POLY_LINE_ROUTING);
        let c_src = conn(&mut router, 5);
        let c_tar = VertInf::new(
            &mut router,
            VertID::new(5, VertID::TAR, VertID::PROP_CONN_POINT),
            Point::new(7.0, 7.0),
            true,
        );
        let s = shape(&mut router, 20, 1);
        let vs = &router.vert_infs;
        // vn == kUnassignedVertexNumber: top bit selects src, else tar.
        let q_src = VertID::new(5 | (1 << 31), K_UNASSIGNED_VERTEX_NUMBER, 0);
        let q_tar = VertID::new(5, K_UNASSIGNED_VERTEX_NUMBER, 0);
        assert_eq!(router.vertices.get_vertex_by_id(vs, &q_src), Some(c_src));
        assert_eq!(router.vertices.get_vertex_by_id(vs, &q_tar), Some(c_tar));
        assert_eq!(router.vertices.get_vertex_by_id(vs, &VertID::new(20, 1, 0)), Some(s));
        // getVertexByPos only searches shape vertices.
        assert_eq!(router.vertices.get_vertex_by_pos(vs, &Point::new(7.0, 7.0)), None);
        assert_eq!(router.vertices.get_vertex_by_pos(vs, &Point::new(20.0, 1.0)), Some(s));
    }

    #[test]
    fn path_leads_back_and_tree_roots() {
        let mut router = Router::new(POLY_LINE_ROUTING);
        let a = shape(&mut router, 1, 0);
        let b = shape(&mut router, 1, 1);
        let c = shape(&mut router, 1, 2);
        router.vert_infs[c].path_next = Some(b);
        router.vert_infs[b].path_next = Some(a);
        assert_eq!(VertInf::path_leads_back_to(&router.vert_infs, c, Some(a)), 3);
        assert_eq!(VertInf::path_leads_back_to(&router.vert_infs, c, Some(c)), 1);
        // Path ends at null before reaching start.
        let d = shape(&mut router, 1, 3);
        assert_eq!(VertInf::path_leads_back_to(&router.vert_infs, c, Some(d)), 0);
        // Cycle.
        router.vert_infs[a].path_next = Some(c);
        assert_eq!(VertInf::path_leads_back_to(&router.vert_infs, c, Some(d)), 0);

        let ptr = VertInf::make_tree_root_pointer(&mut router, a, Some(a));
        router.vert_infs[b].set_tree_root_pointer(Some(ptr));
        assert_eq!(VertInf::tree_root(&router, b), Some(a));
        router.tree_root_cells[ptr] = None; // *oldTreeRootPtr = nullptr
        assert_eq!(VertInf::tree_root(&router, a), None);
        assert_eq!(router.vert_infs[b].tree_root_pointer(), Some(ptr));
        router.vert_infs[b].clear_tree_root_pointer();
        assert_eq!(VertInf::tree_root(&router, b), None);
        VertInf::free_tree_root_pointer(&mut router, ptr);

        router.vert_infs[c].set_sptf_root(Some(a));
        assert_eq!(router.vert_infs[c].sptf_root(), Some(a));
    }

    #[test]
    fn direction_from_flags() {
        let mut router = Router::new(POLY_LINE_ROUTING);
        let o = VertInf::new(&mut router, VertID::new(1, 0, 0), Point::new(0.0, 0.0), false);
        let p = VertInf::new(&mut router, VertID::new(1, 1, 0), Point::new(1.0, 1.0), false);
        let vs = &router.vert_infs;
        assert_eq!(vs[p].direction_from(&vs[o]), CONN_DIR_UP | CONN_DIR_RIGHT);
        assert_eq!(vs[o].direction_from(&vs[p]), CONN_DIR_DOWN | CONN_DIR_LEFT);
        assert_eq!(vs[o].direction_from(&vs[o]), CONN_DIR_NONE);
    }

    #[test]
    fn libstdcxx_sort_matches_reference_merge_order() {
        // Stable for a strict weak ordering.
        let v: Vec<(i32, usize)> = [3, 1, 2, 1, 3, 0, 2, 1, 0, 3, 2]
            .iter()
            .copied()
            .enumerate()
            .map(|(i, k)| (k, i))
            .collect();
        let sorted =
            libstdcxx_list_sort(v.clone(), &mut |a: &(i32, usize), b: &(i32, usize)| a.0 < b.0);
        let mut expect = v;
        expect.sort_by_key(|x| x.0);
        assert_eq!(sorted, expect);
        // Non-transitive comparator: deterministic permutation.
        let w: Vec<i32> = (0..17).collect();
        let mut odd = |a: &i32, b: &i32| (a * 7 + b * 3) % 5 < 2;
        let r1 = libstdcxx_list_sort(w.clone(), &mut odd);
        let r2 = libstdcxx_list_sort(w.clone(), &mut odd);
        assert_eq!(r1, r2);
        let mut sorted_r1 = r1.clone();
        sorted_r1.sort();
        assert_eq!(sorted_r1, w);
    }

    /// The straightforward transcription of libstdc++'s `list::sort()` on
    /// vectors (one vector per `std::list`), kept as the reference for the
    /// index-linked implementation.
    fn reference_list_sort<T, F: FnMut(&T, &T) -> bool>(items: Vec<T>, comp: &mut F) -> Vec<T> {
        fn merge<T, F: FnMut(&T, &T) -> bool>(this: &mut Vec<T>, x: &mut Vec<T>, comp: &mut F) {
            let a = std::mem::take(this);
            let b = std::mem::take(x);
            let mut out = Vec::with_capacity(a.len() + b.len());
            let mut ia = a.into_iter().peekable();
            let mut ib = b.into_iter().peekable();
            while let (Some(f1), Some(f2)) = (ia.peek(), ib.peek()) {
                if comp(f2, f1) {
                    out.push(ib.next().unwrap());
                } else {
                    out.push(ia.next().unwrap());
                }
            }
            out.extend(ia);
            out.extend(ib);
            *this = out;
        }
        if items.len() < 2 {
            return items;
        }
        let mut input = std::collections::VecDeque::from(items);
        let mut carry: Vec<T> = Vec::new();
        let mut tmp: Vec<Vec<T>> = (0..64).map(|_| Vec::new()).collect();
        let mut fill = 0;
        while let Some(front) = input.pop_front() {
            carry.insert(0, front);
            let mut counter = 0;
            while counter != fill && !tmp[counter].is_empty() {
                merge(&mut tmp[counter], &mut carry, comp);
                std::mem::swap(&mut carry, &mut tmp[counter]);
                counter += 1;
            }
            std::mem::swap(&mut carry, &mut tmp[counter]);
            if counter == fill {
                fill += 1;
            }
        }
        for counter in 1..fill {
            let (lo, hi) = tmp.split_at_mut(counter);
            merge(&mut hi[0], &mut lo[counter - 1], comp);
        }
        std::mem::take(&mut tmp[fill - 1])
    }

    #[test]
    fn libstdcxx_sort_same_comparisons_as_reference() {
        // Same result and the same sequence of comparator calls as the
        // reference, also for comparators that are not orderings.
        let mut seed: u32 = 12345;
        let mut rnd = move || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            seed >> 8
        };
        for round in 0..400 {
            let n = (rnd() % 70) as usize;
            let v: Vec<(u32, usize)> = (0..n).map(|i| (rnd() % 9, i)).collect();
            let kind = round % 3;
            let less = move |a: &(u32, usize), b: &(u32, usize)| match kind {
                0 => a.0 < b.0,
                1 => (a.0 * 7 + b.0 * 3 + a.1 as u32) % 5 < 2,
                _ => a.0 <= b.0,
            };
            let mut calls_new = Vec::new();
            let got = libstdcxx_list_sort(v.clone(), &mut |a: &(u32, usize), b: &(u32, usize)| {
                calls_new.push((a.1, b.1));
                less(a, b)
            });
            let mut calls_ref = Vec::new();
            let expect = reference_list_sort(v, &mut |a: &(u32, usize), b: &(u32, usize)| {
                calls_ref.push((a.1, b.1));
                less(a, b)
            });
            assert_eq!(got, expect, "round {round}");
            assert_eq!(calls_new, calls_ref, "round {round}");
        }
    }
}
