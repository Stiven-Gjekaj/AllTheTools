//! The host file of `att timeout` on Linux and on macOS.
//!
//! The command starts in a process group of its own, so the signal reaches
//! each process that the command starts. When the command runs in a
//! terminal, its group gets the terminal, as a shell gives it to a job.

use std::ffi::OsString;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, Command, ExitStatus};
use std::time::{Duration, Instant};

use super::parse::{Ended, Outcome, Signal};
use super::{Failure, Plan};

/// The time between two checks of the command.
const POLL: Duration = Duration::from_millis(10);

pub fn run(plan: &Plan, program: &OsString, rest: &[OsString]) -> Result<Outcome, Failure> {
    let mut command = Command::new(program);
    command.args(rest);
    let terminal = !plan.foreground && owns_terminal();
    if !plan.foreground {
        command.process_group(0);
    }
    if terminal {
        // SAFETY: the closure runs in the child between fork and exec. It
        // calls only functions that are safe there: signal, tcsetpgrp and
        // getpid.
        #[allow(unsafe_code)]
        unsafe {
            command.pre_exec(|| {
                take_terminal(libc::getpid());
                Ok(())
            });
        }
    }

    let mut child = command.spawn()?;
    let target = child.id() as libc::pid_t;
    if terminal {
        // The parent sets it too, so the order of the two does not matter.
        take_terminal(target);
    }

    let result = watch(plan, &mut child, target);
    if terminal {
        // SAFETY: getpgrp has no precondition.
        #[allow(unsafe_code)]
        take_terminal(unsafe { libc::getpgrp() });
    }
    result
}

fn watch(plan: &Plan, child: &mut Child, target: libc::pid_t) -> Result<Outcome, Failure> {
    let group = !plan.foreground;
    let mut timed_out = false;
    let mut killed = false;

    if let Some(limit) = plan.limit {
        if let Some(status) = wait_for(child, limit)? {
            return Ok(outcome(status, false, false));
        }
        timed_out = true;
        killed = plan.signal == Signal::Kill;
        send(target, group, number(plan.signal));

        if let Some(grace) = plan.kill_after.filter(|_| !killed) {
            if let Some(status) = wait_for(child, grace)? {
                return Ok(outcome(status, timed_out, killed));
            }
            killed = true;
            send(target, group, libc::SIGKILL);
        }
    }

    let status = child.wait().map_err(|e| Failure::Other(e.into()))?;
    Ok(outcome(status, timed_out, killed))
}

/// Wait for the command until the time runs out. `None` means that it still
/// runs.
fn wait_for(child: &mut Child, time: Duration) -> Result<Option<ExitStatus>, Failure> {
    let end = Instant::now() + time;
    loop {
        if let Some(status) = child.try_wait().map_err(|e| Failure::Other(e.into()))? {
            return Ok(Some(status));
        }
        let now = Instant::now();
        if now >= end {
            return Ok(None);
        }
        std::thread::sleep(POLL.min(end - now));
    }
}

fn outcome(status: ExitStatus, timed_out: bool, killed: bool) -> Outcome {
    let ended = match (status.code(), status.signal()) {
        (Some(code), _) => Ended::Code(code),
        (None, Some(signal)) => Ended::Signal(signal),
        (None, None) => Ended::Code(1),
    };
    Outcome {
        ended,
        timed_out,
        killed,
    }
}

fn number(signal: Signal) -> libc::c_int {
    match signal {
        Signal::Hup => libc::SIGHUP,
        Signal::Int => libc::SIGINT,
        Signal::Quit => libc::SIGQUIT,
        Signal::Kill => libc::SIGKILL,
        Signal::Usr1 => libc::SIGUSR1,
        Signal::Usr2 => libc::SIGUSR2,
        Signal::Alrm => libc::SIGALRM,
        Signal::Term => libc::SIGTERM,
    }
}

/// Send a signal to the group of the command, or to the command alone.
fn send(target: libc::pid_t, group: bool, signal: libc::c_int) {
    let pid = if group { -target } else { target };
    // SAFETY: kill has no precondition. A process that already ended gives
    // ESRCH, and the wait that comes next reads its status.
    #[allow(unsafe_code)]
    unsafe {
        libc::kill(pid, signal);
    }
}

/// True when the input is a terminal and this process is in the foreground
/// of it.
fn owns_terminal() -> bool {
    // SAFETY: isatty, tcgetpgrp and getpgrp only read.
    #[allow(unsafe_code)]
    unsafe {
        libc::isatty(libc::STDIN_FILENO) == 1
            && libc::tcgetpgrp(libc::STDIN_FILENO) == libc::getpgrp()
    }
}

/// Give the terminal to a process group.
///
/// A process that is not in the foreground gets SIGTTOU when it does this,
/// and that signal stops it. So SIGTTOU is ignored for the call.
fn take_terminal(group: libc::pid_t) {
    // SAFETY: signal and tcsetpgrp have no precondition. The old handler goes
    // back at once.
    #[allow(unsafe_code)]
    unsafe {
        let old = libc::signal(libc::SIGTTOU, libc::SIG_IGN);
        libc::tcsetpgrp(libc::STDIN_FILENO, group);
        libc::signal(libc::SIGTTOU, old);
    }
}
