//! Schnittwerk MCP server (Model Context Protocol over stdio, JSON-RPC 2.0).
//!
//! Lets AI assistants trace images, build print & cut sticker sheets (as
//! `.schnitt` projects for the app) and – only with explicit confirmation –
//! cut SVG files on a connected Silhouette cutter.
//!
//! Register with Claude Code:  `claude mcp add schnittwerk -- schnittwerk-mcp`

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use schnittwerk_core::cut::{self, PathInput, PathOptions};
use schnittwerk_core::gpgl::{self, CutSettings, MatKind, RegMarks, Tool};
use schnittwerk_core::nest::{nest, NestItem, NestRequest};
use schnittwerk_core::sticker::{sticker_outline, MotifMask, StickerParams};
use schnittwerk_core::{TraceMode, TraceParams};
use schnittwerk_plotter::{Outcome, Plotter};
use serde_json::{json, Value};

const PROTOCOL: &str = "2025-06-18";

struct Material {
    id: &'static str,
    name: &'static str,
    depth: u8,
    pressure: u8,
    speed: u8,
    passes: u8,
}

/// Same starting values as the app's material library.
const MATERIALS: &[Material] = &[
    Material { id: "vinyl", name: "Vinyl / Klebefolie", depth: 1, pressure: 10, speed: 5, passes: 1 },
    Material { id: "sticker-matt", name: "Sticker-Papier matt (Kiss-Cut)", depth: 2, pressure: 14, speed: 5, passes: 1 },
    Material { id: "sticker-glossy", name: "Sticker-Papier glänzend (Kiss-Cut)", depth: 2, pressure: 16, speed: 5, passes: 1 },
    Material { id: "sticker-through", name: "Sticker-Papier durchschneiden", depth: 6, pressure: 30, speed: 3, passes: 2 },
    Material { id: "paper", name: "Kopierpapier 80 g", depth: 2, pressure: 14, speed: 8, passes: 1 },
    Material { id: "cardstock", name: "Karton 200–250 g", depth: 5, pressure: 27, speed: 3, passes: 2 },
    Material { id: "htv", name: "Flexfolie (gespiegelt schneiden!)", depth: 2, pressure: 9, speed: 8, passes: 1 },
];

