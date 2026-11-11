//! Differential test against the C++ oracle `tools/oracle/geometry_oracle.cpp`
//! (golden output in `tests/golden/geometry_oracle.txt`).
//!
//! The inputs are regenerated here with the same LCG in the same order, the
//! same text is produced, and compared line by line.  Every value computed
//! only with + - * / sqrt must match bit-for-bit (`%.17g` text equality).
//! Values computed with `acos`/`atan` (`ang`, `ra` fields) are compared with
//! a 1e-12 tolerance in case the platform libm differs (with glibc they are
//! in practice also identical).

#![allow(clippy::uninlined_format_args, clippy::manual_range_contains)]

use libavoid_rust::avoid::geometry::*;
use libavoid_rust::avoid::geomtypes::*;
use libavoid_rust::avoid::graph::{cmp_vis_edge_rotation, EdgeInf, EdgeList, EdgeListId};
use libavoid_rust::avoid::router::{Router, ORTHOGONAL_ROUTING, POLY_LINE_ROUTING};
use libavoid_rust::avoid::vertices::{direct_vis, VertEdgeList, VertID, VertInf};

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
    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / 9007199254740992.0)
    }
    fn coord(&mut self) -> f64 {
        let mode = self.rnd(4);
        if mode == 0 {
            return self.rnd(5) as f64;
        } else if mode == 1 {
            return self.rnd(9) as f64 / 2.0;
        } else if mode == 2 {
            return self.unit() * 20.0 - 10.0;
        }
        self.rnd(7) as f64 - 3.0
    }
    fn point(&mut self) -> Point {
        let x = self.coord();
        let y = self.coord();
        Point::new(x, y)
    }
}

/// C `printf("%.17g")`, except that every NaN prints as "nan" (as in the
/// oracle).
fn g17(v: f64) -> String {
    if v.is_nan() {
        return "nan".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf".into() } else { "-inf".into() };
    }
    if v == 0.0 {
        return if v.is_sign_negative() { "-0".into() } else { "0".into() };
    }
    const P: i32 = 17;
    let e = format!("{:.*e}", (P - 1) as usize, v);
    let (mant, exp) = e.split_once('e').unwrap();
    let x: i32 = exp.parse().unwrap();
    if x < P && x >= -4 {
        let s = format!("{:.*}", (P - 1 - x) as usize, v);
        strip_zeros(&s)
    } else {
        let m = strip_zeros(mant);
        let sign = if x < 0 { '-' } else { '+' };
        format!("{}e{}{:02}", m, sign, x.abs())
    }
}

fn strip_zeros(s: &str) -> String {
    if s.contains('.') {
        let t = s.trim_end_matches('0');
        t.trim_end_matches('.').to_string()
    } else {
        s.to_string()
    }
}

struct Out(String);

impl Out {
    fn s(&mut self, t: &str) {
        self.0.push_str(t);
    }
    fn pd(&mut self, v: f64) {
        let _ = write!(self.0, " {}", g17(v));
    }
    fn pp(&mut self, p: &Point) {
        let _ = write!(self.0, " ({},{})", g17(p.x), g17(p.y));
    }
    fn pp_full(&mut self, p: &Point) {
        let _ = write!(self.0, " ({},{},{},{})", g17(p.x), g17(p.y), p.id, p.vn);
    }
    fn print_poly(&mut self, poly: &Polygon) {
        let _ = write!(self.0, " id={} n={}", poly._id, poly.size());
        for p in &poly.ps {
            self.pp_full(p);
        }
        self.s(" ts=");
        for c in &poly.ts {
            self.0.push(*c);
        }
        let _ = write!(self.0, " cps={}", poly.checkpoints_on_route.len());
        for cp in &poly.checkpoints_on_route {
            let _ = write!(self.0, " [{}", cp.0 as u32);
            self.pp(&cp.1);
            self.s("]");
        }
    }
}

fn b(v: bool) -> i32 {
    v as i32
}

fn random_poly(rng: &mut Rng, min_n: u32, max_n: u32) -> Polygon {
    let n = min_n + rng.rnd(max_n - min_n + 1);
    let mut poly = Polygon::new();
    poly._id = rng.rnd(100) as i32;
    for _ in 0..n {
        let p = rng.point();
        poly.ps.push(p);
    }
    poly
}

