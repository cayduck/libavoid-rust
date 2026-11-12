//! Faithful port of upstream `libavoid/router.{h,cpp}`.
//!
//! (Debug/SVG output — `outputInstanceToSVG`, `outputDiagramSVG`, timers,
//! the `DebugHandler` — is not ported.  `outputDiagramText` returns a
//! `String`.)
//!
//! # Data model (binding for the whole `crate::avoid` tree)
//!
//! ## Handles and arenas
//! Every upstream heap object that is referred to by pointer lives in an
//! [`Arena`](crate::avoid::arena::Arena) owned by the `Router` and is named
//! by a small `Copy` handle (`crate::avoid::arena`):
//!
//! | upstream          | handle         | arena field                  |
//! |-------------------|----------------|------------------------------|
//! | `VertInf *`       | `VertIdx`      | `Router::vert_infs`          |
//! | `EdgeInf *`       | `EdgeIdx`      | `Router::edge_infs`          |
//! | `VertInf **` (MTST tree-root cell) | `TreeRootPtr` | `Router::tree_root_cells` |
//! | `bool *` reroute flag | `RerouteFlagIdx` | `Router::m_conn_reroute_flags` |
//! | `Obstacle *` (`ShapeRef *`, `JunctionRef *`) | `ObstacleIdx` (`ShapeRef`, `JunctionRef` wrappers) | `Router::obstacles` |
//! | `ConnRef *`       | `ConnIdx`      | `Router::conns`              |
//! | `ShapeConnectionPin *` | `PinIdx`  | `Router::pins`               |
//! | `ClusterRef *`    | `ClusterIdx`   | `Router::clusters`           |
//! | `ConnEnd *` (owned by a `ConnRef`) | `ConnEndRef` | inline in `ConnRef` |
//! | `ANode *`         | `ANodeIdx`     | added by the makepath port   |
//!
//! `new T(...)` becomes an associated constructor taking `&mut Router` and
//! returning the handle (`VertInf::new(router, id, pt, true)`,
//! `ShapeRef::new(router, &poly, 0)`); `delete p` becomes
//! `T::delete(router, h)`, which runs the upstream destructor logic and
//! frees the slot.  A null pointer is `Option<Handle>::None`.
//!
//! To add a new object kind: define its handle with
//! `avoid_define_handle!` in `arena.rs`, add an `Arena<Handle, T>` field
//! here, give `T` a `serial: u64` filled from [`Router::next_serial`] if
//! anything upstream orders/compares those pointers, and write
//! `T::new(router, ..) -> Handle` / `T::delete(router, h)`.
//!
//! ## Ordering
//! Arena slots are reused (free list), so **never** let a handle's numeric
//! value influence behaviour.  Where upstream orders by pointer value
//! (`std::set<T*>`, `std::map<T*,..>`, `ptr1 < ptr2`, `std::min` of
//! pointers) compare the objects' creation `serial`s instead (e.g. with
//! [`PtrKey`](crate::avoid::arena::PtrKey)) and mark the site
//! `// PTR-ORDER:`.  Do not iterate `HashMap`/`HashSet`s keyed by handles
//! where order could leak; use `BTreeMap` keyed by serial (or a `Vec` in
//! insertion order for `std::list`s).
//!
//! ## Intrusive lists
//! * `VertInfList` (`Router::vertices`) and `EdgeList`
//!   (`vis_graph`/`invis_graph`/`vis_orthog_graph`) keep their links inside
//!   the objects (`lst_prev`/`lst_next`) exactly as upstream; the list
//!   heads hold the arena-independent bookkeeping and their methods take
//!   the arena explicitly: `router.vertices.add_vertex(&mut
//!   router.vert_infs, v)`.  Iterate as upstream:
//!   `let mut k = router.vertices.conns_begin(); while let Some(v) = k { ..; k = router.vert_infs[v].lst_next; }`.
//! * Per-vertex `EdgeInfList`s (`vis_list`, `orthog_vis_list`,
//!   `invis_list`) are intrusive lists threaded through the edges' two link
//!   slots (see `vertices::EdgeInfList`); they support `iter`, cursor
//!   traversal (`first`/`EdgeInfList::next`), and upstream-identical
//!   `std::list::sort` (`VertInf::sort_edge_list`).
//! * Plain `std::list<T>`s that are only appended to / erased from /
//!   iterated become `Vec<T>` in insertion order (`m_obstacles`,
//!   `conn_refs`, `cluster_refs`, `action_list`).
//!
//! ## Borrow pattern
//! * Methods touching a single object take `&self`/`&mut self`
//!   (`router.edge_infs[e].is_disabled()`).
//! * Methods that read other objects take the arena(s) they need
//!   (`edge.is_orthogonal(&router.vert_infs)`) or `&Router`.
//! * Methods that mutate shared graph state are associated functions taking
//!   `&mut Router` plus handles: `EdgeInf::set_dist(router, e, d)`,
//!   `VertInf::remove_from_graph(router, v, true)`.  Inside them, split
//!   borrows by destructuring (`let Router { vert_infs, edge_infs, .. } =
//!   router;`) rather than cloning.
//! * Upstream method names are kept in snake_case, and every ported
//!   function carries `// upstream: file.cpp Class::method()`.
//! * Public upstream API methods are additionally available as methods on
//!   the handles (`ShapeRef`, `JunctionRef`, `ConnIdx`, `PinIdx`,
//!   `ClusterIdx`, `ObstacleIdx`), taking the router as first argument.
//!
//! ## Virtual hooks
//! * `Router::shouldContinueTransactionWithProgress()` (virtual upstream)
//!   is overridable through [`Router::m_progress_callback`].
//! * `TopologyAddonInterface` is the trait [`TopologyAddonInterface`].
//!
//! ## Dangling handles
//! Arena handles are generation-checked in all builds (`arena.rs`): a
//! handle to a freed object never aliases a later object in the same slot;
//! `Arena::contains`/`get` report it dead and indexing with it panics.
//! Upstream pointers that may legitimately outlive their object are read
//! through liveness checks; everything else indexes directly, so a bug
//! that dereferences a dead handle stops deterministically instead of
//! silently reading another object.
//!
//! ### Stale `pathNext`
//! `VertInf::pathNext` is written by every A* search (the found path) and
//! every MTST construction (the shortest-path forest) and is never cleared
//! when the vertex it points at is deleted (orthogonal visibility graph
//! regeneration, MTST dummy vertices, deleted connector/checkpoint
//! vertices).  Upstream's only *intended* reader of a pre-existing
//! `pathNext` is the start of `AStarPathPrivate::search`
//! (`if (start->pathNext)`): during checkpoint routing the previous
//! segment's search has just set it, so the next segment prefers to leave
//! the checkpoint collinearly.  For any other search the value is a
//! left-over of an unrelated earlier search; upstream still honours it
//! (defined, deterministic behaviour while the target vertex is alive,
//! which this port reproduces exactly) but dereferences freed memory when
//! the target was deleted.  Policy: a `pathNext` link to a deleted vertex
//! reads as null ([`VertInf::live_path_next`]), at the search start, in
//! the rubber-band unwind walk and in `generatePath`'s path check.  This
//! is bit-identical with upstream whenever upstream's behaviour is defined
//! and never touches freed or reused slots.  The hyperedge oracle
//! (`tools/oracle/build_hyperedge_oracle.sh`) patches exactly this rule into
//! its copy of upstream, so the scenes that hit the use-after-free are
//! compared byte for byte instead of being dropped.
//!
//! ## Public-API misuse
//! The release WASM build aborts on panic, so input that upstream leaves
//! undefined (or crashes on) must not panic here either.  Where upstream's
//! behaviour is defined the port reproduces it; elsewhere it does the
//! following (sites are marked `MISUSE`; exercised by
//! `tests/robustness.rs` in all routing/transaction modes):
//! * Handles of freed objects: `Router` delete/move calls and handle
//!   methods are no-ops (getters return empty/default values).  Deleting
//!   twice is a no-op.
//! * A shape/junction deleted in the transaction that created it: the
//!   queued add is cancelled and the object is freed by
//!   `processActions()`.  Moving an object queued for deletion, adding one
//!   already in the scene, or moving a shape to a polygon with a different
//!   number of points is ignored.
//! * `ConnEnd`s (copies in user code, queued actions, hyperedge terminal
//!   lists) whose shape/junction was freed become free point ends at the
//!   position stored when they were made; references to freed pins are
//!   dropped (`ConnEnd::forget_dead_refs`).
//! * Non-finite coordinates: a shape with them, or with an empty polygon,
//!   is *inert* (never added to the scene, moves ignored, deletable);
//!   moves and endpoints with them are ignored; such checkpoints are
//!   dropped; a junction gets 0 for the non-finite components.
//! * Transactions off: a `processTransaction()` nested inside
//!   `processActions()` is a no-op (upstream recurses forever when moving
//!   an obstacle with attached connectors, and dereferences null when
//!   deleting one with pins); the queued action is handled by the running
//!   transaction.
//! * Hyperedges: a registered hyperedge whose MTST has no junction (fewer
//!   than three distinct terminals, unreachable terminals) is skipped; one
//!   that contains a cycle (upstream recurses forever) or a connector
//!   without an endpoint vertex (upstream dereferences null) is treated
//!   as invalid; a freed root junction is ignored; one whose MTST tree
//!   contains a cycle is skipped (upstream's recursive `addConns()`
//!   overflows the stack).
//! * `splitAtSegment(0)` is a no-op; splitting a connector whose
//!   destination is a free point uses a point end for the new connector.
//! * `removeJunctionAndMergeConnectors()` on a junction holding both ends
//!   of one connector is a no-op.
//! * Rubber-band routing whose current route has points without a graph
//!   vertex (e.g. after `setFixedRoute`) searches afresh from the source
//!   (upstream dereferences null).
//! * A VPSC failure during nudging (upstream throws an uncaught exception)
//!   leaves that nudging region unchanged.
//!
//! Upstream undefined behaviour reachable from *valid* input, found with
//! AddressSanitizer by `tools/oracle/audit_scenes.sh` (those scenes have no
//! golden; `tests/audit_scenes.rs` only checks the port runs them): zero
//! extent obstacles leave a deleted `Node` in the channel-info scanline
//! (use-after-free; the port keeps the node in its arena until the sweep
//! ends, the bump-allocator reading), `markAllObstaclesAsMoved()` after a
//! `deleteShape()` in the same transaction deactivates the shape twice
//! (the port ignores the move), cluster-crossing counting in A* overflows
//! a heap buffer, and rubber-band routing dereferences a null start vertex
//! (the port runs all of these without panicking).  A connector with both
//! ends on one junction makes `HyperedgeImprover::execute()` (or
//! `removeZeroLengthEdges()`) erase the `std::set` element it is iterating;
//! upstream's `++curr` then reads the freed tree node (ASan misses it: the
//! increment is in uninstrumented libstdc++; the e2e fuzzer's ASan build
//! detects it, "iterator-erased"); the port continues with the next root.
//!
//! ## Asserts
//! `COLA_ASSERT` becomes `debug_assert!`, with one exception: an assert
//! that upstream itself violates on valid input (or whose violation
//! upstream's release build tolerates for accepted API input) is not
//! ported, so that debug builds never panic where upstream's release build
//! works.  Each such site carries a comment `Upstream: COLA_ASSERT(..).
//! Not ported (assert policy)` with the evidence (the nudging oracle's 29
//! `#assert` scenes run in debug builds too).  Asserts on API misuse are
//! made unreachable by the `MISUSE` handling above.

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
#![allow(clippy::new_ret_no_self, clippy::too_many_arguments)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::avoid::actioninfo::{find_action, ActionInfo, ActionInfoList, ActionObj, ActionType};
use crate::avoid::arena::{
    Arena, ClusterIdx, ConnIdx, ObstacleIdx, PinIdx, PtrKey, RerouteFlagIdx, TreeRootPtr, VertIdx,
};
use crate::avoid::connectionpin::{ConnectionPinIds, PinArena};
use crate::avoid::connector::{
    ConnRef, ConnType, ConnectorCrossings, CROSSING_SHARES_FIXED_SEGMENT, CROSSING_SHARES_PATH,
    CROSSING_SHARES_PATH_AT_END, CROSSING_TOUCHES,
};
use crate::avoid::connend::ConnEnd;
use crate::avoid::geometry::{euclidean_dist, in_poly, in_poly_gen, manhattan_dist, segment_shape_intersect};
use crate::avoid::geomtypes::{cpp_max, cpp_min, Point, Polygon, PolygonInterface, PolygonLookup};
use crate::avoid::graph::{EdgeInf, EdgeList, EdgeListId};
use crate::avoid::hyperedge::{HyperedgeNewAndDeletedObjectLists, HyperedgeRerouter};
use crate::avoid::hyperedgeimprover::HyperedgeImprover;
use crate::avoid::junction::JunctionRef;
use crate::avoid::obstacle::{Obstacle, ObstacleArena};
use crate::avoid::orthogonal::{generate_static_orthogonal_vis_graph, improve_orthogonal_routes};
use crate::avoid::shape::ShapeRef;
use crate::avoid::vertices::{
    libstdcxx_list_sort, ContainsMap, EdgeArena, VertArena, VertInf, VertInfList, DUMMY_ORTHOG_ID,
};
use crate::avoid::viscluster::{ClusterArena, ClusterRef};
use crate::avoid::visibility::{compute_visibility_naive, compute_visibility_sweep};

