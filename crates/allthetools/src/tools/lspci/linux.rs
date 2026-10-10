//! The host file of `att lspci` on Linux: the directories of
//! /sys/bus/pci/devices, and the names in pci.ids when the host has it.

use std::path::Path;

use anyhow::{Context, Result};

use super::parse::SysfsDevice;

const FILES: [&str; 4] = ["vendor", "device", "class", "revision"];

/// The places where a distribution puts pci.ids.
const PCI_IDS: [&str; 3] = [
    "/usr/share/hwdata/pci.ids",
    "/usr/share/misc/pci.ids",
    "/usr/share/pci.ids",
];

pub fn read() -> Result<Vec<SysfsDevice>> {
    let root = Path::new("/sys/bus/pci/devices");
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        // A host with no PCI bus, such as a small board, has no directory.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e).context("could not read /sys/bus/pci/devices"),
    };
    for entry in entries.flatten() {
        let mut device = SysfsDevice::new();
        device.insert(
            "slot".to_owned(),
            entry.file_name().to_string_lossy().into_owned(),
        );
        for name in FILES {
            if let Ok(value) = std::fs::read_to_string(entry.path().join(name)) {
                device.insert(name.to_owned(), value);
            }
        }
        out.push(device);
    }
    Ok(out)
}

/// The text of pci.ids, or `None` when the host has no copy.
pub fn pci_ids() -> Option<String> {
    PCI_IDS
        .iter()
        .find_map(|path| std::fs::read_to_string(path).ok())
}
