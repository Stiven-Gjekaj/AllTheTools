//! `att probe`: find the real size of a flash drive.
//!
//! A fake USB stick or SD card reports a size that it does not have, and it
//! loses the data that goes past its real size. Linux has f3probe, which
//! finds this in a minute. f3write and f3read exist for macOS, and they fill
//! the whole drive, which takes hours. Windows has H2testw, which also
//! fills the whole drive. This tool does the test of f3probe on each host.
//!
//! It writes to a few hundred blocks of the drive, and it writes back what was
//! there before. algo.rs says what the test finds and what it can miss.

use anyhow::Result;

pub mod algo;

use crate::rawdev::{self, guard};

/// Find the real size of a flash drive, as f3probe does.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The drive, as `att lsblk` names it, or an image file.
    pub target: String,

    /// Take a drive that the host does not mark as removable.
    #[arg(long)]
    pub force: bool,

    /// Print JSON for a script, and not text for a person.
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let target = rawdev::find(&args.target)?;
    guard::check(&target, args.force)?;
    guard::ask(
        &target,
        "This writes test data to a few hundred blocks of the target, and then writes back \
         what was there. A fake drive can lose data that it already holds, because that \
         is what makes it fake. Keep a copy of anything on it that you value.",
    )?;
    let mut drive = rawdev::open(&target, true)?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1);
    let quiet = args.json;
    let outcome = algo::probe(drive.as_mut(), nonce, &mut |step| {
        if !quiet {
            eprintln!("att probe: {step}...");
        }
    })?;
    drop(drive);

    if args.json {
        println!("{}", serde_json::to_string(&outcome)?);
    } else {
        print(&outcome);
    }
    // Exit code 2 says that the test ran and found a fake.
    Ok(if outcome.fake() { 2 } else { 0 })
}

fn print(outcome: &algo::Outcome) {
    let size = |bytes: u64| {
        format!(
            "{} ({} bytes)",
            crate::size::decimal(bytes),
            crate::size::grouped(bytes)
        )
    };
    println!("Announced size  {}", size(outcome.announced));
    println!("Usable size     {}", size(outcome.usable));
    if outcome.fake() {
        println!(
            "Result          FAKE. The drive keeps data in the first {} only, and it loses each write past that.",
            crate::size::decimal(outcome.usable)
        );
    } else {
        println!(
            "Result          The drive kept each marker. It is not a fake that drops writes or that wraps at a power of two."
        );
    }
    println!(
        "Restore         Wrote back {} blocks of 4096 bytes.",
        outcome.blocks_written
    );
    if outcome.not_restored > 0 {
        println!(
            "                {} blocks inside the usable size did not read back their data.",
            outcome.not_restored
        );
    }
}
