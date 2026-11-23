//! Field → smooth vector outlines.
//!
//! 1. Marching squares extracts the 0.5 iso-lines of a [`Field`] as closed
//!    polylines with sub-pixel positions.
//! 2. Corners are detected by the turning angle over a short arc and
//!    reconstructed as the intersection of the two adjacent sides.
//! 3. Between corners the polyline is smoothed along its length and cubic
//!    Béziers are fitted with Schneider's algorithm ("An Algorithm for
//!    Automatically Fitting Digitized Curves", Graphics Gems 1990).
//!
//! All tunables are limited per contour by its size, so small shapes
//! (dots, thin strokes, small text) keep their form.

use std::collections::HashMap;

use kurbo::{BezPath, Point, Vec2};

use crate::field::{Field, ISO};

#[derive(Debug, Clone, Copy)]
pub struct ContourParams {
    /// Gaussian sigma (px of arc length) for smoothing between corners.
    pub smooth: f64,
    /// Max allowed distance (px) between fitted curve and contour.
    pub tolerance: f64,
    /// Turning angle (radians) above which a point counts as a corner.
    pub corner_angle: f64,
    /// Contours enclosing less area (px²) are dropped.
    pub min_area: f64,
    /// Fit curves (true) or straight segments (false).
    pub curves: bool,
    /// Work pixels per source pixel (> 1 when the image was upscaled);
    /// all px-based lengths scale with it.
    pub unit: f64,
}

impl ContourParams {
    /// Maps the UI "smoothness" (0..100) onto the internal parameters.
    pub fn from_smoothness(smoothness: f32, min_area: f64, curves: bool, unit: f64) -> Self {
        let s = smoothness.clamp(0.0, 100.0) as f64 / 100.0;
        let unit = unit.max(1.0);
        Self {
            smooth: (0.6 + s * 4.0) * unit,
            tolerance: (0.15 + s * 0.6) * unit,
            corner_angle: (35.0 + s * 75.0f64).to_radians(),
            min_area,
            curves,
            unit,
        }
    }
}

/// Arc length over which corner angles are measured.
const CORNER_WINDOW: f64 = 3.0;

/// Returns the compound outline path (even-odd) and the number of contours.
pub fn trace_field(field: &Field, p: &ContourParams) -> (BezPath, usize) {
    let loops = marching_squares(field);
    let mut out = BezPath::new();
    let mut count = 0;
    for mut pts in loops {
        if pts.len() > 1 && (pts[0] - pts[pts.len() - 1]).hypot() < 1e-9 {
            pts.pop();
        }
        if pts.len() < 3 || signed_area(&pts).abs() < p.min_area.max(0.5) {
            continue;
        }
        let shape = Shape::of(&pts);
        if p.curves {
            fit_closed(&pts, p, &shape, &mut out);
        } else {
            let sm = smooth_closed(&pts, shape.limit_smooth(p.smooth, p.unit) * 0.4);
            polygon_closed(&sm, shape.limit_tol(p.tolerance.max(0.4)), &mut out);
        }
        count += 1;
    }
    (out, count)
}

/// Size measures of a contour used to scale the parameters down.
struct Shape {
    perimeter: f64,
    /// Characteristic radius 2A/P (= r for a circle, ≈ half width for a stroke).
    radius: f64,
}

impl Shape {
    fn of(pts: &[Point]) -> Self {
        let n = pts.len();
        let perimeter: f64 = (0..n).map(|i| (pts[(i + 1) % n] - pts[i]).hypot()).sum();
        let radius = 2.0 * signed_area(pts).abs() / perimeter.max(1e-9);
        Self { perimeter, radius }
    }
    /// Gaussian smoothing shrinks curves by ≈ σ²/2R; keep that below
    /// 0.2 source px (`unit` = work px per source px).
    fn limit_smooth(&self, s: f64, unit: f64) -> f64 {
        s.min((0.4 * self.radius * unit).sqrt()).max(0.0)
    }
    fn limit_tol(&self, t: f64) -> f64 {
        t.min((self.radius * 0.08).max(0.06))
    }
}

