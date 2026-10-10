//! The host file of `att whoport` on Windows: the extended TCP and UDP
//! tables of IP Helper, which give the process of each socket, and a
//! snapshot of the processes for their names.

use std::collections::HashMap;

use anyhow::{Result, bail};
use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE, NO_ERROR};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, GetExtendedUdpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_OWNER_PID,
    MIB_UDP6ROW_OWNER_PID, MIB_UDPROW_OWNER_PID, TCP_TABLE_OWNER_PID_ALL, UDP_TABLE_OWNER_PID,
};
use windows_sys::Win32::Networking::WinSock::{AF_INET, AF_INET6};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess};

use super::parse::{self, Proto, Socket};

/// Read one table into a buffer. The first call gives the size, and the
/// second fills it. A table that grows between the two calls asks again.
fn table(read: impl Fn(*mut core::ffi::c_void, *mut u32) -> u32) -> Result<Vec<u8>> {
    let mut size = 0u32;
    for _ in 0..5 {
        let mut buffer = vec![0u8; size as usize];
        let pointer = if buffer.is_empty() {
            std::ptr::null_mut()
        } else {
            buffer.as_mut_ptr().cast()
        };
        let status = read(pointer, &mut size);
        if status == NO_ERROR {
            return Ok(buffer);
        }
        // ERROR_INSUFFICIENT_BUFFER: size now holds the size that it needs.
        if status != 122 {
            bail!("Windows did not give the socket table (error {status})");
        }
    }
    bail!("the socket table changed size five times while att read it")
}

/// The rows of a table: a count of four bytes, then the rows, aligned to
/// four bytes.
fn rows<T: Copy>(buffer: &[u8]) -> Vec<T> {
    if buffer.len() < 4 {
        return Vec::new();
    }
    let count = u32::from_ne_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as usize;
    let size = std::mem::size_of::<T>();
    (0..count)
        .filter_map(|i| {
            let start = 4 + i * size;
            let bytes = buffer.get(start..start + size)?;
            // SAFETY: the slice holds size_of::<T>() bytes, and T is a plain
            // struct of integers and byte arrays, which any bytes make valid.
            #[allow(unsafe_code)]
            Some(unsafe { std::ptr::read_unaligned(bytes.as_ptr().cast::<T>()) })
        })
        .collect()
}

pub fn sockets() -> Result<Vec<Socket>> {
    let names = names();
    let owner = |pid: u32| (Some(pid), names.get(&pid).cloned());
    let mut out = Vec::new();

    // SAFETY: each call gets a buffer and its size from table().
    #[allow(unsafe_code)]
    let tcp4 = table(|p, s| unsafe {
        GetExtendedTcpTable(p, s, 0, AF_INET as u32, TCP_TABLE_OWNER_PID_ALL, 0)
    })?;
    for row in rows::<MIB_TCPROW_OWNER_PID>(&tcp4) {
        let (pid, process) = owner(row.dwOwningPid);
        let remote_port = parse::windows_port(row.dwRemotePort);
        let state = parse::windows_state(row.dwState);
        out.push(Socket {
            proto: Proto::Tcp,
            address: parse::windows_address(&row.dwLocalAddr.to_ne_bytes()),
            port: parse::windows_port(row.dwLocalPort),
            remote: (state != "LISTEN").then(|| {
                parse::join(
                    &parse::windows_address(&row.dwRemoteAddr.to_ne_bytes()),
                    remote_port,
                )
            }),
            state: Some(state.to_owned()),
            pid,
            process,
        });
    }

    // SAFETY: as above.
    #[allow(unsafe_code)]
    let tcp6 = table(|p, s| unsafe {
        GetExtendedTcpTable(p, s, 0, AF_INET6 as u32, TCP_TABLE_OWNER_PID_ALL, 0)
    })?;
    for row in rows::<MIB_TCP6ROW_OWNER_PID>(&tcp6) {
        let (pid, process) = owner(row.dwOwningPid);
        let state = parse::windows_state(row.dwState);
        out.push(Socket {
            proto: Proto::Tcp,
            address: parse::windows_address(&row.ucLocalAddr),
            port: parse::windows_port(row.dwLocalPort),
            remote: (state != "LISTEN").then(|| {
                parse::join(
                    &parse::windows_address(&row.ucRemoteAddr),
                    parse::windows_port(row.dwRemotePort),
                )
            }),
            state: Some(state.to_owned()),
            pid,
            process,
        });
    }

    // SAFETY: as above.
    #[allow(unsafe_code)]
    let udp4 = table(|p, s| unsafe {
        GetExtendedUdpTable(p, s, 0, AF_INET as u32, UDP_TABLE_OWNER_PID, 0)
    })?;
    for row in rows::<MIB_UDPROW_OWNER_PID>(&udp4) {
        let (pid, process) = owner(row.dwOwningPid);
        out.push(Socket {
            proto: Proto::Udp,
            address: parse::windows_address(&row.dwLocalAddr.to_ne_bytes()),
            port: parse::windows_port(row.dwLocalPort),
            remote: None,
            state: None,
            pid,
            process,
        });
    }

    // SAFETY: as above.
    #[allow(unsafe_code)]
    let udp6 = table(|p, s| unsafe {
        GetExtendedUdpTable(p, s, 0, AF_INET6 as u32, UDP_TABLE_OWNER_PID, 0)
    })?;
    for row in rows::<MIB_UDP6ROW_OWNER_PID>(&udp6) {
        let (pid, process) = owner(row.dwOwningPid);
        out.push(Socket {
            proto: Proto::Udp,
            address: parse::windows_address(&row.ucLocalAddr),
            port: parse::windows_port(row.dwLocalPort),
            remote: None,
            state: None,
            pid,
            process,
        });
    }
    Ok(out)
}

/// The name of each process, from a snapshot. A standard user gets the
/// names of all processes this way.
fn names() -> HashMap<u32, String> {
    let mut map = HashMap::new();
    // SAFETY: the snapshot handle is checked and closed one time. dwSize is
    // set before the first call, as the API asks.
    #[allow(unsafe_code)]
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return map;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut more = Process32FirstW(snapshot, &mut entry) != 0;
        while more {
            let end = entry
                .szExeFile
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(entry.szExeFile.len());
            map.insert(
                entry.th32ProcessID,
                String::from_utf16_lossy(&entry.szExeFile[..end]),
            );
            more = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
    }
    map
}

pub fn stop(pid: u32) -> std::io::Result<()> {
    // SAFETY: the handle is checked and closed one time.
    #[allow(unsafe_code)]
    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
        if handle.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        let ok = TerminateProcess(handle, 1) != 0;
        let error = std::io::Error::last_os_error();
        CloseHandle(handle);
        if ok { Ok(()) } else { Err(error) }
    }
}