thread_local! {
    static E2E_REROUTE_COUNT: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Test hook (not upstream): number of `rerouteAndCallbackConnectors()`
/// runs on this thread.  Used by the end-to-end fuzzer
/// (`tests/e2e_fuzz_dsl`), whose C++ driver counts the same calls in a
/// patched copy of router.cpp, to know when the hyperedge rerouter /
/// improver object lists have been regenerated.
pub fn e2e_reroute_count() -> u64 {
    E2E_REROUTE_COUNT.with(|c| c.get())
}

/// Arena of all connectors of a router.
pub type ConnArena = Arena<ConnIdx, ConnRef>;

// upstream: router.h enum RouterFlag
pub const POLY_LINE_ROUTING: u32 = 1;
pub const ORTHOGONAL_ROUTING: u32 = 2;

// upstream: router.h runningTo / runningFrom / runningToAndFrom
pub const RUNNING_TO: u32 = 1;
pub const RUNNING_FROM: u32 = 2;
pub const RUNNING_TO_AND_FROM: u32 = RUNNING_TO | RUNNING_FROM;

// upstream: router.h zeroParamValue / chooseSensibleParamValue
pub const ZERO_PARAM_VALUE: f64 = 0.0;
pub const CHOOSE_SENSIBLE_PARAM_VALUE: f64 = -1.0;

/// Types of routing parameters and penalties that can be used to tailor
/// the style and improve the quality of the connector routes produced.
// upstream: router.h enum RoutingParameter
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RoutingParameter {
    SegmentPenalty = 0,
    AnglePenalty = 1,
    CrossingPenalty = 2,
    ClusterCrossingPenalty = 3,
    FixedSharedPathPenalty = 4,
    PortDirectionPenalty = 5,
    ShapeBufferDistance = 6,
    IdealNudgingDistance = 7,
    ReverseDirectionPenalty = 8,
}
// upstream: router.h lastRoutingParameterMarker
pub const LAST_ROUTING_PARAMETER_MARKER: usize = 9;
// upstream: router.h typedef enum RoutingParameter PenaltyType
pub type PenaltyType = RoutingParameter;

impl RoutingParameter {
    pub const ALL: [RoutingParameter; LAST_ROUTING_PARAMETER_MARKER] = [
        RoutingParameter::SegmentPenalty,
        RoutingParameter::AnglePenalty,
        RoutingParameter::CrossingPenalty,
        RoutingParameter::ClusterCrossingPenalty,
        RoutingParameter::FixedSharedPathPenalty,
        RoutingParameter::PortDirectionPenalty,
        RoutingParameter::ShapeBufferDistance,
        RoutingParameter::IdealNudgingDistance,
        RoutingParameter::ReverseDirectionPenalty,
    ];
}

/// Types of routing options that can be enabled.
// upstream: router.h enum RoutingOption
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RoutingOption {
    NudgeOrthogonalSegmentsConnectedToShapes = 0,
    ImproveHyperedgeRoutesMovingJunctions = 1,
    PenaliseOrthogonalSharedPathsAtConnEnds = 2,
    NudgeOrthogonalTouchingColinearSegments = 3,
    PerformUnifyingNudgingPreprocessingStep = 4,
    ImproveHyperedgeRoutesMovingAddingAndDeletingJunctions = 5,
    NudgeSharedPathsWithCommonEndPoint = 6,
}
// upstream: router.h lastRoutingOptionMarker
pub const LAST_ROUTING_OPTION_MARKER: usize = 7;

impl RoutingOption {
    pub const ALL: [RoutingOption; LAST_ROUTING_OPTION_MARKER] = [
        RoutingOption::NudgeOrthogonalSegmentsConnectedToShapes,
        RoutingOption::ImproveHyperedgeRoutesMovingJunctions,
        RoutingOption::PenaliseOrthogonalSharedPathsAtConnEnds,
        RoutingOption::NudgeOrthogonalTouchingColinearSegments,
        RoutingOption::PerformUnifyingNudgingPreprocessingStep,
        RoutingOption::ImproveHyperedgeRoutesMovingAddingAndDeletingJunctions,
        RoutingOption::NudgeSharedPathsWithCommonEndPoint,
    ];
}

// upstream: router.h enum TransactionPhases
pub const TRANSACTION_PHASE_ORTHOGONAL_VISIBILITY_GRAPH_SCAN_X: u32 = 1;
pub const TRANSACTION_PHASE_ORTHOGONAL_VISIBILITY_GRAPH_SCAN_Y: u32 = 2;
pub const TRANSACTION_PHASE_ROUTE_SEARCH: u32 = 3;
pub const TRANSACTION_PHASE_CROSSING_DETECTION: u32 = 4;
pub const TRANSACTION_PHASE_REROUTE_SEARCH: u32 = 5;
pub const TRANSACTION_PHASE_ORTHOGONAL_NUDGING_X: u32 = 6;
pub const TRANSACTION_PHASE_ORTHOGONAL_NUDGING_Y: u32 = 7;
pub const TRANSACTION_PHASE_COMPLETED: u32 = 8;

// upstream: router.h struct LineRep / typedef LineReps
#[derive(Clone, Copy, Debug, Default)]
pub struct LineRep {
    pub begin: Point,
    pub end: Point,
}
pub type LineReps = Vec<LineRep>;

// upstream: router.h typedef std::list<unsigned int> IntList
pub type IntList = Vec<u32>;

/// Upstream `ConnRerouteFlagDelegate`: owns the `bool` flags that
/// `EdgeInf::m_conns` point at.  Upstream stores them in a `std::list` that
/// is only appended to, so a `Vec` index is a stable "pointer".
// upstream: router.h class ConnRerouteFlagDelegate
#[derive(Clone, Debug, Default)]
pub struct ConnRerouteFlagDelegate {
    pub m_mapping: Vec<(Option<ConnIdx>, bool)>,
}

impl ConnRerouteFlagDelegate {
    // upstream: router.cpp ConnRerouteFlagDelegate::addConn()
    pub fn add_conn(&mut self, conn: ConnIdx) -> RerouteFlagIdx {
        self.m_mapping.push((Some(conn), false));
        RerouteFlagIdx((self.m_mapping.len() - 1) as u32)
    }

    // upstream: router.cpp ConnRerouteFlagDelegate::removeConn()
    pub fn remove_conn(&mut self, conn: ConnIdx) {
        for it in self.m_mapping.iter_mut() {
            if it.0 == Some(conn) {
                it.0 = None;
            }
        }
    }

    /// `*flag` for a `bool *` handed out by `add_conn`.
    #[inline]
    pub fn flag_mut(&mut self, flag: RerouteFlagIdx) -> &mut bool {
        &mut self.m_mapping[flag.0 as usize].1
    }

    // upstream: router.cpp ConnRerouteFlagDelegate::alertConns()
    pub fn alert_conns(&mut self, conns: &mut ConnArena) {
        for it in self.m_mapping.iter_mut() {
            if let (Some(c), true) = (it.0, it.1) {
                it.1 = false;
                conns[c].m_needs_reroute_flag = true;
            }
        }
    }
}

/// Interface for a topology improvement addon (upstream
/// `TopologyAddonInterface`; the default implementation does nothing).
// upstream: router.h class TopologyAddonInterface
pub trait TopologyAddonInterface {
    /// Upstream `clone()`.
    fn clone_box(&self) -> std::boxed::Box<dyn TopologyAddonInterface>;
    /// Upstream `improveOrthogonalTopology(Router *)`.
    fn improve_orthogonal_topology(&mut self, router: &mut Router) {
        let _ = router;
    }
    /// Upstream `outputCode(FILE *)`: appends code to `out`, returns whether
    /// anything was written.
    fn output_code(&self, out: &mut String) -> bool {
        let _ = out;
        false
    }
    /// Upstream `outputDeletionCode(FILE *)`.
    fn output_deletion_code(&self, out: &mut String) -> bool {
        let _ = out;
        false
    }
}

/// The default (no-op) `TopologyAddonInterface`.
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultTopologyAddon;

impl TopologyAddonInterface for DefaultTopologyAddon {
    fn clone_box(&self) -> std::boxed::Box<dyn TopologyAddonInterface> {
        std::boxed::Box::new(*self)
    }
}

/// Progress/continuation hook (`shouldContinueTransactionWithProgress`
/// override): `(elapsed_ms, phase, total_phases, proportion) -> continue`.
pub type ProgressCallback = std::boxed::Box<dyn FnMut(u32, u32, u32, f64) -> bool>;

/// The Router class represents a libavoid router instance.
// upstream: router.h class Router
pub struct Router {
    // --- arenas (no upstream counterpart; see module docs) ---
    pub vert_infs: VertArena,
    pub edge_infs: EdgeArena,
    /// Cells allocated by `VertInf::makeTreeRootPointer()`.
    pub tree_root_cells: Arena<TreeRootPtr, Option<VertIdx>>,
    pub obstacles: ObstacleArena,
    pub conns: ConnArena,
    pub pins: PinArena,
    pub clusters: ClusterArena,
    next_serial: u64,

    // --- upstream public members ---
    /// `ObstacleList m_obstacles` (std::list, list order).
    pub m_obstacles: Vec<ObstacleIdx>,
    /// `ConnRefList connRefs` (std::list, list order).
    pub conn_refs: Vec<ConnIdx>,
    /// `ClusterRefList clusterRefs` (std::list, list order).
    pub cluster_refs: Vec<ClusterIdx>,
    pub vis_graph: EdgeList,
    pub invis_graph: EdgeList,
    pub vis_orthog_graph: EdgeList,
    pub contains: ContainsMap,
    pub vertices: VertInfList,
    pub enclosing_clusters: ContainsMap,

    pub partial_time: bool,
    pub simple_routing: bool,
    pub clustered_routing: bool,

    // Poly-line routing options:
    pub ignore_regions: bool,
    pub use_lees_algorithm: bool,
    pub invisibility_grph: bool,

    // General routing options:
    pub selective_reroute: bool,

    pub partial_feedback: bool,
    pub rubber_band_routing: bool,

    // Instrumentation:
    pub st_checked_edges: i32,

    // --- upstream private members ---
    pub action_list: ActionInfoList,
    pub m_largest_assigned_id: u32,
    pub m_consolidate_actions: bool,
    pub m_currently_calling_destructors: bool,
    pub m_routing_parameters: [f64; LAST_ROUTING_PARAMETER_MARKER],
    pub m_routing_options: [bool; LAST_ROUTING_OPTION_MARKER],

    pub m_conn_reroute_flags: ConnRerouteFlagDelegate,
    pub m_hyperedge_rerouter: HyperedgeRerouter,

    /// `clock_t m_transaction_start_time` (None on targets without a
    /// clock, e.g. wasm32-unknown-unknown; elapsed time is then 0).
    pub m_transaction_start_time: Option<TransactionClock>,
    pub m_abort_transaction: bool,

    pub m_topology_addon: std::boxed::Box<dyn TopologyAddonInterface>,
    pub m_allows_polyline_routing: bool,
    pub m_allows_orthogonal_routing: bool,

    pub m_static_orthogonal_graph_invalidated: bool,
    pub m_in_crossing_rerouting_stage: bool,
    pub m_settings_changes: bool,

    pub m_hyperedge_improver: HyperedgeImprover,
    // (m_debug_handler: DebugHandler is debugging-only and not ported.)

    // --- port additions ---
    /// Connectors whose callback was performed, in call order, during the
    /// most recent `rerouteAndCallbackConnectors()` (see connector.rs).
    pub m_callback_log: Vec<ConnIdx>,
    /// Override of the virtual `shouldContinueTransactionWithProgress()`.
    pub m_progress_callback: Option<ProgressCallback>,
    /// True while `processActions()` runs (port addition; see
    /// `process_transaction`).
    pub m_in_process_actions: bool,
    /// Nesting depth of `processTransaction()` reroutes (port addition;
    /// see `process_transaction`).
    pub m_transaction_reroute_depth: u32,
}

/// Transaction clock (stand-in for `clock()`).
#[cfg(not(target_arch = "wasm32"))]
pub type TransactionClock = std::time::Instant;
#[cfg(target_arch = "wasm32")]
pub type TransactionClock = ();

#[cfg(not(target_arch = "wasm32"))]
fn clock_now() -> Option<TransactionClock> {
    Some(std::time::Instant::now())
}
#[cfg(target_arch = "wasm32")]
fn clock_now() -> Option<TransactionClock> {
    None
}
#[cfg(not(target_arch = "wasm32"))]
fn elapsed_msec(start: &Option<TransactionClock>) -> u32 {
    start.map(|s| s.elapsed().as_millis() as u32).unwrap_or(0)
}
#[cfg(target_arch = "wasm32")]
fn elapsed_msec(_start: &Option<TransactionClock>) -> u32 {
    0
}

impl fmt::Debug for Router {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Router")
            .field("vertices", &self.vert_infs.len())
            .field("edges", &self.edge_infs.len())
            .field("m_obstacles", &self.m_obstacles)
            .field("conn_refs", &self.conn_refs)
            .field("cluster_refs", &self.cluster_refs)
            .field("action_list_len", &self.action_list.len())
            .finish_non_exhaustive()
    }
}

impl PolygonLookup for Router {
    fn referenced_polygon(&self, obstacle: ObstacleIdx) -> &Polygon {
        &self.obstacles[obstacle].m_polygon
    }
}

/// C `printf("%.<precision>g", v)` (glibc formatting).
pub fn format_g(v: f64, precision: usize) -> String {
    if v.is_nan() {
        return if v.is_sign_negative() { "-nan".into() } else { "nan".into() };
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf".into() } else { "-inf".into() };
    }
    let p = if precision == 0 { 1 } else { precision } as i32;
    if v == 0.0 {
        return if v.is_sign_negative() { "-0".into() } else { "0".into() };
    }
    let e = format!("{:.*e}", (p - 1) as usize, v);
    let (mant, exp) = e.split_once('e').unwrap();
    let x: i32 = exp.parse().unwrap();
    fn strip_zeros(s: &str) -> String {
        if s.contains('.') {
            let t = s.trim_end_matches('0');
            t.trim_end_matches('.').to_string()
        } else {
            s.to_string()
        }
    }
    if x < p && x >= -4 {
        let s = format!("{:.*}", (p - 1 - x) as usize, v);
        strip_zeros(&s)
    } else {
        let m = strip_zeros(mant);
        let sign = if x < 0 { '-' } else { '+' };
        format!("{}e{}{:02}", m, sign, x.abs())
    }
}

// upstream: router.cpp typedef std::pair<double, ConnRef *> ConnCostRef
type ConnCostRef = (f64, Option<ConnIdx>);

/// `std::set<ConnCostRef, CmpConnCostRef>` (ordered by connector id; an
/// insert of an already-present id is ignored).
// upstream: router.cpp CmpConnCostRef / typedef ConnCostRefSet
type ConnCostRefSet = BTreeMap<u32, (f64, ConnIdx)>;
type ConnCostRefSetList = Vec<ConnCostRefSet>;

// upstream: router.cpp typedef std::map<ConnRef *, std::set<ConnRef *> > CrossingConnectorsMap
// PTR-ORDER: keyed and ordered by connector creation serial.
type CrossingConnectorsMap = BTreeMap<PtrKey<ConnIdx>, BTreeSet<PtrKey<ConnIdx>>>;

// upstream: router.cpp cheapEstimatedCost() (static)
fn cheap_estimated_cost(router: &mut Router, line_ref: ConnIdx) -> f64 {
    // We use an estimate of overall connector length, reduced by a count
    // of the number of segments.  In the case of equal length, This causes
    // straight line connectors to be left alone and connectors with more
    // complex paths to be rerouted.
    let is_poly_line = router.conns[line_ref].routing_type() == ConnType::ConnTypePolyLine;
    let route = router.conns[line_ref].display_route();
    let mut length = 0.0;

    for i in 1..route.size() {
        let a = &route.ps[i - 1];
        let b = &route.ps[i];

        let segment_length = if is_poly_line {
            euclidean_dist(a, b)
        } else {
            manhattan_dist(a, b)
        };
        length += segment_length;
    }
    length - (route.size() + 1) as f64
}

/// This class maintains the list of connector crossing maps and provides
/// methods for adding crossing connector pairs and getting the minimal
/// sets of connectors that can be removed from each group to prevent
/// crossings.
// upstream: router.cpp class CrossingConnectorsInfo
#[derive(Default)]
struct CrossingConnectorsInfo {
    /// `CrossingConnectorsMapList` (std::list).
    pairs_set_list: Vec<CrossingConnectorsMap>,
}

impl CrossingConnectorsInfo {
    fn key(router: &Router, c: ConnIdx) -> PtrKey<ConnIdx> {
        PtrKey::new(router.conns[c].serial, c)
    }

    // upstream: router.cpp CrossingConnectorsInfo::addCrossing()
    fn add_crossing(&mut self, router: &Router, conn1: ConnIdx, conn2: ConnIdx) {
        // Find the existing or new group that this crossing
        // should be recorded in.
        let it = self.group_for_crossing_conns(router, conn1, conn2);
        let pairs_set = &mut self.pairs_set_list[it];

        // Record the crossing.
        let (k1, k2) = (Self::key(router, conn1), Self::key(router, conn2));
        pairs_set.entry(k1).or_default().insert(k2);
        pairs_set.entry(k2).or_default().insert(k1);
    }

