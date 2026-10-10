//! The host file of `att whoholds` on Linux: the links in /proc/<pid>/fd,
//! cwd, root and exe, and the file names in /proc/<pid>/maps.
//!
//! A standard user reads these for their own processes only.

use std::path::Path;

use anyhow::Result;

use super::parse::{self, Holder};

pub fn holders(target: &Path) -> Result<Vec<Holder>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir("/proc")?.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        let dir = entry.path();
        let process = std::fs::read_to_string(dir.join("comm"))
            .map(|c| c.trim().to_owned())
            .unwrap_or_default();
        let mut add = |how: &str, path: &Path| {
            if parse::inside(path, target) {
                let holder = Holder {
                    pid,
                    process: process.clone(),
                    how: how.to_owned(),
                    path: Some(path.to_string_lossy().into_owned()),
                };
                if !out.contains(&holder) {
                    out.push(holder);
                }
            }
        };
        for link in ["cwd", "root", "exe"] {
            if let Ok(path) = std::fs::read_link(dir.join(link)) {
                add(parse::linux_how(link), &path);
            }
        }
        if let Ok(fds) = std::fs::read_dir(dir.join("fd")) {
            for fd in fds.flatten() {
                if let Ok(path) = std::fs::read_link(fd.path()) {
                    add(parse::linux_how("fd"), &path);
                }
            }
        }
        if let Ok(maps) = std::fs::read_to_string(dir.join("maps")) {
            for path in parse::linux_maps(&maps) {
                add(parse::linux_how("maps"), Path::new(&path));
            }
        }
    }
    Ok(out)
}
