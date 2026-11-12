//! Faithful port of upstream `libavoid/mtst.{h,cpp}`.
//!
//! The minimum terminal spanning tree (MTST) builds the initial route and
//! junction positions of a hyperedge.  The temporary hyperedge tree it
//! builds lives in the struct's own [`HyperedgeTreeStore`].
//!
//! # Heaps
//! The vertex heap and the bridging edge heap are `Vec`s manipulated with
//! the exact libstdc++ heap algorithm ports from `makepath.rs`
//! (`make_heap`/`push_heap`/`pop_heap`), because ties are broken by heap
//! layout.
//!
//! # Pointer ordering (`// PTR-ORDER:`)
//! * `terminals` / `origTerminals` are `std::set<VertInf *>`: iterated in
//!   vertex creation-serial order (vertices, including the dummy vertices
//!   created here, take their serial from `Router::next_serial`).
//! * `commitToBridgingEdge` picks the new and old tree roots with
//!   `std::min`/`std::max` of two `VertInf *`: compared by serial.
//!
//! # Deleted dummy vertices
//! Both constructions leave `pathNext` (and `m_orthogonalPartner`) of router
//! vertices pointing at the dummy vertices they create and then delete.
//! Upstream later dereferences such dangling pointers: the next A* search
//! of a connector whose source vertex was explored by the MTST reads
//! `start->pathNext` (makepath.cpp, "If we are doing checkpoint routing
//! ...") and creates a search node on the deleted vertex.  That is
//! undefined behaviour upstream.  The dummy vertices are freed immediately
//! here, as upstream; the generation-checked handle left in `pathNext`
//! reads as null at those sites (`VertInf::live_path_next`, see "Stale
//! `pathNext`" in the `router.rs` module docs).  `m_orthogonalPartner`
//! links to them are only read by later MTST constructions, after their
//! initialisation loop has reset them.
//!
//! (`drawForest`, `setDebuggingOutput` and the `DEBUGHANDLER` hooks are
//! debug output and are not ported; timers are not ported.)

#![allow(dead_code)]
#![allow(clippy::collapsible_if, clippy::collapsible_else_if, clippy::needless_range_loop)]
// Keep upstream statement shapes.
#![allow(
    clippy::unnecessary_unwrap,
    clippy::only_used_in_recursion,
    clippy::needless_range_loop,
    clippy::if_same_then_else,
    clippy::print_with_newline
)]

use std::collections::BTreeSet;

use crate::avoid::arena::{EdgeIdx, PtrKey, TreeRootPtr, VertIdx};
use crate::avoid::geometry::colinear;
use crate::avoid::graph::EdgeInf;
use crate::avoid::hyperedgetree::{HeNodeIdx, HyperedgeTreeStore, VertexNodeMap};
use crate::avoid::junction::JunctionRef;
use crate::avoid::makepath::{make_heap, pop_heap, push_heap};
use crate::avoid::obstacle::Obstacle;
use crate::avoid::router::Router;
use crate::avoid::vertices::{VertID, VertInf};

const DBL_MAX: f64 = f64::MAX;

/// PTR-ORDER: upstream `std::set<VertInf *>` (`VertexSet`), ordered by the
/// vertex creation serial.
pub type VertexSet = BTreeSet<PtrKey<VertIdx>>;
// upstream: mtst.h typedef std::list<VertexSet> VertexSetList
pub type VertexSetList = Vec<VertexSet>;
// upstream: mtst.h typedef std::pair<EdgeInf *, VertInf *> LayeredOrthogonalEdge
pub type LayeredOrthogonalEdge = (EdgeIdx, VertIdx);
// upstream: mtst.h typedef std::list<LayeredOrthogonalEdge> LayeredOrthogonalEdgeList
pub type LayeredOrthogonalEdgeList = Vec<LayeredOrthogonalEdge>;
// upstream: vertices.h typedef std::pair<VertInf *, VertInf *> VertexPair
pub type VertexPair = (VertIdx, VertIdx);

/// PTR-ORDER key of a vertex.
#[inline]
pub fn vert_key(router: &Router, v: VertIdx) -> PtrKey<VertIdx> {
    PtrKey::new(router.vert_infs[v].serial, v)
}

// Comparison for the vertex heap in the extended Dijkstra's algorithm.
// upstream: mtst.cpp HeapCmpVertInf::operator()()
pub fn heap_cmp_vert_inf(router: &Router, a: VertIdx, b: VertIdx) -> bool {
    router.vert_infs[a].sptf_dist > router.vert_infs[b].sptf_dist
}

// Comparison for the bridging edge heap in the extended Kruskal's algorithm.
// upstream: mtst.cpp CmpEdgeInf::operator()()
pub fn cmp_edge_inf(router: &Router, a: EdgeIdx, b: EdgeIdx) -> bool {
    router.edge_infs[a].mtst_dist() > router.edge_infs[b].mtst_dist()
}

/// Upstream `delete_vertex`: removes the dummy vertex from the graph and
/// frees it (see the module docs, "Deleted dummy vertices").
// upstream: mtst.cpp struct delete_vertex
fn delete_vertex(router: &mut Router, ptr: VertIdx) {
    VertInf::remove_from_graph(router, ptr, false);
    VertInf::delete(router, ptr);
}

