//! Xbox controllers through XInput. Windows gives their DirectInput side only to the program in
//! the foreground and merges both triggers into one axis, so a controller whose device path has
//! `IG_` is read through XInput instead, under its DirectInput id and with this layout:
//!
//! - axes: left stick X / Y, LT on Z, right stick Rx / Ry, RT on Rz (Y up is negative, like
//!   DirectInput; a released trigger is -1);
//! - buttons 1-10 as DirectInput numbers them (A, B, X, Y, LB, RB, Back, Start, left stick,
//!   right stick), 11 Guide;
//! - the d-pad as hat 1.
//!
//! The XInput slot of a controller is found by its vendor and product id
//! (`XInputGetCapabilitiesEx`, as SDL does); until one is found it is read through DirectInput
//! into the same layout. `xinput1_4.dll` is loaded at run time: without it every controller is
//! read through DirectInput as before.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::core::{PCSTR, w};

use crate::{Calibration, DeviceInfo, DeviceState};

/// XInput has four slots.
const SLOTS: u32 = 4;
const ERROR_SUCCESS: u32 = 0;
/// How often an unpaired controller looks for its slot (an empty slot is slow to query).
const PAIR_RETRY: Duration = Duration::from_secs(1);

/// Our layout: axis slots X, Y, Z, Rx, Ry, Rz.
pub const AXES: [u8; 6] = [1, 2, 3, 4, 5, 6];
pub const BUTTONS: usize = 11;

/// `wButtons` bits in our button order (1-11).
const BUTTON_BITS: [u16; BUTTONS] = [
    0x1000, // A
    0x2000, // B
    0x4000, // X
    0x8000, // Y
    0x0100, // LB
    0x0200, // RB
    0x0020, // Back
    0x0010, // Start
    0x0040, // left stick
    0x0080, // right stick
    0x0400, // Guide (XInputGetStateEx only)
];
const DPAD_UP: u16 = 0x1;
const DPAD_DOWN: u16 = 0x2;
const DPAD_LEFT: u16 = 0x4;
const DPAD_RIGHT: u16 = 0x8;

/// `XINPUT_GAMEPAD`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Gamepad {
    pub buttons: u16,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub lx: i16,
    pub ly: i16,
    pub rx: i16,
    pub ry: i16,
}

/// `XINPUT_STATE`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct State {
    packet: u32,
    gamepad: Gamepad,
}

/// `XINPUT_CAPABILITIES` followed by the ids, as `XInputGetCapabilitiesEx` fills it.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct CapabilitiesEx {
    kind: u8,
    sub_type: u8,
    flags: u16,
    gamepad: Gamepad,
    vibration: [u16; 2],
    vendor: u16,
    product: u16,
    version: u16,
    unknown1: u16,
    unknown2: u32,
}

/// What `GetProcAddress` returns.
type Proc = unsafe extern "system" fn() -> isize;
type GetState = unsafe extern "system" fn(u32, *mut State) -> u32;
type GetCapabilitiesEx = unsafe extern "system" fn(u32, u32, u32, *mut CapabilitiesEx) -> u32;

/// What the pads need from XInput; the tests use a simulated one.
pub trait Runtime: Send + Sync {
    /// The slot's packet number and state; `None` when nothing is connected there.
    fn state(&self, slot: u32) -> Option<(u32, Gamepad)>;
    /// Vendor and product id of the controller in `slot`, where the runtime tells them.
    fn ids(&self, slot: u32) -> Option<(u16, u16)>;
}

/// `xinput1_4.dll`, loaded once and kept for the process.
pub struct XInput {
    get_state: GetState,
    capabilities: Option<GetCapabilitiesEx>,
}

