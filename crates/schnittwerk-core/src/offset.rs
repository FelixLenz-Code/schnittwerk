//! Outline offsetting via a Euclidean distance transform.
//!
//! A shape grown (or shrunk) by `r` is the set of pixels whose distance to
//! the shape is ≤ r. Building a scalar field from that distance and taking
//! its 0.5 iso-line (see `contour`) yields smooth, correctly rounded offset
//! outlines – merging nearby parts and closing small gaps on the way,
//! which is exactly what a sticker contour needs.

use crate::field::Field;
use crate::preprocess::Mask;

const INF: f32 = 1e20;

/// 1-D squared distance transform (Felzenszwalb & Huttenlocher 2012).
fn dt_1d(f: &[f32], out: &mut [f32], v: &mut [usize], z: &mut [f32]) {
    let n = f.len();
    let mut k = 0usize;
    v[0] = 0;
    z[0] = -INF;
    z[1] = INF;
    for q in 1..n {
        loop {
            let p = v[k];
            let s = ((f[q] + (q * q) as f32) - (f[p] + (p * p) as f32)) / (2.0 * (q as f32 - p as f32));
            if s <= z[k] && k > 0 {
                k -= 1;
            } else if s <= z[k] {
                // k == 0 and s <= -INF cannot happen for finite input; guard anyway.
                v[0] = q;
                z[1] = INF;
                break;
            } else {
                k += 1;
                v[k] = q;
                z[k] = s;
                z[k + 1] = INF;
                break;
            }
        }
    }
    k = 0;
    for (q, o) in out.iter_mut().enumerate().take(n) {
        while z[k + 1] < q as f32 {
            k += 1;
        }
        let d = q as f32 - v[k] as f32;
        *o = d * d + f[v[k]];
    }
}

/// Squared distance (px²) from every pixel to the nearest pixel where
/// `mask == target`.
pub fn edt_sq(mask: &Mask, target: bool) -> Vec<f32> {
    let (w, h) = (mask.width, mask.height);
    let mut g: Vec<f32> = mask.data.iter().map(|&b| if b == target { 0.0 } else { INF }).collect();
    let n = w.max(h);
    let (mut f, mut out, mut v, mut z) = (vec![0f32; n], vec![0f32; n], vec![0usize; n], vec![0f32; n + 1]);
    for x in 0..w {
        for y in 0..h {
            f[y] = g[y * w + x];
        }
        dt_1d(&f[..h], &mut out[..h], &mut v, &mut z);
        for y in 0..h {
            g[y * w + x] = out[y];
        }
    }
    for y in 0..h {
        f[..w].copy_from_slice(&g[y * w..(y + 1) * w]);
        dt_1d(&f[..w], &mut out[..w], &mut v, &mut z);
        g[y * w..(y + 1) * w].copy_from_slice(&out[..w]);
    }
    g
}

/// Field whose 0.5 iso-line is the mask outline offset by `r` px
/// (positive = outwards, negative = inwards).
pub fn offset_field(mask: &Mask, r: f64) -> Field {
    let r = r as f32;
    let data: Vec<f32> = if r >= 0.0 {
        // Distance from outside pixels to the shape edge ≈ √d − 0.5.
        edt_sq(mask, true)
            .iter()
            .map(|&d| {
                let edge = if d == 0.0 { -0.5 } else { d.sqrt() - 0.5 };
                0.5 + (r - edge) * 0.5
            })
            .collect()
    } else {
        edt_sq(mask, false)
            .iter()
            .map(|&d| {
                let edge = if d == 0.0 { -0.5 } else { d.sqrt() - 0.5 };
                0.5 + (edge + r) * 0.5
            })
            .collect()
    };
    Field { width: mask.width, height: mask.height, data }
}

