//! Port of upstream `libavoid/router.{h,cpp}` — **skeleton only**.
//!
//! Only the state needed by geomtypes/geometry/vertices/graph is present;
//! the agent porting router.cpp extends this struct.
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
//! | `Obstacle *`, `ConnRef *`, `ANode *` | `ObstacleIdx`, `ConnIdx`, `ANodeIdx` | added by later agents |
//!
//! `new T(...)` becomes an associated constructor taking `&mut Router` and
//! returning the handle (`VertInf::new(router, id, pt, true)`,
//! `EdgeInf::new(router, v1, v2, orth)`); `delete p` becomes
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
//! pointers) compare the objects' creation `serial`s instead and mark the
//! site `// PTR-ORDER:`.  Do not iterate `HashMap`/`HashSet`s keyed by
//! handles where order could leak; use `BTreeMap` keyed by serial (or a
//! `Vec` in insertion order for `std::list`s).
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
//!   iterated become `Vec<T>` in insertion order.
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

#![allow(dead_code)]

use crate::avoid::arena::{Arena, ConnIdx, RerouteFlagIdx, TreeRootPtr, VertIdx};
use crate::avoid::graph::{EdgeList, EdgeListId};
use crate::avoid::vertices::{ContainsMap, EdgeArena, VertArena, VertInfList};

// upstream: router.h enum RouterFlag
pub const POLY_LINE_ROUTING: u32 = 1;
pub const ORTHOGONAL_ROUTING: u32 = 2;

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

    // NOTE: ConnRerouteFlagDelegate::alertConns() needs ConnRef and is left
    // to the router/connector port.
}

/// Upstream `class Router` (skeleton).
// upstream: router.h class Router
#[derive(Debug)]
pub struct Router {
    // --- arenas (no upstream counterpart; see module docs) ---
    pub vert_infs: VertArena,
    pub edge_infs: EdgeArena,
    /// Cells allocated by `VertInf::makeTreeRootPointer()`.
    pub tree_root_cells: Arena<TreeRootPtr, Option<VertIdx>>,
    next_serial: u64,

    // --- upstream public members ---
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

    // --- upstream private members needed so far ---
    pub m_conn_reroute_flags: ConnRerouteFlagDelegate,
    pub m_allows_polyline_routing: bool,
    pub m_allows_orthogonal_routing: bool,
}

impl Router {
    /// Partial port of the constructor: initialises the members present in
    /// this skeleton with upstream defaults.
    // upstream: router.cpp Router::Router()
    pub fn new(flags: u32) -> Router {
        // At least one of the Routing modes must be set.
        debug_assert!(flags & (POLY_LINE_ROUTING | ORTHOGONAL_ROUTING) != 0);
        Router {
            vert_infs: Arena::new(),
            edge_infs: Arena::new(),
            tree_root_cells: Arena::new(),
            next_serial: 0,
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
            ignore_regions: true,
            use_lees_algorithm: true,
            invisibility_grph: true,
            selective_reroute: true,
            partial_feedback: false,
            rubber_band_routing: false,
            st_checked_edges: 0,
            m_conn_reroute_flags: ConnRerouteFlagDelegate::default(),
            m_allows_polyline_routing: (flags & POLY_LINE_ROUTING) != 0,
            m_allows_orthogonal_routing: (flags & ORTHOGONAL_ROUTING) != 0,
        }
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
}
