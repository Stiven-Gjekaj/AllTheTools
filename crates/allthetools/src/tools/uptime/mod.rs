//! `att uptime`: show how long the host has run since its start, and its
//! load.
//!
//! macOS and Linux have `uptime`. Windows shows the time since the start in
//! Task Manager only, and `systeminfo` takes seconds and writes a date in
//! the language of the host. This tool gives the same report on each host.

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

/// Show how long this computer has run since its start, and its load.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Give only the time since the start.
    #[arg(short, long, conflicts_with = "since")]
    pub pretty: bool,

    /// Give only the date and the time of the start, in UTC.
    #[arg(short, long)]
    pub since: bool,

    /// Print JSON for a script, and not text for a person.
    #[arg(long, conflicts_with_all = ["pretty", "since"])]
    pub json: bool,
}

/// The seconds since 1970 now.
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn run(args: Args) -> Result<i32> {
    let up = host::read()?;
    if args.json {
        println!("{}", serde_json::to_string(&up)?);
    } else if args.pretty {
        println!("up {}", parse::duration(up.seconds));
    } else if args.since {
        println!("{}", parse::utc(up.boot));
    } else {
        let lines = up.lines();
        let width = lines.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
        for (key, value) in lines {
            println!("{key:width$}  {value}");
        }
    }
    Ok(0)
}
