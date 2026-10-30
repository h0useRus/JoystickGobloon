//! Hiding through SystemGobloon (crate `system-gobloon`), when it runs: the service keeps the
//! state and closes hidden devices' nodes (event, js, hidraw) to every program, so nothing is
//! `exposed`. Their nodes reach this program from the service (gob-input's opener); it still
//! grabs them, which also stops programs that opened them before they were hidden.
//!
//! The first time, the devices hidden in `hidden.gob` move to the service; the file is then
//! renamed to `hidden.gob.migrated`.

use std::sync::{Mutex, PoisonError};

use gob_system::protocol::{Hidden, Text, caps};
use gob_system::{Client, Error as ServiceError};

use super::{State, apply, list, read_state, state_path};
use crate::{Backend, Capabilities, Device, Error};

pub const CAPABILITIES: Capabilities = Capabilities {
    name: "SystemGobloon",
    app_list: false,
    hides_everywhere: true,
};

/// Reported to the service's log.
const CLIENT_VERSION: &str = match option_env!("JOYSTICK_GOBLOON_VERSION") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};

pub struct Service {
    client: Mutex<Option<Client>>,
    version: String,
}

fn failed(e: ServiceError) -> Error {
    Error::Failed(e.to_string())
}

fn local_state(enabled: bool, hidden: &[Hidden]) -> State {
    let devices = hidden
        .iter()
        .map(|h| super::Hidden {
            id: h.id.as_str().to_string(),
            name: h.name.as_str().to_string(),
            vendor: h.vendor,
            product: h.product,
        })
        .collect();
    State { enabled, devices }
}

/// The service's hidden devices written to `hidden.gob` (it is about to be removed); without a
/// running service, nothing.
pub fn keep_locally() -> Result<(), String> {
    let Ok(mut client) = Client::connect() else {
        return Ok(());
    };
    let (enabled, hidden) = client.hide_state().map_err(|e| e.to_string())?;
    super::write_state(&local_state(enabled, &hidden)).map_err(|e| e.to_string())?;
    if let Some(path) = state_path() {
        let _ = std::fs::remove_file(path.with_extension("gob.migrated"));
    }
    Ok(())
}

impl Service {
    /// The service, when it runs and hides; also sets gob-input's opener and the grabs.
    pub fn connect() -> Option<Service> {
        let mut client = Client::connect().ok()?;
        let hello = match client.hello(CLIENT_VERSION) {
            Ok(h) => h,
            Err(e) => {
                log::warn!("{e}");
                return None;
            }
        };
        if hello.capabilities & caps::HIDE_EVERYWHERE == 0 {
            return None;
        }
        let service = Service {
            client: Mutex::new(Some(client)),
            version: hello.service_version.as_str().to_string(),
        };
        log::info!("device hiding through SystemGobloon {}", service.version);
        service.migrate();
        gob_input::set_opener(Some(opener()));
        match service.state() {
            Ok(state) => apply(&state),
            Err(e) => log::warn!("{e}"),
        }
        Some(service)
    }

    /// Runs `f` on the connection, connecting again once when it broke (service restarted).
    fn call<T>(
        &self,
        mut f: impl FnMut(&mut Client) -> Result<T, ServiceError>,
    ) -> Result<T, Error> {
        let mut client = self.client.lock().unwrap_or_else(PoisonError::into_inner);
        for retry in [true, false] {
            if client.is_none() {
                *client = Some(Client::connect().map_err(failed)?);
            }
            let Some(c) = client.as_mut() else { continue };
            match f(c) {
                Err(ServiceError::Io(e)) if retry => {
                    log::info!("SystemGobloon connection: {e}; connecting again");
                    *client = None;
                }
                other => return other.map_err(failed),
            }
        }
        Err(Error::Failed("SystemGobloon: no connection".into()))
    }

    /// The service's state in the form of `hidden.gob`.
    fn state(&self) -> Result<State, Error> {
        let (enabled, hidden) = self.call(Client::hide_state)?;
        Ok(local_state(enabled, &hidden))
    }

