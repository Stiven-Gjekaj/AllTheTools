//! `att trash`: move files to the trash, where a person can get them back.
//!
//! Linux has `gio trash`, and macOS has `trash` since version 15. Windows
//! has no command that moves a file to the Recycle Bin, and `del` deletes it
//! for good. This tool uses the trash of the host on each one: the Trash of
//! the Finder, the Recycle Bin, and the trash of the freedesktop.org
//! specification, which the file managers of Linux read.

pub mod parse;
