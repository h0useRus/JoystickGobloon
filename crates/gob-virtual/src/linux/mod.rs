//! Linux backend: uinput through gob-uinput. A device exists only while it is acquired (a
//! profile runs or the tester holds it); nothing to install or enable. The layout of each
//! device is kept in `virtual.gob` (TOML) in the data directory.
//!
//! Devices have vJoy's vendor 0x1234 and each its own product id from [`crate::PRODUCTS`] (games
//! keep bindings by the SDL GUID, made from them), are named "JoyGob N" or
//! as set, and sit at `phys` "joystick-gobloon/joystick-N", by which gob-input knows them as our
//! output and keeps their ids when they are renamed. Axes use the codes gob-input reads
//! back into the same slots (X..Rz, throttle and rudder as the sliders). Buttons use joystick
//! codes only, never gamepad ones (SDL2/3 would map the device as a gamepad, #31) nor `KEY_MAX`
//! (SDL does not count it, #32): 1-16 `BTN_JOYSTICK`..., 17-79 `BTN_TRIGGER_HAPPY1-63`,
//! 80-95 `BTN_0`..., the order in which SDL, joydev and gob-input count them.
//!
//! Gamepads ("GameGob N", `[[gamepad]]` in `virtual.gob`) have gob-uinput's
//! fixed Xbox 360 layout and ids, so SDL, Steam and Proton take them for an Xbox controller;
//! gob-input knows them as ours by their `phys` "joystick-gobloon/gamepad-N".
//!
//! With SystemGobloon running, `service.rs` is used instead: the service keeps the devices (and
//! their layouts) all the time and lends this program their uinput descriptors. It makes them
//! with the same code ([`crate::uinput`]).

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gob_system::Watch;
use gob_uinput::{Axis, Emitter, Spec, VirtualDevice, codes, gamepad};
use serde::{Deserialize, Serialize};

use crate::{
    AXIS_SLOTS, Backend, Capabilities, Change, ControlKind, DeviceConfig, DeviceInfo, Error,
    GamepadControl, GamepadInfo, Outcome, Status,
};

pub const CAPABILITIES: Capabilities = Capabilities {
    name: "Linux uinput",
    max_devices: crate::PROFILE_DEVICES,
    // Joystick button codes that SDL counts as joystick buttons (see the module comment).
    max_buttons: 95,
    max_hats: 4,
    effects: &[],
    switchable: false,
    needs_admin: false,
    // As many as XInput games handle.
    max_gamepads: 4,
    custom_names: true,
    // Made on acquire, gone on release (SystemGobloon keeps them: `service.rs`).
    persistent: false,
};
const GAMEPAD_PREFIX: &str = "GameGob ";
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

/// A joystick or a gamepad: they have ids of their own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Slot {
    Joystick(u32),
    Gamepad(u32),
}

impl Slot {
    fn name(self) -> String {
        match self {
            Self::Joystick(id) => DeviceConfig::default_name(id),
            Self::Gamepad(id) => format!("{GAMEPAD_PREFIX}{id}"),
        }
    }

    fn id(self) -> u32 {
        match self {
            Self::Joystick(id) | Self::Gamepad(id) => id,
        }
    }

    /// Where the device is "plugged in"; unlike the name it never changes.
    fn phys(self) -> String {
        match self {
            Self::Joystick(id) => format!("{}joystick-{id}", gob_uinput::OWN_PHYS),
            Self::Gamepad(id) => format!("{}gamepad-{id}", gob_uinput::OWN_PHYS),
        }
    }
}

/// Devices acquired by this process (every [`load`] shares them).
static OWNED: Mutex<Option<HashSet<Slot>>> = Mutex::new(None);

fn owned<R>(f: impl FnOnce(&mut HashSet<Slot>) -> R) -> R {
    let mut guard = OWNED.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_default())
}

pub(crate) mod service;

pub fn load() -> Result<Arc<dyn Backend>, Error> {
    if let Some(service) = service::Service::connect() {
        return Ok(Arc::new(service));
    }
    gob_uinput::available().map_err(|e| access_error(&e))?;
    Ok(Arc::new(Uinput))
}

