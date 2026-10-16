//! The forms of a hex dump that `xxd` writes, and the reverse of two of
//! them.
//!
//! Each function reads a stream and writes a stream, so a large file never
//! sits in memory whole. This module makes no system call, so its tests run
//! on every host.

use std::io::{self, BufRead, Read, Seek, SeekFrom, Write};

/// The settings of a dump.
#[derive(Debug, Clone)]
pub struct Options {
    /// The bytes on one line.
    pub cols: usize,
    /// The bytes in one group. Zero puts the whole line in one group.
    pub group: usize,
    /// Write the bits of each byte, and not its hex digits.
    pub bits: bool,
    /// Write the hex digits in capitals.
    pub upper: bool,
    /// The offset that the first byte shows.
    pub start: u64,
    /// The most bytes to read. None reads to the end.
    pub limit: Option<u64>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            cols: 16,
            group: 2,
            bits: false,
            upper: false,
            start: 0,
            limit: None,
        }
    }
}

/// Read up to `buffer.len()` bytes. A short count means the end of the
/// input, and not a pause of a pipe.
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

/// The input, cut to the limit.
fn limited<'a>(input: &'a mut dyn Read, limit: Option<u64>) -> Box<dyn Read + 'a> {
    match limit {
        Some(limit) => Box::new(Read::take(input, limit)),
        None => Box::new(input),
    }
}

fn byte_text(byte: u8, bits: bool, upper: bool) -> String {
    match (bits, upper) {
        (true, _) => format!("{byte:08b}"),
        (false, true) => format!("{byte:02X}"),
        (false, false) => format!("{byte:02x}"),
    }
}

/// The dump with an offset, the groups of hex digits and the text, as
/// `xxd` writes it by default.
pub fn dump(input: &mut dyn Read, out: &mut dyn Write, options: &Options) -> io::Result<()> {
    let cols = options.cols.max(1);
    // A group of zero, or one past the line, is one group for the line.
    let group = if options.group == 0 {
        cols
    } else {
        options.group
    };
    let digits = if options.bits { 8 } else { 2 };
    // The text starts at the same column on each line, also on a short last
    // line.
    let width = cols * digits + (cols - 1) / group;
    let mut input = limited(input, options.limit);
    let mut buffer = vec![0u8; cols];
    let mut offset = options.start;
    loop {
        let n = fill(&mut input, &mut buffer)?;
        if n == 0 {
            break;
        }
        let mut hex = String::with_capacity(width);
        for (i, byte) in buffer[..n].iter().enumerate() {
            if i > 0 && i % group == 0 {
                hex.push(' ');
            }
            hex.push_str(&byte_text(*byte, options.bits, options.upper));
        }
        let text: String = buffer[..n]
            .iter()
            .map(|b| {
                if (0x20..0x7f).contains(b) {
                    *b as char
                } else {
                    '.'
                }
            })
            .collect();
        writeln!(out, "{offset:08x}: {hex:width$}  {text}")?;
        offset += n as u64;
        if n < cols {
            break;
        }
    }
    Ok(())
}

/// The hex digits only, `cols` bytes on a line, as `xxd -p` writes them.
pub fn plain(input: &mut dyn Read, out: &mut dyn Write, options: &Options) -> io::Result<()> {
    let mut input = limited(input, options.limit);
    let mut buffer = vec![0u8; options.cols.max(1)];
    loop {
        let n = fill(&mut input, &mut buffer)?;
        if n == 0 {
            break;
        }
        let line: String = buffer[..n]
            .iter()
            .map(|b| byte_text(*b, false, options.upper))
            .collect();
        writeln!(out, "{line}")?;
        if n < buffer.len() {
            break;
        }
    }
    Ok(())
}

/// The name of a C array for a file, as `xxd -i` makes it: each character
/// that is not a letter or a digit becomes `_`, and a name that starts with a
/// digit gets `__` before it.
pub fn c_name(file: &str) -> String {
    let name: String = file
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    if name.starts_with(|c: char| c.is_ascii_digit()) {
        format!("__{name}")
    } else {
        name
    }
}

/// A C array of the bytes, twelve on a line, as `xxd -i` writes it. With no
/// name, it writes the lines of bytes only.
pub fn include(
    input: &mut dyn Read,
    out: &mut dyn Write,
    name: Option<&str>,
    options: &Options,
) -> io::Result<()> {
    const PER_LINE: usize = 12;
    if let Some(name) = name {
        writeln!(out, "unsigned char {name}[] = {{")?;
    }
    let mut input = limited(input, options.limit);
    let mut buffer = vec![0u8; PER_LINE];
    let mut total: u64 = 0;
    // Each line ends with a comma, except the last one, so a line is held
    // until the next read shows that more bytes come.
    let mut pending: Option<String> = None;
    loop {
        let n = fill(&mut input, &mut buffer)?;
        if n == 0 {
            break;
        }
        if let Some(line) = pending.take() {
            writeln!(out, "{line},")?;
        }
        let cells: Vec<String> = buffer[..n]
            .iter()
            .map(|b| {
                if options.upper {
                    format!("0X{b:02X}")
                } else {
                    format!("0x{b:02x}")
                }
            })
            .collect();
        pending = Some(format!("  {}", cells.join(", ")));
        total += n as u64;
        if n < PER_LINE {
            break;
        }
    }
    if let Some(line) = pending {
        writeln!(out, "{line}")?;
    }
    if let Some(name) = name {
        writeln!(out, "}};")?;
        writeln!(out, "unsigned int {name}_len = {total};")?;
    }
    Ok(())
}

