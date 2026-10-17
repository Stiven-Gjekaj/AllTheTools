//! The host file of `att ip` on macOS and Linux: getifaddrs, which gives
//! one record for each address of each interface. The record of the link
//! layer gives the hardware address: AF_LINK on macOS and AF_PACKET on
//! Linux.

use std::ffi::CStr;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use anyhow::{Result, bail};

use super::parse::{self, Address, Interface, Part};

/// The IP address in a socket address of IPv4 or IPv6, or its raw bytes for
/// a netmask.
///
/// # Safety
///
/// `addr` is null, or it points to a socket address of the family that it
/// names.
#[allow(unsafe_code)]
unsafe fn ip_bytes(addr: *const libc::sockaddr) -> Option<Vec<u8>> {
    if addr.is_null() {
        return None;
    }
    // SAFETY: the caller gives a valid socket address, and its family names
    // its type.
    unsafe {
        match i32::from((*addr).sa_family) {
            libc::AF_INET => {
                let v4 = &*addr.cast::<libc::sockaddr_in>();
                Some(v4.sin_addr.s_addr.to_ne_bytes().to_vec())
            }
            libc::AF_INET6 => {
                let v6 = &*addr.cast::<libc::sockaddr_in6>();
                Some(v6.sin6_addr.s6_addr.to_vec())
            }
            _ => None,
        }
    }
}

/// The hardware address and the MTU of a record of the link layer.
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
unsafe fn link(entry: &libc::ifaddrs) -> (Option<String>, Option<u32>) {
    // SAFETY: the record of the family AF_LINK holds a sockaddr_dl, whose
    // data holds the name and then the address, and its ifa_data points to
    // an if_data.
    unsafe {
        let dl = &*entry.ifa_addr.cast::<libc::sockaddr_dl>();
        let data = std::ptr::addr_of!(dl.sdl_data).cast::<u8>();
        let bytes = std::slice::from_raw_parts(
            data.add(usize::from(dl.sdl_nlen)),
            usize::from(dl.sdl_alen),
        );
        let mtu =
            (!entry.ifa_data.is_null()).then(|| (*entry.ifa_data.cast::<libc::if_data>()).ifi_mtu);
        (parse::mac(bytes), mtu)
    }
}

/// The hardware address of a record of the link layer, and the MTU from
/// sysfs.
#[cfg(target_os = "linux")]
#[allow(unsafe_code)]
unsafe fn link(entry: &libc::ifaddrs) -> (Option<String>, Option<u32>) {
    // SAFETY: the record of the family AF_PACKET holds a sockaddr_ll.
    let ll = unsafe { &*entry.ifa_addr.cast::<libc::sockaddr_ll>() };
    let len = usize::from(ll.sll_halen).min(ll.sll_addr.len());
    // SAFETY: the name of the record ends with a null.
    let name = unsafe { CStr::from_ptr(entry.ifa_name) }.to_string_lossy();
    let mtu = std::fs::read_to_string(format!("/sys/class/net/{name}/mtu"))
        .ok()
        .and_then(|s| s.trim().parse().ok());
    (parse::mac(&ll.sll_addr[..len]), mtu)
}

#[cfg(target_os = "macos")]
const LINK: i32 = libc::AF_LINK;
#[cfg(target_os = "linux")]
const LINK: i32 = libc::AF_PACKET;

pub fn interfaces() -> Result<Vec<Interface>> {
    let mut list: *mut libc::ifaddrs = std::ptr::null_mut();
    // SAFETY: getifaddrs points `list` at a chain of records, which
    // freeifaddrs frees at the end.
    #[allow(unsafe_code)]
    if unsafe { libc::getifaddrs(&mut list) } != 0 {
        bail!(
            "the host gave no network interfaces: {}",
            std::io::Error::last_os_error()
        );
    }
    let mut parts = Vec::new();
    let mut at = list;
    // SAFETY: each record of the chain is valid until freeifaddrs, and its
    // name ends with a null.
    #[allow(unsafe_code)]
    unsafe {
        while !at.is_null() {
            let entry = &*at;
            at = entry.ifa_next;
            let flags = entry.ifa_flags as i32;
            let mut part = Part {
                name: CStr::from_ptr(entry.ifa_name)
                    .to_string_lossy()
                    .into_owned(),
                up: flags & libc::IFF_UP != 0,
                running: flags & libc::IFF_RUNNING != 0,
                loopback: flags & libc::IFF_LOOPBACK != 0,
                ..Part::default()
            };
            if entry.ifa_addr.is_null() {
                parts.push(part);
                continue;
            }
            let family = i32::from((*entry.ifa_addr).sa_family);
            if family == LINK {
                (part.mac, part.mtu) = link(entry);
            } else if let Some(bytes) = ip_bytes(entry.ifa_addr) {
                let address = match bytes.len() {
                    4 => IpAddr::V4(Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3])),
                    _ => IpAddr::V6(Ipv6Addr::from(
                        <[u8; 16]>::try_from(bytes.as_slice()).unwrap_or([0; 16]),
                    )),
                };
                let prefix = ip_bytes(entry.ifa_netmask)
                    .map(|m| parse::prefix(&m))
                    .unwrap_or(if address.is_ipv4() { 32 } else { 128 });
                part.address = Some(Address { address, prefix });
            }
            parts.push(part);
        }
        libc::freeifaddrs(list);
    }
    Ok(parse::merge(parts))
}
