//! `att notify`: show a notification on the desktop.
//!
//! Linux has `notify-send`. macOS shows a notification only through an
//! AppleScript, and Windows has no command for it. So a script that tells
//! the person that a long job is done needs a different line on each host.
//! This tool is the same line on each one:
//!
//! ```text
//! make && att notify "The build is done"
//! ```

pub mod parse;
