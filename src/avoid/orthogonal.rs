//! Faithful port of upstream `libavoid/orthogonal.{h,cpp}`.
//!
//! Both halves of orthogonal.cpp are ported: the static orthogonal
//! visibility graph (`generateStaticOrthogonalVisGraph`) and the path
//! adjustment / nudging code (`NudgingShiftSegment`, `CmpLineOrder`,
//! `linesort`, `ImproveOrthogonalRoutes`, `improveOrthogonalRoutes`) in the
//! "Path Adjustment code" section at the end of this file.
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
//!
//! # Pointer ties (`// PTR-ORDER:`) that change the graph
//! * `CmpVertInf` (`u < v` for two vertices at the same point) decides
//!   which of several coincident vertices `PosVertInf` keeps (it treats all
//!   dummies -- `dummyOrthogID` *and* `dummyOrthogShapeID`, since `VertID`
//!   equality ignores props -- at one position as equal) and so which one
//!   gets the edges and, when a plain and a shape-edge dummy coincide, the
//!   `orthogVisPropFlags`.
//! * `compare_events` ties (same position and type, e.g. aligned shape
//!   edges) decide the processing order and thus the vertex creation order.
//!   Upstream computes `(int)(ea->v - eb->v)` on unrelated `Node*`s, whose
//!   sign is only meaningful if the nodes lie in one array.
//! Both are modelled as creation order (serial / node index), which is what
//! upstream computes with a monotonic allocator; with glibc malloc the
//! upstream result depends on the heap history (see
//! `tools/oracle/orthogonal_vis_oracle.cpp`).  `CmpNodePos` ties do not
//! influence this graph (all scanline queries here are symmetric), only
//! `buildOrthogonalChannelInfo`.
//!
//! # Nudging: sorts, ties and pointer order
//! * `NudgingShiftSegment::mergeWith` `std::sort(indexes, CmpIndexes)`:
//!   ties (equal alt-dimension coordinates) cannot matter -- every merged
//!   index gets the same `dimension` coordinate right after the sort, so
//!   tied indexes name equal points, and only point values and the index
//!   set are used later.  Ported as libstdc++'s `__insertion_sort` (exactly
//!   what `std::sort` runs for <= 16 elements).
//! * `linesort` (partial order `CmpLineOrder` with the `comparable` flag):
//!   upstream's hand-written insertion/deferral algorithm, ported verbatim.
//! * `potentialConstraints.sort()` (`std::list::sort`, stable merge sort):
//!   a stable sort of a strict weak order yields a unique permutation, so
//!   `slice::sort_by` (stable) is identical; ties keep list order.
//! * `buildOrthogonalChannelInfo` (scanline.rs) breaks `CmpNodePos` ties by
//!   `ShiftSegment*` / `Obstacle*` address (`// PTR-ORDER:` there); each
//!   `NudgingShiftSegment` takes its creation serial at the upstream
//!   `new NudgingShiftSegment` site, i.e. after every obstacle.
//! * Everything else is pointer *identity* (`connRef == rhs->connRef`,
//!   `constraint->left == vs[i]`), never pointer order; region building and
//!   VPSC variable/constraint creation follow list order exactly.
//!   `m_shared_path_connectors_with_common_endpoints` is keyed by connector
//!   ids (`UnsignedPair`, truncated to `unsigned short` as upstream) and only
//!   queried with `count()`.

#![allow(dead_code)]
#![allow(
    clippy::collapsible_if,
    clippy::collapsible_else_if,
    clippy::needless_range_loop,
    clippy::too_many_arguments,
    clippy::if_same_then_else,
    clippy::nonminimal_bool,
    clippy::bool_comparison
)]

use std::cmp::Ordering;
use std::collections::{BTreeSet, VecDeque};

use crate::avoid::arena::{ConnIdx, VertIdx};
use crate::avoid::connector::{
    split_branching_segments, ConnType, ConnectorCrossings, PtOrderMap, CROSSING_SHARES_PATH_AT_END,
};
use crate::avoid::geomtypes::{cpp_max, cpp_min, Point, PointOrd, Polygon, PolygonInterface, XDIM, YDIM};
use crate::avoid::graph::EdgeInf;
use crate::avoid::obstacle::Obstacle;
use crate::avoid::router::{
    format_g, Router, RoutingOption, RoutingParameter, TRANSACTION_PHASE_ORTHOGONAL_NUDGING_X,
    TRANSACTION_PHASE_ORTHOGONAL_NUDGING_Y, TRANSACTION_PHASE_ORTHOGONAL_VISIBILITY_GRAPH_SCAN_X,
    TRANSACTION_PHASE_ORTHOGONAL_VISIBILITY_GRAPH_SCAN_Y,
};
use crate::avoid::scanline::{
    build_connector_route_checkpoint_cache, build_orthogonal_channel_info, clear_connector_route_checkpoint_cache,
    sort_events, Event, EventType, NodeArena, NodeSet, ShiftSegment, ShiftSegmentBase, CHANNEL_MAX,
};
use crate::avoid::vpsc::{ConIdx, Constraint, IncSolver, VarIdx, Variable};
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
// Second half of upstream orthogonal.cpp: `CmpIndexes` and
// `NudgingShiftSegment` (defined near the top of the file upstream, but only
// used here), `RectBounds`, `buildOrthogonalNudgingSegments`,
// `CmpLineOrder`, `linesort`, `PotentialSegmentConstraint`,
// `ImproveOrthogonalRoutes` and `improveOrthogonalRoutes`.
//
// Data model:
// * Upstream `ShiftSegmentList` (`std::list<ShiftSegment *>`) of
//   `NudgingShiftSegment`s is a `Vec<NudgingShiftSegment>` in list order;
//   segments are owned by the list they are in (upstream deletes them when
//   they leave the last list), and `ShiftSegmentPtrList prevVars` holds
//   indices into the current region.
// * `NudgingShiftSegment::connRef->displayRoute().ps[i]` is
//   `router.conns[conn_ref].m_display_route.ps[i]` (the display routes are
//   always populated by `simplifyOrthogonalRoutes()` before any segment
//   exists).
// * `Variable *` / `Constraint *` are indices into the caller-owned `vs` /
//   `cs` vectors handed to `IncSolver` (see vpsc.rs).  Pointer comparisons
//   `constraint->left == vs[range.first]` become index comparisons (each
//   variable occurs exactly once in `vs`).

/// Test hook (not upstream): when enabled, `nudgeOrthogonalRoutes` records
/// every region's sorted segment list and the VPSC result, in the format
/// of the trace calls that `tools/oracle/nudging_oracle.sh` patches into a
/// copy of the C++ (`tests/nudging_oracle.rs`).  Disabled by default; the
/// trace is per thread.
pub fn nudge_oracle_trace_enable(enable: bool) {
    NUDGE_ORACLE_TRACE.with(|t| *t.borrow_mut() = if enable { Some(Vec::new()) } else { None });
}

/// Takes the lines recorded since the last call (see
/// [`nudge_oracle_trace_enable`]).
pub fn nudge_oracle_trace_take() -> Vec<String> {
    NUDGE_ORACLE_TRACE.with(|t| t.borrow_mut().as_mut().map(std::mem::take).unwrap_or_default())
}

thread_local! {
    static NUDGE_ORACLE_TRACE: std::cell::RefCell<Option<Vec<String>>> = const { std::cell::RefCell::new(None) };
}

fn nudge_oracle_tracing() -> bool {
    NUDGE_ORACLE_TRACE.with(|t| t.borrow().is_some())
}

fn nudge_oracle_trace(s: String) {
    NUDGE_ORACLE_TRACE.with(|t| {
        if let Some(v) = t.borrow_mut().as_mut() {
            v.push(s);
        }
    });
}

fn nudge_oracle_g17(v: f64) -> String {
    if v.is_nan() {
        return "nan".to_string();
    }
    format_g(v, 17)
}

/// libstdc++ `std::__insertion_sort`, which is exactly what `std::sort`
/// does for ranges of at most 16 elements (`__introsort_loop` returns
/// immediately and `__final_insertion_sort` insertion-sorts the range).
/// Only used for `NudgingShiftSegment::mergeWith`, where ties cannot change
/// the result (see the note there), so it is also used for longer ranges.
fn libstdcxx_insertion_sort<T: Copy, F: FnMut(&T, &T) -> bool>(v: &mut [T], comp: &mut F) {
    if v.is_empty() {
        return;
    }
    for i in 1..v.len() {
        let val = v[i];
        if comp(&val, &v[0]) {
            // std::move_backward(first, i, i + 1); *first = val;
            let mut k = i;
            while k > 0 {
                v[k] = v[k - 1];
                k -= 1;
            }
            v[0] = val;
        } else {
            // std::__unguarded_linear_insert(i)
            let mut last = i;
            let mut next = i - 1;
            while comp(&val, &v[next]) {
                v[last] = v[next];
                last = next;
                next -= 1;
            }
            v[last] = val;
        }
    }
}

