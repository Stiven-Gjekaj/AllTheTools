//! `att hexdump`: show the bytes of a file, as `xxd` does.
//!
//! macOS and Linux have `xxd`, and Windows has no command that shows the
//! bytes of a file. `Format-Hex` of PowerShell has other output and no way
//! back. This tool takes the options of `xxd` and writes the same text, and
//! `-r` turns that text back into bytes.

pub mod format;
