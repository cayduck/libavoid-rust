//! Faithful port of upstream `libavoid/vpsc.{h,cpp}`: the incremental VPSC
//! (Variable Placement with Separation Constraints) solver used by
//! orthogonal nudging.
//!
//! This is the variant compiled when `USELIBVPSC` is *not* defined (the
//! default libavoid build), with `LIBVPSC_LOGGING` undefined: at this
//! upstream commit `LIBVPSC_LOGGING` does not compile (`LOGFILE` is never
//! defined, and `printBlocks()` iterates `Blocks` as a set, which it no
//! longer is), so its logging has no reference behaviour to port.  The
//! debug printers it uses (`operator<<` of `Variable`/`Block`/`Constraint`,
//! `Constraint::toString()`, `IncSolver::printBlocks()`) are ported as
//! string writers.
//!
//! # Ownership model
//!
//! Upstream callers own `Variables` / `Constraints` (vectors of pointers)
//! and lend them to `IncSolver(vs, cs)`.  Here callers own a
//! `Vec<Variable>` and a `Vec<Constraint>`; constraints refer to variables
//! by index into that vector ([`VarIdx`]) and the solver refers to
//! constraints by index into the constraint vector ([`ConIdx`]).
//! [`IncSolver::new`] mutably borrows both vectors for the solver's
//! lifetime, exactly like upstream holds references to them.  Blocks live
//! in an arena owned by the solver and are referred to by [`BlockIdx`].
//!
//! State that upstream keeps on the `Variable` / `Constraint` objects
//! across solver instances (notably `Constraint::unsatisfiable`, which the
//! solver never resets) persists here too, because it lives in the
//! caller-owned vectors.
//!
//! # Exceptions
//!
//! * `UnsatisfiableException` is thrown by `Block::findMinLMBetween` and is
//!   always caught in `IncSolver::satisfy`, which marks the violated
//!   constraint unsatisfiable and continues.  Ported as a `Result` with the
//!   same control flow.
//! * `satisfy()` throws a `const char*` ("Unsatisfied constraint: ...")
//!   when a constraint is still violated after merging.  Nothing in
//!   libavoid catches it (it would terminate the process).  Ported as
//!   `Err(UnsatisfiedConstraint)` from `satisfy()` / `solve()`.
//!
//! # Heaps
//!
//! Upstream's `Heap` is `std::priority_queue<Constraint*, vector,
//! CompareConstraints>`.  Since the comparator reads mutable state (slacks
//! and block timestamps), the exact libstdc++ `push_heap`/`pop_heap`
//! algorithms are reproduced in [`pq_push`] / [`pq_pop`].  Note that the
//! heap code is only reachable through `Blocks::split`/`mergeLeft`/
//! `mergeRight` and `blockGraphIsCyclic`, none of which are called by
//! `IncSolver::solve`/`satisfy` upstream; it is ported for completeness and
//! exercised by the differential test.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// upstream: vpsc.cpp `static const double ZERO_UPPERBOUND`
pub const ZERO_UPPERBOUND: f64 = -1e-10;
/// upstream: vpsc.cpp `static const double LAGRANGIAN_TOLERANCE`
pub const LAGRANGIAN_TOLERANCE: f64 = -1e-4;

/// Index of a [`Variable`] in the caller's variable vector.
pub type VarIdx = usize;
/// Index of a [`Constraint`] in the caller's constraint vector.
pub type ConIdx = usize;
/// Index of a block in the solver's block arena.
pub type BlockIdx = usize;

// ---------------------------------------------------------------------------
// Variable
// ---------------------------------------------------------------------------

/// upstream: vpsc.h `class Variable`
#[derive(Clone, Debug)]
pub struct Variable {
    /// useful in log files
    pub id: i32,
    pub desired_position: f64,
    /// Written by `copyResult()`.  Upstream leaves it uninitialised until
    /// then; here it starts at 0.0.
    pub final_position: f64,
    /// how much the variable wants to be at it's desired position
    pub weight: f64,
    /// translates variable to another space
    pub scale: f64,
    pub offset: f64,
    /// Block of the solver currently using this variable (only meaningful
    /// while that solver is alive, like upstream's raw pointer).
    pub(crate) block: Option<BlockIdx>,
    pub visited: bool,
    pub fixed_desired_position: bool,
    pub in_: Vec<ConIdx>,
    pub out: Vec<ConIdx>,
}

impl Variable {
    /// upstream: vpsc.h `Variable::Variable(id, desiredPos=-1.0, weight=1.0,
    /// scale=1.0)`
    pub fn new(id: i32, desired_pos: f64, weight: f64, scale: f64) -> Self {
        Variable {
            id,
            desired_position: desired_pos,
            final_position: 0.0,
            weight,
            scale,
            offset: 0.0,
            block: None,
            visited: false,
            fixed_desired_position: false,
            in_: Vec::new(),
            out: Vec::new(),
        }
    }

    /// Convenience for upstream's `Variable(id, desiredPos, weight)` (scale
    /// defaulted to 1.0).
    pub fn with_weight(id: i32, desired_pos: f64, weight: f64) -> Self {
        Variable::new(id, desired_pos, weight, 1.0)
    }
}

// ---------------------------------------------------------------------------
// Constraint
// ---------------------------------------------------------------------------

/// upstream: vpsc.h `class Constraint`
#[derive(Clone, Debug)]
pub struct Constraint {
    pub left: VarIdx,
    pub right: VarIdx,
    pub gap: f64,
    /// Upstream leaves `lm` uninitialised; it is only ever read after being
    /// written by `compute_dfdv`.
    pub lm: f64,
    pub time_stamp: i64,
    pub active: bool,
    /// `const` upstream; treat as read-only.
    pub equality: bool,
    pub unsatisfiable: bool,
    pub needs_scaling: bool,
    /// upstream `void *creator`; opaque caller tag.
    pub creator: Option<usize>,
}

/// C++ `ostream << double` (default float formatting: `%g`).
fn g6(v: f64) -> String {
    crate::avoid::router::format_g(v, 6)
}

impl Constraint {
    /// `Constraint: var(<left id>) [+|-] <gap> [==|<=] var(<right id>) `
    /// (`vs`: the variables the constraint refers to).
    // upstream: vpsc.cpp Constraint::toString()
    pub fn to_string(&self, vs: &[Variable]) -> String {
        let mut stream = format!("Constraint: var({}) ", vs[self.left].id);
        if self.gap < 0.0 {
            stream.push_str(&format!("- {} ", g6(-self.gap)));
        } else {
            stream.push_str(&format!("+ {} ", g6(self.gap)));
        }
        stream.push_str(if self.equality { "==" } else { "<=" });
        stream.push_str(&format!(" var({}) ", vs[self.right].id));
        stream
    }

    /// upstream: vpsc.cpp `Constraint::Constraint(left, right, gap,
    /// equality=false)`
    pub fn new(left: VarIdx, right: VarIdx, gap: f64, equality: bool) -> Self {
        Constraint {
            left,
            right,
            gap,
            lm: 0.0,
            time_stamp: 0,
            active: false,
            equality,
            unsatisfiable: false,
            needs_scaling: true,
            creator: None,
        }
    }
}

// ---------------------------------------------------------------------------
// PositionStats / Block / Blocks
// ---------------------------------------------------------------------------

/// upstream: vpsc.h `struct PositionStats`
#[derive(Clone, Debug, Default)]
pub struct PositionStats {
    pub scale: f64,
    pub ab: f64,
    pub ad: f64,
    pub a2: f64,
}

impl PositionStats {
    // upstream: vpsc.cpp PositionStats::addVariable()
    fn add_variable(&mut self, v: &Variable) {
        let ai = self.scale / v.scale;
        let bi = v.offset / v.scale;
        let wi = v.weight;
        self.ab += wi * ai * bi;
        self.ad += wi * ai * v.desired_position;
        self.a2 += wi * ai * ai;
    }
}

/// upstream: vpsc.h `class Block`
#[derive(Clone, Debug)]
struct Block {
    vars: Vec<VarIdx>,
    posn: f64,
    ps: PositionStats,
    deleted: bool,
    time_stamp: i64,
    /// upstream `Heap *in` (nullptr == None).
    in_heap: Option<Vec<ConIdx>>,
    /// upstream `Heap *out` (nullptr == None).
    out_heap: Option<Vec<ConIdx>>,
}

/// upstream: vpsc.h `class Blocks`
#[derive(Clone, Debug, Default)]
struct Blocks {
    /// Every block ever created by this solver (upstream heap allocations).
    /// Slots are never reused.
    arena: Vec<Block>,
    /// upstream `std::vector<Block*> m_blocks`.
    m_blocks: Vec<BlockIdx>,
    nvs: usize,
    block_time_ctr: i64,
}

/// upstream: vpsc.h `struct UnsatisfiableException`
#[derive(Clone, Debug, Default)]
pub struct UnsatisfiableException {
    pub path: Vec<ConIdx>,
}

/// The `throw "Unsatisfied constraint: ..."` at the end of
/// `IncSolver::satisfy()`.  Carries the offending constraint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnsatisfiedConstraint {
    pub constraint: ConIdx,
}

impl std::fmt::Display for UnsatisfiedConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Unsatisfied constraint: #{}", self.constraint)
    }
}

impl std::error::Error for UnsatisfiedConstraint {}

// ---------------------------------------------------------------------------
// Free helpers usable while parts of the solver are borrowed.
// ---------------------------------------------------------------------------

/// Recursive traversals of a block's active constraint tree (the `Block`
/// methods of the same names delegate here).  They recurse on plain
/// slices rather than on `&mut IncSolver`, so the variable and constraint
/// arrays stay in registers across the recursion; with the solver borrowed
/// mutably every access re-loaded `self.vs` / `self.cs` and their lengths,
/// which made these traversals -- the bulk of nudging time when a region
/// has many equality constraints -- about twice as slow as upstream.
/// Statements and floating-point operations are exactly upstream's.
mod block_traversal {
    use super::{dfdv_of, Block, BlockIdx, ConIdx, Constraint, VarIdx, Variable};

    // upstream: vpsc.cpp Block::canFollowLeft()
    #[inline(always)]
    fn can_follow_left(vs: &[Variable], c: &Constraint, this: BlockIdx, last: Option<VarIdx>) -> bool {
        vs[c.left].block == Some(this) && c.active && last != Some(c.left)
    }

    // upstream: vpsc.cpp Block::canFollowRight()
    #[inline(always)]
    fn can_follow_right(vs: &[Variable], c: &Constraint, this: BlockIdx, last: Option<VarIdx>) -> bool {
        vs[c.right].block == Some(this) && c.active && last != Some(c.right)
    }

