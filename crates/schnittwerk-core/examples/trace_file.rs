//! Debug helper: traces an image and prints the outline as flattened
//! polylines (JSON) plus the number of Bézier segments.
//! Usage: cargo run --example trace_file -- <image> [smoothness]
use kurbo::{BezPath, PathEl};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let img = image::open(&args[1]).expect("Bild lesen");
    let mut p = schnittwerk_core::TraceParams::default();
    if let Some(s) = args.get(2) {
        p.smoothness = s.parse().unwrap();
    }
    if args.get(3).map(|s| s == "center").unwrap_or(false) {
        p.mode = schnittwerk_core::TraceMode::Centerline { level: None, invert: false };
    }
    let r = schnittwerk_core::trace(&img, &p);
    let mut polys = Vec::new();
    let mut segs = 0;
    for l in &r.layers {
        let path = BezPath::from_svg(&l.d).unwrap();
        segs += path.elements().iter().filter(|e| matches!(e, PathEl::CurveTo(..) | PathEl::LineTo(..))).count();
        let mut cur: Vec<String> = Vec::new();
        kurbo::flatten(&path, 0.05, |el| match el {
            PathEl::MoveTo(q) => {
                if !cur.is_empty() { polys.push(format!("[{}]", cur.join(","))); }
                cur = vec![format!("[{:.3},{:.3}]", q.x, q.y)];
            }
            PathEl::LineTo(q) => cur.push(format!("[{:.3},{:.3}]", q.x, q.y)),
            _ => {}
        });
        if !cur.is_empty() { polys.push(format!("[{}]", cur.join(","))); }
    }
    println!("{{\"segments\":{},\"polys\":[{}]}}", segs, polys.join(","));
}
