//! The lines of a list of checksums, in the two forms that the tools of the
//! hosts write:
//!
//! - GNU: `<digest>  <name>`, with `*` before the name in binary mode.
//! - BSD: `SHA256 (<name>) = <digest>`.
//!
//! This module makes no system call, so its tests run on every host.

use super::digest::Algorithm;

/// One line of a list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub algorithm: Algorithm,
    pub digest: String,
    pub name: String,
}

/// Read one line. A blank line, a comment, or a line of another form gives
/// None.
pub fn line(text: &str) -> Option<Line> {
    let text = text.trim_end_matches(['\r', '\n']);
    if text.trim().is_empty() || text.starts_with('#') {
        return None;
    }
    // BSD: TAG (name) = digest
    if let Some((left, digest)) = text.rsplit_once(") = ")
        && let Some((tag, name)) = left.split_once(" (")
        && let Some(algorithm) = Algorithm::from_tag(tag)
        && is_hex(digest, algorithm)
    {
        return Some(Line {
            algorithm,
            digest: digest.to_ascii_lowercase(),
            name: name.to_owned(),
        });
    }
    // GNU: digest, a space, a space or a star, name
    let (digest, rest) = text.split_once(' ')?;
    let name = rest.strip_prefix([' ', '*'])?;
    let algorithm = Algorithm::from_hex_len(digest.len())?;
    if !is_hex(digest, algorithm) || name.is_empty() {
        return None;
    }
    Some(Line {
        algorithm,
        digest: digest.to_ascii_lowercase(),
        name: name.to_owned(),
    })
}

fn is_hex(digest: &str, algorithm: Algorithm) -> bool {
    Algorithm::from_hex_len(digest.len()) == Some(algorithm)
        && digest.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    #[test]
    fn the_gnu_form_is_read_in_text_and_in_binary_mode() {
        let expected = Line {
            algorithm: Algorithm::Sha256,
            digest: DIGEST.to_owned(),
            name: "my file.iso".to_owned(),
        };
        assert_eq!(
            line(&format!("{DIGEST}  my file.iso")),
            Some(expected.clone())
        );
        assert_eq!(line(&format!("{DIGEST} *my file.iso\r\n")), Some(expected));
    }

    #[test]
    fn the_bsd_form_is_read() {
        let got = line(&format!("SHA256 (a (1).iso) = {}", DIGEST.to_uppercase())).unwrap();
        assert_eq!(got.algorithm, Algorithm::Sha256);
        assert_eq!(got.name, "a (1).iso");
        assert_eq!(got.digest, DIGEST);
        let md5 = line("MD5 (x) = d41d8cd98f00b204e9800998ecf8427e").unwrap();
        assert_eq!(md5.algorithm, Algorithm::Md5);
    }

    #[test]
    fn other_lines_are_skipped() {
        assert_eq!(line(""), None);
        assert_eq!(line("# a comment"), None);
        assert_eq!(line("not a checksum"), None);
        assert_eq!(line(&format!("{}  x", &DIGEST[..63])), None);
        assert_eq!(line(&format!("{}zz  x", &DIGEST[..62])), None);
        assert_eq!(line(&format!("{DIGEST}x name")), None);
    }
}
