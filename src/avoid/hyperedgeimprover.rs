//! Faithful port of upstream `libavoid/hyperedgeimprover.{h,cpp}`.
//!
//! Methods that need router access take `&mut Router`; the improver lives
//! in `Router::m_hyperedge_improver` and [`HyperedgeImprover::execute`]
//! temporarily moves it out of the router while it runs (nothing reached
//! from `execute()` reads the router's improver).
//!
//! # Data model
//! * The hyperedge trees live in [`HyperedgeImprover::m_tree`] (see
//!   `hyperedgetree.rs`: deleted nodes stay readable, as under the
//!   oracle's bump allocator).
//! * `HyperedgeShiftSegment`s (upstream heap objects) live in
//!   [`HyperedgeImprover::m_segments`] and are named by their index;
//!   `ShiftSegmentList`s are `Vec`s of indices.  A segment's `nodes` set is
//!   an `OrderedHENodeSet` in the tree store, which the nodes point back to
//!   (`HyperedgeTreeNode::shiftSegmentNodeSet`), exactly as upstream.
//! * Each segment's `ShiftSegmentBase` is created (taking its serial from
//!   `Router::next_serial`) where upstream does `new HyperedgeShiftSegment`.
//!
//! # Pointer ordering (`// PTR-ORDER:`)
//! `JunctionSet` / `JunctionHyperedgeTreeNodeMap` / `RootSegmentsMap` are
//! keyed by `JunctionRef *`: ordered by junction creation serial.  Their
//! iteration order decides the order in which trees are processed and in
//! which shift segments are prepended to `m_all_shift_segments`.
//!
//! # Upstream quirks preserved
//! * `execute()` passes `m_router` as the `bool forward` argument of
//!   `updateConnEnds()`, i.e. `true`.
//! * `HyperedgeShiftSegment::adjustPosition()` can insert a node into its
//!   set while the node is still in another segment's set; the node then
//!   points at the newer set only, so deleting it leaves it in the older
//!   set (tombstone reads are reproduced by the tree store).
//! * Segments of a tree whose root junction changes during a pass are
//!   never deleted (leaked upstream), so their destructor never resets the
//!   nodes' set pointers.
//!
//! (`outputHyperedgesToSVG` is a no-op unless upstream's `HYPEREDGE_DEBUG`
//! is defined, here the `hyperedge-debug` feature; the SVG itself is
//! available through [`HyperedgeImprover::write_hyperedges_svg`].  The
//! `MAJOR_HYPEREDGE_IMPROVEMENT_DEBUG` reports are compiled in with the
//! `major-hyperedge-improvement-debug` feature.  The unused
//! `CmpHyperedgeSegmentDirOrder` is `#if 0` upstream.)

#![allow(dead_code)]
#![allow(clippy::new_without_default, clippy::collapsible_if, clippy::collapsible_else_if)]
// Keep upstream statement shapes.
#![allow(
    clippy::unnecessary_unwrap,
    clippy::only_used_in_recursion,
    clippy::needless_range_loop,
    clippy::if_same_then_else,
    clippy::print_with_newline
)]

use std::collections::BTreeMap;
use std::ops::Bound;

use crate::avoid::actioninfo::ActionObj;
use crate::avoid::arena::VertIdx;
use crate::avoid::connector::ConnRef;
use crate::avoid::connend::ConnEnd;
use crate::avoid::geometry::point_on_line;
use crate::avoid::geomtypes::{cpp_max, cpp_min, Point};
use crate::avoid::hyperedge::{ConnRefList, HyperedgeNewAndDeletedObjectLists, JunctionRefList};
use crate::avoid::hyperedgetree::{
    junction_key, HeEdgeIdx, HeNodeIdx, HeNodeSetIdx, HyperedgeTreeStore, JunctionHyperedgeTreeNodeMap, JunctionKey,
    JunctionSet,
};
use crate::avoid::junction::JunctionRef;
use crate::avoid::mtst::{vert_key, VertexSet};
use crate::avoid::obstacle::Obstacle;
use crate::avoid::router::Router;
use crate::avoid::scanline::{build_orthogonal_channel_info, ShiftSegment, ShiftSegmentBase, CHANNEL_MAX};
use crate::avoid::vertices::VertID;

/// Index of a `HyperedgeShiftSegment` in `HyperedgeImprover::m_segments`.
pub type HyperedgeShiftSegmentIdx = usize;
// upstream: hyperedgeimprover.h typedef std::list<ShiftSegment *> ShiftSegmentList
pub type ShiftSegmentList = Vec<HyperedgeShiftSegmentIdx>;
/// PTR-ORDER: upstream `std::map<JunctionRef *, ShiftSegmentList>`.
// upstream: hyperedgeimprover.h typedef RootSegmentsMap
pub type RootSegmentsMap = BTreeMap<JunctionKey, ShiftSegmentList>;

// upstream: hyperedgeimprover.cpp class HyperedgeShiftSegment
#[derive(Clone, Debug)]
pub struct HyperedgeShiftSegment {
    pub base: ShiftSegmentBase,
    /// `std::set<HyperedgeTreeNode *, CmpNodesInDim> nodes` (in the tree
    /// store).
    pub nodes: HeNodeSetIdx,
    pub is_immovable: bool,
    m_balance_count: i32,
    m_balance_count_set: bool,
    /// Uninitialised upstream until `setBalanceCount()`; 0.0 here.
    m_next_pos_lower: f64,
    /// Uninitialised upstream until `setBalanceCount()`; 0.0 here.
    m_next_pos_upper: f64,
    m_at_limit: bool,
    /// Set when deleted.
    pub deleted: bool,
}

impl HyperedgeShiftSegment {
    /// Upstream `new HyperedgeShiftSegment(n1, n2, dim, immovable)`.
    // upstream: hyperedgeimprover.cpp HyperedgeShiftSegment::HyperedgeShiftSegment()
    pub fn new(
        router: &mut Router,
        tree: &mut HyperedgeTreeStore,
        n1: HeNodeIdx,
        n2: HeNodeIdx,
        dim: usize,
        immovable: bool,
    ) -> HyperedgeShiftSegment {
        let mut base = ShiftSegmentBase::new(router, dim);
        let nodes = tree.new_node_set((dim + 1) % 2);
        tree.set_insert(nodes, n1);
        tree.set_insert(nodes, n2);
        tree.node_mut(n1).shift_segment_node_set = Some(nodes);
        tree.node_mut(n2).shift_segment_node_set = Some(nodes);

        base.min_space_limit = -CHANNEL_MAX;
        base.max_space_limit = CHANNEL_MAX;
        HyperedgeShiftSegment {
            base,
            nodes,
            is_immovable: immovable,
            m_balance_count: 0,
            m_balance_count_set: false,
            m_next_pos_lower: 0.0,
            m_next_pos_upper: 0.0,
            m_at_limit: false,
            deleted: false,
        }
    }

    /// Upstream `delete segment`.
    // upstream: hyperedgeimprover.cpp HyperedgeShiftSegment::~HyperedgeShiftSegment()
    pub fn destruct(&mut self, tree: &mut HyperedgeTreeStore) {
        for curr in tree.set_nodes(self.nodes) {
            tree.node_mut(curr).shift_segment_node_set = None;
        }
        self.deleted = true;
    }

    #[inline]
    fn dimension(&self) -> usize {
        self.base.dimension
    }

    // upstream: hyperedgeimprover.cpp HyperedgeShiftSegment::lowPoint()
    pub fn low_point_node(&self, tree: &HyperedgeTreeStore) -> HeNodeIdx {
        tree.set_first(self.nodes).expect("HyperedgeShiftSegment::lowPoint(): empty node set")
    }

