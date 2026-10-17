//! Each parser takes input at random and must not panic.
//!
//! A parser reads what a host, a file or a network gives, and SECURITY.md
//! counts a parser that a crafted input can crash as a vulnerability. These
//! tests give each parser three kinds of input: bytes at random, text from
//! the characters that its format uses, and JSON and property lists whose
//! keys are the names that the parsers look for, with values of each type.
//! The source of chance has a fixed seed, so a failure repeats.

use crate::tools;

/// xorshift64*, a small source of chance with a fixed seed.
struct Chance(u64);

impl Chance {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }

    fn bytes(&mut self, max: usize) -> Vec<u8> {
        let len = self.below(max + 1);
        (0..len).map(|_| self.next() as u8).collect()
    }

    /// Text from characters that the line formats use: digits, hex
    /// letters, spaces, separators and a few others.
    fn text(&mut self, max: usize) -> String {
        const PARTS: &[&str] = &[
            "0",
            "1",
            "9",
            "a",
            "f",
            "x",
            "K",
            "M",
            " ",
            "  ",
            "\t",
            "\n",
            ":",
            "/",
            "\\",
            ".",
            "-",
            "*",
            "(",
            ")",
            "=",
            "%",
            "#",
            "\\040",
            "0x",
            "ff",
            "SHA256",
            "MD5",
            "1/389",
            "\u{e9}",
            "\u{1f600}",
            "\r\n",
        ];
        let len = self.below(max + 1);
        (0..len).map(|_| PARTS[self.below(PARTS.len())]).collect()
    }
}

/// The keys that the parsers of JSON and of property lists look for.
const KEYS: &[&str] = &[
    "Name",
    "PNPDeviceID",
    "CompatibleID",
    "Manufacturer",
    "DeviceID",
    "Status",
    "FriendlyName",
    "DiskNumber",
    "Number",
    "Size",
    "BusType",
    "IsBoot",
    "IsSystem",
    "IsReadOnly",
    "MediaName",
    "Partitions",
    "PartitionNumber",
    "DriveLetter",
    "FileSystem",
    "FileSystemLabel",
    "static",
    "full",
    "cycles",
    "status",
    "win32",
    "DesignedCapacity",
    "FullChargedCapacity",
    "CycleCount",
    "RemainingCapacity",
    "Charging",
    "Voltage",
    "EstimatedChargeRemaining",
    "EstimatedRunTime",
    "DeviceName",
    "ManufactureName",
    "SerialNumber",
    "PowerOnline",
    "Discharging",
    "Temperature",
    "CurrentReading",
    "HighPrecisionTemperature",
    "AllDisksAndPartitions",
    "DeviceIdentifier",
    "Content",
    "VolumeName",
    "MountPoint",
    "APFSVolumes",
    "APFSPhysicalStores",
    "Removable",
    "RemovableMedia",
    "Ejectable",
    "Internal",
    "WritableMedia",
    "BusProtocol",
    "TotalSize",
    "ParentWholeDisk",
    "system-entities",
    "dev-entry",
    "mount-point",
    "volume-kind",
    "images",
    "image-path",
    "idVendor",
    "idProduct",
    "USB Vendor Name",
    "USB Product Name",
    "USB Serial Number",
    "kUSBVendorString",
    "kUSBProductString",
    "locationID",
    "UsbLinkSpeed",
    "IORegistryEntryChildren",
    "vendor-id",
    "device-id",
    "class-code",
    "revision-id",
    "name",
    "pcidebug",
    "model",
    "CurrentCapacity",
    "MaxCapacity",
    "DesignCapacity",
    "IsCharging",
    "FullyCharged",
    "ExternalConnected",
    "TimeRemaining",
    "BatteryData",
    "AppleRawMaxCapacity",
    "NominalChargeCapacity",
    "EDID",
    "IODisplayEDID",
    "TransportDescription",
    "bDeviceClass",
];

