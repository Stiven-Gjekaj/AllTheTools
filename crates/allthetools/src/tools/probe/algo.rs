//! The test of `att probe`: find how much of a drive keeps its data.
//!
//! A fake drive reports a size that it does not have. Its controller takes a
//! write past the real size and either drops it, or wraps it onto a lower
//! address, over data that the drive already holds. Most fakes wrap at a
//! power of two, because the controller ignores the high bits of the
//! address. This test finds both kinds, as f3probe of Linux does:
//!
//! 1. It reads the samples: about 256 blocks across the drive, at a step
//!    that is a power of two, and each power of two below the step. It keeps
//!    a copy of each one. Each place that a sample can wrap to is a sample
//!    too, so a drive that wraps at a power of two puts each marker on a
//!    block that has a copy.
//! 2. It writes a marker to each sample, from the highest to the lowest,
//!    and reads each one back. A sample that does not give its own marker
//!    is past the real size. A drive that wraps puts the high markers onto
//!    low blocks, and the low markers come last, so the high reads give
//!    the low markers.
//! 3. It tests the last block of the drive by itself, as step 3 below tests
//!    a block, because the last block is not on the step.
//! 4. It searches between the last good sample and the first bad one. Each
//!    test writes a marker to one block, and then a marker to each place
//!    that the block can wrap to: the block modulo each power of two. The
//!    block is good only when its own marker is still there.
//! 5. It writes each copy back, from the highest block to the lowest, so
//!    the real blocks get the last write.
//!
//! A drive that wraps at a size that is no power of two can put a marker on
//! a block that is not a sample. That block loses its data. The test can
//! then miss the fake, or give a size that is too large: the real size is
//! at most the size that it gives. A drive with a large cache can hide a
//! part of the fake. f3probe has the same two limits. `att surface --write`
//! reads back each sector, and it has neither of them.
//!
//! The test works on [`Blocks`], so a test of this file gives it fake
//! drives in memory, and no test opens a real drive.

use std::collections::BTreeMap;
use std::io;

use serde::Serialize;

use crate::rawdev::{Aligned, Blocks};

/// The size of one block of the test. A sector of 4096 bytes is the largest
/// common one, so each block is whole sectors on each drive.
pub const BLOCK: u64 = 4096;

/// What the test found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Outcome {
    /// The size that the drive reports.
    pub announced: u64,
    /// The bytes from the start of the drive that keep their data. It is
    /// equal to `announced` for a drive that passed.
    pub usable: u64,
    /// The blocks that the test wrote and wrote back.
    pub blocks_written: usize,
    /// The blocks that did not read back their copy after the restore. Each
    /// one is past the usable size, or it is a block that a read refused.
    pub not_restored: usize,
}

impl Outcome {
    pub fn fake(&self) -> bool {
        self.usable < self.announced
    }
}

fn marker(nonce: u64, block: u64, buffer: &mut [u8]) {
    for (i, word) in buffer.chunks_mut(8).enumerate() {
        let value = match i {
            0 => u64::from_le_bytes(*b"ATTPROBE"),
            1 => nonce,
            2 => block,
            _ => {
                let mut z = nonce ^ block.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (i as u64);
                z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
                z ^ (z >> 31)
            }
        };
        word.copy_from_slice(&value.to_le_bytes()[..word.len()]);
    }
}

