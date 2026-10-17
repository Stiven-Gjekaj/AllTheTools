//! `att hash`: give the checksum of files, and check a list of checksums.
//!
//! Linux has `sha256sum` and its family, and macOS has `shasum`. Windows has
//! `certutil -hashfile` and `Get-FileHash`, which take one file and cannot
//! check the `SHA256SUMS` file of a download. This tool writes the form of
//! `sha256sum`, and `-c` checks a list in the form of GNU or of BSD.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

use anyhow::{Context, Result, bail};

pub mod check;
pub mod digest;

use digest::Algorithm;

/// Give the checksum of files, or check a list of checksums.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The files. With none, or with `-`, the input stream.
    pub files: Vec<String>,

    /// The algorithm.
    #[arg(short, long, value_enum, default_value_t = Algorithm::Sha256)]
    pub algorithm: Algorithm,

    /// Read each file as a list of checksums, and check each file that it
    /// names. The algorithm of each line comes from its length or its tag.
    #[arg(short, long)]
    pub check: bool,

    /// With --check, name only the files that fail.
    #[arg(short, long, requires = "check")]
    pub quiet: bool,

    /// With --check, skip a file of the list that does not exist.
    #[arg(long, requires = "check")]
    pub ignore_missing: bool,
}

fn open(name: &str) -> Result<Box<dyn Read>> {
    if name == "-" {
        return Ok(Box::new(io::stdin().lock()));
    }
    let file = File::open(name).with_context(|| format!("could not open {name}"))?;
    Ok(Box::new(file))
}

fn digest_of(mut input: impl Read, algorithm: Algorithm) -> io::Result<String> {
    let mut hasher = algorithm.hasher();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        match input.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => hasher.update(&buffer[..n]),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(hasher.hex())
}

pub fn run(args: Args) -> Result<i32> {
    let files = if args.files.is_empty() {
        vec!["-".to_owned()]
    } else {
        args.files.clone()
    };
    if args.check {
        return check_lists(&files, &args);
    }
    let mut failed = false;
    for name in &files {
        let result = open(name).and_then(|input| Ok(digest_of(input, args.algorithm)?));
        match result {
            Ok(hex) => println!("{hex}  {name}"),
            Err(e) => {
                eprintln!("att: {e:#}");
                failed = true;
            }
        }
    }
    Ok(if failed { 1 } else { 0 })
}

/// Check each list. The names in a list are relative to the folder of the
/// list, as a SHA256SUMS file next to its downloads expects.
fn check_lists(lists: &[String], args: &Args) -> Result<i32> {
    let (mut bad, mut missing, mut checked) = (0u64, 0u64, 0u64);
    for list in lists {
        let base = if list == "-" {
            Path::new(".").to_path_buf()
        } else {
            Path::new(list)
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default()
        };
        let reader = BufReader::new(open(list)?);
        let mut found = 0;
        for text in reader.lines() {
            let Some(line) = check::line(&text?) else {
                continue;
            };
            found += 1;
            let path = base.join(&line.name);
            let file = match File::open(&path) {
                Ok(file) => file,
                Err(e) if e.kind() == io::ErrorKind::NotFound && args.ignore_missing => continue,
                Err(e) => {
                    println!("{}: FAILED to open ({e})", line.name);
                    missing += 1;
                    continue;
                }
            };
            checked += 1;
            if digest_of(file, line.algorithm)? == line.digest {
                if !args.quiet {
                    println!("{}: OK", line.name);
                }
            } else {
                println!("{}: FAILED", line.name);
                bad += 1;
            }
        }
        if found == 0 {
            bail!("{list} holds no line of a checksum");
        }
    }
    if bad > 0 {
        eprintln!("att: {bad} of {checked} files did not match");
    }
    if missing > 0 {
        eprintln!("att: {missing} files could not be read");
    }
    if checked == 0 && missing == 0 {
        bail!("no file of the list exists here");
    }
    // Exit code 2 says that the check ran and a file did not match.
    Ok(if bad > 0 {
        2
    } else if missing > 0 {
        1
    } else {
        0
    })
}
