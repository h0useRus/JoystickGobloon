//! Linux: controllers and keyboards through evdev (`/dev/input/event*`, readable for the `input`
//! group or with a udev `uaccess` rule), controller numbers from joydev (`/dev/input/jsN`).
//!
//! Ids are GUID-formatted like Gremlin's: vendor and product in the first part (as in
//! DirectInput's product GUID), the bus type, then a hash of the serial number (or, without
//! one, of where the device is plugged in) and the name. Profiles made on Windows are matched
//! with "Map controls". Axes take DirectInput's slots (X, Y, Z, Rx, Ry, Rz, then the next two
//! axes as sliders), buttons follow SDL's order, so games and profiles count them the same way.

mod evdev;
mod hid;
pub(crate) mod keyboard;

use std::cell::Cell;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use evdev::{ABS_CNT, Bits, Caps, EV_ABS, EV_KEY, EV_SYN, KEY_CNT, Node, SYN_DROPPED, SYN_REPORT};

use super::{format_guid, normalize_axis};
use crate::{AXIS_SLOTS, Calibration, Calibrations, DeviceInfo, DeviceState, InputEvent};

/// Event mode: devices are also read this often without new events, as a fallback.
const EVENT_TIMEOUT: Duration = Duration::from_millis(20);
/// Buttons and hats a device can have, as on Windows (DIJOYSTATE2).
const MAX_BUTTONS: usize = 128;
const MAX_HATS: usize = 4;
/// The most buttons the kernel's HID driver passes on to evdev for a joystick (16
/// `BTN_JOYSTICK` + 64 `BTN_TRIGGER_HAPPY`). Only a node with that many can have lost buttons;
/// one with fewer may be another collection of the same HID device, whose buttons it must not
/// take.
const EVDEV_MAX_BUTTONS: usize = 80;
/// A device with buttons evdev does not pass on tries to open its raw HID node this often, so
/// access granted while it is connected takes effect without replugging.
const RAW_RETRY: Duration = Duration::from_secs(2);

const BTN_MOUSE: usize = 0x110;
const BTN_JOYSTICK: usize = 0x120;
/// End of the joystick and gamepad buttons (`BTN_DIGI`).
const BTN_DIGI: usize = 0x140;
const BTN_TOOL_PEN: usize = 0x140;
const BTN_TOOL_FINGER: usize = 0x145;
const BTN_TOUCH: usize = 0x14a;
const BTN_STYLUS: usize = 0x14b;
const BTN_TRIGGER_HAPPY: usize = 0x2c0;
const ABS_RZ: usize = 0x05;
const ABS_HAT0X: usize = 0x10;
const ABS_HAT3Y: usize = 0x17;
/// First multi-touch axis; it and everything after it are not joystick axes.
const ABS_MT_SLOT: usize = 0x2f;
const INPUT_PROP_ACCELEROMETER: usize = 0x06;

pub(crate) struct Platform {
    /// The "no access" warning is logged once.
    warned: Cell<bool>,
    /// Reused by `wait`, so waiting does not allocate.
    fds: std::cell::RefCell<Vec<libc::pollfd>>,
}

/// A controller [`Platform::attached`] found.
pub(crate) struct Found {
    path: PathBuf,
    guid: String,
}

impl super::Backend for Platform {
    type Found = Found;
    type Device = Device;
    const EXCLUSIVE: bool = true;

    fn new() -> Option<Self> {
        Some(Self {
            warned: Cell::new(false),
            fds: Default::default(),
        })
    }

    fn attached(&self) -> Vec<(String, Found)> {
        let mut found: Vec<(String, Found)> = Vec::new();
        let mut denied = 0;
        for path in evdev::event_nodes() {
            let node = match Node::open(&path) {
                Ok(node) => node,
                Err(e) => {
                    if e.kind() == io::ErrorKind::PermissionDenied {
                        denied += 1;
                    }
                    continue;
                }
            };
            if !is_joystick(&node.caps()) {
                continue;
            }
            let [bus, vendor, product, _] = node.id();
            let mut identity = identity(&node.uniq(), &node.phys(), &node.name());
            // Two nodes that look the same (e.g. no serial and no location): tell them apart
            // by their node, which keeps them separate at least until they are replugged.
            if found
                .iter()
                .any(|(id, _)| *id == device_guid(vendor, product, bus, &identity))
            {
                identity.push_str(&path.to_string_lossy());
            }
            let guid = device_guid(vendor, product, bus, &identity);
            found.push((guid.clone(), Found { path, guid }));
        }
        if denied > 0 && found.is_empty() && !self.warned.replace(true) {
            log::warn!(
                "no access to {denied} input device(s) in /dev/input: add the user to the \
                 input group or install the udev rule"
            );
        }
        found
    }