    // upstream: vpsc.cpp Block::compute_dfdv(v, u, min_lm)
    pub(super) fn compute_dfdv_min(
        vs: &[Variable],
        cs: &mut [Constraint],
        arena: &[Block],
        this: BlockIdx,
        v: VarIdx,
        u: Option<VarIdx>,
        min_lm: &mut Option<ConIdx>,
    ) -> f64 {
        let var = &vs[v];
        let mut dfdv = dfdv_of(vs, arena, v);
        for &c in var.out.iter() {
            if can_follow_right(vs, &cs[c], this, u) {
                let right = cs[c].right;
                let lm = compute_dfdv_min(vs, cs, arena, this, right, Some(v), min_lm);
                cs[c].lm = lm;
                dfdv += cs[c].lm * vs[cs[c].left].scale;
                if !cs[c].equality && (min_lm.is_none() || cs[c].lm < cs[min_lm.unwrap()].lm) {
                    *min_lm = Some(c);
                }
            }
        }
        for &c in var.in_.iter() {
            if can_follow_left(vs, &cs[c], this, u) {
                let left = cs[c].left;
                let lm = -compute_dfdv_min(vs, cs, arena, this, left, Some(v), min_lm);
                cs[c].lm = lm;
                dfdv -= cs[c].lm * vs[cs[c].right].scale;
                if !cs[c].equality && (min_lm.is_none() || cs[c].lm < cs[min_lm.unwrap()].lm) {
                    *min_lm = Some(c);
                }
            }
        }
        dfdv / var.scale
    }

    // upstream: vpsc.cpp Block::compute_dfdv(v, u)
    pub(super) fn compute_dfdv(
        vs: &[Variable],
        cs: &mut [Constraint],
        arena: &[Block],
        this: BlockIdx,
        v: VarIdx,
        u: Option<VarIdx>,
    ) -> f64 {
        let var = &vs[v];
        let mut dfdv = dfdv_of(vs, arena, v);
        for &c in var.out.iter() {
            if can_follow_right(vs, &cs[c], this, u) {
                let right = cs[c].right;
                let lm = compute_dfdv(vs, cs, arena, this, right, Some(v));
                cs[c].lm = lm;
                dfdv += cs[c].lm * vs[cs[c].left].scale;
            }
        }
        for &c in var.in_.iter() {
            if can_follow_left(vs, &cs[c], this, u) {
                let left = cs[c].left;
                let lm = -compute_dfdv(vs, cs, arena, this, left, Some(v));
                cs[c].lm = lm;
                dfdv -= cs[c].lm * vs[cs[c].right].scale;
            }
        }
        dfdv / var.scale
    }

    // upstream: vpsc.cpp Block::split_path()
    #[allow(clippy::too_many_arguments)]
    pub(super) fn split_path(
        vs: &[Variable],
        cs: &[Constraint],
        this: BlockIdx,
        r: VarIdx,
        v: VarIdx,
        u: Option<VarIdx>,
        m: &mut Option<ConIdx>,
        desperation: bool,
    ) -> bool {
        for &c in vs[v].in_.iter() {
            let con = &cs[c];
            if can_follow_left(vs, con, this, u) {
                if con.left == r {
                    if desperation && !con.equality {
                        *m = Some(c);
                    }
                    return true;
                } else if split_path(vs, cs, this, r, con.left, Some(v), m, false) {
                    if desperation && !con.equality && (m.is_none() || con.lm < cs[m.unwrap()].lm) {
                        *m = Some(c);
                    }
                    return true;
                }
            }
        }
        for &c in vs[v].out.iter() {
            let con = &cs[c];
            if can_follow_right(vs, con, this, u) {
                if con.right == r {
                    if !con.equality {
                        *m = Some(c);
                    }
                    return true;
                } else if split_path(vs, cs, this, r, con.right, Some(v), m, false) {
                    if !con.equality && (m.is_none() || con.lm < cs[m.unwrap()].lm) {
                        *m = Some(c);
                    }
                    return true;
                }
            }
        }
        false
    }

    /// Not upstream: walks the active tree exactly as `split_path(r, v, u,
    /// m, false)` does and tells whether that call would find the path to
    /// `r` (`Some`) and set `m` on it (`Some(true)`).  Without desperation
    /// `split_path` only picks non-equality constraints followed rightwards,
    /// which does not depend on the Lagrange multipliers, so this needs no
    /// `compute_dfdv` first.
    pub(super) fn split_path_has_candidate(
        vs: &[Variable],
        cs: &[Constraint],
        this: BlockIdx,
        r: VarIdx,
        v: VarIdx,
        u: Option<VarIdx>,
    ) -> Option<bool> {
        for &c in vs[v].in_.iter() {
            let con = &cs[c];
            if can_follow_left(vs, con, this, u) {
                if con.left == r {
                    return Some(false);
                } else if let Some(found) = split_path_has_candidate(vs, cs, this, r, con.left, Some(v)) {
                    return Some(found);
                }
            }
        }
        for &c in vs[v].out.iter() {
            let con = &cs[c];
            if can_follow_right(vs, con, this, u) {
                if con.right == r {
                    return Some(!con.equality);
                } else if let Some(found) = split_path_has_candidate(vs, cs, this, r, con.right, Some(v)) {
                    return Some(found || !con.equality);
                }
            }
        }
        None
    }

    // upstream: vpsc.cpp Block::reset_active_lm()
    pub(super) fn reset_active_lm(
        vs: &[Variable],
        cs: &mut [Constraint],
        this: BlockIdx,
        v: VarIdx,
        u: Option<VarIdx>,
    ) {
        for &c in vs[v].out.iter() {
            if can_follow_right(vs, &cs[c], this, u) {
                cs[c].lm = 0.0;
                let right = cs[c].right;
                reset_active_lm(vs, cs, this, right, Some(v));
            }
        }
        for &c in vs[v].in_.iter() {
            if can_follow_left(vs, &cs[c], this, u) {
                cs[c].lm = 0.0;
                let left = cs[c].left;
                reset_active_lm(vs, cs, this, left, Some(v));
            }
        }
    }
}

// upstream: vpsc.h Variable::position()
fn position_of(vs: &[Variable], arena: &[Block], v: VarIdx) -> f64 {
    let var = &vs[v];
    let b = &arena[var.block.expect("variable has no block")];
    (b.ps.scale * b.posn + var.offset) / var.scale
}

// upstream: vpsc.h Variable::unscaledPosition()
fn unscaled_position_of(vs: &[Variable], arena: &[Block], v: VarIdx) -> f64 {
    let var = &vs[v];
    let b = &arena[var.block.expect("variable has no block")];
    debug_assert!(b.ps.scale == 1.0);
    debug_assert!(var.scale == 1.0);
    b.posn + var.offset
}

// upstream: vpsc.h Variable::dfdv()
fn dfdv_of(vs: &[Variable], arena: &[Block], v: VarIdx) -> f64 {
    2. * vs[v].weight * (position_of(vs, arena, v) - vs[v].desired_position)
}

// upstream: vpsc.h Constraint::slack()
fn slack_of(vs: &[Variable], cs: &[Constraint], arena: &[Block], c: ConIdx) -> f64 {
    let con = &cs[c];
    if con.unsatisfiable {
        return f64::MAX;
    }
    if con.needs_scaling {
        return vs[con.right].scale * position_of(vs, arena, con.right)
            - con.gap
            - vs[con.left].scale * position_of(vs, arena, con.left);
    }
    debug_assert!(vs[con.left].scale == 1.0);
    debug_assert!(vs[con.right].scale == 1.0);
    unscaled_position_of(vs, arena, con.right) - con.gap - unscaled_position_of(vs, arena, con.left)
}

// upstream: vpsc.cpp CompareConstraints::operator()
// Returns true when `l` orders *below* `r` in the max-heap, i.e. the heap
// top is the constraint with minimum (effective) slack.
fn compare_constraints(
    vs: &[Variable],
    cs: &[Constraint],
    arena: &[Block],
    l: ConIdx,
    r: ConIdx,
) -> bool {
    let lc = &cs[l];
    let rc = &cs[r];
    let l_lb = vs[lc.left].block.expect("no block");
    let l_rb = vs[lc.right].block.expect("no block");
    let r_lb = vs[rc.left].block.expect("no block");
    let r_rb = vs[rc.right].block.expect("no block");
    let sl = if arena[l_lb].time_stamp > lc.time_stamp || l_lb == l_rb {
        -f64::MAX
    } else {
        slack_of(vs, cs, arena, l)
    };
    let sr = if arena[r_lb].time_stamp > rc.time_stamp || r_lb == r_rb {
        -f64::MAX
    } else {
        slack_of(vs, cs, arena, r)
    };
    if sl == sr {
        // arbitrary choice based on id
        if vs[lc.left].id == vs[rc.left].id {
            if vs[lc.right].id < vs[rc.right].id {
                return true;
            }
            return false;
        }
        if vs[lc.left].id < vs[rc.left].id {
            return true;
        }
        return false;
    }
    sl > sr
}

// libstdc++ std::__push_heap()
fn std_push_heap_hole<F: Fn(usize, usize) -> bool>(
    h: &mut [usize],
    mut hole: isize,
    top: isize,
    value: usize,
    comp: &F,
) {
    let mut parent = (hole - 1) / 2;
    while hole > top && comp(h[parent as usize], value) {
        h[hole as usize] = h[parent as usize];
        hole = parent;
        parent = (hole - 1) / 2;
    }
    h[hole as usize] = value;
}

// libstdc++ std::__adjust_heap()
fn std_adjust_heap<F: Fn(usize, usize) -> bool>(
    h: &mut [usize],
    mut hole: isize,
    len: isize,
    value: usize,
    comp: &F,
) {
    let top = hole;
    let mut second = hole;
    while second < (len - 1) / 2 {
        second = 2 * (second + 1);
        if comp(h[second as usize], h[(second - 1) as usize]) {
            second -= 1;
        }
        h[hole as usize] = h[second as usize];
        hole = second;
    }
    if (len & 1) == 0 && second == (len - 2) / 2 {
        second = 2 * (second + 1);
        h[hole as usize] = h[(second - 1) as usize];
        hole = second - 1;
    }
    std_push_heap_hole(h, hole, top, value, comp);
}

/// `std::priority_queue::push` (libstdc++): `c.push_back(x);
/// std::push_heap(c.begin(), c.end(), comp)`.
fn pq_push<F: Fn(usize, usize) -> bool>(h: &mut Vec<usize>, x: usize, comp: &F) {
    h.push(x);
    let len = h.len() as isize;
    std_push_heap_hole(h, len - 1, 0, x, comp);
}

/// `std::priority_queue::pop` (libstdc++): `std::pop_heap(c.begin(),
/// c.end(), comp); c.pop_back()`.
fn pq_pop<F: Fn(usize, usize) -> bool>(h: &mut Vec<usize>, comp: &F) {
    let len = h.len();
    debug_assert!(len > 0);
    if len > 1 {
        let last = len - 1;
        let value = h[last];
        h[last] = h[0];
        std_adjust_heap(&mut h[..last], 0, last as isize, value, comp);
    }
    h.pop();
}

// ---------------------------------------------------------------------------
// IncSolver
// ---------------------------------------------------------------------------

