//! Differential test against the C++ oracle `tools/oracle/crossings_oracle.cpp`
//! (golden output in `tests/golden/crossings_oracle.txt`).
//!
//! Regenerates exactly the same inputs (same LCG, same consumption order),
//! runs the Rust port of `splitBranchingSegments`,
//! `ConnectorCrossings::countForSegment` and `PtOrder` on them and must
//! reproduce the oracle's output byte for byte.

#![allow(clippy::uninlined_format_args, clippy::needless_range_loop)]

use libavoid_rust::avoid::arena::ConnIdx;
use libavoid_rust::avoid::connector::*;
use libavoid_rust::avoid::connend::ConnEnd;
use libavoid_rust::avoid::geometry::point_on_line;
use libavoid_rust::avoid::geomtypes::{Point, Polygon, PolygonInterface};
use libavoid_rust::avoid::junction::JunctionRef;
use libavoid_rust::avoid::router::{format_g, Router, ORTHOGONAL_ROUTING};

use std::fmt::Write as _;

struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
    fn rnd(&mut self, n: u32) -> u32 {
        ((self.next_u64() >> 33) % n as u64) as u32
    }
    fn grid_pt(&mut self, range: u32) -> Point {
        let x = self.rnd(range) as f64;
        let y = self.rnd(range) as f64;
        Point::new(x, y)
    }
}

fn g17(v: f64) -> String {
    format_g(v, 17)
}

fn pt_str(p: &Point) -> String {
    format!("({},{},{},{})", g17(p.x), g17(p.y), p.id, p.vn)
}

fn poly_str(p: &Polygon) -> String {
    let v: Vec<String> = p.ps.iter().map(pt_str).collect();
    format!("[{}]", v.join(" "))
}

// ---------------------------------------------------------------------------
// Geometry generators (mirroring the oracle).

fn orth_walk(r: &mut Rng, n: u32, start: Point) -> Vec<Point> {
    let mut ps = vec![start];
    let mut axis = r.rnd(2);
    for _ in 1..n {
        let mut q = *ps.last().unwrap();
        if r.rnd(10) < 7 {
            axis = 1 - axis;
        }
        let mut len = (r.rnd(3) + 1) as f64;
        if r.rnd(2) == 0 {
            len = -len;
        }
        if axis == 0 {
            q.x += len;
        } else {
            q.y += len;
        }
        ps.push(q);
    }
    ps
}

fn free_walk(r: &mut Rng, n: u32, start: Point) -> Vec<Point> {
    let mut ps = vec![start];
    for _ in 1..n {
        let mut q = r.grid_pt(6);
        if q == *ps.last().unwrap() {
            q.x += 1.0;
        }
        ps.push(q);
    }
    ps
}

fn walk(r: &mut Rng, orth: bool, n: u32, start: Point) -> Vec<Point> {
    if orth {
        orth_walk(r, n, start)
    } else {
        free_walk(r, n, start)
    }
}

fn assign_ids(r: &mut Rng, ps: &mut [Point], orth: bool) {
    for p in ps.iter_mut() {
        p.id = r.rnd(4);
        p.vn = r.rnd(if orth { 9 } else { 8 }) as u16;
    }
}

fn to_poly(ps: Vec<Point>) -> Polygon {
    Polygon {
        ps,
        ..Default::default()
    }
}

fn gen_conn_pair(r: &mut Rng, orth: bool) -> (Polygon, Polygon) {
    let cn = 2 + r.rnd(6);
    let cs = r.grid_pt(5);
    let mut c = walk(r, orth, cn, cs);
    let mut p: Vec<Point>;
    let strategy = r.rnd(5);
    if strategy == 0 {
        let pn = 2 + r.rnd(6);
        let ps0 = r.grid_pt(5);
        p = walk(r, orth, pn, ps0);
    } else if strategy == 1 || strategy == 2 {
        let a = r.rnd(c.len() as u32 - 1) as usize;
        let b = a + 1 + r.rnd(c.len() as u32 - 1 - a as u32) as usize;
        let slice: Vec<Point> = c[a..b + 1].to_vec();
        let pre_n = 1 + r.rnd(3);
        let mut pre = walk(r, orth, pre_n, slice[0]);
        pre.reverse();
        pre.pop();
        let suf_n = 1 + r.rnd(3);
        let suf = walk(r, orth, suf_n, *slice.last().unwrap());
        p = pre;
        p.extend_from_slice(&slice);
        p.extend_from_slice(&suf[1..]);
        if r.rnd(2) == 0 {
            p.reverse();
        }
    } else if strategy == 3 {
        p = c.clone();
        if r.rnd(2) == 0 {
            p.reverse();
        }
    } else {
        let start = if r.rnd(2) == 0 { c[0] } else { *c.last().unwrap() };
        let pn = 2 + r.rnd(5);
        p = walk(r, orth, pn, start);
        if r.rnd(2) == 0 {
            p.reverse();
        }
    }
    assign_ids(r, &mut c, orth);
    assign_ids(r, &mut p, orth);
    (to_poly(c), to_poly(p))
}