    // upstream: hyperedgeimprover.cpp HyperedgeShiftSegment::highPoint()
    pub fn high_point_node(&self, tree: &HyperedgeTreeStore) -> HeNodeIdx {
        tree.set_last(self.nodes).expect("HyperedgeShiftSegment::highPoint(): empty node set")
    }

    // Counts the number of segments diverging on each side and returns
    // a count: a negative number if there a more on the lower side,
    // a positive number if there are more on the upper side, or zero if
    // there are an equal number of segments.
    // upstream: hyperedgeimprover.cpp HyperedgeShiftSegment::setBalanceCount()
    pub fn set_balance_count(&mut self, tree: &HyperedgeTreeStore) {
        let dimension = self.dimension();
        let alt_dim = (dimension + 1) % 2;
        self.m_next_pos_lower = self.base.min_space_limit;
        self.m_next_pos_upper = self.base.max_space_limit;
        self.m_balance_count = 0;
        if self.is_immovable {
            self.m_balance_count_set = true;
            return;
        }
        for curr in tree.set_nodes(self.nodes) {
            let curr_point = tree.node(curr).point;
            for &curr_edge in tree.node(curr).edges.iter() {
                let node = tree.edge_follow_from(curr_edge, curr);
                let other_point = tree.node(node).point;
                if curr_point[alt_dim] == other_point[alt_dim] {
                    if other_point[dimension] < curr_point[dimension] {
                        self.m_next_pos_lower = cpp_max(self.m_next_pos_lower, other_point[dimension]);
                        self.m_balance_count -= 1;
                    } else if other_point[dimension] > curr_point[dimension] {
                        self.m_next_pos_upper = cpp_min(self.m_next_pos_upper, other_point[dimension]);
                        self.m_balance_count += 1;
                    }
                }
            }
        }
        self.m_balance_count_set = true;
    }

    // upstream: hyperedgeimprover.cpp HyperedgeShiftSegment::balanceCount()
    pub fn balance_count(&self) -> i32 {
        debug_assert!(self.m_balance_count_set);
        self.m_balance_count
    }

    // upstream: hyperedgeimprover.cpp HyperedgeShiftSegment::adjustPosition()
    pub fn adjust_position(&mut self, tree: &mut HyperedgeTreeStore) {
        debug_assert!(self.m_balance_count_set);
        debug_assert!(self.m_balance_count != 0);
        let dimension = self.dimension();

        let new_pos = if self.m_balance_count < 0 {
            self.m_next_pos_lower
        } else {
            self.m_next_pos_upper
        };
        let limit = if self.m_balance_count < 0 {
            self.base.min_space_limit
        } else {
            self.base.max_space_limit
        };
        if tree.node(self.low_point_node(tree)).point[dimension] == new_pos {
            // If aren't actually moving this segment, then mark it
            // as at-limit.
            // XXX This seems to be because of a segment with an
            //     incorrectly marked limit, possibly due to a
            //     junction positioned within a shape.
            self.m_at_limit = true;
        }

        // (Only the coordinate in `dimension` changes; the set is ordered by
        // the other one.)
        for curr in tree.set_nodes(self.nodes) {
            tree.node_mut(curr).point[dimension] = new_pos;
        }

        if new_pos == limit {
            self.m_at_limit = true;
        }

        // Add nodes from collapsed segments, in case they are not part of
        // a segment that will be merged.
        // (std::set iteration while inserting: step to the in-order
        // successor of the current element each time.)
        let mut curr = tree.set_first(self.nodes);
        while let Some(c) = curr {
            let curr_point = tree.node(c).point;
            let mut i = 0;
            while let Some(&curr_edge) = tree.node(c).edges.get(i) {
                let node = tree.edge_follow_from(curr_edge, c);
                let other_point = tree.node(node).point;
                if curr_point == other_point {
                    tree.set_insert(self.nodes, node);
                    tree.node_mut(node).shift_segment_node_set = Some(self.nodes);
                }
                i += 1;
            }
            curr = tree.set_next(self.nodes, c);
        }
    }

    // upstream: hyperedgeimprover.cpp HyperedgeShiftSegment::overlapsWith()
    pub fn overlaps_with_impl(&self, rhs: &HyperedgeShiftSegment, dim: usize, tree: &HyperedgeTreeStore) -> bool {
        let alt_dim = (dim + 1) % 2;
        let low_pt = tree.node(self.low_point_node(tree)).point;
        let high_pt = tree.node(self.high_point_node(tree)).point;
        let rhs_low_pt = tree.node(rhs.low_point_node(tree)).point;
        let rhs_high_pt = tree.node(rhs.high_point_node(tree)).point;
        if (low_pt[alt_dim] <= rhs_high_pt[alt_dim]) && (rhs_low_pt[alt_dim] <= high_pt[alt_dim]) {
            // The segments overlap.
            if (self.base.min_space_limit <= rhs.base.max_space_limit)
                && (rhs.base.min_space_limit <= self.base.max_space_limit)
            {
                return true;
            }
        }
        false
    }

    // upstream: hyperedgeimprover.cpp HyperedgeShiftSegment::settled()
    pub fn settled(&self) -> bool {
        self.is_immovable || self.m_at_limit || (self.balance_count() == 0)
    }
}

impl ShiftSegment<HyperedgeTreeStore> for HyperedgeShiftSegment {
    fn base(&self) -> &ShiftSegmentBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut ShiftSegmentBase {
        &mut self.base
    }
    // upstream: hyperedgeimprover.cpp HyperedgeShiftSegment::lowPoint()
    fn low_point(&self, ctx: &HyperedgeTreeStore) -> Point {
        ctx.node(self.low_point_node(ctx)).point
    }
    // upstream: hyperedgeimprover.cpp HyperedgeShiftSegment::highPoint()
    fn high_point(&self, ctx: &HyperedgeTreeStore) -> Point {
        ctx.node(self.high_point_node(ctx)).point
    }
    // upstream: hyperedgeimprover.cpp HyperedgeShiftSegment::overlapsWith()
    fn overlaps_with(&self, rhs: &Self, dim: usize, ctx: &HyperedgeTreeStore) -> bool {
        self.overlaps_with_impl(rhs, dim, ctx)
    }
    // upstream: hyperedgeimprover.cpp HyperedgeShiftSegment::immovable()
    fn immovable(&self, _ctx: &HyperedgeTreeStore) -> bool {
        self.is_immovable
    }
}

// upstream: hyperedgeimprover.h class HyperedgeImprover
#[derive(Clone, Debug, Default)]
pub struct HyperedgeImprover {
    pub m_hyperedge_tree_junctions: JunctionHyperedgeTreeNodeMap,
    pub m_hyperedge_tree_roots: JunctionSet,
    pub m_root_shift_segments: RootSegmentsMap,
    pub m_all_shift_segments: ShiftSegmentList,
    pub m_new_junctions: JunctionRefList,
    pub m_deleted_junctions: JunctionRefList,
    pub m_new_connectors: ConnRefList,
    pub m_deleted_connectors: ConnRefList,
    pub m_changed_connectors: ConnRefList,
    pub m_debug_count: i32,
    /// Uninitialised until `execute()` upstream; false here.
    pub m_can_make_major_changes: bool,

    /// Storage of the hyperedge tree objects (upstream heap objects).
    pub m_tree: HyperedgeTreeStore,
    /// Storage of the `HyperedgeShiftSegment`s (upstream heap objects).
    pub m_segments: Vec<HyperedgeShiftSegment>,
}

impl HyperedgeImprover {
    // Constructor.
    // upstream: hyperedgeimprover.cpp HyperedgeImprover::HyperedgeImprover()
    pub fn new() -> HyperedgeImprover {
        let mut h = HyperedgeImprover::default();
        h.clear();
        h
    }