fn access_error(e: &io::Error) -> Error {
    match e.kind() {
        io::ErrorKind::PermissionDenied => Error::NoAccess(format!("/dev/uinput: {e}")),
        _ => Error::NotInstalled(format!("/dev/uinput: {e}")),
    }
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
    #[serde(default, rename = "gamepad", skip_serializing_if = "Vec::is_empty")]
    gamepads: Vec<StoredGamepad>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct StoredGamepad {
    id: u32,
}

/// Every configured device: joysticks with their layouts, gamepad ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub devices: Vec<DeviceConfig>,
    pub gamepads: Vec<u32>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    product: Option<u16>,
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
            name: c.name.clone(),
            product: c.product,
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
            name: s.name.clone().filter(|n| crate::name_allowed(n)),
            product: s.product.filter(|&p| crate::product_allowed(p)),
        }
    }
}

/// The text of `virtual.gob` -> configurations; without a file, one default joystick.
pub fn parse(text: Option<&str>) -> Result<Layout, String> {
    let Some(text) = text else {
        return Ok(Layout {
            devices: vec![DeviceConfig::new(1, &CAPABILITIES)],
            gamepads: Vec::new(),
        });
    };
    let stored: Stored = toml::from_str(text).map_err(|e| format!("{CONFIG_FILE}: {e}"))?;
    let mut devices: Vec<DeviceConfig> = stored
        .devices
        .iter()
        .filter(|d| (1..=CAPABILITIES.max_devices).contains(&d.id))
        .map(DeviceConfig::from)
        .collect();
    devices.sort_by_key(|c| c.id);
    devices.dedup_by_key(|c| c.id);
    let mut gamepads: Vec<u32> = stored
        .gamepads
        .iter()
        .map(|g| g.id)
        .filter(|id| (1..=CAPABILITIES.max_gamepads).contains(id))
        .collect();
    gamepads.sort_unstable();
    gamepads.dedup();
    Ok(Layout { devices, gamepads })
}

fn read_configs() -> Result<Layout, String> {
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
pub fn changed(mut layout: Layout, change: &Change) -> Result<Layout, String> {
    let configs = &mut layout.devices;
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
        Change::AddGamepad(id) => {
            if !(1..=CAPABILITIES.max_gamepads).contains(id) {
                return Err(format!("gamepad id {id} is out of range"));
            }
            if !layout.gamepads.contains(id) {
                layout.gamepads.push(*id);
                layout.gamepads.sort_unstable();
            }
        }
        Change::DeleteGamepad(id) => layout.gamepads.retain(|g| g != id),
    }
    Ok(layout)
}

pub fn to_text(layout: &Layout) -> Result<String, String> {
    let stored = Stored {
        devices: layout.devices.iter().map(StoredDevice::from).collect(),
        gamepads: layout
            .gamepads
            .iter()
            .map(|&id| StoredGamepad { id })
            .collect(),
    };
    toml::to_string_pretty(&stored).map_err(|e| e.to_string())
}

fn info(c: &DeviceConfig) -> DeviceInfo {
    DeviceInfo {
        id: c.id,
        status: status(Slot::Joystick(c.id), true),
        axes: (1..=AXIS_SLOTS as u8)
            .filter(|&s| c.axes[usize::from(s) - 1])
            .collect(),
        buttons: c.buttons,
        cont_hats: if c.discrete_hats { 0 } else { c.hats },
        disc_hats: if c.discrete_hats { c.hats } else { 0 },
        name: Some(
            c.name
                .clone()
                .unwrap_or_else(|| DeviceConfig::default_name(c.id)),
        ),
    }
}