// Used to sort points when merging NudgingShiftSegments.
// Sorts the indexes, by point position in one dimension.
// upstream: orthogonal.cpp CmpIndexes::operator()()
fn cmp_indexes(route: &Polygon, dimension: usize, lhs: usize, rhs: usize) -> bool {
    route.ps[lhs][dimension] < route.ps[rhs][dimension]
}

/// Upstream `class NudgingShiftSegment : public ShiftSegment`.
// upstream: orthogonal.cpp class NudgingShiftSegment
#[derive(Clone, Debug)]
pub struct NudgingShiftSegment {
    base: ShiftSegmentBase,
    pub conn_ref: ConnIdx,
    /// Index of the segment's solver variable in the `vs` vector of the
    /// current `nudgeOrthogonalRoutes` region.
    pub variable: Option<VarIdx>,
    pub indexes: Vec<usize>,
    pub fixed: bool,
    pub final_segment: bool,
    pub ends_in_shape: bool,
    pub single_connected_segment: bool,
    pub checkpoints: Vec<Point>,
    s_bend: bool,
    z_bend: bool,
}

impl NudgingShiftSegment {
    /// For shiftable segments.
    // upstream: orthogonal.cpp NudgingShiftSegment::NudgingShiftSegment(conn, low, high, isSBend, isZBend, dim, minLim, maxLim)
    pub fn new_shiftable(
        router: &mut Router,
        conn: ConnIdx,
        low: usize,
        high: usize,
        is_s_bend: bool,
        is_z_bend: bool,
        dim: usize,
        min_lim: f64,
        max_lim: f64,
    ) -> NudgingShiftSegment {
        let mut seg = NudgingShiftSegment {
            base: ShiftSegmentBase::new(router, dim),
            conn_ref: conn,
            variable: None,
            indexes: Vec::new(),
            fixed: false,
            final_segment: false,
            ends_in_shape: false,
            single_connected_segment: false,
            checkpoints: Vec::new(),
            s_bend: is_s_bend,
            z_bend: is_z_bend,
        };
        seg.indexes.push(low);
        seg.indexes.push(high);
        seg.base.min_space_limit = min_lim;
        seg.base.max_space_limit = max_lim;
        seg
    }

    /// For fixed segments.
    // upstream: orthogonal.cpp NudgingShiftSegment::NudgingShiftSegment(conn, low, high, dim)
    pub fn new_fixed(router: &mut Router, conn: ConnIdx, low: usize, high: usize, dim: usize) -> NudgingShiftSegment {
        let mut seg = NudgingShiftSegment {
            base: ShiftSegmentBase::new(router, dim),
            conn_ref: conn,
            variable: None,
            indexes: Vec::new(),
            fixed: true,
            final_segment: false,
            ends_in_shape: false,
            single_connected_segment: false,
            checkpoints: Vec::new(),
            s_bend: false,
            z_bend: false,
        };
        seg.indexes.push(low);
        seg.indexes.push(high);
        // This has no space to shift.
        seg.base.min_space_limit = seg.low_point_of(router)[dim];
        seg.base.max_space_limit = seg.low_point_of(router)[dim];
        seg
    }

