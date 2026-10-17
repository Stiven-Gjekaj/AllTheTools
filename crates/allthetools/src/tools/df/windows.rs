//! The host file of `att df` on Windows: each drive letter, with
//! GetVolumeInformationW and GetDiskFreeSpaceExW. A drive with no medium,
//! such as an empty card reader, is skipped.

use anyhow::Result;
use windows_sys::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetLogicalDriveStringsW, GetVolumeInformationW,
};

use super::parse::Volume;

fn wide_text(buffer: &[u16]) -> String {
    let end = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..end])
}

pub fn volumes() -> Result<Vec<Volume>> {
    let mut list = vec![0u16; 512];
    // SAFETY: the buffer has room for the length that the call gets.
    #[allow(unsafe_code)]
    let len = unsafe { GetLogicalDriveStringsW(list.len() as u32, list.as_mut_ptr()) } as usize;
    list.truncate(len.min(list.len()));
    let mut out = Vec::new();
    for root in list.split(|c| *c == 0).filter(|r| !r.is_empty()) {
        let mut root: Vec<u16> = root.to_vec();
        let mount = String::from_utf16_lossy(&root);
        root.push(0);
        let mut label = vec![0u16; 261];
        let mut fstype = vec![0u16; 261];
        let (mut available, mut total, mut free) = (0u64, 0u64, 0u64);
        // SAFETY: the root ends with a null, and each buffer has the room
        // that the call gets.
        #[allow(unsafe_code)]
        let ready = unsafe {
            GetVolumeInformationW(
                root.as_ptr(),
                label.as_mut_ptr(),
                label.len() as u32,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                fstype.as_mut_ptr(),
                fstype.len() as u32,
            ) != 0
                && GetDiskFreeSpaceExW(root.as_ptr(), &mut available, &mut total, &mut free) != 0
        };
        if !ready {
            continue;
        }
        let label = wide_text(&label);
        out.push(Volume {
            source: if label.is_empty() {
                mount.trim_end_matches('\\').to_owned()
            } else {
                label
            },
            fstype: wide_text(&fstype),
            mount,
            size: total,
            used: total.saturating_sub(free),
            available,
        });
    }
    Ok(out)
}
