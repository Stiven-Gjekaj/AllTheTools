//! The scans of `att surface`. Each one works on [`Blocks`], so a test gives
//! it a drive in memory with bad sectors, and no test opens a real drive.

use std::io;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::rawdev::{Aligned, Blocks};

/// The most bad sectors that the report names one by one. It counts the
/// others.
pub const NAMED: usize = 100;

/// What a scan found.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Report {
    pub size: u64,
    pub sector: u32,
    /// The bytes that the scan read, or wrote and read back.
    pub checked: u64,
    /// The offset in bytes of each bad sector, up to [`NAMED`].
    pub bad: Vec<u64>,
    pub bad_count: u64,
    /// The offset of each chunk that took longer than the slow limit, and
    /// its time in milliseconds.
    pub slow: Vec<(u64, u64)>,
    pub seconds: f64,
}

impl Report {
    fn bad_sector(&mut self, offset: u64) {
        self.bad_count += 1;
        if self.bad.len() < NAMED {
            self.bad.push(offset);
        }
    }
}

/// The settings of a scan.
pub struct Options {
    /// The bytes of one read or write. A multiple of the sector.
    pub chunk: usize,
    /// A chunk that takes longer than this is slow.
    pub slow: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            chunk: 4 << 20,
            slow: Duration::from_secs(1),
        }
    }
}

/// The part of a drive at `offset`, as long as one chunk or the rest of the
/// drive.
fn span(size: u64, offset: u64, chunk: usize) -> usize {
    (size - offset).min(chunk as u64) as usize
}

/// Read each sector of the drive one time. A chunk that fails is read again
/// sector by sector, so the report names each bad sector and not the chunk.
pub fn read(
    drive: &mut dyn Blocks,
    options: &Options,
    progress: &mut dyn FnMut(u64, u64),
) -> Report {
    let started = Instant::now();
    let sector = drive.sector();
    let size = drive.size() / u64::from(sector) * u64::from(sector);
    let chunk = options.chunk / sector as usize * sector as usize;
    let mut report = Report {
        size,
        sector,
        ..Report::default()
    };
    let mut buffer = Aligned::new(chunk);
    let mut offset = 0;
    while offset < size {
        let len = span(size, offset, chunk);
        let begin = Instant::now();
        if drive.read_at(offset, &mut buffer[..len]).is_err() {
            sectors(drive, offset, len, &mut buffer, &mut report);
        }
        note_slow(&mut report, offset, begin.elapsed(), options);
        offset += len as u64;
        report.checked = offset;
        progress(offset, size);
    }
    report.seconds = started.elapsed().as_secs_f64();
    report
}

fn sectors(
    drive: &mut dyn Blocks,
    offset: u64,
    len: usize,
    buffer: &mut Aligned,
    report: &mut Report,
) {
    let sector = drive.sector() as usize;
    for at in (0..len).step_by(sector) {
        let place = offset + at as u64;
        if drive.read_at(place, &mut buffer[at..at + sector]).is_err() {
            report.bad_sector(place);
        }
    }
}

fn note_slow(report: &mut Report, offset: u64, time: Duration, options: &Options) {
    if time > options.slow && report.slow.len() < NAMED {
        report.slow.push((offset, time.as_millis() as u64));
    }
}

/// Fill a buffer with the pattern of the bytes at `offset`. The pattern
/// comes from the seed and the offset only, so the check that reads the
/// drive back makes the same bytes again with no copy kept.
pub fn pattern(seed: u64, offset: u64, buffer: &mut [u8]) {
    for (i, word) in buffer.chunks_mut(8).enumerate() {
        // splitmix64 of the seed and the place of each word.
        let mut z = seed ^ (offset + i as u64 * 8).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        let bytes = z.to_le_bytes();
        word.copy_from_slice(&bytes[..word.len()]);
    }
}

