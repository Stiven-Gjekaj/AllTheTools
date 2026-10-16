//! The host file of `att notify` on Windows: a toast, through the
//! notification API of Windows Runtime in PowerShell.

use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

use super::parse;

pub fn show(title: &str, message: &str) -> Result<()> {
    let script = format!("$ErrorActionPreference = 'Stop'; {}", parse::POWERSHELL);
    let out = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .env("ATT_NOTIFY_TITLE", title)
        .env("ATT_NOTIFY_MESSAGE", message)
        .stdin(Stdio::null())
        .output()
        .context("could not start PowerShell")?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let first = stderr.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
        bail!("Windows showed no notification: {}", first.trim());
    }
    Ok(())
}
