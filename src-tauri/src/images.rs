//! Pictures in memory, how the web view gets them and how they are written
//! into project files.
//!
//! The web view never receives picture data as (huge) data URLs: it loads
//! `schnittwerk://localhost/img/<session>/<id>` from the custom protocol –
//! a reduced preview for the work area, `?full` for the original. Projects
//! and the recovery copy embed the pictures here in the backend, so saving
//! does not push megabytes through the IPC channel (that blocked the UI for
//! seconds with several photos).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use image::{DynamicImage, GenericImageView, ImageFormat};
use serde::Serialize;
use serde_json::Value;
use tauri::http::{header, Response, StatusCode};
use tauri::{AppHandle, Manager, State};

/// Longest side of the preview shown on the work area.
const PREVIEW_MAX: u32 = 2048;

struct Entry {
    img: Arc<DynamicImage>,
    /// Encoded picture as shown/saved (PNG for formats WebKit cannot show).
    bytes: Arc<Vec<u8>>,
    mime: &'static str,
    preview: OnceLock<(Arc<Vec<u8>>, &'static str)>,
}

/// Decoded pictures kept in memory so live tracing does not re-decode files.
#[derive(Default)]
pub struct ImageStore {
    entries: Mutex<HashMap<String, Arc<Entry>>>,
    next_id: Mutex<u64>,
}

impl ImageStore {
    fn insert(&self, img: DynamicImage, bytes: Vec<u8>, mime: &'static str) -> String {
        let mut n = self.next_id.lock().unwrap();
        *n += 1;
        let id = format!("img{}", *n);
        let entry = Entry { img: Arc::new(img), bytes: Arc::new(bytes), mime, preview: OnceLock::new() };
        self.entries.lock().unwrap().insert(id.clone(), Arc::new(entry));
        id
    }

    fn entry(&self, id: &str) -> Option<Arc<Entry>> {
        self.entries.lock().unwrap().get(id).cloned()
    }

    pub fn get(&self, id: &str) -> Result<Arc<DynamicImage>, String> {
        self.entry(id).map(|e| e.img.clone()).ok_or_else(|| "Bild nicht mehr im Speicher – bitte neu laden.".to_string())
    }