fn nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Turn the hex digits of `xxd -p` back into bytes. Each character that is
/// not a hex digit is skipped, so the length of a line does not matter.
pub fn unplain(input: &mut dyn BufRead, out: &mut dyn Write) -> io::Result<()> {
    let mut high: Option<u8> = None;
    let mut bytes = Vec::new();
    for byte in input.bytes() {
        let Some(n) = nibble(byte?) else {
            continue;
        };
        match high.take() {
            Some(h) => bytes.push(h << 4 | n),
            None => high = Some(n),
        }
        if bytes.len() >= 1 << 16 {
            out.write_all(&bytes)?;
            bytes.clear();
        }
    }
    out.write_all(&bytes)
}

/// The offset and the bytes of one line of a dump. The hex digits end at
/// two spaces in a row, where the text starts, so a text that holds hex
/// letters does not become bytes.
pub fn dump_line(line: &str) -> Option<(u64, Vec<u8>)> {
    let (offset, rest) = line.split_once(':')?;
    let offset = u64::from_str_radix(offset.trim(), 16).ok()?;
    let rest = rest.strip_prefix(' ').unwrap_or(rest);
    let hex = rest.split("  ").next().unwrap_or("");
    let mut bytes = Vec::new();
    let mut high: Option<u8> = None;
    for c in hex.bytes() {
        if c == b' ' {
            continue;
        }
        let n = nibble(c)?;
        match high.take() {
            Some(h) => bytes.push(h << 4 | n),
            None => high = Some(n),
        }
    }
    Some((offset, bytes))
}

/// Turn a dump back into bytes, as `xxd -r` does. Each line goes to its
/// offset. A gap between two lines becomes zeros on a stream, and stays a
/// hole in a file that can seek.
pub fn undump<W: Write + Seek>(input: &mut dyn BufRead, out: &mut W) -> io::Result<()> {
    for line in input.lines() {
        let line = line?;
        let Some((offset, bytes)) = dump_line(&line) else {
            continue;
        };
        out.seek(SeekFrom::Start(offset))?;
        out.write_all(&bytes)?;
    }
    Ok(())
}

/// A stream that cannot seek, such as the output stream, as one that can
/// move forward. A move forward writes zeros. A move back fails, because the
/// bytes left already.
pub struct Forward<W: Write> {
    inner: W,
    at: u64,
}

impl<W: Write> Forward<W> {
    pub fn new(inner: W) -> Self {
        Forward { inner, at: 0 }
    }
}

impl<W: Write> Write for Forward<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.at += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<W: Write> Seek for Forward<W> {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let SeekFrom::Start(target) = to else {
            return Err(io::Error::other("a stream moves to an offset only"));
        };
        if target < self.at {
            return Err(io::Error::other(format!(
                "the dump goes back to offset {target:#x}, and a stream cannot go back. Give an output file"
            )));
        }
        let zeros = [0u8; 4096];
        while self.at < target {
            let n = (target - self.at).min(zeros.len() as u64) as usize;
            self.write_all(&zeros[..n])?;
        }
        Ok(self.at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The expected text of each test is what xxd 2025-08-24 writes for the
    // same bytes.
    const SAMPLE: &[u8] = b"Hello world.\nThis is att\x00\x01\xff";

    fn run(f: impl Fn(&mut dyn Read, &mut dyn Write) -> io::Result<()>) -> String {
        let mut out = Vec::new();
        f(&mut &SAMPLE[..], &mut out).unwrap();
        String::from_utf8(out).unwrap()
    }

    fn dumped(options: Options) -> String {
        run(|i, o| dump(i, o, &options))
    }

    #[test]
    fn the_default_dump_matches_xxd() {
        assert_eq!(
            dumped(Options::default()),
            "00000000: 4865 6c6c 6f20 776f 726c 642e 0a54 6869  Hello world..Thi\n\
             00000010: 7320 6973 2061 7474 0001 ff              s is att...\n"
        );
    }

    #[test]
    fn groups_and_columns_match_xxd() {
        let one = Options {
            group: 1,
            cols: 8,
            ..Options::default()
        };
        assert_eq!(
            dumped(one).lines().last(),
            Some("00000018: 00 01 ff                 ...")
        );
        let none = Options {
            group: 0,
            ..Options::default()
        };
        assert_eq!(
            dumped(none).lines().last(),
            Some("00000010: 73206973206174740001ff            s is att...")
        );
        let three = Options {
            group: 3,
            ..Options::default()
        };
        assert_eq!(
            dumped(three).lines().next(),
            Some("00000000: 48656c 6c6f20 776f72 6c642e 0a5468 69  Hello world..Thi")
        );
    }

    #[test]
    fn bits_match_xxd() {
        let bits = Options {
            bits: true,
            group: 1,
            cols: 6,
            ..Options::default()
        };
        let text = dumped(bits);
        assert_eq!(
            text.lines().next(),
            Some("00000000: 01001000 01100101 01101100 01101100 01101111 00100000  Hello ")
        );
        assert_eq!(
            text.lines().last(),
            Some("00000018: 00000000 00000001 11111111                             ...")
        );
    }

    #[test]
    fn the_start_and_the_limit_show_in_the_offset() {
        let mut input = &SAMPLE[10..];
        let mut out = Vec::new();
        let options = Options {
            start: 10,
            limit: Some(4),
            upper: true,
            ..Options::default()
        };
        dump(&mut input, &mut out, &options).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "0000000a: 642E 0A54                                d..T\n"
        );
    }

