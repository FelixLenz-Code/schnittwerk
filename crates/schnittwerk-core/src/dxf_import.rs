//! DXF import: lines, polylines (with arc bulges), circles, arcs, ellipses,
//! splines and block references → one compound path in mm.

use std::f64::consts::TAU;

use dxf::entities::{EntityType, Spline};
use dxf::enums::Units;
use dxf::Drawing;
use kurbo::{Affine, Arc as KArc, BezPath, Point, Shape, Vec2};

use crate::geom::{bbox, to_svg_d};
use crate::svg_import::ImportedVector;

fn unit_to_mm(u: Units) -> f64 {
    match u {
        Units::Inches => 25.4,
        Units::Feet => 304.8,
        Units::Centimeters => 10.0,
        Units::Meters => 1000.0,
        Units::Decimeters => 100.0,
        Units::Mils => 0.0254,
        Units::Microns => 0.001,
        _ => 1.0, // millimetres or unitless
    }
}

struct Ctx<'a> {
    drawing: &'a Drawing,
    out: BezPath,
    count: usize,
}

pub fn import_dxf(data: &[u8]) -> crate::Result<ImportedVector> {
    let drawing = Drawing::load(&mut std::io::Cursor::new(data)).map_err(|e| crate::Error::Other(format!("DXF konnte nicht gelesen werden: {e}")))?;
    let scale = unit_to_mm(drawing.header.default_drawing_units);
    // DXF's y axis points up; the mat's points down.
    let to_mm = Affine::new([scale, 0.0, 0.0, -scale, 0.0, 0.0]);
    let mut ctx = Ctx { drawing: &drawing, out: BezPath::new(), count: 0 };
    let entities: Vec<&dxf::entities::Entity> = drawing.entities().collect();
    emit(&mut ctx, &entities, to_mm, 0);
    if ctx.count == 0 {
        return Err(crate::Error::Other("Die DXF-Datei enthält keine unterstützten Formen.".into()));
    }
    Ok(ImportedVector { d: to_svg_d(&ctx.out), bbox: bbox(&ctx.out), path_count: ctx.count })
}

fn pt(p: &dxf::Point) -> Point {
    Point::new(p.x, p.y)
}

fn emit(ctx: &mut Ctx, entities: &[&dxf::entities::Entity], t: Affine, depth: usize) {
    for e in entities {
        let mut p = BezPath::new();
        match &e.specific {
            EntityType::Line(l) => {
                p.move_to(pt(&l.p1));
                p.line_to(pt(&l.p2));
            }
            EntityType::LwPolyline(l) => {
                let v: Vec<(Point, f64)> = l.vertices.iter().map(|v| (Point::new(v.x, v.y), v.bulge)).collect();
                polyline(&mut p, &v, l.flags & 1 == 1);
            }
            EntityType::Polyline(l) => {
                let v: Vec<(Point, f64)> = l.vertices().map(|v| (pt(&v.location), v.bulge)).collect();
                polyline(&mut p, &v, l.flags & 1 == 1);
            }
            EntityType::Circle(c) => {
                p = kurbo::Circle::new(pt(&c.center), c.radius).to_path(0.01);
            }
            EntityType::Arc(a) => {
                let start = a.start_angle.to_radians();
                let mut sweep = a.end_angle.to_radians() - start;
                if sweep <= 0.0 {
                    sweep += TAU;
                }
                let arc = KArc::new(pt(&a.center), Vec2::new(a.radius, a.radius), start, sweep, 0.0);
                p.move_to(arc.center + Vec2::from_angle(start) * a.radius);
                arc.to_cubic_beziers(0.01, |p1, p2, p3| p.curve_to(p1, p2, p3));
            }
            EntityType::Ellipse(el) => {
                let major = Vec2::new(el.major_axis.x, el.major_axis.y);
                let rx = major.hypot();
                let ry = rx * el.minor_axis_ratio;
                let rot = major.atan2();
                let mut sweep = el.end_parameter - el.start_parameter;
                if sweep <= 0.0 {
                    sweep += TAU;
                }
                let arc = KArc::new(pt(&el.center), Vec2::new(rx, ry), el.start_parameter, sweep, rot);
                let (sa, ca) = el.start_parameter.sin_cos();
                let (sr, cr) = rot.sin_cos();
                let (lx, ly) = (rx * ca, ry * sa);
                let start = arc.center + Vec2::new(lx * cr - ly * sr, lx * sr + ly * cr);
                p.move_to(start);
                arc.to_cubic_beziers(0.01, |p1, p2, p3| p.curve_to(p1, p2, p3));
                if (sweep - TAU).abs() < 1e-9 {
                    p.close_path();
                }
            }
            EntityType::Spline(s) => spline(&mut p, s),
            EntityType::Insert(ins) => {
                if depth > 8 {
                    continue;
                }
                let Some(block) = ctx.drawing.blocks().find(|b| b.name == ins.name) else { continue };
                let local = Affine::translate((ins.location.x, ins.location.y))
                    * Affine::rotate(ins.rotation.to_radians())
                    * Affine::scale_non_uniform(ins.x_scale_factor, ins.y_scale_factor)
                    * Affine::translate((-block.base_point.x, -block.base_point.y));
                let children: Vec<&dxf::entities::Entity> = block.entities.iter().collect();
                emit(ctx, &children, t * local, depth + 1);
                continue;
            }
            _ => continue,
        }
        if p.elements().is_empty() {
            continue;
        }
        ctx.out.extend((t * p).elements().iter().copied());
        ctx.count += 1;
    }
}

