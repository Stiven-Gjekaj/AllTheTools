//! The host file of `att edid` on macOS: the IOKit registry, as `ioreg`
//! gives it in a property list.

use anyhow::Result;

use super::parse;
use crate::host;

pub fn read() -> Result<Vec<(Option<String>, Vec<u8>)>> {
    let mut out = Vec::new();
    // Apple silicon, then an Intel Mac.
    for class in ["IOPortTransportState", "IODisplay"] {
        let plist = host::output("ioreg", &["-r", "-c", class, "-a", "-w0"], None)?;
        for found in parse::from_ioreg(&plist)? {
            if !out.iter().any(|(_, e)| *e == found.1) {
                out.push(found);
            }
        }
    }
    Ok(out)
}
