//! Turn what each host says about its USB devices into one list.
//!
//! This module makes no system call and names no type of a host crate, so
//! its tests run on every host.

use std::collections::BTreeMap;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// One USB device.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Device {
    /// The number of the bus. Windows gives no bus number.
    pub bus: Option<u32>,
    /// The address of the device on its bus. Windows gives none.
    pub address: Option<u32>,
    pub vendor_id: u16,
    pub product_id: u16,
    /// The name of the maker, as the device reports it.
    pub vendor: Option<String>,
    /// The name of the product, as the device reports it.
    pub product: Option<String>,
    pub serial: Option<String>,
    /// The speed of the link in megabits per second.
    pub speed_mbps: Option<f64>,
    /// The class of the device: 9 is a hub.
    pub class: Option<u8>,
}

impl Device {
    /// The line that `lsusb` of Linux prints for the device.
    pub fn line(&self) -> String {
        let number = |n: Option<u32>| n.map_or("---".to_owned(), |n| format!("{n:03}"));
        let mut name = String::new();
        for part in [&self.vendor, &self.product].into_iter().flatten() {
            if !name.is_empty() {
                name.push(' ');
            }
            name.push_str(part);
        }
        format!(
            "Bus {} Device {}: ID {:04x}:{:04x} {}",
            number(self.bus),
            number(self.address),
            self.vendor_id,
            self.product_id,
            name
        )
        .trim_end()
        .to_owned()
    }
}

/// A speed for a person: 480M, 5G, 1.5M.
pub fn speed(mbps: f64) -> String {
    if mbps >= 1000.0 {
        let gbps = mbps / 1000.0;
        if gbps.fract() == 0.0 {
            format!("{gbps}G")
        } else {
            format!("{gbps:.1}G")
        }
    } else if mbps.fract() == 0.0 {
        format!("{mbps}M")
    } else {
        format!("{mbps:.1}M")
    }
}

/// Put the devices in the order of their bus and their address.
pub fn sort(devices: &mut [Device]) {
    devices.sort_by_key(|d| {
        (
            d.bus.unwrap_or(u32::MAX),
            d.address.unwrap_or(u32::MAX),
            d.vendor_id,
            d.product_id,
        )
    });
}

/// A text that a device reports, or `None` when it reports nothing useful.
fn text(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_owned())
    }
}

/// The files of one directory under /sys/bus/usb/devices on Linux, by name.
pub type SysfsDevice = BTreeMap<String, String>;

/// Read the devices from sysfs. A directory with no `idVendor` is an
/// interface of a device, and not a device.
pub fn linux(dirs: &[SysfsDevice]) -> Vec<Device> {
    let mut devices: Vec<Device> = dirs
        .iter()
        .filter_map(|attrs| {
            let get = |name: &str| attrs.get(name).map(|v| v.trim());
            let hex = |name: &str| get(name).and_then(|v| u16::from_str_radix(v, 16).ok());
            Some(Device {
                bus: get("busnum").and_then(|v| v.parse().ok()),
                address: get("devnum").and_then(|v| v.parse().ok()),
                vendor_id: hex("idVendor")?,
                product_id: hex("idProduct")?,
                vendor: text(get("manufacturer")),
                product: text(get("product")),
                serial: text(get("serial")),
                speed_mbps: get("speed").and_then(|v| v.parse().ok()),
                class: get("bDeviceClass").and_then(|v| u8::from_str_radix(v, 16).ok()),
            })
        })
        .collect();
    sort(&mut devices);
    devices
}

/// Read the devices from `ioreg -p IOUSB -l -r -c IOUSBHostDevice -a` on
/// macOS. Each device holds the devices behind it, so the walk goes into
/// each one.
pub fn macos(plist: &[u8]) -> Result<Vec<Device>> {
    let value = plist::Value::from_reader_xml(plist)
        .context("ioreg gave a property list that is not valid")?;
    let mut devices = Vec::new();
    if let Some(top) = value.as_array() {
        for entry in top {
            mac_walk(entry, &mut devices);
        }
    }
    sort(&mut devices);
    Ok(devices)
}

