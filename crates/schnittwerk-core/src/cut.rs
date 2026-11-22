//! Turning vector paths into an ordered list of cut polylines.
//!
//! - Béziers are flattened to polylines (device resolution is 0.05 mm).
//! - Order: a closed shape is only cut after every shape inside it (so the
//!   material does not shift once an outer piece is free), otherwise the
//!   nearest next path is taken. Closed paths start at the vertex nearest
//!   to the current blade position.
//! - Closed paths get an optional overcut so the cut closes cleanly.
//! - Multiple passes repeat each path in place.

use kurbo::{flatten, Affine, BezPath, PathEl, Point, Rect};
use serde::{Deserialize, Serialize};

/// Flattening tolerance in mm (half the device resolution).
const FLATTEN_TOL: f64 = 0.025;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathInput {
    /// SVG path data in the object's local units.
    pub d: String,
    /// Affine `[a, b, c, d, e, f]` (like SVG `matrix()`) to mat millimetres.
    pub matrix: [f64; 6],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Polyline {
    pub points: Vec<Point>,
    pub closed: bool,
}

impl Polyline {
    pub fn length(&self) -> f64 {
        self.points.windows(2).map(|w| (w[1] - w[0]).hypot()).sum()
    }
    fn bbox(&self) -> Rect {
        let mut r = Rect::from_points(self.points[0], self.points[0]);
        for p in &self.points {
            r = r.union_pt(*p);
        }
        r
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathOptions {
    /// Extra length (mm) cut past the start of closed paths.
    pub overcut: f64,
    /// How often each path is cut (1 = once).
    pub passes: u8,
    /// Optimize the order (otherwise: document order).
    pub optimize: bool,
}

impl Default for PathOptions {
    fn default() -> Self {
        Self { overcut: 0.3, passes: 1, optimize: true }
    }
}

/// Parses and flattens all inputs into polylines in mat millimetres.
pub fn flatten_inputs(inputs: &[PathInput]) -> Result<Vec<Polyline>, String> {
    let mut out = Vec::new();
    for inp in inputs {
        let path = BezPath::from_svg(&inp.d).map_err(|e| format!("Ungültige Pfaddaten: {e}"))?;
        let path = Affine::new(inp.matrix) * path;
        out.extend(flatten_path(&path));
    }
    Ok(out)
}

pub fn flatten_path(path: &BezPath) -> Vec<Polyline> {
    let mut out = Vec::new();
    let mut cur: Vec<Point> = Vec::new();
    let finish = |cur: &mut Vec<Point>, closed: bool, out: &mut Vec<Polyline>| {
        dedupe(cur);
        if closed && cur.len() > 2 && cur.first() == cur.last() {
            cur.pop();
        }
        if cur.len() >= 2 {
            let closed = closed && cur.len() > 2;
            out.push(Polyline { points: std::mem::take(cur), closed });
        }
        cur.clear();
    };
    flatten(path, FLATTEN_TOL, |el| match el {
        PathEl::MoveTo(p) => {
            finish(&mut cur, false, &mut out);
            cur.push(p);
        }
        PathEl::LineTo(p) => cur.push(p),
        PathEl::ClosePath => finish(&mut cur, true, &mut out),
        _ => {}
    });
    finish(&mut cur, false, &mut out);
    out
}

/// Removes consecutive points closer than the device resolution.
fn dedupe(pts: &mut Vec<Point>) {
    pts.dedup_by(|b, a| (*b - *a).hypot() < 0.02);
}

fn point_in_polygon(p: Point, poly: &[Point]) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// For every path, the indices of the closed paths that contain it.
fn containers(paths: &[Polyline]) -> Vec<Vec<usize>> {
    let boxes: Vec<Rect> = paths.iter().map(|p| p.bbox()).collect();
    (0..paths.len())
        .map(|i| {
            (0..paths.len())
                .filter(|&j| {
                    j != i
                        && paths[j].closed
                        && boxes[j].contains(boxes[i].origin())
                        && boxes[j].contains(Point::new(boxes[i].x1, boxes[i].y1))
                        && (boxes[j].area() > boxes[i].area())
                        && point_in_polygon(paths[i].points[0], &paths[j].points)
                })
                .collect()
        })
        .collect()
}

/// Orders the paths for cutting, starting from `start` (blade position).
pub fn order(paths: Vec<Polyline>, start: Point) -> Vec<Polyline> {
    let n = paths.len();
    let cont = containers(&paths);
    // blockers[j] = number of not-yet-cut paths inside closed path j.
    let mut blockers = vec![0usize; n];
    for c in &cont {
        for &j in c {
            blockers[j] += 1;
        }
    }
    let mut done = vec![false; n];
    let mut pos = start;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let mut best: Option<(usize, usize, f64)> = None; // (path, start vertex, dist²)
        for i in 0..n {
            if done[i] || blockers[i] > 0 {
                continue;
            }
            let p = &paths[i];
            let (v, d) = if p.closed {
                p.points
                    .iter()
                    .enumerate()
                    .map(|(k, q)| (k, (*q - pos).hypot2()))
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                    .unwrap()
            } else {
                let d0 = (p.points[0] - pos).hypot2();
                let d1 = (p.points[p.points.len() - 1] - pos).hypot2();
                if d1 < d0 { (usize::MAX, d1) } else { (0, d0) }
            };
            if best.is_none_or(|b| d < b.2) {
                best = Some((i, v, d));
            }
        }
        let Some((i, v, _)) = best else { break };
        done[i] = true;
        for &j in &cont[i] {
            blockers[j] -= 1;
        }
        let mut p = paths[i].clone();
        if p.closed {
            p.points.rotate_left(v);
        } else if v == usize::MAX {
            p.points.reverse();
        }
        pos = *p.points.last().unwrap();
        if p.closed {
            pos = p.points[0];
        }
        out.push(p);
    }
    out
}

/// Expands each path into the exact point sequence the blade travels while
/// down: closed loops are closed explicitly, repeated for each pass and
/// extended by the overcut.
pub fn cut_strokes(paths: &[Polyline], opts: &PathOptions) -> Vec<Vec<Point>> {
    let passes = opts.passes.max(1) as usize;
    paths
        .iter()
        .map(|p| {
            if p.closed {
                // p0 → p1 … pn-1 → p0, once per pass, then the overcut.
                let lap: Vec<Point> = p.points.iter().skip(1).copied().chain(std::iter::once(p.points[0])).collect();
                let mut s = vec![p.points[0]];
                for _ in 0..passes {
                    s.extend(lap.iter().copied());
                }
                append_overcut(&mut s, &p.points, opts.overcut);
                s
            } else {
                let mut s = p.points.clone();
                for k in 1..passes {
                    // Go back and forth for open paths.
                    let back: Vec<Point> = if k % 2 == 1 {
                        p.points.iter().rev().skip(1).copied().collect()
                    } else {
                        p.points.iter().skip(1).copied().collect()
                    };
                    s.extend(back);
                }
                s
            }
        })
        .collect()
}

fn append_overcut(stroke: &mut Vec<Point>, ring: &[Point], overcut: f64) {
    if overcut <= 0.0 {
        return;
    }
    let mut left = overcut;
    let n = ring.len();
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        let seg = (b - a).hypot();
        if seg >= left {
            stroke.push(a + (b - a) * (left / seg));
            return;
        }
        stroke.push(b);
        left -= seg;
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CutPlan {
    /// Ordered strokes (blade down), in mm.
    pub strokes: Vec<Vec<[f64; 2]>>,
    pub cut_length: f64,
    pub travel_length: f64,
    /// Bounding box `[x, y, w, h]` of all strokes in mm.
    pub bbox: [f64; 4],
}

pub fn plan(inputs: &[PathInput], opts: &PathOptions) -> Result<(CutPlan, Vec<Vec<Point>>), String> {
    let mut paths = flatten_inputs(inputs)?;
    if opts.optimize {
        paths = order(paths, Point::ZERO);
    }
    let strokes = cut_strokes(&paths, opts);
    let mut cut_length = 0.0;
    let mut travel = 0.0;
    let mut pos = Point::ZERO;
    let mut bb: Option<Rect> = None;
    for s in &strokes {
        travel += (s[0] - pos).hypot();
        for w in s.windows(2) {
            cut_length += (w[1] - w[0]).hypot();
        }
        pos = *s.last().unwrap();
        for p in s {
            bb = Some(bb.map_or(Rect::from_points(*p, *p), |r| r.union_pt(*p)));
        }
    }
    let b = bb.unwrap_or_default();
    let plan = CutPlan {
        strokes: strokes.iter().map(|s| s.iter().map(|p| [p.x, p.y]).collect()).collect(),
        cut_length,
        travel_length: travel,
        bbox: [b.x0, b.y0, b.width(), b.height()],
    };
    Ok((plan, strokes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x: f64, y: f64, s: f64) -> Polyline {
        Polyline {
            points: vec![Point::new(x, y), Point::new(x + s, y), Point::new(x + s, y + s), Point::new(x, y + s)],
            closed: true,
        }
    }

    #[test]
    fn inner_shapes_are_cut_first() {
        // Outer square listed first and nearest to the origin.
        let paths = vec![square(0.0, 0.0, 100.0), square(40.0, 40.0, 10.0), square(200.0, 0.0, 10.0)];
        let ordered = order(paths, Point::ZERO);
        let first_outer = ordered.iter().position(|p| p.points.iter().any(|q| q.x == 100.0 && q.y == 0.0)).unwrap();
        let inner = ordered.iter().position(|p| p.points.iter().any(|q| q.x == 50.0 && q.y == 50.0)).unwrap();
        assert!(inner < first_outer, "inner square must come before the outer one");
    }

    #[test]
    fn closed_strokes_close_and_overcut() {
        let s = cut_strokes(&[square(0.0, 0.0, 10.0)], &PathOptions { overcut: 2.0, passes: 1, optimize: true });
        let s = &s[0];
        assert_eq!(s[0], Point::new(0.0, 0.0));
        assert!(s.contains(&Point::new(0.0, 0.0)) && s.iter().filter(|p| **p == Point::new(0.0, 0.0)).count() == 2);
        assert_eq!(*s.last().unwrap(), Point::new(2.0, 0.0), "overcut 2 mm along the first edge");
    }

    #[test]
    fn passes_repeat_the_loop() {
        let one = cut_strokes(&[square(0.0, 0.0, 10.0)], &PathOptions { overcut: 0.0, passes: 1, optimize: true });
        let two = cut_strokes(&[square(0.0, 0.0, 10.0)], &PathOptions { overcut: 0.0, passes: 2, optimize: true });
        assert_eq!(one[0].len(), 5);
        assert_eq!(two[0].len(), 9);
    }

    #[test]
    fn plan_applies_matrix_and_arcs() {
        let inputs = vec![PathInput { d: "M0 5 A5 5 0 1 0 10 5 A5 5 0 1 0 0 5 Z".into(), matrix: [2.0, 0.0, 0.0, 2.0, 10.0, 20.0] }];
        let (plan, _) = plan(&inputs, &PathOptions::default()).unwrap();
        let b = plan.bbox;
        assert!((b[0] - 10.0).abs() < 0.1 && (b[1] - 20.0).abs() < 0.1 && (b[2] - 20.0).abs() < 0.2, "{b:?}");
        assert!((plan.cut_length - std::f64::consts::PI * 20.0).abs() < 1.0, "{}", plan.cut_length);
    }
}
