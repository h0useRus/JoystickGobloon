//! Platforms without HID access yet (Linux gets hidraw, docs/LINUX_PLAN.md phase 5): no devices.

use std::io;

use super::Collection;

pub(crate) struct Hid;

impl super::Transport for Hid {
    fn collections(_vendor: u16) -> Vec<Collection> {
        Vec::new()
    }

    fn get_feature(_path: &str, _id: u8, _len: usize) -> io::Result<Vec<u8>> {
        Err(io::ErrorKind::Unsupported.into())
    }

    fn set_feature(_path: &str, _report: &[u8]) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
}
