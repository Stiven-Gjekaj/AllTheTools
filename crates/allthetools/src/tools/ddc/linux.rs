//! The host file of `att ddc` on Linux: the I2C bus of each connector in
//! /sys/class/drm, through /dev/i2c-N.
//!
//! The kernel module i2c-dev makes the /dev/i2c-N files, and a user needs
//! the group of those files, often `i2c`, to open them. ddcutil has the same
//! needs.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::time::Duration;

use anyhow::{Context, Result};

use super::parse::{self, Reply};

/// I2C_SLAVE of linux/i2c-dev.h: the address of the next reads and writes.
const I2C_SLAVE: libc::c_ulong = 0x0703;

pub struct Monitor {
    pub name: String,
    device: String,
}

/// The connected connectors that have an I2C bus.
pub fn monitors() -> Result<Vec<Monitor>> {
    let mut out = Vec::new();
    let entries = match std::fs::read_dir("/sys/class/drm") {
        Ok(entries) => entries,
        Err(_) => return Ok(out),
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let dir = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some((_, connector)) = name.split_once('-') else {
            continue;
        };
        let connected =
            std::fs::read_to_string(dir.join("status")).is_ok_and(|s| s.trim() == "connected");
        if !connected {
            continue;
        }
        // The bus is the link `ddc`, or an i2c-N directory of a DisplayPort
        // connector.
        let bus = std::fs::read_link(dir.join("ddc"))
            .ok()
            .and_then(|l| l.file_name().map(|n| n.to_string_lossy().into_owned()))
            .or_else(|| {
                std::fs::read_dir(&dir).ok()?.flatten().find_map(|e| {
                    let n = e.file_name().to_string_lossy().into_owned();
                    n.starts_with("i2c-").then_some(n)
                })
            });
        let Some(bus) = bus else {
            continue;
        };
        let model = std::fs::read(dir.join("edid"))
            .ok()
            .and_then(|e| parse::edid_name(&e));
        out.push(Monitor {
            name: match model {
                Some(model) => format!("{model} on {connector}"),
                None => connector.to_owned(),
            },
            device: format!("/dev/{bus}"),
        });
    }
    Ok(out)
}

fn open(monitor: &Monitor) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&monitor.device)
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => anyhow::anyhow!(
                "{} does not exist. Load the kernel module with `sudo modprobe i2c-dev`",
                monitor.device
            ),
            std::io::ErrorKind::PermissionDenied => anyhow::anyhow!(
                "the host refused {}. Add your user to the group of that file, often i2c, or run with sudo",
                monitor.device
            ),
            _ => anyhow::Error::new(e).context(format!("could not open {}", monitor.device)),
        })?;
    // SAFETY: I2C_SLAVE takes the address as its argument.
    #[allow(unsafe_code)]
    if unsafe {
        libc::ioctl(
            file.as_raw_fd(),
            I2C_SLAVE,
            libc::c_ulong::from(parse::ADDRESS),
        )
    } != 0
    {
        return Err(std::io::Error::last_os_error())
            .context("the bus refused the address of the monitor");
    }
    Ok(file)
}

pub fn get(monitor: &Monitor, code: u8) -> Result<Reply> {
    let mut file = open(monitor)?;
    file.write_all(&parse::with_host(&parse::get_request(code)))
        .context("the monitor took no request. It may not support DDC/CI, or it has DDC/CI off in its menu")?;
    // DDC/CI gives the monitor 40 ms to answer.
    std::thread::sleep(Duration::from_millis(50));
    let mut reply = [0u8; 11];
    file.read_exact(&mut reply)
        .context("the monitor gave no reply")?;
    parse::reply(&reply, code)
}

pub fn set(monitor: &Monitor, code: u8, value: u16) -> Result<()> {
    let mut file = open(monitor)?;
    file.write_all(&parse::with_host(&parse::set_request(code, value)))
        .context("the monitor took no request")?;
    std::thread::sleep(Duration::from_millis(50));
    Ok(())
}
