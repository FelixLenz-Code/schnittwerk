//! SVG import: flattens groups/transforms/text into one compound path in mm.

use kurbo::{BezPath, Point};
use serde::Serialize;
use usvg::tiny_skia_path::PathSegment;

use crate::geom::{bbox, to_svg_d};

/// usvg works in CSS px (96 per inch).
const PX_TO_MM: f64 = 25.4 / 96.0;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedVector {
    /// Path data in mm.
    pub d: String,
    /// `[x, y, w, h]` in mm.
    pub bbox: [f64; 4],
    pub path_count: usize,
}

pub fn import_svg(data: &[u8]) -> crate::Result<ImportedVector> {
    let opt = usvg::Options { fontdb: crate::text::fonts(), ..Default::default() };
    import_svg_with(data, &opt)
}

pub fn import_svg_with(data: &[u8], opt: &usvg::Options) -> crate::Result<ImportedVector> {
    let tree = usvg::Tree::from_data(data, opt)?;
    let mut out = BezPath::new();
    let mut count = 0;
    walk(tree.root(), usvg::Transform::identity(), &mut out, &mut count);
    if count == 0 {
        return Err(crate::Error::Other("Die SVG-Datei enthält keine Pfade.".into()));
    }
    Ok(ImportedVector { d: to_svg_d(&out), bbox: bbox(&out), path_count: count })
}

/// `extra` is applied on top of each path's own transform; text glyphs
/// (`flattened`) carry only transforms relative to their text node.
fn walk(group: &usvg::Group, extra: usvg::Transform, out: &mut BezPath, count: &mut usize) {
    for node in group.children() {
        match node {
            usvg::Node::Group(g) => walk(g, extra, out, count),
            usvg::Node::Text(t) => walk(t.flattened(), extra.pre_concat(t.abs_transform()), out, count),
            usvg::Node::Path(p) => {
                if !p.is_visible() {
                    continue;
                }
                let Some(path) = p.data().clone().transform(extra.pre_concat(p.abs_transform())) else { continue };
                let mm = |pt: usvg::tiny_skia_path::Point| Point::new(pt.x as f64 * PX_TO_MM, pt.y as f64 * PX_TO_MM);
                for seg in path.segments() {
                    match seg {
                        PathSegment::MoveTo(a) => out.move_to(mm(a)),
                        PathSegment::LineTo(a) => out.line_to(mm(a)),
                        PathSegment::QuadTo(a, b) => out.quad_to(mm(a), mm(b)),
                        PathSegment::CubicTo(a, b, c) => out.curve_to(mm(a), mm(b), mm(c)),
                        PathSegment::Close => out.close_path(),
                    }
                }
                *count += 1;
            }
            usvg::Node::Image(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn imports_rect_in_mm() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="50mm" viewBox="0 0 100 50">
            <g transform="translate(10 5)"><rect width="20" height="10"/></g></svg>"#;
        let v = super::import_svg(svg).unwrap();
        assert_eq!(v.path_count, 1);
        assert!((v.bbox[0] - 10.0).abs() < 0.01 && (v.bbox[2] - 20.0).abs() < 0.01, "{:?}", v.bbox);
    }
}

#[cfg(test)]
mod text_tests {
    #[test]
    fn text_in_viewbox_svg_is_scaled() {
        let fam = crate::text::families();
        let Some(f) = fam.iter().find(|f| f.contains("DejaVu Sans") || f.contains("Noto Sans")).or(fam.first()) else { return };
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="100mm" viewBox="0 0 100 100"><text x="10" y="50" font-family="{f}" font-size="20">H</text></svg>"#
        );
        let v = super::import_svg(svg.as_bytes()).unwrap();
        // Capital H of a 20 mm font ≈ 14 mm tall.
        assert!(v.bbox[3] > 11.0 && v.bbox[3] < 17.0, "{:?}", v.bbox);
        assert!((v.bbox[0] - 10.0).abs() < 3.0, "{:?}", v.bbox);
    }
}
