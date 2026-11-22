//! Sticker outlines: the motif is separated from its background, holes are
//! filled and the outline is grown by a margin, giving one smooth cut
//! contour around the whole sticker.

use image::{imageops::FilterType, DynamicImage, RgbaImage};
use kurbo::Affine;
use serde::{Deserialize, Serialize};

use crate::contour::{trace_field, ContourParams};
use crate::field::Field;
use crate::geom::{bbox, to_svg_d};
use crate::offset::{offset_field, pad_mask};
use crate::preprocess::Mask;
use crate::trace::{TraceLayer, TraceResult};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum MotifMask {
    /// Non-transparent pixels (PNG with transparency).
    Alpha,
    /// Everything that differs from the background colour at the border.
    Background { tolerance: f32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StickerParams {
    pub mask: MotifMask,
    /// Margin around the motif in source pixels (may be 0).
    pub offset_px: f64,
    #[serde(default = "default_smooth")]
    pub smoothness: f32,
    /// Fill holes so the sticker is one solid piece.
    #[serde(default = "default_true")]
    pub fill_holes: bool,
    /// Drop motif specks smaller than this (source px²) before offsetting.
    #[serde(default = "default_speck")]
    pub despeckle: f64,
}

fn default_smooth() -> f32 { 60.0 }
fn default_true() -> bool { true }
fn default_speck() -> f64 { 30.0 }

/// Work resolution for the distance transform.
const MAX_WORK: u32 = 1600;

/// Colour that dominates the image border.
fn border_color(img: &RgbaImage) -> [u8; 3] {
    let (w, h) = (img.width(), img.height());
    let mut samples: Vec<[u8; 3]> = Vec::new();
    for x in 0..w {
        for y in [0, h - 1] {
            let p = img.get_pixel(x, y);
            samples.push([p[0], p[1], p[2]]);
        }
    }
    for y in 0..h {
        for x in [0, w - 1] {
            let p = img.get_pixel(x, y);
            samples.push([p[0], p[1], p[2]]);
        }
    }
    // Median per channel is robust against a motif touching the border.
    let mut out = [0u8; 3];
    for (c, o) in out.iter_mut().enumerate() {
        let mut v: Vec<u8> = samples.iter().map(|s| s[c]).collect();
        v.sort_unstable();
        *o = v[v.len() / 2];
    }
    out
}

fn motif_mask(img: &RgbaImage, mode: MotifMask) -> Mask {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let mut m = Mask::new(w, h);
    match mode {
        MotifMask::Alpha => {
            for (i, p) in img.pixels().enumerate() {
                m.data[i] = p[3] > 127;
            }
        }
        MotifMask::Background { tolerance } => {
            let bg = border_color(img);
            let max = (tolerance.clamp(1.0, 100.0) / 100.0 * 441.7).powi(2);
            for (i, p) in img.pixels().enumerate() {
                let d: f32 = (0..3).map(|k| (p[k] as f32 - bg[k] as f32).powi(2)).sum();
                m.data[i] = p[3] > 127 && d > max;
            }
        }
    }
    m
}

/// Removes connected motif regions smaller than `min_area` px.
fn despeckle(m: &mut Mask, min_area: usize) {
    if min_area <= 1 {
        return;
    }
    let (w, h) = (m.width, m.height);
    let mut seen = vec![false; w * h];
    let mut stack = Vec::new();
    let mut region = Vec::new();
    for start in 0..w * h {
        if !m.data[start] || seen[start] {
            continue;
        }
        region.clear();
        stack.push(start);
        seen[start] = true;
        while let Some(i) = stack.pop() {
            region.push(i);
            let (x, y) = (i % w, i / w);
            let mut visit = |j: usize| {
                if m.data[j] && !seen[j] {
                    seen[j] = true;
                    stack.push(j);
                }
            };
            if x > 0 { visit(i - 1); }
            if x + 1 < w { visit(i + 1); }
            if y > 0 { visit(i - w); }
            if y + 1 < h { visit(i + w); }
        }
        if region.len() < min_area {
            for &i in &region {
                m.data[i] = false;
            }
        }
    }
}

/// Traces the sticker contour; coordinates are in source pixels and may lie
/// outside the image when the margin extends past its edge.
pub fn sticker_outline(img: &DynamicImage, p: &StickerParams) -> TraceResult {
    let (w0, h0) = (img.width(), img.height());
    let work = if w0.max(h0) > MAX_WORK { img.resize(MAX_WORK, MAX_WORK, FilterType::Triangle) } else { img.clone() };
    let scale = w0 as f64 / work.width() as f64;
    let rgba = work.to_rgba8();
    let mut mask = motif_mask(&rgba, p.mask);
    despeckle(&mut mask, (p.despeckle / (scale * scale)).round() as usize);
    if p.fill_holes {
        mask.fill_holes();
    }
    let r = (p.offset_px / scale).max(0.0);
    // Room for the margin to grow beyond the image edge.
    let pad = (r.ceil() as usize) + 4;
    let padded = pad_mask(&mask, pad);
    let mut field: Field = if r < 0.5 { Field::from_mask(&padded, 0.6) } else { offset_field(&padded, r) };
    if p.fill_holes {
        field.fill_holes();
    }
    let unit = (1.0 / scale).max(1.0);
    let cp = ContourParams::from_smoothness(p.smoothness, (16.0 / (scale * scale)).max(2.0), true, unit);
    let (path, contours) = trace_field(&field, &cp);
    let to_source = Affine::scale(scale) * Affine::translate((-(pad as f64), -(pad as f64)));
    let path = to_source * path;
    let layers = if contours == 0 {
        vec![]
    } else {
        vec![TraceLayer { d: to_svg_d(&path), color: "#e5484d".into(), bbox: bbox(&path), contours }]
    };
    TraceResult { width: w0, height: h0, layers, threshold: None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn two_blobs_merge_into_one_sticker() {
        // Two dots 20 px apart on white; a 15 px margin joins them.
        let mut img = RgbaImage::from_pixel(120, 80, Rgba([255, 255, 255, 255]));
        for y in 0..80u32 {
            for x in 0..120u32 {
                let d1 = ((x as f64 - 40.0).powi(2) + (y as f64 - 40.0).powi(2)).sqrt();
                let d2 = ((x as f64 - 80.0).powi(2) + (y as f64 - 40.0).powi(2)).sqrt();
                if d1 < 10.0 || d2 < 10.0 {
                    img.put_pixel(x, y, Rgba([200, 20, 20, 255]));
                }
            }
        }
        let p = StickerParams { mask: MotifMask::Background { tolerance: 20.0 }, offset_px: 15.0, smoothness: 50.0, fill_holes: true, despeckle: 4.0 };
        let r = sticker_outline(&DynamicImage::ImageRgba8(img), &p);
        assert_eq!(r.layers.len(), 1);
        assert_eq!(r.layers[0].contours, 1, "one merged contour");
        let b = r.layers[0].bbox;
        assert!((b[0] - 15.0).abs() < 1.5 && (b[2] - 90.0).abs() < 2.5, "{b:?}");
    }

    #[test]
    fn alpha_outline_without_margin() {
        let mut img = RgbaImage::from_pixel(60, 60, Rgba([0, 0, 0, 0]));
        for y in 10..50 {
            for x in 10..50 {
                img.put_pixel(x, y, Rgba([0, 100, 255, 255]));
            }
        }
        let p = StickerParams { mask: MotifMask::Alpha, offset_px: 0.0, smoothness: 40.0, fill_holes: true, despeckle: 4.0 };
        let r = sticker_outline(&DynamicImage::ImageRgba8(img), &p);
        let b = r.layers[0].bbox;
        assert!((b[0] - 10.0).abs() < 0.6 && (b[2] - 40.0).abs() < 1.0, "{b:?}");
    }
}

// ---------------------------------------------------------------- bleed

/// Extends the motif's edge colours outwards by `bleed_px`, so a slightly
/// misaligned cut never shows a white edge. The image is padded by
/// `bleed_px` on every side; returns the new image.
pub fn bleed(img: &DynamicImage, mode: MotifMask, bleed_px: u32) -> RgbaImage {
    use std::collections::VecDeque;
    let src = img.to_rgba8();
    let pad = bleed_px;
    let (w0, h0) = (src.width(), src.height());
    let (w, h) = (w0 + 2 * pad, h0 + 2 * pad);
    let motif = motif_mask(&src, mode);
    let mut out = RgbaImage::new(w, h);
    // dist = BFS steps from the motif (u32::MAX = unreached).
    let mut dist = vec![u32::MAX; (w * h) as usize];
    let mut queue = VecDeque::new();
    for y in 0..h0 {
        for x in 0..w0 {
            let p = *src.get_pixel(x, y);
            let (ox, oy) = (x + pad, y + pad);
            if motif.get(x as usize, y as usize) {
                out.put_pixel(ox, oy, image::Rgba([p[0], p[1], p[2], 255]));
                let i = (oy * w + ox) as usize;
                dist[i] = 0;
                queue.push_back((ox, oy));
            } else if matches!(mode, MotifMask::Background { .. }) {
                // Keep the original background where no bleed lands.
                out.put_pixel(ox, oy, p);
            }
        }
    }
    let limit = bleed_px.max(1);
    while let Some((x, y)) = queue.pop_front() {
        let i = (y * w + x) as usize;
        let d = dist[i];
        if d >= limit {
            continue;
        }
        let c = *out.get_pixel(x, y);
        for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                continue;
            }
            let j = (ny as u32 * w + nx as u32) as usize;
            if dist[j] == u32::MAX {
                dist[j] = d + 1;
                out.put_pixel(nx as u32, ny as u32, c);
                queue.push_back((nx as u32, ny as u32));
            }
        }
    }
    out
}

#[cfg(test)]
mod bleed_tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn colours_spread_into_transparent_area() {
        let mut img = RgbaImage::from_pixel(20, 20, Rgba([0, 0, 0, 0]));
        for y in 5..15 {
            for x in 5..15 {
                img.put_pixel(x, y, Rgba([10, 200, 30, 255]));
            }
        }
        let out = bleed(&DynamicImage::ImageRgba8(img), MotifMask::Alpha, 4);
        assert_eq!((out.width(), out.height()), (28, 28));
        // 3 px left of the motif (motif starts at 5+4 = 9 in the padded image).
        assert_eq!(*out.get_pixel(6, 15), Rgba([10, 200, 30, 255]));
        // Far corner stays transparent.
        assert_eq!(out.get_pixel(0, 0)[3], 0);
    }
}
