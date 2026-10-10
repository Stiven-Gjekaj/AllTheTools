//! The host file of `att clip` on macOS and on Linux.
//!
//! macOS ships pbcopy and pbpaste. A Linux desktop has the program of its
//! display server, and parse.rs chooses it.

use anyhow::Result;

use crate::host;

/// A program and its arguments.
type Program = (&'static str, &'static [&'static str]);

#[cfg(target_os = "macos")]
fn programs() -> Result<(Program, Program)> {
    Ok((("pbcopy", &[]), ("pbpaste", &[])))
}

#[cfg(target_os = "linux")]
fn programs() -> Result<(Program, Program)> {
    let set = |name| std::env::var_os(name).is_some_and(|v| !v.is_empty());
    let backend = super::parse::linux(set("WAYLAND_DISPLAY"), set("DISPLAY"), &host::on_path)?;
    Ok((backend.copy(), backend.paste()))
}

pub fn copy(text: &str) -> Result<()> {
    let ((program, args), _) = programs()?;
    // pbcopy reads its input in the encoding of the locale. A locale of
    // UTF-8 keeps each character whole.
    // SAFETY: set_var runs before any other thread of att starts.
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("LC_CTYPE", "UTF-8");
    }
    host::feed(program, args, text.as_bytes(), None)
}

pub fn paste() -> Result<String> {
    let (_, (program, args)) = programs()?;
    // SAFETY: as in copy.
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("LC_CTYPE", "UTF-8");
    }
    let bytes = host::output(program, args, None)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
