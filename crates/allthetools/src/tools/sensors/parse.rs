//! Turn what each host says about its sensors into one list.
//!
//! This module makes no system call and names no type of a host crate, so
//! its tests run on every host.

use std::collections::{BTreeMap, HashSet};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// What a sensor measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Degrees Celsius.
    Temperature,
    /// Turns in a minute.
    Fan,
    /// Volts.
    Voltage,
}

/// One value of one sensor.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Reading {
    /// The chip or the group that the sensor belongs to.
    pub chip: String,
    pub label: String,
    pub kind: Kind,
    pub value: f64,
    /// The value where the chip starts to slow down or to warn.
    pub high: Option<f64>,
    /// The value where the chip stops.
    pub critical: Option<f64>,
}

impl Reading {
    /// The value for a person, with its unit.
    pub fn show(&self) -> String {
        let unit = |v: f64| match self.kind {
            Kind::Temperature => format!("{v:.1} \u{b0}C"),
            Kind::Fan => format!("{v:.0} RPM"),
            Kind::Voltage => format!("{v:.3} V"),
        };
        let mut out = unit(self.value);
        let limits: Vec<String> = [("high", self.high), ("critical", self.critical)]
            .into_iter()
            .filter_map(|(name, v)| v.map(|v| format!("{name} {}", unit(v))))
            .collect();
        if !limits.is_empty() {
            out.push_str(&format!("  ({})", limits.join(", ")));
        }
        out
    }
}

/// A temperature that no sensor of a working computer reports. A sensor that
/// is not connected gives such a value.
fn plausible(kind: Kind, value: f64) -> bool {
    match kind {
        Kind::Temperature => value.is_finite() && value > -40.0 && value < 150.0 && value != 0.0,
        Kind::Fan => value.is_finite() && (0.0..30000.0).contains(&value),
        Kind::Voltage => value.is_finite() && value > 0.0 && value < 100.0,
    }
}

/// One directory under /sys/class/hwmon on Linux: the files by name.
pub type Hwmon = BTreeMap<String, String>;

/// Read the sensors from hwmon. `temp1_input` is in millidegrees, `fan1_input`
/// is in turns a minute, and `in0_input` is in millivolts. Each one can have a
/// `_label`, a `_max` and a `_crit`.
pub fn linux(chips: &[Hwmon]) -> Vec<Reading> {
    let mut out = Vec::new();
    for files in chips {
        let chip = files
            .get("name")
            .map(|n| n.trim().to_owned())
            .unwrap_or_else(|| "hwmon".to_owned());
        for (name, value) in files {
            let Some(base) = name.strip_suffix("_input") else {
                continue;
            };
            let (kind, scale) = if base.starts_with("temp") {
                (Kind::Temperature, 1000.0)
            } else if base.starts_with("fan") {
                (Kind::Fan, 1.0)
            } else if base.starts_with("in") {
                (Kind::Voltage, 1000.0)
            } else {
                continue;
            };
            let number = |suffix: &str| {
                files
                    .get(&format!("{base}_{suffix}"))
                    .and_then(|v| v.trim().parse::<f64>().ok())
                    .map(|v| v / scale)
            };
            let Ok(raw) = value.trim().parse::<f64>() else {
                continue;
            };
            let value = raw / scale;
            if !plausible(kind, value) {
                continue;
            }
            let label = files
                .get(&format!("{base}_label"))
                .map(|l| l.trim().to_owned())
                .unwrap_or_else(|| base.to_owned());
            out.push(Reading {
                chip: chip.clone(),
                label,
                kind,
                value,
                high: number("max").filter(|v| plausible(kind, *v)),
                critical: number("crit").filter(|v| plausible(kind, *v)),
            });
        }
    }
    out
}

/// Decode the value of an SMC key of macOS from its type and its bytes.
///
/// Apple silicon uses `flt ` (a little-endian float). Intel uses `sp78`
/// (signed, 8 bits after the point) and `fpe2` (unsigned, 2 bits after the
/// point), both big-endian. The integer types are big-endian too.
pub fn smc_value(kind: [u8; 4], bytes: &[u8]) -> Option<f64> {
    let take = |n: usize| bytes.get(..n);
    match &kind {
        b"flt " => take(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64),
        b"sp78" => take(2).map(|b| i16::from_be_bytes([b[0], b[1]]) as f64 / 256.0),
        b"fpe2" => take(2).map(|b| u16::from_be_bytes([b[0], b[1]]) as f64 / 4.0),
        b"ui8 " => take(1).map(|b| b[0] as f64),
        b"ui16" => take(2).map(|b| u16::from_be_bytes([b[0], b[1]]) as f64),
        b"ui32" => take(4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as f64),
        _ => None,
    }
}

/// The SMC keys of the fans: FNum gives the count, and F0Ac, F0Mn and F0Mx
/// give the speed, the least and the most of fan 0.
pub fn smc_fan_keys(index: u32) -> [String; 3] {
    [
        format!("F{index}Ac"),
        format!("F{index}Mn"),
        format!("F{index}Mx"),
    ]
}

/// True for an SMC key that holds a temperature: it starts with `T`, and its
/// type is a number with a point.
pub fn smc_is_temperature(key: [u8; 4], kind: [u8; 4]) -> bool {
    key[0] == b'T' && matches!(&kind, b"flt " | b"sp78")
}

/// Make the readings of the SMC temperatures. Each key is its own label,
/// because Apple publishes no names for them.
pub fn smc_temperatures(values: &[(String, f64)]) -> Vec<Reading> {
    values
        .iter()
        .filter(|(_, v)| plausible(Kind::Temperature, *v))
        .map(|(key, value)| Reading {
            chip: "SMC".to_owned(),
            label: key.clone(),
            kind: Kind::Temperature,
            value: *value,
            high: None,
            critical: None,
        })
        .collect()
}

