//! USB connection to Silhouette cutters (Graphtec vendor id 0x0b4d).
//!
//! Bulk OUT endpoint 0x01 carries commands, bulk IN 0x82 carries replies.
//! The status query `ESC ENQ` answers `0` (ready), `1` (moving) or
//! `2` (no media loaded), each followed by ETX.

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::{Duration, Instant};

use rusb::{DeviceHandle, GlobalContext};
use schnittwerk_core::gpgl::{DeviceLimits, Job, ESC, ETX};
use serde::Serialize;

pub const VENDOR_GRAPHTEC: u16 = 0x0b4d;

// ---------------------------------------------------------------- log

/// USB traffic log for diagnosis: `~/.local/state/schnittwerk/plotter.log`.
pub fn log_path() -> std::path::PathBuf {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/state")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("schnittwerk").join("plotter.log")
}

fn log(line: &str) {
    use std::io::Write;
    let path = log_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // Keep the log small: start over once it exceeds 1 MB.
    if std::fs::metadata(&path).map(|m| m.len() > 1_000_000).unwrap_or(false) {
        let _ = std::fs::remove_file(&path);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
        let _ = writeln!(f, "{t:.3} {line}");
    }
}

/// Printable form of raw bytes (ESC/ETX made visible), shortened.
fn show(bytes: &[u8]) -> String {
    let mut s = String::new();
    for &b in bytes.iter().take(160) {
        match b {
            0x03 => s.push_str("<ETX>"),
            0x1b => s.push_str("<ESC>"),
            32..=126 => s.push(b as char),
            _ => s.push_str(&format!("<{b:02x}>")),
        }
    }
    if bytes.len() > 160 {
        s.push_str(&format!("… ({} Bytes)", bytes.len()));
    }
    s
}
const EP_OUT: u8 = 0x01;
const EP_IN: u8 = 0x82;
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
/// Plot data goes out in packets of at most 1 KiB (like Silhouette Studio).
pub const PLOT_CHUNK: usize = 1024;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub product_id: u16,
    pub name: &'static str,
    pub width_mm: f64,
    pub length_mm: f64,
    /// Uses the Cameo 4 command set (Portrait 3/4, Cameo 4/5).
    pub cameo4_line: bool,
}

impl Model {
    pub fn limits(&self) -> DeviceLimits {
        DeviceLimits { width_mm: self.width_mm, length_mm: self.length_mm }
    }
}

pub const MODELS: &[Model] = &[
    Model { product_id: 0x113a, name: "Silhouette Portrait 3", width_mm: 203.0, length_mm: 18290.0, cameo4_line: true },
    Model { product_id: 0x113f, name: "Silhouette Portrait 4", width_mm: 216.0, length_mm: 18290.0, cameo4_line: true },
    Model { product_id: 0x1132, name: "Silhouette Portrait 2", width_mm: 203.0, length_mm: 3000.0, cameo4_line: false },
    Model { product_id: 0x1123, name: "Silhouette Portrait", width_mm: 206.0, length_mm: 3000.0, cameo4_line: false },
    Model { product_id: 0x1137, name: "Silhouette Cameo 4", width_mm: 304.8, length_mm: 3000.0, cameo4_line: true },
    Model { product_id: 0x1140, name: "Silhouette Cameo 5", width_mm: 330.2, length_mm: 3000.0, cameo4_line: true },
];

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("USB-Fehler: {0}")]
    Usb(#[from] rusb::Error),
    #[error("Keine Berechtigung für den Plotter – udev-Regel installieren (siehe README).")]
    Permission,
    #[error("Plotter antwortet nicht ({0}).")]
    Timeout(&'static str),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum State {
    Ready,
    Moving,
    /// No mat / media loaded.
    Unloaded,
    Unknown(String),
}

/// A Graphtec device seen on the bus (may not be openable).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub product_id: u16,
    pub name: String,
    pub supported: bool,
}

/// Lists Silhouette devices without opening them.
pub fn scan() -> Vec<Found> {
    let Ok(list) = rusb::devices() else { return vec![] };
    list.iter()
        .filter_map(|d| {
            let desc = d.device_descriptor().ok()?;
            if desc.vendor_id() != VENDOR_GRAPHTEC {
                return None;
            }
            let model = MODELS.iter().find(|m| m.product_id == desc.product_id());
            Some(Found {
                product_id: desc.product_id(),
                name: model.map(|m| m.name.to_string()).unwrap_or_else(|| format!("Silhouette (0x{:04x})", desc.product_id())),
                supported: model.is_some_and(|m| m.cameo4_line),
            })
        })
        .collect()
}

pub struct Plotter {
    handle: DeviceHandle<GlobalContext>,
    pub model: Model,
}

pub enum Outcome {
    Done,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Registering,
    Cutting,
    /// Starting pass n (0-based) of a multi-pass job.
    Pass(usize),
}

