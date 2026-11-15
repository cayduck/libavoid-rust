//! Faithful port of upstream `libavoid/scanline.{h,cpp}`.
//!
//! # Data model
//!
//! * `Node` objects (upstream `new Node(..)` / `delete v`) live in a
//!   per-sweep [`NodeArena`] and are named by [`NodeIdx`] (the index in the
//!   arena, i.e. the creation order within the sweep).  A deleted node simply
//!   stays in the arena until the sweep finishes.
//! * `NodeSet` (`std::set<Node*, CmpNodePos>`) is a [`NodeSet`] of
//!   [`NodeKey`]s.  `Node::iter` is not stored: the key of a node is
//!   recomputed from the node (`pos` and the base-object serial never change
//!   while it is in the set).
//! * `Event`s are plain values in a `Vec`.
//!
//! # Pointer ordering (`// PTR-ORDER:`)
//! * `CmpNodePos` breaks position ties by comparing the *addresses of the
//!   base objects* (`Obstacle*`, `VertInf*` or `ShiftSegment*`, possibly of
//!   different types).  Here we compare creation serials taken from the one
//!   global counter [`Router::next_serial`] (obstacles, vertices, edges and
//!   shift segments all draw from it), i.e. "pointer order == allocation
//!   order".  That is what a monotonic allocator gives upstream; with
//!   glibc's malloc it holds for a fresh heap only.
//! * `compare_events` breaks `(pos, type)` ties with `(int)(ea->v - eb->v)`,
//!   the difference of two `Node*`s.  Nodes are allocated in a loop right
//!   before the sort, so we use the node's creation index in the sweep.
//!
//! # `ShiftSegment` extension point
//! Upstream `ShiftSegment` is an abstract base class with two concrete
//! subclasses, `NudgingShiftSegment` (orthogonal.cpp) and
//! `HyperedgeShiftSegment` (hyperedgeimprover.cpp).  Here:
//! * the base-class data members live in [`ShiftSegmentBase`] (embed one in
//!   the concrete struct, create it with [`ShiftSegmentBase::new`] at the
//!   point where upstream does `new XxxShiftSegment(..)` so its `serial`
//!   models the allocation order);
//! * the pure virtual interface is the trait [`ShiftSegment<C>`], generic
//!   over a context type `C` through which the segment can reach the data
//!   its `lowPoint()`/`highPoint()` refer to (`Router` for nudging segments,
//!   whose points live in `ConnRef::displayRoute()`; the hyperedge
//!   improver's node storage for hyperedge segments).  Points are returned
//!   by value; subclasses add their own mutators.
//! * `ShiftSegmentList` (`std::list<ShiftSegment *>`) is any slice of
//!   segments, `&mut [S]` with `S: ShiftSegment<C>` (blanket impls make
//!   `Box<T>` and `&mut T` usable as `S`).

#![allow(dead_code)]
#![allow(
    clippy::collapsible_if,
    clippy::collapsible_else_if,
    clippy::needless_range_loop,
    clippy::nonminimal_bool
)]

use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::ops::Bound;

use crate::avoid::arena::{ObstacleIdx, VertIdx};
use crate::avoid::connector::ConnType;
use crate::avoid::geometry::point_on_line;
use crate::avoid::geomtypes::{cpp_max, cpp_min, Point, XDIM, YDIM};
use crate::avoid::obstacle::Obstacle;
use crate::avoid::router::Router;

// upstream: scanline.h CHANNEL_MAX
pub const CHANNEL_MAX: f64 = 100000000.0;

const DBL_MAX: f64 = f64::MAX;

//============================================================================
//                              ShiftSegment
//============================================================================

/// Data members of upstream's abstract `class ShiftSegment`.
// upstream: scanline.h class ShiftSegment
#[derive(Clone, Copy, Debug)]
pub struct ShiftSegmentBase {
    pub dimension: usize,
    /// Uninitialised upstream (every subclass constructor sets it); 0.0 here.
    pub min_space_limit: f64,
    /// Uninitialised upstream (every subclass constructor sets it); 0.0 here.
    pub max_space_limit: f64,
    /// Monotonic creation serial modelling the `ShiftSegment*` address
    /// (`// PTR-ORDER:` in `CmpNodePos`).
    pub serial: u64,
}

impl ShiftSegmentBase {
    /// Call where upstream allocates the concrete segment
    /// (`new NudgingShiftSegment(..)` / `new HyperedgeShiftSegment(..)`).
    // upstream: scanline.h ShiftSegment::ShiftSegment(const size_t dim)
    pub fn new(router: &mut Router, dim: usize) -> ShiftSegmentBase {
        ShiftSegmentBase {
            dimension: dim,
            min_space_limit: 0.0,
            max_space_limit: 0.0,
            serial: router.next_serial(),
        }
    }
}

