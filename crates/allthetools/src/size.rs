//! Sizes for a person.
//!
//! This module makes no system call, so its tests run on every host.

/// A size the way the label on a drive gives it: powers of ten, one decimal.
///
/// A drive is sold in powers of ten, so a person recognises this number from
/// the box.
pub fn decimal(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["kB", "MB", "GB", "TB", "PB", "EB"];
    if bytes < 1000 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit = "B";
    for next in UNITS {
        if value < 1000.0 {
            break;
        }
        value /= 1000.0;
        unit = next;
    }
    format!("{value:.1} {unit}")
}

/// A size in powers of two, one decimal, as memory and a partition tool give
/// it.
pub fn binary(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit = "B";
    for next in UNITS {
        if value < 1024.0 {
            break;
        }
        value /= 1024.0;
        unit = next;
    }
    format!("{value:.1} {unit}")
}

/// A count with a comma between each group of three digits.
pub fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{binary, decimal, grouped};

    #[test]
    fn decimal_matches_the_label_on_the_box() {
        assert_eq!(decimal(0), "0 B");
        assert_eq!(decimal(999), "999 B");
        assert_eq!(decimal(1000), "1.0 kB");
        assert_eq!(decimal(62_521_344_000), "62.5 GB");
        assert_eq!(decimal(1_000_204_886_016), "1.0 TB");
    }

    #[test]
    fn binary_uses_powers_of_two() {
        assert_eq!(binary(1023), "1023 B");
        assert_eq!(binary(1024), "1.0 KiB");
        assert_eq!(binary(16 * 1024 * 1024 * 1024), "16.0 GiB");
    }

    #[test]
    fn grouped_puts_a_comma_between_groups_of_three() {
        assert_eq!(grouped(0), "0");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(1000), "1,000");
        assert_eq!(grouped(62_521_344_000), "62,521,344,000");
    }
}