fn tools() -> Value {
    json!([
        {
            "name": "plotter_status",
            "description": "Prüft, ob ein Silhouette-Plotter per USB verbunden ist, und meldet Modell, Firmware und Zustand (bereit / Matte laden / beschäftigt). Schlägt fehl, wenn die Schnittwerk-App den Plotter gerade benutzt.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "list_materials",
            "description": "Listet die Materialvorlagen (Messertiefe 1–10, Druck 1–33, Geschwindigkeit 1–30, Durchgänge). Startwerte – vor teurem Material einen Testschnitt machen.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "trace_image",
            "description": "Traced ein Bild (PNG/JPG/…) zu Schnittlinien und speichert sie als SVG in Millimetern. Modi: threshold (dunkle Bereiche, Logos/Schrift), color_layers (eine Ebene je Farbe), alpha (Umriss eines freigestellten PNG), centerline (eine Linie mitten durch jeden Strich, zum Zeichnen).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "image_path": { "type": "string", "description": "Pfad zum Bild" },
                    "output_path": { "type": "string", "description": "Ziel-SVG (Standard: neben dem Bild)" },
                    "mode": { "type": "string", "enum": ["threshold", "color_layers", "alpha", "centerline"], "default": "threshold" },
                    "width_mm": { "type": "number", "description": "Breite des Ergebnisses in mm (Standard: 150 dpi)" },
                    "smoothness": { "type": "number", "minimum": 0, "maximum": 100, "default": 40 },
                    "threshold": { "type": "integer", "minimum": 1, "maximum": 254, "description": "Schwellwert (Standard: automatisch)" },
                    "invert": { "type": "boolean", "default": false },
                    "colors": { "type": "integer", "minimum": 2, "maximum": 12, "default": 4, "description": "Nur color_layers" }
                },
                "required": ["image_path"]
            }
        },
        {
            "name": "sticker_sheet",
            "description": "Erstellt aus einem oder mehreren Motiven einen Print-&-Cut-Sticker-Bogen: Schnittkontur mit Rand, Silhouette-Passermarken, Sticker platzsparend verschachtelt (mehrere Motive abwechselnd, gleich oft). Speichert eine .schnitt-Projektdatei, die man in der Schnittwerk-App öffnet, druckt und schneidet.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "image_path": { "type": "string", "description": "Ein Motiv (oder image_paths verwenden)" },
                    "image_paths": {
                        "type": "array",
                        "description": "Mehrere Motive auf einem Bogen: Pfade oder {path, width_mm}",
                        "items": { "anyOf": [
                            { "type": "string" },
                            { "type": "object", "properties": { "path": { "type": "string" }, "width_mm": { "type": "number" } }, "required": ["path"] }
                        ] }
                    },
                    "output_path": { "type": "string", "description": "Ziel (.schnitt), Standard: neben dem (ersten) Bild" },
                    "sticker_width_mm": { "type": "number", "default": 50 },
                    "margin_mm": { "type": "number", "default": 3, "description": "Rand um das Motiv" },
                    "gap_mm": { "type": "number", "default": 3, "description": "Abstand zwischen Stickern" },
                    "background": { "type": "string", "enum": ["auto", "transparent", "remove"], "default": "auto", "description": "Freistellen über Transparenz oder durch Entfernen der Randfarbe" },
                    "page": { "type": "string", "enum": ["A4", "Letter"], "default": "A4" },
                    "count": { "type": "integer", "description": "Höchstens so viele Sticker (Standard: Bogen füllen)" },
                    "die_cut": { "type": "boolean", "default": true, "description": "Jeden Sticker anschneiden (rot, abziehbar) und mit Trägerrand durchschneiden (blau). false = nur anschneiden" },
                    "backing_margin_mm": { "type": "number", "default": 2, "description": "Trägerrand zwischen Sticker-Kontur und Durchschnitt" }
                },
                "required": []
            }
        },
        {
            "name": "cut_svg",
            "description": "Schneidet die Pfade einer SVG-Datei (Koordinaten in mm, oben links = Ursprung der Matte) auf dem per USB verbundenen Plotter. Ohne confirm=true wird NUR eine Vorschau berechnet (Pfade, Schnittlänge, Größe). Mit confirm=true bewegt sich die Maschine – nur nach ausdrücklicher Zustimmung des Nutzers, Matte muss geladen sein.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "svg_path": { "type": "string" },
                    "material": { "type": "string", "description": "ID aus list_materials", "default": "vinyl" },
                    "pressure": { "type": "integer", "minimum": 1, "maximum": 33 },
                    "speed": { "type": "integer", "minimum": 1, "maximum": 30 },
                    "depth": { "type": "integer", "minimum": 1, "maximum": 10 },
                    "passes": { "type": "integer", "minimum": 1, "maximum": 5 },
                    "mirror": { "type": "boolean", "default": false, "description": "Für Flexfolie" },
                    "confirm": { "type": "boolean", "default": false, "description": "true = wirklich schneiden" }
                },
                "required": ["svg_path"]
            }
        }
    ])
}

// ---------------------------------------------------------------- helpers

fn arg_str<'a>(a: &'a Value, k: &str) -> Option<&'a str> {
    a.get(k).and_then(Value::as_str)
}
fn arg_f64(a: &Value, k: &str) -> Option<f64> {
    a.get(k).and_then(Value::as_f64)
}
fn arg_bool(a: &Value, k: &str) -> bool {
    a.get(k).and_then(Value::as_bool).unwrap_or(false)
}

fn with_ext(p: &Path, ext: &str) -> PathBuf {
    p.with_extension(ext)
}

