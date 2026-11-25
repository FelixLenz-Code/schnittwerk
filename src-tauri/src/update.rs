//! Updates from GitHub releases: check for a newer version, download the
//! matching package (.deb or Flatpak), verify it and install it.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};

const REPO: &str = "FelixLenz-Code/schnittwerk";
const APP_ID: &str = "de.schnittwerk.desktop";

/// How this copy was installed – decides how it can update itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InstallKind {
    Deb,
    Flatpak,
    /// Built from source or unknown: only point to the release page.
    Manual,
}

fn install_kind() -> InstallKind {
    if crate::in_flatpak() {
        return InstallKind::Flatpak;
    }
    let exe = std::env::current_exe().ok();
    if exe.as_deref() == Some(Path::new("/usr/bin/schnittwerk")) && Path::new("/var/lib/dpkg/info/schnittwerk.list").exists() {
        return InstallKind::Deb;
    }
    InstallKind::Manual
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    name: String,
    url: String,
    size: u64,
    /// "sha256:…" as reported by GitHub (may be missing on old releases).
    digest: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    current: String,
    latest: String,
    newer: bool,
    kind: InstallKind,
    /// Release notes (Markdown).
    notes: String,
    /// Release page.
    url: String,
    /// Package for this installation, if the release has one.
    asset: Option<Asset>,
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    size: u64,
    #[serde(default)]
    digest: Option<String>,
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(10))
        .user_agent(concat!("schnittwerk/", env!("CARGO_PKG_VERSION")))
        .build()
}

/// "v1.2.3" → (1, 2, 3); missing parts count as 0.
fn parse_version(v: &str) -> (u64, u64, u64) {
    let mut it = v.trim().trim_start_matches('v').split(['.', '-', '+']).map(|p| p.parse::<u64>().unwrap_or(0));
    (it.next().unwrap_or(0), it.next().unwrap_or(0), it.next().unwrap_or(0))
}

fn pick_asset(assets: &[GhAsset], version: &str, kind: InstallKind) -> Option<Asset> {
    let wanted: Vec<String> = match kind {
        InstallKind::Deb => vec![format!("Schnittwerk_{version}_amd64.deb"), "Schnittwerk_amd64.deb".into()],
        InstallKind::Flatpak => vec![format!("Schnittwerk_{version}_x86_64.flatpak"), "Schnittwerk.flatpak".into()],
        InstallKind::Manual => vec![],
    };
    wanted.iter().find_map(|w| assets.iter().find(|a| &a.name == w)).map(|a| Asset {
        name: a.name.clone(),
        url: a.browser_download_url.clone(),
        size: a.size,
        digest: a.digest.clone(),
    })
}

fn check_blocking() -> Result<UpdateInfo, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let rel: GhRelease = agent()
        .get(&url)
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("Update-Prüfung fehlgeschlagen: {e}"))?
        .into_string()
        .map_err(|e| format!("Antwort von GitHub nicht lesbar: {e}"))
        .and_then(|s| serde_json::from_str(&s).map_err(|e| format!("Antwort von GitHub nicht lesbar: {e}")))?;
    let current = env!("CARGO_PKG_VERSION").to_string();
    let latest = rel.tag_name.trim_start_matches('v').to_string();
    let newer = !rel.draft && !rel.prerelease && parse_version(&latest) > parse_version(&current);
    let kind = install_kind();
    Ok(UpdateInfo {
        asset: pick_asset(&rel.assets, &latest, kind),
        current,
        latest,
        newer,
        kind,
        notes: rel.body.unwrap_or_default(),
        url: rel.html_url,
    })
}

#[tauri::command]
pub async fn update_check() -> Result<UpdateInfo, String> {
    tauri::async_runtime::spawn_blocking(check_blocking).await.map_err(|e| e.to_string())?
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    /// download | install
    phase: &'static str,
    done: u64,
    total: u64,
}

/// Downloads the asset and checks its SHA-256 (when GitHub reports one).
fn download(app: &AppHandle, asset: &Asset, dir: &Path) -> Result<PathBuf, String> {
    if asset.name.contains('/') || asset.name.contains("..") || !asset.url.starts_with("https://github.com/") {
        return Err("Ungültige Datei im Release.".into());
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.join(&asset.name);
    let resp = agent().get(&asset.url).call().map_err(|e| format!("Download fehlgeschlagen: {e}"))?;
    let total = resp.header("Content-Length").and_then(|v| v.parse().ok()).unwrap_or(asset.size);
    let mut reader = resp.into_reader();
    let mut file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut done = 0u64;
    let mut last_emit = 0u64;
    loop {
        let n = reader.read(&mut buf).map_err(|e| format!("Download abgebrochen: {e}"))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        hasher.update(&buf[..n]);
        done += n as u64;
        if done - last_emit > 256 * 1024 || done == total {
            last_emit = done;
            let _ = app.emit("update-progress", Progress { phase: "download", done, total });
        }
    }
    file.flush().map_err(|e| e.to_string())?;
    if let Some(expected) = asset.digest.as_deref().and_then(|d| d.strip_prefix("sha256:")) {
        let got = format!("{:x}", hasher.finalize());
        if !got.eq_ignore_ascii_case(expected) {
            let _ = std::fs::remove_file(&path);
            return Err("Prüfsumme der heruntergeladenen Datei stimmt nicht – Update abgebrochen.".into());
        }
    }
    Ok(path)
}

/// `--system` or `--user` installation of the running Flatpak.
fn flatpak_installation() -> &'static str {
    let info = std::fs::read_to_string("/.flatpak-info").unwrap_or_default();
    let app_path = info.lines().find_map(|l| l.strip_prefix("app-path=")).unwrap_or("");
    if app_path.starts_with("/var/lib/flatpak") {
        "--system"
    } else {
        "--user"
    }
}

