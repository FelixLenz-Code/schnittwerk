//! Image preparation before vectorizing: tone adjustments, thresholds,
//! masks and color quantization.

use image::{imageops, DynamicImage, RgbaImage};

/// A simple boolean mask, row-major.
#[derive(Clone)]
pub struct Mask {
    pub width: usize,
    pub height: usize,
    pub data: Vec<bool>,
}

impl Mask {
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height, data: vec![false; width * height] }
    }
    #[inline]
    pub fn get(&self, x: usize, y: usize) -> bool {
        self.data[y * self.width + x]
    }
    pub fn count(&self) -> usize {
        self.data.iter().filter(|v| **v).count()
    }

    /// Fills every background region that does not touch the image border,
    /// so only the outer contours remain.
    pub fn fill_holes(&mut self) {
        let (w, h) = (self.width, self.height);
        let mut reached = vec![false; w * h];
        let mut stack = Vec::new();
        let push = |x: usize, y: usize, stack: &mut Vec<usize>, reached: &mut Vec<bool>, data: &Vec<bool>| {
            let i = y * w + x;
            if !data[i] && !reached[i] {
                reached[i] = true;
                stack.push(i);
            }
        };
        for x in 0..w {
            push(x, 0, &mut stack, &mut reached, &self.data);
            push(x, h - 1, &mut stack, &mut reached, &self.data);
        }
        for y in 0..h {
            push(0, y, &mut stack, &mut reached, &self.data);
            push(w - 1, y, &mut stack, &mut reached, &self.data);
        }
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            if x > 0 { push(x - 1, y, &mut stack, &mut reached, &self.data); }
            if x + 1 < w { push(x + 1, y, &mut stack, &mut reached, &self.data); }
            if y > 0 { push(x, y - 1, &mut stack, &mut reached, &self.data); }
            if y + 1 < h { push(x, y + 1, &mut stack, &mut reached, &self.data); }
        }
        for i in 0..w * h {
            if !reached[i] {
                self.data[i] = true;
            }
        }
    }

    /// Grows (positive) or shrinks (negative) the mask by `r` pixels
    /// using a square structuring element (separable, fast).
    pub fn morph(&mut self, r: i32) {
        if r == 0 {
            return;
        }
        let grow = r > 0;
        let r = r.unsigned_abs() as usize;
        let (w, h) = (self.width, self.height);
        let pass = |src: &Vec<bool>, horizontal: bool| -> Vec<bool> {
            let mut out = vec![!grow; w * h];
            for y in 0..h {
                for x in 0..w {
                    let (lo, hi, fixed) = if horizontal {
                        (x.saturating_sub(r), (x + r).min(w - 1), y)
                    } else {
                        (y.saturating_sub(r), (y + r).min(h - 1), x)
                    };
                    let mut v = !grow;
                    for k in lo..=hi {
                        let i = if horizontal { fixed * w + k } else { k * w + fixed };
                        if src[i] == grow {
                            v = grow;
                            break;
                        }
                    }
                    out[y * w + x] = v;
                }
            }
            out
        };
        let tmp = pass(&self.data, true);
        self.data = pass(&tmp, false);
    }
}

/// Brightness/contrast (both -100..100) and optional blur (sigma in px).
/// Transparent pixels are kept transparent.
pub fn adjust(img: &DynamicImage, brightness: f32, contrast: f32, blur: f32) -> RgbaImage {
    let mut rgba = img.to_rgba8();
    if brightness != 0.0 || contrast != 0.0 {
        let c = ((100.0 + contrast.clamp(-99.0, 100.0)) / 100.0).powi(2);
        let b = brightness * 1.28;
        let mut lut = [0u8; 256];
        for (i, v) in lut.iter_mut().enumerate() {
            *v = (((i as f32 - 128.0) * c + 128.0 + b).round()).clamp(0.0, 255.0) as u8;
        }
        for p in rgba.pixels_mut() {
            p[0] = lut[p[0] as usize];
            p[1] = lut[p[1] as usize];
            p[2] = lut[p[2] as usize];
        }
    }
    if blur > 0.05 {
        rgba = imageops::blur(&rgba, blur);
    }
    rgba
}

/// Luminance (0..255) with transparent areas treated as white paper.
pub fn luminance(img: &RgbaImage) -> Vec<u8> {
    img.pixels()
        .map(|p| {
            let a = p[3] as f32 / 255.0;
            let l = 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32;
            (l * a + 255.0 * (1.0 - a)).round() as u8
        })
        .collect()
}

/// Otsu's method: threshold that best separates two tone groups.
pub fn otsu(lum: &[u8]) -> u8 {
    let mut hist = [0u64; 256];
    for &v in lum {
        hist[v as usize] += 1;
    }
    let total = lum.len() as f64;
    let sum_all: f64 = hist.iter().enumerate().map(|(i, &c)| i as f64 * c as f64).sum();
    let (mut sum_b, mut w_b, mut best, mut best_var) = (0.0, 0.0, 128u8, -1.0);
    for t in 0..256 {
        w_b += hist[t] as f64;
        if w_b == 0.0 {
            continue;
        }
        let w_f = total - w_b;
        if w_f == 0.0 {
            break;
        }
        sum_b += t as f64 * hist[t] as f64;
        let m_b = sum_b / w_b;
        let m_f = (sum_all - sum_b) / w_f;
        let var = w_b * w_f * (m_b - m_f).powi(2);
        if var > best_var {
            best_var = var;
            best = t as u8;
        }
    }
    best
}

