//! Faithful port of upstream `libavoid/visibility.{h,cpp}`: poly-line
//! visibility graph construction, both the naive O(n^2) method and Lee's
//! rotational sweep (`UseLeesAlgorithm`).
//!
//! The Visibility Sweep technique is based upon the method described
//! in Section 5.2 of:
//!     Lee, D.-T. (1978). Proximity and reachability in the plane.,
//!     PhD thesis, Department of Electrical Engineering,
//!     University of Illinois, Urbana, IL.
//!
//! Container notes:
//! * `VertSet` (`std::set<PointPair>`) is a vector kept sorted by the
//!   upstream `PointPair::operator<`; inserting an element equivalent to an
//!   existing one is a no-op, exactly like `std::set::insert`.
//! * `SweepEdgeList` (`std::list<EdgePair>`) is a vector in list order;
//!   `push_front`/`push_back`/`remove` keep list semantics and `sort()` uses
//!   the step-by-step port of libstdc++'s `std::list::sort`
//!   ([`libstdcxx_list_sort`]).
//! * `EdgePair` / `PointPair` hold `VertInf *` upstream; here they keep the
//!   handle plus copies of the (immutable during a sweep) vertex id and
//!   point they read.

#![allow(dead_code)]
#![allow(clippy::collapsible_if, clippy::collapsible_else_if)]

use std::collections::BTreeSet;

use crate::avoid::arena::{ObstacleIdx, VertIdx};
use crate::avoid::geometry::{
    euclidean_dist, in_valid_region, point_on_line, ray_intersect_point, rotational_angle,
    segment_intersect, vec_dir, DO_INTERSECT,
};
use crate::avoid::geomtypes::{cpp_min, Point};
use crate::avoid::graph::EdgeInf;
use crate::avoid::obstacle::Obstacle;
use crate::avoid::router::Router;
use crate::avoid::vertices::{libstdcxx_list_sort, ShapeSet, VertID, VertInf, DUMMY_ORTHOG_ID};

// upstream: visibility.cpp Obstacle::computeVisibilityNaive()
pub fn compute_visibility_naive(router: &mut Router, obstacle: ObstacleIdx) {
    if !router.invisibility_grph {
        // Clear shape from graph.
        Obstacle::remove_from_graph(router, obstacle);
    }

    let shape_begin = router.obstacles[obstacle].first_vert();
    let shape_end = router.vert_infs[router.obstacles[obstacle].last_vert().unwrap()].lst_next;

    let points_begin = router.vertices.conns_begin();
    let mut curr = shape_begin;
    while curr != shape_end {
        let cv = curr.unwrap();
        let known_new = true;

        // First Half:
        let mut j = points_begin;
        while j != curr {
            let jv = j.unwrap();
            let next = router.vert_infs[jv].lst_next;
            if router.vert_infs[jv].id == DUMMY_ORTHOG_ID {
                // Don't include orthogonal dummy vertices.
                j = next;
                continue;
            }
            EdgeInf::check_edge_visibility(router, cv, jv, known_new);
            j = next;
        }

        // Second Half:
        let points_end = router.vertices.end();
        let mut k = shape_end;
        while k != points_end {
            let kv = k.unwrap();
            let next = router.vert_infs[kv].lst_next;
            if router.vert_infs[kv].id == DUMMY_ORTHOG_ID {
                // Don't include orthogonal dummy vertices.
                k = next;
                continue;
            }
            EdgeInf::check_edge_visibility(router, cv, kv, known_new);
            k = next;
        }
        curr = router.vert_infs[cv].lst_next;
    }
}

// upstream: visibility.cpp Obstacle::computeVisibilitySweep()
pub fn compute_visibility_sweep(router: &mut Router, obstacle: ObstacleIdx) {
    if !router.invisibility_grph {
        // Clear shape from graph.
        Obstacle::remove_from_graph(router, obstacle);
    }

    let start_iter = router.obstacles[obstacle].first_vert();
    let end_iter = router.vert_infs[router.obstacles[obstacle].last_vert().unwrap()].lst_next;

    let mut i = start_iter;
    while i != end_iter {
        let iv = i.unwrap();
        vertex_sweep(router, iv);
        i = router.vert_infs[iv].lst_next;
    }
}