/// The blocks that the first pass writes: each multiple of a step that is a
/// power of two, with about 256 steps across the drive, and each power of
/// two below the step.
///
/// A block modulo a power of two is then a sample again: a multiple of the
/// step stays a multiple of the step, or it becomes 0.
pub fn samples(blocks: u64) -> Vec<u64> {
    let mut step = 1;
    while step * 2 <= (blocks / 256).max(1) {
        step *= 2;
    }
    let mut out: Vec<u64> = (0..blocks).step_by(step as usize).collect();
    let mut power = 1;
    while power < step {
        out.push(power);
        power *= 2;
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// The places where a block can wrap to: the block modulo each power of two
/// below it.
pub fn wrap_places(block: u64) -> Vec<u64> {
    let mut out = Vec::new();
    let mut power = 1;
    while power <= block {
        let place = block % power;
        if place != block {
            out.push(place);
        }
        power *= 2;
    }
    out.sort_unstable();
    out.dedup();
    out
}

struct Run<'a> {
    drive: &'a mut dyn Blocks,
    nonce: u64,
    /// The copy of each block that the test wrote, from before its first
    /// write.
    copies: BTreeMap<u64, Vec<u8>>,
    buffer: Aligned,
}

impl Run<'_> {
    fn offset(block: u64) -> u64 {
        block * BLOCK
    }

    /// Keep a copy of the block, the first time only. A block that a read
    /// refuses has no copy, and the test does not write it.
    fn keep(&mut self, block: u64) -> bool {
        if self.copies.contains_key(&block) {
            return true;
        }
        if self
            .drive
            .read_at(Self::offset(block), &mut self.buffer)
            .is_err()
        {
            return false;
        }
        self.copies.insert(block, self.buffer.to_vec());
        true
    }

    fn write_marker(&mut self, block: u64) -> io::Result<()> {
        marker(self.nonce, block, &mut self.buffer);
        self.drive.write_at(Self::offset(block), &self.buffer)
    }

    fn holds_marker(&mut self, block: u64) -> bool {
        let mut expected = vec![0u8; BLOCK as usize];
        marker(self.nonce, block, &mut expected);
        self.drive
            .read_at(Self::offset(block), &mut self.buffer)
            .is_ok()
            && self.buffer[..] == expected[..]
    }

    /// True when the block keeps its own data, with each place that it can
    /// wrap to written after it.
    fn good(&mut self, block: u64) -> io::Result<bool> {
        let places = wrap_places(block);
        if !self.keep(block) || !places.iter().all(|p| self.keep(*p)) {
            return Ok(false);
        }
        self.write_marker(block)?;
        for place in &places {
            self.write_marker(*place)?;
        }
        self.drive.sync()?;
        Ok(self.holds_marker(block))
    }

    /// Write each copy back, from the highest block to the lowest, and count
    /// the blocks under the usable size that do not read back their copy.
    fn restore(&mut self, usable_blocks: u64) -> io::Result<usize> {
        let copies = std::mem::take(&mut self.copies);
        for (block, copy) in copies.iter().rev() {
            self.buffer.copy_from_slice(copy);
            self.drive.write_at(Self::offset(*block), &self.buffer)?;
        }
        self.drive.sync()?;
        let mut wrong = 0;
        for (block, copy) in &copies {
            if *block >= usable_blocks {
                continue;
            }
            let ok = self
                .drive
                .read_at(Self::offset(*block), &mut self.buffer)
                .is_ok()
                && self.buffer[..] == copy[..];
            if !ok {
                wrong += 1;
            }
        }
        self.copies = copies;
        Ok(wrong)
    }
}