pub fn threshold_mask(img: &RgbaImage, level: u8, invert: bool) -> Mask {
    let lum = luminance(img);
    let mut m = Mask::new(img.width() as usize, img.height() as usize);
    for (i, &l) in lum.iter().enumerate() {
        m.data[i] = (l <= level) != invert;
    }
    m
}

/// Opaque area of the image (for stickers with transparent background).
pub fn alpha_mask(img: &RgbaImage, level: u8) -> Mask {
    let mut m = Mask::new(img.width() as usize, img.height() as usize);
    for (i, p) in img.pixels().enumerate() {
        m.data[i] = p[3] > level;
    }
    m
}

/// Pixels whose color lies within `tolerance` (0..100 %) of `color`.
pub fn color_mask(img: &RgbaImage, color: [u8; 3], tolerance: f32) -> Mask {
    let max = (tolerance.clamp(0.0, 100.0) / 100.0 * 441.7).powi(2);
    let mut m = Mask::new(img.width() as usize, img.height() as usize);
    for (i, p) in img.pixels().enumerate() {
        if p[3] < 128 {
            continue;
        }
        let d: f32 = (0..3).map(|k| (p[k] as f32 - color[k] as f32).powi(2)).sum();
        m.data[i] = d <= max;
    }
    m
}

/// K-means color quantization. Returns the palette and a per-pixel palette
/// index (`u8::MAX` for transparent pixels).
pub fn quantize(img: &RgbaImage, k: usize) -> (Vec<[u8; 3]>, Vec<u8>) {
    let k = k.clamp(2, 16);
    let px: Vec<[f32; 3]> = img
        .pixels()
        .filter(|p| p[3] >= 128)
        .map(|p| [p[0] as f32, p[1] as f32, p[2] as f32])
        .collect();
    let mut idx = vec![u8::MAX; (img.width() * img.height()) as usize];
    if px.is_empty() {
        return (vec![], idx);
    }
    let step = (px.len() / 40_000).max(1);
    let sample: Vec<[f32; 3]> = px.iter().step_by(step).copied().collect();
    let dist = |a: &[f32; 3], b: &[f32; 3]| (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f32>();

    // Deterministic k-means++ style init: farthest-point seeding.
    let mut centers = vec![sample[0]];
    while centers.len() < k {
        let far = sample
            .iter()
            .max_by(|a, b| {
                let da = centers.iter().map(|c| dist(a, c)).fold(f32::MAX, f32::min);
                let db = centers.iter().map(|c| dist(b, c)).fold(f32::MAX, f32::min);
                da.total_cmp(&db)
            })
            .copied()
            .unwrap();
        centers.push(far);
    }
    for _ in 0..20 {
        let mut sums = vec![[0f64; 4]; k];
        for p in &sample {
            let c = nearest(&centers, p);
            for i in 0..3 {
                sums[c][i] += p[i] as f64;
            }
            sums[c][3] += 1.0;
        }
        let mut moved = 0.0;
        for (c, s) in centers.iter_mut().zip(&sums) {
            if s[3] > 0.0 {
                let n = [(s[0] / s[3]) as f32, (s[1] / s[3]) as f32, (s[2] / s[3]) as f32];
                moved += dist(c, &n);
                *c = n;
            }
        }
        if moved < 1.0 {
            break;
        }
    }
    for (i, p) in img.pixels().enumerate() {
        if p[3] >= 128 {
            idx[i] = nearest(&centers, &[p[0] as f32, p[1] as f32, p[2] as f32]) as u8;
        }
    }
    let palette = centers
        .iter()
        .map(|c| [c[0].round() as u8, c[1].round() as u8, c[2].round() as u8])
        .collect();
    (palette, idx)
}

fn nearest(centers: &[[f32; 3]], p: &[f32; 3]) -> usize {
    let mut best = (0, f32::MAX);
    for (i, c) in centers.iter().enumerate() {
        let d = (0..3).map(|k| (p[k] - c[k]).powi(2)).sum::<f32>();
        if d < best.1 {
            best = (i, d);
        }
    }
    best.0
}

/// Palette index that occurs most often along the image border.
pub fn border_majority(idx: &[u8], w: usize, h: usize) -> Option<u8> {
    let mut counts = [0usize; 256];
    let mut add = |i: usize| counts[idx[i] as usize] += 1;
    for x in 0..w {
        add(x);
        add((h - 1) * w + x);
    }
    for y in 0..h {
        add(y * w);
        add(y * w + w - 1);
    }
    let (best, n) = counts.iter().enumerate().max_by_key(|(_, n)| **n)?;
    if *n == 0 || best == 255 {
        None
    } else {
        Some(best as u8)
    }
}
