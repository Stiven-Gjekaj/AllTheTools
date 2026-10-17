//! `att df`: show the size and the free space of each mounted volume.
//!
//! macOS and Linux have `df`. Windows has no command that lists the volumes
//! with their free space, and `fsutil volume diskfree` takes one volume and
//! needs Administrator. This tool gives the same table on each host.

use std::path::Path;

use anyhow::{Result, bail};

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

use parse::Volume;

/// Show the size and the free space of each mounted volume.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Show only the volumes that hold these files or folders.
    pub paths: Vec<String>,

    /// Show the volumes with no size too, such as proc on Linux.
    #[arg(short, long)]
    pub all: bool,

    /// Give each size in bytes.
    #[arg(short, long)]
    pub bytes: bool,

    /// Print JSON for a script, and not text for a person. Each size is in
    /// bytes.
    #[arg(long)]
    pub json: bool,
}

/// The volume that holds `path`.
#[cfg(target_os = "macos")]
fn holder<'a>(path: &Path, volumes: &'a [Volume]) -> Option<&'a Volume> {
    let mount = host::mount_of(path)?;
    volumes.iter().find(|v| v.mount == mount)
}

/// The volume that holds `path`: the one on the same device. A device can
/// be mounted at more than one place, and the shortest mount point is the
/// main one.
#[cfg(target_os = "linux")]
fn holder<'a>(path: &Path, volumes: &'a [Volume]) -> Option<&'a Volume> {
    use std::os::unix::fs::MetadataExt;
    let dev = std::fs::metadata(path).ok()?.dev();
    volumes
        .iter()
        .filter(|v| std::fs::metadata(&v.mount).is_ok_and(|m| m.dev() == dev))
        .min_by_key(|v| v.mount.len())
}

/// The volume that holds `path`: the one with the longest mount point that
/// starts the full path, such as `C:\` for `C:\Users`.
#[cfg(windows)]
fn holder<'a>(path: &Path, volumes: &'a [Volume]) -> Option<&'a Volume> {
    let full = std::path::absolute(path).ok()?;
    let full = full.to_string_lossy().to_ascii_lowercase();
    volumes
        .iter()
        .filter(|v| full.starts_with(&v.mount.to_ascii_lowercase()))
        .max_by_key(|v| v.mount.len())
}

pub fn run(args: Args) -> Result<i32> {
    let volumes = host::volumes()?;
    let volumes = if args.paths.is_empty() {
        parse::shown(volumes, args.all)
    } else {
        let mut picked: Vec<Volume> = Vec::new();
        for path in &args.paths {
            let Some(volume) = holder(Path::new(path), &volumes) else {
                bail!("{path} does not exist, or no mounted volume holds it");
            };
            if !picked.contains(volume) {
                picked.push(volume.clone());
            }
        }
        picked
    };
    if args.json {
        println!("{}", serde_json::to_string(&volumes)?);
        return Ok(0);
    }
    crate::table::print(
        &[
            "FILESYSTEM",
            "TYPE",
            "SIZE",
            "USED",
            "AVAIL",
            "USE%",
            "MOUNTED ON",
        ],
        &parse::rows(&volumes, args.bytes),
    );
    Ok(0)
}
