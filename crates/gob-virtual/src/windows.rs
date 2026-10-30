//! Windows backend: vJoy through gob-vjoy (its driver directly; configuration through the
//! registry, applied by an elevated copy of the program).

use std::sync::Arc;

use gob_vjoy::config as vconfig;
use gob_vjoy::configure as vconfigure;

use crate::{Backend, Capabilities, Change, DeviceConfig, DeviceInfo, Error, Outcome, Status};

pub const CAPABILITIES: Capabilities = Capabilities {
    name: "vJoy",
    max_devices: vconfig::MAX_DEVICES,
    max_buttons: vconfig::MAX_BUTTONS,
    max_hats: vconfig::MAX_HATS,
    effects: &vconfig::EFFECTS,
    switchable: true,
    needs_admin: true,
    // vJoy has no gamepads; XInput ones need a driver of their own.
    max_gamepads: 0,
    // One name and product id for every vJoy device, set by its driver.
    custom_names: false,
    persistent: true,
};

pub fn load() -> Result<Arc<dyn Backend>, Error> {
    let vjoy = gob_vjoy::VJoy::load().map_err(error)?;
    Ok(Arc::new(VJoy(vjoy)))
}

pub fn admin_entry(args: &[String]) -> Option<i32> {
    match args {
        [first, rest @ ..] if first == vconfigure::ARG => Some(vconfigure::run_admin(rest)),
        _ => None,
    }
}

struct VJoy(Arc<gob_vjoy::VJoy>);

impl Backend for VJoy {
    fn capabilities(&self) -> Capabilities {
        CAPABILITIES
    }

    fn enabled(&self) -> bool {
        self.0.enabled()
    }

    fn version(&self) -> String {
        self.0.version()
    }

    fn status(&self, id: u32) -> Status {
        status(self.0.status(id))
    }

    fn devices(&self) -> Vec<DeviceInfo> {
        self.0.devices().into_iter().map(info).collect()
    }

    fn acquire(&self, id: u32) -> Result<Box<dyn crate::Device>, Error> {
        Ok(Box::new(Device(self.0.acquire(id).map_err(error)?)))
    }

    fn read_config(&self) -> Result<Vec<DeviceConfig>, String> {
        let configs = gob_vjoy::descriptor::read_registry()?;
        Ok(configs.into_iter().map(from_vjoy).collect())
    }

    fn apply(&self, change: &Change) -> Result<Outcome, String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let change = match change {
            Change::Set(c) => vconfigure::Change::Set(to_vjoy(c)),
            Change::Delete(id) => vconfigure::Change::Delete(*id),
            Change::Enable(on) => vconfigure::Change::Enable(*on),
            Change::AddGamepad(_) | Change::DeleteGamepad(_) => {
                return Err("vJoy has no gamepads".into());
            }
        };
        match vconfigure::run_elevated(&exe, &change)? {
            vconfigure::Outcome::Done => Ok(Outcome::Done),
            vconfigure::Outcome::RebootNeeded => Ok(Outcome::RebootNeeded),
        }
    }
}

struct Device(gob_vjoy::Device);

impl crate::Device for Device {
    fn set_axis(&self, slot: u8, value: f32) -> bool {
        self.0.set_axis(slot, value)
    }

    fn set_button(&self, n: u8, pressed: bool) -> bool {
        self.0.set_button(n, pressed)
    }

    fn set_hat(&self, n: u8, direction: Option<u16>) -> bool {
        self.0.set_hat(n, direction)
    }

    fn reset(&self) -> bool {
        self.0.reset()
    }
}

fn error(e: gob_vjoy::Error) -> Error {
    match e {
        gob_vjoy::Error::NotInstalled(e) => Error::NotInstalled(e),
        gob_vjoy::Error::Disabled => Error::Disabled,
        gob_vjoy::Error::Unavailable { id, status: s } => Error::Unavailable {
            id,
            status: status(s),
        },
    }
}

fn status(s: gob_vjoy::Status) -> Status {
    match s {
        gob_vjoy::Status::Own => Status::Own,
        gob_vjoy::Status::Free => Status::Free,
        gob_vjoy::Status::Busy => Status::Busy,
        gob_vjoy::Status::Missing => Status::Missing,
        gob_vjoy::Status::Unknown => Status::Unknown,
    }
}

fn info(d: gob_vjoy::DeviceInfo) -> DeviceInfo {
    DeviceInfo {
        id: d.id,
        status: status(d.status),
        axes: d.axes,
        buttons: d.buttons,
        cont_hats: d.cont_hats,
        disc_hats: d.disc_hats,
    }
}

fn from_vjoy(c: vconfig::DeviceConfig) -> DeviceConfig {
    DeviceConfig {
        id: c.id,
        axes: c.axes,
        buttons: c.buttons,
        hats: c.hats,
        discrete_hats: c.discrete_hats,
        effects: c.effects.to_vec(),
        name: None,
        product: None,
    }
}

fn to_vjoy(c: &DeviceConfig) -> vconfig::DeviceConfig {
    let mut effects = [false; vconfig::EFFECTS.len()];
    for (slot, on) in effects.iter_mut().zip(&c.effects) {
        *slot = *on;
    }
    vconfig::DeviceConfig {
        id: c.id,
        axes: c.axes,
        buttons: c.buttons,
        hats: c.hats,
        discrete_hats: c.discrete_hats,
        effects,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_round_trip() {
        let mut c = vconfig::DeviceConfig::new(3);
        c.effects[2] = true;
        c.discrete_hats = true;
        assert_eq!(to_vjoy(&from_vjoy(c.clone())), c);
    }
}
