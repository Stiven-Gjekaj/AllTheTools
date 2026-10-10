//! The host file of `att mount` on Windows: Mount-DiskImage, which attaches
//! an ISO, a VHD or a VHDX, and gives each volume a letter.

use std::path::Path;

use anyhow::{Result, bail};

use super::parse::{self, Attached, Kind};
use crate::host;

/// A path for a PowerShell string in single quotes.
fn quoted(path: &Path) -> Result<String> {
    let full = std::fs::canonicalize(path)?;
    // canonicalize gives the \\?\ form, which Mount-DiskImage refuses.
    let text = full.to_string_lossy();
    let text = text.strip_prefix(r"\\?\").unwrap_or(&text);
    Ok(text.replace('\'', "''"))
}

pub fn attach(image: &Path, kind: Kind, read_only: bool) -> Result<Vec<Attached>> {
    if matches!(kind, Kind::Raw | Kind::Dmg) {
        bail!("Windows attaches an ISO, a VHD or a VHDX only");
    }
    let path = quoted(image)?;
    let access = if read_only || kind == Kind::Iso {
        "ReadOnly"
    } else {
        "ReadWrite"
    };
    let script = format!(
        "$i = Mount-DiskImage -ImagePath '{path}' -Access {access} -PassThru
        Start-Sleep -Milliseconds 500
        $v = @($i | Get-Volume -ErrorAction SilentlyContinue | Select-Object DriveLetter, FileSystem)
        Write-Output $i.DevicePath
        ConvertTo-Json -Compress -InputObject $v"
    );
    let text = host::powershell(&script)?;
    let mut lines = text.lines();
    let device = lines.next().unwrap_or("").trim().to_owned();
    let json: String = lines.collect::<Vec<_>>().join("\n");
    parse::windows(&json, &device)
}

pub fn detach(target: &str) -> Result<String> {
    let path = Path::new(target);
    let script = if path.is_file() {
        format!(
            "Dismount-DiskImage -ImagePath '{}' | Out-Null",
            quoted(path)?
        )
    } else {
        let letter = target.trim_end_matches('\\').trim_end_matches(':');
        if letter.len() != 1 {
            bail!("give the image file or its drive letter, such as F:");
        }
        format!("Get-Volume -DriveLetter {letter} | Get-DiskImage | Dismount-DiskImage | Out-Null")
    };
    host::powershell(&script)?;
    Ok(target.to_owned())
}
