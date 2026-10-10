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
    use super::output;

    #[test]
    fn a_missing_program_gives_an_error_that_names_it() {
        let error = output("att-no-such-program", &[], Some("Install it.")).unwrap_err();
        let text = error.to_string();
        assert!(text.contains("att-no-such-program"), "{text}");
        assert!(text.contains("Install it."), "{text}");
    }
}
