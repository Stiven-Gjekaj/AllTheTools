//! `att awake`: keep the system awake.
//!
//! macOS has `caffeinate`. Linux has `systemd-inhibit`, with other options.
//! Windows has no command for it. This tool takes the options of
//! `caffeinate` on each host.

use std::ffi::OsString;
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

pub mod parse;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as host;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as host;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as host;

use parse::Until;

/// Keep the system awake for a time, while a process runs, or while a
/// command runs.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Keep the display on too.
    #[arg(short, long)]
    pub display: bool,

    /// Keep the system awake for this time: seconds, or a number with s, m,
    /// h or d.
    #[arg(short, long, value_name = "TIME")]
    pub time: Option<String>,

    /// Keep the system awake until the process with this number ends.
    #[arg(short, long, value_name = "PID")]
    pub wait: Option<u32>,

    /// The command to run. The system stays awake until it ends, and att
    /// awake gives its exit code.
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        conflicts_with_all = ["time", "wait"]
    )]
    pub command: Vec<OsString>,
}

/// The time between two checks of a process.
const POLL: Duration = Duration::from_millis(500);

pub fn run(args: Args) -> Result<i32> {
    let limit = args
        .time
        .as_deref()
        .map(crate::time::duration)
        .transpose()?;
    let until = parse::until(!args.command.is_empty(), args.wait, limit);
    if let Until::Process { pid, .. } = until {
        anyhow::ensure!(host::alive(pid), "no process has the number {pid}");
    }

    let guard = host::hold(args.display)?;
    eprintln!("att awake: {}", parse::describe(&until, args.display));

    let code = match until {
        Until::Command => {
            let (program, rest) = args
                .command
                .split_first()
                .expect("Until::Command needs a command");
            let status = Command::new(program)
                .args(rest)
                .status()
                .with_context(|| format!("could not run {}", program.to_string_lossy()))?;
            status.code().unwrap_or(1)
        }
        Until::Process { pid, limit } => {
            let end = limit.map(|l| Instant::now() + l);
            while host::alive(pid) && end.is_none_or(|end| Instant::now() < end) {
                std::thread::sleep(POLL);
            }
            0
        }
        Until::Time(limit) => {
            std::thread::sleep(limit);
            0
        }
        Until::Stopped => loop {
            std::thread::sleep(Duration::from_secs(3600));
        },
    };
    drop(guard);
    Ok(code)
}

/// True when the process exists. A process of another user gives EPERM,
/// and it exists too.
#[cfg(unix)]
fn unix_alive(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    // SAFETY: signal 0 checks the process and sends nothing.
    #[allow(unsafe_code)]
    let result = unsafe { libc::kill(pid, 0) };
    result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}