/// Upstream default for `gen_contains` is false.
// upstream: visibility.cpp vertexVisibility()
pub fn vertex_visibility(
    router: &mut Router,
    point: VertIdx,
    partner: Option<VertIdx>,
    known_new: bool,
    gen_contains: bool,
) {
    let p_id = router.vert_infs[point].id;

    // Make sure we're only doing ptVis for endpoints.
    debug_assert!(p_id.is_conn_pt());

    if !router.invisibility_grph {
        VertInf::remove_from_graph(router, point, true);
    }

    if gen_contains && p_id.is_conn_pt() {
        router.generate_contains(point);
    }

    if router.use_lees_algorithm {
        vertex_sweep(router, point);
    } else {
        let shapes_end = router.vertices.end();
        let mut k = router.vertices.conns_begin();
        while k != shapes_end {
            let kv = k.unwrap();
            let next = router.vert_infs[kv].lst_next;
            let k_id = router.vert_infs[kv].id;
            if k_id == DUMMY_ORTHOG_ID {
                // Don't include orthogonal dummy vertices.
                k = next;
                continue;
            } else if k_id.is_conn_pt()
                && !k_id.is_connection_pin()
                && !(k_id.is_conn_checkpoint() && k_id.obj_id == p_id.obj_id)
            {
                // Include connection pins, but not connectors.
                // Also include checkpoints with same ID as sweep point.
                k = next;
                continue;
            }
            EdgeInf::check_edge_visibility(router, point, kv, known_new);
            k = next;
        }
        if let Some(partner) = partner {
            EdgeInf::check_edge_visibility(router, point, partner, known_new);
        }
    }
}

//============================================================================
//  SWEEP CODE
//

// upstream: visibility.cpp class PointPair
#[derive(Clone, Copy, Debug)]
struct PointPair {
    v_inf: VertIdx,
    /// Copy of `vInf->id` (read by the comparator).
    v_id: VertID,
    /// Copy of `vInf->point`.
    v_point: Point,
    angle: f64,
    distance: f64,
    center_point: Point,
}

impl PointPair {
    // upstream: visibility.cpp PointPair::PointPair()
    fn new(router: &Router, center_point: &Point, inf: VertIdx) -> PointPair {
        let v = &router.vert_infs[inf];
        let angle = rotational_angle(&(v.point - *center_point));
        let distance = euclidean_dist(center_point, &v.point);
        PointPair {
            v_inf: inf,
            v_id: v.id,
            v_point: v.point,
            angle,
            distance,
            center_point: *center_point,
        }
    }

    // upstream: visibility.cpp PointPair::operator<()
    fn less(&self, rhs: &PointPair) -> bool {
        // Firstly order by angle.
        if self.angle == rhs.angle {
            // If the points are collinear, then order them in increasing
            // distance from the point we are sweeping around.
            if self.distance == rhs.distance {
                // XXX: Add this assertion back if we require that
                //      connector endpoints have unique IDs. For the
                //      moment it is okay for them to have the same ID.
                //COLA_ASSERT(vInf->id != rhs.vInf->id);

                // If comparing two points at the same physical
                // position, then order them by their VertIDs.
                return self.v_id < rhs.v_id;
            }
            return self.distance < rhs.distance;
        }
        self.angle < rhs.angle
    }
}

/// `std::set<PointPair>` ordered by `PointPair::operator<`.
// upstream: visibility.cpp typedef std::set<PointPair > VertSet
#[derive(Default)]
struct VertSet {
    items: Vec<PointPair>,
}

impl VertSet {
    /// `std::set::insert`: no-op if an equivalent element exists.
    fn insert(&mut self, pp: PointPair) {
        // lower_bound: first element not less than pp.
        let pos = self.items.partition_point(|e| e.less(&pp));
        if pos < self.items.len() && !pp.less(&self.items[pos]) {
            // Equivalent element already present.
            return;
        }
        self.items.insert(pos, pp);
    }
}

// upstream: visibility.cpp class EdgePair
#[derive(Clone, Copy, Debug)]
struct EdgePair {
    v_inf1: VertIdx,
    v_inf2: VertIdx,
    /// Copies of `vInf1->id`, `vInf2->id`, `vInf1->point`, `vInf2->point`.
    id1: VertID,
    id2: VertID,
    pt1: Point,
    pt2: Point,
    dist1: f64,
    dist2: f64,
    angle: f64,
    angle_dist: f64,
    center_point: Point,
}

impl EdgePair {
    // upstream: visibility.cpp EdgePair::EdgePair(const PointPair&, VertInf *)
    fn new(router: &Router, p1: &PointPair, v: VertIdx) -> EdgePair {
        let v2 = &router.vert_infs[v];
        EdgePair {
            v_inf1: p1.v_inf,
            v_inf2: v,
            id1: p1.v_id,
            id2: v2.id,
            pt1: p1.v_point,
            pt2: v2.point,
            dist1: p1.distance,
            dist2: euclidean_dist(&v2.point, &p1.center_point),
            angle: p1.angle,
            angle_dist: p1.distance,
            center_point: p1.center_point,
        }
    }

