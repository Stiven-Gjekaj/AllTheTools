//! `att lsusb`: list the USB devices.
//!
//! Linux has `lsusb`. macOS has `system_profiler` and `ioreg`, and Windows
//! has Device Manager, and none of them gives the list of `lsusb`. This tool
//! gives that list on each host.

use anyhow::Result;

pub mod parse;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

use parse::Device;

/// List the USB devices.
#[derive(Debug, clap::Args)]
pub struct Args {
    /// Show the speed and the serial number in a table.
    #[arg(short, long)]
    pub long: bool,

    /// Print JSON for a script, and not text for a person.
    #[arg(long, conflicts_with = "long")]
    pub json: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let devices = devices()?;
    if args.json {
        println!("{}", serde_json::to_string(&devices)?);
        return Ok(0);
    }
    if devices.is_empty() {
        println!("This computer reports no USB device.");
        return Ok(0);
    }
    if args.long {
        let rows: Vec<Vec<String>> = devices.iter().map(row).collect();
        crate::table::print(&["BUS", "DEV", "ID", "SPEED", "SERIAL", "NAME"], &rows);
    } else {
        for device in &devices {
            println!("{}", device.line());
        }
    }
    Ok(0)
}

fn row(device: &Device) -> Vec<String> {
    let number = |n: Option<u32>| n.map_or("-".to_owned(), |n| format!("{n:03}"));
    let name = [&device.vendor, &device.product]
        .into_iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    vec![
        number(device.bus),
        number(device.address),
        format!("{:04x}:{:04x}", device.vendor_id, device.product_id),
        device.speed_mbps.map_or("-".to_owned(), parse::speed),
        device.serial.clone().unwrap_or_else(|| "-".to_owned()),
        name,
    ]
}

#[cfg(target_os = "linux")]
fn devices() -> Result<Vec<Device>> {
    Ok(parse::linux(&linux::read()?))
}

#[cfg(target_os = "macos")]
fn devices() -> Result<Vec<Device>> {
    parse::macos(&macos::read()?)
}

#[cfg(windows)]
fn devices() -> Result<Vec<Device>> {
    parse::windows(&windows::read()?)
}
