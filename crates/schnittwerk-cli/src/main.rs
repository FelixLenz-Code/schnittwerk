//! Schnittwerk on the command line: trace images to SVG and cut SVG, DXF
//! or `.schnitt` project files on a Silhouette cutter (USB or Bluetooth).

use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use kurbo::Affine;
use schnittwerk_core::cut::{self, PathInput, PathOptions};
use schnittwerk_core::gpgl::{self, CutSettings, MatKind, RegMarks, Tool};
use schnittwerk_core::{TraceMode, TraceParams};
use schnittwerk_plotter::{ble, Outcome, Phase, Plotter, MODELS};
use serde::Deserialize;

const HELP: &str = "\
Schnittwerk – Kommandozeile

Aufruf:
  schnittwerk-cli status [--ble ID]
  schnittwerk-cli scan [SEKUNDEN]
  schnittwerk-cli materials
  schnittwerk-cli trace BILD [Optionen]
  schnittwerk-cli cut DATEI [Optionen]

trace – Bild in Schnittlinien umwandeln (SVG):
  -o, --output DATEI     Ziel (Standard: BILD.svg)
  --mode MODUS           sw (Standard), mehrfarbig, alpha, linie
  --threshold N          Schwellwert 1–254 (Standard: automatisch)
  --invert               helle statt dunkle Bereiche
  --colors N             Farben im Modus mehrfarbig (2–12)
  --smooth N             Glättung 0–100 (Standard 40)
  --width MM             Breite des Ergebnisses in mm
  --flatten              Beleuchtung ausgleichen (Fotos von Zeichnungen)

cut – SVG, DXF oder Schnittwerk-Projekt (.schnitt) schneiden:
  --material ID          Material für alle Linien (siehe materials)
  --material FARBE=ID    Material je Schnittfarbe im Projekt, z. B. '#e5484d=sticker-matt'
  --pressure N --speed N --depth N --passes N   Werte des Materials überschreiben
  --mat 8x12|8.5x12|none Schneidematte (Standard: 8x12, Print & Cut: none)
  --mirror               spiegeln (Flexfolie)
  --offset DX,DY         Feinjustierung Druck ↔ Schnitt in mm (Print & Cut)
  --ble ID               per Bluetooth statt USB (ID aus scan)
  --dry-run              nur Vorschau, nicht schneiden
  -y, --yes              ohne Rückfrage schneiden
";

// ---------------------------------------------------------------- materials

struct Material {
    id: &'static str,
    name: &'static str,
    tool: Tool,
    depth: u8,
    pressure: u8,
    speed: u8,
    passes: u8,
    overcut: f64,
}

/// Same starting values as the app's material library.
const MATERIALS: &[Material] = &[
    Material { id: "vinyl", name: "Vinyl / Klebefolie", tool: Tool::Blade, depth: 1, pressure: 10, speed: 5, passes: 1, overcut: 0.3 },
    Material { id: "sticker-matt", name: "Sticker-Papier matt", tool: Tool::Blade, depth: 2, pressure: 14, speed: 5, passes: 1, overcut: 0.3 },
    Material { id: "sticker-glossy", name: "Sticker-Papier glänzend", tool: Tool::Blade, depth: 2, pressure: 16, speed: 5, passes: 1, overcut: 0.3 },
    Material { id: "sticker-through", name: "Sticker-Papier durchschneiden", tool: Tool::Blade, depth: 6, pressure: 30, speed: 3, passes: 2, overcut: 0.5 },
    Material { id: "sticker-vinyl", name: "Sticker-Folie (bedruckbar)", tool: Tool::Blade, depth: 2, pressure: 12, speed: 5, passes: 1, overcut: 0.3 },
    Material { id: "paper", name: "Kopierpapier 80 g", tool: Tool::Blade, depth: 2, pressure: 14, speed: 8, passes: 1, overcut: 0.3 },
    Material { id: "photo", name: "Fotopapier", tool: Tool::Blade, depth: 3, pressure: 20, speed: 5, passes: 1, overcut: 0.3 },
    Material { id: "cardstock", name: "Karton 200–250 g", tool: Tool::Blade, depth: 5, pressure: 27, speed: 3, passes: 2, overcut: 0.5 },
    Material { id: "htv", name: "Flexfolie (Bügelfolie, --mirror!)", tool: Tool::Blade, depth: 2, pressure: 9, speed: 8, passes: 1, overcut: 0.3 },
    Material { id: "transparency", name: "Transparentfolie", tool: Tool::Blade, depth: 2, pressure: 12, speed: 5, passes: 1, overcut: 0.3 },
    Material { id: "magnet", name: "Magnetfolie", tool: Tool::Blade, depth: 9, pressure: 30, speed: 3, passes: 2, overcut: 0.5 },
    Material { id: "pen", name: "Stift (Zeichnen)", tool: Tool::Pen, depth: 0, pressure: 18, speed: 10, passes: 1, overcut: 0.0 },
];

