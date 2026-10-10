//! The decisions of `att mount`: the kind of an image, and what each host
//! says after it attaches one.
//!
//! This module makes no system call, so its tests run on every host.

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// The kind of an image file, from its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// An ISO 9660 or UDF image of an optical disc.
    Iso,
    /// A raw copy of a drive, byte for byte.
    Raw,
    /// An Apple disk image.
    Dmg,
    /// A virtual hard disk of Microsoft.
    Vhd,
    Vhdx,
}

pub fn kind(path: &Path) -> Option<Kind> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match extension.as_str() {
        "iso" | "cdr" => Kind::Iso,
        "img" | "raw" | "bin" | "ima" => Kind::Raw,
        "dmg" | "sparseimage" | "sparsebundle" => Kind::Dmg,
        "vhd" => Kind::Vhd,
        "vhdx" => Kind::Vhdx,
        _ => return None,
    })
}

/// One device that an attach made, and its mount point when it has one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Attached {
    pub device: String,
    pub mount: Option<String>,
    pub filesystem: Option<String>,
}

/// Read the property list of `hdiutil attach -plist`. The first entry is
/// the whole image device, and a device with a mount point comes first after
/// it.
pub fn hdiutil(bytes: &[u8]) -> Result<Vec<Attached>> {
    let value = plist::Value::from_reader_xml(bytes)
        .context("hdiutil gave a property list that is not valid")?;
    let entities = value
        .as_dictionary()
        .and_then(|d| d.get("system-entities"))
        .and_then(plist::Value::as_array)
        .context("hdiutil gave no device")?;
    let mut out: Vec<Attached> = entities
        .iter()
        .filter_map(|e| {
            let d = e.as_dictionary()?;
            let string = |key: &str| {
                d.get(key)
                    .and_then(plist::Value::as_string)
                    .map(str::to_owned)
            };
            Some(Attached {
                device: string("dev-entry")?,
                mount: string("mount-point"),
                filesystem: string("volume-kind"),
            })
        })
        .collect();
    out.sort_by(|a, b| (a.device.len(), &a.device).cmp(&(b.device.len(), &b.device)));
    Ok(out)
}

/// Read each attached image of `hdiutil info -plist`: its path, as the
/// person gave it to hdiutil, and its whole device.
pub fn hdiutil_images(bytes: &[u8]) -> Result<Vec<(String, String)>> {
    let value = plist::Value::from_reader_xml(bytes)
        .context("hdiutil gave a property list that is not valid")?;
    let images = value
        .as_dictionary()
        .and_then(|d| d.get("images"))
        .and_then(plist::Value::as_array);
    let mut out = Vec::new();
    for entry in images.into_iter().flatten() {
        let Some(d) = entry.as_dictionary() else {
            continue;
        };
        let Some(path) = d.get("image-path").and_then(plist::Value::as_string) else {
            continue;
        };
        let device = d
            .get("system-entities")
            .and_then(plist::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|e| e.as_dictionary()?.get("dev-entry")?.as_string())
            .min_by_key(|d| d.len());
        if let Some(device) = device {
            out.push((path.to_owned(), device.to_owned()));
        }
    }
    Ok(out)
}

/// Read the device of `udisksctl loop-setup`, which writes
/// "Mapped file /path/x.iso as /dev/loop0."
pub fn udisks_loop(text: &str) -> Option<String> {
    text.split_whitespace()
        .rev()
        .find(|w| w.starts_with("/dev/"))
        .map(|w| w.trim_end_matches('.').to_owned())
}

/// Read the mount point of `udisksctl mount`, which writes
/// "Mounted /dev/loop0p1 at /media/me/LABEL"
pub fn udisks_mount(text: &str) -> Option<String> {
    let (_, point) = text.trim().split_once(" at ")?;
    Some(point.trim().trim_end_matches('.').to_owned())
}

/// One volume of a mounted image on Windows, from Get-Volume in JSON.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WinVolume {
    drive_letter: Option<serde_json::Value>,
    file_system: Option<String>,
}

