//! Scalar "inside-ness" fields. A contour is the 0.5 iso-line of a field.
//!
//! Building the field straight from the image's grey values / alpha keeps
//! the anti-aliasing information, so edges land at sub-pixel positions
//! without blurring – which would erase thin strokes and small text.

use image::RgbaImage;

use crate::preprocess::{luminance, Mask};

pub const ISO: f32 = 0.5;

#[derive(Clone)]
pub struct Field {
    pub width: usize,
    pub height: usize,
    /// > 0.5 means inside. Values are not clamped (keeps interpolation linear).
    pub data: Vec<f32>,
}

impl Field {
    fn new(width: usize, height: usize, data: Vec<f32>) -> Self {
        Self { width, height, data }
    }

    /// Dark areas inside; the iso-line sits exactly at luminance == level.
    pub fn threshold(img: &RgbaImage, level: u8, invert: bool) -> Self {
        let lum = luminance(img);
        let l = level as f32 + 0.5;
        let data = lum
            .iter()
            .map(|&v| {
                let d = (l - v as f32) / 255.0;
                ISO + if invert { -d } else { d }
            })
            .collect();
        Self::new(img.width() as usize, img.height() as usize, data)
    }

    /// Opaque areas inside (iso at alpha ≈ 50 %).
    pub fn alpha(img: &RgbaImage) -> Self {
        let data = img.pixels().map(|p| p[3] as f32 / 255.0).collect();
        Self::new(img.width() as usize, img.height() as usize, data)
    }

    /// Pixels within `tolerance` % of `color` inside; linear in color distance.
    pub fn color_key(img: &RgbaImage, color: [u8; 3], tolerance: f32) -> Self {
        let max = tolerance.clamp(0.0, 100.0) / 100.0 * 441.7;
        let data = img
            .pixels()
            .map(|p| {
                if p[3] < 128 {
                    return -0.5;
                }
                let d = (0..3).map(|k| (p[k] as f32 - color[k] as f32).powi(2)).sum::<f32>().sqrt();
                ISO + (max - d) / 441.7
            })
            .collect();
        Self::new(img.width() as usize, img.height() as usize, data)
    }

    /// From a hard mask, softened slightly so edges are not stair-stepped.
    pub fn from_mask(mask: &Mask, sigma: f32) -> Self {
        let data: Vec<f32> = mask.data.iter().map(|&b| if b { 1.0 } else { 0.0 }).collect();
        let mut f = Self::new(mask.width, mask.height, data);
        f.blur(sigma);
        f
    }

    pub fn is_empty(&self) -> bool {
        !self.data.iter().any(|&v| v > ISO)
    }

    pub fn to_mask(&self) -> Mask {
        Mask { width: self.width, height: self.height, data: self.data.iter().map(|&v| v > ISO).collect() }
    }

    /// Separable gaussian blur.
    pub fn blur(&mut self, sigma: f32) {
        if sigma <= 0.05 {
            return;
        }
        let (w, h) = (self.width, self.height);
        let r = (sigma * 3.0).ceil() as isize;
        let k: Vec<f32> = (-r..=r).map(|i| (-((i * i) as f32) / (2.0 * sigma * sigma)).exp()).collect();
        let sum: f32 = k.iter().sum();
        let k: Vec<f32> = k.iter().map(|v| v / sum).collect();
        let clamp = |v: isize, n: usize| v.clamp(0, n as isize - 1) as usize;
        let mut tmp = vec![0f32; w * h];
        for y in 0..h {
            for x in 0..w {
                tmp[y * w + x] = k.iter().enumerate().map(|(i, kv)| self.data[y * w + clamp(x as isize + i as isize - r, w)] * kv).sum();
            }
        }
        for y in 0..h {
            for x in 0..w {
                self.data[y * w + x] = k.iter().enumerate().map(|(i, kv)| tmp[clamp(y as isize + i as isize - r, h) * w + x] * kv).sum();
            }
        }
    }

    /// Grey-value dilation (r > 0) or erosion (r < 0) with a square of radius |r|.
    pub fn morph(&mut self, r: i32) {
        if r == 0 {
            return;
        }
        let grow = r > 0;
        let r = r.unsigned_abs() as usize;
        let (w, h) = (self.width, self.height);
        let pick = |a: f32, b: f32| if grow { a.max(b) } else { a.min(b) };
        let pass = |src: &[f32], horizontal: bool| -> Vec<f32> {
            let mut out = vec![0f32; w * h];
            for y in 0..h {
                for x in 0..w {
                    let (lo, hi) = if horizontal {
                        (x.saturating_sub(r), (x + r).min(w - 1))
                    } else {
                        (y.saturating_sub(r), (y + r).min(h - 1))
                    };
                    let mut v = src[y * w + x];
                    for k in lo..=hi {
                        let i = if horizontal { y * w + k } else { k * w + x };
                        v = pick(v, src[i]);
                    }
                    out[y * w + x] = v;
                }
            }
            out
        };
        let tmp = pass(&self.data, true);
        self.data = pass(&tmp, false);
    }

    /// Fills enclosed holes (outside regions not connected to the border).
    pub fn fill_holes(&mut self) {
        let mut m = self.to_mask();
        let before = m.data.clone();
        m.fill_holes();
        for (i, (&was, &now)) in before.iter().zip(&m.data).enumerate() {
            if now && !was {
                self.data[i] = 1.0;
            }
        }
    }
}
