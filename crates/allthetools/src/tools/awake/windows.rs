//! The host file of `att awake` on Windows: the execution state of the
//! thread, which PowerToys Awake also sets.

use anyhow::{Result, bail};
use windows_sys::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
use windows_sys::Win32::System::Power::{
    ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED, SetThreadExecutionState,
};
use windows_sys::Win32::System::Threading::{
    GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};

/// The execution state of this thread. It ends when this goes out of scope,
/// and Windows ends it too when the thread ends. So the thread that holds it
/// is the thread that waits.
pub struct Guard;

impl Drop for Guard {
    fn drop(&mut self) {
        // SAFETY: SetThreadExecutionState has no precondition.
        #[allow(unsafe_code)]
        unsafe {
            SetThreadExecutionState(ES_CONTINUOUS);
        }
    }
}

pub fn hold(display: bool) -> Result<Guard> {
    let mut flags = ES_CONTINUOUS | ES_SYSTEM_REQUIRED;
    if display {
        flags |= ES_DISPLAY_REQUIRED;
    }
    // SAFETY: SetThreadExecutionState has no precondition.
    #[allow(unsafe_code)]
    let previous = unsafe { SetThreadExecutionState(flags) };
    if previous == 0 {
        bail!("Windows refused the execution state");
    }
    Ok(Guard)
}

pub fn alive(pid: u32) -> bool {
    // SAFETY: the handle is checked, and closed one time.
    #[allow(unsafe_code)]
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        let mut code = 0u32;
        let ok = GetExitCodeProcess(handle, &mut code) != 0;
        CloseHandle(handle);
        ok && code == STILL_ACTIVE as u32
    }
}