    fn open(&self, found: &Found, events: bool) -> Option<Device> {
        match Device::open(found, events) {
            Ok(dev) => Some(dev),
            Err(e) => {
                log::warn!("{}: {e}", found.path.display());
                None
            }
        }
    }

    fn wait(&self, devices: &[Device], events: bool, poll: Duration, wake: Option<Duration>) {
        let at_most = |d: Duration| wake.map_or(d, |w| w.min(d));
        if !events || devices.is_empty() {
            std::thread::sleep(at_most(if events { EVENT_TIMEOUT } else { poll }));
            return;
        }
        let mut fds = self.fds.borrow_mut();
        fds.clear();
        let fds_of = |d: &Device| [Some(d.node.fd()), d.raw.as_ref().map(hid::Raw::fd)];
        fds.extend(
            devices
                .iter()
                .flat_map(fds_of)
                .flatten()
                .map(|fd| libc::pollfd {
                    fd,
                    events: libc::POLLIN,
                    revents: 0,
                }),
        );
        let timeout = at_most(EVENT_TIMEOUT).as_micros().div_ceil(1000) as i32;
        evdev::wait_readable(&mut fds, timeout);
    }
}

/// Whether the capabilities are a joystick's, gamepad's, wheel's or pedals' (after udev's
/// `input_id`): joystick buttons, or joystick axes without being a mouse, touch surface, pen or
/// motion sensor.
fn is_joystick(caps: &Caps) -> bool {
    let joy_buttons = caps.keys.any(BTN_JOYSTICK..BTN_DIGI)
        || caps.keys.any(BTN_TRIGGER_HAPPY..BTN_TRIGGER_HAPPY + 40);
    let joy_axes = caps.events.has(EV_ABS.into()) && caps.abs.any(0..ABS_HAT3Y + 1);
    let pointer = [BTN_TOOL_PEN, BTN_TOOL_FINGER, BTN_TOUCH, BTN_STYLUS]
        .iter()
        .any(|&b| caps.keys.has(b))
        || caps.abs.any(ABS_MT_SLOT..ABS_CNT)
        || (caps.keys.has(BTN_MOUSE) && !joy_buttons);
    !caps.props.has(INPUT_PROP_ACCELEROMETER) && !pointer && (joy_buttons || joy_axes)
}

/// What identifies a device across restarts: its serial number, or where it is plugged in, and
/// its name (one USB device can have several input nodes, each with its own name). Our own
/// virtual devices by their place alone: they keep their id when renamed.
fn identity(uniq: &str, phys: &str, name: &str) -> String {
    if phys.starts_with(gob_uinput::OWN_PHYS) {
        format!("phys:{phys}")
    } else if uniq.is_empty() {
        format!("phys:{phys}\n{name}")
    } else {
        format!("uniq:{uniq}\n{name}")
    }
}

/// The device id: vendor and product like DirectInput's product GUID, the bus type and a hash
/// of the device's identity.
fn device_guid(vendor: u16, product: u16, bus: u16, identity: &str) -> String {
    let data1 = u32::from(vendor) | (u32::from(product) << 16);
    format_guid(data1, bus, 0, fnv1a(identity.as_bytes()).to_be_bytes())
}

/// 64-bit FNV-1a: stable across builds and platforms, unlike std's hasher.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// The axes in DirectInput's slots: (event code, 0-based slot). X..Rz keep their slots, the
/// next two other axes (throttle, rudder, wheel, ... in code order) become the sliders.
fn axis_slots(abs: &Bits) -> Vec<(usize, usize)> {
    let mut slots: Vec<(usize, usize)> = (0..=ABS_RZ)
        .filter(|&c| abs.has(c))
        .map(|c| (c, c))
        .collect();
    let sliders = (ABS_RZ + 1..ABS_MT_SLOT)
        .filter(|c| !(ABS_HAT0X..=ABS_HAT3Y).contains(c))
        .filter(|&c| abs.has(c));
    slots.extend(sliders.zip(ABS_RZ + 1..AXIS_SLOTS));
    slots
}

