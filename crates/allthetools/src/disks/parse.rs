//! Turn what each host says about its drives into one list.
//!
//! The disk tools share this list: lsblk shows it, eject and probe choose a
//! target from it, and probe refuses the drive that the system starts from.
//!
//! This module makes no system call and names no type of a host crate, so
//! its tests run on every host.

use std::collections::{BTreeMap, HashMap, HashSet};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// One whole drive.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Disk {
    /// The name that the host gives: sda, disk4, or 2 for PhysicalDrive2.
    pub id: String,
    /// The path of the device, as a person types it.
    pub path: String,
    /// The path to read and write the drive with no cache of the host.
    pub raw_path: String,
    pub size: u64,
    pub model: Option<String>,
    pub serial: Option<String>,
    /// USB, SATA, NVMe, SD, and so on.
    pub bus: Option<String>,
    pub removable: bool,
    /// The drive holds the running system, or a part of it.
    pub system: bool,
    pub read_only: bool,
    /// The drive is a file or a container that the host made, and not a
    /// device.
    pub virtual_disk: bool,
    pub partitions: Vec<Partition>,
}

/// One partition or volume of a drive.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Partition {
    pub id: String,
    pub path: String,
    pub size: u64,
    pub filesystem: Option<String>,
    pub label: Option<String>,
    pub mount: Option<String>,
}

fn text(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

/// Find the drive that a person names. Each form that a tool of the host
/// shows is accepted: `sdb` and `/dev/sdb`, `disk4`, `/dev/disk4` and
/// `/dev/rdisk4`, `2`, `PhysicalDrive2` and `\\.\PhysicalDrive2`. A name of a
/// partition, or its mount point such as `/Volumes/STICK` or `E:`, gives the
/// drive that holds it.
pub fn select<'a>(disks: &'a [Disk], name: &str) -> Result<&'a Disk> {
    let name = name.trim();
    let bare = name
        .trim_start_matches("\\\\.\\")
        .trim_start_matches("/dev/r")
        .trim_start_matches("/dev/");
    let lower = bare.to_ascii_lowercase();
    let bare = lower.strip_prefix("physicaldrive").unwrap_or(&lower);
    let point = name.trim_end_matches(['/', '\\']);
    let mounted_at = |disk: &Disk| {
        disk.partitions.iter().any(|p| {
            p.mount.as_deref().is_some_and(|m| {
                let m = m.trim_end_matches(['/', '\\']);
                !m.is_empty() && m.eq_ignore_ascii_case(point)
            })
        })
    };
    let matches = |disk: &Disk| {
        disk.id.eq_ignore_ascii_case(bare)
            || disk.path.eq_ignore_ascii_case(name)
            || disk.raw_path.eq_ignore_ascii_case(name)
            || disk
                .partitions
                .iter()
                .any(|p| p.id.eq_ignore_ascii_case(bare) || p.path.eq_ignore_ascii_case(name))
                && !disk.partitions.iter().any(|p| p.id.parse::<u32>().is_ok())
    };
    match disks
        .iter()
        .find(|d| matches(d))
        .or_else(|| disks.iter().find(|d| mounted_at(d)))
    {
        Some(disk) => Ok(disk),
        None => anyhow::bail!("no drive has the name `{name}`. `att lsblk` lists the drives"),
    }
}

/// Compare two names so that a number in them counts as a number: sda2
/// comes before sda10.
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let split = |s: &str| {
        let digits = s.len() - s.trim_end_matches(|c: char| c.is_ascii_digit()).len();
        let (head, tail) = s.split_at(s.len() - digits);
        (head.to_owned(), tail.parse::<u64>().unwrap_or(0))
    };
    split(a).cmp(&split(b))
}

// ---------------------------------------------------------------- Linux --

/// What the Linux host file reads for one block device in /sys/block.
#[derive(Debug, Clone, Default)]
pub struct SysBlock {
    pub name: String,
    /// The target of the link /sys/block/<name>, which names the bus.
    pub link: String,
    /// The files of the directory, and of its `device` directory with the
    /// prefix `device/`.
    pub files: BTreeMap<String, String>,
    pub partitions: Vec<SysPartition>,
}

#[derive(Debug, Clone, Default)]
pub struct SysPartition {
    pub name: String,
    pub files: BTreeMap<String, String>,
}

