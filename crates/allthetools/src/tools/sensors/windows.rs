//! The host file of `att sensors` on Windows: the thermal zones of ACPI, from
//! the performance counters, as PowerShell gives them in JSON.
//!
//! Windows has no public API for the temperature of each core or for the
//! fans. A tool that shows them loads a kernel driver of its own. The
//! thermal zones are what Windows itself reads, and a standard user can read
//! them.

use anyhow::Result;

use super::parse::{self, Reading};
use crate::host;

pub fn read() -> Result<Vec<Reading>> {
    let json = host::powershell(
        r"ConvertTo-Json -Compress -InputObject @(
            Get-CimInstance Win32_PerfFormattedData_Counters_ThermalZoneInformation -ErrorAction SilentlyContinue |
            Select-Object Name, Temperature, HighPrecisionTemperature
        )",
    )?;
    parse::windows(&json)
}
