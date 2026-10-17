//! The host file of `att free` on macOS: the page counts of the Mach kernel,
//! and the sysctl values of the memory size and the swap.

use anyhow::{Result, bail};

use super::parse::{self, MachPages, Memory};
use crate::sysctl;

pub fn read() -> Result<Memory> {
    let total: u64 = sysctl::number("hw.memsize")?;
    let page = sysctl::number::<libc::c_int>("hw.pagesize")? as u64;
    let swap: libc::xsw_usage = sysctl::record("vm.swapusage")?;

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