/// The bus of a block device, from the path of its link in sysfs.
pub fn linux_bus(link: &str) -> Option<String> {
    let bus = if link.contains("/usb") {
        "USB"
    } else if link.contains("/nvme") {
        "NVMe"
    } else if link.contains("/mmc") {
        "SD/MMC"
    } else if link.contains("/virtio") {
        "Virtio"
    } else if link.contains("/ata") {
        "SATA"
    } else if link.contains("/host") && link.contains("/target") {
        "SCSI"
    } else {
        return None;
    };
    Some(bus.to_owned())
}

/// Read the mount points of /proc/self/mountinfo, by the device number
/// `major:minor`.
pub fn linux_mounts(mountinfo: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for line in mountinfo.lines() {
        let fields: Vec<&str> = line.split(' ').collect();
        if let (Some(dev), Some(point)) = (fields.get(2), fields.get(4)) {
            // The kernel writes a space in a path as \040.
            let point = point.replace("\\040", " ");
            out.entry((*dev).to_owned()).or_insert(point);
        }
    }
    out
}

/// The device number of the mount of `/`, from /proc/self/mountinfo.
pub fn linux_root(mountinfo: &str) -> Option<String> {
    mountinfo.lines().find_map(|line| {
        let fields: Vec<&str> = line.split(' ').collect();
        (fields.get(4) == Some(&"/")).then(|| fields.get(2).map(|d| (*d).to_owned()))?
    })
}

/// Read the file system and its label from a database file of udev,
/// /run/udev/data/b<major>:<minor>.
pub fn linux_udev(data: &str) -> (Option<String>, Option<String>) {
    let mut kind = None;
    let mut label = None;
    for line in data.lines() {
        if let Some(value) = line.strip_prefix("E:ID_FS_TYPE=") {
            kind = text(Some(value));
        } else if let Some(value) = line.strip_prefix("E:ID_FS_LABEL=") {
            label = text(Some(value));
        }
    }
    (kind, label)
}

/// Make the list of drives of Linux. `udev` gives the database text of a
/// device number. `root` is the device number of `/`, and `slaves` gives the
/// devices under a device mapper, so the drive under an encrypted root is a
/// system drive too.
pub fn linux(
    blocks: &[SysBlock],
    mountinfo: &str,
    udev: &dyn Fn(&str) -> Option<String>,
    slaves: &dyn Fn(&str) -> Vec<String>,
) -> Vec<Disk> {
    let mounts = linux_mounts(mountinfo);
    let root = linux_root(mountinfo);

    // The names of the block devices that hold `/`, down through any device
    // mapper.
    let mut system_names: HashSet<String> = HashSet::new();
    if let Some(root) = &root {
        let mut queue: Vec<String> = Vec::new();
        for block in blocks {
            if block.files.get("dev").map(|d| d.trim()) == Some(root.as_str()) {
                queue.push(block.name.clone());
            }
            for part in &block.partitions {
                if part.files.get("dev").map(|d| d.trim()) == Some(root.as_str()) {
                    queue.push(part.name.clone());
                }
            }
        }
        while let Some(name) = queue.pop() {
            if system_names.insert(name.clone()) {
                queue.extend(slaves(&name));
            }
        }
    }

    let mut out = Vec::new();
    for block in blocks {
        let get = |name: &str| block.files.get(name).map(|v| v.trim());
        let number = |name: &str| get(name).and_then(|v| v.parse::<u64>().ok());
        let size = number("size").unwrap_or(0) * 512;
        if size == 0 || block.name.starts_with("ram") || block.name.starts_with("zram") {
            continue;
        }
        let model = match (text(get("device/vendor")), text(get("device/model"))) {
            (Some(vendor), Some(model)) if !model.starts_with(&vendor) => {
                Some(format!("{vendor} {model}"))
            }
            (_, Some(model)) => Some(model),
            (vendor, None) => vendor,
        };
        let partitions: Vec<Partition> = block
            .partitions
            .iter()
            .map(|part| {
                let dev = part
                    .files
                    .get("dev")
                    .map(|d| d.trim().to_owned())
                    .unwrap_or_default();
                let (filesystem, label) = udev(&dev).map(|d| linux_udev(&d)).unwrap_or_default();
                Partition {
                    id: part.name.clone(),
                    path: format!("/dev/{}", part.name),
                    size: part
                        .files
                        .get("size")
                        .and_then(|v| v.trim().parse::<u64>().ok())
                        .unwrap_or(0)
                        * 512,
                    filesystem,
                    label,
                    mount: mounts.get(&dev).cloned(),
                }
            })
            .collect();
        let system = system_names.contains(&block.name)
            || block
                .partitions
                .iter()
                .any(|p| system_names.contains(&p.name));
        let bus = linux_bus(&block.link);
        out.push(Disk {
            id: block.name.clone(),
            path: format!("/dev/{}", block.name),
            raw_path: format!("/dev/{}", block.name),
            size,
            model,
            serial: text(get("device/serial")),
            removable: get("removable") == Some("1") || bus.as_deref() == Some("USB"),
            system,
            read_only: get("ro") == Some("1"),
            virtual_disk: block.link.contains("/virtual/"),
            bus,
            partitions,
        });
    }
    out.sort_by(|a, b| natural_cmp(&a.id, &b.id));
    out
}

