//! The decisions of `att clip`: which tool holds the clipboard of Linux, and
//! the text format of the clipboard of Windows.
//!
//! This module makes no system call, so its tests run on every host.

use anyhow::{Result, bail};

/// A program that reads and writes the clipboard of a Linux desktop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Linux {
    /// wl-copy and wl-paste, for Wayland.
    Wayland,
    /// xclip, for X11.
    Xclip,
    /// xsel, for X11.
    Xsel,
}

impl Linux {
    /// The program and its arguments that write the clipboard.
    pub fn copy(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Linux::Wayland => ("wl-copy", &[]),
            Linux::Xclip => ("xclip", &["-selection", "clipboard", "-in"]),
            Linux::Xsel => ("xsel", &["--clipboard", "--input"]),
        }
    }

    /// The program and its arguments that read the clipboard.
    pub fn paste(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Linux::Wayland => ("wl-paste", &["--no-newline"]),
            Linux::Xclip => ("xclip", &["-selection", "clipboard", "-out"]),
            Linux::Xsel => ("xsel", &["--clipboard", "--output"]),
        }
    }
}

/// Choose the program for the clipboard of Linux.
///
/// The display server comes first: a Wayland session uses wl-clipboard, and
/// an X11 session uses xclip or xsel. A Wayland session that has only xclip
/// uses it through XWayland.
pub fn linux(wayland: bool, x11: bool, has: &dyn Fn(&str) -> bool) -> Result<Linux> {
    if wayland && has("wl-copy") && has("wl-paste") {
        return Ok(Linux::Wayland);
    }
    if x11 || wayland {
        if has("xclip") {
            return Ok(Linux::Xclip);
        }
        if has("xsel") {
            return Ok(Linux::Xsel);
        }
    }
    match (wayland, x11) {
        (true, _) => {
            bail!("this Wayland session has no clipboard program. Install the package wl-clipboard")
        }
        (false, true) => {
            bail!("this X11 session has no clipboard program. Install the package xclip")
        }
        (false, false) => bail!(
            "no desktop session is open, so there is no clipboard. \
             WAYLAND_DISPLAY and DISPLAY are both empty"
        ),
    }
}

/// Text for the clipboard of Windows: UTF-16 with a null at the end.
pub fn utf16(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Text from the clipboard of Windows, up to the first null.
pub fn from_utf16(units: &[u16]) -> String {
    let end = units.iter().position(|u| *u == 0).unwrap_or(units.len());
    String::from_utf16_lossy(&units[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn only(names: &'static [&'static str]) -> impl Fn(&str) -> bool {
        move |name| names.contains(&name)
    }

    #[test]
    fn wayland_uses_wl_clipboard() {
        let has = only(&["wl-copy", "wl-paste", "xclip"]);
        assert_eq!(linux(true, true, &has).unwrap(), Linux::Wayland);
    }

    #[test]
    fn wayland_without_wl_clipboard_uses_xclip() {
        let has = only(&["xclip"]);
        assert_eq!(linux(true, true, &has).unwrap(), Linux::Xclip);
    }

    #[test]
    fn x11_prefers_xclip_to_xsel() {
        assert_eq!(
            linux(false, true, &only(&["xclip", "xsel"])).unwrap(),
            Linux::Xclip
        );
        assert_eq!(linux(false, true, &only(&["xsel"])).unwrap(), Linux::Xsel);
    }

    #[test]
    fn x11_never_uses_wl_clipboard() {
        let error = linux(false, true, &only(&["wl-copy", "wl-paste"])).unwrap_err();
        assert!(error.to_string().contains("xclip"), "{error}");
    }

    #[test]
    fn the_error_names_the_package_to_install() {
        let error = linux(true, false, &only(&[])).unwrap_err();
        assert!(error.to_string().contains("wl-clipboard"), "{error}");
        let error = linux(false, false, &only(&["xclip"])).unwrap_err();
        assert!(error.to_string().contains("no desktop session"), "{error}");
    }

    #[test]
    fn utf16_round_trips_and_ends_with_a_null() {
        let text = "caf\u{e9} \u{1f600}\r\nline";
        let units = utf16(text);
        assert_eq!(units.last(), Some(&0));
        assert_eq!(from_utf16(&units), text);
    }

    #[test]
    fn from_utf16_stops_at_the_first_null() {
        assert_eq!(from_utf16(&[104, 105, 0, 120]), "hi");
        assert_eq!(from_utf16(&[104, 105]), "hi");
    }
}
