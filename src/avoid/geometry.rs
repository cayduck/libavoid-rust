//! Faithful port of upstream `libavoid/geometry.{h,cpp}`.
//!
//! All functions keep the upstream expression order so results are
//! bit-identical with the C++ (verified by `tests/geometry_oracle.rs`
//! against `tests/golden/geometry_oracle.txt`).  Output pointer arguments
//! (`double *x, double *y`) become `&mut f64`.

#![allow(dead_code)]
// Keep upstream statement shapes (e.g. `x = x - y`, `new` returning a handle).
#![allow(
    clippy::assign_op_pattern,
    clippy::field_reassign_with_default,
    clippy::new_ret_no_self,
    clippy::while_let_loop
)]

use crate::avoid::geomtypes::{Point, Polygon, PolygonInterface};

// upstream: geometry.h DONT_INTERSECT / DO_INTERSECT / PARALLEL
pub const DONT_INTERSECT: i32 = 0;
pub const DO_INTERSECT: i32 = 1;
pub const PARALLEL: i32 = 3;

/// Direction from vector.  Looks at the position of point c from the
/// directed segment ab and returns 1 (counterclockwise), 0 (collinear) or
/// -1 (clockwise).  Upstream default for `maybe_zero` is 0.0.
// upstream: geometry.h vecDir() (static inline)
#[inline]
pub fn vec_dir(a: &Point, b: &Point, c: &Point, maybe_zero: f64) -> i32 {
    debug_assert!(maybe_zero >= 0.0);

    let area2 = ((b.x - a.x) * (c.y - a.y)) - ((c.x - a.x) * (b.y - a.y));

    if area2 < (-maybe_zero) {
        return -1;
    } else if area2 > maybe_zero {
        return 1;
    }
    0
}

/// Finds the projection point of (a,b) onto (a,c).
// upstream: geometry.h projection() (static inline)
#[inline]
pub fn projection(a: &Point, b: &Point, c: &Point) -> Point {
    let ux = c.x - a.x;
    let uy = c.y - a.y;
    let vx = b.x - a.x;
    let vy = b.y - a.y;
    let mut scalar_proj = ux * vx + uy * vy;
    scalar_proj /= ux * ux + uy * uy;
    let mut p = Point::default();
    p.x = scalar_proj * ux + a.x;
    p.y = scalar_proj * uy + a.y;
    p
}

/// Returns true iff the point c lies on the closed segment ab.  To be used
/// when the points are known to be collinear.
// upstream: geometry.cpp inBetween()
pub fn in_between(a: &Point, b: &Point, c: &Point) -> bool {
    let epsilon = f64::EPSILON;

    // We only call this when we know the points are collinear,
    // otherwise we should be checking this here.
    debug_assert!(vec_dir(a, b, c, epsilon) == 0);

    if (a.x - b.x).abs() > epsilon {
        // not vertical
        ((a.x < c.x) && (c.x < b.x)) || ((b.x < c.x) && (c.x < a.x))
    } else {
        ((a.y < c.y) && (c.y < b.y)) || ((b.y < c.y) && (c.y < a.y))
    }
}

/// Returns true iff the three points are colinear.  Upstream default
/// tolerance is 0.0.
// upstream: geometry.cpp colinear()
pub fn colinear(a: &Point, b: &Point, c: &Point, tolerance: f64) -> bool {
    // Do this a bit more optimally for orthogonal AB line segments.
    if a == b {
        return true;
    } else if a.x == b.x {
        return a.x == c.x;
    } else if a.y == b.y {
        return a.y == c.y;
    }

    // Or use the general case.
    vec_dir(a, b, c, tolerance) == 0
}