// ---------------------------------------------------------------- macOS --

fn plist_value(bytes: &[u8]) -> Result<plist::Value> {
    plist::Value::from_reader_xml(bytes).context("diskutil gave a property list that is not valid")
}

/// One whole disk of `diskutil list -plist`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MacWhole {
    pub id: String,
    pub size: u64,
    pub partitions: Vec<Partition>,
    /// For an APFS container: the partitions that hold it.
    pub stores: Vec<String>,
}

/// Read the whole disks of `diskutil list -plist`, with their partitions and
/// their APFS volumes.
pub fn mac_list(bytes: &[u8]) -> Result<Vec<MacWhole>> {
    let value = plist_value(bytes)?;
    let all = value
        .as_dictionary()
        .and_then(|d| d.get("AllDisksAndPartitions"))
        .and_then(plist::Value::as_array)
        .context("diskutil gave no list of disks")?;
    let mut out = Vec::new();
    for entry in all {
        let Some(dict) = entry.as_dictionary() else {
            continue;
        };
        let string =
            |d: &plist::Dictionary, key: &str| text(d.get(key).and_then(plist::Value::as_string));
        let Some(id) = string(dict, "DeviceIdentifier") else {
            continue;
        };
        let mut partitions = Vec::new();
        for (key, apfs) in [("Partitions", false), ("APFSVolumes", true)] {
            for part in dict
                .get(key)
                .and_then(plist::Value::as_array)
                .into_iter()
                .flatten()
            {
                let Some(part) = part.as_dictionary() else {
                    continue;
                };
                let Some(part_id) = string(part, "DeviceIdentifier") else {
                    continue;
                };
                let filesystem = if apfs {
                    Some("apfs".to_owned())
                } else {
                    string(part, "Content").map(|c| mac_content(&c))
                };
                partitions.push(Partition {
                    path: format!("/dev/{part_id}"),
                    id: part_id,
                    size: part
                        .get("Size")
                        .and_then(plist::Value::as_unsigned_integer)
                        .unwrap_or(0),
                    filesystem,
                    label: string(part, "VolumeName"),
                    mount: string(part, "MountPoint"),
                });
            }
        }
        let stores = dict
            .get("APFSPhysicalStores")
            .and_then(plist::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|s| {
                s.as_dictionary()
                    .and_then(|s| string(s, "DeviceIdentifier"))
            })
            .collect();
        out.push(MacWhole {
            size: dict
                .get("Size")
                .and_then(plist::Value::as_unsigned_integer)
                .unwrap_or(0),
            id,
            partitions,
            stores,
        });
    }
    Ok(out)
}

/// A short name for the content of a partition of diskutil.
fn mac_content(content: &str) -> String {
    match content {
        "DOS_FAT_32" | "Windows_FAT_32" | "DOS_FAT_16" | "DOS_FAT_12" => "vfat",
        "Apple_HFS" => "hfs",
        "Apple_APFS" => "apfs container",
        "EFI" => "EFI",
        "Windows_NTFS" => "ntfs",
        "Linux" | "Linux Filesystem" => "linux",
        "Linux_Swap" => "swap",
        other => other,
    }
    .to_owned()
}

/// What `diskutil info -plist` says about one disk or one volume.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MacInfo {
    pub id: String,
    pub model: Option<String>,
    pub bus: Option<String>,
    pub internal: bool,
    pub removable: bool,
    pub writable: bool,
    pub virtual_disk: bool,
    pub parent: Option<String>,
    pub stores: Vec<String>,
    pub filesystem: Option<String>,
}

