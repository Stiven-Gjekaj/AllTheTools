//! The host file of `att free` on macOS: the page counts of the Mach kernel,
//! and the sysctl values of the memory size and the swap.

use std::ffi::CStr;

use anyhow::{Result, bail};

use super::parse::{self, MachPages, Memory};

/// Read one sysctl value of a fixed size by its name.
fn sysctl<T: Default>(name: &CStr) -> Result<T> {
    let mut value = T::default();
    let mut len = size_of::<T>();
    // SAFETY: the name ends with a null, and the value has room for len
    // bytes.
    #[allow(unsafe_code)]
    let status = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            (&raw mut value).cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if status != 0 || len != size_of::<T>() {
        bail!("the host gave no value for {}", name.to_string_lossy());
    }
    Ok(value)
}

pub fn read() -> Result<Memory> {
    let total: u64 = sysctl(c"hw.memsize")?;
    let page: u64 = sysctl::<libc::c_int>(c"hw.pagesize")? as u64;
    // SAFETY: xsw_usage holds only numbers, so all zeros is a valid value.
    #[allow(unsafe_code)]
    let swap: libc::xsw_usage = {
        let mut swap = unsafe { std::mem::zeroed::<libc::xsw_usage>() };
        let mut len = size_of::<libc::xsw_usage>();
        // SAFETY: the name ends with a null, and the value has room for len
        // bytes.
        let status = unsafe {
            libc::sysctlbyname(
                c"vm.swapusage".as_ptr(),
                (&raw mut swap).cast(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        };
        if status != 0 {
            bail!("the host gave no value for vm.swapusage");
        }
        swap
    };

    // SAFETY: vm_statistics64 holds only numbers, so all zeros is a valid
    // value. The count tells the kernel how many integers it can write.
    #[allow(unsafe_code, deprecated)]
    let stats = unsafe {
        let mut stats = std::mem::zeroed::<libc::vm_statistics64>();
        let mut count = libc::HOST_VM_INFO64_COUNT;
        let status = libc::host_statistics64(
            libc::mach_host_self(),
            libc::HOST_VM_INFO64,
            (&raw mut stats).cast(),
            &mut count,
        );
        if status != libc::KERN_SUCCESS {
            bail!("the kernel gave no page counts ({status})");
        }
        stats
    };
    let pages = MachPages {
        free: u64::from(stats.free_count),
        speculative: u64::from(stats.speculative_count),
        wired: u64::from(stats.wire_count),
        purgeable: u64::from(stats.purgeable_count),
        compressor: u64::from(stats.compressor_page_count),
        external: u64::from(stats.external_page_count),
        internal: u64::from(stats.internal_page_count),
    };
    Ok(parse::macos(
        total,
        page,
        pages,
        swap.xsu_total,
        swap.xsu_used,
    ))
}
