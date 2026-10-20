//! Linux input without hardware: a virtual stick on uinput, read back through `InputService`.
//! Needs write access to /dev/uinput (root or a udev rule), so it is ignored by default:
//!   tools\linux\check.ps1 --devices cargo test -p gob-input --test uinput -- --ignored
#![cfg(target_os = "linux")]

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::fd::AsRawFd;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use gob_input::{InputEvent, InputService};

const EV_SYN: u16 = 0;
const EV_KEY: u16 = 1;
const EV_ABS: u16 = 3;
const EV_REP: u16 = 0x14;
const KEY_A: u16 = 30;
const KEY_RIGHTCTRL: u16 = 97;
const BTN_TRIGGER: u16 = 0x120;
const BTN_TRIGGER_HAPPY: u16 = 0x2c0;
const ABS_X: u16 = 0;
const ABS_Y: u16 = 1;
const ABS_RZ: u16 = 5;
const ABS_THROTTLE: u16 = 6;
const ABS_HAT0X: u16 = 0x10;
const ABS_HAT0Y: u16 = 0x11;
/// 32 joystick/gamepad codes + 64 "trigger happy" codes: everything below KEY_MAX the
/// kernel's HID driver gives joystick buttons.
const BUTTONS: usize = 96;
const NAME: &str = "Test VKB Gladiator";

/// One test at a time: each starts its own input service, and the exclusive list is shared by
/// every service of the process (the program has only one).
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

/// `_IOW('U', nr, size)` / `_IO('U', nr)`.
fn uinput_request(nr: u32, size: usize) -> u32 {
    let dir = if size > 0 { 1u32 << 30 } else { 0 };
    dir | ((size as u32) << 16) | (u32::from(b'U') << 8) | nr
}

fn ioctl_int(file: &File, nr: u32, value: libc::c_int) {
    // SAFETY: UI_SET_*BIT take an int argument.
    let r = unsafe {
        libc::ioctl(
            file.as_raw_fd(),
            uinput_request(nr, size_of::<libc::c_int>()) as _,
            value,
        )
    };
    assert!(
        r >= 0,
        "uinput ioctl {nr}: {}",
        std::io::Error::last_os_error()
    );
}

fn ioctl_ptr<T>(file: &File, nr: u32, value: &T) {
    // SAFETY: `value` is the struct this request reads.
    let r = unsafe {
        libc::ioctl(
            file.as_raw_fd(),
            uinput_request(nr, size_of::<T>()) as _,
            std::ptr::from_ref(value),
        )
    };
    assert!(
        r >= 0,
        "uinput ioctl {nr}: {}",
        std::io::Error::last_os_error()
    );
}

/// `struct uinput_setup`.
#[repr(C)]
struct Setup {
    id: [u16; 4],
    name: [u8; 80],
    ff_effects_max: u32,
}

/// `struct uinput_abs_setup`.
#[repr(C)]
struct AbsSetup {
    code: u16,
    info: libc::input_absinfo,
}

/// A uinput device, removed on drop.
struct Virtual(File);

impl Virtual {
    /// The test stick: 96 buttons, X, Y, Rz, throttle and a hat.
    fn stick() -> Self {
        let buttons =
            (BTN_TRIGGER..BTN_TRIGGER + 32).chain(BTN_TRIGGER_HAPPY..BTN_TRIGGER_HAPPY + 64);
        let axes = [
            (ABS_X, 0, 4095),
            (ABS_Y, 0, 4095),
            (ABS_RZ, 0, 4095),
            (ABS_THROTTLE, 0, 1023),
            (ABS_HAT0X, -1, 1),
            (ABS_HAT0Y, -1, 1),
        ];
        Self::create(NAME, [0x03, 0x231d, 0x0200, 1], &[], buttons, &axes)
    }

    /// A keyboard with key repeat and the first 127 key codes.
    fn keyboard(name: &str) -> Self {
        Self::create(name, [0x03, 0x1234, 0x5678, 1], &[EV_REP], 1..128, &[])
    }

    fn create(
        name: &str,
        id: [u16; 4],
        events: &[u16],
        keys: impl Iterator<Item = u16>,
        axes: &[(u16, i32, i32)],
    ) -> Self {
        let file = OpenOptions::new()
            .write(true)
            .open("/dev/uinput")
            .expect("/dev/uinput (run as root, see the file comment)");
        for &ev in [EV_KEY, EV_ABS].iter().chain(events) {
            ioctl_int(&file, 100, ev.into());
        }
        for code in keys {
            ioctl_int(&file, 101, code.into());
        }
        for &(code, minimum, maximum) in axes {
            ioctl_int(&file, 103, code.into());
            let info = libc::input_absinfo {
                value: (minimum + maximum) / 2,
                minimum,
                maximum,
                fuzz: 0,
                flat: 0,
                resolution: 0,
            };
            ioctl_ptr(&file, 4, &AbsSetup { code, info });
        }
        let mut setup = Setup {
            id,
            name: [0; 80],
            ff_effects_max: 0,
        };
        setup.name[..name.len()].copy_from_slice(name.as_bytes());
        ioctl_ptr(&file, 3, &setup);
        // SAFETY: UI_DEV_CREATE has no argument.
        let r = unsafe { libc::ioctl(file.as_raw_fd(), uinput_request(1, 0) as _) };
        assert!(r >= 0, "UI_DEV_CREATE: {}", std::io::Error::last_os_error());
        Self(file)
    }

