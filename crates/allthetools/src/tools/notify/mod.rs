//! `att notify`: show a notification on the desktop.
//!
//! Linux has `notify-send`. macOS shows a notification only through an
//! AppleScript, and Windows has no command for it. So a script that tells
//! the person that a long job is done needs a different line on each host.
//! This tool is the same line on each one:
//!
//! ```text
//! make && att notify "The build is done"
//! ```

use anyhow::Result;

pub mod parse;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
use linux as host;
#[cfg(target_os = "macos")]
use macos as host;
#[cfg(windows)]
use windows as host;

/// Show a notification on the desktop.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The text of the notification.
    pub message: String,

    /// The title of the notification.
    #[arg(short, long, default_value = "att")]
    pub title: String,
}

pub fn run(args: Args) -> Result<i32> {
    host::show(&args.title, &args.message)?;
    Ok(0)
}
