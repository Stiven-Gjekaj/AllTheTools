//! The host file of `att unquarantine` on Linux: the extended attributes
//! `user.xdg.origin.url` and `user.xdg.referrer.url`.
//!
//! Linux does not stop a file that has these marks. They only say where the
//! file came from.

use std::ffi::{CStr, CString};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use super::parse::{self, Mark};

const ORIGIN: &CStr = c"user.xdg.origin.url";
const REFERRER: &CStr = c"user.xdg.referrer.url";

fn c_path(path: &Path) -> io::Result<CString> {
    CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))
}

/// True for the errors that mean "this file has no such attribute".
fn absent(error: &io::Error) -> bool {
    matches!(
        error.raw_os_error(),
        Some(libc::ENODATA) | Some(libc::ENOTSUP)
    )
}

pub fn read(path: &Path) -> io::Result<Option<Mark>> {
    let path = c_path(path)?;
    for name in [ORIGIN, REFERRER] {
        let mut buffer = vec![0u8; 4096];
        // SAFETY: both strings end with a null, and the buffer holds the
        // length that the call receives.
        #[allow(unsafe_code)]
        let size = unsafe {
            libc::getxattr(
                path.as_ptr(),
                name.as_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
            )
        };
        if size >= 0 {
            buffer.truncate(size as usize);
            return Ok(Some(parse::xdg_origin(&buffer)));
        }
        let error = io::Error::last_os_error();
        if !absent(&error) {
            return Err(error);
        }
    }
    Ok(None)
}

pub fn remove(path: &Path) -> io::Result<bool> {
    let path = c_path(path)?;
    let mut removed = false;
    for name in [ORIGIN, REFERRER] {
        // SAFETY: both strings end with a null.
        #[allow(unsafe_code)]
        let result = unsafe { libc::removexattr(path.as_ptr(), name.as_ptr()) };
        if result == 0 {
            removed = true;
            continue;
        }
        let error = io::Error::last_os_error();
        if !absent(&error) {
            return Err(error);
        }
    }
    Ok(removed)
}