fn v_make_heap(router: &Router, heap: &mut [VertIdx]) {
    make_heap(heap, &mut |a: &VertIdx, b: &VertIdx| heap_cmp_vert_inf(router, *a, *b));
}
fn v_push_heap(router: &Router, heap: &mut [VertIdx]) {
    push_heap(heap, &mut |a: &VertIdx, b: &VertIdx| heap_cmp_vert_inf(router, *a, *b));
}
fn v_pop_heap(router: &Router, heap: &mut [VertIdx]) {
    pop_heap(heap, &mut |a: &VertIdx, b: &VertIdx| heap_cmp_vert_inf(router, *a, *b));
}
fn e_make_heap(router: &Router, heap: &mut [EdgeIdx]) {
    make_heap(heap, &mut |a: &EdgeIdx, b: &EdgeIdx| cmp_edge_inf(router, *a, *b));
}
fn e_push_heap(router: &Router, heap: &mut [EdgeIdx]) {
    push_heap(heap, &mut |a: &EdgeIdx, b: &EdgeIdx| cmp_edge_inf(router, *a, *b));
}
fn e_pop_heap(router: &Router, heap: &mut [EdgeIdx]) {
    pop_heap(heap, &mut |a: &EdgeIdx, b: &EdgeIdx| cmp_edge_inf(router, *a, *b));
}

// This class is not intended for public use.
// It is used by the hyperedge routing code to build a minimum terminal
// spanning tree for a set of terminal vertices.
// upstream: mtst.h class MinimumTerminalSpanningTree
#[derive(Debug)]
pub struct MinimumTerminalSpanningTree {
    pub is_orthogonal: bool,
    pub terminals: VertexSet,
    pub orig_terminals: VertexSet,
    /// Upstream `JunctionHyperedgeTreeNodeMap *hyperedgeTreeJunctions`; the
    /// map itself is never accessed, only tested for null.
    pub hyperedge_tree_junctions: bool,

    pub nodes: VertexNodeMap,
    pub m_root_junction: Option<HeNodeIdx>,
    pub bend_penalty: f64,
    pub allsets: VertexSetList,
    pub visited_vertices: Vec<VertIdx>,
    pub extra_vertices: Vec<VertIdx>,
    pub unused_vertices: Vec<VertIdx>,
    pub root_vertex_pointers: Vec<TreeRootPtr>,

    // Vertex heap for extended Dijkstra's algorithm.
    pub v_heap: Vec<VertIdx>,

    // Bridging edge heap for the extended Kruskal's algorithm.
    pub be_heap: Vec<EdgeIdx>,

    pub dimension_change_vertex_id: VertID,

    /// Storage of the hyperedge tree objects (upstream heap objects).
    pub tree: HyperedgeTreeStore,
}

impl MinimumTerminalSpanningTree {
    // upstream: mtst.cpp MinimumTerminalSpanningTree::MinimumTerminalSpanningTree()
    pub fn new(terminals: VertexSet, hyperedge_tree_junctions: bool) -> MinimumTerminalSpanningTree {
        MinimumTerminalSpanningTree {
            is_orthogonal: true,
            terminals,
            orig_terminals: VertexSet::new(),
            hyperedge_tree_junctions,
            nodes: VertexNodeMap::new(),
            m_root_junction: None,
            bend_penalty: 2000.0,
            allsets: VertexSetList::new(),
            visited_vertices: Vec::new(),
            extra_vertices: Vec::new(),
            unused_vertices: Vec::new(),
            root_vertex_pointers: Vec::new(),
            v_heap: Vec::new(),
            be_heap: Vec::new(),
            dimension_change_vertex_id: VertID::new(0, 42, 0),
            tree: HyperedgeTreeStore::new(),
        }
    }

    /// Upstream destructor: frees the temporary hyperedge tree.  (The
    /// objects live in `self.tree`, so only the deletion order matters, and
    /// nothing observes it; kept for fidelity.)
    // upstream: mtst.cpp MinimumTerminalSpanningTree::~MinimumTerminalSpanningTree()
    pub fn destruct(&mut self) {
        // Free the temporary hyperedge tree representation.
        // (Upstream dereferences a null root junction here; see
        // HyperedgeRerouter::perform_rerouting.  The tree nodes live in
        // `self.tree` and are dropped with it.)
        let Some(root) = self.m_root_junction else {
            return;
        };
        self.tree.node_delete_edges_except(root, None);
        self.tree.delete_node(root);
        self.m_root_junction = None;
    }

    // upstream: mtst.cpp MinimumTerminalSpanningTree::rootJunction()
    pub fn root_junction(&self) -> Option<HeNodeIdx> {
        self.m_root_junction
    }

    // upstream: mtst.cpp MinimumTerminalSpanningTree::makeSet()
    fn make_set(&mut self, router: &Router, vertex: VertIdx) {
        let mut new_set = VertexSet::new();
        new_set.insert(vert_key(router, vertex));
        self.allsets.push(new_set);
    }

    /// Returns the index into `allsets` (`allsets.end()` is `None`).
    // upstream: mtst.cpp MinimumTerminalSpanningTree::findSet()
    fn find_set(&self, router: &Router, vertex: Option<VertIdx>) -> Option<usize> {
        let vertex = vertex?;
        let key = vert_key(router, vertex);
        self.allsets.iter().position(|s| s.contains(&key))
    }

    // upstream: mtst.cpp MinimumTerminalSpanningTree::unionSets()
    fn union_sets(&mut self, s1: usize, s2: usize) {
        let mut s = self.allsets[s1].clone();
        s.extend(self.allsets[s2].iter().copied());

        // allsets.erase(s1); allsets.erase(s2);
        let (hi, lo) = if s1 > s2 { (s1, s2) } else { (s2, s1) };
        self.allsets.remove(hi);
        self.allsets.remove(lo);
        self.allsets.push(s);
    }