pub fn mac_info(bytes: &[u8]) -> Result<MacInfo> {
    let value = plist_value(bytes)?;
    let dict = value
        .as_dictionary()
        .context("diskutil gave no information")?;
    let string = |key: &str| text(dict.get(key).and_then(plist::Value::as_string));
    let flag = |key: &str| dict.get(key).and_then(plist::Value::as_boolean) == Some(true);
    let stores = dict
        .get("APFSPhysicalStores")
        .and_then(plist::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|s| {
            s.as_dictionary()
                .and_then(|s| s.get("APFSPhysicalStore"))
                .and_then(plist::Value::as_string)
                .map(str::to_owned)
        })
        .collect();
    Ok(MacInfo {
        id: string("DeviceIdentifier").unwrap_or_default(),
        model: string("MediaName"),
        bus: string("BusProtocol"),
        internal: flag("Internal"),
        removable: flag("Removable") || flag("RemovableMedia") || flag("Ejectable"),
        writable: flag("WritableMedia"),
        virtual_disk: string("VirtualOrPhysical").as_deref() == Some("Virtual")
            || string("BusProtocol").as_deref() == Some("Disk Image"),
        parent: string("ParentWholeDisk"),
        stores,
        filesystem: string("FilesystemType"),
    })
}

/// The whole disk of a partition name: disk0s2 gives disk0.
pub fn mac_whole(id: &str) -> String {
    match id[4.min(id.len())..].find('s') {
        Some(at) => id[..4 + at].to_owned(),
        None => id.to_owned(),
    }
}

/// Make the list of drives of macOS from the list, the information of each
/// whole disk, and the information of the volume `/`.
pub fn mac(list: &[MacWhole], infos: &HashMap<String, MacInfo>, root: &MacInfo) -> Vec<Disk> {
    // The disk of `/`, and each disk that holds its APFS container.
    let mut system: HashSet<String> = HashSet::new();
    if let Some(parent) = &root.parent {
        system.insert(parent.clone());
    }
    for store in &root.stores {
        system.insert(mac_whole(store));
    }
    // Each other APFS container on the system drive, such as the one of
    // Recovery, is a part of the system too.
    let physical: Vec<String> = system.iter().cloned().collect();
    for whole in list {
        if whole
            .stores
            .iter()
            .any(|s| physical.contains(&mac_whole(s)))
        {
            system.insert(whole.id.clone());
        }
    }

    list.iter()
        .map(|whole| {
            let info = infos.get(&whole.id).cloned().unwrap_or_default();
            let bus = info.bus.clone().map(|b| match b.as_str() {
                "Apple Fabric" => "Apple Fabric".to_owned(),
                "PCI-Express" => "NVMe".to_owned(),
                "Secure Digital" => "SD".to_owned(),
                other => other.to_owned(),
            });
            Disk {
                path: format!("/dev/{}", whole.id),
                raw_path: format!("/dev/r{}", whole.id),
                size: whole.size,
                model: info.model.clone(),
                serial: None,
                removable: !info.internal && info.removable,
                system: system.contains(&whole.id),
                read_only: !info.writable && infos.contains_key(&whole.id),
                virtual_disk: info.virtual_disk || !whole.stores.is_empty(),
                bus,
                partitions: whole.partitions.clone(),
                id: whole.id.clone(),
            }
        })
        .collect()
}

// -------------------------------------------------------------- Windows --