    // upstream: visibility.cpp EdgePair::operator<()
    fn less(&self, rhs: &EdgePair) -> bool {
        debug_assert!(self.angle == rhs.angle);
        if self.angle_dist == rhs.angle_dist {
            return self.dist2 < rhs.dist2;
        }
        self.angle_dist < rhs.angle_dist
    }

    // upstream: visibility.cpp EdgePair::operator==()
    fn equals(&self, rhs: &EdgePair) -> bool {
        if ((self.id1 == rhs.id1) && (self.id2 == rhs.id2))
            || ((self.id1 == rhs.id2) && (self.id2 == rhs.id1))
        {
            return true;
        }
        false
    }

    // upstream: visibility.cpp EdgePair::operator!=()
    fn not_equals(&self, rhs: &EdgePair) -> bool {
        if ((self.id1 == rhs.id1) && (self.id2 == rhs.id2))
            || ((self.id1 == rhs.id2) && (self.id2 == rhs.id1))
        {
            return false;
        }
        true
    }

    // upstream: visibility.cpp EdgePair::setNegativeAngle()
    fn set_negative_angle(&mut self) {
        self.angle = -1.0;
    }

    // upstream: visibility.cpp EdgePair::setCurrAngle()
    fn set_curr_angle(&mut self, p: &PointPair) -> f64 {
        if p.v_point == self.pt1 {
            self.angle_dist = self.dist1;
            self.angle = p.angle;
        } else if p.v_point == self.pt2 {
            self.angle_dist = self.dist2;
            self.angle = p.angle;
        } else if p.angle != self.angle {
            debug_assert!(p.angle > self.angle);
            self.angle = p.angle;
            let mut pp = Point::default();
            let result = ray_intersect_point(
                &self.pt1,
                &self.pt2,
                &self.center_point,
                &p.v_point,
                &mut pp.x,
                &mut pp.y,
            );
            if result != DO_INTERSECT {
                // This can happen with points that appear to have the
                // same angle but at are at slightly different positions
                self.angle_dist = cpp_min(self.dist1, self.dist2);
            } else {
                self.angle_dist = euclidean_dist(&pp, &self.center_point);
            }
        }

        self.angle_dist
    }
}

/// `std::list<EdgePair>` in list order.
// upstream: visibility.cpp typedef std::list<EdgePair> SweepEdgeList
type SweepEdgeList = Vec<EdgePair>;

/// `std::list<EdgePair>::remove(value)`: erases every element equal to it.
fn sweep_edge_list_remove(list: &mut SweepEdgeList, value: &EdgePair) {
    list.retain(|e| !e.equals(value));
}

/// `std::list<EdgePair>::sort()`.
fn sweep_edge_list_sort(list: &mut SweepEdgeList) {
    let items = std::mem::take(list);
    *list = libstdcxx_list_sort(items, &mut |a: &EdgePair, b: &EdgePair| a.less(b));
}

// upstream: visibility.cpp #define AHEAD / BEHIND
const AHEAD: i32 = 1;
const BEHIND: i32 = -1;

// upstream: visibility.cpp class isBoundingShape
struct IsBoundingShape<'a> {
    ss: &'a ShapeSet,
}

impl IsBoundingShape<'_> {
    // upstream: visibility.cpp isBoundingShape::operator()()
    fn call(&self, pp: &PointPair) -> bool {
        if !pp.v_id.is_conn_pt() && self.ss.contains(&pp.v_id.obj_id) {
            return true;
        }
        false
    }
}

