//! Faithful port of upstream `libavoid/hyperedge.{h,cpp}`.
//!
//! Methods that need router access are associated functions taking
//! `&mut Router` (the rerouter lives in `Router::m_hyperedge_rerouter`).
//!
//! (`setRouter()` has no counterpart: the router is passed explicitly.
//! `outputInstanceToSVG(FILE *)` writes to a `std::fmt::Write`.)

#![allow(dead_code)]
#![allow(clippy::new_without_default)]
// Keep upstream statement shapes.
#![allow(
    clippy::unnecessary_unwrap,
    clippy::only_used_in_recursion,
    clippy::needless_range_loop,
    clippy::if_same_then_else,
    clippy::print_with_newline
)]

use std::collections::BTreeSet;

use crate::avoid::arena::{ConnIdx, PtrKey, VertIdx};
use crate::avoid::connector::ConnRef;
use crate::avoid::connend::ConnEnd;
use crate::avoid::junction::JunctionRef;
use crate::avoid::mtst::{vert_key, MinimumTerminalSpanningTree};
use crate::avoid::obstacle::Obstacle;
use crate::avoid::router::Router;
use crate::avoid::vertices::VertInf;

// upstream: hyperedge.h typedefs
pub type ConnEndList = Vec<ConnEnd>;
pub type ConnRefList = Vec<ConnIdx>;
pub type JunctionRefList = Vec<JunctionRef>;
pub type VertexList = Vec<VertIdx>;
/// PTR-ORDER: upstream `std::set<ConnRef *>`, ordered by connector serial.
pub type ConnRefSet = BTreeSet<PtrKey<ConnIdx>>;
/// PTR-ORDER: upstream `std::set<VertInf *>`, ordered by vertex serial.
pub type VertexSet = BTreeSet<PtrKey<VertIdx>>;
pub type JunctionRefVector = Vec<Option<JunctionRef>>;
pub type ConnEndListVector = Vec<ConnEndList>;
pub type ConnRefListVector = Vec<ConnRefList>;
pub type JunctionRefListVector = Vec<JunctionRefList>;
pub type VertexSetVector = Vec<VertexSet>;

/// Lists of the junctions and connectors created and deleted during
/// hyperedge improvement.
// upstream: hyperedge.h struct HyperedgeNewAndDeletedObjectLists
#[derive(Clone, Debug, Default)]
pub struct HyperedgeNewAndDeletedObjectLists {
    pub new_junction_list: JunctionRefList,
    pub new_connector_list: ConnRefList,
    pub deleted_junction_list: JunctionRefList,
    pub deleted_connector_list: ConnRefList,
    pub changed_connector_list: ConnRefList,
}

// upstream: hyperedge.h class HyperedgeRerouter
#[derive(Clone, Debug, Default)]
pub struct HyperedgeRerouter {
    pub m_terminals_vector: ConnEndListVector,
    pub m_root_junction_vector: JunctionRefVector,
    pub m_new_junctions_vector: JunctionRefListVector,
    pub m_deleted_junctions_vector: JunctionRefListVector,
    pub m_new_connectors_vector: ConnRefListVector,
    pub m_deleted_connectors_vector: ConnRefListVector,
    pub m_terminal_vertices_vector: VertexSetVector,
    pub m_added_vertices: VertexList,
    /// Port addition: set by `find_attached_objects_*` when the registered
    /// hyperedge contains a cycle or a connector without an endpoint
    /// vertex (see there).
    pub m_cycle_detected: bool,
}

impl HyperedgeRerouter {
    // upstream: hyperedge.cpp HyperedgeRerouter::HyperedgeRerouter()
    pub fn new() -> HyperedgeRerouter {
        HyperedgeRerouter::default()
    }

    // upstream: hyperedge.cpp HyperedgeRerouter::registerHyperedgeForRerouting(ConnEndList)
    pub fn register_hyperedge_for_rerouting(&mut self, terminals: ConnEndList) -> usize {
        self.m_terminals_vector.push(terminals);
        self.m_root_junction_vector.push(None);

        self.m_terminals_vector.len() - 1
    }