/// The JSON of the PowerShell script of the Windows host.
#[derive(Debug, Default, Deserialize)]
pub struct WindowsReport {
    #[serde(default)]
    pub disks: Vec<WinDisk>,
    #[serde(default)]
    pub partitions: Vec<WinPartition>,
    #[serde(default)]
    pub volumes: Vec<WinVolume>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct WinDisk {
    pub number: u32,
    pub friendly_name: Option<String>,
    pub serial_number: Option<String>,
    pub size: Option<u64>,
    /// The bus as the name that ConvertTo-Json writes for the enum, or as
    /// its number.
    pub bus_type: Option<serde_json::Value>,
    pub is_boot: Option<bool>,
    pub is_system: Option<bool>,
    pub is_read_only: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct WinPartition {
    pub disk_number: u32,
    pub partition_number: u32,
    pub size: Option<u64>,
    /// A letter as a number of a character, or as a string, or nothing.
    pub drive_letter: Option<serde_json::Value>,
    pub access_paths: Option<serde_json::Value>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct WinVolume {
    pub drive_letter: Option<serde_json::Value>,
    /// The name of the file system, such as NTFS. MSFT_Volume also has
    /// FileSystemType, a number, and the name is easier to trust.
    pub file_system: Option<String>,
    pub file_system_label: Option<String>,
}

/// The names of the values of the enum BusType of Windows Storage
/// Management.
fn windows_bus(value: &serde_json::Value) -> Option<String> {
    if let Some(name) = value.as_str() {
        return Some(name.to_owned());
    }
    let name = match value.as_u64()? {
        1 => "SCSI",
        2 => "ATAPI",
        3 => "ATA",
        4 => "1394",
        5 => "SSA",
        6 => "Fibre Channel",
        7 => "USB",
        8 => "RAID",
        9 => "iSCSI",
        10 => "SAS",
        11 => "SATA",
        12 => "SD",
        13 => "MMC",
        14 => "Virtual",
        15 => "File Backed Virtual",
        16 => "Storage Spaces",
        17 => "NVMe",
        _ => return None,
    };
    Some(name.to_owned())
}

/// A drive letter of PowerShell: a character as a number, as `"E"`, or as
/// nothing when the partition has no letter.
fn windows_letter(value: Option<&serde_json::Value>) -> Option<char> {
    let letter = match value? {
        serde_json::Value::Number(n) => char::from_u32(n.as_u64()? as u32)?,
        serde_json::Value::String(s) => s.chars().next()?,
        _ => return None,
    };
    letter
        .is_ascii_alphabetic()
        .then_some(letter.to_ascii_uppercase())
}

pub fn windows(json: &str) -> Result<Vec<Disk>> {
    let report: WindowsReport =
        serde_json::from_str(json).context("PowerShell gave JSON that is not a list of disks")?;
    let volumes: HashMap<char, &WinVolume> = report
        .volumes
        .iter()
        .filter_map(|v| windows_letter(v.drive_letter.as_ref()).map(|l| (l, v)))
        .collect();
    let mut out: Vec<Disk> = report
        .disks
        .iter()
        .map(|disk| {
            let bus = disk.bus_type.as_ref().and_then(windows_bus);
            let mut partitions: Vec<Partition> = report
                .partitions
                .iter()
                .filter(|p| p.disk_number == disk.number)
                .map(|p| {
                    let letter = windows_letter(p.drive_letter.as_ref());
                    let volume = letter.and_then(|l| volumes.get(&l));
                    Partition {
                        id: format!("{}", p.partition_number),
                        path: format!(
                            "\\\\?\\GLOBALROOT\\Device\\Harddisk{}\\Partition{}",
                            disk.number, p.partition_number
                        ),
                        size: p.size.unwrap_or(0),
                        filesystem: volume.and_then(|v| text(v.file_system.as_deref())),
                        label: volume.and_then(|v| text(v.file_system_label.as_deref())),
                        mount: letter.map(|l| format!("{l}:\\")),
                    }
                })
                .collect();
            partitions.sort_by_key(|p| p.id.parse::<u32>().unwrap_or(0));
            let removable = matches!(bus.as_deref(), Some("USB" | "SD" | "MMC"));
            Disk {
                id: disk.number.to_string(),
                path: format!("\\\\.\\PhysicalDrive{}", disk.number),
                raw_path: format!("\\\\.\\PhysicalDrive{}", disk.number),
                size: disk.size.unwrap_or(0),
                model: text(disk.friendly_name.as_deref()),
                serial: text(disk.serial_number.as_deref()),
                removable,
                system: disk.is_boot == Some(true) || disk.is_system == Some(true),
                read_only: disk.is_read_only == Some(true),
                virtual_disk: matches!(bus.as_deref(), Some("Virtual" | "File Backed Virtual")),
                bus,
                partitions,
            }
        })
        .collect();
    out.sort_by_key(|d| d.id.parse::<u32>().unwrap_or(u32::MAX));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), format!("{v}\n")))
            .collect()
    }

    // Three lines in the form of /proc/self/mountinfo: the root on an
    // encrypted volume, a USB stick with a space in its mount point, and a
    // file system with no device.
    const MOUNTINFO: &str = "\
29 1 253:0 / / rw,relatime shared:1 - ext4 /dev/mapper/root rw
45 29 8:17 / /media/me/MY\\040STICK rw,nosuid shared:2 - vfat /dev/sdb1 rw
22 29 0:21 / /proc rw shared:3 - proc proc rw
";

    fn blocks() -> Vec<SysBlock> {
        vec![
            SysBlock {
                name: "nvme0n1".into(),
                link: "../devices/pci0000:00/0000:00:1d.0/0000:02:00.0/nvme/nvme0/nvme0n1".into(),
                files: files(&[
                    ("size", "1000215216"),
                    ("removable", "0"),
                    ("ro", "0"),
                    ("dev", "259:0"),
                    ("device/model", "Samsung SSD 980 PRO 500GB"),
                ]),
                partitions: vec![SysPartition {
                    name: "nvme0n1p2".into(),
                    files: files(&[("size", "999165952"), ("dev", "259:2")]),
                }],
            },
            SysBlock {
                name: "sdb".into(),
                link: "../devices/pci0000:00/0000:00:14.0/usb2/2-1/2-1:1.0/host3/target3:0:0/3:0:0:0/block/sdb".into(),
                files: files(&[
                    ("size", "122142720"),
                    ("removable", "1"),
                    ("ro", "0"),
                    ("dev", "8:16"),
                    ("device/vendor", "SanDisk"),
                    ("device/model", "Ultra Fit"),
                ]),
                partitions: vec![SysPartition {
                    name: "sdb1".into(),
                    files: files(&[("size", "122140672"), ("dev", "8:17")]),
                }],
            },
            SysBlock {
                name: "dm-0".into(),
                link: "../devices/virtual/block/dm-0".into(),
                files: files(&[("size", "999133184"), ("dev", "253:0")]),
                partitions: vec![],
            },
            SysBlock {
                name: "loop0".into(),
                link: "../devices/virtual/block/loop0".into(),
                files: files(&[("size", "0"), ("dev", "7:0")]),
                partitions: vec![],
            },
        ]
    }

    #[test]
    fn linux_finds_the_system_drive_under_an_encrypted_root() {
        let udev = |dev: &str| {
            (dev == "8:17").then(|| "E:ID_FS_TYPE=vfat\nE:ID_FS_LABEL=MY STICK\n".to_owned())
        };
        let slaves = |name: &str| match name {
            "dm-0" => vec!["nvme0n1p2".to_owned()],
            _ => Vec::new(),
        };
        let disks = linux(&blocks(), MOUNTINFO, &udev, &slaves);
        let ids: Vec<&str> = disks.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(
            ids,
            ["dm-0", "nvme0n1", "sdb"],
            "a device of size 0 is skipped"
        );

        let nvme = &disks[1];
        assert!(
            nvme.system,
            "the drive under the encrypted root holds the system"
        );
        assert_eq!(nvme.bus.as_deref(), Some("NVMe"));
        assert!(!nvme.removable);
        assert_eq!(nvme.size, 1000215216 * 512);

        let stick = &disks[2];
        assert!(!stick.system);
        assert!(stick.removable);
        assert_eq!(stick.bus.as_deref(), Some("USB"));
        assert_eq!(stick.model.as_deref(), Some("SanDisk Ultra Fit"));
        let part = &stick.partitions[0];
        assert_eq!(part.mount.as_deref(), Some("/media/me/MY STICK"));
        assert_eq!(part.filesystem.as_deref(), Some("vfat"));
        assert_eq!(part.label.as_deref(), Some("MY STICK"));

        assert!(disks[0].virtual_disk);
        assert!(disks[0].system);
    }

    // Captured with `diskutil list -plist` on a MacBook with Apple silicon,
    // and cut down to the physical disk, its main APFS container, and the
    // keys that the parser reads, with the container of iSC on disk0s1. The
    // USB stick, disk4, is added in the same form.
    const LIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>AllDisksAndPartitions</key>
  <array>
    <dict>
      <key>Content</key><string>GUID_partition_scheme</string>
      <key>DeviceIdentifier</key><string>disk0</string>
      <key>Partitions</key>
      <array>
        <dict>
          <key>Content</key><string>Apple_APFS_ISC</string>
          <key>DeviceIdentifier</key><string>disk0s1</string>
          <key>Size</key><integer>576716800</integer>
        </dict>
        <dict>
          <key>Content</key><string>Apple_APFS</string>
          <key>DeviceIdentifier</key><string>disk0s2</string>
          <key>Size</key><integer>494332366848</integer>
        </dict>
      </array>
      <key>Size</key><integer>500277792768</integer>
    </dict>
    <dict>
      <key>APFSPhysicalStores</key>
      <array><dict><key>DeviceIdentifier</key><string>disk0s2</string></dict></array>
      <key>APFSVolumes</key>
      <array>
        <dict>
          <key>DeviceIdentifier</key><string>disk3s1s1</string>
          <key>MountPoint</key><string>/</string>
          <key>Size</key><integer>494332366848</integer>
          <key>VolumeName</key><string>Internal SSD</string>
        </dict>
        <dict>
          <key>DeviceIdentifier</key><string>disk3s5</string>
          <key>MountPoint</key><string>/System/Volumes/Data</string>
          <key>Size</key><integer>494332366848</integer>
          <key>VolumeName</key><string>Data</string>
        </dict>
      </array>
      <key>Content</key><string>Apple_APFS_Container</string>
      <key>DeviceIdentifier</key><string>disk3</string>
      <key>Size</key><integer>494332366848</integer>
    </dict>
    <dict>
      <key>APFSPhysicalStores</key>
      <array><dict><key>DeviceIdentifier</key><string>disk0s1</string></dict></array>
      <key>Content</key><string>Apple_APFS_Container</string>
      <key>DeviceIdentifier</key><string>disk1</string>
      <key>Size</key><integer>576716800</integer>
    </dict>
    <dict>
      <key>Content</key><string>FDisk_partition_scheme</string>
      <key>DeviceIdentifier</key><string>disk4</string>
      <key>Partitions</key>
      <array>
        <dict>
          <key>Content</key><string>DOS_FAT_32</string>
          <key>DeviceIdentifier</key><string>disk4s1</string>
          <key>MountPoint</key><string>/Volumes/STICK</string>
          <key>Size</key><integer>62513152000</integer>
          <key>VolumeName</key><string>STICK</string>
        </dict>
      </array>
      <key>Size</key><integer>62521344000</integer>
    </dict>
  </array>