/// Button codes in button order, like SDL: joystick, gamepad and the extra ("trigger happy")
/// buttons first, then whatever else the device reports.
fn button_codes(keys: &Bits) -> Vec<u16> {
    (BTN_JOYSTICK..KEY_CNT)
        .chain(0..BTN_JOYSTICK)
        .filter(|&c| keys.has(c))
        .take(MAX_BUTTONS)
        .map(|c| c as u16)
        .collect()
}

/// Hats present (as `ABS_HATnX` / `ABS_HATnY` pairs), counted up to the last one present.
fn hat_count(abs: &Bits) -> usize {
    (0..MAX_HATS)
        .rev()
        .find(|n| abs.has(ABS_HAT0X + 2 * n) || abs.has(ABS_HAT0X + 2 * n + 1))
        .map_or(0, |n| n + 1)
}

/// Hat axis values (negative = left/up) -> direction in hundredths of a degree, 0 = up.
fn hat_direction(x: i32, y: i32) -> Option<u16> {
    Some(match (x.signum(), y.signum()) {
        (0, 0) => return None,
        (0, -1) => 0,
        (1, -1) => 4500,
        (1, 0) => 9000,
        (1, 1) => 13500,
        (0, 1) => 18000,
        (-1, 1) => 22500,
        (-1, 0) => 27000,
        _ => 31500,
    })
}

/// An axis value in `min..=max` -> DirectInput's -32768..=32767.
fn scale_axis(value: i32, min: i32, max: i32) -> i32 {
    if max <= min {
        return 0;
    }
    let v = i64::from(value.clamp(min, max)) - i64::from(min);
    (v * 65535 / (i64::from(max) - i64::from(min)) - 32768) as i32
}

/// Opens hidden devices' nodes for this user (see [`set_opener`]).
pub type Opener = Box<dyn Fn(&Path) -> io::Result<std::os::fd::OwnedFd> + Send + Sync>;

static OPENER: std::sync::RwLock<Option<Opener>> = std::sync::RwLock::new(None);

/// Sets where nodes closed to this user come from: SystemGobloon hands out the nodes of hidden
/// devices (gob-hide sets it when the service runs). `None`: such nodes stay closed.
pub fn set_opener(opener: Option<Opener>) {
    *OPENER.write().unwrap_or_else(|e| e.into_inner()) = opener;
}

/// Opens a device node for non-blocking reads; one closed to this user through the opener.
fn open_node(path: &Path) -> io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    let denied = match std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
    {
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => e,
        other => return other,
    };
    let opener = OPENER.read().unwrap_or_else(|e| e.into_inner());
    match opener.as_ref().map(|open| open(path)) {
        Some(Ok(fd)) => Ok(fd.into()),
        // Not hidden (the service refuses) or no service: the system's answer stands.
        _ => Err(denied),
    }
}

/// The USB device's product string (e.g. "VKBsim Gladiator EVO R"), the name Windows shows. The
/// evdev name has the manufacturer in front ("VKB-Sim (C) Alex Oz 2023  VKBsim ..."), which
/// would keep "Map controls" from matching profiles made on Windows. `None` for devices that
/// are not on USB (e.g. Bluetooth): their evdev name is used.
fn usb_product(event_node: &Path) -> Option<String> {
    let product = std::fs::read_to_string(usb_device(event_node)?.join("product")).ok()?;
    let product = product.trim();
    (!product.is_empty()).then(|| product.to_string())
}

/// The USB device's folder in sysfs behind an event node; `None` off USB.
fn usb_device(event_node: &Path) -> Option<PathBuf> {
    let name = event_node.file_name()?;
    // eventN/device = the input device, its device = the HID device, two levels up = USB.
    let hid = Path::new("/sys/class/input")
        .join(name)
        .join("device/device")
        .canonicalize()
        .ok()?;
    let usb = hid.parent()?.parent()?;
    usb.join("idVendor").exists().then(|| usb.to_path_buf())
}

