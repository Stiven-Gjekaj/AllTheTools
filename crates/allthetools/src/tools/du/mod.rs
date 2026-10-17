//! `att du`: show which files and folders take the most space.
//!
//! macOS and Linux have `du`. Windows has no command that adds up the size
//! of a folder, and Explorer counts one folder at a time. This tool walks
//! each path and lists its largest entries, the largest first.
//!
//! On macOS and Linux a size is the space on the disk, as `du` counts it,
//! and a file with more than one hard link counts one time. Windows gives
//! the length of each file.

pub mod tally;