    // upstream: hyperedge.cpp HyperedgeRerouter::registerHyperedgeForRerouting(JunctionRef *)
    pub fn register_hyperedge_for_rerouting_junction(&mut self, junction: JunctionRef) -> usize {
        self.m_terminals_vector.push(ConnEndList::new());
        self.m_root_junction_vector.push(Some(junction));

        self.m_terminals_vector.len() - 1
    }

    // upstream: hyperedge.cpp HyperedgeRerouter::count()
    pub fn count(&self) -> usize {
        self.m_terminals_vector.len()
    }

    // upstream: hyperedge.cpp HyperedgeRerouter::newAndDeletedObjectLists()
    pub fn new_and_deleted_object_lists(&self, index: usize) -> HyperedgeNewAndDeletedObjectLists {
        // Upstream: COLA_ASSERT(index <= count()).  Not ported: count() is 0
        // after every transaction (performRerouting() clears the terminals)
        // while the result vectors stay filled, so the assert fails on the
        // intended use (querying results after processTransaction()); it
        // is also off by one.  MISUSE: an index without results gives
        // empty lists (upstream reads out of bounds).
        if index >= self.m_new_junctions_vector.len()
            || index >= self.m_deleted_junctions_vector.len()
            || index >= self.m_new_connectors_vector.len()
            || index >= self.m_deleted_connectors_vector.len()
        {
            return HyperedgeNewAndDeletedObjectLists::default();
        }

        HyperedgeNewAndDeletedObjectLists {
            new_junction_list: self.m_new_junctions_vector[index].clone(),
            deleted_junction_list: self.m_deleted_junctions_vector[index].clone(),
            new_connector_list: self.m_new_connectors_vector[index].clone(),
            deleted_connector_list: self.m_deleted_connectors_vector[index].clone(),
            changed_connector_list: ConnRefList::new(),
        }
    }


