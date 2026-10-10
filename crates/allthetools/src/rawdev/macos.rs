//! The host file of a raw drive on macOS.
//!
//! The raw node /dev/rdiskN skips the buffer cache, so a read after a write
//! reads the medium. Its size comes from two ioctl calls, because a seek to
//! the end of a raw node gives zero.

use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;

use anyhow::{Result, bail};

use super::blocks::Blocks;
use super::file::{read_exact_at, write_all_at};
use crate::disks::Disk;
use crate::host;

/// DKIOCGETBLOCKSIZE: _IOR('d', 24, uint32_t).
const BLOCK_SIZE: libc::c_ulong = 0x4004_6418;
/// DKIOCGETBLOCKCOUNT: _IOR('d', 25, uint64_t).
const BLOCK_COUNT: libc::c_ulong = 0x4008_6419;
/// DKIOCSYNCHRONIZE: _IO('d', 22). It asks the drive to empty its cache.
const SYNCHRONIZE: libc::c_ulong = 0x2000_6416;

pub struct Device {
    file: File,
    size: u64,
    sector: u32,
}

pub fn open(disk: &Disk, write: bool) -> Result<Device> {
    if write {
        host::output("diskutil", &["unmountDisk", &disk.path], None)?;
    }
    let file = OpenOptions::new()
        .read(true)
        .write(write)
        .open(&disk.raw_path)
        .map_err(|e| super::explain(e, &disk.raw_path))?;
    let mut sector = 0u32;
    let mut count = 0u64;
    // SAFETY: each ioctl writes one number of the type that it names, into
    // a variable of that type.
    #[allow(unsafe_code)]
    unsafe {
        if libc::ioctl(file.as_raw_fd(), BLOCK_SIZE, &mut sector) != 0
            || libc::ioctl(file.as_raw_fd(), BLOCK_COUNT, &mut count) != 0
        {
            return Err(io::Error::last_os_error().into());
        }
    }
    if sector == 0 || count == 0 {
        bail!("{} reports a size of zero", disk.path);
    }
    Ok(Device {
        file,
        size: count * u64::from(sector),
        sector,
    })
}

impl Blocks for Device {
    fn size(&self) -> u64 {
        self.size
    }

    fn sector(&self) -> u32 {
        self.sector
    }

    fn read_at(&mut self, offset: u64, buffer: &mut [u8]) -> io::Result<()> {
        read_exact_at(&self.file, offset, buffer)
    }

    fn write_at(&mut self, offset: u64, buffer: &[u8]) -> io::Result<()> {
        write_all_at(&self.file, offset, buffer)
    }

    fn sync(&mut self) -> io::Result<()> {
        // SAFETY: DKIOCSYNCHRONIZE takes no argument. A drive that does not
        // know it gives an error, and the data is on the medium already.
        #[allow(unsafe_code)]
        unsafe {
            libc::ioctl(self.file.as_raw_fd(), SYNCHRONIZE);
        }
        Ok(())
    }
}