    // upstream: mtst.cpp MinimumTerminalSpanningTree::addNode()
    fn add_node(&mut self, router: &mut Router, vertex: VertIdx, prev_node: Option<HeNodeIdx>) -> HeNodeIdx {
        let node;

        // Do we already have a node for this vertex?
        match self.nodes.get(&vertex).copied() {
            None => {
                // Not found.  Create new node.
                let new_node = self.tree.new_node();
                self.tree.node_mut(new_node).point = router.vert_infs[vertex].point;
                // Remember it.
                self.nodes.insert(vertex, new_node);

                node = new_node;
            }
            Some(junction_node) => {
                // Found.
                if self.tree.node(junction_node).junction.is_none() {
                    // Create a junction, if one has not already been created.
                    let point = router.vert_infs[vertex].point;
                    let junction = JunctionRef::new(router, point, 0);
                    self.tree.node_mut(junction_node).junction = Some(junction);
                    if self.m_root_junction.is_none() {
                        // Remember the first junction node, so we can use it to
                        // traverse the tree, added and connecting connectors to
                        // junctions and endpoints.
                        self.m_root_junction = Some(junction_node);
                    }
                    router.remove_object_from_queued_actions(crate::avoid::actioninfo::ActionObj::Obstacle(
                        junction.0,
                    ));
                    Obstacle::make_active(router, junction.0);
                }
                node = junction_node;
            }
        }

        if let Some(prev_node) = prev_node {
            // Join this node to the previous node.
            self.tree.new_edge(router, prev_node, node, None);
        }

        node
    }

    // upstream: mtst.cpp MinimumTerminalSpanningTree::buildHyperedgeTreeToRoot()
    fn build_hyperedge_tree_to_root(
        &mut self,
        router: &mut Router,
        curr_vert: Option<VertIdx>,
        prev_node: Option<HeNodeIdx>,
        prev_vert: VertIdx,
        mark_edges: bool,
    ) {
        // (upstream dereferences prevNode unconditionally; it is null only
        // when hyperedgeTreeJunctions is null, which never happens with
        // these call sites in practice.)
        let mut prev_node = prev_node.expect("buildHyperedgeTreeToRoot(): null prevNode");
        let mut prev_vert = prev_vert;
        if self.tree.node(prev_node).junction.is_some() {
            // We've reached a junction, so stop.
            return;
        }

        // (upstream: COLA_ASSERT(currVert != nullptr).  Not ported: it fails
        // whenever a bridging edge ends at an original terminal, whose
        // pathNext is null; the loop below then does nothing, which is the
        // upstream release-build behaviour.  The oracle build removes it too.)

        // This method follows branches in a shortest path tree back to the
        // root, generating hyperedge tree nodes and branches as it goes.
        let mut curr_vert = curr_vert;
        while let Some(cv) = curr_vert {
            // Add the node, if necessary.
            let current_node = self.add_node(router, cv, Some(prev_node));

            if mark_edges {
                let mut edge = VertInf::has_neighbour(router, prev_vert, cv, self.is_orthogonal);
                if edge.is_none() && (router.vert_infs[cv].id == self.dimension_change_vertex_id) {
                    let mod_curr = if router.vert_infs[cv].id == self.dimension_change_vertex_id {
                        router.vert_infs[cv].m_orthogonal_partner.unwrap()
                    } else {
                        cv
                    };
                    let mod_prev = if router.vert_infs[prev_vert].id == self.dimension_change_vertex_id {
                        router.vert_infs[prev_vert].m_orthogonal_partner.unwrap()
                    } else {
                        prev_vert
                    };
                    edge = VertInf::has_neighbour(router, mod_prev, mod_curr, self.is_orthogonal);
                }
                let edge = edge.expect("buildHyperedgeTreeToRoot(): no edge");
                router.edge_infs[edge].set_hyperedge_segment(true);
            }

            if self.tree.node(current_node).junction.is_some() {
                // We've reached a junction, so stop.
                break;
            }

            if router.vert_infs[cv].path_next.is_none() {
                // This is a terminal of the hyperedge, mark the node with the
                // vertex representing the endpoint of the connector so we can
                // later use this to set the correct ConnEnd for the connector.
                self.tree.node_mut(current_node).final_vertex = Some(cv);
            }

            if router.vert_infs[cv].id.is_dummy_pin_helper() {
                // Note if we have an extra dummy vertex for connecting
                // to possible connection pins.
                self.tree.node_mut(current_node).is_pin_dummy_endpoint = true;
            }

            prev_node = current_node;
            prev_vert = cv;
            curr_vert = router.vert_infs[cv].path_next;
        }
    }

    // upstream: mtst.cpp MinimumTerminalSpanningTree::resetDistsForPath()
    fn reset_dists_for_path(
        &mut self,
        router: &mut Router,
        curr_vert: VertIdx,
        new_root_vert_ptr: TreeRootPtr,
    ) -> Option<TreeRootPtr> {
        // This method follows branches in a shortest path tree back to the
        // root, generating hyperedge tree nodes and branches as it goes.
        let mut curr_vert = Some(curr_vert);
        while let Some(cv) = curr_vert {
            if router.vert_infs[cv].sptf_dist == 0.0 {
                let old_tree_root_ptr = router.vert_infs[cv].tree_root_pointer();
                // We've reached a junction, so stop.
                self.rewrite_rest_of_hyperedge(router, cv, new_root_vert_ptr);
                return old_tree_root_ptr;
            }

            router.vert_infs[cv].sptf_dist = 0.0;
            router.vert_infs[cv].set_tree_root_pointer(Some(new_root_vert_ptr));

            self.terminals.insert(vert_key(router, cv));

            curr_vert = router.vert_infs[cv].path_next;
        }

        // Shouldn't get here.
        debug_assert!(false);
        None
    }

