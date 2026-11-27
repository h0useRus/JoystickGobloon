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
