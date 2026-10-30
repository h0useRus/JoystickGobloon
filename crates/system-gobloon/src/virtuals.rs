//! Virtual devices: the service makes every joystick and gamepad of `virtual.toml` when it
//! starts (in id order, so their order in games stays the same) and keeps them until it stops,
//! like vJoy's devices on Windows. A client acquires one and gets its uinput descriptor, writes
//! events itself; when it releases the device or its connection ends, the device is centered.
//! Layouts are gob-virtual's (`gob_virtual::uinput`), the file has `virtual.gob`'s format.
//! The keyboard and mouse of keyboard and mouse output are made when first acquired and kept
//! too; released, their keys and buttons go up.

use std::collections::BTreeMap;
use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::{Path, PathBuf};

use gob_system::protocol::{
    ApplyVirtual, ChangeKind, DeviceKind, ErrorCode, Gamepad, Joystick, Owner,
};
use gob_uinput::VirtualDevice;
use gob_virtual::uinput::{self, Layout};
use gob_virtual::{Change, DeviceConfig};

const STATE_FILE: &str = "virtual.toml";

/// A client connection, to know who has a device.
pub type Conn = u64;

/// A kept device; tests use something else than uinput.
pub trait Held: Send {
    fn emit(&self, events: &[(u16, u16, i32)]) -> io::Result<()>;
    /// What the client writes its events to.
    fn fd(&self) -> BorrowedFd<'_>;
}

impl Held for VirtualDevice {
    fn emit(&self, events: &[(u16, u16, i32)]) -> io::Result<()> {
        VirtualDevice::emit(self, events)
    }

    fn fd(&self) -> BorrowedFd<'_> {
        self.as_fd()
    }
}

/// Makes devices.
pub trait Factory: Send {
    fn joystick(&self, config: &DeviceConfig) -> io::Result<Box<dyn Held>>;
    fn gamepad(&self, id: u32) -> io::Result<Box<dyn Held>>;
    fn keyboard(&self) -> io::Result<Box<dyn Held>>;
    fn mouse(&self) -> io::Result<Box<dyn Held>>;
}

/// uinput, as gob-virtual makes the devices.
pub struct Uinput;

impl Factory for Uinput {
    fn joystick(&self, config: &DeviceConfig) -> io::Result<Box<dyn Held>> {
        Ok(Box::new(uinput::create_joystick(config)?))
    }

    fn gamepad(&self, id: u32) -> io::Result<Box<dyn Held>> {
        Ok(Box::new(uinput::create_gamepad(id)?))
    }

    fn keyboard(&self) -> io::Result<Box<dyn Held>> {
        Ok(Box::new(gob_uinput::output::keyboard()?))
    }

    fn mouse(&self) -> io::Result<Box<dyn Held>> {
        Ok(Box::new(gob_uinput::output::mouse()?))
    }
}

struct Kept {
    held: Box<dyn Held>,
    owner: Option<Conn>,
}

/// The kept devices and their layouts.
pub struct Virtuals {
    layout: Layout,
    path: PathBuf,
    factory: Box<dyn Factory>,
    /// By kind and id (keyboard and mouse: id 0).
    devices: BTreeMap<(DeviceKind, u32), Kept>,
}

fn owner(kept: Option<&Kept>, conn: Conn) -> Owner {
    match kept.and_then(|k| k.owner) {
        None => Owner::Free,
        Some(c) if c == conn => Owner::You,
        Some(_) => Owner::Other,
    }
}

impl Virtuals {
    /// Reads the layouts from `state_dir` (no file: one joystick, as without the service) and
    /// makes the devices.
    pub fn load(state_dir: &Path, factory: Box<dyn Factory>) -> Virtuals {
        let path = state_dir.join(STATE_FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => Some(text),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => {
                log::error!("{}: {e}", path.display());
                None
            }
        };
        let layout = uinput::parse(text.as_deref()).unwrap_or_else(|e| {
            log::error!("{e}");
            uinput::parse(None).expect("default layout")
        });
        let mut virtuals = Virtuals {
            layout,
            path,
            factory,
            devices: BTreeMap::new(),
        };
        let configs = virtuals.layout.devices.clone();
        configs.iter().for_each(|c| virtuals.make_joystick(c));
        let pads = virtuals.layout.gamepads.clone();
        pads.iter().for_each(|&id| virtuals.make_gamepad(id));
        virtuals
    }

    fn make_joystick(&mut self, config: &DeviceConfig) {
        match self.factory.joystick(config) {
            Ok(held) => {
                log::info!("joystick {} made", config.id);
                let kept = Kept { held, owner: None };
                self.devices.insert((DeviceKind::Joystick, config.id), kept);
            }
            Err(e) => log::error!("joystick {}: {e}", config.id),
        }
    }

    fn make_gamepad(&mut self, id: u32) {
        match self.factory.gamepad(id) {
            Ok(held) => {
                log::info!("gamepad {id} made");
                let kept = Kept { held, owner: None };
                self.devices.insert((DeviceKind::Gamepad, id), kept);
            }
            Err(e) => log::error!("gamepad {id}: {e}"),
        }
    }

    /// The devices with their owner as `conn` sees them.
    pub fn state(&self, conn: Conn) -> (Vec<Joystick>, Vec<Gamepad>) {
        let joysticks = self
            .layout
            .devices
            .iter()
            .map(|c| {
                let kept = self.devices.get(&(DeviceKind::Joystick, c.id));
                uinput::joystick_record(c, owner(kept, conn))
            })
            .collect();
        let gamepads = self
            .layout
            .gamepads
            .iter()
            .map(|&id| Gamepad {
                id,
                owner: owner(self.devices.get(&(DeviceKind::Gamepad, id)), conn) as u8,
                reserved: [0; 3],
            })
            .collect();
        (joysticks, gamepads)
    }

