//! The host file of `att trash` on Windows: SHFileOperationW with the flag
//! that allows an undo, which sends each file to the Recycle Bin.
//!
//! A volume with no Recycle Bin, such as a network share, cannot keep the
//! file. Windows then asks the person before it deletes the file for good,
//! because of FOF_WANTNUKEWARNING.

use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use anyhow::{Result, bail};
use windows_sys::Win32::UI::Shell::{
    FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, FOF_WANTNUKEWARNING,
    SHFILEOPSTRUCTW, SHFileOperationW,
};

pub fn trash(path: &Path) -> Result<()> {
    // The list of paths ends with two nulls: one for the path, and one for
    // the list.
    let mut from: Vec<u16> = path.as_os_str().encode_wide().collect();
    from.extend([0, 0]);
    let mut op = SHFILEOPSTRUCTW {
        hwnd: std::ptr::null_mut(),
        wFunc: FO_DELETE,
        pFrom: from.as_ptr(),
        pTo: std::ptr::null(),
        fFlags: (FOF_ALLOWUNDO
            | FOF_NOCONFIRMATION
            | FOF_WANTNUKEWARNING
            | FOF_SILENT
            | FOF_NOERRORUI) as u16,
        fAnyOperationsAborted: 0,
        hNameMappings: std::ptr::null_mut(),
        lpszProgressTitle: std::ptr::null(),
    };
    // SAFETY: the record points to a list of paths that lives for the call.
    #[allow(unsafe_code)]
    let status = unsafe { SHFileOperationW(&mut op) };
    if status != 0 {
        bail!("Windows refused it (code {status:#x})");
    }
    if op.fAnyOperationsAborted != 0 {
        bail!("the person stopped it");
    }
    Ok(())
}
