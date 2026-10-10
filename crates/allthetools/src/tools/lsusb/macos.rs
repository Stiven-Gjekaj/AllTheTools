//! The host file of `att lsusb` on macOS: the IOUSB plane of the IOKit
//! registry, as `ioreg` gives it in a property list.

use anyhow::Result;

use crate::host;

pub fn read() -> Result<Vec<u8>> {
    host::output(
        "ioreg",
        &[
            "-p",
            "IOUSB",
            "-l",
            "-w0",
            "-r",
            "-c",
            "IOUSBHostDevice",
            "-a",
        ],
        None,
    )
}
