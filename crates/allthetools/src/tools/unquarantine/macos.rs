//! The host file of `att unquarantine` on macOS: the extended attribute
//! `com.apple.quarantine`.

use std::ffi::{CStr, CString};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use super::parse::{self, Mark};

const NAME: &CStr = c"com.apple.quarantine";

fn c_path(path: &Path) -> io::Result<CString> {
    CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))
}

pub fn read(path: &Path) -> io::Result<Option<Mark>> {
    let path = c_path(path)?;
    let mut buffer = vec![0u8; 1024];
    // SAFETY: both strings end with a null, and the buffer holds the length
    // that the call receives.
    #[allow(unsafe_code)]
    let size = unsafe {
        libc::getxattr(
            path.as_ptr(),
            NAME.as_ptr(),
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            0,
            0,
        )
    };
    if size < 0 {
        let error = io::Error::last_os_error();
        return match error.raw_os_error() {
            Some(libc::ENOATTR) | Some(libc::ENOTSUP) => Ok(None),
            _ => Err(error),
        };
    }
    buffer.truncate(size as usize);
    Ok(Some(parse::apple(&buffer)))
}

pub fn remove(path: &Path) -> io::Result<bool> {
    let path = c_path(path)?;
    // SAFETY: both strings end with a null.
    #[allow(unsafe_code)]
    let result = unsafe { libc::removexattr(path.as_ptr(), NAME.as_ptr(), 0) };
    if result == 0 {
        return Ok(true);
    }
    let error = io::Error::last_os_error();
    match error.raw_os_error() {
        Some(libc::ENOATTR) | Some(libc::ENOTSUP) => Ok(false),
        _ => Err(error),
    }
}