    // upstream: mtst.cpp MinimumTerminalSpanningTree::constructSequential()
    pub fn construct_sequential(&mut self, router: &mut Router) {
        // First, perform extended Dijkstra's algorithm
        // ============================================
        //

        // Vertex heap for extended Dijkstra's algorithm.
        let mut v_heap: Vec<VertIdx> = Vec::new();

        // Bridging edge heap for the extended Kruskal's algorithm.
        let mut be_heap: Vec<EdgeIdx> = Vec::new();

        // Initialisation
        //
        let mut k = router.vertices.conns_begin();
        while let Some(kv) = k {
            let v = &mut router.vert_infs[kv];
            v.sptf_dist = DBL_MAX;
            v.path_next = None;
            v.set_sptf_root(Some(kv));
            k = v.lst_next;
        }
        // PTR-ORDER: iterates std::set<VertInf *> terminals.
        let terminals: Vec<VertIdx> = self.terminals.iter().map(|k| k.idx).collect();
        for t in terminals {
            // This is a terminal, set a distance of zero.
            router.vert_infs[t].sptf_dist = 0.0;
            self.make_set(router, t);
            v_heap.push(t);
        }
        v_make_heap(router, &mut v_heap);

        // Shortest path terminal forest construction
        //
        while !v_heap.is_empty() {
            // Take the lowest vertex from heap.
            let u = v_heap[0];

            // Pop the lowest vertex off the heap.
            v_pop_heap(router, &mut v_heap);
            v_heap.pop();

            // For each edge from this vertex...
            // (Only other vertices' edge lists are modified in the loop.)
            let vis_list: Vec<EdgeIdx> = if !self.is_orthogonal {
                router.vert_infs[u].vis_list.to_vec(&router.edge_infs)
            } else {
                router.vert_infs[u].orthog_vis_list.to_vec(&router.edge_infs)
            };
            let mut extra_vertex: Option<VertIdx> = None;
            for &edge in vis_list.iter() {
                let v = router.edge_infs[edge].other_vert(u);
                let mut edge_dist = router.edge_infs[edge].get_dist();

                // Assign a distance (length) of 1 for dummy visibility edges
                // which may not accurately reflect the real distance of the edge.
                if router.vert_infs[v].id.is_dummy_pin_helper() || router.vert_infs[u].id.is_dummy_pin_helper() {
                    edge_dist = 1.0;
                }

                // Ignore an edge we have already explored.
                let u_path_next = router.vert_infs[u].path_next;
                if u_path_next == Some(v)
                    || (u_path_next.is_some() && router.vert_infs[u_path_next.unwrap()].path_next == Some(v))
                {
                    continue;
                }

                // Don't do anything more here if this is an intra-tree edge that
                // would just bridge branches of the same tree.
                if router.vert_infs[u].sptf_root() == router.vert_infs[v].sptf_root() {
                    continue;
                }

                // This is an extension to the original method that takes a bend
                // cost into account.  When edges from this node, we take into
                // account the direction of the branch in the tree that got us
                // here.  For an edge colinear to this we do the normal thing,
                // and add it to the heap.  For edges at right angle, we don't
                // immediately add these, but instead add a dummy segment and node
                // at the current position and give the edge an distance equal to
                // the bend penalty.  We add equivalent edges for the right-angled
                // original edges, so these may be explored when the algorithm
                // explores the dummy node.  Obviously we also need to clean up
                // these dummy nodes and edges later.
                let new_cost = router.vert_infs[u].sptf_dist + edge_dist;

                // (upstream stores the bool in a double.)
                let free_connection: f64 = if self.connects_without_bend(router, u, v) { 1.0 } else { 0.0 };
                if free_connection == 0.0 {
                    // This edge is not colinear, so add it to the dummy node and
                    // ignore it.
                    debug_assert!(router.vert_infs[u].id != self.dimension_change_vertex_id);
                    if extra_vertex.is_none() {
                        // Create the dummy node if necessary.
                        let point = router.vert_infs[u].point;
                        let ev = VertInf::new(router, self.dimension_change_vertex_id, point, false);
                        extra_vertex = Some(ev);
                        self.extra_vertices.push(ev);
                        router.vert_infs[ev].sptf_dist = self.bend_penalty + router.vert_infs[u].sptf_dist;
                        router.vert_infs[ev].path_next = Some(u);
                        let root = router.vert_infs[u].sptf_root();
                        router.vert_infs[ev].set_sptf_root(root);
                        v_heap.push(ev);
                        v_push_heap(router, &mut v_heap);
                    }
                    // Add a copy of the ignored edge to the dummy node, so it
                    // may be explored later.
                    let extra_edge = EdgeInf::new(router, extra_vertex.unwrap(), v, self.is_orthogonal);
                    EdgeInf::set_dist(router, extra_edge, edge_dist);
                    continue;
                }

                if new_cost < router.vert_infs[v].sptf_dist && router.vert_infs[v].sptf_root() == Some(v) {
                    // We have got to a node we haven't explored to from any tree.
                    // So attach it to the tree and update it with the distance
                    // from the root to reach this vertex.  Then add the vertex
                    // to the heap of potentials to explore.
                    router.vert_infs[v].sptf_dist = new_cost;
                    router.vert_infs[v].path_next = Some(u);
                    let root = router.vert_infs[u].sptf_root();
                    router.vert_infs[v].set_sptf_root(root);
                    v_heap.push(v);
                    v_push_heap(router, &mut v_heap);
                } else {
                    // We have reached a node that has been reached already through
                    // a different tree.  Set the MTST distance for the bridging
                    // edge and push it to the priority queue of edges to consider
                    // during the extended Kruskal's algorithm.
                    let second_join_cost = if self.connects_without_bend(router, v, u) {
                        0.0
                    } else {
                        self.bend_penalty
                    };

                    // The default cost is the cost back to the root of each
                    // forest plus the length of this edge.
                    let e = &router.edge_infs[edge];
                    let cost = router.vert_infs[e.m_vert1].sptf_dist
                        + router.vert_infs[e.m_vert2].sptf_dist
                        + second_join_cost
                        + e.get_dist();
                    router.edge_infs[edge].set_mtst_dist(cost);
                    be_heap.push(edge);
                }
            }
        }
        // Make the bridging edge heap.
        e_make_heap(router, &mut be_heap);

        // Next, perform extended Kruskal's algorithm
        // ==========================================
        //
        while !be_heap.is_empty() {
            // Take the lowest cost edge.
            let e = be_heap[0];

            // Pop the lowest cost edge off of the heap.
            e_pop_heap(router, &mut be_heap);
            be_heap.pop();

            // Find the sets of terminals that each of the trees connects.
            let (ev1, ev2) = (router.edge_infs[e].m_vert1, router.edge_infs[e].m_vert2);
            let s1 = self.find_set(router, router.vert_infs[ev1].sptf_root());
            let s2 = self.find_set(router, router.vert_infs[ev2].sptf_root());

            if s1.is_none() || s2.is_none() {
                // This is a special case if we would be connecting to something
                // that isn't a standard terminal shortest path tree, and thus
                // doesn't have a root.
                continue;
            }

            // We ignore edges that don't connect us to anything new, i.e., when
            // the terminal sets are the same.
            if s1 != s2 {
                // This is bridging us to one or more new terminals.

                // Union the terminal sets.
                self.union_sets(s1.unwrap(), s2.unwrap());

                // Connect this edge into the MTST by building HyperedgeTree nodes
                // and edges for this edge and the path back to the tree root.
                let mut node1 = None;
                let mut node2 = None;
                if self.hyperedge_tree_junctions {
                    node1 = Some(self.add_node(router, ev1, None));
                    node2 = Some(self.add_node(router, ev2, node1));
                }

                let pn1 = router.vert_infs[ev1].path_next;
                self.build_hyperedge_tree_to_root(router, pn1, node1, ev1, false);
                let pn2 = router.vert_infs[ev2].path_next;
                self.build_hyperedge_tree_to_root(router, pn2, node2, ev2, false);
            }
        }

        // Free the dummy nodes and edges created earlier.
        for ev in std::mem::take(&mut self.extra_vertices) {
            delete_vertex(router, ev);
        }
        self.nodes.clear();
        self.allsets.clear();
    }