</dict>
</plist>"#;

    fn info(id: &str, bus: &str, internal: bool, removable: bool, virtual_disk: bool) -> MacInfo {
        MacInfo {
            id: id.to_owned(),
            model: Some(format!("model of {id}")),
            bus: Some(bus.to_owned()),
            internal,
            removable,
            writable: true,
            virtual_disk,
            parent: Some(id.to_owned()),
            stores: Vec::new(),
            filesystem: None,
        }
    }

    #[test]
    fn mac_marks_the_container_of_root_and_the_disk_under_it() {
        let list = mac_list(LIST.as_bytes()).unwrap();
        assert_eq!(list.len(), 4);
        assert_eq!(list[1].stores, ["disk0s2"]);

        let infos = HashMap::from([
            (
                "disk0".to_owned(),
                info("disk0", "Apple Fabric", true, false, false),
            ),
            (
                "disk3".to_owned(),
                info("disk3", "Apple Fabric", true, false, true),
            ),
            ("disk4".to_owned(), info("disk4", "USB", false, true, false)),
        ]);
        let root = MacInfo {
            parent: Some("disk3".to_owned()),
            stores: vec!["disk0s2".to_owned()],
            ..MacInfo::default()
        };
        let disks = mac(&list, &infos, &root);
        assert!(disks[0].system, "disk0 holds the container of /");
        assert!(disks[1].system && disks[1].virtual_disk);
        assert!(disks[2].system, "the iSC container lives on disk0");
        let stick = &disks[3];
        assert!(!stick.system && stick.removable);
        assert_eq!(stick.raw_path, "/dev/rdisk4");
        assert_eq!(stick.partitions[0].filesystem.as_deref(), Some("vfat"));
        assert_eq!(stick.partitions[0].mount.as_deref(), Some("/Volumes/STICK"));
        assert_eq!(disks[1].partitions[0].mount.as_deref(), Some("/"));
        assert!(!disks[3].system);
    }

    // Captured with `diskutil info -plist /` on the same MacBook, and cut
    // down to the keys that the parser reads.
    const ROOT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>APFSPhysicalStores</key>
  <array><dict><key>APFSPhysicalStore</key><string>disk0s2</string></dict></array>
  <key>BusProtocol</key><string>Apple Fabric</string>
  <key>DeviceIdentifier</key><string>disk3s1s1</string>
  <key>Ejectable</key><false/>
  <key>FilesystemType</key><string>apfs</string>
  <key>Internal</key><true/>
  <key>MediaName</key><string></string>
  <key>ParentWholeDisk</key><string>disk3</string>
  <key>Removable</key><false/>
  <key>RemovableMedia</key><false/>
  <key>WritableMedia</key><false/>
