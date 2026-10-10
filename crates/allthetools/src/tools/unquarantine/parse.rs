//! Read the download mark of each host, so `att unquarantine --check` says
//! where a file came from.
//!
//! This module makes no system call, so its tests run on every host.

/// What a download mark says.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Mark {
    /// The program that downloaded the file, such as Safari.
    pub agent: Option<String>,
    /// The address that the file came from.
    pub url: Option<String>,
    /// The zone of Windows: 3 is the internet.
    pub zone: Option<u32>,
    /// The time of the download, in seconds after 1970.
    pub time: Option<u64>,
}

/// Read the value of `com.apple.quarantine` on macOS.
///
/// The value is `flags;time;agent;id`, and the time is in hexadecimal.
pub fn apple(value: &[u8]) -> Mark {
    let text = String::from_utf8_lossy(value);
    let fields: Vec<&str> = text.trim_end_matches('\0').split(';').collect();
    let nonempty = |i: usize| {
        fields
            .get(i)
            .map(|f| f.trim())
            .filter(|f| !f.is_empty())
            .map(str::to_owned)
    };
    Mark {
        agent: nonempty(2),
        url: None,
        zone: None,
        time: fields
            .get(1)
            .and_then(|t| u64::from_str_radix(t.trim(), 16).ok()),
    }
}

/// Read the `Zone.Identifier` stream of Windows, which is a small INI file.
pub fn zone_identifier(value: &[u8]) -> Mark {
    let text = decode(value);
    let mut mark = Mark::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "ZoneId" => mark.zone = value.parse().ok(),
            "HostUrl" => mark.url = Some(value.to_owned()),
            "ReferrerUrl" if mark.url.is_none() => mark.url = Some(value.to_owned()),
            _ => {}
        }
    }
    mark
}

/// A stream that a program wrote in UTF-16 starts with its byte order mark.
fn decode(value: &[u8]) -> String {
    if value.starts_with(&[0xFF, 0xFE]) {
        let units: Vec<u16> = value[2..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    let value = value.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(value);
    String::from_utf8_lossy(value).into_owned()
}

/// Read the origin mark that Chromium and other programs write on Linux, in
/// `user.xdg.origin.url`.
pub fn xdg_origin(value: &[u8]) -> Mark {
    Mark {
        url: Some(
            String::from_utf8_lossy(value)
                .trim_end_matches('\0')
                .to_owned(),
        ),
        ..Mark::default()
    }
}

/// The mark for a person, in one line.
pub fn describe(mark: &Mark) -> String {
    let mut parts = Vec::new();
    if let Some(agent) = &mark.agent {
        parts.push(format!("downloaded by {agent}"));
    }
    if let Some(url) = &mark.url {
        parts.push(format!("from {url}"));
    }
    if let Some(zone) = mark.zone {
        let name = match zone {
            0 => "this computer",
            1 => "the local intranet",
            2 => "trusted sites",
            3 => "the internet",
            4 => "restricted sites",
            _ => "an unknown zone",
        };
        parts.push(format!("zone {zone}, {name}"));
    }
    if parts.is_empty() {
        "marked as downloaded".to_owned()
    } else {
        parts.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apple_reads_the_agent_and_the_time() {
        let mark = apple(b"0083;66f1c2a0;Safari;6F2E1A3B-0000-0000-0000-000000000000");
        assert_eq!(mark.agent.as_deref(), Some("Safari"));
        assert_eq!(mark.time, Some(0x66f1c2a0));
    }

    #[test]
    fn apple_takes_a_value_with_empty_fields() {
        let mark = apple(b"0081;66f1c2a0;;");
        assert_eq!(mark.agent, None);
        assert_eq!(describe(&mark), "marked as downloaded");
        assert_eq!(apple(b""), Mark::default());
    }

    #[test]
    fn zone_identifier_reads_the_zone_and_the_address() {
        let stream = b"[ZoneTransfer]\r\nZoneId=3\r\nReferrerUrl=https://example.com/\r\nHostUrl=https://example.com/a.exe\r\n";
        let mark = zone_identifier(stream);
        assert_eq!(mark.zone, Some(3));
        assert_eq!(mark.url.as_deref(), Some("https://example.com/a.exe"));
        assert_eq!(
            describe(&mark),
            "from https://example.com/a.exe, zone 3, the internet"
        );
    }

    #[test]
    fn zone_identifier_takes_utf16() {
        let text = "[ZoneTransfer]\r\nZoneId=3\r\n";
        let mut stream = vec![0xFF, 0xFE];
        for unit in text.encode_utf16() {
            stream.extend_from_slice(&unit.to_le_bytes());
        }
        assert_eq!(zone_identifier(&stream).zone, Some(3));
    }

    #[test]
    fn xdg_origin_is_the_address() {
        let mark = xdg_origin(b"https://example.com/tool.tar.gz");
        assert_eq!(describe(&mark), "from https://example.com/tool.tar.gz");
    }
}
