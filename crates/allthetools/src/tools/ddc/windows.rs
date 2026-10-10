//! The host file of `att ddc` on Windows: the Monitor Configuration API of
//! dxva2, which sends the DDC/CI messages for a program.

use anyhow::{Result, bail};
use windows_sys::Win32::Devices::Display::{
    DestroyPhysicalMonitor, GetNumberOfPhysicalMonitorsFromHMONITOR,
    GetPhysicalMonitorsFromHMONITOR, GetVCPFeatureAndVCPFeatureReply, PHYSICAL_MONITOR,
    SetVCPFeature,
};
use windows_sys::Win32::Foundation::{LPARAM, RECT};
use windows_sys::Win32::Graphics::Gdi::{EnumDisplayMonitors, HDC, HMONITOR};

use super::parse::Reply;

pub struct Monitor {
    pub name: String,
    handle: PHYSICAL_MONITOR,
}

impl Drop for Monitor {
    fn drop(&mut self) {
        // SAFETY: the handle came from GetPhysicalMonitorsFromHMONITOR.
        #[allow(unsafe_code)]
        unsafe {
            DestroyPhysicalMonitor(self.handle.hPhysicalMonitor);
        }
    }
}

#[allow(unsafe_code)]
unsafe extern "system" fn collect(monitor: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> i32 {
    // SAFETY: data is the address of the vector that monitors() passes.
    #[allow(unsafe_code)]
    let list = unsafe { &mut *(data as *mut Vec<HMONITOR>) };
    list.push(monitor);
    1
}

pub fn monitors() -> Result<Vec<Monitor>> {
    let mut handles: Vec<HMONITOR> = Vec::new();
    let mut out = Vec::new();
    // SAFETY: the callback gets the address of handles, which lives until
    // the call returns. Each physical monitor is destroyed by Monitor.
    #[allow(unsafe_code)]
    unsafe {
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null(),
            Some(collect),
            &mut handles as *mut Vec<HMONITOR> as LPARAM,
        );
        for handle in handles {
            let mut count = 0u32;
            if GetNumberOfPhysicalMonitorsFromHMONITOR(handle, &mut count) == 0 || count == 0 {
                continue;
            }
            let mut physical: Vec<PHYSICAL_MONITOR> = vec![std::mem::zeroed(); count as usize];
            if GetPhysicalMonitorsFromHMONITOR(handle, count, physical.as_mut_ptr()) == 0 {
                continue;
            }
            for monitor in physical {
                // The struct is packed, so the name is copied out first.
                let text = monitor.szPhysicalMonitorDescription;
                let end = text.iter().position(|c| *c == 0).unwrap_or(text.len());
                out.push(Monitor {
                    name: String::from_utf16_lossy(&text[..end]),
                    handle: monitor,
                });
            }
        }
    }
    Ok(out)
}

pub fn get(monitor: &Monitor, code: u8) -> Result<Reply> {
    let mut kind = 0;
    let mut current = 0u32;
    let mut max = 0u32;
    // SAFETY: each output is a variable of this function.
    #[allow(unsafe_code)]
    let ok = unsafe {
        GetVCPFeatureAndVCPFeatureReply(
            monitor.handle.hPhysicalMonitor,
            code,
            &mut kind,
            &mut current,
            &mut max,
        )
    };
    if ok == 0 {
        bail!(
            "the monitor gave no value for {}. It may not support it, or it has DDC/CI off in its menu",
            super::parse::feature_name(code)
        );
    }
    Ok(Reply {
        code,
        current: current as u16,
        max: max as u16,
    })
}

pub fn set(monitor: &Monitor, code: u8, value: u16) -> Result<()> {
    // SAFETY: the handle is a physical monitor of this process.
    #[allow(unsafe_code)]
    let ok = unsafe { SetVCPFeature(monitor.handle.hPhysicalMonitor, code, u32::from(value)) };
    if ok == 0 {
        bail!("the monitor refused the value");
    }
    Ok(())
}
