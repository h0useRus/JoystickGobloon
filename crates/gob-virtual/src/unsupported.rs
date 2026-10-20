//! Platforms without a backend yet (Linux gets uinput, docs/LINUX_PLAN.md phase 3).

use std::sync::Arc;

use crate::{Backend, Error};

pub fn load() -> Result<Arc<dyn Backend>, Error> {
    Err(Error::Unsupported)
}
