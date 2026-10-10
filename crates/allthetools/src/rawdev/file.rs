//! A regular file that acts as a drive, so `att surface` and `att probe`
//! can test an image, and so CI can run them with no drive to spare.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

use super::blocks::Blocks;

pub struct FileBlocks {
    file: File,
    size: u64,
}

impl FileBlocks {
    pub fn open(path: &Path, write: bool) -> io::Result<Self> {
        let file = OpenOptions::new().read(true).write(write).open(path)?;
        let size = file.metadata()?.len();
        Ok(FileBlocks { file, size })
    }
}

impl Blocks for FileBlocks {
    fn size(&self) -> u64 {
        // A file acts as a drive of whole sectors.
        self.size / 512 * 512
    }

    fn sector(&self) -> u32 {
        512
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

#[cfg(unix)]
pub fn read_exact_at(file: &File, offset: u64, buffer: &mut [u8]) -> io::Result<()> {
    std::os::unix::fs::FileExt::read_exact_at(file, buffer, offset)
}

#[cfg(unix)]
pub fn write_all_at(file: &File, offset: u64, buffer: &[u8]) -> io::Result<()> {
    std::os::unix::fs::FileExt::write_all_at(file, buffer, offset)
}

#[cfg(windows)]
pub fn read_exact_at(file: &File, mut offset: u64, mut buffer: &mut [u8]) -> io::Result<()> {
    use std::os::windows::fs::FileExt;
    while !buffer.is_empty() {
        match file.seek_read(buffer, offset)? {
            0 => return Err(io::ErrorKind::UnexpectedEof.into()),
            n => {
                buffer = &mut buffer[n..];
                offset += n as u64;
            }
        }
    }
    Ok(())
}

#[cfg(windows)]
pub fn write_all_at(file: &File, mut offset: u64, mut buffer: &[u8]) -> io::Result<()> {
    use std::os::windows::fs::FileExt;
    while !buffer.is_empty() {
        match file.seek_write(buffer, offset)? {
            0 => return Err(io::ErrorKind::WriteZero.into()),
            n => {
                buffer = &buffer[n..];
                offset += n as u64;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // The test makes its own file in the directory for temporary files, and
    // removes it. It opens no device.
    #[test]
    fn a_file_reads_back_what_it_got() {
        let path = std::env::temp_dir().join(format!("att-rawdev-{}.img", std::process::id()));
        std::fs::write(&path, vec![0u8; 4096 + 100]).unwrap();
        let mut blocks = FileBlocks::open(&path, true).unwrap();
        assert_eq!(blocks.size(), 4096, "the size is whole sectors");
        blocks.write_at(512, &[9u8; 512]).unwrap();
        blocks.sync().unwrap();
        let mut back = [0u8; 1024];
        blocks.read_at(0, &mut back).unwrap();
        assert_eq!(&back[..512], &[0u8; 512]);
        assert_eq!(&back[512..], &[9u8; 512]);
        drop(blocks);
        std::fs::remove_file(&path).unwrap();
    }
}