/// Target file chosen by the client. Only files with the expected extension
/// are written – a tool call (possibly steered by text the AI read
/// somewhere) must not be able to overwrite e.g. ~/.bashrc.
fn output_path(a: &Value, ext: &str, default: PathBuf) -> Result<PathBuf, String> {
    let Some(p) = arg_str(a, "output_path") else { return Ok(default) };
    let p = PathBuf::from(p);
    if p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case(ext)) {
        Ok(p)
    } else {
        Err(format!("output_path muss auf .{ext} enden"))
    }
}

fn mm(v: f64) -> String {
    format!("{:.1}", v).replace('.', ",")
}

fn load_image(path: &str) -> Result<(image::DynamicImage, Vec<u8>), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let img = image::load_from_memory(&bytes).map_err(|e| format!("Bild nicht lesbar: {e}"))?;
    Ok((img, bytes))
}

// ---------------------------------------------------------------- tools

fn plotter_status() -> Result<String, String> {
    match Plotter::open() {
        Ok(Some(p)) => {
            let fw = p.firmware().unwrap_or_default();
            let st = p.status().map_err(|e| e.to_string())?;
            Ok(format!("{} verbunden ({}), Firmware „{}“, Zustand: {:?}", p.model.name, p.connection(), fw.trim(), st))
        }
        Ok(None) => Ok("Kein unterstützter Plotter per USB verbunden.".into()),
        Err(e) => Err(format!("{e} (läuft die Schnittwerk-App und benutzt den Plotter?)")),
    }
}

fn list_materials() -> String {
    let mut s = String::from("Materialvorlagen (Startwerte, vorher Testschnitt):\n");
    for m in MATERIALS {
        s += &format!("- {}: {} – Tiefe {}, Druck {}, Geschwindigkeit {}, {}× \n", m.id, m.name, m.depth, m.pressure, m.speed, m.passes);
    }
    s
}

fn trace_image(a: &Value) -> Result<String, String> {
    let path = arg_str(a, "image_path").ok_or("image_path fehlt")?;
    let (img, _) = load_image(path)?;
    let level = a.get("threshold").and_then(Value::as_u64).map(|v| v.clamp(1, 254) as u8);
    let invert = arg_bool(a, "invert");
    let mode = match arg_str(a, "mode").unwrap_or("threshold") {
        "color_layers" => TraceMode::MultiColor { colors: a.get("colors").and_then(Value::as_u64).unwrap_or(4).clamp(2, 12) as u8, skip_background: true },
        "alpha" => TraceMode::Alpha,
        "centerline" => TraceMode::Centerline { level, invert },
        _ => TraceMode::Threshold { level, invert },
    };
    let params = TraceParams { mode, smoothness: arg_f64(a, "smoothness").unwrap_or(40.0) as f32, ..Default::default() };
    let r = schnittwerk_core::trace(&img, &params);
    if r.layers.is_empty() {
        return Err("Nichts gefunden – anderen Modus oder Schwellwert probieren.".into());
    }
    let width_mm = arg_f64(a, "width_mm").unwrap_or(r.width as f64 * 25.4 / 150.0);
    let s = width_mm / r.width as f64;
    let (w, h) = (r.width as f64 * s, r.height as f64 * s);
    let out = output_path(a, "svg", with_ext(Path::new(path), "svg"))?;
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.3}mm\" height=\"{h:.3}mm\" viewBox=\"0 0 {w:.3} {h:.3}\">\n"
    );
    let fill = matches!(params.mode, TraceMode::MultiColor { .. });
    for l in &r.layers {
        svg += &format!(
            "  <path d=\"{}\" transform=\"scale({s})\" fill=\"{}\" fill-rule=\"evenodd\" stroke=\"{}\" stroke-width=\"{:.4}\"/>\n",
            l.d,
            if fill { l.color.as_str() } else { "none" },
            if fill { "none" } else { "#e5484d" },
            0.25 / s
        );
    }
    svg += "</svg>\n";
    std::fs::write(&out, svg).map_err(|e| format!("{}: {e}", out.display()))?;
    let contours: usize = r.layers.iter().map(|l| l.contours).sum();
    Ok(format!(
        "Gespeichert: {}\n{} Ebene(n), {} Konturen, Größe {} × {} mm{}",
        out.display(),
        r.layers.len(),
        contours,
        mm(w),
        mm(h),
        r.threshold.map(|t| format!(", Schwellwert {t}")).unwrap_or_default()
    ))
}

