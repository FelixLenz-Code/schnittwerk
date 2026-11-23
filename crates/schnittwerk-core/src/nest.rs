//! Shape nesting: places items by their true outline (not their bounding
//! box) so irregular stickers interlock and use less material.
//!
//! Items are rasterised at `RES` cells per mm and grown by the gap. Each
//! raster row is stored as intervals; the sheet keeps per-row prefix sums,
//! so testing a position costs one O(1) check per interval. Placement is
//! greedy bottom-left (largest items first), trying 0/90/180/270°.

use kurbo::{Affine, BezPath, PathEl, Point, Rect, Shape};
use serde::{Deserialize, Serialize};

use crate::cut::PathInput;
use crate::preprocess::Mask;

/// Raster cells per mm (0.5 mm grid).
const RES: f64 = 2.0;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NestItem {
    /// Closed outline(s) in mat mm.
    pub paths: Vec<PathInput>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NestRequest {
    pub items: Vec<NestItem>,
    /// Usable area `[x, y, w, h]` in mm.
    pub area: [f64; 4],
    /// Rectangles `[x, y, w, h]` that must stay free (e.g. mark zones).
    #[serde(default)]
    pub keepouts: Vec<[f64; 4]>,
    /// Minimum distance between items in mm.
    pub gap: f64,
    pub rotate: bool,
    /// Fill the area with copies of the first item instead of placing each once.
    #[serde(default)]
    pub fill: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Placement {
    pub item: usize,
    /// Rotation in degrees (clockwise) about `center`.
    pub rot: f64,
    pub center: [f64; 2],
    /// Translation applied after the rotation.
    pub dx: f64,
    pub dy: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NestResult {
    pub placements: Vec<Placement>,
    /// Items that did not fit.
    pub unplaced: Vec<usize>,
}

/// One rotated variant of an item, as row intervals.
struct Variant {
    rot: f64,
    w: usize,
    h: usize,
    /// rows[y] = [(x0, x1)) occupied intervals.
    rows: Vec<Vec<(usize, usize)>>,
    /// Top-left of the rotated outline bbox relative to the raster origin (mm).
    pad: f64,
    /// Rotated outline bbox top-left (mm, before translation).
    tl: Point,
    area: usize,
}

struct Sheet {
    w: usize,
    h: usize,
    /// prefix[y][x] = occupied cells in row y before x.
    prefix: Vec<Vec<u32>>,
    occ: Vec<Vec<bool>>,
}

impl Sheet {
    fn new(w: usize, h: usize) -> Self {
        Self { w, h, prefix: vec![vec![0; w + 1]; h], occ: vec![vec![false; w]; h] }
    }
    fn mark(&mut self, x0: usize, y0: usize, v: &Variant) {
        for (dy, row) in v.rows.iter().enumerate() {
            let y = y0 + dy;
            if y >= self.h {
                continue;
            }
            for &(a, b) in row {
                for x in (x0 + a)..(x0 + b).min(self.w) {
                    self.occ[y][x] = true;
                }
            }
            self.rebuild(y);
        }
    }
    fn block(&mut self, x0: usize, y0: usize, x1: usize, y1: usize) {
        for y in y0..y1.min(self.h) {
            for x in x0..x1.min(self.w) {
                self.occ[y][x] = true;
            }
            self.rebuild(y);
        }
    }
    fn rebuild(&mut self, y: usize) {
        let mut acc = 0;
        for x in 0..self.w {
            self.prefix[y][x] = acc;
            acc += self.occ[y][x] as u32;
        }
        self.prefix[y][self.w] = acc;
    }
    fn fits(&self, x0: usize, y0: usize, v: &Variant) -> bool {
        if x0 + v.w > self.w || y0 + v.h > self.h {
            return false;
        }
        v.rows.iter().enumerate().all(|(dy, row)| {
            let p = &self.prefix[y0 + dy];
            row.iter().all(|&(a, b)| p[x0 + b] == p[x0 + a])
        })
    }
    /// First free position scanning rows top-down, columns left-right.
    fn find(&self, v: &Variant) -> Option<(usize, usize)> {
        if v.w > self.w || v.h > self.h {
            return None;
        }
        for y in 0..=(self.h - v.h) {
            for x in 0..=(self.w - v.w) {
                if self.fits(x, y, v) {
                    return Some((x, y));
                }
            }
        }
        None
    }
}

fn outline(item: &NestItem) -> Result<BezPath, String> {
    let mut all = BezPath::new();
    for inp in &item.paths {
        let p = BezPath::from_svg(&inp.d).map_err(|e| format!("Ungültige Pfaddaten: {e}"))?;
        all.extend((Affine::new(inp.matrix) * p).elements().iter().copied());
    }
    Ok(all)
}

fn rasterize(path: &BezPath, gap: f64, rot: f64) -> Option<Variant> {
    let b0: Rect = path.bounding_box();
    let c = b0.center();
    let rotated = Affine::rotate_about(rot.to_radians(), c) * path.clone();
    let b = rotated.bounding_box();
    let pad = gap / 2.0 + 0.5 / RES;
    let w = ((b.width() + 2.0 * pad) * RES).ceil() as usize + 1;
    let h = ((b.height() + 2.0 * pad) * RES).ceil() as usize + 1;
    let mut pix = tiny_skia::Pixmap::new(w as u32, h as u32)?;
    let mut pb = tiny_skia::PathBuilder::new();
    let to_px = Affine::scale(RES) * Affine::translate((pad - b.x0, pad - b.y0));
    for el in (to_px * rotated).elements() {
        match *el {
            PathEl::MoveTo(p) => pb.move_to(p.x as f32, p.y as f32),
            PathEl::LineTo(p) => pb.line_to(p.x as f32, p.y as f32),
            PathEl::QuadTo(a, p) => pb.quad_to(a.x as f32, a.y as f32, p.x as f32, p.y as f32),
            PathEl::CurveTo(a, q, p) => pb.cubic_to(a.x as f32, a.y as f32, q.x as f32, q.y as f32, p.x as f32, p.y as f32),
            PathEl::ClosePath => pb.close(),
        }
    }
    let tp = pb.finish()?;
    let mut paint = tiny_skia::Paint::default();
    paint.set_color_rgba8(0, 0, 0, 255);
    paint.anti_alias = true;
    // Fill the outline and stroke it with the gap, so the gap is kept on all sides.
    pix.fill_path(&tp, &paint, tiny_skia::FillRule::Winding, tiny_skia::Transform::identity(), None);
    if gap > 0.0 {
        let stroke = tiny_skia::Stroke { width: (gap * RES) as f32, line_join: tiny_skia::LineJoin::Round, ..Default::default() };
        pix.stroke_path(&tp, &paint, &stroke, tiny_skia::Transform::identity(), None);
    }
    let mut mask = Mask::new(w, h);
    for (i, p) in pix.pixels().iter().enumerate() {
        mask.data[i] = p.alpha() > 0; // conservative: any coverage counts
    }
    let rows: Vec<Vec<(usize, usize)>> = (0..h)
        .map(|y| {
            let mut iv = Vec::new();
            let mut x = 0;
            while x < w {
                if mask.get(x, y) {
                    let s = x;
                    while x < w && mask.get(x, y) {
                        x += 1;
                    }
                    iv.push((s, x));
                } else {
                    x += 1;
                }
            }
            iv
        })
        .collect();
    let area = mask.count();
    Some(Variant { rot, w, h, rows, pad, tl: Point::new(b.x0, b.y0), area })
}

pub fn nest(req: &NestRequest) -> Result<NestResult, String> {
    let [ax, ay, aw, ah] = req.area;
    let (gw, gh) = ((aw * RES).floor() as usize, (ah * RES).floor() as usize);
    if gw == 0 || gh == 0 {
        return Err("Fläche zu klein.".into());
    }
    let mut sheet = Sheet::new(gw, gh);
    for k in &req.keepouts {
        let x0 = ((k[0] - ax) * RES).floor().max(0.0) as usize;
        let y0 = ((k[1] - ay) * RES).floor().max(0.0) as usize;
        let x1 = ((k[0] + k[2] - ax) * RES).ceil().max(0.0) as usize;
        let y1 = ((k[1] + k[3] - ay) * RES).ceil().max(0.0) as usize;
        sheet.block(x0, y0, x1, y1);
    }
    let rots: &[f64] = if req.rotate { &[0.0, 90.0, 180.0, 270.0] } else { &[0.0] };
    let mut variants: Vec<(usize, BezPath, Vec<Variant>)> = Vec::new();
    for (i, item) in req.items.iter().enumerate() {
        let path = outline(item)?;
        if path.elements().is_empty() {
            continue;
        }
        let vs: Vec<Variant> = rots.iter().filter_map(|&r| rasterize(&path, req.gap, r)).collect();
        variants.push((i, path, vs));
    }
    // Largest first packs better.
    variants.sort_by_key(|(_, _, vs)| std::cmp::Reverse(vs.first().map(|v| v.area).unwrap_or(0)));

    let mut placements = Vec::new();
    let mut unplaced = Vec::new();
    let place = |sheet: &mut Sheet, i: usize, path: &BezPath, vs: &[Variant]| -> Option<Placement> {
        // Bottom-left rule over all rotations: smallest y, then x.
        let best = vs.iter().filter_map(|v| sheet.find(v).map(|(x, y)| (y, x, v))).min_by_key(|(y, x, _)| (*y, *x))?;
        let (gy, gx, v) = best;
        sheet.mark(gx, gy, v);
        let c = path.bounding_box().center();
        let target = Point::new(ax + gx as f64 / RES + v.pad, ay + gy as f64 / RES + v.pad);
        Some(Placement { item: i, rot: v.rot, center: [c.x, c.y], dx: target.x - v.tl.x, dy: target.y - v.tl.y })
    };
    if req.fill {
        if let Some((i, path, vs)) = variants.first() {
            while let Some(p) = place(&mut sheet, *i, path, vs) {
                placements.push(p);
                if placements.len() > 2000 {
                    break;
                }
            }
        }
    } else {
        for (i, path, vs) in &variants {
            match place(&mut sheet, *i, path, vs) {
                Some(p) => placements.push(p),
                None => unplaced.push(*i),
            }
        }
    }
    Ok(NestResult { placements, unplaced })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(w: f64, h: f64) -> NestItem {
        NestItem { paths: vec![PathInput { d: format!("M0 0H{w}V{h}H0Z"), matrix: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0] }] }
    }

    #[test]
    fn fills_area_with_copies() {
        let req = NestRequest { items: vec![rect(20.0, 20.0)], area: [0.0, 0.0, 100.0, 50.0], keepouts: vec![], gap: 2.0, rotate: false, fill: true };
        let r = nest(&req).unwrap();
        // 22 mm pitch: 4 columns × 2 rows.
        assert_eq!(r.placements.len(), 8, "{}", r.placements.len());
    }

    #[test]
    fn rotates_long_items_to_fit() {
        let req = NestRequest { items: vec![rect(40.0, 10.0)], area: [0.0, 0.0, 15.0, 50.0], keepouts: vec![], gap: 1.0, rotate: true, fill: false };
        let r = nest(&req).unwrap();
        assert_eq!(r.placements.len(), 1);
        assert!((r.placements[0].rot - 90.0).abs() < 1e-9 || (r.placements[0].rot - 270.0).abs() < 1e-9);
    }

    #[test]
    fn l_shapes_interlock() {
        // Two L shapes (30×30, arm 10) fit in a 44×34 box only when interlocked
        // (one rotated 180°); side by side or stacked they would need > 60 mm.
        let l = NestItem { paths: vec![PathInput { d: "M0 0H10V20H30V30H0Z".into(), matrix: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0] }] };
        let req = NestRequest { items: vec![l.clone(), l], area: [0.0, 0.0, 44.0, 34.0], keepouts: vec![], gap: 1.0, rotate: true, fill: false };
        let r = nest(&req).unwrap();
        assert_eq!(r.placements.len(), 2, "unplaced: {:?}", r.unplaced);
    }

    #[test]
    fn keepouts_are_respected() {
        let req = NestRequest { items: vec![rect(10.0, 10.0)], area: [0.0, 0.0, 30.0, 12.0], keepouts: vec![[0.0, 0.0, 15.0, 12.0]], gap: 0.0, rotate: false, fill: false };
        let r = nest(&req).unwrap();
        let p = &r.placements[0];
        assert!(p.dx >= 15.0 - 1e-9, "{p:?}");
    }
}