    #[inline]
    fn route<'r>(&self, router: &'r Router) -> &'r Polygon {
        &router.conns[self.conn_ref].m_display_route
    }

    // upstream: orthogonal.cpp NudgingShiftSegment::lowPoint()
    #[inline]
    pub fn low_point_of(&self, router: &Router) -> Point {
        self.route(router).ps[self.indexes[0]]
    }

    // upstream: orthogonal.cpp NudgingShiftSegment::highPoint()
    #[inline]
    pub fn high_point_of(&self, router: &Router) -> Point {
        self.route(router).ps[self.indexes[self.indexes.len() - 1]]
    }

    #[inline]
    pub fn min_space_limit(&self) -> f64 {
        self.base.min_space_limit
    }

    #[inline]
    pub fn max_space_limit(&self) -> f64 {
        self.base.max_space_limit
    }

    #[inline]
    pub fn dimension(&self) -> usize {
        self.base.dimension
    }

    #[inline]
    pub fn is_s_bend(&self) -> bool {
        self.s_bend
    }

    #[inline]
    pub fn is_z_bend(&self) -> bool {
        self.z_bend
    }

    // upstream: orthogonal.cpp NudgingShiftSegment::nudgeDistance()
    pub fn nudge_distance(&self, router: &Router) -> f64 {
        router.routing_parameter(RoutingParameter::IdealNudgingDistance)
    }

    /// Creates the solver variable, appends it to `vs` and records its
    /// index (upstream `variable = new Variable(..)`; the caller then does
    /// `vs.push_back(currSegment->variable)`).
    // upstream: orthogonal.cpp NudgingShiftSegment::createSolverVariable()
    pub fn create_solver_variable(&mut self, router: &Router, just_unifying: bool, vs: &mut Vec<Variable>) {
        let nudge_final_segments = router.routing_option(RoutingOption::NudgeOrthogonalSegmentsConnectedToShapes);
        let mut var_id = FREE_SEGMENT_ID;
        let mut var_pos = self.low_point_of(router)[self.base.dimension];
        let mut weight = FREE_WEIGHT;
        if nudge_final_segments && self.final_segment {
            weight = STRONG_WEIGHT;

            if self.single_connected_segment && !just_unifying {
                // This is a single segment connector bridging
                // two shapes.  So, we want to try to keep it
                // centred rather than shift it.
                // Don't do this during Unifying stage, or else
                // these connectors could end up at slightly
                // different positions and get the wrong ordering
                // for nudging.
                weight = STRONGER_WEIGHT;
            }
        } else if !self.checkpoints.is_empty() {
            weight = STRONG_WEIGHT;
        } else if self.zigzag() {
            debug_assert!(self.base.min_space_limit > -CHANNEL_MAX);
            debug_assert!(self.base.max_space_limit < CHANNEL_MAX);

            // For zigzag bends, take the middle as ideal.
            var_pos = self.base.min_space_limit + ((self.base.max_space_limit - self.base.min_space_limit) / 2.0);
        } else if self.fixed {
            // Fixed segments shouldn't get moved.
            weight = FIXED_WEIGHT;
            var_id = FIXED_SEGMENT_ID;
        } else if !self.final_segment {
            // Set a higher weight for c-bends to stop them sometimes
            // getting pushed out into channels by more-free connectors
            // to the "inner" side of them.
            weight = STRONG_WEIGHT;
        }

        vs.push(Variable::with_weight(var_id, var_pos, weight));
        self.variable = Some(vs.len() - 1);
    }

    // upstream: orthogonal.cpp NudgingShiftSegment::updatePositionsFromSolver()
    pub fn update_positions_from_solver(&self, router: &mut Router, vs: &[Variable], just_unifying: bool) {
        let _ = just_unifying; // (only used by the DEBUGHANDLER build)
        if self.fixed {
            return;
        }
        let mut new_pos = vs[self.variable.expect("solver variable")].final_position;

        // The solver can sometimes cause variables to be outside their
        // limits by a tiny amount, since all variables are held by
        // weights.  Thus, just make sure they stay in their limits.
        new_pos = cpp_max(new_pos, self.base.min_space_limit);
        new_pos = cpp_min(new_pos, self.base.max_space_limit);

        let dimension = self.base.dimension;
        let route = &mut router.conns[self.conn_ref].m_display_route;
        for it in 0..self.indexes.len() {
            let index = self.indexes[it];
            route.ps[index][dimension] = new_pos;
        }
    }

    // upstream: orthogonal.cpp NudgingShiftSegment::fixedOrder()
    pub fn fixed_order(&self, router: &Router, is_fixed: &mut bool) -> i32 {
        let nudge_dist = self.nudge_distance(router);
        let pos = self.low_point_of(router)[self.base.dimension];
        let min_limited = (pos - self.base.min_space_limit) < nudge_dist;
        let max_limited = (self.base.max_space_limit - pos) < nudge_dist;

        if self.fixed || (min_limited && max_limited) {
            *is_fixed = true;
            return 0;
        } else if min_limited {
            return 1;
        } else if max_limited {
            return -1;
        }
        0
    }

    // upstream: orthogonal.cpp NudgingShiftSegment::order()
    pub fn order(&self, router: &Router) -> i32 {
        if self.low_c(router) {
            return -1;
        } else if self.high_c(router) {
            return 1;
        }
        0
    }

    // upstream: orthogonal.cpp NudgingShiftSegment::zigzag()
    #[inline]
    pub fn zigzag(&self) -> bool {
        self.s_bend || self.z_bend
    }

    /// This counts segments that are collinear and share an endpoint as
    /// overlapping.  This allows them to be nudged apart where possible.
    // upstream: orthogonal.cpp NudgingShiftSegment::overlapsWith()
    pub fn nudging_overlaps_with(&self, rhs: &NudgingShiftSegment, dim: usize, router: &Router) -> bool {
        let alt_dim = (dim + 1) % 2;
        let low_pt = self.low_point_of(router);
        let high_pt = self.high_point_of(router);
        let rhs_low_pt = rhs.low_point_of(router);
        let rhs_high_pt = rhs.high_point_of(router);
        if (low_pt[alt_dim] < rhs_high_pt[alt_dim]) && (rhs_low_pt[alt_dim] < high_pt[alt_dim]) {
            // The segments overlap.
            if (self.base.min_space_limit <= rhs.base.max_space_limit)
                && (rhs.base.min_space_limit <= self.base.max_space_limit)
            {
                return true;
            }
        } else if (low_pt[alt_dim] == rhs_high_pt[alt_dim]) || (rhs_low_pt[alt_dim] == high_pt[alt_dim]) {
            let nudge_colinear_segments =
                router.routing_option(RoutingOption::NudgeOrthogonalTouchingColinearSegments);

            if (self.base.min_space_limit <= rhs.base.max_space_limit)
                && (rhs.base.min_space_limit <= self.base.max_space_limit)
            {
                // The segments could touch at one end.
                if router.routing_parameter(RoutingParameter::FixedSharedPathPenalty) > 0.0 {
                    // If we are routing with a fixedSharedPathPenalty
                    // then we don't want these segments to ever touch
                    // or slide past each other, so they are always
                    // considered to be overlapping.
                    return true;
                } else if (rhs.s_bend && self.s_bend) || (rhs.z_bend && self.z_bend) {
                    // Count them as overlapping for nudging if they
                    // are both s-bends or both z-bends, i.e., when
                    // the ordering would matter.
                    return nudge_colinear_segments;
                } else if (rhs.final_segment && self.final_segment) && (rhs.conn_ref == self.conn_ref) {
                    return nudge_colinear_segments;
                }
            }
        }
        false
    }

    /// These segments are allowed to drift into alignment but don't have to.
    // upstream: orthogonal.cpp NudgingShiftSegment::canAlignWith()
    pub fn can_align_with(&self, rhs: &NudgingShiftSegment, dim: usize) -> bool {
        let _ = dim;

        if self.conn_ref != rhs.conn_ref {
            return false;
        }

        // Don't allow segments of the same connector to drift together
        // where one of them goes via a checkpoint.  We want the path
        // through the checkpoint to be maintained.
        let has_checkpoints = !self.checkpoints.is_empty();
        let rhs_has_checkpoints = !rhs.checkpoints.is_empty();
        if has_checkpoints || rhs_has_checkpoints {
            return false;
        }
        true
    }

    /// These segments should align with each other.
    // upstream: orthogonal.cpp NudgingShiftSegment::shouldAlignWith()
    pub fn should_align_with(&self, rhs: &NudgingShiftSegment, dim: usize, router: &Router) -> bool {
        if (self.conn_ref == rhs.conn_ref)
            && self.final_segment
            && rhs.final_segment
            && self.nudging_overlaps_with(rhs, dim, router)
        {
            // If both the segments are in shapes then we know limits
            // and can align.  Otherwise we do this just for segments
            // that are very close together, since these will often
            // prevent nudging, or force it to have a tiny separation
            // value.
            if (self.ends_in_shape && rhs.ends_in_shape)
                || ((self.low_point_of(router)[dim] - rhs.low_point_of(router)[dim]).abs() < 10.0)
            {
                return true;
            }
        } else if (self.conn_ref == rhs.conn_ref) &&
            // Not both final
            ((self.final_segment & rhs.final_segment) != true)
        {
            let has_checkpoints = !self.checkpoints.is_empty();
            let rhs_has_checkpoints = !rhs.checkpoints.is_empty();

            if has_checkpoints != rhs_has_checkpoints {
                // At least one segment has checkpoints, but not both.

                let alt_dim = (dim + 1) % 2;
                let space = (self.low_point_of(router)[dim] - rhs.low_point_of(router)[dim]).abs();
                // (Uninitialised upstream; only read when couldTouch.)
                let mut touch_pos = 0.0;
                let mut could_touch = false;
                if self.low_point_of(router)[alt_dim] == rhs.high_point_of(router)[alt_dim] {
                    could_touch = true;
                    touch_pos = self.low_point_of(router)[alt_dim];
                } else if self.high_point_of(router)[alt_dim] == rhs.low_point_of(router)[alt_dim] {
                    could_touch = true;
                    touch_pos = self.high_point_of(router)[alt_dim];
                }

                // We should align these so long as they are close
                // together (<= 10) and there isn't a checkpoint at the
                // touch point, i.e., we'd be altering the edges leading
                // into the checkpoint.  We want to keep these in place
                // and opportunistically move other edges to align with
                // them.
                return could_touch
                    && (space <= 10.0)
                    && !self.has_checkpoint_at_position(touch_pos, alt_dim)
                    && !rhs.has_checkpoint_at_position(touch_pos, alt_dim);
            }
        }
        false
    }

    /// Used for merging segments with end segments that should appear as
    /// a single segment.
    // upstream: orthogonal.cpp NudgingShiftSegment::mergeWith()
    pub fn merge_with(&mut self, rhs: &NudgingShiftSegment, dim: usize, router: &mut Router) {
        // Adjust limits.
        self.base.min_space_limit = cpp_max(self.base.min_space_limit, rhs.base.min_space_limit);
        self.base.max_space_limit = cpp_min(self.base.max_space_limit, rhs.base.max_space_limit);

        // Find a new position for the segment, taking into account
        // the two original positions and the combined limits.
        let dimension = self.base.dimension;
        let mut segment_pos = self.low_point_of(router)[dimension];
        let segment2_pos = rhs.low_point_of(router)[dimension];
        if segment2_pos < segment_pos {
            segment_pos -= (segment_pos - segment2_pos) / 2.0;
        } else if segment2_pos > segment_pos {
            segment_pos += (segment2_pos - segment_pos) / 2.0;
        }
        segment_pos = cpp_max(self.base.min_space_limit, segment_pos);
        segment_pos = cpp_min(self.base.max_space_limit, segment_pos);

        // Merge the index lists and sort the new list.
        self.indexes.extend_from_slice(&rhs.indexes);
        let alt_dim = (dim + 1) % 2;
        // std::sort(indexes, CmpIndexes(connRef, altDim)).  Ties (equal
        // altDim coordinates) cannot influence anything: every index gets
        // the same `dimension` coordinate just below, so tied indexes name
        // equal points, and only the point values (lowPoint()/highPoint())
        // and the full index set are ever used.  Without ties every correct
        // sort gives the same order; libstdc++ uses exactly this insertion
        // sort for up to 16 elements.
        {
            let route = &router.conns[self.conn_ref].m_display_route;
            libstdcxx_insertion_sort(&mut self.indexes, &mut |&l: &usize, &r: &usize| {
                cmp_indexes(route, alt_dim, l, r)
            });
        }

        // Apply the new positon to all points to keep them constant.
        let route = &mut router.conns[self.conn_ref].m_display_route;
        for it in 0..self.indexes.len() {
            let index = self.indexes[it];
            route.ps[index][dimension] = segment_pos;
        }
    }

    // upstream: orthogonal.cpp NudgingShiftSegment::hasCheckpointAtPosition()
    pub fn has_checkpoint_at_position(&self, position: f64, dim: usize) -> bool {
        for cp in 0..self.checkpoints.len() {
            if self.checkpoints[cp][dim] == position {
                return true;
            }
        }
        false
    }

    // This is true if this is a cBend and its adjoining points
    // are at lower positions.
    // upstream: orthogonal.cpp NudgingShiftSegment::lowC()
    fn low_c(&self, router: &Router) -> bool {
        if !self.final_segment
            && !self.zigzag()
            && !self.fixed
            && (self.base.min_space_limit == self.low_point_of(router)[self.base.dimension])
        {
            return true;
        }
        false
    }

    // This is true if this is a cBend and its adjoining points
    // are at higher positions.
    // upstream: orthogonal.cpp NudgingShiftSegment::highC()
    fn high_c(&self, router: &Router) -> bool {
        if !self.final_segment
            && !self.zigzag()
            && !self.fixed
            && (self.base.max_space_limit == self.low_point_of(router)[self.base.dimension])
        {
            return true;
        }
        false
    }
}

impl ShiftSegment<Router> for NudgingShiftSegment {
    fn base(&self) -> &ShiftSegmentBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut ShiftSegmentBase {
        &mut self.base
    }
    // upstream: orthogonal.cpp NudgingShiftSegment::lowPoint()
    fn low_point(&self, ctx: &Router) -> Point {
        self.low_point_of(ctx)
    }
    // upstream: orthogonal.cpp NudgingShiftSegment::highPoint()
    fn high_point(&self, ctx: &Router) -> Point {
        self.high_point_of(ctx)
    }
    // upstream: orthogonal.cpp NudgingShiftSegment::overlapsWith()
    fn overlaps_with(&self, rhs: &Self, dim: usize, ctx: &Router) -> bool {
        self.nudging_overlaps_with(rhs, dim, ctx)
    }
    // upstream: orthogonal.cpp NudgingShiftSegment::immovable()
    fn immovable(&self, _ctx: &Router) -> bool {
        !self.zigzag()
    }
}