// upstream: visibility.cpp sweepVisible() (static)
fn sweep_visible(
    router: &mut Router,
    t: &SweepEdgeList,
    point: &PointPair,
    on_border_ids: &BTreeSet<u32>,
    blocker: &mut i32,
) -> bool {
    if t.is_empty() {
        // No blocking edges.
        return true;
    }

    let mut visible = true;

    let mut closest_it = 0;
    let end = t.len();
    while closest_it != end {
        if (point.v_point == t[closest_it].pt1) || (point.v_point == t[closest_it].pt2) {
            // If the ray intersects just the endpoint of a
            // blocking edge then ignore that edge.
            closest_it += 1;
            continue;
        }
        break;
    }
    if closest_it == end {
        return true;
    }

    if point.v_id.is_conn_pt() {
        // It's a connector endpoint, so we have to ignore
        // edges of containing shapes for determining visibility.
        // (`router->contains[...]` default-inserts, as upstream.)
        let rss = router.contains.entry(point.v_id).or_default();
        while closest_it != end {
            let closest = &t[closest_it];
            if !rss.contains(&closest.id1.obj_id) {
                // This is not a containing edge so do the normal
                // test and then stop.
                if point.distance > closest.angle_dist {
                    visible = false;
                } else if (point.distance == closest.angle_dist)
                    && on_border_ids.contains(&closest.id1.obj_id)
                {
                    // Touching, but centerPoint is on another edge of
                    // shape shape, so count as blocking.
                    visible = false;
                }
                break;
            }
            // This was a containing edge, so consider the next along.
            closest_it += 1;
        }
    } else {
        // Just test to see if this point is closer than the closest
        // edge blocking this ray.
        let closest = &t[closest_it];
        if point.distance > closest.angle_dist {
            visible = false;
        } else if (point.distance == closest.angle_dist) && on_border_ids.contains(&closest.id1.obj_id) {
            // Touching, but centerPoint is on another edge of
            // shape shape, so count as blocking.
            visible = false;
        }
    }

    if !visible {
        // (Upstream dereferences closestIt here; it is never `end` when
        // `visible` is false.)
        *blocker = t[closest_it].id1.obj_id as i32;
    }
    visible
}

