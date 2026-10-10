//! Turn what each host says about its PCI devices into one list.
//!
//! This module makes no system call and names no type of a host crate, so
//! its tests run on every host.

use std::collections::{BTreeMap, HashMap, HashSet};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// One PCI function.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Device {
    /// The bus, the device and the function, as `00:02.0`. Windows gives
    /// none.
    pub slot: Option<String>,
    pub vendor_id: u16,
    pub device_id: u16,
    /// The class, the subclass and the programming interface, in 24 bits.
    pub class: Option<u32>,
    pub revision: Option<u8>,
    pub vendor: Option<String>,
    pub device: Option<String>,
}

impl Device {
    /// The line that `lspci -nn` of Linux prints for the device.
    pub fn line(&self) -> String {
        let mut out = String::new();
        if let Some(slot) = &self.slot {
            out.push_str(slot);
            out.push(' ');
        }
        out.push_str(&class_name(self.class));
        if let Some(class) = self.class {
            out.push_str(&format!(" [{:04x}]", class >> 8));
        }
        out.push_str(": ");
        let vendor = self
            .vendor
            .clone()
            .or_else(|| vendor_name(self.vendor_id).map(str::to_owned));
        let mut name = vendor.unwrap_or_else(|| "Unknown vendor".to_owned());
        if let Some(device) = &self.device {
            name.push(' ');
            name.push_str(device);
        }
        out.push_str(&name);
        out.push_str(&format!(" [{:04x}:{:04x}]", self.vendor_id, self.device_id));
        if let Some(revision) = self.revision.filter(|r| *r != 0) {
            out.push_str(&format!(" (rev {revision:02x})"));
        }
        out
    }
}

/// The name of a class from the PCI specification. A subclass that the table
/// does not hold gives the name of its base class.
pub fn class_name(class: Option<u32>) -> String {
    let Some(class) = class else {
        return "Device".to_owned();
    };
    let base = (class >> 16) as u8;
    let sub = (class >> 8) as u8;
    let name = match (base, sub) {
        (0x01, 0x00) => "SCSI storage controller",
        (0x01, 0x01) => "IDE interface",
        (0x01, 0x04) => "RAID bus controller",
        (0x01, 0x06) => "SATA controller",
        (0x01, 0x07) => "Serial Attached SCSI controller",
        (0x01, 0x08) => "Non-Volatile memory controller",
        (0x01, _) => "Mass storage controller",
        (0x02, 0x00) => "Ethernet controller",
        (0x02, 0x80) => "Network controller",
        (0x02, _) => "Network controller",
        (0x03, 0x00) => "VGA compatible controller",
        (0x03, 0x02) => "3D controller",
        (0x03, _) => "Display controller",
        (0x04, 0x01) => "Multimedia audio controller",
        (0x04, 0x03) => "Audio device",
        (0x04, _) => "Multimedia controller",
        (0x05, _) => "Memory controller",
        (0x06, 0x00) => "Host bridge",
        (0x06, 0x01) => "ISA bridge",
        (0x06, 0x04) => "PCI bridge",
        (0x06, _) => "Bridge",
        (0x07, _) => "Communication controller",
        (0x08, _) => "System peripheral",
        (0x09, _) => "Input device controller",
        (0x0b, _) => "Processor",
        (0x0c, 0x03) => "USB controller",
        (0x0c, 0x05) => "SMBus",
        (0x0c, 0x80) => "Serial bus controller",
        (0x0c, _) => "Serial bus controller",
        (0x0d, _) => "Wireless controller",
        (0x10, _) => "Encryption controller",
        (0x11, _) => "Signal processing controller",
        (0x12, _) => "Processing accelerators",
        (0xff, _) => "Unassigned class",
        _ => "Device",
    };
    name.to_owned()
}

