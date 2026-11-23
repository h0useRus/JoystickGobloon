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
fn sixteen_devices_fit_the_driver() {
    // The driver joins every DeviceNN descriptor into one (USHORT size) and keeps a device
    // only when its report id (byte 11) is new; ids 1..=16, the most it supports.
    let mut joined = Vec::new();
    let mut ids = Vec::new();
    for id in 1..=crate::MAX_DEVICES {
        let mut c = DeviceConfig::new(id);
        c.buttons = MAX_BUTTONS;
        c.hats = MAX_HATS;
        c.effects = [true; 11];
        let d = c.descriptor();
        assert_eq!((d[10], d[11]), (0x85, id as u8));
        assert_eq!(DeviceConfig::from_descriptor(id, &d), Some(c));
        ids.push(d[11]);
        joined.extend(d);
    }
    ids.dedup();
    assert_eq!(ids.len(), 16);
    assert!(joined.len() <= usize::from(u16::MAX), "{}", joined.len());
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