// upstream: orthogonal.cpp typedef std::pair<Point, Point> RectBounds
type RectBounds = (Point, Point);

// upstream: orthogonal.cpp insideRectBounds()
fn inside_rect_bounds(point: &Point, rect_bounds: &RectBounds) -> bool {
    let zero = Point::new(0.0, 0.0);
    if (rect_bounds.0 == zero) && (rect_bounds.1 == zero) {
        // We can't be inside the invalid rectangle.
        return false;
    }

    for i in 0..2 {
        if point[i] < rect_bounds.0[i] {
            return false;
        }
        if point[i] > rect_bounds.1[i] {
            return false;
        }
    }
    true
}

// upstream: orthogonal.cpp buildOrthogonalNudgingSegments()
fn build_orthogonal_nudging_segments(router: &mut Router, dim: usize, segment_list: &mut Vec<NudgingShiftSegment>) {
    if router.routing_parameter(RoutingParameter::SegmentPenalty) == 0.0 {
        // The nudging code assumes the routes are pretty optimal.  This will
        // only be true if a segment penalty is set, so just return if this
        // is not the case.
        return;
    }
    let nudge_final_segments = router.routing_option(RoutingOption::NudgeOrthogonalSegmentsConnectedToShapes);
    let mut shape_limits: Vec<RectBounds> = Vec::new();
    if nudge_final_segments {
        // If we're going to nudge final segments, then cache the shape
        // rectangles to save us rebuilding them multiple times.
        let n = router.m_obstacles.len();
        // (Upstream value-initialises `Point`s, whose default constructor
        // leaves x/y uninitialised; every obstacle is a shape or a
        // junction, so every entry is overwritten below.)
        shape_limits = vec![(Point::default(), Point::default()); n];

        let zero_buffer_dist = 0.0;

        for i in 0..n {
            let obstacle = router.m_obstacles[i];
            let shape = Obstacle::as_shape(router, obstacle);
            let junction = Obstacle::as_junction(router, obstacle);
            if shape.is_some() {
                // Take the real bounds of the shape
                let b_box = router.obstacles[obstacle].polygon().offset_bounding_box(zero_buffer_dist);
                shape_limits[i] = (b_box.min, b_box.max);
            } else if let Some(junction) = junction {
                // Don't nudge segments attached to junctions,
                // so just use the junction position here.
                let pos = junction.position(router);
                shape_limits[i] = (pos, pos);
            }
        }
    }

    let alt_dim = (dim + 1) % 2;
    // For each connector.
    let conn_refs = router.conn_refs.clone();
    for &curr in conn_refs.iter() {
        if router.conns[curr].routing_type() != ConnType::ConnTypeOrthogonal {
            continue;
        }
        // (A read-only copy: nothing below modifies the display route.)
        let display_route: Polygon = router.conns[curr].display_route().clone();
        // Determine all line segments that we are interested in shifting.
        // We don't consider the first or last segment of a path.
        for i in 1..display_route.size() {
            if display_route.ps[i - 1][dim] == display_route.ps[i][dim] {
                // It's a segment in the dimension we are processing,
                let mut index_low = i - 1;
                let mut index_high = i;
                if display_route.ps[i - 1][alt_dim] == display_route.ps[i][alt_dim] {
                    // This is a zero length segment, so ignore it.
                    continue;
                } else if display_route.ps[i - 1][alt_dim] > display_route.ps[i][alt_dim] {
                    index_low = i;
                    index_high = i - 1;
                }

                // Find the checkpoints on the current segment and the
                // checkpoints on the adjoining segments that aren't on
                // the corner (hence the +1 and -1 modifiers).
                let checkpoints = display_route.checkpoints_on_segment(i - 1, 0);
                // (size_t arithmetic: `i - 2` wraps for i == 1, as upstream.)
                let prev_checkpoints = display_route.checkpoints_on_segment(i.wrapping_sub(2), -1);
                let next_checkpoints = display_route.checkpoints_on_segment(i, 1);
                let has_checkpoints = !checkpoints.is_empty();
                if has_checkpoints && !nudge_final_segments {
                    // This segment includes one of the routing
                    // checkpoints so we shouldn't shift it.
                    let seg = NudgingShiftSegment::new_fixed(router, curr, index_low, index_high, dim);
                    segment_list.push(seg);
                    continue;
                }

                let this_pos = display_route.ps[i][dim];

                if (i == 1) || ((i + 1) == display_route.size()) {
                    // Is first or last segment of route.

                    if nudge_final_segments {
                        // Determine available space for nudging these
                        // final segments.
                        let mut min_lim = -CHANNEL_MAX;
                        let mut max_lim = CHANNEL_MAX;

                        // If the position of the opposite end of the
                        // attached segment is within the shape boundaries
                        // then we want to use this as an ideal position
                        // for the segment.

                        // Bitflags indicating whether this segment starts
                        // and/or ends in a shape.
                        let mut ends_in_shapes: u32 = 0;
                        // Also limit their movement to the edges of the
                        // shapes they begin or end within.
                        for k in 0..shape_limits.len() {
                            let shape_min = shape_limits[k].0[dim];
                            let shape_max = shape_limits[k].1[dim];
                            if inside_rect_bounds(&display_route.ps[i - 1], &shape_limits[k]) {
                                min_lim = cpp_max(min_lim, shape_min);
                                max_lim = cpp_min(max_lim, shape_max);
                                ends_in_shapes |= 0x01;
                            }
                            if inside_rect_bounds(&display_route.ps[i], &shape_limits[k]) {
                                min_lim = cpp_max(min_lim, shape_min);
                                max_lim = cpp_min(max_lim, shape_max);
                                ends_in_shapes |= 0x10;
                            }
                        }

                        if ends_in_shapes == 0 {
                            // If the segment is not within a shape, then we
                            // should limit it's nudging buffer so we don't
                            // combine many unnecessary regions.
                            let pos = display_route.ps[i - 1][dim];
                            let free_conn_buffer = 15.0;
                            min_lim = cpp_max(min_lim, pos - free_conn_buffer);
                            max_lim = cpp_min(max_lim, pos + free_conn_buffer);
                        }

                        if (min_lim == max_lim) || router.conns[curr].has_fixed_route() {
                            // Fixed.
                            let seg = NudgingShiftSegment::new_fixed(router, curr, index_low, index_high, dim);
                            segment_list.push(seg);
                        } else {
                            // Shiftable.
                            let mut segment = NudgingShiftSegment::new_shiftable(
                                router, curr, index_low, index_high, false, false, dim, min_lim, max_lim,
                            );
                            segment.final_segment = true;
                            segment.ends_in_shape = ends_in_shapes > 0;
                            if (display_route.size() == 2) && (ends_in_shapes == 0x11) {
                                // This is a single segment connector bridging
                                // two shapes.  So, we want to try to keep the
                                // segment centred rather than shift it.
                                segment.single_connected_segment = true;
                            }
                            segment_list.push(segment);
                        }
                    } else {
                        // The first and last segment of a connector can't be
                        // shifted.  We call them fixed segments.
                        let seg = NudgingShiftSegment::new_fixed(router, curr, index_low, index_high, dim);
                        segment_list.push(seg);
                    }
                    continue;
                }

                // The segment probably has space to be shifted.
                let mut min_lim = -CHANNEL_MAX;
                let mut max_lim = CHANNEL_MAX;

                // Constrain these segments by checkpoints along the
                // adjoining segments.  Ignore checkpoints at ends of
                // those segments.  XXX Perhaps this should not
                // affect the ideal centre position in the channel.
                for cp in 0..next_checkpoints.len() {
                    if next_checkpoints[cp][dim] < this_pos {
                        // Not at thisPoint, so constrain.
                        min_lim = cpp_max(min_lim, next_checkpoints[cp][dim]);
                    } else if next_checkpoints[cp][dim] > this_pos {
                        // Not at thisPoint, so constrain.
                        max_lim = cpp_min(max_lim, next_checkpoints[cp][dim]);
                    }
                }
                for cp in 0..prev_checkpoints.len() {
                    if prev_checkpoints[cp][dim] < this_pos {
                        // Not at thisPoint, so constrain.
                        min_lim = cpp_max(min_lim, prev_checkpoints[cp][dim]);
                    } else if prev_checkpoints[cp][dim] > this_pos {
                        // Not at thisPoint, so constrain.
                        max_lim = cpp_min(max_lim, prev_checkpoints[cp][dim]);
                    }
                }

                let mut is_s_bend = false;
                let mut is_z_bend = false;

                if checkpoints.is_empty() {
                    // Segments with checkpoints are held in place, but for
                    // other segments, we should limit their movement based
                    // on the limits of the segments at either end.

                    let prev_pos = display_route.ps[i - 2][dim];
                    let next_pos = display_route.ps[i + 1][dim];
                    if ((prev_pos < this_pos) && (next_pos > this_pos))
                        || ((prev_pos > this_pos) && (next_pos < this_pos))
                    {
                        // Determine limits if the s-bend is not due to an
                        // obstacle.  In this case we need to limit the channel
                        // to the span of the adjoining segments to this one.
                        if (prev_pos < this_pos) && (next_pos > this_pos) {
                            min_lim = cpp_max(min_lim, prev_pos);
                            max_lim = cpp_min(max_lim, next_pos);
                            is_z_bend = true;
                        } else {
                            // if ((prevPos > thisPos) && (nextPos < thisPos))
                            min_lim = cpp_max(min_lim, next_pos);
                            max_lim = cpp_min(max_lim, prev_pos);
                            is_s_bend = true;
                        }
                    }
                }

                let mut nss = NudgingShiftSegment::new_shiftable(
                    router, curr, index_low, index_high, is_s_bend, is_z_bend, dim, min_lim, max_lim,
                );
                nss.checkpoints = checkpoints;
                segment_list.push(nss);
            }
        }
    }
}

