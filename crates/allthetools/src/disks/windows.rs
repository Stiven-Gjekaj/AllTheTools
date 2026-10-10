//! The host file of the drive list on Windows: Get-Disk, Get-Partition and
//! Get-Volume of Windows Storage Management, in JSON.

use anyhow::Result;

use super::parse::{self, Disk};
use crate::host;

pub fn list() -> Result<Vec<Disk>> {
    let json = host::powershell(
        r"$d = @(Get-Disk | Select-Object Number, FriendlyName, SerialNumber, Size, BusType, IsBoot, IsSystem, IsReadOnly)
        $p = @(Get-Partition -ErrorAction SilentlyContinue | Select-Object DiskNumber, PartitionNumber, Size, DriveLetter)
        $v = @(Get-Volume -ErrorAction SilentlyContinue | Select-Object DriveLetter, FileSystem, FileSystemLabel)
        ConvertTo-Json -Compress -Depth 3 -InputObject @{ disks = $d; partitions = $p; volumes = $v }",
    )?;
    parse::windows(&json)
}
