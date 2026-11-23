//! PixScan mat: find the printed frame in a photo of the mat, work out which
//! way round the mat lies, and straighten the photo into true millimetres.
//!
//! Coordinates in mm are *plotter coordinates relative to the frame*: x
//! across the mat, y along the feed direction, origin at the frame corner
//! next to the large square (the leading edge is y = 0, the edge with the
//! arrow pointing into the machine).

use image::{imageops, imageops::FilterType, DynamicImage, GrayImage, RgbaImage};
use serde::Serialize;

use crate::photo::{max_filter, warp, Homography, Quad};

/// Inner black frame line (mm): across the mat × along the feed.
pub const FRAME_W: f64 = 195.0;
pub const FRAME_H: f64 = 295.0;
/// Centre of the large orientation square, relative to the frame (mm).
const BIG_SQUARE: [f64; 2] = [1.1, -14.7];
/// Extra photo kept around the frame when straightening (mm).
pub const MARGIN: f64 = 8.0;

/// Frame mm (TL, TR, BR, BL) as a quad.
fn frame_rect() -> Quad {
    [[0.0, 0.0], [FRAME_W, 0.0], [FRAME_W, FRAME_H], [0.0, FRAME_H]]
}

#[derive(Debug, Clone, Serialize)]
pub struct Detection {
    /// Frame corners in image pixels: TL, TR, BR, BL in plotter orientation
    /// (TL = corner at the large square).
    pub frame: Quad,
    /// The large square was found, so the orientation is certain.
    pub oriented: bool,
}

/// Grey image with its paper-brightness estimate (both 0..255, f32).
struct Work {
    w: usize,
    h: usize,
    lum: Vec<f32>,
    bg: Vec<f32>,
}

impl Work {
    fn new(img: &DynamicImage, max_side: u32) -> (Work, f64) {
        let longest = img.width().max(img.height());
        let small = if longest > max_side { img.resize(max_side, max_side, FilterType::Triangle) } else { img.clone() };
        let scale = img.width() as f64 / small.width() as f64;
        let gray = small.to_luma8();
        let (w, h) = (gray.width() as usize, gray.height() as usize);
        // Paper brightness: thin dark lines vanish in a max filter.
        let f = (w.max(h) as f64 / 256.0).max(1.0);
        let (sw, sh) = (((w as f64 / f).round() as u32).max(4), ((h as f64 / f).round() as u32).max(4));
        let tiny = imageops::resize(&gray, sw, sh, FilterType::Triangle);
        let r = ((sw.max(sh) as f64 * 0.025).ceil() as i64).max(2);
        let bg_small = imageops::blur(&max_filter(&tiny, r), r as f32);
        let bg_img: GrayImage = imageops::resize(&bg_small, w as u32, h as u32, FilterType::Triangle);
        let lum = gray.pixels().map(|p| p.0[0] as f32).collect();
        let bg = bg_img.pixels().map(|p| (p.0[0] as f32).max(1.0)).collect();
        (Work { w, h, lum, bg }, scale)
    }

    /// Bilinear sample at a continuous position (pixel centres at i + 0.5).
    fn sample(&self, v: &[f32], x: f64, y: f64) -> Option<f32> {
        let (x, y) = (x - 0.5, y - 0.5);
        if x < 0.0 || y < 0.0 || x > (self.w - 1) as f64 || y > (self.h - 1) as f64 {
            return None;
        }
        let (x0, y0) = (x.floor() as usize, y.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(self.w - 1), (y0 + 1).min(self.h - 1));
        let (fx, fy) = ((x - x0 as f64) as f32, (y - y0 as f64) as f32);
        let g = |xx: usize, yy: usize| v[yy * self.w + xx];
        let top = g(x0, y0) + (g(x1, y0) - g(x0, y0)) * fx;
        let bot = g(x0, y1) + (g(x1, y1) - g(x0, y1)) * fx;
        Some(top + (bot - top) * fy)
    }

    /// 0 = paper, 1 = black.
    fn darkness(&self, x: f64, y: f64) -> Option<f32> {
        let l = self.sample(&self.lum, x, y)?;
        let b = self.sample(&self.bg, x, y)?;
        Some(((b - l) / b).clamp(0.0, 1.0))
    }
}

