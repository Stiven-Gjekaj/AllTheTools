//! The decisions of `att timeout`: the time, the signal and the exit code.
//!
//! This module makes no system call and names no type of a host crate, so
//! its tests run on every host.

use std::time::Duration;

use anyhow::{Result, bail};

/// The exit code when the command runs past its time, as GNU `timeout` gives
/// it.
pub const TIMED_OUT: i32 = 124;
/// The exit code when `timeout` itself fails.
pub const FAILED: i32 = 125;
/// The exit code when the command exists and does not start.
pub const CANNOT_RUN: i32 = 126;
/// The exit code when the command does not exist.
pub const NOT_FOUND: i32 = 127;

/// Read a time such as `10`, `1.5s`, `2m`, `1h` or `1d`.
///
/// A number with no unit is seconds, as in GNU `timeout`. A time of zero
/// turns the limit off.
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

/// A signal that `att timeout` can send.
///
/// The number of a signal is different on Linux and on macOS, so this
/// module names the signal and the host file gives the number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    Hup,
    Int,
    Quit,
    Kill,
    Usr1,
    Usr2,
    Alrm,
    Term,
}

/// Read a signal name, with or without `SIG`, in any case.
///
/// A number is accepted for the signals whose number is the same on Linux and
/// on macOS.
pub fn signal(text: &str) -> Result<Signal> {
    let upper = text.trim().to_ascii_uppercase();
    let name = upper.strip_prefix("SIG").unwrap_or(&upper);
    let signal = match name {
        "HUP" | "1" => Signal::Hup,
        "INT" | "2" => Signal::Int,
        "QUIT" | "3" => Signal::Quit,
        "KILL" | "9" => Signal::Kill,
        "USR1" => Signal::Usr1,
        "USR2" => Signal::Usr2,
        "ALRM" | "14" => Signal::Alrm,
        "TERM" | "15" => Signal::Term,
        _ => bail!(
            "`{text}` is not a signal that att timeout sends. \
             Use HUP, INT, QUIT, KILL, USR1, USR2, ALRM or TERM"
        ),
    };
    Ok(signal)
}

/// How the command ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    /// The command gave this exit code.
    Code(i32),
    /// A signal with this number stopped the command.
    Signal(i32),
}

/// What happened in one run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Outcome {
    pub ended: Ended,
    /// The time ran out, and the signal went to the command.
    pub timed_out: bool,
    /// The kill signal went to the command, at the end of the time or at the
    /// end of the time after it.
    pub killed: bool,
}

/// The exit code of `att timeout`, by the rules of GNU `timeout`.
///
/// - A command that the kill signal stopped gives 137, which is 128 and 9.
/// - A command that ran past its time gives 124, unless `preserve_status`.
/// - Any other command gives its own code, or 128 and the signal number.
pub fn exit_code(outcome: Outcome, preserve_status: bool) -> i32 {
    let own = match outcome.ended {
        Ended::Code(code) => code,
        Ended::Signal(number) => 128 + number,
    };
    if outcome.killed {
        128 + 9
    } else if outcome.timed_out && !preserve_status {
        TIMED_OUT
    } else {
        own
    }
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

    #[test]
    fn a_signal_name_takes_any_form() {
        assert_eq!(signal("TERM").unwrap(), Signal::Term);
        assert_eq!(signal("sigkill").unwrap(), Signal::Kill);
        assert_eq!(signal("Int").unwrap(), Signal::Int);
        assert_eq!(signal("9").unwrap(), Signal::Kill);
    }

    #[test]
    fn a_number_that_changes_between_hosts_is_refused() {
        // SIGUSR1 is 10 on Linux and 30 on macOS.
        assert!(signal("10").is_err());
        assert!(signal("30").is_err());
        assert!(signal("STOP").is_err());
    }

    fn outcome(ended: Ended, timed_out: bool, killed: bool) -> Outcome {
        Outcome {
            ended,
            timed_out,
            killed,
        }
    }

    #[test]
    fn a_command_that_ends_in_time_gives_its_own_code() {
        assert_eq!(exit_code(outcome(Ended::Code(3), false, false), false), 3);
        assert_eq!(
            exit_code(outcome(Ended::Signal(2), false, false), false),
            130
        );
    }

    #[test]
    fn a_command_past_its_time_gives_124() {
        let late = outcome(Ended::Signal(15), true, false);
        assert_eq!(exit_code(late, false), 124);
    }

    #[test]
    fn preserve_status_gives_the_code_of_the_command() {
        let late = outcome(Ended::Signal(15), true, false);
        assert_eq!(exit_code(late, true), 143);
        let caught = outcome(Ended::Code(0), true, false);
        assert_eq!(exit_code(caught, true), 0);
    }

    #[test]
    fn the_kill_signal_gives_137() {
        let killed = outcome(Ended::Signal(9), true, true);
        assert_eq!(exit_code(killed, false), 137);
        assert_eq!(exit_code(killed, true), 137);
    }
}
