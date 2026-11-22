//! Printing for print & cut: the page arrives as a 300 dpi PNG rendered by
//! the UI, gets wrapped into a single-page PDF of the exact paper size and
//! goes to CUPS with scaling disabled (scaling would break registration).

use std::io::Write;
use std::process::Command;

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use flate2::{write::ZlibEncoder, Compression};
use serde::Serialize;

/// Builds a one-page PDF that shows `rgb` (w×h pixels) over the full page.
pub fn page_pdf(rgb: &[u8], w: u32, h: u32, page_w_mm: f64, page_h_mm: f64) -> Vec<u8> {
    let pt = |mm: f64| mm * 72.0 / 25.4;
    let (pw, ph) = (pt(page_w_mm), pt(page_h_mm));
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
    enc.write_all(rgb).expect("in-memory write");
    let img = enc.finish().expect("in-memory write");
    let content = format!("q {pw:.3} 0 0 {ph:.3} 0 0 cm /Im0 Do Q\n");

    let mut out: Vec<u8> = Vec::new();
    let mut offsets = Vec::new();
    out.extend_from_slice(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n");
    let mut obj = |out: &mut Vec<u8>, body: &[u8]| {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", offsets.len()).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    };
    obj(&mut out, b"<< /Type /Catalog /Pages 2 0 R >>");
    obj(&mut out, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
    obj(
        &mut out,
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {pw:.3} {ph:.3}] /Resources << /XObject << /Im0 4 0 R >> >> /Contents 5 0 R >>").as_bytes(),
    );
    let mut image = format!(
        "<< /Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode /Length {} >>\nstream\n",
        img.len()
    )
    .into_bytes();
    image.extend_from_slice(&img);
    image.extend_from_slice(b"\nendstream");
    obj(&mut out, &image);
    obj(&mut out, format!("<< /Length {} >>\nstream\n{content}endstream", content.len()).as_bytes());

    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1).as_bytes());
    for o in &offsets {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", offsets.len() + 1).as_bytes());
    out
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Printers {
    names: Vec<String>,
    default: Option<String>,
}

#[tauri::command]
pub async fn list_printers() -> Printers {
    let run = |args: &[&str]| Command::new("lpstat").args(args).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
    let names = run(&["-e"]).map(|s| s.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()).unwrap_or_default();
    let default = run(&["-d"]).and_then(|s| s.split(':').nth(1).map(|n| n.trim().to_string())).filter(|n| !n.is_empty() && !n.contains(' '));
    Printers { names, default }
}

/// `png` is a data URL. Prints with `printer` (CUPS name; `None` = default)
/// or, if `save_path` is set, only writes the PDF there.
#[tauri::command]
pub async fn print_page(
    png: String,
    page_w_mm: f64,
    page_h_mm: f64,
    media: String,
    printer: Option<String>,
    save_path: Option<String>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let b64 = png.split_once(',').map(|(_, b)| b).unwrap_or(&png);
        let bytes = B64.decode(b64).map_err(|e| e.to_string())?;
        let img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?.to_rgb8();
        let pdf = page_pdf(img.as_raw(), img.width(), img.height(), page_w_mm, page_h_mm);
        if let Some(path) = save_path {
            return std::fs::write(&path, pdf).map_err(|e| format!("{path}: {e}"));
        }
        let file = std::env::temp_dir().join(format!("schnittwerk-{}.pdf", std::process::id()));
        std::fs::write(&file, pdf).map_err(|e| e.to_string())?;
        let mut cmd = Command::new("lp");
        if let Some(p) = printer.filter(|p| !p.is_empty()) {
            cmd.args(["-d", &p]);
        }
        cmd.args(["-t", "Schnittwerk Print & Cut", "-o", &format!("media={media}"), "-o", "print-scaling=none", "-o", "fit-to-page=false"]);
        cmd.arg(&file);
        let out = cmd.output().map_err(|e| format!("lp konnte nicht gestartet werden: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(format!("Drucken fehlgeschlagen: {}", String::from_utf8_lossy(&out.stderr).trim()))
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    #[test]
    fn writes_a4_pdf() {
        let rgb = vec![200u8; 30 * 40 * 3];
        let pdf = super::page_pdf(&rgb, 30, 40, 210.0, 297.0);
        assert!(pdf.starts_with(b"%PDF-1.4"));
        if let Ok(p) = std::env::var("PDF_OUT") {
            std::fs::write(p, &pdf).unwrap();
        }
    }
}
