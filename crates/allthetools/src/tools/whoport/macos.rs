//! The host file of `att whoport` on macOS: `lsof`, which ships with macOS.
//!
//! A standard user sees the sockets of their own processes only.

use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

use super::parse::{self, Socket};

pub fn sockets() -> Result<Vec<Socket>> {
    let out = Command::new("lsof")
        .args(["-nP", "-i", "-F", "pcftPnT"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .context("could not start lsof")?;
    // lsof gives exit code 1 when it finds no socket at all.
    if !out.status.success() && !out.stdout.is_empty() {
        bail!("lsof failed ({})", out.status);
    }
    Ok(parse::lsof(&String::from_utf8_lossy(&out.stdout)))
}

pub fn stop(pid: u32) -> std::io::Result<()> {
    super::unix_stop(pid)
}
