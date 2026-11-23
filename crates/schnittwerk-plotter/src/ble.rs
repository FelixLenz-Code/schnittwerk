//! Bluetooth LE connection to Silhouette cutters (Portrait 3/4, Cameo 4/5).
//!
//! The cutter exposes a vendor GATT service carrying the same GPGL byte
//! stream as USB: commands go to WRITE in acknowledged 20-byte chunks,
//! replies arrive as notifications on READ. CONTROL only reports movement
//! events and must not be mixed into replies (it would e.g. turn the initial
//! "moving" event into a false registration result). Service/characteristic
//! roles and the handshake follow inkscape-silhouette's BLETransport, which
//! cross-checked them against bob-takuya/cameo-cut.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use btleplug::api::{Central, Characteristic, Manager as _, Peripheral as _, ScanFilter, WriteType};
use btleplug::platform::{Adapter, Manager, Peripheral};
use futures::StreamExt;
use serde::Serialize;
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::transport::Transport;
use crate::{Error, Model, Result, ETX, MODELS};

const SERVICE: Uuid = Uuid::from_u128(0xe2088282_4fde_42f9_bb22_6ec3c7ed8f91);
const WRITE: Uuid = Uuid::from_u128(0x6d92661d_f429_4d67_929b_28e7a9780912);
const READ: Uuid = Uuid::from_u128(0x8dcf199a_30e7_4bd4_beb6_beb57dca866c);
const CONTROL: Uuid = Uuid::from_u128(0x61490654_b5b4_458c_a867_9e15bc1471e0);
const INIT: [u8; 2] = [0x1b, 0x04];
const CHUNK: usize = 20;
/// Longest a single acknowledged write may take before it counts as failed.
const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(35);
/// During a job: how long to keep retrying / reconnecting after the first
/// failed write before giving up (radio interference, e.g. a phone call).
const RECOVERY: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BleDevice {
    /// Opaque id to connect with.
    pub id: String,
    pub name: String,
    pub address: String,
    pub rssi: Option<i16>,
}

fn ble_err(e: impl std::fmt::Display) -> Error {
    Error::Other(format!("Bluetooth: {e}"))
}

async fn adapter(manager: &Manager) -> Result<Adapter> {
    manager
        .adapters()
        .await
        .map_err(ble_err)?
        .into_iter()
        .next()
        .ok_or_else(|| Error::Other("Kein Bluetooth-Adapter gefunden.".into()))
}

fn looks_like_cutter(name: &str) -> bool {
    let n = name.to_uppercase();
    n.contains("PORTRAIT") || n.contains("CAMEO")
}

/// Model for an advertised name / firmware string ("PORTRAIT3-…", "Portrait 3 V1.05").
pub fn model_for(name: &str) -> Option<Model> {
    let n: String = name.to_uppercase().chars().filter(|c| !c.is_whitespace()).collect();
    MODELS.iter().copied().find(|m| {
        let key: String = m.name.trim_start_matches("Silhouette ").to_uppercase().chars().filter(|c| !c.is_whitespace()).collect();
        n.contains(&key)
    })
}

/// Scans `seconds` for nearby cutters.
pub fn scan(seconds: u64) -> Result<Vec<BleDevice>> {
    let rt = Runtime::new().map_err(ble_err)?;
    rt.block_on(async {
        let manager = Manager::new().await.map_err(ble_err)?;
        let central = adapter(&manager).await?;
        central.start_scan(ScanFilter::default()).await.map_err(ble_err)?;
        tokio::time::sleep(Duration::from_secs(seconds)).await;
        let _ = central.stop_scan().await;
        let mut found = Vec::new();
        for p in central.peripherals().await.map_err(ble_err)? {
            let Ok(Some(props)) = p.properties().await else { continue };
            let name = props.local_name.clone().unwrap_or_default();
            if looks_like_cutter(&name) || props.services.contains(&SERVICE) {
                found.push(BleDevice { id: p.id().to_string(), name, address: props.address.to_string(), rssi: props.rssi });
            }
        }
        Ok(found)
    })
}

/// Bytes from the READ (replies) and CONTROL (state events) characteristics.
#[derive(Default)]
struct Buffers {
    read: VecDeque<u8>,
    control: VecDeque<u8>,
    /// Last complete CONTROL message, e.g. `0<ETX>` (ready).
    last_control: Option<Vec<u8>>,
}