/// Make the readings of the temperature sensors of the HID event system on
/// Apple silicon, such as "PMU tdie1". A name that comes again is the same
/// sensor through a second service, so the first one stays.
pub fn hid_temperatures(values: &[(String, f64)]) -> Vec<Reading> {
    let mut seen = HashSet::new();
    let mut out: Vec<Reading> = values
        .iter()
        .filter(|(name, v)| plausible(Kind::Temperature, *v) && seen.insert(name.clone()))
        .map(|(name, value)| Reading {
            chip: "HID".to_owned(),
            label: name.clone(),
            kind: Kind::Temperature,
            value: *value,
            high: None,
            critical: None,
        })
        .collect();
    out.sort_by(|a, b| a.label.cmp(&b.label));
    out
}

/// One thermal zone of Windows, from the performance counters, as PowerShell
/// writes it in JSON. The temperature is in kelvin, and the high precision
/// one in tenths of a kelvin.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Zone {
    name: Option<String>,
    temperature: Option<f64>,
    high_precision_temperature: Option<f64>,
}

/// Read the thermal zones of ACPI from the JSON of the Windows host.
pub fn windows(json: &str) -> Result<Vec<Reading>> {
    let zones: Vec<Zone> = if json.trim().is_empty() {
        Vec::new()
    } else {
        serde_json::from_str(json).context("PowerShell gave JSON that is not a list of zones")?
    };
    Ok(zones
        .iter()
        .filter_map(|zone| {
            let kelvin = zone
                .high_precision_temperature
                .map(|t| t / 10.0)
                .or(zone.temperature)?;
            let value = kelvin - 273.15;
            plausible(Kind::Temperature, value).then(|| Reading {
                chip: "ACPI".to_owned(),
                label: zone
                    .name
                    .clone()
                    .unwrap_or_else(|| "thermal zone".to_owned()),
                kind: Kind::Temperature,
                value,
                high: None,
                critical: None,
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hwmon(pairs: &[(&str, &str)]) -> Hwmon {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), format!("{v}\n")))
            .collect()
    }

    #[test]
    fn linux_reads_temperatures_fans_and_their_limits() {
        let readings = linux(&[
            hwmon(&[
                ("name", "coretemp"),
                ("temp1_input", "45000"),
                ("temp1_label", "Package id 0"),
                ("temp1_max", "80000"),
                ("temp1_crit", "100000"),
                ("temp2_input", "-273000"),
            ]),
            hwmon(&[
                ("name", "thinkpad"),
                ("fan1_input", "2380"),
                ("in0_input", "12100"),
            ]),
        ]);
        assert_eq!(
            readings.len(),
            3,
            "a sensor that is not connected is skipped"
        );
        let package = &readings[0];
        assert_eq!(package.label, "Package id 0");
        assert_eq!(package.value, 45.0);
        assert_eq!(
            package.show(),
            "45.0 \u{b0}C  (high 80.0 \u{b0}C, critical 100.0 \u{b0}C)"
        );
        let fan = readings.iter().find(|r| r.kind == Kind::Fan).unwrap();
        assert_eq!(fan.label, "fan1");
        assert_eq!(fan.show(), "2380 RPM");
        let volts = readings.iter().find(|r| r.kind == Kind::Voltage).unwrap();
        assert_eq!(volts.value, 12.1);
    }

    #[test]
    fn smc_value_decodes_each_type() {
        assert_eq!(smc_value(*b"flt ", &42.5f32.to_le_bytes()), Some(42.5));
        assert_eq!(smc_value(*b"sp78", &[0x2d, 0x80]), Some(45.5));
        assert_eq!(smc_value(*b"sp78", &[0xff, 0x00]), Some(-1.0));
        assert_eq!(smc_value(*b"fpe2", &[0x17, 0x70]), Some(1500.0));
        assert_eq!(smc_value(*b"ui8 ", &[2]), Some(2.0));
        assert_eq!(smc_value(*b"ui32", &[0, 0, 0x08, 0x00]), Some(2048.0));
        assert_eq!(smc_value(*b"flag", &[1]), None);
        assert_eq!(smc_value(*b"flt ", &[0, 0]), None, "too few bytes");
    }

    #[test]
    fn smc_temperature_keys_start_with_t() {
        assert!(smc_is_temperature(*b"Tp09", *b"flt "));
        assert!(smc_is_temperature(*b"TC0P", *b"sp78"));
        assert!(!smc_is_temperature(*b"F0Ac", *b"flt "));
        assert!(!smc_is_temperature(*b"TZ0c", *b"ui8 "));
    }

    #[test]
    fn hid_keeps_each_name_one_time_and_drops_bad_values() {
        let readings = hid_temperatures(&[
            ("PMU tdie1".to_owned(), 41.2),
            ("NAND CH0 temp".to_owned(), 33.0),
            ("PMU tdie1".to_owned(), 99.0),
            ("PMU tcal".to_owned(), 0.0),
        ]);
        let labels: Vec<&str> = readings.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(labels, ["NAND CH0 temp", "PMU tdie1"]);
        assert_eq!(readings[1].value, 41.2);
    }

    #[test]
    fn windows_turns_kelvin_into_celsius() {
        let json = r#"[{"Name":"\\_TZ.THM0","Temperature":321,"HighPrecisionTemperature":3212},
                       {"Name":"\\_TZ.THM1","Temperature":0,"HighPrecisionTemperature":0}]"#;
        let readings = windows(json).unwrap();
        assert_eq!(readings.len(), 1);
        assert_eq!(readings[0].label, "\\_TZ.THM0");
        assert_eq!(format!("{:.2}", readings[0].value), "48.05");
        assert!(windows("").unwrap().is_empty());
    }
}
