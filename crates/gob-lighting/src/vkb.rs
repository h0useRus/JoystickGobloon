//! VKB LED protocol (after pyvkb `vkb/led/__init__.py` and the Gremlin plugin
//! vkb_led_jg_plugin): HID feature report `0x59`, 129 bytes.
//!
//! Write: `59 A5 0A` · CRC16 (LE) · 2 random bytes · N · N 4-byte LED configs · zeros.
//! The list replaces the previous one; an LED dropped from it keeps its last state (EVO R,
//! firmware 2.20). Everything lasts until the device is reconnected (firmware and VKBDevCfg
//! settings stay). LEDs on Gladiator NXT / EVO: see [`KNOWN_LEDS`].

use std::io;

use crate::hid::Collection;
use crate::{Blend, Device, KnownLed, Led, LedKind, Mode, Vendor};

/// VKB vendor id.
pub const VENDOR_ID: u16 = 0x231D;
pub const REPORT_ID: u8 = 0x59;
pub const REPORT_LEN: usize = 129;
const OP_SET: [u8; 3] = [0x59, 0xA5, 0x0A];
/// Most configs one report carries, the dummy included. pyvkb allows 12, but a Gladiator EVO
/// (firmware 2.20) reads back garbage after the fourth; the Gremlin plugin also stops at 4.
pub const MAX_CONFIGS: usize = 5;
/// LEDs kept in the list. A LED dropped from it keeps its color, so only the latest count.
const MAX_LEDS: usize = MAX_CONFIGS - 1;
/// Gladiator NXT / EVO (checked on an EVO R): 10 RGB on top of the grip, 11 small red at
/// the hat, 0 the base: two LEDs on one number, blue (right) = red channel of color1, red
/// (left) = red channel of color2, one blink mode for both.
pub const KNOWN_LEDS: &[KnownLed] = &[
    KnownLed {
        id: 10,
        element: 0,
        kind: LedKind::Rgb,
    },
    KnownLed {
        id: 11,
        element: 0,
        kind: LedKind::Single,
    },
    KnownLed {
        id: 0,
        element: 1,
        kind: LedKind::Single,
    },
    KnownLed {
        id: 0,
        element: 2,
        kind: LedKind::Single,
    },
];
/// Appended to every list: the device spoils `led_mode` of the last config when it is
/// read back (vkb_led_jg_plugin), so the last one is never a real LED.
pub const DUMMY_LED: u8 = 99;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LedMode {
    #[default]
    Off = 0,
    Constant = 1,
    SlowBlink = 2,
    FastBlink = 3,
    UltraBlink = 4,
}

impl LedMode {
    fn from_bits(v: u32) -> Self {
        match v {
            1 => Self::Constant,
            2 => Self::SlowBlink,
            3 => Self::FastBlink,
            4 => Self::UltraBlink,
            _ => Self::Off,
        }
    }
}

/// How `color1` and `color2` combine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorMode {
    #[default]
    Color1 = 0,
    Color2 = 1,
    /// Alternates color1 / color2 when blinking.
    Color1Then2 = 2,
    Color2Then1 = 3,
    /// Both mixed.
    Color1Plus2 = 4,
}

impl ColorMode {
    fn from_bits(v: u32) -> Self {
        match v {
            1 => Self::Color2,
            2 => Self::Color1Then2,
            3 => Self::Color2Then1,
            4 => Self::Color1Plus2,
            _ => Self::Color1,
        }
    }
}

/// VKB color: brightness 0..=7 per channel. Single-color LEDs use only `r`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// From 8-bit RGB: `round(c / 255 * 7)` like pyvkb.
    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        let c = |v: u8| ((v as u32 * 7 + 127) / 255) as u8;
        Self::new(c(r), c(g), c(b))
    }
}

/// One LED entry of the report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LedConfig {
    pub led: u8,
    pub color_mode: ColorMode,
    pub led_mode: LedMode,
    pub color1: Color,
    pub color2: Color,
}

impl LedConfig {
    /// 24 bits `clm lem b2 g2 r2 b1 g1 r1` (3 bits each, clm highest), stored little-endian.
    pub fn to_bytes(&self) -> [u8; 4] {
        let f = |v: u8| (v & 7) as u32;
        let (c1, c2) = (self.color1, self.color2);
        let v = (self.color_mode as u32) << 21
            | (self.led_mode as u32) << 18
            | f(c2.b) << 15
            | f(c2.g) << 12
            | f(c2.r) << 9
            | f(c1.b) << 6
            | f(c1.g) << 3
            | f(c1.r);
        let b = v.to_le_bytes();
        [self.led, b[0], b[1], b[2]]
    }