    /// Writes C++ code that registers the pending hyperedges with a
    /// `hyperedgeRerouter` (reproduction code in the style of
    /// `Router::outputInstanceToSVG()`; upstream never calls it itself).
    /// `router` is the router this rerouter belongs to, for object ids.
    // upstream: hyperedge.cpp HyperedgeRerouter::outputInstanceToSVG()
    pub fn output_instance_to_svg(&self, router: &Router, fp: &mut dyn std::fmt::Write) -> std::fmt::Result {
        if self.count() == 0 {
            return Ok(());
        }

        fp.write_str("    HyperedgeRerouter *hyperedgeRerouter = router->hyperedgeRerouter();\n")?;
        let num_hyperedges = self.count();
        for i in 0..num_hyperedges {
            if let Some(root) = self.m_root_junction_vector[i] {
                // MISUSE: a root junction freed since registration prints id
                // 0 (upstream would read freed memory).
                write!(
                    fp,
                    "    hyperedgeRerouter->registerHyperedgeForRerouting(junctionRef{});\n",
                    root.id(router)
                )?;
            } else {
                write!(fp, "    ConnEndList heConnList{};\n", i as u32)?;
                for it in self.m_terminals_vector[i].iter() {
                    it.output_code(router, fp, "heEnd")?;
                    write!(fp, "    heConnList{}.push_back(heEndPt);\n", i as u32)?;
                }
                write!(
                    fp,
                    "    hyperedgeRerouter->registerHyperedgeForRerouting(heConnList{});\n",
                    i as u32
                )?;
            }
        }
        fp.write_str("\n")
    }
    // Follow connected junctions and connectors from the given connector to
    // determine the hyperedge topology, saving objects to the deleted-objects
    // vectors as we go.
    /// Operates on `router.m_hyperedge_rerouter`.
    // upstream: hyperedge.cpp HyperedgeRerouter::findAttachedObjects(size_t, ConnRef *, JunctionRef *, ConnRefSet&)
    pub fn find_attached_objects_conn(
        router: &mut Router,
        index: usize,
        connector: ConnIdx,
        ignore: Option<JunctionRef>,
        hyperedge_conns: &mut ConnRefSet,
    ) -> bool {
        let mut valid_hyperedge = false;

        // MISUSE / upstream bug (port addition): the traversal only ignores
        // the object it came from, so a hyperedge containing a cycle
        // (e.g. two junctions joined by two connectors) recurses forever
        // upstream.  A connector reached a second time is a cycle: stop
        // and let calc_hyperedge_connectors treat the hyperedge as invalid.
        if router.m_hyperedge_rerouter.m_deleted_connectors_vector[index].contains(&connector) {
            router.m_hyperedge_rerouter.m_cycle_detected = true;
            return false;
        }

        ConnRef::assign_connection_pin_visibility(router, connector, true);

        router.m_hyperedge_rerouter.m_deleted_connectors_vector[index].push(connector);
        hyperedge_conns.insert(PtrKey::new(router.conns[connector].serial, connector));

        let anchors = router.conns[connector].endpoint_anchors();
        let j_first = anchors.0.and_then(|o| Obstacle::as_junction(router, o));
        let j_second = anchors.1.and_then(|o| Obstacle::as_junction(router, o));

        if let Some(j_first) = j_first {
            // If attached to a junction and not one we've explored, then continue.
            if Some(j_first) != ignore {
                valid_hyperedge |= HyperedgeRerouter::find_attached_objects_junction(
                    router,
                    index,
                    j_first,
                    Some(connector),
                    hyperedge_conns,
                );
            }
        } else {
            // If its an endpoint, then record the vertex for this endpoint.
            // MISUSE / upstream bug (port addition): the vertex can be
            // null here (a connector whose endpoints were never set up);
            // upstream dereferences null (ASan: null VertInf in
            // ConnEnd::assignPinVisibilityTo()).  The hyperedge is treated
            // as invalid.
            if let Some(v) = router.conns[connector].m_src_vert {
                let key = vert_key(router, v);
                router.m_hyperedge_rerouter.m_terminal_vertices_vector[index].insert(key);
            } else {
                router.m_hyperedge_rerouter.m_cycle_detected = true;
            }
        }

        if let Some(j_second) = j_second {
            // If attached to a junction and not one we've explored, then continue.
            if Some(j_second) != ignore {
                valid_hyperedge |= HyperedgeRerouter::find_attached_objects_junction(
                    router,
                    index,
                    j_second,
                    Some(connector),
                    hyperedge_conns,
                );
            }
        } else {
            // If its an endpoint, then record the vertex for this endpoint.
            // MISUSE / upstream bug (port addition): the vertex can be
            // null here (a connector whose endpoints were never set up);
            // upstream dereferences null (ASan: null VertInf in
            // ConnEnd::assignPinVisibilityTo()).  The hyperedge is treated
            // as invalid.
            if let Some(v) = router.conns[connector].m_dst_vert {
                let key = vert_key(router, v);
                router.m_hyperedge_rerouter.m_terminal_vertices_vector[index].insert(key);
            } else {
                router.m_hyperedge_rerouter.m_cycle_detected = true;
            }
        }
        valid_hyperedge
    }

    // Follow connected junctions and connectors from the given junction to
    // determine the hyperedge topology, saving objects to the deleted-objects
    // vectors as we go.
    /// Operates on `router.m_hyperedge_rerouter`.
    // upstream: hyperedge.cpp HyperedgeRerouter::findAttachedObjects(size_t, JunctionRef *, ConnRef *, ConnRefSet&)
    pub fn find_attached_objects_junction(
        router: &mut Router,
        index: usize,
        junction: JunctionRef,
        ignore: Option<ConnIdx>,
        hyperedge_conns: &mut ConnRefSet,
    ) -> bool {
        let mut valid_hyperedge = false;

        router.m_hyperedge_rerouter.m_deleted_junctions_vector[index].push(junction);

        // PTR-ORDER: attachedConnectors() iterates std::set<ConnEnd *>.
        let connectors = junction.attached_connectors(router);

        if connectors.len() > 2 {
            // A valid hyperedge must have at least one junction with three
            // connectors attached, i.e., more than two endpoints.
            valid_hyperedge |= true;
        }

        for &curr in connectors.iter() {
            if Some(curr) == ignore {
                continue;
            }

            valid_hyperedge |=
                HyperedgeRerouter::find_attached_objects_conn(router, index, curr, Some(junction), hyperedge_conns);
        }
        valid_hyperedge
    }