type Inbox = Arc<(Mutex<Buffers>, Condvar)>;

fn take_message(buf: &mut VecDeque<u8>) -> Option<Vec<u8>> {
    let pos = buf.iter().position(|b| *b == ETX)?;
    Some(buf.drain(..=pos).collect())
}

/// Splits a byte stream into BLE writes of at most `max` bytes that end on
/// command boundaries (ETX) and start a new write at every move command.
///
/// When a write fails, it is unknown whether the cutter already got it, so it
/// is sent again. Packed like this, a write that arrives twice only repeats
/// whole commands, and its first command is either a move (tool up) or a cut
/// back along the stroke that was just cut – never a stray line.
fn pieces(data: &[u8], max: usize) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut cur = 0..0;
    let mut s = 0;
    while s < data.len() {
        let e = data[s..].iter().position(|b| *b == ETX).map_or(data.len(), |i| s + i + 1);
        if !cur.is_empty() && (cur.len() + (e - s) > max || data[s] == b'M') {
            out.push(&data[cur.clone()]);
            cur = s..s;
        }
        if e - s > max {
            // A single command longer than one write (not used for plot data).
            out.extend(data[s..e].chunks(max));
            cur = e..e;
        } else {
            cur.end = e;
        }
        s = e;
    }
    if !cur.is_empty() {
        out.push(&data[cur]);
    }
    out
}

/// Connects (or reconnects) `peripheral`, subscribes to replies and state
/// events and starts feeding them into `inbox`. The init handshake goes to
/// the given characteristics. Returns the WRITE characteristic and the task
/// that forwards notifications.
async fn link(peripheral: &Peripheral, inbox: &Inbox, init_write: bool) -> Result<(Characteristic, JoinHandle<()>)> {
    if !peripheral.is_connected().await.unwrap_or(false) {
        peripheral.connect().await.map_err(ble_err)?;
    }
    peripheral.discover_services().await.map_err(ble_err)?;
    let chars = peripheral.characteristics();
    let find = |u: Uuid| chars.iter().find(|c| c.uuid == u).cloned();
    let (Some(w), Some(r), Some(c)) = (find(WRITE), find(READ), find(CONTROL)) else {
        let _ = peripheral.disconnect().await;
        return Err(Error::Other("Gerät hat nicht den Silhouette-Bluetooth-Dienst.".into()));
    };
    peripheral.subscribe(&c).await.map_err(ble_err)?;
    peripheral.subscribe(&r).await.map_err(ble_err)?;
    // Replies arrive on READ; CONTROL carries state events ("0"/"1"/"2"),
    // kept apart so they never pose as a command reply.
    let mut stream = peripheral.notifications().await.map_err(ble_err)?;
    let sink = inbox.clone();
    let task = tokio::spawn(async move {
        while let Some(n) = stream.next().await {
            let (lock, cv) = &*sink;
            let mut b = lock.lock().unwrap();
            if n.uuid == READ {
                crate::log(&format!("BLE READ {}", crate::show(&n.value)));
                b.read.extend(n.value);
            } else if n.uuid == CONTROL {
                crate::log(&format!("BLE CONTROL {}", crate::show(&n.value)));
                b.control.extend(n.value);
                // Remember the newest complete state message.
                let mut copy = b.control.clone();
                while let Some(m) = take_message(&mut copy) {
                    b.last_control = Some(m);
                }
            }
            cv.notify_all();
        }
    });
    // ESC EOT on WRITE also reaches the cutter's command stream, so a
    // reconnect in the middle of a job leaves it out.
    let init: Vec<&Characteristic> = if init_write { vec![&c, &r, &w] } else { vec![&c, &r] };
    for ch in init {
        if let Err(e) = peripheral.write(ch, &INIT, WriteType::WithResponse).await {
            task.abort();
            return Err(ble_err(e));
        }
    }
    Ok((w, task))
}

