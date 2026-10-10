//! The host file of `att lsusb` on Linux: the directories of
//! /sys/bus/usb/devices.

use std::path::Path;

use anyhow::{Context, Result};

use super::parse::SysfsDevice;

/// The files that parse.rs reads. A file that a device does not have is left
/// out.
const FILES: [&str; 9] = [
    "busnum",
    "devnum",
    "idVendor",
    "idProduct",
    "manufacturer",
    "product",
    "serial",
    "speed",
    "bDeviceClass",
];

pub fn read() -> Result<Vec<SysfsDevice>> {
    let root = Path::new("/sys/bus/usb/devices");
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        // A host with no USB controller has no directory.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e).context("could not read /sys/bus/usb/devices"),
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        let mut device = SysfsDevice::new();
        for name in FILES {
            if let Ok(value) = std::fs::read_to_string(dir.join(name)) {
                device.insert(name.to_owned(), value);
            }
        }
        out.push(device);
    }
    Ok(out)
}
