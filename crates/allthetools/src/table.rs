//! A table for a person: columns of text, each as wide as its widest cell.
//!
//! This module makes no system call, so its tests run on every host.

/// Lay out a header and rows as lines of text.
///
/// Two spaces separate the columns. The last column has no padding after
/// it, so no line ends with a space. A row that is shorter than the header
/// gets empty cells.
pub fn render(header: &[&str], rows: &[Vec<String>]) -> String {
    let columns = header.len();
    let mut widths: Vec<usize> = header.iter().map(|h| h.chars().count()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate().take(columns) {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }

    let mut out = String::new();
    let header: Vec<String> = header.iter().map(|h| (*h).to_owned()).collect();
    for row in std::iter::once(&header).chain(rows) {
        let mut line = String::new();
        for (i, width) in widths.iter().enumerate() {
            let cell = row.get(i).map(String::as_str).unwrap_or("");
            if i > 0 {
                line.push_str("  ");
            }
            line.push_str(cell);
            if i + 1 < columns {
                let pad = width - cell.chars().count();
                line.extend(std::iter::repeat_n(' ', pad));
            }
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

/// Print a table on the output stream.
pub fn print(header: &[&str], rows: &[Vec<String>]) {
    print!("{}", render(header, rows));
}

#[cfg(test)]
mod tests {
    use super::render;

    fn row(cells: &[&str]) -> Vec<String> {
        cells.iter().map(|c| (*c).to_owned()).collect()
    }

    #[test]
    fn each_column_is_as_wide_as_its_widest_cell() {
        let text = render(&["A", "B", "C"], &[row(&["long cell", "x", "end"])]);
        assert_eq!(text, "A          B  C\nlong cell  x  end\n");
    }

    #[test]
    fn no_line_ends_with_a_space() {
        let text = render(&["NAME", "VALUE"], &[row(&["a", ""]), row(&["b", "1"])]);
        assert!(text.lines().all(|l| !l.ends_with(' ')), "{text:?}");
    }

    #[test]
    fn a_short_row_gets_empty_cells() {
        let text = render(&["A", "B"], &[row(&["only"])]);
        assert_eq!(text, "A     B\nonly\n");
    }

    #[test]
    fn width_counts_characters_and_not_bytes() {
        let text = render(&["X", "Y"], &[row(&["\u{e9}\u{e9}", "1"])]);
        assert_eq!(text.lines().nth(1), Some("\u{e9}\u{e9}  1"));
    }
}
