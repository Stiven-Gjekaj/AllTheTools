//! The host file of `att lspci` on macOS: the IOPCIDevice entries of the
//! IOKit registry, as `ioreg` gives them in a property list.

use anyhow::Result;

use crate::host;

pub fn read() -> Result<Vec<u8>> {
    host::output(
        "ioreg",
        &["-r", "-c", "IOPCIDevice", "-a", "-w0", "-d1"],
        None,
    )
}
