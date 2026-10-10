//! `att sensors`: show the temperatures and the fans.
//!
//! Linux has `sensors` from lm-sensors. macOS asks for root in
//! `powermetrics`, and Windows has no command at all. This tool reads the
//! sensors that each host gives to a standard user.

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

/// Show the temperatures, the fans and the voltages that the host reports.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Print JSON for a script, and not text for a person.
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let readings = host::read()?;
    if args.json {
        println!("{}", serde_json::to_string(&readings)?);
        return Ok(0);
    }
    if readings.is_empty() {
        println!("This computer reports no sensor to a standard user.");
        return Ok(0);
    }
    let rows: Vec<Vec<String>> = readings
        .iter()
        .map(|r| vec![r.chip.clone(), r.label.clone(), r.show()])
        .collect();
    crate::table::print(&["CHIP", "SENSOR", "VALUE"], &rows);
    Ok(0)
}
