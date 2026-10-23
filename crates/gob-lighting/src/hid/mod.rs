//! HID access: present HID interfaces of a vendor, their feature reports, Get/SetFeature. One
//! interface for every vendor's protocol (`vkb.rs`), one implementation per platform:
//! `windows.rs` (SetupAPI + `HidD_*`); `none.rs` until
//! Linux gets hidraw.

use std::io;

#[cfg(not(windows))]
mod none;
#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
use none::Hid as Current;
#[cfg(windows)]
use windows::Hid as Current;

/// One HID top-level collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collection {
    /// Interface path (Windows: `\\?\hid#vid_231d&pid_...`), the same one the input layer reports
    /// for its device (gob-input's `DeviceInfo::hid_path`).
    pub path: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub product: String,
    pub usage_page: u16,
    pub usage: u16,
    /// Feature report length with the report id byte, 0 = none.
    pub feature_len: u16,
    /// Report ids of the feature reports.
    pub feature_ids: Vec<u8>,
}

/// A platform's HID access.
pub(crate) trait Transport {
    /// Present HID collections of `vendor`.
    fn collections(vendor: u16) -> Vec<Collection>;
    /// Feature report `id` (`len` bytes with the id byte).
    fn get_feature(path: &str, id: u8, len: usize) -> io::Result<Vec<u8>>;
    /// Sends a feature report; `report[0]` is its id.
    fn set_feature(path: &str, report: &[u8]) -> io::Result<()>;
}

/// Present HID collections of `vendor`.
pub fn collections(vendor: u16) -> Vec<Collection> {
    <Current as Transport>::collections(vendor)
}

/// Feature report `id` (`len` bytes with the id byte).
pub fn get_feature(path: &str, id: u8, len: usize) -> io::Result<Vec<u8>> {
    <Current as Transport>::get_feature(path, id, len)
}

/// Sends a feature report; `report[0]` is its id.
pub fn set_feature(path: &str, report: &[u8]) -> io::Result<()> {
    <Current as Transport>::set_feature(path, report)
}
