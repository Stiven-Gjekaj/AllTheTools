//! `att mount`: attach a disk image and mount its volumes, or detach it.
//!
//! Each host has its own command and its own kinds of image: `hdiutil` on
//! macOS, `udisksctl loop-setup` or `losetup` on Linux, and Mount-DiskImage
//! on Windows. This tool is one command on each host, and it says which
//! kinds the host takes.

use std::path::PathBuf;

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

/// Attach a disk image and mount its volumes, or detach it.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The image: .iso, .img, .dmg, .vhd or .vhdx. With --detach, the image,
    /// its device or its drive letter.
    pub image: PathBuf,

    /// Detach the image, after its volumes are unmounted.
    #[arg(short, long)]
    pub detach: bool,

    /// Attach the image so that nothing can write to it.
    #[arg(short, long, conflicts_with = "detach")]
    pub read_only: bool,

    /// Print JSON for a script, and not a table for a person.
    #[arg(long, conflicts_with = "detach")]
    pub json: bool,
}

pub fn run(args: Args) -> Result<i32> {
    if args.detach {
        let device = host::detach(&args.image.to_string_lossy())?;
        println!("Detached {device}.");
        return Ok(0);
    }
    if !args.image.is_file() {
        bail!("{} is not a file", args.image.display());
    }
    let Some(kind) = parse::kind(&args.image) else {
        bail!(
            "att mount does not know the kind of {}. Give a file that ends in .iso, .img, .dmg, .vhd or .vhdx",
            args.image.display()
        );
    };
    let attached = host::attach(&args.image, kind, args.read_only)?;
    if args.json {
        println!("{}", serde_json::to_string(&attached)?);
        return Ok(0);
    }
    let dash = || "-".to_owned();
    let rows: Vec<Vec<String>> = attached
        .iter()
        .map(|a| {
            vec![
                a.device.clone(),
                a.filesystem.clone().unwrap_or_else(dash),
                a.mount.clone().unwrap_or_else(dash),
            ]
        })
        .collect();
    crate::table::print(&["DEVICE", "FSTYPE", "MOUNTPOINT"], &rows);
    Ok(0)
}