/// Finds the PixScan frame in a photo. `None` if no frame is visible.
pub fn detect(img: &DynamicImage) -> Option<Detection> {
    let (work, scale) = Work::new(img, 2000);
    // Largest candidates first; the frame is the first one whose sides are
    // thin lines with paper on both sides (the mat edge against the table is
    // a step, not a line, and fails here).
    let (frame, oriented) = rough_frames(&work)
        .iter()
        .find_map(|rough| refine(&work, rough).and_then(|q| orient(&work, &q)))?;
    let s = |p: [f64; 2]| [p[0] * scale, p[1] * scale];
    Some(Detection { frame: frame.map(s), oriented })
}

/// Frame candidates: large, sparse dark components, biggest first; corners
/// from extreme points, clockwise in the image.
fn rough_frames(wk: &Work) -> Vec<Quad> {
    let (w, h) = (wk.w, wk.h);
    let dark: Vec<bool> = (0..w * h).map(|i| wk.bg[i] > 60.0 && wk.lum[i] < wk.bg[i] * 0.65).collect();
    let mut label = vec![0u32; w * h];
    let mut stack = Vec::new();
    let mut cands: Vec<(f64, u32)> = Vec::new();
    let mut next = 0u32;
    for start in 0..w * h {
        if !dark[start] || label[start] != 0 {
            continue;
        }
        next += 1;
        label[start] = next;
        stack.push(start);
        let (mut count, mut x0, mut y0, mut x1, mut y1) = (0usize, w, h, 0usize, 0usize);
        while let Some(i) = stack.pop() {
            count += 1;
            let (x, y) = (i % w, i / w);
            x0 = x0.min(x);
            x1 = x1.max(x);
            y0 = y0.min(y);
            y1 = y1.max(y);
            for dy in -1i64..=1 {
                for dx in -1i64..=1 {
                    let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                    if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                        continue;
                    }
                    let j = ny as usize * w + nx as usize;
                    if dark[j] && label[j] == 0 {
                        label[j] = next;
                        stack.push(j);
                    }
                }
            }
        }
        let (bw, bh) = ((x1 - x0 + 1) as f64, (y1 - y0 + 1) as f64);
        let min_dim = w.min(h) as f64 * 0.25;
        let fill = count as f64 / (bw * bh);
        // The frame lies on the mat, so it never touches the photo border.
        let inside = x0 > 0 && y0 > 0 && x1 + 1 < w && y1 + 1 < h;
        if inside && bw >= min_dim && bh >= min_dim && fill < 0.35 {
            cands.push((bw * bh, next));
        }
    }
    cands.sort_by(|a, b| b.0.total_cmp(&a.0));
    cands.truncate(4);
    // Two quads per candidate: extremes along the diagonals and the axes.
    let n = cands.len();
    let mut diag = vec![[(f64::MIN, [0.0, 0.0]); 4]; n];
    let mut axis = vec![[(f64::MIN, [0.0, 0.0]); 4]; n];
    for (i, l) in label.iter().enumerate() {
        let Some(c) = cands.iter().position(|(_, id)| id == l) else { continue };
        let (x, y) = ((i % w) as f64 + 0.5, (i / w) as f64 + 0.5);
        let ds = [-x - y, x - y, x + y, -x + y];
        let as_ = [-y, x, y, -x];
        for k in 0..4 {
            if ds[k] > diag[c][k].0 {
                diag[c][k] = (ds[k], [x, y]);
            }
            if as_[k] > axis[c][k].0 {
                axis[c][k] = (as_[k], [x, y]);
            }
        }
    }
    (0..n)
        .map(|c| {
            let qd: Quad = diag[c].map(|e| e.1);
            let qa: Quad = axis[c].map(|e| e.1);
            if area(&qd) >= area(&qa) { qd } else { qa }
        })
        .collect()
}

fn area(q: &Quad) -> f64 {
    let mut a = 0.0;
    for i in 0..4 {
        let (p, n) = (q[i], q[(i + 1) % 4]);
        a += p[0] * n[1] - n[0] * p[1];
    }
    (a / 2.0).abs()
}

/// A line through `p` with unit direction `d`.
#[derive(Clone, Copy)]
struct Line {
    p: [f64; 2],
    d: [f64; 2],
}

