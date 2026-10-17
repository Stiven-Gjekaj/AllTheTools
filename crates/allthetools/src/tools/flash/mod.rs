//! `att flash`: write a disk image, such as the ISO of a Linux installer,
//! to a USB drive, and read it back to check each byte.
//!
//! Linux and macOS have `dd`, which writes to any drive that a person names,
//! the system drive too, and checks nothing. Windows has no command for it.
//! This tool takes the drive by the name that `att lsblk` gives, refuses the
//! system drive, asks the person to type the name, and reads the drive back
//! after the write.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use serde::Serialize;

pub mod copy;

use crate::progress::Progress;
use crate::rawdev::{self, Target, guard};

/// Write a disk image to a drive, and read it back to check it.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The image to write, such as an ISO or an IMG file.
    pub image: PathBuf,

    /// The drive, as `att lsblk` names it, or an image file.
    pub target: String,

    /// Take a drive that the host does not mark as removable.
    #[arg(long)]
    pub force: bool,

    /// Do not read the drive back after the write.
    #[arg(long)]
    pub no_verify: bool,

    /// Print JSON for a script, and not text for a person.
    #[arg(long)]
    pub json: bool,
}

/// What the tool did.
#[derive(Debug, Default, Serialize)]
pub(crate) struct Report {
    pub(crate) image: String,
    pub(crate) target: String,
    pub(crate) bytes: u64,
    pub(crate) verified: bool,
    /// The offset of the first byte that read back wrong.
    pub(crate) mismatch: Option<u64>,
    pub(crate) seconds: f64,
}

/// Each mount point of the target, so the tool can refuse an image that
/// lives on it.
fn mounts(target: &Target) -> Vec<PathBuf> {
    match target {
        Target::Drive(disk) => disk
            .partitions
            .iter()
            .filter_map(|p| p.mount.as_deref())
            .map(|m| {
                Path::new(m)
                    .canonicalize()
                    .unwrap_or_else(|_| PathBuf::from(m))
            })
            .collect(),
        Target::File(path) => vec![path.canonicalize().unwrap_or_else(|_| path.clone())],
    }
}

pub fn run(args: Args) -> Result<i32> {
    let image_path = args
        .image
        .canonicalize()
        .with_context(|| format!("could not find {}", args.image.display()))?;
    let mut image = File::open(&image_path)
        .with_context(|| format!("could not open {}", args.image.display()))?;
    let size = image.metadata()?.len();
    if size == 0 {
        bail!("{} is empty", args.image.display());
    }
    let mut magic = [0u8; 8];
    let n = image.read(&mut magic)?;
    if let Some(kind) = copy::compressed(&magic[..n]) {
        bail!(
            "{} is a {kind} archive. Extract the image from it first, and give the image",
            args.image.display()
        );
    }

    let target = rawdev::find(&args.target)?;
    if copy::inside(&image_path, &mounts(&target)) {
        bail!("the image is on the target. The write would overwrite it while it is read");
    }
    guard::check(&target, args.force)?;
    if let Target::Drive(disk) = &target
        && size > disk.size
    {
        bail!(
            "the image is {}, and {} is {}. It does not fit",
            crate::size::decimal(size),
            disk.id,
            crate::size::decimal(disk.size)
        );
    }
    guard::ask(
        &target,
        &format!(
            "This writes {} ({}) to the target, and erases what it holds. Nothing undoes it.",
            args.image.display(),
            crate::size::decimal(size)
        ),
    )?;

    let started = Instant::now();
    let mut drive = rawdev::open(&target, true)?;
    image.seek(SeekFrom::Start(0))?;
    let mut progress = Progress::new("write");
    let bytes = copy::write(
        &mut BufReader::with_capacity(copy::CHUNK, &mut image),
        size,
        drive.as_mut(),
        &mut |d, t| progress.update(d, t),
    )?;
    progress.finish();

    let mut mismatch = None;
    if !args.no_verify {
        image.seek(SeekFrom::Start(0))?;
        let mut progress = Progress::new("read back");
        mismatch = copy::verify(
            &mut BufReader::with_capacity(copy::CHUNK, &mut image),
            size,
            drive.as_mut(),
            &mut |d, t| progress.update(d, t),
        )?;
        progress.finish();
    }
    drop(drive);

    let report = Report {
        image: args.image.display().to_string(),
        target: target.describe(),
        bytes,
        verified: !args.no_verify && mismatch.is_none(),
        mismatch,
        seconds: started.elapsed().as_secs_f64(),
    };
    if args.json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        let rate = bytes as f64 / report.seconds.max(0.001);
        println!(
            "Wrote {} ({} bytes) to {} in {:.0} s, at {}/s.",
            crate::size::decimal(bytes),
            crate::size::grouped(bytes),
            report.target,
            report.seconds,
            crate::size::decimal(rate as u64)
        );
        match (args.no_verify, mismatch) {
            (true, _) => println!("The drive was not read back."),
            (false, None) => println!("The drive read back the same bytes as the image."),
            (false, Some(at)) => println!(
                "The drive read back a different byte at offset {}. The drive is bad or fake: `att probe` tests it.",
                crate::size::grouped(at)
            ),
        }
        if let Target::Drive(disk) = &target {
            println!("`att eject {}` makes it safe to remove.", disk.id);
        }
    }
    // Exit code 2 says that the write ran and the drive read back wrong.
    Ok(if mismatch.is_some() { 2 } else { 0 })
}
