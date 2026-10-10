//! The arithmetic of `att netquality`, and the head of an HTTP response.
//!
//! This module makes no system call, so its tests run on every host.

use std::time::Duration;

/// Megabits per second, from a count of bytes in a time.
pub fn mbps(bytes: u64, time: Duration) -> f64 {
    let seconds = time.as_secs_f64();
    if seconds <= 0.0 {
        return 0.0;
    }
    bytes as f64 * 8.0 / seconds / 1_000_000.0
}

/// The middle value of the samples, or `None` for no sample.
pub fn median(samples: &[f64]) -> Option<f64> {
    let mut sorted: Vec<f64> = samples.iter().copied().filter(|s| s.is_finite()).collect();
    if sorted.is_empty() {
        return None;
    }
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    Some(if sorted.len().is_multiple_of(2) {
        (sorted[middle - 1] + sorted[middle]) / 2.0
    } else {
        sorted[middle]
    })
}

/// Round trips per minute: how many times a request and its answer fit into
/// one minute, at this latency. A higher number is a network that answers
/// faster while it is busy.
pub fn rpm(latency_ms: f64) -> Option<f64> {
    (latency_ms > 0.0).then(|| 60_000.0 / latency_ms)
}

/// The request for a probe or a download.
pub fn get(host: &str, path: &str) -> String {
    format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: att-netquality\r\n\
         Accept: */*\r\nConnection: keep-alive\r\n\r\n"
    )
}

/// The head of an upload, before its body.
pub fn post(host: &str, path: &str, length: u64) -> String {
    format!(
        "POST {path} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: att-netquality\r\n\
         Content-Type: application/octet-stream\r\nContent-Length: {length}\r\n\
         Connection: keep-alive\r\n\r\n"
    )
}

/// What the head of a response says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Head {
    pub status: u16,
    pub length: Option<u64>,
    /// The number of bytes of the head, with the empty line at its end.
    pub size: usize,
}

/// Read the head of a response from the first bytes, or `None` when the
/// empty line that ends the head has not arrived yet.
pub fn head(bytes: &[u8]) -> Option<Head> {
    let end = bytes.windows(4).position(|w| w == b"\r\n\r\n")?;
    let text = std::str::from_utf8(&bytes[..end]).ok()?;
    let mut lines = text.split("\r\n");
    let status = lines.next()?.split_whitespace().nth(1)?.parse().ok()?;
    let length = lines.find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.trim()
            .eq_ignore_ascii_case("content-length")
            .then(|| value.trim().parse().ok())?
    });
    Some(Head {
        status,
        length,
        size: end + 4,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mbps_counts_bits() {
        assert_eq!(mbps(12_500_000, Duration::from_secs(1)), 100.0);
        assert_eq!(mbps(1_000_000, Duration::ZERO), 0.0);
    }

    #[test]
    fn median_takes_the_middle() {
        assert_eq!(median(&[]), None);
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&[4.0, 1.0, 3.0, 2.0]), Some(2.5));
        assert_eq!(median(&[f64::NAN, 5.0]), Some(5.0));
    }

    #[test]
    fn rpm_is_a_minute_over_the_latency() {
        assert_eq!(rpm(60.0), Some(1000.0));
        assert_eq!(rpm(0.0), None);
    }

    #[test]
    fn head_reads_the_status_and_the_length() {
        let reply = b"HTTP/1.1 200 OK\r\nDate: x\r\ncontent-length: 1000\r\n\r\nbody";
        assert_eq!(
            head(reply),
            Some(Head {
                status: 200,
                length: Some(1000),
                size: reply.len() - 4
            })
        );
    }

    #[test]
    fn head_waits_for_the_empty_line() {
        assert_eq!(head(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n"), None);
    }

    #[test]
    fn head_without_a_length_has_none() {
        let reply = b"HTTP/1.1 204 No Content\r\n\r\n";
        assert_eq!(head(reply).map(|h| (h.status, h.length)), Some((204, None)));
    }

    #[test]
    fn requests_name_the_host_and_the_length() {
        assert!(get("example.com", "/x").starts_with("GET /x HTTP/1.1\r\nHost: example.com\r\n"));
        assert!(post("example.com", "/up", 42).contains("Content-Length: 42\r\n"));
        assert!(post("example.com", "/up", 42).ends_with("\r\n\r\n"));
    }
}
