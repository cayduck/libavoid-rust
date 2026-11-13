//! Faithful port of upstream `libavoid/makepath.{h,cpp}`: the A* search
//! (`AStarPath`) used to route a connector through the visibility graph,
//! with its cost function and heuristics.
//!
//! Port notes:
//! * `ANode`s are allocated in blocks of 5000 upstream and never freed
//!   until the `AStarPath` is destroyed; here they live in a `Vec<ANode>`
//!   owned by the search object and are named by [`ANodeIdx`] (index into
//!   that vector).  Node identity comparisons (`node.prevNode ==
//!   ati.prevNode`) compare indices, which is equivalent.
//! * The `PENDING` heap is a `Vec<ANodeIdx>` manipulated with exact ports of
//!   libstdc++'s `std::make_heap` / `std::push_heap` / `std::pop_heap`
//!   (see [`make_heap`], [`push_heap`], [`pop_heap`]), because the order in
//!   which nodes of (epsilon-)equal cost are popped depends on the precise
//!   sift sequence.
//! * `VertInf::aStarPendingNodes` / `aStarDoneNodes` (`std::list<ANode *>`)
//!   are `Vec<ANodeIdx>` on the vertex; they are only appended to, erased
//!   from (first match) and iterated, and are cleared after every search.

#![allow(dead_code)]
#![allow(clippy::new_without_default)]
#![allow(
    clippy::collapsible_if,
    clippy::collapsible_else_if,
    clippy::too_many_arguments,
    clippy::nonminimal_bool,
    clippy::if_same_then_else,
    clippy::needless_range_loop
)]

use std::f64::consts::PI as M_PI;

use crate::avoid::arena::{ANodeIdx, ConnIdx, VertIdx};
use crate::avoid::connector::{
    validate_bend_point, ConnType, ConnectorCrossings, CROSSING_SHARES_FIXED_SEGMENT, CROSSING_SHARES_PATH,
    CROSSING_SHARES_PATH_AT_END,
};
use crate::avoid::geometry::{euclidean_dist, manhattan_dist, vec_dir};
use crate::avoid::geomtypes::{cpp_min, Point, Polygon, PolygonInterface, XDIM, YDIM};
use crate::avoid::router::{Router, RoutingOption, RoutingParameter};
use crate::avoid::vertices::{VertID, VertIDProps, VertInf, XH_EDGE, XL_EDGE, YH_EDGE, YL_EDGE};

// upstream: makepath.cpp class ANode
#[derive(Clone, Copy, Debug)]
struct ANode {
    inf: Option<VertIdx>,
    /// Gone
    g: f64,
    /// Heuristic
    h: f64,
    /// Formula f = g + h
    f: f64,

    /// VertInf for the previous ANode.
    prev_node: Option<ANodeIdx>,
    /// Time-stamp used to determine exploration order of seemingly equal
    /// paths during orthogonal routing.
    time_stamp: i32,
}

impl ANode {
    // upstream: makepath.cpp ANode::ANode(VertInf *, int)
    fn new(vinf: VertIdx, time: i32) -> ANode {
        ANode {
            inf: Some(vinf),
            g: 0.0,
            h: 0.0,
            f: 0.0,
            prev_node: None,
            time_stamp: time,
        }
    }
}

impl Default for ANode {
    // upstream: makepath.cpp ANode::ANode()
    fn default() -> ANode {
        ANode {
            inf: None,
            g: 0.0,
            h: 0.0,
            f: 0.0,
            prev_node: None,
            time_stamp: -1,
        }
    }
}

/// Node storage (upstream: the ANode memory blocks).
type ANodes = Vec<ANode>;

#[inline]
fn inf_of(nodes: &ANodes, n: ANodeIdx) -> VertIdx {
    nodes[n.0 as usize].inf.expect("ANode without VertInf")
}

/// This returns the opposite result (>) so that when used with
/// stl::make_heap, the head node of the heap will be the smallest value,
/// rather than the largest.
// upstream: makepath.cpp class ANodeCmp::operator()
fn a_node_cmp(a: &ANode, b: &ANode) -> bool {
    // We need to use an epsilon here since otherwise the multiple addition
    // of floating point numbers that makes up the 'f' values cause a problem
    // with routings occasionally being non-deterministic.
    if (a.f - b.f).abs() > 0.0000001 {
        return a.f > b.f;
    }
    if a.time_stamp != b.time_stamp {
        // Tiebreaker, if two paths have equal cost, then choose the one with
        // the highest timeStamp.  This corresponds to the furthest point
        // explored along the straight-line path.  When exploring we give the
        // directions the following timeStamps; left:1, right:2 and forward:3,
        // then we always try to explore forward first.
        return a.time_stamp < b.time_stamp;
    }
    false
}

// ---------------------------------------------------------------------------
// libstdc++ heap algorithms (bits/stl_heap.h), ported step by step.
// ---------------------------------------------------------------------------

/// libstdc++ `std::__push_heap`.
fn libstdcxx_push_heap_hole<T: Copy, F: FnMut(&T, &T) -> bool>(
    v: &mut [T],
    mut hole_index: isize,
    top_index: isize,
    value: T,
    comp: &mut F,
) {
    let mut parent = (hole_index - 1) / 2;
    while hole_index > top_index && comp(&v[parent as usize], &value) {
        v[hole_index as usize] = v[parent as usize];
        hole_index = parent;
        parent = (hole_index - 1) / 2;
    }
    v[hole_index as usize] = value;
}

/// libstdc++ `std::__adjust_heap`.
fn libstdcxx_adjust_heap<T: Copy, F: FnMut(&T, &T) -> bool>(
    v: &mut [T],
    mut hole_index: isize,
    len: isize,
    value: T,
    comp: &mut F,
) {
    let top_index = hole_index;
    let mut second_child = hole_index;
    while second_child < (len - 1) / 2 {
        second_child = 2 * (second_child + 1);
        if comp(&v[second_child as usize], &v[(second_child - 1) as usize]) {
            second_child -= 1;
        }
        v[hole_index as usize] = v[second_child as usize];
        hole_index = second_child;
    }
    if (len & 1) == 0 && second_child == (len - 2) / 2 {
        second_child = 2 * (second_child + 1);
        v[hole_index as usize] = v[(second_child - 1) as usize];
        hole_index = second_child - 1;
    }
    libstdcxx_push_heap_hole(v, hole_index, top_index, value, comp);
}

/// libstdc++ `std::make_heap(first, last, comp)`.
pub fn make_heap<T: Copy, F: FnMut(&T, &T) -> bool>(v: &mut [T], comp: &mut F) {
    let len = v.len() as isize;
    if len < 2 {
        return;
    }
    let mut parent = (len - 2) / 2;
    loop {
        let value = v[parent as usize];
        libstdcxx_adjust_heap(v, parent, len, value, comp);
        if parent == 0 {
            return;
        }
        parent -= 1;
    }
}

/// libstdc++ `std::push_heap(first, last, comp)` (the new element is the
/// last one of `v`).
pub fn push_heap<T: Copy, F: FnMut(&T, &T) -> bool>(v: &mut [T], comp: &mut F) {
    let len = v.len() as isize;
    if len == 0 {
        return;
    }
    let value = v[(len - 1) as usize];
    libstdcxx_push_heap_hole(v, len - 1, 0, value, comp);
}