// upstream: orthogonal.cpp class CmpLineOrder
struct CmpLineOrder<'a> {
    orders: &'a mut PtOrderMap,
    dimension: usize,
}

impl CmpLineOrder<'_> {
    /// `operator()(lhs, rhs, &comparable)`.  (`PtOrder::positionFor` sorts
    /// lazily, hence `&mut self`; `orders[unchanged]` may insert.)
    // upstream: orthogonal.cpp CmpLineOrder::operator()()
    fn call(
        &mut self,
        router: &Router,
        lhs: &NudgingShiftSegment,
        rhs: &NudgingShiftSegment,
        comparable: &mut bool,
    ) -> bool {
        let dimension = self.dimension;
        *comparable = true;
        let lhs_low = lhs.low_point_of(router);
        let rhs_low = rhs.low_point_of(router);
        let alt_dim = (dimension + 1) % 2;
        debug_assert!(lhs_low[dimension] == lhs.high_point_of(router)[dimension]);
        debug_assert!(rhs_low[dimension] == rhs.high_point_of(router)[dimension]);

        // We consider things at effectively the same position to
        // be ordered based on their order and fixedOrder, so only
        // compare segments further apart than the nudgeDistance.
        if lhs_low[dimension] != rhs_low[dimension] {
            return lhs_low[dimension] < rhs_low[dimension];
        }

        // If one of these is fixed, then determine order based on
        // fixed segment, that is, order so the fixed segment doesn't
        // block movement.
        let mut one_is_fixed = false;
        let lhs_fixed_order = lhs.fixed_order(router, &mut one_is_fixed);
        let rhs_fixed_order = rhs.fixed_order(router, &mut one_is_fixed);
        if one_is_fixed && (lhs_fixed_order != rhs_fixed_order) {
            return lhs_fixed_order < rhs_fixed_order;
        }

        // C-bends that did not have a clear order with s-bends might
        // not have a good ordering here, so compare their order in
        // terms of C-bend direction and S-bends and use that if it
        // differs for the two segments.
        let lhs_order = lhs.order(router);
        let rhs_order = rhs.order(router);
        if lhs_order != rhs_order {
            return lhs_order < rhs_order;
        }

        // Need to index using the original point into the map, so find it.
        let unchanged = if lhs_low[alt_dim] > rhs_low[alt_dim] { lhs_low } else { rhs_low };

        let low_order = self.orders.entry(PointOrd(unchanged)).or_default();
        let lhs_pos = low_order.position_for(dimension, Some(lhs.conn_ref));
        let rhs_pos = low_order.position_for(dimension, Some(rhs.conn_ref));
        if (lhs_pos == -1) || (rhs_pos == -1) {
            // A value for rhsPos or lhsPos mean the points are not directly
            // comparable, meaning they are at the same position but cannot
            // overlap (they are just collinear.  The relative order for
            // these segments is not important since we do not constrain
            // them against each other.
            //COLA_ASSERT(lhs->overlapsWith(rhs, dimension) == false);
            // We do need to be consistent though.
            *comparable = false;
            return lhs_low[alt_dim] < rhs_low[alt_dim];
        }
        lhs_pos < rhs_pos
    }
}

// We can't use the normal sort algorithm for lists since it is not possible
// to compare all elements, but there will be an ordering defined between
// most of the elements.  Hence we order these, using insertion sort, and
// the case of them not being able to be compared is handled by not setting
// up any constraints between such segments when doing the nudging.
//
// upstream: orthogonal.cpp linesort()
fn linesort(
    router: &mut Router,
    nudge_final_segments: bool,
    mut orig_list: Vec<NudgingShiftSegment>,
    comparison: &mut CmpLineOrder,
) -> Vec<NudgingShiftSegment> {
    // Cope with end segments that are getting moved and will line up with
    // other segments of the same connector.  We do this by merging them into
    // a single NudgingShiftSegment.
    if nudge_final_segments {
        let mut curr_seg_it = 0;
        while curr_seg_it < orig_list.len() {
            let mut other_seg_it = curr_seg_it;
            while other_seg_it < orig_list.len() {
                if (curr_seg_it != other_seg_it)
                    && orig_list[curr_seg_it].should_align_with(&orig_list[other_seg_it], comparison.dimension, router)
                {
                    // (curr_seg_it < other_seg_it, so removing first keeps
                    // curr_seg_it valid.)
                    let other_seg = orig_list.remove(other_seg_it);
                    orig_list[curr_seg_it].merge_with(&other_seg, comparison.dimension, router);
                    // delete otherSeg;
                } else {
                    other_seg_it += 1;
                }
            }
            curr_seg_it += 1;
        }
    }

    let mut orig_list: VecDeque<NudgingShiftSegment> = orig_list.into();
    let mut result_list: Vec<NudgingShiftSegment> = Vec::new();

    let mut orig_list_size = orig_list.len();
    let mut deferred_n: usize = 0;
    while let Some(segment) = orig_list.pop_front() {
        // Get and remove the first element from the origList.

        // Find the insertion point in the resultList.
        let mut all_comparable = true;
        let mut curr = 0;
        while curr < result_list.len() {
            let mut comparable = false;
            let less_than = comparison.call(router, &segment, &result_list[curr], &mut comparable);
            all_comparable &= comparable;

            if comparable && less_than {
                // If it is comparable and lessThan, then we have found the
                // insertion point.
                break;
            }
            curr += 1;
        }

        if result_list.is_empty() || all_comparable || (deferred_n >= orig_list_size) {
            // Insert the element into the resultList at the required point.
            result_list.insert(curr, segment);
            // Reset the origListSize and deferred counter.
            deferred_n = 0;
            orig_list_size = orig_list.len();
        } else {
            // This wasn't comparable to anything in the sorted list,
            // so defer addition of the segment till later.
            orig_list.push_back(segment);
            deferred_n += 1;
        }
    }

    result_list
}

// upstream: orthogonal.cpp class PotentialSegmentConstraint
#[derive(Clone, Copy, Debug)]
struct PotentialSegmentConstraint {
    index1: usize,
    index2: usize,
}

impl PotentialSegmentConstraint {
    // upstream: orthogonal.cpp PotentialSegmentConstraint::PotentialSegmentConstraint()
    fn new(index1: usize, index2: usize) -> PotentialSegmentConstraint {
        PotentialSegmentConstraint { index1, index2 }
    }

    // upstream: orthogonal.cpp PotentialSegmentConstraint::operator<()
    fn less_than(&self, rhs: &PotentialSegmentConstraint, vs: &[Variable]) -> bool {
        self.sep_distance(vs) < rhs.sep_distance(vs)
    }

    // upstream: orthogonal.cpp PotentialSegmentConstraint::sepDistance()
    fn sep_distance(&self, vs: &[Variable]) -> f64 {
        if !self.still_valid() {
            return 0.0;
        }
        (vs[self.index1].final_position - vs[self.index2].final_position).abs()
    }

    // upstream: orthogonal.cpp PotentialSegmentConstraint::stillValid()
    fn still_valid(&self) -> bool {
        self.index1 != self.index2
    }

    // upstream: orthogonal.cpp PotentialSegmentConstraint::rewriteIndex()
    fn rewrite_index(&mut self, old_index: usize, new_index: usize) {
        if self.index1 == old_index {
            self.index1 = new_index;
        }

        if self.index2 == old_index {
            self.index2 = new_index;
        }
    }
}

// upstream: orthogonal.cpp class ImproveOrthogonalRoutes
struct ImproveOrthogonalRoutes {
    m_point_orders: PtOrderMap,
    m_shared_path_connectors_with_common_endpoints: UnsignedPairSet,
    m_segment_list: Vec<NudgingShiftSegment>,
}

