//! Centerline tracing: one open path along the middle of each stroke
//! (pen drawing, single-line designs) instead of an outline around it.
//!
//! 1. Zhang–Suen thinning reduces the mask to a one-pixel skeleton.
//! 2. Skeleton pixels are classified by their crossing number (number of
//!    background→foreground transitions around them): 1 = line end,
//!    2 = line, ≥3 = junction. Staircase pixels count as line pixels.
//! 3. Lines are walked from ends/junctions; leftover pixels are loops.
//! 4. Each line is smoothed and fitted with Béziers (see `contour`).

use kurbo::{BezPath, Point};

use crate::contour::{fit_cubic, max_chord_dist, rdp, smooth_open, unit, ContourParams};
use crate::preprocess::Mask;

/// Neighbour offsets P2..P9 (N, NE, E, SE, S, SW, W, NW).
const NB: [(isize, isize); 8] = [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1)];

struct Grid {
    w: usize,
    h: usize,
    on: Vec<bool>,
}

impl Grid {
    /// Copy of the mask with a one-pixel empty border.
    fn from_mask(m: &Mask) -> Self {
        let (w, h) = (m.width + 2, m.height + 2);
        let mut on = vec![false; w * h];
        for y in 0..m.height {
            for x in 0..m.width {
                on[(y + 1) * w + x + 1] = m.get(x, y);
            }
        }
        Self { w, h, on }
    }

    fn nb(&self, i: usize) -> [bool; 8] {
        let (x, y) = ((i % self.w) as isize, (i / self.w) as isize);
        NB.map(|(dx, dy)| self.on[((y + dy) as usize) * self.w + (x + dx) as usize])
    }

    fn idx(&self, i: usize, k: usize) -> usize {
        let (x, y) = ((i % self.w) as isize, (i / self.w) as isize);
        let (dx, dy) = NB[k];
        ((y + dy) as usize) * self.w + (x + dx) as usize
    }
}

fn transitions(n: &[bool; 8]) -> usize {
    (0..8).filter(|&k| !n[k] && n[(k + 1) % 8]).count()
}

fn thin(g: &mut Grid) {
    loop {
        let mut changed = false;
        for step in 0..2 {
            let mut del = Vec::new();
            for y in 1..g.h - 1 {
                for x in 1..g.w - 1 {
                    let i = y * g.w + x;
                    if !g.on[i] {
                        continue;
                    }
                    let n = g.nb(i);
                    let b = n.iter().filter(|v| **v).count();
                    if !(2..=6).contains(&b) || transitions(&n) != 1 {
                        continue;
                    }
                    let [p2, _, p4, _, p6, _, p8, _] = n;
                    let ok = if step == 0 { !(p2 && p4 && p6) && !(p4 && p6 && p8) } else { !(p2 && p4 && p8) && !(p2 && p6 && p8) };
                    if ok {
                        del.push(i);
                    }
                }
            }
            changed |= !del.is_empty();
            for i in del {
                g.on[i] = false;
            }
        }
        if !changed {
            break;
        }
    }
}