    // upstream: mtst.cpp MinimumTerminalSpanningTree::orthogonalPartner()
    fn orthogonal_partner(&mut self, router: &mut Router, vert: VertIdx, penalty: f64) -> VertIdx {
        let mut penalty = penalty;
        if penalty == 0.0 {
            penalty = self.bend_penalty;
        }
        if router.vert_infs[vert].m_orthogonal_partner.is_none() {
            let point = router.vert_infs[vert].point;
            let partner = VertInf::new(router, self.dimension_change_vertex_id, point, false);
            router.vert_infs[vert].m_orthogonal_partner = Some(partner);
            router.vert_infs[partner].m_orthogonal_partner = Some(vert);
            self.extra_vertices.push(partner);
            let extra_edge = EdgeInf::new(router, partner, vert, self.is_orthogonal);
            EdgeInf::set_dist(router, extra_edge, penalty);
        }
        router.vert_infs[vert].m_orthogonal_partner.unwrap()
    }

    // upstream: mtst.cpp MinimumTerminalSpanningTree::removeInvalidBridgingEdges()
    fn remove_invalid_bridging_edges(&mut self, router: &Router) {
        // Look through the bridging edge heap for any now invalidated edges and
        // remove these by only copying valid edges to the beHeapNew array.
        let mut be_heap_new: Vec<EdgeIdx> = Vec::with_capacity(self.be_heap.len());
        for i in 0..self.be_heap.len() {
            let e = self.be_heap[i];

            let ends = self.real_vertices_counting_partners(router, e);
            let r1 = VertInf::tree_root(router, ends.0);
            let r2 = VertInf::tree_root(router, ends.1);
            let valid = (r1 != r2)
                && r1.is_some()
                && r2.is_some()
                && self.orig_terminals.contains(&vert_key(router, r1.unwrap()))
                && self.orig_terminals.contains(&vert_key(router, r2.unwrap()));
            if !valid {
                // This is an invalid edge, don't copy it to beHeapNew.
                continue;
            }

            // Copy the other bridging edges to beHeapNew.
            be_heap_new.push(self.be_heap[i]);
        }
        // Replace beHeap with beHeapNew
        self.be_heap = be_heap_new;

        // Remake the bridging edge heap, since we've deleted many elements.
        e_make_heap(router, &mut self.be_heap);
    }

