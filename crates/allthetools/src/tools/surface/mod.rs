//! `att surface`: scan a drive for bad sectors.
//!
//! Linux has `badblocks`. macOS and Windows have no command that reads each
//! sector of a drive and names the ones that fail. This tool reads each
//! sector by default, and changes nothing. With --write it writes a pattern
//! to each sector and reads it back, as `badblocks -w` does, and that erases
//! the drive.

use std::time::Duration;

use anyhow::Result;

pub mod scan;

use crate::progress::Progress;
use crate::rawdev::{self, guard};

/// Read each sector of a drive, and name the sectors that fail.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The drive, as `att lsblk` names it, or an image file.
    pub target: String,

    /// Write a pattern to each sector and read it back. This erases the
    /// drive.
    #[arg(short, long)]
    pub write: bool,

    /// With --write, take a drive that the host does not mark as removable.
    #[arg(long, requires = "write")]
    pub force: bool,

    /// A read of one chunk that takes longer than this time is slow.
    #[arg(long, default_value = "1s", value_name = "TIME")]
    pub slow: String,

    /// Print JSON for a script, and not text for a person.
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let slow = crate::time::duration(&args.slow)?;
    let target = rawdev::find(&args.target)?;
    let options = scan::Options {
        slow: if slow.is_zero() { Duration::MAX } else { slow },
        ..scan::Options::default()
    };

    let report = if args.write {
        guard::check(&target, args.force)?;
        guard::ask(
            &target,
            "This writes over each sector of the target, and erases it. Nothing undoes it.",
        )?;
        let mut drive = rawdev::open(&target, true)?;
        let mut progress = Progress::new("write and read");
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(1);
        scan::write(drive.as_mut(), &options, seed, &mut |d, t| {
            progress.update(d, t)
        })?
    } else {
        let mut drive = rawdev::open(&target, false)?;
        let mut progress = Progress::new("read");
        scan::read(drive.as_mut(), &options, &mut |d, t| progress.update(d, t))
    };

    if args.json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        print(&report, &target.describe(), args.write);
    }
    // Exit code 2 says that the scan ran and found bad sectors.
    Ok(if report.bad_count > 0 { 2 } else { 0 })
}

fn print(report: &scan::Report, target: &str, write: bool) {
    let rate = report.checked as f64 / report.seconds.max(0.001);
    let verb = if write { "Wrote and read back" } else { "Read" };
    println!(
        "{verb} {} ({} bytes) of {target} in {:.0} s, at {}/s.",
        crate::size::decimal(report.checked),
        crate::size::grouped(report.checked),
        report.seconds,
        crate::size::decimal(rate as u64)
    );
    if report.bad_count == 0 {
        println!("No sector failed.");
    } else {
        println!(
            "{} sectors of {} bytes failed:",
            report.bad_count, report.sector
        );
        for offset in &report.bad {
            println!(
                "    sector {} at byte {}",
                crate::size::grouped(offset / u64::from(report.sector)),
                crate::size::grouped(*offset)
            );
        }
        if report.bad_count > report.bad.len() as u64 {
            println!(
                "    and {} more",
                report.bad_count - report.bad.len() as u64
            );
        }
    }
    if !report.slow.is_empty() {
        println!("{} chunks of 4 MiB were slow:", report.slow.len());
        for (offset, ms) in &report.slow {
            println!("    at byte {}: {ms} ms", crate::size::grouped(*offset));
        }
    }
}
