//! Byte transports to the cutter. The GPGL protocol is the same over USB
//! and Bluetooth LE; only how bytes travel differs.

use std::time::Duration;

use rusb::{DeviceHandle, GlobalContext};

use crate::{Error, Result};

pub trait Transport: Send + Sync {
    /// Sends all bytes; may block while the device buffer is full.
    fn write(&self, data: &[u8], timeout: Duration) -> Result<()>;
    /// Returns the next chunk of reply bytes (usually one ETX-terminated message).
    fn read(&self, timeout: Duration) -> Result<Vec<u8>>;
    /// Reply to a status query (ESC ENQ). Over Bluetooth this may arrive on
    /// a different channel than other replies.
    fn read_status(&self, timeout: Duration) -> Result<Vec<u8>> {
        self.read(timeout)
    }
    /// False once the connection is known to be gone.
    fn is_alive(&self) -> bool {
        true
    }
    /// A job is running: ride out short connection dropouts (retry,
    /// reconnect) instead of failing at the first error.
    fn set_job_mode(&self, _on: bool) {}
    /// Discards pending replies.
    fn drain(&self);
    /// "USB" or "Bluetooth".
    fn kind(&self) -> &'static str;
}

const EP_OUT: u8 = 0x01;
const EP_IN: u8 = 0x82;

pub struct UsbTransport {
    pub(crate) handle: DeviceHandle<GlobalContext>,
}

impl Transport for UsbTransport {
    fn write(&self, data: &[u8], timeout: Duration) -> Result<()> {
        let mut off = 0;
        let mut zero_writes = 0;
        while off < data.len() {
            match self.handle.write_bulk(EP_OUT, &data[off..], timeout) {
                Ok(0) if zero_writes < 5 => {
                    zero_writes += 1;
                    std::thread::sleep(Duration::from_millis(200));
                }
                Ok(0) => return Err(Error::Other("Plotter nimmt keine Daten an.".into())),
                Ok(n) => off += n,
                // No blind retry: part of the packet may already be transferred.
                Err(rusb::Error::Timeout) => return Err(Error::Timeout("Schreiben")),
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }

    fn read(&self, timeout: Duration) -> Result<Vec<u8>> {
        let mut buf = [0u8; 64];
        match self.handle.read_bulk(EP_IN, &mut buf, timeout) {
            Ok(n) => Ok(buf[..n].to_vec()),
            Err(rusb::Error::Timeout) => Err(Error::Timeout("Lesen")),
            Err(e) => Err(e.into()),
        }
    }

    fn drain(&self) {
        let mut buf = [0u8; 64];
        while let Ok(n) = self.handle.read_bulk(EP_IN, &mut buf, Duration::from_millis(10)) {
            if n == 0 {
                break;
            }
        }
    }

    fn kind(&self) -> &'static str {
        "USB"
    }
}