/// A controller's device nodes, for SystemGobloon (root) to close them to others while hidden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nodes {
    /// Id as in [`crate::DeviceInfo::guid`].
    pub guid: String,
    pub name: String,
    pub vendor: u16,
    pub product: u16,
    /// On USB: the USB device's serial number (udev `ATTRS{serial}`), empty without one.
    pub usb_serial: Option<String>,
    /// The event node, its joydev node and the HID device's hidraw node, as far as present.
    pub paths: Vec<PathBuf>,
}

/// The attached controllers (not our own virtual devices) and their nodes. Needs read access to
/// the event nodes: root.
pub fn controller_nodes() -> Vec<Nodes> {
    use super::Backend;
    let Some(platform) = Platform::new() else {
        return Vec::new();
    };
    platform
        .attached()
        .into_iter()
        .filter_map(|(guid, found)| {
            let node = Node::open(&found.path).ok()?;
            if keyboard::is_own(&node) {
                return None;
            }
            let [_, vendor, product, _] = node.id();
            let usb = usb_device(&found.path);
            let usb_serial = usb.map(|dir| {
                std::fs::read_to_string(dir.join("serial"))
                    .map(|s| s.trim().to_string())
                    .unwrap_or_default()
            });
            let mut paths = vec![found.path.clone()];
            let input = Path::new("/sys/class/input")
                .join(found.path.file_name()?)
                .join("device");
            let joydev = std::fs::read_dir(input).into_iter().flatten().flatten();
            paths.extend(joydev.filter_map(|e| {
                let n = e.file_name();
                n.to_str()?
                    .starts_with("js")
                    .then(|| Path::new("/dev/input").join(&n))
            }));
            let hidraw = hidraw_path(&found.path);
            if !hidraw.is_empty() {
                paths.push(hidraw.into());
            }
            Some(Nodes {
                guid,
                name: node.name(),
                vendor,
                product,
                usb_serial,
                paths,
            })
        })
        .collect()
}

/// The `hidraw` node of the HID device behind an event node (for gob-lighting), if any.
fn hidraw_path(event_node: &Path) -> String {
    let Some(name) = event_node.file_name() else {
        return String::new();
    };
    let dir = Path::new("/sys/class/input")
        .join(name)
        .join("device/device/hidraw");
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .find_map(|e| {
            let n = e.file_name();
            n.to_str()?
                .starts_with("hidraw")
                .then(|| format!("/dev/{}", n.to_string_lossy()))
        })
        .unwrap_or_default()
}

/// Buttons to read from the raw HID node because evdev has fewer: where they sit in the input
/// reports and how many there are (the highest button number).
struct RawPlan {
    path: PathBuf,
    layout: hid::Layout,
    count: usize,
}

impl RawPlan {
    /// A plan when evdev got the kernel's most buttons and the report descriptor of `hidraw`
    /// declares more.
    fn new(hidraw: &str, evdev_buttons: usize) -> Option<Self> {
        if hidraw.is_empty() || evdev_buttons < EVDEV_MAX_BUTTONS {
            return None;
        }
        let path = PathBuf::from(hidraw);
        let mut layout = hid::buttons(&hid::descriptor(&path)?);
        layout
            .buttons
            .retain(|b| usize::from(b.usage) <= MAX_BUTTONS);
        let count = layout.buttons.iter().map(|b| usize::from(b.usage)).max()?;
        (count > evdev_buttons).then_some(Self {
            path,
            layout,
            count,
        })
    }

    fn open(&self) -> io::Result<hid::Raw> {
        hid::Raw::open(&self.path, self.layout.clone())
    }
}

/// How one axis event code maps to the state.
#[derive(Debug, Clone, Copy)]
struct AxisMap {
    slot: usize,
    min: i32,
    max: i32,
}

/// One hat: its axis codes' ranges and current values.
#[derive(Debug, Clone, Copy, Default)]
struct Hat {
    /// (center * 2) of each axis, so `2 * value - center2` has the direction's sign.
    center2: [i32; 2],
    value: [i32; 2],
}

impl Hat {
    fn direction(&self) -> Option<u16> {
        hat_direction(
            2 * self.value[0] - self.center2[0],
            2 * self.value[1] - self.center2[1],
        )
    }
}