fn status(slot: Slot, configured: bool) -> Status {
    if owned(|o| o.contains(&slot)) {
        Status::Own
    } else if !configured {
        Status::Missing
    } else if gob_uinput::device_phys().contains(&slot.phys()) {
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
        read_configs().is_ok_and(|l| !l.devices.is_empty() || !l.gamepads.is_empty())
    }

    fn version(&self) -> String {
        std::fs::read_to_string("/proc/sys/kernel/osrelease")
            .map(|v| v.trim().to_string())
            .unwrap_or_default()
    }

    fn status(&self, id: u32) -> Status {
        let configured = read_configs().is_ok_and(|l| l.devices.iter().any(|d| d.id == id));
        status(Slot::Joystick(id), configured)
    }

    fn devices(&self) -> Vec<DeviceInfo> {
        match read_configs() {
            Ok(layout) => layout.devices.iter().map(info).collect(),
            Err(e) => {
                log::warn!("{e}");
                Vec::new()
            }
        }
    }

    fn acquire(&self, id: u32) -> Result<Box<dyn crate::Device>, Error> {
        let config = read_configs()
            .map_err(Error::NotInstalled)?
            .devices
            .into_iter()
            .find(|c| c.id == id)
            .ok_or(Error::Unavailable {
                id,
                status: Status::Missing,
            })?;
        let slot = take(Slot::Joystick(id))?;
        match Device::create(config) {
            Ok(dev) => Ok(Box::new(dev)),
            Err(e) => {
                owned(|o| o.remove(&slot));
                Err(access_error(&e))
            }
        }
    }

    fn read_config(&self) -> Result<Vec<DeviceConfig>, String> {
        read_configs().map(|l| l.devices)
    }

    fn gamepads(&self) -> Vec<GamepadInfo> {
        match read_configs() {
            Ok(layout) => layout
                .gamepads
                .iter()
                .map(|&id| GamepadInfo {
                    id,
                    status: status(Slot::Gamepad(id), true),
                })
                .collect(),
            Err(e) => {
                log::warn!("{e}");
                Vec::new()
            }
        }
    }

    fn acquire_gamepad(&self, id: u32) -> Result<Box<dyn crate::Gamepad>, Error> {
        let layout = read_configs().map_err(Error::NotInstalled)?;
        if !layout.gamepads.contains(&id) {
            return Err(Error::Unavailable {
                id,
                status: Status::Missing,
            });
        }
        let slot = take(Slot::Gamepad(id))?;
        match Pad::create(id) {
            Ok(pad) => Ok(Box::new(pad)),
            Err(e) => {
                owned(|o| o.remove(&slot));
                Err(access_error(&e))
            }
        }
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

/// Marks a configured device as ours, unless this process or another program has it.
fn take(slot: Slot) -> Result<Slot, Error> {
    if gob_uinput::device_phys().contains(&slot.phys()) || !owned(|o| o.insert(slot)) {
        return Err(Error::Unavailable {
            id: slot.id(),
            status: status(slot, true),
        });
    }
    Ok(slot)
}

/// Button `n` (1-based) -> its key code.
fn button_code(n: u32) -> Option<u16> {
    match n {
        1..=16 => Some(codes::BTN_JOYSTICK + (n - 1) as u16),
        17..=79 => Some(codes::BTN_TRIGGER_HAPPY + (n - 17) as u16),
        80..=95 => Some(codes::BTN_MISC + (n - 80) as u16),
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

/// Where a device's events go: its own uinput device, or one SystemGobloon keeps and lends.
enum Sink {
    Own(VirtualDevice),
    Lent(RefCell<Lent>),
}

impl Sink {
    fn emit(&self, events: &[(u16, u16, i32)]) -> io::Result<()> {
        match self {
            Sink::Own(dev) => dev.emit(events),
            Sink::Lent(lent) => lent.borrow_mut().emit(events),
        }
    }
}

/// Takes a device from the service: its descriptor, and a watch on the service that lent it.
type Take = Box<dyn Fn() -> Result<(Emitter, Watch), gob_system::Error> + Send>;

/// How often a lent device asks whether the service that lent it still runs.
const SERVICE_CHECK: Duration = Duration::from_millis(250);

/// A device SystemGobloon keeps and lends. When the service stops or restarts, the old
/// descriptor is dropped (it would keep a copy of the device alive next to the new service's
/// own) and the device is taken again; events in between are lost.
struct Lent {
    name: String,
    emitter: Option<(Emitter, Watch)>,
    take: Take,
    checked: Instant,
    /// Taking it again failed since it was given up: logged once.
    warned: bool,
}

impl Lent {
    fn new(name: String, take: Take) -> Result<Self, gob_system::Error> {
        Ok(Self {
            name,
            emitter: Some(take()?),
            take,
            checked: Instant::now(),
            warned: false,
        })
    }

    fn emit(&mut self, events: &[(u16, u16, i32)]) -> io::Result<()> {
        if self.checked.elapsed() >= SERVICE_CHECK {
            self.checked = Instant::now();
            if self.emitter.as_ref().is_some_and(|(_, watch)| watch.gone()) {
                log::info!("{}: SystemGobloon stopped, device given up", self.name);
                self.emitter = None;
            }
            if self.emitter.is_none() {
                match (self.take)() {
                    Ok(taken) => {
                        log::info!("{}: taken again from SystemGobloon", self.name);
                        self.emitter = Some(taken);
                        self.warned = false;
                    }
                    Err(e) if !self.warned => {
                        log::warn!("{}: taking it again: {e}", self.name);
                        self.warned = true;
                    }
                    Err(_) => {}
                }
            }
        }
        match &self.emitter {
            Some((emitter, _)) => emitter.emit(events),
            None => Ok(()),
        }
    }
}

/// Runs when a lent device is dropped: gives it back to the service.
type Release = Box<dyn FnOnce() + Send>;

/// Makes joystick `config.id` with that layout.
pub fn create_joystick(config: &DeviceConfig) -> io::Result<VirtualDevice> {
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
    let slot = Slot::Joystick(config.id);
    let name = config.name.clone().unwrap_or_else(|| slot.name());
    let spec = Spec {
        name: &name,
        phys: &slot.phys(),
        bus: codes::BUS_VIRTUAL,
        vendor: crate::VENDOR,
        product: config.product_id(),
        version: config.id as u16,
        keys: &keys,
        axes: &axes,
        rel: &[],
    };
    VirtualDevice::create(&spec)
}

/// Makes gamepad `id`.
pub fn create_gamepad(id: u32) -> io::Result<VirtualDevice> {
    let slot = Slot::Gamepad(id);
    VirtualDevice::create(&gamepad::spec(&slot.name(), &slot.phys()))
}

/// The events that center a joystick's axes and hats and release its buttons.
pub fn joystick_reset(config: &DeviceConfig) -> Vec<(u16, u16, i32)> {
    let mut events: Vec<(u16, u16, i32)> = AXIS_CODES
        .iter()
        .zip(config.axes)
        .filter(|(_, on)| *on)
        .map(|(&code, _)| (codes::EV_ABS, code, axis_value(0.0)))
        .collect();
    events.extend(
        (1..=config.buttons)
            .filter_map(button_code)
            .map(|code| (codes::EV_KEY, code, 0)),
    );
    for n in 0..config.hats as u16 {
        events.push((codes::EV_ABS, codes::ABS_HAT0X + n * 2, 0));
        events.push((codes::EV_ABS, codes::ABS_HAT0Y + n * 2, 0));
    }
    events
}

/// The events that center a gamepad's sticks and d-pad and release its buttons and triggers.
pub fn gamepad_reset() -> Vec<(u16, u16, i32)> {
    let mut events: Vec<(u16, u16, i32)> = gamepad::BUTTONS
        .iter()
        .map(|&code| (codes::EV_KEY, code, 0))
        .collect();
    for c in GamepadControl::ALL {
        let value = match c.kind() {
            ControlKind::Stick => gamepad::stick(0.0),
            ControlKind::Trigger => 0,
            _ => continue,
        };
        events.extend(pad_code(c).map(|code| (codes::EV_ABS, code, value)));
    }
    events.extend([
        (codes::EV_ABS, codes::ABS_HAT0X, 0),
        (codes::EV_ABS, codes::ABS_HAT0Y, 0),
    ]);
    events
}

struct Device {
    sink: Sink,
    config: DeviceConfig,
    release: Option<Release>,
}

impl Device {
    fn create(config: DeviceConfig) -> io::Result<Self> {
        Ok(Self {
            sink: Sink::Own(create_joystick(&config)?),
            config,
            release: None,
        })
    }

    /// A device SystemGobloon keeps: events go to its descriptor, `release` gives it back.
    fn lent(config: DeviceConfig, lent: Lent, release: Release) -> Self {
        Self {
            sink: Sink::Lent(RefCell::new(lent)),
            config,
            release: Some(release),
        }
    }

    fn emit(&self, events: &[(u16, u16, i32)]) -> bool {
        match self.sink.emit(events) {
            Ok(()) => true,
            Err(e) => {
                log::warn!("{}: {e}", Slot::Joystick(self.config.id).name());
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
        self.emit(&joystick_reset(&self.config))
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        // Own: the kernel removes the device with it, games see it unplugged. Lent: the service
        // keeps it and centers it.
        if let Some(release) = self.release.take() {
            release();
        }
        owned(|o| o.remove(&Slot::Joystick(self.config.id)));
    }
}

/// A gamepad as gob-input reads it back: axes X..Rz keep their slots, buttons count in code
/// order (like SDL), the d-pad is hat 1.
pub(crate) fn gamepad_inputs(device_name: &str) -> Option<crate::GamepadInputs> {
    device_name
        .strip_prefix(GAMEPAD_PREFIX)?
        .trim()
        .parse::<u32>()
        .ok()?;
    let coded = |kinds: &[ControlKind]| {
        let mut v: Vec<(u16, GamepadControl)> = GamepadControl::ALL
            .into_iter()
            .filter(|c| kinds.contains(&c.kind()))
            .filter_map(|c| Some((pad_code(c)?, c)))
            .collect();
        v.sort_unstable_by_key(|&(code, _)| code);
        v
    };
    Some(crate::GamepadInputs {
        axes: coded(&[ControlKind::Stick, ControlKind::Trigger])
            .into_iter()
            .map(|(code, c)| (code as u8 + 1, c))
            .collect(),
        buttons: coded(&[ControlKind::Button])
            .into_iter()
            .enumerate()
            .map(|(i, (_, c))| (i as u32 + 1, c))
            .collect(),
        dpad: Some(1),
    })
}

/// A gamepad button's key code, or a stick's or trigger's axis code; `None` for the d-pad.
fn pad_code(control: GamepadControl) -> Option<u16> {
    use GamepadControl as C;
    Some(match control {
        C::A => codes::BTN_SOUTH,
        C::B => codes::BTN_EAST,
        C::X => codes::BTN_NORTH,
        C::Y => codes::BTN_WEST,
        C::Lb => codes::BTN_TL,
        C::Rb => codes::BTN_TR,
        C::Back => codes::BTN_SELECT,
        C::Start => codes::BTN_START,
        C::Guide => codes::BTN_MODE,
        C::LeftStick => codes::BTN_THUMBL,
        C::RightStick => codes::BTN_THUMBR,
        C::LeftX => codes::ABS_X,
        C::LeftY => codes::ABS_Y,
        C::RightX => codes::ABS_RX,
        C::RightY => codes::ABS_RY,
        C::Lt => codes::ABS_Z,
        C::Rt => codes::ABS_RZ,
        C::DpadUp | C::DpadDown | C::DpadLeft | C::DpadRight | C::Dpad => return None,
    })
}

/// D-pad direction bits, pressed one by one (`DpadUp`...) or set from a hat.
const UP: u8 = 1;
const DOWN: u8 = 2;
const LEFT: u8 = 4;
const RIGHT: u8 = 8;

/// D-pad bits -> `ABS_HAT0X` / `ABS_HAT0Y` (up is -1); opposite directions cancel.
fn dpad_xy(bits: u8) -> (i32, i32) {
    let on = |b: u8| i32::from(bits & b != 0);
    (on(RIGHT) - on(LEFT), on(DOWN) - on(UP))
}

/// Hat values -> d-pad bits.
fn dpad_bits((x, y): (i32, i32)) -> u8 {
    [(y < 0, UP), (y > 0, DOWN), (x < 0, LEFT), (x > 0, RIGHT)]
        .into_iter()
        .filter(|(on, _)| *on)
        .fold(0, |bits, (_, b)| bits | b)
}

struct Pad {
    id: u32,
    sink: Sink,
    /// D-pad directions held.
    dpad: Cell<u8>,
    release: Option<Release>,
}

impl Pad {
    fn create(id: u32) -> io::Result<Self> {
        Ok(Self {
            id,
            sink: Sink::Own(create_gamepad(id)?),
            dpad: Cell::new(0),
            release: None,
        })
    }

    /// A gamepad SystemGobloon keeps, like [`Device::lent`].
    fn lent(id: u32, lent: Lent, release: Release) -> Self {
        Self {
            id,
            sink: Sink::Lent(RefCell::new(lent)),
            dpad: Cell::new(0),
            release: Some(release),
        }
    }

    fn emit(&self, events: &[(u16, u16, i32)]) -> bool {
        match self.sink.emit(events) {
            Ok(()) => true,
            Err(e) => {
                log::warn!("{}: {e}", Slot::Gamepad(self.id).name());
                false
            }
        }
    }

    fn emit_dpad(&self, bits: u8) -> bool {
        self.dpad.set(bits);
        let (x, y) = dpad_xy(bits);
        self.emit(&[
            (codes::EV_ABS, codes::ABS_HAT0X, x),
            (codes::EV_ABS, codes::ABS_HAT0Y, y),
        ])
    }
}

impl crate::Gamepad for Pad {
    fn set_button(&self, control: GamepadControl, pressed: bool) -> bool {
        let bit = match control {
            GamepadControl::DpadUp => UP,
            GamepadControl::DpadDown => DOWN,
            GamepadControl::DpadLeft => LEFT,
            GamepadControl::DpadRight => RIGHT,
            c if c.kind() == ControlKind::Button => {
                return pad_code(c)
                    .is_some_and(|code| self.emit(&[(codes::EV_KEY, code, i32::from(pressed))]));
            }
            _ => return false,
        };
        let bits = self.dpad.get();
        self.emit_dpad(if pressed { bits | bit } else { bits & !bit })
    }

    fn set_axis(&self, control: GamepadControl, value: f32) -> bool {
        let value = match control.kind() {
            ControlKind::Stick => gamepad::stick(value),
            ControlKind::Trigger => gamepad::trigger(value),
            _ => return false,
        };
        pad_code(control).is_some_and(|code| self.emit(&[(codes::EV_ABS, code, value)]))
    }

    fn set_dpad(&self, direction: Option<u16>) -> bool {
        self.emit_dpad(dpad_bits(gamepad::dpad(direction)))
    }

    fn reset(&self) -> bool {
        self.dpad.set(0);
        self.emit(&gamepad_reset())
    }
}

impl Drop for Pad {
    fn drop(&mut self) {
        // As for [`Device`].
        if let Some(release) = self.release.take() {
            release();
        }
        owned(|o| o.remove(&Slot::Gamepad(self.id)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gamepad_read_back_as_input_device() {
        use GamepadControl as C;
        assert_eq!(gamepad_inputs("JoyGob 1"), None);
        assert_eq!(gamepad_inputs("Some Pad"), None);
        let pad = gamepad_inputs("GameGob 2").unwrap();
        assert_eq!(
            pad.axes,
            [
                (1, C::LeftX),
                (2, C::LeftY),
                (3, C::Lt),
                (4, C::RightX),
                (5, C::RightY),
                (6, C::Rt)
            ]
        );
        let buttons: Vec<C> = pad.buttons.iter().map(|&(_, c)| c).collect();
        assert_eq!(
            buttons,
            [
                C::A,
                C::B,
                C::X,
                C::Y,
                C::Lb,
                C::Rb,
                C::Back,
                C::Start,
                C::Guide,
                C::LeftStick,
                C::RightStick
            ]
        );
        assert_eq!(pad.buttons[0].0, 1);
        assert_eq!(pad.dpad, Some(1));
    }

    #[test]
    fn buttons_hats_and_axes() {
        assert_eq!(button_code(1), Some(0x120));
        assert_eq!(button_code(16), Some(0x12f));
        assert_eq!(button_code(17), Some(0x2c0));
        assert_eq!(button_code(79), Some(0x2fe));
        assert_eq!(button_code(80), Some(0x100));
        assert_eq!(button_code(95), Some(0x10f));
        assert_eq!(button_code(96), None);
        let codes: Vec<u16> = (1..=95).filter_map(button_code).collect();
        assert!(
            codes
                .iter()
                .all(|c| !(0x130..0x140).contains(c) && *c != 0x2ff),
            "no gamepad codes, no KEY_MAX"
        );
        // SDL's order: BTN_JOYSTICK up to KEY_MAX, then everything below BTN_JOYSTICK.
        let mut sdl = codes.clone();
        sdl.sort_by_key(|&c| (c < 0x120, c));
        assert_eq!(sdl, codes, "button numbers follow SDL's order");
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
        let default = parse(None).unwrap();
        assert_eq!(default.devices, vec![DeviceConfig::new(1, &CAPABILITIES)]);
        assert!(default.gamepads.is_empty(), "no gamepad until one is added");
        let joystick = &default.devices[0];
        assert_eq!(
            (joystick.buttons, joystick.hats),
            (95, 4),
            "the whole layout"
        );
        assert_eq!(joystick.axes, [true; AXIS_SLOTS]);
        let mut c = DeviceConfig::new(3, &CAPABILITIES);
        c.axes[2] = false;
        c.discrete_hats = true;
        c.name = Some("Gobloon Throttle".into());
        c.product = Some(0xBE42);
        let layout = changed(default, &Change::Set(c.clone())).unwrap();
        let layout = changed(layout, &Change::AddGamepad(2)).unwrap();
        let text = to_text(&layout).unwrap();
        assert!(text.contains("[[device]]"), "{text}");
        assert!(text.contains("name = \"Gobloon Throttle\""), "{text}");
        assert!(
            !text.contains("name = \"Joystick"),
            "default names are not stored"
        );
        assert!(text.contains("[[gamepad]]\nid = 2"), "{text}");
        assert_eq!(parse(Some(&text)).unwrap(), layout);
        let layout = changed(layout, &Change::Delete(1)).unwrap();
        assert_eq!(layout.devices, vec![c]);
        assert!(changed(layout.clone(), &Change::Enable(false)).is_err());
        assert!(
            changed(layout.clone(), &Change::AddGamepad(5)).is_err(),
            "4 at most"
        );
        let layout = changed(layout, &Change::DeleteGamepad(2)).unwrap();
        assert!(layout.gamepads.is_empty());
        assert!(!to_text(&layout).unwrap().contains("gamepad"));
        let bad = parse(Some(
            "[[device]]
id = 1
axes = []
buttons = 1
hats = 0
name = \"\"
product = 48813
",
        ))
        .unwrap();
        assert_eq!(
            (&bad.devices[0].name, bad.devices[0].product),
            (&None, None),
            "an empty name and vJoy's product id are dropped"
        );
        let empty = parse(Some("")).unwrap();
        assert!(
            empty.devices.is_empty() && empty.gamepads.is_empty(),
            "no devices"
        );
    }

    #[test]
    fn gamepad_controls_and_dpad() {
        let buttons: Vec<u16> = GamepadControl::ALL
            .into_iter()
            .filter(|c| c.kind() == ControlKind::Button)
            .filter_map(pad_code)
            .collect();
        assert_eq!(
            buttons.len(),
            gamepad::BUTTONS.len(),
            "every button has a code"
        );
        assert!(buttons.iter().all(|b| gamepad::BUTTONS.contains(b)));
        for c in GamepadControl::ALL {
            let axis = matches!(c.kind(), ControlKind::Stick | ControlKind::Trigger);
            if axis {
                assert!(pad_code(c).is_some(), "{c:?}");
            }
        }
        assert_eq!(dpad_xy(0), (0, 0));
        assert_eq!(dpad_xy(UP | RIGHT), (1, -1));
        assert_eq!(dpad_xy(UP | DOWN), (0, 0), "opposites cancel");
        for xy in [(0, 0), (1, -1), (-1, 0), (0, 1)] {
            assert_eq!(dpad_xy(dpad_bits(xy)), xy);
        }
    }
}