    // upstream: hyperedgeimprover.cpp HyperedgeImprover::clear()
    pub fn clear(&mut self) {
        self.m_hyperedge_tree_junctions.clear();
        self.m_hyperedge_tree_roots.clear();
        self.m_root_shift_segments.clear();
        self.m_all_shift_segments.clear();
        self.m_new_junctions.clear();
        self.m_deleted_junctions.clear();
        self.m_new_connectors.clear();
        self.m_deleted_connectors.clear();
        self.m_changed_connectors.clear();
        self.m_debug_count = 0;
        // (Release the storage of objects from a previous execute().)
        self.m_tree.clear();
        self.m_segments.clear();
    }

    // upstream: hyperedgeimprover.cpp HyperedgeImprover::newAndDeletedObjectLists()
    pub fn new_and_deleted_object_lists(&self) -> HyperedgeNewAndDeletedObjectLists {
        HyperedgeNewAndDeletedObjectLists {
            new_junction_list: self.m_new_junctions.clone(),
            deleted_junction_list: self.m_deleted_junctions.clone(),
            new_connector_list: self.m_new_connectors.clone(),
            deleted_connector_list: self.m_deleted_connectors.clone(),
            changed_connector_list: self.m_changed_connectors.clone(),
        }
    }

    /// `m_hyperedge_tree_junctions[junction]` for a junction known to be in
    /// the map (upstream `operator[]` would insert a null node otherwise and
    /// then dereference it).
    fn tree_junction_node(&self, junction: &JunctionKey) -> HeNodeIdx {
        *self
            .m_hyperedge_tree_junctions
            .get(junction)
            .expect("HyperedgeImprover: junction missing from m_hyperedge_tree_junctions")
    }

    /// In-order successor of `key` in `m_hyperedge_tree_roots` (models
    /// `++it` on a std::set that may be modified during the iteration).
    fn next_root(&self, key: &JunctionKey) -> Option<JunctionKey> {
        self.m_hyperedge_tree_roots
            .range((Bound::Excluded(*key), Bound::Unbounded))
            .next()
            .copied()
    }

    fn first_root(&self) -> Option<JunctionKey> {
        self.m_hyperedge_tree_roots.iter().next().copied()
    }

    /// Upstream `new HyperedgeShiftSegment(..)`; returns its index.
    fn new_segment(
        &mut self,
        router: &mut Router,
        n1: HeNodeIdx,
        n2: HeNodeIdx,
        dim: usize,
        immovable: bool,
    ) -> HyperedgeShiftSegmentIdx {
        let seg = HyperedgeShiftSegment::new(router, &mut self.m_tree, n1, n2, dim, immovable);
        self.m_segments.push(seg);
        self.m_segments.len() - 1
    }

    /// Upstream `delete segment`.
    fn delete_segment(&mut self, seg: HyperedgeShiftSegmentIdx) {
        let mut s = self.m_segments[seg].clone();
        s.destruct(&mut self.m_tree);
        self.m_segments[seg] = s;
    }

    // Helper method for buildHyperedgeSegments() for hyperedge tree nodes.
    // upstream: hyperedgeimprover.cpp HyperedgeImprover::createShiftSegmentsForDimensionExcluding(HyperedgeTreeNode *, ..)
    fn create_shift_segments_for_dimension_excluding_node(
        &mut self,
        router: &mut Router,
        node: HeNodeIdx,
        dim: usize,
        ignore: Option<HeEdgeIdx>,
        segments: &mut ShiftSegmentList,
    ) {
        let mut i = 0;
        while let Some(&edge) = self.m_tree.node(node).edges.get(i) {
            if Some(edge) != ignore {
                self.create_shift_segments_for_dimension_excluding_edge(router, edge, dim, Some(node), segments);
            }
            i += 1;
        }
    }

    // Helper method for buildHyperedgeSegments() for hyperedge tree edges.
    // upstream: hyperedgeimprover.cpp HyperedgeImprover::createShiftSegmentsForDimensionExcluding(HyperedgeTreeEdge *, ..)
    fn create_shift_segments_for_dimension_excluding_edge(
        &mut self,
        router: &mut Router,
        edge: HeEdgeIdx,
        dim: usize,
        ignore: Option<HeNodeIdx>,
        segments: &mut ShiftSegmentList,
    ) {
        if self.m_tree.edge_has_orientation(edge, dim) && !self.m_tree.edge_zero_length(edge) {
            let ends = self.m_tree.edge(edge).ends;
            let immovable = self.m_tree.node_is_immovable(router, ends.0.unwrap())
                || self.m_tree.node_is_immovable(router, ends.1.unwrap());

            let new_segment = self.new_segment(router, ends.0.unwrap(), ends.1.unwrap(), dim, immovable);
            segments.push(new_segment);
        }

        let first = self.m_tree.edge(edge).ends.0;
        if first.is_some() && (first != ignore) {
            self.create_shift_segments_for_dimension_excluding_node(router, first.unwrap(), dim, Some(edge), segments);
        }

        let second = self.m_tree.edge(edge).ends.1;
        if second.is_some() && (second != ignore) {
            self.create_shift_segments_for_dimension_excluding_node(router, second.unwrap(), dim, Some(edge), segments);
        }
    }

    // upstream: hyperedgeimprover.cpp HyperedgeShiftSegment::mergesWith()
    fn merges_with(&mut self, this: HyperedgeShiftSegmentIdx, other: HyperedgeShiftSegmentIdx) -> bool {
        let tree = &mut self.m_tree;
        let (this_seg, other_seg) = {
            debug_assert!(this != other);
            (self.m_segments[this].clone(), self.m_segments[other].clone())
        };
        let mut this_seg = this_seg;
        let mut other_seg = other_seg;
        let dimension = this_seg.dimension();
        let alt_dim = (dimension + 1) % 2;
        let low_pt = tree.node(this_seg.low_point_node(tree)).point;
        let high_pt = tree.node(this_seg.high_point_node(tree)).point;
        let other_low_pt = tree.node(other_seg.low_point_node(tree)).point;
        let other_high_pt = tree.node(other_seg.high_point_node(tree)).point;
        let result;
        if (low_pt[dimension] == other_low_pt[dimension])
            && (low_pt[alt_dim] <= other_high_pt[alt_dim])
            && (other_low_pt[alt_dim] <= high_pt[alt_dim])
        {
            this_seg.is_immovable |= other_seg.is_immovable;
            this_seg.m_at_limit |= this_seg.m_at_limit;
            this_seg.base.min_space_limit = cpp_max(this_seg.base.min_space_limit, other_seg.base.min_space_limit);
            this_seg.base.max_space_limit = cpp_min(this_seg.base.max_space_limit, other_seg.base.max_space_limit);
            for n in tree.set_nodes(other_seg.nodes) {
                tree.set_insert(this_seg.nodes, n);
            }
            tree.set_clear(other_seg.nodes);
            for curr in tree.set_nodes(this_seg.nodes) {
                tree.node_mut(curr).shift_segment_node_set = Some(this_seg.nodes);
            }
            this_seg.set_balance_count(tree);
            result = true;
        } else {
            this_seg.set_balance_count(tree);
            result = false;
        }
        let _ = &mut other_seg;
        self.m_segments[this] = this_seg;
        result
    }

