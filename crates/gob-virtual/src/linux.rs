//! Linux backend: uinput through gob-uinput. A device exists only while it is acquired (a
//! profile runs or the tester holds it); nothing to install or enable. The layout of each
//! device is kept in `virtual.gob` (TOML) in the data directory.
//!
//! Devices look like vJoy's (vendor 0x1234, product 0xBEAD, so gob-input knows them as our
//! output) and are named "Joystick Gobloon Joystick N". Axes use the codes gob-input reads
//! back into the same slots (X..Rz, throttle and rudder as the sliders); buttons 1-32 are the
//! joystick/gamepad codes, 33-96 the extra ("trigger happy") ones, in the order SDL counts them.

use std::collections::HashSet;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use gob_uinput::{Axis, Spec, VirtualDevice, codes};
use serde::{Deserialize, Serialize};

use crate::{
    AXIS_SLOTS, Backend, Capabilities, Change, DeviceConfig, DeviceInfo, Error, Outcome, Status,
};

pub const CAPABILITIES: Capabilities = Capabilities {
    name: "Linux uinput",
    max_devices: crate::PROFILE_DEVICES,
    // Joystick buttons the kernel has codes for below KEY_MAX; more would need other key codes,
    // which games may count differently.
    max_buttons: 96,
    max_hats: 4,
    effects: &[],
    switchable: false,
    needs_admin: false,
};
const NAME_PREFIX: &str = "Joystick Gobloon Joystick ";
const VENDOR: u16 = 0x1234;
const PRODUCT: u16 = 0xBEAD;
const CONFIG_FILE: &str = "virtual.gob";
/// Event codes of axis slots 1..=8.
const AXIS_CODES: [u16; AXIS_SLOTS] = [
    codes::ABS_X,
    codes::ABS_Y,
    codes::ABS_Z,
    codes::ABS_RX,
    codes::ABS_RY,
    codes::ABS_RZ,
    codes::ABS_THROTTLE,
    codes::ABS_RUDDER,
];
const AXIS_MIN: i32 = -32768;
const AXIS_MAX: i32 = 32767;
/// Hat directions by 45° sector from up, clockwise, as (x, y).
const HAT_SECTORS: [(i32, i32); 8] = [
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
];

/// Device ids acquired by this process (every [`load`] shares them).
static OWNED: Mutex<Option<HashSet<u32>>> = Mutex::new(None);

fn owned<R>(f: impl FnOnce(&mut HashSet<u32>) -> R) -> R {
    let mut guard = OWNED.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_default())
}

pub fn load() -> Result<Arc<dyn Backend>, Error> {
    gob_uinput::available().map_err(|e| access_error(&e))?;
    Ok(Arc::new(Uinput))
}

fn access_error(e: &io::Error) -> Error {
    match e.kind() {
        io::ErrorKind::PermissionDenied => Error::NoAccess(format!("/dev/uinput: {e}")),
        _ => Error::NotInstalled(format!("/dev/uinput: {e}")),
    }
}

fn device_name(id: u32) -> String {
    format!("{NAME_PREFIX}{id}")
}

fn config_path() -> Option<PathBuf> {
    let dir = crate::data_dir().or_else(|| {
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|d| !d.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(config.join("JoystickGobloon"))
    })?;
    Some(dir.join(CONFIG_FILE))
}

/// `virtual.gob`.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Stored {
    #[serde(default, rename = "device")]
    devices: Vec<StoredDevice>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct StoredDevice {
    id: u32,
    /// Axis slots, 1 = X ... 8 = Slider 1.
    axes: Vec<u8>,
    buttons: u32,
    hats: u32,
    #[serde(default)]
    discrete_hats: bool,
}

impl From<&DeviceConfig> for StoredDevice {
    fn from(c: &DeviceConfig) -> Self {
        Self {
            id: c.id,
            axes: (1..=AXIS_SLOTS as u8)
                .filter(|&s| c.axes[usize::from(s) - 1])
                .collect(),
            buttons: c.buttons,
            hats: c.hats,
            discrete_hats: c.discrete_hats,
        }
    }
}

