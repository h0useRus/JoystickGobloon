//! HID report descriptors of vJoy devices, as vJoy 2.1.9 `vJoyConfig` writes them
//! (`CreateHidReportDesc` in ref/vJoy apps/vJoyConf/vJoyConfig/vJoyConfig.cpp), and the way back:
//! a [`DeviceConfig`] read from a descriptor in the registry.

use crate::config::{DeviceConfig, EFFECTS, MAX_BUTTONS, MAX_HATS};
use crate::ffb_desc::FFB_CHUNKS;

/// Driver parameters; device N lives in `DeviceNN`.
pub const REG_PARAMS: &str = r"SYSTEM\CurrentControlSet\services\vjoy\Parameters";
/// Value names as vJoy spells them (sic).
pub const REG_DESC: &str = "HidReportDesctiptor";
pub const REG_DESC_SIZE: &str = "HidReportDesctiptorSize";

/// HID usages of the force feedback effect types, by [`EFFECTS`] index; 0x29 = reserved (off).
const EFFECT_USAGES: [u8; 11] = [
    0x26, 0x27, 0x30, 0x31, 0x32, 0x33, 0x34, 0x40, 0x41, 0x42, 0x43,
];
const EFFECT_OFF: u8 = 0x29;
/// `USAGE (Effect Type)`, `COLLECTION (Logical)` - the effect list follows.
const EFFECT_LIST: [u8; 4] = [0x09, 0x25, 0xA1, 0x02];

/// Registry key of device `id` below HKLM.
pub fn reg_key(id: u32) -> String {
    format!(r"{REG_PARAMS}\Device{id:02}")
}