fn uid(n: usize) -> String {
    format!("mcp{n:05}")
}

/// One prepared motif: image data plus its kiss-cut and optional through-cut outline.
struct Motif {
    name: String,
    src: String,
    width_mm: f64,
    s: f64,
    pw: f64,
    ph: f64,
    layer: schnittwerk_core::TraceLayer,
    through: Option<schnittwerk_core::TraceLayer>,
}

fn prepare_motif(path: &str, width_mm: f64, a: &Value) -> Result<Motif, String> {
    let (img, bytes) = load_image(path)?;
    let (pw, ph) = (img.width() as f64, img.height() as f64);
    let margin = arg_f64(a, "margin_mm").unwrap_or(3.0).max(0.0);
    let s = width_mm / pw;
    let has_alpha = img.color().has_alpha() && img.to_rgba8().pixels().any(|p| p[3] < 128);
    let mask = match arg_str(a, "background").unwrap_or("auto") {
        "transparent" => MotifMask::Alpha,
        "remove" => MotifMask::Background { tolerance: 12.0 },
        _ if has_alpha => MotifMask::Alpha,
        _ => MotifMask::Background { tolerance: 12.0 },
    };
    let params = |offset_mm: f64| StickerParams { mask, offset_px: offset_mm / s, smoothness: 60.0, fill_holes: true, despeckle: 30.0 };
    let name = Path::new(path).file_stem().and_then(|n| n.to_str()).unwrap_or("Sticker").to_string();
    let layer = sticker_outline(&img, &params(margin))
        .layers
        .into_iter()
        .next()
        .ok_or_else(|| format!("{name}: kein Motiv gefunden – Hintergrund-Modus ändern?"))?;
    let through = if a.get("die_cut").and_then(Value::as_bool).unwrap_or(true) {
        let backing = arg_f64(a, "backing_margin_mm").unwrap_or(2.0).max(0.5);
        sticker_outline(&img, &params(margin + backing)).layers.into_iter().next()
    } else {
        None
    };
    let mime = image::guess_format(&bytes).ok().map(|f| f.to_mime_type()).unwrap_or("image/png");
    let src = format!("data:{mime};base64,{}", B64.encode(&bytes));
    Ok(Motif { name, src, width_mm, s, pw, ph, layer, through })
}

/// Motif list from `image_paths` (strings or {path, width_mm}) or `image_path`.
fn motif_specs(a: &Value) -> Result<Vec<(String, f64)>, String> {
    let default_w = arg_f64(a, "sticker_width_mm").unwrap_or(50.0);
    let mut specs = Vec::new();
    if let Some(list) = a.get("image_paths").and_then(Value::as_array) {
        for v in list {
            match v {
                Value::String(p) => specs.push((p.clone(), default_w)),
                Value::Object(_) => {
                    let p = arg_str(v, "path").ok_or("image_paths: Eintrag ohne path")?;
                    specs.push((p.to_string(), arg_f64(v, "width_mm").unwrap_or(default_w)));
                }
                _ => return Err("image_paths: Pfad oder {path, width_mm} erwartet".into()),
            }
        }
    } else if let Some(p) = arg_str(a, "image_path") {
        specs.push((p.to_string(), default_w));
    }
    if specs.is_empty() {
        return Err("image_path oder image_paths fehlt".into());
    }
    Ok(specs.into_iter().map(|(p, w)| (p, w.max(5.0))).collect())
}