pub(crate) struct Device {
    node: Node,
    state: DeviceState,
    previous: DeviceState,
    calibration: [Option<Calibration>; AXIS_SLOTS],
    /// Indexed by axis event code.
    axes: [Option<AxisMap>; ABS_CNT],
    /// Button index per key event code (`u8::MAX`: not a button).
    buttons: Vec<u8>,
    hats: [Hat; MAX_HATS],
    events: bool,
    /// Events were lost (`SYN_DROPPED`): read the whole state after this report.
    dropped: bool,
    /// Grabbed (`EVIOCGRAB`): only this process gets its events.
    grabbed: bool,
    /// Buttons come from the raw HID node (the device has more than evdev passes on).
    raw: Option<hid::Raw>,
    /// The raw HID node could not be opened yet (no access): tried again every [`RAW_RETRY`].
    raw_pending: Option<(RawPlan, Instant)>,
}

impl Device {
    fn open(found: &Found, events: bool) -> io::Result<Self> {
        let node = Node::open(&found.path)?;
        let caps = node.caps();
        let codes = button_codes(&caps.keys);
        let mut buttons = vec![u8::MAX; KEY_CNT];
        for (i, &c) in codes.iter().enumerate() {
            buttons[usize::from(c)] = i as u8;
        }
        let mut axes = [None; ABS_CNT];
        for (code, slot) in axis_slots(&caps.abs) {
            if let Some((_, min, max)) = node.abs_info(code) {
                axes[code] = Some(AxisMap { slot, min, max });
            }
        }
        let hat_count = hat_count(&caps.abs);
        let mut hats = [Hat::default(); MAX_HATS];
        for (n, hat) in hats.iter_mut().enumerate().take(hat_count) {
            for (i, center) in hat.center2.iter_mut().enumerate() {
                if let Some((_, min, max)) = node.abs_info(ABS_HAT0X + 2 * n + i) {
                    *center = min + max;
                }
            }
        }
        let [_, vendor_id, product_id, _] = node.id();
        let mut slots: Vec<u8> = axes.iter().flatten().map(|a| a.slot as u8 + 1).collect();
        slots.sort_unstable();
        let hid_path = hidraw_path(&found.path);
        let (raw, raw_pending) = match RawPlan::new(&hid_path, codes.len()) {
            Some(plan) => match plan.open() {
                Ok(raw) => {
                    log::info!(
                        "{}: {} buttons read from {hid_path}",
                        found.path.display(),
                        plan.count
                    );
                    (Some((raw, plan.count)), None)
                }
                Err(e) => {
                    log::info!(
                        "{}: buttons {}-{} need read access to {hid_path}: {e}",
                        found.path.display(),
                        codes.len() + 1,
                        plan.count
                    );
                    (None, Some((plan, Instant::now() + RAW_RETRY)))
                }
            },
            None => (None, None),
        };
        if raw.is_some() {
            // Every button comes from the raw reports; evdev's key events are ignored.
            buttons.fill(u8::MAX);
        }
        let info = DeviceInfo {
            guid: found.guid.clone(),
            name: usb_product(&found.path).unwrap_or_else(|| node.name()),
            vendor_id,
            product_id,
            axes: slots,
            buttons: raw.as_ref().map_or(codes.len(), |(_, n)| *n),
            hats: hat_count,
            hid_path,
            own: keyboard::is_own(&node),
            unread_buttons: raw_pending
                .as_ref()
                .map_or(0, |(p, _)| p.count - codes.len()),
        };
        let state = DeviceState::new(info);
        let mut dev = Self {
            node,
            previous: state.clone(),
            state,
            calibration: [None; AXIS_SLOTS],
            axes,
            buttons,
            hats,
            events,
            dropped: false,
            grabbed: false,
            raw: raw.map(|(r, _)| r),
            raw_pending,
        };
        dev.sync();
        dev.sync_raw();
        dev.previous.copy_readings(&dev.state);
        Ok(dev)
    }