impl Plotter {
    /// Opens the first supported device. `Ok(None)` if none is connected.
    pub fn open() -> Result<Option<Plotter>> {
        for dev in rusb::devices()?.iter() {
            let Ok(desc) = dev.device_descriptor() else { continue };
            if desc.vendor_id() != VENDOR_GRAPHTEC {
                continue;
            }
            let Some(model) = MODELS.iter().find(|m| m.product_id == desc.product_id() && m.cameo4_line) else { continue };
            let handle = match dev.open() {
                Ok(h) => h,
                Err(rusb::Error::Access) => return Err(Error::Permission),
                Err(e) => return Err(e.into()),
            };
            let _ = handle.set_auto_detach_kernel_driver(true);
            match handle.claim_interface(0) {
                Ok(()) => {}
                Err(rusb::Error::Access) => return Err(Error::Permission),
                Err(rusb::Error::Busy) => return Err(Error::Other("Plotter wird von einem anderen Programm benutzt.".into())),
                Err(e) => return Err(e.into()),
            }
            let p = Plotter { handle, model: *model };
            p.drain();
            return Ok(Some(p));
        }
        Ok(None)
    }

    pub fn write(&self, data: &[u8]) -> Result<()> {
        self.write_timeout(data, WRITE_TIMEOUT)
    }

