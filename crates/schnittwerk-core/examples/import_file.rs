//! Debug helper: `cargo run --example import_file -- <file.svg|file.dxf>` prints bbox (mm).
fn main() {
    let path = std::env::args().nth(1).expect("Datei");
    let data = std::fs::read(&path).unwrap();
    let v = if path.to_lowercase().ends_with(".dxf") {
        schnittwerk_core::dxf_import::import_dxf(&data)
    } else {
        schnittwerk_core::import_svg(&data)
    };
    match v {
        Ok(v) => println!("{} Pfade, bbox {:?}", v.path_count, v.bbox),
        Err(e) => println!("Fehler: {e}"),
    }
}
