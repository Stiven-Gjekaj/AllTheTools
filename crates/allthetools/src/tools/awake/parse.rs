//! The decisions of `att awake`: when to let the system sleep again, and
//! what to ask of each host.
//!
//! This module makes no system call, so its tests run on every host.

use std::time::Duration;

/// The event that ends the hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Until {
    /// The command that att awake runs ends.
    Command,
    /// The process with this number ends, or the time runs out.
    Process { pid: u32, limit: Option<Duration> },
    /// The time runs out.
    Time(Duration),
    /// The person stops att awake with Ctrl-C.
    Stopped,
}

/// Choose the end of the hold from the options.
pub fn until(has_command: bool, pid: Option<u32>, limit: Option<Duration>) -> Until {
    match (has_command, pid, limit) {
        (true, _, _) => Until::Command,
        (false, Some(pid), limit) => Until::Process { pid, limit },
        (false, None, Some(limit)) => Until::Time(limit),
        (false, None, None) => Until::Stopped,
    }
}

/// The sentence that says how long the system stays awake.
pub fn describe(until: &Until, display: bool) -> String {
    let what = if display {
        "The system and the display stay awake"
    } else {
        "The system stays awake"
    };
    let end = match until {
        Until::Command => "until the command ends".to_owned(),
        Until::Process { pid, limit: None } => format!("until process {pid} ends"),
        Until::Process {
            pid,
            limit: Some(limit),
        } => format!(
            "until process {pid} ends, or for {} at most",
            seconds(*limit)
        ),
        Until::Time(limit) => format!("for {}", seconds(*limit)),
        Until::Stopped => "until you press Ctrl-C".to_owned(),
    };
    format!("{what} {end}.")
}

fn seconds(time: Duration) -> String {
    let total = time.as_secs_f64();
    if total == 1.0 {
        "1 second".to_owned()
    } else if total.fract() == 0.0 {
        format!("{total} seconds")
    } else {
        format!("{total:.1} seconds")
    }
}

/// The arguments of systemd-inhibit on Linux.
///
/// logind has no lock for the display alone. The idle lock stops the idle
/// action of logind, and a desktop that reads it also keeps the display on.
/// So the arguments are the same with and without `--display`.
pub fn inhibit_args() -> Vec<&'static str> {
    vec![
        "--what=sleep:idle",
        "--who=att awake",
        "--why=att awake holds the system awake",
        "--mode=block",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_ends_the_hold_before_anything_else() {
        let limit = Some(Duration::from_secs(5));
        assert_eq!(until(true, Some(7), limit), Until::Command);
    }

    #[test]
    fn a_process_takes_the_time_as_a_limit() {
        let limit = Some(Duration::from_secs(5));
        assert_eq!(
            until(false, Some(7), limit),
            Until::Process { pid: 7, limit }
        );
    }

    #[test]
    fn with_no_option_the_hold_lasts_until_ctrl_c() {
        assert_eq!(until(false, None, None), Until::Stopped);
        assert_eq!(
            until(false, None, Some(Duration::from_secs(3))),
            Until::Time(Duration::from_secs(3))
        );
    }

    #[test]
    fn describe_says_what_stays_awake_and_for_how_long() {
        assert_eq!(
            describe(&Until::Time(Duration::from_secs(90)), false),
            "The system stays awake for 90 seconds."
        );
        assert_eq!(
            describe(&Until::Stopped, true),
            "The system and the display stay awake until you press Ctrl-C."
        );
        assert_eq!(
            describe(
                &Until::Process {
                    pid: 42,
                    limit: Some(Duration::from_secs(1))
                },
                false
            ),
            "The system stays awake until process 42 ends, or for 1 second at most."
        );
    }

    #[test]
    fn inhibit_blocks_sleep_and_idle() {
        let args = inhibit_args();
        assert!(args.contains(&"--what=sleep:idle"), "{args:?}");
        assert!(args.contains(&"--mode=block"));
    }
}
