//! A drive opened for raw reads and writes, which `att surface` and
//! `att probe` share.
//!
//! A tool names a drive as `att lsblk` does, or an image file. It gets a
//! [`Blocks`], and it never sees a host type.

use std::io;
use std::path::Path;

use anyhow::Result;

pub mod blocks;
pub mod file;
pub mod guard;

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

pub use blocks::{Aligned, Blocks};

use crate::disks::{self, Disk};

/// What a tool opened.
pub enum Target {
    Drive(Disk),
    File(std::path::PathBuf),
}

impl Target {
    /// The target for a person.
    pub fn describe(&self) -> String {
        match self {
            Target::Drive(disk) => {
                let mut text = disk.id.clone();
                if let Some(model) = &disk.model {
                    text.push_str(&format!(" ({model})"));
                }
                text.push_str(&format!(", {}", disk.raw_path));
                text
            }
            Target::File(path) => format!("the file {}", path.display()),
        }
    }
}

/// Find the target that a person names: an image file when the name is a
/// regular file, and a drive of `att lsblk` when it is not.
pub fn find(name: &str) -> Result<Target> {
    let path = Path::new(name);
    if path.is_file() {
        return Ok(Target::File(path.to_path_buf()));
    }
    let list = disks::list()?;
    Ok(Target::Drive(disks::parse::select(&list, name)?.clone()))
}

/// Open the target. A drive opened for a write has no mounted volume while
/// it stays open.
pub fn open(target: &Target, write: bool) -> Result<Box<dyn Blocks>> {
    Ok(match target {
        Target::Drive(disk) => Box::new(host::open(disk, write)?),
        Target::File(path) => Box::new(file::FileBlocks::open(path, write)?),
    })
}

/// An error of an open, with what to do when the host refused access.
fn explain(error: io::Error, what: &str) -> anyhow::Error {
    #[cfg(windows)]
    let denied = host::denied(&error);
    #[cfg(not(windows))]
    let denied = error.kind() == io::ErrorKind::PermissionDenied;
    if denied {
        let how = if cfg!(windows) {
            "Run the command in a terminal that runs as Administrator"
        } else {
            "Run the command with sudo"
        };
        return anyhow::anyhow!("the host refused access to {what}. {how}");
    }
    if error.raw_os_error() == Some(16) && cfg!(unix) {
        return anyhow::anyhow!("{what} is busy. A program or a file system still holds it");
    }
    anyhow::Error::new(error).context(format!("could not open {what}"))
}

/// The disk numbers in a VOLUME_DISK_EXTENTS of Windows: a count of four
/// bytes, four bytes of padding, then an extent of 24 bytes for each disk,
/// which starts with its number.
pub fn parse_extents(bytes: &[u8]) -> Vec<u32> {
    let count = bytes
        .get(0..4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
        .unwrap_or(0);
    (0..count)
        .filter_map(|i| {
            let at = 8 + i * 24;
            let b = bytes.get(at..at + 4)?;
            Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::parse_extents;

    #[test]
    fn extents_give_each_disk_number() {
        let mut bytes = vec![0u8; 8 + 24 * 2];
        bytes[0] = 2;
        bytes[8] = 3;
        bytes[32] = 5;
        assert_eq!(parse_extents(&bytes), [3, 5]);
        assert!(parse_extents(&[]).is_empty());
        let mut short = vec![0u8; 8];
        short[0] = 4;
        assert!(
            parse_extents(&short).is_empty(),
            "a count past the end gives nothing"
        );
    }
}
