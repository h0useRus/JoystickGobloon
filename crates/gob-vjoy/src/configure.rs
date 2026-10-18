//! Changes vJoy's configuration without vJoyConfig: device descriptors go to the registry
//! (`descriptor::reg_key`), then the vJoy device node is restarted so the driver hands the new
//! descriptor to HIDCLASS (it re-reads the registry on every descriptor request, see
//! `UpdateCollections` in ref/vJoy driver/sys/hid.c). Needs administrator rights: the app runs
//! itself elevated with [`Change::args`] and calls [`apply`] there.

use crate::config::DeviceConfig;

/// One change, with the same arguments vJoyConfig takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Create or replace a device: `<id> -f -a X Y ... -b 32 -p 1`.
    Set(DeviceConfig),
    /// `-d <id>`
    Delete(u32),
    /// `enable on|off` - switches the whole vJoy driver.
    Enable(bool),
}

impl Change {
    pub fn args(&self) -> Vec<String> {
        match self {
            Self::Set(c) => c.create_args(),
            Self::Delete(id) => vec!["-d".into(), id.to_string()],
            Self::Enable(on) => vec!["enable".into(), if *on { "on" } else { "off" }.into()],
        }
    }

    pub fn from_args(args: &[String]) -> Option<Self> {
        match args {
            [d, id] if d == "-d" => {
                let id = id.parse().ok()?;
                (1..=crate::MAX_DEVICES)
                    .contains(&id)
                    .then_some(Self::Delete(id))
            }
            [e, on] if e.eq_ignore_ascii_case("enable") => match on.as_str() {
                "on" => Some(Self::Enable(true)),
                "off" => Some(Self::Enable(false)),
                _ => None,
            },
            _ => {
                let c = DeviceConfig::parse(&format!("vJoyConfig {}", args.join(" ")))?;
                (1..=crate::MAX_DEVICES)
                    .contains(&c.id)
                    .then_some(Self::Set(c))
            }
        }
    }
}

/// Exit code of the elevated process when [`apply`] returns [`Outcome::RebootNeeded`].
pub const EXIT_REBOOT: u32 = 3;

/// What [`apply`] left to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Done,
    /// The device could not restart now (a handle is still open); Windows wants a reboot.
    RebootNeeded,
}

/// Applies a change (administrator rights needed).
pub fn apply(change: &Change) -> Result<Outcome, String> {
    #[cfg(windows)]
    {
        use windows::Win32::Devices::DeviceAndDriverInstallation::{
            DICS_DISABLE, DICS_ENABLE, DICS_PROPCHANGE,
        };
        let state = match change {
            Change::Set(c) => {
                let desc = c.descriptor();
                let key = crate::descriptor::reg_key(c.id);
                sys::set_binary(&key, crate::descriptor::REG_DESC, &desc)?;
                sys::set_dword(&key, crate::descriptor::REG_DESC_SIZE, desc.len() as u32)?;
                DICS_PROPCHANGE
            }
            Change::Delete(id) => {
                sys::delete_key(&crate::descriptor::reg_key(*id))?;
                DICS_PROPCHANGE
            }
            Change::Enable(true) => DICS_ENABLE,
            Change::Enable(false) => DICS_DISABLE,
        };
        sys::change_state(state)
    }
    #[cfg(not(windows))]
    {
        let _ = change;
        Err("vJoy needs Windows".into())
    }
}