/// Polyline with optional arc segments (bulge = tan(sweep/4)).
fn polyline(p: &mut BezPath, v: &[(Point, f64)], closed: bool) {
    if v.len() < 2 {
        return;
    }
    p.move_to(v[0].0);
    let n = v.len();
    let segs = if closed { n } else { n - 1 };
    for i in 0..segs {
        let (a, bulge) = v[i];
        let b = v[(i + 1) % n].0;
        if bulge.abs() < 1e-9 {
            p.line_to(b);
            continue;
        }
        let sweep = 4.0 * bulge.atan();
        let chord = b - a;
        let len = chord.hypot();
        // Standard bulge geometry (y up, positive = counter-clockwise): the
        // centre sits on the left normal at (len/2)·(1−b²)/(2b).
        let normal = Vec2::new(-chord.y, chord.x) / len;
        let r = len / 2.0 * (1.0 + bulge * bulge) / (2.0 * bulge.abs());
        let c = a.midpoint(b) + normal * (len / 2.0 * (1.0 - bulge * bulge) / (2.0 * bulge));
        let start = (a - c).atan2();
        let arc = KArc::new(c, Vec2::new(r, r), start, sweep, 0.0);
        arc.to_cubic_beziers(0.01, |p1, p2, p3| p.curve_to(p1, p2, p3));
    }
    if closed {
        p.close_path();
    }
}

/// NURBS evaluation with de Boor's algorithm, sampled densely.
fn spline(p: &mut BezPath, s: &Spline) {
    let ctrl: Vec<Point> = s.control_points.iter().map(pt).collect();
    let k = s.degree_of_curve.max(1) as usize;
    let knots = &s.knot_values;
    if ctrl.len() <= k || knots.len() != ctrl.len() + k + 1 {
        // No usable knot vector: fall back to fit points / control polygon.
        let pts: Vec<Point> = if !s.fit_points.is_empty() { s.fit_points.iter().map(pt).collect() } else { ctrl };
        if pts.len() >= 2 {
            p.move_to(pts[0]);
            for q in &pts[1..] {
                p.line_to(*q);
            }
        }
        return;
    }
    let w: Vec<f64> = if s.weight_values.len() == ctrl.len() { s.weight_values.clone() } else { vec![1.0; ctrl.len()] };
    let (t0, t1) = (knots[k], knots[ctrl.len()]);
    let samples = (ctrl.len() * 16).max(32);
    for i in 0..=samples {
        let t = t0 + (t1 - t0) * i as f64 / samples as f64;
        let q = de_boor(t, k, knots, &ctrl, &w);
        if i == 0 { p.move_to(q) } else { p.line_to(q) }
    }
    if s.flags & 1 == 1 {
        p.close_path();
    }
}