impl XInput {
    pub fn load() -> Option<Self> {
        // SAFETY: loading a system DLL by name; it stays loaded for the process.
        let module: HMODULE = match unsafe { LoadLibraryW(w!("xinput1_4.dll")) } {
            Ok(m) => m,
            Err(e) => {
                log::info!(
                    "xinput1_4.dll not loaded ({e}): Xbox controllers are read via DirectInput"
                );
                return None;
            }
        };
        let by_ordinal = |n: usize| {
            // SAFETY: an ordinal passed as the name, like MAKEINTRESOURCEA.
            unsafe { GetProcAddress(module, PCSTR(n as *const u8)) }
        };
        // XInputGetStateEx (ordinal 100) also reports the Guide button; same signature.
        let get_state = by_ordinal(100)
            // SAFETY: the name of an export of xinput1_4.dll.
            .or_else(|| unsafe {
                GetProcAddress(module, PCSTR(c"XInputGetState".as_ptr().cast()))
            })?;
        // SAFETY: both exports have the GetState signature; ordinal 108 is
        // XInputGetCapabilitiesEx(1, slot, flags, *caps) since Windows 10.
        Some(Self {
            get_state: unsafe { std::mem::transmute::<Proc, GetState>(get_state) },
            capabilities: by_ordinal(108)
                .map(|f| unsafe { std::mem::transmute::<Proc, GetCapabilitiesEx>(f) }),
        })
    }
}

impl Runtime for XInput {
    fn state(&self, slot: u32) -> Option<(u32, Gamepad)> {
        let mut state = State::default();
        // SAFETY: `state` is an XINPUT_STATE for the call to fill.
        (unsafe { (self.get_state)(slot, &mut state) } == ERROR_SUCCESS)
            .then_some((state.packet, state.gamepad))
    }

    fn ids(&self, slot: u32) -> Option<(u16, u16)> {
        let get = self.capabilities?;
        let mut caps = CapabilitiesEx::default();
        // SAFETY: `caps` is the struct XInputGetCapabilitiesEx fills.
        (unsafe { get(1, slot, 0, &mut caps) } == ERROR_SUCCESS)
            .then_some((caps.vendor, caps.product))
    }
}

/// Whether DirectInput's device path names an XInput controller.
pub fn is_xinput_path(hid_path: &str) -> bool {
    hid_path.to_ascii_lowercase().contains("ig_")
}

/// `info` with our XInput layout.
pub fn layout(info: &DeviceInfo) -> DeviceInfo {
    DeviceInfo {
        axes: AXES.to_vec(),
        buttons: BUTTONS,
        hats: 1,
        ..info.clone()
    }
}

/// Slot for a controller with these ids: the first free connected one with the same ids, else
/// (ids unknown, or none matches) the first free connected one.
fn choose_slot(free: &[(u32, Option<(u16, u16)>)], ids: (u16, u16)) -> Option<u32> {
    free.iter()
        .find(|(_, i)| *i == Some(ids))
        .or_else(|| free.first())
        .map(|(slot, _)| *slot)
}

/// Raw axis values (DirectInput's -32768..=32767, slots 1..6), buttons and hat of a pad.
pub fn readings(pad: &Gamepad) -> ([i32; 6], [bool; BUTTONS], Option<u16>) {
    let trigger = |t: u8| i32::from(t) * 257 - 32768;
    // XInput's Y points up; DirectInput's (and ours) down, like `xpad`'s bitwise not.
    let flip = |v: i16| i32::from(!v);
    let axes = [
        i32::from(pad.lx),
        flip(pad.ly),
        trigger(pad.left_trigger),
        i32::from(pad.rx),
        flip(pad.ry),
        trigger(pad.right_trigger),
    ];
    let buttons = BUTTON_BITS.map(|bit| pad.buttons & bit != 0);
    (axes, buttons, dpad_direction(pad.buttons))
}

/// D-pad bits -> a hat direction (hundredths of a degree), opposite directions cancelling.
fn dpad_direction(buttons: u16) -> Option<u16> {
    let on = |b: u16| i32::from(buttons & b != 0);
    let (x, y) = (on(DPAD_RIGHT) - on(DPAD_LEFT), on(DPAD_UP) - on(DPAD_DOWN));
    Some(match (x, y) {
        (0, 1) => 0,
        (1, 1) => 4500,
        (1, 0) => 9000,
        (1, -1) => 13500,
        (0, -1) => 18000,
        (-1, -1) => 22500,
        (-1, 0) => 27000,
        (-1, 1) => 31500,
        _ => return None,
    })
}

/// A controller read through DirectInput (no XInput slot yet) in our layout: DirectInput's Xbox
/// driver has the sticks on X, Y, Rx, Ry, both triggers on Z (LT up, RT down), buttons 1-10 and
/// the d-pad as hat 1.
pub fn from_dinput(di: &DeviceState) -> ([i32; 6], [bool; BUTTONS], Option<u16>) {
    let z = di.raw[2];
    let trigger = |t: i32| (t.clamp(0, 32767) * 2 - 32767).clamp(-32768, 32767);
    let axes = [
        di.raw[0],
        di.raw[1],
        trigger(z),
        di.raw[3],
        di.raw[4],
        trigger(-z),
    ];
    let mut buttons = [false; BUTTONS];
    for (b, &pressed) in buttons.iter_mut().zip(&di.buttons) {
        *b = pressed;
    }
    (axes, buttons, di.hats.first().copied().flatten())
}