/// Returns true iff the point c lies on the closed segment ab.  Upstream
/// default tolerance is 0.0.
// upstream: geometry.cpp pointOnLine()
pub fn point_on_line(a: &Point, b: &Point, c: &Point, tolerance: f64) -> bool {
    // Do this a bit more optimally for orthogonal AB line segments.
    if a.x == b.x {
        return (a.x == c.x) && (((a.y < c.y) && (c.y < b.y)) || ((b.y < c.y) && (c.y < a.y)));
    } else if a.y == b.y {
        return (a.y == c.y) && (((a.x < c.x) && (c.x < b.x)) || ((b.x < c.x) && (c.x < a.x)));
    }

    // Or use the general case.
    (vec_dir(a, b, c, tolerance) == 0) && in_between(a, b, c)
}

/// Returns true if the segment cd intersects the segment ab, blocking
/// visibility.
// upstream: geometry.cpp segmentIntersect()
pub fn segment_intersect(a: &Point, b: &Point, c: &Point, d: &Point) -> bool {
    let ab_c = vec_dir(a, b, c, 0.0);
    if ab_c == 0 {
        return false;
    }

    let ab_d = vec_dir(a, b, d, 0.0);
    if ab_d == 0 {
        return false;
    }

    // It's ok for either of the points a or b to be on the line cd,
    // so we don't have to check the other two cases.

    let cd_a = vec_dir(c, d, a, 0.0);
    let cd_b = vec_dir(c, d, b, 0.0);

    // Is an intersection if a and b are on opposite sides of cd,
    // and c and d are on opposite sides of the line ab.
    ((ab_c * ab_d) < 0) && ((cd_a * cd_b) < 0)
}

/// Returns true if the segment e1-e2 intersects the shape boundary segment
/// s1-s2, blocking visibility.
// upstream: geometry.cpp segmentShapeIntersect()
pub fn segment_shape_intersect(
    e1: &Point,
    e2: &Point,
    s1: &Point,
    s2: &Point,
    seen_intersection_at_endpoint: &mut bool,
) -> bool {
    if segment_intersect(e1, e2, s1, s2) {
        // Basic intersection of segments.
        return true;
    } else if (((s2 == e1) || point_on_line(s1, s2, e1, 0.0)) && (vec_dir(s1, s2, e2, 0.0) != 0))
        || (((s2 == e2) || point_on_line(s1, s2, e2, 0.0)) && (vec_dir(s1, s2, e1, 0.0) != 0))
    {
        // Segments intersect at the endpoint of one of the segments.  We
        // allow this once, but the second one blocks visibility.  Otherwise
        // shapes butted up against each other could have visibility through
        // shapes.
        if *seen_intersection_at_endpoint {
            return true;
        }
        *seen_intersection_at_endpoint = true;
    }
    false
}

/// Returns true iff the point p in a valid region that can contain shortest
/// paths.  a0, a1, a2 are ordered vertices of a shape.
// upstream: geometry.cpp inValidRegion()
pub fn in_valid_region(ignore_regions: bool, a0: &Point, a1: &Point, a2: &Point, b: &Point) -> bool {
    // r is a0--a1
    // s is a1--a2

    let r_side = vec_dir(b, a0, a1, 0.0);
    let s_side = vec_dir(b, a1, a2, 0.0);

    let r_out_on = r_side <= 0;
    let s_out_on = s_side <= 0;

    let r_out = r_side < 0;
    let s_out = s_side < 0;

    if vec_dir(a0, a1, a2, 0.0) > 0 {
        // Convex at a1
        if ignore_regions {
            return (r_out_on && !s_out) || (!r_out && s_out_on);
        }
        r_out_on || s_out_on
    } else {
        // Concave at a1
        if ignore_regions {
            false
        } else {
            r_out_on && s_out_on
        }
    }
}

