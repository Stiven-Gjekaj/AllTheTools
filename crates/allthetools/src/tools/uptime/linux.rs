//! The host file of `att uptime` on Linux: /proc/uptime and /proc/loadavg.

use anyhow::{Context, Result, bail};

use super::parse::{self, Uptime};

pub fn read() -> Result<Uptime> {
    let text = std::fs::read_to_string("/proc/uptime").context("could not read /proc/uptime")?;
    let Some(seconds) = parse::proc_uptime(&text) else {
        bail!("/proc/uptime gives no number");
    };
    let load = std::fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|t| parse::loadavg(&t));
    Ok(Uptime {
        seconds,
        boot: super::now().saturating_sub(seconds),
        load,
    })
}