    // upstream: router.cpp CrossingConnectorsInfo::crossingSetsListToRemoveCrossingsFromGroups()
    fn crossing_sets_list_to_remove_crossings_from_groups(&mut self, router: &mut Router) -> ConnCostRefSetList {
        // A list of the minimal sets of connectors that cause crossings
        // in all groups of crossingconnectors.
        let mut crossing_sets_list = ConnCostRefSetList::new();

        // For each group of crossing connectors.
        for gi in 0..self.pairs_set_list.len() {
            // The minimal set of crossing-causing connectors.
            // We will build this.
            let mut crossing_set = ConnCostRefSet::new();

            // Set of exclusive pins that the crossing-causing connectors
            // attach to.
            let mut exclusive_pins: BTreeSet<ConnectionPinIds> = BTreeSet::new();

            // For each crossing-causing connector.
            loop {
                let crossing_causing_connector =
                    Self::remove_connector_with_most_crossings(router, &mut self.pairs_set_list[gi]);
                let conn = match crossing_causing_connector.1 {
                    None => break,
                    Some(c) => c,
                };
                // Add it to our crossing-causing set.
                crossing_set
                    .entry(router.conns[conn].m_id)
                    .or_insert((crossing_causing_connector.0, conn));

                // Determine if it attaches to any exclusive pins and
                // record these.
                let ends = router.conns[conn].endpoint_conn_ends(router);
                // Look at one end.
                if let Some(pin) = ends.0.m_active_pin {
                    if router.pins[pin].is_exclusive() {
                        exclusive_pins.insert(router.pins[pin].ids(&router.obstacles));
                    }
                }
                // Look at other end.
                if let Some(pin) = ends.1.m_active_pin {
                    if router.pins[pin].is_exclusive() {
                        exclusive_pins.insert(router.pins[pin].ids(&router.obstacles));
                    }
                }
            }

            // For each of the remaining connectors which are not
            // crossing-causing, add them to our crossing set if they
            // attach to one of the exclusive pin classes which the
            // crossing-causing connectors attached to.
            let keys: Vec<ConnIdx> = self.pairs_set_list[gi].keys().map(|k| k.idx).collect();
            for conn in keys {
                let ends = router.conns[conn].endpoint_conn_ends(router);
                // Look at one end.
                if let Some(pin) = ends.0.m_active_pin {
                    if router.pins[pin].is_exclusive()
                        && exclusive_pins.contains(&router.pins[pin].ids(&router.obstacles))
                    {
                        // Attaches to an exclusive pin and it matches
                        // one attached to by the crossing-causing
                        // connectors.  So add the conn to the
                        // crossing set and continue..
                        crossing_set.entry(router.conns[conn].m_id).or_insert((0.0, conn));
                        continue;
                    }
                }
                // Look at other end.
                if let Some(pin) = ends.1.m_active_pin {
                    if router.pins[pin].is_exclusive()
                        && exclusive_pins.contains(&router.pins[pin].ids(&router.obstacles))
                    {
                        crossing_set.entry(router.conns[conn].m_id).or_insert((0.0, conn));
                    }
                }
            }

            if !crossing_set.is_empty() {
                crossing_sets_list.push(crossing_set);
            }
        }
        crossing_sets_list
    }

    // upstream: router.cpp CrossingConnectorsInfo::connsKnownToCross()
    fn conns_known_to_cross(&self, router: &Router, conn1: ConnIdx, conn2: ConnIdx) -> bool {
        let it1 = self.group_for_conn(router, conn1);
        let it2 = self.group_for_conn(router, conn2);

        // The connectors cross if
        if (it1 == it2) && it1.is_some() {
            // They are in the same group and conn2 is in crossing
            // connectors set of conn1.
            let pairs_set = &self.pairs_set_list[it1.unwrap()];
            let k1 = Self::key(router, conn1);
            let k2 = Self::key(router, conn2);
            return pairs_set.get(&k1).is_some_and(|s| s.contains(&k2));
        }
        false
    }

    // upstream: router.cpp CrossingConnectorsInfo::removeConnectorWithMostCrossings()
    fn remove_connector_with_most_crossings(
        router: &mut Router,
        pairs_set: &mut CrossingConnectorsMap,
    ) -> ConnCostRef {
        // Tracking of the greatest number of crossings.
        let mut candidate_connector: Option<PtrKey<ConnIdx>> = None;
        let mut candidate_crossing_count: usize = 0;
        let mut candidate_estimated_cost = 0.0;

        // For each connector in the group...
        for (k, v) in pairs_set.iter() {
            // ... check if it has any crossings.
            let crossings = v.len();
            if crossings == 0 {
                // If not, skip.
                continue;
            }

            // It has crossings.  Determine if it has the most crossings
            // of the connectors we've seen, or if it is tied but has
            // a greater estimated cost.  If so, it is our new candidate.
            let cost = cheap_estimated_cost(router, k.idx);
            if (crossings > candidate_crossing_count)
                || ((crossings == candidate_crossing_count) && (cost > candidate_estimated_cost))
            {
                // Update with new candidate.
                candidate_connector = Some(*k);
                candidate_crossing_count = crossings;
                candidate_estimated_cost = cost;
            }
        }

        let candidate = match candidate_connector {
            // If no candidate, return nullptr connector.
            None => return (0.0, None),
            Some(c) => c,
        };

        // Remove the candidate from the group.  To do this we find the
        // set of all connectors it crosses.
        let conn_set: Vec<PtrKey<ConnIdx>> = pairs_set.entry(candidate).or_default().iter().copied().collect();
        // For each of these
        for it in conn_set {
            // we remove the candidate from their crossing lists
            pairs_set.entry(it).or_default().remove(&candidate);
        }
        // And then clear the crossing list for the candidate itself.
        pairs_set.entry(candidate).or_default().clear();

        // Return the candidate connector and its original crossing count.
        (candidate_crossing_count as f64, Some(candidate.idx))
    }

    // upstream: router.cpp CrossingConnectorsInfo::groupForConn()
    fn group_for_conn(&self, router: &Router, conn: ConnIdx) -> Option<usize> {
        let k = Self::key(router, conn);
        // For each group...
        self.pairs_set_list.iter().position(|g| g.contains_key(&k))
    }

    // upstream: router.cpp CrossingConnectorsInfo::groupForCrossingConns()
    fn group_for_crossing_conns(&mut self, router: &Router, conn1: ConnIdx, conn2: ConnIdx) -> usize {
        let it1 = self.group_for_conn(router, conn1);
        let it2 = self.group_for_conn(router, conn2);

        match (it1, it2) {
            (None, None) => {
                // Neither are part of a group.  Create one.
                self.pairs_set_list.push(CrossingConnectorsMap::new());
                self.pairs_set_list.len() - 1
            }
            // it1 is the appropriate existing group.
            (Some(i1), None) => i1,
            // it2 is the appropriate exisitng group.
            (None, Some(i2)) => i2,
            (Some(i1), Some(i2)) if i1 != i2 => {
                // There are two different existing groups, so merge them.
                let g2 = self.pairs_set_list[i2].clone();
                for (k, v) in g2 {
                    // std::map::insert(range) keeps existing keys' values.
                    self.pairs_set_list[i1].entry(k).or_insert(v);
                }
                self.pairs_set_list.remove(i2);
                if i2 < i1 {
                    i1 - 1
                } else {
                    i1
                }
            }
            // These are already part of the same group.  Do nothing.
            (Some(i1), Some(_)) => i1,
        }
    }
}

impl Router {
    /// Constructor for router instance.  `flags` is a combination of
    /// `POLY_LINE_ROUTING` / `ORTHOGONAL_ROUTING`.
    // upstream: router.cpp Router::Router()
    pub fn new(flags: u32) -> Router {
        // At least one of the Routing modes must be set.
        debug_assert!(flags & (POLY_LINE_ROUTING | ORTHOGONAL_ROUTING) != 0);

        let mut m_routing_parameters = [0.0; LAST_ROUTING_PARAMETER_MARKER];
        m_routing_parameters[RoutingParameter::SegmentPenalty as usize] = 10.0;
        m_routing_parameters[RoutingParameter::ClusterCrossingPenalty as usize] = 4000.0;
        m_routing_parameters[RoutingParameter::IdealNudgingDistance as usize] = 4.0;

        let mut m_routing_options = [false; LAST_ROUTING_OPTION_MARKER];
        m_routing_options[RoutingOption::NudgeOrthogonalSegmentsConnectedToShapes as usize] = false;
        m_routing_options[RoutingOption::ImproveHyperedgeRoutesMovingJunctions as usize] = true;
        m_routing_options[RoutingOption::PenaliseOrthogonalSharedPathsAtConnEnds as usize] = false;
        m_routing_options[RoutingOption::NudgeOrthogonalTouchingColinearSegments as usize] = false;
        m_routing_options[RoutingOption::PerformUnifyingNudgingPreprocessingStep as usize] = true;
        m_routing_options[RoutingOption::ImproveHyperedgeRoutesMovingAddingAndDeletingJunctions as usize] =
            false;
        m_routing_options[RoutingOption::NudgeSharedPathsWithCommonEndPoint as usize] = true;

        // (m_hyperedge_improver.setRouter(this) / m_hyperedge_rerouter
        // .setRouter(this): the router is passed explicitly here.)
        Router {
            vert_infs: Arena::new(),
            edge_infs: Arena::new(),
            tree_root_cells: Arena::new(),
            obstacles: Arena::new(),
            conns: Arena::new(),
            pins: Arena::new(),
            clusters: Arena::new(),
            next_serial: 0,
            m_obstacles: Vec::new(),
            conn_refs: Vec::new(),
            cluster_refs: Vec::new(),
            vis_graph: EdgeList::new(false),
            invis_graph: EdgeList::new(false),
            // Upstream default-constructs visOrthogGraph (orthogonal=false).
            vis_orthog_graph: EdgeList::new(false),
            contains: ContainsMap::new(),
            vertices: VertInfList::new(),
            enclosing_clusters: ContainsMap::new(),
            partial_time: false,
            simple_routing: false,
            clustered_routing: true,
            // Poly-line algorithm options:
            ignore_regions: true,
            use_lees_algorithm: true,
            invisibility_grph: true,
            // General algorithm options:
            selective_reroute: true,
            partial_feedback: false,
            rubber_band_routing: false,
            // Instrumentation:
            st_checked_edges: 0,
            action_list: Vec::new(),
            m_largest_assigned_id: 0,
            m_consolidate_actions: true,
            m_currently_calling_destructors: false,
            m_routing_parameters,
            m_routing_options,
            m_conn_reroute_flags: ConnRerouteFlagDelegate::default(),
            m_hyperedge_rerouter: HyperedgeRerouter::new(),
            m_transaction_start_time: None,
            m_abort_transaction: false,
            m_topology_addon: std::boxed::Box::new(DefaultTopologyAddon),
            // Mode options:
            m_allows_polyline_routing: (flags & POLY_LINE_ROUTING) != 0,
            m_allows_orthogonal_routing: (flags & ORTHOGONAL_ROUTING) != 0,
            m_static_orthogonal_graph_invalidated: true,
            m_in_crossing_rerouting_stage: false,
            m_settings_changes: false,
            m_hyperedge_improver: HyperedgeImprover::new(),
            m_callback_log: Vec::new(),
            m_progress_callback: None,
            m_in_process_actions: false,
            m_transaction_reroute_depth: 0,
        }
    }

    /// Runs the upstream destructor logic (deletes all connectors and
    /// obstacles and the orthogonal visibility graph).  Not run on `Drop`
    /// (dropping the router frees everything anyway); call explicitly where
    /// the side effects matter.
    // upstream: router.cpp Router::~Router()
    pub fn destruct(&mut self) {
        self.m_currently_calling_destructors = true;

        // Delete remaining connectors.
        while let Some(&conn) = self.conn_refs.first() {
            ConnRef::delete(self, conn);
        }

        // Remove remaining obstacles (shapes and junctions).
        while let Some(&obstacle) = self.m_obstacles.first() {
            if self.obstacles[obstacle].is_active() {
                Obstacle::remove_from_graph(self, obstacle);
                Obstacle::make_inactive(self, obstacle);
            }
            Obstacle::delete(self, obstacle);
        }
        self.m_currently_calling_destructors = false;

        // Cleanup orphaned orthogonal graph vertices.
        self.destroy_orthogonal_vis_graph();

        debug_assert!(self.m_obstacles.is_empty());
        debug_assert!(self.conn_refs.is_empty());
        debug_assert!(self.vis_graph.size() == 0);

        self.m_topology_addon = std::boxed::Box::new(DefaultTopologyAddon);
    }

    /// Next monotonic creation serial (replacement for pointer identity
    /// ordering; see `// PTR-ORDER:`).
    #[inline]
    pub fn next_serial(&mut self) -> u64 {
        let s = self.next_serial;
        self.next_serial += 1;
        s
    }

    /// One of the three global edge lists.
    #[inline]
    pub fn edge_list(&self, which: EdgeListId) -> &EdgeList {
        match which {
            EdgeListId::VisGraph => &self.vis_graph,
            EdgeListId::InvisGraph => &self.invis_graph,
            EdgeListId::VisOrthogGraph => &self.vis_orthog_graph,
        }
    }

    #[inline]
    pub fn edge_list_mut(&mut self, which: EdgeListId) -> &mut EdgeList {
        match which {
            EdgeListId::VisGraph => &mut self.vis_graph,
            EdgeListId::InvisGraph => &mut self.invis_graph,
            EdgeListId::VisOrthogGraph => &mut self.vis_orthog_graph,
        }
    }

    /// `(obstacle->id(), obstacle)` for every obstacle in `m_obstacles`
    /// order (input for `ReferencingPolygon::from_polygon`).
    pub fn obstacle_id_list(&self) -> Vec<(u32, ObstacleIdx)> {
        self.m_obstacles.iter().map(|&o| (self.obstacles[o].m_id, o)).collect()
    }

    // (upstream: router.cpp Router::setDebugHandler()/debugHandler() —
    // debugging only, not ported.)

    // upstream: router.cpp Router::shapeContainingPoint()
    pub fn shape_containing_point(&self, point: &Point) -> Option<ShapeRef> {
        // Count points on the border as being inside.
        let count_border = true;

        // Compute enclosing shapes.
        for &i in self.m_obstacles.iter() {
            if let Some(shape) = Obstacle::as_shape(self, i) {
                if in_poly(&Obstacle::routing_polygon(self, i), point, count_border) {
                    return Some(shape);
                }
            }
        }
        None
    }

    /// Upstream default for `conn_pin_move_update` is false.
    // upstream: router.cpp Router::modifyConnector(ConnRef *, unsigned int, const ConnEnd&, bool)
    pub fn modify_connector_end(&mut self, conn: ConnIdx, type_: u32, conn_end: &ConnEnd, conn_pin_move_update: bool) {
        // MISUSE (port addition): an endpoint at non-finite coordinates is
        // ignored (the connector keeps its previous end, if any).
        if !conn_end.position(self).is_finite() {
            return;
        }
        let mut mod_info = ActionInfo::new_conn(self, ActionType::ConnChange, conn);

        match find_action(&self.action_list, &mod_info) {
            None => {
                // Matching action not found, so add.
                mod_info.conns.push((type_, conn_end.clone()));
                self.action_list.push(mod_info);
            }
            Some(found) => {
                // Update the found action as necessary.
                self.action_list[found].add_conn_end_update(type_, conn_end, conn_pin_move_update);
            }
        }

        if !self.m_consolidate_actions {
            self.process_transaction();
        }
    }

