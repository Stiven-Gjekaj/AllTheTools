//! The host file of `att wifi` on Windows: the native WLAN API. Its text
//! twin, `netsh wlan show networks`, changes its words with the language of
//! Windows, and the API does not.
//!
//! Windows 11 asks for permission for the location before it gives a scan
//! to a program. Without it, the API refuses with "access denied".

use std::collections::HashMap;

use anyhow::{Result, bail};
use windows_sys::Win32::Foundation::{ERROR_ACCESS_DENIED, HANDLE};
use windows_sys::Win32::NetworkManagement::WiFi::{
    DOT11_CIPHER_ALGO_NONE, WLAN_AVAILABLE_NETWORK_LIST, WLAN_BSS_LIST, WLAN_INTERFACE_INFO_LIST,
    WlanCloseHandle, WlanEnumInterfaces, WlanFreeMemory, WlanGetAvailableNetworkList,
    WlanGetNetworkBssList, WlanOpenHandle, WlanScan, dot11_BSS_type_any,
};

use super::parse::{self, Network};

struct Client(HANDLE);

impl Drop for Client {
    fn drop(&mut self) {
        // SAFETY: the handle came from WlanOpenHandle.
        #[allow(unsafe_code)]
        unsafe {
            WlanCloseHandle(self.0, std::ptr::null());
        }
    }
}

fn check(status: u32, what: &str) -> Result<()> {
    if status == 0 {
        return Ok(());
    }
    if status == ERROR_ACCESS_DENIED {
        bail!(
            "Windows refused the {what}. Windows 11 gives a Wi-Fi scan only to a program with \
             permission for the location. Turn on Location services, and \"Let desktop apps \
             access your location\", in Settings, Privacy and security, Location"
        );
    }
    bail!("Windows refused the {what} (error {status})")
}

pub fn scan() -> Result<Vec<Network>> {
    let mut version = 0u32;
    let mut handle: HANDLE = std::ptr::null_mut();
    // SAFETY: each pointer is valid. Each list that the API gives is freed
    // with WlanFreeMemory, and each read stays inside the count of its list.
    #[allow(unsafe_code)]
    unsafe {
        let status = WlanOpenHandle(2, std::ptr::null(), &mut version, &mut handle);
        // The WLAN service does not run on a computer with no Wi-Fi card.
        if status == 1062 {
            return Ok(Vec::new());
        }
        check(status, "connection to the WLAN service")?;
        let client = Client(handle);

        let mut interfaces: *mut WLAN_INTERFACE_INFO_LIST = std::ptr::null_mut();
        check(
            WlanEnumInterfaces(client.0, std::ptr::null(), &mut interfaces),
            "list of Wi-Fi cards",
        )?;
        let count = (*interfaces).dwNumberOfItems as usize;
        let first = std::ptr::addr_of!((*interfaces).InterfaceInfo)
            .cast::<windows_sys::Win32::NetworkManagement::WiFi::WLAN_INTERFACE_INFO>();
        let guids: Vec<_> = (0..count).map(|i| (*first.add(i)).InterfaceGuid).collect();
        WlanFreeMemory(interfaces.cast());

        let mut out = Vec::new();
        for guid in &guids {
            // A scan takes about four seconds. The list that comes after it
            // holds what the card found.
            WlanScan(
                client.0,
                guid,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
            );
            std::thread::sleep(std::time::Duration::from_secs(4));

            let mut security: HashMap<Vec<u8>, String> = HashMap::new();
            let mut available: *mut WLAN_AVAILABLE_NETWORK_LIST = std::ptr::null_mut();
            if WlanGetAvailableNetworkList(client.0, guid, 0, std::ptr::null(), &mut available) == 0
            {
                let count = (*available).dwNumberOfItems as usize;
                let first = std::ptr::addr_of!((*available).Network)
                    .cast::<windows_sys::Win32::NetworkManagement::WiFi::WLAN_AVAILABLE_NETWORK>(
                );
                for i in 0..count {
                    let n = &*first.add(i);
                    let ssid = n.dot11Ssid.ucSSID[..n.dot11Ssid.uSSIDLength as usize].to_vec();
                    let name = parse::windows_security(
                        n.dot11DefaultAuthAlgorithm,
                        n.dot11DefaultCipherAlgorithm == DOT11_CIPHER_ALGO_NONE,
                    );
                    security.entry(ssid).or_insert_with(|| name.to_owned());
                }
                WlanFreeMemory(available.cast());
            }

            let mut list: *mut WLAN_BSS_LIST = std::ptr::null_mut();
            check(
                WlanGetNetworkBssList(
                    client.0,
                    guid,
                    std::ptr::null(),
                    dot11_BSS_type_any,
                    0,
                    std::ptr::null(),
                    &mut list,
                ),
                "list of networks",
            )?;
            let count = (*list).dwNumberOfItems as usize;
            let first = std::ptr::addr_of!((*list).wlanBssEntries)
                .cast::<windows_sys::Win32::NetworkManagement::WiFi::WLAN_BSS_ENTRY>();
            for i in 0..count {
                let entry = &*first.add(i);
                let ssid = entry.dot11Ssid.ucSSID[..entry.dot11Ssid.uSSIDLength as usize].to_vec();
                let mhz = entry.ulChCenterFrequency / 1000;
                out.push(Network {
                    ssid: (!ssid.is_empty()).then(|| String::from_utf8_lossy(&ssid).into_owned()),
                    bssid: Some(parse::mac(&entry.dot11Bssid)),
                    signal_dbm: Some(entry.lRssi),
                    channel: parse::channel(mhz),
                    band_ghz: parse::band(mhz),
                    security: security
                        .get(&ssid)
                        .cloned()
                        .unwrap_or_else(|| "unknown".to_owned()),
                });
            }
            WlanFreeMemory(list.cast());
        }
        Ok(out)
    }
}

pub const HIDDEN_NAMES: &str = "";
