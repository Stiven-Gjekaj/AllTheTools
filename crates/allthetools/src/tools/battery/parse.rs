//! Turn what each host says about its batteries into one report.
//!
//! This module makes no system call and names no type of a host crate, so
//! its tests run on every host.

use std::collections::BTreeMap;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// The unit of a capacity. Hosts report energy or charge, and a health
/// figure compares two values of the same unit only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Unit {
    #[serde(rename = "mWh")]
    MilliwattHours,
    #[serde(rename = "mAh")]
    MilliampHours,
}

impl Unit {
    fn label(self) -> &'static str {
        match self {
            Unit::MilliwattHours => "mWh",
            Unit::MilliampHours => "mAh",
        }
    }
}

/// What the battery does now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Charging,
    Discharging,
    Full,
    /// The computer is on external power, and the battery does not charge.
    NotCharging,
    Unknown,
}

/// One battery.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Battery {
    pub name: Option<String>,
    pub maker: Option<String>,
    pub serial: Option<String>,
    pub state: State,
    /// The charge now, as a percentage of the full charge.
    pub percent: Option<f64>,
    pub unit: Unit,
    /// The capacity of the battery when it was new.
    pub design_capacity: Option<u64>,
    /// The capacity of a full charge now.
    pub full_capacity: Option<u64>,
    pub cycles: Option<u32>,
    /// The number of cycles that the maker designs the battery for.
    pub design_cycles: Option<u32>,
    pub voltage: Option<f64>,
    pub temperature_c: Option<f64>,
    /// The time to empty, or to full, as the host gives it.
    pub minutes_left: Option<u32>,
}

impl Battery {
    /// The full capacity now as a percentage of the design capacity.
    pub fn health(&self) -> Option<f64> {
        match (self.full_capacity, self.design_capacity) {
            (Some(full), Some(design)) if design > 0 => Some(full as f64 * 100.0 / design as f64),
            _ => None,
        }
    }

    /// The lines of the report for a person.
    pub fn lines(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let name = [&self.maker, &self.name]
            .into_iter()
            .flatten()
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        if !name.is_empty() {
            out.push(("Battery".to_owned(), name));
        }
        if let Some(serial) = &self.serial {
            out.push(("Serial".to_owned(), serial.clone()));
        }
        let state = match self.state {
            State::Charging => "charging",
            State::Discharging => "on battery power",
            State::Full => "fully charged",
            State::NotCharging => "on external power, not charging",
            State::Unknown => "unknown",
        };
        out.push(("State".to_owned(), state.to_owned()));
        if let Some(percent) = self.percent {
            out.push(("Charge".to_owned(), format!("{percent:.0}%")));
        }
        if let Some(minutes) = self.minutes_left {
            let what = if self.state == State::Charging {
                "to full"
            } else {
                "left"
            };
            out.push((
                "Time".to_owned(),
                format!("{}:{:02} {what}", minutes / 60, minutes % 60),
            ));
        }
        let unit = self.unit.label();
        match (self.health(), self.full_capacity, self.design_capacity) {
            (Some(health), Some(full), Some(design)) => out.push((
                "Health".to_owned(),
                format!("{health:.1}% ({full} of {design} {unit} when new)"),
            )),
            (None, Some(full), _) => out.push(("Full charge".to_owned(), format!("{full} {unit}"))),
            _ => {}
        }
        match (self.cycles, self.design_cycles) {
            (Some(cycles), Some(design)) => {
                out.push(("Cycles".to_owned(), format!("{cycles} of {design}")))
            }
            (Some(cycles), None) => out.push(("Cycles".to_owned(), cycles.to_string())),
            _ => {}
        }
        if let Some(t) = self.temperature_c {
            out.push(("Temperature".to_owned(), format!("{t:.1} \u{b0}C")));
        }
        if let Some(v) = self.voltage {
            out.push(("Voltage".to_owned(), format!("{v:.2} V")));
        }
        out
    }
}