/// libstdc++ `std::pop_heap(first, last, comp)`: moves the top to the last
/// position and re-heapifies the rest.
pub fn pop_heap<T: Copy, F: FnMut(&T, &T) -> bool>(v: &mut [T], comp: &mut F) {
    let len = v.len() as isize;
    if len > 1 {
        let last = len - 1;
        // __pop_heap(first, last, last, comp)
        let value = v[last as usize];
        v[last as usize] = v[0];
        libstdcxx_adjust_heap(&mut v[..last as usize], 0, last, value, comp);
    }
}

// ---------------------------------------------------------------------------

// upstream: makepath.cpp Dot() (static)
fn dot(l: &Point, r: &Point) -> f64 {
    (l.x * r.x) + (l.y * r.y)
}

// upstream: makepath.cpp CrossLength() (static)
fn cross_length(l: &Point, r: &Point) -> f64 {
    (l.x * r.y) - (l.y * r.x)
}

/// Return the angle between the two line segments made by the
/// points p1--p2 and p2--p3.  Return value is in radians.
// upstream: makepath.cpp angleBetween() (static)
fn angle_between(p1: &Point, p2: &Point, p3: &Point) -> f64 {
    if (p1.x == p2.x && p1.y == p2.y) || (p2.x == p3.x && p2.y == p3.y) {
        // If two of the points are the same, then we can't say anything
        // about the angle between.  Treat them as being collinear.
        return M_PI;
    }

    let v1 = Point::new(p1.x - p2.x, p1.y - p2.y);
    let v2 = Point::new(p3.x - p2.x, p3.y - p2.y);

    cross_length(&v1, &v2).atan2(dot(&v1, &v2)).abs()
}

/// Construct a temporary Polygon path given several VertInf's for a
/// connector.
// upstream: makepath.cpp constructPolygonPath() (static)
fn construct_polygon_path(
    router: &Router,
    nodes: &ANodes,
    conn_route: &mut Polygon,
    inf2: VertIdx,
    inf3: VertIdx,
    inf1_node: Option<ANodeIdx>,
) {
    let verts = &router.vert_infs;
    // Don't include colinear points.
    let simplified = true;

    let mut route_size: isize = 2;
    let mut curr = inf1_node;
    while let Some(c) = curr {
        route_size += 1;
        curr = nodes[c.0 as usize].prev_node;
    }
    conn_route.ps.resize(route_size as usize, Point::default());
    let array_size = route_size;
    conn_route.ps[(route_size - 1) as usize] = verts[inf3].point;
    conn_route.ps[(route_size - 2) as usize] = verts[inf2].point;
    route_size -= 3;
    let mut curr = inf1_node;
    while let Some(c) = curr {
        let c_inf = inf_of(nodes, c);
        // For connection pins, we stop and don't include the fake shape
        // center as part of this path.
        let is_connection_pin = verts[c_inf].id.is_connection_pin();

        if !simplified {
            // If this is non-simplified, we don't need to do anything
            // clever and can simply add the new point.
            conn_route.ps[route_size as usize] = verts[c_inf].point;
            route_size -= 1;

            if is_connection_pin {
                // Stop at the connection pin.
                break;
            }
            curr = nodes[c.0 as usize].prev_node;
            continue;
        }

        if (Some(c) == inf1_node)
            || vec_dir(
                &verts[c_inf].point,
                &conn_route.ps[(route_size + 1) as usize],
                &conn_route.ps[(route_size + 2) as usize],
                0.0,
            ) != 0
        {
            // Add new point if this is the earlier than the last segment
            // and it is not colinear with the other points.
            // Note, you can't collapse the 'last' segment with previous
            // segments, or if this just intersects another line you risk
            // penalising it once for each collapsed line segment.
            conn_route.ps[route_size as usize] = verts[c_inf].point;
            route_size -= 1;
        } else {
            // The last point is inline with this one, so update it.
            conn_route.ps[(route_size + 1) as usize] = verts[c_inf].point;
        }

        if is_connection_pin {
            // Stop at the connection pin.
            break;
        }
        curr = nodes[c.0 as usize].prev_node;
    }

    // If the vector is not filled, move entries to the beginning and
    // remove the unused end of the vector.
    let diff = route_size + 1;
    debug_assert!(simplified || (diff == 0));
    if diff > 0 {
        for i in diff..array_size {
            conn_route.ps[(i - diff) as usize] = conn_route.ps[i as usize];
        }
        let new_len = conn_route.size() - diff as usize;
        conn_route.ps.truncate(new_len);
    }
}

/// Used to get an indication of if a diffence is positive (1),
/// negative (-1) or no different (0).
// upstream: makepath.cpp dimDirection() (static inline)
#[inline]
fn dim_direction(difference: f64) -> i32 {
    if difference > 0.0 {
        return 1;
    } else if difference < 0.0 {
        return -1;
    }
    0
}

