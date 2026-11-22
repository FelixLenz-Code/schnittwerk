//! Hardware check: `cargo run -p schnittwerk-plotter --example status`
fn main() {
    let found = schnittwerk_plotter::scan();
    println!("Gefundene Geräte: {found:?}");
    match schnittwerk_plotter::Plotter::open() {
        Ok(Some(p)) => {
            println!("Verbunden: {}", p.model.name);
            println!("Firmware: {:?}", p.firmware());
            println!("Status: {:?}", p.status());
        }
        Ok(None) => println!("Kein unterstützter Plotter angeschlossen."),
        Err(e) => println!("Fehler: {e}"),
    }
}
