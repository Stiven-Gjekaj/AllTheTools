//! The host file of `att df` on Linux: /proc/self/mounts, and statvfs of
//! each mount point.

use std::ffi::CString;

use anyhow::{Context, Result};

use super::parse::{self, Volume};

pub fn volumes() -> Result<Vec<Volume>> {
    let table =
        std::fs::read_to_string("/proc/self/mounts").context("could not read /proc/self/mounts")?;
    let mut out = Vec::new();
    for line in table.lines() {
        let Some((source, mount, fstype)) = parse::mount_line(line) else {
            continue;
        };
        let Ok(path) = CString::new(mount.as_str()) else {
            continue;
        };
        // SAFETY: statvfs holds only numbers, so all zeros is a valid value,
        // and the path ends with a null.
        #[allow(unsafe_code)]
        let stat = unsafe {
            let mut stat: libc::statvfs = std::mem::zeroed();
            if libc::statvfs(path.as_ptr(), &mut stat) != 0 {
                // A mount point that this user cannot reach is skipped, as
                // df skips it.
                continue;
            }
            stat
        };
        let unit = stat.f_frsize as u64;
        let size = stat.f_blocks as u64 * unit;
        out.push(Volume {
            source,
            fstype,
            mount,
            size,
            used: size.saturating_sub(stat.f_bfree as u64 * unit),
            available: stat.f_bavail as u64 * unit,
        });
    }
    Ok(out)
}