// ---------------------------------------------------------------- marching squares

#[inline]
fn h_edge(i: usize, j: usize, w: usize) -> u64 {
    ((j * w + i) as u64) << 1
}
#[inline]
fn v_edge(i: usize, j: usize, w: usize) -> u64 {
    (((j * w + i) as u64) << 1) | 1
}

/// Extracts closed iso-contours in field pixel space (pixel centers at x+0.5).
fn marching_squares(field: &Field) -> Vec<Vec<Point>> {
    // Pad with an "outside" border so every contour closes.
    let (w, h) = (field.width + 2, field.height + 2);
    let mut f = vec![-1.0f32; w * h];
    for y in 0..field.height {
        let row = &field.data[y * field.width..(y + 1) * field.width];
        f[(y + 1) * w + 1..(y + 1) * w + 1 + field.width].copy_from_slice(row);
    }
    let inside = |i: usize, j: usize| f[j * w + i] > ISO;
    let mut segs: Vec<(u64, u64)> = Vec::new();
    for j in 0..h - 1 {
        for i in 0..w - 1 {
            let case = (inside(i, j) as u8) << 3
                | (inside(i + 1, j) as u8) << 2
                | (inside(i + 1, j + 1) as u8) << 1
                | inside(i, j + 1) as u8;
            if case == 0 || case == 15 {
                continue;
            }
            let t = h_edge(i, j, w);
            let b = h_edge(i, j + 1, w);
            let l = v_edge(i, j, w);
            let r = v_edge(i + 1, j, w);
            let center_in = || (f[j * w + i] + f[j * w + i + 1] + f[(j + 1) * w + i] + f[(j + 1) * w + i + 1]) / 4.0 > ISO;
            match case {
                1 | 14 => segs.push((l, b)),
                2 | 13 => segs.push((b, r)),
                3 | 12 => segs.push((l, r)),
                4 | 11 => segs.push((t, r)),
                6 | 9 => segs.push((t, b)),
                7 | 8 => segs.push((t, l)),
                5 => {
                    if center_in() {
                        segs.extend([(l, t), (b, r)]);
                    } else {
                        segs.extend([(t, r), (l, b)]);
                    }
                }
                10 => {
                    if center_in() {
                        segs.extend([(t, r), (l, b)]);
                    } else {
                        segs.extend([(t, l), (b, r)]);
                    }
                }
                _ => {}
            }
        }
    }

    let pos = |id: u64| -> Point {
        let cell = (id >> 1) as usize;
        let (i, j) = (cell % w, cell / w);
        let (i2, j2) = if id & 1 == 0 { (i + 1, j) } else { (i, j + 1) };
        let (a, b) = (f[j * w + i], f[j2 * w + i2]);
        let t = if (b - a).abs() < 1e-6 { 0.5 } else { ((ISO - a) / (b - a)).clamp(0.0, 1.0) } as f64;
        // Grid point (i,j) is the center of field pixel (i-1, j-1).
        Point::new(i as f64 - 0.5 + t * (i2 as f64 - i as f64), j as f64 - 0.5 + t * (j2 as f64 - j as f64))
    };

    let mut adj: HashMap<u64, [usize; 2]> = HashMap::with_capacity(segs.len() * 2);
    for (k, &(a, b)) in segs.iter().enumerate() {
        for e in [a, b] {
            adj.entry(e).and_modify(|v| v[1] = k).or_insert([k, usize::MAX]);
        }
    }
    let mut used = vec![false; segs.len()];
    let mut loops = Vec::new();
    for start in 0..segs.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let (first, mut cur_edge) = segs[start];
        let mut pts = vec![pos(first)];
        let mut cur_seg = start;
        loop {
            pts.push(pos(cur_edge));
            let pair = adj[&cur_edge];
            let next = if pair[0] == cur_seg { pair[1] } else { pair[0] };
            if next == usize::MAX || used[next] {
                break;
            }
            used[next] = true;
            let (a, b) = segs[next];
            cur_edge = if a == cur_edge { b } else { a };
            cur_seg = next;
            if cur_edge == first {
                break;
            }
        }
        loops.push(pts);
    }
    loops
}

