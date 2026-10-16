//! Turn what each host says about its memory into one report.
//!
//! This module makes no system call and names no type of a host crate, so
//! its tests run on every host.

use anyhow::{Result, bail};
use serde::Serialize;

/// The memory and the swap of the host, in bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Memory {
    pub total: u64,
    /// The memory that programs and the system hold, and that the host
    /// cannot give back with no cost.
    pub used: u64,
    /// The memory that holds nothing. A host that does not report it gives
    /// None.
    pub free: Option<u64>,
    /// The memory that holds a copy of files, which the host gives back when
    /// a program needs it.
    pub cache: Option<u64>,
    /// The memory that a new program can get with no swap.
    pub available: u64,
    pub swap_total: u64,
    pub swap_used: u64,
}

/// The lines of /proc/meminfo, as `free` of procps reads them. The values
/// are in KiB.
pub fn linux(meminfo: &str) -> Result<Memory> {
    let value = |key: &str| -> Option<u64> {
        meminfo.lines().find_map(|line| {
            let (name, rest) = line.split_once(':')?;
            if name.trim() != key {
                return None;
            }
            let number: u64 = rest.split_whitespace().next()?.parse().ok()?;
            Some(number * 1024)
        })
    };
    let Some(total) = value("MemTotal") else {
        bail!("/proc/meminfo gives no MemTotal");
    };
    let free = value("MemFree").unwrap_or(0);
    let cache = value("Buffers").unwrap_or(0)
        + value("Cached").unwrap_or(0)
        + value("SReclaimable").unwrap_or(0);
    // A kernel before 3.14 has no MemAvailable. procps then takes the free
    // memory and the cache.
    let available = value("MemAvailable").unwrap_or(free + cache).min(total);
    let swap_total = value("SwapTotal").unwrap_or(0);
    let swap_used = swap_total.saturating_sub(value("SwapFree").unwrap_or(0));
    Ok(Memory {
        total,
        used: total - available,
        free: Some(free),
        cache: Some(cache),
        available,
        swap_total,
        swap_used,
    })
}

/// The page counts of the virtual memory of macOS, from host_statistics64.
#[derive(Debug, Clone, Copy, Default)]
pub struct MachPages {
    pub free: u64,
    pub speculative: u64,
    pub wired: u64,
    pub purgeable: u64,
    pub compressor: u64,
    pub external: u64,
    pub internal: u64,
}

/// The memory of macOS, with the sums that Activity Monitor shows: the used
/// memory is the memory of the apps, the wired memory and the compressed
/// memory, and the cache is the memory of the files and the purgeable
/// memory.
pub fn macos(total: u64, page: u64, pages: MachPages, swap_total: u64, swap_used: u64) -> Memory {
    let apps = pages.internal.saturating_sub(pages.purgeable);
    let used = ((apps + pages.wired + pages.compressor) * page).min(total);
    Memory {
        total,
        used,
        free: Some((pages.free.saturating_sub(pages.speculative)) * page),
        cache: Some((pages.external + pages.purgeable) * page),
        available: total - used,
        swap_total,
        swap_used,
    }
}

/// The memory of Windows, from GetPerformanceInfo and the page files.
/// Windows gives no count of the memory that holds nothing.
pub fn windows(total: u64, available: u64, cache: u64, swap_total: u64, swap_used: u64) -> Memory {
    let available = available.min(total);
    Memory {
        total,
        used: total - available,
        free: None,
        cache: Some(cache),
        available,
        swap_total,
        swap_used,
    }
}

/// The rows of the table for a person. Each cell is a size, in bytes or in
/// powers of two.
pub fn rows(memory: &Memory, bytes: bool) -> Vec<Vec<String>> {
    let size = |n: u64| {
        if bytes {
            n.to_string()
        } else {
            crate::size::binary(n)
        }
    };
    let maybe = |n: Option<u64>| n.map(size).unwrap_or_else(|| "-".to_owned());
    vec![
        vec![
            "Memory".to_owned(),
            size(memory.total),
            size(memory.used),
            maybe(memory.free),
            maybe(memory.cache),
            size(memory.available),
        ],
        vec![
            "Swap".to_owned(),
            size(memory.swap_total),
            size(memory.swap_used),
            size(memory.swap_total.saturating_sub(memory.swap_used)),
        ],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const MEMINFO: &str = "MemTotal:       16303428 kB
MemFree:         1234560 kB
MemAvailable:    9876540 kB
Buffers:          200000 kB
Cached:          6000000 kB
SwapCached:            0 kB
SReclaimable:     500000 kB
SwapTotal:       2097148 kB
SwapFree:        2000000 kB
";

    #[test]
    fn linux_follows_procps() {
        let m = linux(MEMINFO).unwrap();
        assert_eq!(m.total, 16303428 * 1024);
        assert_eq!(m.available, 9876540 * 1024);
        assert_eq!(m.used, (16303428 - 9876540) * 1024);
        assert_eq!(m.free, Some(1234560 * 1024));
        assert_eq!(m.cache, Some((200000 + 6000000 + 500000) * 1024));
        assert_eq!(m.swap_total, 2097148 * 1024);
        assert_eq!(m.swap_used, (2097148 - 2000000) * 1024);
    }

    #[test]
    fn an_old_kernel_with_no_mem_available_uses_free_and_cache() {
        let old: String = MEMINFO
            .lines()
            .filter(|l| !l.starts_with("MemAvailable"))
            .map(|l| format!("{l}\n"))
            .collect();
        let m = linux(&old).unwrap();
        assert_eq!(m.available, (1234560 + 200000 + 6000000 + 500000) * 1024);
    }

    #[test]
    fn meminfo_with_no_total_is_an_error() {
        assert!(linux("MemFree: 10 kB\n").is_err());
    }

    #[test]
    fn macos_follows_activity_monitor() {
        let pages = MachPages {
            free: 29883 + 13838,
            speculative: 13838,
            wired: 175961,
            purgeable: 3568,
            compressor: 278911,
            external: 207121,
            internal: 314924,
        };
        let page = 16384;
        let total = 17_179_869_184;
        let m = macos(total, page, pages, 0, 0);
        let used = (314924 - 3568 + 175961 + 278911) * page;
        assert_eq!(m.used, used);
        assert_eq!(m.available, total - used);
        assert_eq!(m.free, Some(29883 * page));
        assert_eq!(m.cache, Some((207121 + 3568) * page));
    }

    #[test]
    fn used_memory_never_passes_the_total() {
        let pages = MachPages {
            wired: 1000,
            ..MachPages::default()
        };
        let m = macos(4096, 4096, pages, 0, 0);
        assert_eq!(m.used, 4096);
        assert_eq!(m.available, 0);
    }

    #[test]
    fn windows_has_no_free_count() {
        let m = windows(8 << 30, 3 << 30, 1 << 30, 2 << 30, 1 << 29);
        assert_eq!(m.used, 5 << 30);
        assert_eq!(m.free, None);
        let rows = rows(&m, false);
        assert_eq!(rows[0][3], "-");
        assert_eq!(rows[0][1], "8.0 GiB");
        assert_eq!(rows[1][3], "1.5 GiB");
    }

    #[test]
    fn bytes_give_each_size_whole() {
        let m = windows(1000, 400, 0, 0, 0);
        assert_eq!(rows(&m, true)[0][1..3], ["1000", "600"]);
    }
}
