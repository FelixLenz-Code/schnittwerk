//! Text → outlines with the system's fonts (via usvg's text layout).

use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};

use crate::svg_import::{import_svg_with, ImportedVector};

static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();

/// System font database, loaded once.
pub fn fonts() -> Arc<usvg::fontdb::Database> {
    FONTS
        .get_or_init(|| {
            let mut db = usvg::fontdb::Database::new();
            db.load_system_fonts();
            Arc::new(db)
        })
        .clone()
}

/// Sorted, de-duplicated font family names.
pub fn families() -> Vec<String> {
    let db = fonts();
    let mut v: Vec<String> = db.faces().filter_map(|f| f.families.first().map(|(n, _)| n.clone())).collect();
    v.sort_by_key(|s| s.to_lowercase());
    v.dedup();
    v
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TextSpec {
    pub content: String,
    pub family: String,
    /// Font size (em) in mm.
    pub size: f64,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    /// Extra letter spacing in mm.
    #[serde(default)]
    pub spacing: f64,
    /// Line height as a multiple of the size.
    #[serde(default = "default_line")]
    pub line_height: f64,
    /// "start" | "middle" | "end"
    #[serde(default = "default_align")]
    pub align: String,
    /// Text along this path (SVG path data in mm) instead of straight lines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Shift along the path in % of its length.
    #[serde(default)]
    pub path_offset: f64,
}

fn default_line() -> f64 {
    1.2
}
fn default_align() -> String {
    "start".into()
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Lays out the text and returns its outlines in mm.
pub fn text_to_path(t: &TextSpec) -> crate::Result<ImportedVector> {
    if t.content.trim().is_empty() {
        return Err(crate::Error::Other("Kein Text eingegeben.".into()));
    }
    // usvg works in CSS px (96/in); the importer converts px back to mm.
    let px = 96.0 / 25.4;
    let lines: Vec<&str> = t.content.lines().collect();
    let anchor = match t.align.as_str() {
        "middle" | "end" => t.align.as_str(),
        _ => "start",
    };
    let mut defs = String::new();
    let mut tspans = String::new();
    if let Some(d) = t.path.as_deref().filter(|d| !d.trim().is_empty()) {
        // One line along the path; the path is given in mm, the text in px.
        let guide = kurbo::BezPath::from_svg(d).map_err(|e| crate::Error::Other(format!("Ungültiger Textpfad: {e}")))?;
        let guide = kurbo::Affine::scale(px) * guide;
        defs = format!(r#"<defs><path id="guide" d="{}"/></defs>"#, crate::geom::to_svg_d(&guide));
        let base = match anchor {
            "middle" => 50.0,
            "end" => 100.0,
            _ => 0.0,
        };
        let line = lines.iter().map(|l| l.trim()).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ");
        tspans = format!(r##"<textPath href="#guide" startOffset="{}%">{}</textPath>"##, base + t.path_offset, escape(&line));
    } else {
        for (i, line) in lines.iter().enumerate() {
            let y = t.size * px * (1.0 + i as f64 * t.line_height);
            let text = if line.is_empty() { " " } else { line };
            tspans.push_str(&format!(r#"<tspan x="0" y="{y}">{}</tspan>"#, escape(text)));
        }
    }
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="40000" height="40000">{defs}<text font-family="{}" font-size="{}" font-weight="{}" font-style="{}" letter-spacing="{}" text-anchor="{anchor}">{tspans}</text></svg>"#,
        escape(&t.family),
        t.size * px,
        if t.bold { "bold" } else { "normal" },
        if t.italic { "italic" } else { "normal" },
        t.spacing * px,
    );
    let mut opt = usvg::Options { fontdb: fonts(), ..Default::default() };
    opt.font_family = t.family.clone();
    import_svg_with(svg.as_bytes(), &opt).map_err(|_| crate::Error::Other("Schrift enthält keine Zeichen für diesen Text.".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_text_with_some_font() {
        let fam = families();
        if fam.is_empty() {
            return; // no fonts on this machine
        }
        let family = fam.iter().find(|f| f.contains("DejaVu Sans") || f.contains("Noto Sans")).unwrap_or(&fam[0]).clone();
        let v = text_to_path(&TextSpec {
            content: "Hallo".into(),
            family,
            size: 20.0,
            bold: false,
            italic: false,
            spacing: 0.0,
            line_height: 1.2,
            align: "start".into(),
            path: None,
            path_offset: 0.0,
        })
        .unwrap();
        assert!(v.path_count >= 1);
        // Cap height of a 20 mm font is roughly 14 mm.
        assert!(v.bbox[3] > 8.0 && v.bbox[3] < 22.0, "{:?}", v.bbox);
    }

    #[test]
    fn text_follows_an_arc() {
        let fam = families();
        let Some(family) = fam.iter().find(|f| f.contains("DejaVu Sans") || f.contains("Noto Sans")).or(fam.first()).cloned() else { return };
        let spec = |path: Option<String>| TextSpec {
            content: "HALLO WELT".into(),
            family: family.clone(),
            size: 10.0,
            bold: false,
            italic: false,
            spacing: 0.0,
            line_height: 1.2,
            align: "middle".into(),
            path,
            path_offset: 0.0,
        };
        // Circle of radius 40 mm around (0, 0), starting at the bottom, clockwise.
        let arc = "M0 40A40 40 0 1 1 0 -40A40 40 0 1 1 0 40Z".to_string();
        let straight = text_to_path(&spec(None)).unwrap();
        let bent = text_to_path(&spec(Some(arc))).unwrap();
        // Centred on top of the circle: around x = 0, above y = -40 + cap height.
        let cx = bent.bbox[0] + bent.bbox[2] / 2.0;
        assert!(cx.abs() < 3.0, "{:?}", bent.bbox);
        assert!(bent.bbox[1] < -40.0 && bent.bbox[1] > -55.0, "{:?}", bent.bbox);
        // Bent text is taller than straight text (the ends go down).
        assert!(bent.bbox[3] > straight.bbox[3] + 2.0, "{:?} vs {:?}", bent.bbox, straight.bbox);
    }
}