fn mac_walk(entry: &plist::Value, out: &mut Vec<Device>) {
    let Some(dict) = entry.as_dictionary() else {
        return;
    };
    let int = |key: &str| dict.get(key).and_then(plist::Value::as_unsigned_integer);
    let string = |key: &str| text(dict.get(key).and_then(plist::Value::as_string));
    if let (Some(vendor_id), Some(product_id)) = (int("idVendor"), int("idProduct")) {
        let location = int("locationID");
        out.push(Device {
            // The first byte of the location is the controller, from zero.
            // Linux counts its buses from one, so this does too.
            bus: location.map(|l| ((l >> 24) & 0xFF) as u32 + 1),
            address: int("USB Address").map(|a| a as u32),
            vendor_id: vendor_id as u16,
            product_id: product_id as u16,
            vendor: string("USB Vendor Name").or_else(|| string("kUSBVendorString")),
            product: string("USB Product Name").or_else(|| string("kUSBProductString")),
            serial: string("USB Serial Number").or_else(|| string("kUSBSerialNumberString")),
            speed_mbps: int("UsbLinkSpeed").map(|bits| bits as f64 / 1_000_000.0),
            class: int("bDeviceClass").map(|c| c as u8),
        });
    }
    if let Some(children) = dict
        .get("IORegistryEntryChildren")
        .and_then(plist::Value::as_array)
    {
        for child in children {
            mac_walk(child, out);
        }
    }
}

/// One row of Win32_PnPEntity, as PowerShell writes it in JSON.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PnpEntity {
    #[serde(rename = "PNPDeviceID")]
    pnp_device_id: Option<String>,
    name: Option<String>,
    manufacturer: Option<String>,
}

/// Read the devices from the JSON of Win32_PnPEntity on Windows.
///
/// The ID of a device is `USB\VID_05AC&PID_12A8\<instance>`. An ID with
/// `&MI_` is an interface of a device, and an ID with no `VID_` is a root
/// hub. Neither is a device of its own. The instance is the serial number
/// when it holds no `&`, because Windows makes up an instance with `&` when
/// the device has no serial number.
pub fn windows(json: &str) -> Result<Vec<Device>> {
    let rows: Vec<PnpEntity> = if json.trim().is_empty() {
        Vec::new()
    } else {
        serde_json::from_str(json).context("PowerShell gave JSON that is not a list of devices")?
    };
    let mut devices: Vec<Device> = rows
        .iter()
        .filter_map(|row| {
            let id = row.pnp_device_id.as_deref()?;
            let mut parts = id.split('\\');
            let bus = parts.next()?;
            let ids = parts.next()?.to_ascii_uppercase();
            let instance = parts.next().unwrap_or("");
            if !bus.eq_ignore_ascii_case("USB") || ids.contains("&MI_") {
                return None;
            }
            let field = |key: &str| {
                let start = ids.find(key)? + key.len();
                u16::from_str_radix(ids.get(start..start + 4)?, 16).ok()
            };
            Some(Device {
                bus: None,
                address: None,
                vendor_id: field("VID_")?,
                product_id: field("PID_")?,
                vendor: text(row.manufacturer.as_deref()).filter(|m| !m.starts_with('(')),
                product: text(row.name.as_deref()),
                serial: (!instance.contains('&'))
                    .then(|| text(Some(instance)))
                    .flatten(),
                speed_mbps: None,
                class: None,
            })
        })
        .collect();
    sort(&mut devices);
    Ok(devices)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sysfs(pairs: &[(&str, &str)]) -> SysfsDevice {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), format!("{v}\n")))
            .collect()
    }

    #[test]
    fn linux_reads_each_device_and_skips_each_interface() {
        let dirs = [
            sysfs(&[
                ("busnum", "1"),
                ("devnum", "4"),
                ("idVendor", "0781"),
                ("idProduct", "5583"),
                ("manufacturer", "SanDisk"),
                ("product", "Ultra Fit"),
                ("serial", "4C530001"),
                ("speed", "5000"),
                ("bDeviceClass", "00"),
            ]),
            sysfs(&[("bInterfaceClass", "08"), ("bInterfaceNumber", "00")]),
            sysfs(&[
                ("busnum", "1"),
                ("devnum", "1"),
                ("idVendor", "1d6b"),
                ("idProduct", "0002"),
                ("manufacturer", "Linux 6.8.0 xhci-hcd"),
                ("product", "xHCI Host Controller"),
                ("speed", "480"),
                ("bDeviceClass", "09"),
            ]),
        ];
        let devices = linux(&dirs);
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].address, Some(1), "the order is by address");
        let stick = &devices[1];
        assert_eq!(
            stick.line(),
            "Bus 001 Device 004: ID 0781:5583 SanDisk Ultra Fit"
        );
        assert_eq!(stick.serial.as_deref(), Some("4C530001"));
        assert_eq!(stick.speed_mbps, Some(5000.0));
        assert_eq!(devices[0].class, Some(9));
    }

    // Captured with `ioreg -p IOUSB -l -w0 -r -c IOUSBHostDevice -a` on a
    // MacBook with a hub, and cut down to the keys that the parser reads.
    const IOREG: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<array>
  <dict>
    <key>IORegistryEntryName</key><string>USB2.1 Hub</string>
    <key>idVendor</key><integer>1507</integer>
    <key>idProduct</key><integer>1552</integer>
    <key>USB Vendor Name</key><string>GenesysLogic</string>
    <key>USB Product Name</key><string>USB2.1 Hub</string>
    <key>locationID</key><integer>17825792</integer>
    <key>USB Address</key><integer>2</integer>
    <key>UsbLinkSpeed</key><integer>480000000</integer>
    <key>bDeviceClass</key><integer>9</integer>
    <key>IORegistryEntryChildren</key>
    <array>
      <dict>
        <key>IORegistryEntryName</key><string>Disk 2.0</string>
        <key>idVendor</key><integer>13421</integer>
        <key>idProduct</key><integer>22136</integer>
        <key>USB Vendor Name</key><string>USB</string>
        <key>USB Product Name</key><string>Disk 2.0</string>
        <key>USB Serial Number</key><string>7505029105146747</string>
        <key>locationID</key><integer>17969152</integer>
        <key>USB Address</key><integer>8</integer>
        <key>UsbLinkSpeed</key><integer>480000000</integer>
        <key>bDeviceClass</key><integer>0</integer>
      </dict>
    </array>
  </dict>
  <dict>
    <key>IORegistryEntryName</key><string>AX88179A</string>
    <key>idVendor</key><integer>2965</integer>
    <key>idProduct</key><integer>6032</integer>
    <key>kUSBVendorString</key><string>ASIX</string>
    <key>kUSBProductString</key><string>AX88179A</string>
    <key>locationID</key><integer>19070976</integer>
    <key>USB Address</key><integer>7</integer>
    <key>UsbLinkSpeed</key><integer>5000000000</integer>
  </dict>
