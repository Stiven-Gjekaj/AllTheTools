//! The blocks of a drive, as the disk tools that write see them.
//!
//! `att surface` and `att probe` read and write a drive through the trait
//! [`Blocks`] only. A test gives them a drive in memory, which can be a fake
//! drive on purpose, and no test opens a real drive.
//!
//! This module makes no system call, so its tests run on every host.

use std::io;

/// A drive, or a file that acts as one.
pub trait Blocks {
    /// The size that the drive reports, in bytes.
    fn size(&self) -> u64;
    /// The smallest unit of a read or a write, in bytes.
    fn sector(&self) -> u32;
    /// Read `buffer.len()` bytes at `offset`. Both are multiples of the
    /// sector.
    fn read_at(&mut self, offset: u64, buffer: &mut [u8]) -> io::Result<()>;
    /// Write the buffer at `offset`. Both are multiples of the sector.
    fn write_at(&mut self, offset: u64, buffer: &[u8]) -> io::Result<()>;
    /// Make each write reach the drive.
    fn sync(&mut self) -> io::Result<()>;
}

/// A buffer with an address that is a multiple of 4096. A read or a write
/// with no cache of the host needs such an address on Linux and Windows.
pub struct Aligned {
    pointer: std::ptr::NonNull<u8>,
    len: usize,
}

const ALIGN: usize = 4096;

impl Aligned {
    pub fn new(len: usize) -> Self {
        let size = len.max(1).div_ceil(ALIGN) * ALIGN;
        let layout = std::alloc::Layout::from_size_align(size, ALIGN).expect("a valid layout");
        // SAFETY: the layout has a size above zero.
        #[allow(unsafe_code)]
        let pointer = unsafe { std::alloc::alloc_zeroed(layout) };
        let Some(pointer) = std::ptr::NonNull::new(pointer) else {
            std::alloc::handle_alloc_error(layout);
        };
        Aligned { pointer, len }
    }
}

impl std::ops::Deref for Aligned {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        // SAFETY: the memory holds at least len bytes and lives as long as
        // self.
        #[allow(unsafe_code)]
        unsafe {
            std::slice::from_raw_parts(self.pointer.as_ptr(), self.len)
        }
    }
}

impl std::ops::DerefMut for Aligned {
    fn deref_mut(&mut self) -> &mut [u8] {
        // SAFETY: as in deref, and self is borrowed as mutable.
        #[allow(unsafe_code)]
        unsafe {
            std::slice::from_raw_parts_mut(self.pointer.as_ptr(), self.len)
        }
    }
}

impl Drop for Aligned {
    fn drop(&mut self) {
        let size = self.len.max(1).div_ceil(ALIGN) * ALIGN;
        let layout = std::alloc::Layout::from_size_align(size, ALIGN).expect("a valid layout");
        // SAFETY: the pointer came from alloc_zeroed with this layout.
        #[allow(unsafe_code)]
        unsafe {
            std::alloc::dealloc(self.pointer.as_ptr(), layout)
        }
    }
}

/// A drive in memory, for the tests of the tools that write. It can act as
/// three kinds of drive:
///
/// - an honest drive, which keeps each byte;
/// - a drive that wraps: a write past its real size lands at the offset
///   modulo the real size, over data that it already holds;
/// - a drive that drops: a write past its real size goes nowhere, and a read
///   there gives zeros.
///
/// A sector can also be bad, and then each read of it fails.
#[cfg(test)]
pub struct Memory {
    pub data: Vec<u8>,
    pub announced: u64,
    pub sector: u32,
    pub wraps: bool,
    pub bad: Vec<u64>,
}

#[cfg(test)]
impl Memory {
    pub fn honest(size: u64) -> Self {
        Memory {
            data: (0..size).map(|i| (i % 251) as u8).collect(),
            announced: size,
            sector: 512,
            wraps: false,
            bad: Vec::new(),
        }
    }

    pub fn fake(real: u64, announced: u64, wraps: bool) -> Self {
        Memory {
            announced,
            wraps,
            ..Memory::honest(real)
        }
    }

    fn real(&self) -> u64 {
        self.data.len() as u64
    }
}

#[cfg(test)]
impl Blocks for Memory {
    fn size(&self) -> u64 {
        self.announced
    }

    fn sector(&self) -> u32 {
        self.sector
    }

    fn read_at(&mut self, offset: u64, buffer: &mut [u8]) -> io::Result<()> {
        let sector = u64::from(self.sector);
        for (i, chunk) in buffer.chunks_mut(self.sector as usize).enumerate() {
            let at = offset + i as u64 * sector;
            if self.bad.contains(&(at / sector)) {
                return Err(io::Error::other("bad sector"));
            }
            let real = self.real();
            if at >= real && !self.wraps {
                chunk.fill(0);
                continue;
            }
            let at = (at % real) as usize;
            chunk.copy_from_slice(&self.data[at..at + chunk.len()]);
        }
        Ok(())
    }

    fn write_at(&mut self, offset: u64, buffer: &[u8]) -> io::Result<()> {
        let sector = u64::from(self.sector);
        for (i, chunk) in buffer.chunks(self.sector as usize).enumerate() {
            let at = offset + i as u64 * sector;
            let real = self.real();
            if at >= real && !self.wraps {
                continue;
            }
            let at = (at % real) as usize;
            self.data[at..at + chunk.len()].copy_from_slice(chunk);
        }
        Ok(())
    }

    fn sync(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligned_buffers_start_at_a_multiple_of_4096() {
        for len in [1, 512, 4096, 1 << 20] {
            let buffer = Aligned::new(len);
            assert_eq!(buffer.as_ptr() as usize % 4096, 0);
            assert_eq!(buffer.len(), len);
            assert!(buffer.iter().all(|b| *b == 0));
        }
    }

    #[test]
    fn a_drive_that_wraps_writes_over_its_start() {
        let mut drive = Memory::fake(4096, 8192, true);
        drive.write_at(4096, &[7u8; 512]).unwrap();
        let mut back = [0u8; 512];
        drive.read_at(0, &mut back).unwrap();
        assert_eq!(back, [7u8; 512]);
    }

    #[test]
    fn a_drive_that_drops_gives_zeros_past_its_real_size() {
        let mut drive = Memory::fake(4096, 8192, false);
        drive.write_at(4096, &[7u8; 512]).unwrap();
        let mut back = [1u8; 512];
        drive.read_at(4096, &mut back).unwrap();
        assert_eq!(back, [0u8; 512]);
    }

    #[test]
    fn a_bad_sector_fails_each_read() {
        let mut drive = Memory::honest(4096);
        drive.bad.push(2);
        let mut back = [0u8; 1024];
        assert!(drive.read_at(1024, &mut back).is_err());
        assert!(drive.read_at(0, &mut back).is_ok());
    }
}
