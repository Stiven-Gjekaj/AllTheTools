//! The devices of the Plug and Play manager of Windows, which `att lsusb`
//! and `att lspci` share.
//!
//! A [`Row`] holds what each tool reads of a device. On Windows, [`list`]
//! gives the rows of the devices that are present, through SetupAPI. The
//! type has no part of a host, so the parsers and their tests compile on
//! every host.

/// One device, as the PnP manager names it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Row {
    /// The instance ID, such as `USB\VID_0781&PID_5583\4C530001260305117454`.
    pub id: String,
    /// The friendly name, or the description when the device has no
    /// friendly name.
    pub name: Option<String>,
    pub manufacturer: Option<String>,
    /// The compatible IDs, such as `PCI\CC_030000`.
    pub compatible: Vec<String>,
}

/// Read rows from the JSON of Win32_PnPEntity, as PowerShell writes it with
/// `Select-Object PNPDeviceID, Name, Manufacturer, CompatibleID`. The tests
/// keep samples of real hosts in this form.
#[cfg(test)]
pub fn from_json(json: &str) -> anyhow::Result<Vec<Row>> {
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Ids {
        One(String),
        Many(Vec<String>),
    }

    #[derive(Deserialize)]
    struct Entity {
        #[serde(rename = "PNPDeviceID")]
        id: Option<String>,
        #[serde(rename = "Name")]
        name: Option<String>,
        #[serde(rename = "Manufacturer")]
        manufacturer: Option<String>,
        #[serde(rename = "CompatibleID", default)]
        compatible: Option<Ids>,
    }

    if json.trim().is_empty() {
        return Ok(Vec::new());
    }
    let entities: Vec<Entity> = serde_json::from_str(json)?;
    Ok(entities
        .into_iter()
        .filter_map(|e| {
            Some(Row {
                id: e.id?,
                name: e.name,
                manufacturer: e.manufacturer,
                compatible: match e.compatible {
                    Some(Ids::One(one)) => vec![one],
                    Some(Ids::Many(many)) => many,
                    None => Vec::new(),
                },
            })
        })
        .collect())
}

/// The present devices of one enumerator, such as `USB` or `PCI`.
#[cfg(windows)]
pub fn list(enumerator: &str) -> anyhow::Result<Vec<Row>> {
    use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
        DIGCF_ALLCLASSES, DIGCF_PRESENT, SP_DEVINFO_DATA, SPDRP_COMPATIBLEIDS, SPDRP_DEVICEDESC,
        SPDRP_FRIENDLYNAME, SPDRP_MFG, SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo,
        SetupDiGetClassDevsW, SetupDiGetDeviceInstanceIdW, SetupDiGetDeviceRegistryPropertyW,
    };
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;

    /// The strings of a value of the registry: one string, or a list of
    /// strings that ends with an empty one.
    fn strings(buffer: &[u16]) -> Vec<String> {
        buffer
            .split(|c| *c == 0)
            .take_while(|s| !s.is_empty())
            .map(String::from_utf16_lossy)
            .collect()
    }

    let wide: Vec<u16> = enumerator
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut rows = Vec::new();
    // SAFETY: each call gets the set that SetupDiGetClassDevsW gave, a
    // record whose size field is set, and buffers with the room that their
    // sizes give. The set is destroyed at the end.
    #[allow(unsafe_code)]
    unsafe {
        let set = SetupDiGetClassDevsW(
            std::ptr::null(),
            wide.as_ptr(),
            std::ptr::null_mut(),
            DIGCF_ALLCLASSES | DIGCF_PRESENT,
        );
        if set == INVALID_HANDLE_VALUE as isize {
            anyhow::bail!(
                "Windows gave no list of devices: {}",
                std::io::Error::last_os_error()
            );
        }
        let property = |info: &SP_DEVINFO_DATA, key: u32| -> Vec<String> {
            let mut buffer = vec![0u16; 2048];
            let ok = SetupDiGetDeviceRegistryPropertyW(
                set,
                info,
                key,
                std::ptr::null_mut(),
                buffer.as_mut_ptr().cast(),
                (buffer.len() * 2) as u32,
                std::ptr::null_mut(),
            );
            if ok == 0 {
                Vec::new()
            } else {
                strings(&buffer)
            }
        };
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
            let mut id = vec![0u16; 1024];
            if SetupDiGetDeviceInstanceIdW(
                set,
                &info,
                id.as_mut_ptr(),
                id.len() as u32,
                std::ptr::null_mut(),
            ) == 0
            {
                continue;
            }
            let Some(id) = strings(&id).into_iter().next() else {
                continue;
            };
            let name = property(&info, SPDRP_FRIENDLYNAME)
                .into_iter()
                .next()
                .or_else(|| property(&info, SPDRP_DEVICEDESC).into_iter().next());
            rows.push(Row {
                id,
                name,
                manufacturer: property(&info, SPDRP_MFG).into_iter().next(),
                compatible: property(&info, SPDRP_COMPATIBLEIDS),
            });
        }
        SetupDiDestroyDeviceInfoList(set);
    }
    Ok(rows)
}
