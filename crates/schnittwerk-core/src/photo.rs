//! Preparing photos of drawings: perspective correction (page corners →
//! rectangle) and lighting compensation (shadows, vignetting, uneven paper).

use image::{imageops, imageops::FilterType, DynamicImage, GrayImage, Luma, Rgba, RgbaImage};

use crate::preprocess;

/// Corner points in image pixels, order: top-left, top-right, bottom-right,
/// bottom-left.
pub type Quad = [[f64; 2]; 4];

/// Projective transform as a row-major 3×3 matrix (h33 = 1).
#[derive(Debug, Clone, Copy)]
pub struct Homography([f64; 9]);

impl Homography {
    /// Maps the four `src` points onto the four `dst` points.
    pub fn from_points(src: &Quad, dst: &Quad) -> Option<Homography> {
        // Solve A·h = b for h11..h32 (8 unknowns).
        let mut a = [[0.0f64; 9]; 8];
        for i in 0..4 {
            let [x, y] = src[i];
            let [u, v] = dst[i];
            a[2 * i] = [x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y, u];
            a[2 * i + 1] = [0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y, v];
        }
        // Gaussian elimination with partial pivoting.
        for col in 0..8 {
            let piv = (col..8).max_by(|&r1, &r2| a[r1][col].abs().total_cmp(&a[r2][col].abs()))?;
            if a[piv][col].abs() < 1e-12 {
                return None;
            }
            a.swap(col, piv);
            for r in 0..8 {
                if r != col {
                    let f = a[r][col] / a[col][col];
                    for c in col..9 {
                        a[r][c] -= f * a[col][c];
                    }
                }
            }
        }
        let mut h = [0.0; 9];
        for i in 0..8 {
            h[i] = a[i][8] / a[i][i];
        }
        h[8] = 1.0;
        Some(Homography(h))
    }

    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        let h = &self.0;
        let w = h[6] * x + h[7] * y + h[8];
        ((h[0] * x + h[1] * y + h[2]) / w, (h[3] * x + h[4] * y + h[5]) / w)
    }
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// Output size for a quad: the average lengths of opposite sides, or the
/// given aspect ratio (width / height) at the same area.
pub fn output_size(q: &Quad, aspect: Option<f64>, max_side: u32) -> (u32, u32) {
    let w = (dist(q[0], q[1]) + dist(q[3], q[2])) / 2.0;
    let h = (dist(q[0], q[3]) + dist(q[1], q[2])) / 2.0;
    let (mut w, mut h) = match aspect {
        Some(r) if r > 0.0 => {
            let area = (w * h).max(1.0);
            ((area * r).sqrt(), (area / r).sqrt())
        }
        _ => (w, h),
    };
    let longest = w.max(h);
    if longest > max_side as f64 {
        let f = max_side as f64 / longest;
        w *= f;
        h *= f;
    }
    (w.round().max(1.0) as u32, h.round().max(1.0) as u32)
}

/// Warps the quad `q` of `img` onto an upright `out_w × out_h` image
/// (bilinear sampling; outside the source becomes transparent).
pub fn warp(img: &DynamicImage, q: &Quad, out_w: u32, out_h: u32) -> Option<RgbaImage> {
    let src = img.to_rgba8();
    let (w, h) = (out_w as f64, out_h as f64);
    let rect: Quad = [[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]];
    // Inverse mapping: output pixel → source position.
    let hm = Homography::from_points(&rect, q)?;
    let (sw, sh) = (src.width() as i64, src.height() as i64);
    let mut out = RgbaImage::new(out_w, out_h);
    for oy in 0..out_h {
        for ox in 0..out_w {
            let (x, y) = hm.apply(ox as f64 + 0.5, oy as f64 + 0.5);
            let (x, y) = (x - 0.5, y - 0.5);
            let (x0, y0) = (x.floor() as i64, y.floor() as i64);
            if x0 < -1 || y0 < -1 || x0 >= sw || y0 >= sh {
                continue;
            }
            let (fx, fy) = (x - x0 as f64, y - y0 as f64);
            let px = |xx: i64, yy: i64| -> [f64; 4] {
                let p = src.get_pixel(xx.clamp(0, sw - 1) as u32, yy.clamp(0, sh - 1) as u32).0;
                [p[0] as f64, p[1] as f64, p[2] as f64, p[3] as f64]
            };
            let (a, b, c, d) = (px(x0, y0), px(x0 + 1, y0), px(x0, y0 + 1), px(x0 + 1, y0 + 1));
            let mut v = [0u8; 4];
            for k in 0..4 {
                let top = a[k] + (b[k] - a[k]) * fx;
                let bot = c[k] + (d[k] - c[k]) * fx;
                v[k] = (top + (bot - top) * fy).round().clamp(0.0, 255.0) as u8;
            }
            out.put_pixel(ox, oy, Rgba(v));
        }
    }
    Some(out)
}