fn json_value(c: &mut Chance, depth: usize) -> serde_json::Value {
    use serde_json::Value;
    match c.below(if depth > 3 { 5 } else { 7 }) {
        0 => Value::Null,
        1 => Value::Bool(c.below(2) == 1),
        2 => Value::from(c.next() as i64 >> c.below(64)),
        3 => Value::from((c.next() as f64) / 1e9 - 9e9),
        4 => Value::String(c.text(12)),
        5 => Value::Array((0..c.below(4)).map(|_| json_value(c, depth + 1)).collect()),
        _ => {
            let mut map = serde_json::Map::new();
            for _ in 0..c.below(8) {
                map.insert(
                    KEYS[c.below(KEYS.len())].to_owned(),
                    json_value(c, depth + 1),
                );
            }
            Value::Object(map)
        }
    }
}

fn plist_value(c: &mut Chance, depth: usize) -> plist::Value {
    use plist::Value;
    match c.below(if depth > 3 { 6 } else { 8 }) {
        0 => Value::Boolean(c.below(2) == 1),
        1 => Value::Integer((c.next() as i64 >> c.below(64)).into()),
        2 => Value::Real((c.next() as f64) / 1e9),
        3 => Value::String(c.text(12)),
        4 => Value::Data(c.bytes(300)),
        5 => Value::Integer(c.next().into()),
        6 => Value::Array((0..c.below(4)).map(|_| plist_value(c, depth + 1)).collect()),
        _ => {
            let mut dict = plist::Dictionary::new();
            for _ in 0..c.below(8) {
                dict.insert(
                    KEYS[c.below(KEYS.len())].to_owned(),
                    plist_value(c, depth + 1),
                );
            }
            Value::Dictionary(dict)
        }
    }
}

fn plist_bytes(c: &mut Chance) -> Vec<u8> {
    let top = plist::Value::Array((0..c.below(4)).map(|_| plist_value(c, 0)).collect());
    let mut out = Vec::new();
    top.to_writer_xml(&mut out).expect("a plist writes");
    out
}

const ROUNDS: usize = 2000;

#[test]
fn line_parsers_take_any_text() {
    let mut c = Chance(0x5eed_0001);
    for _ in 0..ROUNDS {
        let text = c.text(60);
        let _ = tools::df::parse::mount_line(&text);
        let _ = tools::hash::check::line(&text);
        let _ = tools::hexdump::format::dump_line(&text);
        let _ = tools::free::parse::linux(&text);
        let _ = tools::uptime::parse::loadavg(&text);
        let _ = tools::uptime::parse::proc_uptime(&text);
        let _ = tools::lscpu::parse::sysfs_size(&text);
        let _ = tools::lscpu::parse::linux("x86_64", &text, &[], None);
        let _ = tools::wifi::parse::nmcli(&text);
        let _ = tools::whoholds::parse::linux_maps(&text);
        let _ = tools::whoholds::parse::lsof(&text);
        let _ = tools::whoport::parse::lsof(&text);
        let _ = tools::mount::parse::udisks_loop(&text);
        let _ = tools::mount::parse::udisks_mount(&text);
        let _ = tools::ddc::parse::feature(&text);
        let _ = tools::ddc::parse::value(&text, 0x10, Some((50, 100)));
        let _ = tools::wol::parse::mac(&text);
        let _ = tools::wol::parse::password(&text);
        let _ = tools::timeout::parse::signal(&text);
        let _ = tools::watch::screen::fit(&text, 5, 20);
        let _ = tools::notify::parse::gvariant_string(&text);
        let _ = tools::ldd::parse::ld_so_conf(&text, &mut |_| Vec::new());
        let _ = crate::time::duration(&text);
        let _ = crate::disks::parse::linux_udev(&text);
    }
}