/// The XInput side of a controller: its slot (once found) and its state in our layout.
pub struct Pad {
    xinput: Arc<dyn Runtime>,
    /// Slots read by open controllers, shared by all of them.
    taken: Arc<Mutex<HashSet<u32>>>,
    ids: (u16, u16),
    slot: Option<u32>,
    packet: Option<u32>,
    next_pairing: Instant,
    pub state: DeviceState,
    pub previous: DeviceState,
    calibration: [Option<Calibration>; 6],
}

impl Pad {
    pub fn new(
        xinput: Arc<dyn Runtime>,
        taken: Arc<Mutex<HashSet<u32>>>,
        info: &DeviceInfo,
    ) -> Self {
        let state = DeviceState::new(layout(info));
        let mut pad = Self {
            xinput,
            taken,
            ids: (info.vendor_id, info.product_id),
            slot: None,
            packet: None,
            next_pairing: Instant::now(),
            previous: state.clone(),
            state,
            calibration: [None; 6],
        };
        pad.pair();
        pad
    }

    pub fn slot(&self) -> Option<u32> {
        self.slot
    }

    pub fn set_calibrations(&mut self, calibrations: &crate::Calibrations) {
        for (i, c) in self.calibration.iter_mut().enumerate() {
            *c = calibrations
                .get(&(self.state.info.guid.clone(), i as u8 + 1))
                .copied();
        }
    }

    /// Looks for this controller's slot among the free connected ones.
    fn pair(&mut self) {
        self.next_pairing = Instant::now() + PAIR_RETRY;
        let mut taken = self.taken.lock().unwrap_or_else(|e| e.into_inner());
        let free: Vec<(u32, Option<(u16, u16)>)> = (0..SLOTS)
            .filter(|s| !taken.contains(s))
            .filter(|&s| self.xinput.state(s).is_some())
            .map(|s| (s, self.xinput.ids(s)))
            .collect();
        if let Some(slot) = choose_slot(&free, self.ids) {
            taken.insert(slot);
            self.slot = Some(slot);
            self.packet = None;
            log::info!("{}: read via XInput slot {slot}", self.state.info.name);
        }
    }

    fn unpair(&mut self) {
        if let Some(slot) = self.slot.take() {
            self.taken
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&slot);
            log::info!("{}: XInput slot {slot} gone", self.state.info.name);
        }
    }

    /// Reads the pad (`dinput`: the controller's DirectInput state, read only while no slot is
    /// found). Returns false when it is gone.
    pub fn read(&mut self, dinput: impl FnOnce() -> Option<DeviceState>) -> bool {
        self.previous.copy_readings(&self.state);
        if self.slot.is_none() && Instant::now() >= self.next_pairing {
            self.pair();
        }
        let readings = match self.slot {
            Some(slot) => match self.xinput.state(slot) {
                Some((packet, _)) if Some(packet) == self.packet => return true,
                Some((packet, pad)) => {
                    self.packet = Some(packet);
                    readings(&pad)
                }
                None => {
                    self.unpair();
                    return true;
                }
            },
            None => match dinput() {
                Some(di) => from_dinput(&di),
                None => return false,
            },
        };
        self.apply(readings);
        true
    }

    fn apply(&mut self, (axes, buttons, hat): ([i32; 6], [bool; BUTTONS], Option<u16>)) {
        let s = &mut self.state;
        for (i, &raw) in axes.iter().enumerate() {
            s.raw[i] = raw;
            s.axes[i] = match &self.calibration[i] {
                Some(c) => c.apply(raw),
                None => super::normalize_axis(raw),
            };
        }
        s.buttons.copy_from_slice(&buttons);
        s.hats[0] = hat;
    }
}

