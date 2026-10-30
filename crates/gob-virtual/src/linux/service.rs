//! Virtual devices through SystemGobloon (crate `system-gobloon`), when it runs: the service
//! keeps every configured joystick and gamepad all the time, so games see them from their start
//! and do not lose them when a profile stops or Joystick Gobloon restarts; without an owner a
//! device stays centered. Acquiring lends this program the device's uinput descriptor, and it
//! writes the events itself as without the service. Layouts change without administrator rights.
//!
//! The first time, the layouts of `virtual.gob` move to the service; the file is then renamed to
//! `virtual.gob.migrated`.

use std::sync::{Arc, Mutex, PoisonError};

use gob_system::protocol::{
    ApplyVirtual, ChangeKind, DeviceKind, ErrorCode, Joystick, Owner, Text, caps,
};
use gob_system::{Client, Error as ServiceError};
use gob_uinput::Emitter;

use super::{CAPABILITIES as DIRECT, Device, Pad, Slot, config_path, info, owned, parse};
use crate::{
    AXIS_SLOTS, Backend, Capabilities, Change, DeviceConfig, DeviceInfo, Error, GamepadInfo,
    Outcome, Status,
};

pub const CAPABILITIES: Capabilities = Capabilities {
    name: "SystemGobloon",
    ..DIRECT
};

/// Reported to the service's log.
const CLIENT_VERSION: &str = match option_env!("JOYSTICK_GOBLOON_VERSION") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};

type Shared = Arc<Mutex<Option<Client>>>;

pub struct Service {
    client: Shared,
    version: String,
}

/// A joystick's layout as the protocol carries it.
pub fn joystick_record(c: &DeviceConfig, owner: Owner) -> Joystick {
    Joystick {
        id: c.id,
        buttons: c.buttons.min(u32::from(u16::MAX)) as u16,
        axes: (0..AXIS_SLOTS)
            .filter(|&i| c.axes[i])
            .fold(0, |bits, i| bits | 1 << i),
        hats: c.hats.min(u32::from(u8::MAX)) as u8,
        flags: if c.discrete_hats {
            Joystick::DISCRETE_HATS
        } else {
            0
        },
        owner: owner as u8,
        product: c.product.unwrap_or(0),
        name: Text::new(c.name.as_deref().unwrap_or_default()),
    }
}

/// A joystick record's layout, limited to what uinput devices here can have.
pub fn joystick_config(j: &Joystick) -> DeviceConfig {
    let mut axes = [false; AXIS_SLOTS];
    for (i, a) in axes.iter_mut().enumerate() {
        *a = j.axes & (1 << i) != 0;
    }
    DeviceConfig {
        id: j.id,
        axes,
        buttons: u32::from(j.buttons).min(DIRECT.max_buttons),
        hats: u32::from(j.hats).min(DIRECT.max_hats),
        discrete_hats: j.flags & Joystick::DISCRETE_HATS != 0,
        effects: Vec::new(),
        name: Some(j.name.as_str())
            .filter(|n| crate::name_allowed(n))
            .map(str::to_string),
        product: Some(j.product).filter(|&p| crate::product_allowed(p)),
    }
}

/// The service's layouts written to `virtual.gob` (it is about to be removed); without a
/// running service, nothing.
pub fn keep_locally() -> Result<(), String> {
    let Ok(mut client) = Client::connect() else {
        return Ok(());
    };
    let (joysticks, gamepads) = client.virtual_state().map_err(|e| e.to_string())?;
    let layout = super::Layout {
        devices: joysticks.iter().map(joystick_config).collect(),
        gamepads: gamepads.iter().map(|g| g.id).collect(),
    };
    let path = config_path().ok_or("no folder for the configuration")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, super::to_text(&layout)?)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let _ = std::fs::remove_file(path.with_extension("gob.migrated"));
    Ok(())
}

fn change_record(change: &Change) -> Option<ApplyVirtual> {
    let id_only = |id: u32| Joystick {
        id,
        buttons: 0,
        axes: 0,
        hats: 0,
        flags: 0,
        owner: 0,
        product: 0,
        name: Text::new(""),
    };
    let (kind, joystick) = match change {
        Change::Set(c) => (ChangeKind::SetJoystick, joystick_record(c, Owner::Free)),
        Change::Delete(id) => (ChangeKind::DeleteJoystick, id_only(*id)),
        Change::AddGamepad(id) => (ChangeKind::AddGamepad, id_only(*id)),
        Change::DeleteGamepad(id) => (ChangeKind::DeleteGamepad, id_only(*id)),
        Change::Enable(_) => return None,
    };
    Some(ApplyVirtual {
        kind: kind as u8,
        reserved: [0; 3],
        joystick,
    })
}