fn signed_area(pts: &[Point]) -> f64 {
    let n = pts.len();
    (0..n).map(|i| pts[i].to_vec2().cross(pts[(i + 1) % n].to_vec2())).sum::<f64>() / 2.0
}

// ---------------------------------------------------------------- helpers

pub(crate) fn unit(v: Vec2) -> Vec2 {
    let l = v.hypot();
    if l < 1e-12 { Vec2::new(1.0, 0.0) } else { v / l }
}

/// Cumulative arc length; `cum[n]` is the total length of the closed ring.
fn cumulative(ring: &[Point]) -> Vec<f64> {
    let n = ring.len();
    let mut cum = vec![0.0; n + 1];
    for i in 0..n {
        cum[i + 1] = cum[i] + (ring[(i + 1) % n] - ring[i]).hypot();
    }
    cum
}

/// Point at arc distance `dist` from index i, walking forward or backward.
fn point_at_arc(ring: &[Point], i: usize, dist: f64, forward: bool) -> Point {
    let n = ring.len();
    let (mut j, mut acc) = (i, 0.0);
    for _ in 0..n {
        let nj = if forward { (j + 1) % n } else { (j + n - 1) % n };
        let step = (ring[nj] - ring[j]).hypot();
        if acc + step >= dist && step > 0.0 {
            return ring[j] + (ring[nj] - ring[j]) * ((dist - acc) / step);
        }
        acc += step;
        j = nj;
    }
    ring[j]
}

/// Indices of corner points on a closed polyline.
fn find_corners(pts: &[Point], max_angle: f64, window: f64) -> Vec<usize> {
    let n = pts.len();
    if n < 6 {
        return vec![];
    }
    let angle: Vec<f64> = (0..n)
        .map(|i| {
            let (a, b, c) = (point_at_arc(pts, i, window, false), pts[i], point_at_arc(pts, i, window, true));
            let (u, v) = (b - a, c - b);
            let d = u.hypot() * v.hypot();
            if d < 1e-9 { 0.0 } else { (u.dot(v) / d).clamp(-1.0, 1.0).acos() }
        })
        .collect();
    let k = (window.ceil() as usize).clamp(1, n / 3);
    (0..n)
        .filter(|&i| {
            angle[i] >= max_angle
                && (1..=k).all(|o| angle[(i + o) % n] < angle[i] && angle[(i + n - o) % n] <= angle[i])
        })
        .collect()
}

/// Gaussian smoothing along a closed polyline (index-based, ≤1.4 px spacing).
fn smooth_closed(pts: &[Point], sigma: f64) -> Vec<Point> {
    let n = pts.len();
    let r = ((sigma * 2.5).ceil() as usize).min(n.saturating_sub(1) / 2);
    if r == 0 || sigma < 0.05 {
        return pts.to_vec();
    }
    let w: Vec<f64> = (0..=r).map(|k| (-((k * k) as f64) / (2.0 * sigma * sigma)).exp()).collect();
    (0..n)
        .map(|i| {
            let (mut acc, mut ws) = (pts[i].to_vec2() * w[0], w[0]);
            for k in 1..=r {
                acc += (pts[(i + k) % n].to_vec2() + pts[(i + n - k) % n].to_vec2()) * w[k];
                ws += 2.0 * w[k];
            }
            (acc / ws).to_point()
        })
        .collect()
}