const N: i32 = 500;

fn generate() -> String {
    let mut rng = Rng(0x853c49e6748fea9b);
    let mut o = Out(String::new());

    // ---------------- hand-picked degenerate points ----------------
    let special = [
        Point::new(0.0, 0.0),
        Point::new(1.0, 0.0),
        Point::new(0.0, 1.0),
        Point::new(1.0, 1.0),
        Point::new(2.0, 2.0),
        Point::new(-1.0, -1.0),
        Point::new(2.0, 0.0),
        Point::new(0.5, 0.5),
        Point::new(1e-300, 0.0),
        Point::new(1e300, -1e300),
        Point::new(-0.0, 0.0),
        Point::new(0.1, 0.2),
        Point::new(0.3, 0.6),
    ];
    let s = special.len();
    let eps = f64::EPSILON;
    for i in 0..s {
        for j in 0..s {
            for k in 0..s {
                let (a, bb, c) = (&special[i], &special[j], &special[k]);
                let _ = write!(o.0, "S3 {} {} {}", i, j, k);
                let _ = write!(o.0, " vd={} vdz={}", vec_dir(a, bb, c, 0.0), vec_dir(a, bb, c, 0.5));
                let _ = write!(
                    o.0,
                    " col={} colt={}",
                    b(colinear(a, bb, c, 0.0)),
                    b(colinear(a, bb, c, 0.5))
                );
                let _ = write!(o.0, " pol={}", b(point_on_line(a, bb, c, 0.0)));
                if vec_dir(a, bb, c, eps) == 0 {
                    let _ = write!(o.0, " ib={}", b(in_between(a, bb, c)));
                }
                let _ = write!(o.0, " cs={}", corner_side(a, bb, c, &special[(i + j + k) % s]));
                o.s(" proj");
                o.pp(&projection(a, bb, c));
                o.s(" ang");
                o.pd(angle(a, bb, c));
                o.s("\n");
            }
        }
    }

    // ---------------- random point-level functions ----------------
    for it in 0..N {
        let a = rng.point();
        let bb = rng.point();
        let c = rng.point();
        let d = rng.point();
        let _ = write!(o.0, "P {}", it);
        o.pp(&a);
        o.pp(&bb);
        o.pp(&c);
        o.pp(&d);
        let _ = write!(o.0, " vd={} vdz={}", vec_dir(&a, &bb, &c, 0.0), vec_dir(&a, &bb, &c, 0.25));
        let _ = write!(
            o.0,
            " col={} colt={}",
            b(colinear(&a, &bb, &c, 0.0)),
            b(colinear(&a, &bb, &c, 0.25))
        );
        let _ = write!(o.0, " pol={}", b(point_on_line(&a, &bb, &c, 0.0)));
        let general_case = (a.x != bb.x) && (a.y != bb.y);
        if !general_case || (vec_dir(&a, &bb, &c, 0.25) != 0) || (vec_dir(&a, &bb, &c, eps) == 0) {
            let _ = write!(o.0, " polt={}", b(point_on_line(&a, &bb, &c, 0.25)));
        }
        if vec_dir(&a, &bb, &c, eps) == 0 {
            let _ = write!(o.0, " ib={}", b(in_between(&a, &bb, &c)));
        }
        let _ = write!(o.0, " si={}", b(segment_intersect(&a, &bb, &c, &d)));
        let mut seen = false;
        let r1 = segment_shape_intersect(&a, &bb, &c, &d, &mut seen);
        let _ = write!(o.0, " ssi={},{}", b(r1), b(seen));
        seen = true;
        let r2 = segment_shape_intersect(&a, &bb, &c, &d, &mut seen);
        let _ = write!(o.0, " ssi2={},{}", b(r2), b(seen));
        let _ = write!(
            o.0,
            " ivr={},{}",
            b(in_valid_region(true, &a, &bb, &c, &d)),
            b(in_valid_region(false, &a, &bb, &c, &d))
        );
        let _ = write!(o.0, " cs={}", corner_side(&a, &bb, &c, &d));
        o.s(" ed");
        o.pd(euclidean_dist(&a, &d));
        o.s(" md");
        o.pd(manhattan_dist(&a, &d));
        o.s(" ang");
        o.pd(angle(&a, &bb, &c));
        o.s(" proj");
        o.pp(&projection(&a, &bb, &c));
        let (mut x, mut y) = (-12345.0, -12345.0);
        let sip = segment_intersect_point(&a, &bb, &c, &d, &mut x, &mut y);
        let _ = write!(o.0, " sip={}", sip);
        o.pd(x);
        o.pd(y);
        let (mut x, mut y) = (-12345.0, -12345.0);
        let rip = ray_intersect_point(&a, &bb, &c, &d, &mut x, &mut y);
        let _ = write!(o.0, " rip={}", rip);
        o.pd(x);
        o.pd(y);
        o.s(" ra");
        o.pd(rotational_angle(&(a - d)));
        let _ = write!(
            o.0,
            " peq={},{},{},{},{}",
            b(a == bb),
            b(a != bb),
            b(a.less_than(&bb)),
            b(a.equals_default(&bb)),
            b(a.equals(&bb, 0.5))
        );
        let sum = a + bb;
        let diff = a - bb;
        o.s(" sum");
        o.pp_full(&sum);
        o.s(" diff");
        o.pp_full(&diff);
        o.s(" idx");
        o.pd(a[0]);
        o.pd(a[1]);
        o.s("\n");
    }

    // ---------------- polygon-level functions ----------------
    for it in 0..N {
        let mut poly = random_poly(&mut rng, 0, 7);
        let ncp = rng.rnd(4);
        for _ in 0..ncp {
            let max_val = if poly.size() > 0 { 2 * poly.size() - 1 } else { 1 };
            let v = rng.rnd(max_val as u32) as usize;
            let cp = rng.point();
            poly.checkpoints_on_route.push((v, cp));
        }
        let q = rng.point();
        let offset = rng.coord();
        let curve = rng.unit() * 3.0;

        let _ = write!(o.0, "G {}", it);
        o.print_poly(&poly);
        o.s(" q");
        o.pp(&q);
        o.s(" off");
        o.pd(offset);
        o.s(" curve");
        o.pd(curve);
        o.s("\n");

        o.s("  tl");
        o.pd(total_length(&poly));
        o.s("\n");
        let _ = write!(o.0, "  ipg={}", b(in_poly_gen(&poly, &q)));
        if poly.size() > 0 {
            let _ = write!(o.0, " ip={},{}", b(in_poly(&poly, &q, true)), b(in_poly(&poly, &q, false)));
        }
        o.s("\n");
        if poly.size() > 0 {
            let bbx = poly.offset_bounding_box(offset);
            o.s("  obb");
            o.pp(&bbx.min);
            o.pp(&bbx.max);
            o.pd(bbx.width());
            o.pd(bbx.height());
            o.pd(bbx.length(0));
            o.pd(bbx.length(1));
            o.s("\n");
            o.s("  brp");
            o.print_poly(&poly.bounding_rect_polygon());
            o.s("\n");
        }
        if poly.size() > 0 {
            o.s("  op");
            o.print_poly(&poly.offset_polygon(offset));
            o.s("\n");
            o.s("  op0");
            o.print_poly(&poly.offset_polygon(0.0));
            o.s("\n");
        }
        o.s("  simp");
        o.print_poly(&poly.simplify());
        o.s("\n");
        if poly.size() > 0 {
            o.s("  cpl");
            o.print_poly(&poly.curved_polyline(curve, false));
            o.s("\n");
        }
        if poly.size() > 2 {
            o.s("  cplc");
            o.print_poly(&poly.curved_polyline(curve, true));
            o.s("\n");
        }
        let mut seg = 0;
        while seg + 1 < poly.size() {
            for m in -1..=1 {
                let cps = poly.checkpoints_on_segment(seg, m);
                let _ = write!(o.0, "  cos {} {} n={}", seg, m, cps.len());
                for cp in &cps {
                    o.pp(cp);
                }
                o.s("\n");
            }
            seg += 1;
        }
        let mut copy = poly.clone();
        copy.translate(offset, curve);
        o.s("  tr");
        o.print_poly(&copy);
        o.s("\n");
        let from_iface = Polygon::from_interface(&poly);
        o.s("  fi");
        o.print_poly(&from_iface);
        o.s("\n");

        let p1 = rng.point();
        let p2 = rng.point();
        let w = rng.coord();
        let h = rng.coord();
        o.s("  rect");
        o.print_poly(&Rectangle::new(&p1, &p2));
        o.s("\n");
        o.s("  rectc");
        o.print_poly(&Rectangle::from_centre(&p1, w, h));
        o.s("\n");
    }

    // ---------------- EdgeInf::rotationLessThan + list sort ----------------
    {
        let mut router = Router::new(ORTHOGONAL_ROUTING);
        for it in 0..300 {
            let cx = rng.rnd(5) as f64;
            let cy = rng.rnd(5) as f64;
            let centre = VertInf::new(&mut router, VertID::new(1, 0, 0), Point::new(cx, cy), false);
            let n = 2 + rng.rnd(7);
            let mut others = Vec::new();
            let mut edges = Vec::new();
            for i in 0..n {
                let dir = rng.rnd(5);
                let dist = (1 + rng.rnd(3)) as f64;
                let mut p = Point::new(cx, cy);
                if dir == 0 {
                    p.x += dist;
                } else if dir == 1 {
                    p.x -= dist;
                } else if dir == 2 {
                    p.y += dist;
                } else if dir == 3 {
                    p.y -= dist;
                } else {
                    p.x += dist;
                    p.y += 1.0;
                }
                let ov = VertInf::new(&mut router, VertID::new(2, i as u16, 0), p, false);
                others.push(ov);
                let swap_ends = rng.rnd(2) == 1;
                let e = if swap_ends {
                    EdgeInf::new(&mut router, ov, centre, true)
                } else {
                    EdgeInf::new(&mut router, centre, ov, true)
                };
                edges.push(e);
            }
            let mut last = None;
            let last_mode = rng.rnd(3);
            if last_mode != 0 {
                let mut lp = rng.point();
                if last_mode == 1 {
                    if rng.rnd(2) == 0 {
                        lp.y = cy;
                    } else {
                        lp.x = cx;
                    }
                }
                last = Some(VertInf::new(&mut router, VertID::new(3, 0, 0), lp, false));
            }
            let _ = write!(o.0, "R {}", it);
            o.pp_full(&router.vert_infs[centre].point);
            for &ov in &others {
                o.pp(&router.vert_infs[ov].point);
            }
            match last {
                Some(l) => o.pp(&router.vert_infs[l].point),
                None => o.s(" null"),
            }
            o.s("\n  ");
            for &ei in &edges {
                for &ej in &edges {
                    let r = router.edge_infs[ei].rotation_less_than(
                        &router.vert_infs,
                        last,
                        &router.edge_infs[ej],
                    );
                    let _ = write!(o.0, "{}", b(r));
                }
                o.s(" ");
            }
            o.s("\n");

            for (i, &e) in edges.iter().enumerate() {
                if router.edge_infs[e].is_orthogonal(&router.vert_infs) {
                    EdgeInf::set_dist(&mut router, e, 1.0 + i as f64);
                }
            }
            let index_of = |e| edges.iter().position(|&x| x == e).unwrap();
            o.s("  pre");
            for e in router.vert_infs[centre]
                .orthog_vis_list
                .iter(&router.edge_infs)
            {
                let _ = write!(o.0, " {}", index_of(e));
            }
            VertInf::sort_edge_list(&mut router, centre, VertEdgeList::OrthogVis, |r, u, v| {
                cmp_vis_edge_rotation(r, last, u, v)
            });
            o.s(" sorted");
            for e in router.vert_infs[centre]
                .orthog_vis_list
                .iter(&router.edge_infs)
            {
                let _ = write!(o.0, " {}", index_of(e));
            }
            let _ = writeln!(o.0, " graph={}", router.vis_orthog_graph.size());

            for &e in &edges {
                EdgeInf::delete(&mut router, e);
            }
            for &ov in &others {
                VertInf::delete(&mut router, ov);
            }
            VertInf::delete(&mut router, centre);
            if let Some(l) = last {
                VertInf::delete(&mut router, l);
            }
        }
        assert_eq!(router.vis_orthog_graph.size(), 0);
        assert!(router.vert_infs.is_empty());
        assert!(router.edge_infs.is_empty());
    }

    // ------- poly-line visibility: checkEdgeVisibility / directVis -------
    for it in 0..80 {
        let mut router = Router::new(POLY_LINE_ROUTING);
        router.ignore_regions = rng.rnd(2) == 0;
        router.invisibility_grph = rng.rnd(3) != 0;
        let n_shapes = 1 + rng.rnd(4);
        let mut all = Vec::new();
        let mut rects = Vec::new();
        let _ = write!(
            o.0,
            "V {} ir={} ig={}",
            it,
            b(router.ignore_regions),
            b(router.invisibility_grph)
        );
        for s in 0..n_shapes {
            let p1 = rng.point();
            let p2 = rng.point();
            let r = Rectangle::new(&p1, &p2);
            let id = 10 + s;
            let mut first = None;
            let mut last = None;
            for k in 0..r.size() {
                let v = VertInf::new(&mut router, VertID::new(id, k as u16, 0), r.ps[k], true);
                all.push(v);
                if first.is_none() {
                    first = Some(v);
                } else {
                    router.vert_infs[v].sh_prev = last;
                    router.vert_infs[last.unwrap()].sh_next = Some(v);
                }
                last = Some(v);
            }
            router.vert_infs[last.unwrap()].sh_next = first;
            router.vert_infs[first.unwrap()].sh_prev = last;
            let _ = write!(o.0, " shape{}", id);
            o.pp(&r.ps[0]);
            o.pp(&r.ps[2]);
            rects.push(r);
        }
        let n_conns = 1 + rng.rnd(3);
        for c in 0..n_conns {
            let p = rng.point();
            let vid = VertID::new(100 + c, VertID::SRC, VertID::PROP_CONN_POINT);
            let v = VertInf::new(&mut router, vid, p, true);
            all.push(v);
            for s in 0..n_shapes {
                if in_poly(&rects[s as usize], &p, false) {
                    router.contains.entry(vid).or_default().insert(10 + s);
                }
            }
            let _ = write!(o.0, " conn{}", 100 + c);
            o.pp(&p);
        }
        o.s("\n");

        let vid_str = |r: &Router, v| {
            let id = r.vert_infs[v].id;
            format!("{}:{}", id.obj_id, id.vn)
        };
        for pass in 0..2 {
            let mut i = router.vertices.conns_begin();
            while let Some(iv) = i {
                let mut j = router.vert_infs[iv].lst_next;
                while let Some(jv) = j {
                    let e = EdgeInf::check_edge_visibility(&mut router, iv, jv, false);
                    let _ = write!(
                        o.0,
                        "  ce{} {} {}",
                        pass,
                        vid_str(&router, iv),
                        vid_str(&router, jv)
                    );
                    match e {
                        Some(e) => {
                            let ed = &router.edge_infs[e];
                            let _ = write!(o.0, " added={} blocker={} dist", b(ed.added()), ed.blocker());
                            let d = ed.get_dist();
                            o.pd(d);
                        }
                        None => o.s(" null"),
                    }
                    let dv = direct_vis(&mut router, iv, jv);
                    let _ = writeln!(o.0, " dv={}", b(dv));
                    j = router.vert_infs[jv].lst_next;
                }
                i = router.vert_infs[iv].lst_next;
            }
        }
        let _ = writeln!(
            o.0,
            "  checked={} vis={} invis={}",
            router.st_checked_edges,
            router.vis_graph.size(),
            router.invis_graph.size()
        );
        for iv in router.vertices.to_vec(&router.vert_infs) {
            let vi = &router.vert_infs[iv];
            let _ = write!(
                o.0,
                "  vl {} n={},{}:",
                vid_str(&router, iv),
                vi.vis_list_size,
                vi.invis_list_size
            );
            for e in vi.vis_list.iter(&router.edge_infs) {
                let ov = router.edge_infs[e].other_vert(iv);
                let _ = write!(o.0, " {}", vid_str(&router, ov));
            }
            o.s(" |");
            for e in vi.invis_list.iter(&router.edge_infs) {
                let ov = router.edge_infs[e].other_vert(iv);
                let _ = write!(o.0, " {}", vid_str(&router, ov));
            }
            o.s("\n");
        }
        o.s("  vg");
        for e in router.vis_graph.to_vec(&router.edge_infs) {
            let (a, c) = router.edge_infs[e].ids(&router.vert_infs);
            let _ = write!(o.0, " {}:{}-{}:{}", a.obj_id, a.vn, c.obj_id, c.vn);
        }
        o.s("\n  ig");
        for e in router.invis_graph.to_vec(&router.edge_infs) {
            let (a, c) = router.edge_infs[e].ids(&router.vert_infs);
            let _ = write!(
                o.0,
                " {}:{}-{}:{}/{}",
                a.obj_id,
                a.vn,
                c.obj_id,
                c.vn,
                router.edge_infs[e].blocker()
            );
        }
        o.s("\n");

        let first_conn = router.vertices.conns_begin().unwrap();
        VertInf::remove_from_graph(&mut router, first_conn, true);
        let first_shape = router.vertices.shapes_begin().unwrap();
        VertInf::orphan(&mut router, first_shape);
        let _ = writeln!(
            o.0,
            "  after vis={} invis={} orphaned={},{}",
            router.vis_graph.size(),
            router.invis_graph.size(),
            b(router.vert_infs[first_conn].orphaned()),
            b(router.vert_infs[first_shape].orphaned())
        );

        EdgeList::clear(&mut router, EdgeListId::VisGraph);
        EdgeList::clear(&mut router, EdgeListId::InvisGraph);
        for &v in &all {
            router.vertices.remove_vertex(&mut router.vert_infs, Some(v));
            VertInf::delete(&mut router, v);
        }
        assert!(router.vert_infs.is_empty());
        // Edges made inactive by orphan() are not in any list and are leaked
        // (as they are in the C++ driver).
        assert!(router.vis_graph.size() == 0 && router.invis_graph.size() == 0);
    }
    o.0
}