/// Given the two points for a new segment of a path (inf2 & inf3)
/// as well as the distance between these points (dist), as well as
/// possibly the previous point (inf1) [from inf1--inf2], return a
/// cost associated with this route.
// upstream: makepath.cpp cost() (static)
fn cost(
    router: &mut Router,
    nodes: &ANodes,
    line_ref: ConnIdx,
    dist: f64,
    inf2: VertIdx,
    inf3: VertIdx,
    inf1_node: Option<ANodeIdx>,
) -> f64 {
    let is_orthogonal = router.conns[line_ref].routing_type() == ConnType::ConnTypeOrthogonal;
    let inf1 = inf1_node.map(|n| inf_of(nodes, n));
    let mut result = dist;
    let mut conn_route = Polygon::new();

    if let Some(inf1) = inf1 {
        let angle_penalty = router.routing_parameter(RoutingParameter::AnglePenalty);
        let segmt_penalty = router.routing_parameter(RoutingParameter::SegmentPenalty);

        // This is not the first segment, so there is a bend
        // between it and the last one in the existing path.
        if (angle_penalty > 0.0) || (segmt_penalty > 0.0) {
            let p1 = router.vert_infs[inf1].point;
            let p2 = router.vert_infs[inf2].point;
            let p3 = router.vert_infs[inf3].point;

            let rad = M_PI - angle_between(&p1, &p2, &p3);

            if (rad > 0.0) && !is_orthogonal {
                // Make `xval' between 0--10 then take its log so small
                // angles are not penalised as much as large ones.
                //
                let xval = rad * 10.0 / M_PI;
                let yval = xval * (xval + 1.0).log10() / 10.5;
                result += angle_penalty * yval;
            }

            if rad == M_PI {
                // Needs to double back
                result += 2.0 * segmt_penalty;
            } else if rad > 0.0 {
                // Only penalise as an extra segment if the two
                // segments are not collinear.
                result += segmt_penalty;
            }
        }
    }

    let cluster_crossing_penalty = router.routing_parameter(RoutingParameter::ClusterCrossingPenalty);
    // XXX: Clustered routing doesn't yet work with orthogonal connectors.
    if router.clustered_routing && !router.cluster_refs.is_empty() && (cluster_crossing_penalty > 0.0) {
        if conn_route.empty() {
            construct_polygon_path(router, nodes, &mut conn_route, inf2, inf3, inf1_node);
        }
        // There are clusters so do cluster routing.
        let clusters = router.cluster_refs.clone();
        for cl in clusters {
            let mut c_boundary = if is_orthogonal {
                router.clusters[cl].rectangular_polygon().clone()
            } else {
                Polygon::from_interface(&router.clusters[cl].polygon().view(&*router))
            };
            if c_boundary.size() <= 2 {
                continue;
            }
            debug_assert!(c_boundary.ps[0] != c_boundary.ps[c_boundary.size() - 1]);
            // Upstream also asserts that every non-orthogonal cluster
            // boundary point is a router vertex ("should correspond to shape
            // vertices").  Not ported (assert policy): ClusterRef accepts any
            // polygon, the check has no effect on the result and the NDEBUG
            // build works (tests/robustness.rs clusters).

            let is_conn = false;
            let mut dynamic_conn_route = conn_route.clone();
            let final_segment = Some(inf3) == router.conns[line_ref].dst();
            let mut cross = ConnectorCrossings::new(is_conn, None, None);
            cross.check_for_branching_segments = true;
            cross.count_for_segment(
                &*router,
                &mut c_boundary,
                &mut dynamic_conn_route,
                conn_route.size() - 1,
                final_segment,
            );

            result += cross.crossing_count as f64 * cluster_crossing_penalty;
        }
    }

    // This penalty penalises route segments that head in a direction opposite
    // of the direction(s) toward the target point.
    let reverse_penalty = router.routing_parameter(RoutingParameter::ReverseDirectionPenalty);
    if reverse_penalty != 0.0 {
        // X and Y direction of destination from source point.
        let src_point = router.vert_infs[router.conns[line_ref].src().unwrap()].point;
        let dst_point = router.vert_infs[router.conns[line_ref].dst().unwrap()].point;
        let x_dir = dim_direction(dst_point.x - src_point.x);
        let y_dir = dim_direction(dst_point.y - src_point.y);

        let mut does_reverse = false;

        let p2 = router.vert_infs[inf2].point;
        let p3 = router.vert_infs[inf3].point;
        if (x_dir != 0) && (-x_dir == dim_direction(p3.x - p2.x)) {
            // Connector has an X component and the segment heads in the
            // opposite direction.
            does_reverse |= true;
        }

        if (y_dir != 0) && (-y_dir == dim_direction(p3.y - p2.y)) {
            // Connector has an Y component and the segment heads in the
            // opposite direction.
            does_reverse |= true;
        }

        if does_reverse {
            result += reverse_penalty;
        }
    }

    if !router.is_in_crossing_penalty_rerouting_stage() {
        // Return here if we are not in the post-processing stage
        return result;
    }

    let crossing_penalty = router.routing_parameter(RoutingParameter::CrossingPenalty);
    let shared_path_penalty = router.routing_parameter(RoutingParameter::FixedSharedPathPenalty);
    if (shared_path_penalty > 0.0) || (crossing_penalty > 0.0) {
        if conn_route.empty() {
            construct_polygon_path(router, nodes, &mut conn_route, inf2, inf3, inf1_node);
        }
        let line_ref_id = router.conns[line_ref].id();
        let conn_refs = router.conn_refs.clone();
        for conn_ref in conn_refs {
            if router.conns[conn_ref].id() == line_ref_id {
                continue;
            }
            let route2 = router.conns[conn_ref].display_route().clone();

            let is_conn = true;
            let mut dynamic_route2 = route2;
            let mut dynamic_conn_route = conn_route.clone();
            let dst_point = router.vert_infs[router.conns[line_ref].dst().unwrap()].point;
            let final_segment = router.vert_infs[inf3].point == dst_point;
            let mut cross = ConnectorCrossings::new(is_conn, Some(conn_ref), Some(line_ref));
            cross.check_for_branching_segments = true;
            cross.count_for_segment(
                &*router,
                &mut dynamic_route2,
                &mut dynamic_conn_route,
                conn_route.size() - 1,
                final_segment,
            );

            if (cross.crossing_flags & CROSSING_SHARES_PATH) != 0
                && (cross.crossing_flags & CROSSING_SHARES_FIXED_SEGMENT) != 0
                && (router.routing_option(RoutingOption::PenaliseOrthogonalSharedPathsAtConnEnds)
                    || (cross.crossing_flags & CROSSING_SHARES_PATH_AT_END) == 0)
            {
                // Penalise unnecessary shared paths in the middle of
                // connectors.
                result += shared_path_penalty;
            }
            result += cross.crossing_count as f64 * crossing_penalty;
        }
    }

    result
}

// Directions for estimated orthgonal cost, as bitflags.
// upstream: makepath.cpp CostDirectionN/E/S/W
const COST_DIRECTION_N: u32 = 1;
const COST_DIRECTION_E: u32 = 2;
const COST_DIRECTION_S: u32 = 4;
const COST_DIRECTION_W: u32 = 8;

/// Returns the number of directions for the argument.
// upstream: makepath.cpp orthogonalDirectionsCount() (static)
/// Writes the direction flags as `N E S W ` (upstream: only with
/// `ESTIMATED_COST_DEBUG`, here the `estimated-cost-debug` feature, which
/// prints the A* cost targets to stderr).
// upstream: makepath.cpp printDirections() (static)
#[cfg_attr(not(feature = "estimated-cost-debug"), allow(dead_code))]
fn print_directions(fp: &mut dyn std::fmt::Write, directions: u32) -> std::fmt::Result {
    if (directions & COST_DIRECTION_N) != 0 {
        fp.write_str("N ")?;
    }
    if (directions & COST_DIRECTION_E) != 0 {
        fp.write_str("E ")?;
    }
    if (directions & COST_DIRECTION_S) != 0 {
        fp.write_str("S ")?;
    }
    if (directions & COST_DIRECTION_W) != 0 {
        fp.write_str("W ")?;
    }
    Ok(())
}

fn orthogonal_directions_count(directions: u32) -> u32 {
    let mut count = 0;
    if directions & COST_DIRECTION_N != 0 {
        count += 1;
    }
    if directions & COST_DIRECTION_E != 0 {
        count += 1;
    }
    if directions & COST_DIRECTION_S != 0 {
        count += 1;
    }
    if directions & COST_DIRECTION_W != 0 {
        count += 1;
    }
    count
}

/// Returns the directions of point b from point a.
// upstream: makepath.cpp orthogonalDirection() (static)
fn orthogonal_direction(a: &Point, b: &Point) -> u32 {
    let mut result = 0;

    if b.y > a.y {
        result |= COST_DIRECTION_S;
    } else if b.y < a.y {
        result |= COST_DIRECTION_N;
    }

    if b.x > a.x {
        result |= COST_DIRECTION_E;
    } else if b.x < a.x {
        result |= COST_DIRECTION_W;
    }

    result
}

/// Returns the direction to the right of the given direction.
// upstream: makepath.cpp dirRight() (static)
fn dir_right(direction: u32) -> u32 {
    if direction == COST_DIRECTION_N {
        return COST_DIRECTION_E;
    } else if direction == COST_DIRECTION_E {
        return COST_DIRECTION_S;
    } else if direction == COST_DIRECTION_S {
        return COST_DIRECTION_W;
    } else if direction == COST_DIRECTION_W {
        return COST_DIRECTION_N;
    }

    // Should not be possible to reach here.
    debug_assert!(false);
    direction
}