/// The makers that a person meets most often. Linux reads the full list from
/// pci.ids, and the other hosts use this one.
pub fn vendor_name(id: u16) -> Option<&'static str> {
    let name = match id {
        0x1002 => "Advanced Micro Devices, Inc. [AMD/ATI]",
        0x1022 => "Advanced Micro Devices, Inc. [AMD]",
        0x104c => "Texas Instruments",
        0x106b => "Apple Inc.",
        0x10de => "NVIDIA Corporation",
        0x10ec => "Realtek Semiconductor Co., Ltd.",
        0x1106 => "VIA Technologies, Inc.",
        0x126f => "Silicon Motion, Inc.",
        0x1344 => "Micron Technology Inc",
        0x1414 => "Microsoft Corporation",
        0x144d => "Samsung Electronics Co Ltd",
        0x14c3 => "MEDIATEK Corp.",
        0x14e4 => "Broadcom Inc. and subsidiaries",
        0x15ad => "VMware",
        0x15b3 => "Mellanox Technologies",
        0x15b7 => "Sandisk Corp",
        0x168c => "Qualcomm Atheros",
        0x17cb => "Qualcomm Technologies, Inc",
        0x1912 => "Renesas Technology Corp.",
        0x1987 => "Phison Electronics Corporation",
        0x1af4 => "Red Hat, Inc.",
        0x1b21 => "ASMedia Technology Inc.",
        0x1b36 => "Red Hat, Inc.",
        0x1b4b => "Marvell Technology Group Ltd.",
        0x1b73 => "Fresco Logic",
        0x1c5c => "SK hynix",
        0x1d0f => "Amazon.com, Inc.",
        0x2646 => "Kingston Technology Company, Inc.",
        0x8086 => "Intel Corporation",
        0x80ee => "InnoTek Systemberatung GmbH",
        _ => return None,
    };
    Some(name)
}

/// Put the devices in the order of their slot.
pub fn sort(devices: &mut [Device]) {
    devices.sort_by(|a, b| {
        (&a.slot, a.vendor_id, a.device_id).cmp(&(&b.slot, b.vendor_id, b.device_id))
    });
}

/// The names of the vendors and the devices in pci.ids, for the IDs in
/// `wanted` only. The file lists each vendor at the start of a line, and each
/// device of it after one tab. The classes come last, from a line that starts
/// with `C `.
pub fn pci_ids(
    text: &str,
    wanted: &HashSet<(u16, u16)>,
) -> (HashMap<u16, String>, HashMap<(u16, u16), String>) {
    let vendors_wanted: HashSet<u16> = wanted.iter().map(|(v, _)| *v).collect();
    let mut vendors = HashMap::new();
    let mut devices = HashMap::new();
    let mut current: Option<u16> = None;
    for line in text.lines() {
        if line.starts_with("C ") {
            break;
        }
        if line.starts_with('#') || line.trim().is_empty() || line.starts_with("\t\t") {
            continue;
        }
        let (rest, nested) = match line.strip_prefix('\t') {
            Some(rest) => (rest, true),
            None => (line, false),
        };
        let Some((id, name)) = rest.split_once("  ") else {
            continue;
        };
        let Ok(id) = u16::from_str_radix(id.trim(), 16) else {
            continue;
        };
        if nested {
            if let Some(vendor) = current
                && wanted.contains(&(vendor, id))
            {
                devices.insert((vendor, id), name.trim().to_owned());
            }
        } else {
            current = Some(id);
            if vendors_wanted.contains(&id) {
                vendors.insert(id, name.trim().to_owned());
            }
        }
    }
    (vendors, devices)
}

/// The files of one directory under /sys/bus/pci/devices on Linux, by name.
/// The key `slot` holds the name of the directory.
pub type SysfsDevice = BTreeMap<String, String>;