</dict>
</plist>"#;

    #[test]
    fn mac_info_reads_the_parent_and_the_stores() {
        let root = mac_info(ROOT.as_bytes()).unwrap();
        assert_eq!(root.parent.as_deref(), Some("disk3"));
        assert_eq!(root.stores, ["disk0s2"]);
        assert_eq!(root.model, None, "an empty name is no name");
        assert!(root.internal && !root.removable && !root.writable);
        assert_eq!(root.filesystem.as_deref(), Some("apfs"));
    }

    #[test]
    fn select_takes_each_form_of_a_name() {
        let disk = |id: &str, path: &str, raw: &str, part: &str| Disk {
            id: id.to_owned(),
            path: path.to_owned(),
            raw_path: raw.to_owned(),
            partitions: vec![Partition {
                id: part.to_owned(),
                path: format!("/dev/{part}"),
                ..Partition::default()
            }],
            ..Disk::default()
        };
        let disks = [
            disk("disk4", "/dev/disk4", "/dev/rdisk4", "disk4s1"),
            disk("sdb", "/dev/sdb", "/dev/sdb", "sdb1"),
            disk("2", "\\\\.\\PhysicalDrive2", "\\\\.\\PhysicalDrive2", "1"),
        ];
        for name in [
            "disk4",
            "/dev/disk4",
            "/dev/rdisk4",
            "disk4s1",
            "/dev/disk4s1",
        ] {
            assert_eq!(select(&disks, name).unwrap().id, "disk4", "{name}");
        }
        for name in ["sdb", "/dev/sdb", "sdb1"] {
            assert_eq!(select(&disks, name).unwrap().id, "sdb", "{name}");
        }
        for name in [
            "2",
            "PhysicalDrive2",
            "\\\\.\\PhysicalDrive2",
            "physicaldrive2",
        ] {
            assert_eq!(select(&disks, name).unwrap().id, "2", "{name}");
        }
        assert!(
            select(&disks, "1").is_err(),
            "a partition number of Windows is no drive"
        );
        assert!(select(&disks, "sdc").is_err());
    }

    #[test]
    fn natural_cmp_counts_the_number_at_the_end() {
        let mut names = vec!["sda10", "sda2", "sda1", "nvme0n1"];
        names.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(names, ["nvme0n1", "sda1", "sda2", "sda10"]);
    }

    #[test]
    fn mac_whole_strips_the_slice() {
        assert_eq!(mac_whole("disk0s2"), "disk0");
        assert_eq!(mac_whole("disk3s1s1"), "disk3");
        assert_eq!(mac_whole("disk12"), "disk12");
    }

    // The JSON of the PowerShell script, in the shape that ConvertTo-Json
    // gives: enums as numbers, and a drive letter as the number of its
    // character.
    const WINDOWS: &str = r#"{
      "disks": [
        {"Number":0,"FriendlyName":"Samsung SSD 980 PRO 1TB","SerialNumber":"S5GX","Size":1000204886016,"BusType":17,"IsBoot":true,"IsSystem":true,"IsReadOnly":false},
        {"Number":1,"FriendlyName":"SanDisk Ultra Fit","SerialNumber":"4C53","Size":61530439680,"BusType":7,"IsBoot":false,"IsSystem":false,"IsReadOnly":false}
      ],
      "partitions": [
        {"DiskNumber":0,"PartitionNumber":3,"Size":999000000000,"DriveLetter":67},
        {"DiskNumber":0,"PartitionNumber":1,"Size":104857600,"DriveLetter":0},
        {"DiskNumber":1,"PartitionNumber":1,"Size":61529391104,"DriveLetter":69}
      ],
      "volumes": [
        {"DriveLetter":67,"FileSystem":"NTFS","FileSystemLabel":"Windows"},
        {"DriveLetter":"E","FileSystem":"exFAT","FileSystemLabel":"STICK"}
      ]
    }"#;

    #[test]
    fn windows_joins_disks_partitions_and_volumes() {
        let disks = windows(WINDOWS).unwrap();
        assert_eq!(disks.len(), 2);
        let system = &disks[0];
        assert!(system.system && !system.removable);
        assert_eq!(system.bus.as_deref(), Some("NVMe"));
        assert_eq!(system.path, "\\\\.\\PhysicalDrive0");
        assert_eq!(system.partitions[0].id, "1", "partitions are in order");
        assert_eq!(system.partitions[0].mount, None, "letter 0 is no letter");
        assert_eq!(system.partitions[1].mount.as_deref(), Some("C:\\"));
        assert_eq!(system.partitions[1].label.as_deref(), Some("Windows"));
        let stick = &disks[1];
        assert!(stick.removable && !stick.system);
        assert_eq!(stick.partitions[0].filesystem.as_deref(), Some("exFAT"));
        assert_eq!(stick.partitions[0].mount.as_deref(), Some("E:\\"));
    }
}
