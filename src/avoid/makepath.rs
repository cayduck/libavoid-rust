//! Faithful port of upstream `libavoid/makepath.{h,cpp}`: the A* search
//! (`AStarPath`) used to route a connector through the visibility graph,
//! with its cost function and heuristics.
//!
//! Port notes:
//! * `ANode`s are allocated in blocks of 5000 upstream and never freed
//!   until the `AStarPath` is destroyed; here they live in a `Vec<ANode>`
//!   owned by the search object and are named by [`ANodeIdx`] (index into
//!   that vector).  Node identity comparisons (`node.prevNode ==
//!   ati.prevNode`) compare indices, which is equivalent.
//! * The `PENDING` heap is a `Vec<ANodeIdx>` manipulated with exact ports of
//!   libstdc++'s `std::make_heap` / `std::push_heap` / `std::pop_heap`
//!   (see [`make_heap`], [`push_heap`], [`pop_heap`]), because the order in
//!   which nodes of (epsilon-)equal cost are popped depends on the precise
//!   sift sequence.
//! * `VertInf::aStarPendingNodes` / `aStarDoneNodes` (`std::list<ANode *>`)
//!   are `Vec<ANodeIdx>` on the vertex; they are only appended to, erased
//!   from (first match) and iterated, and are cleared after every search.

#![allow(dead_code)]
#![allow(clippy::new_without_default)]
#![allow(
    clippy::collapsible_if,
    clippy::collapsible_else_if,
    clippy::too_many_arguments,
    clippy::nonminimal_bool,
    clippy::if_same_then_else,
    clippy::needless_range_loop
)]

use std::f64::consts::PI as M_PI;

use crate::avoid::arena::{ANodeIdx, ConnIdx, EdgeIdx, VertIdx};
use crate::avoid::connector::{
    split_branching_segments_from, validate_bend_point, ConnType, ConnectorCrossings, RouteBounds, CROSSING_SHARES_FIXED_SEGMENT, CROSSING_SHARES_PATH,
    CROSSING_SHARES_PATH_AT_END,
};
use crate::avoid::geometry::{euclidean_dist, manhattan_dist, point_on_line, vec_dir};
use crate::avoid::geomtypes::{cpp_min, Point, Polygon, PolygonInterface, XDIM, YDIM};
use crate::avoid::router::{Router, RoutingOption, RoutingParameter};
use crate::avoid::vertices::{VertID, VertIDProps, VertInf, XH_EDGE, XL_EDGE, YH_EDGE, YL_EDGE};

// upstream: makepath.cpp class ANode
#[derive(Clone, Copy, Debug)]
struct ANode {
    inf: Option<VertIdx>,
    /// Gone
    g: f64,
    /// Heuristic
    h: f64,
    /// Formula f = g + h
    f: f64,

    /// VertInf for the previous ANode.
    prev_node: Option<ANodeIdx>,
    /// Time-stamp used to determine exploration order of seemingly equal
    /// paths during orthogonal routing.
    time_stamp: i32,
}

impl ANode {
    // upstream: makepath.cpp ANode::ANode(VertInf *, int)
    fn new(vinf: VertIdx, time: i32) -> ANode {
        ANode {
            inf: Some(vinf),
            g: 0.0,
            h: 0.0,
            f: 0.0,
            prev_node: None,
            time_stamp: time,
        }
    }
}

impl Default for ANode {
    // upstream: makepath.cpp ANode::ANode()
    fn default() -> ANode {
        ANode {
            inf: None,
            g: 0.0,
            h: 0.0,
            f: 0.0,
            prev_node: None,
            time_stamp: -1,
        }
    }
}

/// Node storage (upstream: the ANode memory blocks).
type ANodes = Vec<ANode>;

#[inline]
fn inf_of(nodes: &ANodes, n: ANodeIdx) -> VertIdx {
    nodes[n.0 as usize].inf.expect("ANode without VertInf")
}

/// This returns the opposite result (>) so that when used with
/// stl::make_heap, the head node of the heap will be the smallest value,
/// rather than the largest.
// upstream: makepath.cpp class ANodeCmp::operator()
fn a_node_cmp(a: &ANode, b: &ANode) -> bool {
    // We need to use an epsilon here since otherwise the multiple addition
    // of floating point numbers that makes up the 'f' values cause a problem
    // with routings occasionally being non-deterministic.
    if (a.f - b.f).abs() > 0.0000001 {
        return a.f > b.f;
    }
    if a.time_stamp != b.time_stamp {
        // Tiebreaker, if two paths have equal cost, then choose the one with
        // the highest timeStamp.  This corresponds to the furthest point
        // explored along the straight-line path.  When exploring we give the
        // directions the following timeStamps; left:1, right:2 and forward:3,
        // then we always try to explore forward first.
        return a.time_stamp < b.time_stamp;
    }
    false
}

// ---------------------------------------------------------------------------
// libstdc++ heap algorithms (bits/stl_heap.h), ported step by step.
// ---------------------------------------------------------------------------

/// Element storage for the heap algorithms below (not upstream): a plain
/// slice, or the A* pending heap, which also tracks where each node is.
trait HeapSlots<T: Copy> {
    fn get(&self, i: usize) -> T;
    fn set(&mut self, i: usize, x: T);
}

impl<T: Copy> HeapSlots<T> for [T] {
    #[inline]
    fn get(&self, i: usize) -> T {
        self[i]
    }
    #[inline]
    fn set(&mut self, i: usize, x: T) {
        self[i] = x;
    }
}

/// libstdc++ `std::__push_heap`.
fn libstdcxx_push_heap_hole<T: Copy, S: HeapSlots<T> + ?Sized, F: FnMut(&T, &T) -> bool>(
    v: &mut S,
    mut hole_index: isize,
    top_index: isize,
    value: T,
    comp: &mut F,
) {
    let mut parent = (hole_index - 1) / 2;
    while hole_index > top_index && comp(&v.get(parent as usize), &value) {
        let p = v.get(parent as usize);
        v.set(hole_index as usize, p);
        hole_index = parent;
        parent = (hole_index - 1) / 2;
    }
    v.set(hole_index as usize, value);
}

/// libstdc++ `std::__adjust_heap`.
fn libstdcxx_adjust_heap<T: Copy, S: HeapSlots<T> + ?Sized, F: FnMut(&T, &T) -> bool>(
    v: &mut S,
    mut hole_index: isize,
    len: isize,
    value: T,
    comp: &mut F,
) {
    let top_index = hole_index;
    let mut second_child = hole_index;
    while second_child < (len - 1) / 2 {
        second_child = 2 * (second_child + 1);
        if comp(&v.get(second_child as usize), &v.get((second_child - 1) as usize)) {
            second_child -= 1;
        }
        let c = v.get(second_child as usize);
        v.set(hole_index as usize, c);
        hole_index = second_child;
    }
    if (len & 1) == 0 && second_child == (len - 2) / 2 {
        second_child = 2 * (second_child + 1);
        let c = v.get((second_child - 1) as usize);
        v.set(hole_index as usize, c);
        hole_index = second_child - 1;
    }
    libstdcxx_push_heap_hole(v, hole_index, top_index, value, comp);
}

/// libstdc++ `std::make_heap` on the first `len` slots.
fn libstdcxx_make_heap<T: Copy, S: HeapSlots<T> + ?Sized, F: FnMut(&T, &T) -> bool>(
    v: &mut S,
    len: usize,
    comp: &mut F,
) {
    let len = len as isize;
    if len < 2 {
        return;
    }
    let mut parent = (len - 2) / 2;
    loop {
        let value = v.get(parent as usize);
        libstdcxx_adjust_heap(v, parent, len, value, comp);
        if parent == 0 {
            return;
        }
        parent -= 1;
    }
}

/// libstdc++ `std::make_heap(first, last, comp)`.
pub fn make_heap<T: Copy, F: FnMut(&T, &T) -> bool>(v: &mut [T], comp: &mut F) {
    let len = v.len();
    libstdcxx_make_heap(v, len, comp);
}

/// libstdc++ `std::push_heap(first, last, comp)` (the new element is the
/// last one of `v`).
pub fn push_heap<T: Copy, F: FnMut(&T, &T) -> bool>(v: &mut [T], comp: &mut F) {
    let len = v.len() as isize;
    if len == 0 {
        return;
    }
    let value = v[(len - 1) as usize];
    libstdcxx_push_heap_hole(v, len - 1, 0, value, comp);
}

/// libstdc++ `std::pop_heap(first, last, comp)` on the first `len` slots:
/// moves the top to the last position and re-heapifies the rest.
fn libstdcxx_pop_heap<T: Copy, S: HeapSlots<T> + ?Sized, F: FnMut(&T, &T) -> bool>(
    v: &mut S,
    len: usize,
    comp: &mut F,
) {
    let len = len as isize;
    if len > 1 {
        let last = len - 1;
        // __pop_heap(first, last, last, comp)
        let value = v.get(last as usize);
        let top = v.get(0);
        v.set(last as usize, top);
        libstdcxx_adjust_heap(v, 0, last, value, comp);
    }
}

/// libstdc++ `std::pop_heap(first, last, comp)`: moves the top to the last
/// position and re-heapifies the rest.
pub fn pop_heap<T: Copy, F: FnMut(&T, &T) -> bool>(v: &mut [T], comp: &mut F) {
    let len = v.len();
    libstdcxx_pop_heap(v, len, comp);
}

/// The A* PENDING heap (not upstream as a type): upstream's vector of
/// nodes kept with `std::make_heap` / `push_heap` / `pop_heap`, which this
/// reproduces exactly, plus what lets a node's key change cost less than
/// upstream's full `make_heap()` (see `key_changed()`).
#[derive(Debug, Default)]
struct PendingHeap {
    v: Vec<ANodeIdx>,
    /// Position in `v` of each node (`u32::MAX`: not in the heap).
    pos: Vec<u32>,
    /// The `f` values in the heap.
    keys: KeyTies,
    /// Every heap operation since the last full `make_heap()` ran while the
    /// comparison was a strict total order (see `KeyTies`), so that every
    /// parent orders strictly before its children.
    ordered: bool,
}

/// The heap's slots as seen by the heap algorithms: writes update `pos`.
struct TrackedSlots<'a> {
    v: &'a mut [ANodeIdx],
    pos: &'a mut [u32],
}

impl HeapSlots<ANodeIdx> for TrackedSlots<'_> {
    #[inline]
    fn get(&self, i: usize) -> ANodeIdx {
        self.v[i]
    }
    #[inline]
    fn set(&mut self, i: usize, x: ANodeIdx) {
        self.v[i] = x;
        self.pos[x.0 as usize] = i as u32;
    }
}

/// The multiset of `f` values in the heap, grouped into clusters (not
/// upstream): maximal runs of distinct values in which neighbours differ by
/// no more than `a_node_cmp()`'s epsilon.  When every cluster spans no more
/// than the epsilon, `a_node_cmp()` compares values of one cluster by time
/// stamp (unique per node) and values of different clusters, which differ
/// by more than the epsilon, by `f`: a strict total order.  Otherwise it
/// may not be transitive.
#[derive(Debug, Default)]
struct KeyTies {
    counts: std::collections::BTreeMap<u64, u32>,
    /// Clusters, start -> end.
    clusters: std::collections::BTreeMap<u64, u64>,
    /// Clusters that span more than the epsilon.
    wide: usize,
    nan: usize,
}

impl KeyTies {
    const EPS: f64 = 0.0000001;

    /// f64 to u64, order-preserving (for non-NaN values).
    fn ord(f: f64) -> u64 {
        let b = f.to_bits();
        if b >> 63 == 1 {
            !b
        } else {
            b | (1 << 63)
        }
    }

    fn val(k: u64) -> f64 {
        f64::from_bits(if k >> 63 == 1 { k & !(1 << 63) } else { !k })
    }

    /// `a < b`, close enough to be in one cluster.
    fn close(a: u64, b: u64) -> bool {
        Self::val(b) - Self::val(a) <= Self::EPS
    }

