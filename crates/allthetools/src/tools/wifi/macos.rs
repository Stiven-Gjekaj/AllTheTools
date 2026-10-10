//! The host file of `att wifi` on macOS: a scan of CoreWLAN, through the
//! Objective-C runtime.
//!
//! macOS removed the `airport` command. CoreWLAN is the framework that
//! replaces it. It gives the signal, the channel and the security of each
//! network to any program, and the name and the BSSID only to a program
//! that has permission for the location.

use std::ffi::{CStr, c_char, c_void};

use anyhow::{Result, bail};

use super::parse::{self, Network};

type Id = *mut c_void;
type Sel = *mut c_void;

#[allow(unsafe_code)]
#[link(name = "objc")]
unsafe extern "C" {
    fn objc_getClass(name: *const c_char) -> Id;
    fn sel_registerName(name: *const c_char) -> Sel;
    fn objc_msgSend();
    fn objc_autoreleasePoolPush() -> *mut c_void;
    fn objc_autoreleasePoolPop(pool: *mut c_void);
}

// The framework must be loaded for objc_getClass to find its classes.
#[allow(unsafe_code)]
#[link(name = "CoreWLAN", kind = "framework")]
unsafe extern "C" {}

#[allow(unsafe_code)]
#[link(name = "Foundation", kind = "framework")]
unsafe extern "C" {}

fn sel(name: &CStr) -> Sel {
    // SAFETY: the name is a C string. The runtime keeps each selector.
    #[allow(unsafe_code)]
    unsafe {
        sel_registerName(name.as_ptr())
    }
}

/// Send a message that takes no argument and gives an object.
fn object(receiver: Id, name: &CStr) -> Id {
    if receiver.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: objc_msgSend takes the types of the method that it calls.
    // Each method sent here takes no argument and gives an object.
    #[allow(unsafe_code)]
    unsafe {
        let send: extern "C" fn(Id, Sel) -> Id =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        send(receiver, sel(name))
    }
}

/// Send a message that takes no argument and gives an integer.
fn integer(receiver: Id, name: &CStr) -> Option<i64> {
    if receiver.is_null() {
        return None;
    }
    // SAFETY: as in object(). Each method sent here gives an NSInteger.
    #[allow(unsafe_code)]
    unsafe {
        let send: extern "C" fn(Id, Sel) -> i64 =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        Some(send(receiver, sel(name)))
    }
}

/// The text of an NSString, or `None` for nil.
fn text(string: Id) -> Option<String> {
    if string.is_null() {
        return None;
    }
    // SAFETY: UTF8String gives a C string that lives as long as the pool.
    #[allow(unsafe_code)]
    unsafe {
        let send: extern "C" fn(Id, Sel) -> *const c_char =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        let pointer = send(string, sel(c"UTF8String"));
        if pointer.is_null() {
            return None;
        }
        Some(CStr::from_ptr(pointer).to_string_lossy().into_owned())
    }
}

/// True when the network supports a CWSecurity mode.
fn supports(network: Id, mode: i64) -> bool {
    // SAFETY: supportsSecurity: takes a CWSecurity and gives a BOOL.
    #[allow(unsafe_code)]
    unsafe {
        let send: extern "C" fn(Id, Sel, i64) -> bool =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        send(network, sel(c"supportsSecurity:"), mode)
    }
}

pub fn scan() -> Result<Vec<Network>> {
    // SAFETY: each object that the scan gives is autoreleased, and the pool
    // releases them all at the end.
    #[allow(unsafe_code)]
    let pool = unsafe { objc_autoreleasePoolPush() };
    let result = scan_in_pool();
    // SAFETY: the pool came from objc_autoreleasePoolPush.
    #[allow(unsafe_code)]
    unsafe {
        objc_autoreleasePoolPop(pool)
    };
    result
}

fn scan_in_pool() -> Result<Vec<Network>> {
    // SAFETY: objc_getClass takes a C string, and gives nil for no class.
    #[allow(unsafe_code)]
    let class = unsafe { objc_getClass(c"CWWiFiClient".as_ptr()) };
    if class.is_null() {
        bail!("this Mac has no CoreWLAN framework");
    }
    let client = object(class, c"sharedWiFiClient");
    let interface = object(client, c"interface");
    if interface.is_null() {
        return Ok(Vec::new());
    }

    let mut error: Id = std::ptr::null_mut();
    // SAFETY: scanForNetworksWithName:error: takes an NSString (nil for all
    // networks) and a pointer to an NSError, and gives an NSSet.
    #[allow(unsafe_code)]
    let set = unsafe {
        let send: extern "C" fn(Id, Sel, Id, *mut Id) -> Id =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        send(
            interface,
            sel(c"scanForNetworksWithName:error:"),
            std::ptr::null_mut(),
            &mut error,
        )
    };
    if set.is_null() {
        let reason =
            text(object(error, c"localizedDescription")).unwrap_or_else(|| "no reason".to_owned());
        bail!("the Wi-Fi scan failed: {reason}");
    }

    let array = object(set, c"allObjects");
    let count = integer(array, c"count").unwrap_or(0);
    let mut out = Vec::new();
    for index in 0..count {
        // SAFETY: objectAtIndex: takes an NSUInteger below the count.
        #[allow(unsafe_code)]
        let network = unsafe {
            let send: extern "C" fn(Id, Sel, u64) -> Id =
                std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            send(array, sel(c"objectAtIndex:"), index as u64)
        };
        let channel = object(network, c"wlanChannel");
        let band = match integer(channel, c"channelBand") {
            Some(1) => Some(2.4),
            Some(2) => Some(5.0),
            Some(3) => Some(6.0),
            _ => None,
        };
        let security = parse::APPLE_SECURITY_ORDER
            .iter()
            .find(|mode| supports(network, **mode))
            .map_or("unknown", |mode| parse::apple_security(*mode));
        out.push(Network {
            ssid: text(object(network, c"ssid")),
            bssid: text(object(network, c"bssid")),
            signal_dbm: integer(network, c"rssiValue").map(|v| v as i32),
            channel: integer(channel, c"channelNumber").map(|v| v as u32),
            band_ghz: band,
            security: security.to_owned(),
        });
    }
    Ok(out)
}

/// The sentence for a scan that found networks with no name.
pub const HIDDEN_NAMES: &str = "macOS hides the names of the networks from a program that has no \
    permission for the location. Give that permission to your terminal in System Settings, \
    Privacy and Security, Location Services.";