fn offset_pt(r: &mut Rng, v: &Point) -> Point {
    const DX: [i32; 8] = [1, -1, 0, 0, 1, 1, -1, -1];
    const DY: [i32; 8] = [0, 0, 1, -1, 1, -1, 1, -1];
    let d = r.rnd(8) as usize;
    Point::new(v.x + DX[d] as f64, v.y + DY[d] as f64)
}

fn on_boundary(p: &[Point], q: &Point) -> bool {
    for i in 0..p.len() {
        let a = &p[i];
        let b = &p[(i + 1) % p.len()];
        if (q == a) || point_on_line(a, b, q, 0.0) {
            return true;
        }
    }
    false
}

/// Returns (conn, poly, orth).
fn gen_closed_pair(r: &mut Rng) -> (Polygon, Polygon, bool) {
    let mut p: Vec<Point> = Vec::new();
    let kind = r.rnd(3);
    let orth = kind == 0;
    if kind == 0 {
        let x1 = r.rnd(4) as f64;
        let y1 = r.rnd(4) as f64;
        let x2 = x1 + 1.0 + r.rnd(4) as f64;
        let y2 = y1 + 1.0 + r.rnd(4) as f64;
        p.push(Point::new(x2, y1));
        p.push(Point::new(x2, y2));
        p.push(Point::new(x1, y2));
        p.push(Point::new(x1, y1));
        if r.rnd(2) == 0 {
            let mut q = Vec::new();
            for i in 0..p.len() {
                q.push(p[i]);
                if r.rnd(2) == 0 {
                    let n = p[(i + 1) % p.len()];
                    let m = Point::new((p[i].x + n.x) / 2.0, (p[i].y + n.y) / 2.0);
                    q.push(m);
                }
            }
            p = q;
        }
    } else {
        let x1 = r.rnd(3) as f64;
        let y1 = r.rnd(3) as f64;
        let x2 = x1 + 3.0 + r.rnd(3) as f64;
        let y2 = y1 + 3.0 + r.rnd(3) as f64;
        let corners = [Point::new(x2, y1), Point::new(x2, y2), Point::new(x1, y2), Point::new(x1, y1)];
        for i in 0..4 {
            let prev = corners[(i + 3) % 4];
            let next = corners[(i + 1) % 4];
            if r.rnd(3) != 0 {
                let mut a = corners[i];
                let mut b = corners[i];
                if prev.x == a.x {
                    a.y += if prev.y > a.y { 1.0 } else { -1.0 };
                } else {
                    a.x += if prev.x > a.x { 1.0 } else { -1.0 };
                }
                if next.x == b.x {
                    b.y += if next.y > b.y { 1.0 } else { -1.0 };
                } else {
                    b.x += if next.x > b.x { 1.0 } else { -1.0 };
                }
                p.push(a);
                p.push(b);
            } else {
                p.push(corners[i]);
            }
        }
    }
    let k = r.rnd(p.len() as u32) as usize;
    let len = 1 + r.rnd(p.len() as u32) as usize;
    let fwd = r.rnd(2) == 0;
    let mut path = Vec::new();
    for i in 0..len {
        let idx = if fwd {
            (k + i) % p.len()
        } else {
            (k + p.len() * 4 - i) % p.len()
        };
        path.push(p[idx]);
    }
    let mut c: Vec<Point> = Vec::new();
    if r.rnd(4) != 0 {
        let q = offset_pt(r, &path[0]);
        if !on_boundary(&p, &q) {
            c.push(q);
        }
    }
    c.extend_from_slice(&path);
    if (r.rnd(4) != 0) || (c.len() < 2) {
        let q = offset_pt(r, path.last().unwrap());
        if !on_boundary(&p, &q) || (c.len() < 2) {
            c.push(q);
        }
    }
    for i in 0..p.len() {
        p[i].id = r.rnd(4);
        if orth && p.len() == 4 {
            p[i].vn = i as u16;
            r.rnd(8);
        } else {
            p[i].vn = r.rnd(8) as u16;
        }
    }
    for i in 0..c.len() {
        c[i].id = r.rnd(4);
        c[i].vn = r.rnd(8) as u16;
    }
    (to_poly(c), to_poly(p), orth)
}

