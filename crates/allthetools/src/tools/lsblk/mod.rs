//! `att lsblk`: list the drives and their partitions as a tree.
//!
//! Linux has `lsblk`. macOS has `diskutil list`, and Windows has `Get-Disk`
//! and Disk Management, and each one shows other facts in another shape.
//! This tool gives one tree on each host, and it marks the drive that holds
//! the running system.

use anyhow::Result;

use crate::disks::{self, Disk};

/// List the drives and their partitions as a tree.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Show the drives only, and not their partitions.
    #[arg(short, long)]
    pub drives: bool,

    /// Print JSON for a script, and not a tree for a person.
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let disks = disks::list()?;
    if args.json {
        println!("{}", serde_json::to_string(&disks)?);
        return Ok(0);
    }
    if disks.is_empty() {
        println!("This computer reports no drive.");
        return Ok(0);
    }
    let rows: Vec<Vec<String>> = disks
        .iter()
        .flat_map(|disk| rows(disk, args.drives))
        .collect();
    crate::table::print(
        &[
            "NAME",
            "SIZE",
            "BUS",
            "FSTYPE",
            "LABEL",
            "MOUNTPOINT",
            "MODEL",
        ],
        &rows,
    );
    Ok(0)
}

/// A size in the short form of lsblk: powers of two, one decimal, one
/// letter.
pub fn short(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["K", "M", "G", "T", "P", "E"];
    if bytes < 1024 {
        return format!("{bytes}B");
    }
    let mut value = bytes as f64;
    let mut unit = "B";
    for next in UNITS {
        if value < 1024.0 {
            break;
        }
        value /= 1024.0;
        unit = next;
    }
    format!("{value:.1}{unit}")
}

fn rows(disk: &Disk, drives_only: bool) -> Vec<Vec<String>> {
    let mut notes = Vec::new();
    if disk.system {
        notes.push("system");
    }
    if disk.removable {
        notes.push("removable");
    }
    if disk.read_only {
        notes.push("read only");
    }
    if disk.virtual_disk {
        notes.push("virtual");
    }
    let mut model = disk.model.clone().unwrap_or_default();
    if !notes.is_empty() {
        if !model.is_empty() {
            model.push(' ');
        }
        model.push_str(&format!("({})", notes.join(", ")));
    }
    let mut out = vec![vec![
        disk.id.clone(),
        short(disk.size),
        disk.bus.clone().unwrap_or_default(),
        String::new(),
        String::new(),
        String::new(),
        model,
    ]];
    if drives_only {
        return out;
    }
    let count = disk.partitions.len();
    for (i, part) in disk.partitions.iter().enumerate() {
        let branch = if i + 1 == count {
            "\u{2514}\u{2500}"
        } else {
            "\u{251c}\u{2500}"
        };
        out.push(vec![
            format!("{branch}{}", part.id),
            short(part.size),
            String::new(),
            part.filesystem.clone().unwrap_or_default(),
            part.label.clone().unwrap_or_default(),
            part.mount.clone().unwrap_or_default(),
            String::new(),
        ]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disks::Partition;

    #[test]
    fn short_sizes_use_one_letter() {
        assert_eq!(short(512), "512B");
        assert_eq!(short(500_277_792_768), "465.9G");
        assert_eq!(short(576_716_800), "550.0M");
    }

    #[test]
    fn the_last_partition_closes_the_tree() {
        let disk = Disk {
            id: "sdb".to_owned(),
            size: 1 << 30,
            removable: true,
            model: Some("Ultra Fit".to_owned()),
            partitions: vec![
                Partition {
                    id: "sdb1".to_owned(),
                    ..Partition::default()
                },
                Partition {
                    id: "sdb2".to_owned(),
                    mount: Some("/media/x".to_owned()),
                    ..Partition::default()
                },
            ],
            ..Disk::default()
        };
        let rows = rows(&disk, false);
        assert_eq!(rows[0][6], "Ultra Fit (removable)");
        assert_eq!(rows[1][0], "\u{251c}\u{2500}sdb1");
        assert_eq!(rows[2][0], "\u{2514}\u{2500}sdb2");
        assert_eq!(rows[2][5], "/media/x");
        assert_eq!(super::rows(&disk, true).len(), 1);
    }
}