    // Populate the deleted-object vectors with all the connectors and junctions
    // that form the registered hyperedges.  Then return the set of all these
    // connectors so they can be ignored for individual rerouting.
    /// Operates on `router.m_hyperedge_rerouter`.
    // upstream: hyperedge.cpp HyperedgeRerouter::calcHyperedgeConnectors()
    pub fn calc_hyperedge_connectors(router: &mut Router) -> ConnRefSet {
        let mut all_registered_hyperedge_conns = ConnRefSet::new();

        // Clear the deleted-object vectors.  We populate them here if necessary.
        let this = &mut router.m_hyperedge_rerouter;
        let count = this.count();
        this.m_deleted_junctions_vector.clear();
        this.m_deleted_junctions_vector.resize(count, JunctionRefList::new());
        this.m_deleted_connectors_vector.clear();
        this.m_deleted_connectors_vector.resize(count, ConnRefList::new());

        this.m_terminal_vertices_vector.clear();
        this.m_terminal_vertices_vector.resize(count, VertexSet::new());
        this.m_added_vertices.clear();

        // Populate the deleted-object vectors.
        let num_hyperedges = router.m_hyperedge_rerouter.count();
        for i in 0..num_hyperedges {
            if let Some(root_junction) = router.m_hyperedge_rerouter.m_root_junction_vector[i] {
                // MISUSE: a junction freed since registration is invalid.
                if !router.obstacles.contains(root_junction.0) {
                    crate::avoid::router::e2e_guard_fired("rerouter-freed-root-junction");
                    let this = &mut router.m_hyperedge_rerouter;
                    this.m_terminals_vector[i].clear();
                    continue;
                }
                // Follow objects attached to junction to find the hyperedge.
                router.m_hyperedge_rerouter.m_cycle_detected = false;
                let valid = HyperedgeRerouter::find_attached_objects_junction(
                    router,
                    i,
                    root_junction,
                    None,
                    &mut all_registered_hyperedge_conns,
                );
                let valid = valid && !router.m_hyperedge_rerouter.m_cycle_detected;
                if !valid {
                    crate::err_printf!(
                        "Warning: Hyperedge {} registered with HyperedgeRerouter is invalid and will be ignored.\n",
                        i as i32
                    );
                    // Hyperedge is invalid.  Clear the terminals and other info
                    // so it will be ignored, and rerouted as a normal set of
                    // connectors.
                    let this = &mut router.m_hyperedge_rerouter;
                    this.m_terminals_vector[i].clear();
                    this.m_terminal_vertices_vector[i].clear();
                    this.m_deleted_junctions_vector[i].clear();
                    this.m_deleted_connectors_vector[i].clear();
                }
                continue;
            }

            // Alternatively, we have a set of ConnEnds, so store the
            // corresponding terminals
            let terminals = router.m_hyperedge_rerouter.m_terminals_vector[i].clone();
            for it in terminals.iter() {
                // MISUSE: a terminal whose shape/junction was freed since
                // registration is a point end (upstream: use-after-free in
                // ConnEnd::getHyperedgeVertex(); ConnEnd::forget_dead_refs).
                let mut it = it.clone();
                it.forget_dead_refs(router);
                let maybe_new_vertex = it.get_hyperedge_vertex(router);
                let key = vert_key(router, maybe_new_vertex.1);
                router.m_hyperedge_rerouter.m_terminal_vertices_vector[i].insert(key);

                if maybe_new_vertex.0 {
                    // This is a newly created vertex.  Remember it so we can
                    // free it and it's visibility edges later.
                    router.m_hyperedge_rerouter.m_added_vertices.push(maybe_new_vertex.1);
                }
            }
        }

        // Return these connectors that don't require rerouting.
        all_registered_hyperedge_conns
    }

    /// Port addition (MISUSE, see `perform_rerouting`): a skipped hyperedge
    /// deletes nothing, so its deleted-object lists are cleared, as
    /// upstream's own invalid-hyperedge path in `calcHyperedgeConnectors()`
    /// does (otherwise `newAndDeletedObjectLists()` would report its
    /// connectors and junctions as deleted although they stay in the
    /// router).
    fn forget_skipped(router: &mut Router, i: usize) {
        let this = &mut router.m_hyperedge_rerouter;
        this.m_deleted_junctions_vector[i].clear();
        this.m_deleted_connectors_vector[i].clear();
    }