impl ImproveOrthogonalRoutes {
    // upstream: orthogonal.cpp ImproveOrthogonalRoutes::ImproveOrthogonalRoutes()
    fn new() -> ImproveOrthogonalRoutes {
        ImproveOrthogonalRoutes {
            m_point_orders: PtOrderMap::new(),
            m_shared_path_connectors_with_common_endpoints: UnsignedPairSet::new(),
            m_segment_list: Vec::new(),
        }
    }

    // upstream: orthogonal.cpp ImproveOrthogonalRoutes::execute()
    fn execute(&mut self, router: &mut Router) {
        self.m_shared_path_connectors_with_common_endpoints.clear();

        // Simplify routes.
        self.simplify_orthogonal_routes(router);

        // Build a cache that denotes whether a certain segment of a connector
        // contains a checkpoint.  We can't just compare positions, since routes
        // can be moved away from their original positions during nudging.
        build_connector_route_checkpoint_cache(router);

        // Do Unifying first, by itself.  This greedily tries to position free
        // segments in overlapping channels at the same position.  This way they
        // have correct nudging orders determined for them since they will form
        // shared paths, rather than segments just positioned as an results of
        // the routing process.  Of course, don't do this when rerouting with
        // a fixedSharedPathPenalty since these routes include extra segments
        // we want to keep apart which prevent some shared paths.
        if router.routing_option(RoutingOption::PerformUnifyingNudgingPreprocessingStep)
            && (router.routing_parameter(RoutingParameter::FixedSharedPathPenalty) == 0.0)
        {
            for dimension in 0..2 {
                // Just perform Unifying operation.
                let just_unifying = true;
                self.m_segment_list.clear();
                build_orthogonal_nudging_segments(router, dimension, &mut self.m_segment_list);
                build_orthogonal_channel_info(router, router, dimension, &mut self.m_segment_list);
                self.nudge_orthogonal_routes(router, dimension, just_unifying);
            }
        }

        // Do the Nudging and centring.
        for dimension in 0..2 {
            self.m_point_orders.clear();
            // Build nudging info.
            // XXX Needs to be rebuilt for each dimension, cause of shifting
            //     points.  Maybe we could modify the point orders.
            self.build_orthogonal_nudging_order_info(router);

            // Do the centring and nudging.
            self.m_segment_list.clear();
            build_orthogonal_nudging_segments(router, dimension, &mut self.m_segment_list);
            build_orthogonal_channel_info(router, router, dimension, &mut self.m_segment_list);
            self.nudge_orthogonal_routes(router, dimension, false);
        }

        // Resimplify all the display routes that may have been split.
        self.simplify_orthogonal_routes(router);

        router.improve_orthogonal_topology();

        // Clear the segment-checkpoint cache for connectors.
        clear_connector_route_checkpoint_cache(router);
    }

