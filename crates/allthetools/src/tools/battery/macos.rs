//! The host file of `att battery` on macOS: the AppleSmartBattery entry of
//! the IOKit registry, as `ioreg` gives it in a property list.

use anyhow::Result;

use crate::host;

pub fn read() -> Result<Vec<u8>> {
    host::output(
        "ioreg",
        &["-r", "-c", "AppleSmartBattery", "-a", "-w0"],
        None,
    )
}