    /// Port addition (MISUSE, see `perform_rerouting`): puts back the
    /// `pathNext` links of the vertices that still exist.
    fn restore_path_next(router: &mut Router, saved: Vec<(VertIdx, Option<VertIdx>)>) {
        for (v, pn) in saved {
            if router.vert_infs.contains(v) {
                router.vert_infs[v].path_next = pn;
            }
        }
    }

    /// Port addition (MISUSE, see `perform_rerouting`): frees a junction
    /// that the MTST construction of a skipped hyperedge created and
    /// activated (`MinimumTerminalSpanningTree::addNode()`), as the
    /// router frees a removed junction in `processActions()`.  Nothing is
    /// attached to it yet.
    fn remove_unreported_junction(router: &mut Router, j: JunctionRef) {
        let o = j.0;
        if !router.obstacles.contains(o) {
            return;
        }
        router.remove_object_from_queued_actions(crate::avoid::actioninfo::ActionObj::Obstacle(o));
        if router.obstacles[o].m_active {
            Obstacle::remove_from_graph(router, o);
            Obstacle::make_inactive(router, o);
        }
        let was = router.m_currently_calling_destructors;
        router.m_currently_calling_destructors = true;
        Obstacle::delete(router, o);
        router.m_currently_calling_destructors = was;
    }

