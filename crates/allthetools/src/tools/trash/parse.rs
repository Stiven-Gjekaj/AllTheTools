//! The parts of the trash of the freedesktop.org specification, which the
//! file managers of Linux share: the name of an entry, and the info file
//! that says where the entry came from.
//!
//! This module makes no system call, so its tests run on every host.

/// A local date and time, as the info file gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

/// The bytes of a path in the form of a URL: each byte that is not a
/// letter, a digit, a slash or one of `-._~` becomes `%XX`.
pub fn encode(path: &[u8]) -> String {
    let mut out = String::new();
    for &b in path {
        if b.is_ascii_alphanumeric() || b"/-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The text of the info file of one entry. `path` is absolute for the trash
/// in the home folder, and relative to the top of the volume for the trash
/// of another volume.
pub fn info(path: &[u8], when: Stamp) -> String {
    format!(
        "[Trash Info]\nPath={}\nDeletionDate={:04}-{:02}-{:02}T{:02}:{:02}:{:02}\n",
        encode(path),
        when.year,
        when.month,
        when.day,
        when.hour,
        when.minute,
        when.second
    )
}

/// The name of an entry in the trash. The first try is the name of the
/// file. Each next try puts a number before the extension, as `gio trash`
/// does: `notes.txt`, `notes.2.txt`, `notes.3.txt`.
pub fn candidate(name: &str, attempt: u32) -> String {
    if attempt <= 1 {
        return name.to_owned();
    }
    match name.rfind('.') {
        Some(dot) if dot > 0 => format!("{}.{attempt}{}", &name[..dot], &name[dot..]),
        _ => format!("{name}.{attempt}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHEN: Stamp = Stamp {
        year: 2026,
        month: 10,
        day: 16,
        hour: 9,
        minute: 5,
        second: 7,
    };

    #[test]
    fn the_info_file_follows_the_specification() {
        assert_eq!(
            info(b"/home/me/My Notes.txt", WHEN),
            "[Trash Info]\nPath=/home/me/My%20Notes.txt\nDeletionDate=2026-10-16T09:05:07\n"
        );
    }

    #[test]
    fn each_byte_outside_the_safe_set_is_encoded() {
        assert_eq!(encode(b"a b%c"), "a%20b%25c");
        assert_eq!(encode("\u{e9}".as_bytes()), "%C3%A9");
        assert_eq!(encode(b"dir/x-y_z.~"), "dir/x-y_z.~");
    }

    #[test]
    fn a_later_try_puts_a_number_before_the_extension() {
        assert_eq!(candidate("notes.txt", 1), "notes.txt");
        assert_eq!(candidate("notes.txt", 2), "notes.2.txt");
        assert_eq!(candidate("archive.tar.gz", 3), "archive.tar.3.gz");
        assert_eq!(candidate("README", 2), "README.2");
        assert_eq!(candidate(".bashrc", 2), ".bashrc.2");
    }
}
