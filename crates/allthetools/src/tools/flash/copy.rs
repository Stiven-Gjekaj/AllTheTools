//! The write of an image to a drive, and the check that reads it back.
//!
//! Each step works on [`Blocks`], so a test gives it a drive in memory, and
//! no test opens a real drive.

use std::io::{self, Read};
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use crate::rawdev::{Aligned, Blocks};

/// The bytes of one read or write.
pub const CHUNK: usize = 4 << 20;

/// The kind of a compressed file, from its first bytes. A drive needs the
/// bytes of the image itself, and not the bytes of an archive of it.
pub fn compressed(magic: &[u8]) -> Option<&'static str> {
    const KINDS: [(&[u8], &str); 5] = [
        (&[0x1f, 0x8b], "gzip"),
        (&[0xfd, b'7', b'z', b'X', b'Z', 0x00], "xz"),
        (&[0x28, 0xb5, 0x2f, 0xfd], "zstd"),
        (&[b'P', b'K', 0x03, 0x04], "zip"),
        (b"BZh", "bzip2"),
    ];
    KINDS
        .iter()
        .find(|(start, _)| magic.starts_with(start))
        .map(|(_, kind)| *kind)
}

/// True when the image is on a volume of the drive, at or below one of its
/// mount points. A write to that drive would overwrite the image while it is
/// read.
pub fn inside(image: &Path, mounts: &[PathBuf]) -> bool {
    mounts.iter().any(|mount| image.starts_with(mount))
}

