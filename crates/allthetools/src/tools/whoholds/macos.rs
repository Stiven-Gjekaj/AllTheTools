//! The host file of `att whoholds` on macOS: `lsof`, which ships with macOS.
//!
//! lsof reads a mount point as the whole volume, a folder with +D as each
//! file inside it, and a file as that file.

use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

use super::parse::{self, Holder};

/// True when the path is the root of a volume: its parent is on another
/// device.
fn mount_point(path: &Path) -> bool {
    let (Ok(own), Some(Ok(parent))) = (
        std::fs::metadata(path),
        path.parent().map(std::fs::metadata),
    ) else {
        return path.parent().is_none();
    };
    own.dev() != parent.dev()
}

pub fn holders(target: &Path) -> Result<Vec<Holder>> {
    let path = target.to_string_lossy().into_owned();
    let mut args = vec!["-n", "-P", "-F", "pcfn"];
    if target.is_dir() && !mount_point(target) {
        args.push("+D");
    }
    args.push(&path);
    let out = Command::new("lsof")
        .args(&args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .context("could not start lsof")?;
    // lsof gives exit code 1 when no process holds the path.
    if !out.status.success() && !out.stdout.is_empty() {
        bail!("lsof failed ({})", out.status);
    }
    Ok(parse::lsof(&String::from_utf8_lossy(&out.stdout)))
}