fn intersect(a: Line, b: Line) -> Option<[f64; 2]> {
    let den = a.d[0] * b.d[1] - a.d[1] * b.d[0];
    if den.abs() < 1e-9 {
        return None;
    }
    let t = ((b.p[0] - a.p[0]) * b.d[1] - (b.p[1] - a.p[1]) * b.d[0]) / den;
    Some([a.p[0] + t * a.d[0], a.p[1] + t * a.d[1]])
}

/// Total least squares line through points.
fn fit_line(pts: &[[f64; 2]]) -> Option<Line> {
    if pts.len() < 2 {
        return None;
    }
    let n = pts.len() as f64;
    let (mx, my) = (pts.iter().map(|p| p[0]).sum::<f64>() / n, pts.iter().map(|p| p[1]).sum::<f64>() / n);
    let (mut sxx, mut sxy, mut syy) = (0.0, 0.0, 0.0);
    for p in pts {
        let (dx, dy) = (p[0] - mx, p[1] - my);
        sxx += dx * dx;
        sxy += dx * dy;
        syy += dy * dy;
    }
    let angle = 0.5 * (2.0 * sxy).atan2(sxx - syy);
    Some(Line { p: [mx, my], d: [angle.cos(), angle.sin()] })
}

fn line_dist(l: Line, p: [f64; 2]) -> f64 {
    ((p[0] - l.p[0]) * l.d[1] - (p[1] - l.p[1]) * l.d[0]).abs()
}

/// Sub-pixel edge lines: samples across each side of the rough quad, finds
/// the centre of the dark line there and fits a robust straight line.
fn refine(wk: &Work, q: &Quad) -> Option<Quad> {
    let diag = (wk.w as f64).hypot(wk.h as f64);
    let reach = (diag * 0.012).max(6.0);
    let mut lines = [Line { p: [0.0; 2], d: [1.0, 0.0] }; 4];
    for i in 0..4 {
        let (a, b) = (q[i], q[(i + 1) % 4]);
        let len = (b[0] - a[0]).hypot(b[1] - a[1]);
        let d = [(b[0] - a[0]) / len, (b[1] - a[1]) / len];
        let n = [-d[1], d[0]];
        let mut pts = Vec::new();
        let samples = 120;
        for k in 0..samples {
            let t = 0.06 + 0.88 * k as f64 / (samples - 1) as f64;
            let c = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            if let Some(off) = line_centre(wk, c, n, reach) {
                pts.push([c[0] + n[0] * off, c[1] + n[1] * off]);
            }
        }
        if pts.len() < samples / 3 {
            return None;
        }
        // Drop outliers (material touching the line, printing nearby).
        let mut line = fit_line(&pts)?;
        for _ in 0..3 {
            let mut res: Vec<f64> = pts.iter().map(|&p| line_dist(line, p)).collect();
            res.sort_by(f64::total_cmp);
            let lim = (res[res.len() / 2] * 2.5).max(0.75);
            let keep: Vec<[f64; 2]> = pts.iter().copied().filter(|&p| line_dist(line, p) <= lim).collect();
            if keep.len() < 15 {
                break;
            }
            line = fit_line(&keep)?;
        }
        lines[i] = line;
    }
    let mut out = [[0.0; 2]; 4];
    for i in 0..4 {
        out[i] = intersect(lines[(i + 3) % 4], lines[i])?;
    }
    // The refinement must stay close to the rough corners.
    for i in 0..4 {
        if (out[i][0] - q[i][0]).hypot(out[i][1] - q[i][1]) > reach * 2.0 {
            return None;
        }
    }
    Some(out)
}

