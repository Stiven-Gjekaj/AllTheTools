//! Turn what each host says about the Wi-Fi networks near it into one list.
//!
//! This module makes no system call and names no type of a host crate, so
//! its tests run on every host.

use serde::Serialize;

/// One access point that the scan found.
#[derive(Debug, Default, Clone, PartialEq, Serialize)]
pub struct Network {
    /// The name. macOS and Windows hide it from a program that has no
    /// permission for the location.
    pub ssid: Option<String>,
    pub bssid: Option<String>,
    /// The signal in dBm. Linux gives a percentage, and the tool turns it
    /// into dBm the way NetworkManager turns dBm into the percentage.
    pub signal_dbm: Option<i32>,
    pub channel: Option<u32>,
    /// 2.4, 5 or 6.
    pub band_ghz: Option<f64>,
    pub security: String,
}

/// The channel of a center frequency in MHz.
pub fn channel(mhz: u32) -> Option<u32> {
    match mhz {
        2484 => Some(14),
        2412..=2472 => Some((mhz - 2407) / 5),
        // 6 GHz: channel 2 is a special case, the others count from 5950.
        5935 => Some(2),
        5955..=7115 => Some((mhz - 5950) / 5),
        5150..=5925 => Some((mhz - 5000) / 5),
        _ => None,
    }
}

/// The band of a center frequency in MHz.
pub fn band(mhz: u32) -> Option<f64> {
    match mhz {
        2400..=2500 => Some(2.4),
        5150..=5925 => Some(5.0),
        5926..=7125 => Some(6.0),
        _ => None,
    }
}

/// NetworkManager gives a quality from 0 to 100, from dBm between -100 and
/// -50. This is the same line the other way.
pub fn dbm_from_percent(percent: u32) -> i32 {
    let percent = percent.min(100) as i32;
    percent / 2 - 100
}

/// The name of a security mode of CoreWLAN, from the CWSecurity value.
pub fn apple_security(code: i64) -> &'static str {
    match code {
        0 => "open",
        1 | 6 => "WEP",
        2 => "WPA",
        3 => "WPA/WPA2",
        4 => "WPA2",
        7..=10 => "WPA2 Enterprise",
        11 => "WPA3",
        12 => "WPA3 Enterprise",
        13 => "WPA2/WPA3",
        14 | 15 => "OWE",
        _ => "unknown",
    }
}

/// The CWSecurity values to test, from the strongest to the weakest. A
/// network supports more than one, and the first that it supports names it.
///
/// The values for two modes at once, such as 13 for WPA2/WPA3, are not in
/// the list. CoreWLAN says that a WPA2 network supports 13, because one of
/// the two modes matches. A network that supports WPA3 and WPA2 matches 11
/// first, and is WPA3.
pub const APPLE_SECURITY_ORDER: [i64; 11] = [11, 12, 9, 4, 3, 2, 7, 8, 14, 1, 0];

/// The name of an authentication algorithm of the WLAN API of Windows, from
/// DOT11_AUTH_ALGORITHM.
pub fn windows_security(code: i32, cipher_none: bool) -> &'static str {
    match code {
        1 if cipher_none => "open",
        1 | 2 => "WEP",
        3..=5 => "WPA",
        6 => "WPA2 Enterprise",
        7 => "WPA2",
        8 | 11 => "WPA3 Enterprise",
        9 => "WPA3",
        10 => "OWE",
        _ => "unknown",
    }
}

/// A hardware address of six bytes, as aa:bb:cc:dd:ee:ff.
pub fn mac(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(":")
}

/// Split one line of `nmcli -t`. A colon separates the fields, and a field
/// that holds a colon or a backslash has a backslash before it.
fn nmcli_fields(line: &str) -> Vec<String> {
    let mut fields = vec![String::new()];
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(next) = chars.next() {
                    fields.last_mut().expect("one field at least").push(next);
                }
            }
            ':' => fields.push(String::new()),
            _ => fields.last_mut().expect("one field at least").push(c),
        }
    }
    fields
}

