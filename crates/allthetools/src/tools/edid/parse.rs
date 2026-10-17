//! Read the EDID of a display: the block of bytes in which a monitor says
//! who made it, what it is, and which modes it takes.
//!
//! This module reads the base block of EDID 1.3 and 1.4, and names each
//! extension block. It makes no system call, so its tests run on every
//! host.

use anyhow::{Context, Result, bail};
use serde::Serialize;

const HEADER: [u8; 8] = [0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00];

/// A mode of the display.
#[derive(Debug, Default, Clone, PartialEq, Serialize)]
pub struct Mode {
    pub width: u32,
    pub height: u32,
    pub refresh_hz: f64,
    pub pixel_clock_khz: u32,
}

/// What the EDID of one display says.
#[derive(Debug, Default, Clone, PartialEq, Serialize)]
pub struct Display {
    /// Where the host found the display, such as a connector.
    pub source: Option<String>,
    /// The three letters of the maker, from the PNP ID list.
    pub maker: String,
    pub product: u16,
    pub name: Option<String>,
    /// The serial number as text, when the display gives it.
    pub serial: Option<String>,
    /// The serial number as a number. Zero says that the display gives none.
    pub serial_number: u32,
    /// The week of manufacture. None when the display gives no week.
    pub week: Option<u8>,
    pub year: u16,
    /// True when the year is the model year, and not the year it was made.
    pub model_year: bool,
    pub version: String,
    pub digital: bool,
    pub width_cm: u8,
    pub height_cm: u8,
    /// The mode that the maker prefers, which is usually the native mode.
    pub preferred: Option<Mode>,
    pub vertical_hz: Option<(u8, u8)>,
    pub horizontal_khz: Option<(u8, u8)>,
    pub max_pixel_clock_mhz: Option<u16>,
    /// The name of each extension block.
    pub extensions: Vec<String>,
    /// True when the bytes of the base block add up as the standard says.
    pub checksum_ok: bool,
    /// The whole EDID, as hex digits.
    pub edid: String,
}

/// The text of a display descriptor: up to 13 characters, which end at a
/// line feed.
fn descriptor_text(bytes: &[u8]) -> Option<String> {
    let end = bytes.iter().position(|b| *b == 0x0a).unwrap_or(bytes.len());
    let text: String = bytes[..end]
        .iter()
        .map(|b| {
            if (0x20..0x7f).contains(b) {
                *b as char
            } else {
                '?'
            }
        })
        .collect();
    let text = text.trim().to_owned();
    (!text.is_empty()).then_some(text)
}

fn timing(d: &[u8]) -> Option<Mode> {
    let clock = u32::from(u16::from_le_bytes([d[0], d[1]])) * 10;
    let width = u32::from(d[2]) | (u32::from(d[4] >> 4) << 8);
    let h_blank = u32::from(d[3]) | (u32::from(d[4] & 0x0f) << 8);
    let height = u32::from(d[5]) | (u32::from(d[7] >> 4) << 8);
    let v_blank = u32::from(d[6]) | (u32::from(d[7] & 0x0f) << 8);
    let total = (width + h_blank) * (height + v_blank);
    if clock == 0 || total == 0 {
        return None;
    }
    let refresh = f64::from(clock) * 1000.0 / f64::from(total);
    Some(Mode {
        width,
        height,
        refresh_hz: (refresh * 100.0).round() / 100.0,
        pixel_clock_khz: clock,
    })
}

fn extension_name(tag: u8) -> String {
    match tag {
        0x02 => "CTA-861".to_owned(),
        0x10 => "video timing block".to_owned(),
        0x40 => "display information".to_owned(),
        0x50 => "localized strings".to_owned(),
        0x60 => "digital packet video link".to_owned(),
        0x70 => "DisplayID".to_owned(),
        0xf0 => "block map".to_owned(),
        0xff => "maker specific".to_owned(),
        tag => format!("tag {tag:#04x}"),
    }
}