fn material(id: &str) -> Result<&'static Material, String> {
    MATERIALS.iter().find(|m| m.id == id).ok_or_else(|| format!("Unbekanntes Material „{id}“ – siehe `schnittwerk-cli materials`"))
}

/// Default material of a cut colour (same rules as the app).
fn default_for(color: &str) -> &'static str {
    match color {
        "#3e63dd" => "sticker-through",
        "#30a46c" => "pen",
        "#e5484d" => "sticker-matt",
        _ => "vinyl",
    }
}

// ---------------------------------------------------------------- args

struct Args {
    pos: Vec<String>,
    opts: Vec<(String, Option<String>)>,
}

const FLAGS: &[&str] = &["--invert", "--flatten", "--mirror", "--dry-run", "--yes", "-y", "--help", "-h"];

impl Args {
    fn parse(raw: impl Iterator<Item = String>) -> Args {
        let mut pos = Vec::new();
        let mut opts = Vec::new();
        let mut it = raw.peekable();
        while let Some(a) = it.next() {
            if let Some((k, v)) = a.split_once('=').filter(|(k, _)| k.starts_with("--")) {
                opts.push((k.to_string(), Some(v.to_string())));
            } else if a.starts_with('-') && a.len() > 1 && a.parse::<f64>().is_err() {
                let v = if FLAGS.contains(&a.as_str()) { None } else { it.next() };
                opts.push((a, v));
            } else {
                pos.push(a);
            }
        }
        Args { pos, opts }
    }
    fn get(&self, names: &[&str]) -> Option<&str> {
        self.opts.iter().rev().find(|(k, _)| names.contains(&k.as_str())).and_then(|(_, v)| v.as_deref())
    }
    fn all(&self, name: &str) -> Vec<&str> {
        self.opts.iter().filter(|(k, _)| k == name).filter_map(|(_, v)| v.as_deref()).collect()
    }
    fn flag(&self, names: &[&str]) -> bool {
        self.opts.iter().any(|(k, _)| names.contains(&k.as_str()))
    }
    fn num<T: std::str::FromStr>(&self, name: &str) -> Result<Option<T>, String> {
        match self.get(&[name]) {
            None => Ok(None),
            Some(v) => v.replace(',', ".").parse().map(Some).map_err(|_| format!("{name}: „{v}“ ist keine Zahl")),
        }
    }
}

// ---------------------------------------------------------------- plotter

fn connect(args: &Args) -> Result<Plotter, String> {
    if let Some(id) = args.get(&["--ble"]) {
        eprintln!("Verbinde per Bluetooth …");
        let (t, name) = ble::BleTransport::connect(id).map_err(|e| e.to_string())?;
        let mut model = ble::model_for(&name);
        let p = Plotter::with_transport(Box::new(t), model.unwrap_or(MODELS[0]));
        if model.is_none() {
            model = p.firmware().as_deref().and_then(ble::model_for);
        }
        return Ok(match model {
            Some(m) if m.cameo4_line => Plotter::with_model(p, m),
            Some(m) => return Err(format!("{} wird noch nicht unterstützt.", m.name)),
            None => p,
        });
    }
    Plotter::open()
        .map_err(|e| format!("{e} (benutzt die Schnittwerk-App den Plotter gerade?)"))?
        .ok_or_else(|| "Kein Plotter per USB gefunden. Für Bluetooth: `schnittwerk-cli scan`, dann --ble ID.".into())
}

fn status(args: &Args) -> Result<(), String> {
    let p = connect(args)?;
    let fw = p.firmware().unwrap_or_default();
    let st = p.status().map_err(|e| e.to_string())?;
    println!("{} ({}), Firmware „{}“, Zustand: {:?}", p.model.name, p.connection(), fw.trim(), st);
    Ok(())
}

