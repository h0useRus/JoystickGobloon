//! Joystick Gremlin profile import/export (R14+ XML format).
//!
//! Profiles round-trip without loss: everything JoyGob does not understand is
//! kept verbatim and written back unchanged.

mod convert;
mod report;
pub mod xmltext;

use std::fmt;
use std::path::Path;

use gob_core::Profile;

pub use convert::{profile_from_element, profile_to_element, MIN_VERSION};
pub use report::{report, ImportReport, Warning};

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Xml(String),
    NotAProfile,
    UnsupportedVersion(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Xml(e) => write!(f, "invalid XML: {e}"),
            Self::NotAProfile => write!(f, "not a Joystick Gremlin profile"),
            Self::UnsupportedVersion(v) => {
                write!(
                    f,
                    "profile format version {v:?} is not supported (need {MIN_VERSION} or newer)"
                )
            }
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

pub fn from_str(text: &str) -> Result<Profile, Error> {
    profile_from_element(&xmltext::parse(text)?)
}

pub fn to_string(profile: &Profile) -> String {
    xmltext::write(&profile_to_element(profile))
}

pub fn load(path: impl AsRef<Path>) -> Result<Profile, Error> {
    from_str(&std::fs::read_to_string(path)?)
}

pub fn save(path: impl AsRef<Path>, profile: &Profile) -> Result<(), Error> {
    Ok(std::fs::write(path, to_string(profile))?)
}