    /// Creates, replaces or deletes a device; [`ErrorCode::Busy`] while anyone has it.
    pub fn apply(&mut self, record: &ApplyVirtual) -> Result<(), ErrorCode> {
        let id = record.joystick.id;
        let kind = ChangeKind::from_u8(record.kind).ok_or(ErrorCode::BadRequest)?;
        let (change, device) = match kind {
            ChangeKind::SetJoystick => (
                Change::Set(uinput::joystick_config(&record.joystick)),
                DeviceKind::Joystick,
            ),
            ChangeKind::DeleteJoystick => (Change::Delete(id), DeviceKind::Joystick),
            ChangeKind::AddGamepad => (Change::AddGamepad(id), DeviceKind::Gamepad),
            ChangeKind::DeleteGamepad => (Change::DeleteGamepad(id), DeviceKind::Gamepad),
        };
        if self
            .devices
            .get(&(device, id))
            .is_some_and(|k| k.owner.is_some())
        {
            return Err(ErrorCode::Busy);
        }
        let layout = uinput::changed(self.layout.clone(), &change).map_err(|e| {
            log::warn!("{e}");
            ErrorCode::BadRequest
        })?;
        if layout == self.layout {
            return Ok(());
        }
        let text = uinput::to_text(&layout).map_err(|e| {
            log::error!("{e}");
            ErrorCode::Failed
        })?;
        if let Err(e) = write(&self.path, &text) {
            log::error!("{}: {e}", self.path.display());
            return Err(ErrorCode::Failed);
        }
        self.layout = layout;
        // uinput devices cannot change their layout: made again.
        match change {
            Change::Set(c) => {
                self.devices.remove(&(DeviceKind::Joystick, c.id));
                self.make_joystick(&c);
            }
            Change::Delete(id) => {
                self.devices.remove(&(DeviceKind::Joystick, id));
                log::info!("joystick {id} removed");
            }
            Change::AddGamepad(id) => {
                if !self.devices.contains_key(&(DeviceKind::Gamepad, id)) {
                    self.make_gamepad(id);
                }
            }
            Change::DeleteGamepad(id) => {
                self.devices.remove(&(DeviceKind::Gamepad, id));
                log::info!("gamepad {id} removed");
            }
            Change::Enable(_) => {}
        }
        Ok(())
    }

    /// Gives `conn` a device: its descriptor. [`ErrorCode::Busy`] when another connection has
    /// it, [`ErrorCode::NotFound`] when there is no such device. The keyboard and mouse are
    /// made the first time.
    pub fn acquire(&mut self, conn: Conn, kind: DeviceKind, id: u32) -> Result<OwnedFd, ErrorCode> {
        let key = (kind, own_id(kind, id));
        if !self.devices.contains_key(&key)
            && matches!(kind, DeviceKind::Keyboard | DeviceKind::Mouse)
        {
            let made = if kind == DeviceKind::Keyboard {
                self.factory.keyboard()
            } else {
                self.factory.mouse()
            };
            let held = made.map_err(|e| {
                log::error!("{kind:?}: {e}");
                ErrorCode::Failed
            })?;
            log::info!("{kind:?} made");
            self.devices.insert(key, Kept { held, owner: None });
        }
        let kept = self.devices.get_mut(&key).ok_or(ErrorCode::NotFound)?;
        if kept.owner.is_some_and(|c| c != conn) {
            return Err(ErrorCode::Busy);
        }
        let fd = kept.held.fd().try_clone_to_owned().map_err(|e| {
            log::error!("{e}");
            ErrorCode::Failed
        })?;
        kept.owner = Some(conn);
        Ok(fd)
    }

    /// Takes a device back from `conn` and centers it (nothing when `conn` does not have it).
    pub fn release(&mut self, conn: Conn, kind: DeviceKind, id: u32) {
        let id = own_id(kind, id);
        let reset = match kind {
            DeviceKind::Gamepad => uinput::gamepad_reset(),
            DeviceKind::Keyboard => gob_uinput::output::keyboard_release(),
            DeviceKind::Mouse => gob_uinput::output::mouse_release(),
            DeviceKind::Joystick => match self.layout.devices.iter().find(|c| c.id == id) {
                Some(c) => uinput::joystick_reset(c),
                None => return,
            },
        };
        if let Some(kept) = self
            .devices
            .get_mut(&(kind, id))
            .filter(|k| k.owner == Some(conn))
        {
            kept.owner = None;
            if let Err(e) = kept.held.emit(&reset) {
                log::warn!("releasing {kind:?} {id}: {e}");
            }
        }
    }

    /// Everything `conn` has, released (its connection ended).
    pub fn release_all(&mut self, conn: Conn) {
        let owned: Vec<(DeviceKind, u32)> = self
            .devices
            .iter()
            .filter(|(_, k)| k.owner == Some(conn))
            .map(|(&key, _)| key)
            .collect();
        for (kind, id) in owned {
            self.release(conn, kind, id);
        }
    }
}

/// The keyboard and the mouse are one each: id 0.
fn own_id(kind: DeviceKind, id: u32) -> u32 {
    match kind {
        DeviceKind::Keyboard | DeviceKind::Mouse => 0,
        DeviceKind::Joystick | DeviceKind::Gamepad => id,
    }
}

fn write(path: &Path, text: &str) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}
