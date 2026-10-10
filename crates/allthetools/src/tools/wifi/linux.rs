//! The host file of `att wifi` on Linux: `nmcli` of NetworkManager, which a
//! desktop distribution ships. `iw` can scan too, and it needs root.

use anyhow::Result;

use super::parse::{self, Network};
use crate::host;

pub fn scan() -> Result<Vec<Network>> {
    let text = host::text(
        "nmcli",
        &[
            "-t",
            "-f",
            "SSID,BSSID,SIGNAL,CHAN,FREQ,SECURITY",
            "device",
            "wifi",
            "list",
            "--rescan",
            "auto",
        ],
        Some("Install the package NetworkManager"),
    )?;
    Ok(parse::nmcli(&text))
}

pub const HIDDEN_NAMES: &str = "";
