//! The host file of `att watch` on Windows: the shell, the control
//! sequences of the console, and the size of the console.

use std::os::windows::process::CommandExt;
use std::process::Command;

use windows_sys::Win32::System::Console::{
    CONSOLE_SCREEN_BUFFER_INFO, ENABLE_VIRTUAL_TERMINAL_PROCESSING, GetConsoleMode,
    GetConsoleScreenBufferInfo, GetStdHandle, STD_OUTPUT_HANDLE, SetConsoleMode,
};

/// The command line, run by `cmd /C`. The line goes to cmd as it is, so
/// the quotes of the person stay as they typed them.
pub fn shell(line: &str) -> Command {
    let mut command = Command::new("cmd");
    command.arg("/C").raw_arg(line);
    command
}

/// Ask the console to act on the control sequences that clear the screen
/// and mark the changes. A console of Windows 10 and later can do this, and
/// does not do it by default.
pub fn prepare() {
    // SAFETY: each call takes the handle of the output stream, and a mode
    // that lives for the call.
    #[allow(unsafe_code)]
    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        let mut mode = 0;
        if GetConsoleMode(handle, &mut mode) != 0 {
            SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
        }
    }
}

/// The columns and the rows of the window of the console.
pub fn size() -> Option<(u16, u16)> {
    // SAFETY: the record holds only numbers, so all zeros is a valid value,
    // and the call writes one record into it.
    #[allow(unsafe_code)]
    unsafe {
        let mut info: CONSOLE_SCREEN_BUFFER_INFO = std::mem::zeroed();
        if GetConsoleScreenBufferInfo(GetStdHandle(STD_OUTPUT_HANDLE), &mut info) == 0 {
            return None;
        }
        let window = info.srWindow;
        let cols = u16::try_from(window.Right - window.Left + 1).ok()?;
        let rows = u16::try_from(window.Bottom - window.Top + 1).ok()?;
        Some((cols, rows))
    }
}