    /// Sends events and ends the report.
    fn send(&mut self, events: &[(u16, u16, i32)]) {
        let mut bytes = Vec::new();
        for &(type_, code, value) in events.iter().chain(&[(EV_SYN, 0, 0)]) {
            // SAFETY: `input_event` is plain data; all zeroes is a valid value.
            let mut e: libc::input_event = unsafe { std::mem::zeroed() };
            (e.type_, e.code, e.value) = (type_, code, value);
            // SAFETY: viewing a plain struct as its bytes.
            bytes.extend_from_slice(unsafe {
                std::slice::from_raw_parts(
                    std::ptr::from_ref(&e).cast::<u8>(),
                    size_of::<libc::input_event>(),
                )
            });
        }
        self.0.write_all(&bytes).expect("uinput write");
    }
}

impl Drop for Virtual {
    fn drop(&mut self) {
        // SAFETY: UI_DEV_DESTROY has no argument.
        unsafe { libc::ioctl(self.0.as_raw_fd(), uinput_request(2, 0) as _) };
    }
}

/// The first event matching `want` within `secs` seconds.
fn wait_within(
    secs: u64,
    events: &Receiver<InputEvent>,
    want: impl Fn(&InputEvent) -> bool,
) -> Option<InputEvent> {
    let end = Instant::now() + Duration::from_secs(secs);
    while let Some(left) = end.checked_duration_since(Instant::now()) {
        if let Ok(e) = events.recv_timeout(left)
            && want(&e)
        {
            return Some(e);
        }
    }
    None
}

fn wait_for(events: &Receiver<InputEvent>, want: impl Fn(&InputEvent) -> bool) -> InputEvent {
    wait_within(2, events, want).expect("event not seen in time")
}

#[test]
#[ignore = "needs /dev/uinput"]
fn virtual_stick_reads_back() {
    let _serial = serial();
    let mut stick = Virtual::stick();
    let input = InputService::start(|| {});
    input.set_event_mode(true);
    let events = input.subscribe();

    let end = Instant::now() + Duration::from_secs(5);
    let state = loop {
        if let Some(d) = input.devices().iter().find(|d| d.info.name == NAME) {
            break d.clone();
        }
        assert!(Instant::now() < end, "virtual stick not found");
        std::thread::sleep(Duration::from_millis(50));
    };
    let info = &state.info;
    println!("{info:?}");
    assert_eq!((info.vendor_id, info.product_id), (0x231d, 0x0200));
    assert_eq!(
        info.axes,
        vec![1, 2, 6, 7],
        "X, Y, Rz, throttle as slider 1"
    );
    assert_eq!(info.buttons, BUTTONS);
    assert_eq!(info.hats, 1);
    assert!(
        info.guid.starts_with("0200231d-0003-0000-"),
        "{}",
        info.guid
    );
    let guid = info.guid.clone();
    // Let the event mode reopen settle before sending.
    std::thread::sleep(Duration::from_millis(300));

    // A press and release in one read must both arrive (event mode keeps short presses).
    stick.send(&[(EV_KEY, BTN_TRIGGER_HAPPY + 63, 1)]);
    stick.send(&[(EV_KEY, BTN_TRIGGER_HAPPY + 63, 0)]);
    let pressed = |p: bool| move |e: &InputEvent| matches!(e, InputEvent::Button { id: 96, pressed, .. } if *pressed == p);
    let e = wait_for(&events, pressed(true));
    assert!(matches!(e, InputEvent::Button { device, .. } if device == guid));
    wait_for(&events, pressed(false));

    stick.send(&[(EV_ABS, ABS_HAT0X, 1), (EV_ABS, ABS_HAT0Y, -1)]);
    let e = wait_for(&events, |e| matches!(e, InputEvent::Hat { .. }));
    assert!(
        matches!(
            e,
            InputEvent::Hat {
                id: 1,
                direction: Some(4500),
                ..
            }
        ),
        "one diagonal, not two steps: {e:?}"
    );

    stick.send(&[(EV_ABS, ABS_THROTTLE, 1023)]);
    let e = wait_for(&events, |e| matches!(e, InputEvent::Axis { id: 7, .. }));
    assert!(
        matches!(e, InputEvent::Axis { value, .. } if value > 0.999),
        "{e:?}"
    );

    drop(stick);
    wait_for(&events, |e| matches!(e, InputEvent::DevicesChanged));
    assert!(!input.devices().iter().any(|d| d.info.name == NAME));
}

