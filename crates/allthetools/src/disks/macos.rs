//! The host file of the drive list on macOS: `diskutil list` and
//! `diskutil info`, in property lists.

use std::collections::HashMap;

use anyhow::Result;

use super::parse::{self, Disk};
use crate::host;

fn diskutil(args: &[&str]) -> Result<Vec<u8>> {
    host::output("diskutil", args, None)
}

pub fn list() -> Result<Vec<Disk>> {
    let mut wholes = parse::mac_list(&diskutil(&["list", "-plist"])?)?;
    let mut infos = HashMap::new();
    for whole in &wholes {
        let info = parse::mac_info(&diskutil(&["info", "-plist", &whole.id])?)?;
        infos.insert(whole.id.clone(), info);
    }
    // A partition of an external drive names its file system only in
    // diskutil info. An internal drive has many volumes, and APFS names
    // them already, so the extra calls are for the external drives only.
    for whole in &mut wholes {
        if infos.get(&whole.id).is_some_and(|i| i.internal) {
            continue;
        }
        for part in &mut whole.partitions {
            if let Ok(bytes) = diskutil(&["info", "-plist", &part.id])
                && let Ok(info) = parse::mac_info(&bytes)
                && info.filesystem.is_some()
            {
                part.filesystem = info.filesystem;
            }
        }
    }
    let root = parse::mac_info(&diskutil(&["info", "-plist", "/"])?)?;
    Ok(parse::mac(&wholes, &infos, &root))
}