/// upstream: vpsc.h `class IncSolver` (together with the `Blocks`/`Block`
/// machinery it owns).
pub struct IncSolver<'a> {
    pub split_cnt: u32,
    bs: Blocks,
    m: usize,
    cs: &'a mut Vec<Constraint>,
    n: usize,
    vs: &'a mut Vec<Variable>,
    needs_scaling: bool,
    inactive: Vec<ConIdx>,
    violated: Vec<ConIdx>,
    /// Not upstream: bumped whenever a variable changes block or a
    /// constraint changes `active`, i.e. whenever an active tree changes.
    tree_version: u64,
    tree_cache: ActiveTreeCache,
    /// Not upstream: the active constraints of each variable (in no
    /// particular order), maintained where `active` changes.
    active_adj: Vec<Vec<ConIdx>>,
}

/// Not upstream: the active constraint tree of one block, rooted at its
/// first variable, for `IncSolver::split_candidate_between()`.  Valid while
/// `IncSolver::tree_version` equals `version`.
#[derive(Default)]
struct ActiveTreeCache {
    block: Option<BlockIdx>,
    version: u64,
    /// Per variable (`u32::MAX`: not in the tree).
    depth: Vec<u32>,
    parent: Vec<u32>,
    /// The constraint to the parent has this variable on its left (else
    /// the parent), and is not an equality.
    child_is_left: Vec<bool>,
    non_equality: Vec<bool>,
    stack: Vec<VarIdx>,
    /// The variables with a depth set (to reset them on rebuilding).
    members: Vec<VarIdx>,
}

/// Which heap of a block (upstream `Block::in` / `Block::out`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum HeapSel {
    In,
    Out,
}

impl<'a> IncSolver<'a> {
    /// upstream: vpsc.cpp IncSolver::IncSolver(vs, cs)
    pub fn new(vs: &'a mut Vec<Variable>, cs: &'a mut Vec<Constraint>) -> Self {
        let m = cs.len();
        let n = vs.len();
        let mut needs_scaling = false;
        for v in vs.iter_mut() {
            v.in_.clear();
            v.out.clear();

            // Set needsScaling if any variables have a scale other than 1.
            needs_scaling |= v.scale != 1.0;
        }
        for i in 0..m {
            let (l, r) = (cs[i].left, cs[i].right);
            vs[l].out.push(i);
            vs[r].in_.push(i);
            cs[i].needs_scaling = needs_scaling;
        }
        let mut solver = IncSolver {
            split_cnt: 0,
            bs: Blocks::default(),
            m,
            cs,
            n,
            vs,
            needs_scaling,
            inactive: Vec::new(),
            violated: Vec::new(),
            tree_version: 0,
            tree_cache: ActiveTreeCache::default(),
            active_adj: vec![Vec::new(); n],
        };
        solver.blocks_new();

        solver.inactive = (0..m).collect();
        for &i in &solver.inactive {
            solver.cs[i].active = false;
        }
        solver
    }

    /// upstream: vpsc.cpp IncSolver::addConstraint()
    ///
    /// Upstream expects the caller to have appended the constraint to the
    /// `cs` vector it lent to the solver; here the solver appends it to the
    /// borrowed vector itself and returns its index.
    pub fn add_constraint(&mut self, c: Constraint) -> ConIdx {
        let idx = self.cs.len();
        self.cs.push(c);
        self.m += 1;
        self.cs[idx].active = false;
        self.inactive.push(idx);
        let (l, r) = (self.cs[idx].left, self.cs[idx].right);
        self.vs[l].out.push(idx);
        self.vs[r].in_.push(idx);
        self.cs[idx].needs_scaling = self.needs_scaling;
        idx
    }

    /// upstream: vpsc.h IncSolver::getVariables()
    pub fn get_variables(&self) -> &[Variable] {
        self.vs
    }

    /// Read access to the constraints lent to the solver.
    pub fn get_constraints(&self) -> &[Constraint] {
        self.cs
    }

    // upstream: vpsc.cpp IncSolver::copyResult()
    fn copy_result(&mut self) {
        for i in 0..self.vs.len() {
            let p = position_of(self.vs, &self.bs.arena, i);
            self.vs[i].final_position = p;
            debug_assert!(!self.vs[i].final_position.is_nan());
        }
    }

    // upstream: vpsc.cpp IncSolver::constraintGraphIsCyclic()
    // Upstream takes (n, vs[]); here it operates on the first `n` solver
    // variables.  PTR-ORDER: upstream uses std::set<node*>; only set sizes
    // and membership affect the result, so node index order is used.
    fn constraint_graph_is_cyclic(&self, n: usize) -> bool {
        let mut varmap: BTreeMap<VarIdx, usize> = BTreeMap::new();
        let mut graph: Vec<usize> = Vec::new();
        for i in 0..n {
            graph.push(i);
            varmap.insert(i, i);
        }
        // Option<usize>: upstream map::operator[] yields nullptr for a
        // variable not in vs; such entries are never erased.
        let mut node_in: Vec<BTreeSet<Option<usize>>> = vec![BTreeSet::new(); n];
        let mut node_out: Vec<BTreeSet<Option<usize>>> = vec![BTreeSet::new(); n];
        for i in 0..n {
            for &c in &self.vs[i].in_ {
                let l = self.cs[c].left;
                node_in[i].insert(varmap.get(&l).copied());
            }
            for &c in &self.vs[i].out {
                let r = self.cs[c].right;
                node_out[i].insert(varmap.get(&r).copied());
            }
        }
        while !graph.is_empty() {
            let pos = graph.iter().position(|&u| node_in[u].is_empty());
            match pos {
                None => {
                    // cycle found!
                    return true;
                }
                Some(p) => {
                    let u = graph.remove(p);
                    let outs: Vec<Option<usize>> = node_out[u].iter().copied().collect();
                    for v in outs.into_iter().flatten() {
                        node_in[v].remove(&Some(u));
                    }
                }
            }
        }
        false
    }

    // upstream: vpsc.cpp IncSolver::blockGraphIsCyclic()
    // PTR-ORDER: upstream uses std::map<Block*, node*> / std::set<node*>;
    // only sizes and membership affect the result.
    fn block_graph_is_cyclic(&mut self) -> bool {
        let mut bmap: BTreeMap<BlockIdx, usize> = BTreeMap::new();
        let mut graph: Vec<usize> = Vec::new();
        let length = self.bs.m_blocks.len();
        for i in 0..length {
            let b = self.bs.m_blocks[i];
            graph.push(i);
            bmap.insert(b, i);
        }
        let mut node_in: Vec<BTreeSet<Option<usize>>> = vec![BTreeSet::new(); length];
        let mut node_out: Vec<BTreeSet<Option<usize>>> = vec![BTreeSet::new(); length];
        for i in 0..length {
            let b = self.bs.m_blocks[i];
            self.block_set_up_in_constraints(b);
            let mut c = self.block_find_min_in_constraint(b);
            while let Some(ci) = c {
                let l = self.vs[self.cs[ci].left].block.unwrap();
                node_in[bmap[&b]].insert(bmap.get(&l).copied());
                self.block_delete_min_in_constraint(b);
                c = self.block_find_min_in_constraint(b);
            }

            self.block_set_up_out_constraints(b);
            c = self.block_find_min_out_constraint(b);
            while let Some(ci) = c {
                let r = self.vs[self.cs[ci].right].block.unwrap();
                node_out[bmap[&b]].insert(bmap.get(&r).copied());
                self.block_delete_min_out_constraint(b);
                c = self.block_find_min_out_constraint(b);
            }
        }
        while !graph.is_empty() {
            let pos = graph.iter().position(|&u| node_in[u].is_empty());
            match pos {
                None => {
                    // cycle found!
                    return true;
                }
                Some(p) => {
                    let u = graph.remove(p);
                    let outs: Vec<Option<usize>> = node_out[u].iter().copied().collect();
                    for v in outs.into_iter().flatten() {
                        node_in[v].remove(&Some(u));
                    }
                }
            }
        }
        false
    }

    /// upstream: vpsc.cpp IncSolver::solve()
    ///
    /// Returns `Ok(bs->size() != n)`, or `Err` where upstream throws from
    /// `satisfy()`.
    pub fn solve(&mut self) -> Result<bool, UnsatisfiedConstraint> {
        self.satisfy()?;
        let mut lastcost = f64::MAX;
        let mut cost = self.blocks_cost();
        while (lastcost - cost).abs() > 0.0001 {
            self.satisfy()?;
            lastcost = cost;
            cost = self.blocks_cost();
        }
        self.copy_result();
        Ok(self.bs.m_blocks.len() != self.n)
    }

    /// upstream: vpsc.cpp IncSolver::satisfy()
    ///
    /// incremental version of satisfy that allows refinement after blocks
    /// are moved.
    pub fn satisfy(&mut self) -> Result<bool, UnsatisfiedConstraint> {
        self.split_blocks();
        loop {
            let v = match self.most_violated() {
                Some(v) => v,
                None => break,
            };
            if !(self.cs[v].equality
                || ((self.slack(v) < ZERO_UPPERBOUND) && !self.cs[v].active))
            {
                break;
            }
            debug_assert!(!self.cs[v].active);
            let mut lb = self.vs[self.cs[v].left].block.unwrap();
            let mut rb = self.vs[self.cs[v].right].block.unwrap();
            if lb != rb {
                self.block_merge(lb, rb, v);
            } else {
                let (vl, vr) = (self.cs[v].left, self.cs[v].right);
                if self.directed_path_between(lb, vr, vl) {
                    // cycle found, relax the violated, cyclic constraint
                    self.cs[v].unsatisfiable = true;
                    continue;
                }
                // constraint is within block, need to split first
                match self.block_split_between(lb, vl, vr) {
                    Ok((split_constraint, nlb, nrb)) => {
                        lb = nlb;
                        rb = nrb;
                        debug_assert!(!self.cs[split_constraint].active);
                        self.inactive.push(split_constraint);
                    }
                    Err(_) => {
                        // (upstream appends v to the discarded path)
                        self.cs[v].unsatisfiable = true;
                        continue;
                    }
                }
                if self.slack(v) >= 0.0 {
                    debug_assert!(!self.cs[v].active);
                    // v was satisfied by the above split!
                    self.inactive.push(v);
                    self.bs.m_blocks.push(lb);
                    self.bs.m_blocks.push(rb);
                } else {
                    let mb = self.block_merge(lb, rb, v);
                    self.bs.m_blocks.push(mb);
                    // upstream: delete ((lb->deleted) ? lb : rb);  The
                    // deleted block simply stays unreferenced in the arena.
                }
            }
        }
        self.blocks_cleanup();
        let mut active_constraints = false;
        for i in 0..self.m {
            if self.cs[i].active {
                active_constraints = true;
            }
            if self.slack(i) < ZERO_UPPERBOUND {
                return Err(UnsatisfiedConstraint { constraint: i });
            }
        }
        self.copy_result();
        Ok(active_constraints)
    }

    /// upstream: vpsc.cpp IncSolver::moveBlocks()
    pub fn move_blocks(&mut self) {
        let length = self.bs.m_blocks.len();
        for i in 0..length {
            let b = self.bs.m_blocks[i];
            self.block_update_weighted_position(b);
        }
    }