impl From<&StoredDevice> for DeviceConfig {
    fn from(s: &StoredDevice) -> Self {
        let mut axes = [false; AXIS_SLOTS];
        for &slot in &s.axes {
            if let Some(a) = usize::from(slot)
                .checked_sub(1)
                .and_then(|i| axes.get_mut(i))
            {
                *a = true;
            }
        }
        Self {
            id: s.id,
            axes,
            buttons: s.buttons.min(CAPABILITIES.max_buttons),
            hats: s.hats.min(CAPABILITIES.max_hats),
            discrete_hats: s.discrete_hats,
            effects: Vec::new(),
        }
    }
}

/// The text of `virtual.gob` -> configurations; without a file, one default device.
fn parse(text: Option<&str>) -> Result<Vec<DeviceConfig>, String> {
    let Some(text) = text else {
        return Ok(vec![DeviceConfig::new(1, &CAPABILITIES)]);
    };
    let stored: Stored = toml::from_str(text).map_err(|e| format!("{CONFIG_FILE}: {e}"))?;
    let mut configs: Vec<DeviceConfig> = stored
        .devices
        .iter()
        .filter(|d| (1..=CAPABILITIES.max_devices).contains(&d.id))
        .map(DeviceConfig::from)
        .collect();
    configs.sort_by_key(|c| c.id);
    configs.dedup_by_key(|c| c.id);
    Ok(configs)
}

fn read_configs() -> Result<Vec<DeviceConfig>, String> {
    let text = match config_path().map(std::fs::read_to_string) {
        Some(Ok(text)) => Some(text),
        Some(Err(e)) if e.kind() != io::ErrorKind::NotFound => {
            return Err(format!("{CONFIG_FILE}: {e}"));
        }
        _ => None,
    };
    parse(text.as_deref())
}

/// Configurations with the change applied.
fn changed(mut configs: Vec<DeviceConfig>, change: &Change) -> Result<Vec<DeviceConfig>, String> {
    match change {
        Change::Set(c) => {
            if !(1..=CAPABILITIES.max_devices).contains(&c.id) {
                return Err(format!("device id {} is out of range", c.id));
            }
            configs.retain(|d| d.id != c.id);
            configs.push(c.clone());
            configs.sort_by_key(|c| c.id);
        }
        Change::Delete(id) => configs.retain(|d| d.id != *id),
        Change::Enable(_) => return Err("uinput devices cannot be switched off".into()),
    }
    Ok(configs)
}

fn to_text(configs: &[DeviceConfig]) -> Result<String, String> {
    let stored = Stored {
        devices: configs.iter().map(StoredDevice::from).collect(),
    };
    toml::to_string_pretty(&stored).map_err(|e| e.to_string())
}

fn info(c: &DeviceConfig) -> DeviceInfo {
    DeviceInfo {
        id: c.id,
        status: status(c.id, true),
        axes: (1..=AXIS_SLOTS as u8)
            .filter(|&s| c.axes[usize::from(s) - 1])
            .collect(),
        buttons: c.buttons,
        cont_hats: if c.discrete_hats { 0 } else { c.hats },
        disc_hats: if c.discrete_hats { c.hats } else { 0 },
    }
}

fn status(id: u32, configured: bool) -> Status {
    if owned(|o| o.contains(&id)) {
        Status::Own
    } else if !configured {
        Status::Missing
    } else if gob_uinput::device_names().contains(&device_name(id)) {
        // Another program (e.g. a second copy of ours) has it.
        Status::Busy
    } else {
        Status::Free
    }
}

struct Uinput;

impl Backend for Uinput {
    fn capabilities(&self) -> Capabilities {
        CAPABILITIES
    }

    fn enabled(&self) -> bool {
        read_configs().is_ok_and(|c| !c.is_empty())
    }

