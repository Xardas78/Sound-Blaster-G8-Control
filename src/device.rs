//! HID-Vendor-Kanal der G8 (proprietäres Protokoll, siehe protocol.rs).

use hidapi::{HidApi, HidDevice};

const VID: u16 = 0x041e;
const PID: u16 = 0x329b;
/// Interface 4 ist der proprietäre Vendor-Kanal (Interface 3 ist ebenfalls
/// HID, aber unbenutzt) – bestätigt per `lsusb -v`, siehe HANDOFF.md.
const VENDOR_INTERFACE: i32 = 4;

pub struct G8Device {
    handle: HidDevice,
}

impl G8Device {
    pub fn open() -> Result<Self, String> {
        let api = HidApi::new().map_err(|e| e.to_string())?;
        let info = api
            .device_list()
            .find(|d| d.vendor_id() == VID && d.product_id() == PID && d.interface_number() == VENDOR_INTERFACE)
            .ok_or_else(|| "G8 Vendor-HID-Interface nicht gefunden (Gerät angeschlossen?)".to_string())?;
        let handle = info.open_device(&api).map_err(|e| e.to_string())?;
        Ok(Self { handle })
    }

    /// Sendet einen 64-Byte-Report per SET_REPORT (Report-ID 0) an Interface 4.
    ///
    /// hidapi erwartet als erstes Byte immer die Report-ID (hier 0x00, da das
    /// Gerät unnumbered Reports nutzt) – die eigentlichen 64 Nutzdaten-Bytes
    /// aus protocol.rs folgen danach. Interface 4 hat keinen Interrupt-OUT-
    /// Endpoint, der Linux-hidraw-Treiber schickt den Report deshalb
    /// automatisch per Control-Transfer (siehe HANDOFF.md).
    pub fn send_report(&self, report: &[u8; 64]) -> Result<(), String> {
        let mut buf = [0u8; 65];
        buf[1..].copy_from_slice(report);
        self.handle.write(&buf).map(|_| ()).map_err(|e| e.to_string())
    }
}
