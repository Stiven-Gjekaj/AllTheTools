//! The host file of `att sensors` on macOS.
//!
//! Apple silicon names its temperature sensors in the HID event system, the
//! source that Activity Monitor and most menu bar tools read. The SMC holds
//! the fans, and on an Intel Mac the temperatures too. Neither has a public
//! API, so the calls are declared here as Apple's own tools use them.

use std::ffi::{CStr, c_void};

use anyhow::{Result, bail};
use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::base::{CFRelease, CFType, CFTypeRef, TCFType};
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::number::CFNumber;
use core_foundation::string::{CFString, CFStringRef};

use super::parse;

type IoObject = u32;
type IoConnect = u32;
type KernReturn = i32;

#[allow(unsafe_code)]
#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    static kIOMainPortDefault: u32;
    fn IOServiceMatching(name: *const std::ffi::c_char) -> CFDictionaryRef;
    fn IOServiceGetMatchingService(port: u32, matching: CFDictionaryRef) -> IoObject;
    fn IOServiceOpen(
        service: IoObject,
        task: u32,
        kind: u32,
        connect: *mut IoConnect,
    ) -> KernReturn;
    fn IOServiceClose(connect: IoConnect) -> KernReturn;
    fn IOObjectRelease(object: IoObject) -> KernReturn;
    fn IOConnectCallStructMethod(
        connect: IoConnect,
        selector: u32,
        input: *const c_void,
        input_size: usize,
        output: *mut c_void,
        output_size: *mut usize,
    ) -> KernReturn;

    fn IOHIDEventSystemClientCreate(allocator: *const c_void) -> *mut c_void;
    fn IOHIDEventSystemClientSetMatching(client: *mut c_void, matching: CFDictionaryRef) -> i32;
    fn IOHIDEventSystemClientCopyServices(client: *mut c_void) -> CFArrayRef;
    fn IOHIDServiceClientCopyProperty(service: *const c_void, key: CFStringRef) -> CFTypeRef;
    fn IOHIDServiceClientCopyEvent(
        service: *const c_void,
        kind: i64,
        a: i32,
        b: i64,
    ) -> *mut c_void;
    fn IOHIDEventGetFloatValue(event: *mut c_void, field: i32) -> f64;
}

#[allow(unsafe_code)]
unsafe extern "C" {
    fn mach_task_self() -> u32;
}

/// kIOHIDEventTypeTemperature.
const HID_TEMPERATURE: i64 = 15;

pub fn read() -> Result<Vec<parse::Reading>> {
    let mut readings = parse::hid_temperatures(&hid_temperatures());
    match Smc::open() {
        Ok(smc) => {
            if readings.is_empty() {
                readings = parse::smc_temperatures(&smc.temperatures());
            }
            readings.extend(smc.fans());
        }
        Err(e) if readings.is_empty() => return Err(e),
        Err(_) => {}
    }
    Ok(readings)
}

/// The temperature sensors of the HID event system: the usage page 0xff00
/// and the usage 5 of Apple.
fn hid_temperatures() -> Vec<(String, f64)> {
    let mut out = Vec::new();
    let matching = CFDictionary::from_CFType_pairs(&[
        (
            CFString::new("PrimaryUsagePage").as_CFType(),
            CFNumber::from(0xff00i32).as_CFType(),
        ),
        (
            CFString::new("PrimaryUsage").as_CFType(),
            CFNumber::from(5i32).as_CFType(),
        ),
    ]);
    let product = CFString::new("Product");
    // SAFETY: each object that a Create or Copy call gives is released one
    // time. The array keeps each service alive while this reads it.
    #[allow(unsafe_code)]
    unsafe {
        let client = IOHIDEventSystemClientCreate(std::ptr::null());
        if client.is_null() {
            return out;
        }
        IOHIDEventSystemClientSetMatching(client, matching.as_concrete_TypeRef());
        let services = IOHIDEventSystemClientCopyServices(client);
        if !services.is_null() {
            let services: CFArray<CFType> = CFArray::wrap_under_create_rule(services);
            for service in services.iter() {
                let service = service.as_CFTypeRef();
                let name = IOHIDServiceClientCopyProperty(service, product.as_concrete_TypeRef());
                if name.is_null() {
                    continue;
                }
                let name = CFType::wrap_under_create_rule(name);
                let Some(name) = name.downcast::<CFString>() else {
                    continue;
                };
                let event = IOHIDServiceClientCopyEvent(service, HID_TEMPERATURE, 0, 0);
                if event.is_null() {
                    continue;
                }
                let value = IOHIDEventGetFloatValue(event, (HID_TEMPERATURE << 16) as i32);
                CFRelease(event as CFTypeRef);
                out.push((name.to_string(), value));
            }
        }
        CFRelease(client as CFTypeRef);
    }
    out
}