    #[test]
    fn an_empty_input_gives_no_line() {
        let mut out = Vec::new();
        dump(&mut &[][..], &mut out, &Options::default()).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn plain_matches_xxd() {
        let options = Options {
            cols: 30,
            ..Options::default()
        };
        assert_eq!(
            run(|i, o| plain(i, o, &options)),
            "48656c6c6f20776f726c642e0a54686973206973206174740001ff\n"
        );
        let short = Options {
            cols: 4,
            ..Options::default()
        };
        assert_eq!(
            run(|i, o| plain(i, o, &short)).lines().next(),
            Some("48656c6c")
        );
    }

    #[test]
    fn include_matches_xxd() {
        let options = Options::default();
        assert_eq!(
            run(|i, o| include(i, o, Some("s_bin"), &options)),
            "unsigned char s_bin[] = {\n  \
             0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x20, 0x77, 0x6f, 0x72, 0x6c, 0x64, 0x2e,\n  \
             0x0a, 0x54, 0x68, 0x69, 0x73, 0x20, 0x69, 0x73, 0x20, 0x61, 0x74, 0x74,\n  \
             0x00, 0x01, 0xff\n\
             };\n\
             unsigned int s_bin_len = 27;\n"
        );
        let mut out = Vec::new();
        include(&mut &b"ab"[..], &mut out, None, &options).unwrap();
        assert_eq!(out, b"  0x61, 0x62\n");
    }

    #[test]
    fn a_full_last_line_of_include_has_no_comma() {
        let mut out = Vec::new();
        include(&mut &[7u8; 12][..], &mut out, None, &Options::default()).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert_eq!(text.lines().count(), 1);
        assert!(!text.trim_end().ends_with(','), "{text}");
    }

    #[test]
    fn c_names_match_xxd() {
        assert_eq!(c_name("s.bin"), "s_bin");
        assert_eq!(c_name("9x.bin"), "__9x_bin");
        assert_eq!(c_name("a-b c"), "a_b_c");
    }

    #[test]
    fn a_dump_turns_back_into_the_same_bytes() {
        for options in [
            Options::default(),
            Options {
                group: 0,
                ..Options::default()
            },
            Options {
                group: 3,
                cols: 7,
                upper: true,
                ..Options::default()
            },
        ] {
            let text = dumped(options);
            let mut back = Forward::new(Vec::new());
            undump(&mut text.as_bytes(), &mut back).unwrap();
            assert_eq!(back.inner, SAMPLE);
        }
    }

    #[test]
    fn hex_letters_in_the_text_do_not_become_bytes() {
        let (offset, bytes) =
            dump_line("00000010: 4142                                     cafe").unwrap();
        assert_eq!(offset, 0x10);
        assert_eq!(bytes, b"AB");
    }

    #[test]
    fn a_gap_in_a_dump_becomes_zeros_on_a_stream() {
        let mut back = Forward::new(Vec::new());
        undump(&mut &b"00000004: 4142  AB\n"[..], &mut back).unwrap();
        assert_eq!(back.inner, b"\0\0\0\0AB");
    }

    #[test]
    fn a_dump_that_goes_back_fails_on_a_stream() {
        let mut back = Forward::new(Vec::new());
        let text = b"00000004: 41  A\n00000000: 42  B\n";
        assert!(undump(&mut &text[..], &mut back).is_err());
    }

    #[test]
    fn plain_digits_turn_back_into_bytes() {
        let mut out = Vec::new();
        unplain(&mut &b"4142 43\n44\n"[..], &mut out).unwrap();
        assert_eq!(out, b"ABCD");
    }
}