/// Skeleton polylines in grid coordinates (pixel centres), and whether each is a loop.
fn walk(g: &Grid) -> Vec<(Vec<usize>, bool)> {
    let crossing: Vec<usize> = (0..g.on.len()).map(|i| if g.on[i] { transitions(&g.nb(i)) } else { 0 }).collect();
    let is_node = |i: usize| g.on[i] && crossing[i] != 2;
    let mut visited = vec![false; g.on.len()];
    let mut out = Vec::new();

    // Next unvisited skeleton neighbour, orthogonal neighbours first.
    let next = |i: usize, visited: &Vec<bool>, prev: usize| -> Option<usize> {
        let n = g.nb(i);
        [0, 2, 4, 6, 1, 3, 5, 7].into_iter().filter(|&k| n[k]).map(|k| g.idx(i, k)).find(|&j| j != prev && (!visited[j] || is_node(j)))
    };

    let follow = |start: usize, first: usize, visited: &mut Vec<bool>| -> Vec<usize> {
        let mut path = vec![start, first];
        let (mut prev, mut cur) = (start, first);
        while !is_node(cur) {
            visited[cur] = true;
            match next(cur, visited, prev) {
                Some(n) => {
                    path.push(n);
                    prev = cur;
                    cur = n;
                }
                None => break,
            }
        }
        path
    };

    for i in 0..g.on.len() {
        if !is_node(i) {
            continue;
        }
        let n = g.nb(i);
        for k in [0, 2, 4, 6, 1, 3, 5, 7] {
            if !n[k] {
                continue;
            }
            let j = g.idx(i, k);
            if visited[j] || is_node(j) {
                continue;
            }
            out.push((follow(i, j, &mut visited), false));
        }
        visited[i] = true;
    }
    // Remaining line pixels form closed loops.
    for i in 0..g.on.len() {
        if !g.on[i] || visited[i] || is_node(i) {
            continue;
        }
        visited[i] = true;
        let mut path = vec![i];
        let (mut prev, mut cur) = (i, i);
        while let Some(n) = next(cur, &visited, prev) {
            if n == i {
                break;
            }
            visited[n] = true;
            path.push(n);
            prev = cur;
            cur = n;
        }
        if path.len() > 2 {
            path.push(i);
            out.push((path, true));
        }
    }
    out
}

/// Drops the last `t` px of a polyline and re-extends it straight along the
/// direction it had before that.
fn straighten_end(pts: &mut Vec<Point>, t: f64) {
    if t < 1.0 {
        return;
    }
    let total: f64 = pts.windows(2).map(|w| (w[1] - w[0]).hypot()).sum();
    if total < 4.0 * t {
        return;
    }
    let mut acc = 0.0;
    while pts.len() > 2 {
        let n = pts.len();
        let seg = (pts[n - 1] - pts[n - 2]).hypot();
        if acc + seg > t {
            break;
        }
        acc += seg;
        pts.pop();
    }
    // Direction over the next 2t of the remaining line.
    let n = pts.len();
    let end = pts[n - 1];
    let mut back = 0.0;
    let mut k = n - 1;
    while k > 0 && back < 2.0 * t {
        back += (pts[k] - pts[k - 1]).hypot();
        k -= 1;
    }
    let dir = unit(end - pts[k]);
    pts.push(end + dir * acc);
}