/// Read an EDID. `source` says where the host found it.
pub fn decode(bytes: &[u8], source: Option<String>) -> Result<Display> {
    if bytes.len() < 128 {
        bail!(
            "the EDID has {} bytes, and a base block has 128",
            bytes.len()
        );
    }
    if bytes[..8] != HEADER {
        bail!("the EDID does not start with the header of the standard");
    }
    let maker_bits = u16::from_be_bytes([bytes[8], bytes[9]]);
    let maker: String = [10, 5, 0]
        .iter()
        .map(|shift| {
            let n = ((maker_bits >> shift) & 0x1f) as u8;
            if (1..=26).contains(&n) {
                (b'A' + n - 1) as char
            } else {
                '?'
            }
        })
        .collect();
    let week = bytes[16];
    let mut display = Display {
        source,
        maker,
        product: u16::from_le_bytes([bytes[10], bytes[11]]),
        name: None,
        serial: None,
        serial_number: u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]),
        week: (1..=54).contains(&week).then_some(week),
        year: 1990 + u16::from(bytes[17]),
        model_year: week == 0xff,
        version: format!("{}.{}", bytes[18], bytes[19]),
        digital: bytes[20] & 0x80 != 0,
        width_cm: bytes[21],
        height_cm: bytes[22],
        preferred: None,
        vertical_hz: None,
        horizontal_khz: None,
        max_pixel_clock_mhz: None,
        extensions: Vec::new(),
        checksum_ok: bytes[..128].iter().fold(0u8, |a, b| a.wrapping_add(*b)) == 0,
        edid: bytes.iter().map(|b| format!("{b:02x}")).collect(),
    };
    for (i, d) in bytes[54..126].chunks(18).enumerate() {
        if d[0] != 0 || d[1] != 0 {
            // The first detailed timing is the preferred mode.
            if i == 0 {
                display.preferred = timing(d);
            }
            continue;
        }
        match d[3] {
            0xfc => display.name = descriptor_text(&d[5..]),
            0xff => display.serial = descriptor_text(&d[5..]),
            0xfd => {
                display.vertical_hz = Some((d[5], d[6]));
                display.horizontal_khz = Some((d[7], d[8]));
                display.max_pixel_clock_mhz = Some(u16::from(d[9]) * 10);
            }
            _ => {}
        }
    }
    let count = usize::from(bytes[126]);
    for n in 1..=count {
        match bytes.get(n * 128) {
            Some(tag) => display.extensions.push(extension_name(*tag)),
            None => break,
        }
    }
    Ok(display)
}

/// The lines of the report for a person.
pub fn lines(display: &Display) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut push = |k: &str, v: String| out.push((k.to_owned(), v));
    if let Some(source) = &display.source {
        push("Port", source.clone());
    }
    push("Maker", display.maker.clone());
    let product = format!("product {:#06x}", display.product);
    match &display.name {
        Some(name) => push("Model", format!("{name} ({product})")),
        None => push("Model", product),
    }
    match (&display.serial, display.serial_number) {
        (Some(text), _) => push("Serial", text.clone()),
        (None, n) if n != 0 => push("Serial", n.to_string()),
        _ => {}
    }
    let made = match (display.model_year, display.week) {
        (true, _) => format!("model year {}", display.year),
        (false, Some(week)) => format!("week {week} of {}", display.year),
        (false, None) => display.year.to_string(),
    };
    push("Made", made);
    push(
        "EDID",
        format!(
            "{}, {}{}",
            display.version,
            if display.digital { "digital" } else { "analog" },
            if display.checksum_ok {
                ""
            } else {
                ", bad checksum"
            }
        ),
    );
    if display.width_cm > 0 && display.height_cm > 0 {
        let w = f64::from(display.width_cm);
        let h = f64::from(display.height_cm);
        let inches = (w * w + h * h).sqrt() / 2.54;
        push(
            "Size",
            format!(
                "{} x {} cm, {inches:.1} in",
                display.width_cm, display.height_cm
            ),
        );
    }
    if let Some(mode) = &display.preferred {
        push(
            "Preferred",
            format!(
                "{} x {} at {:.2} Hz",
                mode.width, mode.height, mode.refresh_hz
            ),
        );
    }
    if let (Some(v), Some(h)) = (display.vertical_hz, display.horizontal_khz) {
        push(
            "Ranges",
            format!(
                "{}-{} Hz vertical, {}-{} kHz horizontal",
                v.0, v.1, h.0, h.1
            ),
        );
    }
    if !display.extensions.is_empty() {
        push("Extensions", display.extensions.join(", "));
    }
    out
}

