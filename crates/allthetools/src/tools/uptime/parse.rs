//! The time since the start of the host, and the dates and the load that
//! `att uptime` shows.
//!
//! This module makes no system call, so its tests run on every host.

use serde::Serialize;

/// The time since the start of the host.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Uptime {
    pub seconds: u64,
    /// The start, in seconds since 1970-01-01 UTC.
    pub boot: u64,
    /// The load average of 1, 5 and 15 minutes. Windows has none.
    pub load: Option<[f64; 3]>,
}

/// A time such as `3 days, 4 h 05 min`.
pub fn duration(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = seconds % 86_400 / 3600;
    let minutes = seconds % 3600 / 60;
    let time = format!("{hours} h {minutes:02} min");
    match days {
        0 => time,
        1 => format!("1 day, {time}"),
        n => format!("{n} days, {time}"),
    }
}

/// A date and a time in UTC, such as `2026-10-14 08:12:01 UTC`, from the
/// seconds since 1970. The days become a date with the method of Howard
/// Hinnant, which has no table of months.
pub fn utc(unix: u64) -> String {
    let days = (unix / 86_400) as i64;
    let rest = unix % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// The three numbers of /proc/loadavg.
pub fn loadavg(text: &str) -> Option<[f64; 3]> {
    let mut parts = text.split_whitespace().map(|p| p.parse::<f64>().ok());
    Some([parts.next()??, parts.next()??, parts.next()??])
}

/// The seconds of /proc/uptime: the first number, with its fraction cut.
pub fn proc_uptime(text: &str) -> Option<u64> {
    let first = text.split_whitespace().next()?;
    first.split('.').next()?.parse().ok()
}

impl Uptime {
    pub fn lines(&self) -> Vec<(String, String)> {
        let mut out = vec![
            ("Up".to_owned(), duration(self.seconds)),
            ("Since".to_owned(), utc(self.boot)),
        ];
        if let Some([a, b, c]) = self.load {
            out.push((
                "Load".to_owned(),
                format!("{a:.2}, {b:.2}, {c:.2} (1, 5 and 15 min)"),
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_duration_has_days_hours_and_minutes() {
        assert_eq!(duration(59), "0 h 00 min");
        assert_eq!(duration(3 * 3600 + 5 * 60), "3 h 05 min");
        assert_eq!(duration(86_400 + 60), "1 day, 0 h 01 min");
        assert_eq!(
            duration(3 * 86_400 + 4 * 3600 + 5 * 60 + 9),
            "3 days, 4 h 05 min"
        );
    }

    // Each expected date is what `date -u -r <seconds>` gives.
    #[test]
    fn utc_dates_match_known_days() {
        assert_eq!(utc(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(utc(951_782_400), "2000-02-29 00:00:00 UTC");
        assert_eq!(utc(1_792_154_521), "2026-10-16 12:42:01 UTC");
        assert_eq!(utc(4_107_542_399), "2100-02-28 23:59:59 UTC");
    }

    #[test]
    fn proc_files_give_their_numbers() {
        assert_eq!(proc_uptime("12345.67 54321.00\n"), Some(12345));
        assert_eq!(
            loadavg("0.52 0.58 0.59 1/389 12345\n"),
            Some([0.52, 0.58, 0.59])
        );
        assert_eq!(loadavg("0.5 x"), None);
    }

    #[test]
    fn the_load_line_shows_only_with_a_load() {
        let mut up = Uptime {
            seconds: 60,
            boot: 0,
            load: None,
        };
        assert_eq!(up.lines().len(), 2);
        up.load = Some([1.0, 0.5, 0.25]);
        assert_eq!(up.lines()[2].1, "1.00, 0.50, 0.25 (1, 5 and 15 min)");
    }
}