    fn add_cluster(&mut self, start: u64, end: u64) {
        self.clusters.insert(start, end);
        self.wide += !Self::close(start, end) as usize;
    }

    fn remove_cluster(&mut self, start: u64) -> u64 {
        let end = self.clusters.remove(&start).expect("KeyTies: no such cluster");
        self.wide -= !Self::close(start, end) as usize;
        end
    }

    /// The start of the cluster holding (or ending at or after) key `k`.
    fn cluster_of(&self, k: u64) -> u64 {
        *self.clusters.range(..=k).next_back().expect("KeyTies: key outside clusters").0
    }

    fn insert(&mut self, f: f64) {
        if f.is_nan() {
            self.nan += 1;
            return;
        }
        let k = Self::ord(f);
        let c = self.counts.entry(k).or_insert(0);
        *c += 1;
        if *c > 1 {
            return;
        }
        let lo = self.counts.range(..k).next_back().map(|(&x, _)| x);
        let hi = self.counts.range(k + 1..).next().map(|(&x, _)| x);
        if let (Some(lo), Some(hi)) = (lo, hi) {
            if Self::close(lo, hi) {
                // Inside a cluster: it keeps its ends.
                return;
            }
        }
        let mut start = k;
        let mut end = k;
        if let Some(lo) = lo.filter(|&lo| Self::close(lo, k)) {
            start = self.cluster_of(lo);
            self.remove_cluster(start);
        }
        if let Some(hi) = hi.filter(|&hi| Self::close(k, hi)) {
            // `hi` starts its cluster (it is not close to `lo`).
            end = self.remove_cluster(hi);
        }
        self.add_cluster(start, end);
    }

    fn remove(&mut self, f: f64) {
        if f.is_nan() {
            self.nan -= 1;
            return;
        }
        let k = Self::ord(f);
        let c = self.counts.get_mut(&k).expect("KeyTies: removing an absent key");
        *c -= 1;
        if *c > 0 {
            return;
        }
        self.counts.remove(&k);
        let start = self.cluster_of(k);
        let end = self.remove_cluster(start);
        let lo = self.counts.range(..k).next_back().map(|(&x, _)| x).filter(|&lo| lo >= start);
        let hi = self.counts.range(k + 1..).next().map(|(&x, _)| x).filter(|&hi| hi <= end);
        match (lo, hi) {
            (Some(lo), Some(hi)) if Self::close(lo, hi) => self.add_cluster(start, end),
            _ => {
                if let Some(lo) = lo {
                    self.add_cluster(start, lo);
                }
                if let Some(hi) = hi {
                    self.add_cluster(hi, end);
                }
            }
        }
    }

    fn clear(&mut self) {
        self.counts.clear();
        self.clusters.clear();
        self.wide = 0;
        self.nan = 0;
    }

    /// `a_node_cmp()` is a strict total order on the values present.
    fn strict(&self) -> bool {
        self.wide == 0 && self.nan == 0
    }
}

impl PendingHeap {
    fn clear(&mut self) {
        for &n in &self.v {
            self.pos[n.0 as usize] = u32::MAX;
        }
        self.v.clear();
        self.keys.clear();
        self.ordered = false;
    }

    #[inline]
    fn len(&self) -> usize {
        self.v.len()
    }

    #[inline]
    fn top(&self) -> ANodeIdx {
        self.v[0]
    }

    fn slots(&mut self) -> TrackedSlots<'_> {
        TrackedSlots {
            v: &mut self.v,
            pos: &mut self.pos,
        }
    }

    /// `pending.push_back(n)` (without heap ordering).
    fn push_back(&mut self, nodes: &ANodes, n: ANodeIdx) {
        if self.pos.len() <= n.0 as usize {
            self.pos.resize(nodes.len().max(n.0 as usize + 1), u32::MAX);
        }
        self.pos[n.0 as usize] = self.v.len() as u32;
        self.v.push(n);
        self.keys.insert(nodes[n.0 as usize].f);
    }

    /// `std::make_heap(pending.begin(), pending.end(), ANodeCmp())`.
    fn make_heap(&mut self, nodes: &ANodes) {
        let len = self.v.len();
        let mut cmp = |a: &ANodeIdx, b: &ANodeIdx| a_node_cmp(&nodes[a.0 as usize], &nodes[b.0 as usize]);
        libstdcxx_make_heap(&mut self.slots(), len, &mut cmp);
        self.ordered = self.keys.strict();
    }

    /// `pending.push_back(n)` and `std::push_heap(...)`.
    fn push(&mut self, nodes: &ANodes, n: ANodeIdx) {
        self.push_back(nodes, n);
        let len = self.v.len();
        let value = self.v[len - 1];
        let mut cmp = |a: &ANodeIdx, b: &ANodeIdx| a_node_cmp(&nodes[a.0 as usize], &nodes[b.0 as usize]);
        libstdcxx_push_heap_hole(&mut self.slots(), len as isize - 1, 0, value, &mut cmp);
        self.ordered &= self.keys.strict();
    }

    /// `std::pop_heap(...)` and `pending.pop_back()`.
    fn pop(&mut self, nodes: &ANodes) {
        let len = self.v.len();
        let mut cmp = |a: &ANodeIdx, b: &ANodeIdx| a_node_cmp(&nodes[a.0 as usize], &nodes[b.0 as usize]);
        libstdcxx_pop_heap(&mut self.slots(), len, &mut cmp);
        self.ordered &= self.keys.strict();
        let n = self.v.pop().expect("pop of an empty heap");
        self.pos[n.0 as usize] = u32::MAX;
        self.keys.remove(nodes[n.0 as usize].f);
    }

    /// Node `n` in the heap had key `old_f` and now has `nodes[n]`'s:
    /// upstream calls `std::make_heap()` on the whole heap.
    ///
    /// When every parent orders strictly before its children under a strict
    /// total order (`ordered`, and `strict()` with the new key), each step
    /// of make_heap() -- `__adjust_heap()` at every parent position, last
    /// to first -- leaves the heap unchanged except at the changed node's
    /// position and its ancestors': elsewhere the value at a position
    /// orders before everything below it, so the hole it leaves goes down
    /// and the value comes back up the same path.  Only those steps are
    /// run then, in make_heap()'s order, which gives the same heap.
    fn key_changed(&mut self, nodes: &ANodes, n: ANodeIdx, old_f: f64) {
        self.keys.remove(old_f);
        self.keys.insert(nodes[n.0 as usize].f);
        if !(self.ordered && self.keys.strict()) {
            self.make_heap(nodes);
            return;
        }
        #[cfg(debug_assertions)]
        let expected = {
            let mut v = self.v.clone();
            make_heap(&mut v, &mut |a: &ANodeIdx, b: &ANodeIdx| {
                a_node_cmp(&nodes[a.0 as usize], &nodes[b.0 as usize])
            });
            v
        };
        let len = self.v.len() as isize;
        if len >= 2 {
            let i = self.pos[n.0 as usize] as isize;
            let mut cmp = |a: &ANodeIdx, b: &ANodeIdx| a_node_cmp(&nodes[a.0 as usize], &nodes[b.0 as usize]);
            let mut slots = self.slots();
            if i <= (len - 2) / 2 {
                let value = slots.get(i as usize);
                libstdcxx_adjust_heap(&mut slots, i, len, value, &mut cmp);
            }
            let mut a = i;
            while a > 0 {
                a = (a - 1) / 2;
                let value = slots.get(a as usize);
                libstdcxx_adjust_heap(&mut slots, a, len, value, &mut cmp);
            }
        }
        #[cfg(debug_assertions)]
        debug_assert_eq!(self.v, expected);
    }
}

// ---------------------------------------------------------------------------

// upstream: makepath.cpp Dot() (static)
fn dot(l: &Point, r: &Point) -> f64 {
    (l.x * r.x) + (l.y * r.y)
}

// upstream: makepath.cpp CrossLength() (static)
fn cross_length(l: &Point, r: &Point) -> f64 {
    (l.x * r.y) - (l.y * r.x)
}

/// Return the angle between the two line segments made by the
/// points p1--p2 and p2--p3.  Return value is in radians.
// upstream: makepath.cpp angleBetween() (static)
fn angle_between(p1: &Point, p2: &Point, p3: &Point) -> f64 {
    if (p1.x == p2.x && p1.y == p2.y) || (p2.x == p3.x && p2.y == p3.y) {
        // If two of the points are the same, then we can't say anything
        // about the angle between.  Treat them as being collinear.
        return M_PI;
    }

    let v1 = Point::new(p1.x - p2.x, p1.y - p2.y);
    let v2 = Point::new(p3.x - p2.x, p3.y - p2.y);

    cross_length(&v1, &v2).atan2(dot(&v1, &v2)).abs()
}

/// Construct a temporary Polygon path given several VertInf's for a
/// connector.
// upstream: makepath.cpp constructPolygonPath() (static)
fn construct_polygon_path(
    router: &Router,
    nodes: &ANodes,
    conn_route: &mut Polygon,
    inf2: VertIdx,
    inf3: VertIdx,
    inf1_node: Option<ANodeIdx>,
) {
    let verts = &router.vert_infs;
    // Don't include colinear points.
    let simplified = true;

    let mut route_size: isize = 2;
    let mut curr = inf1_node;
    while let Some(c) = curr {
        route_size += 1;
        curr = nodes[c.0 as usize].prev_node;
    }
    conn_route.ps.resize(route_size as usize, Point::default());
    let array_size = route_size;
    conn_route.ps[(route_size - 1) as usize] = verts[inf3].point;
    conn_route.ps[(route_size - 2) as usize] = verts[inf2].point;
    route_size -= 3;
    let mut curr = inf1_node;
    while let Some(c) = curr {
        let c_inf = inf_of(nodes, c);
        // For connection pins, we stop and don't include the fake shape
        // center as part of this path.
        let is_connection_pin = verts[c_inf].id.is_connection_pin();

        if !simplified {
            // If this is non-simplified, we don't need to do anything
            // clever and can simply add the new point.
            conn_route.ps[route_size as usize] = verts[c_inf].point;
            route_size -= 1;

            if is_connection_pin {
                // Stop at the connection pin.
                break;
            }
            curr = nodes[c.0 as usize].prev_node;
            continue;
        }

        if (Some(c) == inf1_node)
            || vec_dir(
                &verts[c_inf].point,
                &conn_route.ps[(route_size + 1) as usize],
                &conn_route.ps[(route_size + 2) as usize],
                0.0,
            ) != 0
        {
            // Add new point if this is the earlier than the last segment
            // and it is not colinear with the other points.
            // Note, you can't collapse the 'last' segment with previous
            // segments, or if this just intersects another line you risk
            // penalising it once for each collapsed line segment.
            conn_route.ps[route_size as usize] = verts[c_inf].point;
            route_size -= 1;
        } else {
            // The last point is inline with this one, so update it.
            conn_route.ps[(route_size + 1) as usize] = verts[c_inf].point;
        }

        if is_connection_pin {
            // Stop at the connection pin.
            break;
        }
        curr = nodes[c.0 as usize].prev_node;
    }

    // If the vector is not filled, move entries to the beginning and
    // remove the unused end of the vector.
    let diff = route_size + 1;
    debug_assert!(simplified || (diff == 0));
    if diff > 0 {
        for i in diff..array_size {
            conn_route.ps[(i - diff) as usize] = conn_route.ps[i as usize];
        }
        let new_len = conn_route.size() - diff as usize;
        conn_route.ps.truncate(new_len);
    }
}

/// What constructPolygonPath() reads of a node (not upstream), kept next to
/// the nodes: the vertex's point, whether it is a connection pin, and the
/// number of nodes on its path back to the start.
#[derive(Clone, Copy, Debug)]
struct NodeInfo {
    point: Point,
    is_connection_pin: bool,
    path_nodes: u32,
}

impl NodeInfo {
    fn of(router: &Router, infos: &[NodeInfo], node: &ANode) -> NodeInfo {
        let v = &router.vert_infs[node.inf.expect("ANode without VertInf")];
        NodeInfo {
            point: v.point,
            is_connection_pin: v.id.is_connection_pin(),
            path_nodes: 1 + node.prev_node.map_or(0, |p| infos[p.0 as usize].path_nodes),
        }
    }
}

