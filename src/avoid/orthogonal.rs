//! Faithful port of upstream `libavoid/orthogonal.{h,cpp}`.
//!
//! PORTED: the first half of orthogonal.cpp (static orthogonal visibility
//! graph construction, up to and including
//! `generateStaticOrthogonalVisGraph`).  NOT YET PORTED: the second half
//! (path adjustment / nudging: `CmpIndexes`, `NudgingShiftSegment`,
//! `ImproveOrthogonalRoutes`, `CmpLineOrder`, `PtOrder` users, ...);
//! `improve_orthogonal_routes` is still a `todo!` stub.  Append that code in
//! the "Path Adjustment code" section at the end of this file.
//!
//! # Container mapping (iteration order is behaviour)
//! * `VertSet` (`std::set<VertInf*, CmpVertInf>`) is a `BTreeSet<VertKey>`;
//!   a [`VertKey`] carries a copy of the vertex point (vertices never move
//!   while the graph is built) and the vertex creation serial for the
//!   `u < v` pointer tie-break (`// PTR-ORDER:`).
//! * `BreakpointSet` (`std::set<PosVertInf>`) is a `BTreeSet<PosVertInf>`
//!   ordered by `PosVertInf::operator<` (which treats all dummy vertices at
//!   the same position as equal, so `insert` keeps the first one).
//! * `SegmentList` (`std::list<LineSegment>`) is a `Vec<LineSegment>` in
//!   list order; `std::list::sort` (stable) is `Vec::sort_by` (stable) with
//!   the `LineSegment::operator<` strict weak order, which yields the same
//!   sequence; `erase` while iterating keeps the relative order.
//! * `LineSegment *` results of `SegmentListWrapper::insert` are indices
//!   into the list, used before the next insertion exactly as upstream.

#![allow(dead_code)]
#![allow(
    clippy::collapsible_if,
    clippy::collapsible_else_if,
    clippy::needless_range_loop,
    clippy::too_many_arguments
)]

use std::cmp::Ordering;
use std::collections::BTreeSet;

use crate::avoid::arena::VertIdx;
use crate::avoid::geomtypes::{Point, XDIM, YDIM};
use crate::avoid::graph::EdgeInf;
use crate::avoid::obstacle::Obstacle;
use crate::avoid::router::{
    Router, TRANSACTION_PHASE_ORTHOGONAL_VISIBILITY_GRAPH_SCAN_X,
    TRANSACTION_PHASE_ORTHOGONAL_VISIBILITY_GRAPH_SCAN_Y,
};
use crate::avoid::scanline::{sort_events, Event, EventType, NodeArena, NodeSet};
use crate::avoid::vertices::{
    ConnDirFlags, VertArena, VertID, VertInf, CONN_DIR_DOWN, CONN_DIR_LEFT, CONN_DIR_NONE, CONN_DIR_RIGHT,
    CONN_DIR_UP, DUMMY_ORTHOG_ID, DUMMY_ORTHOG_SHAPE_ID, XH_CONN, XH_EDGE, XL_CONN, XL_EDGE, YH_CONN, YH_EDGE,
    YL_CONN, YL_EDGE,
};

const DBL_MAX: f64 = f64::MAX;

// IDs:
// upstream: orthogonal.cpp freeSegmentID / fixedSegmentID / channelLeftID / channelRightID
pub const FREE_SEGMENT_ID: i32 = 0;
pub const FIXED_SEGMENT_ID: i32 = 1;
pub const CHANNEL_LEFT_ID: i32 = 2;
pub const CHANNEL_RIGHT_ID: i32 = 3;
// Weights:
// upstream: orthogonal.cpp freeWeight / strongWeight / strongerWeight / fixedWeight
pub const FREE_WEIGHT: f64 = 0.00001;
pub const STRONG_WEIGHT: f64 = 0.001;
pub const STRONGER_WEIGHT: f64 = 1.0;
pub const FIXED_WEIGHT: f64 = 100000.0;

/// A pair of unsigned values that can be compared and used as the keys
/// for sets and maps.  (Stored as `unsigned short` upstream; the derived
/// ordering is the upstream lexicographic `operator<`.)
// upstream: orthogonal.cpp class UnsignedPair
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnsignedPair {
    m_index1: u16,
    m_index2: u16,
}

impl UnsignedPair {
    // upstream: orthogonal.cpp UnsignedPair::UnsignedPair()
    pub fn new(ind1: u32, ind2: u32) -> UnsignedPair {
        debug_assert!(ind1 != ind2);
        // Assign the lesser value to m_index1.
        let m_index1 = (if ind1 < ind2 { ind1 } else { ind2 }) as u16;
        // Assign the greater value to m_index2.
        let m_index2 = (if ind1 > ind2 { ind1 } else { ind2 }) as u16;
        UnsignedPair { m_index1, m_index2 }
    }
}
// upstream: orthogonal.cpp typedef std::set<UnsignedPair> UnsignedPairSet
pub type UnsignedPairSet = BTreeSet<UnsignedPair>;

//============================================================================
//                    Static orthogonal visibility graph
//============================================================================

// upstream: orthogonal.cpp enum ScanVisDirFlag
pub const VIS_DIR_NONE: ScanVisDirFlags = 0;
pub const VIS_DIR_UP: ScanVisDirFlags = 1;
pub const VIS_DIR_DOWN: ScanVisDirFlags = 2;
// upstream: orthogonal.cpp typedef unsigned int ScanVisDirFlags
pub type ScanVisDirFlags = u32;

// Returns a bitfield of the directions of visibility in terms of the scanline
// in a particular dimension dimension.  It will return either ConnDirDown
// (meaning visibility to lower position values) or ConnDirUp (for visibility
// towards higher position values).
//
// upstream: orthogonal.cpp getPosVertInfDirections()
fn get_pos_vert_inf_directions(v: &VertInf, dim: usize) -> ScanVisDirFlags {
    if dim == XDIM {
        // X-dimension
        let dirs: ConnDirFlags = v.vis_directions & (CONN_DIR_LEFT | CONN_DIR_RIGHT);
        if dirs == (CONN_DIR_LEFT | CONN_DIR_RIGHT) {
            return VIS_DIR_DOWN | VIS_DIR_UP;
        } else if dirs == CONN_DIR_LEFT {
            return VIS_DIR_DOWN;
        } else if dirs == CONN_DIR_RIGHT {
            return VIS_DIR_UP;
        }
    } else if dim == YDIM {
        // Y-dimension
        let dirs: ConnDirFlags = v.vis_directions & (CONN_DIR_DOWN | CONN_DIR_UP);
        if dirs == (CONN_DIR_DOWN | CONN_DIR_UP) {
            return VIS_DIR_DOWN | VIS_DIR_UP;
        } else if dirs == CONN_DIR_DOWN {
            // libavoid's Y-axis points downwards, so where the user has
            // specified visibility downwards, this results in visibility to
            // higher scanline positition values.
            return VIS_DIR_UP;
        } else if dirs == CONN_DIR_UP {
            // libavoid's Y-axis points downwards, so where the user has
            // specified visibility upwards, this results in visibility to
            // lower scanline positition values.
            return VIS_DIR_DOWN;
        }
    }

    // Can occur for ConnDirNone visibility.
    VIS_DIR_NONE
}

