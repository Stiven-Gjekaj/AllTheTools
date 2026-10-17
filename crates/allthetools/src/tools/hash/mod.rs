//! `att hash`: give the checksum of files, and check a list of checksums.
//!
//! Linux has `sha256sum` and its family, and macOS has `shasum`. Windows has
//! `certutil -hashfile` and `Get-FileHash`, which take one file and cannot
//! check the `SHA256SUMS` file of a download. This tool writes the form of
//! `sha256sum`, and `-c` checks a list in the form of GNU or of BSD.

pub mod check;
pub mod digest;
