//! Faithful port of upstream `libavoid/hyperedgetree.{h,cpp}`.
//!
//! # Data model
//!
//! Upstream `HyperedgeTreeNode` / `HyperedgeTreeEdge` are heap objects with
//! cyclic pointers between them, and `OrderedHENodeSet`
//! (`std::set<HyperedgeTreeNode *, CmpNodesInDim>`) is a member of a
//! `HyperedgeShiftSegment` that nodes point back to.  Here all three live in
//! a [`HyperedgeTreeStore`] owned by the structure that builds the tree
//! (`MinimumTerminalSpanningTree` or `HyperedgeImprover`) and are named by
//! the `Copy` handles [`HeNodeIdx`], [`HeEdgeIdx`] and [`HeNodeSetIdx`].
//!
//! * `new` appends to the store; handles are never reused.  `delete` runs
//!   the upstream destructor logic and marks the object dead but keeps its
//!   last state readable ("tombstone").  That is exactly what the upstream
//!   code observes under the oracle's bump allocator (memory is never
//!   reused), and upstream does read deleted nodes in a few places (a node
//!   can be left in a second segment's node set, see
//!   `hyperedgeimprover.rs`).
//! * Each node and node set carries a creation `serial` from a counter in
//!   the store.  Nodes are only ever compared with other nodes
//!   (`CmpNodesInDim` falls back to the node pointer), so a store-local
//!   monotonic counter models "pointer order == allocation order".
//! * `std::list<HyperedgeTreeEdge *> edges` is a `Vec`.  Upstream only
//!   appends to a node's edge list while iterating it, or modifies it right
//!   before returning; iteration below therefore re-reads the `Vec` by
//!   index, which matches `std::list` iteration under appends.
//!
//! Methods are associated functions of the store taking the node/edge
//! handle as `this` (upstream `node->method(..)` becomes
//! `store.node_method(this, ..)`); methods that touch connectors or
//! junctions take the `Router`.
//!
//! (`outputEdgesExcept` / `outputNodesExcept` are SVG debug output and are
//! not ported.  `validateHyperedge` only contains assertions and
//! `MAJOR_HYPEREDGE_IMPROVEMENT_DEBUG` output; the assertions are ported as
//! `debug_assert!`.)

#![allow(dead_code)]
#![allow(clippy::collapsible_if, clippy::collapsible_else_if, clippy::new_without_default)]
// Keep upstream statement shapes.
#![allow(
    clippy::unnecessary_unwrap,
    clippy::only_used_in_recursion,
    clippy::needless_range_loop,
    clippy::if_same_then_else,
    clippy::print_with_newline
)]

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Bound;

use crate::avoid::arena::{ConnIdx, PtrKey, VertIdx};
use crate::avoid::connend::{ConnEnd, ConnEndType};
use crate::avoid::connector::ConnRef;
use crate::avoid::geomtypes::{Point, PolygonInterface};
use crate::avoid::junction::JunctionRef;
use crate::avoid::router::Router;
use crate::avoid::vertices::VertID;

/// Handle of a `HyperedgeTreeNode` (upstream `HyperedgeTreeNode *`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HeNodeIdx(pub usize);

/// Handle of a `HyperedgeTreeEdge` (upstream `HyperedgeTreeEdge *`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HeEdgeIdx(pub usize);

/// Handle of an `OrderedHENodeSet` (upstream `OrderedHENodeSet *`, the
/// `nodes` member of a `HyperedgeShiftSegment`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HeNodeSetIdx(pub usize);

/// PTR-ORDER: upstream `JunctionRef *` keys of `std::set` / `std::map`
/// are ordered by the junction obstacle's creation serial.
pub type JunctionKey = PtrKey<JunctionRef>;

/// Key of `junction` in a pointer-ordered junction container.
pub fn junction_key(router: &Router, junction: JunctionRef) -> JunctionKey {
    PtrKey::new(router.obstacles[junction.0].serial, junction)
}

// upstream: hyperedgetree.h typedefs
/// PTR-ORDER: `std::map<JunctionRef *, HyperedgeTreeNode *>`.
pub type JunctionHyperedgeTreeNodeMap = BTreeMap<JunctionKey, HeNodeIdx>;
/// PTR-ORDER: `std::set<JunctionRef *>`.
pub type JunctionSet = BTreeSet<JunctionKey>;
pub type JunctionRefList = Vec<JunctionRef>;
pub type ConnRefList = Vec<ConnIdx>;
/// Upstream `std::map<VertInf *, HyperedgeTreeNode *>`; only used for
/// lookups (never iterated), so a hash map keyed by handle is fine.
pub type VertexNodeMap = std::collections::HashMap<VertIdx, HeNodeIdx>;

// upstream: hyperedgetree.h struct HyperedgeTreeNode
#[derive(Clone, Debug)]
pub struct HyperedgeTreeNode {
    /// Allocation order (models the node's address).
    pub serial: u64,
    pub edges: Vec<HeEdgeIdx>,
    pub junction: Option<JunctionRef>,
    pub point: Point,
    pub shift_segment_node_set: Option<HeNodeSetIdx>,
    pub final_vertex: Option<VertIdx>,
    pub is_connector_source: bool,
    pub is_pin_dummy_endpoint: bool,
    pub visited: bool,
    /// Set by `delete_node` (the object stays readable, see module docs).
    pub deleted: bool,
}

// upstream: hyperedgetree.h struct HyperedgeTreeEdge
#[derive(Clone, Debug)]
pub struct HyperedgeTreeEdge {
    pub ends: (Option<HeNodeIdx>, Option<HeNodeIdx>),
    pub conn: Option<ConnIdx>,
    pub has_fixed_route: bool,
    /// Set by `delete_edge`.
    pub deleted: bool,
}

