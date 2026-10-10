//! The decisions of `att wol`: the hardware address and the magic packet.
//!
//! This module makes no system call, so its tests run on every host.

use anyhow::{Result, bail};

/// Read a hardware address in any of the forms that people write.
///
/// `aa:bb:cc:dd:ee:ff`, `aa-bb-cc-dd-ee-ff`, `aabb.ccdd.eeff` and
/// `aabbccddeeff` all give the same six bytes.
pub fn mac(text: &str) -> Result<[u8; 6]> {
    let digits: String = text
        .trim()
        .chars()
        .filter(|c| !matches!(c, ':' | '-' | '.'))
        .collect();
    let refuse =
        || anyhow::anyhow!("`{text}` is not a hardware address. Write it as aa:bb:cc:dd:ee:ff");
    if digits.len() != 12 || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(refuse());
    }
    let separators: Vec<char> = text
        .trim()
        .chars()
        .filter(|c| !c.is_ascii_hexdigit())
        .collect();
    let even = match separators.len() {
        0 => true,
        5 => separators.iter().all(|c| *c == separators[0]) && groups_of(text, 2),
        2 => separators.iter().all(|c| *c == '.') && groups_of(text, 4),
        _ => false,
    };
    if !even {
        return Err(refuse());
    }
    let mut bytes = [0u8; 6];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&digits[i * 2..i * 2 + 2], 16).map_err(|_| refuse())?;
    }
    Ok(bytes)
}

/// True when each group between the separators holds `size` digits.
fn groups_of(text: &str, size: usize) -> bool {
    text.trim()
        .split(|c: char| !c.is_ascii_hexdigit())
        .all(|group| group.len() == size)
}

/// Read the SecureOn password of a card, which has the form of a hardware
/// address.
pub fn password(text: &str) -> Result<[u8; 6]> {
    match mac(text) {
        Ok(bytes) => Ok(bytes),
        Err(_) => bail!("`{text}` is not a SecureOn password. Write it as aa:bb:cc:dd:ee:ff"),
    }
}

/// The magic packet: six bytes of 0xFF, then the address sixteen times, then
/// the password when there is one.
pub fn packet(mac: [u8; 6], password: Option<[u8; 6]>) -> Vec<u8> {
    let mut out = vec![0xFF; 6];
    for _ in 0..16 {
        out.extend_from_slice(&mac);
    }
    if let Some(password) = password {
        out.extend_from_slice(&password);
    }
    out
}

/// The address for a person: six bytes, small letters, colons.
pub fn show(mac: [u8; 6]) -> String {
    mac.iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(":")
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADDRESS: [u8; 6] = [0xaa, 0xbb, 0xcc, 0x01, 0x02, 0x03];

    #[test]
    fn each_common_form_gives_the_same_address() {
        for form in [
            "aa:bb:cc:01:02:03",
            "AA-BB-CC-01-02-03",
            "aabb.cc01.0203",
            "aabbcc010203",
            " aa:bb:cc:01:02:03 ",
        ] {
            assert_eq!(mac(form).unwrap(), ADDRESS, "{form}");
        }
    }

    #[test]
    fn a_bad_address_is_refused() {
        for bad in [
            "",
            "aa:bb:cc:01:02",
            "aa:bb:cc:01:02:03:04",
            "gg:bb:cc:01:02:03",
            "aa:bb-cc:01:02:03",
            "aab:bcc:010:203",
            "a:abb:cc:01:02:03",
        ] {
            assert!(mac(bad).is_err(), "{bad} was accepted");
        }
    }

    #[test]
    fn the_packet_holds_the_address_sixteen_times() {
        let packet = packet(ADDRESS, None);
        assert_eq!(packet.len(), 102);
        assert_eq!(&packet[..6], &[0xFF; 6]);
        for copy in packet[6..].chunks(6) {
            assert_eq!(copy, ADDRESS);
        }
    }

    #[test]
    fn the_password_goes_at_the_end() {
        let password = [1, 2, 3, 4, 5, 6];
        let packet = packet(ADDRESS, Some(password));
        assert_eq!(packet.len(), 108);
        assert_eq!(&packet[102..], &password);
    }

    #[test]
    fn show_writes_small_letters_and_colons() {
        assert_eq!(show(ADDRESS), "aa:bb:cc:01:02:03");
    }
}