// ---------------------------------------------------------------------------
// Output.

fn conn_name(router: &Router, c: Option<ConnIdx>) -> String {
    match c {
        None => "-".into(),
        Some(c) => format!("{}", router.conns[c].id()),
    }
}

fn pair_str(router: &Router, pr: &PtConnPtrPair, valid_ptrs: bool) -> String {
    let mut s = conn_name(router, pr.second);
    if valid_ptrs {
        let side = match pr.first.route {
            PtRefRoute::Poly => "P",
            PtRefRoute::Conn => "C",
        };
        write!(s, ":{}{}{}", side, pr.first.index, pt_str(&pr.first.point)).unwrap();
    }
    s
}

fn dump_point_orders(
    o: &mut String,
    router: &Router,
    orders: &mut PtOrderMap,
    valid_ptrs: bool,
    c1: Option<ConnIdx>,
    c2: Option<ConnIdx>,
) {
    writeln!(o, "  orders {}", orders.len()).unwrap();
    for (key, order) in orders.iter_mut() {
        writeln!(o, "   key {}", pt_str(&key.0)).unwrap();
        for dim in 0..2 {
            let v = order.sorted_points(dim);
            let mut s = String::new();
            for pr in v.iter() {
                s.push(' ');
                s.push_str(&pair_str(router, pr, valid_ptrs));
            }
            let pos1 = order.position_for(dim, c1);
            let pos2 = order.position_for(dim, c2);
            let pos_n = order.position_for(dim, None);
            writeln!(o, "    d{} n={}{} | {} {} {}", dim, v.len(), s, pos1, pos2, pos_n).unwrap();
        }
    }
}

fn would_overflow(poly: &Polygon, poly_is_conn: bool, conn: &Polygon, will_split: bool) -> bool {
    let mut p2 = poly.clone();
    let mut c2 = conn.clone();
    if will_split {
        let tolerance = if !poly_is_conn { f64::EPSILON } else { 0.0 };
        split_branching_segments(&mut p2, poly_is_conn, &mut c2, tolerance);
    }
    c2.size() > p2.size()
}

// ---------------------------------------------------------------------------
// Sections.

fn section_split(o: &mut String, r: &mut Rng, n: u32) {
    o.push_str("### split\n");
    for t in 0..n {
        let poly_is_conn = r.rnd(3) != 0;
        let (mut conn, mut poly);
        if poly_is_conn {
            let orth = r.rnd(3) != 0;
            (conn, poly) = gen_conn_pair(r, orth);
        } else {
            let (c, p, _orth) = gen_closed_pair(r);
            conn = c;
            poly = p;
        }
        let mut tolerance = 0.0;
        if r.rnd(2) == 0 {
            tolerance = f64::EPSILON;
        }
        writeln!(o, "case {} pic={} tol={}", t, poly_is_conn as i32, g17(tolerance)).unwrap();
        writeln!(o, "  poly {}", poly_str(&poly)).unwrap();
        writeln!(o, "  conn {}", poly_str(&conn)).unwrap();
        split_branching_segments(&mut poly, poly_is_conn, &mut conn, tolerance);
        writeln!(o, "  poly' {}", poly_str(&poly)).unwrap();
        writeln!(o, "  conn' {}", poly_str(&conn)).unwrap();
    }
}

fn section_split_vn(o: &mut String, r: &mut Rng, n: u32) {
    o.push_str("### splitvn\n");
    for t in 0..n {
        let (mut conn, mut poly) = gen_conn_pair(r, true);
        for k in 0..2 {
            let pg = if k == 0 { &mut conn } else { &mut poly };
            for i in 0..pg.ps.len() {
                let mut vn = 8;
                if r.rnd(2) == 0 {
                    vn = r.rnd(4);
                }
                pg.ps[i].vn = vn as u16;
            }
        }
        writeln!(o, "case {}", t).unwrap();
        writeln!(o, "  poly {}", poly_str(&poly)).unwrap();
        writeln!(o, "  conn {}", poly_str(&conn)).unwrap();
        split_branching_segments(&mut poly, true, &mut conn, 0.0);
        writeln!(o, "  poly' {}", poly_str(&poly)).unwrap();
        writeln!(o, "  conn' {}", poly_str(&conn)).unwrap();
    }
}

