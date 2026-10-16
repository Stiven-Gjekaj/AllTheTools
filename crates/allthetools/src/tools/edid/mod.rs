//! `att edid`: show what each connected display says about itself: the
//! maker, the model, the serial number, the size and the native mode.
//!
//! Linux has `edid-decode`, which reads /sys/class/drm. macOS and Windows
//! keep the EDID of a display deep in the IOKit registry and in the
//! registry of Windows, and have no command that reads it.

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

/// Show the maker, the model, the serial number and the native mode of each
/// connected display.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Print the bytes of each EDID as hex too.
    #[arg(long)]
    pub raw: bool,

    /// Print JSON for a script, and not text for a person.
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let mut displays = Vec::new();
    for (source, bytes) in host::read()? {
        match parse::decode(&bytes, source.clone()) {
            Ok(display) => displays.push(display),
            Err(e) => eprintln!(
                "att: skipped the EDID of {}: {e:#}",
                source.as_deref().unwrap_or("a display")
            ),
        }
    }
    if args.json {
        println!("{}", serde_json::to_string(&displays)?);
        return Ok(0);
    }
    if displays.is_empty() {
        println!("No connected display gives an EDID. A built-in display may give none.");
        return Ok(0);
    }
    for (i, display) in displays.iter().enumerate() {
        if i > 0 {
            println!();
        }
        let lines = parse::lines(display);
        let width = lines.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
        for (key, value) in &lines {
            println!("{key:width$}  {value}");
        }
        if args.raw {
            println!();
            for chunk in display.edid.as_bytes().chunks(32) {
                println!("    {}", String::from_utf8_lossy(chunk));
            }
        }
    }
    Ok(0)
}