// upstream: visibility.cpp vertexSweep() (static)
fn vertex_sweep(router: &mut Router, vert: VertIdx) {
    let p_id = router.vert_infs[vert].id;
    let p_point = router.vert_infs[vert].point;

    let center_inf = vert;
    let center_id = p_id;
    let center_point = p_point;

    // List of shape (and maybe endpt) vertices, except p
    // Sort list, around
    let mut v = VertSet::default();

    // Initialise the vertex list
    // (`router->contains[centerID]` default-inserts, as upstream.)
    let ss: ShapeSet = router.contains.entry(center_id).or_default().clone();
    let begin_vert = router.vertices.conns_begin();
    let end_vert = router.vertices.end();
    let mut inf = begin_vert;
    while inf != end_vert {
        let iv = inf.unwrap();
        let next = router.vert_infs[iv].lst_next;
        let inf_id = router.vert_infs[iv].id;
        if iv == center_inf {
            // Don't include the center point itself.
            inf = next;
            continue;
        } else if inf_id == DUMMY_ORTHOG_ID {
            // Don't include orthogonal dummy vertices.
            inf = next;
            continue;
        }

        if center_id.is_conn_pt() && ss.contains(&inf_id.obj_id) && !inf_id.is_conn_pt() {
            // Don't include edge points of containing shapes.
            inf = next;
            continue;
        }

        if inf_id.is_conn_pt() {
            // Add connector endpoint.
            if center_id.is_conn_pt() {
                if inf_id.is_connection_pin() {
                    v.insert(PointPair::new(router, &center_point, iv));
                } else if center_id.is_connection_pin() {
                    // Connection pins have visibility to everything.
                    v.insert(PointPair::new(router, &center_point, iv));
                } else if inf_id.obj_id == center_id.obj_id {
                    // Center is an endpoint, so only include the other
                    // endpoints or checkpoints from the matching connector.
                    v.insert(PointPair::new(router, &center_point, iv));
                }
            } else {
                // Center is a shape vertex, so add all endpoint vertices.
                v.insert(PointPair::new(router, &center_point, iv));
            }
        } else {
            // Add shape vertex.
            v.insert(PointPair::new(router, &center_point, iv));
        }
        inf = next;
    }
    let mut on_border_ids: BTreeSet<u32> = BTreeSet::new();

    // Add edges to T that intersect the initial ray.
    let mut e: SweepEdgeList = Vec::new();
    let xaxis = Point::new(f64::MAX, router.vert_infs[center_inf].point.y);
    let center_inf_point = router.vert_infs[center_inf].point;
    for t in v.items.iter() {
        let k = t.v_inf;

        debug_assert!(center_inf != k);

        let k_prev = router.vert_infs[k].sh_prev;
        let k_next = router.vert_infs[k].sh_next;
        let k_point = router.vert_infs[k].point;
        let k_id = router.vert_infs[k].id;
        if k_prev.is_some()
            && (k_prev != Some(center_inf))
            && (vec_dir(&center_inf_point, &xaxis, &router.vert_infs[k_prev.unwrap()].point, 0.0) == AHEAD)
        {
            let kp = k_prev.unwrap();
            let kp_point = router.vert_infs[kp].point;
            if segment_intersect(&center_inf_point, &xaxis, &kp_point, &k_point) {
                let int_pair = EdgePair::new(router, t, kp);
                e.push(int_pair);
            }
            if point_on_line(&kp_point, &k_point, &center_inf_point, 0.0) {
                // Record that centerPoint is on an obstacle line.
                on_border_ids.insert(k_id.obj_id);
            }
        } else if k_next.is_some()
            && (k_next != Some(center_inf))
            && (vec_dir(&center_inf_point, &xaxis, &router.vert_infs[k_next.unwrap()].point, 0.0) == AHEAD)
        {
            let kn = k_next.unwrap();
            let kn_point = router.vert_infs[kn].point;
            if segment_intersect(&center_inf_point, &xaxis, &kn_point, &k_point) {
                let int_pair = EdgePair::new(router, t, kn);
                e.push(int_pair);
            }
            if point_on_line(&kn_point, &k_point, &center_inf_point, 0.0) {
                // Record that centerPoint is on an obstacle line.
                on_border_ids.insert(k_id.obj_id);
            }
        }
    }
    for c in e.iter_mut() {
        c.set_negative_angle();
    }

    // Start the actual sweep.
    let _is_bounding = IsBoundingShape { ss: &ss };
    let ignore_regions = router.ignore_regions;
    for t in v.items.iter() {
        let curr_inf = t.v_inf;
        let curr_id = router.vert_infs[curr_inf].id;
        let curr_pt = router.vert_infs[curr_inf].point;

        let curr_dist = t.distance;

        let mut edge = EdgeInf::existing_edge(router, center_inf, curr_inf);
        if edge.is_none() {
            edge = Some(EdgeInf::new(router, center_inf, curr_inf, false));
        }
        let edge = edge.unwrap();

        for c in e.iter_mut() {
            c.set_curr_angle(t);
        }
        sweep_edge_list_sort(&mut e);

        // Check visibility.
        let mut blocker: i32 = 0;
        let curr_visible = sweep_visible(router, &e, t, &on_border_ids, &mut blocker);

        let mut cone1 = true;
        let mut cone2 = true;
        if !center_id.is_conn_pt() {
            let c = &router.vert_infs[center_inf];
            cone1 = in_valid_region(
                ignore_regions,
                &router.vert_infs[c.sh_prev.unwrap()].point,
                &center_point,
                &router.vert_infs[c.sh_next.unwrap()].point,
                &curr_pt,
            );
        }
        if !curr_id.is_conn_pt() {
            let c = &router.vert_infs[curr_inf];
            cone2 = in_valid_region(
                ignore_regions,
                &router.vert_infs[c.sh_prev.unwrap()].point,
                &curr_pt,
                &router.vert_infs[c.sh_next.unwrap()].point,
                &center_point,
            );
        }

        if !cone1 || !cone2 {
            if router.invisibility_grph {
                // Setting invisibility edge...
                EdgeInf::add_blocker(router, edge, 0);
            }
        } else {
            if curr_visible {
                // Setting visibility edge...
                EdgeInf::set_dist(router, edge, curr_dist);
            } else if router.invisibility_grph {
                // Setting invisibility edge...
                EdgeInf::add_blocker(router, edge, blocker);
            }
        }

        if !router.edge_infs[edge].added() && !router.invisibility_grph {
            EdgeInf::delete(router, edge);
        }

        if !curr_id.is_conn_pt() {
            // This is a shape edge
            let curr_sh_prev = router.vert_infs[curr_inf].sh_prev.unwrap();
            let curr_sh_next = router.vert_infs[curr_inf].sh_next.unwrap();

            if curr_sh_prev != center_inf {
                let prev_pt = router.vert_infs[curr_sh_prev].point;
                let prev_dir = vec_dir(&center_point, &curr_pt, &prev_pt, 0.0);
                let prev_pair = EdgePair::new(router, t, curr_sh_prev);

                if prev_dir == BEHIND {
                    sweep_edge_list_remove(&mut e, &prev_pair);
                } else if prev_dir == AHEAD {
                    e.insert(0, prev_pair);
                }
            }

            if curr_sh_next != center_inf {
                let next_pt = router.vert_infs[curr_sh_next].point;
                let next_dir = vec_dir(&center_point, &curr_pt, &next_pt, 0.0);
                let next_pair = EdgePair::new(router, t, curr_sh_next);

                if next_dir == BEHIND {
                    sweep_edge_list_remove(&mut e, &next_pair);
                } else if next_dir == AHEAD {
                    e.insert(0, next_pair);
                }
            }
        }
    }
}
