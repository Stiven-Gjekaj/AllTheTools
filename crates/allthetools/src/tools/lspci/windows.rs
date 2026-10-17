//! The host file of `att lspci` on Windows: the present devices of the PCI
//! enumerator of the PnP manager, through SetupAPI.

use anyhow::Result;

use crate::pnp::{self, Row};

pub fn read() -> Result<Vec<Row>> {
    pnp::list("PCI")
}