/// Fields (by prefix) computed with transcendental functions.
fn tolerant_field(tok_prev: Option<&str>) -> bool {
    matches!(tok_prev, Some("ang") | Some("ra"))
}

#[test]
fn geometry_matches_cpp_oracle() {
    let golden = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/golden/geometry_oracle.txt"
    ))
    .expect("golden file missing; see tools/oracle/geometry_oracle.cpp");
    let ours = generate();

    let g_lines: Vec<&str> = golden.lines().collect();
    let o_lines: Vec<&str> = ours.lines().collect();
    assert_eq!(g_lines.len(), o_lines.len(), "line count differs");

    let mut exact_mismatch = 0;
    let mut tolerant_used = 0;
    for (ln, (g, r)) in g_lines.iter().zip(o_lines.iter()).enumerate() {
        if g == r {
            continue;
        }
        let gt: Vec<&str> = g.split(' ').collect();
        let rt: Vec<&str> = r.split(' ').collect();
        assert_eq!(gt.len(), rt.len(), "line {}:\n C++: {}\nRust: {}", ln + 1, g, r);
        for t in 0..gt.len() {
            if gt[t] == rt[t] {
                continue;
            }
            let prev = if t > 0 { Some(gt[t - 1]) } else { None };
            if tolerant_field(prev) {
                let gv: f64 = gt[t].parse().unwrap();
                let rv: f64 = rt[t].parse().unwrap();
                assert!(
                    (gv - rv).abs() <= 1e-12 * gv.abs().max(1.0),
                    "line {} token {}: {} vs {}",
                    ln + 1,
                    t,
                    gt[t],
                    rt[t]
                );
                tolerant_used += 1;
            } else {
                exact_mismatch += 1;
                eprintln!("line {}:\n C++: {}\nRust: {}", ln + 1, g, r);
                break;
            }
        }
        if exact_mismatch > 10 {
            break;
        }
    }
    assert_eq!(exact_mismatch, 0, "exact mismatches against the C++ oracle");
    eprintln!("tolerance-compared transcendental values: {}", tolerant_used);
}