/// Finds the corners of a sheet of paper lying on a darker background.
/// Returns `None` if no clear sheet is visible.
pub fn detect_page(img: &DynamicImage) -> Option<Quad> {
    const WORK: u32 = 500;
    let (w0, h0) = (img.width(), img.height());
    let small = img.resize(WORK, WORK, FilterType::Triangle).to_rgba8();
    let small = imageops::blur(&small, 1.5);
    let (w, h) = (small.width() as usize, small.height() as usize);
    let lum = preprocess::luminance(&small);
    let level = preprocess::otsu(&lum);
    // Bright pixels = paper candidates; take the largest connected region.
    let bright: Vec<bool> = lum.iter().map(|&l| l > level).collect();
    let mut label = vec![0u32; w * h];
    let mut best = (0usize, 0u32);
    let mut next = 0u32;
    let mut stack = Vec::new();
    for start in 0..w * h {
        if !bright[start] || label[start] != 0 {
            continue;
        }
        next += 1;
        let mut size = 0;
        label[start] = next;
        stack.push(start);
        while let Some(i) = stack.pop() {
            size += 1;
            let (x, y) = (i % w, i / w);
            let mut visit = |j: usize| {
                if bright[j] && label[j] == 0 {
                    label[j] = next;
                    stack.push(j);
                }
            };
            if x > 0 { visit(i - 1); }
            if x + 1 < w { visit(i + 1); }
            if y > 0 { visit(i - w); }
            if y + 1 < h { visit(i + w); }
        }
        if size > best.0 {
            best = (size, next);
        }
    }
    let (size, id) = best;
    // The sheet must be a sizeable part of the photo, but not all of it
    // (then the photo is already cropped to the page).
    let frac = size as f64 / (w * h) as f64;
    if !(0.15..=0.97).contains(&frac) {
        return None;
    }
    // Corners: extreme points along the diagonals.
    let mut c = [(f64::MIN, [0.0, 0.0]); 4];
    for (i, _) in label.iter().enumerate().filter(|(_, l)| **l == id) {
        let (x, y) = ((i % w) as f64, (i / w) as f64);
        let scores = [-x - y, x - y, x + y, -x + y];
        for k in 0..4 {
            if scores[k] > c[k].0 {
                c[k] = (scores[k], [x, y]);
            }
        }
    }
    let sx = w0 as f64 / w as f64;
    let sy = h0 as f64 / h as f64;
    let q: Quad = c.map(|(_, [x, y])| [(x + 0.5) * sx, (y + 0.5) * sy]);
    // Reject degenerate results (corners too close together).
    let min_side = (0..4).map(|i| dist(q[i], q[(i + 1) % 4])).fold(f64::MAX, f64::min);
    if min_side < 0.1 * w0.min(h0) as f64 {
        return None;
    }
    Some(q)
}

/// Evens out lighting on a photographed drawing: estimates the paper
/// brightness (dark strokes removed by a max filter, then smoothed) and
/// divides it out, so shadows and gradients become white paper.
pub fn flatten_light(img: &RgbaImage) -> RgbaImage {
    let (w, h) = img.dimensions();
    if w < 8 || h < 8 {
        return img.clone();
    }
    // Background estimate on a small copy (fast, and smooth by nature).
    let f = (w.max(h) as f64 / 256.0).max(1.0);
    let (sw, sh) = (((w as f64 / f).round() as u32).max(4), ((h as f64 / f).round() as u32).max(4));
    let lum = preprocess::luminance(img);
    let gray = GrayImage::from_raw(w, h, lum.clone()).expect("size matches");
    let small = imageops::resize(&gray, sw, sh, FilterType::Triangle);
    // Strokes up to ~3 % of the image size disappear in the max filter.
    let r = ((sw.max(sh) as f64 * 0.03).ceil() as i64).max(2);
    let dil = max_filter(&small, r);
    let smooth = imageops::blur(&dil, r as f32);
    let bg = imageops::resize(&smooth, w, h, FilterType::Triangle);
    let mut out = img.clone();
    for (x, y, p) in out.enumerate_pixels_mut() {
        let b = bg.get_pixel(x, y).0[0].max(16) as f32;
        let gain = 255.0 / b;
        for c in p.0.iter_mut().take(3) {
            *c = (*c as f32 * gain).round().min(255.0) as u8;
        }
    }
    out
}