fn scan(args: &Args) -> Result<(), String> {
    let secs = args.pos.get(1).and_then(|s| s.parse().ok()).unwrap_or(5);
    eprintln!("Suche {secs} s nach Bluetooth-Plottern …");
    let found = ble::scan(secs).map_err(|e| e.to_string())?;
    if found.is_empty() {
        println!("Keine gefunden – Plotter eingeschaltet und Bluetooth aktiv?");
    }
    for d in found {
        println!("{}  {}  (Signal {})", d.id, d.name, d.rssi.map(|r| format!("{r} dBm")).unwrap_or("–".into()));
    }
    Ok(())
}

// ---------------------------------------------------------------- trace

fn trace(args: &Args) -> Result<(), String> {
    let path = args.pos.get(1).ok_or("Bild fehlt: schnittwerk-cli trace BILD")?;
    let img = image::open(path).map_err(|e| format!("{path}: {e}"))?;
    let level = args.num::<u8>("--threshold")?.map(|v| v.clamp(1, 254));
    let invert = args.flag(&["--invert"]);
    let mode = match args.get(&["--mode"]).unwrap_or("sw") {
        "sw" | "bw" | "threshold" => TraceMode::Threshold { level, invert },
        "mehrfarbig" | "color" | "colors" => TraceMode::MultiColor { colors: args.num::<u8>("--colors")?.unwrap_or(4).clamp(2, 12), skip_background: true },
        "alpha" | "freistellen" => TraceMode::Alpha,
        "linie" | "line" | "centerline" => TraceMode::Centerline { level, invert },
        m => return Err(format!("Unbekannter Modus „{m}“ (sw, mehrfarbig, alpha, linie)")),
    };
    let fill = matches!(mode, TraceMode::MultiColor { .. });
    let params = TraceParams {
        mode,
        smoothness: args.num::<f32>("--smooth")?.unwrap_or(40.0).clamp(0.0, 100.0),
        flatten: args.flag(&["--flatten"]),
        ..Default::default()
    };
    let r = schnittwerk_core::trace(&img, &params);
    if r.layers.is_empty() {
        return Err("Nichts gefunden – anderen Modus oder Schwellwert probieren.".into());
    }
    // Default size: 150 dpi.
    let width_mm = args.num::<f64>("--width")?.unwrap_or(r.width as f64 * 25.4 / 150.0);
    let s = width_mm / r.width as f64;
    let (w, h) = (r.width as f64 * s, r.height as f64 * s);
    let out = args.get(&["-o", "--output"]).map(PathBuf::from).unwrap_or_else(|| Path::new(path).with_extension("svg"));
    let mut svg = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.3}mm\" height=\"{h:.3}mm\" viewBox=\"0 0 {w:.3} {h:.3}\">\n");
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
    println!(
        "{} – {} Ebene(n), {} Konturen, {:.1} × {:.1} mm{}",
        out.display(),
        r.layers.len(),
        contours,
        w,
        h,
        r.threshold.map(|t| format!(", Schwellwert {t}")).unwrap_or_default()
    );
    Ok(())
}