    /// upstream: vpsc.cpp IncSolver::splitBlocks()
    pub fn split_blocks(&mut self) {
        self.move_blocks();
        self.split_cnt = 0;
        // Split each block if necessary on min LM
        let length = self.bs.m_blocks.len();
        for i in 0..length {
            let b = self.bs.m_blocks[i];
            let v = self.block_find_min_lm(b);
            if let Some(v) = v {
                if self.cs[v].lm < LAGRANGIAN_TOLERANCE {
                    debug_assert!(!self.cs[v].equality);
                    self.split_cnt += 1;
                    let b = self.vs[self.cs[v].left].block.unwrap();
                    debug_assert!(self.vs[self.cs[v].left].block == self.vs[self.cs[v].right].block);
                    let (l, r) = self.block_split(b, v);
                    self.block_update_weighted_position(l);
                    self.block_update_weighted_position(r);
                    self.bs.m_blocks.push(l);
                    self.bs.m_blocks.push(r);
                    self.bs.arena[b].deleted = true;
                    debug_assert!(!self.cs[v].active);
                    self.inactive.push(v);
                }
            }
        }
        self.blocks_cleanup();
    }

    // upstream: vpsc.cpp IncSolver::mostViolated()
    // Scan constraint list for the most violated constraint, or the first
    // equality constraint.  (Always operates on `inactive`, the only list
    // upstream passes.)
    fn most_violated(&mut self) -> Option<ConIdx> {
        let mut slack_for_most_violated = f64::MAX;
        let mut most_violated: Option<ConIdx> = None;
        let l_size = self.inactive.len();
        let mut delete_index = l_size;
        for index in 0..l_size {
            let constraint = self.inactive[index];
            let slack = self.slack(constraint);
            if self.cs[constraint].equality || slack < slack_for_most_violated {
                slack_for_most_violated = slack;
                most_violated = Some(constraint);
                delete_index = index;
                if self.cs[constraint].equality {
                    break;
                }
            }
        }
        // Because the constraint list is not order dependent we just
        // move the last element over the deletePoint and resize
        // downwards.  There is always at least 1 element in the
        // vector because of search.
        if delete_index < l_size {
            let mv = most_violated.unwrap();
            if ((slack_for_most_violated < ZERO_UPPERBOUND) && !self.cs[mv].active)
                || self.cs[mv].equality
            {
                self.inactive[delete_index] = self.inactive[l_size - 1];
                self.inactive.truncate(l_size - 1);
            }
        }
        most_violated
    }

    // ----- small accessors ---------------------------------------------

    // upstream: vpsc.h Constraint::slack()
    fn slack(&self, c: ConIdx) -> f64 {
        slack_of(self.vs, self.cs, &self.bs.arena, c)
    }

    // upstream: vpsc.h Variable::position()
    fn position(&self, v: VarIdx) -> f64 {
        position_of(self.vs, &self.bs.arena, v)
    }

    /// upstream: vpsc.h Variable::dfdv()
    pub fn dfdv(&self, v: VarIdx) -> f64 {
        dfdv_of(self.vs, &self.bs.arena, v)
    }

    // ----- debug printing (upstream ostream operators) -------------------
    //
    // Upstream's `ostream << double` prints like `%g` (precision 6).

    /// `os << v`: `(id=position)`, or `(id=desiredPosition)` for a variable
    /// without a block.
    // upstream: vpsc.cpp operator<<(std::ostream &, const Variable &)
    pub fn variable_to_string(&self, v: VarIdx) -> String {
        let var = &self.vs[v];
        if var.block.is_some() {
            format!("({}={})", var.id, g6(self.position(v)))
        } else {
            format!("({}={})", var.id, g6(var.desired_position))
        }
    }

    /// `os << b` for the block `b`.
    // upstream: vpsc.cpp operator<<(ostream &, const Block &)
    fn block_to_string(&self, b: BlockIdx) -> String {
        let block = &self.bs.arena[b];
        let mut os = format!("Block(posn={}):", g6(block.posn));
        for &v in block.vars.iter() {
            os.push(' ');
            os.push_str(&self.variable_to_string(v));
        }
        if block.deleted {
            os.push_str(" Deleted!");
        }
        os
    }

    /// `os << c`.
    // upstream: vpsc.cpp operator<<(std::ostream &, const Constraint &)
    pub fn constraint_to_string(&self, c: ConIdx) -> String {
        let con = &self.cs[c];
        let type_ = if con.equality { "=" } else { "<=" };
        let mut lscale = String::new();
        let mut rscale = String::new();
        if self.vs[con.left].scale != 1.0 {
            lscale = format!("{}*", g6(self.vs[con.left].scale));
        }
        if self.vs[con.right].scale != 1.0 {
            rscale = format!("{}*", g6(self.vs[con.right].scale));
        }
        let mut os = format!(
            "{}{}+{}{}{}{}",
            lscale,
            self.variable_to_string(con.left),
            g6(con.gap),
            type_,
            rscale,
            self.variable_to_string(con.right)
        );
        if self.vs[con.left].block.is_some() && self.vs[con.right].block.is_some() {
            os.push_str(&format!(
                "({}){}(lm={})",
                g6(self.slack(c)),
                if con.active { "-active" } else { "" },
                g6(con.lm)
            ));
        } else {
            os.push_str("(vars have no position)");
        }
        os
    }

    /// Writes every block and every constraint, one per line (useful in
    /// debugging).  Upstream's body exists only under `LIBVPSC_LOGGING`
    /// (appending to its log file), and at this upstream commit that code
    /// does not compile (`LOGFILE` is undefined and `Blocks` has no
    /// `begin()`/`end()`); the port writes what it evidently intends --
    /// the blocks in `Blocks` order, then the constraints -- to `f`.
    // upstream: vpsc.cpp IncSolver::printBlocks()
    pub fn print_blocks(&self, f: &mut dyn std::fmt::Write) -> std::fmt::Result {
        for &b in self.bs.m_blocks.iter() {
            writeln!(f, "  {}", self.block_to_string(b))?;
        }
        for i in 0..self.m {
            writeln!(f, "  {}", self.constraint_to_string(i))?;
        }
        Ok(())
    }

    // ----- heap helpers (upstream Heap = std::priority_queue) ----------

    fn heap_take(&mut self, b: BlockIdx, sel: HeapSel) -> Vec<ConIdx> {
        let h = match sel {
            HeapSel::In => self.bs.arena[b].in_heap.take(),
            HeapSel::Out => self.bs.arena[b].out_heap.take(),
        };
        h.expect("heap not set up")
    }

    fn heap_put(&mut self, b: BlockIdx, sel: HeapSel, h: Vec<ConIdx>) {
        match sel {
            HeapSel::In => self.bs.arena[b].in_heap = Some(h),
            HeapSel::Out => self.bs.arena[b].out_heap = Some(h),
        }
    }

    fn heap_ref(&self, b: BlockIdx, sel: HeapSel) -> &Vec<ConIdx> {
        match sel {
            HeapSel::In => self.bs.arena[b].in_heap.as_ref(),
            HeapSel::Out => self.bs.arena[b].out_heap.as_ref(),
        }
        .expect("heap not set up")
    }

    fn heap_push(&mut self, b: BlockIdx, sel: HeapSel, c: ConIdx) {
        let mut h = self.heap_take(b, sel);
        {
            let (vs, cs, arena) = (&*self.vs, &*self.cs, &self.bs.arena);
            let comp = |x: usize, y: usize| compare_constraints(vs, cs, arena, x, y);
            pq_push(&mut h, c, &comp);
        }
        self.heap_put(b, sel, h);
    }

    fn heap_pop(&mut self, b: BlockIdx, sel: HeapSel) {
        let mut h = self.heap_take(b, sel);
        {
            let (vs, cs, arena) = (&*self.vs, &*self.cs, &self.bs.arena);
            let comp = |x: usize, y: usize| compare_constraints(vs, cs, arena, x, y);
            pq_pop(&mut h, &comp);
        }
        self.heap_put(b, sel, h);
    }

    fn heap_top(&self, b: BlockIdx, sel: HeapSel) -> ConIdx {
        self.heap_ref(b, sel)[0]
    }

    fn heap_empty(&self, b: BlockIdx, sel: HeapSel) -> bool {
        self.heap_ref(b, sel).is_empty()
    }

    // ----- Blocks --------------------------------------------------------

    // upstream: vpsc.cpp Blocks::Blocks(vs)
    fn blocks_new(&mut self) {
        self.bs.block_time_ctr = 0;
        self.bs.nvs = self.vs.len();
        self.bs.m_blocks = Vec::with_capacity(self.bs.nvs);
        for i in 0..self.bs.nvs {
            let b = self.block_new(Some(i));
            self.bs.m_blocks.push(b);
        }
    }

    // upstream: vpsc.cpp Blocks::totalOrder()
    // returns a list of variables with total ordering determined by the
    // constraint DAG
    fn blocks_total_order(&mut self) -> VecDeque<VarIdx> {
        let mut order = VecDeque::new();
        for i in 0..self.bs.nvs {
            self.vs[i].visited = false;
        }
        for i in 0..self.bs.nvs {
            if self.vs[i].in_.is_empty() {
                self.blocks_dfs_visit(i, &mut order);
            }
        }
        order
    }

    // upstream: vpsc.cpp Blocks::dfsVisit()
    fn blocks_dfs_visit(&mut self, v: VarIdx, order: &mut VecDeque<VarIdx>) {
        self.vs[v].visited = true;
        let len = self.vs[v].out.len();
        for k in 0..len {
            let c = self.vs[v].out[k];
            let r = self.cs[c].right;
            if !self.vs[r].visited {
                self.blocks_dfs_visit(r, order);
            }
        }
        order.push_front(v);
    }

    // upstream: vpsc.cpp Blocks::mergeLeft()
    // Processes incoming constraints, most violated to least, merging with
    // the neighbouring (left) block until no more violated constraints are
    // found
    fn blocks_merge_left(&mut self, mut r: BlockIdx) {
        self.bs.block_time_ctr += 1;
        self.bs.arena[r].time_stamp = self.bs.block_time_ctr;
        self.block_set_up_in_constraints(r);
        let mut c = self.block_find_min_in_constraint(r);
        while let Some(ci) = c {
            if !(self.slack(ci) < 0.0) {
                break;
            }
            self.block_delete_min_in_constraint(r);
            let mut l = self.vs[self.cs[ci].left].block.unwrap();
            if self.bs.arena[l].in_heap.is_none() {
                self.block_set_up_in_constraints(l);
            }
            let con = &self.cs[ci];
            let mut dist = self.vs[con.right].offset - self.vs[con.left].offset - con.gap;
            if self.bs.arena[r].vars.len() < self.bs.arena[l].vars.len() {
                dist = -dist;
                std::mem::swap(&mut l, &mut r);
            }
            self.bs.block_time_ctr += 1;
            self.block_merge_dist(r, l, ci, dist);
            self.block_merge_in(r, l);
            self.bs.arena[r].time_stamp = self.bs.block_time_ctr;
            self.blocks_remove_block(l);
            c = self.block_find_min_in_constraint(r);
        }
    }