fn section_split_closed(o: &mut String, r: &mut Rng, n: u32) {
    o.push_str("### splitclosed\n");
    for t in 0..n {
        let (_c, mut poly, _orth) = gen_closed_pair(r);
        let k = r.rnd(poly.size() as u32) as usize;
        let a = poly.ps[k];
        let b = poly.ps[(k + 1) % poly.size()];
        let mut c = vec![Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)];
        let cn = 1 + r.rnd(4);
        for _ in 0..cn {
            let mut q = r.grid_pt(7);
            if q == *c.last().unwrap() {
                q.x += 1.0;
            }
            c.push(q);
        }
        for p in c.iter_mut() {
            p.id = r.rnd(4);
            p.vn = r.rnd(8) as u16;
        }
        let mut conn = to_poly(c);
        let mut tolerance = 0.0;
        if r.rnd(2) == 0 {
            tolerance = f64::EPSILON;
        }
        writeln!(o, "case {} tol={}", t, g17(tolerance)).unwrap();
        writeln!(o, "  poly {}", poly_str(&poly)).unwrap();
        writeln!(o, "  conn {}", poly_str(&conn)).unwrap();
        split_branching_segments(&mut poly, false, &mut conn, tolerance);
        writeln!(o, "  poly' {}", poly_str(&poly)).unwrap();
        writeln!(o, "  conn' {}", poly_str(&conn)).unwrap();
    }
}

fn random_end(r: &mut Rng, router: &Router, j1: JunctionRef, j2: JunctionRef) -> ConnEnd {
    let k = r.rnd(3);
    if k == 1 {
        return ConnEnd::from_junction(router, j1);
    }
    if k == 2 {
        return ConnEnd::from_junction(router, j2);
    }
    let x = r.rnd(50) as f64;
    let y = r.rnd(50) as f64;
    ConnEnd::from_point(Point::new(x + 300.0, y + 300.0))
}

