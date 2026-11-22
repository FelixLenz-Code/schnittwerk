//! Small geometry helpers: building SVG path data with compact number
//! formatting while tracking the bounding box.

use kurbo::{BezPath, PathEl, Point, Rect, Shape};

/// Formats a coordinate with at most 2 decimals and no trailing zeros.
pub fn fmt_num(v: f64) -> String {
    let s = format!("{:.2}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".to_string()
    } else {
        s.to_string()
    }
}

fn pt(out: &mut String, p: Point) {
    out.push_str(&fmt_num(p.x));
    out.push(' ');
    out.push_str(&fmt_num(p.y));
}

/// Serializes a BezPath as absolute SVG path data.
pub fn to_svg_d(path: &BezPath) -> String {
    let mut out = String::new();
    for el in path.elements() {
        if !out.is_empty() {
            out.push(' ');
        }
        match *el {
            PathEl::MoveTo(p) => {
                out.push('M');
                pt(&mut out, p);
            }
            PathEl::LineTo(p) => {
                out.push('L');
                pt(&mut out, p);
            }
            PathEl::QuadTo(a, p) => {
                out.push('Q');
                pt(&mut out, a);
                out.push(' ');
                pt(&mut out, p);
            }
            PathEl::CurveTo(a, b, p) => {
                out.push('C');
                pt(&mut out, a);
                out.push(' ');
                pt(&mut out, b);
                out.push(' ');
                pt(&mut out, p);
            }
            PathEl::ClosePath => out.push('Z'),
        }
    }
    out
}

/// Bounding box as `[x, y, w, h]`; zero box for empty paths.
pub fn bbox(path: &BezPath) -> [f64; 4] {
    if path.elements().is_empty() {
        return [0.0; 4];
    }
    let r: Rect = path.bounding_box();
    [r.x0, r.y0, r.width(), r.height()]
}
