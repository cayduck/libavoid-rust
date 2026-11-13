//! Faithful Rust port of upstream C++ libavoid
//! (`vendor/adaptagrams/cola/libavoid`).
//!
//! This module tree supersedes the earlier simplified routing code in the
//! crate root (`crate::router`, `crate::orthogonal`, ...).  Each file here
//! mirrors the upstream file of the same name and must be ported from the
//! upstream source, function by function, preserving behaviour exactly.
//!
//! # Porting conventions (binding for every file in this tree)
//!
//! * **Source of truth.**  Port from the vendored C++; never invent behaviour.
//!   Every ported function carries a short comment naming its upstream
//!   origin, e.g. `// upstream: connector.cpp ConnRef::generatePath()`.
//! * **Ownership.**  Upstream is a pointer graph owned by `Router`.  Here the
//!   `Router` owns arenas and objects refer to each other by small `Copy`
//!   index handles (`VertIdx`, `EdgeIdx`, `ObstacleIdx`, `ConnIdx`, ...).
//!   No `unsafe`, no `Rc<RefCell<..>>` webs.
//! * **Iteration order is behaviour.**  Routes depend on the exact order in
//!   which vertices, edges, obstacles and connectors are visited.  Preserve
//!   upstream container semantics exactly (intrusive list insertion order,
//!   `std::set`/`std::map` comparator order, `std::list` splice order, heap
//!   tie-breaking).
//! * **Pointer-ordered containers.**  Where upstream orders or hashes by
//!   pointer address (`std::set<T*>`, `std::map<T*, ..>`), order by a
//!   monotonic creation serial instead and mark the site with a
//!   `// PTR-ORDER:` comment.  Never let arena slot reuse leak into ordering.
//! * **Floating point.**  Keep upstream expression order and operations
//!   (no `mul_add`, no algebraic simplification, same epsilon constants) so
//!   results are bit-comparable with the C++ oracle.
//! * **Asserts.**  `COLA_ASSERT` becomes `debug_assert!`, except asserts
//!   upstream itself violates on valid input (see "Asserts" in the
//!   `router.rs` module docs).
//! * **Misuse.**  Never panic on API misuse; see "Public-API misuse" and
//!   "Dangling handles" in the `router.rs` module docs.
//! * **Debug/SVG output code** (`outputInstanceToSVG`, timers, db_printf)
//!   is out of scope unless needed by a test.
//! * **Verification.**  `tools/oracle/build.sh` builds the vendored C++ and
//!   writes full-precision golden routes for every upstream fixture to
//!   `tests/golden/` (`ORACLE_ALLOC=bump`: `tests/golden-bump/`, the goldens
//!   the fixture tests compare with).  Ported fixtures must reproduce those
//!   routes.

pub mod actioninfo;
pub mod arena;
pub mod connectionpin;
pub mod connector;
pub mod connend;
pub mod debug;
pub mod debughandler;
pub mod geometry;
pub mod geomtypes;
pub mod graph;
pub mod hyperedge;
pub mod hyperedgeimprover;
pub mod hyperedgetree;
pub mod junction;
pub mod makepath;
pub mod mtst;
pub mod obstacle;
pub mod orthogonal;
pub mod router;
pub mod scanline;
pub mod shape;
pub mod timer;
pub mod vertices;
pub mod viscluster;
pub mod visibility;
pub mod vpsc;