fn run(mut cmd: std::process::Command, what: &str) -> Result<(), String> {
    let out = cmd.output().map_err(|e| format!("{what} konnte nicht gestartet werden: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    // pkexec: 126 = authentication dismissed, 127 = not authorised.
    if what == "pkexec" && matches!(out.status.code(), Some(126) | Some(127)) {
        return Err("Passwortabfrage abgebrochen – Update nicht installiert.".into());
    }
    let err = String::from_utf8_lossy(&out.stderr);
    let msg = err.lines().filter(|l| !l.trim().is_empty()).last().unwrap_or("unbekannter Fehler").trim().to_string();
    Err(format!("Installation fehlgeschlagen: {msg}"))
}

fn install_blocking(app: &AppHandle, asset: &Asset) -> Result<(), String> {
    match install_kind() {
        InstallKind::Deb => {
            // /tmp: readable for apt's download user, so apt does not warn.
            let file = download(app, asset, &std::env::temp_dir().join("schnittwerk-update"))?;
            let _ = app.emit("update-progress", Progress { phase: "install", done: 0, total: 0 });
            // The system's password dialog (polkit) asks for permission.
            let mut cmd = std::process::Command::new("pkexec");
            cmd.args(["apt-get", "install", "-y", "--allow-downgrades"]).arg(&file);
            let r = run(cmd, "pkexec");
            let _ = std::fs::remove_file(&file);
            r
        }
        InstallKind::Flatpak => {
            // The cache directory has the same path inside and outside the sandbox.
            let file = download(app, asset, &crate::shared_temp_dir().join("update"))?;
            let _ = app.emit("update-progress", Progress { phase: "install", done: 0, total: 0 });
            let mut cmd = crate::host_command("flatpak");
            cmd.args(["install", flatpak_installation(), "-y", "--noninteractive", "--reinstall"]).arg(&file);
            let r = run(cmd, "flatpak");
            let _ = std::fs::remove_file(&file);
            r
        }
        InstallKind::Manual => Err("Diese Installation kann sich nicht selbst aktualisieren.".into()),
    }
}

#[tauri::command]
pub async fn update_install(app: AppHandle, asset: Asset) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || install_blocking(&app, &asset)).await.map_err(|e| e.to_string())?
}

/// Starts the freshly installed version and quits this one.
#[tauri::command]
pub fn update_restart(app: AppHandle) {
    if install_kind() == InstallKind::Flatpak {
        let mut cmd = crate::host_command("flatpak");
        cmd.args(["run", APP_ID]);
        let _ = cmd.spawn();
        app.exit(0);
    } else {
        app.restart();
    }
}

/// Opens a GitHub page in the default browser.
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    if !url.starts_with("https://github.com/") {
        return Err("Nur Links zu GitHub werden geöffnet.".into());
    }
    std::process::Command::new("xdg-open").arg(&url).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_numerically() {
        assert!(parse_version("v0.10.0") > parse_version("0.9.9"));
        assert!(parse_version("0.3.4") > parse_version("0.3.3"));
        assert_eq!(parse_version("v1.2"), (1, 2, 0));
    }

    #[test]
    fn picks_the_package_for_the_installation() {
        let a = |n: &str| GhAsset { name: n.into(), browser_download_url: format!("https://github.com/x/{n}"), size: 1, digest: None };
        let assets = vec![a("Schnittwerk_0.4.0_amd64.deb"), a("Schnittwerk_amd64.deb"), a("Schnittwerk.flatpak")];
        assert_eq!(pick_asset(&assets, "0.4.0", InstallKind::Deb).unwrap().name, "Schnittwerk_0.4.0_amd64.deb");
        assert_eq!(pick_asset(&assets, "0.4.0", InstallKind::Flatpak).unwrap().name, "Schnittwerk.flatpak");
        assert!(pick_asset(&assets, "0.4.0", InstallKind::Manual).is_none());
    }
}