</array>
</plist>"#;

    #[test]
    fn macos_reads_each_device_behind_a_hub() {
        let devices = macos(IOREG.as_bytes()).unwrap();
        let lines: Vec<String> = devices.iter().map(Device::line).collect();
        assert_eq!(
            lines,
            [
                "Bus 002 Device 002: ID 05e3:0610 GenesysLogic USB2.1 Hub",
                "Bus 002 Device 007: ID 0b95:1790 ASIX AX88179A",
                "Bus 002 Device 008: ID 346d:5678 USB Disk 2.0",
            ]
        );
        assert_eq!(devices[2].serial.as_deref(), Some("7505029105146747"));
        assert_eq!(devices[1].speed_mbps, Some(5000.0));
    }

    #[test]
    fn macos_refuses_text_that_is_not_a_property_list() {
        assert!(macos(b"not a plist").is_err());
    }

    // Rows in the shape that Get-CimInstance Win32_PnPEntity and
    // ConvertTo-Json give. The IDs take the four forms that Windows uses: a
    // device with a serial number, an interface, a root hub, and a device
    // with an instance that Windows made up.
    const CIM: &str = r#"[
      {"PNPDeviceID":"USB\\VID_0781&PID_5583\\4C530001260305117454","Name":"USB Mass Storage Device","Manufacturer":"Compatible USB storage device"},
      {"PNPDeviceID":"USB\\VID_046D&PID_C52B&MI_00\\7&2B3F&0&0000","Name":"USB Input Device","Manufacturer":"(Standard system devices)"},
      {"PNPDeviceID":"USB\\ROOT_HUB30\\4&1D1D&0&0","Name":"USB Root Hub (USB 3.0)","Manufacturer":"(Standard USB HUBs)"},
      {"PNPDeviceID":"USB\\VID_046D&PID_C52B\\6&3A4B&0&2","Name":"USB Composite Device","Manufacturer":"(Standard USB Host Controller)"}
    ]"#;

    #[test]
    fn windows_skips_interfaces_and_root_hubs() {
        let devices = windows(CIM).unwrap();
        assert_eq!(devices.len(), 2);
        let receiver = devices.iter().find(|d| d.vendor_id == 0x046d).unwrap();
        assert_eq!(receiver.product_id, 0xc52b);
        assert_eq!(receiver.serial, None, "a made-up instance is not a serial");
        assert_eq!(
            receiver.vendor, None,
            "a generic maker in brackets is no maker"
        );
        let stick = devices.iter().find(|d| d.vendor_id == 0x0781).unwrap();
        assert_eq!(stick.serial.as_deref(), Some("4C530001260305117454"));
        assert_eq!(
            stick.line(),
            "Bus --- Device ---: ID 0781:5583 Compatible USB storage device USB Mass Storage Device"
        );
    }

    #[test]
    fn windows_with_no_device_gives_an_empty_list() {
        assert!(windows("").unwrap().is_empty());
        assert!(windows("[]").unwrap().is_empty());
    }

    #[test]
    fn speed_writes_megabits_and_gigabits() {
        assert_eq!(speed(1.5), "1.5M");
        assert_eq!(speed(480.0), "480M");
        assert_eq!(speed(5000.0), "5G");
        assert_eq!(speed(10000.0), "10G");
        assert_eq!(speed(2500.0), "2.5G");
    }
}