/// Run the test. `nonce` makes the markers of this run differ from any
/// marker that an earlier run left on the drive.
pub fn probe(
    drive: &mut dyn Blocks,
    nonce: u64,
    progress: &mut dyn FnMut(&str),
) -> io::Result<Outcome> {
    let announced = drive.size() / BLOCK * BLOCK;
    let blocks = announced / BLOCK;
    if blocks == 0 {
        return Err(io::Error::other(
            "the drive is smaller than one block of 4096 bytes",
        ));
    }
    let mut run = Run {
        drive,
        nonce,
        copies: BTreeMap::new(),
        buffer: Aligned::new(BLOCK as usize),
    };

    progress("read the samples");
    let samples = samples(blocks);
    let mut usable: Vec<u64> = Vec::new();
    for block in &samples {
        if run.keep(*block) {
            usable.push(*block);
        }
    }

    progress("write a marker to each sample");
    for block in usable.iter().rev() {
        run.write_marker(*block)?;
    }
    run.drive.sync()?;

    progress("read each marker back");
    let mut first_bad = None;
    for block in &samples {
        let good = usable.contains(block) && run.holds_marker(*block);
        if !good {
            first_bad = Some(*block);
            break;
        }
    }

    // The last block is not on the step. It gets the test of one block,
    // which writes each place that it can wrap to after it.
    let last = blocks - 1;
    if first_bad.is_none() && !samples.contains(&last) && !run.good(last)? {
        first_bad = Some(last);
    }

    let usable_blocks = match first_bad {
        None => blocks,
        Some(bad) => {
            progress("search for the edge");
            let mut low = samples
                .iter()
                .copied()
                .filter(|b| *b < bad)
                .max()
                .map_or(0, |b| b + 1);
            let mut high = bad;
            // The first bad block is in [low, high]. Each step halves the
            // range.
            while low < high {
                let middle = low + (high - low) / 2;
                if run.good(middle)? {
                    low = middle + 1;
                } else {
                    high = middle;
                }
            }
            low
        }
    };

    progress("write back the data");
    let not_restored = run.restore(usable_blocks)?;
    Ok(Outcome {
        announced,
        usable: usable_blocks * BLOCK,
        blocks_written: run.copies.len(),
        not_restored,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rawdev::blocks::Memory;

    const MIB: u64 = 1 << 20;

    fn run(drive: &mut Memory) -> Outcome {
        probe(drive, 0x1234, &mut |_| {}).unwrap()
    }

    #[test]
    fn an_honest_drive_passes_and_keeps_each_byte() {
        let mut drive = Memory::honest(8 * MIB);
        let before = drive.data.clone();
        let outcome = run(&mut drive);
        assert_eq!(outcome.usable, 8 * MIB);
        assert!(!outcome.fake());
        assert_eq!(outcome.not_restored, 0);
        assert!(drive.data == before, "the drive holds what it held before");
    }

    #[test]
    fn a_drive_that_drops_writes_shows_its_real_size() {
        let mut drive = Memory::fake(3 * MIB, 8 * MIB, false);
        let before = drive.data.clone();
        let outcome = run(&mut drive);
        assert_eq!(outcome.usable, 3 * MIB);
        assert!(outcome.fake());
        assert!(
            drive.data == before,
            "the real part holds what it held before"
        );
    }

    #[test]
    fn a_drive_that_wraps_at_a_power_of_two_shows_its_real_size() {
        let mut drive = Memory::fake(2 * MIB, 16 * MIB, true);
        let before = drive.data.clone();
        let outcome = run(&mut drive);
        assert_eq!(outcome.usable, 2 * MIB);
        assert_eq!(outcome.not_restored, 0);
        assert!(
            drive.data == before,
            "each block that a marker hit was a sample"
        );
    }

    #[test]
    fn a_fake_that_drops_or_wraps_at_a_power_of_two_shows_its_real_size() {
        for announced in [8 * MIB, 16 * MIB] {
            for real in [
                MIB + 3 * BLOCK,
                4 * MIB - BLOCK,
                5 * MIB,
                7 * MIB + 9 * BLOCK,
            ] {
                let mut drive = Memory::fake(real, announced, false);
                assert_eq!(run(&mut drive).usable, real, "drops: {real} of {announced}");
            }
            for real in [MIB, 2 * MIB, 4 * MIB] {
                let mut drive = Memory::fake(real, announced, true);
                assert_eq!(run(&mut drive).usable, real, "wraps: {real} of {announced}");
            }
        }
    }

    // A drive that wraps at a size that is no power of two can put each
    // marker on a block that is no sample, and then the test does not see
    // the fake. f3probe has the same limit. What the test promises for such
    // a drive: when it says fake, the real size is at most the size that it
    // gives. att surface --write reads back each sector, and finds each one.
    #[test]
    fn a_fake_that_wraps_at_another_size_is_never_larger_than_it_says() {
        let mut found = 0;
        for real in [MIB + 3 * BLOCK, 3 * MIB, 5 * MIB, 7 * MIB + 9 * BLOCK] {
            let mut drive = Memory::fake(real, 16 * MIB, true);
            let outcome = run(&mut drive);
            if outcome.fake() {
                found += 1;
                assert!(
                    outcome.usable >= real,
                    "wraps at {real}: {}",
                    outcome.usable
                );
            }
        }
        assert!(found >= 3, "the test found {found} of the four fakes");
    }

    #[test]
    fn the_edge_is_found_to_the_block() {
        // A real size that is no sample and no power of two.
        let real = 5 * MIB + 7 * BLOCK;
        let mut drive = Memory::fake(real, 8 * MIB, false);
        assert_eq!(run(&mut drive).usable, real);
    }

    #[test]
    fn each_place_that_a_sample_wraps_to_is_a_sample() {
        for blocks in [100, 1000, 4096, 1 << 20, 15_625_000] {
            let samples = samples(blocks);
            assert_eq!(samples[0], 0);
            // A small drive gets each block as a sample.
            let least = blocks.min(128) as usize;
            assert!(
                samples.len() >= least && samples.len() <= 600,
                "{}",
                samples.len()
            );
            assert!(
                samples.windows(2).all(|w| w[0] < w[1]),
                "sorted, with no repeat"
            );
            for sample in &samples {
                for place in wrap_places(*sample) {
                    assert!(samples.contains(&place), "{sample} wraps to {place}");
                }
            }
        }
    }

    #[test]
    fn wrap_places_are_the_block_modulo_each_power_of_two() {
        assert_eq!(wrap_places(5), [0, 1]);
        assert_eq!(wrap_places(12), [0, 4]);
        assert!(wrap_places(0).is_empty());
    }

    #[test]
    fn a_marker_names_its_block_and_its_run() {
        let mut a = vec![0u8; BLOCK as usize];
        let mut b = vec![0u8; BLOCK as usize];
        marker(1, 7, &mut a);
        marker(1, 8, &mut b);
        assert_eq!(&a[..8], b"ATTPROBE");
        assert_ne!(a, b);
        marker(2, 7, &mut b);
        assert_ne!(a, b);
    }
}
