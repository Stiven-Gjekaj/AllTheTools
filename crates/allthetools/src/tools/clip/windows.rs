//! The host file of `att clip` on Windows: the clipboard API of Win32, with
//! the text format CF_UNICODETEXT.

use std::time::Duration;

use anyhow::{Result, bail};
use windows_sys::Win32::Foundation::GlobalFree;
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    SetClipboardData,
};
use windows_sys::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};
use windows_sys::Win32::System::Ole::CF_UNICODETEXT;

use super::parse;

/// The clipboard, open. It closes when this goes out of scope.
struct Open;

impl Open {
    /// Another program can hold the clipboard for a moment, so try for one
    /// second before giving up.
    fn new() -> Result<Self> {
        for _ in 0..20 {
            // SAFETY: a null owner is allowed.
            #[allow(unsafe_code)]
            if unsafe { OpenClipboard(std::ptr::null_mut()) } != 0 {
                return Ok(Open);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        bail!("another program holds the clipboard. Try again")
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        // SAFETY: this process opened the clipboard.
        #[allow(unsafe_code)]
        unsafe {
            CloseClipboard();
        }
    }
}

pub fn copy(text: &str) -> Result<()> {
    let units = parse::utf16(text);
    let bytes = units.len() * 2;
    let _open = Open::new()?;
    // SAFETY: each handle and pointer is checked before use. The memory
    // belongs to the clipboard after SetClipboardData succeeds, and to this
    // process until then.
    #[allow(unsafe_code)]
    unsafe {
        EmptyClipboard();
        let memory = GlobalAlloc(GMEM_MOVEABLE, bytes);
        if memory.is_null() {
            bail!("Windows gave no memory for the clipboard");
        }
        let target = GlobalLock(memory) as *mut u16;
        if target.is_null() {
            GlobalFree(memory);
            bail!("Windows gave no memory for the clipboard");
        }
        std::ptr::copy_nonoverlapping(units.as_ptr(), target, units.len());
        GlobalUnlock(memory);
        if SetClipboardData(u32::from(CF_UNICODETEXT), memory).is_null() {
            GlobalFree(memory);
            bail!("Windows refused the text for the clipboard");
        }
    }
    Ok(())
}

pub fn paste() -> Result<String> {
    let _open = Open::new()?;
    // SAFETY: the handle belongs to the clipboard, which stays open while
    // this reads it. The read stays inside the size of the memory.
    #[allow(unsafe_code)]
    unsafe {
        if IsClipboardFormatAvailable(u32::from(CF_UNICODETEXT)) == 0 {
            return Ok(String::new());
        }
        let memory = GetClipboardData(u32::from(CF_UNICODETEXT));
        if memory.is_null() {
            return Ok(String::new());
        }
        let source = GlobalLock(memory) as *const u16;
        if source.is_null() {
            bail!("Windows did not give the text of the clipboard");
        }
        let units = std::slice::from_raw_parts(source, GlobalSize(memory) / 2);
        let text = parse::from_utf16(units);
        GlobalUnlock(memory);
        Ok(text)
    }
}
