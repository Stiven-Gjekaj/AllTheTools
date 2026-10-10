//! `att unquarantine`: remove the mark that a browser puts on a download.
//!
//! macOS keeps the mark in the attribute `com.apple.quarantine`, and
//! Windows keeps it in the stream `Zone.Identifier`. Each host has its own
//! command to remove it: `xattr -d` and `Unblock-File`. This tool is the same
//! command on each host, and it can say what the mark holds first.

use std::path::{Path, PathBuf};

use anyhow::Result;

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

/// Remove the download mark from files, so the system starts them without a
/// warning.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The files or directories.
    #[arg(required = true)]
    pub paths: Vec<PathBuf>,

    /// Go into each directory, and into each directory in it.
    #[arg(short, long)]
    pub recursive: bool,

    /// Say what each mark holds, and remove nothing.
    #[arg(long)]
    pub check: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let mut paths = Vec::new();
    let mut failed = false;
    for path in &args.paths {
        if let Err(e) = collect(path, args.recursive, &mut paths) {
            eprintln!("att unquarantine: {}: {e}", path.display());
            failed = true;
        }
    }

    let mut marked = 0usize;
    for path in &paths {
        let result = if args.check {
            host::read(path).map(|mark| {
                if let Some(mark) = mark {
                    println!("{}: {}", path.display(), parse::describe(&mark));
                    true
                } else {
                    false
                }
            })
        } else {
            host::remove(path).inspect(|removed| {
                if *removed {
                    println!("Removed the download mark from {}.", path.display());
                }
            })
        };
        match result {
            Ok(true) => marked += 1,
            Ok(false) => {}
            Err(e) => {
                eprintln!("att unquarantine: {}: {e}", path.display());
                failed = true;
            }
        }
    }

    let files = paths.len();
    let none = files - marked;
    if files == 0 {
        // Each path failed, and each failure has its line already.
    } else if args.check {
        println!("{marked} of {files} files have a download mark.");
    } else if marked == 0 {
        println!("No file had a download mark.");
    } else if none > 0 {
        println!("{none} other files had no download mark.");
    }
    Ok(if failed { 1 } else { 0 })
}

/// The path, and with `recursive` each path inside it. A link to a directory
/// is not followed, so a loop of links cannot hold the walk.
fn collect(path: &Path, recursive: bool, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let meta = std::fs::symlink_metadata(path)?;
    out.push(path.to_path_buf());
    if recursive && meta.is_dir() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(path)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .collect();
        entries.sort();
        for entry in entries {
            collect(&entry, true, out)?;
        }
    }
    Ok(())
}
