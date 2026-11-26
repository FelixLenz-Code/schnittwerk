//! Updates from GitHub releases: check for a newer version, download the
//! matching package (.deb or Flatpak), verify it and install it.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};

const REPO: &str = "FelixLenz-Code/schnittwerk";
const APP_ID: &str = "de.schnittwerk.Schnittwerk";

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

/// Only packages of Schnittwerk's own releases are installed.
fn valid_asset(asset: &Asset) -> bool {
    let prefix = format!("https://github.com/{REPO}/releases/download/");
    !asset.name.is_empty()
        && !asset.name.contains('/')
        && !asset.name.contains("..")
        && asset.url.starts_with(&prefix)
        && asset.digest.as_deref().is_some_and(|d| d.starts_with("sha256:"))
}

/// Downloads the asset into `dir` (fresh, created by this process) and
/// checks its SHA-256.
fn download(app: &AppHandle, asset: &Asset, dir: &Path) -> Result<PathBuf, String> {
    if !valid_asset(asset) {
        return Err("Ungültige Datei im Release.".into());
    }
    let path = dir.join(&asset.name);
    let resp = agent().get(&asset.url).call().map_err(|e| format!("Download fehlgeschlagen: {e}"))?;
    let total = resp.header("Content-Length").and_then(|v| v.parse().ok()).unwrap_or(asset.size);
    let mut reader = resp.into_reader();
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path).map_err(|e| e.to_string())?;
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

fn install_blocking(app: &AppHandle, requested: &Asset) -> Result<(), String> {
    // The package is looked up again here instead of trusting the UI: only
    // the asset of the latest release that fits this installation is used.
    let info = check_blocking()?;
    let asset = info.asset.filter(|a| a.name == requested.name).ok_or("Ungültige Datei im Release.")?;
    let (dir, mode) = match install_kind() {
        // Readable for apt's download user (_apt), so apt does not warn;
        // only this user can change the directory.
        InstallKind::Deb => ("schnittwerk-update", 0o755),
        // The cache directory has the same path inside and outside the sandbox.
        InstallKind::Flatpak => ("schnittwerk-update", 0o700),
        InstallKind::Manual => return Err("Diese Installation kann sich nicht selbst aktualisieren.".into()),
    };
    let dir = crate::private_temp_dir(dir, mode)?;
    let result = download(app, &asset, &dir).and_then(|file| {
        let _ = app.emit("update-progress", Progress { phase: "install", done: 0, total: 0 });
        if install_kind() == InstallKind::Deb {
            // The system's password dialog (polkit) asks for permission.
            let mut cmd = std::process::Command::new("pkexec");
            cmd.args(["apt-get", "install", "-y", "--allow-downgrades"]).arg(&file);
            run(cmd, "pkexec")
        } else {
            let mut cmd = crate::host_command("flatpak");
            cmd.args(["install", flatpak_installation(), "-y", "--noninteractive", "--reinstall"]).arg(&file);
            run(cmd, "flatpak")
        }
    });
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[tauri::command]
pub async fn update_install(app: AppHandle, asset: Asset) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || install_blocking(&app, &asset)).await.map_err(|e| e.to_string())?
}

/// Starts the freshly installed version and quits this one.
#[tauri::command]
pub fn update_restart(app: AppHandle) {
    if install_kind() == InstallKind::Flatpak {
        // Detached on the host: flatpak-spawn would otherwise wait for the new
        // instance and keep this sandbox alive until it is closed.
        let mut cmd = crate::host_command("sh");
        cmd.args(["-c", &format!("setsid flatpak run {APP_ID} >/dev/null 2>&1 &")]);
        let _ = cmd.status();
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

    #[test]
    fn only_own_release_assets_with_checksum_are_valid() {
        let ok = Asset {
            name: "Schnittwerk_0.7.0_amd64.deb".into(),
            url: format!("https://github.com/{REPO}/releases/download/v0.7.0/Schnittwerk_0.7.0_amd64.deb"),
            size: 1,
            digest: Some("sha256:00".into()),
        };
        assert!(valid_asset(&ok));
        assert!(!valid_asset(&Asset { url: "https://github.com/someone/else/releases/download/x.deb".into(), ..ok.clone() }));
        assert!(!valid_asset(&Asset { digest: None, ..ok.clone() }));
        assert!(!valid_asset(&Asset { name: "../x.deb".into(), ..ok.clone() }));
    }
}