/// Smoothing of an open run; the kernel shrinks towards the ends so the
/// endpoints stay fixed.
pub(crate) fn smooth_open(pts: &[Point], sigma: f64) -> Vec<Point> {
    let n = pts.len();
    let r = (sigma * 2.5).ceil() as usize;
    if r == 0 || n < 3 || sigma < 0.05 {
        return pts.to_vec();
    }
    let w: Vec<f64> = (0..=r).map(|k| (-((k * k) as f64) / (2.0 * sigma * sigma)).exp()).collect();
    (0..n)
        .map(|i| {
            let ri = r.min(i).min(n - 1 - i);
            let (mut acc, mut ws) = (pts[i].to_vec2() * w[0], w[0]);
            for k in 1..=ri {
                acc += (pts[i + k].to_vec2() + pts[i - k].to_vec2()) * w[k];
                ws += 2.0 * w[k];
            }
            (acc / ws).to_point()
        })
        .collect()
}

pub(crate) fn max_chord_dist(d: &[Point]) -> f64 {
    let (a, b) = (d[0], d[d.len() - 1]);
    let ab = b - a;
    let len = ab.hypot();
    d.iter()
        .map(|p| if len < 1e-9 { (*p - a).hypot() } else { ab.cross(*p - a).abs() / len })
        .fold(0.0, f64::max)
}

// ---------------------------------------------------------------- polygon mode

fn polygon_closed(ring: &[Point], tol: f64, out: &mut BezPath) {
    let far = (1..ring.len()).max_by(|&a, &b| (ring[a] - ring[0]).hypot2().total_cmp(&(ring[b] - ring[0]).hypot2())).unwrap_or(0);
    let half1 = rdp(&ring[..=far], tol);
    let mut tail = ring[far..].to_vec();
    tail.push(ring[0]);
    let half2 = rdp(&tail, tol);
    out.move_to(half1[0]);
    for p in half1.iter().skip(1).chain(half2.iter().skip(1)) {
        out.line_to(*p);
    }
    out.close_path();
}

pub(crate) fn rdp(pts: &[Point], tol: f64) -> Vec<Point> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let (a, b) = (pts[0], pts[pts.len() - 1]);
    let ab = b - a;
    let len = ab.hypot();
    let (mut idx, mut dmax) = (0, 0.0);
    for (i, p) in pts.iter().enumerate().take(pts.len() - 1).skip(1) {
        let d = if len < 1e-9 { (*p - a).hypot() } else { ab.cross(*p - a).abs() / len };
        if d > dmax {
            dmax = d;
            idx = i;
        }
    }
    if dmax <= tol {
        return vec![a, b];
    }
    let mut left = rdp(&pts[..=idx], tol);
    let right = rdp(&pts[idx..], tol);
    left.pop();
    left.extend(right);
    left
}

// ---------------------------------------------------------------- curve mode

struct Corner {
    idx: usize,
    /// Reconstructed corner point.
    pos: Point,
    /// Tangent leaving the corner along the next side.
    t_out: Vec2,
    /// Tangent leaving the corner backwards along the previous side.
    t_back: Vec2,
    /// Arc length trimmed off the neighbouring runs.
    trim_prev: f64,
    trim_next: f64,
}

