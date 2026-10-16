//! `att flash`: write a disk image, such as the ISO of a Linux installer,
//! to a USB drive, and read it back to check each byte.
//!
//! Linux and macOS have `dd`, which writes to any drive that a person names,
//! the system drive too, and checks nothing. Windows has no command for it.
//! This tool takes the drive by the name that `att lsblk` gives, refuses the
//! system drive, asks the person to type the name, and reads the drive back
//! after the write.

pub mod copy;