/// Read the volumes of the image that Mount-DiskImage attached.
pub fn windows(json: &str, device: &str) -> Result<Vec<Attached>> {
    let volumes: Vec<WinVolume> = if json.trim().is_empty() {
        Vec::new()
    } else {
        serde_json::from_str(json).context("PowerShell gave JSON that is not a list of volumes")?
    };
    let mut out = vec![Attached {
        device: device.to_owned(),
        ..Attached::default()
    }];
    for volume in volumes {
        let letter = match volume.drive_letter {
            Some(serde_json::Value::String(s)) => s.chars().next(),
            Some(serde_json::Value::Number(n)) => n.as_u64().and_then(|n| char::from_u32(n as u32)),
            _ => None,
        }
        .filter(char::is_ascii_alphabetic);
        if let Some(letter) = letter {
            out.push(Attached {
                device: format!("{letter}:"),
                mount: Some(format!("{letter}:\\")),
                filesystem: volume.file_system,
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_comes_from_the_extension_in_any_case() {
        assert_eq!(kind(Path::new("a/ubuntu.ISO")), Some(Kind::Iso));
        assert_eq!(kind(Path::new("sd.img")), Some(Kind::Raw));
        assert_eq!(kind(Path::new("x.dmg")), Some(Kind::Dmg));
        assert_eq!(kind(Path::new("x.vhdx")), Some(Kind::Vhdx));
        assert_eq!(kind(Path::new("notes.txt")), None);
        assert_eq!(kind(Path::new("noext")), None);
    }

    // Captured with `hdiutil attach -nobrowse -plist` of a FAT32 image on
    // macOS, with the keys that the parser does not read taken out.
    const ATTACH: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>system-entities</key>
	<array>
		<dict>
			<key>content-hint</key><string>DOS_FAT_32</string>
			<key>dev-entry</key><string>/dev/disk4s1</string>
			<key>mount-point</key><string>/Volumes/ATTTEST</string>
			<key>volume-kind</key><string>msdos</string>
		</dict>
		<dict>
			<key>content-hint</key><string>FDisk_partition_scheme</string>
			<key>dev-entry</key><string>/dev/disk4</string>
		</dict>
	</array>
</dict>
</plist>"#;

    #[test]
    fn hdiutil_puts_the_whole_device_first() {
        let attached = hdiutil(ATTACH.as_bytes()).unwrap();
        assert_eq!(attached[0].device, "/dev/disk4");
        assert_eq!(attached[0].mount, None);
        assert_eq!(attached[1].mount.as_deref(), Some("/Volumes/ATTTEST"));
        assert_eq!(attached[1].filesystem.as_deref(), Some("msdos"));
    }

    // The shape of `hdiutil info -plist` on macOS, with one image and the
    // keys that the parser reads.
    const INFO: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>images</key>
	<array>
		<dict>
			<key>image-path</key><string>/Users/me/test.dmg</string>
			<key>system-entities</key>
			<array>
				<dict><key>dev-entry</key><string>/dev/disk4s1</string><key>mount-point</key><string>/Volumes/ATTTEST</string></dict>
				<dict><key>dev-entry</key><string>/dev/disk4</string></dict>
			</array>
		</dict>
	</array>
</dict>
</plist>"#;

    #[test]
    fn hdiutil_images_gives_the_whole_device_of_each_image() {
        assert_eq!(
            hdiutil_images(INFO.as_bytes()).unwrap(),
            [("/Users/me/test.dmg".to_owned(), "/dev/disk4".to_owned())]
        );
    }

    #[test]
    fn udisks_lines_give_the_device_and_the_mount_point() {
        assert_eq!(
            udisks_loop("Mapped file /home/me/x.iso as /dev/loop3.").as_deref(),
            Some("/dev/loop3")
        );
        assert_eq!(udisks_loop("nothing here"), None);
        assert_eq!(
            udisks_mount("Mounted /dev/loop3 at /media/me/Ubuntu 24.04 LTS amd64\n").as_deref(),
            Some("/media/me/Ubuntu 24.04 LTS amd64")
        );
    }

    #[test]
    fn windows_lists_each_letter_of_the_image() {
        let attached = windows(
            r#"[{"DriveLetter":"F","FileSystem":"UDF"},{"DriveLetter":null,"FileSystem":"FAT32"}]"#,
            "\\\\.\\CDROM1",
        )
        .unwrap();
        assert_eq!(attached.len(), 2);
        assert_eq!(attached[1].mount.as_deref(), Some("F:\\"));
        assert_eq!(attached[1].filesystem.as_deref(), Some("UDF"));
    }
}
