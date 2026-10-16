//! `att hexdump`: show the bytes of a file, as `xxd` does.
//!
//! macOS and Linux have `xxd`, and Windows has no command that shows the
//! bytes of a file. `Format-Hex` of PowerShell has other output and no way
//! back. This tool takes the options of `xxd` and writes the same text, and
//! `-r` turns that text back into bytes.

use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

use anyhow::{Context, Result, bail};

pub mod format;

use format::Options;

/// Show the bytes of a file as hex, as xxd does, or turn the hex back into
/// bytes.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// The file to read. With no file, or with `-`, the input stream.
    pub input: Option<String>,

    /// The file to write. With no file, the output stream. With -r, the file
    /// is changed at the offsets of the dump, and not cut short, as xxd -r
    /// does.
    pub output: Option<String>,

    /// The bytes on one line: 16 by default, 30 with -p, 6 with -b.
    #[arg(short, long, value_name = "N")]
    pub cols: Option<usize>,

    /// The bytes in one group: 2 by default, 1 with -b. 0 puts each line in
    /// one group.
    #[arg(short = 'g', long = "groupsize", value_name = "N")]
    pub group: Option<usize>,

    /// Show the bits of each byte, and not its hex digits.
    #[arg(short, long, conflicts_with_all = ["plain", "include"])]
    pub bits: bool,

    /// Write the hex digits in capitals.
    #[arg(short, long)]
    pub upper: bool,

    /// Start at this offset of the input. Give a number, or a hex number with
    /// 0x.
    #[arg(short, long, value_name = "OFFSET")]
    pub seek: Option<String>,

    /// Stop after this many bytes.
    #[arg(short, long, value_name = "N")]
    pub len: Option<String>,

    /// Write the hex digits only, with no offset and no text.
    #[arg(short, long, conflicts_with = "include")]
    pub plain: bool,

    /// Write a C array of the bytes.
    #[arg(short, long)]
    pub include: bool,

    /// The name of the C array of -i. By default, the name of the file.
    #[arg(short, long, requires = "include")]
    pub name: Option<String>,

    /// Turn a dump, or the digits of -p, back into bytes.
    #[arg(short, long, conflicts_with_all = ["bits", "include", "seek", "len"])]
    pub revert: bool,
}

/// Read a count or an offset: a number, or a hex number with 0x.
fn number(text: &str) -> Result<u64> {
    let text = text.trim();
    let value = match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(hex) => u64::from_str_radix(hex, 16),
        None => text.parse(),
    };
    value.map_err(|_| {
        anyhow::anyhow!("`{text}` is not a number. Give a number, or a hex number with 0x")
    })
}

fn from_stdin(input: Option<&str>) -> bool {
    matches!(input, None | Some("-"))
}

pub fn run(args: Args) -> Result<i32> {
    let result = if args.revert {
        revert(&args)
    } else {
        forward(&args)
    };
    match result {
        // A reader such as `head` that stops early is not an error.
        Err(e)
            if e.downcast_ref::<io::Error>()
                .is_some_and(|e| e.kind() == io::ErrorKind::BrokenPipe) =>
        {
            Ok(0)
        }
        Err(e) => Err(e),
        Ok(()) => Ok(0),
    }
}

fn forward(args: &Args) -> Result<()> {
    let start = args.seek.as_deref().map(number).transpose()?.unwrap_or(0);
    let limit = args.len.as_deref().map(number).transpose()?;
    let mut input: Box<dyn Read> = if from_stdin(args.input.as_deref()) {
        let mut stdin = io::stdin().lock();
        // The input stream cannot seek, so the bytes before the start are
        // read and dropped.
        io::copy(&mut Read::take(&mut stdin, start), &mut io::sink())?;
        Box::new(stdin)
    } else {
        let path = args.input.as_deref().unwrap_or_default();
        let mut file = File::open(path).with_context(|| format!("could not open {path}"))?;
        file.seek(SeekFrom::Start(start))?;
        Box::new(BufReader::new(file))
    };
    let options = Options {
        cols: args.cols.unwrap_or(if args.plain {
            30
        } else if args.bits {
            6
        } else {
            16
        }),
        group: args.group.unwrap_or(if args.bits { 1 } else { 2 }),
        bits: args.bits,
        upper: args.upper,
        start,
        limit,
    };
    let mut out: Box<dyn Write> = match &args.output {
        Some(path) => Box::new(BufWriter::new(
            File::create(path).with_context(|| format!("could not write {path}"))?,
        )),
        None => Box::new(BufWriter::new(io::stdout().lock())),
    };
    if args.include {
        let name = match (&args.name, args.input.as_deref()) {
            (Some(name), _) => Some(name.clone()),
            (None, Some(path)) if path != "-" => Some(format::c_name(path)),
            _ => None,
        };
        format::include(&mut input, &mut out, name.as_deref(), &options)?;
    } else if args.plain {
        format::plain(&mut input, &mut out, &options)?;
    } else {
        format::dump(&mut input, &mut out, &options)?;
    }
    out.flush()?;
    Ok(())
}

fn revert(args: &Args) -> Result<()> {
    let mut input: Box<dyn io::BufRead> = if from_stdin(args.input.as_deref()) {
        Box::new(io::stdin().lock())
    } else {
        let path = args.input.as_deref().unwrap_or_default();
        Box::new(BufReader::new(
            File::open(path).with_context(|| format!("could not open {path}"))?,
        ))
    };
    match &args.output {
        Some(path) => {
            if Path::new(path).is_dir() {
                bail!("{path} is a folder. Give a file");
            }
            let file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .open(path)
                .with_context(|| format!("could not write {path}"))?;
            let mut out = BufWriter::new(file);
            if args.plain {
                format::unplain(&mut input, &mut out)?;
            } else {
                format::undump(&mut input, &mut out)?;
            }
            out.flush()?;
        }
        None => {
            let mut out = format::Forward::new(BufWriter::new(io::stdout().lock()));
            if args.plain {
                format::unplain(&mut input, &mut out)?;
            } else {
                format::undump(&mut input, &mut out)?;
            }
            out.flush()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::number;

    #[test]
    fn a_number_is_decimal_or_hex() {
        assert_eq!(number("16").unwrap(), 16);
        assert_eq!(number("0x10").unwrap(), 16);
        assert_eq!(number("0X1f").unwrap(), 31);
        assert!(number("ten").is_err());
        assert!(number("-1").is_err());
    }
}
