//! Timing check: fill an A4 print & cut frame with 30 mm round stickers.
use schnittwerk_core::cut::PathInput;
use schnittwerk_core::nest::{nest, NestItem, NestRequest};

fn main() {
    let circle = "M0 15A15 15 0 1 0 30 15A15 15 0 1 0 0 15Z";
    let req = NestRequest {
        items: vec![NestItem { paths: vec![PathInput { d: circle.into(), matrix: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0] }] }],
        area: [15.0, 10.0, 180.0, 277.0],
        keepouts: vec![[15.0, 10.0, 20.0, 20.0], [175.0, 10.0, 20.0, 20.0], [15.0, 267.0, 20.0, 20.0]],
        gap: 2.0,
        rotate: true,
        fill: true,
    };
    let t = std::time::Instant::now();
    let r = nest(&req).unwrap();
    println!("{} Sticker in {:?}", r.placements.len(), t.elapsed());
}
