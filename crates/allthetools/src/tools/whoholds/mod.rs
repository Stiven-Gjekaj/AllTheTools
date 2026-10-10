//! `att whoholds`: show which process holds a file, a folder or a drive.
//!
//! The question comes when an eject or a delete fails. macOS and Linux
//! answer it with `lsof` and `fuser`, each with its own options. Windows has
//! no command for it, and Microsoft points to Resource Monitor or to
//! `handle` of Sysinternals. This tool is one command on each host.

use std::path::PathBuf;

use anyhow::{Context, Result};

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

/// Show which process holds a file, a folder or a drive open.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The file, the folder or the mount point. On Windows, a drive letter
    /// such as E: is a drive.
    pub path: PathBuf,

    /// Print JSON for a script, and not a table for a person.
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let mut path = args.path.clone();
    // E: alone is the working directory of drive E, and E:\ is the drive.
    if cfg!(windows) && path.as_os_str().len() == 2 && path.to_string_lossy().ends_with(':') {
        path.push("\\");
    }
    let target = dunce(
        std::fs::canonicalize(&path)
            .with_context(|| format!("{} does not exist", path.display()))?,
    );
    let mut holders = host::holders(&target)?;
    parse::sort(&mut holders);
    if args.json {
        println!("{}", serde_json::to_string(&holders)?);
        return Ok(0);
    }
    if holders.is_empty() {
        println!(
            "No process that this user can see holds {}.",
            target.display()
        );
        return Ok(0);
    }
    let rows: Vec<Vec<String>> = holders
        .iter()
        .map(|h| {
            vec![
                h.pid.to_string(),
                h.process.clone(),
                h.how.clone(),
                h.path.clone().unwrap_or_else(|| "-".to_owned()),
            ]
        })
        .collect();
    crate::table::print(&["PID", "PROCESS", "HOW", "PATH"], &rows);
    if cfg!(unix) {
        eprintln!("att whoholds: run as root to see the processes of other users too.");
    }
    Ok(0)
}

/// canonicalize gives the \\?\ form on Windows, which the Restart Manager
/// and a person do not need.
fn dunce(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest),
        _ => path,
    }
}