    fn version(&self) -> String {
        std::fs::read_to_string("/proc/sys/kernel/osrelease")
            .map(|v| v.trim().to_string())
            .unwrap_or_default()
    }

    fn status(&self, id: u32) -> Status {
        let configured = read_configs().is_ok_and(|c| c.iter().any(|d| d.id == id));
        status(id, configured)
    }

    fn devices(&self) -> Vec<DeviceInfo> {
        match read_configs() {
            Ok(configs) => configs.iter().map(info).collect(),
            Err(e) => {
                log::warn!("{e}");
                Vec::new()
            }
        }
    }

    fn acquire(&self, id: u32) -> Result<Box<dyn crate::Device>, Error> {
        let config = read_configs()
            .map_err(Error::NotInstalled)?
            .into_iter()
            .find(|c| c.id == id)
            .ok_or(Error::Unavailable {
                id,
                status: Status::Missing,
            })?;
        let busy = || Error::Unavailable {
            id,
            status: status(id, true),
        };
        if gob_uinput::device_names().contains(&device_name(id)) || !owned(|o| o.insert(id)) {
            return Err(busy());
        }
        match Device::create(config) {
            Ok(dev) => Ok(Box::new(dev)),
            Err(e) => {
                owned(|o| o.remove(&id));
                Err(access_error(&e))
            }
        }
    }

    fn read_config(&self) -> Result<Vec<DeviceConfig>, String> {
        read_configs()
    }

    fn apply(&self, change: &Change) -> Result<Outcome, String> {
        let path = config_path().ok_or("no folder for the configuration")?;
        let text = to_text(&changed(read_configs()?, change)?)?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Outcome::Done)
    }
}

/// Button `n` (1-based) -> its key code.
fn button_code(n: u32) -> Option<u16> {
    match n {
        1..=32 => Some(codes::BTN_JOYSTICK + (n - 1) as u16),
        33..=96 => Some(codes::BTN_TRIGGER_HAPPY + (n - 33) as u16),
        _ => None,
    }
}

/// Direction -> hat axis values; discrete hats only know up, right, down and left.
fn hat_xy(direction: Option<u16>, discrete: bool) -> (i32, i32) {
    let Some(d) = direction else {
        return (0, 0);
    };
    let d = u32::from(d % 36000);
    let sector = if discrete {
        ((d + 4500) / 9000 % 4) * 2
    } else {
        (d + 2250) / 4500 % 8
    };
    HAT_SECTORS[sector as usize]
}

/// -1.0..=1.0 -> the axis range.
fn axis_value(value: f32) -> i32 {
    let t = (f64::from(value.clamp(-1.0, 1.0)) + 1.0) / 2.0;
    AXIS_MIN + (t * f64::from(AXIS_MAX - AXIS_MIN)).round() as i32
}

struct Device {
    dev: VirtualDevice,
    config: DeviceConfig,
}

impl Device {
    fn create(config: DeviceConfig) -> io::Result<Self> {
        let axes: Vec<Axis> = AXIS_CODES
            .iter()
            .zip(config.axes)
            .filter(|(_, on)| *on)
            .map(|(&code, _)| Axis {
                code,
                min: AXIS_MIN,
                max: AXIS_MAX,
            })
            .chain((0..config.hats as u16 * 2).map(|i| Axis {
                code: codes::ABS_HAT0X + i,
                min: -1,
                max: 1,
            }))
            .collect();
        let keys: Vec<u16> = (1..=config.buttons).filter_map(button_code).collect();
        let name = device_name(config.id);
        let spec = Spec {
            name: &name,
            bus: codes::BUS_VIRTUAL,
            vendor: VENDOR,
            product: PRODUCT,
            version: config.id as u16,
            keys: &keys,
            axes: &axes,
            rel: &[],
        };
        let dev = VirtualDevice::create(&spec)?;
        Ok(Self { dev, config })
    }

    fn emit(&self, events: &[(u16, u16, i32)]) -> bool {
        match self.dev.emit(events) {
            Ok(()) => true,
            Err(e) => {
                log::warn!("{}: {e}", device_name(self.config.id));
                false
            }
        }
    }

