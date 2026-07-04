use crate::constants::EPS;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Point {
    pub(crate) x: f64,
    pub(crate) y: f64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Rect {
    pub(crate) left: f64,
    pub(crate) top: f64,
    pub(crate) right: f64,
    pub(crate) bottom: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Box2D {
    pub(crate) min: Point,
    pub(crate) max: Point,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Polygon {
    pub(crate) id: i32,
    pub(crate) points: Vec<Point>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Node {
    pub(crate) x: i64,
    pub(crate) y: i64,
}

impl Rect {
    pub(crate) fn new(x1: f64, y1: f64, x2: f64, y2: f64) -> Rect {
        Rect {
            left: x1.min(x2),
            top: y1.min(y2),
            right: x1.max(x2),
            bottom: y1.max(y2),
        }
    }

    pub(crate) fn translate(self, x_diff: f64, y_diff: f64) -> Rect {
        Rect {
            left: self.left + x_diff,
            top: self.top + y_diff,
            right: self.right + x_diff,
            bottom: self.bottom + y_diff,
        }
    }

    pub(crate) fn to_polygon(self) -> Polygon {
        Polygon {
            id: 0,
            points: vec![
                Point {
                    x: self.left,
                    y: self.top,
                },
                Point {
                    x: self.right,
                    y: self.top,
                },
                Point {
                    x: self.right,
                    y: self.bottom,
                },
                Point {
                    x: self.left,
                    y: self.bottom,
                },
            ],
        }
    }
}

impl Box2D {
    pub(crate) fn width(self) -> f64 {
        self.max.x - self.min.x
    }

    pub(crate) fn height(self) -> f64 {
        self.max.y - self.min.y
    }

    pub(crate) fn to_rect(self) -> Rect {
        let _ = (self.width(), self.height());
        Rect::new(self.min.x, self.min.y, self.max.x, self.max.y)
    }
}

impl Polygon {
    pub(crate) fn new(points: Vec<Point>) -> Option<Polygon> {
        if points.len() < 2
            || points
                .iter()
                .any(|point| !point.x.is_finite() || !point.y.is_finite())
        {
            return None;
        }
        Some(Polygon { id: 0, points })
    }

    pub(crate) fn rectangle(left: f64, top: f64, right: f64, bottom: f64) -> Polygon {
        Rect::new(left, top, right, bottom).to_polygon()
    }

    pub(crate) fn bounding_box(&self) -> Option<Box2D> {
        let first = *self.points.first()?;
        let mut min = first;
        let mut max = first;
        for point in &self.points[1..] {
            min.x = min.x.min(point.x);
            min.y = min.y.min(point.y);
            max.x = max.x.max(point.x);
            max.y = max.y.max(point.y);
        }
        Some(Box2D { min, max })
    }

    #[allow(dead_code)]
    pub(crate) fn offset_bounding_box(&self, offset: f64) -> Option<Box2D> {
        let bbox = self.bounding_box()?;
        Some(Box2D {
            min: Point {
                x: bbox.min.x - offset,
                y: bbox.min.y - offset,
            },
            max: Point {
                x: bbox.max.x + offset,
                y: bbox.max.y + offset,
            },
        })
    }

    pub(crate) fn offset_polygon(&self, offset: f64) -> Option<Polygon> {
        if !offset.is_finite() || self.points.len() < 2 {
            return None;
        }
        if offset == 0.0 {
            return Some(self.clone());
        }

        let edge_count = self.points.len();
        let normals = self
            .edges()
            .map(|(start, end)| unit_normal_for_edge(start, end))
            .collect::<Vec<_>>();
        let mut points = Vec::new();
        let mut previous = edge_count - 1;
        for current in 0..edge_count {
            let previous_normal = normals[previous];
            let current_normal = normals[current];
            let cross =
                (previous_normal.x * current_normal.y) - (current_normal.x * previous_normal.y);
            if cross * offset >= 0.0 {
                let r = 1.0
                    + ((current_normal.x * previous_normal.x)
                        + (current_normal.y * previous_normal.y));
                if r.abs() < EPS {
                    return None;
                }
                let q = offset / r;
                points.push(Point {
                    x: self.points[current].x + ((previous_normal.x + current_normal.x) * q),
                    y: self.points[current].y + ((previous_normal.y + current_normal.y) * q),
                });
            } else {
                points.push(Point {
                    x: self.points[current].x + (previous_normal.x * offset),
                    y: self.points[current].y + (previous_normal.y * offset),
                });
                points.push(self.points[current]);
                points.push(Point {
                    x: self.points[current].x + (current_normal.x * offset),
                    y: self.points[current].y + (current_normal.y * offset),
                });
            }
            previous = current;
        }
        Some(Polygon {
            id: self.id,
            points,
        })
    }

    pub(crate) fn translate(&mut self, x_diff: f64, y_diff: f64) {
        for point in &mut self.points {
            point.x += x_diff;
            point.y += y_diff;
        }
    }

    pub(crate) fn contains_point(&self, point: Point, count_border: bool) -> bool {
        if self.points.len() < 3 {
            return false;
        }
        if self
            .edges()
            .any(|(a, b)| point_on_line(a, b, point, EPS) || a == point || b == point)
        {
            return count_border;
        }

        let mut inside = false;
        let mut j = self.points.len() - 1;
        for i in 0..self.points.len() {
            let pi = self.points[i];
            let pj = self.points[j];
            if ((pi.y > point.y) != (pj.y > point.y))
                && (point.x < (pj.x - pi.x) * (point.y - pi.y) / (pj.y - pi.y) + pi.x)
            {
                inside = !inside;
            }
            j = i;
        }
        inside
    }

    pub(crate) fn intersects_axis_segment(&self, a: Point, b: Point) -> bool {
        if (a.x - b.x).abs() >= EPS && (a.y - b.y).abs() >= EPS {
            return false;
        }
        if self.contains_point(a, false) || self.contains_point(b, false) {
            return true;
        }

        let mut seen_intersection_at_endpoint = false;
        self.edges().any(|(s1, s2)| {
            segment_shape_intersect(a, b, s1, s2, &mut seen_intersection_at_endpoint)
        })
    }

    pub(crate) fn intersects_segment(&self, a: Point, b: Point) -> bool {
        if self.contains_point(a, false) || self.contains_point(b, false) {
            return true;
        }
        let midpoint = Point {
            x: (a.x + b.x) / 2.0,
            y: (a.y + b.y) / 2.0,
        };
        if self.contains_point(midpoint, false) {
            return true;
        }
        self.edges()
            .any(|(edge_start, edge_end)| segment_intersect(a, b, edge_start, edge_end))
    }

    pub(crate) fn edges(&self) -> impl Iterator<Item = (Point, Point)> + '_ {
        self.points
            .iter()
            .copied()
            .zip(self.points.iter().copied().cycle().skip(1))
            .take(self.points.len())
    }
}

fn unit_normal_for_edge(start: Point, end: Point) -> Point {
    if start == end {
        return Point { x: 0.0, y: 0.0 };
    }
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let factor = 1.0 / ((dx * dx) + (dy * dy)).sqrt();
    Point {
        x: dy * factor,
        y: -dx * factor,
    }
}

pub(crate) fn euclidean_distance(a: Point, b: Point) -> f64 {
    let x_diff = a.x - b.x;
    let y_diff = a.y - b.y;
    ((x_diff * x_diff) + (y_diff * y_diff)).sqrt()
}

pub(crate) fn manhattan_distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
}

#[allow(dead_code)]
pub(crate) fn total_length(polyline: &Polygon) -> f64 {
    polyline
        .points
        .windows(2)
        .map(|points| euclidean_distance(points[0], points[1]))
        .sum()
}

#[allow(dead_code)]
pub(crate) fn angle(a: Point, b: Point, c: Point) -> f64 {
    let ux = b.x - a.x;
    let uy = b.y - a.y;
    let vx = c.x - b.x;
    let vy = c.y - b.y;
    let lu = ((ux * ux) + (uy * uy)).sqrt();
    let lv = ((vx * vx) + (vy * vy)).sqrt();
    let u_dot_v = (ux * vx) + (uy * vy);
    let cos_theta = (u_dot_v / (lu * lv)).clamp(-1.0, 1.0);
    cos_theta.acos()
}

#[allow(dead_code)]
pub(crate) fn projection(a: Point, b: Point, c: Point) -> Point {
    let ux = c.x - a.x;
    let uy = c.y - a.y;
    let vx = b.x - a.x;
    let vy = b.y - a.y;
    let scalar_projection = ((ux * vx) + (uy * vy)) / ((ux * ux) + (uy * uy));
    Point {
        x: (scalar_projection * ux) + a.x,
        y: (scalar_projection * uy) + a.y,
    }
}

#[allow(dead_code)]
pub(crate) fn rotational_angle(point: Point) -> f64 {
    if point.y == 0.0 {
        return if point.x < 0.0 { 180.0 } else { 0.0 };
    }
    if point.x == 0.0 {
        return if point.y < 0.0 { 270.0 } else { 90.0 };
    }

    let mut angle = (point.y / point.x).atan().to_degrees();
    if point.x < 0.0 {
        angle += 180.0;
    } else if point.y < 0.0 {
        angle += 360.0;
    }
    angle
}

pub(crate) fn vec_dir(a: Point, b: Point, c: Point, maybe_zero: f64) -> i32 {
    let area2 = ((b.x - a.x) * (c.y - a.y)) - ((c.x - a.x) * (b.y - a.y));
    if area2 < -maybe_zero {
        -1
    } else if area2 > maybe_zero {
        1
    } else {
        0
    }
}

pub(crate) fn colinear(a: Point, b: Point, c: Point, tolerance: f64) -> bool {
    if a == b {
        true
    } else if (a.x - b.x).abs() < EPS {
        (a.x - c.x).abs() < EPS
    } else if (a.y - b.y).abs() < EPS {
        (a.y - c.y).abs() < EPS
    } else {
        vec_dir(a, b, c, tolerance) == 0
    }
}

pub(crate) fn point_on_line(a: Point, b: Point, c: Point, tolerance: f64) -> bool {
    if (a.x - b.x).abs() < EPS {
        return (a.x - c.x).abs() <= tolerance.max(EPS)
            && (((a.y < c.y) && (c.y < b.y)) || ((b.y < c.y) && (c.y < a.y)));
    }
    if (a.y - b.y).abs() < EPS {
        return (a.y - c.y).abs() <= tolerance.max(EPS)
            && (((a.x < c.x) && (c.x < b.x)) || ((b.x < c.x) && (c.x < a.x)));
    }
    colinear(a, b, c, tolerance) && in_between(a, b, c)
}

pub(crate) fn in_between(a: Point, b: Point, c: Point) -> bool {
    if (a.x - b.x).abs() > f64::EPSILON {
        ((a.x < c.x) && (c.x < b.x)) || ((b.x < c.x) && (c.x < a.x))
    } else {
        ((a.y < c.y) && (c.y < b.y)) || ((b.y < c.y) && (c.y < a.y))
    }
}

#[allow(dead_code)]
pub(crate) fn in_valid_region(
    ignore_regions: bool,
    a0: Point,
    a1: Point,
    a2: Point,
    b: Point,
) -> bool {
    let r_side = vec_dir(b, a0, a1, 0.0);
    let s_side = vec_dir(b, a1, a2, 0.0);

    let r_out_on = r_side <= 0;
    let s_out_on = s_side <= 0;
    let r_out = r_side < 0;
    let s_out = s_side < 0;

    if vec_dir(a0, a1, a2, 0.0) > 0 {
        if ignore_regions {
            return (r_out_on && !s_out) || (!r_out && s_out_on);
        }
        r_out_on || s_out_on
    } else {
        !ignore_regions && r_out_on && s_out_on
    }
}

#[allow(dead_code)]
pub(crate) fn corner_side(c1: Point, c2: Point, c3: Point, p: Point) -> i32 {
    let s123 = vec_dir(c1, c2, c3, 0.0);
    let s12p = vec_dir(c1, c2, p, 0.0);
    let s23p = vec_dir(c2, c3, p, 0.0);

    if s123 == 1 {
        if s12p >= 0 && s23p >= 0 {
            return 1;
        }
        return -1;
    }
    if s123 == -1 {
        if s12p <= 0 && s23p <= 0 {
            return -1;
        }
        return 1;
    }
    s12p
}

pub(crate) fn segment_intersect(a: Point, b: Point, c: Point, d: Point) -> bool {
    let ab_c = vec_dir(a, b, c, 0.0);
    if ab_c == 0 {
        return false;
    }
    let ab_d = vec_dir(a, b, d, 0.0);
    if ab_d == 0 {
        return false;
    }
    let cd_a = vec_dir(c, d, a, 0.0);
    let cd_b = vec_dir(c, d, b, 0.0);
    (ab_c * ab_d) < 0 && (cd_a * cd_b) < 0
}

pub(crate) fn segment_shape_intersect(
    e1: Point,
    e2: Point,
    s1: Point,
    s2: Point,
    seen_intersection_at_endpoint: &mut bool,
) -> bool {
    if segment_intersect(e1, e2, s1, s2) {
        return true;
    }

    let e1_on_shape = s2 == e1 || point_on_line(s1, s2, e1, EPS);
    let e2_on_shape = s2 == e2 || point_on_line(s1, s2, e2, EPS);
    if (e1_on_shape && vec_dir(s1, s2, e2, 0.0) != 0)
        || (e2_on_shape && vec_dir(s1, s2, e1, 0.0) != 0)
    {
        if *seen_intersection_at_endpoint {
            return true;
        }
        *seen_intersection_at_endpoint = true;
    }
    false
}

impl Node {
    pub(crate) fn from_point(point: Point) -> Node {
        Node {
            x: quantize(point.x),
            y: quantize(point.y),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn to_point(self) -> Point {
        Point {
            x: self.x as f64 / 1000.0,
            y: self.y as f64 / 1000.0,
        }
    }
}

fn quantize(value: f64) -> i64 {
    (value * 1000.0).round() as i64
}
