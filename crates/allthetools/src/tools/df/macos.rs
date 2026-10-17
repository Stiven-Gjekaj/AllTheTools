//! The host file of `att df` on macOS: getmntinfo, which gives a statfs
//! record for each mounted volume.
//!
//! The volumes of APFS share the space of their container, so the free
//! space of statfs is the free space of the container. The used space of one
//! volume comes from getattrlist and ATTR_VOL_SPACEUSED, as `df` of macOS
//! reads it.

use std::ffi::{CStr, CString, c_char};
use std::path::Path;

use anyhow::{Result, bail};

use super::parse::Volume;

fn text(field: &[c_char]) -> String {
    // SAFETY: the kernel ends each name in the record with a null, inside
    // the array.
    #[allow(unsafe_code)]
    unsafe { CStr::from_ptr(field.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

/// The bytes that the volume at `mount` uses, with no other volume of its
/// container.
fn space_used(mount: &str) -> Option<u64> {
    let path = CString::new(mount).ok()?;
    let mut list = libc::attrlist {
        bitmapcount: libc::ATTR_BIT_MAP_COUNT,
        reserved: 0,
        commonattr: 0,
        volattr: libc::ATTR_VOL_INFO | libc::ATTR_VOL_SPACEUSED,
        dirattr: 0,
        fileattr: 0,
        forkattr: 0,
    };
    // The reply is the length of the reply, then the value.
    #[repr(C, packed(4))]
    struct Reply {
        length: u32,
        used: libc::off_t,
    }
    let mut reply = Reply { length: 0, used: 0 };
    // SAFETY: the path ends with a null, and the reply has the size that
    // the call gets.
    #[allow(unsafe_code)]
    let status = unsafe {
        libc::getattrlist(
            path.as_ptr(),
            (&raw mut list).cast(),
            (&raw mut reply).cast(),
            size_of::<Reply>(),
            0,
        )
    };
    let used = reply.used;
    (status == 0 && used >= 0).then_some(used as u64)
}

pub fn volumes() -> Result<Vec<Volume>> {
    let mut records: *mut libc::statfs = std::ptr::null_mut();
    // SAFETY: getmntinfo points `records` at an array of `count` records
    // that the C library owns, and that stays valid until the next call.
    #[allow(unsafe_code)]
    let list = unsafe {
        let count = libc::getmntinfo(&mut records, libc::MNT_NOWAIT);
        if count <= 0 || records.is_null() {
            bail!(
                "macOS gave no mounted volumes: {}",
                std::io::Error::last_os_error()
            );
        }
        std::slice::from_raw_parts(records, count as usize)
    };
    Ok(list
        .iter()
        .map(|s| {
            let unit = u64::from(s.f_bsize);
            let size = s.f_blocks * unit;
            let mount = text(&s.f_mntonname);
            let used = space_used(&mount).unwrap_or_else(|| size.saturating_sub(s.f_bfree * unit));
            Volume {
                source: text(&s.f_mntfromname),
                fstype: text(&s.f_fstypename),
                mount,
                size,
                used,
                available: s.f_bavail * unit,
            }
        })
        .collect())
}

/// The mount point of the volume that holds `path`, from statfs. The system
/// volume and the data volume of macOS join through firmlinks, so a device
/// number cannot tell them apart, and statfs can.
pub fn mount_of(path: &Path) -> Option<String> {
    let path = CString::new(path.as_os_str().as_encoded_bytes()).ok()?;
    // SAFETY: statfs holds only numbers and arrays, so all zeros is a valid
    // value, and the path ends with a null.
    #[allow(unsafe_code)]
    unsafe {
        let mut record: libc::statfs = std::mem::zeroed();
        if libc::statfs(path.as_ptr(), &mut record) != 0 {
            return None;
        }
        Some(text(&record.f_mntonname))
    }
}