    fn hat_events(&self, n: u32, direction: Option<u16>) -> [(u16, u16, i32); 2] {
        let (x, y) = hat_xy(direction, self.config.discrete_hats);
        let code = codes::ABS_HAT0X + (n as u16 - 1) * 2;
        [(codes::EV_ABS, code, x), (codes::EV_ABS, code + 1, y)]
    }
}

impl crate::Device for Device {
    fn set_axis(&self, slot: u8, value: f32) -> bool {
        let i = usize::from(slot).wrapping_sub(1);
        if !self.config.axes.get(i).is_some_and(|&on| on) {
            return false;
        }
        self.emit(&[(codes::EV_ABS, AXIS_CODES[i], axis_value(value))])
    }

    fn set_button(&self, n: u8, pressed: bool) -> bool {
        match button_code(u32::from(n)).filter(|_| u32::from(n) <= self.config.buttons) {
            Some(code) => self.emit(&[(codes::EV_KEY, code, i32::from(pressed))]),
            None => false,
        }
    }

    fn set_hat(&self, n: u8, direction: Option<u16>) -> bool {
        if !(1..=self.config.hats).contains(&u32::from(n)) {
            return false;
        }
        self.emit(&self.hat_events(u32::from(n), direction))
    }

    fn reset(&self) -> bool {
        let mut events: Vec<(u16, u16, i32)> = AXIS_CODES
            .iter()
            .zip(self.config.axes)
            .filter(|(_, on)| *on)
            .map(|(&code, _)| (codes::EV_ABS, code, axis_value(0.0)))
            .collect();
        events.extend(
            (1..=self.config.buttons)
                .filter_map(button_code)
                .map(|code| (codes::EV_KEY, code, 0)),
        );
        for n in 1..=self.config.hats {
            events.extend(self.hat_events(n, None));
        }
        self.emit(&events)
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        // The kernel removes the device with `dev`; games see it unplugged.
        owned(|o| o.remove(&self.config.id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_hats_and_axes() {
        assert_eq!(button_code(1), Some(0x120));
        assert_eq!(button_code(32), Some(0x13f));
        assert_eq!(button_code(33), Some(0x2c0));
        assert_eq!(button_code(96), Some(0x2ff));
        assert_eq!(button_code(97), None);
        assert_eq!(hat_xy(None, false), (0, 0));
        assert_eq!(hat_xy(Some(0), false), (0, -1));
        assert_eq!(hat_xy(Some(4500), false), (1, -1));
        assert_eq!(hat_xy(Some(35000), false), (0, -1), "nearest sector");
        assert_eq!(
            hat_xy(Some(4500), true),
            (1, 0),
            "discrete: right wins at 45°"
        );
        assert_eq!(hat_xy(Some(27000), true), (-1, 0));
        assert_eq!(axis_value(-1.0), AXIS_MIN);
        assert_eq!(axis_value(1.0), AXIS_MAX);
        assert_eq!(axis_value(0.0), 0);
    }

    #[test]
    fn config_file_round_trip() {
        assert_eq!(
            parse(None).unwrap(),
            vec![DeviceConfig::new(1, &CAPABILITIES)]
        );
        let mut c = DeviceConfig::new(3, &CAPABILITIES);
        c.axes[2] = false;
        c.discrete_hats = true;
        let configs = changed(parse(None).unwrap(), &Change::Set(c.clone())).unwrap();
        let text = to_text(&configs).unwrap();
        assert!(text.contains("[[device]]"), "{text}");
        assert_eq!(parse(Some(&text)).unwrap(), configs);
        let configs = changed(configs, &Change::Delete(1)).unwrap();
        assert_eq!(configs, vec![c]);
        assert!(changed(configs.clone(), &Change::Enable(false)).is_err());
        assert_eq!(
            parse(Some("")).unwrap(),
            Vec::new(),
            "an empty file: no devices"
        );
    }
}
