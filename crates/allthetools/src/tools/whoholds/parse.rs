//! The decisions of `att whoholds`: which open file is under the path, and
//! what each host says about open files.
//!
//! This module makes no system call, so its tests run on every host.

use std::path::Path;

use serde::Serialize;

/// One process that holds a file under the path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Holder {
    pub pid: u32,
    pub process: String,
    /// How the process holds it: an open file, its working directory, a
    /// library that it maps, and so on.
    pub how: String,
    /// The file. Windows names the process only, and not the file.
    pub path: Option<String>,
}

/// True when `path` is `target` or a path inside it. The test compares
/// whole parts of the path, so /mnt/a10 is not inside /mnt/a1.
pub fn inside(path: &Path, target: &Path) -> bool {
    path.starts_with(target)
}

/// The kind of an entry of /proc/<pid>, for a person.
pub fn linux_how(entry: &str) -> &'static str {
    match entry {
        "cwd" => "working directory",
        "root" => "root directory",
        "exe" => "program",
        "maps" => "mapped file",
        _ => "open file",
    }
}

/// The paths of the mapped files in /proc/<pid>/maps. The sixth column is
/// the path, and a line with no path or a name in brackets is not a file.
pub fn linux_maps(text: &str) -> Vec<String> {
    let mut out: Vec<String> = text
        .lines()
        .filter_map(|line| {
            let mut fields = line.splitn(6, ' ');
            let path = fields.nth(5)?.trim_start();
            path.starts_with('/')
                .then(|| path.trim_end_matches(" (deleted)").to_owned())
        })
        .collect();
    out.dedup();
    out
}

/// The kind of a file descriptor of lsof, for a person.
fn lsof_how(fd: &str) -> &'static str {
    match fd {
        "cwd" => "working directory",
        "rtd" => "root directory",
        "txt" => "program or library",
        "mem" => "mapped file",
        _ => "open file",
    }
}

/// Read the output of `lsof -F pcfn`. Each field is on a line of its own,
/// and the first letter names it: p the process, c the command, f the file
/// descriptor and n the name.
pub fn lsof(text: &str) -> Vec<Holder> {
    let mut out = Vec::new();
    let mut pid = 0u32;
    let mut command = String::new();
    let mut fd = String::new();
    for line in text.lines() {
        let Some(tag) = line.chars().next() else {
            continue;
        };
        let value = &line[tag.len_utf8()..];
        match tag {
            'p' => pid = value.parse().unwrap_or(0),
            'c' => command = value.to_owned(),
            'f' => fd = value.to_owned(),
            'n' => {
                let holder = Holder {
                    pid,
                    process: command.clone(),
                    how: lsof_how(&fd).to_owned(),
                    path: Some(value.to_owned()),
                };
                if !out.contains(&holder) {
                    out.push(holder);
                }
            }
            _ => {}
        }
    }
    out
}

/// The kind of an application of the Restart Manager of Windows, from
/// RM_APP_TYPE, for a person.
pub fn windows_how(kind: i32) -> &'static str {
    match kind {
        1 => "open file, in a program with a window",
        2 => "open file, in a program with no window",
        3 => "open file, in a service",
        4 => "open file, in Explorer",
        5 => "open file, in a console program",
        1000 => "open file, in a critical process",
        _ => "open file",
    }
}

/// Put the holders in the order of the process number, then the path.
pub fn sort(holders: &mut [Holder]) {
    holders.sort_by(|a, b| (a.pid, &a.path).cmp(&(b.pid, &b.path)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inside_compares_whole_parts() {
        assert!(inside(Path::new("/mnt/usb/a.txt"), Path::new("/mnt/usb")));
        assert!(inside(Path::new("/mnt/usb"), Path::new("/mnt/usb")));
        assert!(!inside(Path::new("/mnt/usb2/a.txt"), Path::new("/mnt/usb")));
    }

    #[test]
    fn linux_maps_gives_each_file_one_time() {
        let text = "\
7f0000000000-7f0000001000 r--p 00000000 08:01 1234       /usr/lib/libc.so.6
7f0000001000-7f0000002000 r-xp 00001000 08:01 1234       /usr/lib/libc.so.6
7f0000003000-7f0000004000 rw-p 00000000 00:00 0
7ffd00000000-7ffd00021000 rw-p 00000000 00:00 0          [stack]
7f0000005000-7f0000006000 r--p 00000000 08:11 99         /media/usb/my file.bin (deleted)
";
        assert_eq!(
            linux_maps(text),
            ["/usr/lib/libc.so.6", "/media/usb/my file.bin"]
        );
    }

    #[test]
    fn lsof_reads_each_file_of_each_process() {
        let text = "p501\ncbash\nfcwd\nn/Volumes/STICK\nf3\nn/Volumes/STICK/notes.txt\np777\ncPreview\nftxt\nn/Volumes/STICK/a.pdf\n";
        let holders = lsof(text);
        assert_eq!(holders.len(), 3);
        assert_eq!(holders[0].how, "working directory");
        assert_eq!(holders[1].path.as_deref(), Some("/Volumes/STICK/notes.txt"));
        assert_eq!(holders[1].how, "open file");
        assert_eq!(holders[2].pid, 777);
        assert_eq!(holders[2].process, "Preview");
    }

    #[test]
    fn windows_how_names_the_kind_of_program() {
        assert_eq!(windows_how(4), "open file, in Explorer");
        assert_eq!(windows_how(0), "open file");
    }
}
