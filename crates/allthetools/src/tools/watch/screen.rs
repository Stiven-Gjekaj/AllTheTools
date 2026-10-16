//! The screen of `att watch`: the title, the lines cut to the terminal, and
//! the marks on the characters that changed.
//!
//! This module makes no system call, so its tests run on every host.

use std::time::Duration;

/// The control sequence that starts a mark, and the one that ends it.
pub const MARK: &str = "\x1b[7m";
pub const PLAIN: &str = "\x1b[0m";

/// The title line: the interval and the command, as `watch` shows them.
pub fn title(interval: Duration, command: &str, code: Option<i32>) -> String {
    let mut text = format!("Every {:.1}s: {command}", interval.as_secs_f64());
    match code {
        Some(0) => {}
        Some(code) => text.push_str(&format!("  (exit {code})")),
        None => text.push_str("  (stopped by a signal)"),
    }
    text
}

/// Turn tabs into spaces to the next column that is a multiple of 8, and
/// drop each carriage return, so a line has the width that it shows.
fn expand(line: &str) -> String {
    let mut out = String::new();
    let mut column = 0;
    for c in line.chars() {
        match c {
            '\t' => {
                let next = (column / 8 + 1) * 8;
                out.extend(std::iter::repeat_n(' ', next - column));
                column = next;
            }
            '\r' => {}
            c => {
                out.push(c);
                column += 1;
            }
        }
    }
    out
}

/// The lines of the output that fit in `rows` lines of `cols` characters.
/// A longer line is cut, and the lines past the last row are dropped, as
/// `watch` does.
pub fn fit(output: &str, rows: usize, cols: usize) -> Vec<String> {
    output
        .lines()
        .take(rows)
        .map(|line| expand(line).chars().take(cols).collect())
        .collect()
}

/// The lines with a mark on each character that is not the same as the
/// character at the same place before.
pub fn mark(before: &[String], now: &[String]) -> Vec<String> {
    now.iter()
        .enumerate()
        .map(|(i, line)| {
            let old: Vec<char> = before
                .get(i)
                .map(|l| l.chars().collect())
                .unwrap_or_default();
            let mut out = String::new();
            let mut on = false;
            for (j, c) in line.chars().enumerate() {
                let changed = old.get(j) != Some(&c);
                if changed != on {
                    out.push_str(if changed { MARK } else { PLAIN });
                    on = changed;
                }
                out.push(c);
            }
            if on {
                out.push_str(PLAIN);
            }
            out
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_title_gives_the_interval_the_command_and_a_failure() {
        let every = Duration::from_secs(2);
        assert_eq!(title(every, "ls -l", Some(0)), "Every 2.0s: ls -l");
        assert_eq!(
            title(Duration::from_millis(500), "df", Some(3)),
            "Every 0.5s: df  (exit 3)"
        );
        assert!(title(every, "x", None).contains("signal"));
    }

    #[test]
    fn fit_cuts_long_lines_and_extra_rows() {
        let lines = fit("abcdef\nxy\nthird\n", 2, 4);
        assert_eq!(lines, ["abcd", "xy"]);
    }

    #[test]
    fn a_tab_becomes_spaces_to_the_next_stop() {
        assert_eq!(fit("a\tb\r\n", 5, 80), ["a       b"]);
        assert_eq!(fit("12345678\tx", 5, 80), ["12345678        x"]);
    }

    #[test]
    fn only_the_changed_characters_get_a_mark() {
        let before = vec!["load 0.10".to_owned()];
        let now = vec!["load 0.25".to_owned(), "new".to_owned()];
        let marked = mark(&before, &now);
        assert_eq!(marked[0], format!("load 0.{MARK}25{PLAIN}"));
        assert_eq!(marked[1], format!("{MARK}new{PLAIN}"));
    }

    #[test]
    fn the_same_lines_get_no_mark() {
        let lines = vec!["same".to_owned()];
        assert_eq!(mark(&lines, &lines), lines);
    }
}