/// Upstream's pure virtual `ShiftSegment` interface.  `C` is the context
/// the segment needs to resolve its end points (see module docs).
// upstream: scanline.h class ShiftSegment
pub trait ShiftSegment<C: ?Sized> {
    fn base(&self) -> &ShiftSegmentBase;
    fn base_mut(&mut self) -> &mut ShiftSegmentBase;
    // upstream: scanline.h ShiftSegment::lowPoint()
    fn low_point(&self, ctx: &C) -> Point;
    // upstream: scanline.h ShiftSegment::highPoint()
    fn high_point(&self, ctx: &C) -> Point;
    // upstream: scanline.h ShiftSegment::overlapsWith()
    fn overlaps_with(&self, rhs: &Self, dim: usize, ctx: &C) -> bool;
    // upstream: scanline.h ShiftSegment::immovable()
    fn immovable(&self, ctx: &C) -> bool;
}

impl<C: ?Sized, T: ShiftSegment<C> + ?Sized> ShiftSegment<C> for Box<T> {
    fn base(&self) -> &ShiftSegmentBase {
        (**self).base()
    }
    fn base_mut(&mut self) -> &mut ShiftSegmentBase {
        (**self).base_mut()
    }
    fn low_point(&self, ctx: &C) -> Point {
        (**self).low_point(ctx)
    }
    fn high_point(&self, ctx: &C) -> Point {
        (**self).high_point(ctx)
    }
    fn overlaps_with(&self, rhs: &Self, dim: usize, ctx: &C) -> bool {
        (**self).overlaps_with(&**rhs, dim, ctx)
    }
    fn immovable(&self, ctx: &C) -> bool {
        (**self).immovable(ctx)
    }
}

impl<C: ?Sized, T: ShiftSegment<C> + ?Sized> ShiftSegment<C> for &mut T {
    fn base(&self) -> &ShiftSegmentBase {
        (**self).base()
    }
    fn base_mut(&mut self) -> &mut ShiftSegmentBase {
        (**self).base_mut()
    }
    fn low_point(&self, ctx: &C) -> Point {
        (**self).low_point(ctx)
    }
    fn high_point(&self, ctx: &C) -> Point {
        (**self).high_point(ctx)
    }
    fn overlaps_with(&self, rhs: &Self, dim: usize, ctx: &C) -> bool {
        (**self).overlaps_with(&**rhs, dim, ctx)
    }
    fn immovable(&self, ctx: &C) -> bool {
        (**self).immovable(ctx)
    }
}

//============================================================================
//                                 Node
//============================================================================

/// Index of a [`Node`] in its sweep's [`NodeArena`] (creation order).
pub type NodeIdx = usize;

// upstream: scanline.h class Node
#[derive(Clone, Debug)]
pub struct Node {
    pub v: Option<ObstacleIdx>,
    pub c: Option<VertIdx>,
    /// Index of the shift segment in the `ShiftSegmentList` slice.
    pub ss: Option<usize>,
    /// Creation serial of the base object (`v`, else `c`, else `ss`);
    /// stands in for its address in `CmpNodePos`.
    pub base_serial: u64,
    pub pos: f64,
    pub min: [f64; 2],
    pub max: [f64; 2],
    pub first_above: Option<NodeIdx>,
    pub first_below: Option<NodeIdx>,
}

/// Owner of all `Node`s of one sweep (upstream heap allocations).
#[derive(Clone, Debug, Default)]
pub struct NodeArena {
    pub nodes: Vec<Node>,
}

impl std::ops::Index<NodeIdx> for NodeArena {
    type Output = Node;
    fn index(&self, i: NodeIdx) -> &Node {
        &self.nodes[i]
    }
}

impl std::ops::IndexMut<NodeIdx> for NodeArena {
    fn index_mut(&mut self, i: NodeIdx) -> &mut Node {
        &mut self.nodes[i]
    }
}

impl NodeArena {
    pub fn new() -> NodeArena {
        NodeArena { nodes: Vec::new() }
    }