/// Returns the direction to the left of the given direction.
// upstream: makepath.cpp dirLeft() (static)
fn dir_left(direction: u32) -> u32 {
    if direction == COST_DIRECTION_N {
        return COST_DIRECTION_W;
    } else if direction == COST_DIRECTION_E {
        return COST_DIRECTION_N;
    } else if direction == COST_DIRECTION_S {
        return COST_DIRECTION_E;
    } else if direction == COST_DIRECTION_W {
        return COST_DIRECTION_S;
    }

    // Should not be possible to reach here.
    debug_assert!(false);
    direction
}

/// Returns the reverse direction to the given direction.
// upstream: makepath.cpp dirReverse() (static)
fn dir_reverse(direction: u32) -> u32 {
    if direction == COST_DIRECTION_N {
        return COST_DIRECTION_S;
    } else if direction == COST_DIRECTION_E {
        return COST_DIRECTION_W;
    } else if direction == COST_DIRECTION_S {
        return COST_DIRECTION_N;
    } else if direction == COST_DIRECTION_W {
        return COST_DIRECTION_E;
    }

    // Should not be possible to reach here.
    debug_assert!(false);
    direction
}

/// Given Point curr with a direction of currDir, returns the nimimum
/// number of bends to reach Point dest with the entry direction of
/// destDir.
///
/// This is used for estimating the bend penalty cost to the target point
/// from the current point of the search. The geometry was described in
/// the "Orthogonal Connector Routing" paper, although the version
/// described there is incorrect.
// upstream: makepath.cpp bends()
pub fn bends(curr: &Point, curr_dir: u32, dest: &Point, dest_dir: u32) -> i32 {
    // Bend counts from 'o' to 'D' should be:
    //
    //                1            1            3
    //                v            v            v
    //            2 > o < 2    2 > o < 2    4 > o < 2
    //                ^            ^            ^
    //                3            3            3
    //
    //   0 > o < 4                 D-->             4 > o < 4
    //       ^                                          ^
    //       1                                          3
    //
    debug_assert!(curr_dir != 0);
    let curr_to_dest_dir = orthogonal_direction(curr, dest);
    let reverse_dest_dir = dir_reverse(dest_dir);
    let curr_dir_perpendicular_to_dest_dir = (curr_dir == dir_left(dest_dir)) || (curr_dir == dir_right(dest_dir));

    if (curr_dir == dest_dir) && (curr_to_dest_dir == curr_dir) {
        //
        //   0 > o                     D-->
        //
        return 0;
    } else if curr_dir_perpendicular_to_dest_dir && (curr_to_dest_dir == (dest_dir | curr_dir)) {
        //
        //                1
        //                v
        //                o
        //
        //
        //                             D-->
        //
        return 1;
    } else if curr_dir_perpendicular_to_dest_dir && (curr_to_dest_dir == curr_dir) {
        //
        //                             1
        //                             v
        //                             o
        //
        //
        //                             D-->
        //
        return 1;
    } else if curr_dir_perpendicular_to_dest_dir && (curr_to_dest_dir == dest_dir) {
        //
        //       o                     D-->
        //       ^
        //       1
        //
        return 1;
    } else if (curr_dir == dest_dir) && (curr_to_dest_dir != curr_dir) && (curr_to_dest_dir & reverse_dest_dir) == 0 {
        //
        //            2 > o        2 > o
        //
        //
        //                             D-->
        //
        return 2;
    } else if curr_dir == reverse_dest_dir && (curr_to_dest_dir != dest_dir) && (curr_to_dest_dir != curr_dir) {
        //
        //                o < 2        o < 2       o < 2
        //
        //
        //                             D-->
        //
        return 2;
    } else if curr_dir_perpendicular_to_dest_dir
        && (curr_to_dest_dir != (dest_dir | curr_dir))
        && (curr_to_dest_dir != curr_dir)
    {
        //
        //                                          3
        //                                          v
        //                o            o            o
        //                ^            ^            ^
        //                3            3            3
        //
        //                             D-->                 o
        //                                                  ^
        //                                                  3
        //
        return 3;
    } else if (curr_dir == reverse_dest_dir) && ((curr_to_dest_dir == dest_dir) || (curr_to_dest_dir == curr_dir)) {
        //
        //
        //
        //       o < 4                 D-->                 o < 4
        //
        return 4;
    } else if (curr_dir == dest_dir) && (curr_to_dest_dir & reverse_dest_dir) != 0 {
        //
        //                                      4 > o
        //
        //
        //                             D-->             4 > o
        //
        return 4;
    }

    // Should not be possible to reach here.
    debug_assert!(false);
    0
}

// upstream: makepath.cpp estimatedCostSpecific() (static)
fn estimated_cost_specific(
    router: &Router,
    line_ref: ConnIdx,
    last: Option<&Point>,
    curr: &Point,
    cost_tar: VertIdx,
    cost_tar_dirs: u32,
) -> f64 {
    let cost_tar_point = router.vert_infs[cost_tar].point;

    if router.conns[line_ref].routing_type() == ConnType::ConnTypePolyLine {
        euclidean_dist(curr, &cost_tar_point)
    } else {
        // Orthogonal
        // Really doesn't make sense to route orthogonal paths without
        // a segment penalty.
        // (Upstream asserts segmentPenalty > 0.  Not ported (assert
        // policy): setRoutingParameter(segmentPenalty, 0) is accepted API
        // and the NDEBUG build routes; tests/robustness.rs
        // extreme_penalties.)

        let dist = manhattan_dist(curr, &cost_tar_point);

        let mut bend_count: i32 = 0;
        let xmove = cost_tar_point.x - curr.x;
        let ymove = cost_tar_point.y - curr.y;
        match last {
            None => {
                // This is just the initial point.  Penalise it simply if it is
                // not inline with the target in either the x- or y-dimension.
                if (xmove != 0.0) && (ymove != 0.0) {
                    bend_count += 1;
                }
            }
            Some(last) => {
                if dist > 0.0 {
                    // We have two points and a non-zero distance, so we know
                    // the segment direction.

                    let curr_dir = orthogonal_direction(last, curr);
                    if (curr_dir > 0) && (orthogonal_directions_count(curr_dir) == 1) {
                        // Suitably high value, then we find the minimum.
                        bend_count = 10;

                        // Find the minimum bent penalty given all the possible
                        // directions at the target point.
                        if cost_tar_dirs & COST_DIRECTION_N != 0 {
                            bend_count =
                                std::cmp::min(bend_count, bends(curr, curr_dir, &cost_tar_point, COST_DIRECTION_N));
                        }
                        if cost_tar_dirs & COST_DIRECTION_E != 0 {
                            bend_count =
                                std::cmp::min(bend_count, bends(curr, curr_dir, &cost_tar_point, COST_DIRECTION_E));
                        }
                        if cost_tar_dirs & COST_DIRECTION_S != 0 {
                            bend_count =
                                std::cmp::min(bend_count, bends(curr, curr_dir, &cost_tar_point, COST_DIRECTION_S));
                        }
                        if cost_tar_dirs & COST_DIRECTION_W != 0 {
                            bend_count =
                                std::cmp::min(bend_count, bends(curr, curr_dir, &cost_tar_point, COST_DIRECTION_W));
                        }
                    }
                }
            }
        }
        let penalty = bend_count as f64 * router.routing_parameter(RoutingParameter::SegmentPenalty);

        dist + penalty
    }
}