fn text(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

/// The files of one directory under /sys/class/power_supply on Linux, by
/// name.
pub type SysfsSupply = BTreeMap<String, String>;

/// Read the batteries from sysfs. A supply whose `type` is not `Battery` is
/// a charger or the battery of a mouse, and not a battery of this computer.
pub fn linux(supplies: &[SysfsSupply]) -> Vec<Battery> {
    supplies
        .iter()
        .filter(|s| s.get("type").map(|t| t.trim()) == Some("Battery"))
        .filter(|s| s.get("scope").map(|t| t.trim()) != Some("Device"))
        .map(|s| {
            let get = |name: &str| s.get(name).map(|v| v.trim());
            let number = |name: &str| get(name).and_then(|v| v.parse::<u64>().ok());
            // sysfs gives micro units. A battery reports energy or charge.
            let (unit, design, full) = if number("energy_full").is_some() {
                (
                    Unit::MilliwattHours,
                    number("energy_full_design"),
                    number("energy_full"),
                )
            } else {
                (
                    Unit::MilliampHours,
                    number("charge_full_design"),
                    number("charge_full"),
                )
            };
            let state = match get("status") {
                Some("Charging") => State::Charging,
                Some("Discharging") => State::Discharging,
                Some("Full") => State::Full,
                Some("Not charging") => State::NotCharging,
                _ => State::Unknown,
            };
            Battery {
                name: text(get("model_name")),
                maker: text(get("manufacturer")),
                serial: text(get("serial_number")),
                state,
                percent: number("capacity").map(|p| p as f64),
                unit,
                design_capacity: design.map(|v| v / 1000).filter(|v| *v > 0),
                full_capacity: full.map(|v| v / 1000).filter(|v| *v > 0),
                // A driver that does not count cycles gives 0.
                cycles: number("cycle_count").map(|c| c as u32).filter(|c| *c > 0),
                design_cycles: None,
                voltage: number("voltage_now").map(|v| v as f64 / 1_000_000.0),
                temperature_c: get("temp")
                    .and_then(|t| t.parse::<f64>().ok())
                    .map(|t| t / 10.0),
                minutes_left: None,
            }
        })
        .collect()
}

/// Read the battery from `ioreg -r -c AppleSmartBattery -a` on macOS.
///
/// On Apple silicon, CurrentCapacity is a percentage and MaxCapacity is 100.
/// The capacities in mAh are AppleRawMaxCapacity and DesignCapacity.
pub fn macos(plist: &[u8]) -> Result<Vec<Battery>> {
    if plist.iter().all(u8::is_ascii_whitespace) {
        return Ok(Vec::new());
    }
    let value = plist::Value::from_reader_xml(plist)
        .context("ioreg gave a property list that is not valid")?;
    let mut out = Vec::new();
    for entry in value.as_array().map(Vec::as_slice).unwrap_or(&[]) {
        let Some(dict) = entry.as_dictionary() else {
            continue;
        };
        if dict
            .get("BatteryInstalled")
            .and_then(plist::Value::as_boolean)
            == Some(false)
        {
            continue;
        }
        let int = |key: &str| {
            let value = dict.get(key)?;
            value.as_unsigned_integer().or_else(|| {
                value
                    .as_signed_integer()
                    .and_then(|v| u64::try_from(v).ok())
            })
        };
        let flag = |key: &str| dict.get(key).and_then(plist::Value::as_boolean) == Some(true);
        let string = |key: &str| text(dict.get(key).and_then(plist::Value::as_string));

        let state = if flag("FullyCharged") {
            State::Full
        } else if flag("IsCharging") {
            State::Charging
        } else if flag("ExternalConnected") {
            State::NotCharging
        } else {
            State::Discharging
        };
        let percent = match (int("CurrentCapacity"), int("MaxCapacity")) {
            (Some(now), Some(max)) if max > 0 => Some(now as f64 * 100.0 / max as f64),
            _ => None,
        };
        // 65535 means that macOS does not know the time yet.
        let minutes_left = int("TimeRemaining")
            .filter(|m| *m < 65535 && *m > 0)
            .map(|m| m as u32);
        out.push(Battery {
            name: string("DeviceName"),
            maker: None,
            serial: string("Serial"),
            state,
            percent,
            unit: Unit::MilliampHours,
            design_capacity: int("DesignCapacity"),
            full_capacity: int("AppleRawMaxCapacity").or_else(|| int("NominalChargeCapacity")),
            cycles: int("CycleCount").map(|c| c as u32),
            design_cycles: int("DesignCycleCount9C").map(|c| c as u32),
            voltage: int("Voltage").map(|v| v as f64 / 1000.0),
            temperature_c: int("Temperature").map(|t| t as f64 / 100.0),
            minutes_left,
        });
    }
    Ok(out)
}

/// The JSON that the PowerShell script of the Windows host writes: one list
/// for each WMI class, in the same order of batteries.
#[derive(Debug, Default, Deserialize)]
pub struct WindowsReport {
    #[serde(default, rename = "static")]
    pub static_data: Vec<StaticData>,
    #[serde(default)]
    pub full: Vec<FullCharged>,
    #[serde(default)]
    pub cycles: Vec<CycleCount>,
    #[serde(default)]
    pub status: Vec<Status>,
    #[serde(default)]
    pub win32: Vec<Win32Battery>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct StaticData {
    pub designed_capacity: Option<u64>,
    pub device_name: Option<String>,
    pub manufacture_name: Option<String>,
    pub serial_number: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct FullCharged {
    pub full_charged_capacity: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CycleCount {
    pub cycle_count: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Status {
    pub remaining_capacity: Option<u64>,
    pub charging: Option<bool>,
    pub discharging: Option<bool>,
    pub power_online: Option<bool>,
    pub voltage: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Win32Battery {
    pub estimated_charge_remaining: Option<u32>,
    pub estimated_run_time: Option<u32>,
    pub name: Option<String>,
}

/// Read the batteries from the JSON of the Windows host. WMI gives the
/// capacities in mWh and the voltage in mV.
pub fn windows(json: &str) -> Result<Vec<Battery>> {
    let report: WindowsReport =
        serde_json::from_str(json).context("PowerShell gave JSON that is not a battery report")?;
    let count = report.win32.len().max(report.static_data.len());
    let mut out = Vec::new();
    for i in 0..count {
        let fixed = report.static_data.get(i);
        let status = report.status.get(i);
        let win32 = report.win32.get(i);
        let full = report.full.get(i).and_then(|f| f.full_charged_capacity);
        let flag = |f: fn(&Status) -> Option<bool>| status.and_then(f) == Some(true);
        let state = if flag(|s| s.charging) {
            State::Charging
        } else if flag(|s| s.discharging) {
            State::Discharging
        } else if flag(|s| s.power_online) {
            if status.and_then(|s| s.remaining_capacity).is_some()
                && status.and_then(|s| s.remaining_capacity) >= full
            {
                State::Full
            } else {
                State::NotCharging
            }
        } else {
            State::Unknown
        };
        let percent = win32
            .and_then(|w| w.estimated_charge_remaining)
            .map(f64::from)
            .or_else(|| match (status.and_then(|s| s.remaining_capacity), full) {
                (Some(now), Some(full)) if full > 0 => Some(now as f64 * 100.0 / full as f64),
                _ => None,
            });
        // 71582788 is the value of Win32_Battery for "on external power".
        let minutes_left = win32
            .and_then(|w| w.estimated_run_time)
            .filter(|m| *m < 71_582_788);
        out.push(Battery {
            name: fixed
                .and_then(|f| text(f.device_name.as_deref()))
                .or_else(|| win32.and_then(|w| text(w.name.as_deref()))),
            maker: fixed.and_then(|f| text(f.manufacture_name.as_deref())),
            serial: fixed.and_then(|f| text(f.serial_number.as_deref())),
            state,
            percent,
            unit: Unit::MilliwattHours,
            design_capacity: fixed.and_then(|f| f.designed_capacity).filter(|c| *c > 0),
            full_capacity: full.filter(|c| *c > 0),
            cycles: report
                .cycles
                .get(i)
                .and_then(|c| c.cycle_count)
                .filter(|c| *c > 0),
            design_cycles: None,
            voltage: status
                .and_then(|s| s.voltage)
                .map(|v| v as f64 / 1000.0)
                .filter(|v| *v > 0.0),
            temperature_c: None,
            minutes_left,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn supply(pairs: &[(&str, &str)]) -> SysfsSupply {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), format!("{v}\n")))
            .collect()
    }

    #[test]
    fn linux_reads_a_battery_that_reports_energy() {
        let batteries = linux(&[
            supply(&[("type", "Mains"), ("online", "1")]),
            supply(&[
                ("type", "Battery"),
                ("status", "Discharging"),
                ("capacity", "57"),
                ("energy_full_design", "57000000"),
                ("energy_full", "48450000"),
                ("cycle_count", "312"),
                ("voltage_now", "11820000"),
                ("manufacturer", "SMP"),
                ("model_name", "5B10W13930"),
            ]),
            supply(&[("type", "Battery"), ("scope", "Device"), ("capacity", "90")]),
        ]);
        assert_eq!(
            batteries.len(),
            1,
            "a charger and a mouse are not batteries"
        );
        let b = &batteries[0];
        assert_eq!(b.unit, Unit::MilliwattHours);
        assert_eq!(b.design_capacity, Some(57000));
        assert_eq!(b.full_capacity, Some(48450));
        assert_eq!(b.health(), Some(85.0));
        assert_eq!(b.state, State::Discharging);
        assert_eq!(b.cycles, Some(312));
        assert_eq!(b.voltage, Some(11.82));
    }

    #[test]
    fn linux_reads_a_battery_that_reports_charge() {
        let batteries = linux(&[supply(&[
            ("type", "Battery"),
            ("status", "Not charging"),
            ("charge_full_design", "4000000"),
            ("charge_full", "3000000"),
            ("cycle_count", "0"),
        ])]);
        let b = &batteries[0];
        assert_eq!(b.unit, Unit::MilliampHours);
        assert_eq!(b.health(), Some(75.0));
        assert_eq!(b.state, State::NotCharging);
        assert_eq!(b.cycles, None, "0 means that the driver does not count");
    }

    // Captured with `ioreg -r -c AppleSmartBattery -a -w0` on a MacBook with
    // Apple silicon, and cut down to the keys that the parser reads.
    const IOREG: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<array>
  <dict>
    <key>AppleRawMaxCapacity</key><integer>4443</integer>
    <key>BatteryInstalled</key><true/>
    <key>CurrentCapacity</key><integer>80</integer>
    <key>CycleCount</key><integer>29</integer>
    <key>DesignCapacity</key><integer>4629</integer>
    <key>DesignCycleCount9C</key><integer>1000</integer>
    <key>DeviceName</key><string>bq40z651</string>
    <key>ExternalConnected</key><true/>
    <key>FullyCharged</key><false/>
    <key>IsCharging</key><false/>
    <key>MaxCapacity</key><integer>100</integer>
    <key>NominalChargeCapacity</key><integer>4570</integer>
    <key>Serial</key><string>F5DHN8001390000G45</string>
    <key>Temperature</key><integer>3054</integer>
    <key>TimeRemaining</key><integer>65535</integer>
    <key>Voltage</key><integer>12482</integer>
  </dict>
</array>
</plist>"#;

    #[test]
    fn macos_reads_the_health_from_the_raw_capacity() {
        let batteries = macos(IOREG.as_bytes()).unwrap();
        let b = &batteries[0];
        assert_eq!(b.state, State::NotCharging);
        assert_eq!(b.percent, Some(80.0));
        assert_eq!(format!("{:.1}", b.health().unwrap()), "96.0");
        assert_eq!(b.cycles, Some(29));
        assert_eq!(b.design_cycles, Some(1000));
        assert_eq!(b.temperature_c, Some(30.54));
        assert_eq!(b.minutes_left, None, "65535 means not known");
        let lines = b.lines();
        assert!(lines.contains(&(
            "Health".to_owned(),
            "96.0% (4443 of 4629 mAh when new)".to_owned()
        )));
        assert!(lines.contains(&("Cycles".to_owned(), "29 of 1000".to_owned())));
    }

    #[test]
    fn macos_with_no_battery_gives_an_empty_list() {
        assert!(macos(b"").unwrap().is_empty());
    }

    // The JSON of the PowerShell script of the Windows host, with the values
    // of a laptop battery in the units of WMI.
    #[test]
    fn windows_joins_the_classes_of_one_battery() {
        let json = r#"{"static":[{"DesignedCapacity":56000,"DeviceName":"DELL 7FHHV","ManufactureName":"SMP","SerialNumber":"1234"}],
            "full":[{"FullChargedCapacity":44800}],
            "cycles":[{"CycleCount":0}],
            "status":[{"RemainingCapacity":30000,"Charging":true,"Discharging":false,"PowerOnline":true,"Voltage":12600}],
            "win32":[{"EstimatedChargeRemaining":67,"EstimatedRunTime":71582788,"Name":"DELL 7FHHV"}]}"#;
        let batteries = windows(json).unwrap();
        let b = &batteries[0];
        assert_eq!(b.state, State::Charging);
        assert_eq!(b.percent, Some(67.0));
        assert_eq!(b.health(), Some(80.0));
        assert_eq!(b.cycles, None);
        assert_eq!(b.minutes_left, None);
        assert_eq!(b.voltage, Some(12.6));
        assert_eq!(b.maker.as_deref(), Some("SMP"));
    }

    #[test]
    fn windows_with_no_battery_gives_an_empty_list() {
        let json = r#"{"static":[],"full":[],"cycles":[],"status":[],"win32":[]}"#;
        assert!(windows(json).unwrap().is_empty());
    }
}