/// The EDID of each display in `ioreg -a` of macOS, with the name of its
/// port. Apple silicon keeps it in IOPortTransportState as `EDID`, and an
/// Intel Mac keeps it in IODisplay as `IODisplayEDID`. A display that shows
/// twice is kept once.
pub fn from_ioreg(plist: &[u8]) -> Result<Vec<(Option<String>, Vec<u8>)>> {
    if plist.iter().all(u8::is_ascii_whitespace) {
        return Ok(Vec::new());
    }
    let value = plist::Value::from_reader_xml(plist)
        .context("ioreg gave a property list that is not valid")?;
    let mut out: Vec<(Option<String>, Vec<u8>)> = Vec::new();
    for entry in value.as_array().into_iter().flatten() {
        let Some(dict) = entry.as_dictionary() else {
            continue;
        };
        let Some(edid) = dict
            .get("EDID")
            .or_else(|| dict.get("IODisplayEDID"))
            .and_then(plist::Value::as_data)
        else {
            continue;
        };
        if edid.is_empty() || out.iter().any(|(_, e)| e == edid) {
            continue;
        }
        let source = dict
            .get("TransportDescription")
            .and_then(plist::Value::as_string)
            .map(|s| s.trim_end_matches("/DisplayPort").to_owned());
        out.push((source, edid.to_vec()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The EDID of a real portable monitor, as macOS read it through a USB-C
    // adapter.
    const PORTABLE: &str = "00ffffffffffff000e0f9173130000002d230104b52316781f5d40945b5794291c505421080001010101010101010101010101010101d04180a070004650302053005ee110000018000000ff003030310a202020202020202020000000fd00303e64b41f010a202020202020000000fc004e53454f56530a202020202020015202031cf14590040300002309070783010000e305e301e606000062629c45801871004650302053005ee1100000180000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004e";

    fn bytes(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn a_real_edid_gives_what_the_host_shows() {
        let d = decode(&bytes(PORTABLE), Some("Port-USB-C@2".to_owned())).unwrap();
        // macOS names this display NSEOVS, product 29585, serial 19, maker
        // CPO, made in 2025, with a largest mode of 1920 x 1280.
        assert_eq!(d.maker, "CPO");
        assert_eq!(d.product, 29585);
        assert_eq!(d.name.as_deref(), Some("NSEOVS"));
        assert_eq!(d.serial_number, 19);
        assert_eq!(d.serial.as_deref(), Some("001"));
        assert_eq!((d.week, d.year, d.model_year), (Some(45), 2025, false));
        assert_eq!(d.version, "1.4");
        assert!(d.digital);
        assert_eq!((d.width_cm, d.height_cm), (35, 22));
        let mode = d.preferred.clone().unwrap();
        assert_eq!((mode.width, mode.height), (1920, 1280));
        assert_eq!(mode.pixel_clock_khz, 168_480);
        assert_eq!(mode.refresh_hz, 60.0);
        assert_eq!(d.vertical_hz, Some((48, 62)));
        assert_eq!(d.horizontal_khz, Some((100, 180)));
        assert_eq!(d.max_pixel_clock_mhz, Some(310));
        assert_eq!(d.extensions, ["CTA-861"]);
        assert!(d.checksum_ok);
    }

    #[test]
    fn the_report_names_the_model_and_the_mode() {
        let d = decode(&bytes(PORTABLE), None).unwrap();
        let lines = lines(&d);
        let get = |k: &str| {
            lines
                .iter()
                .find(|(key, _)| key == k)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(get("Model"), Some("NSEOVS (product 0x7391)"));
        assert_eq!(get("Preferred"), Some("1920 x 1280 at 60.00 Hz"));
        assert_eq!(get("Size"), Some("35 x 22 cm, 16.3 in"));
        assert_eq!(get("Port"), None);
    }

    #[test]
    fn a_changed_byte_fails_the_checksum() {
        let mut b = bytes(PORTABLE);
        b[20] ^= 1;
        assert!(!decode(&b, None).unwrap().checksum_ok);
    }

    #[test]
    fn a_short_or_strange_block_is_refused() {
        assert!(decode(&[0u8; 64], None).is_err());
        assert!(decode(&[0u8; 128], None).is_err());
    }

    #[test]
    fn a_week_of_255_gives_a_model_year() {
        let mut b = bytes(PORTABLE);
        b[16] = 0xff;
        let d = decode(&b, None).unwrap();
        assert!(d.model_year);
        assert_eq!(d.week, None);
    }

    fn ioreg(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let array: Vec<plist::Value> = entries
            .iter()
            .map(|(port, edid)| {
                let mut dict = plist::Dictionary::new();
                dict.insert(
                    "TransportDescription".to_owned(),
                    plist::Value::String((*port).to_owned()),
                );
                if !edid.is_empty() {
                    dict.insert("EDID".to_owned(), plist::Value::Data(edid.to_vec()));
                }
                plist::Value::Dictionary(dict)
            })
            .collect();
        let mut out = Vec::new();
        plist::Value::Array(array).to_writer_xml(&mut out).unwrap();
        out
    }

    #[test]
    fn ioreg_gives_each_display_once_with_its_port() {
        let edid = bytes(PORTABLE);
        let plist = ioreg(&[
            ("Port-USB-C@1/DisplayPort", &[]),
            ("Port-USB-C@2/DisplayPort", &edid),
            ("Port-USB-C@2/DisplayPort", &edid),
        ]);
        let found = from_ioreg(&plist).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0.as_deref(), Some("Port-USB-C@2"));
        assert_eq!(found[0].1, edid);
    }

    #[test]
    fn empty_ioreg_output_is_no_display() {
        assert!(from_ioreg(b"").unwrap().is_empty());
    }
}