    // upstream: router.cpp Router::modifyConnector(ConnRef *)
    pub fn modify_connector(&mut self, conn: ConnIdx) {
        let mod_info = ActionInfo::new_conn(self, ActionType::ConnChange, conn);

        if find_action(&self.action_list, &mod_info).is_none() {
            self.action_list.push(mod_info);
        }

        if !self.m_consolidate_actions {
            self.process_transaction();
        }
    }

    // upstream: router.cpp Router::modifyConnectionPin()
    pub fn modify_connection_pin(&mut self, pin: PinIdx) {
        let mod_info = ActionInfo::new_pin(self, ActionType::ConnectionPinChange, pin);

        if find_action(&self.action_list, &mod_info).is_none() {
            self.action_list.push(mod_info);
        }

        if !self.m_consolidate_actions {
            self.process_transaction();
        }
    }

    // upstream: router.cpp Router::removeObjectFromQueuedActions()
    pub fn remove_object_from_queued_actions(&mut self, object: ActionObj) {
        self.action_list.retain(|a| a.obj_ptr != object);
    }

    // upstream: router.cpp Router::addShape()
    pub fn add_shape(&mut self, shape: ShapeRef) {
        // MISUSE (port addition): adding a shape that is already in the
        // scene is ignored (upstream re-activates it: assert / corrupt
        // lists).
        if self.obstacles[shape.0].m_active {
            return;
        }
        // There shouldn't be remove events or move events for the same shape
        // already in the action list.
        // XXX: Possibly we could handle this by ordering them intelligently.
        debug_assert!(find_action(
            &self.action_list,
            &ActionInfo::new_shape(self, ActionType::ShapeRemove, shape)
        )
        .is_none());
        debug_assert!(find_action(
            &self.action_list,
            &ActionInfo::new_shape(self, ActionType::ShapeMove, shape)
        )
        .is_none());

        let add_info = ActionInfo::new_shape(self, ActionType::ShapeAdd, shape);

        if find_action(&self.action_list, &add_info).is_none() {
            self.action_list.push(add_info);
        }

        if !self.m_consolidate_actions {
            self.process_transaction();
        }
    }

    /// Remove a shape from the router scene (queued; the shape is freed
    /// when the transaction is processed).
    // upstream: router.cpp Router::deleteShape()
    pub fn delete_shape(&mut self, shape: ShapeRef) {
        // MISUSE: a handle of an already freed shape is ignored.
        if !self.obstacles.contains(shape.0) {
            return;
        }
        // There shouldn't be add events events for the same shape already
        // in the action list.
        // XXX: Possibly we could handle this by ordering them intelligently.
        // (Upstream asserts here and, in release builds, later erases an
        // uninitialised list iterator.  MISUSE: the queued add is
        // cancelled instead; processActions() frees the never-activated
        // shape.)
        let add = ActionInfo::new_shape(self, ActionType::ShapeAdd, shape);
        if let Some(found) = find_action(&self.action_list, &add) {
            self.action_list.remove(found);
        }

        // Delete any ShapeMove entries for this shape in the action list.
        let mv = ActionInfo::new_shape(self, ActionType::ShapeMove, shape);
        if let Some(found) = find_action(&self.action_list, &mv) {
            self.action_list.remove(found);
        }

        // Add the ShapeRemove entry.
        let rem_info = ActionInfo::new_shape(self, ActionType::ShapeRemove, shape);
        if find_action(&self.action_list, &rem_info).is_none() {
            self.action_list.push(rem_info);
        }

        if !self.m_consolidate_actions {
            self.process_transaction();
        }
    }

    /// Remove a connector from the router scene (immediately).
    // upstream: router.cpp Router::deleteConnector()
    pub fn delete_connector(&mut self, connector: ConnIdx) {
        // MISUSE: a handle of an already freed connector is ignored.
        if !self.conns.contains(connector) {
            return;
        }
        self.m_currently_calling_destructors = true;
        ConnRef::delete(self, connector);
        self.m_currently_calling_destructors = false;
    }

    /// Move or resize an existing shape by a relative offset.
    // upstream: router.cpp Router::moveShape(ShapeRef *, const double, const double)
    pub fn move_shape_by(&mut self, shape: ShapeRef, x_diff: f64, y_diff: f64) {
        // MISUSE: a handle of an already freed shape is ignored.
        if !self.obstacles.contains(shape.0) {
            return;
        }
        let move_info = ActionInfo::new_shape_move(self, ActionType::ShapeMove, shape, &Polygon::new(), false);

        let mut new_poly = match find_action(&self.action_list, &move_info) {
            // The shape already has a queued move, so use that shape position.
            Some(found) => self.action_list[found].new_poly.clone(),
            // Just use the existing position.
            None => self.obstacles[shape.0].m_polygon.clone(),
        };
        new_poly.translate(x_diff, y_diff);

        self.move_shape(shape, &new_poly, false);
    }

    // upstream: router.cpp Router::markAllObstaclesAsMoved()
    pub fn mark_all_obstacles_as_moved(&mut self) {
        let list = self.m_obstacles.clone();
        for o in list {
            if let Some(shape) = Obstacle::as_shape(self, o) {
                self.move_shape_by(shape, 0.0, 0.0);
            } else if let Some(junction) = Obstacle::as_junction(self, o) {
                self.move_junction_by(junction, 0.0, 0.0);
            }
        }
    }

    /// Move or resize an existing shape.  Upstream default for
    /// `first_move` is false.
    // upstream: router.cpp Router::moveShape(ShapeRef *, const Polygon&, const bool)
    pub fn move_shape(&mut self, shape: ShapeRef, new_poly: &Polygon, first_move: bool) {
        // MISUSE: a handle of an already freed shape is ignored, and so is
        // an inert shape (empty polygon, see ShapeRef::new).
        if self.obstacles.get(shape.0).is_none_or(|o| o.m_first_vert.is_none()) {
            return;
        }
        // MISUSE: the new polygon must have as many points as the old one
        // (upstream asserts in Obstacle::setNewPoly() and otherwise leaves
        // the shape's vertices out of step with its polygon).  Such a move
        // is ignored; delete the shape and create a new one instead.
        if self.obstacles[shape.0].m_polygon.size() != new_poly.size() {
            return;
        }
        // MISUSE: so is a move to non-finite coordinates (and any move of
        // a shape created with them, which is inert: ShapeRef::new).
        if !new_poly.ps.iter().all(|p| p.is_finite()) || !self.obstacles[shape.0].m_active && !self.action_list.iter().any(|a| a.type_ == ActionType::ShapeAdd && a.obstacle() == shape.0) {
            return;
        }
        // There shouldn't be remove events or add events for the same shape
        // already in the action list.
        // XXX: Possibly we could handle this by ordering them intelligently.
        // (Upstream asserts; in release builds the shape is moved after
        // being freed.  MISUSE: moving a shape queued for deletion is
        // ignored.)
        if find_action(&self.action_list, &ActionInfo::new_shape(self, ActionType::ShapeRemove, shape)).is_some() {
            return;
        }

        let add = ActionInfo::new_shape(self, ActionType::ShapeAdd, shape);
        if let Some(found) = find_action(&self.action_list, &add) {
            // The Add is enough, no need for the Move action too.
            // The shape will be added with it's existing polygon,
            // so set this to be the newPoly passed for the move.
            let s = self.action_list[found].shape(self).unwrap();
            Obstacle::set_new_poly(self, s.0, new_poly);
            return;
        }

        let move_info = ActionInfo::new_shape_move(self, ActionType::ShapeMove, shape, new_poly, first_move);
        // Sanely cope with the case where the user requests moving the same
        // shape multiple times before rerouting connectors.
        match find_action(&self.action_list, &move_info) {
            Some(found) => {
                // Just update the ActionInfo with the second polygon, but
                // leave the firstMove setting alone.
                self.action_list[found].new_poly = new_poly.clone();
            }
            None => {
                self.action_list.push(move_info);
            }
        }

        if !self.m_consolidate_actions {
            self.process_transaction();
        }
    }

    // upstream: router.cpp Router::setStaticGraphInvalidated()
    pub fn set_static_graph_invalidated(&mut self, invalidated: bool) {
        self.m_static_orthogonal_graph_invalidated = invalidated;
    }

    // upstream: router.cpp Router::destroyOrthogonalVisGraph()
    pub fn destroy_orthogonal_vis_graph(&mut self) {
        // Remove orthogonal visibility graph edges.
        EdgeList::clear(self, EdgeListId::VisOrthogGraph);

        // Remove the now orphaned vertices.
        let mut curr = self.vertices.shapes_begin();
        while let Some(c) = curr {
            if self.vert_infs[c].orphaned() && (self.vert_infs[c].id == DUMMY_ORTHOG_ID) {
                let following = self.vertices.remove_vertex(&mut self.vert_infs, Some(c));
                VertInf::delete(self, c);
                curr = following;
                continue;
            }
            curr = self.vert_infs[c].lst_next;
        }
    }

    // upstream: router.cpp Router::regenerateStaticBuiltGraph()
    pub fn regenerate_static_built_graph(&mut self) {
        // Here we do talks involved in updating the static-built visibility
        // graph (if necessary) before we do any routing.
        if self.m_static_orthogonal_graph_invalidated {
            if self.m_allows_orthogonal_routing {
                self.destroy_orthogonal_vis_graph();

                // Regenerate a new visibility graph.
                generate_static_orthogonal_vis_graph(self);
            }
            self.m_static_orthogonal_graph_invalidated = false;
        }
    }

    // upstream: router.cpp Router::transactionUse()
    pub fn transaction_use(&self) -> bool {
        self.m_consolidate_actions
    }

    // upstream: router.cpp Router::setTransactionUse()
    pub fn set_transaction_use(&mut self, transactions: bool) {
        self.m_consolidate_actions = transactions;
    }

    /// Processes the action list.
    // upstream: router.cpp Router::processActions()
    pub fn process_actions(&mut self) {
        let not_partial_time = !(self.partial_feedback && self.partial_time);
        let mut seen_shape_moves_or_deletes = false;

        self.m_transaction_start_time = clock_now();
        self.m_abort_transaction = false;

        self.m_in_process_actions = true;
        let mut deleted_obstacles: Vec<u32> = Vec::new();
        // actionList.sort()
        let list = std::mem::take(&mut self.action_list);
        self.action_list = libstdcxx_list_sort(list, &mut |a: &ActionInfo, b: &ActionInfo| a.less_than(b));

        // (std::list iteration up to the end() sentinel: elements appended
        // during the loop are visited too.)
        let mut curr = 0;
        while curr < self.action_list.len() {
            let act_inf = &self.action_list[curr];
            curr += 1;
            if !((act_inf.type_ == ActionType::ShapeRemove)
                || (act_inf.type_ == ActionType::ShapeMove)
                || (act_inf.type_ == ActionType::JunctionRemove)
                || (act_inf.type_ == ActionType::JunctionMove))
            {
                // Not a move or remove action, so don't do anything.
                continue;
            }
            seen_shape_moves_or_deletes = true;

            let obstacle = act_inf.obstacle();
            let shape = act_inf.shape(self);
            let junction = act_inf.junction(self);
            let is_move = (act_inf.type_ == ActionType::ShapeMove) || (act_inf.type_ == ActionType::JunctionMove);
            let first_move = act_inf.first_move;
            let new_poly = act_inf.new_poly.clone();
            let new_position = act_inf.new_position;

            let pid = self.obstacles[obstacle].m_id;

            if !self.obstacles[obstacle].m_active {
                // MISUSE (port addition): an obstacle deleted in the same
                // transaction that created it (deleteShape() cancelled its
                // queued add) was never activated: just free it.
                debug_assert!(!is_move);
                self.m_currently_calling_destructors = true;
                deleted_obstacles.push(pid);
                Obstacle::delete(self, obstacle);
                self.m_currently_calling_destructors = false;
                continue;
            }

            // o  Remove entries related to this shape's vertices
            Obstacle::remove_from_graph(self, obstacle);

            if self.selective_reroute && (!is_move || not_partial_time || first_move) {
                self.mark_polyline_connectors_needing_rerouting_for_deleted_obstacle(obstacle);
            }

            self.adjust_contains_with_del(pid as i32);

            if is_move {
                if let Some(s) = shape {
                    s.move_attached_conns(self, &new_poly);
                } else if let Some(j) = junction {
                    j.move_attached_conns(self, &new_position);
                }
            }

            // Ignore this shape for visibility.
            // XXX: We don't really need to do this if we're not using Partial
            //      Feedback.  Without this the blocked edges still route
            //      around the shape until it leaves the connector.
            Obstacle::make_inactive(self, obstacle);

            if !is_move {
                // Free deleted obstacle.
                self.m_currently_calling_destructors = true;
                deleted_obstacles.push(self.obstacles[obstacle].m_id);
                Obstacle::delete(self, obstacle);
                self.m_currently_calling_destructors = false;
            }
        }

        if seen_shape_moves_or_deletes && self.m_allows_polyline_routing {
            if self.invisibility_grph {
                // Check edges for obstacles that were moved or removed.
                let mut curr = 0;
                while curr < self.action_list.len() {
                    let act_inf = &self.action_list[curr];
                    curr += 1;
                    if (act_inf.type_ == ActionType::ShapeMove) || (act_inf.type_ == ActionType::JunctionMove) {
                        // o  Check all edges that were blocked by moved obstacle.
                        let id = self.obstacles[act_inf.obstacle()].m_id;
                        self.check_all_blocked_edges(id as i32);
                    }
                }

                for &it in deleted_obstacles.iter() {
                    // o  Check all edges that were blocked by deleted obstacle.
                    self.check_all_blocked_edges(it as i32);
                }
            } else {
                // check all edges not in graph
                self.check_all_missing_edges();
            }
        }

        let mut curr = 0;
        while curr < self.action_list.len() {
            let act_inf = &self.action_list[curr];
            curr += 1;
            if !((act_inf.type_ == ActionType::ShapeAdd)
                || (act_inf.type_ == ActionType::ShapeMove)
                || (act_inf.type_ == ActionType::JunctionAdd)
                || (act_inf.type_ == ActionType::JunctionMove))
            {
                // Not a move or add action, so don't do anything.
                continue;
            }

            let obstacle = act_inf.obstacle();
            let shape = act_inf.shape(self);
            let junction = act_inf.junction(self);
            let new_poly = act_inf.new_poly.clone();
            let new_position = act_inf.new_position;
            let is_move = (act_inf.type_ == ActionType::ShapeMove) || (act_inf.type_ == ActionType::JunctionMove);

            let pid = self.obstacles[obstacle].m_id;

            // Restore this shape for visibility.
            Obstacle::make_active(self, obstacle);

            if is_move {
                if let Some(s) = shape {
                    Obstacle::set_new_poly(self, s.0, &new_poly);
                } else {
                    junction.unwrap().set_position(self, &new_position);
                }
            }
            let shape_poly = Obstacle::routing_polygon(self, obstacle);

            self.adjust_contains_with_add(&shape_poly, pid as i32);

            if self.m_allows_polyline_routing {
                // o  Check all visibility edges to see if this one shape
                //    blocks them.
                if !is_move || not_partial_time {
                    self.new_blocking_shape(&shape_poly, pid as i32);
                }

                // o  Calculate visibility for the new vertices.
                if self.use_lees_algorithm {
                    compute_visibility_sweep(self, obstacle);
                } else {
                    compute_visibility_naive(self, obstacle);
                }
                Obstacle::update_pin_poly_line_visibility(self, obstacle);
            }
        }

        // Update connector endpoints.
        let mut curr = 0;
        while curr < self.action_list.len() {
            let act_inf = &self.action_list[curr];
            curr += 1;
            if act_inf.type_ != ActionType::ConnChange {
                continue;
            }
            let conn = act_inf.conn();
            let updates = act_inf.conns.clone();
            for (type_, conn_end) in updates.iter() {
                ConnRef::update_end_point(self, conn, *type_, conn_end);
            }
        }
        // Clear the actionList.
        self.action_list.clear();
        self.m_in_process_actions = false;
    }

