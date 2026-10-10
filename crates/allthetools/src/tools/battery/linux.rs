//! The host file of `att battery` on Linux: the directories of
//! /sys/class/power_supply.

use std::path::Path;

use anyhow::{Context, Result};

use super::parse::SysfsSupply;

const FILES: [&str; 16] = [
    "type",
    "scope",
    "status",
    "capacity",
    "energy_full",
    "energy_full_design",
    "charge_full",
    "charge_full_design",
    "cycle_count",
    "voltage_now",
    "temp",
    "manufacturer",
    "model_name",
    "serial_number",
    "technology",
    "present",
];

pub fn read() -> Result<Vec<SysfsSupply>> {
    let root = Path::new("/sys/class/power_supply");
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e).context("could not read /sys/class/power_supply"),
    };
    for entry in entries.flatten() {
        let mut supply = SysfsSupply::new();
        for name in FILES {
            if let Ok(value) = std::fs::read_to_string(entry.path().join(name)) {
                supply.insert(name.to_owned(), value);
            }
        }
        out.push(supply);
    }
    Ok(out)
}
