//! The DDC/CI protocol, and the names of the features of a monitor.
//!
//! DDC/CI is a set of messages on the I2C bus of the display cable. Linux
//! and macOS send the bytes of a message themselves, and Windows sends them
//! for a program. The bytes are the same on each host.
//!
//! This module makes no system call, so its tests run on every host.

use anyhow::{Result, bail};

/// The I2C address of the monitor, in 7 bits.
pub const ADDRESS: u16 = 0x37;
/// The address of the host, which starts each message.
pub const HOST: u8 = 0x51;
/// The address of the monitor in 8 bits, for a write: 0x37 shifted left.
const DESTINATION: u8 = 0x6E;
/// The base of the checksum of a reply: the host address of a read.
const REPLY_BASE: u8 = 0x50;

/// The names of the features that a person sets most often, and their VCP
/// codes from the MCCS standard.
pub const FEATURES: [(&str, u8); 10] = [
    ("brightness", 0x10),
    ("contrast", 0x12),
    ("red", 0x16),
    ("green", 0x18),
    ("blue", 0x1A),
    ("input", 0x60),
    ("volume", 0x62),
    ("sharpness", 0x87),
    ("mute", 0x8D),
    ("power", 0xD6),
];

/// The VCP code of a feature name, or of a number such as 0x10 or 16.
pub fn feature(text: &str) -> Result<u8> {
    let lower = text.trim().to_ascii_lowercase();
    if let Some((_, code)) = FEATURES.iter().find(|(name, _)| *name == lower) {
        return Ok(*code);
    }
    let parsed = match lower.strip_prefix("0x") {
        Some(hex) => u8::from_str_radix(hex, 16).ok(),
        None => lower.parse().ok(),
    };
    match parsed {
        Some(code) => Ok(code),
        None => bail!(
            "`{text}` is no feature. Use a VCP code such as 0x10, or one of: {}",
            FEATURES
                .iter()
                .map(|(n, _)| *n)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// The name of a VCP code, or the code in hexadecimal.
pub fn feature_name(code: u8) -> String {
    FEATURES
        .iter()
        .find(|(_, c)| *c == code)
        .map(|(n, _)| (*n).to_owned())
        .unwrap_or_else(|| format!("0x{code:02x}"))
}

/// The names of the input sources of MCCS, for the feature `input`.
const INPUTS: [(&str, u16); 8] = [
    ("vga", 0x01),
    ("dvi", 0x03),
    ("dvi2", 0x04),
    ("dp", 0x0F),
    ("dp2", 0x10),
    ("hdmi", 0x11),
    ("hdmi2", 0x12),
    ("usbc", 0x1B),
];

/// The value to set: a number, a change such as +10 or -10, or a name of an
/// input. A change stays between 0 and the maximum of the feature.
pub fn value(text: &str, code: u8, current: Option<(u16, u16)>) -> Result<u16> {
    let text = text.trim().to_ascii_lowercase();
    if code == 0x60
        && let Some((_, value)) = INPUTS.iter().find(|(name, _)| *name == text)
    {
        return Ok(*value);
    }
    if let Some(rest) = text.strip_prefix('+').or_else(|| text.strip_prefix('-')) {
        let Some((now, max)) = current else {
            bail!("the monitor did not give the value now, so a change such as {text} has no base");
        };
        let step: i32 = rest
            .parse()
            .map_err(|_| anyhow::anyhow!("`{text}` is no change"))?;
        let step = if text.starts_with('-') { -step } else { step };
        return Ok((i32::from(now) + step).clamp(0, i32::from(max)) as u16);
    }
    let parsed = match text.strip_prefix("0x") {
        Some(hex) => u16::from_str_radix(hex, 16).ok(),
        None => text.parse().ok(),
    };
    match parsed {
        Some(value) => Ok(value),
        None => bail!("`{text}` is no value. Give a number, or a change such as +10"),
    }
}

/// The XOR of the bytes.
fn xor(base: u8, bytes: &[u8]) -> u8 {
    bytes.iter().fold(base, |sum, b| sum ^ b)
}

/// The message that asks for a feature, after the host address: length,
/// opcode 0x01, code, checksum.
pub fn get_request(code: u8) -> Vec<u8> {
    let body = [0x82, 0x01, code];
    let mut out = body.to_vec();
    out.push(xor(DESTINATION ^ HOST, &body));
    out
}

/// The message that sets a feature, after the host address: length, opcode
/// 0x03, code, the value in two bytes, checksum.
pub fn set_request(code: u8, value: u16) -> Vec<u8> {
    let [high, low] = value.to_be_bytes();
    let body = [0x84, 0x03, code, high, low];
    let mut out = body.to_vec();
    out.push(xor(DESTINATION ^ HOST, &body));
    out
}

/// A message as Linux writes it on the bus: the host address first.
pub fn with_host(message: &[u8]) -> Vec<u8> {
    let mut out = vec![HOST];
    out.extend_from_slice(message);
    out
}

/// What a monitor says about one feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reply {
    pub code: u8,
    pub current: u16,
    pub max: u16,
}

/// Read the reply to a request for a feature. The reply is 11 bytes: the
/// address of the monitor, the length 0x88, the opcode 0x02, a result, the
/// code, a type, the maximum in two bytes, the value now in two bytes, and a
/// checksum.
pub fn reply(bytes: &[u8], code: u8) -> Result<Reply> {
    if bytes.len() < 11 {
        bail!("the monitor gave {} bytes, and a reply has 11", bytes.len());
    }
    let b = &bytes[..11];
    if b[1] != 0x88 || b[2] != 0x02 {
        bail!(
            "the monitor gave no reply to the request (a null message, or a monitor that is busy)"
        );
    }
    if xor(REPLY_BASE, &b[..10]) != b[10] {
        bail!("the reply of the monitor has a bad checksum. The cable or the monitor lost a byte");
    }
    if b[3] != 0 {
        bail!(
            "the monitor does not support the feature {}",
            feature_name(code)
        );
    }
    if b[4] != code {
        bail!(
            "the monitor answered for the feature 0x{:02x}, and not for 0x{code:02x}",
            b[4]
        );
    }
    Ok(Reply {
        code,
        max: u16::from_be_bytes([b[6], b[7]]),
        current: u16::from_be_bytes([b[8], b[9]]),
    })
}

/// The name of a monitor in its EDID: the descriptor with tag 0xFC. The
/// four descriptors of 18 bytes start at byte 54, and a name ends with a new
/// line.
pub fn edid_name(edid: &[u8]) -> Option<String> {
    for start in [54, 72, 90, 108] {
        let d = edid.get(start..start + 18)?;
        if d[0..3] == [0, 0, 0] && d[3] == 0xFC {
            let text: String = d[5..]
                .iter()
                .take_while(|b| **b != b'\n')
                .map(|b| *b as char)
                .collect();
            let text = text.trim().to_owned();
            return (!text.is_empty()).then_some(text);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_feature_takes_a_name_or_a_number() {
        assert_eq!(feature("Brightness").unwrap(), 0x10);
        assert_eq!(feature("0x60").unwrap(), 0x60);
        assert_eq!(feature("98").unwrap(), 98);
        assert!(feature("loudness").is_err());
        assert_eq!(feature_name(0x62), "volume");
        assert_eq!(feature_name(0xE0), "0xe0");
    }

    #[test]
    fn a_value_takes_a_number_a_change_or_an_input() {
        assert_eq!(value("70", 0x10, None).unwrap(), 70);
        assert_eq!(value("+10", 0x10, Some((50, 100))).unwrap(), 60);
        assert_eq!(value("-80", 0x10, Some((50, 100))).unwrap(), 0);
        assert_eq!(value("+80", 0x10, Some((50, 100))).unwrap(), 100);
        assert_eq!(value("hdmi2", 0x60, None).unwrap(), 0x12);
        assert_eq!(value("0x0f", 0x60, None).unwrap(), 0x0F);
        assert!(value("+10", 0x10, None).is_err());
        assert!(
            value("hdmi", 0x10, None).is_err(),
            "an input name is for input only"
        );
    }

    // The bytes of a request for the brightness are known from the MCCS
    // standard and from ddcutil: 51 82 01 10 ac.
    #[test]
    fn the_request_for_the_brightness_has_its_known_checksum() {
        assert_eq!(
            with_host(&get_request(0x10)),
            [0x51, 0x82, 0x01, 0x10, 0xAC]
        );
    }

    #[test]
    fn a_set_request_holds_the_value_in_two_bytes() {
        let message = set_request(0x10, 300);
        assert_eq!(&message[..5], &[0x84, 0x03, 0x10, 0x01, 0x2C]);
        assert_eq!(message[5], xor(DESTINATION ^ HOST, &message[..5]));
    }

    fn reply_bytes(result: u8, code: u8, max: u16, current: u16) -> Vec<u8> {
        let [max_high, max_low] = max.to_be_bytes();
        let [high, low] = current.to_be_bytes();
        let mut bytes = vec![
            0x6E, 0x88, 0x02, result, code, 0x00, max_high, max_low, high, low,
        ];
        bytes.push(xor(REPLY_BASE, &bytes));
        bytes
    }

    #[test]
    fn a_reply_gives_the_value_and_the_maximum() {
        let parsed = reply(&reply_bytes(0, 0x10, 100, 42), 0x10).unwrap();
        assert_eq!(
            parsed,
            Reply {
                code: 0x10,
                current: 42,
                max: 100
            }
        );
    }

    #[test]
    fn edid_name_reads_the_descriptor_of_the_name() {
        let mut edid = vec![0u8; 128];
        edid[72..77].copy_from_slice(&[0, 0, 0, 0xFC, 0]);
        edid[77..90].copy_from_slice(b"DELL U2720Q\n ");
        assert_eq!(edid_name(&edid).as_deref(), Some("DELL U2720Q"));
        assert_eq!(edid_name(&[0u8; 128]), None);
        assert_eq!(edid_name(&[0u8; 10]), None);
    }

    #[test]
    fn a_bad_reply_says_why() {
        let mut bad = reply_bytes(0, 0x10, 100, 42);
        bad[9] ^= 1;
        assert!(
            reply(&bad, 0x10)
                .unwrap_err()
                .to_string()
                .contains("checksum")
        );
        assert!(
            reply(&reply_bytes(1, 0x10, 0, 0), 0x10)
                .unwrap_err()
                .to_string()
                .contains("does not support")
        );
        assert!(reply(&[0x6E, 0x80, 0xBE], 0x10).is_err());
        assert!(reply(&reply_bytes(0, 0x12, 100, 1), 0x10).is_err());
    }
}
