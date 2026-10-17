//! Turn what each host says about its sockets into one list.
//!
//! This module makes no system call and names no type of a host crate, so
//! its tests run on every host.

use std::net::{Ipv4Addr, Ipv6Addr};

use serde::Serialize;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Proto {
    #[default]
    Tcp,
    Udp,
}

/// One socket, and the process that holds it when the host says.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct Socket {
    pub proto: Proto,
    /// The local address, with no port. `*` means each address.
    pub address: String,
    pub port: u16,
    /// The other end, as `address:port`, for a connected socket.
    pub remote: Option<String>,
    /// The state of a TCP socket, as LISTEN or ESTABLISHED.
    pub state: Option<String>,
    pub pid: Option<u32>,
    pub process: Option<String>,
}

impl Socket {
    /// A socket that waits for others: a TCP socket in LISTEN, or a UDP
    /// socket with no other end.
    pub fn listens(&self) -> bool {
        match self.proto {
            Proto::Tcp => self.state.as_deref() == Some("LISTEN"),
            Proto::Udp => self.remote.is_none() && self.port != 0,
        }
    }

    /// The local end for a person, as `127.0.0.1:3000` or `[::1]:3000`.
    pub fn local(&self) -> String {
        join(&self.address, self.port)
    }
}

/// An address and a port, with brackets around an IPv6 address.
pub fn join(address: &str, port: u16) -> String {
    if address.contains(':') {
        format!("[{address}]:{port}")
    } else {
        format!("{address}:{port}")
    }
}

fn show_v4(address: Ipv4Addr) -> String {
    if address.is_unspecified() {
        "*".to_owned()
    } else {
        address.to_string()
    }
}

fn show_v6(address: Ipv6Addr) -> String {
    if address.is_unspecified() {
        "*".to_owned()
    } else if let Some(v4) = address.to_ipv4_mapped() {
        v4.to_string()
    } else {
        address.to_string()
    }
}

/// The name of a TCP state of Linux, from its number in /proc/net/tcp.
fn linux_state(code: u8) -> &'static str {
    match code {
        0x01 => "ESTABLISHED",
        0x02 => "SYN_SENT",
        0x03 => "SYN_RECV",
        0x04 => "FIN_WAIT1",
        0x05 => "FIN_WAIT2",
        0x06 => "TIME_WAIT",
        0x07 => "CLOSE",
        0x08 => "CLOSE_WAIT",
        0x09 => "LAST_ACK",
        0x0A => "LISTEN",
        0x0B => "CLOSING",
        _ => "UNKNOWN",
    }
}

/// An address and a port of /proc/net, as `0100007F:0CEA`. Each group of
/// four bytes of the address is in the order of the host, which is little
/// endian on each processor that Linux runs this tool on.
fn linux_end(text: &str) -> Option<(String, u16)> {
    let (address, port) = text.split_once(':')?;
    let port = u16::from_str_radix(port, 16).ok()?;
    let mut bytes = Vec::new();
    for group in address.as_bytes().chunks(8) {
        let group = std::str::from_utf8(group).ok()?;
        bytes.extend_from_slice(&u32::from_str_radix(group, 16).ok()?.to_le_bytes());
    }
    let address = match bytes.len() {
        4 => show_v4(Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3])),
        16 => {
            let array: [u8; 16] = bytes.try_into().ok()?;
            show_v6(Ipv6Addr::from(array))
        }
        _ => return None,
    };
    Some((address, port))
}

/// Read /proc/net/tcp, tcp6, udp or udp6. Each socket comes with its inode,
/// which the host file matches to a process.
pub fn linux(proto: Proto, text: &str) -> Vec<(Socket, u64)> {
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let (address, port) = linux_end(fields.get(1)?)?;
            let (remote, remote_port) = linux_end(fields.get(2)?)?;
            let state = u8::from_str_radix(fields.get(3)?, 16).ok()?;
            let inode = fields.get(9)?.parse().ok()?;
            let connected = remote_port != 0;
            Some((
                Socket {
                    proto,
                    address,
                    port,
                    remote: connected.then(|| join(&remote, remote_port)),
                    state: (proto == Proto::Tcp).then(|| linux_state(state).to_owned()),
                    pid: None,
                    process: None,
                },
                inode,
            ))
        })
        .collect()
}