    // upstream: mtst.cpp MinimumTerminalSpanningTree::getOrthogonalEdgesFromVertex()
    fn get_orthogonal_edges_from_vertex(
        &mut self,
        router: &mut Router,
        vert: VertIdx,
        prev: Option<VertIdx>,
    ) -> LayeredOrthogonalEdgeList {
        let mut edge_list = LayeredOrthogonalEdgeList::new();

        let penalty = if prev.is_none() { 0.1 } else { 0.0 };
        self.orthogonal_partner(router, vert, penalty);

        let is_real_vert = router.vert_infs[vert].id != self.dimension_change_vertex_id;
        let real_vert = if is_real_vert {
            vert
        } else {
            self.orthogonal_partner(router, vert, 0.0)
        };
        debug_assert!(router.vert_infs[real_vert].id != self.dimension_change_vertex_id);
        // (The loop only creates partners of *other* vertices, which
        // modifies their edge lists but never realVert's.)
        let vis_list: Vec<EdgeIdx> = if !self.is_orthogonal {
            router.vert_infs[real_vert].vis_list.to_vec(&router.edge_infs)
        } else {
            router.vert_infs[real_vert].orthog_vis_list.to_vec(&router.edge_infs)
        };
        for &edge in vis_list.iter() {
            let other = router.edge_infs[edge].other_vert(real_vert);

            if other == self.orthogonal_partner(router, real_vert, 0.0) {
                let partner = if is_real_vert {
                    other
                } else {
                    self.orthogonal_partner(router, other, 0.0)
                };
                if Some(partner) != prev {
                    edge_list.push((edge, partner));
                }
                continue;
            }

            let partner = if is_real_vert {
                other
            } else {
                self.orthogonal_partner(router, other, 0.0)
            };

            let other_point = router.vert_infs[other].point;
            let real_point = router.vert_infs[real_vert].point;
            if other_point.y == real_point.y {
                if is_real_vert && (prev != Some(partner)) {
                    edge_list.push((edge, partner));
                }
            } else if other_point.x == real_point.x {
                if !is_real_vert && (prev != Some(partner)) {
                    edge_list.push((edge, partner));
                }
            } else {
                print!("Warning, nonorthogonal edge.\n");
                edge_list.push((edge, other));
            }
        }

        edge_list
    }

    // upstream: mtst.cpp MinimumTerminalSpanningTree::constructInterleaved()
    pub fn construct_interleaved(&mut self, router: &mut Router) {
        // Perform an interleaved construction of the MTST and SPTF
        // ========================================================
        //
        self.orig_terminals = self.terminals.clone();

        // Initialisation
        //
        let mut k = router.vertices.conns_begin();
        while let Some(kv) = k {
            let v = &mut router.vert_infs[kv];
            v.sptf_dist = DBL_MAX;
            v.path_next = None;
            v.set_tree_root_pointer(None);
            v.m_orthogonal_partner = None;
            k = v.lst_next;
        }

        debug_assert!(self.root_vertex_pointers.is_empty());
        // PTR-ORDER: iterates std::set<VertInf *> terminals.
        let terminals: Vec<VertIdx> = self.terminals.iter().map(|k| k.idx).collect();
        for t in terminals {
            // This is a terminal, set a distance of zero.
            router.vert_infs[t].sptf_dist = 0.0;
            let p = VertInf::make_tree_root_pointer(router, t, Some(t));
            self.root_vertex_pointers.push(p);
            self.v_heap.push(t);
        }
        let mut t = router.vis_orthog_graph.begin();
        while let Some(te) = t {
            router.edge_infs[te].set_hyperedge_segment(false);
            t = router.edge_infs[te].lst_next;
        }

        v_make_heap(router, &mut self.v_heap);

        // Shortest Path Terminal Forest construction
        //
        while !self.v_heap.is_empty() {
            // Take the lowest vertex from heap.
            let u = self.v_heap[0];

            // There should be no orphaned vertices.
            debug_assert!(VertInf::tree_root(router, u).is_some());
            debug_assert!(router.vert_infs[u].path_next.is_some() || (router.vert_infs[u].sptf_dist == 0.0));

            if !self.be_heap.is_empty()
                && router.vert_infs[u].sptf_dist >= (0.5 * router.edge_infs[self.be_heap[0]].mtst_dist())
            {
                // Take the lowest cost edge.
                let e = self.be_heap[0];

                // Pop the lowest cost edge off of the heap.
                e_pop_heap(router, &mut self.be_heap);
                self.be_heap.pop();

                #[cfg(debug_assertions)]
                {
                    let ends = self.real_vertices_counting_partners(router, e);
                    debug_assert!(self
                        .orig_terminals
                        .contains(&vert_key(router, VertInf::tree_root(router, ends.0).unwrap())));
                    debug_assert!(self
                        .orig_terminals
                        .contains(&vert_key(router, VertInf::tree_root(router, ends.1).unwrap())));
                }

                self.commit_to_bridging_edge(router, e);

                if self.orig_terminals.len() == 1 {
                    break;
                }

                self.remove_invalid_bridging_edges(router);

                // Don't pop this vertex, but continue.
                continue;
            }

            // Pop the lowest vertex off the heap.
            v_pop_heap(router, &mut self.v_heap);
            self.v_heap.pop();

            // For each edge from this vertex...
            let u_path_next = router.vert_infs[u].path_next;
            let edge_list = self.get_orthogonal_edges_from_vertex(router, u, u_path_next);
            for &(e, v) in edge_list.iter() {
                let mut edge_dist = router.edge_infs[e].get_dist();

                // Assign a distance (length) of 1 for dummy visibility edges
                // which may not accurately reflect the real distance of the edge.
                if router.vert_infs[v].id.is_dummy_pin_helper() || router.vert_infs[u].id.is_dummy_pin_helper() {
                    edge_dist = 1.0;
                }

                // Don't do anything more here if this is an intra-tree edge that
                // would just bridge branches of the same tree.
                if VertInf::tree_root(router, u) == VertInf::tree_root(router, v) {
                    continue;
                }

                // This is an extension to the original method that takes a bend
                // cost into account.  For edges from this node, we take into
                // account the direction of the branch in the tree that got us
                // here.  For an edge colinear to this we do the normal thing,
                // and add it to the heap.  For edges at right angle, we don't
                // immediately add these, but instead add a dummy segment and node
                // at the current position and give the edge an distance equal to
                // the bend penalty.  We add equivalent edges for the right-angled
                // original edges, so these may be explored when the algorithm
                // explores the dummy node.  Obviously we also need to clean up
                // these dummy nodes and edges later.
                if VertInf::tree_root(router, v).is_none() {
                    let new_cost = router.vert_infs[u].sptf_dist + edge_dist;

                    // We have got to a node we haven't explored to from any tree.
                    // So attach it to the tree and update it with the distance
                    // from the root to reach this vertex.  Then add the vertex
                    // to the heap of potentials to explore.
                    router.vert_infs[v].sptf_dist = new_cost;
                    router.vert_infs[v].path_next = Some(u);
                    let p = router.vert_infs[u].tree_root_pointer();
                    router.vert_infs[v].set_tree_root_pointer(p);
                    self.v_heap.push(v);
                    // This can change the cost of other vertices in the heap,
                    // so we need to remake it.
                    v_make_heap(router, &mut self.v_heap);
                } else {
                    // We have reached a node that has been reached already through
                    // a different tree.  Set the MTST distance for the bridging
                    // edge and push it to the priority queue of edges to consider
                    // during the extended Kruskal's algorithm.
                    let cost =
                        router.vert_infs[v].sptf_dist + router.vert_infs[u].sptf_dist + router.edge_infs[e].get_dist();
                    let found = self.be_heap.contains(&e);
                    if !found {
                        // We need to add the edge to the bridging edge heap.
                        router.edge_infs[e].set_mtst_dist(cost);
                        self.be_heap.push(e);
                        e_push_heap(router, &mut self.be_heap);
                    } else {
                        // This edge is already in the bridging edge heap.
                        if cost < router.edge_infs[e].mtst_dist() {
                            // Update the edge's mtstDist if we compute a lower
                            // cost than we had before.
                            router.edge_infs[e].set_mtst_dist(cost);
                            e_make_heap(router, &mut self.be_heap);
                        }
                    }
                }
            }
        }
        // Upstream: COLA_ASSERT(origTerminals.size() == 1).  Not ported
        // (assert policy, router.rs module docs): it fails on valid input
        // whenever a terminal cannot reach the others -- e.g. a free point
        // terminal under orthogonal-only routing gets no orthogonal edges
        // (ConnEnd::getHyperedgeVertex), or a terminal enclosed by shapes;
        // the release build continues (the unreachable terminal is left
        // out of the tree).  Covered by tests/robustness.rs
        // (hyperedge_with_coincident_terminals).

        // Free Root Vertex Points from all vertices.
        for p in std::mem::take(&mut self.root_vertex_pointers) {
            VertInf::free_tree_root_pointer(router, p);
        }

        // Free the dummy nodes and edges created earlier.
        for ev in std::mem::take(&mut self.extra_vertices) {
            delete_vertex(router, ev);
        }
    }

