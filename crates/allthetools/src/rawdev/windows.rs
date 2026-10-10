//! The host file of a raw drive on Windows.
//!
//! The rules come from Burnout, which measured each one on a real drive:
//!
//! - Each volume of the drive is locked and then dismounted, and the locks
//!   stay while the drive is open. A lock that is let go lets the file
//!   system mount the volume again during the write.
//! - The drive goes offline while it is open for a write. An online drive
//!   has partitions that Windows applies, and Windows refuses a write into a
//!   partition that it marks read-only, and it rewrites a table soon after
//!   one lands.
//!
//! The handle has no cache of Windows, so a read after a write reads the
//! drive.

use std::ffi::c_void;
use std::fs::File;
use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle};

use anyhow::{Result, bail};
use windows_sys::Win32::Foundation::{
    ERROR_ACCESS_DENIED, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_NO_BUFFERING, FILE_FLAG_WRITE_THROUGH, FILE_SHARE_READ,
    FILE_SHARE_WRITE, FindFirstVolumeW, FindNextVolumeW, FindVolumeClose,
    IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS, OPEN_EXISTING,
};
use windows_sys::Win32::System::IO::DeviceIoControl;
use windows_sys::Win32::System::Ioctl::{
    FSCTL_DISMOUNT_VOLUME, FSCTL_LOCK_VOLUME, IOCTL_DISK_GET_DRIVE_GEOMETRY_EX,
    IOCTL_DISK_SET_DISK_ATTRIBUTES,
};

use super::blocks::Blocks;
use super::file::{read_exact_at, write_all_at};
use crate::disks::Disk;

const DISK_ATTRIBUTE_OFFLINE: u64 = 0x1;
const DISK_ATTRIBUTE_READ_ONLY: u64 = 0x2;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn create(path: &str, access: u32, flags: u32) -> io::Result<File> {
    let path = wide(path);
    // SAFETY: the path ends with a null. The handle is checked, and File
    // owns it after this.
    #[allow(unsafe_code)]
    unsafe {
        let handle = CreateFileW(
            path.as_ptr(),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            flags,
            std::ptr::null_mut(),
        );
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        Ok(File::from_raw_handle(handle as _))
    }
}

fn control(file: &File, code: u32, input: &[u8], want: usize) -> Option<Vec<u8>> {
    let mut out = vec![0u8; want];
    let mut written = 0u32;
    // SAFETY: both buffers belong to this function, and their sizes go with
    // them.
    #[allow(unsafe_code)]
    let ok = unsafe {
        DeviceIoControl(
            file.as_raw_handle() as HANDLE,
            code,
            if input.is_empty() {
                std::ptr::null()
            } else {
                input.as_ptr().cast::<c_void>()
            },
            input.len() as u32,
            if want == 0 {
                std::ptr::null_mut()
            } else {
                out.as_mut_ptr().cast::<c_void>()
            },
            want as u32,
            &mut written,
            std::ptr::null_mut(),
        ) != 0
    };
    if !ok {
        return None;
    }
    out.truncate(written as usize);
    Some(out)
}

/// The volume names whose extents lie on PhysicalDrive<number>, with no
/// backslash at the end, so CreateFileW opens them.
fn volumes_on(number: u32) -> Vec<String> {
    let mut out = Vec::new();
    let mut name = [0u16; 260];
    // SAFETY: the buffer and its length go together. The search closes one
    // time.
    #[allow(unsafe_code)]
    unsafe {
        let search = FindFirstVolumeW(name.as_mut_ptr(), name.len() as u32);
        if search == INVALID_HANDLE_VALUE {
            return out;
        }
        loop {
            let end = name.iter().position(|c| *c == 0).unwrap_or(name.len());
            let path = String::from_utf16_lossy(&name[..end]);
            let path = path.trim_end_matches('\\').to_owned();
            if let Ok(volume) = create(&path, 0, 0)
                && let Some(bytes) = control(
                    &volume,
                    IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS,
                    &[],
                    8 + 24 * 16,
                )
                && super::parse_extents(&bytes).contains(&number)
            {
                out.push(path);
            }
            if FindNextVolumeW(search, name.as_mut_ptr(), name.len() as u32) == 0 {
                break;
            }
        }
        FindVolumeClose(search);
    }
    out
}

pub struct Device {
    file: File,
    size: u64,
    sector: u32,
    /// The locks of the volumes. They do nothing but exist.
    _locks: Vec<File>,
    offline: bool,
}

impl Device {
    fn set_attributes(&self, attributes: u64, mask: u64) -> bool {
        // SET_DISK_ATTRIBUTES: Version 40, Persist 0, Attributes, Mask.
        let mut input = [0u8; 40];
        input[0..4].copy_from_slice(&40u32.to_le_bytes());
        input[8..16].copy_from_slice(&attributes.to_le_bytes());
        input[16..24].copy_from_slice(&mask.to_le_bytes());
        control(&self.file, IOCTL_DISK_SET_DISK_ATTRIBUTES, &input, 0).is_some()
    }
}

impl Drop for Device {
    /// The drive comes online again, so the person sees its volumes. Persist
    /// is zero, so a removal of the drive brings it online too.
    fn drop(&mut self) {
        if self.offline {
            self.set_attributes(0, DISK_ATTRIBUTE_OFFLINE | DISK_ATTRIBUTE_READ_ONLY);
        }
    }
}

pub fn open(disk: &Disk, write: bool) -> Result<Device> {
    let number: u32 = disk.id.parse()?;
    let mut locks = Vec::new();
    if write {
        for path in volumes_on(number) {
            let volume = create(&path, GENERIC_READ | GENERIC_WRITE, 0)
                .map_err(|e| super::explain(e, &path))?;
            if control(&volume, FSCTL_LOCK_VOLUME, &[], 0).is_none() {
                bail!("a program holds a file on a volume of the drive. Close it, and try again");
            }
            control(&volume, FSCTL_DISMOUNT_VOLUME, &[], 0);
            locks.push(volume);
        }
    }
    let access = if write {
        GENERIC_READ | GENERIC_WRITE
    } else {
        GENERIC_READ
    };
    let file = create(
        &disk.raw_path,
        access,
        FILE_FLAG_NO_BUFFERING | FILE_FLAG_WRITE_THROUGH,
    )
    .map_err(|e| super::explain(e, &disk.raw_path))?;

    // DISK_GEOMETRY_EX: the geometry of 24 bytes, with BytesPerSector at
    // byte 20, then DiskSize at byte 24.
    let Some(geometry) = control(&file, IOCTL_DISK_GET_DRIVE_GEOMETRY_EX, &[], 256) else {
        return Err(io::Error::last_os_error().into());
    };
    if geometry.len() < 32 {
        bail!("Windows gave no geometry for {}", disk.raw_path);
    }
    let sector = u32::from_le_bytes(geometry[20..24].try_into()?);
    let size = u64::from_le_bytes(geometry[24..32].try_into()?);

    let mut device = Device {
        file,
        size,
        sector,
        _locks: locks,
        offline: false,
    };
    if write {
        if !device.set_attributes(
            DISK_ATTRIBUTE_OFFLINE,
            DISK_ATTRIBUTE_OFFLINE | DISK_ATTRIBUTE_READ_ONLY,
        ) {
            bail!(
                "Windows did not take the drive offline: {}",
                io::Error::last_os_error()
            );
        }
        device.offline = true;
    }
    Ok(device)
}

/// The error of an open, with what to do about access denied.
pub fn denied(error: &io::Error) -> bool {
    error.raw_os_error() == Some(ERROR_ACCESS_DENIED as i32)
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
