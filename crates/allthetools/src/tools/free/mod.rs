//! `att free`: show the memory and the swap of the host.
//!
//! Linux has `free`. macOS has `vm_stat`, which counts pages and leaves the
//! sums to the reader, and Windows shows memory in Task Manager only. This
//! tool gives the same table on each host.

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

/// Show the memory and the swap of this computer.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Give each size in bytes.
    #[arg(short, long)]
    pub bytes: bool,

    /// Print JSON for a script, and not text for a person. Each size is in
    /// bytes.
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let memory = host::read()?;
    if args.json {
        println!("{}", serde_json::to_string(&memory)?);
        return Ok(0);
    }
    crate::table::print(
        &["", "TOTAL", "USED", "FREE", "CACHE", "AVAILABLE"],
        &parse::rows(&memory, args.bytes),
    );
    Ok(0)
}
