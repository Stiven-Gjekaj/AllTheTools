//! The host file of `att mount` on Linux.
//!
//! A desktop has udisks, and `udisksctl` attaches an image to a loop device
//! and mounts it with no root. With no udisks, root attaches it with
//! `losetup` and mounts each volume under /mnt/att.

use std::path::Path;

use anyhow::{Context, Result, bail};

use super::parse::{self, Attached, Kind};
use crate::{disks, host};

fn is_root() -> bool {
    // SAFETY: geteuid has no precondition.
    #[allow(unsafe_code)]
    unsafe {
        libc::geteuid() == 0
    }
}

/// The partitions of a loop device, as the drive list gives them, after the
/// kernel has had a moment to read the partition table.
fn volumes(device: &str) -> Result<Vec<disks::Partition>> {
    std::thread::sleep(std::time::Duration::from_millis(500));
    let list = disks::list()?;
    let disk = disks::parse::select(&list, device)?;
    Ok(disk.partitions.clone())
}

pub fn attach(image: &Path, kind: Kind, read_only: bool) -> Result<Vec<Attached>> {
    if matches!(kind, Kind::Dmg | Kind::Vhd | Kind::Vhdx) {
        bail!(
            "Linux attaches an ISO or a raw image. Convert a DMG, a VHD or a VHDX to a raw image first"
        );
    }
    let path = image.to_string_lossy();
    let mut out = Vec::new();
    if host::on_path("udisksctl") {
        let mut args = vec!["loop-setup", "--file", &path];
        if read_only {
            args.push("--read-only");
        }
        let text = host::text("udisksctl", &args, None)?;
        let device = parse::udisks_loop(&text).context("udisksctl named no loop device")?;
        let mount = |part: &str| {
            host::text("udisksctl", &["mount", "--block-device", part], None)
                .ok()
                .and_then(|t| parse::udisks_mount(&t))
        };
        let parts = volumes(&device)?;
        // A disc image has its file system on the loop device itself, so the
        // device is the one line, with its mount point.
        if parts.is_empty() {
            out.push(Attached {
                mount: mount(&device),
                device,
                filesystem: None,
            });
            return Ok(out);
        }
        out.push(Attached {
            device,
            ..Attached::default()
        });
        for part in parts {
            out.push(Attached {
                mount: part.mount.clone().or_else(|| mount(&part.path)),
                device: part.path,
                filesystem: part.filesystem,
            });
        }
        return Ok(out);
    }

    if !is_root() {
        bail!("this host has no udisksctl. Run att mount as root, or install the package udisks2");
    }
    let mut args = vec!["--find", "--show", "--partscan"];
    if read_only {
        args.push("--read-only");
    }
    args.push(&path);
    let device = host::text("losetup", &args, None)?.trim().to_owned();
    let mut parts: Vec<String> = volumes(&device)?.into_iter().map(|p| p.path).collect();
    if parts.is_empty() {
        // A disc image: the device itself is the one line.
        parts.push(device.clone());
    } else {
        out.push(Attached {
            device: device.clone(),
            ..Attached::default()
        });
    }
    for part in parts {
        let name = part.trim_start_matches("/dev/");
        let point = format!("/mnt/att/{name}");
        std::fs::create_dir_all(&point)?;
        let mut args = vec![part.as_str(), point.as_str()];
        if read_only {
            args.splice(0..0, ["-o", "ro"]);
        }
        let mounted = host::output("mount", &args, None).is_ok();
        if !mounted {
            let _ = std::fs::remove_dir(&point);
        }
        out.push(Attached {
            device: part.clone(),
            mount: mounted.then_some(point),
            filesystem: None,
        });
    }
    Ok(out)
}

/// The loop device of an image file, from the backing file of each loop
/// device in sysfs.
fn loop_of(image: &Path) -> Result<Option<String>> {
    let full = std::fs::canonicalize(image)?;
    for entry in std::fs::read_dir("/sys/block")?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("loop") {
            continue;
        }
        if let Ok(backing) = std::fs::read_to_string(entry.path().join("loop/backing_file"))
            && Path::new(backing.trim()) == full
        {
            return Ok(Some(format!("/dev/{name}")));
        }
    }
    Ok(None)
}

pub fn detach(target: &str) -> Result<String> {
    let path = Path::new(target);
    let device = if path.is_file() {
        match loop_of(path)? {
            Some(device) => device,
            None => bail!("{target} is not attached"),
        }
    } else {
        target.to_owned()
    };
    let list = disks::list()?;
    let disk = disks::parse::select(&list, &device)?;
    let mut mounted: Vec<(String, String)> = disk
        .partitions
        .iter()
        .filter_map(|p| p.mount.clone().map(|m| (p.path.clone(), m)))
        .collect();
    if mounted.is_empty() && disk.partitions.is_empty() {
        // A disc image mounts the loop device itself, and the drive list
        // does not show that mount. Try it, and ignore a device that has no
        // mount.
        mounted.push((disk.path.clone(), String::new()));
    }
    if host::on_path("udisksctl") {
        for (part, _) in &mounted {
            let _ = host::output("udisksctl", &["unmount", "--block-device", part], None);
        }
        host::output(
            "udisksctl",
            &["loop-delete", "--block-device", &disk.path],
            None,
        )?;
    } else {
        for (part, point) in &mounted {
            let _ = host::output("umount", &[part], None);
            if point.starts_with("/mnt/att/") {
                let _ = std::fs::remove_dir(point);
            }
        }
        host::output("losetup", &["--detach", &disk.path], None)?;
    }
    Ok(disk.path.clone())
}