/// `construct_polygon_path()` reading `NodeInfo` instead of the vertices
/// (not upstream; the same steps).
fn construct_polygon_path_from_info(
    infos: &[NodeInfo],
    nodes: &ANodes,
    conn_route: &mut Polygon,
    p2: Point,
    p3: Point,
    inf1_node: Option<ANodeIdx>,
) {
    let mut route_size: isize = 2 + inf1_node.map_or(0, |n| infos[n.0 as usize].path_nodes) as isize;
    conn_route.ps.clear();
    conn_route.ps.resize(route_size as usize, Point::default());
    let array_size = route_size;
    conn_route.ps[(route_size - 1) as usize] = p3;
    conn_route.ps[(route_size - 2) as usize] = p2;
    route_size -= 3;
    let mut curr = inf1_node;
    while let Some(c) = curr {
        let info = &infos[c.0 as usize];
        if (Some(c) == inf1_node)
            || vec_dir(
                &info.point,
                &conn_route.ps[(route_size + 1) as usize],
                &conn_route.ps[(route_size + 2) as usize],
                0.0,
            ) != 0
        {
            conn_route.ps[route_size as usize] = info.point;
            route_size -= 1;
        } else {
            conn_route.ps[(route_size + 1) as usize] = info.point;
        }
        if info.is_connection_pin {
            break;
        }
        curr = nodes[c.0 as usize].prev_node;
    }
    let diff = route_size + 1;
    if diff > 0 {
        conn_route.ps.copy_within(diff as usize..array_size as usize, 0);
        let new_len = conn_route.size() - diff as usize;
        conn_route.ps.truncate(new_len);
    }
}

/// Used to get an indication of if a diffence is positive (1),
/// negative (-1) or no different (0).
// upstream: makepath.cpp dimDirection() (static inline)
#[inline]
fn dim_direction(difference: f64) -> i32 {
    if difference > 0.0 {
        return 1;
    } else if difference < 0.0 {
        return -1;
    }
    0
}

/// Given the two points for a new segment of a path (inf2 & inf3)
/// as well as the distance between these points (dist), as well as
/// possibly the previous point (inf1) [from inf1--inf2], return a
/// cost associated with this route.
// upstream: makepath.cpp cost() (static)
fn cost(
    router: &mut Router,
    nodes: &ANodes,
    others: &mut OtherRoutes,
    cluster_boundaries: &[ClusterBoundary],
    node_info: &[NodeInfo],
    line_ref: ConnIdx,
    dist: f64,
    inf2: VertIdx,
    inf3: VertIdx,
    inf1_node: Option<ANodeIdx>,
) -> f64 {
    let is_orthogonal = router.conns[line_ref].routing_type() == ConnType::ConnTypeOrthogonal;
    let inf1 = inf1_node.map(|n| inf_of(nodes, n));
    let mut result = dist;
    let mut conn_route = Polygon::new();

    if let Some(inf1) = inf1 {
        let angle_penalty = router.routing_parameter(RoutingParameter::AnglePenalty);
        let segmt_penalty = router.routing_parameter(RoutingParameter::SegmentPenalty);

        // This is not the first segment, so there is a bend
        // between it and the last one in the existing path.
        if (angle_penalty > 0.0) || (segmt_penalty > 0.0) {
            let p1 = router.vert_infs[inf1].point;
            let p2 = router.vert_infs[inf2].point;
            let p3 = router.vert_infs[inf3].point;

            let rad = M_PI - angle_between(&p1, &p2, &p3);

            if (rad > 0.0) && !is_orthogonal {
                // Make `xval' between 0--10 then take its log so small
                // angles are not penalised as much as large ones.
                //
                let xval = rad * 10.0 / M_PI;
                let yval = xval * (xval + 1.0).log10() / 10.5;
                result += angle_penalty * yval;
            }

            if rad == M_PI {
                // Needs to double back
                result += 2.0 * segmt_penalty;
            } else if rad > 0.0 {
                // Only penalise as an extra segment if the two
                // segments are not collinear.
                result += segmt_penalty;
            }
        }
    }

    let cluster_crossing_penalty = router.routing_parameter(RoutingParameter::ClusterCrossingPenalty);
    // XXX: Clustered routing doesn't yet work with orthogonal connectors.
    if router.clustered_routing && !router.cluster_refs.is_empty() && (cluster_crossing_penalty > 0.0) {
        if conn_route.empty() {
            others.route_prefix.build(router, nodes, node_info, &mut conn_route, inf2, inf3, inf1_node);
        }
        // There are clusters so do cluster routing.
        // (not upstream) The boundaries are prepared once per search; as for
        // the connectors below, a boundary none of whose segments can touch
        // the last segment of `conn_route` adds nothing.
        let last_bounds = RouteBounds::of_points(&conn_route.ps[conn_route.size() - 2..], false);
        others.cluster_prefixes.begin((inf1_node, inf2));
        for (ci, cb) in cluster_boundaries.iter().enumerate() {
            if cb.poly.size() <= 2 {
                continue;
            }
            if let Some(b) = &last_bounds {
                if cb.segments.iter().all(|sb| sb.segment_cannot_touch(b)) {
                    continue;
                }
            }
            debug_assert!(cb.poly.ps[0] != cb.poly.ps[cb.poly.size() - 1]);
            // Upstream also asserts that every non-orthogonal cluster
            // boundary point is a router vertex ("should correspond to shape
            // vertices").  Not ported (assert policy): ClusterRef accepts any
            // polygon, the check has no effect on the result and the NDEBUG
            // build works (tests/robustness.rs clusters).

            let is_conn = false;
            // (splitBranchingSegments() as countForSegment() would run it,
            // from the state cached for this route prefix; see
            // `PrefixSplits`)
            let final_segment = Some(inf3) == router.conns[line_ref].dst();
            let mut cross = ConnectorCrossings::new(is_conn, None, None);
            cross.check_for_branching_segments = true;
            cross.already_split = true;
            let r: &Router = router;
            others.cluster_prefixes.with_split(
                ci,
                &conn_route,
                &cb.poly,
                is_conn,
                f64::EPSILON,
                &mut others.scratch,
                |c_boundary, dynamic_conn_route| {
                    let c_index = dynamic_conn_route.size() - 1;
                    cross.count_for_segment(r, c_boundary, dynamic_conn_route, c_index, final_segment);
                },
            );

            result += cross.crossing_count as f64 * cluster_crossing_penalty;
        }
    }

    // This penalty penalises route segments that head in a direction opposite
    // of the direction(s) toward the target point.
    let reverse_penalty = router.routing_parameter(RoutingParameter::ReverseDirectionPenalty);
    if reverse_penalty != 0.0 {
        // X and Y direction of destination from source point.
        let src_point = router.vert_infs[router.conns[line_ref].src().unwrap()].point;
        let dst_point = router.vert_infs[router.conns[line_ref].dst().unwrap()].point;
        let x_dir = dim_direction(dst_point.x - src_point.x);
        let y_dir = dim_direction(dst_point.y - src_point.y);

        let mut does_reverse = false;

        let p2 = router.vert_infs[inf2].point;
        let p3 = router.vert_infs[inf3].point;
        if (x_dir != 0) && (-x_dir == dim_direction(p3.x - p2.x)) {
            // Connector has an X component and the segment heads in the
            // opposite direction.
            does_reverse |= true;
        }

        if (y_dir != 0) && (-y_dir == dim_direction(p3.y - p2.y)) {
            // Connector has an Y component and the segment heads in the
            // opposite direction.
            does_reverse |= true;
        }

        if does_reverse {
            result += reverse_penalty;
        }
    }

    if !router.is_in_crossing_penalty_rerouting_stage() {
        // Return here if we are not in the post-processing stage
        return result;
    }

    let crossing_penalty = router.routing_parameter(RoutingParameter::CrossingPenalty);
    let shared_path_penalty = router.routing_parameter(RoutingParameter::FixedSharedPathPenalty);
    if (shared_path_penalty > 0.0) || (crossing_penalty > 0.0) {
        if conn_route.empty() {
            others.route_prefix.build(router, nodes, node_info, &mut conn_route, inf2, inf3, inf1_node);
        }
        // (not upstream) `others` are the other connectors in
        // `router.conn_refs` order, with the bounds of their display routes.
        // countForSegment() looks at the last segment of `conn_route` only:
        // where it cannot touch any segment of the other route (see
        // `RouteBounds`; the route's bounds are checked first),
        // splitting inserts no point into it or at its ends (points
        // inserted before it leave its ends unchanged), so no point of it
        // equals a point of the other route, and no segment intersection is
        // found (the intersection test starts with an exact bounding box
        // test).  The connector then adds nothing (adding 0.0 leaves
        // `result` unchanged).
        let conn_bounds = RouteBounds::of_points(&conn_route.ps[conn_route.size() - 2..], false);
        // Upstream computes the lazily set display route of every other
        // connector here; doing it once per search stores the same values
        // (no route changes during a search).
        if !others.display_routes_set {
            for other in &others.list {
                let _ = router.conns[other.conn].display_route();
            }
            others.display_routes_set = true;
        }
        // The connectors with a segment that may touch the last segment of
        // `conn_route`, in `others` order (all of them where the index
        // cannot tell).
        let candidates = conn_bounds.as_ref().and_then(|b| others.grid.query(b));
        let n_candidates = candidates.map_or(others.list.len(), |c| c.len());
        // Every connector left out has no segment that can touch.
        #[cfg(debug_assertions)]
        if let (Some(c), Some(b)) = (candidates, &conn_bounds) {
            for (i, other) in others.list.iter().enumerate() {
                if c.binary_search(&(i as u32)).is_err() {
                    let route2 = &router.conns[other.conn].m_display_route;
                    debug_assert!(route2.ps.windows(2).all(|seg| {
                        RouteBounds::of_points(seg, false).is_some_and(|sb| sb.segment_cannot_touch(b))
                    }));
                }
            }
        }
        others.conn_prefixes.begin((inf1_node, inf2));
        for k in 0..n_candidates {
            let oi = match candidates {
                Some(c) => c[k] as usize,
                None => k,
            };
            let other = &others.list[oi];
            let conn_ref = other.conn;
            let route2 = &router.conns[conn_ref].m_display_route;
            match (&other.bounds, &conn_bounds) {
                (Some(a), Some(b)) if a.cannot_touch(b) => continue,
                // An empty route has no segment to split or count.
                (None, _) => continue,
                // The same, segment by segment (a single point has none).
                (Some(_), Some(b))
                    if others.seg_bounds[other.segs.start as usize..other.segs.end as usize]
                        .iter()
                        .all(|sb| sb.segment_cannot_touch(b)) =>
                {
                    debug_assert!(route2.ps.windows(2).all(|seg| {
                        RouteBounds::of_points(seg, false).is_some_and(|sb| sb.segment_cannot_touch(b))
                    }));
                    continue
                }
                _ => {}
            }
            // (copies into reused buffers, split as countForSegment() would
            // split them, from the state cached for this route prefix; see
            // `PrefixSplits`)
            let is_conn = true;
            let dst_point = router.vert_infs[router.conns[line_ref].dst().unwrap()].point;
            let final_segment = router.vert_infs[inf3].point == dst_point;
            let mut cross = ConnectorCrossings::new(is_conn, Some(conn_ref), Some(line_ref));
            cross.check_for_branching_segments = true;
            cross.already_split = true;
            let r: &Router = router;
            others.conn_prefixes.with_split(
                oi,
                &conn_route,
                &r.conns[conn_ref].m_display_route,
                is_conn,
                0.0,
                &mut others.scratch,
                |dynamic_route2, dynamic_conn_route| {
                    let c_index = dynamic_conn_route.size() - 1;
                    cross.count_for_segment(r, dynamic_route2, dynamic_conn_route, c_index, final_segment);
                },
            );

            if (cross.crossing_flags & CROSSING_SHARES_PATH) != 0
                && (cross.crossing_flags & CROSSING_SHARES_FIXED_SEGMENT) != 0
                && (router.routing_option(RoutingOption::PenaliseOrthogonalSharedPathsAtConnEnds)
                    || (cross.crossing_flags & CROSSING_SHARES_PATH_AT_END) == 0)
            {
                // Penalise unnecessary shared paths in the middle of
                // connectors.
                result += shared_path_penalty;
            }
            result += cross.crossing_count as f64 * crossing_penalty;
        }
    }

    result
}

