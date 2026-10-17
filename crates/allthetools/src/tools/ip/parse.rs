//! The network interfaces of `att ip`: the state, the hardware address, the
//! MTU and the addresses of each one, and the table for a person.
//!
//! This module makes no system call, so its tests run on every host.

use std::net::IpAddr;

use serde::Serialize;

/// One address of an interface, with the length of its prefix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Address {
    pub address: IpAddr,
    pub prefix: u8,
}

/// One network interface.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct Interface {
    pub name: String,
    /// `up`, `no link` or `down`.
    pub state: String,
    pub loopback: bool,
    pub mac: Option<String>,
    pub mtu: Option<u32>,
    pub addresses: Vec<Address>,
}

/// The state from the flags of the interface: on and with a link, on with
/// no link, or off.
pub fn state(up: bool, running: bool) -> String {
    match (up, running) {
        (true, true) => "up",
        (true, false) => "no link",
        _ => "down",
    }
    .to_owned()
}

/// The length of the prefix of a netmask: the count of its leading one
/// bits.
pub fn prefix(mask: &[u8]) -> u8 {
    let mut count = 0;
    for byte in mask {
        count += byte.leading_ones();
        if *byte != 0xff {
            break;
        }
    }
    count as u8
}

/// A hardware address as hex pairs with colons. A missing address, or one
/// of zeros only, gives None.
pub fn mac(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() || bytes.iter().all(|b| *b == 0) {
        return None;
    }
    Some(
        bytes
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(":"),
    )
}

/// The parts of one interface, as the host gives them one at a time.
#[derive(Debug, Clone, Default)]
pub struct Part {
    pub name: String,
    pub up: bool,
    pub running: bool,
    pub loopback: bool,
    pub mac: Option<String>,
    pub mtu: Option<u32>,
    pub address: Option<Address>,
}

/// Put the parts of each interface together, in the order of the first part
/// of each.
pub fn merge(parts: Vec<Part>) -> Vec<Interface> {
    let mut out: Vec<Interface> = Vec::new();
    for part in parts {
        let index = match out.iter().position(|i| i.name == part.name) {
            Some(i) => i,
            None => {
                out.push(Interface {
                    name: part.name.clone(),
                    state: state(part.up, part.running),
                    loopback: part.loopback,
                    mac: None,
                    mtu: None,
                    addresses: Vec::new(),
                });
                out.len() - 1
            }
        };
        let interface = &mut out[index];
        interface.mac = interface.mac.take().or(part.mac);
        interface.mtu = interface.mtu.or(part.mtu);
        if let Some(address) = part.address
            && !interface.addresses.contains(&address)
        {
            interface.addresses.push(address);
        }
    }
    for interface in &mut out {
        // IPv4 first, then IPv6, each in its own order.
        interface.addresses.sort_by_key(|a| a.address.is_ipv6());
    }
    out
}

/// The rows of the table: one row for each address, and the name, the state,
/// the MAC and the MTU on the first row of each interface.
pub fn rows(interfaces: &[Interface]) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    for i in interfaces {
        let first = vec![
            i.name.clone(),
            i.state.clone(),
            i.mac.clone().unwrap_or_else(|| "-".to_owned()),
            i.mtu
                .map(|m| m.to_string())
                .unwrap_or_else(|| "-".to_owned()),
        ];
        if i.addresses.is_empty() {
            rows.push(first);
            continue;
        }
        for (n, a) in i.addresses.iter().enumerate() {
            let mut row = if n == 0 {
                first.clone()
            } else {
                vec![String::new(); 4]
            };
            row.push(format!("{}/{}", a.address, a.prefix));
            rows.push(row);
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_netmask_gives_its_prefix() {
        assert_eq!(prefix(&[255, 255, 255, 0]), 24);
        assert_eq!(prefix(&[255, 255, 240, 0]), 20);
        assert_eq!(prefix(&[0, 0, 0, 0]), 0);
        assert_eq!(prefix(&[0xff; 16]), 128);
        let mut v6 = [0u8; 16];
        v6[..8].fill(0xff);
        assert_eq!(prefix(&v6), 64);
    }

    #[test]
    fn a_mac_of_zeros_is_none() {
        assert_eq!(
            mac(&[0xaa, 0xbb, 0xcc, 0x00, 0x11, 0x22]).as_deref(),
            Some("aa:bb:cc:00:11:22")
        );
        assert_eq!(mac(&[0; 6]), None);
        assert_eq!(mac(&[]), None);
    }

    #[test]
    fn the_flags_give_the_state() {
        assert_eq!(state(true, true), "up");
        assert_eq!(state(true, false), "no link");
        assert_eq!(state(false, false), "down");
    }

    fn part(name: &str, address: Option<(&str, u8)>) -> Part {
        Part {
            name: name.to_owned(),
            up: true,
            running: true,
            address: address.map(|(a, p)| Address {
                address: a.parse().unwrap(),
                prefix: p,
            }),
            ..Part::default()
        }
    }

    #[test]
    fn the_parts_of_an_interface_join_with_ipv4_first() {
        let mut link = part("en0", None);
        link.mac = Some("aa:bb:cc:dd:ee:ff".to_owned());
        link.mtu = Some(1500);
        let list = merge(vec![
            part("lo0", Some(("127.0.0.1", 8))),
            part("en0", Some(("fe80::1", 64))),
            link,
            part("en0", Some(("192.168.1.20", 24))),
            part("en0", Some(("192.168.1.20", 24))),
        ]);
        assert_eq!(list.len(), 2);
        let en0 = &list[1];
        assert_eq!(en0.mac.as_deref(), Some("aa:bb:cc:dd:ee:ff"));
        assert_eq!(en0.mtu, Some(1500));
        assert_eq!(en0.addresses.len(), 2, "the same address counts once");
        assert!(en0.addresses[0].address.is_ipv4());
        let rows = rows(&list);
        assert_eq!(
            rows[1],
            ["en0", "up", "aa:bb:cc:dd:ee:ff", "1500", "192.168.1.20/24"]
        );
        assert_eq!(rows[2], ["", "", "", "", "fe80::1/64"]);
    }
}
