//! Faithful port of upstream `libavoid/geomtypes.{h,cpp}`.
//!
//! Notes on the translation:
//! * `Point::operator==`/`!=` compare position only (`id`/`vn` ignored), so
//!   `Point` implements `PartialEq` by hand.  `Point::operator<` is exposed
//!   as [`Point::less_than`] (and via the [`PointOrd`] wrapper for use as an
//!   ordered-container key, mirroring `std::set<Point>`).
//! * Upstream's default `Point()` leaves `x`/`y` uninitialised; here they
//!   are `0.0`.
//! * `PolygonInterface` is a trait.  `Polygon` implements it directly.
//!   `ReferencingPolygon` stores `(ObstacleIdx, vn)` references instead of
//!   `const Polygon *`, so reading its points needs a [`PolygonLookup`]
//!   (implemented by `Router` once obstacles exist); use
//!   [`ReferencingPolygon::view`] to get a `PolygonInterface` implementor.
//! * `Rectangle` has no state of its own upstream, so its constructors are
//!   associated functions returning a `Polygon`.
//! * `std::min`/`std::max` are reproduced by [`cpp_min`]/[`cpp_max`]
//!   (argument-order sensitive for NaN / signed zero, unlike `f64::min`).

#![allow(dead_code)]
// Keep upstream statement shapes (e.g. `x = x - y`, `new` returning a handle).
#![allow(
    clippy::assign_op_pattern,
    clippy::field_reassign_with_default,
    clippy::new_ret_no_self,
    clippy::while_let_loop
)]

use std::cmp::Ordering;
use std::ops::{Add, Index, IndexMut, Sub};

use crate::avoid::arena::ObstacleIdx;
use crate::avoid::geometry::vec_dir;

// upstream: geomtypes.h XDIM / YDIM
pub const XDIM: usize = 0;
pub const YDIM: usize = 1;

/// Constant value representing an unassigned vertex number.
// upstream: geomtypes.h kUnassignedVertexNumber
pub const K_UNASSIGNED_VERTEX_NUMBER: u16 = 8;

/// Constant value representing a ShapeConnectionPin.
// upstream: geomtypes.h kShapeConnectionPin
pub const K_SHAPE_CONNECTION_PIN: u16 = 9;

/// `std::min(a, b)` for doubles: returns `a` unless `b < a`.
#[inline]
pub fn cpp_min(a: f64, b: f64) -> f64 {
    if b < a {
        b
    } else {
        a
    }
}

/// `std::max(a, b)` for doubles: returns `a` unless `a < b`.
#[inline]
pub fn cpp_max(a: f64, b: f64) -> f64 {
    if a < b {
        b
    } else {
        a
    }
}

/// The Point class defines a point in the plane.
// upstream: geomtypes.h class Point
#[derive(Clone, Copy, Debug)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    /// The ID associated with this point.
    pub id: u32,
    /// The vertex number associated with this point.
    pub vn: u16,
}

/// A vector, represented by the Point class.
// upstream: geomtypes.h typedef Point Vector
pub type Vector = Point;

impl Default for Point {
    // upstream: geomtypes.cpp Point::Point()
    fn default() -> Self {
        Point {
            x: 0.0,
            y: 0.0,
            id: 0,
            vn: K_UNASSIGNED_VERTEX_NUMBER,
        }
    }
}

impl Point {
    /// Port addition: both coordinates finite (see "Public-API misuse"
    /// in the router.rs module docs: non-finite input is rejected).
    #[inline]
    pub fn is_finite(&self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    // upstream: geomtypes.cpp Point::Point(const double xv, const double yv)
    #[inline]
    pub const fn new(xv: f64, yv: f64) -> Point {
        Point {
            x: xv,
            y: yv,
            id: 0,
            vn: K_UNASSIGNED_VERTEX_NUMBER,
        }
    }

    // upstream: geomtypes.cpp Point::equals()
    #[inline]
    pub fn equals(&self, rhs: &Point, epsilon: f64) -> bool {
        if ((self.x - rhs.x).abs() < epsilon) && ((self.y - rhs.y).abs() < epsilon) {
            return true;
        }
        false
    }

    /// `equals()` with the upstream default epsilon of 0.0001.
    #[inline]
    pub fn equals_default(&self, rhs: &Point) -> bool {
        self.equals(rhs, 0.0001)
    }

    // upstream: geomtypes.cpp Point::operator<()
    #[inline]
    pub fn less_than(&self, rhs: &Point) -> bool {
        if self.x == rhs.x {
            return self.y < rhs.y;
        }
        self.x < rhs.x
    }
}

impl PartialEq for Point {
    // upstream: geomtypes.cpp Point::operator==()
    #[inline]
    fn eq(&self, rhs: &Point) -> bool {
        if (self.x == rhs.x) && (self.y == rhs.y) {
            return true;
        }
        false
    }

