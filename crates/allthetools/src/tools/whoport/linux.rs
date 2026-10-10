//! The host file of `att whoport` on Linux: the tables of /proc/net, and the
//! links in /proc/<pid>/fd that name each socket.
//!
//! A standard user reads the links of their own processes only. A socket of
//! another user then has no process.

use std::collections::HashMap;

use anyhow::Result;

use super::parse::{self, Proto, Socket};

pub fn sockets() -> Result<Vec<Socket>> {
    let owners = owners();
    let mut out = Vec::new();
    for (file, proto) in [
        ("tcp", Proto::Tcp),
        ("tcp6", Proto::Tcp),
        ("udp", Proto::Udp),
        ("udp6", Proto::Udp),
    ] {
        // A kernel with no IPv6 has no tcp6 and no udp6.
        let Ok(text) = std::fs::read_to_string(format!("/proc/net/{file}")) else {
            continue;
        };
        for (mut socket, inode) in parse::linux(proto, &text) {
            if let Some((pid, name)) = owners.get(&inode) {
                socket.pid = Some(*pid);
                socket.process = Some(name.clone());
            }
            out.push(socket);
        }
    }
    Ok(out)
}

/// The process of each socket inode that this user can see.
fn owners() -> HashMap<u64, (u32, String)> {
    let mut map = HashMap::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return map;
    };
    for entry in entries.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        let Ok(fds) = std::fs::read_dir(entry.path().join("fd")) else {
            continue;
        };
        let name = std::fs::read_to_string(entry.path().join("comm"))
            .map(|n| n.trim().to_owned())
            .unwrap_or_default();
        for fd in fds.flatten() {
            if let Ok(link) = std::fs::read_link(fd.path())
                && let Some(inode) = parse::linux_socket_inode(&link.to_string_lossy())
            {
                map.entry(inode).or_insert_with(|| (pid, name.clone()));
            }
        }
    }
    map
}

pub fn stop(pid: u32) -> std::io::Result<()> {
    super::unix_stop(pid)
}