    /// Reads the whole current state through ioctls (at open and after lost events).
    fn sync(&mut self) {
        let pressed = self.node.pressed();
        for (code, &i) in self.buttons.iter().enumerate() {
            if let Some(b) = self.state.buttons.get_mut(usize::from(i)) {
                *b = pressed.has(code);
            }
        }
        for code in 0..ABS_CNT {
            let is_hat = (ABS_HAT0X..=ABS_HAT3Y).contains(&code);
            if (self.axes[code].is_some() || is_hat)
                && let Some((value, _, _)) = self.node.abs_info(code)
            {
                self.set_abs(code, value);
            }
        }
        for (h, hat) in self.state.hats.iter_mut().zip(&self.hats) {
            *h = hat.direction();
        }
    }

    /// Button `i` (0-based) changed; sent as an event in event mode.
    fn set_button(&mut self, i: usize, pressed: bool, emit: &mut dyn FnMut(InputEvent)) {
        if let Some(b) = self.state.buttons.get_mut(i)
            && *b != pressed
        {
            *b = pressed;
            if self.events {
                emit(InputEvent::Button {
                    device: self.state.info.guid.clone(),
                    id: i as u16 + 1,
                    pressed,
                });
            }
        }
    }

    /// The buttons held right now, from the raw HID node (at open, no events).
    fn sync_raw(&mut self) {
        let Some(raw) = &self.raw else {
            return;
        };
        let buttons = &mut self.state.buttons;
        raw.read_current(&mut |b, pressed| {
            if let Some(v) = buttons.get_mut(usize::from(b) - 1) {
                *v = pressed;
            }
        });
    }

    /// Opens the raw HID node once access is there, and from then on reads every button from it.
    fn retry_raw(&mut self) {
        let Some((plan, at)) = &mut self.raw_pending else {
            return;
        };
        let now = Instant::now();
        if now < *at {
            return;
        }
        *at = now + RAW_RETRY;
        let Ok(raw) = plan.open() else {
            return;
        };
        let count = plan.count;
        log::info!(
            "{}: {count} buttons read from {}",
            self.state.info.name,
            plan.path.display()
        );
        self.raw_pending = None;
        self.raw = Some(raw);
        self.buttons.fill(u8::MAX);
        for s in [&mut self.state, &mut self.previous] {
            s.info.buttons = count;
            s.info.unread_buttons = 0;
            s.buttons.resize(count, false);
        }
        // The input filter sends what changed against its own view.
        self.sync_raw();
    }

    fn set_axis(&mut self, axis: AxisMap, value: i32) {
        let raw = scale_axis(value, axis.min, axis.max);
        self.state.raw[axis.slot] = raw;
        self.state.axes[axis.slot] = match &self.calibration[axis.slot] {
            Some(c) => c.apply(raw),
            None => normalize_axis(raw),
        };
    }

    /// An axis or hat axis value; hats take effect at the end of the report.
    fn set_abs(&mut self, code: usize, value: i32) {
        if let Some(axis) = self.axes.get(code).copied().flatten() {
            self.set_axis(axis, value);
        } else if (ABS_HAT0X..=ABS_HAT3Y).contains(&code) {
            let n = (code - ABS_HAT0X) / 2;
            self.hats[n].value[(code - ABS_HAT0X) % 2] = value;
        }
    }

    /// End of a report: hat changes become directions (both hat axes of a diagonal arrive in
    /// one report).
    fn end_report(&mut self, emit: &mut dyn FnMut(InputEvent)) {
        for (n, hat) in self.hats.iter().enumerate().take(self.state.hats.len()) {
            let direction = hat.direction();
            if self.state.hats[n] != direction {
                self.state.hats[n] = direction;
                if self.events {
                    emit(InputEvent::Hat {
                        device: self.state.info.guid.clone(),
                        id: n as u8 + 1,
                        direction,
                    });
                }
            }
        }
    }

    fn apply(&mut self, event: &evdev::Event, emit: &mut dyn FnMut(InputEvent)) {
        let code = usize::from(event.code);
        match event.type_ {
            EV_SYN if event.code == SYN_DROPPED => self.dropped = true,
            EV_SYN if event.code == SYN_REPORT => {
                if std::mem::take(&mut self.dropped) {
                    self.sync();
                } else {
                    self.end_report(emit);
                }
            }
            _ if self.dropped => {}
            EV_KEY => {
                let Some(&i) = self.buttons.get(code) else {
                    return;
                };
                self.set_button(usize::from(i), event.value != 0, emit);
            }
            EV_ABS => self.set_abs(code, event.value),
            _ => {}
        }
    }
}