#[test]
fn byte_parsers_take_any_bytes() {
    let mut c = Chance(0x5eed_0002);
    for _ in 0..ROUNDS {
        let bytes = c.bytes(400);
        let _ = tools::edid::parse::decode(&bytes, None);
        let _ = tools::ddc::parse::reply(&bytes, 0x10);
        let _ = tools::netquality::parse::head(&bytes);
        let _ = tools::wifi::parse::mac(&bytes);
        let _ = tools::ip::parse::mac(&bytes);
        let _ = tools::ip::parse::prefix(&bytes);
        let _ = tools::whoport::parse::windows_address(&bytes);
        let _ = tools::lscpu::parse::windows("x86_64", &bytes, None, None, None);
        let _ = tools::ldd::parse::read(&bytes, None);
    }
}

#[test]
fn an_edid_with_a_valid_header_takes_any_body() {
    let mut c = Chance(0x5eed_0003);
    for _ in 0..ROUNDS {
        let mut bytes = vec![0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00];
        bytes.extend(c.bytes(500));
        let _ = tools::edid::parse::decode(&bytes, Some("port".to_owned()));
    }
}

#[test]
fn windows_records_with_valid_heads_take_any_body() {
    // Each record starts with a relation and a size that fits the buffer,
    // so the parser reaches the fields inside.
    let mut c = Chance(0x5eed_0004);
    for _ in 0..ROUNDS {
        let mut buffer = Vec::new();
        for _ in 0..c.below(6) {
            let size = 8 + c.below(80);
            let mut record = c.bytes(size);
            record.resize(size, 0);
            record[0..4].copy_from_slice(&(c.below(5) as u32).to_le_bytes());
            record[4..8].copy_from_slice(&(size as u32).to_le_bytes());
            buffer.extend(record);
        }
        let _ = tools::lscpu::parse::windows("x86_64", &buffer, None, None, None);
    }
}

#[test]
fn a_binary_with_changed_bytes_does_not_crash_ldd() {
    // The test binary itself is an ELF, a Mach-O or a PE, so each change of
    // it reaches deep into the parser of its format.
    let real = std::fs::read(std::env::current_exe().expect("the test binary")).expect("readable");
    let head = &real[..real.len().min(64 * 1024)];
    let mut c = Chance(0x5eed_0005);
    for _ in 0..300 {
        let mut bytes = head.to_vec();
        for _ in 0..1 + c.below(16) {
            let at = c.below(bytes.len());
            bytes[at] = c.next() as u8;
        }
        let _ = tools::ldd::parse::read(&bytes, None);
    }
}

#[test]
fn json_parsers_take_any_json() {
    let mut c = Chance(0x5eed_0006);
    for _ in 0..ROUNDS {
        let value = if c.below(3) == 0 {
            json_value(&mut c, 0)
        } else {
            serde_json::Value::Array((0..c.below(4)).map(|_| json_value(&mut c, 1)).collect())
        };
        let text = value.to_string();
        let _ = tools::lsusb::parse::windows(&text);
        let _ = tools::lspci::parse::windows(&text);
        let _ = tools::battery::parse::windows(&text);
        let _ = tools::sensors::parse::windows(&text);
        let _ = tools::mount::parse::windows(&text, "x");
        let _ = crate::disks::parse::windows(&text);
    }
}

#[test]
fn plist_parsers_take_any_plist() {
    let mut c = Chance(0x5eed_0007);
    for _ in 0..ROUNDS {
        let bytes = plist_bytes(&mut c);
        let _ = tools::lsusb::parse::macos(&bytes);
        let _ = tools::lspci::parse::macos(&bytes);
        let _ = tools::battery::parse::macos(&bytes);
        let _ = tools::edid::parse::from_ioreg(&bytes);
        let _ = tools::mount::parse::hdiutil(&bytes);
        let _ = tools::mount::parse::hdiutil_images(&bytes);
        let _ = crate::disks::parse::mac_list(&bytes);
        let _ = crate::disks::parse::mac_info(&bytes);
    }
}

#[test]
fn plist_parsers_take_any_bytes() {
    let mut c = Chance(0x5eed_0008);
    for _ in 0..ROUNDS {
        let bytes = c.bytes(300);
        let _ = tools::lsusb::parse::macos(&bytes);
        let _ = tools::battery::parse::macos(&bytes);
        let _ = crate::disks::parse::mac_info(&bytes);
    }
}
