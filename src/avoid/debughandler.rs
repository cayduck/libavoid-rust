//! Port of upstream `libavoid/debughandler.h`.
//!
//! You can implement [`DebugHandler`] and register an instance with
//! [`Router::set_debug_handler`] to receive debugging information.  This is
//! currently intended for internal use.
//!
//! Upstream only calls the handler when `DEBUGHANDLER` is defined (which
//! `debughandler.h` does whenever `NDEBUG` is not); here the calls are
//! compiled in with the `debug-handler` Cargo feature, off by default.
//! Without the feature a registered handler is never called.
//!
//! Differences from upstream:
//! * Every method also receives the router (upstream's `ConnRef *` and
//!   `VertInf *` arguments give the handler access to it); the handler is
//!   taken out of the router for the duration of the call, so
//!   `router.debug_handler()` is `None` inside a callback.
//! * `VertInf *` / `ConnRef *` are handles; the `std::set<VertInf *>` of
//!   `beginningHyperedgeReroutingWithEndpoints` is a slice in the set's
//!   order (PTR-ORDER: vertex creation order).
//! * The router owns the handler (upstream stores a pointer it does not own).

use crate::avoid::arena::{ConnIdx, VertIdx};
use crate::avoid::geomtypes::{Box, PolyLine};
use crate::avoid::router::Router;

/// Receives debugging information from the router (upstream `class
/// DebugHandler`; every method does nothing by default).
// upstream: debughandler.h class DebugHandler
pub trait DebugHandler {
    /// The obstacles being routed around.
    // upstream: debughandler.h DebugHandler::updateObstacleBoxes()
    fn update_obstacle_boxes(&mut self, router: &Router, obstacles: Vec<Box>) {
        let _ = (router, obstacles);
    }

    /// An updated connector route.  Optionally the ends of a just updated
    /// segment within the route between the indexes `index1` and `index2`.
    // upstream: debughandler.h DebugHandler::updateConnectorRoute()
    fn update_connector_route(&mut self, router: &Router, conn: ConnIdx, index1: i32, index2: i32) {
        let _ = (router, conn, index1, index2);
    }

    /// The current endpoints that a path is being searched for between
    /// `src` and `tar`.
    // upstream: debughandler.h DebugHandler::beginningSearchWithEndpoints()
    fn beginning_search_with_endpoints(&mut self, router: &Router, src: VertIdx, tar: VertIdx) {
        let _ = (router, src, tar);
    }

    /// The current search path.
    // upstream: debughandler.h DebugHandler::updateCurrentSearchPath()
    fn update_current_search_path(&mut self, router: &Router, current_path: PolyLine) {
        let _ = (router, current_path);
    }

    /// The current hyperedge endpoints for hyperedge rerouting.
    // upstream: debughandler.h DebugHandler::beginningHyperedgeReroutingWithEndpoints()
    fn beginning_hyperedge_rerouting_with_endpoints(&mut self, router: &Router, endpoints: &[VertIdx]) {
        let _ = (router, endpoints);
    }

    /// The Minimum Terminal Spanning Tree for hyperedge rerouting is being
    /// grown with the edge between vertices `u` and `v`.  `should_wait`:
    /// the forest is being grown with this edge, or otherwise being
    /// immediately repopulated after pruning.
    // upstream: debughandler.h DebugHandler::mtstGrowForestWithEdge()
    fn mtst_grow_forest_with_edge(&mut self, router: &Router, u: VertIdx, v: VertIdx, should_wait: bool) {
        let _ = (router, u, v, should_wait);
    }

    /// The Minimum Terminal Spanning Tree for hyperedge rerouting is
    /// potentially bridged by the edge between vertices `u` and `v`.
    // upstream: debughandler.h DebugHandler::mtstPotentialBridgingEdge()
    fn mtst_potential_bridging_edge(&mut self, router: &Router, u: VertIdx, v: VertIdx) {
        let _ = (router, u, v);
    }

    /// The Minimum Terminal Spanning Tree for hyperedge rerouting is being
    /// finalised with the edge between vertices `u` and `v`.  `is_bridge`:
    /// whether the edge was a bridge.
    // upstream: debughandler.h DebugHandler::mtstCommitToEdge()
    fn mtst_commit_to_edge(&mut self, router: &Router, u: VertIdx, v: VertIdx, is_bridge: bool) {
        let _ = (router, u, v, is_bridge);
    }
}

impl Router {
    /// Calls `f` with the registered debug handler, if any (upstream
    /// `if (router->debugHandler()) router->debugHandler()->...`).  The
    /// handler is taken out of the router during the call.
    #[cfg_attr(not(feature = "debug-handler"), allow(dead_code))]
    pub(crate) fn call_debug_handler(&mut self, f: impl FnOnce(&mut dyn DebugHandler, &Router)) {
        if let Some(mut handler) = self.m_debug_handler.take() {
            f(handler.as_mut(), self);
            if self.m_debug_handler.is_none() {
                self.m_debug_handler = Some(handler);
            }
        }
    }

    /// Whether a debug handler is registered (upstream
    /// `router->debugHandler() != nullptr`).
    #[cfg_attr(not(feature = "debug-handler"), allow(dead_code))]
    #[inline]
    pub(crate) fn has_debug_handler(&self) -> bool {
        self.m_debug_handler.is_some()
    }

    /// The obstacle boxes upstream reports to `updateObstacleBoxes()`:
    /// the routing boxes of all obstacles except junctions that are free
    /// to move (those are not treated as obstacles).
    #[cfg_attr(not(feature = "debug-handler"), allow(dead_code))]
    pub(crate) fn debug_obstacle_boxes(&self) -> Vec<Box> {
        let mut obstacle_boxes = Vec::new();
        for &obstacle in self.m_obstacles.iter() {
            if let Some(junction) = crate::avoid::obstacle::Obstacle::as_junction(self, obstacle) {
                if !junction.position_fixed(self) {
                    // Junctions that are free to move are not treated as obstacles.
                    continue;
                }
            }
            obstacle_boxes.push(crate::avoid::obstacle::Obstacle::routing_box(self, obstacle));
        }
        obstacle_boxes
    }
}