impl super::Device for Device {
    fn set_exclusive(&mut self, on: bool) {
        if on == self.grabbed {
            return;
        }
        match self.node.grab(on) {
            Ok(()) => {
                self.grabbed = on;
                log::info!(
                    "{}: {}",
                    self.state.info.name,
                    if on {
                        "hidden (grabbed)"
                    } else {
                        "shown again"
                    }
                );
            }
            Err(e) => log::warn!("{}: grab {on}: {e}", self.state.info.name),
        }
    }

    fn state(&self) -> &DeviceState {
        &self.state
    }

    fn previous(&self) -> &DeviceState {
        &self.previous
    }

    fn set_calibrations(&mut self, calibrations: &Calibrations) {
        for (i, c) in self.calibration.iter_mut().enumerate() {
            *c = calibrations
                .get(&(self.state.info.guid.clone(), i as u8 + 1))
                .copied();
        }
    }

    /// Applies every waiting event in order. In event mode button and hat changes are sent
    /// as they come (so short presses are kept); in polling mode the caller's diff finds them.
    fn read(&mut self, emit: &mut dyn FnMut(InputEvent)) -> bool {
        self.previous.copy_readings(&self.state);
        // SAFETY: `input_event` is plain data; all zeroes is a valid value.
        let mut buf: [evdev::Event; 64] = unsafe { std::mem::zeroed() };
        loop {
            let count = match self.node.read(&mut buf) {
                Ok(n) => n,
                Err(_) => return false,
            };
            for event in &buf[..count] {
                self.apply(event, emit);
            }
            if count < buf.len() {
                break;
            }
        }
        match self.raw.take() {
            Some(raw) => {
                let read = raw.read(&mut |b, pressed| {
                    self.set_button(usize::from(b) - 1, pressed, emit);
                });
                if read.is_err() {
                    return false;
                }
                self.raw = Some(raw);
            }
            None => self.retry_raw(),
        }
        if self.events {
            // Already sent: the diff must not send these again.
            self.previous.buttons.clone_from(&self.state.buttons);
            self.previous.hats.clone_from(&self.state.hats);
        }
        true
    }
}