/// Another connector as seen by `cost()` during a search (not upstream).
#[derive(Debug)]
struct OtherRoute {
    conn: ConnIdx,
    /// Bounds of its display route (which no search changes).
    bounds: Option<RouteBounds>,
    /// Its segments' bounds, in `OtherRoutes::seg_bounds`.
    segs: std::ops::Range<u32>,
}

/// A cluster boundary as `cost()` uses it, with the bounds of its segments
/// (closing one included).  Not upstream.
#[derive(Debug)]
struct ClusterBoundary {
    poly: Polygon,
    segments: Vec<RouteBounds>,
}

/// The cluster boundaries of `router.cluster_refs`, in order, for a search
/// routing `line_ref` (when cost() will use them).
fn cluster_boundaries(router: &Router, line_ref: ConnIdx, out: &mut Vec<ClusterBoundary>) {
    out.clear();
    let cluster_crossing_penalty = router.routing_parameter(RoutingParameter::ClusterCrossingPenalty);
    if !(router.clustered_routing && !router.cluster_refs.is_empty() && (cluster_crossing_penalty > 0.0)) {
        return;
    }
    let is_orthogonal = router.conns[line_ref].routing_type() == ConnType::ConnTypeOrthogonal;
    for &cl in &router.cluster_refs {
        let poly = if is_orthogonal {
            router.clusters[cl].rectangular_polygon().clone()
        } else {
            Polygon::from_interface(&router.clusters[cl].polygon().view(router))
        };
        let n = poly.size();
        let segments = (0..n)
            .filter_map(|j| RouteBounds::of_points(&[poly.ps[(j + n - 1) % n], poly.ps[j]], false))
            .collect();
        out.push(ClusterBoundary { poly, segments });
    }
}

/// The other connectors of a search, with an index of their segments.
#[derive(Debug, Default)]
struct OtherRoutes {
    list: Vec<OtherRoute>,
    seg_bounds: Vec<RouteBounds>,
    grid: SegmentGrid,
    /// `cost()` has computed their display routes in this search.
    display_routes_set: bool,
    /// Buffers for the copies `cost()` hands to countForSegment().
    scratch: (Polygon, Polygon),
    /// Split states of the route prefix against each other connector and
    /// each cluster boundary.
    conn_prefixes: PrefixSplits,
    cluster_prefixes: PrefixSplits,
    route_prefix: RoutePrefix,
}

/// constructPolygonPath() for the edges of one expansion (not upstream):
/// every point of the route but the last (`inf3`) depends only on `inf2`
/// and the node before it (see `PrefixSplits`), so that prefix is built
/// once per expansion.
#[derive(Debug, Default)]
struct RoutePrefix {
    key: Option<(Option<ANodeIdx>, VertIdx)>,
    prefix: Polygon,
}

impl RoutePrefix {
    #[allow(clippy::too_many_arguments)]
    fn build(
        &mut self,
        router: &Router,
        nodes: &ANodes,
        node_info: &[NodeInfo],
        conn_route: &mut Polygon,
        inf2: VertIdx,
        inf3: VertIdx,
        inf1_node: Option<ANodeIdx>,
    ) {
        let key = (inf1_node, inf2);
        if self.key == Some(key) {
            conn_route.assign_from(&self.prefix);
            conn_route.ps.push(router.vert_infs[inf3].point);
            #[cfg(debug_assertions)]
            {
                let mut full = Polygon::new();
                construct_polygon_path(router, nodes, &mut full, inf2, inf3, inf1_node);
                debug_assert!(full.ps.len() == conn_route.ps.len()
                    && full.ps.iter().zip(&conn_route.ps).all(|(a, b)| a == b && a.id == b.id && a.vn == b.vn));
            }
            return;
        }
        construct_polygon_path_from_info(
            node_info,
            nodes,
            conn_route,
            router.vert_infs[inf2].point,
            router.vert_infs[inf3].point,
            inf1_node,
        );
        #[cfg(debug_assertions)]
        {
            let mut full = Polygon::new();
            construct_polygon_path(router, nodes, &mut full, inf2, inf3, inf1_node);
            debug_assert!(full.ps.len() == conn_route.ps.len()
                && full.ps.iter().zip(&conn_route.ps).all(|(a, b)| a == b && a.id == b.id && a.vn == b.vn));
        }
        self.key = Some(key);
        self.prefix.assign_from(conn_route);
        self.prefix.ps.pop();
    }
}

/// `cost()` builds the route `conn_route` from the expanded node's path
/// plus the edge's far end `inf3`; every point but that last one depends
/// only on the node (`inf2` and the node before it), so it is the same for
/// all edges of one expansion.  splitBranchingSegments() of the route
/// against another polygon handles the route's segments in order without
/// reading ahead (see `split_branching_segments_from()`), so its state
/// after the prefix is kept per polygon and only the last segment is split
/// for each edge (not upstream).
#[derive(Debug, Default)]
struct PrefixSplits {
    key: Option<(Option<ANodeIdx>, VertIdx)>,
    stamp: u64,
    /// Per polygon: (stamp, polygon after the prefix, prefix after it).
    entries: Vec<(u64, Polygon, Polygon)>,
}

impl PrefixSplits {
    fn reset(&mut self) {
        self.key = None;
    }

    fn begin(&mut self, key: (Option<ANodeIdx>, VertIdx)) {
        if self.key != Some(key) {
            self.key = Some(key);
            self.stamp += 1;
        }
    }

    /// Calls `f` with `poly` (polygon number `i`) and `conn_route` after
    /// splitBranchingSegments(poly, poly_is_conn, conn_route, tolerance);
    /// `f` must not change them.  When the last segment inserts nothing
    /// (`last_segment_splits()`), they are the cached state plus the last
    /// point, lent without copying; otherwise copies in `scratch` are split.
    #[allow(clippy::too_many_arguments)]
    fn with_split<R>(
        &mut self,
        i: usize,
        conn_route: &Polygon,
        poly: &Polygon,
        poly_is_conn: bool,
        tolerance: f64,
        scratch: &mut (Polygon, Polygon),
        f: impl FnOnce(&mut Polygon, &mut Polygon) -> R,
    ) -> R {
        if self.entries.len() <= i {
            self.entries.resize_with(i + 1, || (0, Polygon::new(), Polygon::new()));
        }
        let e = &mut self.entries[i];
        if e.0 != self.stamp {
            e.0 = self.stamp;
            e.1.assign_from(poly);
            e.2.assign_from(conn_route);
            e.2.ps.pop();
            split_branching_segments_from(&mut e.1, poly_is_conn, &mut e.2, tolerance, 0);
        }
        let last = *conn_route.ps.last().expect("empty route");
        let c0 = *e.2.ps.last().expect("empty route prefix");
        #[cfg(debug_assertions)]
        let expected = {
            let (mut p, mut c) = (poly.clone(), conn_route.clone());
            crate::avoid::connector::split_branching_segments(&mut p, poly_is_conn, &mut c, tolerance);
            (p, c)
        };
        #[cfg(debug_assertions)]
        let same = |a: &Polygon, b: &Polygon| {
            a.ps.len() == b.ps.len()
                && a.ps.iter().zip(&b.ps).all(|(x, y)| {
                    x.x.to_bits() == y.x.to_bits() && x.y.to_bits() == y.y.to_bits() && x.vn == y.vn && x.id == y.id
                })
        };
        if !last_segment_splits(&e.1, poly_is_conn, c0, last, e.2.ps.len() == 1, tolerance) {
            e.2.ps.push(last);
            #[cfg(debug_assertions)]
            debug_assert!(same(&expected.0, &e.1) && same(&expected.1, &e.2));
            let r = f(&mut e.1, &mut e.2);
            #[cfg(debug_assertions)]
            debug_assert!(same(&expected.0, &e.1) && same(&expected.1, &e.2), "f changed the cached state");
            e.2.ps.pop();
            return r;
        }
        let (out_poly, out_conn) = (&mut scratch.0, &mut scratch.1);
        out_poly.assign_from(&e.1);
        out_conn.assign_from(&e.2);
        out_conn.ps.push(last);
        let start = e.2.ps.len();
        split_branching_segments_from(out_poly, poly_is_conn, out_conn, tolerance, start);
        #[cfg(debug_assertions)]
        debug_assert!(same(&expected.0, out_poly) && same(&expected.1, out_conn));
        f(out_poly, out_conn)
    }
}

/// Whether splitBranchingSegments(), handling the segment `c0`-`c1` of the
/// route (its first one if `first`) against `poly`, would insert a point:
/// the tests its inner loop makes before it changes anything (not
/// upstream).
fn last_segment_splits(poly: &Polygon, poly_is_conn: bool, c0: Point, c1: Point, first: bool, tolerance: f64) -> bool {
    let n = poly.ps.len();
    for j in 0..n {
        if poly_is_conn && j == 0 {
            continue;
        }
        let p0 = poly.ps[if j == 0 { n - 1 } else { j - 1 }];
        let p1 = poly.ps[j];
        if (first && point_on_line(&p0, &p1, &c0, tolerance))
            || point_on_line(&p0, &p1, &c1, tolerance)
            || (poly_is_conn && j == 1 && point_on_line(&c0, &c1, &p0, tolerance))
            || point_on_line(&c0, &c1, &p1, tolerance)
        {
            return true;
        }
    }
    false
}

/// Uniform grid over the axis-aligned segments of the other routes, by
/// index into `OtherRoutes::list` (not upstream).  A segment is entered in
/// every cell its closed bounding box meets, so two axis-aligned segments
/// whose boxes meet (the only way they can touch, see `RouteBounds`) share
/// a cell: cell coordinates are monotonic in the position.  Segments that
/// are not axis-aligned may touch a segment whose box they only meet in
/// one dimension; their routes are always candidates.
#[derive(Debug, Default)]
struct SegmentGrid {
    /// Bounds of the indexed segments.
    extent: Option<RouteBounds>,
    origin: (f64, f64),
    inv_cell: (f64, f64),
    n: (usize, usize),
    cells: Vec<Vec<u32>>,
    always: Vec<u32>,
    stamp: Vec<u32>,
    query_id: u32,
    candidates: Vec<u32>,
}