    // upstream: vpsc.cpp Blocks::mergeRight()
    // Symmetrical to mergeLeft
    fn blocks_merge_right(&mut self, mut l: BlockIdx) {
        self.block_set_up_out_constraints(l);
        let mut c = self.block_find_min_out_constraint(l);
        while let Some(ci) = c {
            if !(self.slack(ci) < 0.0) {
                break;
            }
            self.block_delete_min_out_constraint(l);
            let mut r = self.vs[self.cs[ci].right].block.unwrap();
            self.block_set_up_out_constraints(r);
            let con = &self.cs[ci];
            let mut dist = self.vs[con.left].offset + con.gap - self.vs[con.right].offset;
            if self.bs.arena[l].vars.len() > self.bs.arena[r].vars.len() {
                dist = -dist;
                std::mem::swap(&mut l, &mut r);
            }
            self.block_merge_dist(l, r, ci, dist);
            self.block_merge_out(l, r);
            self.blocks_remove_block(r);
            c = self.block_find_min_out_constraint(l);
        }
    }

    // upstream: vpsc.cpp Blocks::removeBlock()
    fn blocks_remove_block(&mut self, doomed: BlockIdx) {
        self.bs.arena[doomed].deleted = true;
    }

    // upstream: vpsc.cpp Blocks::cleanup()
    // Clears up deleted blocks from the blocks list (order preserving).
    fn blocks_cleanup(&mut self) {
        let arena = &self.bs.arena;
        self.bs.m_blocks.retain(|&b| !arena[b].deleted);
    }

    // upstream: vpsc.cpp Blocks::split()
    // Splits block b across constraint c into two new blocks, l and r (c's
    // left and right sides respectively)
    fn blocks_split(&mut self, b: BlockIdx, c: ConIdx) -> (BlockIdx, BlockIdx) {
        let (l, mut r) = self.block_split(b, c);
        self.bs.m_blocks.push(l);
        self.bs.m_blocks.push(r);
        self.bs.arena[r].posn = self.bs.arena[b].posn;
        self.blocks_merge_left(l);
        // r may have been merged!
        r = self.vs[self.cs[c].right].block.unwrap();
        self.block_update_weighted_position(r);
        self.blocks_merge_right(r);
        self.blocks_remove_block(b);

        debug_assert!(!self.bs.arena[l].posn.is_nan());
        debug_assert!(!self.bs.arena[r].posn.is_nan());
        (l, r)
    }

    // upstream: vpsc.cpp Blocks::cost()
    // returns the cost total squared distance of variables from their
    // desired positions
    fn blocks_cost(&self) -> f64 {
        let mut c = 0.0;
        for &b in &self.bs.m_blocks {
            c += self.block_cost(b);
        }
        c
    }

    // ----- Block ---------------------------------------------------------

    // upstream: vpsc.cpp Block::Block(blocks, v)
    fn block_new(&mut self, v: Option<VarIdx>) -> BlockIdx {
        let b = self.bs.arena.len();
        self.bs.arena.push(Block {
            vars: Vec::new(),
            posn: 0.0,
            ps: PositionStats::default(),
            deleted: false,
            time_stamp: 0,
            in_heap: None,
            out_heap: None,
        });
        if let Some(v) = v {
            self.vs[v].offset = 0.0;
            self.block_add_variable(b, v);
        }
        b
    }

    // upstream: vpsc.cpp Block::addVariable()
    fn block_add_variable(&mut self, b: BlockIdx, v: VarIdx) {
        self.tree_version += 1;
        self.vs[v].block = Some(b);
        let blk = &mut self.bs.arena[b];
        blk.vars.push(v);
        if blk.ps.a2 == 0.0 {
            blk.ps.scale = self.vs[v].scale;
        }
        blk.ps.add_variable(&self.vs[v]);
        blk.posn = (blk.ps.ad - blk.ps.ab) / blk.ps.a2;
        debug_assert!(!blk.posn.is_nan());
    }

    // upstream: vpsc.cpp Block::updateWeightedPosition()
    fn block_update_weighted_position(&mut self, b: BlockIdx) {
        let blk = &mut self.bs.arena[b];
        blk.ps.ab = 0.0;
        blk.ps.ad = 0.0;
        blk.ps.a2 = 0.0;
        for &v in &blk.vars {
            blk.ps.add_variable(&self.vs[v]);
        }
        blk.posn = (blk.ps.ad - blk.ps.ab) / blk.ps.a2;
        debug_assert!(!blk.posn.is_nan());
    }

    // upstream: vpsc.cpp Block::setUpInConstraints()
    fn block_set_up_in_constraints(&mut self, b: BlockIdx) {
        self.block_set_up_constraint_heap(b, true);
    }

    // upstream: vpsc.cpp Block::setUpOutConstraints()
    fn block_set_up_out_constraints(&mut self, b: BlockIdx) {
        self.block_set_up_constraint_heap(b, false);
    }

    // upstream: vpsc.cpp Block::setUpConstraintHeap()
    fn block_set_up_constraint_heap(&mut self, b: BlockIdx, in_: bool) {
        let sel = if in_ { HeapSel::In } else { HeapSel::Out };
        self.heap_put(b, sel, Vec::new());
        let nvars = self.bs.arena[b].vars.len();
        for i in 0..nvars {
            let v = self.bs.arena[b].vars[i];
            let ncs = if in_ { self.vs[v].in_.len() } else { self.vs[v].out.len() };
            for j in 0..ncs {
                let c = if in_ { self.vs[v].in_[j] } else { self.vs[v].out[j] };
                self.cs[c].time_stamp = self.bs.block_time_ctr;
                let lblock = self.vs[self.cs[c].left].block.unwrap();
                let rblock = self.vs[self.cs[c].right].block.unwrap();
                if (lblock != b && in_) || (rblock != b && !in_) {
                    self.heap_push(b, sel, c);
                }
            }
        }
    }

    // upstream: vpsc.cpp Block::merge(Block *b, Constraint *c)
    fn block_merge(&mut self, this: BlockIdx, b: BlockIdx, c: ConIdx) -> BlockIdx {
        let con = &self.cs[c];
        let dist = self.vs[con.right].offset - self.vs[con.left].offset - con.gap;
        let l = self.vs[con.left].block.unwrap();
        let r = self.vs[con.right].block.unwrap();
        if self.bs.arena[l].vars.len() < self.bs.arena[r].vars.len() {
            self.block_merge_dist(r, l, c, dist);
        } else {
            self.block_merge_dist(l, r, c, -dist);
        }
        if self.bs.arena[b].deleted {
            this
        } else {
            b
        }
    }

    // upstream: vpsc.cpp Block::merge(Block *b, Constraint *c, double dist)
    // Merges b into this block across c.  Can be either a right merge or a
    // left merge.
    fn block_merge_dist(&mut self, this: BlockIdx, b: BlockIdx, c: ConIdx, dist: f64) {
        self.tree_version += 1;
        debug_assert!(!self.cs[c].active);
        self.cs[c].active = true;
        let (l, r) = (self.cs[c].left, self.cs[c].right);
        self.active_adj[l].push(c);
        self.active_adj[r].push(c);
        let bvars = self.bs.arena[b].vars.clone();
        for v in bvars {
            self.vs[v].offset += dist;
            self.block_add_variable(this, v);
        }
        let blk = &mut self.bs.arena[this];
        blk.posn = (blk.ps.ad - blk.ps.ab) / blk.ps.a2;
        debug_assert!(!blk.posn.is_nan());
        self.bs.arena[b].deleted = true;
    }

    // upstream: vpsc.cpp Block::mergeIn()
    fn block_merge_in(&mut self, this: BlockIdx, b: BlockIdx) {
        // We check the top of the heaps to remove possible internal
        // constraints
        self.block_find_min_in_constraint(this);
        self.block_find_min_in_constraint(b);
        while !self.heap_empty(b, HeapSel::In) {
            let top = self.heap_top(b, HeapSel::In);
            self.heap_push(this, HeapSel::In, top);
            self.heap_pop(b, HeapSel::In);
        }
    }

    // upstream: vpsc.cpp Block::mergeOut()
    fn block_merge_out(&mut self, this: BlockIdx, b: BlockIdx) {
        self.block_find_min_out_constraint(this);
        self.block_find_min_out_constraint(b);
        while !self.heap_empty(b, HeapSel::Out) {
            let top = self.heap_top(b, HeapSel::Out);
            self.heap_push(this, HeapSel::Out, top);
            self.heap_pop(b, HeapSel::Out);
        }
    }

    // upstream: vpsc.cpp Block::findMinInConstraint()
    fn block_find_min_in_constraint(&mut self, this: BlockIdx) -> Option<ConIdx> {
        let mut out_of_date: Vec<ConIdx> = Vec::new();
        while !self.heap_empty(this, HeapSel::In) {
            let v = self.heap_top(this, HeapSel::In);
            let lb = self.vs[self.cs[v].left].block.unwrap();
            let rb = self.vs[self.cs[v].right].block.unwrap();
            // rb may not be this if called between merge and mergeIn
            if lb == rb {
                // constraint has been merged into the same block
                self.heap_pop(this, HeapSel::In);
            } else if self.cs[v].time_stamp < self.bs.arena[lb].time_stamp {
                // block at other end of constraint has been moved since this
                self.heap_pop(this, HeapSel::In);
                out_of_date.push(v);
            } else {
                break;
            }
        }
        for v in out_of_date {
            self.cs[v].time_stamp = self.bs.block_time_ctr;
            self.heap_push(this, HeapSel::In, v);
        }
        if self.heap_empty(this, HeapSel::In) {
            None
        } else {
            Some(self.heap_top(this, HeapSel::In))
        }
    }

    // upstream: vpsc.cpp Block::findMinOutConstraint()
    fn block_find_min_out_constraint(&mut self, this: BlockIdx) -> Option<ConIdx> {
        if self.heap_empty(this, HeapSel::Out) {
            return None;
        }
        let mut v = self.heap_top(this, HeapSel::Out);
        while self.vs[self.cs[v].left].block == self.vs[self.cs[v].right].block {
            self.heap_pop(this, HeapSel::Out);
            if self.heap_empty(this, HeapSel::Out) {
                return None;
            }
            v = self.heap_top(this, HeapSel::Out);
        }
        Some(v)
    }

    // upstream: vpsc.cpp Block::deleteMinInConstraint()
    fn block_delete_min_in_constraint(&mut self, this: BlockIdx) {
        self.heap_pop(this, HeapSel::In);
    }

    // upstream: vpsc.cpp Block::deleteMinOutConstraint()
    fn block_delete_min_out_constraint(&mut self, this: BlockIdx) {
        self.heap_pop(this, HeapSel::Out);
    }

    // upstream: vpsc.cpp Block::canFollowLeft()
    fn can_follow_left(&self, this: BlockIdx, c: ConIdx, last: Option<VarIdx>) -> bool {
        let con = &self.cs[c];
        self.vs[con.left].block == Some(this) && con.active && last != Some(con.left)
    }

