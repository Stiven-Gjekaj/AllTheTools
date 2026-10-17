//! Turn what each host says about its processor into one report.
//!
//! This module makes no system call and names no type of a host crate, so
//! its tests run on every host.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

/// A kind of core, on a processor that has more than one, such as the
/// performance and the efficiency cores of Apple and Intel.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct Kind {
    pub name: String,
    pub cores: u32,
}

/// One group of the same caches.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct Cache {
    pub level: u8,
    /// `data`, `instruction` or `unified`.
    pub kind: String,
    /// The size of one cache, in bytes.
    pub size: u64,
    /// The count of caches of this size.
    pub count: u32,
}

/// The processor of the host.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct Cpu {
    pub architecture: String,
    pub model: Option<String>,
    pub vendor: Option<String>,
    pub sockets: Option<u32>,
    pub cores: u32,
    pub threads: u32,
    pub kinds: Vec<Kind>,
    pub max_mhz: Option<u32>,
    pub caches: Vec<Cache>,
    pub flags: Vec<String>,
}

fn kind_label(kind: &str) -> &str {
    match kind {
        "data" => "d",
        "instruction" => "i",
        _ => "",
    }
}

/// Put caches of the same level, kind and size into one group, in the order
/// L1d, L1i, L2, L3.
pub fn group(caches: impl IntoIterator<Item = (u8, String, u64)>) -> Vec<Cache> {
    let mut counts: BTreeMap<(u8, u8, String, std::cmp::Reverse<u64>), u32> = BTreeMap::new();
    for (level, kind, size) in caches {
        let order = match kind.as_str() {
            "data" => 0,
            "instruction" => 1,
            _ => 2,
        };
        *counts
            .entry((level, order, kind, std::cmp::Reverse(size)))
            .or_insert(0) += 1;
    }
    counts
        .into_iter()
        .map(|((level, _, kind, size), count)| Cache {
            level,
            kind,
            size: size.0,
            count,
        })
        .collect()
}

impl Cpu {
    /// The lines of the report for a person.
    pub fn lines(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let mut push = |k: &str, v: String| out.push((k.to_owned(), v));
        push("Architecture", self.architecture.clone());
        if let Some(model) = &self.model {
            push("Model", model.clone());
        }
        if let Some(vendor) = &self.vendor {
            push("Vendor", vendor.clone());
        }
        if let Some(sockets) = self.sockets {
            push("Sockets", sockets.to_string());
        }
        let kinds = if self.kinds.len() > 1 {
            let parts: Vec<String> = self
                .kinds
                .iter()
                .map(|k| format!("{} {}", k.cores, k.name.to_lowercase()))
                .collect();
            format!(" ({})", parts.join(", "))
        } else {
            String::new()
        };
        push("Cores", format!("{}{kinds}", self.cores));
        push("Threads", self.threads.to_string());
        if let Some(mhz) = self.max_mhz {
            push("Max speed", format!("{:.2} GHz", f64::from(mhz) / 1000.0));
        }
        let mut levels: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for cache in &self.caches {
            let name = format!("L{}{}", cache.level, kind_label(&cache.kind));
            levels.entry(name).or_default().push(format!(
                "{} x {}",
                crate::size::binary(cache.size),
                cache.count
            ));
        }
        // L1d sorts before L1i, and both before L2.
        for (name, parts) in levels {
            push(&format!("{name} cache"), parts.join(", "));
        }
        if !self.flags.is_empty() {
            push("Flags", self.flags.join(" "));
        }
        out
    }
}

/// A size of the caches of Linux sysfs, such as `32K` or `8M`.
pub fn sysfs_size(text: &str) -> Option<u64> {
    let text = text.trim();
    let (number, scale) = match text.chars().last()? {
        'K' => (&text[..text.len() - 1], 1024),
        'M' => (&text[..text.len() - 1], 1024 * 1024),
        'G' => (&text[..text.len() - 1], 1024 * 1024 * 1024),
        _ => (text, 1),
    };
    Some(number.parse::<u64>().ok()? * scale)
}

/// One logical processor of Linux, from /sys/devices/system/cpu.
#[derive(Debug, Clone, Default)]
pub struct LinuxCpu {
    pub package: Option<u32>,
    pub core: Option<u32>,
    /// The level, the type, the size and the list of processors that share
    /// each cache.
    pub caches: Vec<(u8, String, String, String)>,
}

