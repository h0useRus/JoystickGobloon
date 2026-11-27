//! Keyboard and mouse output through uinput: a virtual keyboard and a virtual mouse that the
//! desktop (X11 or Wayland) and games read like real ones. With SystemGobloon running they are
//! the service's, lent to this program (no access to /dev/uinput needed); otherwise made here.
//! Their `phys` marks them as ours, so gob-input does not read our own keys back as input.

use std::io;
use std::sync::Mutex;

use gob_core::MouseButton;
use gob_system::Client;
use gob_system::protocol::{DeviceKind, caps};
use gob_uinput::{Emitter, VirtualDevice, codes, keys, output};

pub(crate) struct Injector;

/// Where a device's events go.
enum Sink {
    Own(VirtualDevice),
    Lent(Emitter),
}

impl Sink {
    fn emit(&self, events: &[(u16, u16, i32)]) -> io::Result<()> {
        match self {
            Sink::Own(dev) => dev.emit(events),
            Sink::Lent(emitter) => emitter.emit(events),
        }
    }
}

/// The devices, made on first use and kept for the rest of the program.
#[derive(Default)]
struct Devices {
    keyboard: Option<Sink>,
    mouse: Option<Sink>,
    /// The connection the lent devices belong to: closing it gives them back.
    service: Option<Client>,
    /// Creating them failed (e.g. no access to /dev/uinput): logged, and not tried again until
    /// the next profile start.
    failed: bool,
}

static DEVICES: Mutex<Option<Devices>> = Mutex::new(None);

/// Reported to the service's log.
const CLIENT_VERSION: &str = match option_env!("JOYSTICK_GOBLOON_VERSION") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};

/// The keyboard and mouse SystemGobloon keeps, when it runs and has them free.
fn lent() -> Option<(Client, Sink, Sink)> {
    let mut client = Client::connect().ok()?;
    let hello = client.hello(CLIENT_VERSION).ok()?;
    if hello.capabilities & caps::KEYBOARD_MOUSE == 0 {
        return None;
    }
    let mut take = |kind| match client.acquire(kind, 0) {
        Ok(fd) => Some(Sink::Lent(Emitter::new(fd))),
        Err(e) => {
            log::warn!("SystemGobloon {kind:?}: {e}");
            None
        }
    };
    let (keyboard, mouse) = (take(DeviceKind::Keyboard)?, take(DeviceKind::Mouse)?);
    Some((client, keyboard, mouse))
}

impl Devices {
    fn create(&mut self) {
        // The service stopped or restarted: the lent descriptors would keep copies of its
        // devices alive next to the new service's own.
        if self.service.as_ref().is_some_and(Client::gone) {
            log::info!("SystemGobloon stopped: keyboard and mouse given up");
            (self.keyboard, self.mouse, self.service) = (None, None, None);
        }
        if self.failed || (self.keyboard.is_some() && self.mouse.is_some()) {
            return;
        }
        if let Some((client, keyboard, mouse)) = lent() {
            log::info!("keyboard and mouse output through SystemGobloon");
            (self.keyboard, self.mouse) = (Some(keyboard), Some(mouse));
            self.service = Some(client);
            return;
        }
        let created = output::keyboard().and_then(|k| Ok((k, output::mouse()?)));
        match created {
            Ok((k, m)) => (self.keyboard, self.mouse) = (Some(Sink::Own(k)), Some(Sink::Own(m))),
            Err(e) => {
                self.failed = true;
                log::error!("keyboard and mouse output is off: /dev/uinput: {e}");
            }
        }
    }
}

fn devices<R>(f: impl FnOnce(&mut Devices) -> R) -> R {
    let mut guard = DEVICES.lock().unwrap_or_else(|e| e.into_inner());
    let devices = guard.get_or_insert_default();
    devices.create();
    f(devices)
}

/// Sends one report to the mouse (`mouse`) or the keyboard, making them if needed.
fn emit(mouse: bool, events: &[(u16, u16, i32)]) {
    devices(|d| {
        let device = if mouse { &d.mouse } else { &d.keyboard };
        if let Some(device) = device
            && let Err(e) = device.emit(events)
        {
            log::error!("keyboard/mouse output: {e}");
        }
    });
}

impl crate::platform::Injector for Injector {
    /// Makes the devices before the first key: the desktop needs a moment to open a new
    /// device, and events sent before that are lost.
    fn prepare(&self) {
        let mut guard = DEVICES.lock().unwrap_or_else(|e| e.into_inner());
        let devices = guard.get_or_insert_default();
        devices.failed = false;
        devices.create();
    }

    fn mouse_move(&self, dx: i32, dy: i32) {
        let events: Vec<_> = [(codes::REL_X, dx), (codes::REL_Y, dy)]
            .into_iter()
            .filter(|&(_, v)| v != 0)
            .map(|(code, v)| (codes::EV_REL, code, v))
            .collect();
        if !events.is_empty() {
            emit(true, &events);
        }
    }

    fn mouse_button(&self, button: MouseButton, pressed: bool) {
        let code = match button {
            MouseButton::Left => codes::BTN_LEFT,
            MouseButton::Right => codes::BTN_RIGHT,
            MouseButton::Middle => codes::BTN_MIDDLE,
            MouseButton::Back => codes::BTN_SIDE,
            MouseButton::Forward => codes::BTN_EXTRA,
            MouseButton::WheelUp | MouseButton::WheelDown => return,
        };
        emit(true, &[(codes::EV_KEY, code, i32::from(pressed))]);
    }

    fn mouse_wheel(&self, notches: i32) {
        if notches != 0 {
            emit(true, &[(codes::EV_REL, codes::REL_WHEEL, notches)]);
        }
    }

    fn key(&self, scan_code: u16, extended: bool, pressed: bool) {
        match keys::key_code(scan_code, extended) {
            Some(code) => emit(false, &[(codes::EV_KEY, code, i32::from(pressed))]),
            None => log::debug!("no Linux key for scan code {scan_code:#x} (E0: {extended})"),
        }
    }

    /// Whether any keyboard holds the key, our own virtual one included (like Windows'
    /// GetAsyncKeyState, which sees injected keys too).
    fn key_pressed(&self, scan_code: u16, extended: bool) -> bool {
        gob_input::key_held(scan_code, extended)
    }
}

#[cfg(test)]
mod tests;
