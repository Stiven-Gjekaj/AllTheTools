//! The host file of `att whoholds` on Windows: the Restart Manager, which
//! names each process that holds one of the files that it gets. It is the
//! API that an installer uses to ask for a program to close.
//!
//! The Restart Manager takes files and not folders. So a folder or a drive
//! gives each file inside it, up to a limit.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use windows_sys::Win32::Foundation::ERROR_MORE_DATA;
use windows_sys::Win32::System::RestartManager::{
    CCH_RM_SESSION_KEY, RM_PROCESS_INFO, RmEndSession, RmGetList, RmRegisterResources,
    RmStartSession,
};

use super::parse::{self, Holder};

/// The most files that a folder or a drive gives to the Restart Manager.
const LIMIT: usize = 20_000;

struct Session(u32);

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: the session came from RmStartSession and ends one time.
        #[allow(unsafe_code)]
        unsafe {
            RmEndSession(self.0);
        }
    }
}

fn wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn files(target: &Path, out: &mut Vec<PathBuf>) {
    if out.len() >= LIMIT {
        return;
    }
    if target.is_file() {
        out.push(target.to_path_buf());
        return;
    }
    let Ok(entries) = std::fs::read_dir(target) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            files(&entry.path(), out);
        } else if kind.is_file() {
            out.push(entry.path());
        }
        if out.len() >= LIMIT {
            return;
        }
    }
}

pub fn holders(target: &Path) -> Result<Vec<Holder>> {
    let mut list = Vec::new();
    files(target, &mut list);
    if list.len() >= LIMIT {
        eprintln!(
            "att whoholds: {} holds more than {LIMIT} files, and att reads the first {LIMIT} only",
            target.display()
        );
    }
    if list.is_empty() {
        return Ok(Vec::new());
    }

    let mut handle = 0u32;
    let mut key = [0u16; CCH_RM_SESSION_KEY as usize + 1];
    // SAFETY: the key holds CCH_RM_SESSION_KEY characters and a null.
    #[allow(unsafe_code)]
    let status = unsafe { RmStartSession(&mut handle, 0, key.as_mut_ptr()) };
    if status != 0 {
        bail!("Windows refused a session of the Restart Manager (error {status})");
    }
    let session = Session(handle);

    let names: Vec<Vec<u16>> = list.iter().map(|p| wide(p)).collect();
    for chunk in names.chunks(1000) {
        let pointers: Vec<*const u16> = chunk.iter().map(|n| n.as_ptr()).collect();
        // SAFETY: each pointer is a path that ends with a null, and it lives
        // until the call returns.
        #[allow(unsafe_code)]
        let status = unsafe {
            RmRegisterResources(
                session.0,
                pointers.len() as u32,
                pointers.as_ptr(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
            )
        };
        if status != 0 {
            bail!("the Restart Manager refused the files (error {status})");
        }
    }

    let mut infos: Vec<RM_PROCESS_INFO> = Vec::new();
    loop {
        let mut needed = 0u32;
        let mut count = infos.len() as u32;
        let mut reasons = 0u32;
        // SAFETY: the buffer holds `count` entries.
        #[allow(unsafe_code)]
        let status = unsafe {
            RmGetList(
                session.0,
                &mut needed,
                &mut count,
                if infos.is_empty() {
                    std::ptr::null_mut()
                } else {
                    infos.as_mut_ptr()
                },
                &mut reasons,
            )
        };
        if status == ERROR_MORE_DATA {
            // SAFETY: RM_PROCESS_INFO is a plain struct, and zero bytes make
            // a valid one.
            #[allow(unsafe_code)]
            infos.resize(needed as usize, unsafe { std::mem::zeroed() });
            continue;
        }
        if status != 0 {
            bail!("the Restart Manager gave no list (error {status})");
        }
        infos.truncate(count as usize);
        break;
    }

    Ok(infos
        .iter()
        .map(|info| {
            let end = info
                .strAppName
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(info.strAppName.len());
            Holder {
                pid: info.Process.dwProcessId,
                process: String::from_utf16_lossy(&info.strAppName[..end]),
                how: parse::windows_how(info.ApplicationType).to_owned(),
                path: None,
            }
        })
        .collect())
}
