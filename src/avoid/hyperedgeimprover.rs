//! Faithful port of upstream `libavoid/hyperedgeimprover.{h,cpp}` —
//! PARTIAL.
//!
//! Only the members and methods the router needs are declared here: the
//! result lists (with `clear()` and `newAndDeletedObjectLists()`), and
//! `execute()` as a `todo!` stub.  The hyperedge-improver port must add the
//! remaining members (`m_hyperedge_tree_junctions`,
//! `m_hyperedge_tree_roots`, `m_root_shift_segments`,
//! `m_all_shift_segments`) and extend `clear()` accordingly, without
//! changing the signatures below.  Methods that need router access are
//! associated functions taking `&mut Router` (the improver lives in
//! `Router::m_hyperedge_improver`).
//!
//! (`setRouter()` has no counterpart: the router is passed explicitly.)

#![allow(dead_code)]
#![allow(clippy::new_without_default)]

use crate::avoid::hyperedge::{ConnRefList, HyperedgeNewAndDeletedObjectLists, JunctionRefList};
use crate::avoid::router::Router;

// upstream: hyperedgeimprover.h class HyperedgeImprover
#[derive(Clone, Debug, Default)]
pub struct HyperedgeImprover {
    pub m_new_junctions: JunctionRefList,
    pub m_deleted_junctions: JunctionRefList,
    pub m_new_connectors: ConnRefList,
    pub m_deleted_connectors: ConnRefList,
    pub m_changed_connectors: ConnRefList,
    pub m_debug_count: i32,
    /// Uninitialised until `execute()` upstream; false here.
    pub m_can_make_major_changes: bool,
}

impl HyperedgeImprover {
    // upstream: hyperedgeimprover.cpp HyperedgeImprover::HyperedgeImprover()
    pub fn new() -> HyperedgeImprover {
        let mut h = HyperedgeImprover::default();
        h.clear();
        h
    }

    // upstream: hyperedgeimprover.cpp HyperedgeImprover::clear()
    pub fn clear(&mut self) {
        // (m_hyperedge_tree_junctions, m_hyperedge_tree_roots,
        // m_root_shift_segments and m_all_shift_segments are added and
        // cleared here by the hyperedge-improver port.)
        self.m_new_junctions.clear();
        self.m_deleted_junctions.clear();
        self.m_new_connectors.clear();
        self.m_deleted_connectors.clear();
        self.m_changed_connectors.clear();
        self.m_debug_count = 0;
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

    /// Operates on `router.m_hyperedge_improver`.
    // upstream: hyperedgeimprover.cpp HyperedgeImprover::execute()
    pub fn execute(router: &mut Router, can_make_major_changes: bool) {
        router.m_hyperedge_improver.m_can_make_major_changes = can_make_major_changes;

        // PARTIAL (added by the visibility/makepath port): only the case
        // where no connector is attached to a junction is ported.  Then no
        // hyperedge tree is built and every later step of upstream
        // execute() iterates empty containers (buildOrthogonalChannelInfo
        // returns immediately for an empty segment list), so nothing else
        // happens.
        // Build Hyperedge trees.
        for &conn_ref in router.conn_refs.iter() {
            let conn = &router.conns[conn_ref];
            let j_front = conn.m_src_connend.as_ref().and_then(|e| e.junction(router));
            let j_back = conn.m_dst_connend.as_ref().and_then(|e| e.junction(router));
            if j_front.is_none() && j_back.is_none() {
                continue;
            }
            todo!("upstream: hyperedgeimprover.cpp HyperedgeImprover::execute — hyperedge trees not yet ported")
        }
    }
}