    // During creation and nudging of shift segments it is often necessary
    // to merge collinear or overlapping segments.  This method does the
    // merging for these cases.  Effectively merging is done by adding
    // additional vertex pointers to the shift segment.
    // upstream: hyperedgeimprover.cpp HyperedgeImprover::mergeOverlappingSegments()
    fn merge_overlapping_segments(&mut self, segments: &mut ShiftSegmentList) {
        let mut curr = 0;
        while curr < segments.len() {
            let edge1 = segments[curr];
            let mut curr2 = 0;
            while curr2 < segments.len() {
                if curr2 == curr {
                    curr2 += 1;
                    continue;
                }
                let edge2 = segments[curr2];
                if self.merges_with(edge1, edge2) {
                    self.delete_segment(edge2);
                    segments.remove(curr2);
                    // (std::list::erase leaves `curr` on the same element.)
                    if curr2 < curr {
                        curr -= 1;
                    }
                } else {
                    curr2 += 1;
                }
            }
            curr += 1;
        }
    }

    // Given a hyperedge tree and a dimension, this method creates shift
    // segments for all edges in that orientation.  These segments are the
    // objects on which the local improvement nudging operates, and they
    // in turn make changes back to the hyperedge tree.
    // upstream: hyperedgeimprover.cpp HyperedgeImprover::buildHyperedgeSegments()
    fn build_hyperedge_segments(&mut self, router: &mut Router, dim: usize) {
        // PTR-ORDER: iterates std::set<JunctionRef *>.
        let roots: Vec<JunctionKey> = self.m_hyperedge_tree_roots.iter().copied().collect();
        for curr in roots {
            let mut segments = self.m_root_shift_segments.remove(&curr).unwrap_or_default();

            let node = self.tree_junction_node(&curr);
            self.create_shift_segments_for_dimension_excluding_node(router, node, dim, None, &mut segments);

            // Merge overlapping segment.
            self.merge_overlapping_segments(&mut segments);

            let mut all = segments.clone();
            all.extend(self.m_all_shift_segments.iter().copied());
            self.m_all_shift_segments = all;
            self.m_root_shift_segments.insert(curr, segments);
        }
    }

    // This method looks for and corrects situations where the middle section
    // of a zigzag is optimised away by bringing the outside segments in line
    // and leading to the middle segment being zero length.  These zero length
    // edges are removed.
    // upstream: hyperedgeimprover.cpp HyperedgeImprover::removeZeroLengthEdges()
    fn remove_zero_length_edges_all(&mut self, router: &Router) {
        // (m_hyperedge_tree_roots may gain an element during the loop.)
        let mut curr = self.first_root();
        while let Some(c) = curr {
            let node = self.tree_junction_node(&c);

            self.remove_zero_length_edges_node(router, node, None);
            curr = self.next_root(&c);
        }
    }

    // This method looks for and correct situations where multiple overlapping
    // edges lead to a junction and one or more of these segments could be
    // removed by moving the junction (and thus divergence point) along the
    // edge.
    // upstream: hyperedgeimprover.cpp HyperedgeImprover::moveJunctionsAlongCommonEdges()
    fn move_junctions_along_common_edges(&mut self, router: &mut Router) {
        // PTR-ORDER: iterates std::map<JunctionRef *, HyperedgeTreeNode *>.
        let mut curr = self.m_hyperedge_tree_junctions.keys().next().copied();
        while let Some(key) = curr {
            let mut node = Some(self.m_hyperedge_tree_junctions[&key]);

            let mut node_map_has_changed = false;
            // For each junction, try and move it.
            loop {
                node = self.move_junction_along_common_edge(router, node.unwrap(), &mut node_map_has_changed);
                match node {
                    Some(n) => {
                        // Junction has moved, rewrite the pointer in
                        // the m_hyperedge_tree_junctions map.
                        self.m_hyperedge_tree_junctions.insert(key, n);
                    }
                    None => break,
                }
            }

            if node_map_has_changed {
                // Objects have been added to m_hyperedge_tree_junctions and
                // this may be earlier than our current iterator, so restart.
                curr = self.m_hyperedge_tree_junctions.keys().next().copied();
            } else {
                curr = self
                    .m_hyperedge_tree_junctions
                    .range((Bound::Excluded(key), Bound::Unbounded))
                    .next()
                    .map(|(k, _)| *k);
            }
        }
    }

    // This method traverses the hyperedge tree removing zero length edges.
    // upstream: hyperedgeimprover.cpp HyperedgeImprover::removeZeroLengthEdges(HyperedgeTreeEdge *, HyperedgeTreeNode *)
    fn remove_zero_length_edges_edge(&mut self, router: &Router, this: HeEdgeIdx, ignored: Option<HeNodeIdx>) {
        let first = self.m_tree.edge(this).ends.0;
        if first != ignored {
            self.remove_zero_length_edges_node(router, first.unwrap(), Some(this));
        }

        let second = self.m_tree.edge(this).ends.1;
        if second != ignored {
            self.remove_zero_length_edges_node(router, second.unwrap(), Some(this));
        }
    }

    // This method traverses the hyperedge tree removing zero length edges.
    // upstream: hyperedgeimprover.cpp HyperedgeImprover::removeZeroLengthEdges(HyperedgeTreeNode *, HyperedgeTreeEdge *)
    fn remove_zero_length_edges_node(&mut self, router: &Router, this: HeNodeIdx, ignored: Option<HeEdgeIdx>) {
        let mut i = 0;
        while let Some(&edge) = self.m_tree.node(this).edges.get(i) {
            if Some(edge) != ignored {
                // UPSTREAM NON-TERMINATION (port addition; router.rs,
                // "Upstream non-termination"): a zero-length edge whose both
                // ends are this node (left by a connector with both ends on
                // one junction) gives other == self, so target == source
                // whenever upstream merges, and spliceEdgesFrom(self) then
                // disconnects and re-appends the same edge forever.  Upstream
                // only terminates here when it does not merge (both nodes
                // junctions without major changes), i.e. it goes straight
                // to the recursive call; the port does that for every such
                // edge, and records when upstream would not have returned
                // (`upstream_self_merge_seen`, used when the tree is
                // written back to the connectors).
                if !self.m_tree.edge(edge).has_fixed_route
                    && self.m_tree.edge_zero_length(edge)
                    && self.m_tree.edge_follow_from(edge, this) == this
                    && (self.m_tree.node(this).junction.is_none() || self.m_can_make_major_changes)
                {
                    self.m_tree.upstream_self_merge_seen = true;
                    crate::avoid::router::e2e_guard_fired("improver-self-merge");
                }
                if !self.m_tree.edge(edge).has_fixed_route
                    && self.m_tree.edge_zero_length(edge)
                    && self.m_tree.edge_follow_from(edge, this) != this
                {
                    let other = self.m_tree.edge_follow_from(edge, this);
                    let mut target: Option<HeNodeIdx> = None;
                    let mut source: Option<HeNodeIdx> = None;
                    let other_junction = self.m_tree.node(other).junction;
                    let self_junction = self.m_tree.node(this).junction;
                    if other_junction.is_some() && self_junction.is_none() {
                        target = Some(other);
                        source = Some(this);
                    } else if other_junction.is_none() && self_junction.is_some() {
                        target = Some(this);
                        source = Some(other);
                    } else if other_junction.is_none() && self_junction.is_none() {
                        target = Some(this);
                        source = Some(other);
                    } else if other_junction.is_some() && self_junction.is_some() && self.m_can_make_major_changes {
                        // Only delete junctions if we can make major changes.
                        let other_junction = other_junction.unwrap();
                        let self_junction = self_junction.unwrap();

                        #[cfg(feature = "major-hyperedge-improvement-debug")]
                        {
                            use crate::avoid::debug::err_printf;
                            err_printf!(
                                "HyperedgeImprover: Coalescing junctions {} and {}:\n",
                                self_junction.id(router),
                                other_junction.id(router)
                            );
                            err_printf!("                   Deleted junction {}\n", other_junction.id(router));
                            err_printf!(
                                "                   Deleted connector {}\n",
                                router.conns[self.m_tree.edge(edge).conn.expect("tree edge without connector")].m_id
                            );
                        }

                        // Delete one of the junctions.
                        self.m_deleted_junctions.push(other_junction);
                        let other_key = junction_key(router, other_junction);
                        self.m_hyperedge_tree_junctions.remove(&other_key);
                        if self.m_hyperedge_tree_roots.contains(&other_key) {
                            // If 'other' was the root for the hyperedge, we need
                            // to make 'self' the new root.
                            self.m_hyperedge_tree_roots.remove(&other_key);
                            let self_key = junction_key(router, self_junction);
                            self.m_hyperedge_tree_roots.insert(self_key);
                            // It should already be in the junctions list.
                            debug_assert!(self.m_hyperedge_tree_junctions.contains_key(&self_key));
                        }
                        self.m_tree.node_mut(other).junction = None;

                        // Delete the connector on the zero length edge.
                        let conn = self.m_tree.edge(edge).conn.expect("zero length edge without connector");
                        self.m_deleted_connectors.push(conn);
                        self.m_tree.edge_mut(edge).conn = None;

                        target = Some(this);
                        source = Some(other);
                    }

                    if let Some(target) = target {
                        let source = source.unwrap();
                        self.m_tree.edge_disconnect_edge(edge);
                        self.m_tree.delete_edge(edge);
                        self.m_tree.node_splice_edges_from(target, source);
                        self.m_tree.delete_node(source);
                        self.remove_zero_length_edges_node(router, target, ignored);
                        return;
                    }
                }

                // Recursive call.
                self.remove_zero_length_edges_edge(router, edge, Some(this));
            }
            i += 1;
        }
    }

