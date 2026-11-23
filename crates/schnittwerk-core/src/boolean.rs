//! Boolean path operations (union, difference, intersection, xor) on closed
//! paths, using the robust polygon overlay of `i_overlay`.

use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::single::SingleFloatOverlay;
use kurbo::{BezPath, Point};
use serde::Deserialize;

use crate::contour::rdp;
use crate::cut::{flatten_inputs, PathInput};
use crate::geom::{bbox, to_svg_d};
use crate::offset::OffsetResult;

type Shapes = Vec<Vec<Vec<[f64; 2]>>>;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BoolOp {
    /// Merge everything into one outline (weld).
    Union,
    /// Remove all other paths from the first (bottom-most) one.
    Difference,
    /// Keep only what all paths have in common.
    Intersect,
    /// Keep what is covered an odd number of times.
    Xor,
}

/// One object's closed contours, normalised with the even-odd rule so its
/// own holes are respected.
fn normalize(input: &PathInput) -> Result<Shapes, String> {
    let contours: Vec<Vec<[f64; 2]>> = flatten_inputs(std::slice::from_ref(input))?
        .into_iter()
        .filter(|p| p.closed && p.points.len() >= 3)
        .map(|p| p.points.iter().map(|q| [q.x, q.y]).collect())
        .collect();
    let empty: Vec<Vec<[f64; 2]>> = Vec::new();
    Ok(contours.overlay(&empty, OverlayRule::Subject, FillRule::EvenOdd))
}

/// Applies `op` to the inputs in order (the first is the base for difference).
pub fn boolean(inputs: &[PathInput], op: BoolOp) -> Result<OffsetResult, String> {
    if inputs.len() < 2 {
        return Err("Für diese Operation mindestens zwei Pfade auswählen.".into());
    }
    let shapes: Vec<Shapes> = inputs.iter().map(normalize).collect::<Result<_, _>>()?;
    let mut acc = shapes[0].clone();
    match op {
        BoolOp::Difference => {
            let mut rest = shapes[1].clone();
            for s in &shapes[2..] {
                rest = rest.overlay(s, OverlayRule::Union, FillRule::NonZero);
            }
            acc = acc.overlay(&rest, OverlayRule::Difference, FillRule::NonZero);
        }
        _ => {
            let rule = match op {
                BoolOp::Union => OverlayRule::Union,
                BoolOp::Intersect => OverlayRule::Intersect,
                BoolOp::Xor => OverlayRule::Xor,
                BoolOp::Difference => unreachable!(),
            };
            for s in &shapes[1..] {
                acc = acc.overlay(s, rule, FillRule::NonZero);
            }
        }
    }

    let mut out = BezPath::new();
    let mut contours = 0;
    for shape in &acc {
        for c in shape {
            if c.len() < 3 {
                continue;
            }
            // Drop collinear points left over from flattening.
            let mut ring: Vec<Point> = c.iter().map(|p| Point::new(p[0], p[1])).collect();
            ring.push(ring[0]);
            let simple = rdp(&ring, 0.005);
            out.move_to(simple[0]);
            for q in &simple[1..simple.len() - 1] {
                out.line_to(*q);
            }
            out.close_path();
            contours += 1;
        }
    }
    if contours == 0 {
        return Err("Das Ergebnis ist leer.".into());
    }
    Ok(OffsetResult { d: to_svg_d(&out), bbox: bbox(&out), contours })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Shape;

    fn square(x: f64, y: f64, s: f64) -> PathInput {
        PathInput { d: format!("M{x} {y} H{} V{} H{x} Z", x + s, y + s), matrix: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0] }
    }

    fn area(r: &OffsetResult) -> f64 {
        BezPath::from_svg(&r.d).unwrap().area().abs()
    }

    #[test]
    fn union_of_overlapping_squares() {
        let r = boolean(&[square(0.0, 0.0, 10.0), square(5.0, 5.0, 10.0)], BoolOp::Union).unwrap();
        assert_eq!(r.contours, 1);
        assert!((area(&r) - 175.0).abs() < 0.1, "{}", area(&r));
    }

    #[test]
    fn difference_punches_a_hole() {
        let r = boolean(&[square(0.0, 0.0, 20.0), square(5.0, 5.0, 10.0)], BoolOp::Difference).unwrap();
        assert_eq!(r.contours, 2, "outer + hole");
        // Hole is oriented opposite to the outline, so signed areas give 400 − 100.
        assert!((area(&r) - 300.0).abs() < 0.1, "{}", area(&r));
        assert!((r.bbox[2] - 20.0).abs() < 0.01);
    }

    #[test]
    fn intersection_and_xor() {
        let a = square(0.0, 0.0, 10.0);
        let b = square(5.0, 0.0, 10.0);
        let i = boolean(&[a.clone(), b.clone()], BoolOp::Intersect).unwrap();
        assert!((area(&i) - 50.0).abs() < 0.1);
        let x = boolean(&[a, b], BoolOp::Xor).unwrap();
        assert_eq!(x.contours, 2);
        assert!((x.bbox[2] - 15.0).abs() < 0.01);
    }
}