    /// Moves the devices hidden in `hidden.gob` to the service (when it hides nothing yet).
    fn migrate(&self) {
        let Some(path) = state_path().filter(|p| p.exists()) else {
            return;
        };
        let (Ok(local), Ok(remote)) = (read_state(), self.state()) else {
            return;
        };
        if remote.devices.is_empty() {
            for d in &local.devices {
                let record = record(&d.id, &d.name, d.vendor, d.product);
                if let Err(e) = self.call(|c| c.set_hidden(record, true)) {
                    log::warn!("moving {} to SystemGobloon: {e}", d.name);
                    return;
                }
            }
            if !local.enabled
                && let Err(e) = self.call(|c| c.set_hide_enabled(false))
            {
                log::warn!("{e}");
            }
            log::info!(
                "{} hidden device(s) moved to SystemGobloon",
                local.devices.len()
            );
        }
        let done = path.with_extension("gob.migrated");
        if let Err(e) = std::fs::rename(&path, &done) {
            log::warn!("{}: {e}", path.display());
        }
    }
}

fn record(id: &str, name: &str, vendor: u16, product: u16) -> Hidden {
    Hidden {
        id: Text::new(id),
        name: Text::new(name),
        vendor,
        product,
        present: 0,
        reserved: [0; 3],
    }
}

/// gob-input's opener: nodes of hidden controllers and raw HID nodes from the service, on a
/// connection of its own (the input thread asks while the GUI may be using the other one).
fn opener() -> gob_input::Opener {
    let client: Mutex<Option<Client>> = Mutex::new(None);
    Box::new(move |path| {
        let to_io = |e: ServiceError| match e {
            ServiceError::Io(e) => e,
            other => std::io::Error::other(other.to_string()),
        };
        let mut client = client.lock().unwrap_or_else(PoisonError::into_inner);
        if client.is_none() {
            *client = Some(Client::connect().map_err(to_io)?);
        }
        let Some(c) = client.as_mut() else {
            return Err(std::io::ErrorKind::NotConnected.into());
        };
        c.open_node(path).map_err(|e| {
            if matches!(e, ServiceError::Io(_)) {
                *client = None;
            }
            to_io(e)
        })
    })
}

impl Backend for Service {
    fn capabilities(&self) -> Capabilities {
        CAPABILITIES
    }

    fn version(&self) -> String {
        self.version.clone()
    }

    fn enabled(&self) -> Result<bool, Error> {
        Ok(self.state()?.enabled)
    }

    fn set_enabled(&self, on: bool) -> Result<(), Error> {
        self.call(|c| c.set_hide_enabled(on))?;
        apply(&self.state()?);
        Ok(())
    }

    /// Every device here is a game controller, so `gaming_only` changes nothing. Nothing is
    /// `exposed`: the service closes every node.
    fn devices(&self, _gaming_only: bool) -> Result<Vec<Device>, Error> {
        let mut devices = list(&gob_input::exclusive::devices(), &self.state()?);
        devices.iter_mut().for_each(|d| d.exposed = false);
        Ok(devices)
    }

    fn set_hidden(&self, device: &Device, hidden: bool) -> Result<(), Error> {
        let id = device
            .key
            .first()
            .ok_or_else(|| Error::Failed("device without an id".into()))?;
        let (vendor, product) = device
            .parts
            .first()
            .and_then(|p| p.vid_pid)
            .unwrap_or_default();
        let record = record(id, &device.name, vendor, product);
        self.call(|c| c.set_hidden(record, hidden))?;
        apply(&self.state()?);
        Ok(())
    }

    fn apps(&self) -> Result<Vec<std::path::PathBuf>, Error> {
        Ok(Vec::new())
    }

    /// No list, as without the service.
    fn set_app(&self, _exe: &std::path::Path, listed: bool) -> Result<(), Error> {
        if listed {
            Err(Error::Unsupported)
        } else {
            Ok(())
        }
    }

    fn apps_inverted(&self) -> Result<bool, Error> {
        Ok(false)
    }

    fn set_apps_inverted(&self, _on: bool) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
}