/// Read the devices from sysfs. A slot in domain 0000 loses its domain, as
/// lspci shows it.
pub fn linux(dirs: &[SysfsDevice]) -> Vec<Device> {
    let mut devices: Vec<Device> = dirs
        .iter()
        .filter_map(|attrs| {
            let hex = |name: &str| {
                let value = attrs.get(name)?.trim();
                u32::from_str_radix(value.trim_start_matches("0x"), 16).ok()
            };
            let slot = attrs.get("slot")?.trim();
            let slot = slot.strip_prefix("0000:").unwrap_or(slot).to_owned();
            Some(Device {
                slot: Some(slot),
                vendor_id: hex("vendor")? as u16,
                device_id: hex("device")? as u16,
                class: hex("class"),
                revision: hex("revision").map(|r| r as u8),
                vendor: None,
                device: None,
            })
        })
        .collect();
    sort(&mut devices);
    devices
}

/// Read the devices from `ioreg -r -c IOPCIDevice -a -d1` on macOS.
///
/// Each number is a little-endian data value of four bytes. The slot is in
/// `pcidebug` as `bus:device:function`, and the name of the device is in
/// `name`.
pub fn macos(plist: &[u8]) -> Result<Vec<Device>> {
    if plist.iter().all(u8::is_ascii_whitespace) {
        return Ok(Vec::new());
    }
    let value = plist::Value::from_reader_xml(plist)
        .context("ioreg gave a property list that is not valid")?;
    let mut devices = Vec::new();
    for entry in value.as_array().map(Vec::as_slice).unwrap_or(&[]) {
        let Some(dict) = entry.as_dictionary() else {
            continue;
        };
        let number = |key: &str| {
            let data = dict.get(key)?.as_data()?;
            let mut bytes = [0u8; 4];
            for (i, b) in data.iter().take(4).enumerate() {
                bytes[i] = *b;
            }
            Some(u32::from_le_bytes(bytes))
        };
        let (Some(vendor_id), Some(device_id)) = (number("vendor-id"), number("device-id")) else {
            continue;
        };
        let slot = dict
            .get("pcidebug")
            .and_then(plist::Value::as_string)
            .and_then(mac_slot);
        let name = dict
            .get("name")
            .and_then(plist::Value::as_data)
            .map(|d| String::from_utf8_lossy(d).trim_end_matches('\0').to_owned())
            .filter(|n| !n.is_empty());
        devices.push(Device {
            slot,
            vendor_id: vendor_id as u16,
            device_id: device_id as u16,
            class: number("class-code").map(|c| c & 0x00FF_FFFF),
            revision: number("revision-id").map(|r| r as u8),
            vendor: None,
            device: name,
        });
    }
    sort(&mut devices);
    Ok(devices)
}

/// `1:0:2` or `0:0:0(1:1)` gives `01:00.2`.
fn mac_slot(text: &str) -> Option<String> {
    let text = text.split('(').next()?;
    let mut parts = text.split(':').map(|p| p.trim().parse::<u32>().ok());
    let (bus, device, function) = (parts.next()??, parts.next()??, parts.next()??);
    Some(format!("{bus:02x}:{device:02x}.{function:x}"))
}

/// One row of Win32_PnPEntity, as PowerShell writes it in JSON.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PnpEntity {
    #[serde(rename = "PNPDeviceID")]
    pnp_device_id: Option<String>,
    name: Option<String>,
    manufacturer: Option<String>,
    #[serde(rename = "CompatibleID", default)]
    compatible_id: Option<CompatibleIds>,
}

/// PowerShell writes one compatible ID as a string, and more than one as a
/// list.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum CompatibleIds {
    One(String),
    Many(Vec<String>),
}

