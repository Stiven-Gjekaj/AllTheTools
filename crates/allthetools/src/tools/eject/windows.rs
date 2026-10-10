//! The host file of `att eject` on Windows.
//!
//! Each volume of the drive is locked and dismounted first, so no program
//! writes to it after the last flush. Then the Plug and Play manager ejects
//! the device that holds the drive, as "Safely Remove Hardware" does. A
//! program that still holds a file on the drive makes Windows refuse, and
//! Windows names that program.

use std::ffi::c_void;
use std::time::Duration;

use anyhow::{Result, bail};
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
    CM_Get_Parent, CM_Request_Device_EjectW, CR_SUCCESS, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT,
    PNP_VETO_TYPE, SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W, SP_DEVINFO_DATA,
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
    SetupDiGetDeviceInterfaceDetailW,
};
use windows_sys::Win32::Foundation::{
    CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::IO::DeviceIoControl;
use windows_sys::Win32::System::Ioctl::{
    FSCTL_DISMOUNT_VOLUME, FSCTL_LOCK_VOLUME, GUID_DEVINTERFACE_DISK,
    IOCTL_STORAGE_GET_DEVICE_NUMBER, STORAGE_DEVICE_NUMBER,
};

use super::parse::Plan;

struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle came from CreateFileW and closes one time.
        #[allow(unsafe_code)]
        unsafe {
            CloseHandle(self.0);
        }
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn open(path: &str, access: u32) -> Option<Handle> {
    let path = wide(path);
    // SAFETY: the path ends with a null. The handle is checked.
    #[allow(unsafe_code)]
    let handle = unsafe {
        CreateFileW(
            path.as_ptr(),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    };
    (handle != INVALID_HANDLE_VALUE).then_some(Handle(handle))
}

fn control(handle: &Handle, code: u32, output: *mut c_void, size: u32) -> bool {
    let mut returned = 0u32;
    // SAFETY: the output buffer holds `size` bytes, or is null with size 0.
    #[allow(unsafe_code)]
    unsafe {
        DeviceIoControl(
            handle.0,
            code,
            std::ptr::null(),
            0,
            output,
            size,
            &mut returned,
            std::ptr::null_mut(),
        ) != 0
    }
}

pub fn eject(plan: &Plan) -> Result<()> {
    // Lock and dismount each volume, and keep the handles open until the
    // eject is done.
    let mut volumes = Vec::new();
    for (_, mount) in &plan.mounted {
        let letter = mount.trim_end_matches('\\').trim_end_matches(':');
        let Some(volume) = open(&format!("\\\\.\\{letter}:"), GENERIC_READ | GENERIC_WRITE) else {
            bail!("could not open the volume {letter}:");
        };
        let mut locked = false;
        for _ in 0..10 {
            if control(&volume, FSCTL_LOCK_VOLUME, std::ptr::null_mut(), 0) {
                locked = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        if !locked {
            bail!(
                "a program holds a file on {letter}:. Close it, and try again. att whoholds {letter}: names it"
            );
        }
        control(&volume, FSCTL_DISMOUNT_VOLUME, std::ptr::null_mut(), 0);
        volumes.push(volume);
    }

    let number: u32 = plan.disk.id.parse()?;
    let instance = disk_instance(number)?;
    let mut parent = 0u32;
    let mut veto: PNP_VETO_TYPE = 0;
    let mut name = [0u16; 260];
    // SAFETY: each output points to memory of this function, of the size
    // that the call receives.
    #[allow(unsafe_code)]
    unsafe {
        let target = if CM_Get_Parent(&mut parent, instance, 0) == CR_SUCCESS {
            parent
        } else {
            instance
        };
        let result =
            CM_Request_Device_EjectW(target, &mut veto, name.as_mut_ptr(), name.len() as u32, 0);
        if result != CR_SUCCESS || veto != 0 {
            let end = name.iter().position(|c| *c == 0).unwrap_or(name.len());
            let who = String::from_utf16_lossy(&name[..end]);
            bail!(
                "Windows refused to eject the drive (veto {veto}, {who}). Close each program that uses it, and try again"
            );
        }
    }
    drop(volumes);
    Ok(())
}

/// The device instance of PhysicalDrive<number>, from the disk interfaces of
/// SetupAPI.
fn disk_instance(number: u32) -> Result<u32> {
    // SAFETY: the device information set is destroyed one time. Each
    // structure has its size set before the call that fills it, and the
    // detail buffer has the size that the first call asks for.
    #[allow(unsafe_code)]
    unsafe {
        let set = SetupDiGetClassDevsW(
            &GUID_DEVINTERFACE_DISK,
            std::ptr::null(),
            std::ptr::null_mut(),
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        );
        if set == INVALID_HANDLE_VALUE as isize {
            bail!("Windows gave no list of disks");
        }
        let mut found = None;
        let mut index = 0;
        loop {
            let mut interface: SP_DEVICE_INTERFACE_DATA = std::mem::zeroed();
            interface.cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32;
            if SetupDiEnumDeviceInterfaces(
                set,
                std::ptr::null(),
                &GUID_DEVINTERFACE_DISK,
                index,
                &mut interface,
            ) == 0
            {
                break;
            }
            index += 1;
            let mut size = 0u32;
            SetupDiGetDeviceInterfaceDetailW(
                set,
                &interface,
                std::ptr::null_mut(),
                0,
                &mut size,
                std::ptr::null_mut(),
            );
            if size == 0 {
                continue;
            }
            let mut buffer = vec![0u32; (size as usize).div_ceil(4)];
            let detail = buffer
                .as_mut_ptr()
                .cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
            (*detail).cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
            let mut info: SP_DEVINFO_DATA = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<SP_DEVINFO_DATA>() as u32;
            if SetupDiGetDeviceInterfaceDetailW(
                set,
                &interface,
                detail,
                size,
                std::ptr::null_mut(),
                &mut info,
            ) == 0
            {
                continue;
            }
            let path_start = std::ptr::addr_of!((*detail).DevicePath).cast::<u16>();
            let mut length = 0;
            while *path_start.add(length) != 0 {
                length += 1;
            }
            let path = String::from_utf16_lossy(std::slice::from_raw_parts(path_start, length));
            let Some(device) = open(&path, 0) else {
                continue;
            };
            let mut device_number: STORAGE_DEVICE_NUMBER = std::mem::zeroed();
            if control(
                &device,
                IOCTL_STORAGE_GET_DEVICE_NUMBER,
                (&mut device_number as *mut STORAGE_DEVICE_NUMBER).cast(),
                std::mem::size_of::<STORAGE_DEVICE_NUMBER>() as u32,
            ) && device_number.DeviceNumber == number
            {
                found = Some(info.DevInst);
                break;
            }
        }
        SetupDiDestroyDeviceInfoList(set);
        match found {
            Some(instance) => Ok(instance),
            None => bail!("Windows has no device for PhysicalDrive{number}"),
        }
    }
}