// upstream: makepath.cpp class CmpVisEdgeRotation — see
// crate::avoid::graph::cmp_vis_edge_rotation.
use crate::avoid::graph::cmp_vis_edge_rotation;

// upstream: makepath.cpp pointAlignedWithOneOf() (static inline)
#[inline]
fn point_aligned_with_one_of(point: &Point, points: &[Point], dim: usize) -> bool {
    for p in points.iter() {
        if point[dim] == p[dim] {
            return true;
        }
    }
    false
}

// upstream: makepath.cpp class AStarPathPrivate
#[derive(Debug, Default)]
struct AStarPathPrivate {
    /// Upstream `m_available_nodes` blocks (with the array/node indices).
    m_available_nodes: Vec<ANode>,

    // For determining estimated cost target.
    m_cost_targets: Vec<VertIdx>,
    m_cost_targets_directions: Vec<u32>,
    m_cost_targets_displacements: Vec<f64>,
}

impl AStarPathPrivate {
    /// Returns a handle to an ANode for aStar search.  Upstream default for
    /// `add_to_pending` is true.
    // upstream: makepath.cpp AStarPathPrivate::newANode()
    fn new_a_node(&mut self, router: &mut Router, node: &ANode, add_to_pending: bool) -> ANodeIdx {
        let new_node = ANodeIdx(self.m_available_nodes.len() as u32);
        self.m_available_nodes.push(*node);
        if add_to_pending {
            router.vert_infs[node.inf.unwrap()].a_star_pending_nodes.push(new_node);
        }
        new_node
    }

    // upstream: makepath.cpp AStarPathPrivate::determineEndPointLocation()
    fn determine_end_point_location(
        &mut self,
        router: &Router,
        dist: f64,
        start: VertIdx,
        target: VertIdx,
        other: VertIdx,
        level: i32,
    ) {
        #[cfg(not(feature = "estimated-cost-debug"))]
        let _ = (dist, start, level);

        let other_point = router.vert_infs[other].point;
        let target_point = router.vert_infs[target].point;
        let this_dirs = orthogonal_direction(&other_point, &target_point);
        // Upstream: COLA_ASSERT(orthogonalDirectionsCount(thisDirs) > 0).
        // Not ported (assert policy): upstream's asserting build fails it on
        // valid input -- a junction at (50,0) between point ends at (0,0)
        // and (100,0), orthogonal routing (e2e fuzz regression scene
        // reg-assert-endpoint-direction, and 24 `#assert` scenes of the
        // committed e2e fuzz golden) -- while its NDEBUG build routes the
        // scene (thisDirs == 0 only weakens the cost estimate); the port
        // matches that output.
        let displacement = manhattan_dist(&other_point, &target_point);

        self.m_cost_targets.push(other);
        self.m_cost_targets_directions.push(this_dirs);
        self.m_cost_targets_displacements.push(displacement);

        #[cfg(feature = "estimated-cost-debug")]
        {
            use crate::avoid::debug::g;
            let mut fp = String::new();
            fp.push_str(&format!(" - {} {} ", g(other_point.x), g(other_point.y)));
            if manhattan_dist(&router.vert_infs[start].point, &other_point) > dist {
                fp.push_str("far ");
            }
            fp.push_str(if level == 1 { "--" } else { "- " });
            let _ = print_directions(&mut fp, this_dirs);
            fp.push('\n');
            eprint!("{}", fp);
        }
    }

    // upstream: makepath.cpp AStarPathPrivate::estimatedCost()
    fn estimated_cost(&self, router: &Router, line_ref: ConnIdx, last: Option<&Point>, curr: &Point) -> f64 {
        let mut estimate = f64::MAX;
        debug_assert!(!self.m_cost_targets.is_empty());

        // Find the minimum cost from the estimates to each of the possible
        // target points from this current point.
        for i in 0..self.m_cost_targets.len() {
            let mut i_estimate = estimated_cost_specific(
                router,
                line_ref,
                last,
                curr,
                self.m_cost_targets[i],
                self.m_cost_targets_directions[i],
            );

            // Add on the distance to the real target, otherwise this difference
            // might may make the comparisons unfair if they vary between targets.
            i_estimate += self.m_cost_targets_displacements[i];

            estimate = cpp_min(estimate, i_estimate);
        }
        estimate
    }

