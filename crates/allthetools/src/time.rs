//! Read a time that a person types.
//!
//! This module makes no system call, so its tests run on every host.

use std::time::Duration;

use anyhow::{Result, bail};

/// Read a time such as `10`, `1.5s`, `2m`, `1h` or `1d`.
///
/// A number with no unit is seconds, as in GNU `timeout`. Zero is a time,
/// and the tool that reads it says what zero means.
pub fn duration(text: &str) -> Result<Duration> {
    let text = text.trim();
    let (number, scale) = match text.char_indices().last() {
        Some((i, 's')) => (&text[..i], 1.0),
        Some((i, 'm')) => (&text[..i], 60.0),
        Some((i, 'h')) => (&text[..i], 3600.0),
        Some((i, 'd')) => (&text[..i], 86400.0),
        _ => (text, 1.0),
    };
    let value: f64 = match number.parse() {
        Ok(v) => v,
        Err(_) => bail!("`{text}` is not a time. Give a number of seconds, or add s, m, h or d"),
    };
    if !value.is_finite() || value < 0.0 {
        bail!("`{text}` is not a time. Give a number that is zero or more");
    }
    Ok(Duration::from_secs_f64(value * scale))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_number_is_seconds() {
        assert_eq!(duration("10").unwrap(), Duration::from_secs(10));
        assert_eq!(duration("0.5").unwrap(), Duration::from_millis(500));
    }

    #[test]
    fn each_unit_scales_the_number() {
        assert_eq!(duration("1.5s").unwrap(), Duration::from_millis(1500));
        assert_eq!(duration("2m").unwrap(), Duration::from_secs(120));
        assert_eq!(duration("1h").unwrap(), Duration::from_secs(3600));
        assert_eq!(duration("1d").unwrap(), Duration::from_secs(86400));
    }

    #[test]
    fn zero_is_a_time() {
        assert_eq!(duration("0").unwrap(), Duration::ZERO);
    }

    #[test]
    fn a_bad_time_is_refused() {
        for bad in ["", "s", "abc", "-1", "1x", "inf", "NaN"] {
            assert!(duration(bad).is_err(), "{bad} was accepted");
        }
    }
}
