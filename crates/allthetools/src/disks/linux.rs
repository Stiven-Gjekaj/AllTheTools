//! The host file of the drive list on Linux: /sys/block, the database of
//! udev in /run/udev/data, and /proc/self/mountinfo. A standard user reads
//! all three.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};

use super::parse::{self, Disk, SysBlock, SysPartition};

const FILES: [&str; 7] = [
    "size",
    "removable",
    "ro",
    "dev",
    "device/model",
    "device/vendor",
    "device/serial",
];

fn read_files(dir: &Path, names: &[&str]) -> BTreeMap<String, String> {
    names
        .iter()
        .filter_map(|name| {
            std::fs::read_to_string(dir.join(name))
                .ok()
                .map(|v| ((*name).to_owned(), v))
        })
        .collect()
}

pub fn list() -> Result<Vec<Disk>> {
    let root = Path::new("/sys/block");
    let mut blocks = Vec::new();
    for entry in std::fs::read_dir(root)
        .context("could not read /sys/block")?
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        let dir = entry.path();
        let link = std::fs::read_link(&dir)
            .map(|l| l.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut partitions = Vec::new();
        if let Ok(inside) = std::fs::read_dir(&dir) {
            for part in inside.flatten() {
                if part.path().join("partition").is_file() {
                    partitions.push(SysPartition {
                        name: part.file_name().to_string_lossy().into_owned(),
                        files: read_files(&part.path(), &["size", "dev"]),
                    });
                }
            }
        }
        partitions.sort_by(|a, b| parse::natural_cmp(&a.name, &b.name));
        blocks.push(SysBlock {
            name,
            link,
            files: read_files(&dir, &FILES),
            partitions,
        });
    }
    let mountinfo = std::fs::read_to_string("/proc/self/mountinfo").unwrap_or_default();
    let udev = |dev: &str| std::fs::read_to_string(format!("/run/udev/data/b{dev}")).ok();
    let slaves = |name: &str| {
        std::fs::read_dir(format!("/sys/class/block/{name}/slaves"))
            .map(|entries| {
                entries
                    .flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default()
    };
    Ok(parse::linux(&blocks, &mountinfo, &udev, &slaves))
}
