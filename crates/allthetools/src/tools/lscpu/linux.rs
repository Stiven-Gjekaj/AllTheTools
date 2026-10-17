//! The host file of `att lscpu` on Linux: /proc/cpuinfo, and the topology
//! and the caches of each processor in /sys/devices/system/cpu.

use std::path::Path;

use anyhow::{Context, Result};

use super::parse::{self, Cpu, LinuxCpu};

fn text(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_owned())
}

pub fn read_cpu(dir: &Path) -> LinuxCpu {
    let number = |name: &str| text(&dir.join("topology").join(name)).and_then(|s| s.parse().ok());
    let mut caches = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir.join("cache")) {
        for entry in entries.flatten() {
            let p = entry.path();
            if !entry.file_name().to_string_lossy().starts_with("index") {
                continue;
            }
            if let (Some(level), Some(kind), Some(size), Some(shared)) = (
                text(&p.join("level")).and_then(|s| s.parse().ok()),
                text(&p.join("type")),
                text(&p.join("size")),
                text(&p.join("shared_cpu_list")),
            ) {
                caches.push((level, kind, size, shared));
            }
        }
    }
    LinuxCpu {
        package: number("physical_package_id"),
        core: number("core_id"),
        caches,
    }
}

pub fn read(arch: &str) -> Result<Cpu> {
    let cpuinfo =
        std::fs::read_to_string("/proc/cpuinfo").context("could not read /proc/cpuinfo")?;
    let root = Path::new("/sys/devices/system/cpu");
    let mut dirs: Vec<_> = std::fs::read_dir(root)
        .context("could not read /sys/devices/system/cpu")?
        .flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.strip_prefix("cpu")
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        })
        // A processor that is offline has no topology.
        .filter(|e| e.path().join("topology").exists())
        .map(|e| e.path())
        .collect();
    dirs.sort();
    let cpus: Vec<LinuxCpu> = dirs.iter().map(|d| read_cpu(d)).collect();
    let max_khz = text(&root.join("cpu0/cpufreq/cpuinfo_max_freq")).and_then(|s| s.parse().ok());
    Ok(parse::linux(arch, &cpuinfo, &cpus, max_khz))
}