fn fit_closed(ring: &[Point], p: &ContourParams, shape: &Shape, out: &mut BezPath) {
    let n = ring.len();
    let smooth = shape.limit_smooth(p.smooth, p.unit);
    let tol = shape.limit_tol(p.tolerance);
    let err2 = tol * tol;
    // Never measure corners across more than the stroke width, or the two
    // corners at the end of a thin stroke merge into one spike.
    let u = p.unit;
    let window = (CORNER_WINDOW * u).min(shape.perimeter / 8.0).min((2.0 * shape.radius).max(u));
    let corner_idx = if shape.perimeter < 10.0 * u { vec![] } else { find_corners(ring, p.corner_angle, window) };

    if corner_idx.is_empty() {
        let sm = smooth_closed(ring, smooth);
        let tan = |i: usize| unit(sm[(i + 2) % n] - sm[(i + n - 2) % n]);
        let h = n / 2;
        out.move_to(sm[0]);
        let first = sm[..=h].to_vec();
        let mut second = sm[h..].to_vec();
        second.push(sm[0]);
        fit_cubic(&first, tan(0), -tan(h), err2, out);
        fit_cubic(&second, tan(h), -tan(0), err2, out);
        out.close_path();
        return;
    }

    let cum = cumulative(ring);
    let total = cum[n];
    let arc_between = |a: usize, b: usize| (cum[b] - cum[a] + total) % total;
    let m = corner_idx.len();
    let corners: Vec<Corner> = (0..m)
        .map(|k| {
            let i = corner_idx[k];
            let prev = corner_idx[(k + m - 1) % m];
            let next = corner_idx[(k + 1) % m];
            let side_prev = if m == 1 { total } else { arc_between(prev, i) };
            let side_next = if m == 1 { total } else { arc_between(i, next) };
            // Sample each side well inside its own half so short sides do not
            // reach into the neighbouring corner.
            let trim_prev = (side_prev * 0.15).min(1.2 * u);
            let trim_next = (side_next * 0.15).min(1.2 * u);
            let reach_prev = (side_prev * 0.45).min(trim_prev + 3.0 * u);
            let reach_next = (side_next * 0.45).min(trim_next + 3.0 * u);
            let b1 = point_at_arc(ring, i, trim_prev, false);
            let b2 = point_at_arc(ring, i, reach_prev, false);
            let f1 = point_at_arc(ring, i, trim_next, true);
            let f2 = point_at_arc(ring, i, reach_next, true);
            let d_in = unit(b1 - b2);
            let d_out = unit(f2 - f1);
            let den = d_in.cross(d_out);
            let pos = if den.abs() > 0.1 {
                let t = (f1 - b1).cross(d_out) / den;
                let c = b1 + d_in * t;
                // The iso-line is only slightly rounded – never move far,
                // and not at all next to very short sides.
                let max_move = (0.3 * side_prev.min(side_next)).min(1.2 * u);
                if (c - ring[i]).hypot() <= max_move { c } else { ring[i] }
            } else {
                ring[i]
            };
            Corner { idx: i, pos, t_out: d_out, t_back: -d_in, trim_prev, trim_next }
        })
        .collect();

    out.move_to(corners[0].pos);
    for ci in 0..m {
        let (a, b) = (&corners[ci], &corners[(ci + 1) % m]);
        let len = if b.idx > a.idx { b.idx - a.idx } else { b.idx + n - a.idx };
        let run: Vec<Point> = (0..=len).map(|o| ring[(a.idx + o) % n]).collect();
        let mut arc = vec![0.0; run.len()];
        for k in 1..run.len() {
            arc[k] = arc[k - 1] + (run[k] - run[k - 1]).hypot();
        }
        let run_len = *arc.last().unwrap();
        let inner: Vec<Point> = run
            .iter()
            .zip(&arc)
            .filter(|(_, &s)| s > a.trim_next && s < run_len - b.trim_prev)
            .map(|(q, _)| *q)
            .collect();
        // Limit smoothing by this run's own curvature radius (from chord
        // length L and bulge h): long, nearly straight edges get full
        // smoothing (removes stair-steps), tight bowls of small letters
        // are not shrunk.
        let mut raw = vec![a.pos];
        raw.extend(inner.iter().copied());
        raw.push(b.pos);
        let chord = (b.pos - a.pos).hypot();
        let bulge = max_chord_dist(&raw);
        let run_radius = if bulge < 1e-6 { f64::INFINITY } else { chord * chord / (8.0 * bulge) + bulge / 2.0 };
        let run_smooth = p.smooth.min((0.4 * run_radius).sqrt());
        let mut data = vec![a.pos];
        data.extend(smooth_open(&inner, run_smooth));
        data.push(b.pos);
        if data.len() <= 3 || max_chord_dist(&data) < tol * 0.5 {
            out.line_to(b.pos);
        } else {
            fit_cubic(&data, a.t_out, b.t_back, err2, out);
        }
    }
    out.close_path();
}

