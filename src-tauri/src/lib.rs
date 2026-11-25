//! Tauri backend: exposes the core library to the web UI as commands.

mod plotter;
mod print;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use image::{DynamicImage, GenericImageView, ImageFormat};
use schnittwerk_core::cut::PathInput;
use schnittwerk_core::offset::OffsetResult;
use schnittwerk_core::sticker::{MotifMask, StickerParams};
use schnittwerk_core::text::TextSpec;
use schnittwerk_core::{ImportedVector, TraceParams, TraceResult};
use serde::Serialize;
use tauri::State;

/// Decoded images kept in memory so live tracing does not re-decode files.
#[derive(Default)]
struct ImageStore {
    images: Mutex<HashMap<String, Arc<DynamicImage>>>,
    next_id: Mutex<u64>,
}

impl ImageStore {
    fn insert(&self, img: DynamicImage) -> String {
        let mut n = self.next_id.lock().unwrap();
        *n += 1;
        let id = format!("img{}", *n);
        self.images.lock().unwrap().insert(id.clone(), Arc::new(img));
        id
    }
    fn get(&self, id: &str) -> Result<Arc<DynamicImage>, String> {
        self.images
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or_else(|| "Bild nicht mehr im Speicher – bitte neu laden.".to_string())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImageInfo {
    id: String,
    name: String,
    width: u32,
    height: u32,
    /// Data URL the web view can display directly.
    src: String,
}

fn register_image(store: &ImageStore, bytes: Vec<u8>, name: String) -> Result<ImageInfo, String> {
    let format = image::guess_format(&bytes).map_err(|e| format!("Unbekanntes Bildformat: {e}"))?;
    let img = image::load_from_memory_with_format(&bytes, format).map_err(|e| e.to_string())?;
    // WebKitGTK cannot show every format (e.g. TIFF) – re-encode those as PNG.
    let (mime, display_bytes) = match format {
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP | ImageFormat::Gif => {
            (format.to_mime_type(), bytes)
        }
        _ => {
            let mut buf = std::io::Cursor::new(Vec::new());
            img.write_to(&mut buf, ImageFormat::Png).map_err(|e| e.to_string())?;
            ("image/png", buf.into_inner())
        }
    };
    let (width, height) = (img.width(), img.height());
    let id = store.insert(img);
    Ok(ImageInfo { id, name, width, height, src: format!("data:{mime};base64,{}", B64.encode(display_bytes)) })
}

fn file_stem(path: &str) -> String {
    Path::new(path).file_stem().and_then(|s| s.to_str()).unwrap_or("Bild").to_string()
}

#[tauri::command]
async fn load_image(path: String, store: State<'_, ImageStore>) -> Result<ImageInfo, String> {
    let bytes = std::fs::read(&path).map_err(|e| format!("{path}: {e}"))?;
    register_image(&store, bytes, file_stem(&path))
}

/// Registers an image given as data URL (project files, clipboard).
#[tauri::command]
async fn load_image_data(data_url: String, name: String, store: State<'_, ImageStore>) -> Result<ImageInfo, String> {
    let b64 = data_url.split_once(',').map(|(_, b)| b).unwrap_or(&data_url);
    let bytes = B64.decode(b64).map_err(|e| e.to_string())?;
    register_image(&store, bytes, name)
}

/// Corners of a sheet of paper in a photo (image pixels), if one is visible.
#[tauri::command]
async fn detect_page(id: String, store: State<'_, ImageStore>) -> Result<Option<schnittwerk_core::photo::Quad>, String> {
    let img = store.get(&id)?;
    tauri::async_runtime::spawn_blocking(move || schnittwerk_core::photo::detect_page(&img))
        .await
        .map_err(|e| e.to_string())
}

/// Straightens the quad of an image into a rectangle (perspective
/// correction). `aspect` = width / height of the target (e.g. a paper size).
#[tauri::command]
async fn warp_image(
    id: String,
    quad: schnittwerk_core::photo::Quad,
    aspect: Option<f64>,
    name: String,
    store: State<'_, ImageStore>,
) -> Result<ImageInfo, String> {
    let img = store.get(&id)?;
    let png = tauri::async_runtime::spawn_blocking(move || -> Result<Vec<u8>, String> {
        let (w, h) = schnittwerk_core::photo::output_size(&quad, aspect, 6000);
        let out = schnittwerk_core::photo::warp(&img, &quad, w, h).ok_or("Die Ecken bilden kein gültiges Viereck.")?;
        let mut buf = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(out).write_to(&mut buf, ImageFormat::Png).map_err(|e| e.to_string())?;
        Ok(buf.into_inner())
    })
    .await
    .map_err(|e| e.to_string())??;
    register_image(&store, png, name)
}

/// Finds the frame of a PixScan mat in a photo.
#[tauri::command]
async fn detect_pixscan(id: String, store: State<'_, ImageStore>) -> Result<Option<schnittwerk_core::pixscan::Detection>, String> {
    let img = store.get(&id)?;
    tauri::async_runtime::spawn_blocking(move || schnittwerk_core::pixscan::detect(&img))
        .await
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PixScanImage {
    info: ImageInfo,
    /// Photo kept around the frame on every side (mm).
    margin: f64,
    frame_w: f64,
    frame_h: f64,
}

/// Straightens a PixScan photo from its frame corners (plotter orientation)
/// into an image in true millimetres.
#[tauri::command]
async fn pixscan_rectify(
    id: String,
    frame: schnittwerk_core::photo::Quad,
    name: String,
    store: State<'_, ImageStore>,
) -> Result<PixScanImage, String> {
    use schnittwerk_core::pixscan;
    let img = store.get(&id)?;
    let png = tauri::async_runtime::spawn_blocking(move || -> Result<Vec<u8>, String> {
        let (out, _) = pixscan::rectify(&img, &frame).ok_or("Die Ecken bilden kein gültiges Viereck.")?;
        let mut buf = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(out).write_to(&mut buf, ImageFormat::Png).map_err(|e| e.to_string())?;
        Ok(buf.into_inner())
    })
    .await
    .map_err(|e| e.to_string())??;
    let info = register_image(&store, png, name)?;
    Ok(PixScanImage { info, margin: pixscan::MARGIN, frame_w: pixscan::FRAME_W, frame_h: pixscan::FRAME_H })
}

#[tauri::command]
async fn trace_image(id: String, params: TraceParams, store: State<'_, ImageStore>) -> Result<TraceResult, String> {
    let img = store.get(&id)?;
    tauri::async_runtime::spawn_blocking(move || schnittwerk_core::trace(&img, &params))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn sticker_outline(id: String, params: StickerParams, store: State<'_, ImageStore>) -> Result<TraceResult, String> {
    let img = store.get(&id)?;
    tauri::async_runtime::spawn_blocking(move || schnittwerk_core::sticker::sticker_outline(&img, &params))
        .await
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BleedResult {
    info: ImageInfo,
    /// Pixels added on every side.
    pad: u32,
}

/// Extends the motif colours outwards; returns a new, padded image.
#[tauri::command]
async fn bleed_image(id: String, mask: MotifMask, bleed_px: u32, store: State<'_, ImageStore>) -> Result<BleedResult, String> {
    let img = store.get(&id)?;
    let bleed_px = bleed_px.clamp(1, 400);
    let png = tauri::async_runtime::spawn_blocking(move || {
        let out = schnittwerk_core::sticker::bleed(&img, mask, bleed_px);
        let mut buf = std::io::Cursor::new(Vec::new());
        out.write_to(&mut buf, ImageFormat::Png).map(|_| buf.into_inner()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;
    let info = register_image(&store, png, "Bleed".into())?;
    Ok(BleedResult { info, pad: bleed_px })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
enum ClipboardContent {
    Image { info: ImageInfo },
    Svg { vector: ImportedVector },
    None,
}

/// Reads an image (or SVG markup) from the system clipboard.
#[tauri::command]
async fn clipboard_read(store: State<'_, ImageStore>) -> Result<ClipboardContent, String> {
    let read = tauri::async_runtime::spawn_blocking(|| -> Result<(Option<Vec<u8>>, Option<String>), String> {
        let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
        if let Ok(img) = cb.get_image() {
            let rgba = image::RgbaImage::from_raw(img.width as u32, img.height as u32, img.bytes.into_owned())
                .ok_or("Ungültiges Bild in der Zwischenablage")?;
            let mut buf = std::io::Cursor::new(Vec::new());
            rgba.write_to(&mut buf, ImageFormat::Png).map_err(|e| e.to_string())?;
            return Ok((Some(buf.into_inner()), None));
        }
        Ok((None, cb.get_text().ok()))
    })
    .await
    .map_err(|e| e.to_string())??;
    match read {
        (Some(png), _) => Ok(ClipboardContent::Image { info: register_image(&store, png, "Zwischenablage".into())? }),
        (None, Some(text)) if text.contains("<svg") => {
            let vector = schnittwerk_core::import_svg(text.as_bytes()).map_err(|e| e.to_string())?;
            Ok(ClipboardContent::Svg { vector })
        }
        _ => Ok(ClipboardContent::None),
    }
}

/// Replaces the system clipboard with plain text, so that a following
/// paste inserts the objects copied inside Schnittwerk.
#[tauri::command]
async fn clipboard_mark() {
    let _ = tauri::async_runtime::spawn_blocking(|| {
        if let Ok(mut cb) = arboard::Clipboard::new() {
            let _ = cb.set_text("Schnittwerk-Objekte");
        }
    })
    .await;
}

#[tauri::command]
async fn font_families() -> Vec<String> {
    tauri::async_runtime::spawn_blocking(schnittwerk_core::text::families).await.unwrap_or_default()
}

#[tauri::command]
async fn text_to_path(spec: TextSpec) -> Result<ImportedVector, String> {
    tauri::async_runtime::spawn_blocking(move || schnittwerk_core::text::text_to_path(&spec).map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn nest(req: schnittwerk_core::nest::NestRequest) -> Result<schnittwerk_core::nest::NestResult, String> {
    tauri::async_runtime::spawn_blocking(move || schnittwerk_core::nest::nest(&req))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn boolean_paths(paths: Vec<PathInput>, op: schnittwerk_core::boolean::BoolOp) -> Result<OffsetResult, String> {
    tauri::async_runtime::spawn_blocking(move || schnittwerk_core::boolean::boolean(&paths, op))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn offset_paths(paths: Vec<PathInput>, offset: f64, smoothness: f32) -> Result<OffsetResult, String> {
    tauri::async_runtime::spawn_blocking(move || schnittwerk_core::offset::offset_paths(&paths, offset, smoothness))
        .await
        .map_err(|e| e.to_string())?
}

/// Color at a pixel position (for the color picker in trace mode).
#[tauri::command]
async fn sample_color(id: String, x: u32, y: u32, store: State<'_, ImageStore>) -> Result<[u8; 3], String> {
    let img = store.get(&id)?;
    let x = x.min(img.width().saturating_sub(1));
    let y = y.min(img.height().saturating_sub(1));
    let p = img.get_pixel(x, y).0;
    Ok([p[0], p[1], p[2]])
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NamedVector {
    name: String,
    #[serde(flatten)]
    vector: ImportedVector,
}

#[tauri::command]
async fn import_svg(path: String) -> Result<NamedVector, String> {
    let bytes = std::fs::read(&path).map_err(|e| format!("{path}: {e}"))?;
    let vector = schnittwerk_core::import_svg(&bytes).map_err(|e| e.to_string())?;
    Ok(NamedVector { name: file_stem(&path), vector })
}

#[tauri::command]
async fn import_dxf(path: String) -> Result<NamedVector, String> {
    let bytes = std::fs::read(&path).map_err(|e| format!("{path}: {e}"))?;
    let vector = tauri::async_runtime::spawn_blocking(move || schnittwerk_core::dxf_import::import_dxf(&bytes))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    Ok(NamedVector { name: file_stem(&path), vector })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
enum PdfContent {
    Vector { vector: NamedVector },
    Image { info: ImageInfo },
}

/// Imports page `page` (1-based) of a PDF via poppler's `pdftocairo`:
/// vector content as paths, otherwise (scans) the page as a 300 dpi image.
#[tauri::command]
async fn import_pdf(path: String, page: Option<u32>, store: State<'_, ImageStore>) -> Result<PdfContent, String> {
    let page = page.unwrap_or(1).max(1).to_string();
    let dir = std::env::temp_dir().join(format!("schnittwerk-pdf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let run = |args: &[&str]| -> Result<(), String> {
        let out = std::process::Command::new("pdftocairo")
            .args(args)
            .output()
            .map_err(|e| format!("pdftocairo nicht gefunden (Paket poppler-utils): {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(format!("PDF konnte nicht gelesen werden: {}", String::from_utf8_lossy(&out.stderr).trim()))
        }
    };
    let svg = dir.join("page.svg");
    let svg_s = svg.to_string_lossy().to_string();
    // Exact 1:1 geometry: no fitting to a paper size, no centring.
    run(&["-svg", "-origpagesizes", "-noshrink", "-nocenter", "-f", &page, "-l", &page, &path, &svg_s])?;
    if let Ok(bytes) = std::fs::read(&svg) {
        if let Ok(vector) = schnittwerk_core::import_svg(&bytes) {
            let _ = std::fs::remove_dir_all(&dir);
            return Ok(PdfContent::Vector { vector: NamedVector { name: file_stem(&path), vector } });
        }
    }
    // No vector paths: render the page.
    let base = dir.join("page");
    let base_s = base.to_string_lossy().to_string();
    run(&["-png", "-r", "300", "-singlefile", "-f", &page, "-l", &page, &path, &base_s])?;
    let png = std::fs::read(dir.join("page.png")).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&dir);
    Ok(PdfContent::Image { info: register_image(&store, png, file_stem(&path))? })
}

#[tauri::command]
async fn read_text(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))
}

#[tauri::command]
async fn write_text(path: String, contents: String) -> Result<(), String> {
    std::fs::write(&path, contents).map_err(|e| format!("{path}: {e}"))
}

/// Fewer nodes for a path (tolerance in path units).
#[tauri::command]
async fn simplify_path(d: String, tolerance: f64, corner_deg: f64) -> Result<schnittwerk_core::simplify::SimplifyResult, String> {
    tauri::async_runtime::spawn_blocking(move || schnittwerk_core::simplify::simplify(&d, tolerance, corner_deg))
        .await
        .map_err(|e| e.to_string())?
}

// ---------- autosave ----------

/// `~/.local/share/<app>/autosave.schnitt`
fn autosave_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    use tauri::Manager;
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("autosave.schnitt"))
}

/// Writes the recovery copy atomically (temp file + rename), so a crash
/// while saving never leaves a broken file behind.
#[tauri::command]
async fn autosave_write(app: tauri::AppHandle, contents: String) -> Result<(), String> {
    let path = autosave_path(&app)?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, contents).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

#[tauri::command]
async fn autosave_read(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let path = autosave_path(&app)?;
    match std::fs::read_to_string(&path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
async fn autosave_clear(app: tauri::AppHandle) -> Result<(), String> {
    let path = autosave_path(&app)?;
    match std::fs::remove_file(&path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
        _ => Ok(()),
    }
}

/// Whether a file still exists (recent files list).
#[tauri::command]
async fn file_exists(path: String) -> bool {
    Path::new(&path).is_file()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(ImageStore::default())
        .manage(plotter::PlotterManager::default())
        .invoke_handler(tauri::generate_handler![
            load_image,
            load_image_data,
            trace_image,
            detect_page,
            warp_image,
            detect_pixscan,
            pixscan_rectify,
            sample_color,
            import_svg,
            import_dxf,
            import_pdf,
            read_text,
            write_text,
            simplify_path,
            autosave_write,
            autosave_read,
            autosave_clear,
            file_exists,
            plotter::plotter_status,
            plotter::cut_preview,
            plotter::cut_start,
            plotter::cut_test,
            plotter::cut_regtest,
            plotter::ble_scan,
            plotter::ble_connect,
            plotter::plotter_disconnect,
            plotter::cut_pause,
            plotter::cut_cancel,
            sticker_outline,
            offset_paths,
            boolean_paths,
            nest,
            bleed_image,
            font_families,
            clipboard_read,
            clipboard_mark,
            text_to_path,
            print::list_printers,
            print::print_page
        ])
        .run(tauri::generate_context!())
        .expect("Fehler beim Starten von Schnittwerk");
}