    /// Finishes the current transaction and processes all the queued
    /// object changes efficiently.  Returns whether any changes were made.
    // upstream: router.cpp Router::processTransaction()
    pub fn process_transaction(&mut self) -> bool {
        // MISUSE / upstream bug (port addition): with transactions off,
        // processActions() itself queues actions whose queueing calls
        // processTransaction() again -- moving a shape or junction with
        // attached connectors (moveAttachedConns -> modifyConnector)
        // recurses until the stack overflows upstream, and deleting an
        // obstacle with connection pins (~Obstacle -> pin removal ->
        // modifyConnectionPin) re-processes the half-deleted obstacle
        // (null dereference).  A nested call is a no-op here: the action
        // just queued is appended to the list being processed and handled
        // by the running processActions() exactly as in transaction mode.
        //
        // Likewise for a call nested in the rerouting phase.  With
        // transactions off, hyperedge rerouting / improvement creates and
        // deletes junctions (JunctionRef ctor / deleteJunction ->
        // processTransaction) from inside rerouteAndCallbackConnectors();
        // upstream then runs a whole nested transaction, which regenerates
        // the orthogonal visibility graph and frees vertices the outer
        // reroute is still using, and can re-delete forever (found by the
        // e2e fuzzer: tests/golden/e2e_fuzz_regressions.txt,
        // reg-nested-reroute -- upstream never returns; the port used freed
        // handles or overflowed its stack).  MISUSE (port addition): the
        // nested call is a no-op; the queued action waits for the next
        // transaction, as in transaction mode.
        if self.m_in_process_actions || self.m_transaction_reroute_depth > 0 {
            return false;
        }
        // If SimpleRouting, then don't update here.
        if (self.action_list.is_empty()
            && (self.m_hyperedge_rerouter.count() == 0)
            && !self.m_settings_changes)
            || self.simple_routing
        {
            return false;
        }
        self.m_settings_changes = false;

        self.process_actions();

        self.m_static_orthogonal_graph_invalidated = true;
        self.m_transaction_reroute_depth += 1;
        self.reroute_and_callback_connectors();
        self.m_transaction_reroute_depth -= 1;

        true
    }

    // upstream: router.cpp Router::addJunction()
    pub fn add_junction(&mut self, junction: JunctionRef) {
        // MISUSE / upstream bug (port addition): adding a junction that is
        // already in the scene is ignored.  Upstream re-activates it; this
        // happens in ConnRef::splitAtSegment() with transactions off
        // (the JunctionRef constructor already added it).
        if self.obstacles[junction.0].m_active {
            return;
        }
        // There shouldn't be remove events or move events for the same junction
        // already in the action list.
        // XXX: Possibly we could handle this by ordering them intelligently.
        debug_assert!(find_action(
            &self.action_list,
            &ActionInfo::new_junction(self, ActionType::JunctionRemove, junction)
        )
        .is_none());
        debug_assert!(find_action(
            &self.action_list,
            &ActionInfo::new_junction(self, ActionType::JunctionMove, junction)
        )
        .is_none());

        let add_info = ActionInfo::new_junction(self, ActionType::JunctionAdd, junction);

        if find_action(&self.action_list, &add_info).is_none() {
            self.action_list.push(add_info);
        }

        if !self.m_consolidate_actions {
            self.process_transaction();
        }
    }

    /// Remove a junction from the router scene (queued).
    // upstream: router.cpp Router::deleteJunction()
    pub fn delete_junction(&mut self, junction: JunctionRef) {
        // MISUSE: a handle of an already freed junction is ignored.
        if !self.obstacles.contains(junction.0) {
            return;
        }
        // There shouldn't be add events events for the same junction already
        // in the action list.
        // XXX: Possibly we could handle this by ordering them intelligently.
        // (Upstream asserts.  MISUSE: the queued add is cancelled instead,
        // as in deleteShape().)
        let add = ActionInfo::new_junction(self, ActionType::JunctionAdd, junction);
        if let Some(found) = find_action(&self.action_list, &add) {
            self.action_list.remove(found);
        }

        // Delete any ShapeMove entries for this shape in the action list.
        let mv = ActionInfo::new_junction(self, ActionType::JunctionMove, junction);
        if let Some(found) = find_action(&self.action_list, &mv) {
            self.action_list.remove(found);
        }

        // Add the ShapeRemove entry.
        let rem_info = ActionInfo::new_junction(self, ActionType::JunctionRemove, junction);
        if find_action(&self.action_list, &rem_info).is_none() {
            self.action_list.push(rem_info);
        }

        if !self.m_consolidate_actions {
            self.process_transaction();
        }
    }

    /// Move an existing junction by a relative offset.
    // upstream: router.cpp Router::moveJunction(JunctionRef *, const double, const double)
    pub fn move_junction_by(&mut self, junction: JunctionRef, x_diff: f64, y_diff: f64) {
        // MISUSE: a handle of an already freed junction is ignored.
        if !self.obstacles.contains(junction.0) {
            return;
        }
        let move_info = ActionInfo::new_junction_move(self, ActionType::JunctionMove, junction, &Point::default());

        let mut new_position = match find_action(&self.action_list, &move_info) {
            // The junction already has a queued move, so use that position.
            Some(found) => self.action_list[found].new_position,
            // Just use the existing position.
            None => junction.position(self),
        };
        new_position.x += x_diff;
        new_position.y += y_diff;

        self.move_junction(junction, &new_position);
    }

    /// Move an existing junction to a new position.
    // upstream: router.cpp Router::moveJunction(JunctionRef *, const Point&)
    pub fn move_junction(&mut self, junction: JunctionRef, new_position: &Point) {
        // MISUSE: a handle of an already freed junction is ignored.
        if !self.obstacles.contains(junction.0) {
            return;
        }
        // There shouldn't be remove events or add events for the same junction
        // already in the action list.
        // XXX: Possibly we could handle this by ordering them intelligently.
        // (MISUSE: moving a junction queued for deletion, or to non-finite
        // coordinates, is ignored.)
        if !new_position.is_finite() {
            return;
        }
        if find_action(&self.action_list, &ActionInfo::new_junction(self, ActionType::JunctionRemove, junction))
            .is_some()
        {
            return;
        }

        let add = ActionInfo::new_junction(self, ActionType::JunctionAdd, junction);
        if let Some(found) = find_action(&self.action_list, &add) {
            // The Add is enough, no need for the Move action too.
            // The junction will be added with the new position.
            let j = self.action_list[found].junction(self).unwrap();
            j.set_position(self, new_position);
            return;
        }

        let move_info = ActionInfo::new_junction_move(self, ActionType::JunctionMove, junction, new_position);
        // Sanely cope with the case where the user requests moving the same
        // shape multiple times before rerouting connectors.
        match find_action(&self.action_list, &move_info) {
            Some(found) => {
                // Just update the ActionInfo with the second position.
                self.action_list[found].new_position = *new_position;
            }
            None => {
                self.action_list.push(move_info);
            }
        }

        if !self.m_consolidate_actions {
            self.process_transaction();
        }
    }

    // upstream: router.cpp Router::addCluster()
    pub fn add_cluster(&mut self, cluster: ClusterIdx) {
        ClusterRef::make_active(self, cluster);

        let pid = self.clusters[cluster].m_id;
        // (The ReferencingPolygon is read through a Polygon snapshot of its
        // current points; inPolyGen only uses size()/at().)
        let poly = Polygon::from_interface(&self.clusters[cluster].m_polygon.view(&*self));

        self.adjust_clusters_with_add(&poly, pid as i32);
    }

    /// Remove a cluster from the router scene.  (Like upstream, the
    /// `ClusterRef` itself is not freed.)
    // upstream: router.cpp Router::deleteCluster()
    pub fn delete_cluster(&mut self, cluster: ClusterIdx) {
        // MISUSE: deleting a cluster that is not in the scene (deleted
        // already) is ignored; upstream erases an invalid list iterator.
        if !self.clusters.get(cluster).is_some_and(|c| c.m_active) {
            return;
        }
        ClusterRef::make_inactive(self, cluster);

        let pid = self.clusters[cluster].m_id;

        self.adjust_clusters_with_del(pid as i32);
    }

    // upstream: router.cpp Router::newObjectId()
    pub fn new_object_id(&self) -> u32 {
        self.m_largest_assigned_id.wrapping_add(1)
    }

    // upstream: router.cpp Router::assignId()
    pub fn assign_id(&mut self, suggested_id: u32) -> u32 {
        // If the suggestedId is zero, then we assign the object the next
        // smallest unassigned ID, otherwise we trust the ID given is unique.
        let assigned_id = if suggested_id == 0 {
            self.new_object_id()
        } else {
            suggested_id
        };

        // If assertions are enabled, then we check that this ID really is unique.
        debug_assert!(self.object_id_is_unused(assigned_id));

        // Have the router record if this ID is larger than the largest assigned ID.
        self.m_largest_assigned_id = std::cmp::max(self.m_largest_assigned_id, assigned_id);

        assigned_id
    }

    /// Returns whether the given ID is unique among all objects known by
    /// the router.
    // upstream: router.cpp Router::objectIdIsUnused()
    pub fn object_id_is_unused(&self, id: u32) -> bool {
        // Examine shapes/junctions.
        for &i in self.m_obstacles.iter() {
            if self.obstacles[i].m_id == id {
                return false;
            }
        }

        // Examine connectors.
        for &i in self.conn_refs.iter() {
            if self.conns[i].m_id == id {
                return false;
            }
        }

        // Examine clusters.
        for &i in self.cluster_refs.iter() {
            if self.clusters[i].m_id == id {
                return false;
            }
        }

        true
    }

    /// Appends to `conns` the ids of all the connectors of type `type_`
    /// attached to the shape with the ID `shape_id`.
    // upstream: router.cpp Router::attachedConns()
    pub fn attached_conns(&self, conns: &mut IntList, shape_id: u32, type_: u32) {
        for &i in self.conn_refs.iter() {
            let anchors = self.conns[i].endpoint_anchors();

            if (type_ & RUNNING_TO) != 0 && anchors.1.is_some_and(|a| self.obstacles[a].m_id == shape_id) {
                conns.push(self.conns[i].m_id);
            } else if (type_ & RUNNING_FROM) != 0
                && anchors.0.is_some_and(|a| self.obstacles[a].m_id == shape_id)
            {
                conns.push(self.conns[i].m_id);
            }
        }
    }

    /// Appends to `shapes` the ids of all the shapes attached to the shape
    /// with the ID `shape_id` with connection type `type_`.
    // upstream: router.cpp Router::attachedShapes()
    pub fn attached_shapes(&self, shapes: &mut IntList, shape_id: u32, type_: u32) {
        for &i in self.conn_refs.iter() {
            let anchors = self.conns[i].endpoint_anchors();

            if (type_ & RUNNING_TO) != 0 && anchors.1.is_some_and(|a| self.obstacles[a].m_id == shape_id) {
                if let Some(a) = anchors.0 {
                    // Only if there is a shape attached to the other end.
                    shapes.push(self.obstacles[a].m_id);
                }
            } else if (type_ & RUNNING_FROM) != 0
                && anchors.0.is_some_and(|a| self.obstacles[a].m_id == shape_id)
            {
                if let Some(a) = anchors.1 {
                    // Only if there is a shape attached to the other end.
                    shapes.push(self.obstacles[a].m_id);
                }
            }
        }
    }

    /// It's intended this function is called after visibility changes
    /// resulting from shape movement have happened.  It will alert
    /// rerouted connectors (via a callback) that they need to be redrawn.
    // upstream: router.cpp Router::rerouteAndCallbackConnectors()
    pub fn reroute_and_callback_connectors(&mut self) {
        E2E_REROUTE_COUNT.with(|c| c.set(c.get() + 1));
        let mut rerouted_conns: Vec<ConnIdx> = Vec::new();
        self.m_callback_log.clear();

        {
            let Router {
                m_conn_reroute_flags,
                conns,
                ..
            } = self;
            m_conn_reroute_flags.alert_conns(conns);
        }

        // Updating the orthogonal visibility graph if necessary.
        self.regenerate_static_built_graph();

        let conn_list = self.conn_refs.clone();
        for &i in conn_list.iter() {
            ConnRef::free_active_pins(self, i);
        }

        // Calculate and return connectors that are part of hyperedges and will
        // be completely rerouted by that code and thus don't need to have routes
        // generated here.
        let hyperedge_conns = HyperedgeRerouter::calc_hyperedge_connectors(self);

        // TODO: It might be worth sorting connectors and routing them from
        //       smallest to largest estimated cost.  This way we likely get
        //       better exclusive pin assignment during initial routing.

        let total_conns = self.conn_refs.len();
        let mut num_of_rerouted_conns = 0usize;
        for &connector in conn_list.iter() {
            // Progress reporting and continuation check.
            self.perform_continuation_check(TRANSACTION_PHASE_ROUTE_SEARCH, num_of_rerouted_conns, total_conns);
            num_of_rerouted_conns += 1;

            let key = PtrKey::new(self.conns[connector].serial, connector);
            if hyperedge_conns.contains(&key) {
                // This will be rerouted by the hyperedge code, so do nothing.
                continue;
            }

            if self.conns[connector].has_fixed_route() {
                // We don't reroute connectors with fixed routes.
                continue;
            }

            self.conns[connector].m_needs_repaint = false;
            let rerouted = ConnRef::generate_path(self, connector);
            if rerouted {
                rerouted_conns.push(connector);
            }
        }

        // Perform any complete hyperedge rerouting that has been requested.
        HyperedgeRerouter::perform_rerouting(self);

        // Find and reroute crossing connectors if crossing penalties are set.
        self.improve_crossings();

        let with_minor_improvements = self.routing_option(RoutingOption::ImproveHyperedgeRoutesMovingJunctions);
        let with_major_improvements =
            self.routing_option(RoutingOption::ImproveHyperedgeRoutesMovingAddingAndDeletingJunctions);
        if with_minor_improvements || with_major_improvements {
            self.m_hyperedge_improver.clear();
            HyperedgeImprover::execute(self, with_major_improvements);
        }

        // Perform centring and nudging for orthogonal routes.
        improve_orthogonal_routes(self);

        // Find a list of all the deleted connectors in hyperedges.
        let changed_hyperedge_objs = self.m_hyperedge_improver.new_and_deleted_object_lists();
        let mut deleted_conns = changed_hyperedge_objs.deleted_connector_list;
        for index in 0..self.m_hyperedge_rerouter.count() {
            let changed = self.m_hyperedge_rerouter.new_and_deleted_object_lists(index);
            // PTR-ORDER: upstream std::list::merge orders by pointer; the
            // list is only used for membership tests below, so its order
            // cannot matter and the elements are simply appended.
            deleted_conns.extend(changed.deleted_connector_list);
        }

        // Alert connectors that they need redrawing.
        for &conn in rerouted_conns.iter() {
            // Skip hyperedge connectors which have been deleted.
            if deleted_conns.contains(&conn) {
                // Connector deleted, skip.
                continue;
            }

            self.conns[conn].m_needs_repaint = true;
            ConnRef::perform_callback(self, conn);
        }

        // Progress reporting.
        self.perform_continuation_check(TRANSACTION_PHASE_COMPLETED, 1, 1);
    }