    /// Returns the best path from src to tar using the cost function.
    ///
    /// The path is worked out using the aStar algorithm, and is encoded via
    /// prevNode values for each ANode which point back to the previous
    /// ANode.  At completion, this order is written into the pathNext links
    /// in each of the VerInfs along the path.
    // upstream: makepath.cpp AStarPathPrivate::search()
    fn search(
        &mut self,
        router: &mut Router,
        line_ref: ConnIdx,
        src: VertIdx,
        tar: VertIdx,
        start: Option<VertIdx>,
    ) {
        let is_orthogonal = router.conns[line_ref].routing_type() == ConnType::ConnTypeOrthogonal;

        let start = match start {
            None => src,
            Some(s) => s,
        };

        #[cfg(feature = "debug-handler")]
        router.call_debug_handler(|h, r| h.beginning_search_with_endpoints(r, start, tar));

        // Find a target point to use for cost estimate for orthogonal routing.
        //
        // If the connectivity is only on the far side we need to estimate to the
        // point on the far side.  Otherwise for orthogonal routing we can explore
        // all the space in between before we pay the extra cost to explore this
        // area.  This is especially true given many orthogonal routes have
        // equivalent costs.
        #[cfg(feature = "estimated-cost-debug")]
        eprint!(
            "== aStar  {} {} ==\n",
            crate::avoid::debug::g(router.vert_infs[tar].point.x),
            crate::avoid::debug::g(router.vert_infs[tar].point.y)
        );
        let tar_id = router.vert_infs[tar].id;
        if is_orthogonal && tar_id.is_conn_pt() && !tar_id.is_conn_checkpoint() {
            // The target is a connector endpoint and the connector is orthogonal.
            let dist = manhattan_dist(&router.vert_infs[start].point, &router.vert_infs[tar].point);
            let tar_edges = router.vert_infs[tar].orthog_vis_list.to_vec(&router.edge_infs);
            for edge in tar_edges {
                // For each edge from the target endpoint, find the other vertex.
                let other = router.edge_infs[edge].other_vert(tar);
                if router.vert_infs[other].id.is_connection_pin() {
                    // If this is a connection pin we need to do this process
                    // another time since the current edge will be a dummy
                    // zero-length edge.
                    let replacement_tar = other;
                    let rep_edges = router.vert_infs[replacement_tar]
                        .orthog_vis_list
                        .to_vec(&router.edge_infs);
                    for edge in rep_edges {
                        let other = router.edge_infs[edge].other_vert(replacement_tar);
                        if (other == tar) || (router.vert_infs[other].point == router.vert_infs[tar].point) {
                            // Ignore edge we came from, or zero-length edges.
                            continue;
                        }

                        // Determine possible target endpoint directions and
                        // position.
                        self.determine_end_point_location(router, dist, start, replacement_tar, other, 2);
                    }
                    continue;
                }

                // Determine possible target endpoint directions and position.
                self.determine_end_point_location(router, dist, start, tar, other, 1);
            }
        }

        if self.m_cost_targets.is_empty() {
            self.m_cost_targets.push(tar);
            // For polyline routing, assume target has visibility is all
            // directions for the purpose of cost estimations.
            self.m_cost_targets_directions
                .push(COST_DIRECTION_N | COST_DIRECTION_E | COST_DIRECTION_S | COST_DIRECTION_W);
            self.m_cost_targets_displacements.push(0.0);
        }

        #[cfg(feature = "estimated-cost-debug")]
        {
            use crate::avoid::debug::g;
            let mut fp = String::from("------------\n");
            for i in 0..self.m_cost_targets.len() {
                let p = router.vert_infs[self.m_cost_targets[i]].point;
                fp.push_str(&format!("== {} {} - ", g(p.x), g(p.y)));
                let _ = print_directions(&mut fp, self.m_cost_targets_directions[i]);
                fp.push('\n');
            }
            eprint!("{}", fp);
        }

        let dist: fn(&Point, &Point) -> f64 = if is_orthogonal { manhattan_dist } else { euclidean_dist };

        // We need to know the possible endpoints for doing an orthogonal
        // routing optimisation where we only turn when we are heading beside
        // a shape or are in line with a possible endpoint.
        let mut end_points: Vec<Point> = Vec::new();
        if is_orthogonal {
            end_points = router.conns[line_ref].possible_dst_pin_points(router);
        }
        end_points.push(router.vert_infs[tar].point);

        // Heap of PENDING nodes.
        let mut pending: Vec<ANodeIdx> = Vec::with_capacity(1000);

        let mut explored_count: usize = 0;
        let mut node: ANode;
        let mut ati: ANode;
        let mut best_node: Option<ANodeIdx> = None; // Temporary bestNode
        let mut b_node_found: bool; // Flag if node is found in container
        let mut timestamp: i32 = 1;

        // Port addition (robustness): upstream walks the current route from
        // its first point until it reaches `start`, dereferencing every
        // vertex looked up by id.  If a route point has no vertex (e.g. a
        // fixed route through arbitrary points) or `start` is not on the
        // route, upstream dereferences null / reads past the route.  That
        // walk is checked first (read-only); if it would fail, the search
        // starts afresh from `src` as for a non-rubber-band search.
        let rubber_band_walk_ok = router.rubber_band_routing && (start != src) && {
            let curr_route = router.conns[line_ref].route();
            let mut ok = false;
            for r_indx in 0..curr_route.size() {
                let pnt = curr_route.at(r_indx);
                let props: VertIDProps = if r_indx > 0 { 0 } else { VertID::PROP_CONN_POINT };
                match router.vertices.get_vertex_by_id(&router.vert_infs, &VertID::new(pnt.id, pnt.vn, props)) {
                    None => break,
                    Some(v) if v == start => {
                        ok = true;
                        break;
                    }
                    Some(_) => {}
                }
            }
            ok
        };
        if rubber_band_walk_ok {
            debug_assert!(router.ignore_regions);

            let curr_route = router.conns[line_ref].route().clone();
            let mut last: Option<VertIdx> = None;
            let mut r_indx: usize = 0;
            while last != Some(start) {
                let pnt = curr_route.at(r_indx);
                let props: VertIDProps = if r_indx > 0 { 0 } else { VertID::PROP_CONN_POINT };
                let v_id = VertID::new(pnt.id, pnt.vn, props);

                let curr = router.vertices.get_vertex_by_id(&router.vert_infs, &v_id);
                debug_assert!(curr.is_some());
                let curr = curr.unwrap();

                node = ANode::new(curr, timestamp);
                timestamp += 1;
                if last.is_none() {
                    node.inf = Some(src);
                    node.g = 0.0;
                    let p = router.vert_infs[src].point;
                    node.h = self.estimated_cost(router, line_ref, None, &p);

                    node.f = node.g + node.h;
                } else {
                    let bn = best_node.unwrap();
                    let bn_node = self.m_available_nodes[bn.0 as usize];
                    let bn_inf = bn_node.inf.unwrap();
                    let edge_dist = dist(&router.vert_infs[bn_inf].point, &router.vert_infs[curr].point);

                    node.g = bn_node.g
                        + cost(
                            router,
                            &self.m_available_nodes,
                            line_ref,
                            edge_dist,
                            bn_inf,
                            node.inf.unwrap(),
                            bn_node.prev_node,
                        );

                    // Calculate the Heuristic.
                    let bp = router.vert_infs[bn_inf].point;
                    let np = router.vert_infs[node.inf.unwrap()].point;
                    node.h = self.estimated_cost(router, line_ref, Some(&bp), &np);

                    // The A* formula
                    node.f = node.g + node.h;

                    // Point parent to last bestNode
                    node.prev_node = Some(bn);
                }

                if curr != start {
                    let add_to_pending = false;
                    let bn = self.new_a_node(router, &node, add_to_pending);
                    best_node = Some(bn);
                    let bn_inf = inf_of(&self.m_available_nodes, bn);
                    router.vert_infs[bn_inf].a_star_done_nodes.push(bn);
                    explored_count += 1;
                } else {
                    let new_node = self.new_a_node(router, &node, true);
                    pending.push(new_node);
                }

                r_indx += 1;
                last = Some(curr);
            }
        } else {
            // Stale-pathNext policy: a link to a deleted vertex reads as
            // null (VertInf::live_path_next; router.rs module docs).
            if let Some(start_next) = VertInf::live_path_next(router, start) {
                // If we are doing checkpoint routing and have already done one
                // path, then we have an existing segment to consider for the
                // cost of the  choice from the start node, so we add a dummy
                // nodes as if they were already in the Done set.  This causes
                // us to first search in a collinear direction from the previous
                // segment.
                let add_to_pending = false;
                let n = ANode::new(start_next, timestamp);
                timestamp += 1;
                let bn = self.new_a_node(router, &n, add_to_pending);
                best_node = Some(bn);
                router.vert_infs[start_next].a_star_done_nodes.push(bn);
                explored_count += 1;
            }

            // Create the start node
            node = ANode::new(src, timestamp);
            timestamp += 1;
            node.g = 0.0;
            let p = router.vert_infs[src].point;
            node.h = self.estimated_cost(router, line_ref, None, &p);
            node.f = node.g + node.h;
            // Set a nullptr parent, so cost function knows this is the first segment.
            node.prev_node = best_node;

            // Populate the PENDING container with the first location
            let new_node = self.new_a_node(router, &node, true);
            pending.push(new_node);
        }

        router.vert_infs[tar].path_next = None;

        // Create a heap from PENDING for sorting
        {
            let nodes = &self.m_available_nodes;
            make_heap(&mut pending, &mut |a: &ANodeIdx, b: &ANodeIdx| {
                a_node_cmp(&nodes[a.0 as usize], &nodes[b.0 as usize])
            });
        }

        let src_point = router.vert_infs[src].point;
        let line_src = router.conns[line_ref].src();
        let line_dst = router.conns[line_ref].dst();

        // Continue until the queue is empty.
        while !pending.is_empty() {
            // Set the Node with lowest f value to BESTNODE.
            // Since the ANode operator< is reversed, the head of the
            // heap is the node with the lowest f value.
            crate::avoid::timer::timer_var_add!(router, 0, 1);
            let bn = pending[0];
            // (upstream assigns bestNode here; `bn` plays that role below.)
            let best_node_inf = inf_of(&self.m_available_nodes, bn);

            #[cfg(feature = "debug-handler")]
            if router.has_debug_handler() {
                let mut current_search_path = crate::avoid::geomtypes::PolyLine::new();

                let mut curr = Some(bn);
                while let Some(c) = curr {
                    current_search_path.ps.push(router.vert_infs[inf_of(&self.m_available_nodes, c)].point);
                    curr = self.m_available_nodes[c.0 as usize].prev_node;
                }
                router.call_debug_handler(|h, r| h.update_current_search_path(r, current_search_path));
            }

            // Remove this node from the aStarPendingList
            {
                let list = &mut router.vert_infs[best_node_inf].a_star_pending_nodes;
                if let Some(pos) = list.iter().position(|&n| n == bn) {
                    list.remove(pos);
                }
            }

            // Pop off the heap.  Actually this moves the
            // far left value to the far right.  The node
            // is not actually removed since the pop is to
            // the heap and not the container.
            {
                let nodes = &self.m_available_nodes;
                pop_heap(&mut pending, &mut |a: &ANodeIdx, b: &ANodeIdx| {
                    a_node_cmp(&nodes[a.0 as usize], &nodes[b.0 as usize])
                });
            }
            // Remove node from right (the value we pop_heap'd)
            pending.pop();

            // Add the bestNode into the Done set.
            router.vert_infs[best_node_inf].a_star_done_nodes.push(bn);
            explored_count += 1;

            let bn_node = self.m_available_nodes[bn.0 as usize];
            let prev_inf: Option<VertIdx> = bn_node.prev_node.map(|p| inf_of(&self.m_available_nodes, p));

            if best_node_inf == tar {
                crate::avoid::timer::timer_var_add!(router, 1, pending.len() as u32);
                // This node is our goal.

                // Correct all the pathNext pointers.
                let mut curr = bn;
                while let Some(prev) = self.m_available_nodes[curr.0 as usize].prev_node {
                    let c_inf = inf_of(&self.m_available_nodes, curr);
                    router.vert_infs[c_inf].path_next = Some(inf_of(&self.m_available_nodes, prev));
                    curr = prev;
                }

                // Exit from the search
                break;
            }

            // Check adjacent points in graph and add them to the queue.
            let which = if !is_orthogonal {
                crate::avoid::vertices::VertEdgeList::Vis
            } else {
                crate::avoid::vertices::VertEdgeList::OrthogVis
            };
            if is_orthogonal {
                // We would like to explore in a structured way,
                // so sort the points in the visList...
                VertInf::sort_edge_list(router, best_node_inf, which, |r, u, v| {
                    cmp_vis_edge_rotation(r, prev_inf, u, v)
                });
            }
            let vis_list = router.vert_infs[best_node_inf].edge_list(which).to_vec(&router.edge_infs);
            for edge in vis_list {
                if router.edge_infs[edge].is_disabled() {
                    // Skip disabled edges.
                    continue;
                }

                let other = router.edge_infs[edge].other_vert(best_node_inf);
                node = ANode::new(other, timestamp);
                timestamp += 1;

                // Set the index to the previous ANode that we reached
                // this ANode via.
                node.prev_node = Some(bn);

                let node_inf = other;
                let node_id = router.vert_infs[node_inf].id;

                // Don't bother looking at the segment we just arrived along.
                if prev_inf.is_some() && (prev_inf == Some(node_inf)) {
                    continue;
                }
                if node_id.is_connection_pin() && !node_id.is_conn_checkpoint() {
                    let src_dummy = (Some(best_node_inf) == line_src)
                        && router.vert_infs[line_src.unwrap()].id.is_dummy_pin_helper();
                    let dst_dummy = !src_dummy && {
                        let dst = line_dst.unwrap();
                        VertInf::has_neighbour(router, node_inf, dst, is_orthogonal).is_some()
                            && router.vert_infs[dst].id.is_dummy_pin_helper()
                    };
                    if !src_dummy && !dst_dummy {
                        // Don't check connection pins if they don't have the
                        // target vertex as a direct neighbour, or are directly
                        // leaving the source vertex.
                        continue;
                    }
                } else if node_id.is_conn_pt() {
                    if node_inf != tar {
                        // Don't check connector endpoints vertices unless they
                        // are the target endpoint.
                        continue;
                    }
                }

                if is_orthogonal && !router.edge_infs[edge].is_dummy_connection(&router.vert_infs) {
                    // Orthogonal routing optimisation.
                    // Skip the edges that don't lead to shape edges, or the
                    // connection point we are looking for.  Though allow them
                    // if we haven't yet turned from the source point, since it
                    // may be a free-floating endpoint with directional visibility.
                    // Also, don't check if the previous point was a dummy for a
                    // connection pin and this happens to be placed diagonally
                    // from here, i.e., when both of notInline{X,Y} are true.
                    let best_pt = router.vert_infs[best_node_inf].point;
                    let next_pt = router.vert_infs[node_inf].point;
                    let best_flags = router.vert_infs[best_node_inf].orthog_vis_prop_flags;

                    let not_inline_x = prev_inf.is_some_and(|p| router.vert_infs[p].point.x != best_pt.x);
                    let not_inline_y = prev_inf.is_some_and(|p| router.vert_infs[p].point.y != best_pt.y);
                    if (best_pt.x == next_pt.x) && not_inline_x && !not_inline_y && (best_pt[YDIM] != src_point[YDIM])
                    {
                        if next_pt.y < best_pt.y {
                            if (best_flags & YL_EDGE) == 0 && !point_aligned_with_one_of(&best_pt, &end_points, XDIM) {
                                continue;
                            }
                        } else if next_pt.y > best_pt.y {
                            if (best_flags & YH_EDGE) == 0 && !point_aligned_with_one_of(&best_pt, &end_points, XDIM) {
                                continue;
                            }
                        }
                    }
                    if (best_pt.y == next_pt.y) && not_inline_y && !not_inline_x && (best_pt[XDIM] != src_point[XDIM])
                    {
                        if next_pt.x < best_pt.x {
                            if (best_flags & XL_EDGE) == 0 && !point_aligned_with_one_of(&best_pt, &end_points, YDIM) {
                                continue;
                            }
                        } else if next_pt.x > best_pt.x {
                            if (best_flags & XH_EDGE) == 0 && !point_aligned_with_one_of(&best_pt, &end_points, YDIM) {
                                continue;
                            }
                        }
                    }
                }

                let edge_dist = router.edge_infs[edge].get_dist();

                if edge_dist == 0.0 {
                    continue;
                }

                if !is_orthogonal
                    && (!router.rubber_band_routing || (start == src))
                    && !validate_bend_point(router, prev_inf, best_node_inf, Some(node_inf))
                {
                    // The bendpoint is not valid, i.e., is a zigzag corner, so...
                    continue;
                    // For RubberBand routing we want to allow these routes and
                    // unwind them later, otherwise instead or unwinding, paths
                    // can go the *really* long way round.
                }

                // Figure out if we are at one of the cost targets.
                let mut at_cost_target = false;
                for i in 0..self.m_cost_targets.len() {
                    if best_node_inf == self.m_cost_targets[i] {
                        at_cost_target = true;
                        break;
                    }
                }

                if at_cost_target && (node_id.is_connection_pin() || (node_inf == tar)) {
                    // This is a point on the side of an obstacle that connects
                    // to the target or a connection pin.  It should have no
                    // further cost and the heuristic should be zero.
                    node.g = bn_node.g;
                    node.h = 0.0;
                } else {
                    if node_inf == tar {
                        // We've reached the target.  The heuristic should be zero.
                        node.h = 0.0;
                    } else {
                        // Otherwise, calculate the heuristic value.
                        let bp = router.vert_infs[best_node_inf].point;
                        let np = router.vert_infs[node_inf].point;
                        node.h = self.estimated_cost(router, line_ref, Some(&bp), &np);
                    }

                    if node_id.is_dummy_pin_helper() {
                        // This is connecting to a connection pin helper vertex.
                        // There should be no additional cost for this step.
                        node.g = bn_node.g;
                    } else {
                        // Otherwise, calculate the cost of this step.
                        node.g = bn_node.g
                            + cost(
                                router,
                                &self.m_available_nodes,
                                line_ref,
                                edge_dist,
                                best_node_inf,
                                node_inf,
                                bn_node.prev_node,
                            );
                    }
                }

                // The A* formula
                node.f = node.g + node.h;

                b_node_found = false;

                // Check to see if already on PENDING
                let pending_list = router.vert_infs[node_inf].a_star_pending_nodes.clone();
                for curr_ind in pending_list {
                    ati = self.m_available_nodes[curr_ind.0 as usize];
                    // The (node.prevNode == ati.prevNode) is redundant, but may
                    // save checking the mosre costly prevNode->inf test if the
                    // Nodes are the same.
                    if (node.inf == ati.inf)
                        && ((node.prev_node == ati.prev_node)
                            || (inf_of(&self.m_available_nodes, node.prev_node.unwrap())
                                == inf_of(
                                    &self.m_available_nodes,
                                    ati.prev_node.expect("pending ANode without prevNode"),
                                )))
                    {
                        // If already on PENDING
                        if node.g < ati.g {
                            // Replace the existing node in PENDING
                            self.m_available_nodes[curr_ind.0 as usize] = node;
                            let nodes = &self.m_available_nodes;
                            make_heap(&mut pending, &mut |a: &ANodeIdx, b: &ANodeIdx| {
                                a_node_cmp(&nodes[a.0 as usize], &nodes[b.0 as usize])
                            });
                        }
                        b_node_found = true;
                        break;
                    }
                }
                if !b_node_found {
                    // If Node NOT found on PENDING
                    // Check to see if it is already in the Done set for this
                    // vertex.
                    for &curr_ind in router.vert_infs[node_inf].a_star_done_nodes.iter() {
                        ati = self.m_available_nodes[curr_ind.0 as usize];
                        // The (node.prevNode == ati.prevNode) is redundant, but may
                        // save checking the mosre costly prevNode->inf test if the
                        // Nodes are the same.
                        if (node.inf == ati.inf)
                            && ati.prev_node.is_some()
                            && ((node.prev_node == ati.prev_node)
                                || (inf_of(&self.m_available_nodes, node.prev_node.unwrap())
                                    == inf_of(&self.m_available_nodes, ati.prev_node.unwrap())))
                        {
                            //COLA_ASSERT(node.g >= (ati.g - 10e-10));
                            // This node is already in the Done set and the
                            // current node also has a higher g-value, so we
                            // don't need to consider this node.
                            b_node_found = true;
                            break;
                        }
                    }
                }

                if !b_node_found {
                    // If Node NOT in either Pending or Done.
                    // Push NewNode onto PENDING
                    let new_node = self.new_a_node(router, &node, true);
                    pending.push(new_node);
                    // Push NewNode onto heap
                    let nodes = &self.m_available_nodes;
                    push_heap(&mut pending, &mut |a: &ANodeIdx, b: &ANodeIdx| {
                        a_node_cmp(&nodes[a.0 as usize], &nodes[b.0 as usize])
                    });
                }
            }
        }
        let _ = explored_count;

        // Cleanup lists used to store Done and Pending sets for each vertex.
        let end_vert = router.vertices.end();
        let mut k = router.vertices.conns_begin();
        while k != end_vert {
            let kv = k.unwrap();
            let v = &mut router.vert_infs[kv];
            v.a_star_done_nodes.clear();
            v.a_star_pending_nodes.clear();
            k = v.lst_next;
        }
    }
}

