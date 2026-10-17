//! `att lscpu`: show the processor: its model, its cores and threads, its
//! caches and its features.
//!
//! Linux has `lscpu`. macOS keeps these values in sysctl, under names that a
//! person must know, and Windows shows them in Task Manager only. This tool
//! gives the same report on each host.

use anyhow::Result;

pub mod parse;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
use linux as host;
#[cfg(target_os = "macos")]
use macos as host;
#[cfg(windows)]
use windows as host;

/// Show the model, the cores, the threads and the caches of the processor.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Print JSON for a script, and not text for a person.
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let cpu = host::read(std::env::consts::ARCH)?;
    if args.json {
        println!("{}", serde_json::to_string(&cpu)?);
        return Ok(0);
    }
    let lines = cpu.lines();
    let width = lines.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    for (key, value) in lines {
        println!("{key:width$}  {value}");
    }
    Ok(0)
}