/// Offset along `n` (within ±reach) of the dark line nearest to `c`.
fn line_centre(wk: &Work, c: [f64; 2], n: [f64; 2], reach: f64) -> Option<f64> {
    let step = 0.25;
    let m = (reach / step) as i64;
    let prof: Vec<(f64, f32)> = (-m..=m)
        .filter_map(|k| {
            let s = k as f64 * step;
            wk.darkness(c[0] + n[0] * s, c[1] + n[1] * s).map(|v| (s, v))
        })
        .collect();
    if prof.len() < 5 {
        return None;
    }
    // Local maxima that are clearly dark; the nearest one wins.
    let mut peak: Option<usize> = None;
    for i in 1..prof.len() - 1 {
        let v = prof[i].1;
        let is_peak = v > 0.3 && v >= prof[i - 1].1 && v >= prof[i + 1].1;
        if is_peak && peak.is_none_or(|p| prof[i].0.abs() < prof[p].0.abs()) {
            peak = Some(i);
        }
    }
    let p = peak?;
    // Centroid of the run above half the peak.
    let half = prof[p].1 * 0.5;
    let (mut lo, mut hi) = (p, p);
    while lo > 0 && prof[lo - 1].1 > half {
        lo -= 1;
    }
    while hi + 1 < prof.len() && prof[hi + 1].1 > half {
        hi += 1;
    }
    if lo == 0 || hi == prof.len() - 1 {
        return None; // runs off the window: not a thin line
    }
    let (mut sw, mut sv) = (0.0, 0.0);
    for &(s, v) in &prof[lo..=hi] {
        let wgt = (v - half) as f64;
        sw += wgt;
        sv += wgt * s;
    }
    (sw > 0.0).then(|| sv / sw)
}

/// Orders the corners so that TL is the corner at the large square and
/// TL→TR runs along a short side. Returns (quad, square found).
fn orient(wk: &Work, q: &Quad) -> Option<(Quad, bool)> {
    let side = |i: usize| (q[(i + 1) % 4][0] - q[i][0]).hypot(q[(i + 1) % 4][1] - q[i][1]);
    let (a, b) = (side(0) + side(2), side(1) + side(3));
    let ratio = a.max(b) / a.min(b).max(1e-9);
    if !(1.15..=2.0).contains(&ratio) {
        return None; // not the 195 × 295 frame
    }
    let short = if a < b { 0 } else { 1 };
    let mut best: Option<(f32, Quad)> = None;
    let mut scores = Vec::new();
    for k in [short, short + 2] {
        let cand: Quad = [q[k % 4], q[(k + 1) % 4], q[(k + 2) % 4], q[(k + 3) % 4]];
        let hm = Homography::from_points(&frame_rect(), &cand)?;
        // Mean darkness over a 4 × 4 mm patch at the square's position.
        let mut sum = 0.0;
        let mut cnt = 0;
        for iy in 0..9 {
            for ix in 0..9 {
                let (mx, my) = (BIG_SQUARE[0] - 2.0 + ix as f64 * 0.5, BIG_SQUARE[1] - 2.0 + iy as f64 * 0.5);
                let (x, y) = hm.apply(mx, my);
                if let Some(v) = wk.darkness(x, y) {
                    sum += v;
                    cnt += 1;
                }
            }
        }
        let score = if cnt > 40 { sum / cnt as f32 } else { 0.0 };
        scores.push(score);
        if best.as_ref().is_none_or(|(s, _)| score > *s) {
            best = Some((score, cand));
        }
    }
    let (top, quad) = best?;
    let other = scores.iter().copied().fold(f32::MAX, f32::min);
    Some((quad, top > 0.4 && top - other > 0.25))
}

