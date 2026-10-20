//! Platforms without a backend yet (Linux gets an exclusive grab, docs/LINUX_PLAN.md phase 4).

use std::sync::Arc;

use crate::{Backend, Error};

pub fn load() -> Result<Arc<dyn Backend>, Error> {
    Err(Error::Unsupported)
}