    // upstream: router.cpp Router::performContinuationCheck()
    pub fn perform_continuation_check(&mut self, phase_number: u32, step_number: usize, total_steps: usize) {
        // Compute the elapsed time in msec since the beginning of the transaction.
        let elapsed_msec = elapsed_msec(&self.m_transaction_start_time);

        let should_continue = self.should_continue_transaction_with_progress(
            elapsed_msec,
            phase_number,
            TRANSACTION_PHASE_COMPLETED,
            step_number as f64 / total_steps as f64,
        );
        if !should_continue {
            // Host program has asked us not to continue the transaction.
            self.m_abort_transaction = true;
        }
    }

    /// Virtual upstream; override through `m_progress_callback`.  The
    /// default always continues.
    // upstream: router.cpp Router::shouldContinueTransactionWithProgress()
    pub fn should_continue_transaction_with_progress(
        &mut self,
        elapsed_time: u32,
        phase_number: u32,
        total_phases: u32,
        proportion: f64,
    ) -> bool {
        if let Some(cb) = self.m_progress_callback.as_mut() {
            return cb(elapsed_time, phase_number, total_phases, proportion);
        }
        // We always continue.  Subclasses can override this behaviour.
        true
    }

    // upstream: router.cpp Router::improveCrossings()
    pub fn improve_crossings(&mut self) {
        let crossing_penalty = self.routing_parameter(RoutingParameter::CrossingPenalty);
        let shared_path_penalty = self.routing_parameter(RoutingParameter::FixedSharedPathPenalty);
        if (crossing_penalty == 0.0) && (shared_path_penalty == 0.0) {
            // No penalties, return.
            return;
        }

        // Information on crossing connector groups.
        let mut crossing_conn_info = CrossingConnectorsInfo::default();

        let num_of_conns = self.conn_refs.len();
        let mut num_of_conns_checked = 0usize;

        // Find crossings and reroute connectors.
        self.m_in_crossing_rerouting_stage = true;
        let conn_list = self.conn_refs.clone();
        for (ii, &i) in conn_list.iter().enumerate() {
            // Progress reporting and continuation check.
            num_of_conns_checked += 1;
            self.perform_continuation_check(TRANSACTION_PHASE_CROSSING_DETECTION, num_of_conns_checked, num_of_conns);
            if self.m_abort_transaction {
                self.m_in_crossing_rerouting_stage = false;
                return;
            }

            if self.conns[i].route().size() == 0 {
                // Rerouted hyperedges will have an empty route.
                // We can't reroute these.
                continue;
            }
            for &j in conn_list[ii + 1..].iter() {
                if crossing_conn_info.conns_known_to_cross(self, i, j) {
                    // We already know both these have crossings.
                    continue;
                }

                // Determine if this pair cross.
                // (The two routes are moved out of their ConnRefs for the
                // duration of the check; see ConnectorCrossings docs.)
                let mut i_route = std::mem::take(&mut self.conns[i].m_route);
                let mut j_route = std::mem::take(&mut self.conns[j].m_route);
                let mut add = false;
                {
                    let mut cross = ConnectorCrossings::new(true, Some(i), Some(j));
                    let mut j_ind = 1;
                    while j_ind < j_route.size() {
                        let final_segment = (j_ind + 1) == j_route.size();
                        cross.count_for_segment(self, &mut i_route, &mut j_route, j_ind, final_segment);

                        if (shared_path_penalty > 0.0)
                            && (cross.crossing_flags & CROSSING_SHARES_PATH) != 0
                            && (cross.crossing_flags & CROSSING_SHARES_FIXED_SEGMENT) != 0
                            && (self.m_routing_options
                                [RoutingOption::PenaliseOrthogonalSharedPathsAtConnEnds as usize]
                                || (cross.crossing_flags & CROSSING_SHARES_PATH_AT_END) == 0)
                        {
                            // We are penalising fixedSharedPaths and there is a
                            // fixedSharedPath.
                            add = true;
                            break;
                        } else if (crossing_penalty > 0.0) && (cross.crossing_count > 0) {
                            // We are penalising crossings and this is a crossing.
                            add = true;
                            break;
                        }
                        j_ind += 1;
                    }
                }
                self.conns[i].m_route = i_route;
                self.conns[j].m_route = j_route;
                if add {
                    crossing_conn_info.add_crossing(self, i, j);
                }
            }
        }

        // Find the list of connector sets that need to be removed to avoid any
        // crossings in all crossing groups.  This is our candidate set for
        // rerouting.  Where these connectors connect to exlusive pins, all
        // connectors attached to exclusive pins with the same IDs will also
        // be rerouted, starting with the shortest.
        let crossing_conns_groups = crossing_conn_info.crossing_sets_list_to_remove_crossings_from_groups(self);

        // At this point we have a list containing crossings for rerouting.
        // We do this rerouting via two passes, for each group of interacting
        // crossing connectors:
        //  1) clear existing routes and free pin assignments, and
        //  2) compute new routes.
        let mut num_of_conns_to_reroute: u32 = 1;
        let mut num_of_conns_rerouted: u32 = 1;
        for set in crossing_conns_groups.iter() {
            // Sort the connectors we will be rerouting from lowest to
            // highest cost.
            let ordered: Vec<(f64, ConnIdx)> = set.values().copied().collect();
            let ordered_conn_list =
                libstdcxx_list_sort(ordered, &mut |u: &(f64, ConnIdx), v: &(f64, ConnIdx)| u.0 < v.0);

            // Perform rerouting of this set of connectors.
            for pass in 0..2 {
                for &(_, conn) in ordered_conn_list.iter() {
                    if pass == 0 {
                        num_of_conns_to_reroute += 1;

                        // Mark the fixed shared path as being invalid.
                        self.conns[conn].make_path_invalid();

                        // Free the previous path, so it is not noticed by other
                        // connectors during rerouting.
                        self.conns[conn].free_routes();

                        // Free pin assignments.
                        ConnRef::free_active_pins(self, conn);
                    } else if pass == 1 {
                        // Progress reporting and continuation check.
                        self.perform_continuation_check(
                            TRANSACTION_PHASE_REROUTE_SEARCH,
                            num_of_conns_rerouted as usize,
                            num_of_conns_to_reroute as usize,
                        );
                        if self.m_abort_transaction {
                            self.m_in_crossing_rerouting_stage = false;
                            return;
                        }
                        num_of_conns_rerouted += 1;

                        // Recompute this path.
                        ConnRef::generate_path(self, conn);
                    }
                }
            }
        }
        self.m_in_crossing_rerouting_stage = false;
    }

    // upstream: router.cpp Router::newBlockingShape()
    pub fn new_blocking_shape(&mut self, poly: &Polygon, pid: i32) {
        // o  Check all visibility edges to see if this one shape
        //    blocks them.
        let finish = self.vis_graph.end();
        let mut iter = self.vis_graph.begin();
        while iter != finish {
            let tmp = iter.unwrap();
            iter = self.edge_infs[tmp].lst_next;

            if self.edge_infs[tmp].get_dist() != 0.0 {
                let ids = self.edge_infs[tmp].ids(&self.vert_infs);
                let e_id1 = ids.0;
                let e_id2 = ids.1;
                let points = self.edge_infs[tmp].points(&self.vert_infs);
                let e1 = points.0;
                let e2 = points.1;
                let mut blocked = false;

                let count_border = false;
                let ep_in_poly1 = if e_id1.is_conn_pt() { in_poly(poly, &e1, count_border) } else { false };
                let ep_in_poly2 = if e_id2.is_conn_pt() { in_poly(poly, &e2, count_border) } else { false };
                if ep_in_poly1 || ep_in_poly2 {
                    // Don't check edges that have a connector endpoint
                    // and are inside the shape being added.
                    continue;
                }

                let mut seen_intersection_at_endpoint = false;
                for pt_i in 0..poly.size() {
                    let pt_n = if pt_i == (poly.size() - 1) { 0 } else { pt_i + 1 };
                    let pi = &poly.ps[pt_i];
                    let pn = &poly.ps[pt_n];
                    if segment_shape_intersect(&e1, &e2, pi, pn, &mut seen_intersection_at_endpoint) {
                        blocked = true;
                        break;
                    }
                }
                if blocked {
                    EdgeInf::alert_conns(self, tmp);
                    if self.invisibility_grph {
                        EdgeInf::add_blocker(self, tmp, pid);
                    } else {
                        EdgeInf::delete(self, tmp);
                    }
                }
            }
        }
    }

    // upstream: router.cpp Router::checkAllBlockedEdges()
    pub fn check_all_blocked_edges(&mut self, pid: i32) {
        debug_assert!(self.invisibility_grph);

        let mut iter = self.invis_graph.begin();
        while iter != self.invis_graph.end() {
            let tmp = iter.unwrap();
            iter = self.edge_infs[tmp].lst_next;

            if self.edge_infs[tmp].blocker() == -1 {
                EdgeInf::alert_conns(self, tmp);
                EdgeInf::check_vis(self, tmp);
            } else if self.edge_infs[tmp].blocker() == pid {
                EdgeInf::check_vis(self, tmp);
            }
        }
    }

    // upstream: router.cpp Router::checkAllMissingEdges()
    pub fn check_all_missing_edges(&mut self) {
        debug_assert!(!self.invisibility_grph);

        let first = self.vertices.conns_begin();

        let pend = self.vertices.end();
        let mut i = first;
        while i != pend {
            let iv = i.unwrap();
            let i_id = self.vert_infs[iv].id;

            // Check remaining, earlier vertices
            let mut j = first;
            while j != i {
                let jv = j.unwrap();
                let j_id = self.vert_infs[jv].id;
                if i_id.is_conn_pt() && !i_id.is_connection_pin() && (i_id.obj_id != j_id.obj_id) {
                    // Don't keep visibility between edges of different conns
                    j = self.vert_infs[jv].lst_next;
                    continue;
                }

                // See if the edge is already there?
                let found = EdgeInf::existing_edge(self, iv, jv).is_some();

                if !found {
                    // Didn't already exist, check.
                    let known_new = true;
                    EdgeInf::check_edge_visibility(self, iv, jv, known_new);
                }
                j = self.vert_infs[jv].lst_next;
            }
            i = self.vert_infs[iv].lst_next;
        }
    }

    // upstream: router.cpp Router::generateContains()
    pub fn generate_contains(&mut self, pt: VertIdx) {
        let pt_id = self.vert_infs[pt].id;
        let point = self.vert_infs[pt].point;
        self.contains.entry(pt_id).or_default().clear();
        self.enclosing_clusters.entry(pt_id).or_default().clear();

        // Don't count points on the border as being inside.
        let count_border = false;

        // Compute enclosing shapes.
        let mut found_shapes = Vec::new();
        for &i in self.m_obstacles.iter() {
            if in_poly(&Obstacle::routing_polygon(self, i), &point, count_border) {
                found_shapes.push(self.obstacles[i].m_id);
            }
        }
        let entry = self.contains.entry(pt_id).or_default();
        for id in found_shapes {
            entry.insert(id);
        }

        // Computer enclosing Clusters
        let mut found_clusters = Vec::new();
        for &i in self.cluster_refs.iter() {
            if in_poly_gen(&self.clusters[i].m_polygon.view(self), &point) {
                found_clusters.push(self.clusters[i].m_id);
            }
        }
        let entry = self.enclosing_clusters.entry(pt_id).or_default();
        for id in found_clusters {
            entry.insert(id);
        }
    }

    // upstream: router.cpp Router::adjustClustersWithAdd()
    pub fn adjust_clusters_with_add<P: PolygonInterface + ?Sized>(&mut self, poly: &P, p_cluster: i32) {
        let shapes_begin = self.vertices.shapes_begin();
        let mut k = self.vertices.conns_begin();
        while k != shapes_begin {
            let kv = k.unwrap();
            if in_poly_gen(poly, &self.vert_infs[kv].point) {
                let id = self.vert_infs[kv].id;
                self.enclosing_clusters.entry(id).or_default().insert(p_cluster as u32);
            }
            k = self.vert_infs[kv].lst_next;
        }
    }

    // upstream: router.cpp Router::adjustClustersWithDel()
    pub fn adjust_clusters_with_del(&mut self, p_cluster: i32) {
        for (_, v) in self.enclosing_clusters.iter_mut() {
            v.remove(&(p_cluster as u32));
        }
    }

    // upstream: router.cpp Router::adjustContainsWithAdd()
    pub fn adjust_contains_with_add(&mut self, poly: &Polygon, p_shape: i32) {
        // Don't count points on the border as being inside.
        let count_border = false;

        let shapes_begin = self.vertices.shapes_begin();
        let mut k = self.vertices.conns_begin();
        while k != shapes_begin {
            let kv = k.unwrap();
            if in_poly(poly, &self.vert_infs[kv].point, count_border) {
                let id = self.vert_infs[kv].id;
                self.contains.entry(id).or_default().insert(p_shape as u32);
            }
            k = self.vert_infs[kv].lst_next;
        }
    }

    // upstream: router.cpp Router::adjustContainsWithDel()
    pub fn adjust_contains_with_del(&mut self, p_shape: i32) {
        for (_, v) in self.contains.iter_mut() {
            v.remove(&(p_shape as u32));
        }
    }