    // upstream: mtst.cpp MinimumTerminalSpanningTree::connectsWithoutBend()
    fn connects_without_bend(&self, router: &Router, old_leaf: VertIdx, new_leaf: VertIdx) -> bool {
        debug_assert!(self.is_orthogonal);

        if router.vert_infs[old_leaf].sptf_dist == 0.0 {
            let mut hyperedge_connection = false;
            let vis_list = if !self.is_orthogonal {
                &router.vert_infs[old_leaf].vis_list
            } else {
                &router.vert_infs[old_leaf].orthog_vis_list
            };
            let old_point = router.vert_infs[old_leaf].point;
            let new_point = router.vert_infs[new_leaf].point;
            for edge in vis_list.iter(&router.edge_infs) {
                let other = router.edge_infs[edge].other_vert(old_leaf);

                if other == new_leaf {
                    continue;
                }

                let other_point = router.vert_infs[other].point;
                if other_point == old_point {
                    continue;
                }

                if router.edge_infs[edge].is_hyperedge_segment() {
                    hyperedge_connection = true;
                    if colinear(&other_point, &old_point, &new_point, 0.0) {
                        return true;
                    }
                }
            }
            // If there was no hyperedge to connect to, then its a tree source
            // and we can connect without a bend.
            !hyperedge_connection
        } else {
            if let Some(pn) = router.vert_infs[old_leaf].path_next {
                colinear(
                    &router.vert_infs[pn].point,
                    &router.vert_infs[old_leaf].point,
                    &router.vert_infs[new_leaf].point,
                    0.0,
                )
            } else {
                // No penalty.
                true
            }
        }
    }

    // upstream: mtst.cpp MinimumTerminalSpanningTree::rewriteRestOfHyperedge()
    fn rewrite_rest_of_hyperedge(&mut self, router: &mut Router, vert: VertIdx, new_tree_root_ptr: TreeRootPtr) {
        router.vert_infs[vert].set_tree_root_pointer(Some(new_tree_root_ptr));

        let edge_list = self.get_orthogonal_edges_from_vertex(router, vert, None);
        for &(_, v) in edge_list.iter() {
            if router.vert_infs[v].tree_root_pointer() == Some(new_tree_root_ptr) {
                // Already marked.
                continue;
            }

            if router.vert_infs[v].sptf_dist == 0.0 {
                // This is part of the rest of an existing hyperedge,
                // so mark it and continue.
                self.rewrite_rest_of_hyperedge(router, v, new_tree_root_ptr);
            }
        }
    }

