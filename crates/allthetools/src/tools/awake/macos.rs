//! The host file of `att awake` on macOS: a power assertion of IOKit, the
//! same call that `caffeinate` makes.

use anyhow::{Result, bail};
use core_foundation::base::TCFType;
use core_foundation::string::{CFString, CFStringRef};

// The power management calls of IOKit. No crate binds them, so they are
// declared here, as Apple gives them in IOKit/pwr_mgt/IOPMLib.h.
#[allow(unsafe_code)]
#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOPMAssertionCreateWithName(
        kind: CFStringRef,
        level: u32,
        name: CFStringRef,
        id: *mut u32,
    ) -> i32;
    fn IOPMAssertionRelease(id: u32) -> i32;
}

/// kIOPMAssertionLevelOn.
const LEVEL_ON: u32 = 255;

/// The assertions that att awake holds. Each one ends when this goes out of
/// scope, and macOS ends them too when the process ends.
pub struct Guard(Vec<u32>);

impl Drop for Guard {
    fn drop(&mut self) {
        for id in &self.0 {
            // SAFETY: each id came from IOPMAssertionCreateWithName.
            #[allow(unsafe_code)]
            unsafe {
                IOPMAssertionRelease(*id);
            }
        }
    }
}

pub fn hold(display: bool) -> Result<Guard> {
    let mut kinds = vec!["PreventUserIdleSystemSleep"];
    if display {
        kinds.push("PreventUserIdleDisplaySleep");
    }
    let name = CFString::new("att awake");
    let mut guard = Guard(Vec::new());
    for kind in kinds {
        let kind = CFString::new(kind);
        let mut id = 0u32;
        // SAFETY: both strings are valid CFString objects, and id is a
        // valid place for the result.
        #[allow(unsafe_code)]
        let status = unsafe {
            IOPMAssertionCreateWithName(
                kind.as_concrete_TypeRef(),
                LEVEL_ON,
                name.as_concrete_TypeRef(),
                &mut id,
            )
        };
        if status != 0 {
            bail!("macOS refused the power assertion (IOReturn {status:#x})");
        }
        guard.0.push(id);
    }
    Ok(guard)
}

pub fn alive(pid: u32) -> bool {
    super::unix_alive(pid)
}
