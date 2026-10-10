//! The host file of `att ddc` on macOS with Apple silicon: IOAVService,
//! which sends I2C messages to an external display. m1ddc and BetterDisplay
//! use the same calls. Apple does not publish them, so they are declared
//! here.
//!
//! An Intel Mac has no IOAVService, and the built-in display of a Mac has no
//! DDC/CI.

use std::ffi::c_void;
use std::time::Duration;

use anyhow::{Result, bail};
use core_foundation::base::{CFType, TCFType};
use core_foundation::dictionary::CFDictionaryRef;
use core_foundation::string::CFString;

use super::parse::{self, Reply};

type IoObject = u32;

#[allow(unsafe_code)]
#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    static kIOMainPortDefault: u32;
    fn IOServiceMatching(name: *const std::ffi::c_char) -> CFDictionaryRef;
    fn IOServiceGetMatchingServices(
        port: u32,
        matching: CFDictionaryRef,
        iterator: *mut IoObject,
    ) -> i32;
    fn IOIteratorNext(iterator: IoObject) -> IoObject;
    fn IOObjectRelease(object: IoObject) -> i32;
    fn IORegistryEntryCreateCFProperty(
        entry: IoObject,
        key: core_foundation::string::CFStringRef,
        allocator: *const c_void,
        options: u32,
    ) -> core_foundation::base::CFTypeRef;
    fn IOAVServiceCreateWithService(allocator: *const c_void, service: IoObject) -> *mut c_void;
    fn IOAVServiceReadI2C(
        service: *mut c_void,
        chip: u32,
        offset: u32,
        output: *mut u8,
        size: u32,
    ) -> i32;
    fn IOAVServiceWriteI2C(
        service: *mut c_void,
        chip: u32,
        offset: u32,
        input: *const u8,
        size: u32,
    ) -> i32;
}

pub struct Monitor {
    pub name: String,
    service: *mut c_void,
}

impl Drop for Monitor {
    fn drop(&mut self) {
        // SAFETY: the service came from IOAVServiceCreateWithService.
        #[allow(unsafe_code)]
        unsafe {
            core_foundation::base::CFRelease(self.service as core_foundation::base::CFTypeRef);
        }
    }
}

/// The external displays: the DCPAVServiceProxy entries whose Location is
/// External.
pub fn monitors() -> Result<Vec<Monitor>> {
    let mut out = Vec::new();
    let location = CFString::new("Location");
    // SAFETY: each object from the iterator is released one time. The
    // matching dictionary goes to IOServiceGetMatchingServices, which takes
    // it.
    #[allow(unsafe_code)]
    unsafe {
        let mut iterator = 0;
        if IOServiceGetMatchingServices(
            kIOMainPortDefault,
            IOServiceMatching(c"DCPAVServiceProxy".as_ptr()),
            &mut iterator,
        ) != 0
        {
            return Ok(out);
        }
        loop {
            let service = IOIteratorNext(iterator);
            if service == 0 {
                break;
            }
            let value = IORegistryEntryCreateCFProperty(
                service,
                location.as_concrete_TypeRef(),
                std::ptr::null(),
                0,
            );
            let external = !value.is_null()
                && CFType::wrap_under_create_rule(value)
                    .downcast::<CFString>()
                    .is_some_and(|s| s == CFString::from_static_string("External"));
            if external {
                let av = IOAVServiceCreateWithService(std::ptr::null(), service);
                if !av.is_null() {
                    out.push(Monitor {
                        name: format!("external display {}", out.len() + 1),
                        service: av,
                    });
                }
            }
            IOObjectRelease(service);
        }
        IOObjectRelease(iterator);
    }
    Ok(out)
}

fn write(monitor: &Monitor, message: &[u8]) -> Result<()> {
    // A display can miss the first message, so it goes two times, as m1ddc
    // sends it.
    let mut status = 0;
    for _ in 0..2 {
        std::thread::sleep(Duration::from_millis(10));
        // SAFETY: the message lives until the call returns, and its length
        // goes with it.
        #[allow(unsafe_code)]
        {
            status = unsafe {
                IOAVServiceWriteI2C(
                    monitor.service,
                    u32::from(parse::ADDRESS),
                    u32::from(parse::HOST),
                    message.as_ptr(),
                    message.len() as u32,
                )
            };
        }
    }
    if status != 0 {
        bail!("the display took no request ({status:#x}). It may have DDC/CI off in its menu");
    }
    Ok(())
}

pub fn get(monitor: &Monitor, code: u8) -> Result<Reply> {
    write(monitor, &parse::get_request(code))?;
    std::thread::sleep(Duration::from_millis(50));
    let mut reply = [0u8; 12];
    // SAFETY: the buffer holds 12 bytes, the size that the call receives.
    #[allow(unsafe_code)]
    let status = unsafe {
        IOAVServiceReadI2C(
            monitor.service,
            u32::from(parse::ADDRESS),
            u32::from(parse::HOST),
            reply.as_mut_ptr(),
            reply.len() as u32,
        )
    };
    if status != 0 {
        bail!("the display gave no reply ({status:#x})");
    }
    parse::reply(&reply, code)
}

pub fn set(monitor: &Monitor, code: u8, value: u16) -> Result<()> {
    write(monitor, &parse::set_request(code, value))
}
