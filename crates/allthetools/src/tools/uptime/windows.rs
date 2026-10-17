//! The host file of `att uptime` on Windows: GetTickCount64, the
//! milliseconds since the start. Windows has no load average.

use anyhow::Result;
use windows_sys::Win32::System::SystemInformation::GetTickCount64;

use super::parse::Uptime;

pub fn read() -> Result<Uptime> {
    // SAFETY: GetTickCount64 has no argument and cannot fail.
    #[allow(unsafe_code)]
    let seconds = unsafe { GetTickCount64() } / 1000;
    Ok(Uptime {
        seconds,
        boot: super::now().saturating_sub(seconds),
        load: None,
    })
}