/// Separable square max filter with radius `r`.
fn max_filter(img: &GrayImage, r: i64) -> GrayImage {
    let (w, h) = (img.width() as i64, img.height() as i64);
    let get = |im: &GrayImage, x: i64, y: i64| im.get_pixel(x.clamp(0, w - 1) as u32, y.clamp(0, h - 1) as u32).0[0];
    let mut tmp = GrayImage::new(w as u32, h as u32);
    for y in 0..h {
        for x in 0..w {
            let m = (-r..=r).map(|d| get(img, x + d, y)).max().unwrap_or(0);
            tmp.put_pixel(x as u32, y as u32, Luma([m]));
        }
    }
    let mut out = GrayImage::new(w as u32, h as u32);
    for y in 0..h {
        for x in 0..w {
            let m = (-r..=r).map(|d| get(&tmp, x, y + d)).max().unwrap_or(0);
            out.put_pixel(x as u32, y as u32, Luma([m]));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn homography_maps_corners() {
        let src: Quad = [[10.0, 20.0], [200.0, 5.0], [220.0, 180.0], [0.0, 160.0]];
        let dst: Quad = [[0.0, 0.0], [100.0, 0.0], [100.0, 80.0], [0.0, 80.0]];
        let hm = Homography::from_points(&src, &dst).unwrap();
        for i in 0..4 {
            let (u, v) = hm.apply(src[i][0], src[i][1]);
            assert!((u - dst[i][0]).abs() < 1e-6 && (v - dst[i][1]).abs() < 1e-6);
        }
    }

    /// A white sheet (tilted quad) on a dark table.
    fn photo() -> (DynamicImage, Quad) {
        let q: Quad = [[60.0, 40.0], [330.0, 70.0], [310.0, 380.0], [40.0, 350.0]];
        let mut img = RgbaImage::from_pixel(400, 420, Rgba([40, 40, 45, 255]));
        let inside = |x: f64, y: f64| {
            (0..4).all(|i| {
                let (a, b) = (q[i], q[(i + 1) % 4]);
                (b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0]) >= 0.0
            })
        };
        for (x, y, p) in img.enumerate_pixels_mut() {
            if inside(x as f64, y as f64) {
                *p = Rgba([235, 235, 230, 255]);
            }
        }
        (DynamicImage::ImageRgba8(img), q)
    }

    #[test]
    fn detects_tilted_page() {
        let (img, q) = photo();
        let found = detect_page(&img).expect("page found");
        for i in 0..4 {
            assert!(dist(found[i], q[i]) < 6.0, "corner {i}: {:?} vs {:?}", found[i], q[i]);
        }
    }

    #[test]
    fn warp_makes_the_page_fill_the_output() {
        let (img, q) = photo();
        let (w, h) = output_size(&q, None, 2000);
        let out = warp(&img, &q, w, h).unwrap();
        // Well inside: all paper.
        for (x, y) in [(5, 5), (w - 6, 5), (w / 2, h / 2), (5, h - 6)] {
            assert!(out.get_pixel(x, y).0[0] > 200, "({x},{y}) = {:?}", out.get_pixel(x, y));
        }
    }

    #[test]
    fn flatten_removes_a_gradient_but_keeps_strokes() {
        // Paper getting darker to the right, with a black line.
        let mut img = RgbaImage::new(300, 200);
        for (x, y, p) in img.enumerate_pixels_mut() {
            let paper = 240.0 - x as f32 * 0.4; // 240 → 120
            let v = if (95..100).contains(&y) { 20.0 } else { paper };
            *p = Rgba([v as u8, v as u8, v as u8, 255]);
        }
        let out = flatten_light(&img);
        let l = |x, y| out.get_pixel(x, y).0[0];
        assert!(l(10, 40) > 225 && l(290, 40) > 225, "paper evened out: {} {}", l(10, 40), l(290, 40));
        assert!(l(150, 97) < 90, "stroke stays dark: {}", l(150, 97));
    }

    #[test]
    fn output_size_respects_aspect() {
        let q: Quad = [[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]];
        let (w, h) = output_size(&q, Some(210.0 / 297.0), 5000);
        assert!(((w as f64 / h as f64) - 210.0 / 297.0).abs() < 0.01);
    }
}