pub struct BleTransport {
    /// Own runtime for btleplug; `Option` so `Drop` can hand it to a plain
    /// thread (a runtime must not be dropped inside another one).
    rt: Option<Runtime>,
    peripheral: Peripheral,
    write_char: Characteristic,
    inbox: Inbox,
    /// Task forwarding notifications of the current link.
    notify_task: Mutex<Option<JoinHandle<()>>>,
    /// A job is running: ride out dropouts instead of failing fast.
    job_mode: AtomicBool,
}

impl BleTransport {
    /// Connects to a device found by [`scan`] and runs the init handshake.
    pub fn connect(id: &str) -> Result<(BleTransport, String)> {
        let rt = Runtime::new().map_err(ble_err)?;
        let inbox: Inbox = Arc::new((Mutex::new(Buffers::default()), Condvar::new()));
        let (peripheral, write_char, name) = rt.block_on(async {
            let manager = Manager::new().await.map_err(ble_err)?;
            let central = adapter(&manager).await?;
            // The adapter must have seen the device recently; scan briefly until it shows up.
            central.start_scan(ScanFilter::default()).await.map_err(ble_err)?;
            let deadline = Instant::now() + Duration::from_secs(10);
            let peripheral = loop {
                if let Some(p) = central.peripherals().await.map_err(ble_err)?.into_iter().find(|p| p.id().to_string() == id) {
                    break p;
                }
                if Instant::now() > deadline {
                    let _ = central.stop_scan().await;
                    return Err(Error::Other("Plotter per Bluetooth nicht gefunden – eingeschaltet und in Reichweite?".into()));
                }
                tokio::time::sleep(Duration::from_millis(300)).await;
            };
            let _ = central.stop_scan().await;
            let name = peripheral.properties().await.ok().flatten().and_then(|p| p.local_name).unwrap_or_default();
            let (w, task) = link(&peripheral, &inbox, true).await?;
            tokio::time::sleep(Duration::from_secs(1)).await;
            Ok::<_, Error>((peripheral, (w, task), name))
        })?;
        let (write_char, task) = write_char;
        {
            let mut b = inbox.0.lock().unwrap();
            b.read.clear();
            b.control.clear();
        }
        let t = BleTransport { rt: Some(rt), peripheral, write_char, inbox, notify_task: Mutex::new(Some(task)), job_mode: AtomicBool::new(false) };
        Ok((t, name))
    }

    /// Re-establishes a dropped link to the same cutter (same runtime and
    /// inbox), keeping the cutter's job state.
    fn relink(&self) -> Result<()> {
        if let Some(old) = self.notify_task.lock().unwrap().take() {
            old.abort();
        }
        let (_, task) = self.rt().block_on(async {
            tokio::time::timeout(Duration::from_secs(20), link(&self.peripheral, &self.inbox, false))
                .await
                .unwrap_or(Err(Error::Timeout("Bluetooth neu verbinden")))
        })?;
        *self.notify_task.lock().unwrap() = Some(task);
        Ok(())
    }
}

impl BleTransport {
    fn rt(&self) -> &Runtime {
        self.rt.as_ref().expect("runtime present until drop")
    }
}