/// The names that the kernel gives to the makers and the parts of Arm
/// cores, as /proc/cpuinfo has numbers only.
fn arm_name(implementer: &str, part: &str) -> (Option<String>, Option<String>) {
    let maker = match implementer {
        "0x41" => Some("ARM"),
        "0x42" => Some("Broadcom"),
        "0x48" => Some("HiSilicon"),
        "0x4e" => Some("NVIDIA"),
        "0x51" => Some("Qualcomm"),
        "0x61" => Some("Apple"),
        "0xc0" => Some("Ampere"),
        _ => None,
    };
    let model = match (implementer, part) {
        ("0x41", "0xd03") => Some("Cortex-A53"),
        ("0x41", "0xd05") => Some("Cortex-A55"),
        ("0x41", "0xd07") => Some("Cortex-A57"),
        ("0x41", "0xd08") => Some("Cortex-A72"),
        ("0x41", "0xd0b") => Some("Cortex-A76"),
        ("0x41", "0xd0c") => Some("Neoverse-N1"),
        ("0x41", "0xd40") => Some("Neoverse-V1"),
        ("0x41", "0xd41") => Some("Cortex-A78"),
        ("0x41", "0xd49") => Some("Neoverse-N2"),
        ("0x41", "0xd4f") => Some("Neoverse-V2"),
        ("0xc0", "0xac3") => Some("Ampere-1"),
        _ => None,
    };
    (maker.map(str::to_owned), model.map(str::to_owned))
}

/// The processor of Linux, from /proc/cpuinfo and from sysfs.
pub fn linux(arch: &str, cpuinfo: &str, cpus: &[LinuxCpu], max_khz: Option<u64>) -> Cpu {
    let field = |key: &str| -> Option<String> {
        cpuinfo.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            (name.trim() == key)
                .then(|| value.trim().to_owned())
                .filter(|v| !v.is_empty())
        })
    };
    let mut model = field("model name").or_else(|| field("Model"));
    let mut vendor = field("vendor_id");
    if let (Some(implementer), Some(part)) = (field("CPU implementer"), field("CPU part")) {
        let (maker, part_name) = arm_name(&implementer, &part);
        vendor = vendor.or(maker);
        model = model.or(part_name);
    }
    let flags = field("flags")
        .or_else(|| field("Features"))
        .map(|f| f.split_whitespace().map(str::to_owned).collect())
        .unwrap_or_default();
    let packages: BTreeSet<u32> = cpus.iter().filter_map(|c| c.package).collect();
    let cores: BTreeSet<(Option<u32>, Option<u32>)> = cpus
        .iter()
        .enumerate()
        .map(|(i, c)| (c.package, c.core.or(Some(i as u32))))
        .collect();
    // Each cache counts once for each set of processors that share it.
    let mut seen = BTreeSet::new();
    let mut caches = Vec::new();
    for cpu in cpus {
        for (level, kind, size, shared) in &cpu.caches {
            let kind = kind.to_lowercase();
            if seen.insert((*level, kind.clone(), shared.clone()))
                && let Some(size) = sysfs_size(size)
            {
                caches.push((*level, kind, size));
            }
        }
    }
    Cpu {
        architecture: arch.to_owned(),
        model,
        vendor,
        sockets: (!packages.is_empty()).then_some(packages.len() as u32),
        cores: cores.len() as u32,
        threads: cpus.len() as u32,
        kinds: Vec::new(),
        max_mhz: max_khz.map(|k| (k / 1000) as u32),
        caches: group(caches),
        flags,
    }
}