impl SegmentGrid {
    fn build(&mut self, segments: &[(RouteBounds, u32)], n_routes: usize) {
        self.always.clear();
        for c in self.cells.iter_mut() {
            c.clear();
        }
        self.stamp.clear();
        self.stamp.resize(n_routes, 0);
        self.query_id = 0;
        let mut ext: Option<RouteBounds> = None;
        for (b, idx) in segments {
            if !b.axis_aligned() {
                self.always.push(*idx);
                continue;
            }
            ext = Some(match ext {
                None => *b,
                Some(e) => e.union(b),
            });
        }
        self.always.dedup();
        self.extent = ext;
        let Some(ext) = ext else {
            self.n = (0, 0);
            return;
        };
        let side = ((segments.len() as f64).sqrt().ceil() as usize).clamp(1, 128);
        let (w, h) = (ext.max_x() - ext.min_x(), ext.max_y() - ext.min_y());
        self.n = (if w > 0.0 { side } else { 1 }, if h > 0.0 { side } else { 1 });
        self.origin = (ext.min_x(), ext.min_y());
        self.inv_cell = (
            if w > 0.0 { self.n.0 as f64 / w } else { 0.0 },
            if h > 0.0 { self.n.1 as f64 / h } else { 0.0 },
        );
        let n_cells = self.n.0 * self.n.1;
        if self.cells.len() < n_cells {
            self.cells.resize_with(n_cells, Vec::new);
        }
        for (b, idx) in segments {
            if !b.axis_aligned() {
                continue;
            }
            let (x0, x1, y0, y1) = self.cell_range(b);
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let cell = &mut self.cells[y * self.n.0 + x];
                    if cell.last() != Some(idx) {
                        cell.push(*idx);
                    }
                }
            }
        }
    }

    #[inline]
    fn cell_of(&self, v: f64, dim: usize) -> isize {
        let (o, inv) = if dim == 0 { (self.origin.0, self.inv_cell.0) } else { (self.origin.1, self.inv_cell.1) };
        ((v - o) * inv).floor() as isize
    }

    /// Cell ranges of a box, clamped to the grid.
    fn cell_range(&self, b: &RouteBounds) -> (usize, usize, usize, usize) {
        let clamp = |c: isize, n: usize| c.clamp(0, n as isize - 1) as usize;
        (
            clamp(self.cell_of(b.min_x(), 0), self.n.0),
            clamp(self.cell_of(b.max_x(), 0), self.n.0),
            clamp(self.cell_of(b.min_y(), 1), self.n.1),
            clamp(self.cell_of(b.max_y(), 1), self.n.1),
        )
    }

    /// The routes that may have a segment touching a segment with bounds
    /// `b`, in ascending order; None if `b` is not axis-aligned.
    fn query(&mut self, b: &RouteBounds) -> Option<&[u32]> {
        if !b.axis_aligned() {
            return None;
        }
        self.query_id = self.query_id.wrapping_add(1);
        if self.query_id == 0 {
            self.stamp.fill(0);
            self.query_id = 1;
        }
        self.candidates.clear();
        let q = self.query_id;
        for &idx in &self.always {
            if self.stamp[idx as usize] != q {
                self.stamp[idx as usize] = q;
                self.candidates.push(idx);
            }
        }
        if let Some(ext) = self.extent {
            // A box that does not meet the extent meets no indexed segment
            // (clamping would wrongly pick border cells).  Otherwise both
            // go through the same clamped, monotonic cell mapping.
            let outside = b.max_x() < ext.min_x()
                || b.min_x() > ext.max_x()
                || b.max_y() < ext.min_y()
                || b.min_y() > ext.max_y();
            if !outside {
                let (x0, x1, y0, y1) = self.cell_range(b);
                for y in y0..=y1 {
                    for x in x0..=x1 {
                        for &idx in &self.cells[y * self.n.0 + x] {
                            if self.stamp[idx as usize] != q {
                                self.stamp[idx as usize] = q;
                                self.candidates.push(idx);
                            }
                        }
                    }
                }
            }
        }
        self.candidates.sort_unstable();
        Some(&self.candidates)
    }
}

/// The connectors `cost()` compares a route with (all of `router.conn_refs`
/// but `line_ref`, by id as upstream), when it will compare at all: only in
/// the crossing penalty rerouting stage with a crossing or shared path
/// penalty.
fn other_routes(router: &Router, line_ref: ConnIdx, others: &mut OtherRoutes, segments: &mut Vec<(RouteBounds, u32)>) {
    others.display_routes_set = false;
    others.conn_prefixes.reset();
    others.cluster_prefixes.reset();
    others.route_prefix.key = None;
    let out = &mut others.list;
    out.clear();
    others.seg_bounds.clear();
    segments.clear();
    let crossing_penalty = router.routing_parameter(RoutingParameter::CrossingPenalty);
    let shared_path_penalty = router.routing_parameter(RoutingParameter::FixedSharedPathPenalty);
    if !router.is_in_crossing_penalty_rerouting_stage() || !((shared_path_penalty > 0.0) || (crossing_penalty > 0.0)) {
        return;
    }
    let line_ref_id = router.conns[line_ref].id();
    for &conn_ref in &router.conn_refs {
        let c = &router.conns[conn_ref];
        if c.id() == line_ref_id {
            continue;
        }
        // displayRoute() without storing it: it is the simplified route
        // when unset, and cost() still sets it as upstream does.
        let simplified;
        let route = if c.m_display_route.empty() {
            simplified = c.route().simplify();
            &simplified
        } else {
            &c.m_display_route
        };
        let bounds = RouteBounds::of(route, false);
        let idx = out.len() as u32;
        let start = others.seg_bounds.len() as u32;
        for seg in route.ps.windows(2) {
            let sb = RouteBounds::of_points(seg, false).unwrap();
            segments.push((sb, idx));
            others.seg_bounds.push(sb);
        }
        out.push(OtherRoute { conn: conn_ref, bounds, segs: start..others.seg_bounds.len() as u32 });
    }
    others.grid.build(segments, out.len());
}

// Directions for estimated orthgonal cost, as bitflags.
// upstream: makepath.cpp CostDirectionN/E/S/W
const COST_DIRECTION_N: u32 = 1;
const COST_DIRECTION_E: u32 = 2;
const COST_DIRECTION_S: u32 = 4;
const COST_DIRECTION_W: u32 = 8;

/// Returns the number of directions for the argument.
// upstream: makepath.cpp orthogonalDirectionsCount() (static)
/// Writes the direction flags as `N E S W ` (upstream: only with
/// `ESTIMATED_COST_DEBUG`, here the `estimated-cost-debug` feature, which
/// prints the A* cost targets to stderr).
// upstream: makepath.cpp printDirections() (static)
#[cfg_attr(not(feature = "estimated-cost-debug"), allow(dead_code))]
fn print_directions(fp: &mut dyn std::fmt::Write, directions: u32) -> std::fmt::Result {
    if (directions & COST_DIRECTION_N) != 0 {
        fp.write_str("N ")?;
    }
    if (directions & COST_DIRECTION_E) != 0 {
        fp.write_str("E ")?;
    }
    if (directions & COST_DIRECTION_S) != 0 {
        fp.write_str("S ")?;
    }
    if (directions & COST_DIRECTION_W) != 0 {
        fp.write_str("W ")?;
    }
    Ok(())
}

fn orthogonal_directions_count(directions: u32) -> u32 {
    let mut count = 0;
    if directions & COST_DIRECTION_N != 0 {
        count += 1;
    }
    if directions & COST_DIRECTION_E != 0 {
        count += 1;
    }
    if directions & COST_DIRECTION_S != 0 {
        count += 1;
    }
    if directions & COST_DIRECTION_W != 0 {
        count += 1;
    }
    count
}

/// Returns the directions of point b from point a.
// upstream: makepath.cpp orthogonalDirection() (static)
fn orthogonal_direction(a: &Point, b: &Point) -> u32 {
    let mut result = 0;

    if b.y > a.y {
        result |= COST_DIRECTION_S;
    } else if b.y < a.y {
        result |= COST_DIRECTION_N;
    }

    if b.x > a.x {
        result |= COST_DIRECTION_E;
    } else if b.x < a.x {
        result |= COST_DIRECTION_W;
    }

    result
}

/// Returns the direction to the right of the given direction.
// upstream: makepath.cpp dirRight() (static)
fn dir_right(direction: u32) -> u32 {
    if direction == COST_DIRECTION_N {
        return COST_DIRECTION_E;
    } else if direction == COST_DIRECTION_E {
        return COST_DIRECTION_S;
    } else if direction == COST_DIRECTION_S {
        return COST_DIRECTION_W;
    } else if direction == COST_DIRECTION_W {
        return COST_DIRECTION_N;
    }

    // Should not be possible to reach here.
    debug_assert!(false);
    direction
}

/// Returns the direction to the left of the given direction.
// upstream: makepath.cpp dirLeft() (static)
fn dir_left(direction: u32) -> u32 {
    if direction == COST_DIRECTION_N {
        return COST_DIRECTION_W;
    } else if direction == COST_DIRECTION_E {
        return COST_DIRECTION_N;
    } else if direction == COST_DIRECTION_S {
        return COST_DIRECTION_E;
    } else if direction == COST_DIRECTION_W {
        return COST_DIRECTION_S;
    }

    // Should not be possible to reach here.
    debug_assert!(false);
    direction
}

/// Returns the reverse direction to the given direction.
// upstream: makepath.cpp dirReverse() (static)
fn dir_reverse(direction: u32) -> u32 {
    if direction == COST_DIRECTION_N {
        return COST_DIRECTION_S;
    } else if direction == COST_DIRECTION_E {
        return COST_DIRECTION_W;
    } else if direction == COST_DIRECTION_S {
        return COST_DIRECTION_N;
    } else if direction == COST_DIRECTION_W {
        return COST_DIRECTION_E;
    }

    // Should not be possible to reach here.
    debug_assert!(false);
    direction
}

/// Given Point curr with a direction of currDir, returns the nimimum
/// number of bends to reach Point dest with the entry direction of
/// destDir.
///
/// This is used for estimating the bend penalty cost to the target point
/// from the current point of the search. The geometry was described in
/// the "Orthogonal Connector Routing" paper, although the version
/// described there is incorrect.
// upstream: makepath.cpp bends()
pub fn bends(curr: &Point, curr_dir: u32, dest: &Point, dest_dir: u32) -> i32 {
    // Bend counts from 'o' to 'D' should be:
    //
    //                1            1            3
    //                v            v            v
    //            2 > o < 2    2 > o < 2    4 > o < 2
    //                ^            ^            ^
    //                3            3            3
    //
    //   0 > o < 4                 D-->             4 > o < 4
    //       ^                                          ^
    //       1                                          3
    //
    debug_assert!(curr_dir != 0);
    let curr_to_dest_dir = orthogonal_direction(curr, dest);
    let reverse_dest_dir = dir_reverse(dest_dir);
    let curr_dir_perpendicular_to_dest_dir = (curr_dir == dir_left(dest_dir)) || (curr_dir == dir_right(dest_dir));

    if (curr_dir == dest_dir) && (curr_to_dest_dir == curr_dir) {
        //
        //   0 > o                     D-->
        //
        return 0;
    } else if curr_dir_perpendicular_to_dest_dir && (curr_to_dest_dir == (dest_dir | curr_dir)) {
        //
        //                1
        //                v
        //                o
        //
        //
        //                             D-->
        //
        return 1;
    } else if curr_dir_perpendicular_to_dest_dir && (curr_to_dest_dir == curr_dir) {
        //
        //                             1
        //                             v
        //                             o
        //
        //
        //                             D-->
        //
        return 1;
    } else if curr_dir_perpendicular_to_dest_dir && (curr_to_dest_dir == dest_dir) {
        //
        //       o                     D-->
        //       ^
        //       1
        //
        return 1;
    } else if (curr_dir == dest_dir) && (curr_to_dest_dir != curr_dir) && (curr_to_dest_dir & reverse_dest_dir) == 0 {
        //
        //            2 > o        2 > o
        //
        //
        //                             D-->
        //
        return 2;
    } else if curr_dir == reverse_dest_dir && (curr_to_dest_dir != dest_dir) && (curr_to_dest_dir != curr_dir) {
        //
        //                o < 2        o < 2       o < 2
        //
        //
        //                             D-->
        //
        return 2;
    } else if curr_dir_perpendicular_to_dest_dir
        && (curr_to_dest_dir != (dest_dir | curr_dir))
        && (curr_to_dest_dir != curr_dir)
    {
        //
        //                                          3
        //                                          v
        //                o            o            o
        //                ^            ^            ^
        //                3            3            3
        //
        //                             D-->                 o
        //                                                  ^
        //                                                  3
        //
        return 3;
    } else if (curr_dir == reverse_dest_dir) && ((curr_to_dest_dir == dest_dir) || (curr_to_dest_dir == curr_dir)) {
        //
        //
        //
        //       o < 4                 D-->                 o < 4
        //
        return 4;
    } else if (curr_dir == dest_dir) && (curr_to_dest_dir & reverse_dest_dir) != 0 {
        //
        //                                      4 > o
        //
        //
        //                             D-->             4 > o
        //
        return 4;
    }

    // Should not be possible to reach here.
    debug_assert!(false);
    0
}