    // Given a set of hyperedge shift segments in a particular dimension,
    // with limits and balance values precomputed, this method shifts and
    // merges segments to improve the overall cost (length + bend penalties)
    // for the hyperedge.
    // upstream: hyperedgeimprover.cpp HyperedgeImprover::nudgeHyperedgeSegments()
    fn nudge_hyperedge_segments(&mut self, router: &Router, dimension: usize, version_number: &mut u32) {
        // For each hyperedge...
        // PTR-ORDER: iterates std::set<JunctionRef *>.
        let roots: Vec<JunctionKey> = self.m_hyperedge_tree_roots.iter().copied().collect();
        for curr in roots {
            self.m_debug_count += 1;
            *version_number = (dimension as i32 * 10000) as u32;
            *version_number = version_number.wrapping_add((self.m_debug_count * 1000) as u32);

            // Calculate the balance for each shift segment.
            let mut segment_list = self.m_root_shift_segments.remove(&curr).unwrap_or_default();
            for &seg in segment_list.iter() {
                let mut s = self.m_segments[seg].clone();
                s.set_balance_count(&self.m_tree);
                self.m_segments[seg] = s;
            }

            //segmentList.sort(CmpHyperedgeSegmentDirOrder);

            let mut change = false;
            let mut curr_seg = 0;
            while curr_seg < segment_list.len() {
                // While we haven't considered every segment...

                let segment = segment_list[curr_seg];

                if !self.m_segments[segment].settled() {
                    // The segment is not settled, so move it to the next
                    // ideal position and then merge it with overlapping
                    // segments.  Note, the merged segment will have a new
                    // balance value calculated for it.
                    let mut s = self.m_segments[segment].clone();
                    s.adjust_position(&mut self.m_tree);
                    self.m_segments[segment] = s;
                    *version_number = version_number.wrapping_add(1);
                    self.output_hyperedges_to_svg(router, *version_number, Some(segment));
                    self.merge_overlapping_segments(&mut segment_list);
                    change = true;
                }

                if change {
                    // We made a change, so start again from the beginning
                    // of the list of segments.
                    change = false;
                    curr_seg = 0;
                } else {
                    // Consider the next segment.
                    curr_seg += 1;
                }
            }
            self.m_root_shift_segments.insert(curr, segment_list);
        }
    }

    // Write the paths from an improved hyperedgetree object back as routes
    // to the component connectors that form the hyperedge.
    // upstream: hyperedgeimprover.cpp HyperedgeImprover::writeHyperedgeSegmentsBackToConnPaths()
    fn write_hyperedge_segments_back_to_conn_paths(&mut self, router: &mut Router) {
        // Write segments in two passes.  The first to clear the existing
        // connector routes and the second to build and set new routes.
        for pass in 0..2 {
            let roots: Vec<JunctionKey> = self.m_hyperedge_tree_roots.iter().copied().collect();
            for curr in roots {
                let node = self.tree_junction_node(&curr);

                self.m_tree.node_write_edges_to_conns(router, node, None, pass);
            }
        }
    }

    // Output the hyperedge tree to an SVG file, optionally highlighting
    // a segment of interest (usually the segment being moved).
    /// The SVG of `outputHyperedgesToSVG()`: the shapes and the hyperedge
    /// trees, optionally highlighting `active_segment` (usually the segment
    /// being moved).
    // upstream: hyperedgeimprover.cpp HyperedgeImprover::outputHyperedgesToSVG()
    pub fn write_hyperedges_svg(
        &self,
        router: &Router,
        fp: &mut dyn std::fmt::Write,
        pass: u32,
        active_segment: Option<HyperedgeShiftSegmentIdx>,
    ) -> std::fmt::Result {
        use crate::avoid::debug::g;
        use crate::avoid::geomtypes::PolygonInterface;
        // Reasonable initial limit for diagram bounds.
        const LIMIT: f64 = 100000000.0;

        let mut min_x = LIMIT;
        let mut min_y = LIMIT;
        let mut max_x = -LIMIT;
        let mut max_y = -LIMIT;

        let mut curr = router.vertices.conns_begin();
        while let Some(c) = curr {
            let p = router.vert_infs[c].point;

            if p.x > -LIMIT {
                min_x = cpp_min(min_x, p.x);
            }
            if p.x < LIMIT {
                max_x = cpp_max(max_x, p.x);
            }
            if p.y > -LIMIT {
                min_y = cpp_min(min_y, p.y);
            }
            if p.y < LIMIT {
                max_y = cpp_max(max_y, p.y);
            }
            curr = router.vert_infs[c].lst_next;
        }
        min_x -= 50.0;
        min_y -= 50.0;
        max_x += 50.0;
        max_y += 50.0;

        fp.write_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n")?;
        write!(
            fp,
            "<svg xmlns:inkscape=\"http://www.inkscape.org/namespaces/inkscape\" xmlns=\"http://www.w3.org/2000/svg\" width=\"100%\" height=\"100%\" viewBox=\"{} {} {} {}\">\n",
            g(min_x),
            g(min_y),
            g(max_x - min_x),
            g(max_y - min_y)
        )?;

        fp.write_str("<g inkscape:groupmode=\"layer\" inkscape:label=\"ShapesRect\">\n")?;
        for &obstacle in router.m_obstacles.iter() {
            let is_shape = router.obstacles[obstacle].is_shape();

            if !is_shape {
                // Don't output obstacles here, for now.
                continue;
            }

            let b_box = router.obstacles[obstacle].m_polygon.offset_bounding_box(0.0);

            write!(
                fp,
                "<rect id=\"rect-{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" style=\"stroke-width: 1px; stroke: {}; fill: blue; fill-opacity: 0.3;\" />\n",
                router.obstacles[obstacle].m_id,
                g(b_box.min.x),
                g(b_box.min.y),
                g(b_box.max.x - b_box.min.x),
                g(b_box.max.y - b_box.min.y),
                if is_shape { "blue" } else { "red" }
            )?;
        }
        fp.write_str("</g>\n")?;

        write!(fp, "<g inkscape:groupmode=\"layer\" inkscape:label=\"Hyperedge-{}\">\n", pass)?;
        if let Some(active) = active_segment {
            let seg = &self.m_segments[active];
            let low = seg.low_point(&self.m_tree);
            let high = seg.high_point(&self.m_tree);
            write!(
                fp,
                "<path d=\"M {} {} L {} {}\" style=\"fill: none; stroke: {}; stroke-width: 12px; stroke-opacity: 0.5;\" />\n",
                g(low.x),
                g(low.y),
                g(high.x),
                g(high.y),
                if seg.settled() { "red" } else { "orange" }
            )?;
        }

        // PTR-ORDER: iterates std::set<JunctionRef *>.
        for curr in self.m_hyperedge_tree_roots.iter() {
            let node = self.tree_junction_node(curr);

            self.m_tree.node_output_edges_except(node, fp, None)?;
        }
        fp.write_str("</g>\n")?;
        fp.write_str("</svg>\n")
    }

