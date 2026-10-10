//! `att battery`: show the charge and the health of each battery.
//!
//! Each host has its own tool, and each tool gives other facts: `pmset` and
//! `ioreg` on macOS, `upower` on Linux, and `powercfg /batteryreport` on
//! Windows, which writes an HTML file. This tool gives the same report on
//! each host: the charge, the health, the cycles and the temperature.

use anyhow::Result;

pub mod parse;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

use parse::Battery;

/// Show the charge, the health and the cycle count of each battery.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Print JSON for a script, and not text for a person.
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let batteries = batteries()?;
    if args.json {
        println!("{}", serde_json::to_string(&batteries)?);
        return Ok(0);
    }
    if batteries.is_empty() {
        println!("This computer reports no battery.");
        return Ok(0);
    }
    for (i, battery) in batteries.iter().enumerate() {
        if i > 0 {
            println!();
        }
        let lines = battery.lines();
        let width = lines.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
        for (key, value) in lines {
            println!("{key:width$}  {value}");
        }
    }
    Ok(0)
}

#[cfg(target_os = "linux")]
fn batteries() -> Result<Vec<Battery>> {
    Ok(parse::linux(&linux::read()?))
}

#[cfg(target_os = "macos")]
fn batteries() -> Result<Vec<Battery>> {
    parse::macos(&macos::read()?)
}

#[cfg(windows)]
fn batteries() -> Result<Vec<Battery>> {
    parse::windows(&windows::read()?)
}