fn de_boor(t: f64, k: usize, knots: &[f64], ctrl: &[Point], w: &[f64]) -> Point {
    let n = ctrl.len();
    // Knot span.
    let mut span = k;
    while span < n - 1 && t >= knots[span + 1] {
        span += 1;
    }
    // Homogeneous coordinates for rational splines.
    let mut d: Vec<(f64, f64, f64)> = (0..=k).map(|j| {
        let c = ctrl[j + span - k];
        let wj = w[j + span - k];
        (c.x * wj, c.y * wj, wj)
    }).collect();
    for r in 1..=k {
        for j in (r..=k).rev() {
            let i = j + span - k;
            let denom = knots[i + k + 1 - r] - knots[i];
            let a = if denom.abs() < 1e-12 { 0.0 } else { (t - knots[i]) / denom };
            d[j] = (
                (1.0 - a) * d[j - 1].0 + a * d[j].0,
                (1.0 - a) * d[j - 1].1 + a * d[j].1,
                (1.0 - a) * d[j - 1].2 + a * d[j].2,
            );
        }
    }
    let (x, y, wt) = d[k];
    Point::new(x / wt, y / wt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(entities: &str) -> Vec<u8> {
        format!("0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n{entities}0\nENDSEC\n0\nEOF\n").into_bytes()
    }

    #[test]
    fn line_and_circle_in_mm_with_flipped_y() {
        let data = doc("0\nLINE\n8\n0\n10\n0\n20\n0\n30\n0\n11\n50\n21\n0\n31\n0\n0\nCIRCLE\n8\n0\n10\n25\n20\n20\n30\n0\n40\n10\n");
        let v = import_dxf(&data).unwrap();
        assert_eq!(v.path_count, 2);
        // Circle centre y=20 (up) → -20 (down), radius 10.
        assert!((v.bbox[1] + 30.0).abs() < 0.05, "{:?}", v.bbox);
        assert!((v.bbox[2] - 50.0).abs() < 0.05, "{:?}", v.bbox);
    }

    #[test]
    fn negative_bulge_curves_the_other_way() {
        // Quarter arcs (b = tan(22.5°)) from (0,0) to (10,0): CCW dips below
        // the chord (y up), CW rises above it.
        let b = (22.5f64).to_radians().tan();
        let mut ccw = BezPath::new();
        polyline(&mut ccw, &[(Point::new(0.0, 0.0), b), (Point::new(10.0, 0.0), 0.0)], false);
        let mut cw = BezPath::new();
        polyline(&mut cw, &[(Point::new(0.0, 0.0), -b), (Point::new(10.0, 0.0), 0.0)], false);
        let (bc, bw) = (ccw.bounding_box(), cw.bounding_box());
        assert!(bc.y0 < -1.0 && bc.y1 < 0.01, "ccw {bc:?}");
        assert!(bw.y1 > 1.0 && bw.y0 > -0.01, "cw {bw:?}");
        // Sagitta of a 90° arc over a 10 mm chord ≈ 2.07 mm.
        assert!((bc.height() - 2.07).abs() < 0.05, "{bc:?}");
    }

    #[test]
    fn bulge_makes_half_circle() {
        let mut p = BezPath::new();
        polyline(&mut p, &[(Point::new(0.0, 0.0), 1.0), (Point::new(20.0, 0.0), 0.0)], false);
        let b = p.bounding_box();
        assert!((b.width() - 20.0).abs() < 0.05 && (b.height() - 10.0).abs() < 0.05, "{b:?}");
    }
}