impl Transport for BleTransport {
    fn write(&self, data: &[u8], timeout: Duration) -> Result<()> {
        let deadline = Instant::now() + timeout;
        let resilient = self.job_mode.load(Ordering::Relaxed);
        for piece in pieces(data, CHUNK) {
            let mut attempt = 0u32;
            let mut trouble_since: Option<Instant> = None;
            loop {
                let limit = match trouble_since {
                    Some(t) if resilient => deadline.max(t + RECOVERY),
                    _ => deadline,
                };
                let left = limit.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    return Err(Error::Timeout("Bluetooth senden"));
                }
                let r = self.rt().block_on(async {
                    tokio::time::timeout(left.min(ATTEMPT_TIMEOUT), self.peripheral.write(&self.write_char, piece, WriteType::WithResponse)).await
                });
                let err = match r {
                    Ok(Ok(())) => break,
                    Ok(Err(e)) => ble_err(e),
                    Err(_) => Error::Timeout("Bluetooth senden"),
                };
                attempt += 1;
                if !resilient {
                    // Outside a job: the cutter answers "busy" while its buffer
                    // is full; back off a few times, then report the error.
                    if attempt > 8 || matches!(err, Error::Timeout(_)) {
                        return Err(err);
                    }
                    std::thread::sleep(Duration::from_millis(100 * (1 << attempt.min(4))));
                    continue;
                }
                crate::log(&format!("BLE write failed (attempt {attempt}): {err}"));
                trouble_since.get_or_insert_with(Instant::now);
                if self.is_alive() {
                    std::thread::sleep(Duration::from_millis(100 * (1 << attempt.min(4))));
                } else {
                    crate::log("BLE link lost – reconnecting");
                    match self.relink() {
                        Ok(()) => crate::log("BLE reconnected – resending"),
                        Err(e) => {
                            crate::log(&format!("BLE reconnect failed: {e}"));
                            std::thread::sleep(Duration::from_secs(2));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn set_job_mode(&self, on: bool) {
        self.job_mode.store(on, Ordering::Relaxed);
    }

    fn read(&self, timeout: Duration) -> Result<Vec<u8>> {
        let (lock, cv) = &*self.inbox;
        let deadline = Instant::now() + timeout;
        let mut b = lock.lock().unwrap();
        loop {
            if let Some(m) = take_message(&mut b.read) {
                return Ok(m);
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                if b.read.is_empty() {
                    return Err(Error::Timeout("Lesen"));
                }
                return Ok(b.read.drain(..).collect());
            }
            b = cv.wait_timeout(b, left).unwrap().0;
        }
    }

    /// The status reply may come on CONTROL or READ; if neither answers, the
    /// last state event the cutter pushed on CONTROL is used.
    fn read_status(&self, timeout: Duration) -> Result<Vec<u8>> {
        let (lock, cv) = &*self.inbox;
        let deadline = Instant::now() + timeout.min(Duration::from_secs(2));
        let mut b = lock.lock().unwrap();
        loop {
            if let Some(m) = take_message(&mut b.control) {
                return Ok(m);
            }
            if let Some(m) = take_message(&mut b.read) {
                return Ok(m);
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return b.last_control.clone().ok_or(Error::Timeout("Status per Bluetooth"));
            }
            b = cv.wait_timeout(b, left).unwrap().0;
        }
    }

    fn is_alive(&self) -> bool {
        self.rt().block_on(self.peripheral.is_connected()).unwrap_or(false)
    }

    fn drain(&self) {
        let mut b = self.inbox.0.lock().unwrap();
        b.read.clear();
        b.control.clear();
    }

    fn kind(&self) -> &'static str {
        "Bluetooth"
    }
}

impl Drop for BleTransport {
    fn drop(&mut self) {
        // Disconnect and shut the runtime down on a plain thread, so dropping
        // a connection is safe from any context.
        if let Some(rt) = self.rt.take() {
            if let Some(t) = self.notify_task.lock().unwrap().take() {
                t.abort();
            }
            let p = self.peripheral.clone();
            std::thread::spawn(move || {
                let _ = rt.block_on(p.disconnect());
                drop(rt);
            });
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn model_from_advertised_name() {
        assert_eq!(super::model_for("PORTRAIT3-1A2B").map(|m| m.product_id), Some(0x113a));
        assert_eq!(super::model_for("Portrait 3 V1.05").map(|m| m.product_id), Some(0x113a));
        assert!(super::model_for("Kopfhörer").is_none());
    }

    #[test]
    fn pieces_end_on_commands_and_break_before_moves() {
        let data = schnittwerk_core::gpgl::Job::encode(&["M100,200", "D110,200", "D120,210", "D9999,9999", "M5,5", "D6,6"].map(String::from));
        let p = super::pieces(&data, 20);
        assert_eq!(p.concat(), data);
        for w in &p {
            assert!(w.len() <= 20);
            assert_eq!(*w.last().unwrap(), super::ETX);
            // A move only ever opens a write.
            let cmds: Vec<&[u8]> = w.split(|b| *b == super::ETX).filter(|c| !c.is_empty()).collect();
            assert!(cmds.iter().skip(1).all(|c| c[0] != b'M'), "{:?}", String::from_utf8_lossy(w));
        }
        assert_eq!(p[0], b"M100,200\x03D110,200\x03");
        // ESC sequences without ETX stay intact.
        assert_eq!(super::pieces(&[0x1b, 0x05], 20), vec![&[0x1b, 0x05][..]]);
    }
}
