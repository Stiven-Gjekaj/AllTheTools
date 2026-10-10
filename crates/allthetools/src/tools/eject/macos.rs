//! The host file of `att eject` on macOS: `diskutil eject`, which unmounts
//! each volume of the drive and ejects it.

use anyhow::Result;

use super::parse::Plan;
use crate::host;

pub fn eject(plan: &Plan) -> Result<()> {
    host::output("diskutil", &["eject", &plan.disk.path], None)?;
    Ok(())
}