    /// Operates on `router.m_hyperedge_rerouter`.
    // upstream: hyperedge.cpp HyperedgeRerouter::performRerouting()
    pub fn perform_rerouting(router: &mut Router) {
        let this = &mut router.m_hyperedge_rerouter;
        let count = this.count();
        this.m_new_junctions_vector.clear();
        this.m_new_junctions_vector.resize(count, JunctionRefList::new());
        this.m_new_connectors_vector.clear();
        this.m_new_connectors_vector.resize(count, ConnRefList::new());

        #[cfg(feature = "debug-handler")]
        if router.has_debug_handler() {
            let obstacle_boxes = router.debug_obstacle_boxes();
            router.call_debug_handler(|h, r| h.update_obstacle_boxes(r, obstacle_boxes));
        }

        // For each hyperedge...
        let num_hyperedges = router.m_hyperedge_rerouter.count();
        for i in 0..num_hyperedges {
            if router.m_hyperedge_rerouter.m_terminal_vertices_vector[i].is_empty() {
                // Invalid hyperedge, ignore.
                continue;
            }

            // Execute the MTST method to find good junction positions and an
            // initial path.  A hyperedge tree will be built for the new route.
            let terminals = router.m_hyperedge_rerouter.m_terminal_vertices_vector[i].clone();
            // MISUSE / upstream bug (port addition): when two registered
            // hyperedges share objects (e.g. the same junction registered
            // twice), rerouting the first one frees connector vertices that
            // are terminals of the second; upstream reads them (ASan:
            // heap-use-after-free in MinimumTerminalSpanningTree::
            // constructInterleaved()).  Such a hyperedge is skipped.
            if terminals.iter().any(|t| !router.vert_infs.contains(t.idx)) {
                crate::avoid::router::e2e_guard_fired("rerouter-freed-terminal-vertices");
                HyperedgeRerouter::forget_skipped(router, i);
                continue;
            }
            let mut mtst = MinimumTerminalSpanningTree::new(terminals, true);

            // The older MTST construction method (faster, worse results).
            //mtst.constructSequential();

            // The preferred MTST construction method.
            // Slightly slower, better quality results.
            // (Port addition: every vertex's `pathNext` before the MTST
            // construction, restored if the hyperedge is skipped below.)
            let path_next_before: Vec<(VertIdx, Option<VertIdx>)> =
                router.vert_infs.iter_unordered().map(|(v, x)| (v, x.path_next)).collect();
            mtst.construct_interleaved(router);

            // MISUSE / upstream bug (port addition): with fewer than three
            // distinct terminal vertices (one or two terminals, coincident
            // terminals) or terminals the MTST cannot connect, no junction
            // is ever created and the root is null; upstream dereferences
            // it.  The hyperedge is skipped: no new objects; old ones (if
            // registered by junction) keep their routes.
            //
            // A skipped hyperedge leaves the router as if it had not been
            // registered, apart from its connectors not being rerouted in
            // this transaction (as with upstream's own invalid-hyperedge
            // path in calcHyperedgeConnectors()): its deleted-object lists
            // are cleared (it deletes nothing), the junctions the MTST
            // created are freed, and the `pathNext` links the MTST wrote
            // are restored (later A* searches start from a pre-existing
            // `pathNext`, see "Stale pathNext" in router.rs; left in place,
            // they sent fuzz scene s25-1253 on a different, faster-growing
            // trajectory than the same scene without the registration).
            let Some(tree_root) = mtst.root_junction() else {
                crate::avoid::router::e2e_guard_fired("rerouter-no-junction");
                mtst.destruct();
                HyperedgeRerouter::forget_skipped(router, i);
                HyperedgeRerouter::restore_path_next(router, path_next_before);
                continue;
            };
            // MISUSE / upstream bug (port addition): the tree built by the
            // MTST can contain a cycle (seen with registered hyperedges
            // whose terminals share pins/vertices); upstream's recursive
            // addConns() then never terminates (stack overflow, ASan:
            // stack-overflow in Obstacle::addFollowingConnEnd).  Such a
            // hyperedge is skipped like one without a junction (the tree
            // is dropped with `mtst`: destruct() would recurse forever too).
            if mtst.tree.contains_cycle_from(tree_root) {
                crate::avoid::router::e2e_guard_fired("rerouter-cyclic-mtst-tree");
                // The MTST construction has already created (and activated)
                // a junction for every branching node.  They are removed
                // again, so that the skip leaves no objects that no list
                // reports (fuzz scenes s25-1253, s25-1506 kept up to 13
                // such unattached junctions for good).
                let created: Vec<JunctionRef> = mtst.tree.nodes.iter().filter_map(|n| n.junction).collect();
                for j in created {
                    HyperedgeRerouter::remove_unreported_junction(router, j);
                }
                HyperedgeRerouter::forget_skipped(router, i);
                HyperedgeRerouter::restore_path_next(router, path_next_before);
                continue;
            }

            // Fill in connector information and join them to junctions of endpoints
            // of original connectors.
            let old_conns = router.m_hyperedge_rerouter.m_deleted_connectors_vector[i].clone();
            mtst.tree.node_add_conns(router, tree_root, None, &old_conns, None);

            // Output the list of new junctions and connectors from hyperedge tree.
            {
                let this = &mut router.m_hyperedge_rerouter;
                mtst.tree.node_list_junctions_and_connectors(
                    tree_root,
                    None,
                    &mut this.m_new_junctions_vector[i],
                    &mut this.m_new_connectors_vector[i],
                );
            }

            // Write paths from the hyperedge tree back into individual
            // connector routes.
            for pass in 0..2 {
                mtst.tree.node_write_edges_to_conns(router, tree_root, None, pass);
            }

            // Tell the router that we are deleting the objects used for the
            // previous path for the hyperedge.
            let deleted_conns = router.m_hyperedge_rerouter.m_deleted_connectors_vector[i].clone();
            for &curr in deleted_conns.iter() {
                // Clear visibility assigned for connection pins.
                ConnRef::assign_connection_pin_visibility(router, curr, false);

                router.delete_connector(curr);
            }
            let deleted_junctions = router.m_hyperedge_rerouter.m_deleted_junctions_vector[i].clone();
            for &curr in deleted_junctions.iter() {
                router.delete_junction(curr);
            }

            // (~MinimumTerminalSpanningTree)
            mtst.destruct();
        }

        // Clear the input to this class, so that new objects can be registered
        // for rerouting for the next time that transaction that is processed.
        let this = &mut router.m_hyperedge_rerouter;
        this.m_terminals_vector.clear();
        this.m_root_junction_vector.clear();

        // Free temporarily added vertices.
        let added = std::mem::take(&mut router.m_hyperedge_rerouter.m_added_vertices);
        for curr in added {
            VertInf::remove_from_graph(router, curr, true);
            router.vertices.remove_vertex(&mut router.vert_infs, Some(curr));
            VertInf::delete(router, curr);
        }
    }
}