/// Read the output of
/// `nmcli -t -f SSID,BSSID,SIGNAL,CHAN,FREQ,SECURITY device wifi list`.
pub fn nmcli(text: &str) -> Vec<Network> {
    text.lines()
        .filter_map(|line| {
            let f = nmcli_fields(line);
            if f.len() < 6 {
                return None;
            }
            let mhz: Option<u32> = f[4].split_whitespace().next().and_then(|v| v.parse().ok());
            let security = f[5].trim();
            Some(Network {
                ssid: Some(f[0].clone()).filter(|s| !s.is_empty()),
                bssid: Some(f[1].to_ascii_lowercase()).filter(|s| !s.is_empty()),
                signal_dbm: f[2].parse().ok().map(dbm_from_percent),
                channel: f[3].parse().ok(),
                band_ghz: mhz.and_then(band),
                security: if security.is_empty() || security == "--" {
                    "open".to_owned()
                } else {
                    security.to_owned()
                },
            })
        })
        .collect()
}

/// Put the strongest signal first.
pub fn sort(networks: &mut [Network]) {
    networks.sort_by_key(|n| std::cmp::Reverse(n.signal_dbm.unwrap_or(i32::MIN)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_and_band_come_from_the_frequency() {
        assert_eq!(channel(2412), Some(1));
        assert_eq!(channel(2462), Some(11));
        assert_eq!(channel(2484), Some(14));
        assert_eq!(channel(5180), Some(36));
        assert_eq!(channel(5700), Some(140));
        assert_eq!(channel(5955), Some(1));
        assert_eq!(channel(5935), Some(2));
        assert_eq!(channel(6115), Some(33));
        assert_eq!(channel(900), None);
        assert_eq!(band(2437), Some(2.4));
        assert_eq!(band(5180), Some(5.0));
        assert_eq!(band(6115), Some(6.0));
    }

    #[test]
    fn a_percentage_turns_into_dbm() {
        assert_eq!(dbm_from_percent(100), -50);
        assert_eq!(dbm_from_percent(0), -100);
        assert_eq!(dbm_from_percent(70), -65);
        assert_eq!(dbm_from_percent(250), -50);
    }

    #[test]
    fn nmcli_reads_escaped_colons() {
        let text = "Home:AA\\:BB\\:CC\\:DD\\:EE\\:FF:82:36:5180 MHz:WPA2\n\
                    :11\\:22\\:33\\:44\\:55\\:66:40:6:2437 MHz:--\n\
                    Cafe\\:Guest:12\\:34\\:56\\:78\\:9A\\:BC:20:1:2412 MHz:WPA1 WPA2\n";
        let networks = nmcli(text);
        assert_eq!(networks.len(), 3);
        assert_eq!(networks[0].ssid.as_deref(), Some("Home"));
        assert_eq!(networks[0].bssid.as_deref(), Some("aa:bb:cc:dd:ee:ff"));
        assert_eq!(networks[0].signal_dbm, Some(-59));
        assert_eq!(networks[0].band_ghz, Some(5.0));
        assert_eq!(networks[1].ssid, None, "a hidden network has no name");
        assert_eq!(networks[1].security, "open");
        assert_eq!(networks[2].ssid.as_deref(), Some("Cafe:Guest"));
    }

    #[test]
    fn apple_security_names_the_strongest_mode_first() {
        assert_eq!(apple_security(APPLE_SECURITY_ORDER[0]), "WPA3");
        assert_eq!(apple_security(4), "WPA2");
        assert_eq!(apple_security(0), "open");
        assert_eq!(*APPLE_SECURITY_ORDER.last().unwrap(), 0);
    }

    #[test]
    fn windows_security_reads_the_algorithm() {
        assert_eq!(windows_security(7, false), "WPA2");
        assert_eq!(windows_security(1, true), "open");
        assert_eq!(windows_security(1, false), "WEP");
        assert_eq!(windows_security(9, false), "WPA3");
        assert_eq!(windows_security(10, false), "OWE");
        assert_eq!(windows_security(11, false), "WPA3 Enterprise");
    }

    #[test]
    fn sort_puts_the_strongest_signal_first() {
        let net = |dbm| Network {
            ssid: None,
            bssid: None,
            signal_dbm: dbm,
            channel: None,
            band_ghz: None,
            security: "open".to_owned(),
        };
        let mut networks = vec![net(Some(-80)), net(None), net(Some(-40))];
        sort(&mut networks);
        assert_eq!(networks[0].signal_dbm, Some(-40));
        assert_eq!(networks[2].signal_dbm, None);
    }
}
