//! The decisions of `att eject`: which drive may go, and what to unmount
//! first.
//!
//! This module makes no system call, so its tests run on every host.

use anyhow::{Result, bail};

use crate::disks::Disk;

/// What the host file does to eject one drive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// The drive, as the list gives it.
    pub disk: Disk,
    /// The mounted partitions, by path, and their mount points.
    pub mounted: Vec<(String, String)>,
}

/// Check that the drive may be ejected, and list its mounted partitions.
pub fn plan(disk: &Disk) -> Result<Plan> {
    if disk.system {
        bail!(
            "{} holds the running system, and att eject refuses it",
            disk.id
        );
    }
    if disk.virtual_disk && !disk.removable {
        bail!(
            "{} is a virtual drive. Use att mount --detach to detach a disk image",
            disk.id
        );
    }
    let mounted = disk
        .partitions
        .iter()
        .filter_map(|p| p.mount.clone().map(|m| (p.path.clone(), m)))
        .collect();
    Ok(Plan {
        disk: disk.clone(),
        mounted,
    })
}

/// The name of the drive for a person, as `disk4 (SanDisk Ultra, 62.5 GB)`.
pub fn describe(disk: &Disk) -> String {
    let size = crate::size::decimal(disk.size);
    match &disk.model {
        Some(model) => format!("{} ({model}, {size})", disk.id),
        None => format!("{} ({size})", disk.id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disks::Partition;

    fn stick() -> Disk {
        Disk {
            id: "sdb".to_owned(),
            size: 62_521_344_000,
            model: Some("SanDisk Ultra".to_owned()),
            removable: true,
            partitions: vec![
                Partition {
                    path: "/dev/sdb1".to_owned(),
                    mount: Some("/media/me/STICK".to_owned()),
                    ..Partition::default()
                },
                Partition {
                    path: "/dev/sdb2".to_owned(),
                    ..Partition::default()
                },
            ],
            ..Disk::default()
        }
    }

    #[test]
    fn plan_lists_each_mounted_partition() {
        let plan = plan(&stick()).unwrap();
        assert_eq!(
            plan.mounted,
            [("/dev/sdb1".to_owned(), "/media/me/STICK".to_owned())]
        );
    }

    #[test]
    fn the_system_drive_is_refused() {
        let mut disk = stick();
        disk.system = true;
        let error = plan(&disk).unwrap_err().to_string();
        assert!(error.contains("running system"), "{error}");
    }

    #[test]
    fn a_disk_image_goes_to_att_mount() {
        let mut disk = stick();
        disk.removable = false;
        disk.virtual_disk = true;
        assert!(plan(&disk).unwrap_err().to_string().contains("att mount"));
    }

    #[test]
    fn describe_names_the_model_and_the_size() {
        assert_eq!(describe(&stick()), "sdb (SanDisk Ultra, 62.5 GB)");
    }
}