// upstream: makepath.cpp estimatedCostSpecific() (static)
fn estimated_cost_specific(
    router: &Router,
    line_ref: ConnIdx,
    last: Option<&Point>,
    curr: &Point,
    cost_tar: VertIdx,
    cost_tar_dirs: u32,
) -> f64 {
    let cost_tar_point = router.vert_infs[cost_tar].point;

    if router.conns[line_ref].routing_type() == ConnType::ConnTypePolyLine {
        euclidean_dist(curr, &cost_tar_point)
    } else {
        // Orthogonal
        // Really doesn't make sense to route orthogonal paths without
        // a segment penalty.
        // (Upstream asserts segmentPenalty > 0.  Not ported (assert
        // policy): setRoutingParameter(segmentPenalty, 0) is accepted API
        // and the NDEBUG build routes; tests/robustness.rs
        // extreme_penalties.)

        let dist = manhattan_dist(curr, &cost_tar_point);

        let mut bend_count: i32 = 0;
        let xmove = cost_tar_point.x - curr.x;
        let ymove = cost_tar_point.y - curr.y;
        match last {
            None => {
                // This is just the initial point.  Penalise it simply if it is
                // not inline with the target in either the x- or y-dimension.
                if (xmove != 0.0) && (ymove != 0.0) {
                    bend_count += 1;
                }
            }
            Some(last) => {
                if dist > 0.0 {
                    // We have two points and a non-zero distance, so we know
                    // the segment direction.

                    let curr_dir = orthogonal_direction(last, curr);
                    if (curr_dir > 0) && (orthogonal_directions_count(curr_dir) == 1) {
                        // Suitably high value, then we find the minimum.
                        bend_count = 10;

                        // Find the minimum bent penalty given all the possible
                        // directions at the target point.
                        if cost_tar_dirs & COST_DIRECTION_N != 0 {
                            bend_count =
                                std::cmp::min(bend_count, bends(curr, curr_dir, &cost_tar_point, COST_DIRECTION_N));
                        }
                        if cost_tar_dirs & COST_DIRECTION_E != 0 {
                            bend_count =
                                std::cmp::min(bend_count, bends(curr, curr_dir, &cost_tar_point, COST_DIRECTION_E));
                        }
                        if cost_tar_dirs & COST_DIRECTION_S != 0 {
                            bend_count =
                                std::cmp::min(bend_count, bends(curr, curr_dir, &cost_tar_point, COST_DIRECTION_S));
                        }
                        if cost_tar_dirs & COST_DIRECTION_W != 0 {
                            bend_count =
                                std::cmp::min(bend_count, bends(curr, curr_dir, &cost_tar_point, COST_DIRECTION_W));
                        }
                    }
                }
            }
        }
        let penalty = bend_count as f64 * router.routing_parameter(RoutingParameter::SegmentPenalty);

        dist + penalty
    }
}

// upstream: makepath.cpp class CmpVisEdgeRotation — see
// crate::avoid::graph::cmp_vis_edge_rotation.
use crate::avoid::graph::cmp_vis_edge_rotation;

// upstream: makepath.cpp pointAlignedWithOneOf() (static inline)
#[inline]
fn point_aligned_with_one_of(point: &Point, points: &[Point], dim: usize) -> bool {
    for p in points.iter() {
        if point[dim] == p[dim] {
            return true;
        }
    }
    false
}

// upstream: makepath.cpp class AStarPathPrivate
#[derive(Debug, Default)]
struct AStarPathPrivate {
    /// Upstream `m_available_nodes` blocks (with the array/node indices).
    m_available_nodes: Vec<ANode>,
    /// Not upstream: per node, what constructPolygonPath() reads (see
    /// `NodeInfo`).
    m_node_info: Vec<NodeInfo>,

    // For determining estimated cost target.
    m_cost_targets: Vec<VertIdx>,
    m_cost_targets_directions: Vec<u32>,
    m_cost_targets_displacements: Vec<f64>,

    /// Not upstream: the vertices whose `a_star_done_nodes` /
    /// `a_star_pending_nodes` this search filled, so that the cleanup at
    /// the end of `search()` visits them instead of every vertex (upstream
    /// walks the whole vertex list; the lists of all other vertices are
    /// already empty).  May hold duplicates.
    m_touched: Vec<VertIdx>,
    /// Not upstream: reused buffer for the edge list being expanded.
    m_vis_buf: Vec<EdgeIdx>,
    /// Not upstream: the other connectors for `cost()`.
    m_others: OtherRoutes,
    /// Not upstream: the PENDING heap, kept for its buffers.
    m_pending: PendingHeap,
    /// Not upstream: the cluster boundaries for `cost()`.
    m_cluster_boundaries: Vec<ClusterBoundary>,
    m_segments_buf: Vec<(RouteBounds, u32)>,
}

impl AStarPathPrivate {
    /// Returns a handle to an ANode for aStar search.  Upstream default for
    /// `add_to_pending` is true.
    // upstream: makepath.cpp AStarPathPrivate::newANode()
    fn new_a_node(&mut self, router: &mut Router, node: &ANode, add_to_pending: bool) -> ANodeIdx {
        let new_node = ANodeIdx(self.m_available_nodes.len() as u32);
        self.m_available_nodes.push(*node);
        let info = NodeInfo::of(router, &self.m_node_info, node);
        self.m_node_info.push(info);
        if add_to_pending {
            let v = node.inf.unwrap();
            self.touch(router, v);
            router.vert_infs[v].a_star_pending_nodes.push(new_node);
        }
        new_node
    }

    /// Records `v` for the cleanup before one of its A* lists is pushed to.
    #[inline]
    fn touch(&mut self, router: &Router, v: VertIdx) {
        let vi = &router.vert_infs[v];
        if vi.a_star_done_nodes.is_empty() && vi.a_star_pending_nodes.is_empty() {
            self.m_touched.push(v);
        }
    }

    /// Adds `n` to the Done set of vertex `v`.
    #[inline]
    fn push_done(&mut self, router: &mut Router, v: VertIdx, n: ANodeIdx) {
        self.touch(router, v);
        router.vert_infs[v].a_star_done_nodes.push(n);
    }

    // upstream: makepath.cpp AStarPathPrivate::determineEndPointLocation()
    fn determine_end_point_location(
        &mut self,
        router: &Router,
        dist: f64,
        start: VertIdx,
        target: VertIdx,
        other: VertIdx,
        level: i32,
    ) {
        #[cfg(not(feature = "estimated-cost-debug"))]
        let _ = (dist, start, level);

        let other_point = router.vert_infs[other].point;
        let target_point = router.vert_infs[target].point;
        let this_dirs = orthogonal_direction(&other_point, &target_point);
        // Upstream: COLA_ASSERT(orthogonalDirectionsCount(thisDirs) > 0).
        // Not ported (assert policy): upstream's asserting build fails it on
        // valid input -- a junction at (50,0) between point ends at (0,0)
        // and (100,0), orthogonal routing (e2e fuzz regression scene
        // reg-assert-endpoint-direction, and 24 `#assert` scenes of the
        // committed e2e fuzz golden) -- while its NDEBUG build routes the
        // scene (thisDirs == 0 only weakens the cost estimate); the port
        // matches that output.
        let displacement = manhattan_dist(&other_point, &target_point);

        self.m_cost_targets.push(other);
        self.m_cost_targets_directions.push(this_dirs);
        self.m_cost_targets_displacements.push(displacement);

        #[cfg(feature = "estimated-cost-debug")]
        {
            use crate::avoid::debug::g;
            let mut fp = String::new();
            fp.push_str(&format!(" - {} {} ", g(other_point.x), g(other_point.y)));
            if manhattan_dist(&router.vert_infs[start].point, &other_point) > dist {
                fp.push_str("far ");
            }
            fp.push_str(if level == 1 { "--" } else { "- " });
            let _ = print_directions(&mut fp, this_dirs);
            fp.push('\n');
            eprint!("{}", fp);
        }
    }

    // upstream: makepath.cpp AStarPathPrivate::estimatedCost()
    fn estimated_cost(&self, router: &Router, line_ref: ConnIdx, last: Option<&Point>, curr: &Point) -> f64 {
        let mut estimate = f64::MAX;
        debug_assert!(!self.m_cost_targets.is_empty());

        // Find the minimum cost from the estimates to each of the possible
        // target points from this current point.
        for i in 0..self.m_cost_targets.len() {
            let mut i_estimate = estimated_cost_specific(
                router,
                line_ref,
                last,
                curr,
                self.m_cost_targets[i],
                self.m_cost_targets_directions[i],
            );

            // Add on the distance to the real target, otherwise this difference
            // might may make the comparisons unfair if they vary between targets.
            i_estimate += self.m_cost_targets_displacements[i];

            estimate = cpp_min(estimate, i_estimate);
        }
        estimate
    }

