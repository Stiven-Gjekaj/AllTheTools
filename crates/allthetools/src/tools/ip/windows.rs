//! The host file of `att ip` on Windows: GetAdaptersAddresses, which gives
//! each adapter with its name, its state, its hardware address, its MTU and
//! its addresses.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use anyhow::{Result, bail};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST, GetAdaptersAddresses,
    IP_ADAPTER_ADDRESSES_LH,
};
use windows_sys::Win32::Networking::WinSock::{AF_INET, AF_INET6, AF_UNSPEC, SOCKADDR};

use super::parse::{self, Address, Interface, Part};

const ERROR_BUFFER_OVERFLOW: u32 = 111;
/// IfOperStatusUp of IF_OPER_STATUS.
const OPER_UP: i32 = 1;
/// IF_TYPE_SOFTWARE_LOOPBACK.
const LOOPBACK: u32 = 24;

/// The address in a SOCKADDR of IPv4 or IPv6.
///
/// # Safety
///
/// `addr` is null, or it points to a socket address of the family that it
/// names.
#[allow(unsafe_code)]
unsafe fn ip(addr: *const SOCKADDR) -> Option<IpAddr> {
    if addr.is_null() {
        return None;
    }
    // SAFETY: the caller gives a valid socket address. IPv4 holds its
    // address at bytes 4 to 8, and IPv6 at bytes 8 to 24.
    unsafe {
        let bytes = addr.cast::<u8>();
        match (*addr).sa_family {
            AF_INET => {
                let b = std::slice::from_raw_parts(bytes.add(4), 4);
                Some(IpAddr::V4(Ipv4Addr::new(b[0], b[1], b[2], b[3])))
            }
            AF_INET6 => {
                let b = std::slice::from_raw_parts(bytes.add(8), 16);
                Some(IpAddr::V6(Ipv6Addr::from(<[u8; 16]>::try_from(b).ok()?)))
            }
            _ => None,
        }
    }
}

fn wide_text(text: *const u16) -> String {
    if text.is_null() {
        return String::new();
    }
    // SAFETY: Windows ends each name with a null.
    #[allow(unsafe_code)]
    unsafe {
        let mut len = 0;
        while *text.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(text, len))
    }
}

pub fn interfaces() -> Result<Vec<Interface>> {
    let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
    let mut len: u32 = 16 * 1024;
    let mut buffer: Vec<u64>;
    loop {
        // The buffer has the alignment of the records, which hold pointers.
        buffer = vec![0u64; (len as usize).div_ceil(8)];
        // SAFETY: the buffer has room for len bytes.
        #[allow(unsafe_code)]
        let status = unsafe {
            GetAdaptersAddresses(
                u32::from(AF_UNSPEC),
                flags,
                std::ptr::null(),
                buffer.as_mut_ptr().cast(),
                &mut len,
            )
        };
        match status {
            0 => break,
            ERROR_BUFFER_OVERFLOW => continue,
            code => bail!("Windows gave no network adapters (error {code})"),
        }
    }
    let mut parts = Vec::new();
    let mut at = buffer.as_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
    // SAFETY: each record of the chain lives in the buffer, and each pointer
    // in it is null or points into the buffer.
    #[allow(unsafe_code)]
    unsafe {
        while !at.is_null() {
            let a = &*at;
            at = a.Next;
            let up = a.OperStatus == OPER_UP;
            let len = (a.PhysicalAddressLength as usize).min(a.PhysicalAddress.len());
            let base = Part {
                name: wide_text(a.FriendlyName),
                up,
                running: up,
                loopback: a.IfType == LOOPBACK,
                mac: parse::mac(&a.PhysicalAddress[..len]),
                mtu: (a.Mtu != u32::MAX).then_some(a.Mtu),
                address: None,
            };
            parts.push(base.clone());
            let mut unicast = a.FirstUnicastAddress;
            while !unicast.is_null() {
                let u = &*unicast;
                unicast = u.Next;
                if let Some(address) = ip(u.Address.lpSockaddr) {
                    parts.push(Part {
                        address: Some(Address {
                            address,
                            prefix: u.OnLinkPrefixLength,
                        }),
                        ..base.clone()
                    });
                }
            }
        }
    }
    Ok(parse::merge(parts))
}