/// Gives the side of a corner that a point lies on: 1 anticlockwise,
/// -1 clockwise.
// upstream: geometry.cpp cornerSide()
pub fn corner_side(c1: &Point, c2: &Point, c3: &Point, p: &Point) -> i32 {
    let s123 = vec_dir(c1, c2, c3, 0.0);
    let s12p = vec_dir(c1, c2, p, 0.0);
    let s23p = vec_dir(c2, c3, p, 0.0);

    if s123 == 1 {
        if (s12p >= 0) && (s23p >= 0) {
            return 1;
        }
        return -1;
    } else if s123 == -1 {
        if (s12p <= 0) && (s23p <= 0) {
            return -1;
        }
        return 1;
    }

    // c1-c2-c3 are collinear, so just return vecDir from c1-c2
    s12p
}

/// Returns the Euclidean distance between points a and b.
// upstream: geometry.cpp euclideanDist()
#[inline]
pub fn euclidean_dist(a: &Point, b: &Point) -> f64 {
    let xdiff = a.x - b.x;
    let ydiff = a.y - b.y;

    ((xdiff * xdiff) + (ydiff * ydiff)).sqrt()
}

/// Returns the Manhattan distance between points a and b.
// upstream: geometry.cpp manhattanDist()
#[inline]
pub fn manhattan_dist(a: &Point, b: &Point) -> f64 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
}

/// Returns the Euclidean distance between points a and b.
// upstream: geometry.cpp dist()
#[inline]
pub fn dist(a: &Point, b: &Point) -> f64 {
    let xdiff = a.x - b.x;
    let ydiff = a.y - b.y;

    ((xdiff * xdiff) + (ydiff * ydiff)).sqrt()
}

/// Returns the total length of all line segments in the polygon.
// upstream: geometry.cpp totalLength()
pub fn total_length(poly: &Polygon) -> f64 {
    let mut l = 0.0;
    for i in 1..poly.size() {
        l += dist(&poly.ps[i - 1], &poly.ps[i]);
    }
    l
}

/// Uses the dot-product rule to find the angle (radians) between ab and bc.
// upstream: geometry.cpp angle()
pub fn angle(a: &Point, b: &Point, c: &Point) -> f64 {
    let ux = b.x - a.x;
    let uy = b.y - a.y;
    let vx = c.x - b.x;
    let vy = c.y - b.y;
    let lu = (ux * ux + uy * uy).sqrt();
    let lv = (vx * vx + vy * vy).sqrt();
    let udotv = ux * vx + uy * vy;
    let costheta = udotv / (lu * lv);
    costheta.acos()
}

/// Returns true iff the point q is inside (or on the edge of) the polygon.
/// Fast version that only works for convex shapes.  Upstream default for
/// `count_border` is true.
// upstream: geometry.cpp inPoly()
pub fn in_poly(poly: &Polygon, q: &Point, count_border: bool) -> bool {
    let n = poly.size();
    let p = &poly.ps;
    let mut on_border = false;
    for i in 0..n {
        // point index; i1 = i-1 mod n
        let prev = (i + n - 1) % n;
        let dir = vec_dir(&p[prev], &p[i], q, 0.0);
        if dir == -1 {
            // Point is outside
            return false;
        }
        // Record if point was on a boundary.
        on_border |= dir == 0;
    }
    if !count_border && on_border {
        return false;
    }
    true
}

