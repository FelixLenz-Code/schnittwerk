//! Raster → vector tracing.
//!
//! Pipeline: adjust tones → build one scalar field per output layer (from
//! grey values / alpha, so anti-aliasing gives sub-pixel edges) → grow /
//! shrink / fill holes → iso-contours → corner-aware Bézier fitting
//! (see `contour`) → SVG path data in source-pixel units.

use image::{imageops::FilterType, DynamicImage};
use kurbo::Affine;
use serde::{Deserialize, Serialize};

use crate::contour::{trace_field, ContourParams};
use crate::field::Field;
use crate::geom::{bbox, to_svg_d};
use crate::preprocess::{self, Mask};


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TraceMode {
    /// Dark areas become shapes. `level: None` picks the threshold automatically.
    Threshold { level: Option<u8>, invert: bool },
    /// Everything close to one picked color.
    #[serde(rename_all = "camelCase")]
    ColorKey { color: [u8; 3], tolerance: f32 },
    /// Outline of the non-transparent area (stickers, cut-out PNGs).
    Alpha,
    /// Reduce to N colors and trace each color as its own layer.
    #[serde(rename_all = "camelCase")]
    MultiColor { colors: u8, skip_background: bool },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceParams {
    pub mode: TraceMode,
    /// -100..100
    #[serde(default)]
    pub brightness: f32,
    /// -100..100
    #[serde(default)]
    pub contrast: f32,
    /// Gaussian blur sigma in source pixels.
    #[serde(default)]
    pub blur: f32,
    /// Regions smaller than this many px² (in source pixels) are dropped.
    #[serde(default = "default_despeckle")]
    pub despeckle: u32,
    /// 0 = follow every corner, 100 = very round.
    #[serde(default = "default_smoothness")]
    pub smoothness: f32,
    /// Grow (+) or shrink (-) shapes by this many source pixels.
    #[serde(default)]
    pub grow: i32,
    /// Only keep the outer outline (fill all holes).
    #[serde(default)]
    pub outer_only: bool,
    /// Fit curves (true) or straight polygon segments (false).
    #[serde(default = "default_true")]
    pub curves: bool,
    /// Longest edge of the work image, 0 = original resolution.
    #[serde(default = "default_max_size")]
    pub max_size: u32,
}

fn default_despeckle() -> u32 { 6 }
fn default_smoothness() -> f32 { 40.0 }
fn default_true() -> bool { true }
fn default_max_size() -> u32 { 2000 }

impl Default for TraceParams {
    fn default() -> Self {
        Self {
            mode: TraceMode::Threshold { level: None, invert: false },
            brightness: 0.0,
            contrast: 0.0,
            blur: 0.0,
            despeckle: default_despeckle(),
            smoothness: default_smoothness(),
            grow: 0,
            outer_only: false,
            curves: true,
            max_size: default_max_size(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceLayer {
    /// SVG path data in source-image pixel coordinates.
    pub d: String,
    /// `#rrggbb`
    pub color: String,
    /// `[x, y, w, h]` in source pixels.
    pub bbox: [f64; 4],
    /// Number of closed contours.
    pub contours: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceResult {
    pub width: u32,
    pub height: u32,
    pub layers: Vec<TraceLayer>,
    /// Threshold actually used (Threshold mode).
    pub threshold: Option<u8>,
}

/// Images with a shorter longest edge are upscaled before tracing.
const UPSCALE_BELOW: u32 = 1200;

pub fn trace(img: &DynamicImage, p: &TraceParams) -> TraceResult {
    let (w0, h0) = (img.width(), img.height());
    let longest = w0.max(h0);
    let work = if p.max_size > 0 && longest > p.max_size {
        img.resize(p.max_size, p.max_size, FilterType::Triangle)
    } else if longest < UPSCALE_BELOW {
        // Small images: bicubic upscaling interpolates the anti-aliased
        // edges, so contours of small details come out much cleaner.
        let f = (UPSCALE_BELOW as f64 / longest as f64).ceil().min(4.0) as u32;
        img.resize(w0 * f, h0 * f, FilterType::CatmullRom)
    } else {
        img.clone()
    };
    let scale = w0 as f64 / work.width() as f64;
    let rgba = preprocess::adjust(&work, p.brightness, p.contrast, p.blur / scale as f32);
    let min_area = p.despeckle as f64 / (scale * scale);
    let cp = ContourParams::from_smoothness(p.smoothness, min_area, p.curves, 1.0 / scale);

    let mut threshold = None;
    let fields: Vec<(Field, [u8; 3])> = match &p.mode {
        TraceMode::Threshold { level, invert } => {
            let lvl = level.unwrap_or_else(|| preprocess::otsu(&preprocess::luminance(&rgba)));
            threshold = Some(lvl);
            vec![(Field::threshold(&rgba, lvl, *invert), [0x22, 0x22, 0x22])]
        }
        TraceMode::ColorKey { color, tolerance } => {
            vec![(Field::color_key(&rgba, *color, *tolerance), *color)]
        }
        TraceMode::Alpha => vec![(Field::alpha(&rgba), [0x22, 0x22, 0x22])],
        TraceMode::MultiColor { colors, skip_background } => {
            let (palette, idx) = preprocess::quantize(&rgba, *colors as usize);
            let (w, h) = (rgba.width() as usize, rgba.height() as usize);
            let bg = if *skip_background { preprocess::border_majority(&idx, w, h) } else { None };
            palette
                .iter()
                .enumerate()
                .filter(|(i, _)| Some(*i as u8) != bg)
                .map(|(i, c)| {
                    let mut m = Mask::new(w, h);
                    for (j, v) in idx.iter().enumerate() {
                        m.data[j] = *v == i as u8;
                    }
                    (Field::from_mask(&m, 0.6), *c)
                })
                .collect()
        }
    };

    let to_source = Affine::scale(scale);
    let layers = fields
        .into_iter()
        .filter_map(|(mut field, color)| {
            field.morph((p.grow as f64 / scale).round() as i32);
            if p.outer_only {
                field.fill_holes();
            }
            if field.is_empty() {
                return None;
            }
            let (path, contours) = trace_field(&field, &cp);
            if contours == 0 {
                return None;
            }
            let path = to_source * path;
            Some(TraceLayer {
                d: to_svg_d(&path),
                color: format!("#{:02x}{:02x}{:02x}", color[0], color[1], color[2]),
                bbox: bbox(&path),
                contours,
            })
        })
        .collect();

    TraceResult { width: w0, height: h0, layers, threshold }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn ring() -> DynamicImage {
        let mut img = RgbaImage::from_pixel(200, 200, Rgba([255, 255, 255, 255]));
        for y in 0..200 {
            for x in 0..200 {
                let d = ((x as f64 - 100.0).powi(2) + (y as f64 - 100.0).powi(2)).sqrt();
                if d < 80.0 && d > 40.0 {
                    img.put_pixel(x, y, Rgba([10, 10, 10, 255]));
                }
            }
        }
        DynamicImage::ImageRgba8(img)
    }

    #[test]
    fn traces_ring_with_hole() {
        let r = trace(&ring(), &TraceParams::default());
        assert_eq!(r.layers.len(), 1);
        assert_eq!(r.layers[0].contours, 2, "outer + hole");
        let b = r.layers[0].bbox;
        assert!((b[2] - 160.0).abs() < 4.0, "width ~160, got {}", b[2]);
    }

    #[test]
    fn outer_only_drops_hole() {
        let p = TraceParams { outer_only: true, ..Default::default() };
        let r = trace(&ring(), &p);
        assert_eq!(r.layers[0].contours, 1);
    }

    #[test]
    fn downscaled_coords_map_back() {
        let p = TraceParams { max_size: 100, ..Default::default() };
        let r = trace(&ring(), &p);
        let b = r.layers[0].bbox;
        assert!((b[0] - 20.0).abs() < 4.0 && (b[2] - 160.0).abs() < 6.0, "{b:?}");
    }

    #[test]
    fn multicolor_skips_background() {
        let p = TraceParams {
            mode: TraceMode::MultiColor { colors: 2, skip_background: true },
            ..Default::default()
        };
        let r = trace(&ring(), &p);
        assert_eq!(r.layers.len(), 1);
    }
}
