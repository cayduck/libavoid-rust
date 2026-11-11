//! Faithful port of upstream `libavoid/viscluster.{h,cpp}`.
//!
//! `ClusterRef *` is a [`ClusterIdx`] into `Router::clusters`.
//! `m_clusterrefs_pos` (iterator into `Router::clusterRefs`) is not stored:
//! `Router::cluster_refs` is a `Vec` and `makeInactive` erases the unique
//! entry by value.

#![allow(dead_code)]
// Keep upstream statement shapes.
#![allow(
    clippy::collapsible_if,
    clippy::collapsible_else_if,
    clippy::print_with_newline,
    clippy::field_reassign_with_default,
    clippy::assign_op_pattern,
    clippy::explicit_counter_loop,
    clippy::if_same_then_else,
    clippy::question_mark,
    clippy::neg_multiply,
    clippy::needless_late_init,
    clippy::uninlined_format_args
)]
#![allow(clippy::new_ret_no_self)]

use crate::avoid::arena::{Arena, ClusterIdx};
use crate::avoid::geomtypes::{Polygon, PolygonInterface, ReferencingPolygon};
use crate::avoid::router::Router;

/// Arena of all clusters of a router.
pub type ClusterArena = Arena<ClusterIdx, ClusterRef>;

/// The ClusterRef class represents a cluster object.
// upstream: viscluster.h class ClusterRef
#[derive(Clone, Debug)]
pub struct ClusterRef {
    /// Monotonic creation serial (no upstream counterpart).
    pub serial: u64,
    pub m_id: u32,
    pub m_polygon: ReferencingPolygon,
    pub m_rectangular_polygon: Polygon,
    pub m_active: bool,
}

impl ClusterRef {
    /// Cluster reference constructor: creates the cluster and adds it to
    /// the router scene immediately.  `id` 0 means "assign one".
    // upstream: viscluster.cpp ClusterRef::ClusterRef()
    pub fn new(router: &mut Router, polygon: &Polygon, id: u32) -> ClusterIdx {
        let m_polygon = ReferencingPolygon::from_polygon(polygon, router.obstacle_id_list());
        let m_rectangular_polygon = m_polygon.view(&*router).bounding_rect_polygon();
        let serial = router.next_serial();
        let m_id = router.assign_id(id);
        let c = router.clusters.alloc(ClusterRef {
            serial,
            m_id,
            m_polygon,
            m_rectangular_polygon,
            m_active: false,
        });

        router.add_cluster(c);
        c
    }

    /// Upstream `delete cluster` (only valid from the router).
    // upstream: viscluster.cpp ClusterRef::~ClusterRef()
    pub fn delete(router: &mut Router, c: ClusterIdx) {
        if !router.m_currently_calling_destructors {
            eprint!("ERROR: ClusterRef::~ClusterRef() shouldn't be called directly.\n");
            eprint!("       It is owned by the router.  Call Router::deleteCluster() instead.\n");
            panic!("ClusterRef deleted directly instead of through the Router");
        }
        router.clusters.free(c);
    }

    // upstream: viscluster.cpp ClusterRef::makeActive()
    pub fn make_active(router: &mut Router, c: ClusterIdx) {
        debug_assert!(!router.clusters[c].m_active);

        // Add to clusterRefs list.
        router.cluster_refs.insert(0, c);

        router.clusters[c].m_active = true;
    }

    // upstream: viscluster.cpp ClusterRef::makeInactive()
    pub fn make_inactive(router: &mut Router, c: ClusterIdx) {
        debug_assert!(router.clusters[c].m_active);

        // Remove from clusterRefs list.
        let pos = router
            .cluster_refs
            .iter()
            .position(|&x| x == c)
            .expect("active cluster missing from clusterRefs");
        router.cluster_refs.remove(pos);

        router.clusters[c].m_active = false;
    }

    // upstream: viscluster.cpp ClusterRef::setNewPoly()
    pub fn set_new_poly(router: &mut Router, c: ClusterIdx, poly: &Polygon) {
        let m_polygon = ReferencingPolygon::from_polygon(poly, router.obstacle_id_list());
        let rect = m_polygon.view(&*router).bounding_rect_polygon();
        let cl = &mut router.clusters[c];
        cl.m_polygon = m_polygon;
        cl.m_rectangular_polygon = rect;
    }

    // upstream: viscluster.cpp ClusterRef::id()
    #[inline]
    pub fn id(&self) -> u32 {
        self.m_id
    }

    // upstream: viscluster.cpp ClusterRef::polygon()
    #[inline]
    pub fn polygon(&self) -> &ReferencingPolygon {
        &self.m_polygon
    }

    // upstream: viscluster.cpp ClusterRef::rectangularPolygon()
    #[inline]
    pub fn rectangular_polygon(&self) -> &Polygon {
        &self.m_rectangular_polygon
    }
}

/// Public-API convenience methods on cluster handles.
impl ClusterIdx {
    // upstream: viscluster.cpp ClusterRef::id()
    pub fn id(self, router: &Router) -> u32 {
        router.clusters[self].m_id
    }
    // upstream: viscluster.cpp ClusterRef::setNewPoly()
    pub fn set_new_poly(self, router: &mut Router, poly: &Polygon) {
        ClusterRef::set_new_poly(router, self, poly)
    }
    // upstream: viscluster.cpp ClusterRef::rectangularPolygon()
    pub fn rectangular_polygon(self, router: &Router) -> &Polygon {
        &router.clusters[self].m_rectangular_polygon
    }
}