/// The parameter block of the SMC user client, as the SMC tools of the
/// community give it. It is 80 bytes.
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Version {
    major: u8,
    minor: u8,
    build: u8,
    reserved: u8,
    release: u16,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Limits {
    version: u16,
    length: u16,
    cpu: u32,
    gpu: u32,
    memory: u32,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct KeyInfo {
    size: u32,
    kind: u32,
    attributes: u8,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct KeyData {
    key: u32,
    version: Version,
    limits: Limits,
    info: KeyInfo,
    result: u8,
    status: u8,
    command: u8,
    data32: u32,
    bytes: [u8; 32],
}

const _: () = assert!(std::mem::size_of::<KeyData>() == 80);

const READ_BYTES: u8 = 5;
const KEY_BY_INDEX: u8 = 8;
const READ_INFO: u8 = 9;
/// The selector of the SMC user client that takes a KeyData.
const HANDLE_EVENT: u32 = 2;

struct Smc(IoConnect);

impl Drop for Smc {
    fn drop(&mut self) {
        // SAFETY: the connection came from IOServiceOpen.
        #[allow(unsafe_code)]
        unsafe {
            IOServiceClose(self.0);
        }
    }
}

fn code(text: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*text)
}

impl Smc {
    fn open() -> Result<Self> {
        let name: &CStr = c"AppleSMC";
        // SAFETY: IOServiceMatching takes a C string, and
        // IOServiceGetMatchingService takes the reference of the dictionary.
        // The service is released after the open.
        #[allow(unsafe_code)]
        unsafe {
            let service =
                IOServiceGetMatchingService(kIOMainPortDefault, IOServiceMatching(name.as_ptr()));
            if service == 0 {
                bail!("this Mac has no SMC service");
            }
            let mut connect = 0;
            let status = IOServiceOpen(service, mach_task_self(), 0, &mut connect);
            IOObjectRelease(service);
            if status != 0 {
                bail!("macOS refused the connection to the SMC ({status:#x})");
            }
            Ok(Smc(connect))
        }
    }

    fn call(&self, input: &KeyData) -> Option<KeyData> {
        let mut output = KeyData::default();
        let mut size = std::mem::size_of::<KeyData>();
        // SAFETY: both blocks are 80 bytes, the size that the call receives.
        #[allow(unsafe_code)]
        let status = unsafe {
            IOConnectCallStructMethod(
                self.0,
                HANDLE_EVENT,
                (input as *const KeyData).cast(),
                std::mem::size_of::<KeyData>(),
                (&mut output as *mut KeyData).cast(),
                &mut size,
            )
        };
        (status == 0 && output.result == 0).then_some(output)
    }

    /// The type and the bytes of a key.
    fn read(&self, key: u32) -> Option<([u8; 4], Vec<u8>)> {
        let info = self.call(&KeyData {
            key,
            command: READ_INFO,
            ..KeyData::default()
        })?;
        let out = self.call(&KeyData {
            key,
            info: info.info,
            command: READ_BYTES,
            ..KeyData::default()
        })?;
        let size = (info.info.size as usize).min(32);
        Some((info.info.kind.to_be_bytes(), out.bytes[..size].to_vec()))
    }

    fn value(&self, key: &str) -> Option<f64> {
        let key: [u8; 4] = key.as_bytes().try_into().ok()?;
        let (kind, bytes) = self.read(code(&key))?;
        parse::smc_value(kind, &bytes)
    }

    fn temperatures(&self) -> Vec<(String, f64)> {
        let mut out = Vec::new();
        let Some(count) = self.value("#KEY") else {
            return out;
        };
        for index in 0..count as u32 {
            let Some(found) = self.call(&KeyData {
                data32: index,
                command: KEY_BY_INDEX,
                ..KeyData::default()
            }) else {
                continue;
            };
            let key = found.key.to_be_bytes();
            if key[0] != b'T' {
                continue;
            }
            if let Some((kind, bytes)) = self.read(found.key)
                && parse::smc_is_temperature(key, kind)
                && let Some(value) = parse::smc_value(kind, &bytes)
            {
                out.push((String::from_utf8_lossy(&key).into_owned(), value));
            }
        }
        out
    }

    fn fans(&self) -> Vec<parse::Reading> {
        let count = self.value("FNum").unwrap_or(0.0) as u32;
        (0..count)
            .filter_map(|i| {
                let [actual, _, most] = parse::smc_fan_keys(i);
                let value = self.value(&actual)?;
                Some(parse::Reading {
                    chip: "SMC".to_owned(),
                    label: format!("fan {}", i + 1),
                    kind: parse::Kind::Fan,
                    value,
                    high: self.value(&most),
                    critical: None,
                })
            })
            .collect()
    }
}
