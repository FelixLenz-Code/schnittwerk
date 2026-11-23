//! Passive Bluetooth LE scan for Silhouette cutters: `cargo run -p schnittwerk-plotter --example ble_scan`
fn main() {
    match schnittwerk_plotter::ble::scan(6) {
        Ok(list) if list.is_empty() => println!("Kein Plotter per Bluetooth gefunden."),
        Ok(list) => {
            for d in list {
                println!("{} ({}) rssi={:?} → Modell {:?}", d.name, d.address, d.rssi, schnittwerk_plotter::ble::model_for(&d.name).map(|m| m.name));
            }
        }
        Err(e) => println!("Fehler: {e}"),
    }
}