    pub fn from_bytes(b: [u8; 4]) -> Self {
        let v = u32::from_le_bytes([b[1], b[2], b[3], 0]);
        let f = |shift: u32| ((v >> shift) & 7) as u8;
        Self {
            led: b[0],
            color_mode: ColorMode::from_bits((v >> 21) & 7),
            led_mode: LedMode::from_bits((v >> 18) & 7),
            color1: Color::new(f(0), f(3), f(6)),
            color2: Color::new(f(9), f(12), f(15)),
        }
    }
}

/// CRC16 (init `0xFFFF`, reflected poly `0xA001`) over the first `(n + 1) * 3` bytes of
/// `R R N configs…` - that is what the firmware checks.
fn checksum(n: usize, body: &[u8]) -> u16 {
    let mut crc = 0xFFFFu16;
    for &b in body.iter().take((n + 1) * 3) {
        crc ^= b as u16;
        for _ in 0..8 {
            let lsb = crc & 1;
            crc >>= 1;
            if lsb != 0 {
                crc ^= 0xA001;
            }
        }
    }
    crc
}

/// The feature report that sets `configs` (the dummy LED appended), `random` = the two
/// "random" bytes. An empty list is sent without the dummy (N = 0, like pyvkb's
/// `set_leds([])`). `None` when there are too many configs.
pub fn encode(configs: &[LedConfig], random: [u8; 2]) -> Option<[u8; REPORT_LEN]> {
    let dummy = LedConfig {
        led: DUMMY_LED,
        ..Default::default()
    };
    let dummy = (!configs.is_empty()).then_some(&dummy);
    let n = configs.len() + usize::from(dummy.is_some());
    if n > MAX_CONFIGS {
        return None;
    }
    let mut body = vec![random[0], random[1], n as u8];
    for c in configs.iter().chain(dummy) {
        body.extend_from_slice(&c.to_bytes());
    }
    let crc = checksum(n, &body).to_le_bytes();
    let mut report = [0u8; REPORT_LEN];
    report[..3].copy_from_slice(&OP_SET);
    report[3..5].copy_from_slice(&crc);
    report[5..5 + body.len()].copy_from_slice(&body);
    Some(report)
}

/// Configs in a report read back with `GetFeature 0x59`; empty when nothing was set since
/// the device was connected. The dummy LED is dropped.
pub fn decode(report: &[u8]) -> Vec<LedConfig> {
    if report.len() < 8 || report[..3] != OP_SET {
        return Vec::new();
    }
    let n = (report[7] as usize).min(MAX_CONFIGS);
    report[8..]
        .as_chunks::<4>()
        .0
        .iter()
        .take(n)
        .map(|&c| LedConfig::from_bytes(c))
        .filter(|c| c.led != DUMMY_LED)
        .collect()
}

/// `current` with `config` replacing the entry of the same LED (or added).
pub fn merge(current: &[LedConfig], config: LedConfig) -> Vec<LedConfig> {
    let mut list: Vec<LedConfig> = current
        .iter()
        .filter(|c| c.led != config.led)
        .copied()
        .collect();
    list.push(config);
    list
}

/// Brightness (0..=7) of the two elements a config lights.
fn elements(c: &LedConfig) -> (u8, u8) {
    if c.led_mode == LedMode::Off {
        return (0, 0);
    }
    let (first, second) = match c.color_mode {
        ColorMode::Color1 => (true, false),
        ColorMode::Color2 => (false, true),
        _ => (true, true),
    };
    (
        if first { c.color1.r } else { 0 },
        if second { c.color2.r } else { 0 },
    )
}

