//! The host file of `att trash` on macOS: trashItemAtURL of NSFileManager,
//! which the Finder uses too. The Finder can then put the file back.
//!
//! Foundation has no C interface for this call, so the tool sends the
//! Objective-C messages through objc_msgSend. A CFURL is the same object as
//! an NSURL, so Core Foundation makes the URL.

use std::ffi::{CStr, c_char, c_void};
use std::path::Path;

use anyhow::{Result, bail};
use core_foundation::base::TCFType;
use core_foundation::url::CFURL;

type Id = *mut c_void;
type Sel = *mut c_void;

// The link loads Foundation, which registers the class NSFileManager.
#[allow(unsafe_code)]
#[link(name = "Foundation", kind = "framework")]
unsafe extern "C" {}

#[allow(unsafe_code)]
#[link(name = "objc")]
unsafe extern "C" {
    fn objc_getClass(name: *const c_char) -> Id;
    fn sel_registerName(name: *const c_char) -> Sel;
    fn objc_msgSend();
    fn objc_autoreleasePoolPush() -> *mut c_void;
    fn objc_autoreleasePoolPop(pool: *mut c_void);
}

pub fn trash(path: &Path) -> Result<()> {
    let is_dir = std::fs::symlink_metadata(path)?.is_dir();
    let Some(url) = CFURL::from_path(path, is_dir) else {
        bail!("{} has no URL", path.display());
    };
    // SAFETY: each message goes to an object of the class that takes it,
    // and objc_msgSend is called through the type of that method. The pool
    // frees the objects that the calls give back.
    #[allow(unsafe_code)]
    unsafe {
        let pool = objc_autoreleasePoolPush();
        let send0: unsafe extern "C" fn(Id, Sel) -> Id =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        let send_trash: unsafe extern "C" fn(Id, Sel, Id, *mut Id, *mut Id) -> u8 =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        let send_text: unsafe extern "C" fn(Id, Sel) -> *const c_char =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());

        let manager = send0(
            objc_getClass(c"NSFileManager".as_ptr()),
            sel_registerName(c"defaultManager".as_ptr()),
        );
        let mut error: Id = std::ptr::null_mut();
        let done = send_trash(
            manager,
            sel_registerName(c"trashItemAtURL:resultingItemURL:error:".as_ptr()),
            url.as_concrete_TypeRef() as Id,
            std::ptr::null_mut(),
            &mut error,
        );
        let result = if done != 0 {
            Ok(())
        } else if error.is_null() {
            Err(anyhow::anyhow!("the Finder refused it"))
        } else {
            let text = send0(error, sel_registerName(c"localizedDescription".as_ptr()));
            let chars = send_text(text, sel_registerName(c"UTF8String".as_ptr()));
            let message = if chars.is_null() {
                "the Finder refused it".to_owned()
            } else {
                CStr::from_ptr(chars).to_string_lossy().into_owned()
            };
            Err(anyhow::anyhow!(message))
        };
        objc_autoreleasePoolPop(pool);
        result
    }
}
