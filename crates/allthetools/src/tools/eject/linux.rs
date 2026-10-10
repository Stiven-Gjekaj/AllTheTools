//! The host file of `att eject` on Linux.
//!
//! A desktop has udisks, and `udisksctl` unmounts and powers off a drive
//! with no root. With no udisks, root unmounts each partition and asks the
//! kernel to remove the drive.

use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;

use anyhow::{Context, Result, bail};

use super::parse::Plan;
use crate::host;

pub fn eject(plan: &Plan) -> Result<()> {
    if host::on_path("udisksctl") {
        for (path, _) in &plan.mounted {
            host::output("udisksctl", &["unmount", "--block-device", path], None)?;
        }
        host::output(
            "udisksctl",
            &["power-off", "--block-device", &plan.disk.path],
            None,
        )?;
        return Ok(());
    }

    // SAFETY: geteuid has no precondition.
    #[allow(unsafe_code)]
    if unsafe { libc::geteuid() } != 0 {
        bail!("this host has no udisksctl. Run att eject as root, or install the package udisks2");
    }
    for (_, mount) in &plan.mounted {
        let point = CString::new(std::path::Path::new(mount).as_os_str().as_bytes())?;
        // SAFETY: the path ends with a null.
        #[allow(unsafe_code)]
        let result = unsafe { libc::umount2(point.as_ptr(), 0) };
        if result != 0 {
            return Err(std::io::Error::last_os_error())
                .with_context(|| format!("could not unmount {mount}"));
        }
    }
    // SAFETY: sync has no precondition.
    #[allow(unsafe_code)]
    unsafe {
        libc::sync()
    };
    let delete = format!("/sys/block/{}/device/delete", plan.disk.id);
    std::fs::write(&delete, "1").with_context(|| format!("could not write {delete}"))?;
    Ok(())
}