pub(crate) fn fit_cubic(d: &[Point], t1: Vec2, t2: Vec2, err2: f64, out: &mut BezPath) {
    let n = d.len();
    if n < 2 {
        return;
    }
    if n == 2 {
        let dist = (d[1] - d[0]).hypot() / 3.0;
        out.curve_to(d[0] + t1 * dist, d[1] + t2 * dist, d[1]);
        return;
    }
    let mut u = chord_params(d);
    let mut bez = generate_bezier(d, &u, t1, t2);
    let (mut max_err, mut split) = max_error(d, &bez, &u);
    if max_err < err2 {
        out.curve_to(bez[1], bez[2], bez[3]);
        return;
    }
    if max_err < err2 * 4.0 {
        for _ in 0..6 {
            u = reparameterize(d, &u, &bez);
            bez = generate_bezier(d, &u, t1, t2);
            (max_err, split) = max_error(d, &bez, &u);
            if max_err < err2 {
                out.curve_to(bez[1], bez[2], bez[3]);
                return;
            }
        }
    }
    let split = split.clamp(1, n - 2);
    let tc = unit(d[split - 1] - d[split + 1]);
    fit_cubic(&d[..=split], t1, tc, err2, out);
    fit_cubic(&d[split..], -tc, t2, err2, out);
}

fn chord_params(d: &[Point]) -> Vec<f64> {
    let mut u = Vec::with_capacity(d.len());
    u.push(0.0);
    for i in 1..d.len() {
        u.push(u[i - 1] + (d[i] - d[i - 1]).hypot());
    }
    let total = *u.last().unwrap();
    if total > 0.0 {
        for v in &mut u {
            *v /= total;
        }
    }
    u
}

fn bernstein(t: f64) -> [f64; 4] {
    let mt = 1.0 - t;
    [mt * mt * mt, 3.0 * t * mt * mt, 3.0 * t * t * mt, t * t * t]
}

fn generate_bezier(d: &[Point], u: &[f64], t1: Vec2, t2: Vec2) -> [Point; 4] {
    let (p0, p3) = (d[0], d[d.len() - 1]);
    let (mut c00, mut c01, mut c11, mut x0, mut x1) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (pt, &t) in d.iter().zip(u) {
        let b = bernstein(t);
        let (a1, a2) = (t1 * b[1], t2 * b[2]);
        c00 += a1.dot(a1);
        c01 += a1.dot(a2);
        c11 += a2.dot(a2);
        let tmp = *pt - (p0.to_vec2() * (b[0] + b[1]) + p3.to_vec2() * (b[2] + b[3])).to_point();
        x0 += a1.dot(tmp);
        x1 += a2.dot(tmp);
    }
    let det = c00 * c11 - c01 * c01;
    let seg_len = (p3 - p0).hypot();
    let (mut al, mut ar) = if det.abs() > 1e-12 { ((x0 * c11 - x1 * c01) / det, (c00 * x1 - c01 * x0) / det) } else { (0.0, 0.0) };
    let eps = 1e-6 * seg_len;
    if al < eps || ar < eps || al > seg_len * 2.0 || ar > seg_len * 2.0 {
        al = seg_len / 3.0;
        ar = seg_len / 3.0;
    }
    [p0, p0 + t1 * al, p3 + t2 * ar, p3]
}

fn bez_at(b: &[Point; 4], t: f64) -> Point {
    let w = bernstein(t);
    (b[0].to_vec2() * w[0] + b[1].to_vec2() * w[1] + b[2].to_vec2() * w[2] + b[3].to_vec2() * w[3]).to_point()
}

fn max_error(d: &[Point], b: &[Point; 4], u: &[f64]) -> (f64, usize) {
    let mut best = (0.0, d.len() / 2);
    for i in 1..d.len() - 1 {
        let e = (bez_at(b, u[i]) - d[i]).hypot2();
        if e >= best.0 {
            best = (e, i);
        }
    }
    best
}