/// The service's devices: joystick layouts and gamepad ids, each with its owner.
struct State {
    joysticks: Vec<(DeviceConfig, Owner)>,
    gamepads: Vec<(u32, Owner)>,
}

/// Runs `f` on the connection, connecting again once when it broke (service restarted).
fn call<T>(
    client: &Shared,
    mut f: impl FnMut(&mut Client) -> Result<T, ServiceError>,
) -> Result<T, ServiceError> {
    let mut client = client.lock().unwrap_or_else(PoisonError::into_inner);
    for retry in [true, false] {
        if client.is_none() {
            *client = Some(Client::connect()?);
        }
        let Some(c) = client.as_mut() else { continue };
        match f(c) {
            Err(ServiceError::Io(e)) if retry => {
                log::info!("SystemGobloon connection: {e}; connecting again");
                *client = None;
            }
            other => return other,
        }
    }
    Err(ServiceError::Protocol("no connection"))
}

/// The status of a device the service has, for this process.
fn status(slot: Slot, owner: Owner) -> Status {
    match owner {
        Owner::You if owned(|o| o.contains(&slot)) => Status::Own,
        // Our connection, but nothing here holds it any more (released meanwhile).
        Owner::You | Owner::Free => Status::Free,
        Owner::Other => Status::Busy,
    }
}

fn unavailable(id: u32, e: &ServiceError) -> Error {
    match e {
        ServiceError::Service(ErrorCode::Busy) => Error::Unavailable {
            id,
            status: Status::Busy,
        },
        ServiceError::Service(ErrorCode::NotFound) => Error::Unavailable {
            id,
            status: Status::Missing,
        },
        other => Error::NotInstalled(other.to_string()),
    }
}

impl Service {
    /// The service, when it runs and keeps virtual devices.
    pub fn connect() -> Option<Service> {
        let mut client = Client::connect().ok()?;
        let hello = match client.hello(CLIENT_VERSION) {
            Ok(h) => h,
            Err(e) => {
                log::warn!("{e}");
                return None;
            }
        };
        if hello.capabilities & caps::VIRTUAL_JOYSTICKS == 0 {
            return None;
        }
        let service = Service {
            client: Arc::new(Mutex::new(Some(client))),
            version: hello.service_version.as_str().to_string(),
        };
        log::info!("virtual devices through SystemGobloon {}", service.version);
        service.migrate();
        Some(service)
    }

    fn state(&self) -> Result<State, ServiceError> {
        let (joysticks, gamepads) = call(&self.client, Client::virtual_state)?;
        Ok(State {
            joysticks: joysticks
                .iter()
                .map(|j| (joystick_config(j), Owner::from_u8(j.owner)))
                .collect(),
            gamepads: gamepads
                .iter()
                .map(|g| (g.id, Owner::from_u8(g.owner)))
                .collect(),
        })
    }

    fn state_or_log(&self) -> Option<State> {
        self.state().inspect_err(|e| log::warn!("{e}")).ok()
    }

    /// Moves the layouts of `virtual.gob` to the service, unless one of its devices is in use.
    fn migrate(&self) {
        let Some(path) = config_path().filter(|p| p.exists()) else {
            return;
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return;
        };
        let (local, Ok(remote)) = (parse(Some(&text)), self.state()) else {
            return;
        };
        let local = match local {
            Ok(l) => l,
            Err(e) => {
                log::warn!("{e}");
                return;
            }
        };
        let busy = remote.joysticks.iter().any(|(_, o)| *o != Owner::Free)
            || remote.gamepads.iter().any(|(_, o)| *o != Owner::Free);
        if busy {
            return;
        }
        let mut changes: Vec<Change> = remote
            .joysticks
            .iter()
            .filter(|(c, _)| !local.devices.iter().any(|l| l.id == c.id))
            .map(|(c, _)| Change::Delete(c.id))
            .collect();
        changes.extend(
            local
                .devices
                .iter()
                .filter(|l| !remote.joysticks.iter().any(|(c, _)| c == *l))
                .cloned()
                .map(Change::Set),
        );
        changes.extend(
            remote
                .gamepads
                .iter()
                .filter(|(id, _)| !local.gamepads.contains(id))
                .map(|(id, _)| Change::DeleteGamepad(*id)),
        );
        changes.extend(
            local
                .gamepads
                .iter()
                .filter(|id| !remote.gamepads.iter().any(|(g, _)| g == *id))
                .map(|&id| Change::AddGamepad(id)),
        );
        for change in &changes {
            if let Err(e) = self.apply(change) {
                log::warn!("moving virtual devices to SystemGobloon: {e}");
                return;
            }
        }
        log::info!("virtual device layouts moved to SystemGobloon");
        let done = path.with_extension("gob.migrated");
        if let Err(e) = std::fs::rename(&path, &done) {
            log::warn!("{}: {e}", path.display());
        }
    }

