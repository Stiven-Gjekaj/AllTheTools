//! The host file of `att mount` on macOS: `hdiutil`, which attaches each
//! kind of image that macOS reads.

use std::path::Path;

use anyhow::{Result, bail};

use super::parse::{self, Attached, Kind};
use crate::host;

pub fn attach(image: &Path, kind: Kind, read_only: bool) -> Result<Vec<Attached>> {
    let path = image.to_string_lossy();
    let mut args = vec!["attach", "-plist"];
    if read_only {
        args.push("-readonly");
    }
    match kind {
        // hdiutil reads a raw image only when it is told the class.
        Kind::Raw => args.extend(["-imagekey", "diskimage-class=CRawDiskImage"]),
        Kind::Vhd | Kind::Vhdx => bail!("macOS does not attach a VHD or a VHDX image"),
        Kind::Iso | Kind::Dmg => {}
    }
    args.push(&path);
    parse::hdiutil(&host::output("hdiutil", &args, None)?)
}

/// Detach an image by its device, by a mount point, or by its file.
pub fn detach(target: &str) -> Result<String> {
    let path = Path::new(target);
    let device = if path.is_file() {
        // hdiutil keeps the path that it got, and /var is a link to
        // /private/var. So both sides of the test are the full path.
        let full = std::fs::canonicalize(path)?;
        let info = host::output("hdiutil", &["info", "-plist"], None)?;
        let found = parse::hdiutil_images(&info)?
            .into_iter()
            .find(|(image, _)| {
                std::fs::canonicalize(image).ok().as_deref() == Some(full.as_path())
            });
        match found {
            Some((_, device)) => device,
            None => bail!("{target} is not attached"),
        }
    } else {
        target.to_owned()
    };
    host::output("hdiutil", &["detach", &device], None)?;
    Ok(device)
}