impl DeviceConfig {
    /// The HID report descriptor vJoyConfig writes for this device.
    pub fn descriptor(&self) -> Vec<u8> {
        let id = self.id as u8;
        let buttons = self.buttons.min(MAX_BUTTONS) as u8;
        let hats = self.hats.min(MAX_HATS) as u8;
        let mut d = vec![
            0x05, 0x01, // USAGE_PAGE (Generic Desktop)
            0x15, 0x00, // LOGICAL_MINIMUM (0)
            0x09, 0x04, // USAGE (Joystick)
            0xA1, 0x01, // COLLECTION (Application)
            0x05, 0x01, // USAGE_PAGE (Generic Desktop)
            0x85, id, // REPORT_ID
            0x09, 0x01, // USAGE (Pointer)
            0x15, 0x00, // LOGICAL_MINIMUM (0)
            0x26, 0xFF, 0x7F, // LOGICAL_MAXIMUM (32767)
            0x75, 0x20, // REPORT_SIZE (32)
            0x95, 0x01, // REPORT_COUNT (1)
            0xA1, 0x00, // COLLECTION (Physical)
        ];
        for (i, on) in self.axes.iter().enumerate() {
            if *on {
                d.extend([0x09, 0x30 + i as u8, 0x81, 0x02]); // USAGE (X + i), INPUT (Data,Var,Abs)
            } else {
                d.extend([0x81, 0x01]); // INPUT (Cnst) - placeholder
            }
        }
        d.push(0xC0); // END_COLLECTION

        if hats > 0 && self.discrete_hats {
            d.extend([
                0x15, 0x00, // LOGICAL_MINIMUM (0)
                0x25, 0x03, // LOGICAL_MAXIMUM (3)
                0x35, 0x00, // PHYSICAL_MINIMUM (0)
                0x46, 0x0E, 0x01, // PHYSICAL_MAXIMUM (270)
                0x65, 0x14, // UNIT (Eng Rot: Angular Pos)
                0x75, 0x04, // REPORT_SIZE (4)
                0x95, 0x01, // REPORT_COUNT (1)
            ]);
            for _ in 0..hats {
                d.extend([0x09, 0x39, 0x81, 0x02]); // USAGE (Hat switch), INPUT (Data,Var,Abs)
            }
            d.extend([0x95, 0x20 - hats, 0x81, 0x01]); // 4-bit padding up to 32 nibbles
        } else if hats > 0 {
            d.extend([
                0x15, 0x00, // LOGICAL_MINIMUM (0)
                0x27, 0x3C, 0x8C, 0x00, 0x00, // LOGICAL_MAXIMUM (35900)
                0x35, 0x00, // PHYSICAL_MINIMUM (0)
                0x47, 0x3C, 0x8C, 0x00, 0x00, // PHYSICAL_MAXIMUM (35900)
                0x65, 0x14, // UNIT (Eng Rot: Angular Pos)
                0x75, 0x20, // REPORT_SIZE (32)
                0x95, 0x01, // REPORT_COUNT (1)
            ]);
            for _ in 0..hats {
                d.extend([0x09, 0x39, 0x81, 0x02]);
            }
            if hats < 4 {
                d.extend([0x95, 4 - hats, 0x81, 0x01]);
            }
        } else {
            d.extend([0x75, 0x20, 0x95, 0x04, 0x81, 0x01]); // four 32-bit placeholders
        }

        d.extend([
            0x05,
            0x09, // USAGE_PAGE (Button)
            0x15,
            0x00, // LOGICAL_MINIMUM (0)
            0x25,
            0x01, // LOGICAL_MAXIMUM (1)
            0x55,
            0x00, // UNIT_EXPONENT (0)
            0x65,
            0x00, // UNIT (None)
            0x19,
            u8::from(buttons > 0), // USAGE_MINIMUM (1, or 0 without buttons)
            0x29,
            buttons, // USAGE_MAXIMUM
            0x75,
            0x01, // REPORT_SIZE (1)
            0x95,
            buttons, // REPORT_COUNT
            0x81,
            0x02, // INPUT (Data,Var,Abs)
        ]);
        if u32::from(buttons) < MAX_BUTTONS {
            // One padding field (vJoyConfig writes it as a single REPORT_SIZE of the rest).
            d.extend([0x75, MAX_BUTTONS as u8 - buttons, 0x95, 0x01, 0x81, 0x01]);
        }

        if self.effects.iter().any(|e| *e) {
            let start = d.len();
            for (i, chunk) in FFB_CHUNKS.iter().enumerate() {
                d.extend_from_slice(chunk);
                if i > 0 {
                    // Wraps for device 16 like vJoyConfig's BYTE.
                    let at = d.len() - chunk.len();
                    d[at] = d[at].wrapping_add(id.wrapping_mul(0x10));
                }
            }
            let mut at = start;
            while let Some(p) = find(&d[at..], &EFFECT_LIST) {
                let list = at + p + 5;
                for (e, on) in self.effects.iter().enumerate() {
                    d[list + 2 * e] = if *on { EFFECT_USAGES[e] } else { EFFECT_OFF };
                }
                at = list;
            }
        }
        d.push(0xC0); // END_COLLECTION
        d
    }