    fn push(&mut self, node: Node) -> NodeIdx {
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    // upstream: scanline.cpp Node::Node(Obstacle *v, const double p)
    pub fn new_obstacle_node(&mut self, router: &Router, v: ObstacleIdx, p: f64) -> NodeIdx {
        let b_box = Obstacle::routing_box(router, v);
        let mut min = [0.0; 2];
        let mut max = [0.0; 2];
        min[XDIM] = b_box.min.x;
        min[YDIM] = b_box.min.y;
        max[XDIM] = b_box.max.x;
        max[YDIM] = b_box.max.y;
        self.push(Node {
            v: Some(v),
            c: None,
            ss: None,
            // PTR-ORDER: `Obstacle*` address -> obstacle creation serial.
            base_serial: router.obstacles[v].serial,
            pos: p,
            min,
            max,
            first_above: None,
            first_below: None,
        })
    }

    // upstream: scanline.cpp Node::Node(VertInf *c, const double p)
    pub fn new_vert_node(&mut self, router: &Router, c: VertIdx, p: f64) -> NodeIdx {
        let point = router.vert_infs[c].point;
        let mut min = [0.0; 2];
        let mut max = [0.0; 2];
        min[XDIM] = point.x;
        max[XDIM] = point.x;
        min[YDIM] = point.y;
        max[YDIM] = point.y;
        self.push(Node {
            v: None,
            c: Some(c),
            ss: None,
            // PTR-ORDER: `VertInf*` address -> vertex creation serial.
            base_serial: router.vert_infs[c].serial,
            pos: p,
            min,
            max,
            first_above: None,
            first_below: None,
        })
    }

    /// `ss` is the segment's index in the segment slice, `ss_serial` its
    /// `ShiftSegmentBase::serial`.
    // upstream: scanline.cpp Node::Node(ShiftSegment *ss, const double p)
    pub fn new_segment_node(&mut self, ss: usize, ss_serial: u64, p: f64) -> NodeIdx {
        // These values shouldn't ever be used, so they don't matter.
        self.push(Node {
            v: None,
            c: None,
            ss: Some(ss),
            // PTR-ORDER: `ShiftSegment*` address -> segment creation serial.
            base_serial: ss_serial,
            pos: p,
            min: [0.0; 2],
            max: [0.0; 2],
            first_above: None,
            first_below: None,
        })
    }

    /// Key of the node in a `NodeSet`.
    pub fn key(&self, n: NodeIdx) -> NodeKey {
        NodeKey {
            pos: self.nodes[n].pos,
            ptr: self.nodes[n].base_serial,
            node: n,
        }
    }

    // Find the first Node above in the scanline that is a shape edge,
    // and does not have an open or close event at this position (meaning
    // it is just about to be removed).
    // upstream: scanline.cpp Node::firstObstacleAbove()
    pub fn first_obstacle_above(&self, this: NodeIdx, dim: usize) -> f64 {
        let pos = self.nodes[this].pos;
        let mut curr = self.nodes[this].first_above;
        while let Some(c) = curr {
            if self.nodes[c].ss.is_some() || (self.nodes[c].max[dim] > pos) {
                curr = self.nodes[c].first_above;
            } else {
                break;
            }
        }

        if let Some(c) = curr {
            return self.nodes[c].max[dim];
        }
        -DBL_MAX
    }

    // Find the first Node below in the scanline that is a shape edge,
    // and does not have an open or close event at this position (meaning
    // it is just about to be removed).
    // upstream: scanline.cpp Node::firstObstacleBelow()
    pub fn first_obstacle_below(&self, this: NodeIdx, dim: usize) -> f64 {
        let pos = self.nodes[this].pos;
        let mut curr = self.nodes[this].first_below;
        while let Some(c) = curr {
            if self.nodes[c].ss.is_some() || (self.nodes[c].min[dim] < pos) {
                curr = self.nodes[c].first_below;
            } else {
                break;
            }
        }

        if let Some(c) = curr {
            return self.nodes[c].min[dim];
        }
        DBL_MAX
    }

    // Mark all connector segments above in the scanline as being able
    // to see to this shape edge.
    // upstream: scanline.cpp Node::markShiftSegmentsAbove()
    pub fn mark_shift_segments_above<C: ?Sized, S: ShiftSegment<C>>(
        &self,
        this: NodeIdx,
        dim: usize,
        segments: &mut [S],
    ) {
        let min_d = self.nodes[this].min[dim];
        let mut curr = self.nodes[this].first_above;
        while let Some(c) = curr {
            let cn = &self.nodes[c];
            if !(cn.ss.is_some() || (cn.pos > min_d)) {
                break;
            }
            if let Some(ss) = cn.ss {
                if cn.pos <= min_d {
                    let b = segments[ss].base_mut();
                    b.max_space_limit = cpp_min(min_d, b.max_space_limit);
                }
            }
            curr = cn.first_above;
        }
    }

    // Mark all connector segments below in the scanline as being able
    // to see to this shape edge.
    // upstream: scanline.cpp Node::markShiftSegmentsBelow()
    pub fn mark_shift_segments_below<C: ?Sized, S: ShiftSegment<C>>(
        &self,
        this: NodeIdx,
        dim: usize,
        segments: &mut [S],
    ) {
        let max_d = self.nodes[this].max[dim];
        let mut curr = self.nodes[this].first_below;
        while let Some(c) = curr {
            let cn = &self.nodes[c];
            if !(cn.ss.is_some() || (cn.pos < max_d)) {
                break;
            }
            if let Some(ss) = cn.ss {
                if cn.pos >= max_d {
                    let b = segments[ss].base_mut();
                    b.min_space_limit = cpp_max(max_d, b.min_space_limit);
                }
            }
            curr = cn.first_below;
        }
    }

    /// Returns `(firstAbovePos, firstBelowPos, lastAbovePos, lastBelowPos)`
    /// (upstream's four reference out-parameters, in order).
    /// `inside_parent_shapes` is `Router::route_inside_parent_shapes()`
    /// (false: upstream).
    // upstream: scanline.cpp Node::findFirstPointAboveAndBelow()
    pub fn find_first_point_above_and_below(
        &self,
        this: NodeIdx,
        dim: usize,
        line_pos: f64,
        inside_parent_shapes: bool,
    ) -> (f64, f64, f64, f64) {
        let me = &self.nodes[this];
        let not_dim = if dim == 0 { 1 } else { 0 };
        let mut first_above_pos = -DBL_MAX;
        let mut first_below_pos = DBL_MAX;
        // We start looking left from the right side of the shape,
        // and vice versa.
        let mut last_above_pos = me.max[dim];
        let mut last_below_pos = me.min[dim];

        for direction in 0..2 {
            // Look for obstacles in one direction, then the other.
            let mut curr = if direction == 0 { me.first_above } else { me.first_below };

            while let Some(c) = curr {
                let cn = &self.nodes[c];
                // The events are at a shared beginning or end of a shape or
                // connection point.  Note, connection points have the same
                // min and max value in the !dim dimension.
                let events_at_same_pos = ((line_pos == me.max[not_dim]) && (line_pos == cn.max[not_dim]))
                    || ((line_pos == me.min[not_dim]) && (line_pos == cn.min[not_dim]));

                if cn.max[dim] <= me.min[dim] {
                    // Curr shape is completely to the left,
                    // so add it's right side as a limit
                    first_above_pos = cpp_max(cn.max[dim], first_above_pos);
                } else if cn.min[dim] >= me.max[dim] {
                    // Curr shape is completely to the right,
                    // so add it's left side as a limit
                    first_below_pos = cpp_min(cn.min[dim], first_below_pos);
                } else if !events_at_same_pos {
                    if inside_parent_shapes && self.is_shape_in_shape(this, c) {
                        // fork: Aksem/adaptagrams scanline.cpp Node::findFirstPointAboveAndBelow()
                        // `curr` encloses this shape: it limits the view
                        // only with its far sides.
                        last_above_pos = cpp_max(cn.max[dim], last_above_pos);
                        last_below_pos = cpp_min(cn.min[dim], last_below_pos);
                    } else {
                        // Shapes that open or close at the same position do not
                        // block visibility, so if they are not at same position
                        // determine where they overlap.
                        last_above_pos = cpp_min(cn.min[dim], last_above_pos);
                        last_below_pos = cpp_max(cn.max[dim], last_below_pos);
                    }
                }
                curr = if direction == 0 { cn.first_above } else { cn.first_below };
            }
        }
        (first_above_pos, first_below_pos, last_above_pos, last_below_pos)
    }

    // upstream: scanline.cpp Node::firstPointAbove()
    pub fn first_point_above(&self, this: NodeIdx, dim: usize) -> f64 {
        // We are looking for the first obstacle above this position,
        // though we ignore shape edges if this point is inline with
        // the edge of the obstacle.  That is, points have visibility
        // along the edge of shapes.
        let me = &self.nodes[this];
        let alt_dim = (dim + 1) % 2;
        let mut result = -DBL_MAX;
        let mut curr = me.first_above;
        while let Some(c) = curr {
            let cn = &self.nodes[c];
            let in_line_with_edge = (me.min[alt_dim] == cn.min[alt_dim]) || (me.min[alt_dim] == cn.max[alt_dim]);
            if !in_line_with_edge && (cn.max[dim] <= me.pos) {
                result = cpp_max(cn.max[dim], result);
            }
            curr = cn.first_above;
        }
        result
    }

    // upstream: scanline.cpp Node::firstPointBelow()
    pub fn first_point_below(&self, this: NodeIdx, dim: usize) -> f64 {
        // We are looking for the first obstacle below this position,
        // though we ignore shape edges if this point is inline with
        // the edge of the obstacle.  That is, points have visibility
        // along the edge of shapes.
        let me = &self.nodes[this];
        let alt_dim = (dim + 1) % 2;
        let mut result = DBL_MAX;
        let mut curr = me.first_below;
        while let Some(c) = curr {
            let cn = &self.nodes[c];
            let in_line_with_edge = (me.min[alt_dim] == cn.min[alt_dim]) || (me.min[alt_dim] == cn.max[alt_dim]);
            if !in_line_with_edge && (cn.min[dim] >= me.pos) {
                result = cpp_min(cn.min[dim], result);
            }
            curr = cn.first_below;
        }
        result
    }

    /// Whether this node and `another` are shapes and `another` encloses
    /// this one.
    // fork: Aksem/adaptagrams scanline.cpp Node::isShapeInShape()
    pub fn is_shape_in_shape(&self, this: NodeIdx, another: NodeIdx) -> bool {
        let (me, other) = (&self.nodes[this], &self.nodes[another]);
        me.v.is_some()
            && other.v.is_some()
            && me.min[0] >= other.min[0]
            && me.min[1] >= other.min[1]
            && me.max[0] <= other.max[0]
            && me.max[1] <= other.max[1]
    }

    // This is a bit inefficient, but we won't need to do it once we have
    // connection points.
    // upstream: scanline.cpp Node::isInsideShape()
    pub fn is_inside_shape(&self, this: NodeIdx, dimension: usize) -> bool {
        let pos = self.nodes[this].pos;
        let mut curr = self.nodes[this].first_below;
        while let Some(c) = curr {
            if (self.nodes[c].min[dimension] < pos) && (pos < self.nodes[c].max[dimension]) {
                return true;
            }
            curr = self.nodes[c].first_below;
        }
        let mut curr = self.nodes[this].first_above;
        while let Some(c) = curr {
            if (self.nodes[c].min[dimension] < pos) && (pos < self.nodes[c].max[dimension]) {
                return true;
            }
            curr = self.nodes[c].first_above;
        }
        false
    }

    /// The neighbour bookkeeping done after `scanline.insert(v)` in
    /// processShiftEvent / processEventVert / processEventHori.
    pub fn insert_into_scanline(&mut self, scanline: &mut NodeSet, v: NodeIdx) {
        let key = self.key(v);
        let inserted = scanline.insert(key);
        debug_assert!(inserted);

        // Work out neighbours
        if let Some(u) = scanline.prev(&key) {
            self.nodes[v].first_above = Some(u);
            self.nodes[u].first_below = Some(v);
        }
        if let Some(u) = scanline.next(&key) {
            self.nodes[v].first_below = Some(u);
            self.nodes[u].first_above = Some(v);
        }
    }

    /// The "Clean up neighbour pointers" block preceding the scanline
    /// erase in processShiftEvent / processEventVert / processEventHori.
    pub fn unlink_neighbours(&mut self, v: NodeIdx) {
        let l = self.nodes[v].first_above;
        let r = self.nodes[v].first_below;
        if let Some(l) = l {
            self.nodes[l].first_below = self.nodes[v].first_below;
        }
        if let Some(r) = r {
            self.nodes[r].first_above = self.nodes[v].first_above;
        }
    }
}

//============================================================================
//                         CmpNodePos / NodeSet
//============================================================================

/// Element of a [`NodeSet`]; ordered exactly like upstream `CmpNodePos`
/// (`node` does not take part in the comparison).
#[derive(Clone, Copy, Debug)]
pub struct NodeKey {
    pub pos: f64,
    /// Serial of the base object (PTR-ORDER).
    pub ptr: u64,
    pub node: NodeIdx,
}

// upstream: scanline.cpp CmpNodePos::operator()
pub fn cmp_node_pos(u: &NodeKey, v: &NodeKey) -> bool {
    if u.pos != v.pos {
        return u.pos < v.pos;
    }

    // Use the pointers to the base objects to differentiate them.
    // PTR-ORDER: base-object addresses (`Obstacle*`/`VertInf*`/
    // `ShiftSegment*`) -> creation serials from the router-wide counter.
    u.ptr < v.ptr
}

impl Ord for NodeKey {
    fn cmp(&self, other: &Self) -> Ordering {
        if cmp_node_pos(self, other) {
            Ordering::Less
        } else if cmp_node_pos(other, self) {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}
impl PartialOrd for NodeKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for NodeKey {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for NodeKey {}

/// Upstream `typedef std::set<Node*,CmpNodePos> NodeSet`.
#[derive(Clone, Debug, Default)]
pub struct NodeSet {
    set: BTreeSet<NodeKey>,
}

impl NodeSet {
    pub fn new() -> NodeSet {
        NodeSet { set: BTreeSet::new() }
    }

    /// `insert(v).second`.
    pub fn insert(&mut self, key: NodeKey) -> bool {
        self.set.insert(key)
    }

    /// The node preceding `key`'s position (`*(--it)` if `it != begin()`).
    pub fn prev(&self, key: &NodeKey) -> Option<NodeIdx> {
        self.set.range((Bound::Unbounded, Bound::Excluded(*key))).next_back().map(|k| k.node)
    }

    /// The node following `key`'s position (`*(++it)` if `!= end()`).
    pub fn next(&self, key: &NodeKey) -> Option<NodeIdx> {
        self.set.range((Bound::Excluded(*key), Bound::Unbounded)).next().map(|k| k.node)
    }

    /// `erase(v)`: number of elements removed.
    pub fn erase(&mut self, key: &NodeKey) -> usize {
        self.set.remove(key) as usize
    }

    pub fn len(&self) -> usize {
        self.set.len()
    }

    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }

    /// In-order node handles.
    pub fn iter(&self) -> impl Iterator<Item = NodeIdx> + '_ {
        self.set.iter().map(|k| k.node)
    }
}

//============================================================================
//                                 Event
//============================================================================

// Note: Open must come first.
// upstream: scanline.h enum EventType
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventType {
    Open = 1,
    SegOpen = 2,
    ConnPoint = 3,
    SegClose = 4,
    Close = 5,
}

// upstream: scanline.h struct Event
#[derive(Clone, Copy, Debug)]
pub struct Event {
    pub type_: EventType,
    pub v: NodeIdx,
    pub pos: f64,
}

impl Event {
    // upstream: scanline.cpp Event::Event()
    pub fn new(t: EventType, v: NodeIdx, p: f64) -> Event {
        Event { type_: t, v, pos: p }
    }
}

// Used for quicksort.  Must return <0, 0, or >0.
// upstream: scanline.cpp compare_events()
pub fn compare_events(ea: &Event, eb: &Event) -> i32 {
    if ea.pos != eb.pos {
        return if ea.pos < eb.pos { -1 } else { 1 };
    }
    if ea.type_ != eb.type_ {
        return (ea.type_ as i32) - (eb.type_ as i32);
    }
    debug_assert!(ea.v != eb.v);
    // PTR-ORDER: `(int)(ea->v - eb->v)` on `Node*`s; nodes are allocated in
    // a loop just before sorting, so creation index order.
    match ea.v.cmp(&eb.v) {
        Ordering::Less => -1,
        Ordering::Greater => 1,
        Ordering::Equal => 0,
    }
}

/// Upstream `qsort(events, totalEvents, sizeof(Event*), compare_events)`.
///
/// `compare_events` is a strict total order on the events of one sweep
/// (every node has at most one event of each type, and distinct nodes
/// compare by creation order), so the sorted order is unique and the
/// stability of the sort algorithm (glibc's qsort is a merge sort for
/// arrays that fit in memory, but not guaranteed stable) cannot matter.
pub fn sort_events(events: &mut [Event]) {
    events.sort_by(|a, b| compare_events(a, b).cmp(&0));
}

//============================================================================
//                     Checkpoint cache / channel info
//============================================================================

// upstream: scanline.cpp buildConnectorRouteCheckpointCache()
pub fn build_connector_route_checkpoint_cache(router: &mut Router) {
    let conn_refs = router.conn_refs.clone();
    for conn in conn_refs {
        if router.conns[conn].routing_type() != ConnType::ConnTypeOrthogonal {
            continue;
        }

        // (`displayRoute()` computes the display route if unset.)
        let _ = router.conns[conn].display_route();
        let checkpoints = router.conns[conn].routing_checkpoints();
        let display_route = &mut router.conns[conn].m_display_route;

        // Initialise checkpoint vector and set to false.  There will be
        // one entry for each *segment* in the path, and the value indicates
        // whether the segment is affected by a checkpoint.
        display_route.checkpoints_on_route = Vec::new();

        for ind in 0..display_route.ps.len() {
            if ind > 0 {
                for cpi in 0..checkpoints.len() {
                    if point_on_line(&display_route.ps[ind - 1], &display_route.ps[ind], &checkpoints[cpi].point, 0.0) {
                        // The checkpoint is on a segment.
                        display_route.checkpoints_on_route.push(((ind * 2) - 1, checkpoints[cpi].point));
                    }
                }
            }

            for cpi in 0..checkpoints.len() {
                if display_route.ps[ind].equals_default(&checkpoints[cpi].point) {
                    // The checkpoint is at a bendpoint.
                    display_route.checkpoints_on_route.push((ind * 2, checkpoints[cpi].point));
                }
            }
        }
    }
}

// upstream: scanline.cpp clearConnectorRouteCheckpointCache()
pub fn clear_connector_route_checkpoint_cache(router: &mut Router) {
    let conn_refs = router.conn_refs.clone();
    for conn in conn_refs {
        if router.conns[conn].routing_type() != ConnType::ConnTypeOrthogonal {
            continue;
        }

        // Clear the cache.
        let _ = router.conns[conn].display_route();
        router.conns[conn].m_display_route.checkpoints_on_route.clear();
    }
}

// Processes sweep events used to determine each horizontal and vertical
// line segment in a connector's channel of visibility.
// Four calls to this function are made at each position by the scanline:
//   1) Handle all Close event processing.
//   2) Remove Close event objects from the scanline.
//   3) Add Open event objects to the scanline.
//   4) Handle all Open event processing.
//
// upstream: scanline.cpp processShiftEvent()
fn process_shift_event<C: ?Sized, S: ShiftSegment<C>>(
    nodes: &mut NodeArena,
    scanline: &mut NodeSet,
    segments: &mut [S],
    e: &Event,
    dim: usize,
    pass: u32,
) {
    let v = e.v;

    if ((pass == 3) && (e.type_ == EventType::Open)) || ((pass == 3) && (e.type_ == EventType::SegOpen)) {
        nodes.insert_into_scanline(scanline, v);
    }

    if ((pass == 4) && (e.type_ == EventType::Open))
        || ((pass == 4) && (e.type_ == EventType::SegOpen))
        || ((pass == 1) && (e.type_ == EventType::SegClose))
        || ((pass == 1) && (e.type_ == EventType::Close))
    {
        if let Some(ss) = nodes[v].ss {
            // As far as we can see.
            let min_limit = nodes.first_obstacle_above(v, dim);
            let max_limit = nodes.first_obstacle_below(v, dim);

            let b = segments[ss].base_mut();
            b.min_space_limit = cpp_max(min_limit, b.min_space_limit);
            b.max_space_limit = cpp_min(max_limit, b.max_space_limit);
        } else {
            nodes.mark_shift_segments_above(v, dim, segments);
            nodes.mark_shift_segments_below(v, dim, segments);
        }
    }

    if ((pass == 2) && (e.type_ == EventType::SegClose)) || ((pass == 2) && (e.type_ == EventType::Close)) {
        // Clean up neighbour pointers.
        nodes.unlink_neighbours(v);

        let key = nodes.key(v);
        let result = scanline.erase(&key);
        // Upstream: COLA_ASSERT(result == 1).  Not ported (assert policy,
        // router.rs module docs): an obstacle of zero extent in the sweep
        // direction (valid input, tests/robustness.rs zero_size_shapes)
        // has its Close event processed (pass 2) before its Open event
        // (pass 3) at the same position, so nothing is erased and the node
        // stays in the scanline.  The NDEBUG build continues (upstream then
        // keeps a pointer to the deleted node; here the node stays in the
        // arena until the sweep ends, which is what upstream reads).
        let _ = result;
        // (`delete v`: the node stays in the arena until the sweep ends.)
    }
}

/// `segment_list` is upstream's `ShiftSegmentList` (list order); `ctx` is
/// what the segments need to resolve their points (see module docs).  Only
/// `min_space_limit`/`max_space_limit` of the segments are modified.
// upstream: scanline.cpp buildOrthogonalChannelInfo()
pub fn build_orthogonal_channel_info<C: ?Sized, S: ShiftSegment<C>>(
    router: &Router,
    ctx: &C,
    dim: usize,
    segment_list: &mut [S],
) {
    if segment_list.is_empty() {
        // There are no segments, so we can just return now.
        return;
    }

    // Do a sweep to determine space for shifting segments.
    let alt_dim = (dim + 1) % 2;
    let n = router.m_obstacles.len();
    let cpn = segment_list.len();
    // Set up the events for the sweep.
    let mut total_events = 2 * (n + cpn);
    let mut events: Vec<Event> = Vec::with_capacity(total_events);
    let mut nodes = NodeArena::new();
    for &obstacle in router.m_obstacles.iter() {
        if let Some(junction) = Obstacle::as_junction(router, obstacle) {
            if !junction.position_fixed(router) {
                // Junctions that are free to move are not treated as obstacles.
                total_events -= 2;
                continue;
            }
        }
        let b_box = Obstacle::routing_box(router, obstacle);
        let min = b_box.min;
        let max = b_box.max;
        let mid = min[dim] + ((max[dim] - min[dim]) / 2.0);
        let v = nodes.new_obstacle_node(router, obstacle, mid);
        events.push(Event::new(EventType::Open, v, min[alt_dim]));
        events.push(Event::new(EventType::Close, v, max[alt_dim]));
    }
    for (i, seg) in segment_list.iter().enumerate() {
        let low_pt = seg.low_point(ctx);
        let high_pt = seg.high_point(ctx);

        debug_assert!(low_pt[dim] == high_pt[dim]);
        debug_assert!(low_pt[alt_dim] < high_pt[alt_dim]);
        let v = nodes.new_segment_node(i, seg.base().serial, low_pt[dim]);
        events.push(Event::new(EventType::SegOpen, v, low_pt[alt_dim]));
        events.push(Event::new(EventType::SegClose, v, high_pt[alt_dim]));
    }
    debug_assert!(events.len() == total_events);
    sort_events(&mut events);

    // Process the sweep.
    // We do multiple passes over sections of the list so we can add relevant
    // entries to the scanline that might follow, before process them.
    let mut scanline = NodeSet::new();
    let mut this_pos = if total_events > 0 { events[0].pos } else { 0.0 };
    let mut pos_start_index = 0;
    for i in 0..=total_events {
        // If we have finished the current scanline or all events, then we
        // process the events on the current scanline in a couple of passes.
        if (i == total_events) || (events[i].pos != this_pos) {
            let pos_finish_index = i;
            for pass in 2..=4 {
                for j in pos_start_index..pos_finish_index {
                    process_shift_event(&mut nodes, &mut scanline, segment_list, &events[j], dim, pass);
                }
            }

            if i == total_events {
                // We have cleaned up, so we can now break out of loop.
                break;
            }

            this_pos = events[i].pos;
            pos_start_index = i;
        }

        // Do the first sweep event handling -- building the correct
        // structure of the scanline.
        let pass = 1;
        process_shift_event(&mut nodes, &mut scanline, segment_list, &events[i], dim, pass);
    }
    // Upstream: COLA_ASSERT(scanline.empty()).  Not ported (assert
    // policy): fails for zero-extent obstacles, see process_shift_event.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_events_orders_by_pos_type_then_node() {
        let a = Event::new(EventType::Close, 0, 1.0);
        let b = Event::new(EventType::Open, 1, 1.0);
        let c = Event::new(EventType::Open, 2, 1.0);
        let d = Event::new(EventType::Close, 3, 0.5);
        let mut ev = vec![a, b, c, d];
        sort_events(&mut ev);
        let order: Vec<NodeIdx> = ev.iter().map(|e| e.v).collect();
        assert_eq!(order, vec![3, 1, 2, 0]);
        assert!(compare_events(&b, &c) < 0);
        assert!(compare_events(&c, &b) > 0);
    }

    #[test]
    fn node_set_neighbours() {
        let mut s = NodeSet::new();
        let k = |pos: f64, ptr: u64, node: usize| NodeKey { pos, ptr, node };
        assert!(s.insert(k(1.0, 5, 0)));
        assert!(s.insert(k(1.0, 3, 1)));
        assert!(s.insert(k(0.0, 9, 2)));
        assert!(!s.insert(k(1.0, 3, 7)));
        assert_eq!(s.iter().collect::<Vec<_>>(), vec![2, 1, 0]);
        assert_eq!(s.prev(&k(1.0, 3, 1)), Some(2));
        assert_eq!(s.next(&k(1.0, 3, 1)), Some(0));
        assert_eq!(s.next(&k(1.0, 5, 0)), None);
        // -0.0 == 0.0 under `!=`, so the pointer decides.
        assert!(s.insert(k(-0.0, 1, 3)));
        assert_eq!(s.iter().collect::<Vec<_>>(), vec![3, 2, 1, 0]);
        assert_eq!(s.erase(&k(1.0, 3, 99)), 1);
        assert_eq!(s.len(), 3);
    }
}