/// Returns true iff the point q is inside (or on the edge of) the polygon.
// upstream: geometry.cpp inPolyGen()
pub fn in_poly_gen<P: PolygonInterface + ?Sized>(argpoly: &P, q: &Point) -> bool {
    // Numbers of right and left edge/ray crossings.
    let mut rcross: i32 = 0;
    let mut lcross: i32 = 0;

    // Copy the argument polygon
    let mut poly = Polygon::from_interface(argpoly);
    let p = &mut poly.ps;
    let n = p.len();

    // Shift so that q is the origin. This is done for pedagogical clarity.
    for pt in p.iter_mut() {
        pt.x = pt.x - q.x;
        pt.y = pt.y - q.y;
    }

    // For each edge e=(i-1,i), see if crosses ray.
    for i in 0..n {
        // First see if q=(0,0) is a vertex.
        if (p[i].x == 0.0) && (p[i].y == 0.0) {
            // We count a vertex as inside.
            return true;
        }

        // point index; i1 = i-1 mod n
        let i1 = (i + n - 1) % n;

        // if e "straddles" the x-axis...
        if (p[i].y > 0.0) != (p[i1].y > 0.0) {
            // e straddles ray, so compute intersection with ray.
            let x = (p[i].x * p[i1].y - p[i1].x * p[i].y) / (p[i1].y - p[i].y);

            // crosses ray if strictly positive intersection.
            if x > 0.0 {
                rcross += 1;
            }
        }

        // if e straddles the x-axis when reversed...
        if (p[i].y < 0.0) != (p[i1].y < 0.0) {
            // e straddles ray, so compute intersection with ray.
            let x = (p[i].x * p[i1].y - p[i1].x * p[i].y) / (p[i1].y - p[i].y);

            // crosses ray if strictly positive intersection.
            if x < 0.0 {
                lcross += 1;
            }
        }
    }

    // q on the edge if left and right cross are not the same parity.
    if (rcross % 2) != (lcross % 2) {
        // We count the edge as inside.
        return true;
    }

    // Inside iff an odd number of crossings.
    if (rcross % 2) == 1 {
        return true;
    }

    // Outside.
    false
}

/// Line Segment Intersection, original code by Franklin Antonio.
// upstream: geometry.cpp segmentIntersectPoint()
pub fn segment_intersect_point(
    a1: &Point,
    a2: &Point,
    b1: &Point,
    b2: &Point,
    x: &mut f64,
    y: &mut f64,
) -> i32 {
    let x1lo;
    let x1hi;
    let y1lo;
    let y1hi;

    let ax = a2.x - a1.x;
    let bx = b1.x - b2.x;

    // X bound box test:
    if ax < 0.0 {
        x1lo = a2.x;
        x1hi = a1.x;
    } else {
        x1hi = a2.x;
        x1lo = a1.x;
    }
    if bx > 0.0 {
        if x1hi < b2.x || b1.x < x1lo {
            return DONT_INTERSECT;
        }
    } else if x1hi < b1.x || b2.x < x1lo {
        return DONT_INTERSECT;
    }

    let ay = a2.y - a1.y;
    let by = b1.y - b2.y;

    // Y bound box test:
    if ay < 0.0 {
        y1lo = a2.y;
        y1hi = a1.y;
    } else {
        y1hi = a2.y;
        y1lo = a1.y;
    }
    if by > 0.0 {
        if y1hi < b2.y || b1.y < y1lo {
            return DONT_INTERSECT;
        }
    } else if y1hi < b1.y || b2.y < y1lo {
        return DONT_INTERSECT;
    }

    let cx = a1.x - b1.x;
    let cy = a1.y - b1.y;
    // alpha numerator:
    let d = by * cx - bx * cy;
    // Both denominator:
    let f = ay * bx - ax * by;
    // alpha tests:
    if f > 0.0 {
        if d < 0.0 || d > f {
            return DONT_INTERSECT;
        }
    } else if d > 0.0 || d < f {
        return DONT_INTERSECT;
    }

    // beta numerator:
    let e = ax * cy - ay * cx;
    // beta tests:
    if f > 0.0 {
        if e < 0.0 || e > f {
            return DONT_INTERSECT;
        }
    } else if e > 0.0 || e < f {
        return DONT_INTERSECT;
    }

    // compute intersection coordinates:

    if f == 0.0 {
        return PARALLEL;
    }

    // Numerator:
    let mut num = d * ax;
    // Intersection X:
    *x = a1.x + (num) / f;

    num = d * ay;
    // Intersection Y:
    *y = a1.y + (num) / f;

    DO_INTERSECT
}