/// Sort key of a node in an `OrderedHENodeSet`: position in the set's
/// dimension, then node pointer (here: node serial).
#[derive(Clone, Copy, Debug)]
struct NodeDimKey {
    pos: f64,
    serial: u64,
    node: HeNodeIdx,
}

// upstream: hyperedgetree.cpp CmpNodesInDim::operator()()
fn cmp_nodes_in_dim(lhs: &NodeDimKey, rhs: &NodeDimKey) -> bool {
    if lhs.pos != rhs.pos {
        return lhs.pos < rhs.pos;
    }
    // PTR-ORDER: `lhs < rhs` on HyperedgeTreeNode pointers.
    lhs.serial < rhs.serial
}

impl PartialEq for NodeDimKey {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for NodeDimKey {}
impl PartialOrd for NodeDimKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for NodeDimKey {
    fn cmp(&self, other: &Self) -> Ordering {
        if cmp_nodes_in_dim(self, other) {
            Ordering::Less
        } else if cmp_nodes_in_dim(other, self) {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}

/// Upstream `std::set<HyperedgeTreeNode *, CmpNodesInDim>`.
///
/// The key of a node is computed from its position when it is inserted.
/// Upstream only moves nodes in the *other* dimension while they are in a
/// live set, so the key stays valid (see `hyperedgeimprover.rs`).
// upstream: hyperedgetree.h typedef OrderedHENodeSet
#[derive(Clone, Debug)]
pub struct OrderedHENodeSet {
    /// `CmpNodesInDim::m_dimension`.
    pub dimension: usize,
    set: BTreeSet<NodeDimKey>,
}

/// Arena of the hyperedge tree objects of one owner.
#[derive(Clone, Debug, Default)]
pub struct HyperedgeTreeStore {
    pub nodes: Vec<HyperedgeTreeNode>,
    pub edges: Vec<HyperedgeTreeEdge>,
    pub node_sets: Vec<OrderedHENodeSet>,
    next_serial: u64,
    /// Port addition (UPSTREAM NON-TERMINATION): set by
    /// `HyperedgeImprover::removeZeroLengthEdges()` when it meets a
    /// zero-length self-loop edge that upstream would merge into its own
    /// node (upstream never returns from that call).  While set,
    /// `writeEdgesToConns()` does not re-enter a node through a self-loop
    /// edge (see `edge_write_edges_to_conns`).
    pub upstream_self_merge_seen: bool,
}

// upstream: hyperedgetree.cpp travellingForwardOnConnector()
fn travelling_forward_on_connector(router: &Router, conn: ConnIdx, junction: JunctionRef) -> bool {
    let conn_ends = router.conns[conn].endpoint_conn_ends(router);

    if conn_ends.0.junction(router) == Some(junction) {
        return true;
    }
    if conn_ends.1.junction(router) == Some(junction) {
        return false;
    }
    if conn_ends.0.type_() != ConnEndType::ConnEndJunction && conn_ends.0.type_() != ConnEndType::ConnEndEmpty {
        return false;
    }
    if conn_ends.1.type_() != ConnEndType::ConnEndJunction && conn_ends.1.type_() != ConnEndType::ConnEndEmpty {
        return true;
    }
    true
}

impl HyperedgeTreeStore {
    pub fn new() -> HyperedgeTreeStore {
        HyperedgeTreeStore::default()
    }

    /// Drops every object (used by `HyperedgeImprover::clear()`).
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.edges.clear();
        self.node_sets.clear();
        self.upstream_self_merge_seen = false;
    }

    fn next_serial(&mut self) -> u64 {
        let s = self.next_serial;
        self.next_serial += 1;
        s
    }

    //------------------------------------------------------------------------
    // OrderedHENodeSet
    //------------------------------------------------------------------------

    /// `new std::set<HyperedgeTreeNode *, CmpNodesInDim>(CmpNodesInDim(dim))`.
    // upstream: hyperedgetree.cpp CmpNodesInDim::CmpNodesInDim()
    pub fn new_node_set(&mut self, dim: usize) -> HeNodeSetIdx {
        self.node_sets.push(OrderedHENodeSet {
            dimension: dim,
            set: BTreeSet::new(),
        });
        HeNodeSetIdx(self.node_sets.len() - 1)
    }

    fn dim_key(&self, set: HeNodeSetIdx, node: HeNodeIdx) -> NodeDimKey {
        let n = &self.nodes[node.0];
        NodeDimKey {
            pos: n.point[self.node_sets[set.0].dimension],
            serial: n.serial,
            node,
        }
    }

    /// `set.insert(node)`; returns whether it was inserted.
    pub fn set_insert(&mut self, set: HeNodeSetIdx, node: HeNodeIdx) -> bool {
        let key = self.dim_key(set, node);
        self.node_sets[set.0].set.insert(key)
    }

    /// `set.erase(node)`; returns the number of erased elements.
    pub fn set_erase(&mut self, set: HeNodeSetIdx, node: HeNodeIdx) -> usize {
        let key = self.dim_key(set, node);
        if self.node_sets[set.0].set.remove(&key) {
            1
        } else {
            0
        }
    }

    /// `*set.begin()`.
    pub fn set_first(&self, set: HeNodeSetIdx) -> Option<HeNodeIdx> {
        self.node_sets[set.0].set.iter().next().map(|k| k.node)
    }

    /// `*set.rbegin()`.
    pub fn set_last(&self, set: HeNodeSetIdx) -> Option<HeNodeIdx> {
        self.node_sets[set.0].set.iter().next_back().map(|k| k.node)
    }

    /// The in-order successor of `node` (which must be in the set when
    /// called, or have been when its key was taken) -- `++it`.  Using the
    /// key rather than a snapshot reproduces `std::set` iteration while
    /// elements are inserted.
    pub fn set_next(&self, set: HeNodeSetIdx, node: HeNodeIdx) -> Option<HeNodeIdx> {
        let key = self.dim_key(set, node);
        self.node_sets[set.0]
            .set
            .range((Bound::Excluded(key), Bound::Unbounded))
            .next()
            .map(|k| k.node)
    }

    /// The set's elements in order.
    pub fn set_nodes(&self, set: HeNodeSetIdx) -> Vec<HeNodeIdx> {
        self.node_sets[set.0].set.iter().map(|k| k.node).collect()
    }

    pub fn set_len(&self, set: HeNodeSetIdx) -> usize {
        self.node_sets[set.0].set.len()
    }

    pub fn set_clear(&mut self, set: HeNodeSetIdx) {
        self.node_sets[set.0].set.clear();
    }

    //------------------------------------------------------------------------
    // HyperedgeTreeNode
    //------------------------------------------------------------------------

    /// Upstream `new HyperedgeTreeNode()`.
    // upstream: hyperedgetree.cpp HyperedgeTreeNode::HyperedgeTreeNode()
    pub fn new_node(&mut self) -> HeNodeIdx {
        let serial = self.next_serial();
        self.nodes.push(HyperedgeTreeNode {
            serial,
            edges: Vec::new(),
            junction: None,
            point: Point::default(),
            shift_segment_node_set: None,
            final_vertex: None,
            is_connector_source: false,
            is_pin_dummy_endpoint: false,
            visited: false,
            deleted: false,
        });
        HeNodeIdx(self.nodes.len() - 1)
    }

    /// Upstream `delete node`.
    // upstream: hyperedgetree.cpp HyperedgeTreeNode::~HyperedgeTreeNode()
    pub fn delete_node(&mut self, this: HeNodeIdx) {
        if let Some(set) = self.nodes[this.0].shift_segment_node_set {
            self.set_erase(set, this);
            self.nodes[this.0].shift_segment_node_set = None;
        }
        debug_assert!(!self.nodes[this.0].deleted);
        self.nodes[this.0].deleted = true;
    }

    #[inline]
    pub fn node(&self, n: HeNodeIdx) -> &HyperedgeTreeNode {
        &self.nodes[n.0]
    }

    #[inline]
    pub fn node_mut(&mut self, n: HeNodeIdx) -> &mut HyperedgeTreeNode {
        &mut self.nodes[n.0]
    }

    #[inline]
    pub fn edge(&self, e: HeEdgeIdx) -> &HyperedgeTreeEdge {
        &self.edges[e.0]
    }

    #[inline]
    pub fn edge_mut(&mut self, e: HeEdgeIdx) -> &mut HyperedgeTreeEdge {
        &mut self.edges[e.0]
    }

    /// The `i`th entry of `n`'s edge list, re-read on every step (see the
    /// module docs).
    #[inline]
    fn node_edge_at(&self, n: HeNodeIdx, i: usize) -> Option<HeEdgeIdx> {
        self.nodes[n.0].edges.get(i).copied()
    }

    // This method traverses the hyperedge tree and removes from treeRoots any
    // junction nodes (other than this one).
    // upstream: hyperedgetree.cpp HyperedgeTreeNode::removeOtherJunctionsFrom()
    pub fn node_remove_other_junctions_from(
        &mut self,
        router: &Router,
        this: HeNodeIdx,
        ignored: Option<HeEdgeIdx>,
        tree_roots: &mut JunctionSet,
    ) -> bool {
        let mut contains_cycle = false;
        if self.nodes[this.0].visited {
            // We've encountered this node before, so there must be cycles in
            // the hyperedge.  Don't recurse any further.
            contains_cycle = true;
            return contains_cycle;
        }

        if let Some(junction) = self.nodes[this.0].junction {
            if ignored.is_some() {
                // Remove junctions other than the first (when ignored == nullptr).
                tree_roots.remove(&junction_key(router, junction));
            }
        }
        self.nodes[this.0].visited = true;
        let mut i = 0;
        while let Some(curr) = self.node_edge_at(this, i) {
            if Some(curr) != ignored {
                contains_cycle |= self.edge_remove_other_junctions_from(router, curr, Some(this), tree_roots);
            }
            i += 1;
        }
        contains_cycle
    }

    // This method traverses the hyperedge tree and writes each of the paths
    // back to the individual connectors as routes.
    // upstream: hyperedgetree.cpp HyperedgeTreeNode::writeEdgesToConns()
    pub fn node_write_edges_to_conns(
        &mut self,
        router: &mut Router,
        this: HeNodeIdx,
        ignored: Option<HeEdgeIdx>,
        pass: usize,
    ) {
        let mut i = 0;
        while let Some(curr) = self.node_edge_at(this, i) {
            if Some(curr) != ignored {
                self.edge_write_edges_to_conns(router, curr, Some(this), pass);
            }
            i += 1;
        }
    }

    /// Port addition (MISUSE guard, see `HyperedgeRerouter::perform_rerouting`):
    /// whether the graph reachable from `root` contains a cycle, i.e. a node
    /// reached a second time when every edge is followed once away from the
    /// node it was entered from (the traversal of `addConns()` and the
    /// other recursive methods, which never terminates on such a graph).
    /// Iterative; does not touch `visited`.
    pub fn contains_cycle_from(&self, root: HeNodeIdx) -> bool {
        let mut seen = vec![false; self.nodes.len()];
        let mut stack: Vec<(HeNodeIdx, Option<HeEdgeIdx>)> = vec![(root, None)];
        seen[root.0] = true;
        while let Some((node, from)) = stack.pop() {
            for &e in self.nodes[node.0].edges.iter() {
                if Some(e) == from {
                    continue;
                }
                let (a, b) = self.edges[e.0].ends;
                for other in [a, b].into_iter().flatten() {
                    if other == node {
                        continue;
                    }
                    if seen[other.0] {
                        return true;
                    }
                    seen[other.0] = true;
                    stack.push((other, Some(e)));
                }
            }
        }
        false
    }

    /// Port addition (UPSTREAM NON-TERMINATION guard, see
    /// `HyperedgeImprover::execute_impl`): whether the graph reachable from
    /// `root` contains an edge without a fixed route whose two ends are the
    /// same node (left by a connector with both ends on one junction and a
    /// two-point display route).  Iterative; does not touch `visited`.
    pub fn contains_unfixed_self_loop_from(&self, root: HeNodeIdx) -> bool {
        let mut seen = vec![false; self.nodes.len()];
        let mut stack: Vec<HeNodeIdx> = vec![root];
        seen[root.0] = true;
        while let Some(node) = stack.pop() {
            for &e in self.nodes[node.0].edges.iter() {
                let edge = &self.edges[e.0];
                let (a, b) = edge.ends;
                if a == b && !edge.has_fixed_route {
                    return true;
                }
                for other in [a, b].into_iter().flatten() {
                    if !seen[other.0] {
                        seen[other.0] = true;
                        stack.push(other);
                    }
                }
            }
        }
        false
    }

    // This method traverses the hyperedge tree and creates connectors for each
    // segment bridging junction and/or terminals.  It also sets the
    // appropriate ConnEnds for each connector.
    // upstream: hyperedgetree.cpp HyperedgeTreeNode::addConns()
    pub fn node_add_conns(
        &mut self,
        router: &mut Router,
        this: HeNodeIdx,
        ignored: Option<HeEdgeIdx>,
        old_conns: &ConnRefList,
        mut conn: Option<ConnIdx>,
    ) {
        // If no connector is set, then we must be starting off at a junction.
        debug_assert!(conn.is_some() || self.nodes[this.0].junction.is_some());

        let mut i = 0;
        while let Some(curr) = self.node_edge_at(this, i) {
            if Some(curr) != ignored {
                // If we're not at a junction, then use the connector value being
                // passed in to the method.

                if let Some(junction) = self.nodes[this.0].junction {
                    // If we're at a junction, then we are effectively starting
                    // our traversal along a connector, so create this new connector
                    // and set it's start ConnEnd to be this junction.
                    let c = ConnRef::new(router, 0);
                    router.remove_object_from_queued_actions(crate::avoid::actioninfo::ActionObj::Conn(c));
                    ConnRef::make_active(router, c);
                    router.conns[c].m_initialised = true;
                    let connend = ConnEnd::from_junction(router, junction);
                    ConnRef::update_end_point(router, c, VertID::SRC as u32, &connend);
                    conn = Some(c);
                }

                // Set the connector for this edge.
                self.edges[curr.0].conn = conn;

                // Continue recursive traversal.
                self.edge_add_conns(router, curr, Some(this), old_conns);
            }
            i += 1;
        }
    }

    // This method traverses the hyperedge tree and rewrites connector ends
    // that may have changed junctions due to major hyperedge improvement.
    // upstream: hyperedgetree.cpp HyperedgeTreeNode::updateConnEnds()
    pub fn node_update_conn_ends(
        &mut self,
        router: &mut Router,
        this: HeNodeIdx,
        ignored: Option<HeEdgeIdx>,
        mut forward: bool,
        changed_conns: &mut ConnRefList,
    ) {
        let mut i = 0;
        while let Some(edge) = self.node_edge_at(this, i) {
            if Some(edge) != ignored {
                if let Some(junction) = self.nodes[this.0].junction {
                    // If we're at a junction, then we are effectively starting
                    // our traversal along a connector, so create this new connector
                    // and set it's start ConnEnd to be this junction.
                    let edge_conn = self.edges[edge.0].conn.expect("hyperedge tree edge without connector");
                    forward = travelling_forward_on_connector(router, edge_conn, junction);

                    let existing_ends = router.conns[edge_conn].endpoint_conn_ends(router);
                    let existing_end = if forward { &existing_ends.0 } else { &existing_ends.1 };
                    if existing_end.junction(router) != Some(junction) {
                        let end = if forward { VertID::SRC } else { VertID::TAR } as u32;
                        let connend = ConnEnd::from_junction(router, junction);
                        ConnRef::update_end_point(router, edge_conn, end, &connend);
                        changed_conns.push(edge_conn);
                    }
                }

                // Continue recursive traversal.
                self.edge_update_conn_ends(router, edge, Some(this), forward, changed_conns);
            }
            i += 1;
        }
    }

    // This method traverses the hyperedge tree and returns a list of the junctions
    // and connectors that make up the hyperedge.
    // upstream: hyperedgetree.cpp HyperedgeTreeNode::listJunctionsAndConnectors()
    pub fn node_list_junctions_and_connectors(
        &self,
        this: HeNodeIdx,
        ignored: Option<HeEdgeIdx>,
        junctions: &mut JunctionRefList,
        connectors: &mut ConnRefList,
    ) {
        if let Some(junction) = self.nodes[this.0].junction {
            junctions.push(junction);
        }

        let mut i = 0;
        while let Some(curr) = self.node_edge_at(this, i) {
            if Some(curr) != ignored {
                self.edge_list_junctions_and_connectors(curr, Some(this), junctions, connectors);
            }
            i += 1;
        }
    }

    // upstream: hyperedgetree.cpp HyperedgeTreeNode::validateHyperedge()
    pub fn node_validate_hyperedge(&self, router: &Router, this: HeNodeIdx, ignored: Option<HeEdgeIdx>, dist: usize) {
        let new_dist = dist;
        let node = &self.nodes[this.0];
        let mut i = 0;
        while let Some(edge) = self.node_edge_at(this, i) {
            let conn = self.edges[edge.0].conn.expect("hyperedge tree edge without connector");
            let conn_ends = router.conns[conn].endpoint_conn_ends(router);

            if let Some(junction) = node.junction {
                debug_assert!(
                    (conn_ends.0.junction(router) == Some(junction)) || (conn_ends.1.junction(router) == Some(junction))
                );
                // Upstream: COLA_ASSERT(connEnds.first.junction() !=
                // connEnds.second.junction()).  Not ported (assert policy):
                // a connector with both ends on one junction (accepted by
                // the API) violates it.  Upstream never gets here on such
                // scenes: its asserting build fails earlier (or crashes) and
                // its NDEBUG build never returns or crashes (e2e fuzz
                // regression scenes reg-improver-self-loop-edge-splice,
                // reg-improver-edge-without-end-node); the port handles them
                // ("Upstream non-termination" and "Public-API misuse" in
                // router.rs), so debug builds must get past this as well.
            } else if node.edges.len() == 1 {
                debug_assert!(conn_ends.0.junction(router).is_none() || conn_ends.1.junction(router).is_none());
            }
            let _ = &conn_ends;

            if Some(edge) != ignored {
                self.edge_validate_hyperedge(router, edge, Some(this), new_dist);
            }
            i += 1;
        }
    }

    // This method traverses the hyperedge tree, clearing up the objects and
    // memory used to store the tree.
    // upstream: hyperedgetree.cpp HyperedgeTreeNode::deleteEdgesExcept()
    pub fn node_delete_edges_except(&mut self, this: HeNodeIdx, ignored: Option<HeEdgeIdx>) {
        let mut i = 0;
        while let Some(curr) = self.node_edge_at(this, i) {
            if Some(curr) != ignored {
                self.edge_delete_nodes_except(curr, Some(this));
                self.delete_edge(curr);
            }
            i += 1;
        }
        self.nodes[this.0].edges.clear();
    }

    // This method disconnects a specific hyperedge tree edge from the given node.
    // upstream: hyperedgetree.cpp HyperedgeTreeNode::disconnectEdge()
    pub fn node_disconnect_edge(&mut self, this: HeNodeIdx, edge: HeEdgeIdx) {
        self.nodes[this.0].edges.retain(|&e| e != edge);
    }

    // This method moves all edges attached to oldNode to instead be attached to
    // the given hyperedge tree node.
    // upstream: hyperedgetree.cpp HyperedgeTreeNode::spliceEdgesFrom()
    pub fn node_splice_edges_from(&mut self, this: HeNodeIdx, old_node: HeNodeIdx) {
        debug_assert!(old_node != this);
        // UPSTREAM NON-TERMINATION (port addition; router.rs, "Upstream
        // non-termination"): with oldNode == this every iteration
        // disconnects the first edge and appends it to the same list, so
        // upstream never returns unless the list is empty.  The callers
        // avoid this case; should another path reach it, it is a no-op.
        if old_node == this {
            return;
        }
        while let Some(&curr) = self.nodes[old_node.0].edges.first() {
            self.edge_replace_node(curr, old_node, this);
        }
    }

    // upstream: hyperedgetree.cpp HyperedgeTreeNode::isImmovable()
    pub fn node_is_immovable(&self, router: &Router, this: HeNodeIdx) -> bool {
        let node = &self.nodes[this.0];
        if (node.edges.len() == 1) || node.junction.is_some_and(|j| j.position_fixed(router)) {
            return true;
        }
        for &curr in node.edges.iter() {
            if self.edges[curr.0].has_fixed_route {
                return true;
            }
        }
        false
    }

    //------------------------------------------------------------------------
    // HyperedgeTreeEdge
    //------------------------------------------------------------------------

    /// Upstream `new HyperedgeTreeEdge(node1, node2, conn)`.
    // upstream: hyperedgetree.cpp HyperedgeTreeEdge::HyperedgeTreeEdge()
    pub fn new_edge(
        &mut self,
        router: &Router,
        node1: HeNodeIdx,
        node2: HeNodeIdx,
        conn: Option<ConnIdx>,
    ) -> HeEdgeIdx {
        let mut has_fixed_route = false;
        if let Some(c) = conn {
            has_fixed_route = router.conns[c].has_fixed_route();
        }
        self.edges.push(HyperedgeTreeEdge {
            ends: (Some(node1), Some(node2)),
            conn,
            has_fixed_route,
            deleted: false,
        });
        let e = HeEdgeIdx(self.edges.len() - 1);
        self.nodes[node1.0].edges.push(e);
        self.nodes[node2.0].edges.push(e);
        e
    }

    /// Upstream `delete edge` (trivial destructor).
    pub fn delete_edge(&mut self, this: HeEdgeIdx) {
        // MISUSE: deleting an edge twice is a no-op.  An edge whose both
        // ends are one node (left by a connector with both ends on one
        // junction) is in that node's edge list twice, so
        // deleteEdgesExcept() deletes it twice upstream (a double free; ASan:
        // heap-use-after-free in HyperedgeTreeEdge::deleteNodesExcept);
        // reg-improver-self-loop-edge-splice, reg-improver-edge-without-end-node.
        if self.edges[this.0].deleted {
            return;
        }
        self.edges[this.0].deleted = true;
    }

    // Given one endpoint of the hyperedge tree edge, returns the other endpoint.
    // upstream: hyperedgetree.cpp HyperedgeTreeEdge::followFrom()
    pub fn edge_follow_from(&self, this: HeEdgeIdx, from: HeNodeIdx) -> HeNodeIdx {
        let ends = self.edges[this.0].ends;
        let r = if ends.0 == Some(from) { ends.1 } else { ends.0 };
        r.expect("HyperedgeTreeEdge::followFrom(): null end")
    }

    // Returns true if the length of this edge is zero, i.e., the endpoints are
    // located at the same position.
    // upstream: hyperedgetree.cpp HyperedgeTreeEdge::zeroLength()
    pub fn edge_zero_length(&self, this: HeEdgeIdx) -> bool {
        let ends = self.edges[this.0].ends;
        self.nodes[ends.0.unwrap().0].point == self.nodes[ends.1.unwrap().0].point
    }

    // This method returns true if the edge is in the dimension given, i.e.,
    // either horizontal or vertical.
    // upstream: hyperedgetree.cpp HyperedgeTreeEdge::hasOrientation()
    pub fn edge_has_orientation(&self, this: HeEdgeIdx, dimension: usize) -> bool {
        let ends = self.edges[this.0].ends;
        self.nodes[ends.0.unwrap().0].point[dimension] == self.nodes[ends.1.unwrap().0].point[dimension]
    }

    // This method updates any of the given hyperedge tree edge's endpoints that
    // are attached to oldNode to instead be attached to newNode.
    // upstream: hyperedgetree.cpp HyperedgeTreeEdge::replaceNode()
    pub fn edge_replace_node(&mut self, this: HeEdgeIdx, old_node: HeNodeIdx, new_node: HeNodeIdx) {
        if self.edges[this.0].ends.0 == Some(old_node) {
            self.node_disconnect_edge(old_node, this);
            self.nodes[new_node.0].edges.push(this);
            self.edges[this.0].ends.0 = Some(new_node);
        } else if self.edges[this.0].ends.1 == Some(old_node) {
            self.node_disconnect_edge(old_node, this);
            self.nodes[new_node.0].edges.push(this);
            self.edges[this.0].ends.1 = Some(new_node);
        }
    }

    // This method traverses the hyperedge tree and writes each of the paths
    // back to the individual connectors as routes.
    // upstream: hyperedgetree.cpp HyperedgeTreeEdge::writeEdgesToConns()
    pub fn edge_write_edges_to_conns(
        &mut self,
        router: &mut Router,
        this: HeEdgeIdx,
        ignored: Option<HeNodeIdx>,
        pass: usize,
    ) {
        debug_assert!(ignored.is_some());
        let ends = self.edges[this.0].ends;
        debug_assert!(ends.0.is_some());
        debug_assert!(ends.1.is_some());

        let prev_node = if ignored == ends.0 { ends.0 } else { ends.1 }.unwrap();
        let next_node = if ignored == ends.0 { ends.1 } else { ends.0 }.unwrap();

        let conn = self.edges[this.0].conn.expect("hyperedge tree edge without connector");
        if pass == 0 {
            router.conns[conn].m_display_route.clear();
        } else if pass == 1 {
            let prev_point = self.nodes[prev_node.0].point;
            let next_point = self.nodes[next_node.0].point;
            if router.conns[conn].m_display_route.empty() {
                router.conns[conn].m_display_route.ps.push(prev_point);
            }
            router.conns[conn].m_display_route.ps.push(next_point);

            let next_node_edges = self.nodes[next_node.0].edges.len();
            if next_node_edges != 2 {
                // We have finished writing a connector.  If the node has just
                // two edges then it is an intermediate node on a connector.
                let mut should_reverse = false;
                if next_node_edges == 1 {
                    // This connector led to a terminal.
                    if self.nodes[next_node.0].is_connector_source {
                        should_reverse = true;
                    }

                    if self.nodes[next_node.0].is_pin_dummy_endpoint {
                        // If may be that the hyperedge has an extra segment or
                        // two leading to the centre dummy pin used for connection
                        // pin routing.  If so, remove these points from the
                        // resulting route.
                        router.conns[conn].m_display_route.ps.pop();
                        if prev_point == next_point {
                            // Duplicated dummy point.  Remove second one.
                            router.conns[conn].m_display_route.ps.pop();
                        }
                    }
                } else {
                    // if (nextNodeEdges > 2)
                    // This connector was between two junctions.
                    debug_assert!(router.conns[conn].m_dst_connend.is_some());
                    let correct_end_junction =
                        router.conns[conn].m_dst_connend.as_ref().and_then(|ce| ce.junction(router));
                    if self.nodes[next_node.0].junction != correct_end_junction {
                        should_reverse = true;
                    }
                }

                if should_reverse {
                    // Reverse the written connector route.
                    router.conns[conn].m_display_route.ps.reverse();
                }
            }
        }

        // UPSTREAM NON-TERMINATION (port addition; router.rs, "Upstream
        // non-termination"): after upstream would have looped forever
        // merging a self-loop edge's node into itself, the port does not
        // re-enter that node through the self-loop edge: upstream's
        // traversal would write the node's other connectors once more for
        // each of the edge's two entries in the node's edge list, tripling
        // their routes on every transaction.
        if self.upstream_self_merge_seen && next_node == prev_node {
            return;
        }
        self.node_write_edges_to_conns(router, next_node, Some(this), pass);
    }

    // This method traverses the hyperedge tree and creates connectors for each
    // segment bridging junction and/or terminals.  It also sets the
    // appropriate ConnEnds for each connector.
    // upstream: hyperedgetree.cpp HyperedgeTreeEdge::addConns()
    pub fn edge_add_conns(
        &mut self,
        router: &mut Router,
        this: HeEdgeIdx,
        ignored: Option<HeNodeIdx>,
        old_conns: &ConnRefList,
    ) {
        debug_assert!(self.edges[this.0].conn.is_some());
        let mut end_node: Option<HeNodeIdx> = None;
        let first = self.edges[this.0].ends.0;
        if first.is_some() && (first != ignored) {
            end_node = first;
            let conn = self.edges[this.0].conn;
            self.node_add_conns(router, first.unwrap(), Some(this), old_conns, conn);
        }

        let second = self.edges[this.0].ends.1;
        if second.is_some() && (second != ignored) {
            end_node = second;
            let conn = self.edges[this.0].conn;
            self.node_add_conns(router, second.unwrap(), Some(this), old_conns, conn);
        }

        let end_node = end_node.expect("HyperedgeTreeEdge::addConns(): no end node");
        let conn = self.edges[this.0].conn.unwrap();
        if let Some(final_vertex) = self.nodes[end_node.0].final_vertex {
            // We have reached a terminal of the hyperedge, so set a ConnEnd for
            // the original connector endpoint
            let mut connend = ConnEnd::new();
            let mut result = false;
            // Find the ConnEnd from the list of original connectors.
            for &curr in old_conns.iter() {
                result |= router.conns[curr].get_conn_end_for_endpoint_vertex(router, Some(final_vertex), &mut connend);
                if result {
                    break;
                }
            }
            if result {
                // XXX: Create new conn here.
                ConnRef::update_end_point(router, conn, VertID::TAR as u32, &connend);
            }
        } else if let Some(junction) = self.nodes[end_node.0].junction {
            // Or, set a ConnEnd connecting to the junction we have reached.
            let connend = ConnEnd::from_junction(router, junction);
            ConnRef::update_end_point(router, conn, VertID::TAR as u32, &connend);
        }
    }

    // This method traverses the hyperedge tree and rewrites connector ends
    // that may have changed junctions due to major hyperedge improvement.
    // upstream: hyperedgetree.cpp HyperedgeTreeEdge::updateConnEnds()
    pub fn edge_update_conn_ends(
        &mut self,
        router: &mut Router,
        this: HeEdgeIdx,
        ignored: Option<HeNodeIdx>,
        forward: bool,
        changed_conns: &mut ConnRefList,
    ) {
        let mut end_node: Option<HeNodeIdx> = None;
        let first = self.edges[this.0].ends.0;
        if first.is_some() && (first != ignored) {
            end_node = first;
            self.node_update_conn_ends(router, first.unwrap(), Some(this), forward, changed_conns);
        }

        let second = self.edges[this.0].ends.1;
        if second.is_some() && (second != ignored) {
            end_node = second;
            self.node_update_conn_ends(router, second.unwrap(), Some(this), forward, changed_conns);
        }

        // MISUSE / upstream bug (port addition): an edge whose both ends
        // are the node it was entered from (seen with a connector that has
        // both ends on one junction) has no end node; upstream dereferences null (UBSan:
        // hyperedgetree.cpp:638).  Nothing to update for it.
        let Some(end_node) = end_node else {
            return;
        };
        if let Some(end_junction) = self.nodes[end_node.0].junction {
            // We've reached a junction at the end of this connector, and it's
            // not an endpoint of the hyperedge.  So  the connector ConnEnd to
            // connect to the junction we have reached.
            let conn = self.edges[this.0].conn.expect("hyperedge tree edge without connector");
            let existing_ends = router.conns[conn].endpoint_conn_ends(router);
            let existing_end = if forward { &existing_ends.1 } else { &existing_ends.0 };
            if existing_end.junction(router) != Some(end_junction) {
                let connend = ConnEnd::from_junction(router, end_junction);
                let end = if forward { VertID::TAR } else { VertID::SRC } as u32;
                ConnRef::update_end_point(router, conn, end, &connend);

                // Record that this connector was changed (so long as it wasn't
                // already recorded).
                if changed_conns.is_empty() || (*changed_conns.last().unwrap() != conn) {
                    changed_conns.push(conn);
                }
            }
        }
    }

    // This method traverses the hyperedge tree and returns a list of the junctions
    // and connectors that make up the hyperedge.
    // upstream: hyperedgetree.cpp HyperedgeTreeEdge::listJunctionsAndConnectors()
    pub fn edge_list_junctions_and_connectors(
        &self,
        this: HeEdgeIdx,
        ignored: Option<HeNodeIdx>,
        junctions: &mut JunctionRefList,
        connectors: &mut ConnRefList,
    ) {
        let edge = &self.edges[this.0];
        let conn = edge.conn.expect("hyperedge tree edge without connector");
        if !connectors.contains(&conn) {
            // Add connector if it isn't already in the list.
            connectors.push(conn);
        }

        if edge.ends.0 != ignored {
            self.node_list_junctions_and_connectors(edge.ends.0.unwrap(), Some(this), junctions, connectors);
        } else if edge.ends.1 != ignored {
            self.node_list_junctions_and_connectors(edge.ends.1.unwrap(), Some(this), junctions, connectors);
        }
    }

    // upstream: hyperedgetree.cpp HyperedgeTreeEdge::validateHyperedge()
    pub fn edge_validate_hyperedge(&self, router: &Router, this: HeEdgeIdx, ignored: Option<HeNodeIdx>, dist: usize) {
        let edge = &self.edges[this.0];
        if edge.ends.0 != ignored {
            self.node_validate_hyperedge(router, edge.ends.0.unwrap(), Some(this), dist);
        } else if edge.ends.1 != ignored {
            self.node_validate_hyperedge(router, edge.ends.1.unwrap(), Some(this), dist);
        }
    }

    // This method splits the current edge, adding a node at the given point.
    // The current edge will connect the source node and the newly created node.
    // A new edge will connect the new node and the node at the other end of the
    // original edge.
    // upstream: hyperedgetree.cpp HyperedgeTreeEdge::splitFromNodeAtPoint()
    pub fn edge_split_from_node_at_point(
        &mut self,
        router: &Router,
        this: HeEdgeIdx,
        source: HeNodeIdx,
        point: &Point,
    ) {
        // Make the source the first of the two nodes.
        if self.edges[this.0].ends.1 == Some(source) {
            let e = &mut self.edges[this.0];
            std::mem::swap(&mut e.ends.1, &mut e.ends.0);
        }
        debug_assert!(self.edges[this.0].ends.0 == Some(source));

        // Remember the other end.
        let target = self.edges[this.0].ends.1.unwrap();

        // Create a new node for the split point at the given position.
        let split = self.new_node();
        self.nodes[split.0].point = *point;

        // Create a new edge between the split point and the other end.
        let conn = self.edges[this.0].conn;
        self.new_edge(router, split, target, conn);

        // Disconnect the current edge from the other end and connect it to
        // the new split point node.
        self.node_disconnect_edge(target, this);
        self.edges[this.0].ends.1 = Some(split);
        self.nodes[split.0].edges.push(this);
    }

    // This method disconnects the hyperedge tree edge nodes that it's attached to.
    // upstream: hyperedgetree.cpp HyperedgeTreeEdge::disconnectEdge()
    pub fn edge_disconnect_edge(&mut self, this: HeEdgeIdx) {
        let ends = self.edges[this.0].ends;
        debug_assert!(ends.0.is_some());
        debug_assert!(ends.1.is_some());

        self.node_disconnect_edge(ends.0.unwrap(), this);
        self.node_disconnect_edge(ends.1.unwrap(), this);
        self.edges[this.0].ends = (None, None);
    }

    // This method traverses the hyperedge tree and removes from treeRoots any
    // junction nodes.
    // upstream: hyperedgetree.cpp HyperedgeTreeEdge::removeOtherJunctionsFrom()
    pub fn edge_remove_other_junctions_from(
        &mut self,
        router: &Router,
        this: HeEdgeIdx,
        ignored: Option<HeNodeIdx>,
        tree_roots: &mut JunctionSet,
    ) -> bool {
        let mut contains_cycle = false;
        let first = self.edges[this.0].ends.0;
        if first.is_some() && (first != ignored) {
            contains_cycle |= self.node_remove_other_junctions_from(router, first.unwrap(), Some(this), tree_roots);
        }

        let second = self.edges[this.0].ends.1;
        if second.is_some() && (second != ignored) {
            contains_cycle |= self.node_remove_other_junctions_from(router, second.unwrap(), Some(this), tree_roots);
        }
        contains_cycle
    }

    // This method traverses the hyperedge tree, clearing up the objects and
    // memory used to store the tree.
    // upstream: hyperedgetree.cpp HyperedgeTreeEdge::deleteNodesExcept()
    pub fn edge_delete_nodes_except(&mut self, this: HeEdgeIdx, ignored: Option<HeNodeIdx>) {
        let first = self.edges[this.0].ends.0;
        if first.is_some() && (first != ignored) {
            self.node_delete_edges_except(first.unwrap(), Some(this));
            self.delete_node(first.unwrap());
        }
        self.edges[this.0].ends.0 = None;

        let second = self.edges[this.0].ends.1;
        if second.is_some() && (second != ignored) {
            self.node_delete_edges_except(second.unwrap(), Some(this));
            self.delete_node(second.unwrap());
        }
        self.edges[this.0].ends.1 = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_set_orders_by_dim_then_serial_and_iterates_under_insert() {
        let mut s = HyperedgeTreeStore::new();
        let a = s.new_node();
        let b = s.new_node();
        let c = s.new_node();
        s.node_mut(a).point = Point::new(0.0, 5.0);
        s.node_mut(b).point = Point::new(9.0, 1.0);
        s.node_mut(c).point = Point::new(3.0, 5.0);
        let set = s.new_node_set(1);
        assert!(s.set_insert(set, c));
        assert!(s.set_insert(set, b));
        assert!(!s.set_insert(set, b));
        assert_eq!(s.set_nodes(set), vec![b, c]);
        // Insert `a` (same y as c, smaller serial) while "at" b: it comes
        // before c, so the successor of b is now a.
        assert!(s.set_insert(set, a));
        assert_eq!(s.set_next(set, b), Some(a));
        assert_eq!(s.set_next(set, a), Some(c));
        assert_eq!(s.set_first(set), Some(b));
        assert_eq!(s.set_last(set), Some(c));
        s.node_mut(a).shift_segment_node_set = Some(set);
        s.delete_node(a);
        assert_eq!(s.set_nodes(set), vec![b, c]);
        // -0.0 and 0.0 compare equal positionally (pointer tie-break).
        let d = s.new_node();
        s.node_mut(d).point = Point::new(0.0, -0.0);
        let e = s.new_node();
        s.node_mut(e).point = Point::new(0.0, 0.0);
        let set2 = s.new_node_set(1);
        s.set_insert(set2, e);
        s.set_insert(set2, d);
        assert_eq!(s.set_nodes(set2), vec![d, e]);
    }

    #[test]
    fn split_and_splice() {
        let router = Router::new(crate::avoid::router::ORTHOGONAL_ROUTING);
        let mut s = HyperedgeTreeStore::new();
        let a = s.new_node();
        let b = s.new_node();
        s.node_mut(b).point = Point::new(10.0, 0.0);
        let e = s.new_edge(&router, a, b, None);
        s.edge_split_from_node_at_point(&router, e, b, &Point::new(5.0, 0.0));
        // Source b becomes ends.first; e now joins b and the split node.
        let split = s.edge(e).ends.1.unwrap();
        assert_eq!(s.edge(e).ends.0, Some(b));
        assert_eq!(s.node(split).point, Point::new(5.0, 0.0));
        assert_eq!(s.node(a).edges.len(), 1);
        assert_eq!(s.edge_follow_from(s.node(a).edges[0], a), split);
        // Splice a's edges into b.
        s.node_splice_edges_from(b, split);
        assert!(s.node(split).edges.is_empty());
        assert_eq!(s.node(b).edges.len(), 3);
    }
}
