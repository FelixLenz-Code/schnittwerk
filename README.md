# Schnittwerk

Linux-Software für Silhouette-Schneideplotter (Ziel: **Portrait 3**) – mit Bild-Tracing,
moderner Oberfläche und Print & Cut für Sticker.

Rust (Kern + Backend) · Tauri 2 · Svelte 5 + TypeScript (Oberfläche)

## Einrichtung (Ubuntu/Debian)

```bash
# Systempakete für Tauri + USB
sudo apt install pkg-config libwebkit2gtk-4.1-dev libgtk-3-dev libdbus-1-dev \
  librsvg2-dev libayatana-appindicator3-dev libxdo-dev libusb-1.0-0-dev poppler-utils

# Rust (falls nicht vorhanden)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

npm install

# Zugriff auf den Plotter ohne root (einmalig)
sudo cp packaging/70-silhouette.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
```

Danach den Plotter einmal aus- und wieder einstecken.
Hardware-Test ohne Oberfläche: `cargo run -p schnittwerk-plotter --example status`

## Starten

```bash
npm run tauri dev      # Entwicklungsmodus mit Hot-Reload
./scripts/build-deb.sh # .deb mit hochgezählter Version unter target/release/bundle/deb/
```

Tests des Kerns: `cargo test -p schnittwerk-core`

## Kommandozeile

`schnittwerk-cli` (im .deb enthalten) tract Bilder und schneidet ohne Oberfläche:

```bash
schnittwerk-cli trace zeichnung.jpg --flatten --width 120   # → zeichnung.svg
schnittwerk-cli cut logo.svg --material vinyl --dry-run     # nur Vorschau
schnittwerk-cli cut bogen.schnitt                            # Projekt, Material je Schnittfarbe, Rückfrage
schnittwerk-cli cut bogen.schnitt --material '#e5484d=sticker-glossy' --yes
schnittwerk-cli scan && schnittwerk-cli status --ble ID      # Bluetooth
```

Alle Optionen: `schnittwerk-cli --help`.

## MCP-Schnittstelle (KI-Assistenten)

`schnittwerk-mcp` ist ein MCP-Server (stdio). Damit können KI-Assistenten wie
Claude Bilder tracen, Sticker-Bögen als `.schnitt`-Projekt anlegen und SVGs
schneiden – geschnitten wird nur mit ausdrücklicher Bestätigung (`confirm`).

```bash
# nach Installation des .deb
claude mcp add schnittwerk -- /usr/bin/schnittwerk-mcp
# aus dem Quellcode
cargo build --release -p schnittwerk-mcp
claude mcp add schnittwerk -- "$PWD/target/release/schnittwerk-mcp"
```

Werkzeuge: `plotter_status`, `list_materials`, `trace_image`, `sticker_sheet`, `cut_svg`.

## Aufbau

```
crates/schnittwerk-core/   Rust-Bibliothek ohne GUI
  src/trace.rs             Tracing-Pipeline (Masken → Konturen → Bézier)
  src/preprocess.rs        Helligkeit/Kontrast, Otsu, Farbmasken, k-means
  src/svg_import.rs        SVG → ein zusammengesetzter Pfad in mm
  src/cut.rs               Pfade → Schnittreihenfolge, Überschnitt, Durchgänge
  src/gpgl.rs              GPGL-Befehle (Silhouette/Graphtec-Protokoll), Passermarken
  src/offset.rs            Distanztransformation, Konturversatz (Bilder + Vektoren)
  src/sticker.rs           Sticker-Kontur: Motiv freistellen, Rand, Löcher füllen
  src/photo.rs             Fotos: Perspektivkorrektur (Blatt erkennen, entzerren), Beleuchtungsausgleich
crates/schnittwerk-plotter/ USB (rusb) und Bluetooth LE (btleplug): Status, Senden, Pause/Abbruch
crates/schnittwerk-mcp/    MCP-Server für KI-Assistenten
crates/schnittwerk-cli/    Kommandozeile (tracen, schneiden)
src-tauri/                 Tauri-Backend (Befehle, Bildspeicher, Schneide-Thread, PDF/Druck)
src/                       Oberfläche
  lib/editor.svelte.ts     Dokument-Zustand, Auswahl, Undo/Redo
  lib/io.svelte.ts         Import, Projektdateien (.schnitt), SVG-Export
  lib/components/          Canvas, Trace-Panel, Eigenschaften, Ebenen …
```

## Tastenkürzel

| Taste | Funktion |
|---|---|
| `V` / `H` / `R` / `E` / `T` | Auswahl / Ansicht verschieben / Rechteck / Ellipse / Text |
| `Leertaste` + Ziehen, mittlere Maustaste | Ansicht verschieben |
| Mausrad | Zoom (Shift = horizontal scrollen) |
| `Strg+0` | Matte einpassen |
| `Strg+I` / `Strg+E` | Importieren / SVG exportieren |
| `Strg+Enter` / `Strg+P` | Schneiden-Dialog / Drucken |
| `Strg+G` / `Strg+Shift+G` | Gruppieren / Gruppierung lösen |
| `Strg+S` / `Strg+Shift+S` / `Strg+O` | Speichern / Speichern unter / Öffnen |
| `Strg+Z` / `Strg+Y` | Rückgängig / Wiederholen |
| `Strg+D` / `Entf` | Duplizieren / Löschen |
| `Strg+C` / `Strg+X` / `Strg+V` | Kopieren / Ausschneiden / Einfügen (auch Bilder und SVG aus anderen Programmen) |
| Pfeiltasten (+Shift / +Alt) | 1 mm (10 mm / 0,1 mm) verschieben |
| Ecke ziehen (+Shift) | proportional (frei) skalieren |
| Drehgriff (+Shift) | drehen (in 15°-Schritten) |

Siehe [ROADMAP.md](ROADMAP.md) für den Funktionsumfang und den Stand.