/// Read the devices from the JSON of Win32_PnPEntity on Windows. The ID of a
/// device is `PCI\VEN_8086&DEV_9A49&SUBSYS_...&REV_01\<instance>`, and one of
/// its compatible IDs holds the class as `CC_030000`.
pub fn windows(json: &str) -> Result<Vec<Device>> {
    let rows: Vec<PnpEntity> = if json.trim().is_empty() {
        Vec::new()
    } else {
        serde_json::from_str(json).context("PowerShell gave JSON that is not a list of devices")?
    };
    let mut devices: Vec<Device> = rows
        .iter()
        .filter_map(|row| {
            let id = row.pnp_device_id.as_deref()?.to_ascii_uppercase();
            let ids = id.strip_prefix("PCI\\")?.split('\\').next()?;
            let field = |key: &str, len: usize| {
                let start = ids.find(key)? + key.len();
                u32::from_str_radix(ids.get(start..start + len)?, 16).ok()
            };
            let compatible: Vec<String> = match &row.compatible_id {
                Some(CompatibleIds::One(one)) => vec![one.to_ascii_uppercase()],
                Some(CompatibleIds::Many(many)) => {
                    many.iter().map(|c| c.to_ascii_uppercase()).collect()
                }
                None => Vec::new(),
            };
            let class = compatible.iter().find_map(|c| {
                let start = c.find("CC_")? + 3;
                u32::from_str_radix(c.get(start..start + 6)?, 16).ok()
            });
            Some(Device {
                slot: None,
                vendor_id: field("VEN_", 4)? as u16,
                device_id: field("DEV_", 4)? as u16,
                class,
                revision: field("REV_", 2).map(|r| r as u8),
                vendor: row
                    .manufacturer
                    .clone()
                    .filter(|m| !m.trim().is_empty() && !m.starts_with('(')),
                device: row.name.clone().filter(|n| !n.trim().is_empty()),
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
    fn linux_reads_the_slot_the_ids_and_the_class() {
        let devices = linux(&[
            sysfs(&[
                ("slot", "0000:00:02.0"),
                ("vendor", "0x8086"),
                ("device", "0x5917"),
                ("class", "0x030000"),
                ("revision", "0x07"),
            ]),
            sysfs(&[
                ("slot", "0000:00:00.0"),
                ("vendor", "0x8086"),
                ("device", "0x5914"),
                ("class", "0x060000"),
                ("revision", "0x08"),
            ]),
            sysfs(&[("slot", "0000:00:1f.6")]),
        ]);
        assert_eq!(devices.len(), 2, "a slot with no vendor is skipped");
        assert_eq!(
            devices[1].line(),
            "00:02.0 VGA compatible controller [0300]: Intel Corporation [8086:5917] (rev 07)"
        );
        assert_eq!(devices[0].slot.as_deref(), Some("00:00.0"));
    }

    #[test]
    fn pci_ids_names_only_the_wanted_devices() {
        let text = "# comment\n8086  Intel Corporation\n\t5917  UHD Graphics 620\n\t\t1028 0810  Latitude\n\t5914  Xeon E3-1200 v6/7th Gen Core Processor Host Bridge/DRAM Registers\n10de  NVIDIA Corporation\n\t1c8d  GP107M [GeForce GTX 1050 Mobile]\nC 00  Unclassified device\n\t00  Non-VGA unclassified device\n";
        let wanted = HashSet::from([(0x8086, 0x5917), (0x10de, 0x9999)]);
        let (vendors, devices) = pci_ids(text, &wanted);
        assert_eq!(vendors[&0x8086], "Intel Corporation");
        assert_eq!(vendors[&0x10de], "NVIDIA Corporation");
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[&(0x8086, 0x5917)], "UHD Graphics 620");
    }

    // Captured with `ioreg -r -c IOPCIDevice -a -w0 -d1` on a MacBook with
    // Apple silicon, and cut down to two entries and the keys that the
    // parser reads.
    const IOREG: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<array>
  <dict>
    <key>class-code</key><data>AAQGAA==</data>
    <key>device-id</key><data>DBAAAA==</data>
    <key>name</key><data>cGNpLWJyaWRnZTAA</data>
    <key>pcidebug</key><string>0:0:0(1:1)</string>
    <key>revision-id</key><data>AQAAAA==</data>
    <key>vendor-id</key><data>axAAAA==</data>
  </dict>
  <dict>
    <key>class-code</key><data>ACANAA==</data>
    <key>device-id</key><data>AhkAAA==</data>
    <key>name</key><data>Y2VudGF1cmktYWxwaGEA</data>
    <key>pcidebug</key><string>1:0:1</string>
    <key>revision-id</key><data>AQAAAA==</data>
    <key>vendor-id</key><data>axAAAA==</data>
  </dict>
</array>
</plist>"#;

    #[test]
    fn macos_reads_the_little_endian_numbers() {
        let devices = macos(IOREG.as_bytes()).unwrap();
        let lines: Vec<String> = devices.iter().map(Device::line).collect();
        assert_eq!(
            lines,
            [
                "00:00.0 PCI bridge [0604]: Apple Inc. pci-bridge0 [106b:100c] (rev 01)",
                "01:00.1 Wireless controller [0d20]: Apple Inc. centauri-alpha [106b:1902] (rev 01)",
            ]
        );
    }

    #[test]
    fn macos_with_no_device_gives_an_empty_list() {
        assert!(macos(b"").unwrap().is_empty());
    }

    // Rows in the shape that Get-CimInstance Win32_PnPEntity and
    // ConvertTo-Json give: one compatible ID as a string, and more than one
    // as a list.
    const CIM: &str = r#"[
      {"PNPDeviceID":"PCI\\VEN_8086&DEV_9A49&SUBSYS_0A6C1028&REV_01\\3&11583659&0&10","Name":"Intel(R) Iris(R) Xe Graphics","Manufacturer":"Intel Corporation","CompatibleID":["PCI\\VEN_8086&DEV_9A49&REV_01","PCI\\VEN_8086&DEV_9A49","PCI\\VEN_8086&CC_030000","PCI\\VEN_8086&CC_0300","PCI\\VEN_8086","PCI\\CC_030000","PCI\\CC_0300"]},
      {"PNPDeviceID":"PCI\\VEN_144D&DEV_A80A&SUBSYS_A801144D&REV_00\\4&2F0B7A5&0&0008","Name":"Standard NVM Express Controller","Manufacturer":"(Standard NVM Express Controller)","CompatibleID":"PCI\\CC_010802"},
      {"PNPDeviceID":"ACPI\\PNP0A08\\0","Name":"PCI Express Root Complex","Manufacturer":"(Standard system devices)","CompatibleID":null}
    ]"#;

    #[test]
    fn windows_reads_the_ids_and_the_class_of_each_pci_device() {
        let devices = windows(CIM).unwrap();
        assert_eq!(devices.len(), 2, "a device that is not on PCI is skipped");
        let nvme = devices.iter().find(|d| d.vendor_id == 0x144d).unwrap();
        assert_eq!(nvme.class, Some(0x010802));
        assert_eq!(nvme.vendor, None);
        assert_eq!(
            nvme.line(),
            "Non-Volatile memory controller [0108]: Samsung Electronics Co Ltd Standard NVM Express Controller [144d:a80a]"
        );
        let gpu = devices.iter().find(|d| d.vendor_id == 0x8086).unwrap();
        assert_eq!(gpu.revision, Some(1));
        assert_eq!(gpu.class, Some(0x030000));
    }

    #[test]
    fn mac_slot_reads_each_form() {
        assert_eq!(mac_slot("1:0:2").as_deref(), Some("01:00.2"));
        assert_eq!(mac_slot("0:0:0(1:128)").as_deref(), Some("00:00.0"));
        assert_eq!(mac_slot("bad"), None);
    }

    #[test]
    fn an_unknown_class_has_a_name() {
        assert_eq!(class_name(None), "Device");
        assert_eq!(class_name(Some(0x0c0330)), "USB controller");
        assert_eq!(class_name(Some(0x0c0999)), "Serial bus controller");
        assert_eq!(class_name(Some(0xee0000)), "Device");
    }
}