    /// Given an deleted obstacle, uses a simple heauristic to determine
    /// polyline connectors that may now have a better path through the
    /// region occupied by the shape and mark them as needing to be
    /// rerouted.  See the "Incremental Connector Routing" paper.
    // upstream: router.cpp Router::markPolylineConnectorsNeedingReroutingForDeletedObstacle()
    pub fn mark_polyline_connectors_needing_rerouting_for_deleted_obstacle(&mut self, obstacle: ObstacleIdx) {
        if self.rubber_band_routing {
            // When rubber-band routing, we do not reroute connectors that
            // may have a better route, only invalid connectors.
            return;
        }

        debug_assert!(self.selective_reroute);

        // For each connector...
        let conn_list = self.conn_refs.clone();
        for &conn in conn_list.iter() {
            {
                let c = &self.conns[conn];
                if c.m_route.ps.is_empty() {
                    // Ignore uninitialised connectors.
                    continue;
                } else if c.m_needs_reroute_flag {
                    // Already marked, so skip.
                    continue;
                } else if c.routing_type() != ConnType::ConnTypePolyLine {
                    // This test only works for polyline connectors, so skip others.
                    continue;
                }
            }

            let mut start = self.conns[conn].m_route.ps[0];
            let mut end = self.conns[conn].m_route.ps[self.conns[conn].m_route.size() - 1];

            let conndist = self.conns[conn].m_route_dist;

            let mut estdist;
            let mut e1;
            let mut e2;

            // For each vertex of the obstacle...
            let begin_v = self.obstacles[obstacle].first_vert();
            let end_v = self.vert_infs[self.obstacles[obstacle].last_vert().unwrap()].lst_next;
            let mut i = begin_v;
            while i != end_v {
                let iv = i.unwrap();
                let p1 = self.vert_infs[iv].point;
                let p2 = self.vert_infs[self.vert_infs[iv].sh_next.unwrap()].point;
                i = self.vert_infs[iv].lst_next;

                let offy;
                let a;
                let b;
                let c;
                let mut d;

                let min;
                let max;

                if p1.y == p2.y {
                    // Standard case
                    offy = p1.y;
                    a = start.x;
                    b = start.y - offy;
                    c = end.x;
                    d = end.y - offy;

                    min = cpp_min(p1.x, p2.x);
                    max = cpp_max(p1.x, p2.x);
                } else if p1.x == p2.x {
                    // Other Standard case
                    offy = p1.x;
                    a = start.y;
                    b = start.x - offy;
                    c = end.y;
                    d = end.x - offy;

                    min = cpp_min(p1.y, p2.y);
                    max = cpp_max(p1.y, p2.y);
                } else {
                    // Need to do rotation
                    let n_p2 = Point::new(p2.x - p1.x, p2.y - p1.y);
                    let n_start = Point::new(start.x - p1.x, start.y - p1.y);
                    let n_end = Point::new(end.x - p1.x, end.y - p1.y);

                    let theta = 0.0 - n_p2.y.atan2(n_p2.x);

                    let r_p1 = Point::new(0.0, 0.0);
                    let mut r_p2 = n_p2;
                    start = n_start;
                    end = n_end;

                    let cosv = theta.cos();
                    let sinv = theta.sin();

                    r_p2.x = cosv * n_p2.x - sinv * n_p2.y;
                    r_p2.y = cosv * n_p2.y + sinv * n_p2.x;
                    start.x = cosv * n_start.x - sinv * n_start.y;
                    start.y = cosv * n_start.y + sinv * n_start.x;
                    end.x = cosv * n_end.x - sinv * n_end.y;
                    end.y = cosv * n_end.y + sinv * n_end.x;

                    // This might be slightly off.
                    r_p2.y = 0.0;

                    offy = r_p1.y;
                    a = start.x;
                    b = start.y - offy;
                    c = end.x;
                    d = end.y - offy;

                    min = cpp_min(r_p1.x, r_p2.x);
                    max = cpp_max(r_p1.x, r_p2.x);
                }

                let mut x;
                if (b + d) == 0.0 {
                    d = d * -1.0;
                }

                if (b == 0.0) && (d == 0.0) {
                    if ((a < min) && (c < min)) || ((a > max) && (c > max)) {
                        // It's going to get adjusted.
                        x = a;
                    } else {
                        continue;
                    }
                } else {
                    x = ((b * c) + (a * d)) / (b + d);
                }

                x = cpp_max(min, x);
                x = cpp_min(max, x);

                let mut xp = Point::default();
                if p1.x == p2.x {
                    xp.x = offy;
                    xp.y = x;
                } else {
                    xp.x = x;
                    xp.y = offy;
                }

                e1 = euclidean_dist(&start, &xp);
                e2 = euclidean_dist(&xp, &end);
                estdist = e1 + e2;

                if estdist < conndist {
                    self.conns[conn].m_needs_reroute_flag = true;
                    break;
                }
            }
        }
    }

    // upstream: router.cpp Router::validConnType()
    pub fn valid_conn_type(&self, select: ConnType) -> ConnType {
        if select != ConnType::ConnTypeNone {
            if (select == ConnType::ConnTypeOrthogonal) && self.m_allows_orthogonal_routing {
                return ConnType::ConnTypeOrthogonal;
            } else if (select == ConnType::ConnTypePolyLine) && self.m_allows_polyline_routing {
                return ConnType::ConnTypePolyLine;
            }
        }

        if self.m_allows_polyline_routing {
            return ConnType::ConnTypePolyLine;
        } else if self.m_allows_orthogonal_routing {
            return ConnType::ConnTypeOrthogonal;
        }
        ConnType::ConnTypeNone
    }

    /// Sets values for routing parameters.  A negative `value` (upstream
    /// default `chooseSensibleParamValue`) sets a sensible default.
    // upstream: router.cpp Router::setRoutingParameter()
    pub fn set_routing_parameter(&mut self, parameter: RoutingParameter, value: f64) {
        let p = parameter as usize;
        if value < 0.0 {
            // Set some sensible parameter value for the parameter being 'active'.
            self.m_routing_parameters[p] = match parameter {
                RoutingParameter::SegmentPenalty => 50.0,
                RoutingParameter::FixedSharedPathPenalty => 110.0,
                RoutingParameter::AnglePenalty => 50.0,
                RoutingParameter::CrossingPenalty => 200.0,
                RoutingParameter::ClusterCrossingPenalty => 4000.0,
                RoutingParameter::IdealNudgingDistance => 4.0,
                RoutingParameter::PortDirectionPenalty => 100.0,
                _ => 50.0,
            };
        } else {
            self.m_routing_parameters[p] = value;
        }
        self.m_settings_changes = true;
    }

    // upstream: router.cpp Router::routingParameter()
    #[inline]
    pub fn routing_parameter(&self, parameter: RoutingParameter) -> f64 {
        self.m_routing_parameters[parameter as usize]
    }

    // upstream: router.cpp Router::setRoutingOption()
    pub fn set_routing_option(&mut self, option: RoutingOption, value: bool) {
        self.m_routing_options[option as usize] = value;
        self.m_settings_changes = true;
    }

    // upstream: router.cpp Router::routingOption()
    #[inline]
    pub fn routing_option(&self, option: RoutingOption) -> bool {
        self.m_routing_options[option as usize]
    }

    /// Sets or removes penalty values that are applied during connector
    /// routing.  (Use `CHOOSE_SENSIBLE_PARAM_VALUE` for the upstream
    /// default argument.)
    // upstream: router.cpp Router::setRoutingPenalty()
    pub fn set_routing_penalty(&mut self, pen_type: RoutingParameter, pen_val: f64) {
        self.set_routing_parameter(pen_type, pen_val);
    }

    // upstream: router.cpp Router::registerSettingsChange()
    pub fn register_settings_change(&mut self) {
        self.m_settings_changes = true;
    }

    // upstream: router.cpp Router::hyperedgeRerouter()
    pub fn hyperedge_rerouter(&mut self) -> &mut HyperedgeRerouter {
        &mut self.m_hyperedge_rerouter
    }

    // upstream: router.cpp Router::isInCrossingPenaltyReroutingStage()
    pub fn is_in_crossing_penalty_rerouting_stage(&self) -> bool {
        self.m_in_crossing_rerouting_stage
    }

    /// Returns the visibility graph statistics text upstream prints to
    /// stdout.
    // upstream: router.cpp Router::printInfo()
    pub fn print_info(&self) -> String {
        let mut out = String::new();
        out.push_str("\nVisibility Graph info:\n");
        out.push_str("----------------------\n");

        let mut currshape: u32 = 0;
        let mut st_shapes = 0;
        let mut st_vertices = 0;
        let mut st_endpoints = 0;
        let mut st_valid_shape_visedges = 0;
        let mut st_valid_endpt_visedges = 0;
        let mut st_orthogonal_visedges = 0;
        let mut st_invalid_visedges = 0;
        let mut t = self.vertices.conns_begin();
        while let Some(tv) = t {
            let p_id = self.vert_infs[tv].id;

            if !(p_id.is_conn_pt()) && (p_id.obj_id != currshape) {
                currshape = p_id.obj_id;
                st_shapes += 1;
            }
            if !(p_id.is_conn_pt()) {
                st_vertices += 1;
            } else {
                // The shape 0 ones are temporary and not considered.
                st_endpoints += 1;
            }
            t = self.vert_infs[tv].lst_next;
        }
        for e in self.vis_graph.to_vec(&self.edge_infs) {
            let idpair = self.edge_infs[e].ids(&self.vert_infs);

            if idpair.0.is_conn_pt() || idpair.1.is_conn_pt() {
                st_valid_endpt_visedges += 1;
            } else {
                st_valid_shape_visedges += 1;
            }
        }
        st_invalid_visedges += self.invis_graph.to_vec(&self.edge_infs).len() as i32;
        st_orthogonal_visedges += self.vis_orthog_graph.to_vec(&self.edge_infs).len() as i32;
        out.push_str(&format!("Number of shapes: {}\n", st_shapes));
        out.push_str(&format!(
            "Number of vertices: {} ({} real, {} endpoints)\n",
            st_vertices + st_endpoints,
            st_vertices,
            st_endpoints
        ));
        out.push_str(&format!("Number of orthog_vis_edges: {}\n", st_orthogonal_visedges));
        out.push_str(&format!(
            "Number of vis_edges: {} ({} valid [{} normal, {} endpt], {} invalid)\n",
            st_valid_shape_visedges + st_invalid_visedges + st_valid_endpt_visedges,
            st_valid_shape_visedges + st_valid_endpt_visedges,
            st_valid_shape_visedges,
            st_valid_endpt_visedges,
            st_invalid_visedges
        ));
        out.push_str("----------------------\n");
        out.push_str(&format!("checkVisEdge tally: {}\n", self.st_checked_edges));
        out.push_str("----------------------\n");
        out
    }

    // ---------------------------------------------------------------------
    // The following methods are for testing and debugging.
    // ---------------------------------------------------------------------

    /// Runs `ConnectorCrossings` over every connector pair (display
    /// routes) and returns true as soon as `pred(flags)` holds.
    fn exists_pair_with<F: Fn(&ConnectorCrossings) -> bool>(&mut self, pred: F) -> bool {
        let conn_list = self.conn_refs.clone();
        for (ii, &i) in conn_list.iter().enumerate() {
            let mut i_route = self.conns[i].display_route().clone();
            for &j in conn_list[ii + 1..].iter() {
                // Determine if this pair overlap
                let mut j_route = self.conns[j].display_route().clone();
                let mut cross = ConnectorCrossings::new(true, Some(i), Some(j));
                cross.check_for_branching_segments = true;
                let mut j_ind = 1;
                while j_ind < j_route.size() {
                    let final_segment = (j_ind + 1) == j_route.size();
                    cross.count_for_segment(self, &mut i_route, &mut j_route, j_ind, final_segment);

                    if pred(&cross) {
                        return true;
                    }
                    j_ind += 1;
                }
            }
        }
        false
    }

    // upstream: router.cpp Router::existsOrthogonalSegmentOverlap()
    pub fn exists_orthogonal_segment_overlap(&mut self, at_ends: bool) -> bool {
        self.exists_pair_with(|cross| {
            // We are looking for fixedSharedPaths and there is a
            // fixedSharedPath.
            (cross.crossing_flags & CROSSING_SHARES_PATH) != 0
                && (at_ends || (cross.crossing_flags & CROSSING_SHARES_PATH_AT_END) == 0)
        })
    }

    // upstream: router.cpp Router::existsOrthogonalFixedSegmentOverlap()
    pub fn exists_orthogonal_fixed_segment_overlap(&mut self, at_ends: bool) -> bool {
        self.exists_pair_with(|cross| {
            (cross.crossing_flags & CROSSING_SHARES_PATH) != 0
                && (cross.crossing_flags & CROSSING_SHARES_FIXED_SEGMENT) != 0
                && (at_ends || (cross.crossing_flags & CROSSING_SHARES_PATH_AT_END) == 0)
        })
    }

    // upstream: router.cpp Router::existsOrthogonalTouchingPaths()
    pub fn exists_orthogonal_touching_paths(&mut self) -> bool {
        self.exists_pair_with(|cross| (cross.crossing_flags & CROSSING_TOUCHES) != 0)
    }

    /// Counts the number of crossings between all connectors.
    ///
    /// If `optimised_for_connector_type` is set, This will only count
    /// crossings between orthogonal connectors if they appear at segment
    /// endpoints.  Thus, it can be used on raw connectors but not on
    /// simplified orthogonal connectors.
    // upstream: router.cpp Router::existsCrossings()
    pub fn exists_crossings(&mut self, optimised_for_connector_type: bool) -> i32 {
        let mut count: i32 = 0;
        let conn_list = self.conn_refs.clone();
        for (ii, &i) in conn_list.iter().enumerate() {
            let mut i_route = self.conns[i].display_route().clone();
            for &j in conn_list[ii + 1..].iter() {
                // Determine if this pair overlap
                let mut j_route = self.conns[j].display_route().clone();
                let i_conn = if optimised_for_connector_type { Some(i) } else { None };
                let j_conn = if optimised_for_connector_type { Some(j) } else { None };
                let mut cross = ConnectorCrossings::new(true, i_conn, j_conn);
                cross.check_for_branching_segments = true;
                let mut j_ind = 1;
                while j_ind < j_route.size() {
                    let final_segment = (j_ind + 1) == j_route.size();

                    // Normal crossings aren't counted if we pass the pointers
                    // for the connectors, so don't pass them.
                    cross.count_for_segment(self, &mut i_route, &mut j_route, j_ind, final_segment);

                    count += cross.crossing_count as i32;
                    j_ind += 1;
                }
            }
        }
        count
    }

