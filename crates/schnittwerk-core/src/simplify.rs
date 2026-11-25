//! Path simplification: fewer nodes, same shape.
//!
//! Every subpath is split at its corners (nodes where the direction jumps),
//! each run between corners is flattened and refitted with as few cubic
//! Béziers as the tolerance allows (Schneider's fitter from `contour`).
//! Straight runs become single lines.

use kurbo::{flatten, BezPath, ParamCurve, PathEl, PathSeg, Point, Vec2};
use serde::Serialize;

use crate::contour::{fit_cubic, max_chord_dist, unit};
use crate::geom::to_svg_d;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimplifyResult {
    pub d: String,
    pub nodes_before: usize,
    pub nodes_after: usize,
}

/// A subpath as its segments.
struct Sub {
    segs: Vec<PathSeg>,
    closed: bool,
}

fn subpaths(path: &BezPath) -> Vec<Sub> {
    let mut out: Vec<Sub> = Vec::new();
    let mut cur: Vec<PathSeg> = Vec::new();
    let mut closed = false;
    let flush = |cur: &mut Vec<PathSeg>, closed: bool, out: &mut Vec<Sub>| {
        if !cur.is_empty() {
            out.push(Sub { segs: std::mem::take(cur), closed });
        }
    };
    // Built from the elements: `segments()` loses the subpath boundaries.
    let mut start = Point::ZERO;
    let mut last = Point::ZERO;
    for el in path.elements() {
        match *el {
            PathEl::MoveTo(p) => {
                flush(&mut cur, closed, &mut out);
                closed = false;
                start = p;
                last = p;
            }
            PathEl::LineTo(p) => {
                cur.push(PathSeg::Line(kurbo::Line::new(last, p)));
                last = p;
            }
            PathEl::QuadTo(a, p) => {
                cur.push(PathSeg::Quad(kurbo::QuadBez::new(last, a, p)));
                last = p;
            }
            PathEl::CurveTo(a, b, p) => {
                cur.push(PathSeg::Cubic(kurbo::CubicBez::new(last, a, b, p)));
                last = p;
            }
            PathEl::ClosePath => {
                if (last - start).hypot() > 1e-9 {
                    cur.push(PathSeg::Line(kurbo::Line::new(last, start)));
                }
                closed = true;
                flush(&mut cur, closed, &mut out);
                closed = false;
                last = start;
            }
        }
    }
    flush(&mut cur, closed, &mut out);
    out
}

/// Control polygon of a segment (lines and quads as degenerate cubics are
/// not needed: the first / last distinct point gives the tangent).
fn ctrl(s: &PathSeg) -> Vec<Point> {
    match *s {
        PathSeg::Line(l) => vec![l.p0, l.p1],
        PathSeg::Quad(q) => vec![q.p0, q.p1, q.p2],
        PathSeg::Cubic(c) => vec![c.p0, c.p1, c.p2, c.p3],
    }
}

fn tangent_start(s: &PathSeg) -> Vec2 {
    let c = ctrl(s);
    c[1..].iter().find(|p| (**p - c[0]).hypot() > 1e-9).map(|p| unit(*p - c[0])).unwrap_or(Vec2::ZERO)
}

fn tangent_end(s: &PathSeg) -> Vec2 {
    let c = ctrl(s);
    let last = c[c.len() - 1];
    c[..c.len() - 1].iter().rev().find(|p| (last - **p).hypot() > 1e-9).map(|p| unit(last - *p)).unwrap_or(Vec2::ZERO)
}

fn seg_points(s: &PathSeg, tol: f64, out: &mut Vec<Point>) {
    let mut p = BezPath::new();
    p.move_to(s.eval(0.0));
    match *s {
        PathSeg::Line(l) => p.line_to(l.p1),
        PathSeg::Quad(q) => p.quad_to(q.p1, q.p2),
        PathSeg::Cubic(c) => p.curve_to(c.p1, c.p2, c.p3),
    }
    flatten(&p, tol, |el| {
        if let PathEl::LineTo(q) = el {
            out.push(q);
        }
    });
}

