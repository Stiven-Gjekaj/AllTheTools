//! The host file of `att lscpu` on Windows: GetLogicalProcessorInformationEx
//! for the cores and the caches, and the registry key of the first
//! processor for its name and its speed.

use anyhow::{Result, bail};
use windows_sys::Win32::System::Registry::{
    HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegGetValueW,
};
use windows_sys::Win32::System::SystemInformation::{
    GetLogicalProcessorInformationEx, RelationAll,
};

use super::parse::{self, Cpu};

const KEY: &str = r"HARDWARE\DESCRIPTION\System\CentralProcessor\0";

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn registry_text(value: &str) -> Option<String> {
    let mut buffer = vec![0u16; 256];
    let mut len = (buffer.len() * 2) as u32;
    // SAFETY: the key and the value end with a null, and the buffer has the
    // room that len gives.
    #[allow(unsafe_code)]
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            wide(KEY).as_ptr(),
            wide(value).as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut len,
        )
    };
    if status != 0 {
        return None;
    }
    let end = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
    let text = String::from_utf16_lossy(&buffer[..end]).trim().to_owned();
    (!text.is_empty()).then_some(text)
}

fn registry_number(value: &str) -> Option<u32> {
    let mut number = 0u32;
    let mut len = 4u32;
    // SAFETY: the key and the value end with a null, and the number has four
    // bytes.
    #[allow(unsafe_code)]
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            wide(KEY).as_ptr(),
            wide(value).as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&raw mut number).cast(),
            &mut len,
        )
    };
    (status == 0).then_some(number)
}

pub fn read(arch: &str) -> Result<Cpu> {
    let mut len = 0u32;
    // SAFETY: the first call asks for the length only, with no buffer.
    #[allow(unsafe_code)]
    unsafe {
        GetLogicalProcessorInformationEx(RelationAll, std::ptr::null_mut(), &mut len);
    }
    if len == 0 {
        bail!("Windows gave no processor records");
    }
    // The buffer has the alignment of the records, which hold numbers of
    // eight bytes.
    let mut words = vec![0u64; (len as usize).div_ceil(8)];
    // SAFETY: the buffer has room for len bytes.
    #[allow(unsafe_code)]
    let ok = unsafe {
        GetLogicalProcessorInformationEx(RelationAll, words.as_mut_ptr().cast(), &mut len)
    };
    if ok == 0 {
        bail!(
            "Windows gave no processor records: {}",
            std::io::Error::last_os_error()
        );
    }
    let bytes: Vec<u8> = words
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .take(len as usize)
        .collect();
    Ok(parse::windows(
        arch,
        &bytes,
        registry_text("ProcessorNameString"),
        registry_text("VendorIdentifier"),
        registry_number("~MHz"),
    ))
}
