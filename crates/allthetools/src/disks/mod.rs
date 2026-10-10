//! The drives of this computer, which the disk tools share.
//!
//! `att lsblk` shows the list, and `att eject`, `att surface` and
//! `att probe` take a drive from it. One list on each host means that each
//! tool names a drive the same way, and each tool knows which drive holds
//! the running system.

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

pub use parse::{Disk, Partition};

/// The drives of this computer, in the order of their names.
pub fn list() -> Result<Vec<Disk>> {
    host::list()
}
