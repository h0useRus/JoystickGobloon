//! Platforms without device hiding (neither Windows nor Linux).

use std::sync::Arc;

use crate::{Backend, Error};

pub fn load() -> Result<Arc<dyn Backend>, Error> {
    Err(Error::Unsupported)
}