/// Traces stroke centrelines; returns the path (open subpaths, loops closed)
/// and the number of lines. Lines shorter than `min_len` px are dropped.
pub fn trace_centerline(mask: &Mask, p: &ContourParams, min_len: f64) -> (BezPath, usize) {
    let mut g = Grid::from_mask(mask);
    thin(&mut g);
    // Squared distance to the background = local stroke half-width².
    let dist = crate::offset::edt_sq(mask, false);
    let half_width = |i: usize| -> f64 {
        let (x, y) = (i % g.w, i / g.w);
        if x == 0 || y == 0 || x > mask.width || y > mask.height {
            return 1.0;
        }
        (dist[(y - 1) * mask.width + (x - 1)] as f64).sqrt()
    };
    let kind = |i: usize| transitions(&g.nb(i));
    let mut out = BezPath::new();
    let mut count = 0;
    // Pass 1: prune spurs – short branches from a junction to a free end are
    // thinning artefacts at corners and blunt stroke ends.
    let paths: Vec<(Vec<usize>, bool)> = walk(&g)
        .into_iter()
        .filter(|(idx, closed)| {
            if *closed {
                return true;
            }
            let (a, b) = (idx[0], idx[idx.len() - 1]);
            let (ka, kb) = (kind(a), kind(b));
            let junction = if ka >= 3 && kb <= 1 { Some(a) } else if kb >= 3 && ka <= 1 { Some(b) } else { None };
            let Some(j) = junction else { return true };
            let steps: f64 = idx
                .windows(2)
                .map(|w| {
                    let d = (w[0] as isize - w[1] as isize).unsigned_abs();
                    if d == 1 || d == g.w { 1.0 } else { std::f64::consts::SQRT_2 }
                })
                .sum();
            steps >= 3.0 * half_width(j) + 2.0
        })
        .collect();
    // An end is free if it is a line end or the only remaining branch at a junction.
    let mut touches: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for (idx, closed) in &paths {
        if !closed {
            *touches.entry(idx[0]).or_default() += 1;
            *touches.entry(idx[idx.len() - 1]).or_default() += 1;
        }
    }
    let free = |i: usize| kind(i) <= 1 || touches.get(&i) == Some(&1);

    // Pass 2: geometry.
    for (idx, closed) in paths {
        // Grid (x, y) is the centre of mask pixel (x-1, y-1) → +0.5 − 1.
        let mut pts: Vec<Point> = idx.iter().map(|&i| Point::new((i % g.w) as f64 - 0.5, (i / g.w) as f64 - 0.5)).collect();
        // Thinning bends free ends of thick strokes; replace the last half
        // stroke width by a straight continuation of the line.
        if !closed {
            // Typical half-width along the line (the end pixel itself sits
            // near the stroke edge and would under-estimate it).
            let mut hw: Vec<f64> = idx.iter().map(|&i| half_width(i)).collect();
            hw.sort_by(|a, b| a.total_cmp(b));
            let t = 1.0 * hw[hw.len() / 2];
            if free(idx[idx.len() - 1]) {
                straighten_end(&mut pts, t);
            }
            if free(idx[0]) {
                pts.reverse();
                straighten_end(&mut pts, t);
                pts.reverse();
            }
        }
        let len: f64 = pts.windows(2).map(|w| (w[1] - w[0]).hypot()).sum();
        if pts.len() < 2 || len < min_len {
            continue;
        }
        let sm = smooth_open(&pts, p.smooth.min(len / 6.0));
        out.move_to(sm[0]);
        if p.curves && sm.len() > 2 && max_chord_dist(&sm) > p.tolerance * 0.5 {
            let n = sm.len();
            let k = 3.min(n - 1);
            let (t1, t2) = if closed {
                let t = unit(sm[1] - sm[n - 2]);
                (t, -t)
            } else {
                (unit(sm[k] - sm[0]), unit(sm[n - 1 - k] - sm[n - 1]))
            };
            fit_cubic(&sm, t1, t2, p.tolerance * p.tolerance, &mut out);
        } else {
            for q in rdp(&sm, p.tolerance.max(0.3)).iter().skip(1) {
                out.line_to(*q);
            }
        }
        if closed {
            out.close_path();
        }
        count += 1;
    }
    (out, count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Shape;

    fn params() -> ContourParams {
        ContourParams::from_smoothness(40.0, 1.0, true, 1.0)
    }

    #[test]
    fn thick_line_becomes_one_centered_path() {
        let mut m = Mask::new(80, 30);
        for y in 10..17 {
            for x in 10..70 {
                m.data[y * 80 + x] = true;
            }
        }
        let (path, n) = trace_centerline(&m, &params(), 3.0);
        assert_eq!(n, 1, "one stroke → one line");
        let b = path.bounding_box();
        assert!((b.y0 + b.y1) / 2.0 > 12.5 && (b.y0 + b.y1) / 2.0 < 14.5, "centred: {b:?}");
        assert!(b.height() < 2.0, "flat: {b:?}");
        assert!(b.width() > 50.0, "long: {b:?}");
    }

    #[test]
    fn plus_sign_gives_four_arms() {
        let mut m = Mask::new(60, 60);
        for y in 0..60 {
            for x in 0..60 {
                if (27..33).contains(&x) && (8..52).contains(&y) || (27..33).contains(&y) && (8..52).contains(&x) {
                    m.data[y * 60 + x] = true;
                }
            }
        }
        let (_, n) = trace_centerline(&m, &params(), 3.0);
        assert!((4..=6).contains(&n), "arms: {n}");
    }

    #[test]
    fn ring_becomes_closed_loop() {
        let mut m = Mask::new(60, 60);
        for y in 0..60 {
            for x in 0..60 {
                let d = ((x as f64 + 0.5 - 30.0).powi(2) + (y as f64 + 0.5 - 30.0).powi(2)).sqrt();
                m.data[y * 60 + x] = (17.0..22.0).contains(&d);
            }
        }
        let (path, n) = trace_centerline(&m, &params(), 3.0);
        assert_eq!(n, 1);
        let b = path.bounding_box();
        assert!((b.width() - 39.0).abs() < 3.0, "{b:?}");
    }
}