/// Element of a `BreakpointSet`.  `vert_id` is a copy of `vert->id` (ids do
/// not change while the graph is built).
// upstream: orthogonal.cpp struct PosVertInf
#[derive(Clone, Copy, Debug)]
pub struct PosVertInf {
    pub pos: f64,
    pub vert: VertIdx,
    pub vert_id: VertID,

    // A bitfield marking the direction of visibility (in this dimension)
    // made up of VisDirDown (for visibility towards lower position values)
    // and VisDirUp (for visibility towards higher position values).
    //
    pub dirs: ScanVisDirFlags,
}

impl PosVertInf {
    // upstream: orthogonal.cpp PosVertInf::PosVertInf()
    pub fn new(p: f64, verts: &VertArena, v_i: VertIdx, d: ScanVisDirFlags) -> PosVertInf {
        PosVertInf {
            pos: p,
            vert: v_i,
            vert_id: verts[v_i].id,
            dirs: d,
        }
    }

    // upstream: orthogonal.cpp PosVertInf::operator<()
    pub fn less_than(&self, rhs: &PosVertInf) -> bool {
        if self.pos != rhs.pos {
            return self.pos < rhs.pos;
        }
        if (self.vert_id == rhs.vert_id) && (self.vert_id == DUMMY_ORTHOG_ID) {
            // Multiple dummy nodes can get placed at the same point for
            // multiple ShapeConnectionPins on junctions (outside of shapes).
            // We only need one at each position, so multiples can be seen
            // as equal here.
            // (VertID equality ignores the props, so this also holds for
            // dummyOrthogShapeID vertices, exactly as upstream.)
            return false;
        }
        if self.vert_id != rhs.vert_id {
            return self.vert_id < rhs.vert_id;
        }
        self.dirs < rhs.dirs
    }
}