    /// Returns the best path from src to tar using the cost function.
    ///
    /// The path is worked out using the aStar algorithm, and is encoded via
    /// prevNode values for each ANode which point back to the previous
    /// ANode.  At completion, this order is written into the pathNext links
    /// in each of the VerInfs along the path.
    // upstream: makepath.cpp AStarPathPrivate::search()
    fn search(
        &mut self,
        router: &mut Router,
        line_ref: ConnIdx,
        src: VertIdx,
        tar: VertIdx,
        start: Option<VertIdx>,
    ) {
        let is_orthogonal = router.conns[line_ref].routing_type() == ConnType::ConnTypeOrthogonal;
        other_routes(router, line_ref, &mut self.m_others, &mut self.m_segments_buf);
        cluster_boundaries(router, line_ref, &mut self.m_cluster_boundaries);

        let start = match start {
            None => src,
            Some(s) => s,
        };

        #[cfg(feature = "debug-handler")]
        router.call_debug_handler(|h, r| h.beginning_search_with_endpoints(r, start, tar));

        // Find a target point to use for cost estimate for orthogonal routing.
        //
        // If the connectivity is only on the far side we need to estimate to the
        // point on the far side.  Otherwise for orthogonal routing we can explore
        // all the space in between before we pay the extra cost to explore this
        // area.  This is especially true given many orthogonal routes have
        // equivalent costs.
        #[cfg(feature = "estimated-cost-debug")]
        eprint!(
            "== aStar  {} {} ==\n",
            crate::avoid::debug::g(router.vert_infs[tar].point.x),
            crate::avoid::debug::g(router.vert_infs[tar].point.y)
        );
        let tar_id = router.vert_infs[tar].id;
        if is_orthogonal && tar_id.is_conn_pt() && !tar_id.is_conn_checkpoint() {
            // The target is a connector endpoint and the connector is orthogonal.
            let dist = manhattan_dist(&router.vert_infs[start].point, &router.vert_infs[tar].point);
            let tar_edges = router.vert_infs[tar].orthog_vis_list.to_vec(&router.edge_infs);
            for edge in tar_edges {
                // For each edge from the target endpoint, find the other vertex.
                let other = router.edge_infs[edge].other_vert(tar);
                if router.vert_infs[other].id.is_connection_pin() {
                    // If this is a connection pin we need to do this process
                    // another time since the current edge will be a dummy
                    // zero-length edge.
                    let replacement_tar = other;
                    let rep_edges = router.vert_infs[replacement_tar]
                        .orthog_vis_list
                        .to_vec(&router.edge_infs);
                    for edge in rep_edges {
                        let other = router.edge_infs[edge].other_vert(replacement_tar);
                        if (other == tar) || (router.vert_infs[other].point == router.vert_infs[tar].point) {
                            // Ignore edge we came from, or zero-length edges.
                            continue;
                        }

                        // Determine possible target endpoint directions and
                        // position.
                        self.determine_end_point_location(router, dist, start, replacement_tar, other, 2);
                    }
                    continue;
                }

                // Determine possible target endpoint directions and position.
                self.determine_end_point_location(router, dist, start, tar, other, 1);
            }
        }

        if self.m_cost_targets.is_empty() {
            self.m_cost_targets.push(tar);
            // For polyline routing, assume target has visibility is all
            // directions for the purpose of cost estimations.
            self.m_cost_targets_directions
                .push(COST_DIRECTION_N | COST_DIRECTION_E | COST_DIRECTION_S | COST_DIRECTION_W);
            self.m_cost_targets_displacements.push(0.0);
        }

        #[cfg(feature = "estimated-cost-debug")]
        {
            use crate::avoid::debug::g;
            let mut fp = String::from("------------\n");
            for i in 0..self.m_cost_targets.len() {
                let p = router.vert_infs[self.m_cost_targets[i]].point;
                fp.push_str(&format!("== {} {} - ", g(p.x), g(p.y)));
                let _ = print_directions(&mut fp, self.m_cost_targets_directions[i]);
                fp.push('\n');
            }
            eprint!("{}", fp);
        }

        let dist: fn(&Point, &Point) -> f64 = if is_orthogonal { manhattan_dist } else { euclidean_dist };

        // We need to know the possible endpoints for doing an orthogonal
        // routing optimisation where we only turn when we are heading beside
        // a shape or are in line with a possible endpoint.
        let mut end_points: Vec<Point> = Vec::new();
        if is_orthogonal {
            end_points = router.conns[line_ref].possible_dst_pin_points(router);
        }
        end_points.push(router.vert_infs[tar].point);

        // Heap of PENDING nodes.
        let mut pending = std::mem::take(&mut self.m_pending);
        pending.clear();

        let mut explored_count: usize = 0;
        let mut node: ANode;
        let mut ati: ANode;
        let mut best_node: Option<ANodeIdx> = None; // Temporary bestNode
        let mut b_node_found: bool; // Flag if node is found in container
        let mut timestamp: i32 = 1;

        // Port addition (robustness): upstream walks the current route from
        // its first point until it reaches `start`, dereferencing every
        // vertex looked up by id.  If a route point has no vertex (e.g. a
        // fixed route through arbitrary points) or `start` is not on the
        // route, upstream dereferences null / reads past the route.  That
        // walk is checked first (read-only); if it would fail, the search
        // starts afresh from `src` as for a non-rubber-band search.
        let rubber_band_walk_ok = router.rubber_band_routing && (start != src) && {
            let curr_route = router.conns[line_ref].route();
            let mut ok = false;
            for r_indx in 0..curr_route.size() {
                let pnt = curr_route.at(r_indx);
                let props: VertIDProps = if r_indx > 0 { 0 } else { VertID::PROP_CONN_POINT };
                match router.vertices.get_vertex_by_id(&router.vert_infs, &VertID::new(pnt.id, pnt.vn, props)) {
                    None => break,
                    Some(v) if v == start => {
                        ok = true;
                        break;
                    }
                    Some(_) => {}
                }
            }
            ok
        };
        if rubber_band_walk_ok {
            debug_assert!(router.ignore_regions);

            let curr_route = router.conns[line_ref].route().clone();
            let mut last: Option<VertIdx> = None;
            let mut r_indx: usize = 0;
            while last != Some(start) {
                let pnt = curr_route.at(r_indx);
                let props: VertIDProps = if r_indx > 0 { 0 } else { VertID::PROP_CONN_POINT };
                let v_id = VertID::new(pnt.id, pnt.vn, props);

                #[cfg(feature = "pathdebug")]
                crate::avoid::debug::db_printf!("/// {} {}\n", pnt.id as i32, pnt.vn);
                let curr = router.vertices.get_vertex_by_id(&router.vert_infs, &v_id);
                debug_assert!(curr.is_some());
                let curr = curr.unwrap();

                node = ANode::new(curr, timestamp);
                timestamp += 1;
                if last.is_none() {
                    node.inf = Some(src);
                    node.g = 0.0;
                    let p = router.vert_infs[src].point;
                    node.h = self.estimated_cost(router, line_ref, None, &p);

                    node.f = node.g + node.h;
                } else {
                    let bn = best_node.unwrap();
                    let bn_node = self.m_available_nodes[bn.0 as usize];
                    let bn_inf = bn_node.inf.unwrap();
                    let edge_dist = dist(&router.vert_infs[bn_inf].point, &router.vert_infs[curr].point);

                    node.g = bn_node.g
                        + cost(
                            router,
                            &self.m_available_nodes,
                            &mut self.m_others,
                            &self.m_cluster_boundaries,
                            &self.m_node_info,
                            line_ref,
                            edge_dist,
                            bn_inf,
                            node.inf.unwrap(),
                            bn_node.prev_node,
                        );

                    // Calculate the Heuristic.
                    let bp = router.vert_infs[bn_inf].point;
                    let np = router.vert_infs[node.inf.unwrap()].point;
                    node.h = self.estimated_cost(router, line_ref, Some(&bp), &np);

                    // The A* formula
                    node.f = node.g + node.h;

                    // Point parent to last bestNode
                    node.prev_node = Some(bn);
                }

                if curr != start {
                    let add_to_pending = false;
                    let bn = self.new_a_node(router, &node, add_to_pending);
                    best_node = Some(bn);
                    let bn_inf = inf_of(&self.m_available_nodes, bn);
                    self.push_done(router, bn_inf, bn);
                    explored_count += 1;
                } else {
                    let new_node = self.new_a_node(router, &node, true);
                    pending.push_back(&self.m_available_nodes, new_node);
                }

                r_indx += 1;
                last = Some(curr);
            }
        } else {
            // Stale-pathNext policy: a link to a deleted vertex reads as
            // null (VertInf::live_path_next; router.rs module docs).
            if let Some(start_next) = VertInf::live_path_next(router, start) {
                // If we are doing checkpoint routing and have already done one
                // path, then we have an existing segment to consider for the
                // cost of the  choice from the start node, so we add a dummy
                // nodes as if they were already in the Done set.  This causes
                // us to first search in a collinear direction from the previous
                // segment.
                let add_to_pending = false;
                let n = ANode::new(start_next, timestamp);
                timestamp += 1;
                let bn = self.new_a_node(router, &n, add_to_pending);
                best_node = Some(bn);
                self.push_done(router, start_next, bn);
                explored_count += 1;
            }

            // Create the start node
            node = ANode::new(src, timestamp);
            timestamp += 1;
            node.g = 0.0;
            let p = router.vert_infs[src].point;
            node.h = self.estimated_cost(router, line_ref, None, &p);
            node.f = node.g + node.h;
            // Set a nullptr parent, so cost function knows this is the first segment.
            node.prev_node = best_node;

            // Populate the PENDING container with the first location
            let new_node = self.new_a_node(router, &node, true);
            pending.push_back(&self.m_available_nodes, new_node);
        }

        router.vert_infs[tar].path_next = None;

        // Create a heap from PENDING for sorting
        pending.make_heap(&self.m_available_nodes);

        let src_point = router.vert_infs[src].point;
        let line_src = router.conns[line_ref].src();
        let line_dst = router.conns[line_ref].dst();

        // Continue until the queue is empty.
        while pending.len() != 0 {
            // Set the Node with lowest f value to BESTNODE.
            // Since the ANode operator< is reversed, the head of the
            // heap is the node with the lowest f value.
            crate::avoid::timer::timer_var_add!(router, 0, 1);
            let bn = pending.top();
            // (upstream assigns bestNode here; `bn` plays that role below.)
            let best_node_inf = inf_of(&self.m_available_nodes, bn);

            #[cfg(feature = "debug-handler")]
            if router.has_debug_handler() {
                let mut current_search_path = crate::avoid::geomtypes::PolyLine::new();

                let mut curr = Some(bn);
                while let Some(c) = curr {
                    current_search_path.ps.push(router.vert_infs[inf_of(&self.m_available_nodes, c)].point);
                    curr = self.m_available_nodes[c.0 as usize].prev_node;
                }
                router.call_debug_handler(|h, r| h.update_current_search_path(r, current_search_path));
            }

            // Remove this node from the aStarPendingList
            {
                let list = &mut router.vert_infs[best_node_inf].a_star_pending_nodes;
                if let Some(pos) = list.iter().position(|&n| n == bn) {
                    list.remove(pos);
                }
            }

            // Pop off the heap.  Actually this moves the
            // far left value to the far right.  The node
            // is not actually removed since the pop is to
            // the heap and not the container.
            // Remove node from right (the value we pop_heap'd)
            pending.pop(&self.m_available_nodes);

            // Add the bestNode into the Done set.
            self.push_done(router, best_node_inf, bn);
            explored_count += 1;

            let bn_node = self.m_available_nodes[bn.0 as usize];
            let prev_inf: Option<VertIdx> = bn_node.prev_node.map(|p| inf_of(&self.m_available_nodes, p));
            #[cfg(feature = "astar-debug")]
            {
                use crate::avoid::debug::{db_printf, g};
                db_printf!("Considering... ");
                let p = router.vert_infs[best_node_inf].point;
                db_printf!(" {} {}  ", g(p.x), g(p.y));
                router.vert_infs[best_node_inf].id.db_print();
                db_printf!(" - g: {:3.1} h: {:3.1} back: ", bn_node.g, bn_node.h);
                if let Some(prev_inf) = prev_inf {
                    let p = router.vert_infs[prev_inf].point;
                    db_printf!(" {} {}", g(p.x), g(p.y));
                    //prevInf->id.db_print();
                }
                db_printf!("\n");
            }

            if best_node_inf == tar {
                crate::avoid::timer::timer_var_add!(router, 1, pending.len() as u32);
                // This node is our goal.
                #[cfg(feature = "astar-debug")]
                crate::avoid::debug::db_printf!(
                    "LINE {:10}  Steps: {:4}  Cost: {}\n",
                    router.conns[line_ref].m_id as i32,
                    explored_count as i32,
                    crate::avoid::debug::g(bn_node.f)
                );

                // Correct all the pathNext pointers.
                let mut curr = bn;
                while let Some(prev) = self.m_available_nodes[curr.0 as usize].prev_node {
                    let c_inf = inf_of(&self.m_available_nodes, curr);
                    #[cfg(feature = "astar-debug")]
                    {
                        let p = router.vert_infs[c_inf].point;
                        crate::avoid::debug::db_printf!("[{:.12}, {:.12}]\n", p.x, p.y);
                    }
                    router.vert_infs[c_inf].path_next = Some(inf_of(&self.m_available_nodes, prev));
                    curr = prev;
                }
                #[cfg(feature = "astar-debug")]
                crate::avoid::debug::db_printf!("\n");

                // Exit from the search
                break;
            }

            // Check adjacent points in graph and add them to the queue.
            let which = if !is_orthogonal {
                crate::avoid::vertices::VertEdgeList::Vis
            } else {
                crate::avoid::vertices::VertEdgeList::OrthogVis
            };
            if is_orthogonal {
                // We would like to explore in a structured way,
                // so sort the points in the visList...
                VertInf::sort_edge_list(router, best_node_inf, which, |r, u, v| {
                    cmp_vis_edge_rotation(r, prev_inf, u, v)
                });
            }
            let mut vis_list = std::mem::take(&mut self.m_vis_buf);
            vis_list.clear();
            vis_list.extend(router.vert_infs[best_node_inf].edge_list(which).iter(&router.edge_infs));
            for &edge in &vis_list {
                if router.edge_infs[edge].is_disabled() {
                    // Skip disabled edges.
                    continue;
                }

                let other = router.edge_infs[edge].other_vert(best_node_inf);
                node = ANode::new(other, timestamp);
                timestamp += 1;

                // Set the index to the previous ANode that we reached
                // this ANode via.
                node.prev_node = Some(bn);

                let node_inf = other;
                let node_id = router.vert_infs[node_inf].id;

                // Don't bother looking at the segment we just arrived along.
                if prev_inf.is_some() && (prev_inf == Some(node_inf)) {
                    continue;
                }
                if node_id.is_connection_pin() && !node_id.is_conn_checkpoint() {
                    let src_dummy = (Some(best_node_inf) == line_src)
                        && router.vert_infs[line_src.unwrap()].id.is_dummy_pin_helper();
                    let dst_dummy = !src_dummy && {
                        let dst = line_dst.unwrap();
                        VertInf::has_neighbour(router, node_inf, dst, is_orthogonal).is_some()
                            && router.vert_infs[dst].id.is_dummy_pin_helper()
                    };
                    if !src_dummy && !dst_dummy {
                        // Don't check connection pins if they don't have the
                        // target vertex as a direct neighbour, or are directly
                        // leaving the source vertex.
                        continue;
                    }
                } else if node_id.is_conn_pt() {
                    if node_inf != tar {
                        // Don't check connector endpoints vertices unless they
                        // are the target endpoint.
                        continue;
                    }
                }

                if is_orthogonal && !router.edge_infs[edge].is_dummy_connection(&router.vert_infs) {
                    // Orthogonal routing optimisation.
                    // Skip the edges that don't lead to shape edges, or the
                    // connection point we are looking for.  Though allow them
                    // if we haven't yet turned from the source point, since it
                    // may be a free-floating endpoint with directional visibility.
                    // Also, don't check if the previous point was a dummy for a
                    // connection pin and this happens to be placed diagonally
                    // from here, i.e., when both of notInline{X,Y} are true.
                    let best_pt = router.vert_infs[best_node_inf].point;
                    let next_pt = router.vert_infs[node_inf].point;
                    let best_flags = router.vert_infs[best_node_inf].orthog_vis_prop_flags;

                    let not_inline_x = prev_inf.is_some_and(|p| router.vert_infs[p].point.x != best_pt.x);
                    let not_inline_y = prev_inf.is_some_and(|p| router.vert_infs[p].point.y != best_pt.y);
                    if (best_pt.x == next_pt.x) && not_inline_x && !not_inline_y && (best_pt[YDIM] != src_point[YDIM])
                    {
                        if next_pt.y < best_pt.y {
                            if (best_flags & YL_EDGE) == 0 && !point_aligned_with_one_of(&best_pt, &end_points, XDIM) {
                                continue;
                            }
                        } else if next_pt.y > best_pt.y {
                            if (best_flags & YH_EDGE) == 0 && !point_aligned_with_one_of(&best_pt, &end_points, XDIM) {
                                continue;
                            }
                        }
                    }
                    if (best_pt.y == next_pt.y) && not_inline_y && !not_inline_x && (best_pt[XDIM] != src_point[XDIM])
                    {
                        if next_pt.x < best_pt.x {
                            if (best_flags & XL_EDGE) == 0 && !point_aligned_with_one_of(&best_pt, &end_points, YDIM) {
                                continue;
                            }
                        } else if next_pt.x > best_pt.x {
                            if (best_flags & XH_EDGE) == 0 && !point_aligned_with_one_of(&best_pt, &end_points, YDIM) {
                                continue;
                            }
                        }
                    }
                }

                let edge_dist = router.edge_infs[edge].get_dist();

                if edge_dist == 0.0 {
                    continue;
                }

                if !is_orthogonal
                    && (!router.rubber_band_routing || (start == src))
                    && !validate_bend_point(router, prev_inf, best_node_inf, Some(node_inf))
                {
                    // The bendpoint is not valid, i.e., is a zigzag corner, so...
                    continue;
                    // For RubberBand routing we want to allow these routes and
                    // unwind them later, otherwise instead or unwinding, paths
                    // can go the *really* long way round.
                }

                // Figure out if we are at one of the cost targets.
                let mut at_cost_target = false;
                for i in 0..self.m_cost_targets.len() {
                    if best_node_inf == self.m_cost_targets[i] {
                        at_cost_target = true;
                        break;
                    }
                }

                if at_cost_target && (node_id.is_connection_pin() || (node_inf == tar)) {
                    // This is a point on the side of an obstacle that connects
                    // to the target or a connection pin.  It should have no
                    // further cost and the heuristic should be zero.
                    node.g = bn_node.g;
                    node.h = 0.0;
                } else {
                    if node_inf == tar {
                        // We've reached the target.  The heuristic should be zero.
                        node.h = 0.0;
                    } else {
                        // Otherwise, calculate the heuristic value.
                        let bp = router.vert_infs[best_node_inf].point;
                        let np = router.vert_infs[node_inf].point;
                        node.h = self.estimated_cost(router, line_ref, Some(&bp), &np);
                    }

                    if node_id.is_dummy_pin_helper() {
                        // This is connecting to a connection pin helper vertex.
                        // There should be no additional cost for this step.
                        node.g = bn_node.g;
                    } else {
                        // Otherwise, calculate the cost of this step.
                        node.g = bn_node.g
                            + cost(
                                router,
                                &self.m_available_nodes,
                                &mut self.m_others,
                                &self.m_cluster_boundaries,
                            &self.m_node_info,
                                line_ref,
                                edge_dist,
                                best_node_inf,
                                node_inf,
                                bn_node.prev_node,
                            );
                    }
                }

                // The A* formula
                node.f = node.g + node.h;

                #[cfg(feature = "astar-debug")]
                {
                    use crate::avoid::debug::{db_printf, g};
                    let p = router.vert_infs[node_inf].point;
                    db_printf!("-- Adding: {} {}  ", g(p.x), g(p.y));
                    router.vert_infs[node_inf].id.db_print();
                    db_printf!(" - g: {:3.1} h: {:3.1} \n", node.g, node.h);
                }

                b_node_found = false;

                // Check to see if already on PENDING
                for &curr_ind in router.vert_infs[node_inf].a_star_pending_nodes.iter() {
                    ati = self.m_available_nodes[curr_ind.0 as usize];
                    // The (node.prevNode == ati.prevNode) is redundant, but may
                    // save checking the mosre costly prevNode->inf test if the
                    // Nodes are the same.
                    if (node.inf == ati.inf)
                        && ((node.prev_node == ati.prev_node)
                            || (inf_of(&self.m_available_nodes, node.prev_node.unwrap())
                                == inf_of(
                                    &self.m_available_nodes,
                                    ati.prev_node.expect("pending ANode without prevNode"),
                                )))
                    {
                        // If already on PENDING
                        if node.g < ati.g {
                            // Replace the existing node in PENDING
                            let old_f = self.m_available_nodes[curr_ind.0 as usize].f;
                            self.m_available_nodes[curr_ind.0 as usize] = node;
                            // (a pending node has no successors yet)
                            self.m_node_info[curr_ind.0 as usize] = NodeInfo::of(router, &self.m_node_info, &node);
                            // (upstream: std::make_heap(PENDING...))
                            pending.key_changed(&self.m_available_nodes, curr_ind, old_f);
                        }
                        b_node_found = true;
                        break;
                    }
                }
                if !b_node_found {
                    // If Node NOT found on PENDING
                    // Check to see if it is already in the Done set for this
                    // vertex.
                    for &curr_ind in router.vert_infs[node_inf].a_star_done_nodes.iter() {
                        ati = self.m_available_nodes[curr_ind.0 as usize];
                        // The (node.prevNode == ati.prevNode) is redundant, but may
                        // save checking the mosre costly prevNode->inf test if the
                        // Nodes are the same.
                        if (node.inf == ati.inf)
                            && ati.prev_node.is_some()
                            && ((node.prev_node == ati.prev_node)
                                || (inf_of(&self.m_available_nodes, node.prev_node.unwrap())
                                    == inf_of(&self.m_available_nodes, ati.prev_node.unwrap())))
                        {
                            //COLA_ASSERT(node.g >= (ati.g - 10e-10));
                            // This node is already in the Done set and the
                            // current node also has a higher g-value, so we
                            // don't need to consider this node.
                            b_node_found = true;
                            break;
                        }
                    }
                }

                if !b_node_found {
                    // If Node NOT in either Pending or Done.
                    // Push NewNode onto PENDING
                    let new_node = self.new_a_node(router, &node, true);
                    // Push NewNode onto heap
                    pending.push(&self.m_available_nodes, new_node);
                }
            }
            self.m_vis_buf = vis_list;
        }
        let _ = explored_count;
        pending.clear();
        self.m_pending = pending;

        // Cleanup lists used to store Done and Pending sets for each vertex.
        // (Upstream walks every vertex; only the touched ones have entries.)
        for kv in self.m_touched.drain(..) {
            let v = &mut router.vert_infs[kv];
            v.a_star_done_nodes.clear();
            v.a_star_pending_nodes.clear();
        }
        #[cfg(debug_assertions)]
        {
            let end_vert = router.vertices.end();
            let mut k = router.vertices.conns_begin();
            while k != end_vert {
                let v = &router.vert_infs[k.unwrap()];
                debug_assert!(v.a_star_done_nodes.is_empty() && v.a_star_pending_nodes.is_empty());
                k = v.lst_next;
            }
        }
    }
}

