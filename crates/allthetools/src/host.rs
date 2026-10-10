//! Run a command that ships with the host, and read its machine output.
//!
//! A tool uses this only when the host has no public API for the data, or
//! when the API is very large for the job. CONTRIBUTING.md gives the rule.

use std::io::ErrorKind;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

/// Run a program, and give what it wrote on the output stream.
///
/// A program that is missing gives an error that names it, and the `hint`
/// when there is one. A program that fails gives an error with the first
/// line that it wrote on the error stream.
pub fn output(program: &str, args: &[&str], hint: Option<&str>) -> Result<Vec<u8>> {
    let result = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output();
    let out = match result {
        Ok(out) => out,
        Err(e) if e.kind() == ErrorKind::NotFound => match hint {
            Some(hint) => bail!("this host has no `{program}`. {hint}"),
            None => bail!("this host has no `{program}`"),
        },
        Err(e) => return Err(e).with_context(|| format!("could not start `{program}`")),
    };
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let first = stderr.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
        bail!("`{program}` failed ({}): {}", out.status, first.trim());
    }
    Ok(out.stdout)
}

/// Run a program, and give its output as text.
pub fn text(program: &str, args: &[&str], hint: Option<&str>) -> Result<String> {
    let bytes = output(program, args, hint)?;
    String::from_utf8(bytes).with_context(|| format!("`{program}` wrote text that is not UTF-8"))
}

/// True when a program with this name is in a directory of PATH.
///
/// Windows also tries each suffix of PATHEXT, as its shell does.
pub fn on_path(program: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    let suffixes: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT".to_owned())
            .split(';')
            .map(str::to_owned)
            .chain(std::iter::once(String::new()))
            .collect()
    } else {
        vec![String::new()]
    };
    std::env::split_paths(&path).any(|dir| {
        suffixes
            .iter()
            .any(|suffix| dir.join(format!("{program}{suffix}")).is_file())
    })
}

/// Run a program, give it `input` on its input stream, and wait for it.
pub fn feed(program: &str, args: &[&str], input: &[u8], hint: Option<&str>) -> Result<()> {
    use std::io::Write;
    let child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(e) if e.kind() == ErrorKind::NotFound => match hint {
            Some(hint) => bail!("this host has no `{program}`. {hint}"),
            None => bail!("this host has no `{program}`"),
        },
        Err(e) => return Err(e).with_context(|| format!("could not start `{program}`")),
    };
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input)
            .with_context(|| format!("could not write to `{program}`"))?;
    }
    let out = child.wait_with_output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let first = stderr.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
        bail!("`{program}` failed ({}): {}", out.status, first.trim());
    }
    Ok(())
}

/// Run a PowerShell script on Windows, and give its output as text.
///
/// The script sets the output encoding to UTF-8 first, so a name in any
/// language arrives whole. A script asks for JSON with `ConvertTo-Json`, and
/// never for the text that PowerShell writes for a person, because that text
/// changes with the language of the host.
#[cfg(windows)]
pub fn powershell(script: &str) -> Result<String> {
    let script = format!(
        "$ErrorActionPreference = 'Stop'; \
         [Console]::OutputEncoding = [System.Text.Encoding]::UTF8; {script}"
    );
    text(
        "powershell.exe",
        &["-NoProfile", "-NonInteractive", "-Command", &script],
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::{on_path, output};

    #[test]
    fn on_path_finds_a_program_of_the_host_and_not_a_made_up_one() {
        let shell = if cfg!(windows) { "cmd" } else { "sh" };
        assert!(on_path(shell));
        assert!(!on_path("att-no-such-program"));
    }

    #[test]
    fn a_missing_program_gives_an_error_that_names_it() {
        let error = output("att-no-such-program", &[], Some("Install it.")).unwrap_err();
        let text = error.to_string();
        assert!(text.contains("att-no-such-program"), "{text}");
        assert!(text.contains("Install it."), "{text}");
    }
}