    // upstream: vpsc.cpp Block::canFollowRight()
    fn can_follow_right(&self, this: BlockIdx, c: ConIdx, last: Option<VarIdx>) -> bool {
        let con = &self.cs[c];
        self.vs[con.right].block == Some(this) && con.active && last != Some(con.right)
    }

    // upstream: vpsc.cpp Block::compute_dfdv(v, u, min_lm)
    // computes the derivative of v and the lagrange multipliers of v's out
    // constraints (as the recursive sum of those below.  Does not
    // backtrack over u.  also records the constraint with minimum lagrange
    // multiplier in min_lm
    fn block_compute_dfdv_min(
        &mut self,
        this: BlockIdx,
        v: VarIdx,
        u: Option<VarIdx>,
        min_lm: &mut Option<ConIdx>,
    ) -> f64 {
        // (Recursion on split borrows of the solver's slices: the hot
        // traversals of the active constraint tree, see block_traversal.)
        block_traversal::compute_dfdv_min(&*self.vs, &mut *self.cs, &self.bs.arena, this, v, u, min_lm)
    }

    // upstream: vpsc.cpp Block::compute_dfdv(v, u)
    fn block_compute_dfdv(&mut self, this: BlockIdx, v: VarIdx, u: Option<VarIdx>) -> f64 {
        block_traversal::compute_dfdv(&*self.vs, &mut *self.cs, &self.bs.arena, this, v, u)
    }

    // upstream: vpsc.cpp Block::split_path()
    // The top level v and r are variables between which we want to find
    // the constraint with the smallest lm.  We also do not consider
    // equality constraints as potential split points.
    fn block_split_path(
        &self,
        this: BlockIdx,
        r: VarIdx,
        v: VarIdx,
        u: Option<VarIdx>,
        m: &mut Option<ConIdx>,
        desperation: bool,
    ) -> bool {
        block_traversal::split_path(&*self.vs, &*self.cs, this, r, v, u, m, desperation)
    }

    // upstream: vpsc.cpp Block::reset_active_lm()
    // resets LMs for all active constraints to 0 by traversing active
    // constraint tree starting from v, not back tracking over u
    fn block_reset_active_lm(&mut self, this: BlockIdx, v: VarIdx, u: Option<VarIdx>) {
        block_traversal::reset_active_lm(&*self.vs, &mut *self.cs, this, v, u);
    }

    // upstream: vpsc.cpp Block::list_active() is logging-only (its body is
    // entirely under LIBVPSC_LOGGING apart from the traversal) and is not
    // ported.

    // upstream: vpsc.cpp Block::findMinLM()
    // finds the constraint with the minimum lagrange multiplier, that is,
    // the constraint that most wants to split
    fn block_find_min_lm(&mut self, this: BlockIdx) -> Option<ConIdx> {
        let mut min_lm: Option<ConIdx> = None;
        let front = self.bs.arena[this].vars[0];
        self.block_reset_active_lm(this, front, None);
        self.block_compute_dfdv_min(this, front, None, &mut min_lm);
        min_lm
    }

    // upstream: vpsc.cpp Block::findMinLMBetween()
    fn block_find_min_lm_between(
        &mut self,
        this: BlockIdx,
        lv: VarIdx,
        rv: VarIdx,
    ) -> Result<ConIdx, UnsatisfiableException> {
        // (not upstream) When split_path() will find no constraint, which
        // is by far the common case where constraints are unsatisfiable,
        // the multipliers it would have compared are not computed.  They
        // are scratch values: every reader (findMinLM(), and this function)
        // recomputes them for the whole block first.
        if self.split_candidate_between(this, lv, rv) != Some(true) {
            return Err(UnsatisfiableException::default());
        }
        let front = self.bs.arena[this].vars[0];
        self.block_reset_active_lm(this, front, None);
        self.block_compute_dfdv(this, front, None);
        let mut min_lm: Option<ConIdx> = None;
        self.block_split_path(this, rv, lv, None, &mut min_lm, false);
        match min_lm {
            // Upstream fills the exception's path with
            // getActivePathBetween(); its only catcher, satisfy(), discards
            // it, so the (read-only) walk is not made here.
            None => Err(UnsatisfiableException::default()),
            Some(c) => Ok(c),
        }
    }

    /// Not upstream: `block_traversal::split_path_has_candidate(.., rv, lv,
    /// None)`, answered from the block's cached active tree.  The active
    /// constraints of a block form a tree (a constraint is only activated
    /// when it merges two blocks), so the path the walk finds is the tree
    /// path, whatever the order it explores in.  Repeated queries on an
    /// unchanged block (`satisfy()` meeting unsatisfiable constraints one
    /// after another) then cost the path length instead of a walk over the
    /// block's constraint lists.
    fn split_candidate_between(&mut self, this: BlockIdx, lv: VarIdx, rv: VarIdx) -> Option<bool> {
        // (lv == rv: the walk finds no path at all; left to it)
        let fast = if lv == rv { None } else { self.split_candidate_from_tree(this, lv, rv) };
        let result = match fast {
            Some(r) => Some(r),
            // A variable outside the tree: the walk decides.
            None => block_traversal::split_path_has_candidate(&*self.vs, &*self.cs, this, rv, lv, None),
        };
        debug_assert_eq!(
            result,
            block_traversal::split_path_has_candidate(&*self.vs, &*self.cs, this, rv, lv, None)
        );
        result
    }

    fn split_candidate_from_tree(&mut self, this: BlockIdx, lv: VarIdx, rv: VarIdx) -> Option<bool> {
        // split_path() picks a constraint walked from its left variable to
        // its right one.
        self.walk_tree_path(this, lv, rv, |child_is_left, non_equality, upwards| {
            non_equality && (child_is_left == upwards)
        })
        .map(|(any, _)| any)
    }

    /// Not upstream: `block_is_active_directed_path_between(this, u, v)`
    /// from the cached active tree (see `split_candidate_between()`): the
    /// walk follows constraints from left to right only, so it reaches `v`
    /// exactly when every constraint of the tree path is walked that way.
    fn directed_path_between(&mut self, this: BlockIdx, u: VarIdx, v: VarIdx) -> bool {
        let fast = self
            .walk_tree_path(this, u, v, |child_is_left, _, upwards| child_is_left == upwards)
            .map(|(_, all)| all);
        let result = match fast {
            Some(r) => r,
            None => self.block_is_active_directed_path_between(this, u, v),
        };
        debug_assert_eq!(result, self.block_is_active_directed_path_between(this, u, v));
        result
    }

    /// Walks the tree path from `from` to `to` in the cached active tree of
    /// `this` (rebuilt if stale), applying `pred(child_is_left,
    /// non_equality, upwards)` to each constraint on it, `upwards` telling
    /// whether it is walked from child to parent.  Returns whether `pred`
    /// held for any and for all of them, or None if either variable is not
    /// in the tree.
    fn walk_tree_path(
        &mut self,
        this: BlockIdx,
        from: VarIdx,
        to: VarIdx,
        pred: impl Fn(bool, bool, bool) -> bool,
    ) -> Option<(bool, bool)> {
        self.build_active_tree(this);
        let t = &self.tree_cache;
        if t.depth[from] == u32::MAX || t.depth[to] == u32::MAX {
            return None;
        }
        let (mut a, mut b) = (from, to);
        let (mut any, mut all) = (false, true);
        let mut step = |x: usize, upwards: bool| {
            let r = pred(t.child_is_left[x], t.non_equality[x], upwards);
            any |= r;
            all &= r;
        };
        while t.depth[a] > t.depth[b] {
            step(a, true);
            a = t.parent[a] as usize;
        }
        while t.depth[b] > t.depth[a] {
            step(b, false);
            b = t.parent[b] as usize;
        }
        while a != b {
            step(a, true);
            step(b, false);
            a = t.parent[a] as usize;
            b = t.parent[b] as usize;
        }
        Some((any, all))
    }

    /// Makes `tree_cache` hold the active tree of `this`.
    fn build_active_tree(&mut self, this: BlockIdx) {
        let n = self.vs.len();
        let t = &mut self.tree_cache;
        if t.block != Some(this) || t.version != self.tree_version || t.depth.len() != n {
            t.block = Some(this);
            t.version = self.tree_version;
            if t.depth.len() != n {
                t.depth = vec![u32::MAX; n];
                t.parent = vec![u32::MAX; n];
                t.child_is_left = vec![false; n];
                t.non_equality = vec![false; n];
            } else {
                for &v in &t.members {
                    t.depth[v] = u32::MAX;
                }
            }
            t.members.clear();
            let root = self.bs.arena[this].vars[0];
            t.depth[root] = 0;
            t.members.push(root);
            t.parent[root] = u32::MAX;
            t.stack.clear();
            t.stack.push(root);
            while let Some(v) = t.stack.pop() {
                let d = t.depth[v] + 1;
                for &c in self.active_adj[v].iter() {
                    let con = &self.cs[c];
                    debug_assert!(con.active);
                    let child_is_left = con.right == v;
                    let w = if child_is_left { con.left } else { con.right };
                    if self.vs[w].block == Some(this) && t.depth[w] == u32::MAX {
                        t.depth[w] = d;
                        t.parent[w] = v as u32;
                        t.child_is_left[w] = child_is_left;
                        t.non_equality[w] = !con.equality;
                        t.stack.push(w);
                        t.members.push(w);
                    }
                }
            }
        }
    }

    // upstream: vpsc.cpp Block::populateSplitBlock()
    // populates block b by traversing the active constraint tree adding
    // variables as they're visited.  Starts from variable v and does not
    // backtrack over variable u.
    fn block_populate_split_block(
        &mut self,
        this: BlockIdx,
        b: BlockIdx,
        v: VarIdx,
        u: Option<VarIdx>,
    ) {
        self.block_add_variable(b, v);
        let nin = self.vs[v].in_.len();
        for k in 0..nin {
            let c = self.vs[v].in_[k];
            if self.can_follow_left(this, c, u) {
                let left = self.cs[c].left;
                self.block_populate_split_block(this, b, left, Some(v));
            }
        }
        let nout = self.vs[v].out.len();
        for k in 0..nout {
            let c = self.vs[v].out[k];
            if self.can_follow_right(this, c, u) {
                let right = self.cs[c].right;
                self.block_populate_split_block(this, b, right, Some(v));
            }
        }
    }

    // upstream: vpsc.cpp Block::getActivePathBetween()
    // Returns the active path between variables u and v... not back
    // tracking over w
    fn block_get_active_path_between(
        &self,
        this: BlockIdx,
        path: &mut Vec<ConIdx>,
        u: VarIdx,
        v: VarIdx,
        w: Option<VarIdx>,
    ) -> bool {
        if u == v {
            return true;
        }
        for &c in &self.vs[u].in_ {
            if self.can_follow_left(this, c, w)
                && self.block_get_active_path_between(this, path, self.cs[c].left, v, Some(u))
            {
                path.push(c);
                return true;
            }
        }
        for &c in &self.vs[u].out {
            if self.can_follow_right(this, c, w)
                && self.block_get_active_path_between(this, path, self.cs[c].right, v, Some(u))
            {
                path.push(c);
                return true;
            }
        }
        false
    }

