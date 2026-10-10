//! `att link`: make the familiar names of the tools run att.
//!
//! A package of att installs one binary, and no other name. So the package
//! never takes a name that the host or another package owns. This command
//! adds the familiar names on request, and it skips each name that the host
//! has already.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::alias::ALIASES;
use crate::host;

/// Make the familiar names, such as lsusb and f3probe, run att.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The directory for the names. The default is the directory of att.
    pub dir: Option<PathBuf>,

    /// Make a name that another program on PATH has already.
    #[arg(long)]
    pub force: bool,

    /// Remove each name that runs att, and nothing else.
    #[arg(long, conflicts_with = "force")]
    pub remove: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let att = std::env::current_exe().context("could not find the path of att")?;
    let att = std::fs::canonicalize(&att).unwrap_or(att);
    let dir = match args.dir {
        Some(dir) => dir,
        None => att.parent().context("att has no directory")?.to_path_buf(),
    };
    let mut failed = false;
    for (name, words) in ALIASES {
        let path = dir.join(file_name(name));
        let result = if args.remove {
            remove(&path, &att).map(|done| done.then(|| format!("Removed {}.", path.display())))
        } else {
            make(&path, name, words, &att, args.force)
        };
        match result {
            Ok(Some(line)) => println!("{line}"),
            Ok(None) => {}
            Err(e) => {
                eprintln!("att link: {}: {e:#}", path.display());
                failed = true;
            }
        }
    }
    Ok(if failed { 1 } else { 0 })
}

fn file_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.cmd")
    } else {
        name.to_owned()
    }
}

/// True when the file at `path` runs att.
fn runs_att(path: &Path, att: &Path) -> bool {
    if cfg!(windows) {
        std::fs::read_to_string(path).is_ok_and(|text| text.contains(&*att.to_string_lossy()))
    } else {
        std::fs::canonicalize(path).is_ok_and(|target| target == att)
    }
}

fn make(
    path: &Path,
    name: &str,
    words: &[&str],
    att: &Path,
    force: bool,
) -> Result<Option<String>> {
    if runs_att(path, att) {
        return Ok(None);
    }
    if path.exists() || path.symlink_metadata().is_ok() {
        return Ok(Some(format!(
            "Skipped {name}: {} exists, and it is not att.",
            path.display()
        )));
    }
    if !force && host::on_path(name) {
        return Ok(Some(format!(
            "Skipped {name}: the host has a program of that name. Add --force to make it anyway."
        )));
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(att, path).context("could not make the link")?;
    #[cfg(windows)]
    std::fs::write(path, crate::alias::shim(&att.to_string_lossy(), words))
        .context("could not write the file")?;
    Ok(Some(format!("{name} runs att {}.", words.join(" "))))
}

fn remove(path: &Path, att: &Path) -> Result<bool> {
    if !runs_att(path, att) {
        return Ok(false);
    }
    std::fs::remove_file(path).context("could not remove the file")?;
    Ok(true)
}