#[test]
#[ignore = "needs /dev/uinput"]
fn keyboard_keys_as_scan_codes() {
    let _serial = serial();
    let mut keyboard = Virtual::keyboard("Test Keyboard");
    // Our own output device (phase 3 creates it on uinput): its keys are not input.
    let mut own = Virtual::keyboard("Joystick Gobloon Keyboard");
    let input = InputService::start(|| {});
    let events = input.subscribe();
    // The keyboard thread looks for keyboards right away; give it the first scan.
    std::thread::sleep(Duration::from_millis(500));
    let is_key = |e: &InputEvent| matches!(e, InputEvent::Key { .. });

    own.send(&[(EV_KEY, KEY_A, 1)]);
    own.send(&[(EV_KEY, KEY_A, 0)]);
    assert_eq!(
        wait_within(1, &events, is_key),
        None,
        "own keys are ignored"
    );

    keyboard.send(&[(EV_KEY, KEY_RIGHTCTRL, 1)]);
    keyboard.send(&[(EV_KEY, KEY_RIGHTCTRL, 2)]); // auto-repeat: not reported
    keyboard.send(&[(EV_KEY, KEY_RIGHTCTRL, 0)]);
    let key = |pressed| InputEvent::Key {
        scan_code: 0x1d,
        extended: true,
        pressed,
    };
    assert_eq!(wait_for(&events, is_key), key(true));
    assert_eq!(wait_for(&events, is_key), key(false));
}

/// The event node of the input device with this name.
fn event_node(name: &str) -> std::path::PathBuf {
    let dir = std::fs::read_dir("/sys/class/input").unwrap();
    dir.flatten()
        .find_map(|e| {
            let file = e.file_name().into_string().ok()?;
            let found = file.starts_with("event")
                && std::fs::read_to_string(e.path().join("device/name"))
                    .ok()?
                    .trim_end()
                    == name;
            found.then(|| std::path::Path::new("/dev/input").join(file))
        })
        .unwrap_or_else(|| panic!("{name} not found"))
}

/// Whether another reader (a game) gets an event from the node within 300 ms.
fn other_reader_sees(node: &mut File) -> bool {
    use std::io::Read;
    let mut buf = [0u8; 24 * 16];
    let end = Instant::now() + Duration::from_millis(300);
    while Instant::now() < end {
        if node.read(&mut buf).is_ok_and(|n| n > 0) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

#[test]
#[ignore = "needs /dev/uinput"]
fn exclusive_hides_from_other_readers() {
    let _serial = serial();
    use std::os::unix::fs::OpenOptionsExt;

    const GRAB_NAME: &str = "Test Grab Stick";
    let mut stick = Virtual::create(
        GRAB_NAME,
        [0x03, 0x1111, 0x2222, 1],
        &[],
        BTN_TRIGGER..BTN_TRIGGER + 4,
        &[(ABS_X, 0, 255), (ABS_Y, 0, 255)],
    );
    let input = InputService::start(|| {});
    let events = input.subscribe();
    let end = Instant::now() + Duration::from_secs(5);
    let guid = loop {
        if let Some(d) = input.devices().iter().find(|d| d.info.name == GRAB_NAME) {
            break d.info.guid.clone();
        }
        assert!(Instant::now() < end, "stick not found");
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(gob_input::exclusive::supported());
    let listed = gob_input::exclusive::devices();
    assert!(listed.iter().any(|d| d.guid == guid), "{listed:?}");

    let mut game = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(event_node(GRAB_NAME))
        .unwrap();
    let pressed = |e: &InputEvent| {
        matches!(
            e,
            InputEvent::Button {
                id: 1,
                pressed: true,
                ..
            }
        )
    };
    let released = |e: &InputEvent| {
        matches!(
            e,
            InputEvent::Button {
                id: 1,
                pressed: false,
                ..
            }
        )
    };

    gob_input::exclusive::set([guid.clone()]);
    std::thread::sleep(Duration::from_millis(100)); // one round of the input thread
    stick.send(&[(EV_KEY, BTN_TRIGGER, 1)]);
    wait_for(&events, pressed);
    assert!(
        !other_reader_sees(&mut game),
        "hidden: the game gets nothing"
    );

    gob_input::exclusive::set([]);
    std::thread::sleep(Duration::from_millis(100));
    stick.send(&[(EV_KEY, BTN_TRIGGER, 0)]);
    wait_for(&events, released);
    assert!(other_reader_sees(&mut game), "shown again");
}