/// Line (ray) intersection, original code by Franklin Antonio.
// upstream: geometry.cpp rayIntersectPoint()
pub fn ray_intersect_point(
    a1: &Point,
    a2: &Point,
    b1: &Point,
    b2: &Point,
    x: &mut f64,
    y: &mut f64,
) -> i32 {
    let ay = a2.y - a1.y;
    let by = b1.y - b2.y;
    let ax = a2.x - a1.x;
    let bx = b1.x - b2.x;

    let cx = a1.x - b1.x;
    let cy = a1.y - b1.y;
    // alpha numerator:
    let d = by * cx - bx * cy;
    // Both denominator:
    let f = ay * bx - ax * by;

    // compute intersection coordinates:

    if f == 0.0 {
        return PARALLEL;
    }

    // Numerator:
    let mut num = d * ax;
    // Intersection X:
    *x = a1.x + (num) / f;

    num = d * ay;
    // Intersection Y:
    *y = a1.y + (num) / f;

    DO_INTERSECT
}

/// Returns the rotationalAngle, between 0 and 360, of this point from (0,0).
// upstream: geometry.cpp rotationalAngle()
pub fn rotational_angle(p: &Point) -> f64 {
    if p.y == 0.0 {
        return if p.x < 0.0 { 180.0 } else { 0.0 };
    } else if p.x == 0.0 {
        return if p.y < 0.0 { 270.0 } else { 90.0 };
    }

    let mut ang = (p.y / p.x).atan();
    ang = (ang * 180.0) / std::f64::consts::PI;

    if p.x < 0.0 {
        ang += 180.0;
    } else if p.y < 0.0 {
        ang += 360.0;
    }
    debug_assert!(ang >= 0.0);
    debug_assert!(ang <= 360.0);

    ang
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn vec_dir_signs() {
        assert_eq!(vec_dir(&p(0.0, 0.0), &p(1.0, 0.0), &p(1.0, 1.0), 0.0), 1);
        assert_eq!(vec_dir(&p(0.0, 0.0), &p(1.0, 0.0), &p(1.0, -1.0), 0.0), -1);
        assert_eq!(vec_dir(&p(0.0, 0.0), &p(1.0, 0.0), &p(2.0, 0.0), 0.0), 0);
    }

    #[test]
    fn segment_intersections() {
        assert!(segment_intersect(&p(0.0, 0.0), &p(2.0, 2.0), &p(0.0, 2.0), &p(2.0, 0.0)));
        // Touching at an endpoint is not an intersection.
        assert!(!segment_intersect(&p(0.0, 0.0), &p(2.0, 2.0), &p(1.0, 1.0), &p(2.0, 0.0)));
        let (mut x, mut y) = (0.0, 0.0);
        let r = segment_intersect_point(
            &p(0.0, 0.0),
            &p(2.0, 2.0),
            &p(0.0, 2.0),
            &p(2.0, 0.0),
            &mut x,
            &mut y,
        );
        assert_eq!(r, DO_INTERSECT);
        assert_eq!((x, y), (1.0, 1.0));
    }

    #[test]
    fn point_in_polygons() {
        let sq = crate::avoid::geomtypes::Rectangle::new(&p(0.0, 0.0), &p(10.0, 10.0));
        assert!(in_poly(&sq, &p(5.0, 5.0), true));
        assert!(in_poly(&sq, &p(10.0, 5.0), true));
        assert!(!in_poly(&sq, &p(10.0, 5.0), false));
        assert!(in_poly_gen(&sq, &p(5.0, 5.0)));
        assert!(in_poly_gen(&sq, &p(0.0, 0.0)));
        assert!(!in_poly_gen(&sq, &p(15.0, 5.0)));
    }

    #[test]
    fn rotational_angles() {
        assert_eq!(rotational_angle(&p(1.0, 0.0)), 0.0);
        assert_eq!(rotational_angle(&p(-1.0, 0.0)), 180.0);
        assert_eq!(rotational_angle(&p(0.0, -3.0)), 270.0);
        assert!((rotational_angle(&p(1.0, 1.0)) - 45.0).abs() < 1e-12);
    }
}
