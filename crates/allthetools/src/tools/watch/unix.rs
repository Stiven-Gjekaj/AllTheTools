//! The host file of `att watch` on macOS and Linux: the shell, and the size
//! of the terminal.

use std::process::Command;

/// The command line, run by `sh -c`, as `watch` runs it.
pub fn shell(line: &str) -> Command {
    let mut command = Command::new("sh");
    command.arg("-c").arg(line);
    command
}

/// A terminal of macOS and Linux takes the control sequences as they are.
pub fn prepare() {}

/// The columns and the rows of the terminal of the output stream.
pub fn size() -> Option<(u16, u16)> {
    // SAFETY: winsize holds only numbers, so all zeros is a valid value.
    #[allow(unsafe_code)]
    let mut size: libc::winsize = unsafe { std::mem::zeroed() };
    // SAFETY: TIOCGWINSZ writes one winsize, into a variable of that type.
    #[allow(unsafe_code)]
    let status = unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut size) };
    (status == 0 && size.ws_col > 0 && size.ws_row > 0).then_some((size.ws_col, size.ws_row))
}
