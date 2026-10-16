//! The host file of `att trash` on Linux: the trash of the freedesktop.org
//! specification.
//!
//! A file on the volume of the home folder goes to ~/.local/share/Trash. A
//! file on another volume goes to `.Trash-<uid>` at the top of that volume,
//! because a move to another volume is a copy, and a copy of a large folder
//! is no move.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use super::parse::{self, Stamp};

fn home_trash() -> Result<PathBuf> {
    if let Some(data) = std::env::var_os("XDG_DATA_HOME").filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(data).join("Trash"));
    }
    let Some(home) = std::env::var_os("HOME") else {
        bail!("HOME is not set, so the trash has no place");
    };
    Ok(PathBuf::from(home).join(".local/share/Trash"))
}

/// The device of the nearest folder that exists, at or above `path`.
fn device(path: &Path) -> Option<u64> {
    path.ancestors()
        .find_map(|p| fs::metadata(p).ok())
        .map(|m| m.dev())
}

/// The top of the volume of `path`: the highest folder above it on the same
/// device.
fn top(path: &Path, dev: u64) -> PathBuf {
    let mut top = path.parent().unwrap_or(Path::new("/")).to_path_buf();
    while let Some(up) = top.parent() {
        match fs::metadata(up) {
            Ok(m) if m.dev() == dev => top = up.to_path_buf(),
            _ => break,
        }
    }
    top
}

fn make_dirs(trash: &Path) -> Result<()> {
    for sub in ["files", "info"] {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(trash.join(sub))
            .with_context(|| format!("could not make {}", trash.join(sub).display()))?;
    }
    Ok(())
}

fn now() -> Stamp {
    // SAFETY: localtime_r writes one tm into a variable of that type, and
    // tm holds only numbers and a pointer that the call sets.
    #[allow(unsafe_code)]
    let tm = unsafe {
        let time = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&time, &mut tm);
        tm
    };
    Stamp {
        year: tm.tm_year + 1900,
        month: (tm.tm_mon + 1) as u32,
        day: tm.tm_mday as u32,
        hour: tm.tm_hour as u32,
        minute: tm.tm_min as u32,
        second: tm.tm_sec as u32,
    }
}

pub fn trash(path: &Path) -> Result<()> {
    let dev = fs::symlink_metadata(path)?.dev();
    let home = home_trash()?;
    let (trash, recorded) = if device(&home) == Some(dev) {
        (home, path.to_path_buf())
    } else {
        // SAFETY: getuid has no argument and cannot fail.
        #[allow(unsafe_code)]
        let uid = unsafe { libc::getuid() };
        let top = top(path, dev);
        let recorded = path.strip_prefix(&top).unwrap_or(path).to_path_buf();
        (top.join(format!(".Trash-{uid}")), recorded)
    };
    if path.starts_with(&trash) {
        bail!("it is in the trash already");
    }
    make_dirs(&trash)?;

    let Some(name) = path.file_name() else {
        bail!("{} has no name", path.display());
    };
    let name = name.to_string_lossy();
    let text = parse::info(recorded.as_os_str().as_bytes(), now());
    // The info file comes first, and its creation fails when the name is
    // taken. So two runs at the same time never take the same name.
    for attempt in 1..10_000 {
        let entry = parse::candidate(&name, attempt);
        let info = trash.join("info").join(format!("{entry}.trashinfo"));
        let mut file = match OpenOptions::new().write(true).create_new(true).open(&info) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e).with_context(|| format!("could not write {}", info.display())),
        };
        let target = trash.join("files").join(&entry);
        if target.exists() {
            drop(file);
            let _ = fs::remove_file(&info);
            continue;
        }
        file.write_all(text.as_bytes())?;
        drop(file);
        if let Err(e) = fs::rename(path, &target) {
            let _ = fs::remove_file(&info);
            return Err(e).context("the move failed");
        }
        return Ok(());
    }
    bail!("the trash has no free name for {name}")
}