    // upstream: vpsc.cpp Block::isActiveDirectedPathBetween()
    // Search active constraint tree from u to see if there is a directed
    // path to v.
    fn block_is_active_directed_path_between(&self, this: BlockIdx, u: VarIdx, v: VarIdx) -> bool {
        if u == v {
            return true;
        }
        for &c in &self.vs[u].out {
            if self.can_follow_right(this, c, None)
                && self.block_is_active_directed_path_between(this, self.cs[c].right, v)
            {
                return true;
            }
        }
        false
    }

    // upstream: vpsc.cpp Block::getActiveDirectedPathBetween()
    fn block_get_active_directed_path_between(
        &self,
        this: BlockIdx,
        path: &mut Vec<ConIdx>,
        u: VarIdx,
        v: VarIdx,
    ) -> bool {
        if u == v {
            return true;
        }
        for &c in &self.vs[u].out {
            if self.can_follow_right(this, c, None)
                && self.block_get_active_directed_path_between(this, path, self.cs[c].right, v)
            {
                path.push(c);
                return true;
            }
        }
        false
    }

    // upstream: vpsc.cpp Block::splitBetween()
    // Block needs to be split because of a violated constraint between vl
    // and vr.  Returns the split constraint and the new (lb, rb) blocks.
    // Upstream returns nullptr only if findMinLMBetween returned nullptr,
    // which cannot happen (it throws instead), so that path is folded into
    // the Err case.
    fn block_split_between(
        &mut self,
        this: BlockIdx,
        vl: VarIdx,
        vr: VarIdx,
    ) -> Result<(ConIdx, BlockIdx, BlockIdx), UnsatisfiableException> {
        let c = self.block_find_min_lm_between(this, vl, vr)?;
        let (lb, rb) = self.block_split(this, c);
        self.bs.arena[this].deleted = true;
        Ok((c, lb, rb))
    }

    // upstream: vpsc.cpp Block::split()
    // Creates two new blocks, l and r, and splits this block across
    // constraint c, placing the left subtree of constraints (and associated
    // variables) into l and the right into r.
    fn block_split(&mut self, this: BlockIdx, c: ConIdx) -> (BlockIdx, BlockIdx) {
        self.tree_version += 1;
        if self.cs[c].active {
            let (l, r) = (self.cs[c].left, self.cs[c].right);
            for v in [l, r] {
                let list = &mut self.active_adj[v];
                let pos = list.iter().position(|&x| x == c).expect("active constraint not listed");
                list.swap_remove(pos);
            }
        }
        self.cs[c].active = false;
        let (cl, cr) = (self.cs[c].left, self.cs[c].right);
        let l = self.block_new(None);
        self.block_populate_split_block(this, l, cl, Some(cr));
        let r = self.block_new(None);
        self.block_populate_split_block(this, r, cr, Some(cl));
        (l, r)
    }

    // upstream: vpsc.cpp Block::cost()
    // Computes the cost (squared euclidean distance from desired positions)
    // of the current positions for variables in this block
    fn block_cost(&self, b: BlockIdx) -> f64 {
        let mut c = 0.0;
        for &v in &self.bs.arena[b].vars {
            let diff = self.position(v) - self.vs[v].desired_position;
            c += self.vs[v].weight * diff * diff;
        }
        c
    }
}

// ---------------------------------------------------------------------------
// constraintsRemovingRedundantEqualities
// ---------------------------------------------------------------------------

// upstream: vpsc.cpp class EqualityConstraintSet
// PTR-ORDER: upstream keys std::map<Variable*, double> by pointer; only
// membership and per-key values are observable, so variable index order is
// used.
struct EqualityConstraintSet {
    /// upstream `std::list<std::map<Variable*, double>> variableGroups`.
    variable_groups: Vec<BTreeMap<VarIdx, f64>>,
}

impl EqualityConstraintSet {
    // upstream: vpsc.cpp EqualityConstraintSet::EqualityConstraintSet()
    fn new(n_vars: usize) -> Self {
        let mut variable_groups = Vec::with_capacity(n_vars);
        for i in 0..n_vars {
            let mut var_set = BTreeMap::new();
            var_set.insert(i, 0.0);
            variable_groups.push(var_set);
        }
        EqualityConstraintSet { variable_groups }
    }

    // upstream: vpsc.cpp EqualityConstraintSet::isRedundant()
    fn is_redundant(&self, lhs: VarIdx, rhs: VarIdx, sep: f64) -> bool {
        let lhs_set = self.set_for_var(lhs);
        let rhs_set = self.set_for_var(rhs);
        if lhs_set == rhs_set {
            if let Some(s) = lhs_set {
                let g = &self.variable_groups[s];
                // Check if this is a redundant constraint.
                if ((g[&lhs] + sep) - g[&rhs]).abs() < 0.0001 {
                    return true;
                }
            }
        }
        false
    }

    // upstream: vpsc.cpp EqualityConstraintSet::mergeSets()
    fn merge_sets(&mut self, lhs: VarIdx, rhs: VarIdx, sep: f64) {
        let lhs_set = self.set_for_var(lhs);
        let rhs_set = self.set_for_var(rhs);
        if lhs_set == rhs_set {
            return;
        }
        let (ls, rs) = (lhs_set.unwrap(), rhs_set.unwrap());

        let rhs_old_offset = self.variable_groups[rs][&rhs];
        let rhs_new_offset = self.variable_groups[ls][&lhs] + sep;
        let offset = rhs_new_offset - rhs_old_offset;

        // Update offsets
        for val in self.variable_groups[rs].values_mut() {
            *val += offset;
        }

        // Merge rhsSet into lhsSet (std::map::insert does not overwrite).
        let moved: Vec<(VarIdx, f64)> =
            self.variable_groups[rs].iter().map(|(&k, &v)| (k, v)).collect();
        for (k, v) in moved {
            self.variable_groups[ls].entry(k).or_insert(v);
        }
        self.variable_groups.remove(rs);
    }

    // upstream: vpsc.cpp EqualityConstraintSet::setForVar()
    fn set_for_var(&self, var: VarIdx) -> Option<usize> {
        self.variable_groups.iter().position(|g| g.contains_key(&var))
    }
}

/// upstream: vpsc.cpp constraintsRemovingRedundantEqualities()
///
/// Upstream takes the `Variables` and `Constraints` vectors and returns a
/// new `Constraints` vector; here the result is the list of indices (into
/// `constraints`) that are kept, in order.
pub fn constraints_removing_redundant_equalities(
    vars: &[Variable],
    constraints: &[Constraint],
) -> Vec<ConIdx> {
    let mut equality_sets = EqualityConstraintSet::new(vars.len());
    let mut cs = Vec::with_capacity(constraints.len());

    for (i, c) in constraints.iter().enumerate() {
        if c.equality {
            if !equality_sets.is_redundant(c.left, c.right, c.gap) {
                // Only add non-redundant equalities
                equality_sets.merge_sets(c.left, c.right, c.gap);
                cs.push(i);
            }
        } else {
            // Add all non-equalities
            cs.push(i);
        }
    }
    cs
}

