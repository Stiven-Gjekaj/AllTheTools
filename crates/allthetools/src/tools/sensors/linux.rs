//! The host file of `att sensors` on Linux: the directories of
//! /sys/class/hwmon, which lm-sensors reads too.

use std::path::Path;

use anyhow::{Context, Result};

use super::parse::{self, Hwmon, Reading};

pub fn read() -> Result<Vec<Reading>> {
    let root = Path::new("/sys/class/hwmon");
    let mut chips = Vec::new();
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).context("could not read /sys/class/hwmon"),
    };
    for entry in entries.flatten() {
        let mut files = Hwmon::new();
        let Ok(inside) = std::fs::read_dir(entry.path()) else {
            continue;
        };
        for file in inside.flatten() {
            let name = file.file_name().to_string_lossy().into_owned();
            let wanted = name == "name"
                || ["_input", "_label", "_max", "_crit"]
                    .iter()
                    .any(|suffix| name.ends_with(suffix));
            if !wanted {
                continue;
            }
            // A sensor that the chip cannot read gives an error on read, and
            // it is left out.
            if let Ok(value) = std::fs::read_to_string(file.path()) {
                files.insert(name, value);
            }
        }
        chips.push(files);
    }
    Ok(parse::linux(&chips))
}