    /// Output the hyperedge tree to an SVG file
    /// (`DEBUG/hyperedges-NNNNN.svg`, NNNNN = `pass`), optionally
    /// highlighting a segment of interest (usually the segment being
    /// moved).  Does nothing unless the `hyperedge-debug` feature
    /// (upstream's `HYPEREDGE_DEBUG`) is on.  Upstream crashes when the
    /// file cannot be created; the port skips it.
    // upstream: hyperedgeimprover.cpp HyperedgeImprover::outputHyperedgesToSVG()
    #[inline]
    pub fn output_hyperedges_to_svg(
        &self,
        router: &Router,
        pass: u32,
        active_segment: Option<HyperedgeShiftSegmentIdx>,
    ) {
        #[cfg(not(feature = "hyperedge-debug"))]
        {
            let _ = (router, pass, active_segment);
        }
        #[cfg(feature = "hyperedge-debug")]
        {
            let filename = format!("DEBUG/hyperedges-{:05}.svg", pass);
            let _ = crate::avoid::debug::write_file_with(&filename, |fp| {
                self.write_hyperedges_svg(router, fp, pass, active_segment)
            });
        }
    }

    // Given a junction, this method follows the attached connectors and
    // junctions to determine a hyperedge and returns the set of vertices
    // representing its endpoints.
    // upstream: hyperedgeimprover.cpp HyperedgeImprover::getEndpoints()
    pub fn get_endpoints(
        &self,
        router: &Router,
        junction: JunctionRef,
        ignore: Option<JunctionRef>,
        endpoints: &mut VertexSet,
    ) {
        // PTR-ORDER: iterates std::set<ConnEnd *> m_following_conns.
        let following: Vec<_> = router.obstacles[junction.0].m_following_conns.iter().copied().collect();
        for curr in following {
            let conn_end = curr.idx.get(router);
            debug_assert!(conn_end.m_conn_ref.is_some());
            let conn_ref = conn_end.m_conn_ref.unwrap();
            let anchors = router.conns[conn_ref].endpoint_anchors();

            let junction1 = anchors.0.and_then(|o| Obstacle::as_junction(router, o));
            if let Some(junction1) = junction1 {
                if junction1 != junction && Some(junction1) != ignore {
                    self.get_endpoints(router, junction1, Some(junction), endpoints);
                }
            } else {
                let v: VertIdx = router.conns[conn_ref].m_src_vert.unwrap();
                endpoints.insert(vert_key(router, v));
            }

            let junction2 = anchors.1.and_then(|o| Obstacle::as_junction(router, o));
            if let Some(junction2) = junction2 {
                if junction2 != junction && Some(junction2) != ignore {
                    self.get_endpoints(router, junction2, Some(junction), endpoints);
                }
            } else {
                let v: VertIdx = router.conns[conn_ref].m_dst_vert.unwrap();
                endpoints.insert(vert_key(router, v));
            }
        }
    }

    /// Operates on `router.m_hyperedge_improver`.
    // upstream: hyperedgeimprover.cpp HyperedgeImprover::execute()
    pub fn execute(router: &mut Router, can_make_major_changes: bool) {
        let mut this = std::mem::take(&mut router.m_hyperedge_improver);
        this.execute_impl(router, can_make_major_changes);
        router.m_hyperedge_improver = this;
    }

