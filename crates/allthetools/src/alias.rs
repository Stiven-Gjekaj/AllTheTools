//! The familiar names of the tools, such as `lsusb` and `f3probe`.
//!
//! `att link` makes each name run att. On macOS and on Linux the name is a
//! symbolic link to att, and att reads the name that it was started with. On
//! Windows the name is a small .cmd file, because a symbolic link there needs
//! a privilege.
//!
//! This module makes no system call, so its tests run on every host.

use std::ffi::OsString;

/// Each familiar name, and the command of att that it runs.
pub const ALIASES: [(&str, &[&str]); 26] = [
    ("badblocks", &["surface"]),
    ("caffeinate", &["awake"]),
    ("ddcutil", &["ddc"]),
    ("eject", &["eject"]),
    ("f3probe", &["probe"]),
    ("free", &["free"]),
    ("fuser", &["whoholds"]),
    ("ldd", &["ldd"]),
    ("lsblk", &["lsblk"]),
    ("lspci", &["lspci"]),
    ("lsusb", &["lsusb"]),
    ("md5sum", &["hash", "-a", "md5"]),
    ("networkquality", &["netquality"]),
    ("pbcopy", &["clip", "copy"]),
    ("pbpaste", &["clip", "paste"]),
    ("sensors", &["sensors"]),
    ("sha1sum", &["hash", "-a", "sha1"]),
    ("sha256sum", &["hash", "-a", "sha256"]),
    ("sha512sum", &["hash", "-a", "sha512"]),
    ("timeout", &["timeout"]),
    ("trash", &["trash"]),
    ("unquarantine", &["unquarantine"]),
    ("wakeonlan", &["wol"]),
    ("watch", &["watch"]),
    ("whoport", &["whoport"]),
    ("xxd", &["hexdump"]),
];

/// The words of the command of att for a familiar name.
pub fn command(name: &str) -> Option<&'static [&'static str]> {
    ALIASES
        .iter()
        .find(|(alias, _)| *alias == name)
        .map(|(_, words)| *words)
}

/// Turn the arguments of a start under a familiar name into the arguments
/// of att. `lsusb -l` becomes `att lsusb -l`. A start under any other name
/// stays as it is.
pub fn expand(args: Vec<OsString>) -> Vec<OsString> {
    let Some(first) = args.first() else {
        return args;
    };
    // The name ends at the last slash of either kind, so the result is the
    // same on each host, and it loses its .exe on Windows.
    let text = first.to_string_lossy().to_ascii_lowercase();
    let file = text.rsplit(['/', '\\']).next().unwrap_or("");
    let name = file.strip_suffix(".exe").unwrap_or(file);
    let Some(words) = command(name) else {
        return args;
    };
    let mut out = vec![OsString::from("att")];
    out.extend(words.iter().map(OsString::from));
    out.extend(args.into_iter().skip(1));
    out
}

/// The text of the .cmd file of a familiar name on Windows. It passes each
/// argument on, and it gives the exit code of att.
pub fn shim(att: &str, words: &[&str]) -> String {
    format!(
        "@\"{att}\" {} %*\r\n@exit /b %ERRORLEVEL%\r\n",
        words.join(" ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(words: &[&str]) -> Vec<OsString> {
        words.iter().map(OsString::from).collect()
    }

    #[test]
    fn a_familiar_name_becomes_a_command_of_att() {
        assert_eq!(
            expand(args(&["/usr/local/bin/lsusb", "-l"])),
            args(&["att", "lsusb", "-l"])
        );
        assert_eq!(expand(args(&["pbpaste"])), args(&["att", "clip", "paste"]));
        assert_eq!(
            expand(args(&["C:\\tools\\F3PROBE.EXE", "E:"])),
            args(&["att", "probe", "E:"])
        );
    }

    #[test]
    fn att_and_an_unknown_name_stay_as_they_are() {
        assert_eq!(expand(args(&["att", "lsusb"])), args(&["att", "lsusb"]));
        assert_eq!(expand(args(&["/bin/other"])), args(&["/bin/other"]));
        assert!(expand(Vec::new()).is_empty());
    }

    #[test]
    fn each_alias_names_a_command_of_att() {
        use clap::CommandFactory;
        let cli = crate::cli::Cli::command();
        for (alias, words) in ALIASES {
            assert!(
                cli.find_subcommand(words[0]).is_some(),
                "{alias} names {}, which att does not have",
                words[0]
            );
        }
    }

    #[test]
    fn the_shim_passes_each_argument_and_the_exit_code() {
        let text = shim("C:\\att\\att.exe", &["clip", "copy"]);
        assert_eq!(
            text,
            "@\"C:\\att\\att.exe\" clip copy %*\r\n@exit /b %ERRORLEVEL%\r\n"
        );
    }
}