/// The joydev numbers (`/dev/input/jsN`, what `jstest` and older games use) of the connected
/// controllers with this vendor and product id.
pub(crate) fn joystick_ids(vendor_id: u16, product_id: u16) -> Vec<u32> {
    let hex = |path: PathBuf| {
        let text = std::fs::read_to_string(path).ok()?;
        u16::from_str_radix(text.trim(), 16).ok()
    };
    let Ok(dir) = std::fs::read_dir("/sys/class/input") else {
        return Vec::new();
    };
    let mut ids: Vec<u32> = dir
        .flatten()
        .filter_map(|e| {
            let n: u32 = e.file_name().to_str()?.strip_prefix("js")?.parse().ok()?;
            let id = e.path().join("device/id");
            (hex(id.join("vendor"))? == vendor_id && hex(id.join("product"))? == product_id)
                .then_some(n)
        })
        .collect();
    ids.sort_unstable();
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    const BTN_MISC: usize = 0x100;
    const BTN_TRIGGER: usize = 0x120;
    const BTN_SOUTH: usize = 0x130;
    const BTN_LEFT: usize = 0x110;
    const ABS_X: usize = 0;
    const ABS_Y: usize = 1;
    const ABS_THROTTLE: usize = 0x06;
    const ABS_RUDDER: usize = 0x07;
    const ABS_WHEEL: usize = 0x08;

    fn caps(keys: &[usize], abs: &[usize]) -> Caps {
        let mut events = vec![EV_SYN.into()];
        if !keys.is_empty() {
            events.push(EV_KEY.into());
        }
        if !abs.is_empty() {
            events.push(EV_ABS.into());
        }
        Caps {
            events: Bits::of(&events),
            keys: Bits::of(keys),
            abs: Bits::of(abs),
            props: Bits::default(),
        }
    }

    #[test]
    fn joysticks_and_not() {
        assert!(is_joystick(&caps(&[BTN_TRIGGER], &[ABS_X, ABS_Y])), "stick");
        assert!(is_joystick(&caps(&[BTN_SOUTH], &[ABS_X, ABS_Y])), "gamepad");
        assert!(is_joystick(&caps(&[], &[ABS_X, ABS_Y, 0x05])), "pedals");
        assert!(
            is_joystick(&caps(&[BTN_TRIGGER_HAPPY + 3], &[])),
            "button box"
        );
        assert!(
            !is_joystick(&caps(&[BTN_LEFT], &[ABS_X, ABS_Y])),
            "absolute mouse"
        );
        assert!(
            !is_joystick(&caps(&[BTN_TOUCH], &[ABS_X, ABS_Y])),
            "touch screen"
        );
        assert!(!is_joystick(&caps(&[30, 31, 32], &[])), "keyboard");
        let mut motion = caps(&[], &[ABS_X, ABS_Y, 0x02]);
        motion.props = Bits::of(&[INPUT_PROP_ACCELEROMETER]);
        assert!(!is_joystick(&motion), "motion sensor");
    }

    #[test]
    fn axes_take_directinput_slots() {
        let abs = Bits::of(&[
            ABS_X,
            ABS_Y,
            ABS_RZ,
            ABS_HAT0X,
            ABS_RUDDER,
            ABS_THROTTLE,
            ABS_WHEEL,
        ]);
        assert_eq!(
            axis_slots(&abs),
            vec![
                (ABS_X, 0),
                (ABS_Y, 1),
                (ABS_RZ, 5),
                (ABS_THROTTLE, 6),
                (ABS_RUDDER, 7)
            ]
        );
        assert_eq!(hat_count(&abs), 1);
        assert_eq!(hat_count(&Bits::of(&[ABS_HAT0X + 5])), 3);
    }

    #[test]
    fn buttons_in_sdl_order() {
        let keys = Bits::of(&[BTN_TRIGGER_HAPPY, BTN_MISC, BTN_TRIGGER + 1, BTN_TRIGGER]);
        assert_eq!(
            button_codes(&keys),
            vec![
                BTN_TRIGGER as u16,
                BTN_TRIGGER as u16 + 1,
                BTN_TRIGGER_HAPPY as u16,
                BTN_MISC as u16
            ]
        );
    }

    #[test]
    fn hats_and_axis_scaling() {
        assert_eq!(hat_direction(0, 0), None);
        assert_eq!(hat_direction(0, -1), Some(0));
        assert_eq!(hat_direction(1, 1), Some(13500));
        assert_eq!(hat_direction(-1, -1), Some(31500));
        let hat = Hat {
            center2: [2, 2],
            value: [2, 0],
        };
        assert_eq!(hat.direction(), Some(4500), "0..2 range: 2 = right, 0 = up");
        assert_eq!(scale_axis(0, 0, 65535), -32768);
        assert_eq!(scale_axis(65535, 0, 65535), 32767);
        assert_eq!(scale_axis(-1, -1, 1), -32768);
        assert_eq!(scale_axis(1, -1, 1), 32767);
        assert_eq!(scale_axis(5, 3, 3), 0);
    }

    #[test]
    fn ids_are_stable_and_distinct() {
        let a = device_guid(
            0x231d,
            0x0200,
            3,
            &identity("", "usb-0000:00:14.0-2/input0", "VKB"),
        );
        assert_eq!(
            a,
            device_guid(
                0x231d,
                0x0200,
                3,
                &identity("", "usb-0000:00:14.0-2/input0", "VKB")
            )
        );
        assert!(a.starts_with("0200231d-0003-0000-"), "{a}");
        assert_eq!(a.len(), 36);
        let b = device_guid(
            0x231d,
            0x0200,
            3,
            &identity("", "usb-0000:00:14.0-3/input0", "VKB"),
        );
        assert_ne!(a, b);
        let serial = identity("ABC123", "anywhere", "VKB");
        assert_eq!(serial, identity("ABC123", "elsewhere", "VKB"));
        assert_eq!(
            identity("", "joystick-gobloon/joystick-2", "JoyGob 2"),
            identity("", "joystick-gobloon/joystick-2", "Gobloon Throttle"),
            "our devices keep their id when renamed"
        );
    }
}
