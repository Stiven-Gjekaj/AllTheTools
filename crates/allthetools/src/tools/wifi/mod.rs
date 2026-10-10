//! `att wifi`: list the Wi-Fi networks near this computer.
//!
//! macOS removed the `airport` command in version 14.4. Linux has `nmcli`,
//! and Windows has `netsh wlan show networks`, whose words change with the
//! language of Windows. This tool gives one table on each host: the name,
//! the signal, the channel, the band and the security.

use anyhow::Result;

pub mod parse;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as host;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as host;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as host;

/// List the Wi-Fi networks near this computer, the strongest first.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Print JSON for a script, and not a table for a person.
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let mut networks = host::scan()?;
    parse::sort(&mut networks);
    if args.json {
        println!("{}", serde_json::to_string(&networks)?);
        return Ok(0);
    }
    if networks.is_empty() {
        println!("The scan found no Wi-Fi network, or this computer has no Wi-Fi card.");
        return Ok(0);
    }
    let dash = || "-".to_owned();
    let rows: Vec<Vec<String>> = networks
        .iter()
        .map(|n| {
            vec![
                n.ssid.clone().unwrap_or_else(|| "(hidden)".to_owned()),
                n.bssid.clone().unwrap_or_else(dash),
                n.signal_dbm.map_or_else(dash, |s| format!("{s} dBm")),
                n.channel.map_or_else(dash, |c| c.to_string()),
                n.band_ghz.map_or_else(dash, |b| format!("{b} GHz")),
                n.security.clone(),
            ]
        })
        .collect();
    crate::table::print(
        &["SSID", "BSSID", "SIGNAL", "CHANNEL", "BAND", "SECURITY"],
        &rows,
    );
    if !host::HIDDEN_NAMES.is_empty() && networks.iter().all(|n| n.ssid.is_none()) {
        eprintln!("att wifi: {}", host::HIDDEN_NAMES);
    }
    Ok(0)
}
