//! `att ddc`: read and set the brightness and the other features of a
//! monitor.
//!
//! Linux has `ddcutil`. macOS has no command for it, and Apple silicon needs
//! m1ddc or BetterDisplay. Windows has no command either. This tool sends the
//! DDC/CI messages of MCCS on each host, to an external monitor.

use anyhow::{Result, bail};

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

/// Read and set the brightness, the contrast, the volume and the input of
/// an external monitor.
#[derive(Debug, clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub action: Action,
}

#[derive(Debug, clap::Subcommand)]
pub enum Action {
    /// List the monitors that att ddc can reach.
    List,
    /// Read a feature: brightness, contrast, volume, input, or a VCP code
    /// such as 0x10.
    Get {
        feature: String,
        /// The monitor, by its number in `att ddc list`.
        #[arg(short, long, default_value_t = 1)]
        monitor: usize,
    },
    /// Set a feature to a value, or change it by +N or -N. The input takes
    /// a name too: dp, dp2, hdmi, hdmi2, usbc, dvi or vga.
    Set {
        feature: String,
        value: String,
        /// The monitor, by its number in `att ddc list`.
        #[arg(short, long, default_value_t = 1)]
        monitor: usize,
    },
}

pub fn run(args: Args) -> Result<i32> {
    let monitors = host::monitors()?;
    if let Action::List = args.action {
        if monitors.is_empty() {
            println!("No external monitor answers on this computer.");
        }
        for (i, monitor) in monitors.iter().enumerate() {
            println!("{}  {}", i + 1, monitor.name);
        }
        return Ok(0);
    }
    let pick = |index: usize| -> Result<&host::Monitor> {
        if monitors.is_empty() {
            bail!("no external monitor answers on this computer");
        }
        match monitors.get(index.wrapping_sub(1)) {
            Some(monitor) => Ok(monitor),
            None => bail!("there is no monitor {index}. `att ddc list` lists them"),
        }
    };
    match args.action {
        Action::List => unreachable!("handled above"),
        Action::Get { feature, monitor } => {
            let code = parse::feature(&feature)?;
            let reply = host::get(pick(monitor)?, code)?;
            println!(
                "{}: {} of {}",
                parse::feature_name(code),
                reply.current,
                reply.max
            );
        }
        Action::Set {
            feature,
            value,
            monitor,
        } => {
            let code = parse::feature(&feature)?;
            let target = pick(monitor)?;
            let relative = value.starts_with(['+', '-']);
            let current = if relative {
                let reply = host::get(target, code)?;
                Some((reply.current, reply.max))
            } else {
                None
            };
            let new = parse::value(&value, code, current)?;
            host::set(target, code, new)?;
            println!("{}: set to {new}", parse::feature_name(code));
        }
    }
    Ok(0)
}
