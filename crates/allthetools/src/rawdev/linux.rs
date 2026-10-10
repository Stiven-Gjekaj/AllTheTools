//! The host file of a raw drive on Linux.
//!
//! O_DIRECT reads and writes the medium and not the page cache, so a read
//! after a write reads the drive. O_EXCL on a block device asks the kernel
//! for the device alone, and the kernel refuses it while a file system
//! holds the drive.

use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io;
use std::os::unix::fs::OpenOptionsExt;

use anyhow::{Context, Result, bail};

use super::blocks::Blocks;
use super::file::{read_exact_at, write_all_at};
use crate::disks::Disk;

pub struct Device {
    file: File,
    size: u64,
    sector: u32,
}

fn sysfs(disk: &Disk, name: &str) -> Option<u64> {
    std::fs::read_to_string(format!("/sys/block/{}/{name}", disk.id))
        .ok()?
        .trim()
        .parse()
        .ok()
}

pub fn open(disk: &Disk, write: bool) -> Result<Device> {
    if write {
        for part in &disk.partitions {
            if let Some(point) = &part.mount {
                let path = CString::new(point.as_str())?;
                // SAFETY: the path ends with a null.
                #[allow(unsafe_code)]
                if unsafe { libc::umount2(path.as_ptr(), 0) } != 0 {
                    return Err(io::Error::last_os_error())
                        .with_context(|| format!("could not unmount {point}"));
                }
            }
        }
    }
    let mut flags = libc::O_DIRECT;
    if write {
        flags |= libc::O_EXCL;
    }
    let file = OpenOptions::new()
        .read(true)
        .write(write)
        .custom_flags(flags)
        .open(&disk.raw_path)
        .map_err(|e| super::explain(e, &disk.raw_path))?;
    let sector = sysfs(disk, "queue/logical_block_size").unwrap_or(512) as u32;
    let size = sysfs(disk, "size").map(|s| s * 512).unwrap_or(disk.size);
    if size == 0 {
        bail!("{} reports a size of zero", disk.path);
    }
    Ok(Device { file, size, sector })
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
        self.file.sync_all()
    }
}
