//! The host file of `att notify` on Linux: `notify-send` when the host has
//! it, and the notification server of the desktop through `gdbus` when it
//! does not.

use anyhow::{Result, bail};

use super::parse;
use crate::host;

pub fn show(title: &str, message: &str) -> Result<()> {
    if host::on_path("notify-send") {
        host::output("notify-send", &["--", title, message], None)?;
        return Ok(());
    }
    if host::on_path("gdbus") {
        let args = parse::gdbus_args(title, message);
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        host::output("gdbus", &args, None).map_err(|e| {
            e.context("the desktop took no notification. A notification needs a desktop session")
        })?;
        return Ok(());
    }
    bail!(
        "this host has no `notify-send` and no `gdbus`. Install libnotify-bin on Debian and Ubuntu, or libnotify on Fedora and Arch"
    )
}