    // upstream: orthogonal.cpp ImproveOrthogonalRoutes::nudgeOrthogonalRoutes()
    fn nudge_orthogonal_routes(&mut self, router: &mut Router, dimension: usize, just_unifying: bool) {
        let nudge_final_segments = router.routing_option(RoutingOption::NudgeOrthogonalSegmentsConnectedToShapes);
        let nudge_shared_paths_with_common_end =
            router.routing_option(RoutingOption::NudgeSharedPathsWithCommonEndPoint);
        let base_sep_dist = router.routing_parameter(RoutingParameter::IdealNudgingDistance);
        debug_assert!(base_sep_dist >= 0.0);
        // If we can fit things with the desired separation distance, then
        // we try 10 times, reducing each time by a 10th of the original amount.
        let reduction_steps = 10.0;

        let total_segments_to_shift = self.m_segment_list.len();
        let mut num_of_segments_shifted;
        // Do the actual nudging.
        let mut current_region: Vec<NudgingShiftSegment>;
        while !self.m_segment_list.is_empty() {
            // Progress reporting and continuation check.
            num_of_segments_shifted = total_segments_to_shift - self.m_segment_list.len();
            router.perform_continuation_check(
                if dimension == XDIM {
                    TRANSACTION_PHASE_ORTHOGONAL_NUDGING_X
                } else {
                    TRANSACTION_PHASE_ORTHOGONAL_NUDGING_Y
                },
                num_of_segments_shifted,
                total_segments_to_shift,
            );

            // Take a reference segment
            let current_segment = self.m_segment_list.remove(0);
            // Then, find the segments that overlap this one.
            current_region = vec![current_segment];
            let mut curr = 0;
            while curr < self.m_segment_list.len() {
                let mut overlaps = false;
                for curr2 in current_region.iter() {
                    if self.m_segment_list[curr].nudging_overlaps_with(curr2, dimension, router) {
                        overlaps = true;
                        break;
                    }
                }
                if overlaps {
                    let seg = self.m_segment_list.remove(curr);
                    current_region.push(seg);
                    // Consider segments from the beginning, since we may have
                    // since passed segments that overlap with the new set.
                    curr = 0;
                } else {
                    curr += 1;
                }
            }

            if !just_unifying {
                let mut line_sort_comp = CmpLineOrder {
                    orders: &mut self.m_point_orders,
                    dimension,
                };
                current_region = linesort(router, nudge_final_segments, current_region, &mut line_sort_comp);
            }

            if nudge_oracle_tracing() {
                let o_alt_dim = (dimension + 1) % 2;
                nudge_oracle_trace(format!(
                    "R d={} u={} n={}",
                    dimension,
                    just_unifying as i32,
                    current_region.len()
                ));
                for g in current_region.iter() {
                    let mut t = format!("S c={} i=", router.conns[g.conn_ref].id());
                    for (k, ix) in g.indexes.iter().enumerate() {
                        t += &format!("{}{}", if k > 0 { "," } else { "" }, ix);
                    }
                    t += &format!(
                        " p={} a={},{} lim={},{} f={}{}{}{}{}{} cp={}",
                        nudge_oracle_g17(g.low_point_of(router)[dimension]),
                        nudge_oracle_g17(g.low_point_of(router)[o_alt_dim]),
                        nudge_oracle_g17(g.high_point_of(router)[o_alt_dim]),
                        nudge_oracle_g17(g.base.min_space_limit),
                        nudge_oracle_g17(g.base.max_space_limit),
                        g.fixed as i32,
                        g.final_segment as i32,
                        g.ends_in_shape as i32,
                        g.single_connected_segment as i32,
                        g.s_bend as i32,
                        g.z_bend as i32,
                        g.checkpoints.len()
                    );
                    nudge_oracle_trace(t);
                }
            }

            if current_region.len() == 1 {
                // Save creating the solver instance if there is just one
                // immovable segment, or if we are in the unifying stage.
                if current_region[0].immovable(router) || just_unifying {
                    // delete currentRegion.front();
                    continue;
                }
            }

            // Process these segments.
            let mut free_indexes: Vec<usize> = Vec::new();
            let mut vs: Vec<Variable> = Vec::new();
            let mut cs: Vec<Constraint> = Vec::new();
            // (Upstream `gapcs`: filled but never read.)
            let mut gapcs: Vec<ConIdx> = Vec::new();
            // Indexes into current_region.
            let mut prev_vars: Vec<usize> = Vec::new();
            let mut sep_dist = base_sep_dist;
            for curr_segment_it in 0..current_region.len() {
                // Create a solver variable for the position of this segment.
                current_region[curr_segment_it].create_solver_variable(router, just_unifying, &mut vs);
                let curr_segment = &current_region[curr_segment_it];

                let index = vs.len() - 1;

                if just_unifying {
                    // Just doing centring, not nudging.
                    // Record the index of the variable so we can use it as
                    // a segment to potentially constrain to other segments.
                    if vs[index].weight == FREE_WEIGHT {
                        free_indexes.push(index);
                    }
                    // Thus, we don't need to constrain position against other
                    // segments.
                    prev_vars.push(curr_segment_it);
                    continue;
                }

                // The constraints generated here must be in order of
                // leftBoundary-segment ... segment-segment ... segment-rightBoundary
                // since this order is leveraged later for rewriting the
                // separations of unsatisfable channel groups.

                // Constrain to channel boundary.
                if !curr_segment.fixed {
                    // If this segment sees a channel boundary to its left,
                    // then constrain its placement as such.
                    if curr_segment.base.min_space_limit > -CHANNEL_MAX {
                        vs.push(Variable::with_weight(
                            CHANNEL_LEFT_ID,
                            curr_segment.base.min_space_limit,
                            FIXED_WEIGHT,
                        ));
                        cs.push(Constraint::new(vs.len() - 1, index, 0.0, false));
                    }
                }

                // Constrain position in relation to previously seen segments,
                // if necessary (i.e. when they could overlap).
                for &prev_var_it in prev_vars.iter() {
                    let prev_seg = &current_region[prev_var_it];
                    let prev_var = prev_seg.variable.expect("solver variable");

                    if curr_segment.nudging_overlaps_with(prev_seg, dimension, router)
                        && (!curr_segment.fixed || !prev_seg.fixed)
                    {
                        // If there is a previous segment to the left that
                        // could overlap this in the shift direction, then
                        // constrain the two segments to be separated.
                        // Though don't add the constraint if both the
                        // segments are fixed in place.
                        let mut this_sep_dist = sep_dist;
                        let mut equality = false;
                        if curr_segment.should_align_with(prev_seg, dimension, router) {
                            // Handles the case where the two end segments can
                            // be brought together to make a single segment. This
                            // can help in situations where having the small kink
                            // can restrict other kinds of nudging.
                            this_sep_dist = 0.0;
                            equality = true;
                        } else if curr_segment.can_align_with(prev_seg, dimension) {
                            // We need to address the problem of two neighbouring
                            // segments of the same connector being kept separated
                            // due only to a kink created in the other dimension.
                            // Here, we let such segments drift back together.
                            this_sep_dist = 0.0;
                        } else if !nudge_shared_paths_with_common_end
                            && self.m_shared_path_connectors_with_common_endpoints.contains(&UnsignedPair::new(
                                router.conns[curr_segment.conn_ref].id(),
                                router.conns[prev_seg.conn_ref].id(),
                            ))
                        {
                            // We don't want to nudge apart these two segments
                            // since they are from a shared path with a common
                            // endpoint.  There might be multiple chains of
                            // segments that don't all have the same endpoints
                            // so we need to make this an equality to prevent
                            // some of them possibly getting nudged apart.
                            this_sep_dist = 0.0;
                            equality = true;
                        }

                        cs.push(Constraint::new(prev_var, index, this_sep_dist, equality));
                        if this_sep_dist != 0.0 {
                            // Add to the list of gap constraints so we can
                            // rewrite the separation distance later.
                            gapcs.push(cs.len() - 1);
                        }
                    }
                }

                if !curr_segment.fixed {
                    // If this segment sees a channel boundary to its right,
                    // then constrain its placement as such.
                    if curr_segment.base.max_space_limit < CHANNEL_MAX {
                        vs.push(Variable::with_weight(
                            CHANNEL_RIGHT_ID,
                            curr_segment.base.max_space_limit,
                            FIXED_WEIGHT,
                        ));
                        cs.push(Constraint::new(index, vs.len() - 1, 0.0, false));
                    }
                }

                prev_vars.push(curr_segment_it);
            }

            // std::list<PotentialSegmentConstraint>
            let mut potential_constraints: VecDeque<PotentialSegmentConstraint> = VecDeque::new();
            if just_unifying {
                for curr in 0..free_indexes.len() {
                    for curr2 in curr..free_indexes.len() {
                        if curr == curr2 {
                            continue;
                        }
                        potential_constraints
                            .push_back(PotentialSegmentConstraint::new(free_indexes[curr], free_indexes[curr2]));
                    }
                }
            }

            // Repeatedly try solving this.  There are two cases:
            //  -  When Unifying, we greedily place as many free segments as
            //     possible at the same positions, that way they have more
            //     accurate nudging orders determined for them in the Nudging
            //     stage.
            //  -  When Nudging, if we can't fit all the segments with the
            //     default nudging distance we try smaller separation
            //     distances till we find a solution that is satisfied.
            let mut just_added_constraint = false;
            let mut satisfied;

            // typedef std::pair<size_t, size_t> UnsatisfiedRange;
            let mut unsatisfied_ranges: VecDeque<(usize, usize)> = VecDeque::new();
            loop {
                {
                    let mut f = IncSolver::new(&mut vs, &mut cs);
                    if let Err(e) = f.solve() {
                        // Upstream throws a `const char *` nobody catches.
                        panic!("vpsc: unsatisfied constraint {e:?} (uncaught exception upstream)");
                    }
                }

                // Determine if the problem was satisfied.
                satisfied = true;
                for i in 0..vs.len() {
                    // For each variable...
                    if vs[i].id != FREE_SEGMENT_ID {
                        // If it is a fixed segment (should stay still)...
                        if (vs[i].final_position - vs[i].desired_position).abs() > 0.0001 {
                            // and it is not at it's desired position, then
                            // we consider the problem to be unsatisfied.
                            satisfied = false;

                            // We record ranges of unsatisfied variables based on
                            // the channel edges.
                            if vs[i].id == CHANNEL_LEFT_ID {
                                // This is the left-hand-side of a channel.
                                let start_new_range = match unsatisfied_ranges.back() {
                                    None => true,
                                    Some(back) => back.0 != back.1,
                                };
                                if start_new_range {
                                    // There are no existing unsatisfied ranges,
                                    // or there are but they are a valid range
                                    // (we've encountered the right-hand channel
                                    // edges already).
                                    // So, start a new unsatisfied range.
                                    unsatisfied_ranges.push_back((i, i + 1));
                                }
                            } else if vs[i].id == CHANNEL_RIGHT_ID {
                                // This is the right-hand-side of a channel.
                                if unsatisfied_ranges.is_empty() {
                                    // There are no existing unsatisfied ranges,
                                    // so start a new unsatisfied range.
                                    // We are looking at a unsatisfied right side
                                    // where the left side was satisfied, so the
                                    // range begins at the previous variable
                                    // which should be a left channel side.
                                    debug_assert!(i > 0);
                                    debug_assert!(vs[i - 1].id == CHANNEL_LEFT_ID);
                                    unsatisfied_ranges.push_back((i - 1, i));
                                } else {
                                    // Expand the existing range to include index.
                                    unsatisfied_ranges.back_mut().unwrap().1 = i;
                                }
                            } else if vs[i].id == FIXED_SEGMENT_ID {
                                // Fixed connector segments can also start and
                                // extend unsatisfied variable ranges.
                                if unsatisfied_ranges.is_empty() {
                                    // There are no existing unsatisfied ranges,
                                    // so start a new unsatisfied range.
                                    unsatisfied_ranges.push_back((i, i));
                                } else {
                                    // Expand the existing range to include index.
                                    unsatisfied_ranges.back_mut().unwrap().1 = i;
                                }
                            }
                        }
                    }
                }

                if just_unifying {
                    // When we're centring, we'd like to greedily place as many
                    // segments as possible at the same positions, that way they
                    // have more accurate nudging orders determined for them.
                    //
                    // We do this by taking pairs of adjoining free segments and
                    // attempting to constrain them to have the same position,
                    // starting from the closest up to the furthest.

                    if just_added_constraint {
                        debug_assert!(!potential_constraints.is_empty());
                        if !satisfied {
                            // We couldn't satisfy the problem with the added
                            // potential constraint, so we can't position these
                            // segments together.  Roll back.
                            potential_constraints.pop_front();
                            cs.pop();
                        } else {
                            // We could position these two segments together.
                            // (Upstream `pc` is a *reference* to the front
                            // element, and the loop below rewrites that very
                            // element first: afterwards pc.index1 ==
                            // pc.index2, so every later rewriteIndex() call is
                            // a no-op.  The arguments are re-read from the
                            // front element on every iteration to keep that
                            // aliasing behaviour exactly.)

                            // Rewrite the indexes of these two variables to
                            // one, so we need not worry about redundant
                            // equality constraints.
                            for it in 0..potential_constraints.len() {
                                let (pc_index1, pc_index2) =
                                    (potential_constraints[0].index1, potential_constraints[0].index2);
                                potential_constraints[it].rewrite_index(pc_index1, pc_index2);
                            }
                            potential_constraints.pop_front();
                        }
                    }
                    // std::list::sort() is a stable merge sort; any stable
                    // sort gives the identical permutation for this strict
                    // weak ordering (ties keep list order).
                    {
                        let vs_ref = &vs;
                        potential_constraints.make_contiguous().sort_by(|a, b| {
                            if a.less_than(b, vs_ref) {
                                Ordering::Less
                            } else if b.less_than(a, vs_ref) {
                                Ordering::Greater
                            } else {
                                Ordering::Equal
                            }
                        });
                    }
                    just_added_constraint = false;

                    // Remove now invalid potential segment constraints.
                    // This could have been caused by the variable rewriting.
                    while !potential_constraints.is_empty() && !potential_constraints.front().unwrap().still_valid() {
                        potential_constraints.pop_front();
                    }

                    if !potential_constraints.is_empty() {
                        // We still have more possibilities to consider.
                        // Create a constraint for this, add it, and mark as
                        // unsatisfied, so the problem gets re-solved.
                        let pc = *potential_constraints.front().unwrap();
                        debug_assert!(pc.index1 != pc.index2);
                        cs.push(Constraint::new(pc.index1, pc.index2, 0.0, true));
                        satisfied = false;
                        just_added_constraint = true;
                    }
                } else {
                    if !satisfied {
                        debug_assert!(!unsatisfied_ranges.is_empty());
                        // Reduce the separation distance.
                        sep_dist -= base_sep_dist / reduction_steps;
                        if cfg!(debug_assertions) {
                            // (Upstream reads vs[it->second] out of bounds
                            // when a range was started by the last
                            // variable, a channelLeft one; treat that as
                            // passing instead of panicking.)
                            for it in unsatisfied_ranges.iter() {
                                debug_assert!(vs.get(it.0).map_or(true, |v| v.id != FREE_SEGMENT_ID));
                                debug_assert!(vs.get(it.1).map_or(true, |v| v.id != FREE_SEGMENT_ID));
                            }
                        }
                        // And rewrite all the gap constraints to have the new
                        // reduced separation distance.
                        let mut within_unsatisfied_group = false;
                        for c_it in 0..cs.len() {
                            let range = *unsatisfied_ranges.front().expect("unsatisfied range");
                            let constraint = &mut cs[c_it];

                            if constraint.left == range.0 {
                                // Entered an unsatisfied range of variables.
                                within_unsatisfied_group = true;
                            }

                            if within_unsatisfied_group && (constraint.gap > 0.0) {
                                // Rewrite constraints in unsatisfied ranges
                                // that have a non-zero gap.
                                constraint.gap = sep_dist;
                            }

                            if constraint.right == range.1 {
                                // Left an unsatisfied range of variables.
                                within_unsatisfied_group = false;
                                unsatisfied_ranges.pop_front();
                                if unsatisfied_ranges.is_empty() {
                                    // And there are no more unsatisfied variables.
                                    break;
                                }
                            }
                        }
                    }
                }

                if !(!satisfied && (sep_dist > 0.0001)) {
                    break;
                }
            }

            if nudge_oracle_tracing() {
                nudge_oracle_trace(format!(
                    "X sat={} sep={} nv={} nc={}",
                    satisfied as i32,
                    nudge_oracle_g17(sep_dist),
                    vs.len(),
                    cs.len()
                ));
                for v in vs.iter() {
                    nudge_oracle_trace(format!(
                        "v {} {} {} {}",
                        v.id,
                        nudge_oracle_g17(v.desired_position),
                        nudge_oracle_g17(v.final_position),
                        nudge_oracle_g17(v.weight)
                    ));
                }
                for c in cs.iter() {
                    nudge_oracle_trace(format!(
                        "k {} {} {} {} {}",
                        c.left,
                        c.right,
                        nudge_oracle_g17(c.gap),
                        c.equality as i32,
                        c.unsatisfiable as i32
                    ));
                }
            }

            if satisfied {
                for curr_segment in current_region.iter() {
                    curr_segment.update_positions_from_solver(router, &vs, just_unifying);
                }
            }
            // for_each(currentRegion/vs/cs, delete_object()): dropped here.
            let _ = gapcs;
        }
    }