    /// Takes a device from the service; `release` gives it back when the handle is dropped.
    fn lend(&self, slot: Slot) -> Result<(Emitter, super::Release), Error> {
        let (kind, id) = match slot {
            Slot::Joystick(id) => (DeviceKind::Joystick, id),
            Slot::Gamepad(id) => (DeviceKind::Gamepad, id),
        };
        if !owned(|o| o.insert(slot)) {
            return Err(Error::Unavailable {
                id,
                status: Status::Own,
            });
        }
        let fd = match call(&self.client, |c| c.acquire(kind, id)) {
            Ok(fd) => fd,
            Err(e) => {
                owned(|o| o.remove(&slot));
                return Err(unavailable(id, &e));
            }
        };
        let client = self.client.clone();
        let release = Box::new(move || {
            if let Err(e) = call(&client, |c| c.release(kind, id)) {
                log::warn!("giving {} back: {e}", slot.name());
            }
        });
        Ok((Emitter::new(fd), release))
    }
}

impl Backend for Service {
    fn capabilities(&self) -> Capabilities {
        CAPABILITIES
    }

    fn enabled(&self) -> bool {
        self.state_or_log()
            .is_some_and(|s| !s.joysticks.is_empty() || !s.gamepads.is_empty())
    }

    fn version(&self) -> String {
        self.version.clone()
    }

    fn status(&self, id: u32) -> Status {
        let Some(state) = self.state_or_log() else {
            return Status::Unknown;
        };
        match state.joysticks.iter().find(|(c, _)| c.id == id) {
            Some((_, owner)) => status(Slot::Joystick(id), *owner),
            None => Status::Missing,
        }
    }

    fn devices(&self) -> Vec<DeviceInfo> {
        let Some(state) = self.state_or_log() else {
            return Vec::new();
        };
        state
            .joysticks
            .iter()
            .map(|(c, owner)| DeviceInfo {
                status: status(Slot::Joystick(c.id), *owner),
                ..info(c)
            })
            .collect()
    }

    fn acquire(&self, id: u32) -> Result<Box<dyn crate::Device>, Error> {
        let state = self
            .state()
            .map_err(|e| Error::NotInstalled(e.to_string()))?;
        let config = state
            .joysticks
            .into_iter()
            .map(|(c, _)| c)
            .find(|c| c.id == id)
            .ok_or(Error::Unavailable {
                id,
                status: Status::Missing,
            })?;
        let (emitter, release) = self.lend(Slot::Joystick(id))?;
        Ok(Box::new(Device::lent(config, emitter, release)))
    }

    fn read_config(&self) -> Result<Vec<DeviceConfig>, String> {
        let state = self.state().map_err(|e| e.to_string())?;
        Ok(state.joysticks.into_iter().map(|(c, _)| c).collect())
    }

    /// Asks the service; a device in use cannot change (stop the profile first).
    fn apply(&self, change: &Change) -> Result<Outcome, String> {
        let record = change_record(change).ok_or("SystemGobloon devices cannot be switched off")?;
        match call(&self.client, |c| c.apply_virtual(record)) {
            Ok(()) => Ok(Outcome::Done),
            Err(ServiceError::Service(ErrorCode::Busy)) => {
                Err("the device is in use: stop the profile or the test first".into())
            }
            Err(e) => Err(e.to_string()),
        }
    }

    fn gamepads(&self) -> Vec<GamepadInfo> {
        let Some(state) = self.state_or_log() else {
            return Vec::new();
        };
        state
            .gamepads
            .iter()
            .map(|&(id, owner)| GamepadInfo {
                id,
                status: status(Slot::Gamepad(id), owner),
            })
            .collect()
    }

    fn acquire_gamepad(&self, id: u32) -> Result<Box<dyn crate::Gamepad>, Error> {
        let (emitter, release) = self.lend(Slot::Gamepad(id))?;
        Ok(Box::new(Pad::lent(id, emitter, release)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joystick_records_round_trip() {
        let mut c = DeviceConfig::new(3, &DIRECT);
        c.axes = [true, true, false, false, false, true, false, true];
        c.buttons = 32;
        c.hats = 2;
        c.discrete_hats = true;
        c.name = Some("Gobloon Throttle".into());
        c.product = Some(0xBE20);
        let j = joystick_record(&c, Owner::You);
        assert_eq!((j.name.as_str(), j.product), ("Gobloon Throttle", 0xBE20));
        assert_eq!(j.axes, 0b1010_0011);
        assert_eq!(j.owner, Owner::You as u8);
        assert_eq!(joystick_config(&j), c);
        assert!(change_record(&Change::Enable(true)).is_none());
        let delete = change_record(&Change::DeleteGamepad(2)).unwrap();
        assert_eq!(
            (delete.kind, delete.joystick.id),
            (ChangeKind::DeleteGamepad as u8, 2)
        );
    }
}