    // Execute local improvement process.
    // upstream: hyperedgeimprover.cpp HyperedgeImprover::execute()
    fn execute_impl(&mut self, router: &mut Router, can_make_major_changes: bool) {
        self.m_can_make_major_changes = can_make_major_changes;

        // Build Hyperedge trees.
        let conn_refs = router.conn_refs.clone();
        for &conn_ref in conn_refs.iter() {
            let j_front = router.conns[conn_ref].m_src_connend.as_ref().and_then(|ce| ce.junction(router));
            let j_back = router.conns[conn_ref].m_dst_connend.as_ref().and_then(|ce| ce.junction(router));

            if j_front.is_none() && j_back.is_none() {
                continue;
            }

            let front_key = j_front.map(|j| junction_key(router, j));
            let back_key = j_back.map(|j| junction_key(router, j));
            let seen_front = front_key.is_some_and(|k| self.m_hyperedge_tree_junctions.contains_key(&k));
            let seen_back = back_key.is_some_and(|k| self.m_hyperedge_tree_junctions.contains_key(&k));

            let node_front;
            let node_back;

            if let Some(j_front) = j_front {
                if !seen_front {
                    let n = self.m_tree.new_node();
                    self.m_tree.node_mut(n).point = j_front.position(router);
                    self.m_tree.node_mut(n).junction = Some(j_front);

                    self.m_hyperedge_tree_junctions.insert(front_key.unwrap(), n);
                    node_front = n;
                } else {
                    node_front = self.m_hyperedge_tree_junctions[&front_key.unwrap()];
                }
            } else {
                node_front = self.m_tree.new_node();
            }

            if let Some(j_back) = j_back {
                if !seen_back {
                    let n = self.m_tree.new_node();
                    self.m_tree.node_mut(n).point = j_back.position(router);
                    self.m_tree.node_mut(n).junction = Some(j_back);

                    self.m_hyperedge_tree_junctions.insert(back_key.unwrap(), n);
                    node_back = n;
                } else {
                    node_back = self.m_hyperedge_tree_junctions[&back_key.unwrap()];
                }
            } else {
                node_back = self.m_tree.new_node();
            }

            let route = router.conns[conn_ref].display_route().ps.clone();
            let mut prev: Option<HeNodeIdx> = None;
            for i in 1..route.len() {
                let node = if i + 1 == route.len() {
                    node_back
                } else {
                    self.m_tree.new_node()
                };
                self.m_tree.node_mut(node).point = route[i];
                if i == 1 {
                    prev = Some(node_front);
                    self.m_tree.node_mut(node_front).point = route[0];
                    self.m_tree.node_mut(node_front).is_connector_source = true;
                }
                self.m_tree.new_edge(router, prev.unwrap(), node, Some(conn_ref));
                prev = Some(node);
            }
        }

        // Make a list that contains a single junction from each tree.
        // PTR-ORDER: iterates std::map<JunctionRef *, HyperedgeTreeNode *>.
        let nodes: Vec<HeNodeIdx> = self.m_hyperedge_tree_junctions.values().copied().collect();
        for node in nodes {
            let junction = self.m_tree.node(node).junction.unwrap();
            self.m_hyperedge_tree_roots.insert(junction_key(router, junction));
        }
        let mut cyclic_hyperedge_tree_roots: JunctionRefList = Vec::new();
        {
            // (removeOtherJunctionsFrom() erases later elements of the set
            // being iterated.  It can also erase the current element --
            // a connector with both ends on a junction not seen before
            // gives that junction two tree nodes -- after which upstream's
            // ++curr reads a freed rb-tree node (UB; router.rs, "Public-API
            // misuse").  The port continues with the next larger root.)
            let mut curr = self.first_root();
            while let Some(c) = curr {
                let node = self.tree_junction_node(&c);
                let contains_cycle =
                    self.m_tree.node_remove_other_junctions_from(router, node, None, &mut self.m_hyperedge_tree_roots);
                if contains_cycle {
                    // This hyperedge has a cycle.  We will need to remove it.
                    cyclic_hyperedge_tree_roots.push(self.m_tree.node(node).junction.unwrap());
                }
                curr = self.next_root(&c);
            }
        }
        // UPSTREAM NON-TERMINATION (port addition; router.rs, "Upstream
        // non-termination"): with major changes allowed, a tree edge without
        // a fixed route whose two ends are the same junction node (a
        // connector with both ends on one junction and a two-point display
        // route) makes removeZeroLengthEdges() below merge that junction
        // node into itself: spliceEdgesFrom(self) never returns (or, if the
        // self-loop was the node's only edge, the traversal continues on the
        // freed node).  The traversal reaches every edge of a (cycle-free)
        // tree, so upstream is never defined on such a hyperedge (fixed-route
        // edges are skipped upstream, hence not here either).  The port skips it like a
        // cyclic hyperedge.  (Continuing past the merge instead, as the
        // guard in removeZeroLengthEdges() does, leaves the self-loop in the
        // tree; writeEdgesToConns() then re-enters the junction node through
        // it and writes the node's other branches again, so their display
        // routes triple on every transaction: fuzz scene s1-123.)
        if self.m_can_make_major_changes {
            let roots: Vec<JunctionKey> = self.m_hyperedge_tree_roots.iter().copied().collect();
            for curr in roots {
                let node = self.tree_junction_node(&curr);
                let junction = self.m_tree.node(node).junction.unwrap();
                if !cyclic_hyperedge_tree_roots.contains(&junction) && self.m_tree.contains_unfixed_self_loop_from(node)
                {
                    // Skipped everywhere: the junction passes and the write
                    // back of recommended positions iterate all tree
                    // junctions, not just the roots, so the tree's
                    // junctions leave the map too (else the skipped tree is
                    // still split and moved, and gains junctions and
                    // connectors on every transaction: fuzz scene s21-739).
                    crate::avoid::router::e2e_guard_fired("improver-skip-self-loop-tree");
                    self.m_hyperedge_tree_roots.remove(&curr);
                    for j in self.m_tree.junctions_reachable_from(node) {
                        self.m_hyperedge_tree_junctions.remove(&junction_key(router, j));
                    }
                }
            }
        }
        // Remove roots of cyclic hyperedges, we can't improve these.
        for &junction in cyclic_hyperedge_tree_roots.iter() {
            eprint!(
                "Warning: Skipping cyclic hyperedge rooted at junction {}\n",
                junction.id(router)
            );
            self.m_hyperedge_tree_roots.remove(&junction_key(router, junction));
        }

        crate::avoid::timer::timer_start!(router, crate::avoid::timer::TimerIndex::TmHyperedgeImprove);

        // Debug output.
        let mut version_number: u32 = 1;
        self.output_hyperedges_to_svg(router, version_number, None);

        // Remove zero length edges.
        self.remove_zero_length_edges_all(router);

        // Move junctions to divergence points.
        self.move_junctions_along_common_edges(router);

        // Debug output.
        version_number += 1;
        self.output_hyperedges_to_svg(router, version_number, None);

        for count in 0..4usize {
            let dimension = count % 2;

            // Set a version number for debug output.
            version_number = 100 * (dimension as u32 + 1);

            // Build shift segments.
            self.build_hyperedge_segments(router, dimension);
            // Calculate channel information for this dimension.
            {
                let mut segs: Vec<HyperedgeShiftSegment> =
                    self.m_all_shift_segments.iter().map(|&s| self.m_segments[s].clone()).collect();
                build_orthogonal_channel_info(router, &self.m_tree, dimension, &mut segs);
                for (k, &s) in self.m_all_shift_segments.iter().enumerate() {
                    self.m_segments[s].base = segs[k].base;
                }
            }
            // Nudge hyperedge segments to locally improve the route.
            self.nudge_hyperedge_segments(router, dimension, &mut version_number);
            // Remove resulting zero length edges.
            self.remove_zero_length_edges_all(router);
            // Move junctions to divergence points.
            self.move_junctions_along_common_edges(router);
            // Debug output.
            version_number += 1;
            self.output_hyperedges_to_svg(router, version_number, None);

            // Clean up shift segments.
            let roots: Vec<JunctionKey> = self.m_hyperedge_tree_roots.iter().copied().collect();
            for curr in roots {
                let segment_list = self.m_root_shift_segments.entry(curr).or_default().clone();
                for seg in segment_list {
                    self.delete_segment(seg);
                }
            }
            self.m_root_shift_segments.clear();
            self.m_all_shift_segments.clear();
        }

        // Rewrite updated connector attachments to junctions.
        if self.m_can_make_major_changes {
            let roots: Vec<JunctionKey> = self.m_hyperedge_tree_roots.iter().copied().collect();
            for curr in roots {
                let tree_root = self.tree_junction_node(&curr);
                // (upstream passes m_router as the bool `forward`: true.)
                self.m_tree
                    .node_update_conn_ends(router, tree_root, None, true, &mut self.m_changed_connectors);

                // Validate the rewrtten connections.
                self.m_tree.node_validate_hyperedge(router, tree_root, None, 0);
            }
        }

        // Write back final recommended positions to junctions.
        let entries: Vec<HeNodeIdx> = self.m_hyperedge_tree_junctions.values().copied().collect();
        for node in entries {
            let point = self.m_tree.node(node).point;
            let junction = self.m_tree.node(node).junction.expect("tree junction node without junction");
            junction.set_recommended_position(router, &point);
        }

        // Write paths from the hyperedge tree back into individual
        // connector routes.
        self.write_hyperedge_segments_back_to_conn_paths(router);

        // Free HyperedgeTree structure.
        let roots: Vec<JunctionKey> = self.m_hyperedge_tree_roots.iter().copied().collect();
        for curr in roots {
            let node = self.tree_junction_node(&curr);

            self.m_tree.node_delete_edges_except(node, None);
            self.m_tree.delete_node(node);
        }

        // Tell the router that we are deleting the objects used for the
        // previous path for the hyperedge.
        for &curr in self.m_deleted_connectors.iter() {
            // Clear visibility assigned for connection pins.
            ConnRef::assign_connection_pin_visibility(router, curr, false);

            router.delete_connector(curr);
        }
        for &curr in self.m_deleted_junctions.iter() {
            router.delete_junction(curr);
        }

        crate::avoid::timer::timer_stop!(router);
    }