fn sticker_sheet(a: &Value) -> Result<String, String> {
    let specs = motif_specs(a)?;
    let motifs = specs.iter().map(|(p, w)| prepare_motif(p, *w, a)).collect::<Result<Vec<_>, _>>()?;
    let gap = arg_f64(a, "gap_mm").unwrap_or(3.0).max(0.0);
    let margin = arg_f64(a, "margin_mm").unwrap_or(3.0).max(0.0);

    let (page_name, page_w, page_h) = if arg_str(a, "page") == Some("Letter") { ("Letter", 215.9, 279.4) } else { ("A4", 210.0, 297.0) };
    let (inset_x, inset_y) = (15.0, 10.0);
    let reg = RegMarks { origin_x: inset_x, origin_y: inset_y, width: page_w - 2.0 * inset_x, length: page_h - 2.0 * inset_y };
    let zone = 20.0;
    let keepouts = vec![
        [reg.origin_x, reg.origin_y, zone, zone],
        [reg.origin_x + reg.width - zone, reg.origin_y, zone, zone],
        [reg.origin_x, reg.origin_y + reg.length - zone, zone, zone],
    ];
    let area = [reg.origin_x + 1.0, reg.origin_y + 1.0, reg.width - 2.0, reg.length - 2.0];
    // Nest by the outermost cut line. Unit at the origin: image (0,0)-(w,h).
    let unit = |m: &Motif| NestItem {
        paths: vec![PathInput { d: m.through.as_ref().unwrap_or(&m.layer).d.clone(), matrix: [m.s, 0.0, 0.0, m.s, 0.0, 0.0] }],
    };
    let max = a.get("count").and_then(Value::as_u64).map(|c| c as usize).unwrap_or(usize::MAX);
    let try_nest = |list: &[usize]| -> Result<Option<Vec<schnittwerk_core::nest::Placement>>, String> {
        let r = nest(&NestRequest { items: list.iter().map(|&i| unit(&motifs[i])).collect(), area, keepouts: keepouts.clone(), gap, rotate: true, fill: false })?;
        Ok(r.unplaced.is_empty().then_some(r.placements))
    };

    // Balanced fill: the motifs take turns; stop at the first one that no longer
    // fits (or at `count`). Every motif appears equally often (±1).
    let mut list: Vec<usize> = Vec::new();
    let mut best = Vec::new();
    let mut misses = 0;
    let mut next = 0;
    while list.len() < max && misses < motifs.len() && list.len() < 500 {
        list.push(next);
        match try_nest(&list)? {
            Some(p) => {
                best = p;
                misses = 0;
            }
            None => {
                list.pop();
                misses += 1;
            }
        }
        next = (next + 1) % motifs.len();
    }
    if best.is_empty() {
        return Err("Sticker ist zu groß für den Bogen.".into());
    }

    // Objects at their unit pose, then rotated about `center` and shifted.
    // z-order bottom → top: through-cut, kiss-cut, image.
    let mut objects = Vec::new();
    let mut counts = vec![0usize; motifs.len()];
    let mut n = 0;
    for p in &best {
        let mi = list[p.item];
        let m = &motifs[mi];
        counts[mi] += 1;
        let s = m.s;
        let mut base: Vec<(&str, f64, f64, f64, f64)> = Vec::new();
        if let Some(t) = &m.through {
            let [bx, by, bw, bh] = t.bbox;
            base.push(("through", bx * s, by * s, bw * s, bh * s));
        }
        let [bx, by, bw, bh] = m.layer.bbox;
        base.push(("kiss", bx * s, by * s, bw * s, bh * s));
        base.push(("image", 0.0, 0.0, m.pw * s, m.ph * s));
        n += 1;
        let group = uid(n);
        for &(kind, x, y, w, h) in &base {
            let (cx, cy) = (x + w / 2.0, y + h / 2.0);
            let (sin, cos) = p.rot.to_radians().sin_cos();
            let (rx, ry) = (cx - p.center[0], cy - p.center[1]);
            let (nx, ny) = (p.center[0] + rx * cos - ry * sin + p.dx, p.center[1] + rx * sin + ry * cos + p.dy);
            n += 1;
            let common = json!({
                "id": uid(n), "x": nx - w / 2.0, "y": ny - h / 2.0, "w": w, "h": h, "rot": p.rot,
                "flipX": false, "flipY": false, "visible": true, "locked": false, "group": group,
            });
            let mut obj = common.as_object().unwrap().clone();
            if kind == "image" {
                obj.insert("type".into(), json!("image"));
                obj.insert("name".into(), json!(m.name));
                obj.insert("imageId".into(), json!(format!("motif{mi}")));
                obj.insert("opacity".into(), json!(1));
            } else {
                let (l, label, color) = if kind == "through" {
                    (m.through.as_ref().unwrap(), "Durchschneiden", "#3e63dd")
                } else {
                    (&m.layer, "Anschneiden", "#e5484d")
                };
                obj.insert("type".into(), json!("path"));
                obj.insert("name".into(), json!(format!("{} · {label}", m.name)));
                obj.insert("d".into(), json!(l.d));
                obj.insert("bbox".into(), json!(l.bbox));
                obj.insert("color".into(), json!(color));
                obj.insert("fill".into(), json!(false));
                obj.insert("printFill".into(), Value::Null);
            }
            objects.push(Value::Object(obj));
        }
    }
    let images: serde_json::Map<String, Value> = motifs
        .iter()
        .enumerate()
        .filter(|(i, _)| counts[*i] > 0)
        .map(|(i, m)| (format!("motif{i}"), json!({ "src": m.src, "name": m.name })))
        .collect();
    let project = json!({
        "app": "schnittwerk",
        "version": 1,
        "mat": { "name": format!("{page_name} · Print & Cut"), "w": page_w, "h": page_h },
        "printCut": { "enabled": true, "page": page_name, "insetX": inset_x, "insetY": inset_y },
        "objects": objects,
        "images": images,
    });
    let first = Path::new(&specs[0].0);
    let default_out = if motifs.len() > 1 { first.with_file_name("Sticker-Bogen.schnitt") } else { with_ext(first, "schnitt") };
    let out = output_path(a, "schnitt", default_out)?;
    std::fs::write(&out, serde_json::to_string(&project).unwrap()).map_err(|e| format!("{}: {e}", out.display()))?;
    let detail = motifs
        .iter()
        .zip(&counts)
        .map(|(m, c)| format!("{c}× {} ({} mm breit)", m.name, mm(m.width_mm)))
        .collect::<Vec<_>>()
        .join(", ");
    let any_through = motifs.iter().any(|m| m.through.is_some());
    Ok(format!(
        "Sticker-Bogen gespeichert: {}\n{} Sticker auf {page_name}: {detail}; {} mm Rand. {}In Schnittwerk öffnen (Strg+O), drucken (Strg+P), dann schneiden.",
        out.display(),
        best.len(),
        mm(margin),
        if any_through { "Rot wird angeschnitten (Sticker-Papier), blau mit Trägerrand durchgeschnitten. " } else { "Nur angeschnitten (Material z. B. „sticker-matt“). " }
    ))
}

