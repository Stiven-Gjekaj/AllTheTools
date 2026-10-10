//! `att timeout`: run a command, and stop it when its time runs out.
//!
//! macOS has no `timeout`. The `timeout` of Windows waits, and runs nothing.
//! This one takes the options and gives the exit codes of GNU `timeout`.

use std::ffi::OsString;
use std::time::Duration;

use anyhow::Result;

pub mod parse;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

use parse::Signal;

/// Run a command, and stop it when its time runs out.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The signal to send when the time runs out. Windows has no signals, so
    /// there the command always stops at once.
    #[arg(short, long, default_value = "TERM", value_name = "SIGNAL")]
    pub signal: String,

    /// Send KILL when the command still runs this long after the first
    /// signal.
    #[arg(short = 'k', long, value_name = "TIME")]
    pub kill_after: Option<String>,

    /// Give the exit code of the command, also when the time runs out.
    #[arg(long)]
    pub preserve_status: bool,

    /// Send the signal to the command only, and not to the processes that it
    /// starts. The command can then read from the terminal.
    #[arg(long)]
    pub foreground: bool,

    /// The time limit: a number of seconds, or a number with s, m, h or d.
    /// Zero turns the limit off.
    pub duration: String,

    /// The command to run, and its arguments.
    #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
    pub command: Vec<OsString>,
}

/// What the host file needs to run the command.
pub struct Plan {
    pub limit: Option<Duration>,
    pub signal: Signal,
    pub kill_after: Option<Duration>,
    pub foreground: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let plan = match make_plan(&args) {
        Ok(plan) => plan,
        Err(e) => {
            eprintln!("att timeout: {e:#}");
            return Ok(parse::FAILED);
        }
    };
    let (program, rest) = args
        .command
        .split_first()
        .expect("clap requires one word of the command");

    #[cfg(unix)]
    let started = unix::run(&plan, program, rest);
    #[cfg(windows)]
    let started = windows::run(&plan, program, rest);

    match started {
        Ok(outcome) => Ok(parse::exit_code(outcome, args.preserve_status)),
        Err(Failure::NotFound) => {
            eprintln!(
                "att timeout: {}: the command was not found",
                program.to_string_lossy()
            );
            Ok(parse::NOT_FOUND)
        }
        Err(Failure::CannotRun(e)) => {
            eprintln!("att timeout: {}: {e}", program.to_string_lossy());
            Ok(parse::CANNOT_RUN)
        }
        Err(Failure::Other(e)) => {
            eprintln!("att timeout: {e:#}");
            Ok(parse::FAILED)
        }
    }
}

fn make_plan(args: &Args) -> Result<Plan> {
    let limit = parse::duration(&args.duration)?;
    let kill_after = args
        .kill_after
        .as_deref()
        .map(parse::duration)
        .transpose()?;
    Ok(Plan {
        limit: (!limit.is_zero()).then_some(limit),
        signal: parse::signal(&args.signal)?,
        kill_after,
        foreground: args.foreground,
    })
}

/// Why the command did not run to its end.
pub enum Failure {
    NotFound,
    CannotRun(std::io::Error),
    Other(anyhow::Error),
}

impl From<std::io::Error> for Failure {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::NotFound => Failure::NotFound,
            std::io::ErrorKind::PermissionDenied => Failure::CannotRun(e),
            _ => Failure::Other(e.into()),
        }
    }
}