fn section_crossings(o: &mut String, r: &mut Rng, n: u32) {
    o.push_str("### crossings\n");
    let mut null_router = Router::new(ORTHOGONAL_ROUTING);
    for t in 0..n {
        let kr = r.rnd(10);
        let kind = if kr < 7 {
            0
        } else if kr < 8 {
            1
        } else {
            2
        };
        let (mut conn, mut poly);
        let poly_is_conn = kind != 2;
        let orth;
        if kind == 2 {
            let (c, p, ot) = gen_closed_pair(r);
            conn = c;
            poly = p;
            orth = ot;
        } else {
            orth = r.rnd(4) != 0;
            (conn, poly) = gen_conn_pair(r, orth);
        }

        let mut own_router: Option<Router> = None;
        let mut c1: Option<ConnIdx> = None;
        let mut c2: Option<ConnIdx> = None;
        let mut setup = String::new();
        if kind == 0 {
            let mut router = Router::new(ORTHOGONAL_ROUTING);
            let j1 = JunctionRef::new(&mut router, Point::new(100.0, 100.0), 10);
            let j2 = JunctionRef::new(&mut router, Point::new(200.0, 200.0), 11);
            let e1 = random_end(r, &router, j1, j2);
            let e2 = random_end(r, &router, j1, j2);
            let a = ConnRef::new_with_ends(&mut router, &e1, &e2, 1);
            let e3 = random_end(r, &router, j1, j2);
            let e4 = random_end(r, &router, j1, j2);
            let b = ConnRef::new_with_ends(&mut router, &e3, &e4, 2);
            router.process_actions();
            let cs = [a, b];
            for k in 0..2 {
                let mut is_poly = r.rnd(5) == 0;
                if !orth && r.rnd(2) == 0 {
                    is_poly = true;
                }
                router.conns[cs[k]].m_type = if is_poly {
                    ConnType::ConnTypePolyLine
                } else {
                    ConnType::ConnTypeOrthogonal
                };
                router.conns[cs[k]].m_has_fixed_route = r.rnd(6) == 0;
                let ends = router.conns[cs[k]].endpoint_conn_ends(&router);
                write!(
                    setup,
                    " c{}:{}{}[{},{}]",
                    k + 1,
                    if is_poly { "poly" } else { "orth" },
                    if router.conns[cs[k]].m_has_fixed_route { ",fixed" } else { "" },
                    if ends.0.junction(&router).is_some() { "J" } else { "p" },
                    if ends.1.junction(&router).is_some() { "J" } else { "p" }
                )
                .unwrap();
            }
            c1 = Some(a);
            c2 = Some(b);
            own_router = Some(router);
        }
        let router: &mut Router = own_router.as_mut().unwrap_or(&mut null_router);

        let check_branching = r.rnd(3) == 0;
        let pre_split = poly_is_conn && (r.rnd(2) == 0);
        let want_points = r.rnd(4) != 0;
        let want_shared = r.rnd(2) == 0;
        let want_orders = r.rnd(4) != 0;

        writeln!(
            o,
            "case {} kind={} orth={} cb={} pre={}{}",
            t, kind, orth as i32, check_branching as i32, pre_split as i32, setup
        )
        .unwrap();
        writeln!(o, "  poly {}", poly_str(&poly)).unwrap();
        writeln!(o, "  conn {}", poly_str(&conn)).unwrap();
        if pre_split {
            split_branching_segments(&mut poly, true, &mut conn, 0.0);
            split_branching_segments(&mut conn, true, &mut poly, 0.0);
            writeln!(o, "  pre poly {}", poly_str(&poly)).unwrap();
            writeln!(o, "  pre conn {}", poly_str(&conn)).unwrap();
        }

        let poly_is_orthogonal = c1.is_some_and(|c| router.conns[c].routing_type() == ConnType::ConnTypeOrthogonal);
        let conn_is_orthogonal = c2.is_some_and(|c| router.conns[c].routing_type() == ConnType::ConnTypeOrthogonal);
        let will_split = check_branching
            || c1.is_some_and(|c| router.conns[c].has_fixed_route())
            || c2.is_some_and(|c| router.conns[c].has_fixed_route())
            || !poly_is_orthogonal
            || !conn_is_orthogonal;

        let mut crossing_points = PointSet::new();
        let mut shared_paths = SharedPathList::new();
        let mut point_orders = PtOrderMap::new();
        let mut total = 0u32;
        let mut flags = 0u32;
        let mut skipped = false;
        {
            let mut cross = ConnectorCrossings::new(poly_is_conn, c1, c2);
            cross.check_for_branching_segments = check_branching;
            if want_points {
                cross.crossing_points = Some(&mut crossing_points);
            }
            if want_shared {
                cross.shared_paths = Some(&mut shared_paths);
            }
            if want_orders {
                cross.point_orders = Some(&mut point_orders);
            }
            let mut i = 1;
            while i < conn.size() {
                if !poly_is_conn && would_overflow(&poly, poly_is_conn, &conn, will_split) {
                    o.push_str("  skip-overflow\n");
                    skipped = true;
                    break;
                }
                let final_segment = (i + 1) == conn.size();
                cross.count_for_segment(router, &mut poly, &mut conn, i, final_segment);
                writeln!(
                    o,
                    "  seg {} count={} flags={} first={} second={}",
                    i,
                    cross.crossing_count,
                    cross.crossing_flags,
                    g17(cross.first_shared_path_at_end_length),
                    g17(cross.second_shared_path_at_end_length)
                )
                .unwrap();
                total += cross.crossing_count;
                flags |= cross.crossing_flags;
                i += 1;
            }
        }
        if !skipped {
            writeln!(o, "  total={} flags={}", total, flags).unwrap();
            writeln!(o, "  poly' {}", poly_str(&poly)).unwrap();
            writeln!(o, "  conn' {}", poly_str(&conn)).unwrap();
            if want_points {
                let mut s = String::new();
                for p in crossing_points.iter() {
                    s.push(' ');
                    s.push_str(&pt_str(&p.0));
                }
                writeln!(o, "  points{}", s).unwrap();
            }
            if want_shared {
                writeln!(o, "  shared {}", shared_paths.len()).unwrap();
                for path in shared_paths.iter() {
                    let mut s = String::new();
                    for p in path.iter() {
                        s.push(' ');
                        s.push_str(&pt_str(p));
                    }
                    writeln!(o, "   path{}", s).unwrap();
                }
            }
            if want_orders {
                dump_point_orders(o, router, &mut point_orders, !will_split, c1, c2);
            }
        }
        if let Some(mut router) = own_router {
            router.destruct();
        }
    }
    null_router.destruct();
}

fn sorted_str(router: &Router, v: &PointRepVector) -> String {
    let mut s = String::new();
    for pr in v.iter() {
        write!(s, " P{}:{}", pr.first.index, conn_name(router, pr.second)).unwrap();
    }
    s
}