/// What macOS gives through sysctl.
#[derive(Debug, Clone, Default)]
pub struct MacInput {
    pub brand: Option<String>,
    pub vendor: Option<String>,
    pub packages: Option<u32>,
    pub physical: u32,
    pub logical: u32,
    pub max_hz: Option<u64>,
    /// For each level of performance: its name, its cores, and the sizes of
    /// its L1d, L1i and L2 caches, and the cores that share one L2.
    pub levels: Vec<MacLevel>,
    /// The sizes of the caches when the host has one level: L1d, L1i, L2
    /// and L3.
    pub l1d: Option<u64>,
    pub l1i: Option<u64>,
    pub l2: Option<u64>,
    pub l3: Option<u64>,
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct MacLevel {
    pub name: String,
    pub cores: u32,
    pub l1d: u64,
    pub l1i: u64,
    pub l2: u64,
    pub cores_per_l2: u32,
}

/// The processor of macOS.
pub fn macos(arch: &str, input: MacInput) -> Cpu {
    let mut caches = Vec::new();
    let mut kinds = Vec::new();
    if input.levels.is_empty() {
        let cores = input.physical;
        if let Some(size) = input.l1d {
            caches.extend((0..cores).map(|_| (1, "data".to_owned(), size)));
        }
        if let Some(size) = input.l1i {
            caches.extend((0..cores).map(|_| (1, "instruction".to_owned(), size)));
        }
        if let Some(size) = input.l2 {
            caches.extend((0..cores).map(|_| (2, "unified".to_owned(), size)));
        }
        if let Some(size) = input.l3 {
            caches.push((3, "unified".to_owned(), size));
        }
    } else {
        for level in &input.levels {
            kinds.push(Kind {
                name: level.name.clone(),
                cores: level.cores,
            });
            for _ in 0..level.cores {
                caches.push((1, "data".to_owned(), level.l1d));
                caches.push((1, "instruction".to_owned(), level.l1i));
            }
            let l2_count = level.cores.div_ceil(level.cores_per_l2.max(1));
            for _ in 0..l2_count {
                caches.push((2, "unified".to_owned(), level.l2));
            }
        }
    }
    Cpu {
        architecture: arch.to_owned(),
        model: input.brand,
        vendor: input
            .vendor
            .or_else(|| (arch == "aarch64").then(|| "Apple".to_owned())),
        sockets: input.packages,
        cores: input.physical,
        threads: input.logical,
        kinds,
        max_mhz: input.max_hz.map(|hz| (hz / 1_000_000) as u32),
        caches: group(caches.into_iter().filter(|c| c.2 > 0)),
        flags: input.features,
    }
}

/// The records of GetLogicalProcessorInformationEx of Windows, read from
/// its buffer of bytes: the cores with their logical processors and their
/// class of efficiency, the packages, and the caches.
pub fn windows(
    arch: &str,
    buffer: &[u8],
    model: Option<String>,
    vendor: Option<String>,
    mhz: Option<u32>,
) -> Cpu {
    let u16_at = |b: &[u8], at: usize| b.get(at..at + 2).map(|s| u16::from_le_bytes([s[0], s[1]]));
    let u32_at = |b: &[u8], at: usize| {
        b.get(at..at + 4)
            .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    };
    let u64_at = |b: &[u8], at: usize| {
        b.get(at..at + 8)
            .map(|s| u64::from_le_bytes(s.try_into().expect("8 bytes")))
    };
    // The logical processors of a group mask list: the count of set bits.
    let logical = |b: &[u8], count_at: usize, masks_at: usize| -> u32 {
        let count = u16_at(b, count_at).unwrap_or(0) as usize;
        (0..count)
            .filter_map(|i| u64_at(b, masks_at + i * 16))
            .map(u64::count_ones)
            .sum()
    };
    let mut at = 0;
    let (mut cores, mut threads, mut packages) = (0u32, 0u32, 0u32);
    let mut classes: BTreeMap<u8, u32> = BTreeMap::new();
    let mut caches = Vec::new();
    while let (Some(relation), Some(size)) = (u32_at(buffer, at), u32_at(buffer, at + 4)) {
        let size = size as usize;
        if size < 8 || at + size > buffer.len() {
            break;
        }
        let record = &buffer[at..at + size];
        match relation {
            // A core: flags, the class of efficiency, then its masks.
            0 => {
                cores += 1;
                threads += logical(record, 30, 32);
                *classes
                    .entry(record.get(9).copied().unwrap_or(0))
                    .or_insert(0) += 1;
            }
            // A cache: level, associativity, line size, size, type.
            2 => {
                let level = record.get(8).copied().unwrap_or(0);
                let bytes = u64::from(u32_at(record, 12).unwrap_or(0));
                let kind = match u32_at(record, 16).unwrap_or(0) {
                    1 => "instruction",
                    2 => "data",
                    _ => "unified",
                };
                caches.push((level, kind.to_owned(), bytes));
            }
            3 => packages += 1,
            _ => {}
        }
        at += size;
    }
    // Windows gives a higher class to a faster core.
    let kinds = if classes.len() > 1 {
        let top = *classes.keys().last().unwrap_or(&0);
        let (mut fast, mut slow) = (0, 0);
        for (class, count) in classes {
            if class == top {
                fast += count;
            } else {
                slow += count;
            }
        }
        vec![
            Kind {
                name: "Performance".to_owned(),
                cores: fast,
            },
            Kind {
                name: "Efficiency".to_owned(),
                cores: slow,
            },
        ]
    } else {
        Vec::new()
    };
    Cpu {
        architecture: arch.to_owned(),
        model,
        vendor,
        sockets: (packages > 0).then_some(packages),
        cores,
        threads,
        kinds,
        max_mhz: mhz,
        caches: group(caches),
        flags: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_apple_m5_gives_its_kinds_and_caches() {
        // The values that an Apple M5 gives through sysctl.
        let input = MacInput {
            brand: Some("Apple M5".to_owned()),
            packages: Some(1),
            physical: 10,
            logical: 10,
            levels: vec![
                MacLevel {
                    name: "Super".to_owned(),
                    cores: 4,
                    l1d: 131072,
                    l1i: 196608,
                    l2: 16777216,
                    cores_per_l2: 4,
                },
                MacLevel {
                    name: "Efficiency".to_owned(),
                    cores: 6,
                    l1d: 65536,
                    l1i: 131072,
                    l2: 6291456,
                    cores_per_l2: 6,
                },
            ],
            ..MacInput::default()
        };
        let cpu = macos("aarch64", input);
        assert_eq!(cpu.vendor.as_deref(), Some("Apple"));
        let lines = cpu.lines();
        let get = |k: &str| {
            lines
                .iter()
                .find(|(key, _)| key == k)
                .map(|(_, v)| v.clone())
        };
        assert_eq!(get("Cores").as_deref(), Some("10 (4 super, 6 efficiency)"));
        assert_eq!(
            get("L1d cache").as_deref(),
            Some("128.0 KiB x 4, 64.0 KiB x 6")
        );
        assert_eq!(
            get("L2 cache").as_deref(),
            Some("16.0 MiB x 1, 6.0 MiB x 1")
        );
        assert_eq!(get("Max speed"), None);
    }

    const CPUINFO_X86: &str = "processor\t: 0
vendor_id\t: GenuineIntel
model name\t: Intel(R) Xeon(R) Platinum 8370C CPU @ 2.80GHz
flags\t\t: fpu vme sse sse2 avx2
";

    fn x86_cpus() -> Vec<LinuxCpu> {
        // Two cores with two threads each. Each core has its own L1 and L2,
        // and the four threads share one L3.
        (0..4u32)
            .map(|i| {
                let core = i / 2;
                let pair = format!("{}-{}", core * 2, core * 2 + 1);
                LinuxCpu {
                    package: Some(0),
                    core: Some(core),
                    caches: vec![
                        (1, "Data".to_owned(), "48K".to_owned(), pair.clone()),
                        (1, "Instruction".to_owned(), "32K".to_owned(), pair.clone()),
                        (2, "Unified".to_owned(), "1280K".to_owned(), pair),
                        (3, "Unified".to_owned(), "48M".to_owned(), "0-3".to_owned()),
                    ],
                }
            })
            .collect()
    }

    #[test]
    fn linux_counts_cores_threads_and_shared_caches() {
        let cpu = linux("x86_64", CPUINFO_X86, &x86_cpus(), Some(3_500_000));
        assert_eq!((cpu.sockets, cpu.cores, cpu.threads), (Some(1), 2, 4));
        assert_eq!(cpu.vendor.as_deref(), Some("GenuineIntel"));
        assert_eq!(cpu.max_mhz, Some(3500));
        assert_eq!(cpu.flags, ["fpu", "vme", "sse", "sse2", "avx2"]);
        assert_eq!(
            cpu.caches,
            [
                Cache {
                    level: 1,
                    kind: "data".to_owned(),
                    size: 48 * 1024,
                    count: 2
                },
                Cache {
                    level: 1,
                    kind: "instruction".to_owned(),
                    size: 32 * 1024,
                    count: 2
                },
                Cache {
                    level: 2,
                    kind: "unified".to_owned(),
                    size: 1280 * 1024,
                    count: 2
                },
                Cache {
                    level: 3,
                    kind: "unified".to_owned(),
                    size: 48 << 20,
                    count: 1
                },
            ]
        );
    }

    #[test]
    fn linux_on_arm_names_the_part() {
        let info =
            "processor\t: 0\nCPU implementer\t: 0x41\nCPU part\t: 0xd0c\nFeatures\t: fp asimd\n";
        let cpu = linux("aarch64", info, &[LinuxCpu::default()], None);
        assert_eq!(cpu.vendor.as_deref(), Some("ARM"));
        assert_eq!(cpu.model.as_deref(), Some("Neoverse-N1"));
        assert_eq!(cpu.flags, ["fp", "asimd"]);
        assert_eq!((cpu.cores, cpu.threads, cpu.sockets), (1, 1, None));
    }

    #[test]
    fn sysfs_sizes_are_read() {
        assert_eq!(sysfs_size("32K"), Some(32768));
        assert_eq!(sysfs_size("8M\n"), Some(8 << 20));
        assert_eq!(sysfs_size("512"), Some(512));
        assert_eq!(sysfs_size("x"), None);
    }

    /// A record of a core: relation 0, its size, flags, class, 20 bytes,
    /// one group, and one mask.
    fn core(class: u8, mask: u64) -> Vec<u8> {
        let mut r = vec![0u8; 48];
        r[0..4].copy_from_slice(&0u32.to_le_bytes());
        r[4..8].copy_from_slice(&48u32.to_le_bytes());
        r[9] = class;
        r[30..32].copy_from_slice(&1u16.to_le_bytes());
        r[32..40].copy_from_slice(&mask.to_le_bytes());
        r
    }

    fn cache(level: u8, kind: u32, size: u32) -> Vec<u8> {
        let mut r = vec![0u8; 56];
        r[0..4].copy_from_slice(&2u32.to_le_bytes());
        r[4..8].copy_from_slice(&56u32.to_le_bytes());
        r[8] = level;
        r[12..16].copy_from_slice(&size.to_le_bytes());
        r[16..20].copy_from_slice(&kind.to_le_bytes());
        r
    }

    #[test]
    fn windows_records_give_the_cores_the_kinds_and_the_caches() {
        let mut buffer = Vec::new();
        // Two fast cores with two threads each, and two slow cores.
        buffer.extend(core(1, 0b11));
        buffer.extend(core(1, 0b1100));
        buffer.extend(core(0, 0b1_0000));
        buffer.extend(core(0, 0b10_0000));
        buffer.extend(cache(1, 2, 48 * 1024));
        buffer.extend(cache(3, 0, 24 << 20));
        let mut package = vec![0u8; 48];
        package[0..4].copy_from_slice(&3u32.to_le_bytes());
        package[4..8].copy_from_slice(&48u32.to_le_bytes());
        buffer.extend(package);
        let cpu = windows(
            "x86_64",
            &buffer,
            Some("Intel Core".to_owned()),
            None,
            Some(2400),
        );
        assert_eq!((cpu.cores, cpu.threads, cpu.sockets), (4, 6, Some(1)));
        assert_eq!(
            cpu.kinds,
            [
                Kind {
                    name: "Performance".to_owned(),
                    cores: 2
                },
                Kind {
                    name: "Efficiency".to_owned(),
                    cores: 2
                }
            ]
        );
        assert_eq!(cpu.caches.len(), 2);
        assert_eq!(cpu.caches[1].size, 24 << 20);
    }

    #[test]
    fn a_core_record_with_no_class_counts_as_a_core() {
        // A record of eight bytes has a relation and a size, and no field.
        let mut record = vec![0u8; 8];
        record[4..8].copy_from_slice(&8u32.to_le_bytes());
        let cpu = windows("x86_64", &record, None, None, None);
        assert_eq!((cpu.cores, cpu.threads), (1, 0));
    }

    #[test]
    fn a_short_windows_buffer_stops_the_read() {
        let mut buffer = core(0, 1);
        buffer.truncate(20);
        let cpu = windows("x86_64", &buffer, None, None, None);
        assert_eq!(cpu.cores, 0);
    }
}