    /// Plot data: the device NAKs while its buffer is full, so a write may
    /// block for as long as the cutter needs to work through the buffer.
    /// A generous timeout avoids retrying a partially transferred packet.
    fn write_plot(&self, data: &[u8]) -> Result<()> {
        log(&format!("TX plot {} Bytes", data.len()));
        let mut off = 0;
        while off < data.len() {
            match self.handle.write_bulk(EP_OUT, &data[off..], Duration::from_secs(300)) {
                Ok(n) if n > 0 => off += n,
                Ok(_) => return Err(Error::Other("Plotter nimmt keine Daten an.".into())),
                Err(rusb::Error::Timeout) => return Err(Error::Timeout("Schneiddaten senden")),
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }

    fn write_timeout(&self, data: &[u8], timeout: Duration) -> Result<()> {
        log(&format!("TX {}", show(data)));
        let mut off = 0;
        let mut retries = 0;
        while off < data.len() {
            let end = (off + 4096).min(data.len());
            match self.handle.write_bulk(EP_OUT, &data[off..end], timeout) {
                Ok(0) | Err(rusb::Error::Timeout) if retries < 5 => {
                    retries += 1;
                    sleep(Duration::from_secs(1));
                }
                Ok(n) => {
                    off += n;
                    retries = 0;
                }
                Err(rusb::Error::Timeout) => return Err(Error::Timeout("Schreiben")),
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }

    pub fn send(&self, cmds: &[String]) -> Result<()> {
        self.write(&Job::encode(cmds))
    }

    /// Reads one reply (up to ETX or what arrives in one transfer).
    pub fn read(&self, timeout: Duration) -> Result<Vec<u8>> {
        let mut buf = [0u8; 64];
        match self.handle.read_bulk(EP_IN, &mut buf, timeout) {
            Ok(n) => {
                log(&format!("RX {}", show(&buf[..n])));
                Ok(buf[..n].to_vec())
            }
            Err(rusb::Error::Timeout) => Err(Error::Timeout("Lesen")),
            Err(e) => Err(e.into()),
        }
    }

    /// Discards pending replies.
    fn drain(&self) {
        let mut buf = [0u8; 64];
        while let Ok(n) = self.handle.read_bulk(EP_IN, &mut buf, Duration::from_millis(10)) {
            if n == 0 {
                break;
            }
        }
    }

    /// Sends a query command and returns the reply without ETX.
    pub fn query(&self, cmd: &str, timeout: Duration) -> Result<String> {
        self.drain();
        self.send(&[cmd.to_string()])?;
        let r = self.read(timeout)?;
        Ok(String::from_utf8_lossy(r.strip_suffix(&[ETX]).unwrap_or(&r)).trim().to_string())
    }

    pub fn status(&self) -> Result<State> {
        self.drain();
        self.write(&[ESC, 0x05])?;
        let r = self.read(Duration::from_secs(5))?;
        let body = r.strip_suffix(&[ETX]).unwrap_or(&r);
        Ok(match body {
            b"0" => State::Ready,
            b"1" => State::Moving,
            b"2" => State::Unloaded,
            other => State::Unknown(String::from_utf8_lossy(other).into_owned()),
        })
    }

    pub fn firmware(&self) -> Option<String> {
        self.query("FG", Duration::from_secs(5)).ok()
    }

    /// Initializes the device the way Silhouette Studio does for this family.
    pub fn initialize(&self) -> Result<()> {
        self.write(&[ESC, 0x04])?;
        let _ = self.firmware();
        // Sensor/roller calibration queries; replies are not needed.
        let _ = self.query("TB71", Duration::from_secs(1));
        let _ = self.query("FA", Duration::from_secs(1));
        Ok(())
    }

    /// Polls until ready. `Unloaded` is an error unless `allow_unloaded`.
    pub fn wait_ready(&self, timeout: Duration, cancel: &AtomicBool) -> Result<State> {
        let start = Instant::now();
        loop {
            let st = self.status()?;
            match st {
                State::Ready => return Ok(st),
                State::Unloaded => return Err(Error::Other("Keine Matte geladen – bitte Matte einlegen und laden.".into())),
                _ => {}
            }
            if cancel.load(Ordering::Relaxed) || start.elapsed() > timeout {
                return Ok(st);
            }
            sleep(Duration::from_millis(200));
        }
    }

    /// Waits for the result of a registration-mark search. The device may
    /// send several messages (e.g. `    0,    0`, `    0`, `    1`); a
    /// padded `0` on its own means the marks were found.
    fn await_registration(&self, cancel: &AtomicBool) -> Result<()> {
        log("waiting for registration result");
        let deadline = Instant::now() + Duration::from_secs(90);
        let mut buf: Vec<u8> = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        while Instant::now() < deadline {
            if cancel.load(Ordering::Relaxed) {
                return Err(Error::Other("Abgebrochen.".into()));
            }
            match self.read(Duration::from_secs(2)) {
                Ok(bytes) => {
                    buf.extend_from_slice(&bytes);
                    while let Some(pos) = buf.iter().position(|b| *b == ETX) {
                        let msg: Vec<u8> = buf.drain(..=pos).collect();
                        let text = String::from_utf8_lossy(&msg[..msg.len() - 1]).to_string();
                        seen.push(text.clone());
                        // Padded ("    0"), not a bare status digit.
                        if text.trim() == "0" && text.len() > 1 {
                            log("registration marks found");
                            // Swallow trailing status messages.
                            sleep(Duration::from_millis(300));
                            self.drain();
                            return Ok(());
                        }
                    }
                }
                Err(Error::Timeout(_)) => {}
                Err(e) => return Err(e),
            }
        }
        let got = if seen.is_empty() { "keine".to_string() } else { seen.iter().map(|s| format!("„{s}“")).collect::<Vec<_>>().join(", ") };
        Err(Error::Other(format!(
            "Passermarken nicht gefunden (Antworten des Plotters: {got}). Protokoll: {}",
            log_path().display()
        )))
    }

    /// Sends a whole job. Plot data goes out in ≤1 KiB packets; after each
    /// packet the device is polled until ready, which also paces progress.
    pub fn run_job(
        &self,
        job: &Job,
        cancel: &AtomicBool,
        pause: &AtomicBool,
        mut progress: impl FnMut(usize, usize),
        mut on_phase: impl FnMut(Phase),
    ) -> Result<Outcome> {
        let never = AtomicBool::new(false);
        let st = self.status()?;
        if st == State::Unloaded {
            return Err(Error::Other("Keine Matte geladen – bitte Matte einlegen und am Gerät laden.".into()));
        }
        log(&format!(
            "=== job: {} setup, {} passes, {} plot commands, registration: {} ===",
            job.setup.len(),
            job.passes.len(),
            job.plot_len(),
            job.registration.is_some()
        ));
        self.initialize()?;
        self.send(&job.setup)?;
        if let Some(search) = &job.registration {
            on_phase(Phase::Registering);
            self.drain();
            self.send(&[search.clone()])?;
            if let Err(e) = self.await_registration(cancel) {
                let _ = self.send(&job.trailer);
                return Err(e);
            }
        }
        on_phase(Phase::Cutting);
        let pass_chunks: Vec<Vec<Vec<u8>>> = job.passes.iter().map(|p| Job::chunks(&p.plot, PLOT_CHUNK)).collect();
        let total: usize = pass_chunks.iter().map(|c| c.len()).sum();
        let mut done = 0;
        progress(0, total);
        for (pi, (pass, chunks)) in job.passes.iter().zip(&pass_chunks).enumerate() {
            if pi > 0 {
                // Let the previous pass finish before the tool (e.g. AutoBlade
                // depth) is changed.
                self.wait_ready(Duration::from_secs(3600), &never)?;
                on_phase(Phase::Pass(pi));
            }
            self.send(&pass.tool)?;
            // Stream packets back to back: USB flow control keeps the device
            // buffer full, so the cutter never runs dry between packets.
            // Pause/cancel take effect once the device has worked through
            // what is already buffered.
            for chunk in chunks {
                if pause.load(Ordering::Relaxed) {
                    while pause.load(Ordering::Relaxed) && !cancel.load(Ordering::Relaxed) {
                        sleep(Duration::from_millis(100));
                    }
                }
                if cancel.load(Ordering::Relaxed) {
                    self.wait_ready(Duration::from_secs(600), &never)?;
                    self.send(&job.trailer)?;
                    return Ok(Outcome::Cancelled);
                }
                self.write_plot(chunk)?;
                done += 1;
                progress(done, total);
            }
        }
        // All data is in the device; wait until it has finished cutting.
        sleep(Duration::from_millis(200));
        self.wait_ready(Duration::from_secs(3600), &never)?;
        self.send(&job.trailer)?;
        Ok(Outcome::Done)
    }
}
