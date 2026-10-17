//! `att flushdns`: empty the DNS cache of the host, so the next lookup of
//! each name asks the DNS server again.
//!
//! Each host has its own command for this, and macOS needs two of them:
//! `ipconfig /flushdns` on Windows, `dscacheutil` and a signal to
//! mDNSResponder on macOS, and `resolvectl` or `nscd` on Linux. This tool is
//! one command on each host.

use anyhow::Result;

pub mod plan;

use crate::host;

/// Empty the DNS cache of this computer.
#[derive(Debug, clap::Args)]
pub struct Args {}

fn apply(method: &plan::Method) -> Result<()> {
    for command in &method.commands {
        host::output(command[0], &command[1..], None)?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn flush() -> Result<String> {
    // SAFETY: geteuid has no argument and cannot fail.
    #[allow(unsafe_code)]
    if unsafe { libc::geteuid() } != 0 {
        anyhow::bail!("macOS lets only root reset mDNSResponder. Run the command with sudo");
    }
    let method = plan::macos();
    apply(&method)?;
    Ok(method.cache.to_owned())
}

#[cfg(windows)]
fn flush() -> Result<String> {
    let method = plan::windows();
    apply(&method)?;
    Ok(method.cache.to_owned())
}

#[cfg(target_os = "linux")]
fn flush() -> Result<String> {
    let methods = plan::linux(host::on_path);
    if methods.is_empty() {
        anyhow::bail!(
            "this host has no systemd-resolved and no nscd, so it keeps no DNS cache that att knows. A program then asks the DNS server each time"
        );
    }
    let mut errors = Vec::new();
    for method in &methods {
        match apply(method) {
            Ok(()) => return Ok(method.cache.to_owned()),
            Err(e) => errors.push(format!("{}: {e:#}", method.cache)),
        }
    }
    anyhow::bail!(
        "no DNS cache took the flush. {}. A flush can need root: run the command with sudo",
        errors.join("; ")
    )
}

pub fn run(_args: Args) -> Result<i32> {
    let cache = flush()?;
    println!("Flushed the DNS cache of {cache}.");
    Ok(0)
}