impl Ord for PosVertInf {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.less_than(other) {
            Ordering::Less
        } else if other.less_than(self) {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}
impl PartialOrd for PosVertInf {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for PosVertInf {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for PosVertInf {}

/// Element of a `VertSet`: `VertInf*` plus what `CmpVertInf` reads.
#[derive(Clone, Copy, Debug)]
pub struct VertKey {
    pub x: f64,
    pub y: f64,
    /// Vertex creation serial (PTR-ORDER).
    pub serial: u64,
    pub v: VertIdx,
}

impl VertKey {
    pub fn of(verts: &VertArena, v: VertIdx) -> VertKey {
        let vert = &verts[v];
        VertKey {
            x: vert.point.x,
            y: vert.point.y,
            serial: vert.serial,
            v,
        }
    }
}

// upstream: orthogonal.cpp CmpVertInf::operator()
pub fn cmp_vert_inf(u: &VertKey, v: &VertKey) -> bool {
    // Comparator for VertSet, an ordered set of VertInf pointers.
    // It is assumed vertical sets of points will all have the same
    // x position and horizontal sets all share a y position, so this
    // method can be used to sort both these sets.
    debug_assert!((u.x == v.x) || (u.y == v.y));
    if u.x != v.x {
        return u.x < v.x;
    } else if u.y != v.y {
        return u.y < v.y;
    }
    // PTR-ORDER: `u < v` on `VertInf*` -> vertex creation serial.
    u.serial < v.serial
}

impl Ord for VertKey {
    fn cmp(&self, other: &Self) -> Ordering {
        if cmp_vert_inf(self, other) {
            Ordering::Less
        } else if cmp_vert_inf(other, self) {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}
impl PartialOrd for VertKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for VertKey {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for VertKey {}

// upstream: orthogonal.cpp typedef std::set<VertInf *, CmpVertInf> VertSet
pub type VertSet = BTreeSet<VertKey>;

// A set of points to break the line segment,
// along with vertices for these points.
// upstream: orthogonal.cpp typedef std::set<PosVertInf> BreakpointSet
pub type BreakpointSet = BTreeSet<PosVertInf>;

// Temporary structure used to store the possible horizontal visibility
// lines arising from the vertical sweep.
// upstream: orthogonal.cpp class LineSegment
#[derive(Clone, Debug)]
pub struct LineSegment {
    pub begin: f64,
    pub finish: f64,
    pub pos: f64,

    // XXX shapeSide is unused and could possibly be removed?
    pub shape_side: bool,

    pub vert_infs: VertSet,
    pub break_points: BreakpointSet,
}

impl LineSegment {
    // upstream: orthogonal.cpp LineSegment::LineSegment(b, f, p, ss, bvi, fvi)
    pub fn new(
        verts: &VertArena,
        b: f64,
        f: f64,
        p: f64,
        ss: bool,
        bvi: Option<VertIdx>,
        fvi: Option<VertIdx>,
    ) -> LineSegment {
        let mut seg = LineSegment {
            begin: b,
            finish: f,
            pos: p,
            shape_side: ss,
            vert_infs: VertSet::new(),
            break_points: BreakpointSet::new(),
        };
        debug_assert!(seg.begin < seg.finish);

        if let Some(bvi) = bvi {
            seg.vert_infs.insert(VertKey::of(verts, bvi));
        }
        if let Some(fvi) = fvi {
            seg.vert_infs.insert(VertKey::of(verts, fvi));
        }
        seg
    }

    // upstream: orthogonal.cpp LineSegment::LineSegment(bf, p, bfvi)
    pub fn new_point(verts: &VertArena, bf: f64, p: f64, bfvi: Option<VertIdx>) -> LineSegment {
        let mut seg = LineSegment {
            begin: bf,
            finish: bf,
            pos: p,
            shape_side: false,
            vert_infs: VertSet::new(),
            break_points: BreakpointSet::new(),
        };
        if let Some(bfvi) = bfvi {
            seg.vert_infs.insert(VertKey::of(verts, bfvi));
        }
        seg
    }

    // Order by begin, pos, finish.
    // upstream: orthogonal.cpp LineSegment::operator<()
    pub fn less_than(&self, rhs: &LineSegment) -> bool {
        if self.begin != rhs.begin {
            return self.begin < rhs.begin;
        }
        if self.pos != rhs.pos {
            return self.pos < rhs.pos;
        }
        if self.finish != rhs.finish {
            return self.finish < rhs.finish;
        }
        debug_assert!(self.shape_side == rhs.shape_side);
        false
    }

    // upstream: orthogonal.cpp LineSegment::overlaps()
    pub fn overlaps(&self, rhs: &LineSegment) -> bool {
        if (self.begin == rhs.begin) && (self.pos == rhs.pos) && (self.finish == rhs.finish) {
            // Lines are exactly equal.
            return true;
        }

        if self.pos == rhs.pos {
            if ((self.begin >= rhs.begin) && (self.begin <= rhs.finish))
                || ((rhs.begin >= self.begin) && (rhs.begin <= self.finish))
            {
                // They are colinear and overlap by some amount.
                return true;
            }
        }
        false
    }

    // upstream: orthogonal.cpp LineSegment::mergeVertInfs()
    pub fn merge_vert_infs(&mut self, segment: &LineSegment) {
        self.begin = if segment.begin < self.begin { segment.begin } else { self.begin };
        self.finish = if self.finish < segment.finish { segment.finish } else { self.finish };
        for k in segment.vert_infs.iter() {
            self.vert_infs.insert(*k);
        }
    }

    // upstream: orthogonal.cpp LineSegment::beginVertInf()
    pub fn begin_vert_inf(&self) -> Option<VertIdx> {
        let inf = self.vert_infs.first()?;
        if ((inf.y == self.begin) && (inf.x == self.pos)) || ((inf.x == self.begin) && (inf.y == self.pos)) {
            // Only return the point if it is actually at the begin pos.
            return Some(inf.v);
        }
        None
    }

    // upstream: orthogonal.cpp LineSegment::finishVertInf()
    pub fn finish_vert_inf(&self) -> Option<VertIdx> {
        let inf = self.vert_infs.last()?;
        if ((inf.y == self.finish) && (inf.x == self.pos)) || ((inf.x == self.finish) && (inf.y == self.pos)) {
            // Only return the point if it is actually at the finish pos.
            return Some(inf.v);
        }
        None
    }

    // upstream: orthogonal.cpp LineSegment::commitPositionX()
    pub fn commit_position_x(&mut self, router: &mut Router, pos_x: f64) -> VertIdx {
        let mut found: Option<VertIdx> = None;
        for v in self.vert_infs.iter() {
            if v.x == pos_x {
                found = Some(v.v);
                break;
            }
        }
        match found {
            Some(f) => f,
            None => {
                let f = VertInf::new(router, DUMMY_ORTHOG_ID, Point::new(pos_x, self.pos), true);
                self.vert_infs.insert(VertKey::of(&router.vert_infs, f));
                f
            }
        }
    }

    // Set begin endpoint vertex if none has been assigned.
    // upstream: orthogonal.cpp LineSegment::horiCommitBegin()
    pub fn hori_commit_begin(&mut self, router: &mut Router, vert: Option<VertIdx>) {
        if let Some(vert) = vert {
            self.vert_infs.insert(VertKey::of(&router.vert_infs, vert));
        }

        if self.vert_infs.is_empty() || (self.vert_infs.first().unwrap().x != self.begin) {
            if self.begin != -DBL_MAX {
                let v = VertInf::new(router, DUMMY_ORTHOG_ID, Point::new(self.begin, self.pos), true);
                self.vert_infs.insert(VertKey::of(&router.vert_infs, v));
            }
        }
    }

    // Set begin endpoint vertex if none has been assigned.
    // upstream: orthogonal.cpp LineSegment::horiCommitFinish()
    pub fn hori_commit_finish(&mut self, router: &mut Router, vert: Option<VertIdx>) {
        if let Some(vert) = vert {
            self.vert_infs.insert(VertKey::of(&router.vert_infs, vert));
        }

        if self.vert_infs.is_empty() || (self.vert_infs.last().unwrap().x != self.finish) {
            if self.finish != DBL_MAX {
                let v = VertInf::new(router, DUMMY_ORTHOG_ID, Point::new(self.finish, self.pos), true);
                self.vert_infs.insert(VertKey::of(&router.vert_infs, v));
            }
        }
    }

    // Converts a section of the points list to a set of breakPoints.
    // Returns the first of the intersection points occurring at finishPos.
    // (`None` is upstream's `vertInfs.end()`.)
    // upstream: orthogonal.cpp LineSegment::addSegmentsUpTo()
    pub fn add_segments_up_to(&mut self, verts: &VertArena, finish_pos: f64) -> Option<VertKey> {
        let LineSegment {
            vert_infs, break_points, ..
        } = self;
        let mut first_intersection_pt: Option<VertKey> = None;
        for vert in vert_infs.iter() {
            if vert.x > finish_pos {
                // We're done.
                break;
            }

            break_points.insert(PosVertInf::new(
                vert.x,
                verts,
                vert.v,
                get_pos_vert_inf_directions(&verts[vert.v], XDIM),
            ));

            if first_intersection_pt.is_none() && (vert.x == finish_pos) {
                first_intersection_pt = Some(*vert);
            }
        }
        // Returns the first of the intersection points at finishPos.
        first_intersection_pt
    }

    // Add visibility edge(s) for this segment.  There may be multiple if
    // one of the endpoints is shared by multiple connector endpoints.
    // upstream: orthogonal.cpp LineSegment::addEdgeHorizontal()
    pub fn add_edge_horizontal(&mut self, router: &mut Router) {
        self.hori_commit_begin(router, None);
        self.hori_commit_finish(router, None);

        let finish = self.finish;
        self.add_segments_up_to(&router.vert_infs, finish);
    }

    // Set flags to show what can be passed on this visibility line.
    // This can be used later to disregard some edges in the visibility
    // graph when routing particular connectors.
    // upstream: orthogonal.cpp LineSegment::setLongRangeVisibilityFlags()
    pub fn set_long_range_visibility_flags(&self, verts: &mut VertArena, dim: usize) {
        // First, travel in one direction
        let mut seen_conn_pt = false;
        let mut seen_shape_edge = false;
        for nvert in self.break_points.iter() {
            let mut mask: u32 = 0;
            if dim == XDIM {
                if seen_conn_pt {
                    mask |= XL_CONN;
                }
                if seen_shape_edge {
                    mask |= XL_EDGE;
                }
            } else {
                // if (dim == YDIM)
                if seen_conn_pt {
                    mask |= YL_CONN;
                }
                if seen_shape_edge {
                    mask |= YL_EDGE;
                }
            }
            verts[nvert.vert].orthog_vis_prop_flags |= mask;

            if verts[nvert.vert].id.is_conn_pt() {
                seen_conn_pt = true;
            }
            if verts[nvert.vert].id.is_orth_shape_edge() {
                seen_shape_edge = true;
            }
        }
        // Then in the other direction
        seen_conn_pt = false;
        seen_shape_edge = false;
        for rvert in self.break_points.iter().rev() {
            let mut mask: u32 = 0;
            if dim == XDIM {
                if seen_conn_pt {
                    mask |= XH_CONN;
                }
                if seen_shape_edge {
                    mask |= XH_EDGE;
                }
            } else {
                // if (dim == YDIM)
                if seen_conn_pt {
                    mask |= YH_CONN;
                }
                if seen_shape_edge {
                    mask |= YH_EDGE;
                }
            }
            verts[rvert.vert].orthog_vis_prop_flags |= mask;

            if verts[rvert.vert].id.is_conn_pt() {
                seen_conn_pt = true;
            }
            if verts[rvert.vert].id.is_orth_shape_edge() {
                seen_shape_edge = true;
            }
        }
    }

    // Add visibility edge(s) for this segment up until an intersection.
    // Then, move the segment beginning to the intersection point, so we
    // later only consider the remainder of the segment.
    // There may be multiple segments added to the graph if the beginning
    // endpoint of the segment is shared by multiple connector endpoints.
    // (Upstream passes `vertLine`; only its `pos` is read.)
    // upstream: orthogonal.cpp LineSegment::addEdgeHorizontalTillIntersection()
    pub fn add_edge_horizontal_till_intersection(&mut self, router: &mut Router, vert_line_pos: f64) -> VertSet {
        let mut intersection_set = VertSet::new();

        self.hori_commit_begin(router, None);

        // Does a vertex already exist for this point.
        self.commit_position_x(router, vert_line_pos);

        // Generate segments and set end iterator to the first point
        // at the intersection position.
        let rest_begin = self.add_segments_up_to(&router.vert_infs, vert_line_pos);

        // Add the intersections points to intersectionSet.
        // (`rest_begin == None` is `vertInfs.end()`: nothing to add.)
        let rest = match rest_begin {
            Some(rb) => self.vert_infs.split_off(&rb),
            None => VertSet::new(),
        };
        for v in rest.iter() {
            if v.x != vert_line_pos {
                break;
            }
            intersection_set.insert(*v);
        }

        // Adjust segment to remove processed portion.
        self.begin = vert_line_pos;
        // vertInfs.erase(vertInfs.begin(), restBegin);
        self.vert_infs = rest;

        intersection_set
    }

    // Insert vertical breakpoints.
    // upstream: orthogonal.cpp LineSegment::insertBreakpointsBegin()
    pub fn insert_breakpoints_begin(&mut self, router: &mut Router, vert_line: &mut LineSegment) {
        let mut vert: Option<VertIdx> = None;
        if self.pos == vert_line.begin && vert_line.begin_vert_inf().is_some() {
            vert = vert_line.begin_vert_inf();
        } else if self.pos == vert_line.finish && vert_line.finish_vert_inf().is_some() {
            vert = vert_line.finish_vert_inf();
        }
        self.hori_commit_begin(router, vert);

        for v in self.vert_infs.iter() {
            if v.x == self.begin {
                vert_line.break_points.insert(PosVertInf::new(
                    self.pos,
                    &router.vert_infs,
                    v.v,
                    get_pos_vert_inf_directions(&router.vert_infs[v.v], YDIM),
                ));
            }
        }
    }

    // Insert vertical breakpoints.
    // upstream: orthogonal.cpp LineSegment::insertBreakpointsFinish()
    pub fn insert_breakpoints_finish(&mut self, router: &mut Router, vert_line: &mut LineSegment) {
        let mut vert: Option<VertIdx> = None;
        if self.pos == vert_line.begin && vert_line.begin_vert_inf().is_some() {
            vert = vert_line.begin_vert_inf();
        } else if self.pos == vert_line.finish && vert_line.finish_vert_inf().is_some() {
            vert = vert_line.finish_vert_inf();
        }
        self.hori_commit_finish(router, vert);

        for v in self.vert_infs.iter() {
            if v.x == self.finish {
                vert_line.break_points.insert(PosVertInf::new(
                    self.pos,
                    &router.vert_infs,
                    v.v,
                    get_pos_vert_inf_directions(&router.vert_infs[v.v], YDIM),
                ));
            }
        }
    }

    // upstream: orthogonal.cpp LineSegment::generateVisibilityEdgesFromBreakpointSet()
    pub fn generate_visibility_edges_from_breakpoint_set(&mut self, router: &mut Router, dim: usize) {
        if self.break_points.is_empty() || (self.break_points.first().unwrap().pos > self.begin) {
            // Add a begin point if there was not already an intersection
            // found at that point. Though, don't do this if the line
            // segment goes off to infinity -- we can't reach anything
            // by going in that direction!
            if self.begin == -DBL_MAX {
                // Shorten line to first intersection point.
                debug_assert!(!self.break_points.is_empty());
                if let Some(first) = self.break_points.first() {
                    self.begin = first.pos;
                }
            } else {
                // Add begin point.
                let mut point = Point::new(self.pos, self.pos);
                point[dim] = self.begin;
                let vert = VertInf::new(router, DUMMY_ORTHOG_ID, point, true);
                self.break_points
                    .insert(PosVertInf::new(self.begin, &router.vert_infs, vert, VIS_DIR_NONE));
            }
        }
        if self.break_points.is_empty() || (self.break_points.last().unwrap().pos < self.finish) {
            // Add a finish point if there was not already an intersection
            // found at that point. Though, don't do this if the line
            // segment goes off to infinity -- we can't reach anything
            // by going in that direction!
            if self.finish == DBL_MAX {
                // Shorten line to last intersection point.
                // (Undefined behaviour upstream if there are no breakpoints.)
                if let Some(last) = self.break_points.last() {
                    self.finish = last.pos;
                }
            } else {
                // Add finish point.
                let mut point = Point::new(self.pos, self.pos);
                point[dim] = self.finish;
                let vert = VertInf::new(router, DUMMY_ORTHOG_ID, point, true);
                self.break_points
                    .insert(PosVertInf::new(self.finish, &router.vert_infs, vert, VIS_DIR_NONE));
            }
        }

        // Set flags for orthogonal routing optimisation.
        self.set_long_range_visibility_flags(&mut router.vert_infs, dim);

        let orthogonal = true;
        // (The set is not modified below; walk it as a vector so that the
        // bidirectional iterators become indices.)
        let bp: Vec<PosVertInf> = self.break_points.iter().copied().collect();
        let end = bp.len();
        let pt = |router: &Router, i: usize| router.vert_infs[bp[i].vert].point[dim];
        let is_conn_pt = |router: &Router, i: usize| router.vert_infs[bp[i].vert].id.is_conn_pt();

        let mut vert = 0usize;
        let mut last = 0usize;
        while vert != end {
            let first_prev = last;
            while pt(router, last) != pt(router, vert) {
                debug_assert!(vert != last);
                // Assert points are not at the same position.
                debug_assert!(router.vert_infs[bp[vert].vert].point != router.vert_infs[bp[last].vert].point);

                if is_conn_pt(router, vert) && is_conn_pt(router, last) {
                    // Here we have a pair of two endpoints that are both
                    // connector endpoints and both are inside a shape.

                    // Give vert visibility back to the first non-connector
                    // endpoint vertex (i.e., the side of the shape).
                    let mut side = last;
                    while is_conn_pt(router, side) {
                        if side == 0 {
                            break;
                        }
                        side -= 1;
                    }
                    let can_see_down = (bp[vert].dirs & VIS_DIR_DOWN) != 0;
                    if can_see_down && !is_conn_pt(router, side) {
                        let edge = EdgeInf::new(router, bp[side].vert, bp[vert].vert, orthogonal);
                        let d = pt(router, vert) - pt(router, side);
                        EdgeInf::set_dist(router, edge, d);
                    }

                    // Give last visibility back to the first non-connector
                    // endpoint vertex (i.e., the side of the shape).
                    side = vert;
                    while (side != end) && is_conn_pt(router, side) {
                        side += 1;
                    }
                    let can_see_up = (bp[last].dirs & VIS_DIR_UP) != 0;
                    if can_see_up && (side != end) {
                        let edge = EdgeInf::new(router, bp[last].vert, bp[side].vert, orthogonal);
                        let d = pt(router, side) - pt(router, last);
                        EdgeInf::set_dist(router, edge, d);
                    }
                }

                // The normal case.
                //
                // Note: It's okay to give two connector endpoints visibility
                // here since we only consider the partner endpoint as a
                // candidate while searching if it is the other endpoint of
                // the connector in question.
                //
                let mut generate_edge = true;
                if is_conn_pt(router, last) && (bp[last].dirs & VIS_DIR_UP) == 0 {
                    // Don't generate the visibility segment if the ConnEnd
                    // doesn't have visibility in that direction.
                    generate_edge = false;
                } else if is_conn_pt(router, vert) && (bp[vert].dirs & VIS_DIR_DOWN) == 0 {
                    // Don't generate the visibility segment if the ConnEnd
                    // doesn't have visibility in that direction.
                    generate_edge = false;
                }
                if generate_edge {
                    let edge = EdgeInf::new(router, bp[last].vert, bp[vert].vert, orthogonal);
                    let d = pt(router, vert) - pt(router, last);
                    EdgeInf::set_dist(router, edge, d);
                }

                last += 1;
            }

            vert += 1;

            if (vert != end) && (pt(router, last) == pt(router, vert)) {
                // Still looking at same pair, just reset prev number pointer.
                last = first_prev;
            } else {
                // vert has moved to the beginning of a group at a new
                // position.  Last is now in the right place, so do nothing.
            }
        }
    }
}

// upstream: orthogonal.cpp typedef std::list<LineSegment> SegmentList
pub type SegmentList = Vec<LineSegment>;

/// Upstream `std::list<LineSegment>::sort()` (stable merge sort by
/// `LineSegment::operator<`).
pub fn sort_segment_list(list: &mut SegmentList) {
    list.sort_by(|a, b| {
        if a.less_than(b) {
            Ordering::Less
        } else if b.less_than(a) {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    });
}

// upstream: orthogonal.cpp class SegmentListWrapper
#[derive(Clone, Debug, Default)]
pub struct SegmentListWrapper {
    _list: SegmentList,
}

impl SegmentListWrapper {
    pub fn new() -> SegmentListWrapper {
        SegmentListWrapper { _list: Vec::new() }
    }

    /// Returns the index of the (possibly merged) segment in `list()`; it
    /// is valid until the next `insert` (upstream returns a pointer).
    // upstream: orthogonal.cpp SegmentListWrapper::insert()
    pub fn insert(&mut self, segment: LineSegment) -> usize {
        let mut found: Option<usize> = None;
        let mut curr = 0usize;
        while curr < self._list.len() {
            if self._list[curr].overlaps(&segment) {
                if let Some(f) = found {
                    // This is not the first segment that overlaps,
                    // so we need to merge and then delete an existing
                    // segment.
                    // (`f < curr`; erasing it shifts `curr` down by one.)
                    let found_seg = self._list.remove(f);
                    curr -= 1;
                    self._list[curr].merge_vert_infs(&found_seg);
                    found = Some(curr);
                } else {
                    // This is the first overlapping segment, so just
                    // merge the new segment with this one.
                    self._list[curr].merge_vert_infs(&segment);
                    found = Some(curr);
                }
            }
            curr += 1;
        }

        match found {
            None => {
                // Add this line.
                self._list.push(segment);
                self._list.len() - 1
            }
            Some(f) => f,
        }
    }

    // upstream: orthogonal.cpp SegmentListWrapper::list()
    pub fn list(&mut self) -> &mut SegmentList {
        &mut self._list
    }
}

// Given a router instance and a set of possible horizontal segments, and a
// possible vertical visibility segment, compute and add edges to the
// orthogonal visibility graph for all the visibility edges.
// upstream: orthogonal.cpp intersectSegments()
fn intersect_segments(router: &mut Router, segments: &mut SegmentList, vert_line: &mut LineSegment) {
    // XXX: It seems that this case can sometimes occur... maybe when
    // there are many overlapping rectangles.
    //COLA_ASSERT(vertLine.beginVertInf() == nullptr);
    //COLA_ASSERT(vertLine.finishVertInf() == nullptr);

    debug_assert!(!segments.is_empty());
    // (`it = segments.erase(it)` keeps the order of the remaining
    // elements; rebuild the list from the survivors.)
    let old = std::mem::take(segments);
    let mut kept: SegmentList = Vec::with_capacity(old.len());
    for mut hori_line in old {
        let in_vert_seg_region = (vert_line.begin <= hori_line.pos) && (vert_line.finish >= hori_line.pos);

        if vert_line.pos < hori_line.begin {
            // We've yet to reach this segment in the sweep, so ignore.
            kept.push(hori_line);
            continue;
        } else if vert_line.pos == hori_line.begin {
            if in_vert_seg_region {
                hori_line.insert_breakpoints_begin(router, vert_line);
            }
        } else if vert_line.pos == hori_line.finish {
            if in_vert_seg_region {
                // Add horizontal visibility segment.
                hori_line.add_edge_horizontal(router);

                hori_line.insert_breakpoints_finish(router, vert_line);

                let dim = XDIM; // x-dimension
                hori_line.generate_visibility_edges_from_breakpoint_set(router, dim);

                // And we've now finished with the segment, so delete.
                continue;
            }
        } else if vert_line.pos > hori_line.finish {
            // Add horizontal visibility segment.
            hori_line.add_edge_horizontal(router);

            let dim = XDIM; // x-dimension
            hori_line.generate_visibility_edges_from_breakpoint_set(router, dim);

            // We've now swept past this horizontal segment, so delete.
            continue;
        } else {
            debug_assert!(vert_line.pos > hori_line.begin);
            debug_assert!(vert_line.pos < hori_line.finish);

            if in_vert_seg_region {
                // Add horizontal visibility segment.
                let intersection_verts = hori_line.add_edge_horizontal_till_intersection(router, vert_line.pos);

                for v in intersection_verts.iter() {
                    vert_line.break_points.insert(PosVertInf::new(
                        hori_line.pos,
                        &router.vert_infs,
                        v.v,
                        get_pos_vert_inf_directions(&router.vert_infs[v.v], YDIM),
                    ));
                }
            }
        }
        kept.push(hori_line);
    }
    *segments = kept;

    // Split breakPoints set into visibility segments.
    let dimension = YDIM; // y-dimension
    vert_line.generate_visibility_edges_from_breakpoint_set(router, dimension);
}

// Processes an event for the vertical sweep used for computing the static
// orthogonal visibility graph.  This adds possible horizontal visibility
// segments to the segments list.
// The first pass is adding the event to the scanline, the second is for
// processing the event and the third for removing it from the scanline.
// upstream: orthogonal.cpp processEventVert()
fn process_event_vert(
    router: &mut Router,
    nodes: &mut NodeArena,
    scanline: &mut NodeSet,
    segments: &mut SegmentListWrapper,
    e: &Event,
    pass: u32,
) {
    let v = e.v;

    if ((pass == 1) && (e.type_ == EventType::Open)) || ((pass == 2) && (e.type_ == EventType::ConnPoint)) {
        nodes.insert_into_scanline(scanline, v);
    }

    if pass == 2 {
        if (e.type_ == EventType::Open) || (e.type_ == EventType::Close) {
            // Only difference between Open and Close is whether the line
            // segments are at the top or bottom of the shape.  Decide here.
            let line_y = if e.type_ == EventType::Open {
                nodes[v].min[YDIM]
            } else {
                nodes[v].max[YDIM]
            };

            // Shape edge positions.
            let min_shape = nodes[v].min[XDIM];
            let max_shape = nodes[v].max[XDIM];
            // As far as we can see.
            let (min_limit, max_limit, min_limit_max, max_limit_min) =
                nodes.find_first_point_above_and_below(v, XDIM, line_y);

            // Insert possible visibility segments.
            if min_limit_max >= max_limit_min {
                // These vertices represent the shape corners.
                let v_i1 = VertInf::new(router, DUMMY_ORTHOG_SHAPE_ID, Point::new(min_shape, line_y), true);
                let v_i2 = VertInf::new(router, DUMMY_ORTHOG_SHAPE_ID, Point::new(max_shape, line_y), true);

                // There are no overlapping shapes, so give full visibility.
                if min_limit < min_shape {
                    segments.insert(LineSegment::new(
                        &router.vert_infs,
                        min_limit,
                        min_shape,
                        line_y,
                        true,
                        None,
                        Some(v_i1),
                    ));
                }
                segments.insert(LineSegment::new(
                    &router.vert_infs,
                    min_shape,
                    max_shape,
                    line_y,
                    true,
                    Some(v_i1),
                    Some(v_i2),
                ));
                if max_shape < max_limit {
                    segments.insert(LineSegment::new(
                        &router.vert_infs,
                        max_shape,
                        max_limit,
                        line_y,
                        true,
                        Some(v_i2),
                        None,
                    ));
                }
            } else {
                // There are overlapping shapes along this shape edge.

                if (min_limit_max > min_limit) && (min_limit_max >= min_shape) {
                    let line = segments.insert(LineSegment::new(
                        &router.vert_infs,
                        min_limit,
                        min_limit_max,
                        line_y,
                        true,
                        None,
                        None,
                    ));
                    // Shape corner:
                    let v_i1 = VertInf::new(router, DUMMY_ORTHOG_SHAPE_ID, Point::new(min_shape, line_y), true);
                    let key = VertKey::of(&router.vert_infs, v_i1);
                    segments.list()[line].vert_infs.insert(key);
                }
                if (max_limit_min < max_limit) && (max_limit_min <= max_shape) {
                    let line = segments.insert(LineSegment::new(
                        &router.vert_infs,
                        max_limit_min,
                        max_limit,
                        line_y,
                        true,
                        None,
                        None,
                    ));
                    // Shape corner:
                    let v_i2 = VertInf::new(router, DUMMY_ORTHOG_SHAPE_ID, Point::new(max_shape, line_y), true);
                    let key = VertKey::of(&router.vert_infs, v_i2);
                    segments.list()[line].vert_infs.insert(key);
                }
            }
        } else if e.type_ == EventType::ConnPoint {
            // Connection point.
            let centre_vert = nodes[e.v].c.expect("ConnPoint event without vertex");
            let cp = router.vert_infs[centre_vert].point;
            let centre_vis = router.vert_infs[centre_vert].vis_directions;

            // As far as we can see.
            let min_limit = nodes.first_point_above(v, XDIM);
            let max_limit = nodes.first_point_below(v, XDIM);
            let in_shape = nodes.is_inside_shape(v, XDIM);

            // Insert if we have visibility in that direction and the segment
            // length is greater than zero.
            let mut line1: Option<usize> = None;
            let mut line2: Option<usize> = None;
            if (centre_vis & CONN_DIR_LEFT) != 0 && (min_limit < cp.x) {
                line1 = Some(segments.insert(LineSegment::new(
                    &router.vert_infs,
                    min_limit,
                    cp.x,
                    e.pos,
                    true,
                    None,
                    Some(centre_vert),
                )));
            }
            if (centre_vis & CONN_DIR_RIGHT) != 0 && (cp.x < max_limit) {
                line2 = Some(segments.insert(LineSegment::new(
                    &router.vert_infs,
                    cp.x,
                    max_limit,
                    e.pos,
                    true,
                    Some(centre_vert),
                    None,
                )));
                // If there was a line1, then we just merged with it, so
                // that pointer will be invalid (and now unnecessary).
                line1 = None;
            }
            if line1.is_none() && line2.is_none() {
                // Add a point segment for the centre point.
                segments.insert(LineSegment::new_point(&router.vert_infs, cp.x, e.pos, Some(centre_vert)));
            }

            if !in_shape {
                // This is not contained within a shape so add a normal
                // visibility graph point here too (since paths won't route
                // *through* connector endpoint vertices).
                if line1.is_some() || line2.is_some() {
                    let cent = VertInf::new(router, DUMMY_ORTHOG_ID, cp, true);
                    let key = VertKey::of(&router.vert_infs, cent);
                    if let Some(l1) = line1 {
                        segments.list()[l1].vert_infs.insert(key);
                    }
                    if let Some(l2) = line2 {
                        segments.list()[l2].vert_infs.insert(key);
                    }
                }
            }
        }
    }

    if ((pass == 3) && (e.type_ == EventType::Close)) || ((pass == 2) && (e.type_ == EventType::ConnPoint)) {
        // Clean up neighbour pointers.
        nodes.unlink_neighbours(v);

        let key = nodes.key(v);
        if e.type_ == EventType::ConnPoint {
            scanline.erase(&key);
        } else {
            // if (e->type == Close)
            let result = scanline.erase(&key);
            debug_assert!(result == 1);
        }
        // (`delete v`: the node stays in the arena until the sweep ends.)
    }
}

// Processes an event for the vertical sweep used for computing the static
// orthogonal visibility graph.  This adds possible vertical visibility
// segments to the segments list.
// The first pass is adding the event to the scanline, the second is for
// processing the event and the third for removing it from the scanline.
// upstream: orthogonal.cpp processEventHori()
fn process_event_hori(
    router: &mut Router,
    nodes: &mut NodeArena,
    scanline: &mut NodeSet,
    segments: &mut SegmentListWrapper,
    e: &Event,
    pass: u32,
) {
    let v = e.v;

    if ((pass == 1) && (e.type_ == EventType::Open)) || ((pass == 2) && (e.type_ == EventType::ConnPoint)) {
        nodes.insert_into_scanline(scanline, v);
    }

    if pass == 2 {
        if (e.type_ == EventType::Open) || (e.type_ == EventType::Close) {
            // Only difference between Open and Close is whether the line
            // segments are at the left or right of the shape.  Decide here.
            let line_x = if e.type_ == EventType::Open {
                nodes[v].min[XDIM]
            } else {
                nodes[v].max[XDIM]
            };

            // Shape edge positions.
            let min_shape = nodes[v].min[YDIM];
            let max_shape = nodes[v].max[YDIM];
            // As far as we can see.
            let (min_limit, max_limit, min_limit_max, max_limit_min) =
                nodes.find_first_point_above_and_below(v, YDIM, line_x);

            if min_limit_max >= max_limit_min {
                let line = segments.insert(LineSegment::new(
                    &router.vert_infs,
                    min_limit,
                    max_limit,
                    line_x,
                    false,
                    None,
                    None,
                ));

                // Shape corners:
                let v_i1 = VertInf::new(router, DUMMY_ORTHOG_SHAPE_ID, Point::new(line_x, min_shape), true);
                let v_i2 = VertInf::new(router, DUMMY_ORTHOG_SHAPE_ID, Point::new(line_x, max_shape), true);
                let k1 = VertKey::of(&router.vert_infs, v_i1);
                let k2 = VertKey::of(&router.vert_infs, v_i2);
                segments.list()[line].vert_infs.insert(k1);
                segments.list()[line].vert_infs.insert(k2);
            } else {
                if (min_limit_max > min_limit) && (min_limit_max >= min_shape) {
                    let line = segments.insert(LineSegment::new(
                        &router.vert_infs,
                        min_limit,
                        min_limit_max,
                        line_x,
                        false,
                        None,
                        None,
                    ));

                    // Shape corner:
                    let v_i1 = VertInf::new(router, DUMMY_ORTHOG_SHAPE_ID, Point::new(line_x, min_shape), true);
                    let k1 = VertKey::of(&router.vert_infs, v_i1);
                    segments.list()[line].vert_infs.insert(k1);
                }
                if (max_limit_min < max_limit) && (max_limit_min <= max_shape) {
                    let line = segments.insert(LineSegment::new(
                        &router.vert_infs,
                        max_limit_min,
                        max_limit,
                        line_x,
                        false,
                        None,
                        None,
                    ));

                    // Shape corner:
                    let v_i2 = VertInf::new(router, DUMMY_ORTHOG_SHAPE_ID, Point::new(line_x, max_shape), true);
                    let k2 = VertKey::of(&router.vert_infs, v_i2);
                    segments.list()[line].vert_infs.insert(k2);
                }
            }
        } else if e.type_ == EventType::ConnPoint {
            // Connection point.
            let centre_vert = nodes[e.v].c.expect("ConnPoint event without vertex");
            let cp = router.vert_infs[centre_vert].point;
            let centre_vis = router.vert_infs[centre_vert].vis_directions;

            // As far as we can see.
            let min_limit = nodes.first_point_above(v, YDIM);
            let max_limit = nodes.first_point_below(v, YDIM);

            // Insert if we have visibility in that direction and the segment
            // length is greater than zero.
            if (centre_vis & CONN_DIR_UP) != 0 && (min_limit < cp.y) {
                segments.insert(LineSegment::new(&router.vert_infs, min_limit, cp.y, e.pos, false, None, None));
            }

            if (centre_vis & CONN_DIR_DOWN) != 0 && (cp.y < max_limit) {
                segments.insert(LineSegment::new(&router.vert_infs, cp.y, max_limit, e.pos, false, None, None));
            }
        }
    }

    if ((pass == 3) && (e.type_ == EventType::Close)) || ((pass == 2) && (e.type_ == EventType::ConnPoint)) {
        // Clean up neighbour pointers.
        nodes.unlink_neighbours(v);

        let key = nodes.key(v);
        if e.type_ == EventType::ConnPoint {
            scanline.erase(&key);
        } else {
            // if (e->type == Close)
            let result = scanline.erase(&key);
            debug_assert!(result == 1);
        }
        // (`delete v`: the node stays in the arena until the sweep ends.)
    }
}

// Correct visibility for pins or connector endpoints on the leading or
// trailing edge of the visibility graph which may only have visibility in
// the outward direction where there will not be a possible path.
// upstream: orthogonal.cpp fixConnectionPointVisibilityOnOutsideOfVisibilityGraph()
pub fn fix_connection_point_visibility_on_outside_of_visibility_graph(
    router: &mut Router,
    nodes: &NodeArena,
    events: &[Event],
    total_events: usize,
    added_visibility: ConnDirFlags,
) {
    if total_events > 0 {
        let first_pos = events[0].pos;
        let mut index = 0usize;
        while index < total_events {
            if events[index].pos > first_pos {
                break;
            }

            if let Some(c) = nodes[events[index].v].c {
                router.vert_infs[c].vis_directions |= added_visibility;
            }
            index += 1;
        }
        index = 0;
        let last_pos = events[total_events - 1].pos;
        while index < total_events {
            let rev_index = total_events - 1 - index;
            if events[rev_index].pos < last_pos {
                break;
            }

            if let Some(c) = nodes[events[rev_index].v].c {
                router.vert_infs[c].vis_directions |= added_visibility;
            }
            index += 1;
        }
    }
}

// upstream: orthogonal.cpp generateStaticOrthogonalVisGraph()
pub fn generate_static_orthogonal_vis_graph(router: &mut Router) {
    let n = router.m_obstacles.len();
    let cpn = router.vertices.conns_size() as usize;
    // Set up the events for the vertical sweep.
    let mut total_events = (2 * n) + cpn;
    let mut events: Vec<Event> = Vec::with_capacity(total_events);
    let mut nodes = NodeArena::new();
    let obstacles = router.m_obstacles.clone();
    for &obstacle in obstacles.iter() {
        if let Some(junction) = Obstacle::as_junction(router, obstacle) {
            if !junction.position_fixed(router) {
                // Junctions that are free to move are not treated as obstacles.
                total_events -= 2;
                continue;
            }
        }

        let bbox = Obstacle::routing_box(router, obstacle);
        let mid_x = bbox.min.x + ((bbox.max.x - bbox.min.x) / 2.0);
        let v = nodes.new_obstacle_node(router, obstacle, mid_x);
        events.push(Event::new(EventType::Open, v, bbox.min.y));
        events.push(Event::new(EventType::Close, v, bbox.max.y));
    }

    // (DEBUGHANDLER obstacle-box reporting is not ported.)

    let mut curr = router.vertices.conns_begin();
    while let Some(c) = curr {
        if Some(c) == router.vertices.shapes_begin() {
            break;
        }
        if router.vert_infs[c].vis_directions == CONN_DIR_NONE {
            // This is a connector endpoint that is attached to a connection
            // pin on a shape, so it doesn't need to be given visibility.
            // Thus, skip it and record that there is one less total event.
            total_events -= 1;
            curr = router.vert_infs[c].lst_next;
            continue;
        }
        let point = router.vert_infs[c].point;

        let v = nodes.new_vert_node(router, c, point.x);
        events.push(Event::new(EventType::ConnPoint, v, point.y));
        curr = router.vert_infs[c].lst_next;
    }
    debug_assert!(events.len() == total_events);
    sort_events(&mut events);

    // Correct visibility for pins or connector endpoints on the leading or
    // trailing edge of the visibility graph which may only have visibility in
    // the outward direction where there will not be a possible path.  We
    // fix this by giving them visibility left and right.
    fix_connection_point_visibility_on_outside_of_visibility_graph(
        router,
        &nodes,
        &events,
        total_events,
        CONN_DIR_LEFT | CONN_DIR_RIGHT,
    );

    // Process the vertical sweep -- creating cadidate horizontal edges.
    // We do multiple passes over sections of the list so we can add relevant
    // entries to the scanline that might follow, before processing them.
    let mut segments = SegmentListWrapper::new();
    let mut scanline = NodeSet::new();
    let mut this_pos = if total_events > 0 { events[0].pos } else { 0.0 };
    let mut pos_start_index = 0usize;
    for i in 0..=total_events {
        // Progress reporting and continuation check.
        router.perform_continuation_check(TRANSACTION_PHASE_ORTHOGONAL_VISIBILITY_GRAPH_SCAN_X, i, total_events);

        // If we have finished the current scanline or all events, then we
        // process the events on the current scanline in a couple of passes.
        if (i == total_events) || (events[i].pos != this_pos) {
            let pos_finish_index = i;
            for pass in 2..=3 {
                for j in pos_start_index..pos_finish_index {
                    process_event_vert(router, &mut nodes, &mut scanline, &mut segments, &events[j], pass);
                }
            }

            if i == total_events {
                // We have cleaned up, so we can now break out of loop.
                break;
            }

            this_pos = events[i].pos;
            pos_start_index = i;
        }

        // Do the first sweep event handling -- building the correct
        // structure of the scanline.
        let pass = 1;
        process_event_vert(router, &mut nodes, &mut scanline, &mut segments, &events[i], pass);
    }
    debug_assert!(scanline.is_empty());
    // (delete events[i]: dropped below.)

    sort_segment_list(segments.list());

    // Set up the events for the horizontal sweep.
    // (Fresh nodes: upstream allocates new Node objects.)
    let mut vert_segments = SegmentListWrapper::new();
    events.clear();
    let mut nodes = NodeArena::new();
    for &obstacle in obstacles.iter() {
        if let Some(junction) = Obstacle::as_junction(router, obstacle) {
            if !junction.position_fixed(router) {
                // Junctions that are free to move are not treated as obstacles.
                continue;
            }
        }
        let bbox = Obstacle::routing_box(router, obstacle);
        let mid_y = bbox.min.y + ((bbox.max.y - bbox.min.y) / 2.0);
        let v = nodes.new_obstacle_node(router, obstacle, mid_y);
        events.push(Event::new(EventType::Open, v, bbox.min.x));
        events.push(Event::new(EventType::Close, v, bbox.max.x));
    }
    let mut curr = router.vertices.conns_begin();
    while let Some(c) = curr {
        if Some(c) == router.vertices.shapes_begin() {
            break;
        }
        if router.vert_infs[c].vis_directions == CONN_DIR_NONE {
            // This is a connector endpoint that is attached to a connection
            // pin on a shape, so it doesn't need to be given visibility.
            // Thus, skip it.
            curr = router.vert_infs[c].lst_next;
            continue;
        }
        let point = router.vert_infs[c].point;

        let v = nodes.new_vert_node(router, c, point.y);
        events.push(Event::new(EventType::ConnPoint, v, point.x));
        curr = router.vert_infs[c].lst_next;
    }
    debug_assert!(events.len() == total_events);
    sort_events(&mut events);

    // Correct visibility for pins or connector endpoints on the leading or
    // trailing edge of the visibility graph which may only have visibility in
    // the outward direction where there will not be a possible path.  We
    // fix this by giving them visibility up and down.
    fix_connection_point_visibility_on_outside_of_visibility_graph(
        router,
        &nodes,
        &events,
        total_events,
        CONN_DIR_UP | CONN_DIR_DOWN,
    );

    // Process the horizontal sweep -- creating vertical visibility edges.
    this_pos = if total_events > 0 { events[0].pos } else { 0.0 };
    pos_start_index = 0;
    for i in 0..=total_events {
        // Progress reporting and continuation check.
        router.perform_continuation_check(TRANSACTION_PHASE_ORTHOGONAL_VISIBILITY_GRAPH_SCAN_Y, i, total_events);

        // If we have finished the current scanline or all events, then we
        // process the events on the current scanline in a couple of passes.
        if (i == total_events) || (events[i].pos != this_pos) {
            let pos_finish_index = i;
            for pass in 2..=3 {
                for j in pos_start_index..pos_finish_index {
                    process_event_hori(router, &mut nodes, &mut scanline, &mut vert_segments, &events[j], pass);
                }
            }

            // Process the merged line segments.
            sort_segment_list(vert_segments.list());
            let mut vlist = std::mem::take(vert_segments.list());
            for curr in vlist.iter_mut() {
                intersect_segments(router, segments.list(), curr);
            }
            vlist.clear();
            *vert_segments.list() = vlist;

            if i == total_events {
                // We have cleaned up, so we can now break out of loop.
                break;
            }

            this_pos = events[i].pos;
            pos_start_index = i;
        }

        // Do the first sweep event handling -- building the correct
        // structure of the scanline.
        let pass = 1;
        process_event_hori(router, &mut nodes, &mut scanline, &mut vert_segments, &events[i], pass);
    }
    debug_assert!(scanline.is_empty());
    drop(events);

    // Add portions of horizontal lines that are after the final vertical
    // position we considered.
    let remaining = std::mem::take(segments.list());
    for mut hori_line in remaining {
        hori_line.add_edge_horizontal(router);

        let dim = XDIM; // x-dimension
        hori_line.generate_visibility_edges_from_breakpoint_set(router, dim);
    }
}

//============================================================================
//                           Path Adjustment code
//============================================================================
//
// (Second half of upstream orthogonal.cpp, from `typedef std::pair<Point,
// Point> RectBounds` on: not yet ported.  Append it here.)

// upstream: orthogonal.cpp improveOrthogonalRoutes()
pub fn improve_orthogonal_routes(router: &mut Router) {
    let _ = router;
    todo!("upstream: orthogonal.cpp improveOrthogonalRoutes — not yet ported")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsigned_pair_orders_lexicographically() {
        let a = UnsignedPair::new(5, 2);
        let b = UnsignedPair::new(2, 7);
        let c = UnsignedPair::new(3, 1);
        assert!(c < a && a < b);
        assert_eq!(UnsignedPair::new(4, 9), UnsignedPair::new(9, 4));
    }
}