// upstream: makepath.h class AStarPath
#[derive(Debug)]
pub struct AStarPath {
    m_private: AStarPathPrivate,
}

impl AStarPath {
    // upstream: makepath.cpp AStarPath::AStarPath()
    pub fn new() -> AStarPath {
        AStarPath {
            m_private: AStarPathPrivate::default(),
        }
    }

    /// Routes `line_ref` from `src` to `tar` (`start` is the rubber-band
    /// restart vertex, may be None), leaving the result in the vertices'
    /// `path_next` links.
    // upstream: makepath.cpp AStarPath::search()
    pub fn search(
        &mut self,
        router: &mut Router,
        line_ref: ConnIdx,
        src: VertIdx,
        tar: VertIdx,
        start: Option<VertIdx>,
    ) {
        self.m_private.search(router, line_ref, src, tar, start);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The heap ports must agree with a straightforward reference for a
    /// strict ordering (heap property + pop order).
    #[test]
    fn heap_ops_pop_in_order() {
        let mut v: Vec<i32> = vec![5, 3, 9, 1, 7, 2, 8, 6, 4, 0];
        let mut less = |a: &i32, b: &i32| a < b;
        make_heap(&mut v, &mut less);
        v.push(11);
        push_heap(&mut v, &mut less);
        let mut out = Vec::new();
        while !v.is_empty() {
            pop_heap(&mut v, &mut less);
            out.push(v.pop().unwrap());
        }
        assert_eq!(out, vec![11, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]);
    }

    #[test]
    fn heap_layout_matches_libstdcxx() {
        // std::make_heap on {1..=10} with operator< yields this layout in
        // libstdc++.
        let mut v: Vec<i32> = (1..=10).collect();
        make_heap(&mut v, &mut |a: &i32, b: &i32| a < b);
        assert_eq!(v, vec![10, 9, 7, 8, 5, 6, 3, 1, 4, 2]);
    }

    #[test]
    fn bends_examples() {
        let o = Point::new(0.0, 0.0);
        let d = Point::new(10.0, 0.0);
        assert_eq!(bends(&o, COST_DIRECTION_E, &d, COST_DIRECTION_E), 0);
        assert_eq!(bends(&o, COST_DIRECTION_W, &d, COST_DIRECTION_E), 4);
    }
}
