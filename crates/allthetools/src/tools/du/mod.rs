//! `att du`: show which files and folders take the most space.
//!
//! macOS and Linux have `du`. Windows has no command that adds up the size
//! of a folder, and Explorer counts one folder at a time. This tool walks
//! each path and lists its largest entries, the largest first.
//!
//! On macOS and Linux a size is the space on the disk, as `du` counts it,
//! and a file with more than one hard link counts one time. Windows gives
//! the length of each file.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;

pub mod tally;

use tally::{Entry, Tally};

/// Show which files and folders take the most space.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The folders to measure. With none, the current folder.
    pub paths: Vec<PathBuf>,

    /// List the entries down to this many levels under each path.
    #[arg(short = 'd', long, default_value_t = 1, value_name = "N")]
    pub depth: usize,

    /// List at most this many entries for each path. 0 lists each one.
    #[arg(short = 'n', long, default_value_t = 20, value_name = "N")]
    pub top: usize,

    /// Give only the total of each path.
    #[arg(short, long, conflicts_with_all = ["depth", "top"])]
    pub summarize: bool,

    /// Count the length of each file, and not the space on the disk.
    #[arg(long)]
    pub apparent_size: bool,

    /// Stay on the volume of each path, and do not go into other mounts.
    #[arg(short = 'x', long)]
    pub one_file_system: bool,

    /// Give each size in bytes.
    #[arg(short, long)]
    pub bytes: bool,

    /// Print JSON for a script, and not text for a person.
    #[arg(long)]
    pub json: bool,
}

/// The result for one path.
#[derive(Debug, Default, Serialize)]
pub(crate) struct Report {
    pub(crate) path: String,
    pub(crate) total: u64,
    pub(crate) entries: Vec<Entry>,
}

/// What the walk needs to know about a file.
struct Meta {
    size: u64,
    folder: bool,
    link: bool,
    device: u64,
    /// The device and the inode of a file with more than one hard link.
    shared: Option<(u64, u64)>,
}

#[cfg(unix)]
fn meta(path: &Path, apparent: bool) -> std::io::Result<Meta> {
    use std::os::unix::fs::MetadataExt;
    let m = std::fs::symlink_metadata(path)?;
    Ok(Meta {
        size: if apparent { m.len() } else { m.blocks() * 512 },
        folder: m.is_dir(),
        link: m.file_type().is_symlink(),
        device: m.dev(),
        shared: (m.nlink() > 1 && !m.is_dir()).then_some((m.dev(), m.ino())),
    })
}

#[cfg(windows)]
fn meta(path: &Path, _apparent: bool) -> std::io::Result<Meta> {
    let m = std::fs::symlink_metadata(path)?;
    Ok(Meta {
        size: if m.is_dir() { 0 } else { m.len() },
        folder: m.is_dir(),
        link: m.file_type().is_symlink(),
        device: 0,
        shared: None,
    })
}

/// Walk one path, and give its sums and the count of entries it could not
/// read.
fn walk(start: &Path, args: &Args) -> (Tally, u64) {
    let depth = if args.summarize { 0 } else { args.depth };
    let mut tally = Tally::new(depth);
    let mut errors = 0;
    let mut seen = std::collections::HashSet::new();
    let root = match meta(start, args.apparent_size) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("att: could not read {}: {e}", start.display());
            return (tally, 1);
        }
    };
    let mut stack: Vec<(PathBuf, Vec<String>)> = vec![(start.to_path_buf(), Vec::new())];
    while let Some((path, names)) = stack.pop() {
        let m = match meta(&path, args.apparent_size) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("att: could not read {}: {e}", path.display());
                errors += 1;
                continue;
            }
        };
        if let Some(key) = m.shared
            && !seen.insert(key)
        {
            continue;
        }
        tally.add(&names, m.size, m.folder);
        if !m.folder || m.link || (args.one_file_system && m.device != root.device) {
            continue;
        }
        let entries = match std::fs::read_dir(&path) {
            Ok(entries) => entries,
            Err(e) => {
                eprintln!("att: could not read {}: {e}", path.display());
                errors += 1;
                continue;
            }
        };
        for entry in entries.flatten() {
            let mut child = names.clone();
            child.push(entry.file_name().to_string_lossy().into_owned());
            stack.push((entry.path(), child));
        }
    }
    (tally, errors)
}

pub fn run(args: Args) -> Result<i32> {
    let paths = if args.paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        args.paths.clone()
    };
    let mut reports = Vec::new();
    let mut errors = 0;
    for path in &paths {
        let (tally, failed) = walk(path, &args);
        errors += failed;
        reports.push(Report {
            path: path.display().to_string(),
            total: tally.total,
            entries: tally.largest(args.top),
        });
    }
    if args.json {
        println!("{}", serde_json::to_string(&reports)?);
    } else {
        let size = |n: u64| {
            if args.bytes {
                n.to_string()
            } else {
                crate::size::binary(n)
            }
        };
        let mut rows = Vec::new();
        for report in &reports {
            let base = report.path.trim_end_matches(['/', '\\']);
            for entry in &report.entries {
                let slash = if entry.folder { "/" } else { "" };
                rows.push(vec![
                    size(entry.size),
                    format!("{base}/{}{slash}", entry.path),
                ]);
            }
            rows.push(vec![size(report.total), format!("{} (total)", report.path)]);
        }
        crate::table::print(&["SIZE", "PATH"], &rows);
    }
    // Exit code 1 says that some entries could not be read, as du gives it.
    Ok(if errors > 0 { 1 } else { 0 })
}