// ---------------------------------------------------------------- cut

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Project {
    mat: ProjectMat,
    #[serde(default)]
    print_cut: Option<ProjectPrintCut>,
    objects: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct ProjectMat {
    name: String,
    w: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectPrintCut {
    enabled: bool,
    page: String,
    #[serde(default)]
    inset_x: Option<f64>,
    #[serde(default)]
    inset_y: Option<f64>,
    #[serde(default)]
    inset: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjPath {
    d: String,
    bbox: [f64; 4],
    color: String,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    rot: f64,
    flip_x: bool,
    flip_y: bool,
    visible: bool,
    #[serde(default)]
    cut: Option<bool>,
}

/// Same mapping as the app's `pathMatrix`: local path units → mat mm.
fn path_matrix(o: &ProjPath, mirror_width: Option<f64>, shift: (f64, f64)) -> [f64; 6] {
    let [bx, by, bw, bh] = o.bbox;
    let mut m = Affine::translate(shift);
    if let Some(w) = mirror_width {
        m = m * Affine::translate((w, 0.0)) * Affine::scale_non_uniform(-1.0, 1.0);
    }
    m = m
        * Affine::translate((o.x, o.y))
        * Affine::translate((o.w / 2.0, o.h / 2.0))
        * Affine::rotate(o.rot.to_radians())
        * Affine::scale_non_uniform(if o.flip_x { -1.0 } else { 1.0 }, if o.flip_y { -1.0 } else { 1.0 })
        * Affine::translate((-o.w / 2.0, -o.h / 2.0))
        * Affine::scale_non_uniform(o.w / if bw == 0.0 { 1.0 } else { bw }, o.h / if bh == 0.0 { 1.0 } else { bh })
        * Affine::translate((-bx, -by));
    m.as_coeffs()
}

/// A colour group to cut: paths in mat mm and its material.
struct Group {
    label: String,
    paths: Vec<PathInput>,
    mat: &'static Material,
}

/// `--material` values: one plain id (all), and/or `colour=id` pairs.
type MaterialChoice = (Option<&'static Material>, Vec<(String, &'static Material)>);

fn material_choice(args: &Args) -> Result<MaterialChoice, String> {
    let mut all = None;
    let mut by_color = Vec::new();
    for v in args.all("--material") {
        match v.split_once('=') {
            Some((c, id)) => by_color.push((c.trim().to_lowercase(), material(id.trim())?)),
            None => all = Some(material(v.trim())?),
        }
    }
    Ok((all, by_color))
}

fn load_groups(path: &str, args: &Args) -> Result<(Vec<Group>, Option<RegMarks>, MatKind), String> {
    let data = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let mirror = args.flag(&["--mirror"]);
    let (all, by_color) = material_choice(args)?;
    let ext = Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext == "schnitt" {
        let p: Project = serde_json::from_slice(&data).map_err(|e| format!("Projekt nicht lesbar: {e}"))?;
        let reg = p.print_cut.filter(|pc| pc.enabled).map(|pc| {
            let (pw, ph) = if pc.page == "Letter" { (215.9, 279.4) } else { (210.0, 297.0) };
            let ix = pc.inset_x.or(pc.inset).unwrap_or(15.0);
            let iy = pc.inset_y.or(pc.inset).unwrap_or(10.0);
            RegMarks { origin_x: ix, origin_y: iy, width: pw - 2.0 * ix, length: ph - 2.0 * iy }
        });
        let shift = match args.get(&["--offset"]) {
            Some(v) => {
                let (dx, dy) = v.split_once(',').ok_or("--offset DX,DY")?;
                (dx.trim().parse::<f64>().map_err(|_| "--offset DX,DY")?, dy.trim().parse::<f64>().map_err(|_| "--offset DX,DY")?)
            }
            None => (0.0, 0.0),
        };
        let mut groups: Vec<Group> = Vec::new();
        for v in p.objects {
            if v.get("type").and_then(|t| t.as_str()) != Some("path") {
                continue;
            }
            let o: ProjPath = serde_json::from_value(v).map_err(|e| format!("Objekt nicht lesbar: {e}"))?;
            if !o.visible || o.cut == Some(false) {
                continue;
            }
            let color = o.color.to_lowercase();
            let input = PathInput { d: o.d.clone(), matrix: path_matrix(&o, mirror.then_some(p.mat.w), shift) };
            if let Some(g) = groups.iter_mut().find(|g| g.label == color) {
                g.paths.push(input);
            } else {
                let mat = match by_color.iter().find(|(c, _)| *c == color) {
                    Some((_, m)) => *m,
                    None => all.unwrap_or(material(default_for(&color))?),
                };
                groups.push(Group { label: color, paths: vec![input], mat });
            }
        }
        // Lighter passes first so the sheet holds together while kiss-cutting.
        let sev = |m: &Material| if m.tool == Tool::Pen { -1 } else { m.depth as i32 * 10 + m.pressure as i32 + m.passes as i32 * 5 };
        groups.sort_by_key(|g| sev(g.mat));
        let mat = if p.mat.name.starts_with("Portrait 3") { MatKind::Portrait8x12 } else { MatKind::None };
        return Ok((groups, reg, mat));
    }
    let v = match ext.as_str() {
        "svg" => schnittwerk_core::import_svg(&data),
        "dxf" => schnittwerk_core::dxf_import::import_dxf(&data),
        _ => return Err("Unterstützt: .svg, .dxf, .schnitt".into()),
    }
    .map_err(|e| e.to_string())?;
    let matrix = if mirror { [-1.0, 0.0, 0.0, 1.0, 203.2, 0.0] } else { [1.0, 0.0, 0.0, 1.0, 0.0, 0.0] };
    let mat = all.unwrap_or(material("vinyl")?);
    Ok((vec![Group { label: "alle Linien".into(), paths: vec![PathInput { d: v.d, matrix }], mat }], None, MatKind::Portrait8x12))
}

fn cut_file(args: &Args) -> Result<(), String> {
    let path = args.pos.get(1).ok_or("Datei fehlt: schnittwerk-cli cut DATEI")?;
    let (groups, reg, project_mat) = load_groups(path, args)?;
    let mat = match args.get(&["--mat"]) {
        None => project_mat,
        Some("8x12") => MatKind::Portrait8x12,
        Some("8.5x12") => MatKind::Portrait8_5x12,
        Some("none") | Some("ohne") => MatKind::None,
        Some(m) => return Err(format!("--mat {m}: 8x12, 8.5x12 oder none")),
    };
    let over = |name: &str, max: u8| -> Result<Option<u8>, String> { Ok(args.num::<u8>(name)?.map(|v| v.clamp(1, max))) };
    let (p_over, s_over, d_over, n_over) = (over("--pressure", 33)?, over("--speed", 30)?, over("--depth", 10)?, over("--passes", 5)?);

    let mut job_groups = Vec::new();
    println!("{}", Path::new(path).display());
    for (i, g) in groups.iter().enumerate() {
        let m = g.mat;
        let passes = n_over.unwrap_or(m.passes);
        let opts = PathOptions { overcut: if m.tool == Tool::Pen { 0.0 } else { m.overcut }, passes, optimize: true };
        let (plan, strokes) = cut::plan(&g.paths, &opts)?;
        if strokes.is_empty() {
            continue;
        }
        let settings = CutSettings {
            tool: m.tool,
            pressure: p_over.unwrap_or(m.pressure),
            speed: s_over.unwrap_or(m.speed),
            depth: (m.tool == Tool::Blade).then(|| d_over.unwrap_or(m.depth)),
            mat,
            ..Default::default()
        };
        println!(
            "  {}. {}: {} – {} Pfade, {:.1} cm, Bereich {:.1}–{:.1} × {:.1}–{:.1} mm (Druck {}, Tempo {}{}, {}×)",
            i + 1,
            g.label,
            m.name,
            plan.strokes.len(),
            plan.cut_length / 10.0,
            plan.bbox[0],
            plan.bbox[0] + plan.bbox[2],
            plan.bbox[1],
            plan.bbox[1] + plan.bbox[3],
            settings.pressure,
            settings.speed,
            settings.depth.map(|d| format!(", Tiefe {d}")).unwrap_or_default(),
            passes
        );
        job_groups.push((strokes, settings));
    }
    if job_groups.is_empty() {
        return Err("Keine Schnittlinien gefunden.".into());
    }
    if reg.is_some() {
        println!("  Print & Cut: Passermarken werden vor dem Schnitt gesucht.");
    }
    if args.flag(&["--dry-run"]) {
        println!("Vorschau – nichts geschnitten.");
        return Ok(());
    }
    if !args.flag(&["-y", "--yes"]) {
        if !std::io::stdin().is_terminal() {
            return Err("Zum Schneiden ohne Rückfrage --yes angeben.".into());
        }
        print!("Jetzt schneiden? [j/N] ");
        std::io::stdout().flush().ok();
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer).map_err(|e| e.to_string())?;
        if !matches!(answer.trim().to_lowercase().as_str(), "j" | "ja" | "y" | "yes") {
            println!("Nicht geschnitten.");
            return Ok(());
        }
    }
    let plotter = connect(args)?;
    let (job, clamped) = gpgl::build_job(&job_groups, plotter.model.limits(), reg);
    if clamped > 0 {
        eprintln!("Achtung: {clamped} Punkte lagen außerhalb und werden an den Rand gesetzt.");
    }
    let never = AtomicBool::new(false);
    let n = job_groups.len();
    let outcome = plotter
        .run_job(
            &job,
            &never,
            &never,
            |done, total| {
                eprint!("\r  gesendet {:3} %", (done * 100).checked_div(total).unwrap_or(0));
            },
            |phase| match phase {
                Phase::Registering => eprintln!("  Suche Passermarken …"),
                Phase::Cutting => {}
                Phase::Pass(i) => eprintln!("\n  Schritt {}/{n}", i + 1),
                Phase::Draining(i) => eprint!("\r  Schritt {}/{n}: Plotter schneidet den Rest …", i + 1),
            },
        )
        .map_err(|e| e.to_string())?;
    eprintln!();
    match outcome {
        Outcome::Done => println!("Fertig."),
        Outcome::Cancelled => println!("Abgebrochen."),
    }
    Ok(())
}

fn main() {
    let args = Args::parse(std::env::args().skip(1));
    if args.flag(&["--help", "-h"]) || args.pos.is_empty() {
        print!("{HELP}");
        return;
    }
    let r = match args.pos[0].as_str() {
        "status" => status(&args),
        "scan" => scan(&args),
        "materials" | "materialien" => {
            for m in MATERIALS {
                println!("{:16} {} – Tiefe {}, Druck {}, Tempo {}, {}×", m.id, m.name, m.depth, m.pressure, m.speed, m.passes);
            }
            Ok(())
        }
        "trace" => trace(&args),
        "cut" | "schneiden" => cut_file(&args),
        c => Err(format!("Unbekannter Befehl „{c}“ – schnittwerk-cli --help")),
    };
    if let Err(e) = r {
        eprintln!("Fehler: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_mix_flags_options_and_positionals() {
        let a = Args::parse(["cut", "x.svg", "--material", "vinyl", "--mirror", "--offset=0.5,-1", "-y"].map(String::from).into_iter());
        assert_eq!(a.pos, vec!["cut", "x.svg"]);
        assert_eq!(a.get(&["--material"]), Some("vinyl"));
        assert!(a.flag(&["--mirror"]) && a.flag(&["-y"]));
        assert_eq!(a.get(&["--offset"]), Some("0.5,-1"));
    }

    #[test]
    fn matrix_matches_app_mapping() {
        // 10×10 box at (20, 30), path bbox 0..100, rotated 90°.
        let o = ProjPath {
            d: String::new(), bbox: [0.0, 0.0, 100.0, 100.0], color: "#e5484d".into(),
            x: 20.0, y: 30.0, w: 10.0, h: 10.0, rot: 90.0, flip_x: false, flip_y: false, visible: true, cut: None,
        };
        let m = Affine::new(path_matrix(&o, None, (0.0, 0.0)));
        // Local top-left goes to the box's top-right corner after a 90° turn.
        let p = m * kurbo::Point::new(0.0, 0.0);
        assert!((p.x - 30.0).abs() < 1e-9 && (p.y - 30.0).abs() < 1e-9, "{p:?}");
    }

    #[test]
    fn project_groups_by_colour_lighter_first() {
        let dir = std::env::temp_dir().join(format!("swcli-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("t.schnitt");
        let obj = |id: &str, color: &str| {
            format!(r#"{{"id":"{id}","type":"path","name":"x","d":"M0 0H10V10H0Z","bbox":[0,0,10,10],"color":"{color}","fill":false,"x":20,"y":20,"w":10,"h":10,"rot":0,"flipX":false,"flipY":false,"visible":true,"locked":false}}"#)
        };
        std::fs::write(
            &f,
            format!(r#"{{"app":"schnittwerk","version":1,"mat":{{"name":"A4 · Print & Cut","w":210,"h":297}},"printCut":{{"enabled":true,"page":"A4","insetX":15,"insetY":10}},"objects":[{},{}],"images":{{}}}}"#, obj("a", "#3E63DD"), obj("b", "#e5484d")),
        )
        .unwrap();
        let args = Args::parse(std::iter::empty());
        let (groups, reg, mat) = load_groups(f.to_str().unwrap(), &args).unwrap();
        assert_eq!(groups.iter().map(|g| g.mat.id).collect::<Vec<_>>(), vec!["sticker-matt", "sticker-through"]);
        assert!(reg.is_some_and(|r| r.width == 180.0 && r.length == 277.0));
        assert_eq!(mat, MatKind::None);
        std::fs::remove_dir_all(dir).ok();
    }
}