/// Refits one run of segments (no corners inside) from its first to its last point.
fn fit_run(segs: &[PathSeg], tol: f64, out: &mut BezPath) {
    let mut pts = vec![segs[0].eval(0.0)];
    for s in segs {
        seg_points(s, tol * 0.1, &mut pts);
    }
    pts.dedup_by(|b, a| (*b - *a).hypot() < 1e-9);
    let end = *pts.last().unwrap();
    if pts.len() <= 2 || max_chord_dist(&pts) <= tol {
        out.line_to(end);
        return;
    }
    let t1 = tangent_start(&segs[0]);
    let t2 = -tangent_end(&segs[segs.len() - 1]);
    fit_cubic(&pts, t1, t2, tol * tol, out);
}

/// Simplifies path data. `tol` = allowed deviation (path units),
/// `corner_deg` = direction change above which a node stays a corner.
pub fn simplify(d: &str, tol: f64, corner_deg: f64) -> Result<SimplifyResult, String> {
    let path = BezPath::from_svg(d).map_err(|e| format!("Ungültige Pfaddaten: {e}"))?;
    let tol = tol.max(1e-4);
    let cos_limit = corner_deg.to_radians().cos();
    let subs = subpaths(&path);
    let nodes_before: usize = subs.iter().map(|s| s.segs.len() + usize::from(!s.closed)).sum();
    let mut out = BezPath::new();
    for sub in &subs {
        let n = sub.segs.len();
        // Corner at the start of segment i (between i-1 and i).
        let is_corner = |i: usize| -> bool {
            if !sub.closed && i == 0 {
                return true;
            }
            let prev = &sub.segs[(i + n - 1) % n];
            tangent_end(prev).dot(tangent_start(&sub.segs[i])) < cos_limit
        };
        let corners: Vec<usize> = (0..n).filter(|&i| is_corner(i)).collect();
        if corners.is_empty() {
            // Smooth closed loop: split it into two halves.
            let h = (n / 2).max(1);
            out.move_to(sub.segs[0].eval(0.0));
            fit_run(&sub.segs[..h], tol, &mut out);
            if h < n {
                fit_run(&sub.segs[h..], tol, &mut out);
            }
            out.close_path();
            continue;
        }
        let first = corners[0];
        out.move_to(sub.segs[first].eval(0.0));
        let m = corners.len();
        for k in 0..m {
            let a = corners[k];
            let b = if k + 1 < m { corners[k + 1] } else if sub.closed { first + n } else { n };
            let run: Vec<PathSeg> = (a..b).map(|i| sub.segs[i % n]).collect();
            if !run.is_empty() {
                fit_run(&run, tol, &mut out);
            }
        }
        if sub.closed {
            out.close_path();
        }
    }
    let nodes_after = out.elements().iter().filter(|e| !matches!(e, PathEl::ClosePath)).count();
    Ok(SimplifyResult { d: to_svg_d(&out), nodes_before, nodes_after })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Shape;

    #[test]
    fn dense_polygon_circle_becomes_few_curves() {
        let mut d = String::new();
        for i in 0..360 {
            let a = (i as f64).to_radians();
            let (x, y) = (50.0 + 40.0 * a.cos(), 50.0 + 40.0 * a.sin());
            d.push_str(&format!("{}{x:.3} {y:.3} ", if i == 0 { "M" } else { "L" }));
        }
        d.push('Z');
        let r = simplify(&d, 0.1, 30.0).unwrap();
        assert!(r.nodes_before >= 360);
        assert!(r.nodes_after < 20, "{}", r.nodes_after);
        let p = BezPath::from_svg(&r.d).unwrap();
        let b = p.bounding_box();
        assert!((b.width() - 80.0).abs() < 0.5 && (b.height() - 80.0).abs() < 0.5, "{b:?}");
    }

    #[test]
    fn square_keeps_its_corners() {
        let d = "M0 0L5 0L10 0L10 5L10 10L5 10L0 10L0 5Z";
        let r = simplify(d, 0.05, 30.0).unwrap();
        let p = BezPath::from_svg(&r.d).unwrap();
        let b = p.bounding_box();
        assert!((b.width() - 10.0).abs() < 1e-6 && (b.height() - 10.0).abs() < 1e-6);
        assert!(r.nodes_after <= 5, "{} {}", r.nodes_after, r.d);
        assert!((p.area().abs() - 100.0).abs() < 1e-6);
    }

    #[test]
    fn open_paths_stay_open() {
        let r = simplify("M0 0L1 0.001L2 0L3 0.001L4 0", 0.05, 30.0).unwrap();
        assert!(!r.d.contains('Z'));
        assert_eq!(r.nodes_after, 2, "{}", r.d);
    }
}