// ---------------------------------------------------------------------------
// Differential test against the C++ oracle (tools/oracle/vpsc_oracle.cpp).
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write as _;

    // Must match `struct Lcg` in tools/oracle/vpsc_oracle.cpp exactly.
    struct Lcg {
        state: u64,
    }

    impl Lcg {
        fn new(seed: u64) -> Self {
            Lcg { state: seed }
        }
        fn next(&mut self) -> u32 {
            self.state = self
                .state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (self.state >> 33) as u32
        }
        fn below(&mut self, n: u32) -> u32 {
            self.next() % n
        }
    }

    const FREE_WEIGHT: f64 = 0.00001;
    const STRONG_WEIGHT: f64 = 0.001;
    const STRONGER_WEIGHT: f64 = 1.0;
    const FIXED_WEIGHT: f64 = 100000.0;

    fn hex(x: f64) -> String {
        format!("{:016x}", x.to_bits())
    }

    fn dump_state(out: &mut String, vs: &[Variable], cs: &[Constraint]) {
        for (i, v) in vs.iter().enumerate() {
            writeln!(out, "v {} {}", i, hex(v.final_position)).unwrap();
        }
        for (j, c) in cs.iter().enumerate() {
            writeln!(out, "c {} {} {}", j, c.unsatisfiable as i32, c.active as i32).unwrap();
        }
    }

    fn print_solve(out: &mut String, r: Result<bool, UnsatisfiedConstraint>, split_cnt: u32) {
        match r {
            Ok(b) => writeln!(out, "solve {} {}", b as i32, split_cnt).unwrap(),
            Err(_) => writeln!(out, "solve throw").unwrap(),
        }
    }

    /// Extra checks of the code paths not reached from solve() (heap logic,
    /// Blocks::split, mergeLeft/Right, cycle checks, totalOrder), run on a
    /// solver after solving.  Mirrors `extras()` in the C++ driver.
    fn extras(out: &mut String, s: &mut IncSolver<'_>) {
        if PRINT.with(|p| p.get()) {
            // `--print` lines of the C++ driver: printBlocks() and
            // Constraint::toString().
            let mut blocks = String::new();
            s.print_blocks(&mut blocks).unwrap();
            for line in blocks.lines() {
                writeln!(out, "p {}", line).unwrap();
            }
            for c in s.cs.iter() {
                writeln!(out, "p {}", c.to_string(s.vs)).unwrap();
            }
        }
        let n = s.vs.len();
        let cg = s.constraint_graph_is_cyclic(n);
        let bg = s.block_graph_is_cyclic();
        writeln!(out, "cyc {} {}", cg as i32, bg as i32).unwrap();
        let order = s.blocks_total_order();
        let mut line = String::from("order");
        for v in order {
            write!(line, " {}", v).unwrap();
        }
        writeln!(out, "{}", line).unwrap();

        let nb = s.bs.m_blocks.len();
        let mut nsplits = 0;
        for i in 0..nb {
            let b = s.bs.m_blocks[i];
            if s.bs.arena[b].deleted {
                continue;
            }
            let mut found = None;
            for j in 0..s.cs.len() {
                let c = &s.cs[j];
                if c.active
                    && !c.equality
                    && s.vs[c.left].block == Some(b)
                    && s.vs[c.right].block == Some(b)
                {
                    found = Some(j);
                    break;
                }
            }
            if let Some(c) = found {
                s.blocks_split(b, c);
                nsplits += 1;
            }
        }
        s.blocks_cleanup();
        s.copy_result();
        writeln!(out, "split {} {} {}", nsplits, s.bs.m_blocks.len(), s.bs.block_time_ctr).unwrap();
        dump_state(out, s.vs, s.cs);
        let r = s.satisfy();
        let sc = s.split_cnt;
        print_solve(out, r.clone(), sc);
        dump_state(out, s.vs, s.cs);
        writeln!(out, "cost {}", hex(s.blocks_cost())).unwrap();
    }

    fn gen_nudging(rng: &mut Lcg, vs: &mut Vec<Variable>, cs: &mut Vec<Constraint>) -> f64 {
        let nseg = 2 + rng.below(12);
        let sep_dist = (4 + rng.below(8)) as f64;
        let mut prev: Vec<(usize, bool)> = Vec::new();
        for _ in 0..nseg {
            let fixed = rng.below(5) == 0;
            let mut desired = rng.below(400) as f64 * 0.5;
            if rng.below(3) == 0 {
                desired += 1.0 / 3.0;
            }
            let (id, weight) = if fixed {
                (1, FIXED_WEIGHT)
            } else {
                match rng.below(3) {
                    0 => (0, FREE_WEIGHT),
                    1 => (0, STRONG_WEIGHT),
                    _ => (0, STRONGER_WEIGHT),
                }
            };
            let mut var = Variable::with_weight(id, desired, weight);
            var.fixed_desired_position = fixed;
            vs.push(var);
            let index = vs.len() - 1;
            if !fixed && rng.below(2) == 0 {
                let lim = desired - rng.below(60) as f64;
                vs.push(Variable::with_weight(2, lim, FIXED_WEIGHT));
                cs.push(Constraint::new(vs.len() - 1, index, 0.0, false));
            }
            for &(pv, pfixed) in &prev {
                if rng.below(3) == 0 && (!fixed || !pfixed) {
                    let r = rng.below(6);
                    let (gap, eq) = if r == 0 {
                        (0.0, true)
                    } else if r == 1 {
                        (0.0, false)
                    } else {
                        (sep_dist, false)
                    };
                    cs.push(Constraint::new(pv, index, gap, eq));
                }
            }
            if !fixed && rng.below(2) == 0 {
                let lim = desired + rng.below(60) as f64;
                vs.push(Variable::with_weight(3, lim, FIXED_WEIGHT));
                cs.push(Constraint::new(index, vs.len() - 1, 0.0, false));
            }
            prev.push((index, fixed));
        }
        sep_dist
    }

    fn gen_random(rng: &mut Lcg, vs: &mut Vec<Variable>, cs: &mut Vec<Constraint>, kind: u32) {
        let nv = 1 + rng.below(15);
        let weights = [1.0, 0.001, 100000.0, 2.5, 0.00001];
        let scales = [1.0, 2.0, 0.5, 4.0, 3.0, 0.7];
        for _ in 0..nv {
            let id = rng.below(4) as i32;
            let desired = rng.below(1000) as f64 / 4.0 - 50.0;
            let w = weights[rng.below(5) as usize];
            let scale = if kind == 4 { scales[rng.below(6) as usize] } else { 1.0 };
            let mut v = Variable::new(id, desired, w, scale);
            v.fixed_desired_position = w == 100000.0;
            vs.push(v);
        }
        let nc = rng.below(2 * nv + 1);
        for _ in 0..nc {
            let mut l = rng.below(nv) as usize;
            let mut r = rng.below(nv) as usize;
            if kind != 2 {
                if l == r {
                    continue;
                }
                if l > r {
                    std::mem::swap(&mut l, &mut r);
                }
            }
            let mut gap = rng.below(20) as f64 * 0.5;
            if rng.below(5) == 0 {
                gap = -gap;
            }
            let eq = rng.below(6) == 0;
            cs.push(Constraint::new(l, r, gap, eq));
        }
        if kind == 2 && nv >= 2 && rng.below(2) == 0 {
            // explicit cycle
            let a = rng.below(nv) as usize;
            let b = (a + 1) % nv as usize;
            cs.push(Constraint::new(a, b, 3.0, false));
            cs.push(Constraint::new(b, a, 3.0, false));
        }
    }

    /// Dense problems with integer data, to provoke exact slack / lm ties.
    fn gen_dense(rng: &mut Lcg, vs: &mut Vec<Variable>, cs: &mut Vec<Constraint>) {
        let nv = 5 + rng.below(26);
        let weights = [1.0, 1.0, 2.0, 0.001];
        for _ in 0..nv {
            let id = rng.below(4) as i32;
            let desired = rng.below(40) as f64;
            let w = weights[rng.below(4) as usize];
            vs.push(Variable::with_weight(id, desired, w));
        }
        let nc = rng.below(6 * nv + 1);
        for _ in 0..nc {
            let mut l = rng.below(nv) as usize;
            let mut r = rng.below(nv) as usize;
            if l == r {
                continue;
            }
            if rng.below(10) != 0 && l > r {
                std::mem::swap(&mut l, &mut r);
            }
            let gap = rng.below(4) as f64;
            let eq = rng.below(15) == 0;
            cs.push(Constraint::new(l, r, gap, eq));
        }
    }

    /// Direct stress of the constraint heaps: mergeLeft then mergeRight on
    /// every block of a fresh solver.  Mirrors `heapTest()` in the C++
    /// driver.
    fn heap_test(out: &mut String, vs: &mut Vec<Variable>, cs: &mut Vec<Constraint>) {
        let mut s = IncSolver::new(vs, cs);
        let nb = s.bs.m_blocks.len();
        for i in 0..nb {
            let b = s.bs.m_blocks[i];
            if !s.bs.arena[b].deleted {
                s.blocks_merge_left(b);
            }
        }
        s.blocks_cleanup();
        let nb = s.bs.m_blocks.len();
        for i in 0..nb {
            let b = s.bs.m_blocks[i];
            if !s.bs.arena[b].deleted {
                s.blocks_merge_right(b);
            }
        }
        s.blocks_cleanup();
        s.copy_result();
        writeln!(out, "heap {} {}", s.bs.m_blocks.len(), s.bs.block_time_ctr).unwrap();
        dump_state(out, s.vs, s.cs);
        writeln!(out, "cost {}", hex(s.blocks_cost())).unwrap();
    }

    fn run_case(k: u32, out: &mut String) {
        let mut rng = Lcg::new(k as u64 * 7919 + 1);
        let kind = k % 6;
        writeln!(out, "case {} {}", k, kind).unwrap();
        let mut vs: Vec<Variable> = Vec::new();
        let mut cs: Vec<Constraint> = Vec::new();
        if kind == 0 || kind == 3 {
            let sep = gen_nudging(&mut rng, &mut vs, &mut cs);
            let rounds = 3;
            for round in 0..rounds {
                writeln!(out, "round {}", round).unwrap();
                {
                    let mut s = IncSolver::new(&mut vs, &mut cs);
                    let r = s.solve();
                    let sc = s.split_cnt;
                    print_solve(out, r.clone(), sc);
                    if round == rounds - 1 && r.is_ok() {
                        dump_state(out, s.vs, s.cs);
                        extras(out, &mut s);
                        continue;
                    }
                }
                dump_state(out, &vs, &cs);
                // Rewrite gaps like the nudging loop does.
                for c in cs.iter_mut() {
                    if c.gap > 0.0 {
                        c.gap -= sep / 5.0;
                    }
                }
                if kind == 3 && rng.below(2) == 0 && vs.len() >= 2 {
                    let a = rng.below(vs.len() as u32) as usize;
                    let b = rng.below(vs.len() as u32) as usize;
                    if a != b {
                        cs.push(Constraint::new(a, b, 0.0, true));
                    }
                }
            }
        } else {
            if kind == 5 {
                gen_dense(&mut rng, &mut vs, &mut cs);
            } else {
                gen_random(&mut rng, &mut vs, &mut cs, kind);
            }
            let kept = constraints_removing_redundant_equalities(&vs, &cs);
            let mut line = String::from("kept");
            for i in &kept {
                write!(line, " {}", i).unwrap();
            }
            writeln!(out, "{}", line).unwrap();
            let mut s = IncSolver::new(&mut vs, &mut cs);
            let r = s.solve();
            let sc = s.split_cnt;
            print_solve(out, r.clone(), sc);
            dump_state(out, s.vs, s.cs);
            if r.is_ok() {
                extras(out, &mut s);
            }
        }
        heap_test(out, &mut vs, &mut cs);
    }

    pub const NUM_CASES: u32 = 600;

    thread_local! {
        /// The C++ driver's `--print` mode (see `extras`).
        static PRINT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }

    /// The debug printers (`operator<<` of `Block`/`Variable`/`Constraint`
    /// through `printBlocks()`, and `Constraint::toString()`) against the C++
    /// driver's `--print` output for the first 60 cases
    /// (`tests/golden/vpsc_print_oracle.txt`, see tools/oracle/vpsc_oracle.cpp).
    #[test]
    fn vpsc_printing_matches_cpp_oracle() {
        let golden = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/vpsc_print_oracle.txt"));
        PRINT.with(|p| p.set(true));
        let mut out = String::new();
        for k in 0..60 {
            run_case(k, &mut out);
        }
        PRINT.with(|p| p.set(false));
        // `Constraint::lm` is uninitialised upstream until compute_dfdv()
        // writes it, which only happens for active constraints; for the
        // others the C++ prints whatever the memory held (allocation
        // dependent).  Their `(lm=...)` is masked on both sides.
        fn mask_inactive_lm(l: &str) -> String {
            match (l.contains("-active(lm="), l.rfind("(lm=")) {
                (false, Some(p)) => format!("{}(lm=?)", &l[..p]),
                _ => l.to_string(),
            }
        }
        let actual: Vec<String> = out
            .lines()
            .filter(|l| l.starts_with("case ") || l.starts_with("p "))
            .map(mask_inactive_lm)
            .collect();
        let expected: Vec<String> = golden.lines().map(mask_inactive_lm).collect();
        for (i, (e, a)) in expected.iter().zip(actual.iter()).enumerate() {
            assert_eq!(e, a, "first mismatch at line {}", i + 1);
        }
        assert_eq!(expected.len(), actual.len(), "line count differs");
    }

    #[test]
    fn vpsc_matches_cpp_oracle() {
        let golden = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/vpsc_oracle.txt"));
        let mut out = String::new();
        for k in 0..NUM_CASES {
            run_case(k, &mut out);
        }
        let expected: Vec<&str> = golden
            .lines()
            .map(|l| match l.find(" #") {
                Some(p) => &l[..p],
                None => l,
            })
            .collect();
        let actual: Vec<&str> = out.lines().collect();
        let mut current_case = "";
        for (i, (e, a)) in expected.iter().zip(actual.iter()).enumerate() {
            if e.starts_with("case ") {
                current_case = e;
            }
            assert_eq!(e, a, "first mismatch at line {} ({})", i + 1, current_case);
        }
        assert_eq!(expected.len(), actual.len(), "line count differs");
    }

    #[test]
    fn simple_separation() {
        let mut vs = vec![
            Variable::with_weight(0, 10.0, 1.0),
            Variable::with_weight(0, 10.0, 1.0),
        ];
        let mut cs = vec![Constraint::new(0, 1, 4.0, false)];
        let mut s = IncSolver::new(&mut vs, &mut cs);
        s.solve().unwrap();
        drop(s);
        assert_eq!(vs[0].final_position, 8.0);
        assert_eq!(vs[1].final_position, 12.0);
    }
}