impl Drop for Pad {
    fn drop(&mut self) {
        self.unpair();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_state_in_our_layout() {
        let pad = Gamepad {
            buttons: 0x1000 | 0x0400 | 0x0010 | DPAD_UP | DPAD_RIGHT,
            left_trigger: 255,
            right_trigger: 0,
            lx: -32768,
            ly: 32767,
            rx: 0,
            ry: -32768,
        };
        let (axes, buttons, hat) = readings(&pad);
        assert_eq!(
            axes,
            [-32768, -32768, 32767, 0, 32767, -32768],
            "Y flipped, triggers -1..1"
        );
        assert!(buttons[0], "A is button 1");
        assert!(buttons[7], "Start is button 8");
        assert!(buttons[10], "Guide is button 11");
        assert_eq!(buttons.iter().filter(|&&b| b).count(), 3);
        assert_eq!(hat, Some(4500));
        assert_eq!(
            dpad_direction(DPAD_UP | DPAD_DOWN),
            None,
            "opposites cancel"
        );
        assert_eq!(dpad_direction(DPAD_LEFT | DPAD_DOWN), Some(22500));
    }

    #[test]
    fn dinput_state_in_our_layout() {
        let mut di = DeviceState::new(DeviceInfo {
            axes: vec![1, 2, 3, 4, 5],
            buttons: 10,
            hats: 1,
            ..Default::default()
        });
        di.raw = [100, -200, 32767, 5, 6, 0, 0, 0];
        di.buttons[9] = true;
        di.hats[0] = Some(18000);
        let (axes, buttons, hat) = from_dinput(&di);
        assert_eq!(
            axes,
            [100, -200, 32767, 5, 6, -32767],
            "LT pressed, RT released"
        );
        assert!(buttons[9] && !buttons[10]);
        assert_eq!(hat, Some(18000));
    }

    /// A simulated XInput: per slot the packet number, the pad and its ids.
    #[derive(Default)]
    struct Sim(Mutex<[Option<Slot>; 4]>);

    /// Packet number, pad, ids.
    type Slot = (u32, Gamepad, (u16, u16));

    impl Sim {
        fn plug(&self, slot: usize, ids: (u16, u16)) {
            self.0.lock().unwrap()[slot] = Some((1, Gamepad::default(), ids));
        }
        fn unplug(&self, slot: usize) {
            self.0.lock().unwrap()[slot] = None;
        }
        /// A new report: the packet number goes up.
        fn press(&self, slot: usize, buttons: u16, left_trigger: u8) {
            let mut slots = self.0.lock().unwrap();
            let s = slots[slot].as_mut().unwrap();
            s.0 += 1;
            s.1.buttons = buttons;
            s.1.left_trigger = left_trigger;
        }
    }

    impl Runtime for Sim {
        fn state(&self, slot: u32) -> Option<(u32, Gamepad)> {
            self.0.lock().unwrap()[slot as usize].map(|(p, g, _)| (p, g))
        }
        fn ids(&self, slot: u32) -> Option<(u16, u16)> {
            self.0.lock().unwrap()[slot as usize].map(|(_, _, ids)| ids)
        }
    }

    const XBOX_ONE: (u16, u16) = (0x045e, 0x02ff);
    const XBOX_360: (u16, u16) = (0x045e, 0x028e);

    fn info(guid: &str, (vendor_id, product_id): (u16, u16)) -> DeviceInfo {
        DeviceInfo {
            guid: guid.into(),
            name: guid.into(),
            vendor_id,
            product_id,
            axes: vec![1, 2, 3, 4, 5],
            buttons: 10,
            hats: 1,
            ..Default::default()
        }
    }

    /// DirectInput's state of an Xbox controller with button 2 (B) held.
    fn dinput_b() -> Option<DeviceState> {
        let mut di = DeviceState::new(info("di", XBOX_ONE));
        di.buttons[1] = true;
        Some(di)
    }

    fn pressed(pad: &Pad) -> Vec<usize> {
        (0..BUTTONS)
            .filter(|&i| pad.state.buttons[i])
            .map(|i| i + 1)
            .collect()
    }

    #[test]
    fn simulated_pads_find_their_slots() {
        let sim = Arc::new(Sim::default());
        let taken = Arc::<Mutex<HashSet<u32>>>::default();
        sim.plug(0, XBOX_360);
        sim.plug(2, XBOX_ONE);
        let one = Pad::new(sim.clone(), taken.clone(), &info("one", XBOX_ONE));
        let other = Pad::new(sim.clone(), taken.clone(), &info("360", XBOX_360));
        assert_eq!(one.slot(), Some(2), "by its ids, not the first slot");
        assert_eq!(other.slot(), Some(0));
        assert_eq!(one.state.info.axes, AXES, "our layout");
        assert_eq!((one.state.info.buttons, one.state.info.hats), (BUTTONS, 1));
        let third = Pad::new(sim.clone(), taken.clone(), &info("3rd", XBOX_ONE));
        assert_eq!(third.slot(), None, "both slots are taken");
        drop(other);
        assert_eq!(
            *taken.lock().unwrap(),
            HashSet::from([2]),
            "freed when it closes"
        );
    }

    #[test]
    fn simulated_pad_reads_reports() {
        let sim = Arc::new(Sim::default());
        let taken = Arc::<Mutex<HashSet<u32>>>::default();
        sim.plug(1, XBOX_ONE);
        let mut pad = Pad::new(sim.clone(), taken.clone(), &info("pad", XBOX_ONE));
        let no_dinput = || panic!("a paired pad does not read DirectInput");

        sim.press(1, 0x1000 | 0x0400, 255);
        assert!(pad.read(no_dinput));
        assert_eq!(pressed(&pad), [1, 11], "A and Guide");
        assert_eq!(pad.state.axes[2], 1.0, "LT pulled");
        assert_eq!(pad.state.axes[5], -1.0, "RT released");

        // The same packet: nothing changed, and the state is kept as it was.
        assert!(pad.read(no_dinput));
        assert_eq!(pad.previous.buttons, pad.state.buttons);

        sim.press(1, 0, 0);
        assert!(pad.read(no_dinput));
        assert!(pressed(&pad).is_empty());
        assert!(
            pad.previous.buttons[0],
            "previous has the press: the release is seen"
        );
    }

    #[test]
    fn simulated_unplug_falls_back_and_pairs_again() {
        let sim = Arc::new(Sim::default());
        let taken = Arc::<Mutex<HashSet<u32>>>::default();
        let mut pad = Pad::new(sim.clone(), taken.clone(), &info("pad", XBOX_ONE));
        assert_eq!(pad.slot(), None, "no XInput slot yet");
        assert!(pad.read(dinput_b));
        assert_eq!(pressed(&pad), [2], "read through DirectInput meanwhile");
        assert!(!pad.read(|| None), "gone from DirectInput: gone");

        sim.plug(3, XBOX_ONE);
        pad.next_pairing = Instant::now();
        sim.press(3, 0x4000, 0);
        assert!(pad.read(|| panic!("paired before reading")));
        assert_eq!(pad.slot(), Some(3));
        assert_eq!(pressed(&pad), [3], "X");

        sim.unplug(3);
        assert!(pad.read(|| panic!("the unplug itself reads nothing")));
        assert_eq!(pad.slot(), None);
        assert!(taken.lock().unwrap().is_empty(), "slot freed");
        assert!(
            pad.read(dinput_b),
            "next pairing later: DirectInput meanwhile"
        );
        assert_eq!(pressed(&pad), [2]);
    }

    /// The real runtime: loads, finds its exports and answers for every slot (read only).
    #[test]
    fn real_runtime_answers() {
        let x = XInput::load().expect("xinput1_4.dll is part of Windows 10 and 11");
        for slot in 0..SLOTS {
            if x.state(slot).is_none() {
                assert_eq!(x.ids(slot), None, "slot {slot}: nothing connected, no ids");
            }
        }
        assert_eq!(x.state(SLOTS), None, "there is no fifth slot");
    }

    #[test]
    fn slots_by_ids() {
        let xbox = (0x045e, 0x02ff);
        let free = [(0, Some((0x046d, 0xc21d))), (2, Some(xbox))];
        assert_eq!(choose_slot(&free, xbox), Some(2));
        assert_eq!(
            choose_slot(&free, (1, 1)),
            Some(0),
            "no match: the first free one"
        );
        assert_eq!(choose_slot(&[(3, None)], xbox), Some(3), "ids unknown");
        assert_eq!(choose_slot(&[], xbox), None);
        assert!(is_xinput_path(
            r"\\?\hid#vid_045e&pid_02ff&IG_00#7&1&0#{4d1e55b2}"
        ));
        assert!(!is_xinput_path(
            r"\\?\hid#vid_231d&pid_0200#7&1&0#{4d1e55b2}"
        ));
    }
}