/// `current` with `led` applied: a whole LED replaces its entry, an element changes only its
/// half of the entry (the other element as it was; not in the list = off).
pub fn apply(current: &[LedConfig], led: &Led) -> Vec<LedConfig> {
    if led.element == 0 {
        return merge(current, (*led).into());
    }
    let old = current
        .iter()
        .find(|c| c.led == led.id)
        .copied()
        .unwrap_or(LedConfig {
            led: led.id,
            ..Default::default()
        });
    let (mut first, mut second) = elements(&old);
    let [r, ..] = led.color;
    let level = if led.mode == Mode::Off {
        0
    } else {
        Color::from_rgb(r, 0, 0).r
    };
    if led.element == 1 {
        first = level;
    } else {
        second = level;
    }
    // One blink mode for both: the one just set, else the other element's.
    let led_mode = match (first, second) {
        (0, 0) => LedMode::Off,
        _ if led.mode != Mode::Off => LedConfig::from(*led).led_mode,
        _ => old.led_mode,
    };
    let config = LedConfig {
        led: led.id,
        color_mode: match (first > 0, second > 0) {
            (true, true) => ColorMode::Color1Plus2,
            (false, true) => ColorMode::Color2,
            _ => ColorMode::Color1,
        },
        led_mode,
        color1: Color::new(first, 0, 0),
        color2: Color::new(second, 0, 0),
    };
    merge(current, config)
}

/// `current` without `led`: it follows the device profile again.
pub fn remove(current: &[LedConfig], led: u8) -> Vec<LedConfig> {
    current.iter().filter(|c| c.led != led).copied().collect()
}

/// Two bytes that change on every call (the firmware only wants them to differ).
pub fn random_bytes() -> [u8; 2] {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default(),
    );
    let v = h.finish();
    [v as u8, (v >> 8) as u8]
}

/// VKB collections that have the LED report. It is the joystick collection DirectInput reads.
pub fn devices() -> Vec<Device> {
    crate::hid::collections(VENDOR_ID)
        .into_iter()
        .filter(|c| c.feature_ids.contains(&REPORT_ID))
        .map(|hid| Device {
            vendor: Vendor::Vkb,
            hid,
        })
        .collect()
}

fn report_len(hid: &Collection) -> io::Result<usize> {
    let len = hid.feature_len as usize;
    if len < REPORT_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("feature report is {len} bytes, LEDs need {REPORT_LEN}"),
        ));
    }
    Ok(len)
}

/// LEDs set by a write (ours or VKBDevCfg's LED test) since the device was connected.
pub fn read(hid: &Collection) -> io::Result<Vec<LedConfig>> {
    let report = crate::hid::get_feature(&hid.path, REPORT_ID, report_len(hid)?)?;
    Ok(decode(&report))
}

/// Replaces the whole list of set LEDs.
pub fn write(hid: &Collection, configs: &[LedConfig]) -> io::Result<()> {
    let len = report_len(hid)?;
    let report = encode(configs, random_bytes()).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("at most {} LEDs at once", MAX_CONFIGS - 1),
        )
    })?;
    let mut buffer = vec![0u8; len];
    buffer[..report.len()].copy_from_slice(&report);
    crate::hid::set_feature(&hid.path, &buffer)
}

impl From<Led> for LedConfig {
    fn from(led: Led) -> Self {
        let ([r, g, b], [r2, g2, b2]) = (led.color, led.color2);
        Self {
            led: led.id,
            color_mode: match led.blend {
                Blend::First => ColorMode::Color1,
                Blend::Second => ColorMode::Color2,
                Blend::Alternate => ColorMode::Color1Then2,
                Blend::AlternateReverse => ColorMode::Color2Then1,
                Blend::Mix => ColorMode::Color1Plus2,
            },
            led_mode: match led.mode {
                Mode::Off => LedMode::Off,
                Mode::On => LedMode::Constant,
                Mode::SlowBlink => LedMode::SlowBlink,
                Mode::FastBlink => LedMode::FastBlink,
                Mode::FastestBlink => LedMode::UltraBlink,
            },
            color1: Color::from_rgb(r, g, b),
            color2: Color::from_rgb(r2, g2, b2),
        }
    }
}

/// Sets these LEDs with one write, keeping the others in the list.
pub fn set(hid: &Collection, leds: &[Led]) -> io::Result<()> {
    let mut list = leds.iter().fold(read(hid)?, |list, led| apply(&list, led));
    list.drain(..list.len().saturating_sub(MAX_LEDS));
    write(hid, &list)
}

/// Drops one LED from the list.
pub fn release(hid: &Collection, id: u8) -> io::Result<()> {
    let current = read(hid)?;
    if current.iter().any(|c| c.led == id) {
        write(hid, &remove(&current, id))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
