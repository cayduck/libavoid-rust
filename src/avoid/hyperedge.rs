//! Faithful port of upstream `libavoid/hyperedge.{h,cpp}` — PARTIAL.
//!
//! The data members and the trivial accessors (constructor,
//! `registerHyperedgeForRerouting`, `count`, `newAndDeletedObjectLists`)
//! are ported because the router needs them; the algorithmic methods are
//! `todo!` stubs (signatures fixed; bodies to be filled in by the
//! hyperedge port).  Methods that need router access are associated
//! functions taking `&mut Router` (the rerouter lives in
//! `Router::m_hyperedge_rerouter`).
//!
//! (`setRouter()` has no counterpart: the router is passed explicitly.)

#![allow(dead_code)]
#![allow(clippy::new_without_default)]

use std::collections::BTreeSet;

use crate::avoid::arena::{ConnIdx, PtrKey, VertIdx};
use crate::avoid::connend::ConnEnd;
use crate::avoid::junction::JunctionRef;
use crate::avoid::router::Router;

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
        debug_assert!(index <= self.count());

        HyperedgeNewAndDeletedObjectLists {
            new_junction_list: self.m_new_junctions_vector[index].clone(),
            deleted_junction_list: self.m_deleted_junctions_vector[index].clone(),
            new_connector_list: self.m_new_connectors_vector[index].clone(),
            deleted_connector_list: self.m_deleted_connectors_vector[index].clone(),
            changed_connector_list: ConnRefList::new(),
        }
    }

    /// Operates on `router.m_hyperedge_rerouter`.
    // upstream: hyperedge.cpp HyperedgeRerouter::calcHyperedgeConnectors()
    pub fn calc_hyperedge_connectors(router: &mut Router) -> ConnRefSet {
        let _ = router;
        todo!("upstream: hyperedge.cpp HyperedgeRerouter::calcHyperedgeConnectors — not yet ported")
    }

    /// Operates on `router.m_hyperedge_rerouter`.
    // upstream: hyperedge.cpp HyperedgeRerouter::performRerouting()
    pub fn perform_rerouting(router: &mut Router) {
        let _ = router;
        todo!("upstream: hyperedge.cpp HyperedgeRerouter::performRerouting — not yet ported")
    }
}