fn reparameterize(d: &[Point], u: &[f64], b: &[Point; 4]) -> Vec<f64> {
    let q1 = [(b[1] - b[0]) * 3.0, (b[2] - b[1]) * 3.0, (b[3] - b[2]) * 3.0];
    let q2 = [(q1[1] - q1[0]) * 2.0, (q1[2] - q1[1]) * 2.0];
    d.iter()
        .zip(u)
        .map(|(p, &t)| {
            let q = bez_at(b, t);
            let mt = 1.0 - t;
            let d1 = q1[0] * (mt * mt) + q1[1] * (2.0 * t * mt) + q1[2] * (t * t);
            let d2 = q2[0] * mt + q2[1] * t;
            let diff = q - *p;
            let den = d1.dot(d1) + diff.dot(d2);
            if den.abs() < 1e-12 { t } else { (t - diff.dot(d1) / den).clamp(0.0, 1.0) }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preprocess::Mask;
    use kurbo::{ParamCurve, PathSeg, Shape as _};

    fn mask_from(w: usize, h: usize, f: impl Fn(f64, f64) -> bool) -> Mask {
        let mut m = Mask::new(w, h);
        for y in 0..h {
            for x in 0..w {
                m.data[y * w + x] = f(x as f64 + 0.5, y as f64 + 0.5);
            }
        }
        m
    }

    fn params() -> ContourParams {
        ContourParams::from_smoothness(40.0, 1.0, true, 1.0)
    }

    #[test]
    fn circle_is_smooth_and_accurate() {
        let r = 60.0;
        let m = mask_from(140, 140, |x, y| (x - 70.0).hypot(y - 70.0) < r);
        let (path, n) = trace_field(&Field::from_mask(&m, 0.6), &params());
        assert_eq!(n, 1);
        let segs: Vec<PathSeg> = path.segments().collect();
        assert!(segs.len() <= 16, "too many segments for a circle: {}", segs.len());
        for s in &segs {
            for k in 0..=10 {
                let p = s.eval(k as f64 / 10.0);
                let d = (p.x - 70.0).hypot(p.y - 70.0);
                assert!((d - r).abs() < 0.8, "deviation {} at {:?}", d - r, p);
            }
        }
    }

    #[test]
    fn square_keeps_corners() {
        let m = mask_from(100, 100, |x, y| (20.0..80.0).contains(&x) && (20.0..80.0).contains(&y));
        let ring = marching_squares(&Field::from_mask(&m, 0.6)).remove(0);
        let corners = find_corners(&ring, params().corner_angle, CORNER_WINDOW);
        assert_eq!(corners.len(), 4, "corners: {corners:?}");
    }

    #[test]
    fn small_square_keeps_its_size() {
        let m = mask_from(30, 30, |x, y| (10.0..17.0).contains(&x) && (10.0..17.0).contains(&y));
        let (path, n) = trace_field(&Field::from_mask(&m, 0.6), &params());
        assert_eq!(n, 1);
        let b = path.bounding_box();
        assert!((b.x0 - 10.0).abs() < 0.5 && (b.x1 - 17.0).abs() < 0.5, "{b:?}");
        assert!((b.y0 - 10.0).abs() < 0.5 && (b.y1 - 17.0).abs() < 0.5, "{b:?}");
    }

    #[test]
    fn small_dot_has_no_spikes() {
        let m = mask_from(20, 20, |x, y| (x - 10.0).hypot(y - 10.0) < 3.0);
        let (path, n) = trace_field(&Field::from_mask(&m, 0.6), &params());
        assert_eq!(n, 1);
        let b = path.bounding_box();
        assert!(b.width() < 7.0 && b.width() > 5.0, "{b:?}");
    }

    #[test]
    fn thin_line_survives() {
        let m = mask_from(60, 20, |x, y| (5.0..55.0).contains(&x) && (10.0..11.0).contains(&y));
        let (path, n) = trace_field(&Field::from_mask(&m, 0.6), &params());
        assert_eq!(n, 1, "1 px line must not vanish");
        assert!(path.bounding_box().width() > 48.0);
    }
}