/// The number of the socket in a link of /proc/<pid>/fd, as `socket:[1234]`.
pub fn linux_socket_inode(link: &str) -> Option<u64> {
    link.strip_prefix("socket:[")?
        .strip_suffix(']')?
        .parse()
        .ok()
}

/// Read the output of `lsof -nP -i -F pcftPnT` on macOS. Each field is on a
/// line of its own, and the first letter names the field: p is the process
/// number, c the command, f a file, t the type, P the protocol, n the name
/// and T a TCP detail such as `ST=LISTEN`.
pub fn lsof(text: &str) -> Vec<Socket> {
    let mut out: Vec<Socket> = Vec::new();
    let mut pid = None;
    let mut command: Option<String> = None;
    let mut current: Option<Socket> = None;
    let flush = |current: &mut Option<Socket>, out: &mut Vec<Socket>| {
        if let Some(socket) = current.take()
            && !out.contains(&socket)
        {
            out.push(socket);
        }
    };
    for line in text.lines() {
        let Some(tag) = line.chars().next() else {
            continue;
        };
        let value = &line[tag.len_utf8()..];
        match tag {
            'p' => {
                flush(&mut current, &mut out);
                pid = value.parse().ok();
                command = None;
            }
            'c' => command = Some(value.to_owned()),
            'f' => flush(&mut current, &mut out),
            'P' => {
                let proto = match value {
                    "TCP" => Proto::Tcp,
                    "UDP" => Proto::Udp,
                    _ => continue,
                };
                current = Some(Socket {
                    proto,
                    address: String::new(),
                    port: 0,
                    remote: None,
                    state: None,
                    pid,
                    process: command.clone(),
                });
            }
            'n' => {
                if let Some(socket) = current.as_mut() {
                    let (local, remote) = match value.split_once("->") {
                        Some((local, remote)) => (local, Some(remote.to_owned())),
                        None => (value, None),
                    };
                    let (address, port) = split_end(local);
                    socket.address = address;
                    socket.port = port;
                    socket.remote = remote;
                }
            }
            'T' => {
                if let (Some(socket), Some(state)) = (current.as_mut(), value.strip_prefix("ST=")) {
                    socket.state = Some(state.to_owned());
                }
            }
            _ => {}
        }
    }
    flush(&mut current, &mut out);
    out.retain(|s| !s.address.is_empty() || s.port != 0);
    out
}

/// `*:7000`, `127.0.0.1:5000` or `[::1]:5000` gives the address and the
/// port. A port of `*` is 0.
fn split_end(text: &str) -> (String, u16) {
    let (address, port) = match text.rsplit_once(':') {
        Some(parts) => parts,
        None => return (text.to_owned(), 0),
    };
    let address = address.trim_start_matches('[').trim_end_matches(']');
    (address.to_owned(), port.parse().unwrap_or(0))
}

/// The name of a TCP state of Windows, from MIB_TCP_STATE.
pub fn windows_state(code: u32) -> &'static str {
    match code {
        1 => "CLOSED",
        2 => "LISTEN",
        3 => "SYN_SENT",
        4 => "SYN_RECV",
        5 => "ESTABLISHED",
        6 => "FIN_WAIT1",
        7 => "FIN_WAIT2",
        8 => "CLOSE_WAIT",
        9 => "CLOSING",
        10 => "LAST_ACK",
        11 => "TIME_WAIT",
        12 => "DELETE_TCB",
        _ => "UNKNOWN",
    }
}

/// A port of the IP Helper tables: the low two bytes, in network order.
pub fn windows_port(raw: u32) -> u16 {
    u16::from_be((raw & 0xFFFF) as u16)
}