    // upstream: mtst.cpp MinimumTerminalSpanningTree::realVerticesCountingPartners()
    fn real_vertices_counting_partners(&self, router: &Router, edge: EdgeIdx) -> VertexPair {
        let v1 = router.edge_infs[edge].m_vert1;
        let v2 = router.edge_infs[edge].m_vert2;

        let mut real_vertices = (v1, v2);

        let vi1 = &router.vert_infs[v1];
        let vi2 = &router.vert_infs[v2];
        if (vi1.id != self.dimension_change_vertex_id)
            && (vi2.id != self.dimension_change_vertex_id)
            && (vi1.point != vi2.point)
            && (vi1.point.x == vi2.point.x)
        {
            if let Some(p) = vi1.m_orthogonal_partner {
                real_vertices.0 = p;
            }
            if let Some(p) = vi2.m_orthogonal_partner {
                real_vertices.1 = p;
            }
        }

        real_vertices
    }

    // upstream: mtst.cpp MinimumTerminalSpanningTree::commitToBridgingEdge()
    fn commit_to_bridging_edge(&mut self, router: &mut Router, e: EdgeIdx) {
        let ends = self.real_vertices_counting_partners(router, e);
        let r1 = VertInf::tree_root(router, ends.0);
        let r2 = VertInf::tree_root(router, ends.1);
        // PTR-ORDER: std::min / std::max of two VertInf pointers.
        let ptr_less = |a: Option<VertIdx>, b: Option<VertIdx>| -> bool {
            let ka = a.map(|v| router.vert_infs[v].serial);
            let kb = b.map(|v| router.vert_infs[v].serial);
            ka < kb
        };
        // std::min(a, b) == (b < a) ? b : a;  std::max(a, b) == (a < b) ? b : a.
        let new_root = if ptr_less(r2, r1) { r2 } else { r1 };
        let old_root = if ptr_less(r1, r2) { r2 } else { r1 };

        // Connect this edge into the MTST by building HyperedgeTree nodes
        // and edges for this edge and the path back to the tree root.
        let mut node1 = None;
        let mut node2 = None;

        let vert1 = ends.0;
        let vert2 = ends.1;
        if self.hyperedge_tree_junctions {
            node1 = Some(self.add_node(router, vert1, None));
            node2 = Some(self.add_node(router, vert2, node1));
            router.edge_infs[e].set_hyperedge_segment(true);
        }

        let pn1 = router.vert_infs[vert1].path_next;
        self.build_hyperedge_tree_to_root(router, pn1, node1, vert1, true);
        let pn2 = router.vert_infs[vert2].path_next;
        self.build_hyperedge_tree_to_root(router, pn2, node2, vert2, true);

        // We are commmitting to a particular path and pruning back the shortest
        // path terminal forests from the roots of that path.  We do this by
        // rewriting the treeRootPointers for all the points on the current
        // hyperedge path to newTreeRootPtr.  The rest of the vertices in the
        // forest will be pruned by rewriting their treeRootPointer to nullptr.
        let old_tree_root_ptr1 = router.vert_infs[vert1].tree_root_pointer();
        let old_tree_root_ptr2 = router.vert_infs[vert2].tree_root_pointer();
        if let Some(old_root) = old_root {
            let key = vert_key(router, old_root);
            self.orig_terminals.remove(&key);
        }
        let new_tree_root_ptr = VertInf::make_tree_root_pointer(router, vert1, new_root);
        self.root_vertex_pointers.push(new_tree_root_ptr);
        router.vert_infs[vert2].set_tree_root_pointer(Some(new_tree_root_ptr));

        // Zero paths and rewrite the vertices on the hyperedge path to the
        // newTreeRootPtr.  Also, add vertices on path to the terminal set.
        debug_assert!(new_root.is_some());
        self.reset_dists_for_path(router, vert1, new_tree_root_ptr);
        self.reset_dists_for_path(router, vert2, new_tree_root_ptr);

        // Prune the forests from the joined vertex sets by setting their
        // treeRootPointers to nullptr.
        let old_tree_root_ptr1 = old_tree_root_ptr1.expect("commitToBridgingEdge(): null oldTreeRootPtr1");
        let old_tree_root_ptr2 = old_tree_root_ptr2.expect("commitToBridgingEdge(): null oldTreeRootPtr2");
        router.tree_root_cells[old_tree_root_ptr1] = None;
        router.tree_root_cells[old_tree_root_ptr2] = None;

        // We have found the full hyperedge path when we have joined all the
        // terminal sets into one.
        if self.orig_terminals.len() == 1 {
            return;
        }

        // Remove newly orphaned vertices from vertex heap by only copying the
        // valid vertices to vHeapNew array which then replaces vHeap.
        let mut v_heap_new: Vec<VertIdx> = Vec::with_capacity(self.v_heap.len());
        for i in 0..self.v_heap.len() {
            let v = self.v_heap[i];

            if VertInf::tree_root(router, v).is_none() {
                // This is an orphaned vertex.
                continue;
            }

            // Copy the other vertices to vHeapNew.
            v_heap_new.push(self.v_heap[i]);
        }
        // Replace vHeap with vHeapNew
        self.v_heap = v_heap_new;

        // Reset all terminals to zero.
        // PTR-ORDER: iterates std::set<VertInf *> terminals.
        for v2 in self.terminals.iter() {
            debug_assert!(router.vert_infs[v2.idx].sptf_dist == 0.0);
            self.v_heap.push(v2.idx);
        }

        // Rebuild the heap since some terminals will have had distances
        // rewritten as well as the orphaned vertices being removed.
        v_make_heap(router, &mut self.v_heap);
    }
}