    /// Data URL of the stored picture (for project files).
    fn data_url(&self, id: &str) -> Option<String> {
        self.entry(id).map(|e| format!("data:{};base64,{}", e.mime, B64.encode(e.bytes.as_slice())))
    }
}

/// Changes with every start, so the web view never shows a cached picture
/// of an earlier session under the same id.
fn session() -> &'static str {
    static S: OnceLock<String> = OnceLock::new();
    S.get_or_init(|| format!("{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)))
}

fn image_url(id: &str) -> String {
    format!("schnittwerk://localhost/img/{}/{id}", session())
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImageInfo {
    id: String,
    name: String,
    width: u32,
    height: u32,
    /// URL of the picture for the web view (preview size; `?full` = original).
    src: String,
}

pub fn register_image(store: &ImageStore, bytes: Vec<u8>, name: String) -> Result<ImageInfo, String> {
    let format = image::guess_format(&bytes).map_err(|e| format!("Unbekanntes Bildformat: {e}"))?;
    let img = image::load_from_memory_with_format(&bytes, format).map_err(|e| e.to_string())?;
    // WebKitGTK cannot show every format (e.g. TIFF) – re-encode those as PNG.
    let (mime, bytes) = match format {
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP | ImageFormat::Gif => (format.to_mime_type(), bytes),
        _ => {
            let mut buf = std::io::Cursor::new(Vec::new());
            img.write_to(&mut buf, ImageFormat::Png).map_err(|e| e.to_string())?;
            ("image/png", buf.into_inner())
        }
    };
    let (width, height) = (img.width(), img.height());
    let id = store.insert(img, bytes, mime);
    Ok(ImageInfo { src: image_url(&id), id, name, width, height })
}

/// Preview of at most PREVIEW_MAX px (the original when it is small enough).
fn preview(entry: &Entry) -> (Arc<Vec<u8>>, &'static str) {
    entry
        .preview
        .get_or_init(|| {
            let (w, h) = entry.img.dimensions();
            if w.max(h) <= PREVIEW_MAX {
                return (entry.bytes.clone(), entry.mime);
            }
            let small = entry.img.thumbnail(PREVIEW_MAX, PREVIEW_MAX);
            let mut buf = std::io::Cursor::new(Vec::new());
            let alpha = small.color().has_alpha();
            let ok = if alpha {
                small.write_to(&mut buf, ImageFormat::Png).is_ok()
            } else {
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 88).encode_image(&small.to_rgb8()).is_ok()
            };
            if ok {
                (Arc::new(buf.into_inner()), if alpha { "image/png" } else { "image/jpeg" })
            } else {
                (entry.bytes.clone(), entry.mime)
            }
        })
        .clone()
}

/// `schnittwerk://localhost/img/<session>/<id>[?full]`
pub fn handle_protocol(app: &AppHandle, request: &tauri::http::Request<Vec<u8>>) -> Response<Vec<u8>> {
    let not_found = || Response::builder().status(StatusCode::NOT_FOUND).body(Vec::new()).unwrap();
    let uri = request.uri();
    let parts: Vec<&str> = uri.path().trim_start_matches('/').split('/').collect();
    let (Some(&"img"), Some(_), Some(id)) = (parts.first(), parts.get(1), parts.get(2)) else { return not_found() };
    let store = app.state::<ImageStore>();
    let Some(entry) = store.entry(id) else { return not_found() };
    let full = uri.query().is_some_and(|q| q.split('&').any(|p| p == "full"));
    let (bytes, mime) = if full { (entry.bytes.clone(), entry.mime) } else { preview(&entry) };
    Response::builder()
        .header(header::CONTENT_TYPE, mime)
        // Canvas drawing (print, brush) needs CORS for a different origin.
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::CACHE_CONTROL, "max-age=31536000, immutable")
        .body(bytes.as_slice().to_vec())
        .unwrap()
}

// ---------------------------------------------------------------- projects

/// Puts the picture data into the project JSON: `images.<id>.src`.
fn embed_images(store: &ImageStore, json: &str) -> Result<String, String> {
    let mut v: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    if let Some(images) = v.get_mut("images").and_then(|i| i.as_object_mut()) {
        for (id, img) in images.iter_mut() {
            let src = store.data_url(id).ok_or_else(|| "Bild nicht mehr im Speicher – bitte neu laden.".to_string())?;
            if let Some(o) = img.as_object_mut() {
                o.insert("src".into(), Value::String(src));
            }
        }
    }
    serde_json::to_string(&v).map_err(|e| e.to_string())
}

/// Written next to the target and renamed: a crash never leaves half a file.
fn write_atomic(path: &Path, contents: &str) -> Result<(), String> {
    let tmp = path.with_extension("schnitt-tmp");
    std::fs::write(&tmp, contents).map_err(|e| format!("{}: {e}", path.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Registers the embedded pictures and replaces them by their info
/// (`images.<old id>` = { id, name, width, height, src }).
fn load_project(store: &ImageStore, path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut v: Value = serde_json::from_str(&text).map_err(|_| "Keine Schnittwerk-Projektdatei.".to_string())?;
    if v.get("app").and_then(|a| a.as_str()) != Some("schnittwerk") {
        return Err("Keine Schnittwerk-Projektdatei.".into());
    }
    if let Some(images) = v.get_mut("images").and_then(|i| i.as_object_mut()) {
        for img in images.values_mut() {
            let name = img.get("name").and_then(|n| n.as_str()).unwrap_or("Bild").to_string();
            let src = img.get("src").and_then(|s| s.as_str()).unwrap_or("");
            let b64 = src.split_once(',').map(|(_, b)| b).unwrap_or(src);
            let bytes = B64.decode(b64).map_err(|e| e.to_string())?;
            let info = register_image(store, bytes, name)?;
            *img = serde_json::to_value(info).map_err(|e| e.to_string())?;
        }
    }
    Ok(v)
}

#[tauri::command]
pub async fn project_save(path: String, contents: String, store: State<'_, ImageStore>) -> Result<(), String> {
    let json = embed_images(&store, &contents)?;
    write_atomic(Path::new(&path), &json)
}

/// Opens a project (`path`) or the recovery copy (`None`).
#[tauri::command]
pub async fn project_open(app: AppHandle, path: Option<String>, store: State<'_, ImageStore>) -> Result<Value, String> {
    let path = match path {
        Some(p) => PathBuf::from(p),
        None => autosave_path(&app)?,
    };
    load_project(&store, &path)
}

// ---------------------------------------------------------------- autosave

/// `~/.local/share/<app>/autosave.schnitt`
fn autosave_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("autosave.schnitt"))
}

/// The recovery copy; the pictures are embedded here, not sent by the UI.
#[tauri::command]
pub async fn autosave_write(app: AppHandle, contents: String, store: State<'_, ImageStore>) -> Result<(), String> {
    let json = embed_images(&store, &contents)?;
    write_atomic(&autosave_path(&app)?, &json)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutosaveInfo {
    saved_at: f64,
    file_path: Option<String>,
    objects: usize,
}

/// Whether there is a recovery copy, without loading its pictures.
#[tauri::command]
pub async fn autosave_info(app: AppHandle) -> Result<Option<AutosaveInfo>, String> {
    let path = autosave_path(&app)?;
    let text = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let Ok(v) = serde_json::from_str::<Value>(&text) else { return Ok(None) };
    let objects = v.get("objects").and_then(|o| o.as_array()).map_or(0, |a| a.len());
    if v.get("app").and_then(|a| a.as_str()) != Some("schnittwerk") || objects == 0 {
        return Ok(None);
    }
    let meta = v.get("autosave");
    Ok(Some(AutosaveInfo {
        saved_at: meta.and_then(|m| m.get("savedAt")).and_then(|s| s.as_f64()).unwrap_or(0.0),
        file_path: meta.and_then(|m| m.get("filePath")).and_then(|s| s.as_str()).map(String::from),
        objects,
    }))
}

#[tauri::command]
pub async fn autosave_clear(app: AppHandle) -> Result<(), String> {
    match std::fs::remove_file(autosave_path(&app)?) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = DynamicImage::new_rgb8(w, h);
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, ImageFormat::Png).unwrap();
        buf.into_inner()
    }

    #[test]
    fn project_round_trip_embeds_and_registers_pictures() {
        let store = ImageStore::default();
        let info = register_image(&store, png(10, 8), "Test".into()).unwrap();
        assert!(info.src.starts_with("schnittwerk://localhost/img/"));
        let json = format!(r#"{{"app":"schnittwerk","version":1,"objects":[{{"type":"image","imageId":"{0}"}}],"images":{{"{0}":{{"name":"Test"}}}}}}"#, info.id);
        let saved = embed_images(&store, &json).unwrap();
        assert!(saved.contains("data:image/png;base64,"));
        let dir = std::env::temp_dir().join(format!("schnittwerk-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("p.schnitt");
        write_atomic(&path, &saved).unwrap();
        let loaded = load_project(&store, &path).unwrap();
        let img = &loaded["images"][&info.id];
        assert_eq!(img["width"], 10);
        assert!(img["src"].as_str().unwrap().starts_with("schnittwerk://"));
        assert_ne!(img["id"], info.id.as_str(), "registered again under a new id");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn big_pictures_get_a_smaller_preview() {
        let store = ImageStore::default();
        let info = register_image(&store, png(3000, 1000), "Groß".into()).unwrap();
        let entry = store.entry(&info.id).unwrap();
        let (bytes, mime) = preview(&entry);
        let p = image::load_from_memory(&bytes).unwrap();
        assert_eq!(p.width(), PREVIEW_MAX);
        assert_eq!(mime, "image/jpeg");
    }
}