#[cfg(windows)]
mod sys {
    use super::Outcome;
    use windows::core::HSTRING;
    use windows::Win32::Devices::DeviceAndDriverInstallation::{
        SetupDiCallClassInstaller, SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo,
        SetupDiGetClassDevsW, SetupDiGetDeviceInstallParamsW, SetupDiGetDeviceRegistryPropertyW,
        SetupDiSetClassInstallParamsW, DICS_FLAG_CONFIGSPECIFIC, DIF_PROPERTYCHANGE,
        DIGCF_ALLCLASSES, DIGCF_PRESENT, DI_NEEDREBOOT, DI_NEEDRESTART, SETUP_DI_STATE_CHANGE,
        SPDRP_HARDWAREID, SP_CLASSINSTALL_HEADER, SP_DEVINFO_DATA, SP_DEVINSTALL_PARAMS_W,
        SP_PROPCHANGE_PARAMS,
    };
    use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR};
    use windows::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteKeyW, RegSetValueExW, HKEY, HKEY_LOCAL_MACHINE,
        KEY_SET_VALUE, REG_BINARY, REG_DWORD, REG_OPTION_NON_VOLATILE, REG_VALUE_TYPE,
    };

    /// Hardware id of the vJoy device node (`root\VID_1234&PID_BEAD&REV_xxxx`).
    const HWID: &str = r"root\VID_1234&PID_BEAD";

    fn check(r: WIN32_ERROR, what: &str) -> Result<(), String> {
        if r == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(format!("{what}: {}", r.to_hresult().message()))
        }
    }

    fn set_value(key: &str, name: &str, kind: REG_VALUE_TYPE, data: &[u8]) -> Result<(), String> {
        let what = format!(r"HKLM\{key}\{name}");
        let mut hkey = HKEY::default();
        // SAFETY: the key handle is closed below.
        unsafe {
            check(
                RegCreateKeyExW(
                    HKEY_LOCAL_MACHINE,
                    &HSTRING::from(key),
                    None,
                    None,
                    REG_OPTION_NON_VOLATILE,
                    KEY_SET_VALUE,
                    None,
                    &mut hkey,
                    None,
                ),
                &what,
            )?;
            let r = RegSetValueExW(hkey, &HSTRING::from(name), None, kind, Some(data));
            let _ = RegCloseKey(hkey);
            check(r, &what)
        }
    }

    pub fn set_binary(key: &str, name: &str, data: &[u8]) -> Result<(), String> {
        set_value(key, name, REG_BINARY, data)
    }

    pub fn set_dword(key: &str, name: &str, value: u32) -> Result<(), String> {
        set_value(key, name, REG_DWORD, &value.to_le_bytes())
    }

    /// Deletes a key without subkeys; a missing key is fine.
    pub fn delete_key(key: &str) -> Result<(), String> {
        // SAFETY: plain call with an owned string.
        let r = unsafe { RegDeleteKeyW(HKEY_LOCAL_MACHINE, &HSTRING::from(key)) };
        if r == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        check(r, &format!(r"HKLM\{key}"))
    }

    /// Restarts, enables or disables the vJoy device node, like `devcon restart`.
    pub fn change_state(state: SETUP_DI_STATE_CHANGE) -> Result<Outcome, String> {
        // SAFETY: the device info set is destroyed before returning; buffers outlive the calls.
        unsafe {
            let set = SetupDiGetClassDevsW(None, None, None, DIGCF_ALLCLASSES | DIGCF_PRESENT)
                .map_err(|e| e.message())?;
            let mut found = None;
            let mut info = SP_DEVINFO_DATA {
                cbSize: size_of::<SP_DEVINFO_DATA>() as u32,
                ..Default::default()
            };
            let mut i = 0;
            while SetupDiEnumDeviceInfo(set, i, &mut info).is_ok() {
                i += 1;
                let mut buf = [0u8; 1024];
                if SetupDiGetDeviceRegistryPropertyW(
                    set,
                    &info,
                    SPDRP_HARDWAREID,
                    None,
                    Some(&mut buf),
                    None,
                )
                .is_err()
                {
                    continue;
                }
                let wide: Vec<u16> = buf
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| u16::from_le_bytes(*c))
                    .take_while(|c| *c != 0)
                    .collect();
                let id = String::from_utf16_lossy(&wide);
                if id
                    .to_ascii_lowercase()
                    .starts_with(&HWID.to_ascii_lowercase())
                {
                    found = Some(info);
                    break;
                }
            }
            let result = match found {
                None => Err("vJoy device not found".to_string()),
                Some(info) => {
                    let params = SP_PROPCHANGE_PARAMS {
                        ClassInstallHeader: SP_CLASSINSTALL_HEADER {
                            cbSize: size_of::<SP_CLASSINSTALL_HEADER>() as u32,
                            InstallFunction: DIF_PROPERTYCHANGE,
                        },
                        StateChange: state,
                        Scope: DICS_FLAG_CONFIGSPECIFIC,
                        HwProfile: 0,
                    };
                    SetupDiSetClassInstallParamsW(
                        set,
                        Some(&info),
                        Some(&params.ClassInstallHeader),
                        size_of::<SP_PROPCHANGE_PARAMS>() as u32,
                    )
                    .and_then(|()| SetupDiCallClassInstaller(DIF_PROPERTYCHANGE, set, Some(&info)))
                    .map_err(|e| e.message())
                    .map(|()| {
                        let mut p = SP_DEVINSTALL_PARAMS_W {
                            cbSize: size_of::<SP_DEVINSTALL_PARAMS_W>() as u32,
                            ..Default::default()
                        };
                        let _ = SetupDiGetDeviceInstallParamsW(set, Some(&info), &mut p);
                        if p.Flags.0 & (DI_NEEDREBOOT.0 | DI_NEEDRESTART.0) != 0 {
                            Outcome::RebootNeeded
                        } else {
                            Outcome::Done
                        }
                    })
                }
            };
            let _ = SetupDiDestroyDeviceInfoList(set);
            result
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn args_round_trip() {
        for s in [
            "2 -f -a X Y Z Rx Ry Rz Sl0 Sl1 -b 127 -p 4",
            "5 -f -a X Rz -b 8 -s 2 -e Const Fric",
            "-d 3",
            "enable on",
            "enable off",
        ] {
            let c = Change::from_args(&args(s)).unwrap_or_else(|| panic!("{s}"));
            assert_eq!(c.args().join(" "), s);
        }
        assert_eq!(Change::from_args(&args("-d 17")), None);
        assert_eq!(Change::from_args(&args("0 -f -b 8")), None);
        assert_eq!(Change::from_args(&args("enable maybe")), None);
    }
}