    // This method moves the junction at the given node along any shared paths
    // (so long as this action would not create any additional shared paths),
    // while also removing and freeing merged edges and nodes in the process.
    // It returns the new node where the junction is now located.
    // upstream: hyperedgeimprover.cpp HyperedgeImprover::moveJunctionAlongCommonEdge()
    fn move_junction_along_common_edge(
        &mut self,
        router: &mut Router,
        this: HeNodeIdx,
        node_map_has_changed: &mut bool,
    ) -> Option<HeNodeIdx> {
        debug_assert!(self.m_tree.node(this).junction.is_some());

        let mut new_self: Option<HeNodeIdx> = None;
        let mut common_edges: Vec<HeEdgeIdx> = Vec::new();
        let mut other_edges: Vec<HeEdgeIdx> = Vec::new();

        // Consider each edge from this node in turn.
        let mut curr = 0;
        while let Some(&curr_edge) = self.m_tree.node(this).edges.get(curr) {
            let curr_node = self.m_tree.edge_follow_from(curr_edge, this);
            common_edges.clear();
            other_edges.clear();

            if self.m_tree.node(curr_node).junction.is_some() {
                // Don't shift junctions onto other junctions.
                curr += 1;
                continue;
            }
            if self.m_tree.edge(curr_edge).has_fixed_route {
                // Don't move junctions along fixed edges.
                curr += 1;
                continue;
            }

            // The current edge is a common edge we are looking to shift along.
            common_edges.push(curr_edge);

            // Consider each of the other edges.
            let mut curr2 = 0;
            while let Some(&other_edge) = self.m_tree.node(this).edges.get(curr2) {
                if curr == curr2 {
                    // Except the current (curr) one.
                    curr2 += 1;
                    continue;
                }

                if self.m_tree.edge(other_edge).has_fixed_route {
                    // Don't shift junctions along fixed route connectors.
                    other_edges.push(other_edge);
                    curr2 += 1;
                    continue;
                }

                let other_node = self.m_tree.edge_follow_from(other_edge, this);
                let other_point = self.m_tree.node(other_node).point;
                let curr_point = self.m_tree.node(curr_node).point;
                let self_point = self.m_tree.node(this).point;
                if other_point == curr_point {
                    // A common edge can be at the same point, but can't have
                    // a junction at it.
                    if self.m_tree.node(other_node).junction.is_some() {
                        other_edges.push(other_edge);
                    } else {
                        common_edges.push(other_edge);
                    }
                } else if point_on_line(&self_point, &other_point, &curr_point, 0.0) {
                    // A common edge can be a (longer) collinear line, but we
                    // need to split the longer line at the other end of curr.
                    self.m_tree.edge_split_from_node_at_point(router, other_edge, this, &curr_point);
                    common_edges.push(other_edge);
                } else {
                    // If the edge goes in another direction it is not common.
                    other_edges.push(other_edge);
                }
                curr2 += 1;
            }

            // For the purpose of these changes a junction is considered fixed
            // only when not performing major improvements.
            let self_junction = self.m_tree.node(this).junction.unwrap();
            let self_fixed = self_junction.position_fixed(router) && !self.m_can_make_major_changes;

            // UPSTREAM NON-TERMINATION (port addition; router.rs, "Upstream
            // non-termination"): in the split case below, when two common
            // edges lead to the same node (parallel edges), the loop calls
            // targetNode->spliceEdgesFrom(targetNode), which never returns
            // while commonEdges[0] is still attached.  Upstream never
            // finishes then, so the port stops moving this junction in this
            // pass (as if it could not be moved).  (Merely declining the
            // split and trying the junction's other edges kept splitting
            // the hyperedge on every transaction: fuzz scenes s11-263,
            // s24-985, s26-508.)
            if self.m_can_make_major_changes
                && (common_edges.len() > 1)
                && (other_edges.len() > 1)
                && (1..common_edges.len()).any(|i| {
                    self.m_tree.edge_follow_from(common_edges[i], this)
                        == self.m_tree.edge_follow_from(common_edges[0], this)
                })
            {
                crate::avoid::router::e2e_guard_fired("improver-split-parallel-edges");
                return None;
            }
            if (common_edges.len() > 1) && (other_edges.len() <= 1) && !self_fixed {
                // One of the common nodes becomes the target node, we move
                // all connections from the other common nodes to this node.
                // We also move the junction there and remove it from the
                // current node.
                let target_node = self.m_tree.edge_follow_from(common_edges[0], this);
                for i in 1..common_edges.len() {
                    let this_node = self.m_tree.edge_follow_from(common_edges[i], this);
                    self.m_tree.edge_disconnect_edge(common_edges[i]);
                    self.m_tree.node_splice_edges_from(target_node, this_node);
                    self.m_tree.delete_node(this_node);
                    self.m_tree.delete_edge(common_edges[i]);
                }
                self.m_tree.node_mut(target_node).junction = Some(self_junction);
                self.m_tree.node_mut(this).junction = None;

                if other_edges.is_empty() {
                    // Nothing else connected to this node, so remove the node
                    // and the edge to the target node.
                    self.m_tree.edge_disconnect_edge(common_edges[0]);

                    self.m_tree.delete_edge(common_edges[0]);
                    self.m_tree.delete_node(this);
                } else {
                    // We need to mark commonEdges[0] as being from the connector
                    // of the otherEdges[0].
                    let c = self.m_tree.edge(other_edges[0]).conn;
                    self.m_tree.edge_mut(common_edges[0]).conn = c;
                }
                new_self = Some(target_node);

                break;
            } else if self.m_can_make_major_changes && (common_edges.len() > 1) && (other_edges.len() > 1)
            {
                // Case where this is one junction we need to split into two,
                // because we have a common path leading to it that we want to
                // move the junction along, but multiple other edges leaving
                // this junction that need to stay in place.

                // One of the common nodes becomes the target node, we move
                // all connections from the other common nodes to this node.
                // We will also create a new junction there.
                let target_node = self.m_tree.edge_follow_from(common_edges[0], this);
                for i in 1..common_edges.len() {
                    let this_node = self.m_tree.edge_follow_from(common_edges[i], this);
                    self.m_tree.edge_disconnect_edge(common_edges[i]);
                    self.m_tree.node_splice_edges_from(target_node, this_node);
                    self.m_tree.delete_node(this_node);
                    self.m_tree.delete_edge(common_edges[i]);
                }

                // Create the additional junction at the target node for
                // the otherEdges to attach to.
                let target_point = self.m_tree.node(target_node).point;
                let new_junction = JunctionRef::new(router, target_point, 0);
                self.m_tree.node_mut(target_node).junction = Some(new_junction);
                router.remove_object_from_queued_actions(ActionObj::Obstacle(new_junction.0));
                Obstacle::make_active(router, new_junction.0);
                self.m_hyperedge_tree_junctions
                    .insert(junction_key(router, new_junction), target_node);
                *node_map_has_changed = true;
                self.m_new_junctions.push(new_junction);

                // And we create a new connector between the original junction
                // and the new junction.
                let conn = ConnRef::new(router, 0);
                router.remove_object_from_queued_actions(ActionObj::Conn(conn));
                ConnRef::make_active(router, conn);
                router.conns[conn].m_initialised = true;
                let src_connend = ConnEnd::from_junction(router, new_junction);
                ConnRef::update_end_point(router, conn, VertID::SRC as u32, &src_connend);
                let tar_connend = ConnEnd::from_junction(router, self_junction);
                ConnRef::update_end_point(router, conn, VertID::TAR as u32, &tar_connend);
                self.m_tree.edge_mut(common_edges[0]).conn = Some(conn);
                self.m_new_connectors.push(conn);

                #[cfg(feature = "major-hyperedge-improvement-debug")]
                {
                    use crate::avoid::debug::err_printf;
                    err_printf!("HyperedgeImprover: Split junction {}:\n", self_junction.id(router));
                    err_printf!("                   Added junction {}\n", new_junction.id(router));
                    err_printf!(
                        "                   Added connector {} (between junctions {} and {})\n",
                        router.conns[conn].m_id,
                        self_junction.id(router),
                        new_junction.id(router)
                    );
                }

                new_self = Some(this);

                break;
            }
            curr += 1;
        }

        new_self
    }
}