fn cut_svg(a: &Value) -> Result<String, String> {
    let path = arg_str(a, "svg_path").ok_or("svg_path fehlt")?;
    let data = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let v = schnittwerk_core::import_svg(&data).map_err(|e| e.to_string())?;
    let mat_id = arg_str(a, "material").unwrap_or("vinyl");
    let m = MATERIALS.iter().find(|m| m.id == mat_id).ok_or_else(|| format!("Unbekanntes Material „{mat_id}“ – siehe list_materials"))?;
    let get = |k: &str, d: u8, max: u64| a.get(k).and_then(Value::as_u64).map(|v| v.clamp(1, max) as u8).unwrap_or(d);
    let (pressure, speed, depth, passes) = (get("pressure", m.pressure, 33), get("speed", m.speed, 30), get("depth", m.depth, 10), get("passes", m.passes, 5));
    let matrix = if arg_bool(a, "mirror") { [-1.0, 0.0, 0.0, 1.0, 203.2, 0.0] } else { [1.0, 0.0, 0.0, 1.0, 0.0, 0.0] };
    let opts = PathOptions { overcut: 0.3, passes, optimize: true, clip: None };
    let (plan, strokes) = cut::plan(&[PathInput { d: v.d, matrix }], &opts)?;
    let summary = format!(
        "{} Pfade, Schnittlänge {} cm, Bereich x {}–{} mm, y {}–{} mm. Material {} (Tiefe {depth}, Druck {pressure}, Geschwindigkeit {speed}, {passes}×).",
        plan.strokes.len(),
        mm(plan.cut_length / 10.0),
        mm(plan.bbox[0]),
        mm(plan.bbox[0] + plan.bbox[2]),
        mm(plan.bbox[1]),
        mm(plan.bbox[1] + plan.bbox[3]),
        m.name
    );
    if !arg_bool(a, "confirm") {
        return Ok(format!("Vorschau (nicht geschnitten): {summary}\nZum Schneiden erneut mit confirm=true aufrufen – nur nach Zustimmung des Nutzers."));
    }
    let plotter = Plotter::open().map_err(|e| e.to_string())?.ok_or("Kein Plotter per USB verbunden.")?;
    let settings = CutSettings { tool: Tool::Blade, pressure, speed, depth: Some(depth), mat: MatKind::Portrait8x12, ..Default::default() };
    let (job, clamped) = gpgl::build_job(&[(strokes, settings)], plotter.model.limits(), None);
    let never = AtomicBool::new(false);
    match plotter.run_job(&job, &never, &never, |_, _| {}, |_| {}).map_err(|e| e.to_string())? {
        Outcome::Done => Ok(format!(
            "Geschnitten. {summary}{}",
            if clamped > 0 { format!(" Achtung: {clamped} Punkte lagen außerhalb der Matte.") } else { String::new() }
        )),
        Outcome::Cancelled => Ok("Abgebrochen.".into()),
    }
}