    // upstream: geomtypes.cpp Point::operator!=()
    #[allow(clippy::partialeq_ne_impl)]
    #[inline]
    fn ne(&self, rhs: &Point) -> bool {
        if (self.x != rhs.x) || (self.y != rhs.y) {
            return true;
        }
        false
    }
}

impl Index<usize> for Point {
    type Output = f64;
    // upstream: geomtypes.cpp Point::operator[]() const
    #[inline]
    fn index(&self, dimension: usize) -> &f64 {
        debug_assert!((dimension == 0) || (dimension == 1));
        if dimension == 0 {
            &self.x
        } else {
            &self.y
        }
    }
}

impl IndexMut<usize> for Point {
    // upstream: geomtypes.cpp Point::operator[]()
    #[inline]
    fn index_mut(&mut self, dimension: usize) -> &mut f64 {
        debug_assert!((dimension == 0) || (dimension == 1));
        if dimension == 0 {
            &mut self.x
        } else {
            &mut self.y
        }
    }
}

impl Add for Point {
    type Output = Point;
    // upstream: geomtypes.cpp Point::operator+()
    #[inline]
    fn add(self, rhs: Point) -> Point {
        Point::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Point {
    type Output = Point;
    // upstream: geomtypes.cpp Point::operator-()
    #[inline]
    fn sub(self, rhs: Point) -> Point {
        Point::new(self.x - rhs.x, self.y - rhs.y)
    }
}

/// Ordered-container key wrapper using upstream `Point::operator<`
/// (for ports of `std::set<Point>` / `std::map<Point, ..>`).  Two points
/// are equivalent iff neither is `less_than` the other, as in the STL.
#[derive(Clone, Copy, Debug)]
pub struct PointOrd(pub Point);

impl PartialEq for PointOrd {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for PointOrd {}
impl PartialOrd for PointOrd {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for PointOrd {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.0.less_than(&other.0) {
            Ordering::Less
        } else if other.0.less_than(&self.0) {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}

/// A bounding box, represented by the top-left and bottom-right corners.
// upstream: geomtypes.h class Box
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Box {
    pub min: Point,
    pub max: Point,
}

impl Box {
    // upstream: geomtypes.cpp Box::length()
    pub fn length(&self, dimension: usize) -> f64 {
        if dimension == 0 {
            return self.max.x - self.min.x;
        }
        self.max.y - self.min.y
    }

    // upstream: geomtypes.cpp Box::width()
    pub fn width(&self) -> f64 {
        self.max.x - self.min.x
    }

    // upstream: geomtypes.cpp Box::height()
    pub fn height(&self) -> f64 {
        self.max.y - self.min.y
    }
}

/// A line between two points.
// upstream: geomtypes.h class Edge
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Edge {
    pub a: Point,
    pub b: Point,
}

/// A common interface used by the Polygon classes.
// upstream: geomtypes.h class PolygonInterface
pub trait PolygonInterface {
    /// Resets this to the empty polygon.
    fn clear(&mut self);
    /// Returns true if this polygon is empty.
    fn empty(&self) -> bool;
    /// Returns the number of points in this polygon.
    fn size(&self) -> usize;
    /// Returns the ID value associated with this polygon.
    fn id(&self) -> i32;
    /// Returns a specific point in the polygon.
    fn at(&self, index: usize) -> Point;

    // upstream: geomtypes.cpp PolygonInterface::boundingRectPolygon()
    fn bounding_rect_polygon(&self) -> Polygon {
        let bounding_box = self.offset_bounding_box(0.0);
        Rectangle::new(&bounding_box.min, &bounding_box.max)
    }

    // upstream: geomtypes.cpp PolygonInterface::offsetBoundingBox()
    fn offset_bounding_box(&self, offset: f64) -> Box {
        let mut b_box = Box::default();
        b_box.min.x = f64::MAX;
        b_box.min.y = f64::MAX;
        b_box.max.x = -f64::MAX;
        b_box.max.y = -f64::MAX;

        for i in 0..self.size() {
            b_box.min.x = cpp_min(b_box.min.x, self.at(i).x);
            b_box.min.y = cpp_min(b_box.min.y, self.at(i).y);
            b_box.max.x = cpp_max(b_box.max.x, self.at(i).x);
            b_box.max.y = cpp_max(b_box.max.y, self.at(i).y);
        }

        // Add buffer space.
        b_box.min.x -= offset;
        b_box.min.y -= offset;
        b_box.max.x += offset;
        b_box.max.y += offset;

        b_box
    }

    // upstream: geomtypes.cpp PolygonInterface::offsetPolygon()
    fn offset_polygon(&self, offset: f64) -> Polygon {
        let mut new_poly = Polygon::new();
        new_poly._id = self.id();
        if offset == 0.0 {
            for i in 0..self.size() {
                new_poly.ps.push(self.at(i));
            }
            return new_poly;
        }

        let num_of_edges = self.size();
        let mut normals: Vec<Vector> = vec![Point::default(); num_of_edges];
        for (i, normal) in normals.iter_mut().enumerate() {
            *normal = unit_normal_for_edge(&self.at(i), &self.at((i + 1) % num_of_edges));
        }

        let mut j = num_of_edges.wrapping_sub(1);
        for i in 0..num_of_edges {
            let r = 1.0 + ((normals[i].x * normals[j].x) + (normals[i].y * normals[j].y));
            if ((normals[j].x * normals[i].y) - (normals[i].x * normals[j].y)) * offset >= 0.0 {
                let q = offset / r;
                let mut pt = Point::new(
                    self.at(i).x + (normals[j].x + normals[i].x) * q,
                    self.at(i).y + (normals[j].y + normals[i].y) * q,
                );

                pt.id = self.id() as u32;
                pt.vn = new_poly.size() as u16;
                new_poly.ps.push(pt);
            } else {
                let mut pt1 = Point::new(
                    self.at(i).x + normals[j].x * offset,
                    self.at(i).y + normals[j].y * offset,
                );
                let mut pt2 = self.at(i);
                let mut pt3 = Point::new(
                    self.at(i).x + normals[i].x * offset,
                    self.at(i).y + normals[i].y * offset,
                );

                pt1.id = self.id() as u32;
                pt1.vn = new_poly.size() as u16;
                new_poly.ps.push(pt1);

                pt2.id = self.id() as u32;
                pt2.vn = new_poly.size() as u16;
                new_poly.ps.push(pt2);

                pt3.id = self.id() as u32;
                pt3.vn = new_poly.size() as u16;
                new_poly.ps.push(pt3);
            }
            j = i;
        }

        new_poly
    }
}

// upstream: geomtypes.cpp unitNormalForEdge() (static)
fn unit_normal_for_edge(pt1: &Point, pt2: &Point) -> Point {
    if pt2 == pt1 {
        return Point::new(0.0, 0.0);
    }
    let mut dx = pt2.x - pt1.x;
    let mut dy = pt2.y - pt1.y;
    let f = 1.0 / ((dx * dx) + (dy * dy)).sqrt();
    dx *= f;
    dy *= f;
    Point::new(dy, -dx)
}

/// A dynamic Polygon, to which points can be easily added and removed.
///
/// `Clone` is the upstream implicit copy constructor (copies `_id`, `ps`,
/// `ts` and `checkpoints_on_route`); [`Polygon::from_interface`] is the
/// upstream `Polygon(const PolygonInterface&)` constructor (copies only the
/// id and points).
/// An empty polygon, returned by reference-returning getters called on a
/// handle of a freed object (port addition; see "Public-API misuse" in the
/// `router.rs` module docs).
pub static EMPTY_POLYGON: Polygon = Polygon { _id: 0, ps: Vec::new(), ts: Vec::new(), checkpoints_on_route: Vec::new() };

// upstream: geomtypes.h class Polygon
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Polygon {
    /// An ID for the polygon.
    pub _id: i32,
    /// A vector of the points that make up the Polygon.
    pub ps: Vec<Point>,
    /// If used, denotes whether the corresponding point in ps is a move-to
    /// ('M'), line-to ('L'), Bezier curve-to ('C') or close-path ('Z').
    pub ts: Vec<char>,
    /// Checkpoints through which the route travels and the relevant
    /// segment/bendpoint value (0 if on ps[0], 1 if on ps[0]-ps[1], 2 if on
    /// ps[1], ...).
    pub checkpoints_on_route: Vec<(usize, Point)>,
}

/// A multi-segment line, represented with the Polygon class.
// upstream: geomtypes.h typedef Polygon PolyLine
pub type PolyLine = Polygon;

impl PolygonInterface for Polygon {
    // upstream: geomtypes.cpp Polygon::clear()
    fn clear(&mut self) {
        self.ps.clear();
        self.ts.clear();
    }

    // upstream: geomtypes.cpp Polygon::empty()
    fn empty(&self) -> bool {
        self.ps.is_empty()
    }

    // upstream: geomtypes.cpp Polygon::size()
    #[inline]
    fn size(&self) -> usize {
        self.ps.len()
    }

    // upstream: geomtypes.cpp Polygon::id()
    fn id(&self) -> i32 {
        self._id
    }

    // upstream: geomtypes.cpp Polygon::at()
    #[inline]
    fn at(&self, index: usize) -> Point {
        debug_assert!(index < self.size());
        self.ps[index]
    }
}

// upstream: geomtypes.cpp SHORTEN_* constants
const SHORTEN_NONE: u32 = 0;
const SHORTEN_START: u32 = 1;
const SHORTEN_END: u32 = 2;
const SHORTEN_BOTH: u32 = SHORTEN_START | SHORTEN_END;

// upstream: geomtypes.cpp shorten_line() (static)
//     Given the two endpoints of a line segment, this function adjusts the
//     endpoints of the line to shorten the line by shorten_length at either
//     or both ends.
fn shorten_line(
    x1: &mut f64,
    y1: &mut f64,
    x2: &mut f64,
    y2: &mut f64,
    mode: u32,
    shorten_length: f64,
) {
    if mode == SHORTEN_NONE {
        return;
    }

    let rise = *y1 - *y2;
    let run = *x1 - *x2;
    let disty = rise.abs();
    let distx = run.abs();

    // Handle case where shorten length is greater than the length of the
    // line segment.
    if (mode == SHORTEN_BOTH)
        && (((distx > disty) && ((shorten_length * 2.0) > distx))
            || ((disty >= distx) && ((shorten_length * 2.0) > disty)))
    {
        *x1 = *x1 - (run / 2.0);
        *x2 = *x1;
        *y1 = *y1 - (rise / 2.0);
        *y2 = *y1;
        return;
    } else if (mode == SHORTEN_START)
        && (((distx > disty) && (shorten_length > distx))
            || ((disty >= distx) && (shorten_length > disty)))
    {
        *x1 = *x2;
        *y1 = *y2;
        return;
    } else if (mode == SHORTEN_END)
        && (((distx > disty) && (shorten_length > distx))
            || ((disty >= distx) && (shorten_length > disty)))
    {
        *x2 = *x1;
        *y2 = *y1;
        return;
    }

    // Handle orthogonal line segments.
    if *x1 == *x2 {
        // Vertical
        let sign: i32 = if *y1 < *y2 { 1 } else { -1 };

        if (mode & SHORTEN_START) != 0 {
            *y1 += sign as f64 * shorten_length;
        }
        if (mode & SHORTEN_END) != 0 {
            *y2 -= sign as f64 * shorten_length;
        }
        return;
    } else if *y1 == *y2 {
        // Horizontal
        let sign: i32 = if *x1 < *x2 { 1 } else { -1 };

        if (mode & SHORTEN_START) != 0 {
            *x1 += sign as f64 * shorten_length;
        }
        if (mode & SHORTEN_END) != 0 {
            *x2 -= sign as f64 * shorten_length;
        }
        return;
    }

    let xpos: i32 = if *x1 < *x2 { -1 } else { 1 };
    let ypos: i32 = if *y1 < *y2 { -1 } else { 1 };

    let tangent = rise / run;

    if (mode & SHORTEN_END) != 0 {
        if disty > distx {
            *y2 += shorten_length * ypos as f64;
            *x2 += shorten_length * ypos as f64 * (1.0 / tangent);
        } else if disty < distx {
            *y2 += shorten_length * xpos as f64 * tangent;
            *x2 += shorten_length * xpos as f64;
        }
    }

    if (mode & SHORTEN_START) != 0 {
        if disty > distx {
            *y1 -= shorten_length * ypos as f64;
            *x1 -= shorten_length * ypos as f64 * (1.0 / tangent);
        } else if disty < distx {
            *y1 -= shorten_length * xpos as f64 * tangent;
            *x1 -= shorten_length * xpos as f64;
        }
    }
}

// upstream: geomtypes.cpp #define mid(a, b)
#[inline]
fn mid(a: f64, b: f64) -> f64 {
    if a < b {
        a + ((b - a) / 2.0)
    } else {
        b + ((a - b) / 2.0)
    }
}

impl Polygon {
    /// Constructs an empty polygon (with zero points).
    // upstream: geomtypes.cpp Polygon::Polygon()
    pub fn new() -> Polygon {
        Polygon::default()
    }

    /// Constructs a new polygon with n (default-initialised) points.
    // upstream: geomtypes.cpp Polygon::Polygon(const int pn)
    pub fn with_size(pn: usize) -> Polygon {
        Polygon {
            _id: 0,
            ps: vec![Point::default(); pn],
            ts: Vec::new(),
            checkpoints_on_route: Vec::new(),
        }
    }

    /// Constructs a new polygon from an existing PolygonInterface (copies
    /// id and points only).
    // upstream: geomtypes.cpp Polygon::Polygon(const PolygonInterface& poly)
    pub fn from_interface<P: PolygonInterface + ?Sized>(poly: &P) -> Polygon {
        let mut ps = vec![Point::default(); poly.size()];
        for (i, p) in ps.iter_mut().enumerate() {
            *p = poly.at(i);
        }
        Polygon {
            _id: poly.id(),
            ps,
            ts: Vec::new(),
            checkpoints_on_route: Vec::new(),
        }
    }

    // upstream: geomtypes.cpp Polygon::setPoint()
    pub fn set_point(&mut self, index: usize, point: &Point) {
        debug_assert!(index < self.size());
        self.ps[index] = *point;
    }

    // upstream: geomtypes.cpp Polygon::translate()
    pub fn translate(&mut self, x_dist: f64, y_dist: f64) {
        for p in self.ps.iter_mut() {
            p.x += x_dist;
            p.y += y_dist;
        }
    }

    // upstream: geomtypes.cpp Polygon::simplify()
    pub fn simplify(&self) -> Polygon {
        // Copy the PolyLine.
        let mut simplified = self.clone();

        let has_checkpoint_info = !simplified.checkpoints_on_route.is_empty();

        // `it` in upstream always refers to index j - 1.
        // Combine collinear line segments into single segments:
        let mut j: usize = 2;
        while j < simplified.size() {
            if vec_dir(
                &simplified.ps[j - 2],
                &simplified.ps[j - 1],
                &simplified.ps[j],
                0.0,
            ) == 0
            {
                // These three points make up two collinear segments, so just
                // combine them into a single segment.
                simplified.ps.remove(j - 1);

                if has_checkpoint_info {
                    let deleted_point_value = (j - 1) - 1;
                    for cp in simplified.checkpoints_on_route.iter_mut() {
                        // size_t arithmetic: wraps like upstream.
                        if cp.0 == deleted_point_value {
                            cp.0 = cp.0.wrapping_sub(1);
                        } else if cp.0 > deleted_point_value {
                            cp.0 = cp.0.wrapping_sub(2);
                        }
                    }
                }
            } else {
                j += 1;
            }
        }

        simplified
    }

    // upstream: geomtypes.cpp Polygon::checkpointsOnSegment()
    pub fn checkpoints_on_segment(
        &self,
        segment_lower_index: usize,
        index_modifier: i32,
    ) -> Vec<Point> {
        let mut checkpoints = Vec::new();
        // size_t arithmetic: orthogonal.cpp passes `i - 2` with i == 1
        // (i.e. SIZE_MAX), so this must wrap exactly like upstream.
        let mut checkpoint_lower_value = segment_lower_index.wrapping_mul(2);
        let mut checkpoint_upper_value = checkpoint_lower_value.wrapping_add(2);
        let mut index = 0;

        if index_modifier > 0 {
            checkpoint_lower_value = checkpoint_lower_value.wrapping_add(1);
        } else if index_modifier < 0 {
            checkpoint_upper_value = checkpoint_upper_value.wrapping_sub(1);
        }

        while index < self.checkpoints_on_route.len() {
            if (self.checkpoints_on_route[index].0 >= checkpoint_lower_value)
                && (self.checkpoints_on_route[index].0 <= checkpoint_upper_value)
            {
                checkpoints.push(self.checkpoints_on_route[index].1);
            }
            index += 1;
        }
        checkpoints
    }

    // upstream: geomtypes.cpp Polygon::curvedPolyline()
    pub fn curved_polyline(&self, curve_amount: f64, closed: bool) -> Polygon {
        let simplified = self.simplify();

        let mut curved = Polygon::new();
        let num_of_points = self.size();
        if num_of_points <= 2 {
            // There is only a single segment, do nothing.
            curved = self.clone();
            curved.ts.push('M');
            curved.ts.push('L');
            return curved;
        }

        // Build the curved polyline:
        curved._id = self._id;
        let mut last_x = 0.0;
        let mut last_y = 0.0;
        if closed {
            let mut x1 = simplified.ps[0].x;
            let mut y1 = simplified.ps[0].y;
            let mut x2 = simplified.ps[1].x;
            let mut y2 = simplified.ps[1].y;
            shorten_line(&mut x1, &mut y1, &mut x2, &mut y2, SHORTEN_START, curve_amount);
            curved.ps.push(Point::new(x1, y1));
            curved.ts.push('M');
        } else {
            curved.ps.push(self.ps[0]);
            curved.ts.push('M');
        }

        let simp_size = simplified.size();
        let finish = if closed { simp_size + 2 } else { simp_size };
        let mut j = 1;
        while j < finish {
            let mut x1 = simplified.ps[(simp_size + j - 1) % simp_size].x;
            let mut y1 = simplified.ps[(simp_size + j - 1) % simp_size].y;
            let mut x2 = simplified.ps[j % simp_size].x;
            let mut y2 = simplified.ps[j % simp_size].y;

            let old_x = x1;
            let old_y = y1;

            let mut mode = SHORTEN_BOTH;
            if !closed {
                if j == 1 {
                    mode = SHORTEN_END;
                } else if j == (self.size() - 1) {
                    mode = SHORTEN_START;
                }
            }
            shorten_line(&mut x1, &mut y1, &mut x2, &mut y2, mode, curve_amount);

            if j > 1 {
                curved.ts.extend_from_slice(&['C', 'C', 'C']);
                curved
                    .ps
                    .push(Point::new(mid(last_x, old_x), mid(last_y, old_y)));
                curved.ps.push(Point::new(mid(x1, old_x), mid(y1, old_y)));
                curved.ps.push(Point::new(x1, y1));
            }
            if closed && (j == (finish - 1)) {
                // Close the path.
                curved.ts.push('Z');
                curved.ps.push(Point::new(x1, y1));
                break;
            }
            curved.ts.push('L');
            curved.ps.push(Point::new(x2, y2));

            last_x = x2;
            last_y = y2;
            j += 1;
        }

        curved
    }
}

/// A Rectangle, a simpler way to define the polygon for square or
/// rectangular shapes.  Upstream `Rectangle` is a `Polygon` subclass with no
/// additional state, so the constructors here return a `Polygon`.
// upstream: geomtypes.h class Rectangle
pub struct Rectangle;

impl Rectangle {
    /// Constructs a rectangular polygon given two opposing corner points.
    // upstream: geomtypes.cpp Rectangle::Rectangle(const Point&, const Point&)
    #[allow(clippy::new_ret_no_self)]
    pub fn new(top_left: &Point, bottom_right: &Point) -> Polygon {
        let mut poly = Polygon::with_size(4);
        let x_min = cpp_min(top_left.x, bottom_right.x);
        let x_max = cpp_max(top_left.x, bottom_right.x);
        let y_min = cpp_min(top_left.y, bottom_right.y);
        let y_max = cpp_max(top_left.y, bottom_right.y);

        poly.ps[0] = Point::new(x_max, y_min);
        poly.ps[1] = Point::new(x_max, y_max);
        poly.ps[2] = Point::new(x_min, y_max);
        poly.ps[3] = Point::new(x_min, y_min);
        poly
    }

    /// Constructs a rectangular polygon given the centre, width and height.
    // upstream: geomtypes.cpp Rectangle::Rectangle(const Point&, double, double)
    pub fn from_centre(centre: &Point, width: f64, height: f64) -> Polygon {
        let mut poly = Polygon::with_size(4);
        let half_width = width / 2.0;
        let half_height = height / 2.0;
        let x_min = centre.x - half_width;
        let x_max = centre.x + half_width;
        let y_min = centre.y - half_height;
        let y_max = centre.y + half_height;

        poly.ps[0] = Point::new(x_max, y_min);
        poly.ps[1] = Point::new(x_max, y_max);
        poly.ps[2] = Point::new(x_min, y_max);
        poly.ps[3] = Point::new(x_min, y_min);
        poly
    }
}

/// Resolves obstacle polygons for [`ReferencingPolygon`] (upstream holds a
/// `const Polygon *` to `Obstacle::m_polygon`).  The router agent implements
/// this for `Router` once the obstacle arena exists.
pub trait PolygonLookup {
    fn referenced_polygon(&self, obstacle: ObstacleIdx) -> &Polygon;
}

/// A Polygon which just references its points from other Polygons.
///
/// `ps_ref[i] = (Some(obstacle), vn)` refers to point `vn` of that
/// obstacle's (current) polygon; `(None, K_UNASSIGNED_VERTEX_NUMBER)` means
/// the point is stored by value in `ps_points[i]`.
// upstream: geomtypes.h class ReferencingPolygon
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReferencingPolygon {
    pub _id: i32,
    pub ps_ref: Vec<(Option<ObstacleIdx>, u16)>,
    pub ps_points: Vec<Point>,
}

impl ReferencingPolygon {
    // upstream: geomtypes.cpp ReferencingPolygon::ReferencingPolygon()
    pub fn new() -> ReferencingPolygon {
        ReferencingPolygon::default()
    }

    /// `obstacles` must yield `(obstacle->id(), handle)` for every obstacle in
    /// `Router::m_obstacles` **in list order** (the first match wins, as
    /// upstream).
    // upstream: geomtypes.cpp ReferencingPolygon::ReferencingPolygon(const Polygon&, const Router*)
    pub fn from_polygon<I>(poly: &Polygon, obstacles: I) -> ReferencingPolygon
    where
        I: IntoIterator<Item = (u32, ObstacleIdx)> + Clone,
    {
        let mut ps_ref = vec![(None, 0u16); poly.size()];
        let mut ps_points = vec![Point::default(); poly.size()];
        for i in 0..poly.size() {
            if poly.ps[i].id == 0 {
                // Can't be referenced, so just make a copy of the point.
                ps_ref[i] = (None, K_UNASSIGNED_VERTEX_NUMBER);
                ps_points[i] = poly.ps[i];
            } else {
                let mut poly_ptr: Option<ObstacleIdx> = None;
                for (sh_id, sh) in obstacles.clone() {
                    if sh_id == poly.ps[i].id {
                        poly_ptr = Some(sh);
                        break;
                    }
                }
                debug_assert!(poly_ptr.is_some());
                ps_ref[i] = (poly_ptr, poly.ps[i].vn);
            }
        }
        ReferencingPolygon {
            _id: poly._id,
            ps_ref,
            ps_points,
        }
    }

    // upstream: geomtypes.cpp ReferencingPolygon::clear()
    pub fn clear(&mut self) {
        self.ps_ref.clear();
        self.ps_points.clear();
    }

    // upstream: geomtypes.cpp ReferencingPolygon::empty()
    pub fn empty(&self) -> bool {
        self.ps_ref.is_empty()
    }

    // upstream: geomtypes.cpp ReferencingPolygon::size()
    pub fn size(&self) -> usize {
        self.ps_ref.len()
    }

    // upstream: geomtypes.cpp ReferencingPolygon::id()
    pub fn id(&self) -> i32 {
        self._id
    }

    // upstream: geomtypes.cpp ReferencingPolygon::at()
    pub fn at<L: PolygonLookup + ?Sized>(&self, index: usize, lookup: &L) -> Point {
        debug_assert!(index < self.size());

        if let Some(obstacle) = self.ps_ref[index].0 {
            let poly = lookup.referenced_polygon(obstacle);
            let poly_index = self.ps_ref[index].1 as usize;
            debug_assert!(poly_index < poly.size());

            poly.ps[poly_index]
        } else {
            self.ps_points[index]
        }
    }

    /// Binds a lookup so the polygon can be used as a `PolygonInterface`
    /// (e.g. for `boundingRectPolygon()` or `inPolyGen()`).
    pub fn view<'a, L: PolygonLookup + ?Sized>(
        &'a self,
        lookup: &'a L,
    ) -> ReferencingPolygonView<'a, L> {
        ReferencingPolygonView { poly: self, lookup }
    }
}

/// A [`ReferencingPolygon`] bound to a [`PolygonLookup`].
pub struct ReferencingPolygonView<'a, L: PolygonLookup + ?Sized> {
    pub poly: &'a ReferencingPolygon,
    pub lookup: &'a L,
}

impl<L: PolygonLookup + ?Sized> PolygonInterface for ReferencingPolygonView<'_, L> {
    fn clear(&mut self) {
        panic!("ReferencingPolygonView is read-only; clear the ReferencingPolygon itself");
    }
    fn empty(&self) -> bool {
        self.poly.empty()
    }
    fn size(&self) -> usize {
        self.poly.size()
    }
    fn id(&self) -> i32 {
        self.poly.id()
    }
    fn at(&self, index: usize) -> Point {
        self.poly.at(index, self.lookup)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_equality_ignores_id_and_vn() {
        let mut a = Point::new(1.0, 2.0);
        let b = Point::new(1.0, 2.0);
        a.id = 5;
        a.vn = 3;
        assert!(a == b);
        assert!(!(a != b));
        let nan = Point::new(f64::NAN, 0.0);
        assert!(nan != nan);
        assert!(!(nan == nan));
    }

    #[test]
    fn point_ordering() {
        assert!(Point::new(1.0, 5.0).less_than(&Point::new(2.0, 0.0)));
        assert!(Point::new(1.0, 0.0).less_than(&Point::new(1.0, 1.0)));
        assert!(!Point::new(1.0, 1.0).less_than(&Point::new(1.0, 1.0)));
        let mut p = Point::new(3.0, 4.0);
        assert_eq!(p[XDIM], 3.0);
        p[YDIM] = 7.0;
        assert_eq!(p.y, 7.0);
        assert_eq!(Point::default().vn, K_UNASSIGNED_VERTEX_NUMBER);
    }

    #[test]
    fn rectangle_corners() {
        let r = Rectangle::new(&Point::new(10.0, 0.0), &Point::new(0.0, 5.0));
        assert_eq!(r.ps[0], Point::new(10.0, 0.0));
        assert_eq!(r.ps[1], Point::new(10.0, 5.0));
        assert_eq!(r.ps[2], Point::new(0.0, 5.0));
        assert_eq!(r.ps[3], Point::new(0.0, 0.0));
        let b = r.offset_bounding_box(1.0);
        assert_eq!(b.min, Point::new(-1.0, -1.0));
        assert_eq!(b.max, Point::new(11.0, 6.0));
        assert_eq!(b.width(), 12.0);
    }

    struct OneObstacle(Polygon);
    impl PolygonLookup for OneObstacle {
        fn referenced_polygon(&self, _o: ObstacleIdx) -> &Polygon {
            &self.0
        }
    }

    #[test]
    fn referencing_polygon_tracks_obstacle_polygon() {
        let mut shape = Rectangle::new(&Point::new(0.0, 0.0), &Point::new(10.0, 10.0));
        shape._id = 7;
        let mut cluster = Polygon::new();
        let mut p = shape.ps[2];
        p.id = 7;
        p.vn = 2;
        cluster.ps.push(p);
        cluster.ps.push(Point::new(50.0, 50.0)); // id 0: copied by value
        let rp = ReferencingPolygon::from_polygon(&cluster, vec![(7u32, ObstacleIdx::dangling())]);
        let mut lookup = OneObstacle(shape);
        assert_eq!(rp.at(0, &lookup), Point::new(0.0, 10.0));
        assert_eq!(rp.at(1, &lookup), Point::new(50.0, 50.0));
        lookup.0.translate(1.0, 1.0);
        assert_eq!(rp.at(0, &lookup), Point::new(1.0, 11.0));
        let bb = rp.view(&lookup).bounding_rect_polygon();
        assert_eq!(bb.ps[3], Point::new(1.0, 11.0));
    }

    /// Regression: orthogonal.cpp calls `checkpointsOnSegment(i - 2, -1)`
    /// with `i == 1`, i.e. `size_t` `SIZE_MAX`; the bounds wrap to
    /// `[SIZE_MAX - 1, SIZE_MAX]` (and so match only wrapped checkpoint
    /// indices).  This used to overflow-panic in debug builds.
    #[test]
    fn checkpoints_on_segment_wraps_like_size_t() {
        let mut p = Polygon::new();
        p.ps = vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)];
        p.checkpoints_on_route = vec![(1, Point::new(5.0, 0.0)), (usize::MAX, Point::new(0.0, 0.0))];
        let prev = p.checkpoints_on_segment(1usize.wrapping_sub(2), -1);
        assert_eq!(prev, vec![Point::new(0.0, 0.0)]);
        assert_eq!(p.checkpoints_on_segment(0, 0), vec![Point::new(5.0, 0.0)]);
        assert!(p.checkpoints_on_segment(1, 1).is_empty());
    }
}
