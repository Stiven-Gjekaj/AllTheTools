//! The host file of `att free` on Windows: GetPerformanceInfo, and the page
//! files that EnumPageFilesW lists.

use anyhow::{Result, bail};
use windows_sys::Win32::System::ProcessStatus::{
    ENUM_PAGE_FILE_INFORMATION, K32EnumPageFilesW, K32GetPerformanceInfo, PERFORMANCE_INFORMATION,
};
use windows_sys::core::{BOOL, PCWSTR};

use super::parse::{self, Memory};

/// The pages of each page file, added up: the size and the pages in use.
#[derive(Default)]
struct PageFiles {
    total: u64,
    used: u64,
}

// SAFETY: Windows calls this with the context that read gives, which points
// to a PageFiles, and with a valid record of one page file.
#[allow(unsafe_code)]
unsafe extern "system" fn add(
    context: *mut core::ffi::c_void,
    info: *mut ENUM_PAGE_FILE_INFORMATION,
    _name: PCWSTR,
) -> BOOL {
    let files = unsafe { &mut *context.cast::<PageFiles>() };
    let info = unsafe { &*info };
    files.total += info.TotalSize as u64;
    files.used += info.TotalInUse as u64;
    1
}

pub fn read() -> Result<Memory> {
    let mut info = PERFORMANCE_INFORMATION {
        cb: size_of::<PERFORMANCE_INFORMATION>() as u32,
        ..Default::default()
    };
    // SAFETY: the record has the size that cb gives.
    #[allow(unsafe_code)]
    if unsafe { K32GetPerformanceInfo(&mut info, info.cb) } == 0 {
        bail!(
            "Windows gave no memory counts: {}",
            std::io::Error::last_os_error()
        );
    }
    let mut files = PageFiles::default();
    // SAFETY: the context points to a PageFiles that lives for the call.
    #[allow(unsafe_code)]
    unsafe {
        K32EnumPageFilesW(Some(add), (&raw mut files).cast());
    }
    let page = info.PageSize as u64;
    Ok(parse::windows(
        info.PhysicalTotal as u64 * page,
        info.PhysicalAvailable as u64 * page,
        info.SystemCache as u64 * page,
        files.total * page,
        files.used * page,
    ))
}
