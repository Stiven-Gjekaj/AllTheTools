//! The host file of `att free` on Linux: /proc/meminfo.

use anyhow::{Context, Result};

use super::parse::{self, Memory};

pub fn read() -> Result<Memory> {
    let text = std::fs::read_to_string("/proc/meminfo").context("could not read /proc/meminfo")?;
    parse::linux(&text)
}
