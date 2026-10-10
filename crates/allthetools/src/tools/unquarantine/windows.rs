//! The host file of `att unquarantine` on Windows: the alternate data stream
//! `Zone.Identifier`. `Unblock-File` of PowerShell removes the same stream.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use super::parse::{self, Mark};

fn stream(path: &Path) -> PathBuf {
    let mut name = OsString::from(path.as_os_str());
    name.push(":Zone.Identifier");
    PathBuf::from(name)
}

pub fn read(path: &Path) -> io::Result<Option<Mark>> {
    match std::fs::read(stream(path)) {
        Ok(bytes) => Ok(Some(parse::zone_identifier(&bytes))),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

pub fn remove(path: &Path) -> io::Result<bool> {
    match std::fs::remove_file(stream(path)) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}
