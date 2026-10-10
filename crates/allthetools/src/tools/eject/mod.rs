//! `att eject`: unmount a drive and make it safe to remove.
//!
//! macOS has `diskutil eject`, and Linux has `eject` and `udisksctl`.
//! Windows has no command for it, only "Safely Remove Hardware" in the
//! taskbar. This tool is one command on each host, and it takes the names
//! that `att lsblk` shows.

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

use crate::disks;

/// Unmount each volume of a drive, and make the drive safe to remove.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The drive, as `att lsblk` names it: disk4, sdb or 2, for example.
    pub drive: String,
}

pub fn run(args: Args) -> Result<i32> {
    let disks = disks::list()?;
    let disk = disks::parse::select(&disks, &args.drive)?;
    let plan = parse::plan(disk)?;
    host::eject(&plan)?;
    println!(
        "Ejected {}. It is safe to remove it now.",
        parse::describe(&plan.disk)
    );
    Ok(0)
}