// upstream: makepath.h class AStarPath
#[derive(Debug)]
pub struct AStarPath {
    m_private: AStarPathPrivate,
}

impl AStarPath {
    // upstream: makepath.cpp AStarPath::AStarPath()
    pub fn new() -> AStarPath {
        AStarPath {
            m_private: AStarPathPrivate::default(),
        }
    }

    /// Routes `line_ref` from `src` to `tar` (`start` is the rubber-band
    /// restart vertex, may be None), leaving the result in the vertices'
    /// `path_next` links.
    // upstream: makepath.cpp AStarPath::search()
    pub fn search(
        &mut self,
        router: &mut Router,
        line_ref: ConnIdx,
        src: VertIdx,
        tar: VertIdx,
        start: Option<VertIdx>,
    ) {
        self.m_private.search(router, line_ref, src, tar, start);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The heap ports must agree with a straightforward reference for a
    /// strict ordering (heap property + pop order).
    #[test]
    fn heap_ops_pop_in_order() {
        let mut v: Vec<i32> = vec![5, 3, 9, 1, 7, 2, 8, 6, 4, 0];
        let mut less = |a: &i32, b: &i32| a < b;
        make_heap(&mut v, &mut less);
        v.push(11);
        push_heap(&mut v, &mut less);
        let mut out = Vec::new();
        while !v.is_empty() {
            pop_heap(&mut v, &mut less);
            out.push(v.pop().unwrap());
        }
        assert_eq!(out, vec![11, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]);
    }

    /// KeyTies against a recomputation from the sorted values.
    #[test]
    fn key_ties_clusters() {
        fn brute(vals: &[f64]) -> bool {
            let mut v: Vec<f64> = vals.to_vec();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            v.dedup();
            let mut start = 0;
            for i in 1..=v.len() {
                if i == v.len() || v[i] - v[i - 1] > KeyTies::EPS {
                    if v[i - 1] - v[start] > KeyTies::EPS {
                        return false;
                    }
                    start = i;
                }
            }
            true
        }
        let mut seed: u64 = 0x1234_5678_9abc_def1;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for round in 0..200 {
            let mut t = KeyTies::default();
            let mut vals: Vec<f64> = Vec::new();
            for _ in 0..300 {
                if vals.is_empty() || next() % 3 != 0 {
                    // Values on a coarse grid with noise around the epsilon.
                    let base = (next() % 20) as f64 * if round % 2 == 0 { 1e-7 } else { 1.0 };
                    let noise = (next() % 7) as f64 * 3e-8 - 9e-8;
                    let f = if next() % 2 == 0 { base } else { base + noise };
                    t.insert(f);
                    vals.push(f);
                } else {
                    let i = (next() % vals.len() as u64) as usize;
                    t.remove(vals.swap_remove(i));
                }
                assert_eq!(t.strict(), brute(&vals));
            }
        }
    }

    #[test]
    fn heap_layout_matches_libstdcxx() {
        // std::make_heap on {1..=10} with operator< yields this layout in
        // libstdc++.
        let mut v: Vec<i32> = (1..=10).collect();
        make_heap(&mut v, &mut |a: &i32, b: &i32| a < b);
        assert_eq!(v, vec![10, 9, 7, 8, 5, 6, 3, 1, 4, 2]);
    }

    #[test]
    fn bends_examples() {
        let o = Point::new(0.0, 0.0);
        let d = Point::new(10.0, 0.0);
        assert_eq!(bends(&o, COST_DIRECTION_E, &d, COST_DIRECTION_E), 0);
        assert_eq!(bends(&o, COST_DIRECTION_W, &d, COST_DIRECTION_E), 4);
    }
}
