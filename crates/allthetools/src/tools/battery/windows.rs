//! The host file of `att battery` on Windows: the battery classes of WMI, as
//! PowerShell gives them in JSON.
//!
//! A desktop computer has no battery, and some of the classes then give the
//! error "Not supported". Each query ignores its own error, so the result is
//! an empty list.

use anyhow::Result;

use crate::host;

pub fn read() -> Result<String> {
    host::powershell(
        r"$q = { param($c, $n) @(Get-CimInstance -Namespace $n -ClassName $c -ErrorAction SilentlyContinue) }
        ConvertTo-Json -Compress -Depth 3 -InputObject @{
            static = @(& $q BatteryStaticData root/wmi | Select-Object DesignedCapacity, DeviceName, ManufactureName, SerialNumber)
            full = @(& $q BatteryFullChargedCapacity root/wmi | Select-Object FullChargedCapacity)
            cycles = @(& $q BatteryCycleCount root/wmi | Select-Object CycleCount)
            status = @(& $q BatteryStatus root/wmi | Select-Object RemainingCapacity, Charging, Discharging, PowerOnline, Voltage)
            win32 = @(& $q Win32_Battery root/cimv2 | Select-Object EstimatedChargeRemaining, EstimatedRunTime, Name)
        }",
    )
}