fn section_pt_order(o: &mut String, r: &mut Rng, n: u32) {
    o.push_str("### ptorder\n");
    let mut router = Router::new(ORTHOGONAL_ROUTING);
    let mut conns = Vec::new();
    for i in 0..8 {
        conns.push(ConnRef::new(&mut router, 20 + i));
    }
    let pt = |i: u32| PtRef {
        route: PtRefRoute::Poly,
        index: i as usize,
        point: Point::default(),
    };
    for t in 0..n {
        let mut ord = PtOrder::new();
        let nconn = 2 + r.rnd(6);
        let ops = 1 + r.rnd(14);
        writeln!(o, "case {} nconn={} ops={}", t, nconn, ops).unwrap();
        let pick = |ci: u32| if ci == nconn { None } else { Some(conns[ci as usize]) };
        for _ in 0..ops {
            let kind = r.rnd(12);
            let dim = r.rnd(2) as usize;
            if kind < 9 {
                let pi = r.rnd(16);
                let pj = (pi + 1 + r.rnd(15)) % 16;
                let ci = r.rnd(nconn + 1);
                let cj = r.rnd(nconn + 1);
                let a = pick(ci);
                let b = pick(cj);
                if kind < 2 {
                    writeln!(
                        o,
                        "  add {} P{}:{} P{}:{}",
                        dim,
                        pi,
                        conn_name(&router, a),
                        pj,
                        conn_name(&router, b)
                    )
                    .unwrap();
                    ord.add_points(dim, &PtConnPtrPair::new(pt(pi), a), &PtConnPtrPair::new(pt(pj), b));
                } else {
                    let swapped = r.rnd(2) == 0;
                    writeln!(
                        o,
                        "  ord {} P{}:{} P{}:{} {}",
                        dim,
                        pi,
                        conn_name(&router, a),
                        pj,
                        conn_name(&router, b),
                        swapped as i32
                    )
                    .unwrap();
                    ord.add_ordered_points(
                        dim,
                        &PtConnPtrPair::new(pt(pi), a),
                        &PtConnPtrPair::new(pt(pj), b),
                        swapped,
                    );
                }
            } else if kind < 11 {
                let v = ord.sorted_points(dim);
                writeln!(o, "  sorted {} n={}{}", dim, v.len(), sorted_str(&router, &v)).unwrap();
            } else {
                let ci = r.rnd(nconn + 1);
                let a = pick(ci);
                let p = ord.position_for(dim, a);
                writeln!(o, "  pos {} {} = {}", dim, conn_name(&router, a), p).unwrap();
            }
        }
        for dim in 0..2 {
            let v = ord.sorted_points(dim);
            let mut p = String::new();
            for ci in 0..=nconn {
                write!(p, " {}", ord.position_for(dim, pick(ci))).unwrap();
            }
            writeln!(o, "  final {} n={}{} |{}", dim, v.len(), sorted_str(&router, &v), p).unwrap();
        }
    }
    router.destruct();
}

fn run_oracle() -> String {
    let mut r = Rng(0x2545f4914f6cdd1d);
    let mut o = String::new();
    section_split(&mut o, &mut r, 1500);
    section_split_vn(&mut o, &mut r, 1500);
    section_split_closed(&mut o, &mut r, 1500);
    section_crossings(&mut o, &mut r, 6000);
    section_pt_order(&mut o, &mut r, 1500);
    o
}

#[test]
fn crossings_oracle_matches_golden() {
    let golden = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/crossings_oracle.txt"))
        .expect("golden file");
    let ours = run_oracle();
    if ours != golden {
        if let Ok(path) = std::env::var("CROSSINGS_ORACLE_DUMP") {
            std::fs::write(path, &ours).unwrap();
        }
        let gl: Vec<&str> = golden.lines().collect();
        let ol: Vec<&str> = ours.lines().collect();
        for i in 0..gl.len().max(ol.len()) {
            let g = gl.get(i).copied().unwrap_or("<eof>");
            let m = ol.get(i).copied().unwrap_or("<eof>");
            if g != m {
                // Show the case header for context.
                let mut h = i;
                while h > 0 && !gl[h].starts_with("case") && !gl[h].starts_with("###") {
                    h -= 1;
                }
                panic!(
                    "crossings oracle mismatch at line {} (in {}):\n  golden: {}\n  ours:   {}",
                    i + 1,
                    gl.get(h).copied().unwrap_or(""),
                    g,
                    m
                );
            }
        }
    }
}
