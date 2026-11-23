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
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use btleplug::api::{Central, Characteristic, Manager as _, Peripheral as _, ScanFilter, WriteType};
use btleplug::platform::{Adapter, Manager, Peripheral};
use futures::StreamExt;
use serde::Serialize;
use tokio::runtime::Runtime;
use uuid::Uuid;

use crate::transport::Transport;
use crate::{Error, Model, Result, ETX, MODELS};

const SERVICE: Uuid = Uuid::from_u128(0xe2088282_4fde_42f9_bb22_6ec3c7ed8f91);
const WRITE: Uuid = Uuid::from_u128(0x6d92661d_f429_4d67_929b_28e7a9780912);
const READ: Uuid = Uuid::from_u128(0x8dcf199a_30e7_4bd4_beb6_beb57dca866c);
const CONTROL: Uuid = Uuid::from_u128(0x61490654_b5b4_458c_a867_9e15bc1471e0);
const INIT: [u8; 2] = [0x1b, 0x04];
const CHUNK: usize = 20;

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

pub struct BleTransport {
    rt: Runtime,
    peripheral: Peripheral,
    write_char: Characteristic,
    inbox: Inbox,
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
            peripheral.connect().await.map_err(ble_err)?;
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
            tokio::spawn(async move {
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
            for ch in [&c, &r, &w] {
                peripheral.write(ch, &INIT, WriteType::WithResponse).await.map_err(ble_err)?;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
            Ok::<_, Error>((peripheral, w, name))
        })?;
        {
            let mut b = inbox.0.lock().unwrap();
            b.read.clear();
            b.control.clear();
        }
        Ok((BleTransport { rt, peripheral, write_char, inbox }, name))
    }
}

impl Transport for BleTransport {
    fn write(&self, data: &[u8], timeout: Duration) -> Result<()> {
        let deadline = Instant::now() + timeout;
        for chunk in data.chunks(CHUNK) {
            let mut attempt = 0;
            loop {
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    return Err(Error::Timeout("Bluetooth senden"));
                }
                let r = self.rt.block_on(async {
                    tokio::time::timeout(left, self.peripheral.write(&self.write_char, chunk, WriteType::WithResponse)).await
                });
                match r {
                    Ok(Ok(())) => break,
                    // The cutter answers "busy" while its buffer is full: back off and retry.
                    Ok(Err(e)) if attempt < 8 => {
                        let _ = e;
                        attempt += 1;
                        std::thread::sleep(Duration::from_millis(100 * (1 << attempt.min(4))));
                    }
                    Ok(Err(e)) => return Err(ble_err(e)),
                    Err(_) => return Err(Error::Timeout("Bluetooth senden")),
                }
            }
        }
        Ok(())
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
        self.rt.block_on(self.peripheral.is_connected()).unwrap_or(false)
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
        let _ = self.rt.block_on(self.peripheral.disconnect());
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
}
