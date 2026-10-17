//! The host file of `att uptime` on macOS: kern.boottime of sysctl, and
//! getloadavg.

use anyhow::Result;

use super::parse::Uptime;
use crate::sysctl;

pub fn read() -> Result<Uptime> {
    let boot: libc::timeval = sysctl::record("kern.boottime")?;
    let boot = boot.tv_sec.max(0) as u64;
    let mut load = [0f64; 3];
    // SAFETY: the array holds the three numbers that the call writes.
    #[allow(unsafe_code)]
    let count = unsafe { libc::getloadavg(load.as_mut_ptr(), 3) };
    Ok(Uptime {
        seconds: super::now().saturating_sub(boot),
        boot,
        load: (count == 3).then_some(load),
    })
}
