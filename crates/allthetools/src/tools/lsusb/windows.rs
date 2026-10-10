//! The host file of `att lsusb` on Windows: the class Win32_PnPEntity of
//! WMI, as PowerShell gives it in JSON.

use anyhow::Result;

use crate::host;

pub fn read() -> Result<String> {
    host::powershell(
        r"ConvertTo-Json -Compress -InputObject @(
            Get-CimInstance Win32_PnPEntity |
            Where-Object { $_.PNPDeviceID -like 'USB\*' } |
            Select-Object PNPDeviceID, Name, Manufacturer
        )",
    )
}