/// Write a pattern to each sector of the drive, then read each sector back
/// and compare it. This erases the drive. A sector that fails the write, the
/// read or the compare is bad.
pub fn write(
    drive: &mut dyn Blocks,
    options: &Options,
    seed: u64,
    progress: &mut dyn FnMut(u64, u64),
) -> io::Result<Report> {
    let started = Instant::now();
    let sector = drive.sector();
    let size = drive.size() / u64::from(sector) * u64::from(sector);
    let chunk = options.chunk / sector as usize * sector as usize;
    let sector = sector as usize;
    let mut report = Report {
        size,
        sector: sector as u32,
        ..Report::default()
    };
    let mut buffer = Aligned::new(chunk);
    let mut back = Aligned::new(chunk);
    let mut failed_write: Vec<u64> = Vec::new();

    // The total is twice the size: one pass to write and one to read.
    let mut offset = 0;
    while offset < size {
        let len = span(size, offset, chunk);
        pattern(seed, offset, &mut buffer[..len]);
        let begin = Instant::now();
        if drive.write_at(offset, &buffer[..len]).is_err() {
            for at in (0..len).step_by(sector) {
                let place = offset + at as u64;
                if drive.write_at(place, &buffer[at..at + sector]).is_err() {
                    failed_write.push(place);
                }
            }
        }
        note_slow(&mut report, offset, begin.elapsed(), options);
        offset += len as u64;
        progress(offset, size * 2);
    }
    drive.sync()?;

    let mut offset = 0;
    while offset < size {
        let len = span(size, offset, chunk);
        pattern(seed, offset, &mut buffer[..len]);
        let begin = Instant::now();
        let whole = drive.read_at(offset, &mut back[..len]).is_ok();
        for at in (0..len).step_by(sector) {
            let place = offset + at as u64;
            let ok = if whole {
                back[at..at + sector] == buffer[at..at + sector]
            } else {
                drive.read_at(place, &mut back[at..at + sector]).is_ok()
                    && back[at..at + sector] == buffer[at..at + sector]
            };
            if !ok || failed_write.contains(&place) {
                report.bad_sector(place);
            }
        }
        note_slow(&mut report, offset, begin.elapsed(), options);
        offset += len as u64;
        report.checked = offset;
        progress(size + offset, size * 2);
    }
    report.seconds = started.elapsed().as_secs_f64();
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rawdev::blocks::Memory;

    fn small() -> Options {
        Options {
            chunk: 4096,
            slow: Duration::from_secs(60),
        }
    }

    #[test]
    fn a_good_drive_has_no_bad_sector() {
        let mut drive = Memory::honest(64 * 1024);
        let mut calls = 0;
        let report = read(&mut drive, &small(), &mut |_, _| calls += 1);
        assert_eq!(report.checked, 64 * 1024);
        assert_eq!(report.bad_count, 0);
        assert_eq!(calls, 16, "one progress call for each chunk");
    }

    #[test]
    fn a_read_names_each_bad_sector_and_not_the_chunk() {
        let mut drive = Memory::honest(64 * 1024);
        drive.bad = vec![9, 10, 100];
        let report = read(&mut drive, &small(), &mut |_, _| {});
        assert_eq!(report.bad, [9 * 512, 10 * 512, 100 * 512]);
        assert_eq!(report.bad_count, 3);
        assert_eq!(
            report.checked,
            64 * 1024,
            "the scan goes on after a bad sector"
        );
    }

    #[test]
    fn the_last_chunk_can_be_short() {
        let mut drive = Memory::honest(4096 * 3 + 512);
        let report = read(&mut drive, &small(), &mut |_, _| {});
        assert_eq!(report.checked, 4096 * 3 + 512);
    }

    #[test]
    fn a_write_scan_finds_the_sectors_that_a_fake_drive_drops() {
        // The drive says 64 KiB and keeps 48 KiB, so the last 32 sectors
        // read back as zeros.
        let mut drive = Memory::fake(48 * 1024, 64 * 1024, false);
        let report = write(&mut drive, &small(), 7, &mut |_, _| {}).unwrap();
        assert_eq!(report.bad_count, 32);
        assert_eq!(report.bad[0], 48 * 1024);
    }

    #[test]
    fn a_write_scan_finds_a_drive_that_wraps() {
        // The writes past 32 KiB land on the first 32 KiB, so the first
        // half reads back the pattern of the second half.
        let mut drive = Memory::fake(32 * 1024, 64 * 1024, true);
        let report = write(&mut drive, &small(), 7, &mut |_, _| {}).unwrap();
        assert_eq!(
            report.bad_count, 64,
            "each sector of the first half is wrong"
        );
    }

    #[test]
    fn a_write_scan_of_a_good_drive_passes() {
        let mut drive = Memory::honest(64 * 1024);
        let report = write(&mut drive, &small(), 7, &mut |_, _| {}).unwrap();
        assert_eq!(report.bad_count, 0);
        assert_eq!(report.checked, 64 * 1024);
    }

    #[test]
    fn the_pattern_depends_on_the_offset_and_the_seed() {
        let mut a = [0u8; 64];
        let mut b = [0u8; 64];
        pattern(1, 0, &mut a);
        pattern(1, 512, &mut b);
        assert_ne!(a, b);
        pattern(2, 0, &mut b);
        assert_ne!(a, b);
        pattern(1, 0, &mut b);
        assert_eq!(a, b);
    }
}