/// Straightens the photo using the frame corners (plotter orientation).
/// The result covers the frame plus `MARGIN` mm on every side; returns the
/// image and its resolution in pixels per mm.
pub fn rectify(img: &DynamicImage, frame: &Quad) -> Option<(RgbaImage, f64)> {
    let hm = Homography::from_points(&frame_rect(), frame)?;
    let m = MARGIN;
    let ext = [[-m, -m], [FRAME_W + m, -m], [FRAME_W + m, FRAME_H + m], [-m, FRAME_H + m]];
    let quad: Quad = ext.map(|[x, y]| {
        let (u, v) = hm.apply(x, y);
        [u, v]
    });
    // Keep about the photo's own resolution, within sensible limits.
    let d = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]);
    let ppm = ((d(frame[0], frame[1]) + d(frame[3], frame[2])) / (2.0 * FRAME_W))
        .max((d(frame[0], frame[3]) + d(frame[1], frame[2])) / (2.0 * FRAME_H))
        .clamp(3.0, 12.0);
    let (w, h) = (((FRAME_W + 2.0 * m) * ppm).round() as u32, ((FRAME_H + 2.0 * m) * ppm).round() as u32);
    warp(img, &quad, w, h).map(|out| (out, ppm))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    /// A synthetic photo of the mat: lying sideways (leading edge on the
    /// right, like a typical photo), slightly tilted, on a grey table.
    fn mat_photo(flip: bool) -> (DynamicImage, Quad) {
        // Frame mm → photo px: rotate so the leading edge (y = 0) is on the
        // right, scale 3 px/mm, mild perspective.
        let dst: Quad = if !flip {
            [[1080.0, 190.0], [1090.0, 750.0], [220.0, 757.0], [233.0, 202.0]]
        } else {
            [[220.0, 757.0], [233.0, 202.0], [1080.0, 190.0], [1090.0, 750.0]]
        };
        let hm = Homography::from_points(&frame_rect(), &dst).unwrap();
        let inv = Homography::from_points(&dst, &frame_rect()).unwrap();
        let mut img = RgbaImage::from_pixel(1280, 960, Rgba([120, 125, 120, 255]));
        for (x, y, p) in img.enumerate_pixels_mut() {
            let (u, v) = inv.apply(x as f64 + 0.5, y as f64 + 0.5);
            let on_mat = (-16.0..211.0).contains(&u) && (-49.0..333.0).contains(&v);
            if !on_mat {
                continue;
            }
            let line = |a: f64| a.abs() < 0.25;
            let in_frame = (-0.25..=FRAME_W + 0.25).contains(&u) && (-0.25..=FRAME_H + 0.25).contains(&v);
            let frame = in_frame && (line(u) || line(u - FRAME_W) || line(v) || line(v - FRAME_H));
            let big = (u - BIG_SQUARE[0]).abs() < 3.0 && (v - BIG_SQUARE[1]).abs() < 3.0;
            let small = (u - 100.0).abs() < 1.5 && (v + 10.0).abs() < 1.5;
            // A dark sticker sheet touching the frame on one side.
            let sheet = (0.0..60.0).contains(&u) && (100.0..180.0).contains(&v);
            *p = if frame || big || small || sheet { Rgba([25, 25, 30, 255]) } else { Rgba([228, 226, 220, 255]) };
        }
        let _ = hm;
        (DynamicImage::ImageRgba8(img), dst)
    }

    #[test]
    fn finds_frame_and_orientation() {
        for flip in [false, true] {
            let (img, q) = mat_photo(flip);
            let det = detect(&img).expect("frame found");
            assert!(det.oriented, "orientation (flip {flip})");
            for i in 0..4 {
                let e = (det.frame[i][0] - q[i][0]).hypot(det.frame[i][1] - q[i][1]);
                assert!(e < 1.0, "flip {flip} corner {i}: {:?} vs {:?}", det.frame[i], q[i]);
            }
        }
    }

    #[test]
    fn rectified_frame_lands_at_margin() {
        let (img, q) = mat_photo(false);
        let (out, ppm) = rectify(&img, &q).unwrap();
        // The frame line sits MARGIN mm in from the left and top.
        let px = (MARGIN * ppm).round() as u32;
        let y = ((MARGIN + 250.0) * ppm) as u32; // below the dark sheet
        assert!(out.get_pixel(px, y).0[0] < 100, "left frame line");
        assert!(out.get_pixel(px + 6, y).0[0] > 180, "paper inside");
        let x = out.width() / 2 + 50;
        assert!(out.get_pixel(x, px).0[0] < 100, "top frame line");
    }

    #[test]
    fn plain_page_is_not_a_mat() {
        let img = RgbaImage::from_pixel(800, 600, Rgba([230, 230, 230, 255]));
        assert!(detect(&DynamicImage::ImageRgba8(img)).is_none());
    }

    /// Real photo check: `PIXSCAN_PHOTO=/path cargo test -p schnittwerk-core real_photo -- --nocapture`
    #[test]
    fn real_photo() {
        let Ok(path) = std::env::var("PIXSCAN_PHOTO") else { return };
        let img = image::open(&path).unwrap();
        let det = detect(&img).expect("frame found");
        println!("{det:?}");
        let (out, ppm) = rectify(&img, &det.frame).unwrap();
        println!("{}×{} px, {ppm:.2} px/mm", out.width(), out.height());
        if let Ok(o) = std::env::var("PIXSCAN_OUT") {
            out.save(o).unwrap();
        }
    }
}
