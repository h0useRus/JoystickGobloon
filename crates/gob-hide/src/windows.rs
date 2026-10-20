//! Windows backend: HidHide through gob-hidhide (its driver directly).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gob_hidhide::HidHide;

use crate::{Backend, Capabilities, Device, Error, Part};

pub const DOWNLOAD_URL: &str = "https://github.com/nefarius/HidHide/releases";

const CAPABILITIES: Capabilities = Capabilities {
    name: "HidHide",
    app_list: true,
};

pub fn load() -> Result<Arc<dyn Backend>, Error> {
    Ok(Arc::new(Hide(HidHide::find().map_err(error)?)))
}

struct Hide(HidHide);

impl Backend for Hide {
    fn capabilities(&self) -> Capabilities {
        CAPABILITIES
    }

    fn version(&self) -> String {
        self.0.version.clone()
    }

    fn enabled(&self) -> Result<bool, Error> {
        self.0.cloaked().map_err(error)
    }

    fn set_enabled(&self, on: bool) -> Result<(), Error> {
        self.0.set_cloaked(on).map_err(error)
    }

    fn devices(&self, gaming_only: bool) -> Result<Vec<Device>, Error> {
        let hidden = self.0.hidden().map_err(error)?;
        let groups = self.0.device_groups(gaming_only).map_err(error)?;
        Ok(groups
            .iter()
            .map(|g| {
                let key: Vec<String> = g.instance_paths().into_iter().map(String::from).collect();
                Device {
                    name: g.name().to_string(),
                    present: g.present(),
                    gaming: g.gaming(),
                    hidden: g.is_hidden(&hidden),
                    parts: g
                        .devices
                        .iter()
                        .map(|d| Part {
                            present: d.present,
                            vid_pid: d.vid_pid(),
                            product: d.product.clone(),
                        })
                        .collect(),
                    detail: key.join("\n"),
                    key,
                }
            })
            .collect())
    }

    fn set_hidden(&self, device: &Device, hidden: bool) -> Result<(), Error> {
        let paths: Vec<&str> = device.key.iter().map(String::as_str).collect();
        self.0.set_paths_hidden(&paths, hidden).map_err(error)
    }

    fn apps(&self) -> Result<Vec<PathBuf>, Error> {
        self.0.apps().map_err(error)
    }

    fn set_app(&self, exe: &Path, listed: bool) -> Result<(), Error> {
        self.0.set_app(exe, listed).map_err(error)
    }

    fn apps_inverted(&self) -> Result<bool, Error> {
        self.0.inverse().map_err(error)
    }

    fn set_apps_inverted(&self, on: bool) -> Result<(), Error> {
        self.0.set_inverse(on).map_err(error)
    }
}

fn error(e: gob_hidhide::Error) -> Error {
    match e {
        gob_hidhide::Error::NotInstalled => Error::NotInstalled,
        gob_hidhide::Error::Driver(e) => Error::Failed(e),
    }
}
