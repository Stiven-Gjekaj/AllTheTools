//! `att watch`: run a command again and again, and show its output on a
//! clear screen.
//!
//! Linux has `watch` in procps. macOS and Windows have no such command. This
//! tool takes the main options of `watch`: the interval, the marks on the
//! changes, and an exit when the output changes or the command fails.

use std::io::Write;
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result, bail};

pub mod screen;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix as host;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as host;

/// Run a command again and again, and show its output on a clear screen.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The time between two runs: a number of seconds, or a time such as
    /// 500ms or 1m.
    #[arg(short = 'n', long, default_value = "2", value_name = "TIME")]
    pub interval: String,

    /// Mark the characters that changed since the run before.
    #[arg(short, long)]
    pub differences: bool,

    /// Stop when the output changes, with exit code 0.
    #[arg(short = 'g', long = "chgexit")]
    pub change_exit: bool,

    /// Stop when the command fails, with the exit code of the command.
    #[arg(short, long = "errexit")]
    pub error_exit: bool,

    /// Show no title line.
    #[arg(short = 't', long)]
    pub no_title: bool,

    /// Run the command directly, and not through the shell of the host.
    #[arg(short = 'x', long)]
    pub exec: bool,

    /// The command and its arguments.
    #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
    pub command: Vec<String>,
}

/// The output of one run: the output stream, then the error stream.
struct Run {
    text: String,
    code: Option<i32>,
}

fn run_once(args: &Args) -> Result<Run> {
    let mut command = if args.exec {
        let mut command = Command::new(&args.command[0]);
        command.args(&args.command[1..]);
        command
    } else {
        host::shell(&args.command.join(" "))
    };
    let out = command
        .stdin(std::process::Stdio::null())
        .output()
        .with_context(|| format!("could not start `{}`", args.command[0]))?;
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    Ok(Run {
        text,
        code: out.status.code(),
    })
}

pub fn run(args: Args) -> Result<i32> {
    let interval = crate::time::duration(&args.interval)?;
    if interval < Duration::from_millis(100) {
        bail!("the interval is less than 0.1 s. Give a longer time");
    }
    host::prepare();
    let command = args.command.join(" ");
    let mut before: Option<String> = None;
    let mut shown: Vec<String> = Vec::new();
    loop {
        let result = run_once(&args)?;
        let (cols, rows) = host::size().unwrap_or((80, 24));
        let (cols, rows) = (usize::from(cols), usize::from(rows));
        let body_rows = if args.no_title {
            rows
        } else {
            rows.saturating_sub(2)
        };
        let lines = screen::fit(&result.text, body_rows.max(1), cols.max(1));
        let marked = if args.differences && before.is_some() {
            screen::mark(&shown, &lines)
        } else {
            lines.clone()
        };
        let mut out = std::io::stdout().lock();
        // Move to the top left, and clear the screen.
        write!(out, "\x1b[H\x1b[2J")?;
        if !args.no_title {
            let title: String = screen::title(interval, &command, result.code)
                .chars()
                .take(cols)
                .collect();
            writeln!(out, "{title}\n")?;
        }
        for line in &marked {
            writeln!(out, "{line}")?;
        }
        out.flush()?;
        drop(out);

        if args.error_exit && result.code != Some(0) {
            return Ok(result.code.unwrap_or(1));
        }
        if args.change_exit && before.as_ref().is_some_and(|b| *b != result.text) {
            return Ok(0);
        }
        before = Some(result.text);
        shown = lines;
        std::thread::sleep(interval);
    }
}
