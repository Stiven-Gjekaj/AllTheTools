//! `att df`: show the size and the free space of each mounted volume.
//!
//! macOS and Linux have `df`. Windows has no command that lists the volumes
//! with their free space, and `fsutil volume diskfree` takes one volume and
//! needs Administrator. This tool gives the same table on each host.

pub mod parse;