    /// Looks for non-orthogonal segments in orthogonal connectors and
    /// returns true if it finds any.
    // upstream: router.cpp Router::existsInvalidOrthogonalPaths()
    pub fn exists_invalid_orthogonal_paths(&mut self) -> bool {
        // For each connector...
        let conn_list = self.conn_refs.clone();
        for &i in conn_list.iter() {
            // If it is an orthogonal connector...
            if self.conns[i].routing_type() == ConnType::ConnTypeOrthogonal {
                // Check each segment of the path...
                let i_route = self.conns[i].display_route().clone();
                for i_ind in 1..i_route.size() {
                    // And if it isn't either vertical or horizontal...
                    if (i_route.at(i_ind - 1).x != i_route.at(i_ind).x)
                        && (i_route.at(i_ind - 1).y != i_route.at(i_ind).y)
                    {
                        // Then we've found an invalid path.
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Set an addon for doing orthogonal topology improvement (None resets
    /// to the default no-op addon).
    // upstream: router.cpp Router::setTopologyAddon()
    pub fn set_topology_addon(&mut self, topology_addon: Option<&dyn TopologyAddonInterface>) {
        self.m_topology_addon = match topology_addon {
            Some(t) => t.clone_box(),
            None => std::boxed::Box::new(DefaultTopologyAddon),
        };
        self.register_settings_change();
    }

    // upstream: router.cpp Router::improveOrthogonalTopology()
    pub fn improve_orthogonal_topology(&mut self) {
        let mut addon = std::mem::replace(&mut self.m_topology_addon, std::boxed::Box::new(DefaultTopologyAddon));
        addon.improve_orthogonal_topology(self);
        self.m_topology_addon = addon;
    }

    /// Upstream `outputDiagramText()` body, as written to the `.txt` file
    /// (C `%g` formatting, shapes then connectors).
    // upstream: router.cpp Router::outputDiagramText()
    pub fn output_diagram_text_g(&mut self) -> String {
        self.output_diagram_text_impl(6, false)
    }

    /// The oracle-patched `outputDiagramText()` used to produce
    /// `tests/golden/*.txt`: `%.17g` formatting plus a `junction` block for
    /// every junction, between the shapes and the connectors.
    // upstream: router.cpp Router::outputDiagramText() (as patched by tools/oracle/build.sh)
    pub fn output_diagram_text(&mut self) -> String {
        self.output_diagram_text_impl(17, true)
    }

    fn output_diagram_text_impl(&mut self, prec: usize, with_junctions: bool) -> String {
        let g = |v: f64| format_g(v, prec);
        let mut fp = String::new();

        for &obstacle in self.m_obstacles.iter() {
            let is_shape = self.obstacles[obstacle].is_shape();

            if !is_shape {
                // Don't output non-shape obstacles here, for now.
                continue;
            }

            let b_box = self.obstacles[obstacle].m_polygon.offset_bounding_box(0.0);

            fp.push_str("rect\n");
            fp.push_str(&format!("id={}\n", self.obstacles[obstacle].m_id));
            fp.push_str(&format!("x={}\n", g(b_box.min.x)));
            fp.push_str(&format!("y={}\n", g(b_box.min.y)));
            fp.push_str(&format!("width={}\n", g(b_box.max.x - b_box.min.x)));
            fp.push_str(&format!("height={}\n", g(b_box.max.y - b_box.min.y)));
            fp.push('\n');
        }

        if with_junctions {
            for &o in self.m_obstacles.iter() {
                if let Some(junction) = Obstacle::as_junction(self, o) {
                    let pos = junction.position(self);
                    fp.push_str("junction\n");
                    fp.push_str(&format!("id={}\n", self.obstacles[o].m_id));
                    fp.push_str(&format!("x={}\n", format_g(pos.x, 17)));
                    fp.push_str(&format!("y={}\n", format_g(pos.y, 17)));
                    fp.push('\n');
                }
            }
        }

        let conn_list = self.conn_refs.clone();
        for conn_ref in conn_list {
            let route = self.conns[conn_ref].display_route().clone();
            if !route.empty() {
                fp.push_str("path\n");
                fp.push_str(&format!("id={}\n", self.conns[conn_ref].m_id));
                for i in 0..route.size() {
                    fp.push_str(&format!("p{}: {} {} ", i, g(route.ps[i].x), g(route.ps[i].y)));
                    fp.push('\n');
                }
                fp.push('\n');
            }
        }

        fp.push('\n');

        fp
    }

    // upstream: router.cpp Router::newAndDeletedObjectListsFromHyperedgeImprovement()
    pub fn new_and_deleted_object_lists_from_hyperedge_improvement(&self) -> HyperedgeNewAndDeletedObjectLists {
        self.m_hyperedge_improver.new_and_deleted_object_lists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avoid::connectionpin::{ShapeConnectionPin, ATTACH_POS_CENTRE, ATTACH_POS_LEFT, ATTACH_POS_TOP};
    use crate::avoid::connend::{CONN_DIR_ALL, CONN_DIR_LEFT, CONN_DIR_NONE};
    use crate::avoid::geomtypes::Rectangle;
    use crate::avoid::vertices::VertID;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn rect(x1: f64, y1: f64, x2: f64, y2: f64) -> Polygon {
        Rectangle::new(&Point::new(x1, y1), &Point::new(x2, y2))
    }

    #[test]
    fn format_g_matches_c_printf() {
        assert_eq!(format_g(1.0 / 3.0, 6), "0.333333");
        assert_eq!(format_g(123456789.0 - 100.0, 6), "1.23457e+08");
        assert_eq!(format_g(1e-7, 6), "1e-07");
        assert_eq!(format_g(100.0, 6), "100");
        assert_eq!(format_g(-0.0, 6), "-0");
        assert_eq!(format_g(0.0001, 6), "0.0001");
        assert_eq!(format_g(0.00001, 6), "1e-05");
        assert_eq!(format_g(1e21, 17), "1e+21");
        assert_eq!(format_g(0.1, 17), "0.10000000000000001");
        assert_eq!(format_g(50695.0, 17), "50695");
        assert_eq!(format_g(f64::INFINITY, 6), "inf");
    }

    #[test]
    fn pin_set_ignores_equal_pins_and_erases_by_key() {
        let mut r = Router::new(ORTHOGONAL_ROUTING);
        let s = ShapeRef::new(&mut r, &rect(0.0, 0.0, 10.0, 10.0), 1);
        let p1 = ShapeConnectionPin::new_shape(&mut r, s, 1, ATTACH_POS_LEFT, ATTACH_POS_TOP, true, 0.0, CONN_DIR_NONE);
        let p2 = ShapeConnectionPin::new_shape(&mut r, s, 1, ATTACH_POS_LEFT, ATTACH_POS_TOP, true, 0.0, CONN_DIR_NONE);
        let p3 =
            ShapeConnectionPin::new_shape(&mut r, s, 1, ATTACH_POS_CENTRE, ATTACH_POS_TOP, true, 0.0, CONN_DIR_NONE);
        // p2 is "equal" to p1 under operator<, so std::set::insert ignores it.
        assert_eq!(s.connection_pins(&r), vec![p1, p3]);
        // erase(p2) erases the equal element p1 (std::set::erase by key).
        let mut set = r.obstacles[s.0].m_connection_pins.clone();
        assert_eq!(set.erase(&r.pins, &r.obstacles, p2), 1);
        assert_eq!(set.items, vec![p3]);
        assert_eq!(set.erase(&r.pins, &r.obstacles, p1), 0);
        // All three pins have vertices in the router's list.
        assert_eq!(r.vertices.conns_size(), 3);
        r.process_actions();
        r.destruct();
        // The duplicate pin is never deleted (as upstream, where it leaks).
        assert_eq!(r.vertices.conns_size(), 1);
        assert!(r.pins.contains(p2));
        assert!(!r.pins.contains(p1));
    }

    #[test]
    fn destruct_frees_connectors_and_obstacles() {
        let mut r = Router::new(ORTHOGONAL_ROUTING);
        let a = ShapeRef::new(&mut r, &rect(0.0, 0.0, 10.0, 10.0), 0);
        ShapeConnectionPin::new_shape(&mut r, a, 1, ATTACH_POS_LEFT, ATTACH_POS_TOP, true, 0.0, CONN_DIR_NONE);
        let j = JunctionRef::new(&mut r, Point::new(30.0, 30.0), 0);
        let ea = ConnEnd::from_shape_pin(&r, a, 1);
        let ej = ConnEnd::from_junction(&r, j);
        ConnRef::new_with_ends(&mut r, &ea, &ej, 0);
        r.process_actions();
        assert_eq!(r.conn_refs.len(), 1);
        assert_eq!(r.m_obstacles.len(), 2);
        r.destruct();
        assert!(r.conn_refs.is_empty());
        assert!(r.m_obstacles.is_empty());
        assert_eq!(r.vertices.conns_size() + r.vertices.shapes_size(), 0);
        assert!(r.conns.is_empty() && r.obstacles.is_empty() && r.pins.is_empty());
        assert!(r.vert_infs.is_empty());
    }

    #[test]
    fn callbacks_record_and_receive_handle() {
        let mut r = Router::new(ORTHOGONAL_ROUTING);
        let c = ConnRef::new(&mut r, 0);
        let seen = Rc::new(RefCell::new(Vec::new()));
        let seen2 = Rc::clone(&seen);
        c.set_callback(&mut r, Some(std::boxed::Box::new(move |h| seen2.borrow_mut().push(h))));
        ConnRef::perform_callback(&mut r, c);
        let d = ConnRef::new(&mut r, 0);
        ConnRef::perform_callback(&mut r, d); // no callback: not logged
        assert_eq!(*seen.borrow(), vec![c]);
        assert_eq!(r.m_callback_log, vec![c]);
    }

    #[test]
    fn reroute_flag_delegate_alerts_live_connectors() {
        let mut r = Router::new(ORTHOGONAL_ROUTING);
        let c1 = ConnRef::new(&mut r, 0);
        let c2 = ConnRef::new(&mut r, 0);
        r.conns[c1].m_needs_reroute_flag = false;
        r.conns[c2].m_needs_reroute_flag = false;
        let f1 = r.conns[c1].m_reroute_flag_ptr.unwrap();
        let f2 = r.conns[c2].m_reroute_flag_ptr.unwrap();
        *r.m_conn_reroute_flags.flag_mut(f1) = true;
        *r.m_conn_reroute_flags.flag_mut(f2) = true;
        r.m_conn_reroute_flags.remove_conn(c2);
        let Router {
            m_conn_reroute_flags,
            conns,
            ..
        } = &mut r;
        m_conn_reroute_flags.alert_conns(conns);
        assert!(r.conns[c1].m_needs_reroute_flag);
        assert!(!r.conns[c2].m_needs_reroute_flag);
        assert!(!*r.m_conn_reroute_flags.flag_mut(f1));
        // Flags of removed connectors are left alone.
        assert!(*r.m_conn_reroute_flags.flag_mut(f2));
    }

    #[test]
    fn action_list_sort_order() {
        let mut r = Router::new(ORTHOGONAL_ROUTING);
        let s5 = ShapeRef::new(&mut r, &rect(0.0, 0.0, 1.0, 1.0), 5);
        let s2 = ShapeRef::new(&mut r, &rect(0.0, 0.0, 1.0, 1.0), 2);
        let c9 = ConnRef::new(&mut r, 9);
        let c3 = ConnRef::new(&mut r, 3);
        let p_late = ShapeConnectionPin::new_shape(&mut r, s5, 1, 0.5, 0.5, true, 0.0, CONN_DIR_ALL);
        let p_early = ShapeConnectionPin::new_shape(&mut r, s2, 1, 0.5, 0.5, true, 0.0, CONN_DIR_ALL);
        r.modify_connector(c9);
        r.modify_connector(c3);
        r.move_shape(s5, &rect(1.0, 1.0, 2.0, 2.0), false); // folded into the add
        let list = std::mem::take(&mut r.action_list);
        let sorted = libstdcxx_list_sort(list, &mut |a: &ActionInfo, b: &ActionInfo| a.less_than(b));
        let summary: Vec<(ActionType, u32)> = sorted.iter().map(|a| (a.type_, a.obj_id)).collect();
        assert_eq!(
            summary,
            vec![
                (ActionType::ShapeAdd, 2),
                (ActionType::ShapeAdd, 5),
                (ActionType::ConnChange, 3),
                (ActionType::ConnChange, 9),
                (ActionType::ConnectionPinChange, 0),
                (ActionType::ConnectionPinChange, 0),
            ]
        );
        // Pin changes are ordered by creation serial (PTR-ORDER).
        assert_eq!(sorted[4].obj_ptr, ActionObj::Pin(p_late));
        assert_eq!(sorted[5].obj_ptr, ActionObj::Pin(p_early));
        assert_eq!(r.obstacles[s5.0].m_polygon.ps[0], Point::new(2.0, 1.0));
    }

    #[test]
    fn deleting_connector_removes_its_queued_actions() {
        let mut r = Router::new(ORTHOGONAL_ROUTING);
        let c = ConnRef::new(&mut r, 0);
        c.set_source_endpoint(&mut r, &ConnEnd::from_point(Point::new(1.0, 1.0)));
        assert_eq!(r.action_list.len(), 1);
        r.delete_connector(c);
        assert!(r.action_list.is_empty());
        assert!(!r.conns.contains(c));
    }

    #[test]
    fn junction_merge_uses_creation_order() {
        // PTR-ORDER: the first ConnEnd in creation order survives here; the
        // C++ result depends on malloc addresses.
        let mut r = Router::new(ORTHOGONAL_ROUTING);
        let j = JunctionRef::new(&mut r, Point::new(50.0, 50.0), 0);
        let s = ShapeRef::new(&mut r, &rect(90.0, 40.0, 110.0, 60.0), 0);
        ShapeConnectionPin::new_shape(&mut r, s, 1, ATTACH_POS_LEFT, ATTACH_POS_CENTRE, true, 0.0, CONN_DIR_LEFT);
        let ej = ConnEnd::from_junction(&r, j);
        let es = ConnEnd::from_shape_pin(&r, s, 1);
        let c1 = ConnRef::new_with_ends(&mut r, &ConnEnd::from_point(Point::new(0.0, 50.0)), &ej, 0);
        let c2 = ConnRef::new_with_ends(&mut r, &ej, &es, 0);
        r.process_actions();
        let merged = j.remove_junction_and_merge_connectors(&mut r);
        assert_eq!(merged, Some(c1));
        assert!(!r.conns.contains(c2));
        r.process_actions();
        let ends = c1.endpoint_conn_ends(&r);
        assert_eq!(ends.1.m_anchor_obj, Some(s.0));
        assert_eq!(r.m_obstacles, vec![s.0]);
        assert_eq!(r.conns[c1].m_dst_connend.as_ref().unwrap().m_conn_ref, Some(c1));
        let _ = VertID::SRC;
    }

    #[test]
    fn crossing_connectors_info_groups_and_removal() {
        let mut r = Router::new(ORTHOGONAL_ROUTING);
        let cs: Vec<ConnIdx> = (0..5).map(|_| ConnRef::new(&mut r, 0)).collect();
        for (n, &c) in cs.iter().enumerate() {
            // Longer display routes have higher estimated cost.
            let mut route = Polygon::new();
            route.ps.push(Point::new(0.0, 0.0));
            route.ps.push(Point::new(10.0 * (n + 1) as f64, 0.0));
            r.conns[c].set_route(&route);
        }
        let mut info = CrossingConnectorsInfo::default();
        info.add_crossing(&r, cs[0], cs[1]);
        info.add_crossing(&r, cs[2], cs[3]);
        assert_eq!(info.pairs_set_list.len(), 2);
        assert!(info.conns_known_to_cross(&r, cs[1], cs[0]));
        assert!(!info.conns_known_to_cross(&r, cs[1], cs[2]));
        // Joining the two groups merges them.
        info.add_crossing(&r, cs[1], cs[3]);
        assert_eq!(info.pairs_set_list.len(), 1);
        info.add_crossing(&r, cs[4], cs[0]);
        let sets = info.crossing_sets_list_to_remove_crossings_from_groups(&mut r);
        assert_eq!(sets.len(), 1);
        // cs[0] and cs[1] (and cs[3]) have two crossings each, the tie is
        // broken by estimated cost: cs[3] first, then cs[0].
        let chosen: Vec<u32> = sets[0].keys().copied().collect();
        let id = |c: ConnIdx| r.conns[c].m_id;
        assert_eq!(chosen, vec![id(cs[0]), id(cs[3])]);
        assert_eq!(sets[0][&id(cs[3])].0, 2.0);
    }

    #[test]
    fn valid_conn_type_and_parameter_defaults() {
        let r = Router::new(POLY_LINE_ROUTING | ORTHOGONAL_ROUTING);
        assert_eq!(r.valid_conn_type(ConnType::ConnTypeNone), ConnType::ConnTypePolyLine);
        assert_eq!(r.valid_conn_type(ConnType::ConnTypeOrthogonal), ConnType::ConnTypeOrthogonal);
        let mut r = Router::new(ORTHOGONAL_ROUTING);
        r.set_routing_parameter(RoutingParameter::ReverseDirectionPenalty, -3.0);
        assert_eq!(r.routing_parameter(RoutingParameter::ReverseDirectionPenalty), 50.0);
        assert!(r.m_settings_changes);
        let _ = CONN_DIR_LEFT;
    }
}
