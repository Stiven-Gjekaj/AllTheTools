//! The host file of `att notify` on macOS: `display notification` of
//! AppleScript, through `osascript`.

use anyhow::Result;

use super::parse;
use crate::host;

pub fn show(title: &str, message: &str) -> Result<()> {
    let mut args = Vec::new();
    for line in parse::APPLESCRIPT {
        args.extend(["-e", line]);
    }
    args.extend([message, title]);
    host::output("osascript", &args, None)?;
    Ok(())
}
