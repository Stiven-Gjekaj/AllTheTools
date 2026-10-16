//! `att trash`: move files to the trash, where a person can get them back.
//!
//! Linux has `gio trash`, and macOS has `trash` since version 15. Windows
//! has no command that moves a file to the Recycle Bin, and `del` deletes it
//! for good. This tool uses the trash of the host on each one: the Trash of
//! the Finder, the Recycle Bin, and the trash of the freedesktop.org
//! specification, which the file managers of Linux read.

use std::path::{Path, PathBuf};

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

/// Move files and folders to the trash.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The files and the folders to move.
    #[arg(required = true)]
    pub paths: Vec<PathBuf>,

    /// Say nothing about a path that does not exist.
    #[arg(short, long)]
    pub force: bool,

    /// Name each path that goes to the trash.
    #[arg(short, long)]
    pub verbose: bool,
}

/// The absolute path of an entry, with no link followed at its end, so a
/// link goes to the trash and not the file that it points to.
fn absolute(path: &Path) -> std::io::Result<PathBuf> {
    std::path::absolute(path)
}

pub fn run(args: Args) -> Result<i32> {
    let mut failed = false;
    for path in &args.paths {
        if std::fs::symlink_metadata(path).is_err() {
            if !args.force {
                eprintln!("att: {} does not exist", path.display());
                failed = true;
            }
            continue;
        }
        let result = absolute(path)
            .map_err(anyhow::Error::from)
            .and_then(|full| host::trash(&full));
        match result {
            Ok(()) => {
                if args.verbose {
                    println!("Moved {} to the trash.", path.display());
                }
            }
            Err(e) => {
                eprintln!("att: could not move {} to the trash: {e:#}", path.display());
                failed = true;
            }
        }
    }
    Ok(if failed { 1 } else { 0 })
}