// ---------------------------------------------------------------- JSON-RPC

fn call_tool(name: &str, args: &Value) -> Result<String, String> {
    match name {
        "plotter_status" => plotter_status(),
        "list_materials" => Ok(list_materials()),
        "trace_image" => trace_image(args),
        "sticker_sheet" => sticker_sheet(args),
        "cut_svg" => cut_svg(args),
        other => Err(format!("Unbekanntes Werkzeug: {other}")),
    }
}

fn handle(req: &Value) -> Option<Value> {
    let id = req.get("id").cloned();
    let method = req.get("method").and_then(Value::as_str).unwrap_or("");
    let result = match method {
        "initialize" => {
            let version = req.pointer("/params/protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOL);
            json!({
                "protocolVersion": version,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "schnittwerk", "version": env!("CARGO_PKG_VERSION") },
                "instructions": "Schnittwerk steuert Silhouette-Schneideplotter. Pfade sind in Millimetern, Ursprung oben links auf der Matte. cut_svg schneidet nur mit confirm=true – vorher immer die Vorschau zeigen und den Nutzer fragen."
            })
        }
        "ping" => json!({}),
        "tools/list" => json!({ "tools": tools() }),
        "tools/call" => {
            let name = req.pointer("/params/name").and_then(Value::as_str).unwrap_or("");
            let empty = json!({});
            let args = req.pointer("/params/arguments").unwrap_or(&empty);
            match call_tool(name, args) {
                Ok(text) => json!({ "content": [{ "type": "text", "text": text }] }),
                Err(text) => json!({ "content": [{ "type": "text", "text": text }], "isError": true }),
            }
        }
        _ if id.is_none() => return None, // notifications (e.g. notifications/initialized)
        _ => return Some(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": format!("Methode nicht gefunden: {method}") } })),
    };
    id.map(|id| json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

fn main() {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(req) => handle(&req),
            Err(e) => Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": e.to_string() } })),
        };
        if let Some(r) = reply {
            let _ = writeln!(stdout, "{r}");
            let _ = stdout.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_and_list_tools() {
        let r = handle(&json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18" } })).unwrap();
        assert_eq!(r["result"]["serverInfo"]["name"], "schnittwerk");
        let t = handle(&json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).unwrap();
        let names: Vec<&str> = t["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"sticker_sheet") && names.contains(&"cut_svg"));
        assert!(handle(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })).is_none());
    }

    #[test]
    fn cut_without_confirm_is_only_a_preview() {
        let dir = std::env::temp_dir().join("schnittwerk-mcp-test");
        std::fs::create_dir_all(&dir).unwrap();
        let svg = dir.join("sq.svg");
        std::fs::write(&svg, r#"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="100mm" viewBox="0 0 100 100"><rect x="10" y="10" width="20" height="20"/></svg>"#).unwrap();
        let r = cut_svg(&json!({ "svg_path": svg.to_str().unwrap() })).unwrap();
        assert!(output_path(&json!({ "output_path": "/tmp/x/.bashrc" }), "svg", PathBuf::new()).is_err());
        assert!(output_path(&json!({ "output_path": "/tmp/x/a.SVG" }), "svg", PathBuf::new()).is_ok());
        assert!(r.starts_with("Vorschau"), "{r}");
        assert!(r.contains("1 Pfade"), "{r}");
    }

    #[test]
    fn sticker_sheet_writes_project() {
        let dir = std::env::temp_dir().join("schnittwerk-mcp-test");
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("dot.png");
        let mut img = image::RgbaImage::from_pixel(100, 100, image::Rgba([0, 0, 0, 0]));
        for y in 0..100 {
            for x in 0..100 {
                if (x as f64 - 50.0).hypot(y as f64 - 50.0) < 40.0 {
                    img.put_pixel(x, y, image::Rgba([200, 40, 40, 255]));
                }
            }
        }
        img.save(&png).unwrap();
        let r = sticker_sheet(&json!({ "image_path": png.to_str().unwrap(), "sticker_width_mm": 40, "count": 6 })).unwrap();
        assert!(r.contains("6 Sticker"), "{r}");
        let proj: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("dot.schnitt")).unwrap()).unwrap();
        // Through-cut + kiss-cut + image per sticker.
        assert_eq!(proj["objects"].as_array().unwrap().len(), 18);
        assert_eq!(proj["printCut"]["enabled"], true);
    }

    #[test]
    fn sticker_sheet_mixes_motifs_evenly() {
        let dir = std::env::temp_dir().join("schnittwerk-mcp-test-multi");
        std::fs::create_dir_all(&dir).unwrap();
        let mut paths = Vec::new();
        for (name, r) in [("a", 40.0), ("b", 30.0), ("c", 20.0)] {
            let png = dir.join(format!("{name}.png"));
            let mut img = image::RgbaImage::from_pixel(100, 100, image::Rgba([0, 0, 0, 0]));
            for y in 0..100 {
                for x in 0..100 {
                    if (x as f64 - 50.0).hypot(y as f64 - 50.0) < r {
                        img.put_pixel(x, y, image::Rgba([40, 40, 200, 255]));
                    }
                }
            }
            img.save(&png).unwrap();
            paths.push(png.to_str().unwrap().to_string());
        }
        let r = sticker_sheet(&json!({ "image_paths": paths, "sticker_width_mm": 40, "count": 7 })).unwrap();
        assert!(r.contains("7 Sticker"), "{r}");
        assert!(r.contains("3× a") && r.contains("2× b") && r.contains("2× c"), "{r}");
        let proj: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("Sticker-Bogen.schnitt")).unwrap()).unwrap();
        assert_eq!(proj["images"].as_object().unwrap().len(), 3);
        assert_eq!(proj["objects"].as_array().unwrap().len(), 21);
    }
}