/// Adds a transparent margin of `pad` px on every side.
pub fn pad_mask(mask: &Mask, pad: usize) -> Mask {
    let (w, h) = (mask.width + 2 * pad, mask.height + 2 * pad);
    let mut m = Mask::new(w, h);
    for y in 0..mask.height {
        for x in 0..mask.width {
            m.data[(y + pad) * w + x + pad] = mask.get(x, y);
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contour::{trace_field, ContourParams};
    use kurbo::Shape;

    #[test]
    fn edt_matches_brute_force() {
        let mut m = Mask::new(17, 11);
        m.data[3 * 17 + 4] = true;
        m.data[8 * 17 + 13] = true;
        let d = edt_sq(&m, true);
        for y in 0..11 {
            for x in 0..17 {
                let b = [(4.0, 3.0), (13.0, 8.0)]
                    .iter()
                    .map(|(px, py)| (x as f32 - px).powi(2) + (y as f32 - py).powi(2))
                    .fold(f32::MAX, f32::min);
                assert_eq!(d[y * 17 + x], b, "at {x},{y}");
            }
        }
    }

    #[test]
    fn square_grows_by_offset_with_round_corners() {
        let mut m = Mask::new(80, 80);
        for y in 30..50 {
            for x in 30..50 {
                m.data[y * 80 + x] = true;
            }
        }
        let f = offset_field(&m, 10.0);
        let (path, n) = trace_field(&f, &ContourParams::from_smoothness(40.0, 1.0, true, 1.0));
        assert_eq!(n, 1);
        let b = path.bounding_box();
        assert!((b.x0 - 20.0).abs() < 0.6 && (b.x1 - 60.0).abs() < 0.6, "{b:?}");
        // Rounded corners: area is less than the full 40×40 square.
        let area = path.area().abs();
        let expected = 20.0 * 20.0 + 4.0 * 20.0 * 10.0 + std::f64::consts::PI * 100.0;
        assert!((area - expected).abs() / expected < 0.03, "area {area} vs {expected}");
    }

    #[test]
    fn negative_offset_shrinks() {
        let mut m = Mask::new(60, 60);
        for y in 10..50 {
            for x in 10..50 {
                m.data[y * 60 + x] = true;
            }
        }
        let (path, _) = trace_field(&offset_field(&m, -5.0), &ContourParams::from_smoothness(40.0, 1.0, true, 1.0));
        let b = path.bounding_box();
        assert!((b.x0 - 15.0).abs() < 0.6 && (b.x1 - 45.0).abs() < 0.6, "{b:?}");
    }
}

// ---------------------------------------------------------------- vectors

use crate::contour::{trace_field, ContourParams};
use crate::cut::PathInput;
use crate::geom::{bbox, to_svg_d};
use kurbo::{Affine, BezPath, PathEl, Rect, Shape};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OffsetResult {
    /// Path data in mat millimetres.
    pub d: String,
    pub bbox: [f64; 4],
    pub contours: usize,
}

/// Longest edge of the raster used for vector offsets.
const VECTOR_RASTER: f64 = 3000.0;

/// Offsets vector paths (in mat mm after their matrices) by `offset_mm`.
pub fn offset_paths(inputs: &[PathInput], offset_mm: f64, smoothness: f32) -> Result<OffsetResult, String> {
    let mut all = BezPath::new();
    for inp in inputs {
        let p = BezPath::from_svg(&inp.d).map_err(|e| format!("Ungültige Pfaddaten: {e}"))?;
        all.extend((Affine::new(inp.matrix) * p).elements().iter().copied());
    }
    if all.elements().is_empty() {
        return Err("Keine Pfade ausgewählt.".into());
    }
    let b: Rect = all.bounding_box();
    let grow = offset_mm.max(0.0) + 1.0;
    let area = b.inflate(grow, grow);
    // Pixels per mm: fine for small parts, bounded for large ones.
    let k = (VECTOR_RASTER / area.width().max(area.height())).clamp(4.0, 40.0);
    let (w, h) = ((area.width() * k).ceil() as u32 + 1, (area.height() * k).ceil() as u32 + 1);
    let mut pix = tiny_skia::Pixmap::new(w, h).ok_or("Fläche zu groß")?;
    let mut pb = tiny_skia::PathBuilder::new();
    let to_px = Affine::scale(k) * Affine::translate((-area.x0, -area.y0));
    for el in (to_px * all.clone()).elements() {
        match *el {
            PathEl::MoveTo(p) => pb.move_to(p.x as f32, p.y as f32),
            PathEl::LineTo(p) => pb.line_to(p.x as f32, p.y as f32),
            PathEl::QuadTo(a, p) => pb.quad_to(a.x as f32, a.y as f32, p.x as f32, p.y as f32),
            PathEl::CurveTo(a, c, p) => pb.cubic_to(a.x as f32, a.y as f32, c.x as f32, c.y as f32, p.x as f32, p.y as f32),
            PathEl::ClosePath => pb.close(),
        }
    }
    let path = pb.finish().ok_or("Pfad konnte nicht gerastert werden")?;
    let mut paint = tiny_skia::Paint::default();
    paint.set_color_rgba8(0, 0, 0, 255);
    paint.anti_alias = true;
    pix.fill_path(&path, &paint, tiny_skia::FillRule::EvenOdd, tiny_skia::Transform::identity(), None);
    let mut mask = Mask::new(w as usize, h as usize);
    for (i, px) in pix.pixels().iter().enumerate() {
        mask.data[i] = px.alpha() > 127;
    }
    let field = offset_field(&mask, offset_mm * k);
    let cp = ContourParams::from_smoothness(smoothness, 4.0, true, 1.0);
    let (outline, contours) = trace_field(&field, &cp);
    let outline = to_px.inverse() * outline;
    Ok(OffsetResult { d: to_svg_d(&outline), bbox: bbox(&outline), contours })
}

#[cfg(test)]
mod vector_tests {
    use super::*;

    #[test]
    fn rectangle_offset_in_mm() {
        let inp = PathInput { d: "M0 0 L20 0 L20 10 L0 10 Z".into(), matrix: [1.0, 0.0, 0.0, 1.0, 50.0, 50.0] };
        let r = offset_paths(&[inp], 3.0, 40.0).unwrap();
        let b = r.bbox;
        assert!((b[0] - 47.0).abs() < 0.1 && (b[1] - 47.0).abs() < 0.1, "{b:?}");
        assert!((b[2] - 26.0).abs() < 0.15 && (b[3] - 16.0).abs() < 0.15, "{b:?}");
    }
}
