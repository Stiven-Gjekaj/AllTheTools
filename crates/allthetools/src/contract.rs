//! The contract of the JSON output: the name of each field of each record
//! that a tool prints with `--json`, and each word that a field can hold.
//!
//! A test here fails when a field changes its name, goes away or comes new.
//! Each such change breaks a script that reads the output, so it needs a
//! new major version, or a new minor version for a new field.
//! docs/json.md describes each field for a person, and docs/stability.md
//! gives the rules.

use serde::Serialize;

/// The names of the fields of a record, in order of name.
fn fields<T: Serialize>(value: T) -> Vec<String> {
    let value = serde_json::to_value(value).expect("a record serializes");
    let object = value.as_object().expect("a record is a JSON object");
    let mut names: Vec<String> = object.keys().cloned().collect();
    names.sort();
    names
}

/// Check that a record has exactly these fields.
fn holds<T: Serialize + Default>(names: &[&str]) {
    let mut expected: Vec<String> = names.iter().map(|n| (*n).to_owned()).collect();
    expected.sort();
    assert_eq!(
        fields(T::default()),
        expected,
        "the fields of {} changed",
        std::any::type_name::<T>()
    );
}

/// The word that a value of an enum gives in JSON.
fn word<T: Serialize>(value: T) -> String {
    serde_json::to_value(value)
        .expect("a value serializes")
        .as_str()
        .expect("an enum gives a string")
        .to_owned()
}

use crate::tools;

#[test]
fn battery() {
    use tools::battery::parse::{Battery, State, Unit};
    holds::<Battery>(&[
        "name",
        "maker",
        "serial",
        "state",
        "percent",
        "unit",
        "design_capacity",
        "full_capacity",
        "cycles",
        "design_cycles",
        "voltage",
        "temperature_c",
        "minutes_left",
        "host_health",
    ]);
    let states = [
        State::Charging,
        State::Discharging,
        State::Full,
        State::NotCharging,
        State::Unknown,
    ]
    .map(word);
    assert_eq!(
        states,
        ["charging", "discharging", "full", "not_charging", "unknown"]
    );
    assert_eq!(
        [Unit::MilliwattHours, Unit::MilliampHours].map(word),
        ["mWh", "mAh"]
    );
}

#[test]
fn df() {
    holds::<tools::df::parse::Volume>(&["source", "fstype", "mount", "size", "used", "available"]);
}

#[test]
fn du() {
    holds::<tools::du::Report>(&["path", "total", "entries"]);
    holds::<tools::du::tally::Entry>(&["path", "size", "folder"]);
}

#[test]
fn edid() {
    holds::<tools::edid::parse::Display>(&[
        "source",
        "maker",
        "product",
        "name",
        "serial",
        "serial_number",
        "week",
        "year",
        "model_year",
        "version",
        "digital",
        "width_cm",
        "height_cm",
        "preferred",
        "vertical_hz",
        "horizontal_khz",
        "max_pixel_clock_mhz",
        "extensions",
        "checksum_ok",
        "edid",
    ]);
    holds::<tools::edid::parse::Mode>(&["width", "height", "refresh_hz", "pixel_clock_khz"]);
}

#[test]
fn flash() {
    holds::<tools::flash::Report>(&[
        "image", "target", "bytes", "verified", "mismatch", "seconds",
    ]);
}

#[test]
fn free() {
    holds::<tools::free::parse::Memory>(&[
        "total",
        "used",
        "free",
        "cache",
        "available",
        "swap_total",
        "swap_used",
    ]);
}

#[test]
fn ip() {
    use tools::ip::parse::{Address, Interface};
    holds::<Interface>(&["name", "state", "loopback", "mac", "mtu", "addresses"]);
    let address = Address {
        address: std::net::Ipv4Addr::LOCALHOST.into(),
        prefix: 8,
    };
    assert_eq!(fields(address), ["address", "prefix"]);
}

#[test]
fn ldd() {
    holds::<tools::ldd::Report>(&["file", "format", "interpreter", "libraries"]);
    holds::<tools::ldd::Entry>(&["name", "path", "status", "depth", "repeat"]);
}

#[test]
fn lsblk() {
    holds::<crate::disks::Disk>(&[
        "id",
        "path",
        "raw_path",
        "size",
        "model",
        "serial",
        "bus",
        "removable",
        "system",
        "read_only",
        "virtual_disk",
        "partitions",
    ]);
    holds::<crate::disks::parse::Partition>(&[
        "id",
        "path",
        "size",
        "filesystem",
        "label",
        "mount",
    ]);
}

#[test]
fn lscpu() {
    use tools::lscpu::parse::{Cache, Cpu, Kind};
    holds::<Cpu>(&[
        "architecture",
        "model",
        "vendor",
        "sockets",
        "cores",
        "threads",
        "kinds",
        "max_mhz",
        "caches",
        "flags",
    ]);
    holds::<Kind>(&["name", "cores"]);
    holds::<Cache>(&["level", "kind", "size", "count"]);
}

#[test]
fn lspci() {
    holds::<tools::lspci::parse::Device>(&[
        "slot",
        "vendor_id",
        "device_id",
        "class",
        "revision",
        "vendor",
        "device",
    ]);
}

#[test]
fn lsusb() {
    holds::<tools::lsusb::parse::Device>(&[
        "bus",
        "address",
        "vendor_id",
        "product_id",
        "vendor",
        "product",
        "serial",
        "speed_mbps",
        "class",
    ]);
}

#[test]
fn mount() {
    holds::<tools::mount::parse::Attached>(&["device", "mount", "filesystem"]);
}

#[test]
fn netquality() {
    holds::<tools::netquality::Report>(&[
        "server",
        "address",
        "idle_latency_ms",
        "download_mbps",
        "download_latency_ms",
        "upload_mbps",
        "upload_latency_ms",
        "responsiveness_rpm",
        "probes",
    ]);
}

#[test]
fn probe() {
    holds::<tools::probe::algo::Outcome>(&[
        "announced",
        "usable",
        "blocks_written",
        "not_restored",
    ]);
}

#[test]
fn sensors() {
    use tools::sensors::parse::{Kind, Reading};
    holds::<Reading>(&["chip", "label", "kind", "value", "high", "critical"]);
    assert_eq!(
        [Kind::Temperature, Kind::Fan, Kind::Voltage].map(word),
        ["temperature", "fan", "voltage"]
    );
}

#[test]
fn surface() {
    holds::<tools::surface::scan::Report>(&[
        "size",
        "sector",
        "checked",
        "bad",
        "bad_count",
        "slow",
        "seconds",
    ]);
}

#[test]
fn uptime() {
    holds::<tools::uptime::parse::Uptime>(&["seconds", "boot", "load"]);
}

#[test]
fn whoholds() {
    holds::<tools::whoholds::parse::Holder>(&["pid", "process", "how", "path"]);
}

#[test]
fn whoport() {
    use tools::whoport::parse::{Proto, Socket};
    holds::<Socket>(&[
        "proto", "address", "port", "remote", "state", "pid", "process",
    ]);
    assert_eq!([Proto::Tcp, Proto::Udp].map(word), ["tcp", "udp"]);
}

#[test]
fn wifi() {
    holds::<tools::wifi::parse::Network>(&[
        "ssid",
        "bssid",
        "signal_dbm",
        "channel",
        "band_ghz",
        "security",
    ]);
}
