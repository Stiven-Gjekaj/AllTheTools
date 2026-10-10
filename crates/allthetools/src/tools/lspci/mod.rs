//! `att lspci`: list the PCI devices.
//!
//! Linux has `lspci`. Windows has none, and the `lspci` of Homebrew needs a
//! kernel extension that Apple silicon does not load. This tool gives the
//! list of `lspci -nn` on each host.

use anyhow::Result;

pub mod parse;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

use parse::Device;

/// List the PCI devices.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Print JSON for a script, and not text for a person.
    #[arg(long)]
    pub json: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let devices = devices()?;
    if args.json {
        println!("{}", serde_json::to_string(&devices)?);
        return Ok(0);
    }
    if devices.is_empty() {
        println!("This computer reports no PCI device.");
        return Ok(0);
    }
    for device in &devices {
        println!("{}", device.line());
    }
    Ok(0)
}

#[cfg(target_os = "linux")]
fn devices() -> Result<Vec<Device>> {
    let mut devices = parse::linux(&linux::read()?);
    if let Some(text) = linux::pci_ids() {
        let wanted = devices.iter().map(|d| (d.vendor_id, d.device_id)).collect();
        let (vendors, names) = parse::pci_ids(&text, &wanted);
        for device in &mut devices {
            device.vendor = vendors.get(&device.vendor_id).cloned();
            device.device = names.get(&(device.vendor_id, device.device_id)).cloned();
        }
    }
    Ok(devices)
}

#[cfg(target_os = "macos")]
fn devices() -> Result<Vec<Device>> {
    parse::macos(&macos::read()?)
}

#[cfg(windows)]
fn devices() -> Result<Vec<Device>> {
    parse::windows(&windows::read()?)
}
