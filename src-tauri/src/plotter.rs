//! Plotter commands: status polling, cut preview, sending jobs in a
//! background thread with progress events, pause and cancel.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use schnittwerk_core::cut::{self, CutPlan, PathInput, PathOptions};
use schnittwerk_core::gpgl::{self, CutSettings, RegMarks};
use schnittwerk_plotter::{Outcome, Plotter, State};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State as TauriState};

#[derive(Default)]
pub struct PlotterManager {
    conn: Mutex<Option<Arc<Plotter>>>,
    busy: AtomicBool,
    cancel: Arc<AtomicBool>,
    pause: Arc<AtomicBool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusInfo {
    connected: bool,
    model: Option<String>,
    /// "USB" | "Bluetooth"
    connection: Option<String>,
    /// ready | moving | unloaded | cutting | unknown | disconnected | error
    state: String,
    message: Option<String>,
    width_mm: Option<f64>,
}

impl StatusInfo {
    fn disconnected(message: Option<String>) -> Self {
        Self { connected: false, model: None, connection: None, state: if message.is_some() { "error" } else { "disconnected" }.into(), message, width_mm: None }
    }
}

/// A poisoned lock must not take the whole plotter connection down.
fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Plotter I/O blocks (and Bluetooth runs its own async runtime, which must
/// not be driven from inside Tauri's), so it always runs on a worker thread.
#[tauri::command]
pub async fn plotter_status(app: AppHandle) -> Result<StatusInfo, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mgr = tauri::Manager::state::<PlotterManager>(&app);
        status_blocking(&mgr)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn status_blocking(mgr: &PlotterManager) -> Result<StatusInfo, String> {
    if mgr.busy.load(Ordering::Relaxed) {
        let conn = lock(&mgr.conn);
        let model = conn.as_ref().map(|p| p.model);
        return Ok(StatusInfo {
            connected: true,
            model: model.map(|m| m.name.to_string()),
            connection: conn.as_ref().map(|p| p.connection().to_string()),
            state: "cutting".into(),
            message: None,
            width_mm: model.map(|m| m.width_mm),
        });
    }
    let mut conn = lock(&mgr.conn);
    if conn.is_none() {
        match Plotter::open() {
            Ok(Some(p)) => *conn = Some(Arc::new(p)),
            Ok(None) => {
                // A Silhouette that we do not support yet?
                let other = schnittwerk_plotter::scan().into_iter().find(|f| !f.supported);
                return Ok(StatusInfo::disconnected(other.map(|f| format!("{} wird noch nicht unterstützt.", f.name))));
            }
            Err(e) => return Ok(StatusInfo::disconnected(Some(e.to_string()))),
        }
    }
    let p = conn.as_ref().unwrap().clone();
    match p.status() {
        Ok(st) => Ok(StatusInfo {
            connected: true,
            model: Some(p.model.name.to_string()),
            connection: Some(p.connection().to_string()),
            state: match st {
                State::Ready => "ready".into(),
                State::Moving => "moving".into(),
                State::Unloaded => "unloaded".into(),
                State::Unknown(_) => "unknown".into(),
            },
            message: None,
            width_mm: Some(p.model.width_mm),
        }),
        Err(e) if p.connection() == "Bluetooth" && p.is_alive() => {
            // Bluetooth link is up but the cutter did not answer the status
            // query: keep the connection and report an unknown state.
            Ok(StatusInfo {
                connected: true,
                model: Some(p.model.name.to_string()),
                connection: Some("Bluetooth".into()),
                state: "unknown".into(),
                message: Some(format!("Status nicht lesbar: {e}")),
                width_mm: Some(p.model.width_mm),
            })
        }
        Err(_) => {
            // Unplugged, switched off or Bluetooth link lost: forget the
            // handle, reconnect next time.
            *conn = None;
            Ok(StatusInfo::disconnected(None))
        }
    }
}

/// Scans for cutters via Bluetooth LE (takes a few seconds).
#[tauri::command]
pub async fn ble_scan() -> Result<Vec<schnittwerk_plotter::ble::BleDevice>, String> {
    tauri::async_runtime::spawn_blocking(|| schnittwerk_plotter::ble::scan(5).map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

/// Connects to a cutter found by `ble_scan`; replaces any USB connection.
#[tauri::command]
pub async fn ble_connect(id: String, app: AppHandle) -> Result<String, String> {
    if tauri::Manager::state::<PlotterManager>(&app).busy.load(Ordering::Relaxed) {
        return Err("Es läuft gerade ein Schnitt.".into());
    }
    tauri::async_runtime::spawn_blocking(move || -> Result<String, String> {
        let (transport, name) = schnittwerk_plotter::ble::BleTransport::connect(&id).map_err(|e| e.to_string())?;
        let mut model = schnittwerk_plotter::ble::model_for(&name);
        let p = Plotter::with_transport(Box::new(transport), model.unwrap_or(schnittwerk_plotter::MODELS[0]));
        if model.is_none() {
            // Advertised name unknown: ask the firmware ("Portrait 3 V1.05").
            model = p.firmware().as_deref().and_then(schnittwerk_plotter::ble::model_for);
        }
        let plotter = match model {
            Some(m) if m.cameo4_line => Plotter::with_model(p, m),
            Some(m) => return Err(format!("{} wird noch nicht unterstützt.", m.name)),
            None => p,
        };
        let name = plotter.model.name.to_string();
        // Replacing (and dropping) a previous connection happens here, on the
        // worker thread, too.
        let mgr = tauri::Manager::state::<PlotterManager>(&app);
        *lock(&mgr.conn) = Some(Arc::new(plotter));
        Ok(name)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn plotter_disconnect(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mgr = tauri::Manager::state::<PlotterManager>(&app);
        if mgr.busy.load(Ordering::Relaxed) {
            return Err("Es läuft gerade ein Schnitt.".into());
        }
        let old = lock(&mgr.conn).take();
        drop(old);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CutGroup {
    paths: Vec<PathInput>,
    options: PathOptions,
    settings: CutSettings,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CutRequest {
    /// Cut passes in order (e.g. kiss-cut first, through-cut last).
    groups: Vec<CutGroup>,
    /// Print & cut: registration marks on the page.
    #[serde(default)]
    regmarks: Option<RegMarks>,
    /// Roll media: push the media out to this position (mm) at the end.
    #[serde(default)]
    feed: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewGroup {
    paths: Vec<PathInput>,
    options: PathOptions,
}

/// One plan per group, in the order given.
#[tauri::command]
pub async fn cut_preview(groups: Vec<PreviewGroup>) -> Result<Vec<CutPlan>, String> {
    tauri::async_runtime::spawn_blocking(move || groups.iter().map(|g| cut::plan(&g.paths, &g.options).map(|(p, _)| p)).collect())
        .await
        .map_err(|e| e.to_string())?
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    done: usize,
    total: usize,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Finished {
    /// done | cancelled | error
    status: String,
    message: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Started {
    /// Points that were outside the cuttable area and got clamped.
    clamped: usize,
}

fn start_job(
    app: AppHandle,
    mgr: &PlotterManager,
    groups: Vec<(Vec<Vec<kurbo::Point>>, CutSettings)>,
    reg: Option<RegMarks>,
    feed: Option<f64>,
) -> Result<Started, String> {
    if groups.iter().all(|g| g.0.is_empty()) && reg.is_none() {
        return Err("Keine Schnittlinien vorhanden.".into());
    }
    let plotter = {
        let mut conn = lock(&mgr.conn);
        if conn.is_none() {
            *conn = Plotter::open().map_err(|e| e.to_string())?.map(Arc::new);
        }
        conn.clone().ok_or("Kein Plotter verbunden.")?
    };
    if mgr.busy.swap(true, Ordering::SeqCst) {
        return Err("Es läuft bereits ein Schnitt.".into());
    }
    let (mut job, clamped) = gpgl::build_job(&groups, plotter.model.limits(), reg);
    if feed.is_some() {
        job.trailer = gpgl::trailer_commands_feed(feed);
    }
    mgr.cancel.store(false, Ordering::SeqCst);
    mgr.pause.store(false, Ordering::SeqCst);
    let (cancel, pause) = (mgr.cancel.clone(), mgr.pause.clone());

    std::thread::spawn(move || {
        let app2 = app.clone();
        let app3 = app.clone();
        let result = plotter.run_job(
            &job,
            &cancel,
            &pause,
            |done, total| {
                let _ = app2.emit("cut-progress", Progress { done, total });
            },
            |phase| {
                let _ = app3.emit("cut-phase", phase);
            },
        );
        let fin = match result {
            Ok(Outcome::Done) => Finished { status: "done".into(), message: None },
            Ok(Outcome::Cancelled) => Finished { status: "cancelled".into(), message: None },
            Err(e) => Finished { status: "error".into(), message: Some(e.to_string()) },
        };
        let mgr = tauri::Manager::state::<PlotterManager>(&app);
        if fin.status == "error" {
            *lock(&mgr.conn) = None;
        }
        mgr.busy.store(false, Ordering::SeqCst);
        let _ = app.emit("cut-finished", fin);
    });
    Ok(Started { clamped })
}

#[tauri::command]
pub async fn cut_start(app: AppHandle, req: CutRequest, mgr: TauriState<'_, PlotterManager>) -> Result<Started, String> {
    let mut groups = Vec::new();
    for g in req.groups {
        let (_, strokes) = cut::plan(&g.paths, &g.options)?;
        if !strokes.is_empty() {
            groups.push((strokes, g.settings));
        }
    }
    start_job(app, &mgr, groups, req.regmarks, req.feed)
}

/// Cuts the small test pattern (10 mm square + triangle) at (x, y) mm.
#[tauri::command]
pub async fn cut_test(app: AppHandle, x: f64, y: f64, settings: CutSettings, mgr: TauriState<'_, PlotterManager>) -> Result<Started, String> {
    start_job(app, &mgr, vec![(gpgl::test_pattern(x, y), settings)], None, None)
}

/// Only searches the registration marks (no cutting) – for diagnosis.
#[tauri::command]
pub async fn cut_regtest(app: AppHandle, regmarks: RegMarks, settings: CutSettings, mgr: TauriState<'_, PlotterManager>) -> Result<Started, String> {
    start_job(app, &mgr, vec![(vec![], settings)], Some(regmarks), None)
}

#[tauri::command]
pub fn cut_pause(paused: bool, mgr: TauriState<'_, PlotterManager>) {
    mgr.pause.store(paused, Ordering::SeqCst);
}

#[tauri::command]
pub fn cut_cancel(mgr: TauriState<'_, PlotterManager>) {
    mgr.cancel.store(true, Ordering::SeqCst);
}