/// Read up to `buffer.len()` bytes. A short count is the end of the image.
fn fill(input: &mut dyn Read, buffer: &mut [u8]) -> io::Result<usize> {
    let mut done = 0;
    while done < buffer.len() {
        match input.read(&mut buffer[done..]) {
            Ok(0) => break,
            Ok(n) => done += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(done)
}

/// Write the image to the start of the drive. The last chunk gets zeros up
/// to a whole sector, because a drive takes whole sectors only. It gives the
/// bytes that it wrote to the drive.
pub fn write(
    image: &mut dyn Read,
    size: u64,
    drive: &mut dyn Blocks,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<u64> {
    if size > drive.size() {
        bail!(
            "the image has {} bytes, and the drive has {}. It does not fit",
            crate::size::grouped(size),
            crate::size::grouped(drive.size())
        );
    }
    let sector = drive.sector() as usize;
    let mut buffer = Aligned::new(CHUNK);
    let mut offset: u64 = 0;
    loop {
        let n = fill(image, &mut buffer)?;
        if n == 0 {
            break;
        }
        let whole = n.div_ceil(sector) * sector;
        buffer[n..whole].fill(0);
        drive.write_at(offset, &buffer[..whole]).map_err(|e| {
            anyhow::anyhow!(
                "the write at byte {} failed: {e}",
                crate::size::grouped(offset)
            )
        })?;
        offset += n as u64;
        progress(offset, size);
        if n < CHUNK {
            break;
        }
    }
    if offset != size {
        bail!(
            "the image gave {} bytes, and its size is {}. It changed during the write",
            crate::size::grouped(offset),
            crate::size::grouped(size)
        );
    }
    drive.sync()?;
    Ok(offset)
}

/// Read the drive back and compare it with the image. It gives the offset of
/// the first byte that is not the same, or None when each byte is the same.
pub fn verify(
    image: &mut dyn Read,
    size: u64,
    drive: &mut dyn Blocks,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<Option<u64>> {
    let sector = drive.sector() as usize;
    let mut expected = vec![0u8; CHUNK];
    let mut back = Aligned::new(CHUNK);
    let mut offset: u64 = 0;
    while offset < size {
        let n = fill(image, &mut expected)?;
        if n == 0 {
            break;
        }
        let whole = n.div_ceil(sector) * sector;
        drive.read_at(offset, &mut back[..whole]).map_err(|e| {
            anyhow::anyhow!(
                "the read at byte {} failed: {e}",
                crate::size::grouped(offset)
            )
        })?;
        if let Some(i) = (0..n).find(|&i| back[i] != expected[i]) {
            return Ok(Some(offset + i as u64));
        }
        offset += n as u64;
        progress(offset, size);
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rawdev::blocks::Memory;

    // Bytes with no short period, so a drive that wraps cannot hold the
    // same bytes at two places by chance.
    fn image(len: usize) -> Vec<u8> {
        (0..len as u64)
            .map(|i| (i.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 56) as u8)
            .collect()
    }

    #[test]
    fn an_image_lands_at_the_start_and_reads_back_the_same() {
        let data = image(CHUNK + 1000);
        let mut drive = Memory::honest(3 * CHUNK as u64);
        let written = write(
            &mut &data[..],
            data.len() as u64,
            &mut drive,
            &mut |_, _| {},
        )
        .unwrap();
        assert_eq!(written, data.len() as u64);
        assert_eq!(&drive.data[..data.len()], &data[..]);
        let found = verify(
            &mut &data[..],
            data.len() as u64,
            &mut drive,
            &mut |_, _| {},
        )
        .unwrap();
        assert_eq!(found, None);
    }

    #[test]
    fn the_last_sector_gets_zeros_after_the_image() {
        let data = image(1000);
        let mut drive = Memory::honest(4096);
        write(&mut &data[..], 1000, &mut drive, &mut |_, _| {}).unwrap();
        assert!(drive.data[1000..1024].iter().all(|b| *b == 0));
        // The sector after the image keeps what it held.
        assert_eq!(drive.data[1024], (1024 % 251) as u8);
    }

    #[test]
    fn an_image_larger_than_the_drive_is_refused_before_a_write() {
        let data = image(8192);
        let mut drive = Memory::honest(4096);
        let before = drive.data.clone();
        assert!(write(&mut &data[..], 8192, &mut drive, &mut |_, _| {}).is_err());
        assert_eq!(drive.data, before);
    }

    #[test]
    fn a_fake_drive_fails_the_check_at_its_real_size() {
        // The drive says 2 MiB, and it keeps 1 MiB. The write of the second
        // MiB goes over the first one.
        let data = image(2 << 20);
        let mut drive = Memory::fake(1 << 20, 2 << 20, true);
        write(
            &mut &data[..],
            data.len() as u64,
            &mut drive,
            &mut |_, _| {},
        )
        .unwrap();
        let found = verify(
            &mut &data[..],
            data.len() as u64,
            &mut drive,
            &mut |_, _| {},
        )
        .unwrap();
        assert!(found.is_some(), "the check found no difference");
        assert!(found.unwrap() < 1 << 20);
    }

    #[test]
    fn an_image_that_changes_size_is_an_error() {
        let data = image(1000);
        let mut drive = Memory::honest(4096);
        assert!(write(&mut &data[..], 2000, &mut drive, &mut |_, _| {}).is_err());
    }

    #[test]
    fn archives_are_named_by_their_first_bytes() {
        assert_eq!(compressed(&[0x1f, 0x8b, 8, 0]), Some("gzip"));
        assert_eq!(compressed(b"\xfd7zXZ\x00rest"), Some("xz"));
        assert_eq!(compressed(b"PK\x03\x04"), Some("zip"));
        assert_eq!(compressed(&[0x28, 0xb5, 0x2f, 0xfd]), Some("zstd"));
        assert_eq!(compressed(b"BZh9"), Some("bzip2"));
        assert_eq!(compressed(&[0u8; 16]), None);
        assert_eq!(compressed(b"\x33\xed\x90\x90"), None);
    }

    #[test]
    fn an_image_on_the_drive_itself_is_found() {
        let mounts = vec![PathBuf::from("/Volumes/STICK")];
        assert!(inside(Path::new("/Volumes/STICK/ubuntu.iso"), &mounts));
        assert!(!inside(Path::new("/Users/me/ubuntu.iso"), &mounts));
        assert!(!inside(Path::new("/Volumes/STICK2/a.iso"), &mounts));
    }
}
