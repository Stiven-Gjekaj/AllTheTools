//! Read the values of the macOS kernel by name, as `sysctl` shows them.
//!
//! A tool reads a number of a fixed size with [`number`], and a string with
//! [`text`].

use std::ffi::{CStr, CString};

use anyhow::{Result, bail};

/// Read the bytes of a value into `value`, and give their count.
fn raw(name: &str, value: *mut libc::c_void, len: usize) -> Result<usize> {
    let c_name = CString::new(name)?;
    let mut len = len;
    // SAFETY: the name ends with a null, and the value has room for len
    // bytes.
    #[allow(unsafe_code)]
    let status =
        unsafe { libc::sysctlbyname(c_name.as_ptr(), value, &mut len, std::ptr::null_mut(), 0) };
    if status != 0 {
        bail!("the host gave no value for {name}");
    }
    Ok(len)
}

/// Read a value of a fixed size, such as a number or a record of numbers.
/// The type must be one that all zeros make valid.
pub fn number<T: Copy + Default>(name: &str) -> Result<T> {
    let mut value = T::default();
    let len = raw(name, (&raw mut value).cast(), size_of::<T>())?;
    if len != size_of::<T>() {
        bail!(
            "the host gave {len} bytes for {name}, and {} were expected",
            size_of::<T>()
        );
    }
    Ok(value)
}

/// Read a record that has no Default, such as a struct of libc. All zeros
/// must be a valid value of the type.
pub fn record<T: Copy>(name: &str) -> Result<T> {
    // SAFETY: the caller names a type of plain numbers, for which all zeros
    // is a valid value.
    #[allow(unsafe_code)]
    let mut value: T = unsafe { std::mem::zeroed() };
    let len = raw(name, (&raw mut value).cast(), size_of::<T>())?;
    if len != size_of::<T>() {
        bail!("the host gave {len} bytes for {name}");
    }
    Ok(value)
}

/// Read a string value.
pub fn text(name: &str) -> Result<String> {
    let mut buffer = vec![0u8; 1024];
    let len = raw(name, buffer.as_mut_ptr().cast(), buffer.len())?;
    buffer.truncate(len);
    let text = CStr::from_bytes_until_nul(&buffer)
        .map(|c| c.to_string_lossy().into_owned())
        .unwrap_or_else(|_| String::from_utf8_lossy(&buffer).into_owned());
    Ok(text.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::{number, text};

    // The test reads values that each Mac has, and changes nothing.
    #[test]
    fn a_mac_gives_its_memory_and_its_name() {
        assert!(number::<u64>("hw.memsize").unwrap() > 0);
        assert!(!text("kern.ostype").unwrap().is_empty());
        assert!(number::<u64>("no.such.value").is_err());
    }
}
