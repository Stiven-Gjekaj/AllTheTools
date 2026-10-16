//! The host file of `att edid` on Windows: the `EDID` value in the device
//! key of each monitor that is present, through SetupAPI.

use anyhow::Result;
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    DICS_FLAG_GLOBAL, DIGCF_PRESENT, DIREG_DEV, GUID_DEVCLASS_MONITOR, SP_DEVINFO_DATA,
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo, SetupDiGetClassDevsW,
    SetupDiOpenDevRegKey,
};
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::System::Registry::{KEY_READ, RegCloseKey, RegQueryValueExW};

pub fn read() -> Result<Vec<(Option<String>, Vec<u8>)>> {
    let name: Vec<u16> = "EDID\0".encode_utf16().collect();
    let mut out = Vec::new();
    // SAFETY: each call gets the set that SetupDiGetClassDevsW gave, and a
    // record whose size field is set. The set and each key are closed after
    // use.
    #[allow(unsafe_code)]
    unsafe {
        let set = SetupDiGetClassDevsW(
            &GUID_DEVCLASS_MONITOR,
            std::ptr::null(),
            std::ptr::null_mut(),
            DIGCF_PRESENT,
        );
        if set == INVALID_HANDLE_VALUE as isize {
            return Ok(out);
        }
        let mut index = 0;
        loop {
            let mut info = SP_DEVINFO_DATA {
                cbSize: size_of::<SP_DEVINFO_DATA>() as u32,
                ..std::mem::zeroed()
            };
            if SetupDiEnumDeviceInfo(set, index, &mut info) == 0 {
                break;
            }
            index += 1;
            let key = SetupDiOpenDevRegKey(set, &info, DICS_FLAG_GLOBAL, 0, DIREG_DEV, KEY_READ);
            if std::ptr::eq(key, INVALID_HANDLE_VALUE) || key.is_null() {
                continue;
            }
            let mut buffer = vec![0u8; 1024];
            let mut len = buffer.len() as u32;
            let status = RegQueryValueExW(
                key,
                name.as_ptr(),
                std::ptr::null(),
                std::ptr::null_mut(),
                buffer.as_mut_ptr(),
                &mut len,
            );
            RegCloseKey(key);
            if status == 0 && len > 0 {
                buffer.truncate(len as usize);
                out.push((Some(format!("monitor {index}")), buffer));
            }
        }
        SetupDiDestroyDeviceInfoList(set);
    }
    Ok(out)
}
