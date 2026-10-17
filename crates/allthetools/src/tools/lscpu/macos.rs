//! The host file of `att lscpu` on macOS: the hw and machdep values of
//! sysctl.

use anyhow::Result;

use super::parse::{self, Cpu, MacInput, MacLevel};
use crate::sysctl;

fn number(name: &str) -> Option<u64> {
    sysctl::number::<u64>(name)
        .ok()
        .or_else(|| sysctl::number::<u32>(name).ok().map(u64::from))
}

pub fn read(arch: &str) -> Result<Cpu> {
    let levels = number("hw.nperflevels").unwrap_or(0);
    let levels = (0..levels)
        .map(|n| {
            let key = |name: &str| format!("hw.perflevel{n}.{name}");
            MacLevel {
                name: sysctl::text(&key("name")).unwrap_or_else(|_| format!("Level {n}")),
                cores: number(&key("physicalcpu")).unwrap_or(0) as u32,
                l1d: number(&key("l1dcachesize")).unwrap_or(0),
                l1i: number(&key("l1icachesize")).unwrap_or(0),
                l2: number(&key("l2cachesize")).unwrap_or(0),
                cores_per_l2: number(&key("cpusperl2")).unwrap_or(1) as u32,
            }
        })
        .collect();
    let mut features: Vec<String> = Vec::new();
    for name in ["machdep.cpu.features", "machdep.cpu.leaf7_features"] {
        if let Ok(text) = sysctl::text(name) {
            features.extend(text.split_whitespace().map(|f| f.to_lowercase()));
        }
    }
    Ok(parse::macos(
        arch,
        MacInput {
            brand: sysctl::text("machdep.cpu.brand_string").ok(),
            vendor: sysctl::text("machdep.cpu.vendor").ok(),
            packages: number("hw.packages").map(|n| n as u32),
            physical: number("hw.physicalcpu").unwrap_or(0) as u32,
            logical: number("hw.logicalcpu").unwrap_or(0) as u32,
            max_hz: number("hw.cpufrequency_max"),
            levels,
            l1d: number("hw.l1dcachesize"),
            l1i: number("hw.l1icachesize"),
            l2: number("hw.l2cachesize"),
            l3: number("hw.l3cachesize"),
            features,
        },
    ))
}