/// An address of the IP Helper tables: four or sixteen bytes in network
/// order.
pub fn windows_address(bytes: &[u8]) -> String {
    match bytes.len() {
        4 => show_v4(Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3])),
        16 => {
            let mut array = [0u8; 16];
            array.copy_from_slice(bytes);
            show_v6(Ipv6Addr::from(array))
        }
        _ => "?".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The lines of /proc/net/tcp and /proc/net/tcp6 in the form that the
    // kernel writes them, with a listening socket and a connected one.
    const TCP: &str = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode
   0: 0100007F:0CEA 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 23456 1 0000000000000000 100 0 0 10 0
   1: 0F02000A:C8F2 22D8B85D:01BB 01 00000000:00000000 02:00000BB8 00000000  1000        0 34567 2 0000000000000000 20 4 30 10 -1
";
    const TCP6: &str = "  sl  local_address                         remote_address                        st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode
   0: 00000000000000000000000001000000:1F90 00000000000000000000000000000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 45678 1 0000000000000000 100 0 0 10 0
   1: 00000000000000000000000000000000:0016 00000000000000000000000000000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 56789 1 0000000000000000 100 0 0 10 0
";

    #[test]
    fn linux_reads_the_address_the_state_and_the_inode() {
        let sockets = linux(Proto::Tcp, TCP);
        assert_eq!(sockets.len(), 2);
        let (listen, inode) = &sockets[0];
        assert_eq!(listen.local(), "127.0.0.1:3306");
        assert_eq!(listen.state.as_deref(), Some("LISTEN"));
        assert!(listen.listens());
        assert_eq!(*inode, 23456);
        let (connected, _) = &sockets[1];
        assert_eq!(connected.local(), "10.0.2.15:51442");
        assert_eq!(connected.remote.as_deref(), Some("93.184.216.34:443"));
        assert!(!connected.listens());
    }

    #[test]
    fn linux_reads_ipv6() {
        let sockets = linux(Proto::Tcp, TCP6);
        assert_eq!(sockets[0].0.local(), "[::1]:8080");
        assert_eq!(sockets[1].0.local(), "*:22");
    }

    #[test]
    fn a_socket_link_gives_its_inode() {
        assert_eq!(linux_socket_inode("socket:[23456]"), Some(23456));
        assert_eq!(linux_socket_inode("/dev/null"), None);
        assert_eq!(linux_socket_inode("pipe:[12]"), None);
    }

    // Lines that `lsof -nP -i -F pcftPnT` gave on macOS, put together as two
    // processes with four sockets.
    const LSOF: &str = "p446
cControlCenter
f9
tIPv4
PTCP
n*:7000
TST=LISTEN
TQR=0
TQS=0
f10
tIPv6
PTCP
n*:7000
TST=LISTEN
TQR=0
TQS=0
p612
crapportd
f12
tIPv6
PTCP
n[fe80:c::9f:1497:dfea:4b4f]:49152->[fe80:c::1f:3475:600e:e0ea]:49808
TST=ESTABLISHED
f13
tIPv4
PUDP
n*:3722
";

    #[test]
    fn lsof_reads_each_socket_of_each_process() {
        let sockets = lsof(LSOF);
        assert_eq!(sockets.len(), 3, "two sockets on one port are one line");
        assert_eq!(sockets[0].local(), "*:7000");
        assert_eq!(sockets[0].pid, Some(446));
        assert_eq!(sockets[0].process.as_deref(), Some("ControlCenter"));
        assert!(sockets[0].listens());
        let connected = &sockets[1];
        assert_eq!(connected.port, 49152);
        assert_eq!(connected.state.as_deref(), Some("ESTABLISHED"));
        assert_eq!(
            connected.remote.as_deref(),
            Some("[fe80:c::1f:3475:600e:e0ea]:49808")
        );
        let udp = &sockets[2];
        assert_eq!(udp.proto, Proto::Udp);
        assert!(udp.listens());
        assert_eq!(udp.process.as_deref(), Some("rapportd"));
    }

    #[test]
    fn windows_ports_are_in_network_order() {
        // Port 3000 is 0x0BB8, and the table holds it as B8 0B in the low
        // bytes of a little-endian number: 0x0000B80B.
        assert_eq!(windows_port(0x0000_B80B), 3000);
        assert_eq!(windows_address(&[127, 0, 0, 1]), "127.0.0.1");
        assert_eq!(windows_address(&[0; 4]), "*");
        assert_eq!(windows_state(2), "LISTEN");
    }
}
