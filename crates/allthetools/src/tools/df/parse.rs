//! The volumes of `att df`: the mount table of Linux, the use of each
//! volume, and the table for a person.
//!
//! This module makes no system call, so its tests run on every host.

use serde::Serialize;

/// One mounted volume, with sizes in bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Volume {
    /// The device or the label of the volume.
    pub source: String,
    pub fstype: String,
    pub mount: String,
    pub size: u64,
    pub used: u64,
    /// The bytes that a user who is not root can still write.
    pub available: u64,
}

impl Volume {
    /// The used part, as `df` gives it: the used bytes of the bytes that a
    /// user can have, rounded up, so a full volume shows 100%.
    pub fn percent(&self) -> Option<u64> {
        let base = self.used + self.available;
        if base == 0 {
            return None;
        }
        Some((self.used * 100).div_ceil(base))
    }
}

/// One line of /proc/self/mounts: the source, the mount point and the type.
/// A space, a tab, a line feed and a backslash in a name come as octal
/// escapes, such as `\040`.
pub fn mount_line(line: &str) -> Option<(String, String, String)> {
    let mut fields = line.split_whitespace();
    let source = unescape(fields.next()?);
    let mount = unescape(fields.next()?);
    let fstype = unescape(fields.next()?);
    Some((source, mount, fstype))
}

fn unescape(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\'
            && i + 3 < bytes.len()
            && bytes[i + 1..i + 4]
                .iter()
                .all(|b| (b'0'..=b'7').contains(b))
        {
            let n = (bytes[i + 1] - b'0') * 64 + (bytes[i + 2] - b'0') * 8 + (bytes[i + 3] - b'0');
            out.push(n);
            i += 4;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The volumes to show: each one with a size, unless `all` asks for each
/// one. A volume with no size is a file system of the kernel, such as proc.
pub fn shown(volumes: Vec<Volume>, all: bool) -> Vec<Volume> {
    volumes.into_iter().filter(|v| all || v.size > 0).collect()
}

/// The rows of the table for a person.
pub fn rows(volumes: &[Volume], bytes: bool) -> Vec<Vec<String>> {
    let size = |n: u64| {
        if bytes {
            n.to_string()
        } else {
            crate::size::binary(n)
        }
    };
    volumes
        .iter()
        .map(|v| {
            vec![
                v.source.clone(),
                v.fstype.clone(),
                size(v.size),
                size(v.used),
                size(v.available),
                v.percent()
                    .map(|p| format!("{p}%"))
                    .unwrap_or_else(|| "-".to_owned()),
                v.mount.clone(),
            ]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn volume(size: u64, used: u64, available: u64) -> Volume {
        Volume {
            source: "/dev/sda1".to_owned(),
            fstype: "ext4".to_owned(),
            mount: "/".to_owned(),
            size,
            used,
            available,
        }
    }

    #[test]
    fn the_percent_is_rounded_up_as_df_does() {
        assert_eq!(volume(100, 1, 99).percent(), Some(1));
        assert_eq!(volume(100, 0, 100).percent(), Some(0));
        // 5% is held for root, so a volume with no space for a user is full.
        assert_eq!(volume(100, 95, 0).percent(), Some(100));
        assert_eq!(volume(300, 1, 2).percent(), Some(34));
        assert_eq!(volume(0, 0, 0).percent(), None);
    }

    #[test]
    fn a_mount_line_gives_the_names_with_their_escapes_undone() {
        let line = "/dev/sdb1 /media/me/My\\040Stick vfat rw,nosuid 0 0";
        assert_eq!(
            mount_line(line),
            Some((
                "/dev/sdb1".to_owned(),
                "/media/me/My Stick".to_owned(),
                "vfat".to_owned()
            ))
        );
        assert_eq!(mount_line("short"), None);
        assert_eq!(unescape("a\\134b"), "a\\b");
        assert_eq!(unescape("end\\04"), "end\\04");
    }

    #[test]
    fn a_volume_with_no_size_shows_only_with_all() {
        let list = vec![volume(0, 0, 0), volume(10, 1, 9)];
        assert_eq!(shown(list.clone(), false).len(), 1);
        assert_eq!(shown(list, true).len(), 2);
    }

    #[test]
    fn rows_give_each_column() {
        let rows = rows(&[volume(1 << 30, 1 << 29, 1 << 29)], false);
        assert_eq!(
            rows[0],
            [
                "/dev/sda1",
                "ext4",
                "1.0 GiB",
                "512.0 MiB",
                "512.0 MiB",
                "50%",
                "/"
            ]
        );
    }
}
