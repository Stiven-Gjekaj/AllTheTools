//! `att ip`: list the network interfaces with their state, their hardware
//! address and their IP addresses.
//!
//! Each host has a command for this, and each one has its own output: `ip`
//! on Linux, `ifconfig` on macOS and `ipconfig` on Windows. This tool gives
//! one table on each host, as `ip -brief address` does.

use anyhow::Result;

pub mod parse;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix as host;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as host;

/// List the network interfaces with their state and their addresses.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Show only the interfaces that are up.
    #[arg(short, long)]
    pub up: bool,

    /// Print JSON for a script, and not text for a person.
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let mut interfaces = host::interfaces()?;
    if args.up {
        interfaces.retain(|i| i.state == "up");
    }
    if args.json {
        println!("{}", serde_json::to_string(&interfaces)?);
        return Ok(0);
    }
    crate::table::print(
        &["NAME", "STATE", "MAC", "MTU", "ADDRESS"],
        &parse::rows(&interfaces),
    );
    Ok(0)
}