    /// Reads a descriptor written by vJoyConfig (or by [`Self::descriptor`]) back.
    pub fn from_descriptor(id: u32, desc: &[u8]) -> Option<Self> {
        let mut c = Self {
            id,
            axes: [false; 8],
            buttons: 0,
            hats: 0,
            discrete_hats: false,
            effects: [false; EFFECTS.len()],
        };
        let mut page = 0u32;
        let mut usages: Vec<u32> = Vec::new();
        let mut log_max = 0u32;
        let mut usage_max = 0u32;
        let mut count = 0u32;
        let mut joystick = false;
        let mut i = 0;
        while i < desc.len() {
            let prefix = desc[i];
            let len = [0, 1, 2, 4][usize::from(prefix & 3)];
            let data = desc.get(i + 1..i + 1 + len)?;
            let value = data
                .iter()
                .rev()
                .fold(0u32, |v, b| (v << 8) | u32::from(*b));
            match prefix & 0xFC {
                0x04 => page = value,
                0x24 => log_max = value,
                0x94 => count = value,
                0x08 => usages.push(value),
                0x28 => usage_max = value,
                0x80 if value & 1 == 0 && page == 0x01 => {
                    for u in usages.drain(..) {
                        match u {
                            0x30..=0x37 => c.axes[(u - 0x30) as usize] = true,
                            0x39 => {
                                c.hats += 1;
                                c.discrete_hats = log_max == 3;
                            }
                            _ => {}
                        }
                    }
                }
                0x80 if value & 1 == 0 && page == 0x09 => c.buttons = usage_max.min(count),
                0xA0 => {
                    joystick |= page == 0x01 && usages.contains(&0x04);
                    if page == 0x0F && usages.last() == Some(&0x25) && c.effects == [false; 11] {
                        let list = desc.get(i + 2..i + 2 + 2 * EFFECTS.len())?;
                        for (e, u) in EFFECT_USAGES.iter().enumerate() {
                            c.effects[e] = list[2 * e] == 0x09 && list[2 * e + 1] == *u;
                        }
                    }
                }
                _ => {}
            }
            // Local items (usages) belong to the next main item only.
            if prefix & 0x0C == 0 {
                usages.clear();
                usage_max = 0;
            }
            i += 1 + len;
        }
        joystick.then_some(c)
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Configuration of every device in the registry (no rights needed).
pub fn read_registry() -> Result<Vec<DeviceConfig>, String> {
    #[cfg(windows)]
    {
        let mut out = Vec::new();
        for id in 1..=crate::MAX_DEVICES {
            if let Some(desc) = reg::read_binary(&reg_key(id), REG_DESC)? {
                out.extend(DeviceConfig::from_descriptor(id, &desc));
            }
        }
        Ok(out)
    }
    #[cfg(not(windows))]
    Err("vJoy needs Windows".into())
}

#[cfg(windows)]
mod reg {
    use windows::core::HSTRING;
    use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_BINARY};

    /// A REG_BINARY value; `None` when the key or the value is missing.
    pub fn read_binary(key: &str, name: &str) -> Result<Option<Vec<u8>>, String> {
        let (key, name) = (HSTRING::from(key), HSTRING::from(name));
        let mut size = 0u32;
        // SAFETY: sizing call without a buffer, then a call with a buffer of that size.
        unsafe {
            let r = RegGetValueW(
                HKEY_LOCAL_MACHINE,
                &key,
                &name,
                RRF_RT_REG_BINARY,
                None,
                None,
                Some(&mut size),
            );
            if r == ERROR_FILE_NOT_FOUND {
                return Ok(None);
            }
            if r != ERROR_SUCCESS {
                return Err(format!(
                    "registry {key}\\{name}: {}",
                    r.to_hresult().message()
                ));
            }
            let mut buf = vec![0u8; size as usize];
            let r = RegGetValueW(
                HKEY_LOCAL_MACHINE,
                &key,
                &name,
                RRF_RT_REG_BINARY,
                None,
                Some(buf.as_mut_ptr().cast()),
                Some(&mut size),
            );
            if r != ERROR_SUCCESS {
                return Err(format!(
                    "registry {key}\\{name}: {}",
                    r.to_hresult().message()
                ));
            }
            buf.truncate(size as usize);
            Ok(Some(buf))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace()
            .map(|b| u8::from_str_radix(b, 16).unwrap())
            .collect()
    }

    /// Device01 on the developer's PC (vJoy 2.1.9.1): 8 axes, 128 buttons, 4 continuous hats.
    const DEV1: &str = "05 01 15 00 09 04 A1 01 05 01 85 01 09 01 15 00 26 FF 7F 75 20 95 01 A1 00 \
        09 30 81 02 09 31 81 02 09 32 81 02 09 33 81 02 09 34 81 02 09 35 81 02 09 36 81 02 09 37 81 02 \
        C0 15 00 27 3C 8C 00 00 35 00 47 3C 8C 00 00 65 14 75 20 95 01 09 39 81 02 09 39 81 02 09 39 81 02 \
        09 39 81 02 05 09 15 00 25 01 55 00 65 00 19 01 29 80 75 01 95 80 81 02 C0";
    /// Device02: the same with 127 buttons (one padding bit).
    const DEV2: &str = "05 01 15 00 09 04 A1 01 05 01 85 02 09 01 15 00 26 FF 7F 75 20 95 01 A1 00 \
        09 30 81 02 09 31 81 02 09 32 81 02 09 33 81 02 09 34 81 02 09 35 81 02 09 36 81 02 09 37 81 02 \
        C0 15 00 27 3C 8C 00 00 35 00 47 3C 8C 00 00 65 14 75 20 95 01 09 39 81 02 09 39 81 02 09 39 81 02 \
        09 39 81 02 05 09 15 00 25 01 55 00 65 00 19 01 29 7F 75 01 95 7F 81 02 75 01 95 01 81 01 C0";

    fn dev(id: u32, buttons: u32) -> DeviceConfig {
        DeviceConfig {
            buttons,
            hats: 4,
            ..DeviceConfig::new(id)
        }
    }

    #[test]
    fn matches_vjoyconfig_bytes() {
        assert_eq!(dev(1, 128).descriptor(), hex(DEV1));
        assert_eq!(dev(2, 127).descriptor(), hex(DEV2));
        assert_eq!(
            DeviceConfig::from_descriptor(1, &hex(DEV1)),
            Some(dev(1, 128))
        );
        assert_eq!(
            DeviceConfig::from_descriptor(2, &hex(DEV2)),
            Some(dev(2, 127))
        );
    }

    #[test]
    fn round_trips_other_layouts() {
        let mut c = DeviceConfig::new(5);
        c.axes = [true, false, false, false, false, true, false, false];
        c.buttons = 8;
        c.hats = 2;
        c.discrete_hats = true;
        c.effects[0] = true;
        c.effects[10] = true;
        let d = c.descriptor();
        assert_eq!(DeviceConfig::from_descriptor(5, &d), Some(c.clone()));

        c.effects = [false; 11];
        c.hats = 0;
        c.discrete_hats = false; // not stored without hats
        c.buttons = 0;
        assert_eq!(
            DeviceConfig::from_descriptor(5, &c.descriptor()),
            Some(c.clone())
        );
        c.hats = 3;
        assert_eq!(DeviceConfig::from_descriptor(5, &c.descriptor()), Some(c));
    }

    #[test]
    fn ffb_report_ids_follow_the_device_id() {
        let ids: Vec<u8> = FFB_CHUNKS[1..].iter().map(|c| c[0]).collect();
        assert_eq!(
            ids,
            [2, 1, 2, 3, 4, 5, 6, 7, 8, 0xA, 0xB, 0xC, 0xD, 0xE, 1, 2, 3]
        );
        let mut c = DeviceConfig::new(3);
        c.effects = [true; 11];
        let d = c.descriptor();
        // The first FFB report id: 0x10 * 3 + HID_ID_STATE.
        let ffb = find(&d, &[0x05, 0x0F, 0x09, 0x92, 0xA1, 0x02, 0x85]).unwrap();
        assert_eq!(d[ffb + 7], 0x32);
        assert_eq!(*d.last().unwrap(), 0xC0);
    }

    #[cfg(windows)]
    #[test]
    fn regenerates_the_registry_if_present() {
        // Read-only: every descriptor vJoyConfig left in the registry comes out byte for byte.
        for id in 1..=crate::MAX_DEVICES {
            let Ok(Some(desc)) = reg::read_binary(&reg_key(id), REG_DESC) else {
                continue;
            };
            let c = DeviceConfig::from_descriptor(id, &desc).expect("vJoy descriptor");
            assert_eq!(c.descriptor(), desc, "device {id}: {c:?}");
        }
    }
}
