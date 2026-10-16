//! The host file of `att edid` on Linux: the `edid` file of each connector
//! in /sys/class/drm.

use std::path::Path;

use anyhow::Result;

pub fn read() -> Result<Vec<(Option<String>, Vec<u8>)>> {
    let root = Path::new("/sys/class/drm");
    let Ok(entries) = std::fs::read_dir(root) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(edid) = std::fs::read(path.join("edid")) else {
            continue;
        };
        if edid.is_empty() {
            continue;
        }
        // A connector is named card0-HDMI-A-1. The port is the part after
        // the card.
        let name = entry.file_name().to_string_lossy().into_owned();
        let port = name
            .split_once('-')
            .map(|(_, p)| p.to_owned())
            .unwrap_or(name);
        out.push((Some(port), edid));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}