    // upstream: orthogonal.cpp ImproveOrthogonalRoutes::simplifyOrthogonalRoutes()
    fn simplify_orthogonal_routes(&mut self, router: &mut Router) {
        // Simplify routes.
        let conn_refs = router.conn_refs.clone();
        for &curr in conn_refs.iter() {
            if router.conns[curr].routing_type() != ConnType::ConnTypeOrthogonal {
                continue;
            }
            let simplified = router.conns[curr].display_route().simplify();
            router.conns[curr].set_route(&simplified);
        }
    }

    // Populates m_point_orders and m_shared_path_connectors_with_common_endpoints.
    // upstream: orthogonal.cpp ImproveOrthogonalRoutes::buildOrthogonalNudgingOrderInfo()
    fn build_orthogonal_nudging_order_info(&mut self, router: &mut Router) {
        // Simplify routes.
        self.simplify_orthogonal_routes(router);

        let mut crossings_n: i32 = 0;

        let mut build_shared_path_info = false;
        if !router.routing_option(RoutingOption::NudgeSharedPathsWithCommonEndPoint)
            && self.m_shared_path_connectors_with_common_endpoints.is_empty()
        {
            // We're not going to nudge apart shared paths with common ends so we
            // will need to store information about this during the crossing
            // detection.
            build_shared_path_info = true;
        }

        // Make a vector of the ConnRefList, for convenience.
        let conn_refs: Vec<ConnIdx> = router.conn_refs.clone();

        // Make a temporary copy of all the connector displayRoutes.
        let mut conn_routes: Vec<Polygon> = Vec::with_capacity(conn_refs.len());
        for ind in 0..conn_refs.len() {
            conn_routes.push(router.conns[conn_refs[ind]].display_route().clone());
        }

        // Do segment splitting.
        for ind1 in 0..conn_refs.len() {
            let conn = conn_refs[ind1];
            if router.conns[conn].routing_type() != ConnType::ConnTypeOrthogonal {
                continue;
            }

            for ind2 in 0..conn_refs.len() {
                if ind1 == ind2 {
                    continue;
                }

                let conn2 = conn_refs[ind2];
                if router.conns[conn2].routing_type() != ConnType::ConnTypeOrthogonal {
                    continue;
                }

                let (route, route2) = two_mut(&mut conn_routes, ind1, ind2);
                split_branching_segments(route2, true, route, 0.0);
            }
        }

        let router_ref: &Router = router;
        for ind1 in 0..conn_refs.len() {
            let conn = conn_refs[ind1];
            if router_ref.conns[conn].routing_type() != ConnType::ConnTypeOrthogonal {
                continue;
            }

            for ind2 in (ind1 + 1)..conn_refs.len() {
                let conn2 = conn_refs[ind2];
                if router_ref.conns[conn2].routing_type() != ConnType::ConnTypeOrthogonal {
                    continue;
                }

                let (route, route2) = two_mut(&mut conn_routes, ind1, ind2);
                let mut crossings: i32 = 0;
                let mut crossing_flags: u32 = 0;
                {
                    let mut cross = ConnectorCrossings::new(true, Some(conn2), Some(conn));
                    cross.point_orders = Some(&mut self.m_point_orders);
                    let mut i = 1;
                    while i < route.size() {
                        let final_segment = (i + 1) == route.size();
                        cross.count_for_segment(router_ref, route2, route, i, final_segment);

                        crossings += cross.crossing_count as i32;
                        crossing_flags |= cross.crossing_flags;
                        i += 1;
                    }
                }
                if crossings > 0 {
                    crossings_n += crossings;
                }

                if build_shared_path_info && ((crossing_flags & CROSSING_SHARES_PATH_AT_END) != 0) {
                    // Record if these two connectors have a shared path with a
                    // common end point.
                    self.m_shared_path_connectors_with_common_endpoints.insert(UnsignedPair::new(
                        router_ref.conns[conn].id(),
                        router_ref.conns[conn2].id(),
                    ));
                }
            }
        }
        let _ = crossings_n;
    }
}

/// Two distinct mutable elements of a slice.
fn two_mut<T>(v: &mut [T], a: usize, b: usize) -> (&mut T, &mut T) {
    debug_assert!(a != b);
    if a < b {
        let (lo, hi) = v.split_at_mut(b);
        (&mut lo[a], &mut hi[0])
    } else {
        let (lo, hi) = v.split_at_mut(a);
        (&mut hi[0], &mut lo[b])
    }
}

// upstream: orthogonal.cpp improveOrthogonalRoutes()
pub fn improve_orthogonal_routes(router: &mut Router) {
    let mut improver = ImproveOrthogonalRoutes::new();
    improver.execute(router);
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

    #[test]
    fn segment_list_wrapper_merges_into_later_overlapping_segment() {
        use crate::avoid::router::ORTHOGONAL_ROUTING;
        let r = Router::new(ORTHOGONAL_ROUTING);
        let v = &r.vert_infs;
        let mut w = SegmentListWrapper::new();
        assert_eq!(w.insert(LineSegment::new(v, 0.0, 10.0, 5.0, true, None, None)), 0);
        assert_eq!(w.insert(LineSegment::new(v, 20.0, 30.0, 5.0, true, None, None)), 1);
        assert_eq!(w.insert(LineSegment::new(v, 0.0, 50.0, 7.0, true, None, None)), 2);
        // Overlaps the first two: the first absorbs the new segment, then
        // the second absorbs the first, which is erased.
        assert_eq!(w.insert(LineSegment::new(v, 8.0, 22.0, 5.0, true, None, None)), 0);
        let l = w.list();
        assert_eq!(l.len(), 2);
        assert_eq!((l[0].begin, l[0].finish, l[0].pos), (0.0, 30.0, 5.0));
        assert_eq!((l[1].begin, l[1].finish, l[1].pos), (0.0, 50.0, 7.0));
        // Touching end points count as overlapping; point segments too.
        assert_eq!(w.insert(LineSegment::new_point(v, 30.0, 5.0, None)), 0);
        assert_eq!(w.insert(LineSegment::new_point(v, 60.0, 7.0, None)), 2);
        sort_segment_list(w.list());
        let order: Vec<(f64, f64)> = w.list().iter().map(|s| (s.begin, s.pos)).collect();
        assert_eq!(order, vec![(0.0, 5.0), (0.0, 7.0), (60.0, 7.0)]);
    }
}
