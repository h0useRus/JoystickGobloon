use super::*;

use gob_virtual::Write;

/// A virtual joystick that records each driver call.
struct Calls(Arc<Mutex<Vec<Vec<Write>>>>);

impl gob_virtual::Device for Calls {
    fn set_axis(&self, slot: u8, value: f32) -> bool {
        self.write(&[Write::Axis(slot, value)])
    }
    fn set_button(&self, n: u8, pressed: bool) -> bool {
        self.write(&[Write::Button(n, pressed)])
    }
    fn set_hat(&self, n: u8, direction: Option<u16>) -> bool {
        self.write(&[Write::Hat(n, direction)])
    }
    fn reset(&self) -> bool {
        true
    }
    fn write(&self, changes: &[Write]) -> bool {
        self.0.lock().unwrap().push(changes.to_vec());
        true
    }
}

fn output() -> (SystemOutput, Arc<Mutex<Vec<Vec<Write>>>>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut out = SystemOutput::new(
        crate::lighting::Lighting::new(HashMap::new(), None),
        None,
        std::sync::mpsc::channel().0,
    );
    out.devices.insert(1, Box::new(Calls(calls.clone())));
    (out, calls)
}

#[test]
fn device_ids_are_made_once_per_device() {
    let mut ids = DeviceIds::default();
    let a = ids.key("AAAA-01", InputKind::Axis, 1);
    let b = ids.key("AAAA-01", InputKind::Button, 3);
    assert_eq!(&*a.device, "aaaa-01");
    assert!(Arc::ptr_eq(&a.device, &b.device));
    assert_eq!(a, InputKey::new("aaaa-01", InputKind::Axis, 1));
}

#[test]
fn the_tap_shows_what_the_driver_took() {
    let (mut out, _calls) = output();
    let tap = out.tap.clone();
    out.button(1, 3, true);
    out.axis(1, 1, 0.5);
    // Device 2 is not held: nothing of it was sent.
    out.button(2, 1, true);
    assert_eq!(
        tap.get(crate::Sent::Joystick(1, InputKind::Button, 3)),
        None
    );
    out.flush();
    let value = |s| tap.get(s).map(|t| t.value);
    assert_eq!(
        value(crate::Sent::Joystick(1, InputKind::Button, 3)),
        Some(Value::Button(true))
    );
    assert_eq!(
        value(crate::Sent::Joystick(1, InputKind::Axis, 1)),
        Some(Value::Axis(0.5))
    );
    assert_eq!(value(crate::Sent::Joystick(2, InputKind::Button, 1)), None);
    let room = out.sent.capacity();
    out.button(1, 3, false);
    out.flush();
    assert_eq!(
        value(crate::Sent::Joystick(1, InputKind::Button, 3)),
        Some(Value::Button(false))
    );
    assert_eq!(
        out.sent.capacity(),
        room,
        "the buffer is kept between passes"
    );
}

#[test]
fn one_driver_call_per_device_and_pass() {
    let (mut out, calls) = output();
    out.axis(1, 1, 0.5);
    out.axis(1, 2, -0.5);
    out.button(1, 3, true);
    out.axis(1, 1, 0.25);
    out.flush();
    assert_eq!(
        *calls.lock().unwrap(),
        [vec![
            Write::Axis(1, 0.25),
            Write::Axis(2, -0.5),
            Write::Button(3, true)
        ]],
        "the newer axis value replaces the queued one"
    );
    out.flush();
    assert_eq!(
        calls.lock().unwrap().len(),
        1,
        "nothing queued, nothing sent"
    );
}

#[test]
fn unchanged_values_are_not_sent() {
    let (mut out, calls) = output();
    out.axis(1, 1, 0.5);
    out.flush();
    out.axis(1, 1, 0.5);
    out.button(1, 1, false);
    out.button(1, 1, false);
    out.flush();
    assert_eq!(
        *calls.lock().unwrap(),
        [vec![Write::Axis(1, 0.5)], vec![Write::Button(1, false)]]
    );
    assert_eq!(
        out.vjoy_state(1, InputKind::Axis, 1),
        Some(Value::Axis(0.5)),
        "conditions still read the value"
    );
}

#[test]
fn a_press_and_release_in_one_pass_both_reach_the_driver() {
    let (mut out, calls) = output();
    out.button(1, 4, true);
    out.button(1, 4, false);
    out.hat(1, 1, Some(9000));
    out.flush();
    assert_eq!(
        *calls.lock().unwrap(),
        [
            vec![Write::Button(4, true)],
            vec![Write::Button(4, false), Write::Hat(1, Some(9000))]
        ]
    );
}

#[test]
fn a_write_that_did_not_arrive_is_sent_again() {
    let (mut out, calls) = output();
    out.button(2, 1, true);
    out.flush();
    // Device 2 is held now: the same value goes out although it is unchanged.
    out.devices.insert(2, Box::new(Calls(calls.clone())));
    out.button(2, 1, true);
    out.flush();
    out.button(2, 1, true);
    out.flush();
    assert_eq!(
        *calls.lock().unwrap(),
        [vec![Write::Button(1, true)]],
        "sent once, then unchanged again"
    );
}

/// A virtual joystick without axis 7: a call with it is refused, the rest of it is taken.
struct NoAxis7(Arc<Mutex<Vec<Vec<Write>>>>);

impl gob_virtual::Device for NoAxis7 {
    fn set_axis(&self, slot: u8, value: f32) -> bool {
        self.write(&[Write::Axis(slot, value)])
    }
    fn set_button(&self, n: u8, pressed: bool) -> bool {
        self.write(&[Write::Button(n, pressed)])
    }
    fn set_hat(&self, n: u8, direction: Option<u16>) -> bool {
        self.write(&[Write::Hat(n, direction)])
    }
    fn reset(&self) -> bool {
        true
    }
    fn write(&self, changes: &[Write]) -> bool {
        self.0.lock().unwrap().push(changes.to_vec());
        !changes.iter().any(|w| matches!(w, Write::Axis(7, _)))
    }
}

#[test]
fn one_refused_write_does_not_make_the_others_unsent() {
    let (mut out, calls) = output();
    out.devices.insert(1, Box::new(NoAxis7(calls.clone())));
    out.axis(1, 1, 0.5);
    out.axis(1, 7, 0.5);
    out.button(1, 2, true);
    out.flush();
    calls.lock().unwrap().clear();
    // The same values again: only the refused one is tried again.
    out.axis(1, 1, 0.5);
    out.axis(1, 7, 0.5);
    out.button(1, 2, true);
    out.flush();
    let calls = calls.lock().unwrap();
    assert!(
        calls
            .iter()
            .flatten()
            .all(|w| matches!(w, Write::Axis(7, _))),
        "only axis 7 goes out again: {calls:?}"
    );
    assert!(out.failing.contains(&write_key(1, Write::Axis(7, 0.5)).0));
    assert!(!out.failing.contains(&write_key(1, Write::Axis(1, 0.5)).0));
}

fn listening(microphone: &str, on: bool) -> gob_voice::VoiceEvent {
    gob_voice::VoiceEvent::Listening {
        microphone: microphone.into(),
        on,
    }
}

#[test]
fn listening_while_any_microphone_listens() {
    let mut service = ServiceState::default();
    service.handle(listening("left", true));
    service.handle(listening("right", true));
    service.handle(listening("left", false));
    assert!(service.apply(VoiceStatus::default()).listening);
    service.handle(listening("right", false));
    assert!(!service.apply(VoiceStatus::default()).listening);
}

#[test]
fn the_service_state_outlives_profiles() {
    let mut service = ServiceState::default();
    service.handle(gob_voice::VoiceEvent::Failed("no library".into()));
    // A profile without voice in between does not lose it: the next one with voice shows it.
    let next = service.apply(VoiceStatus::default());
    assert_eq!(next.error.as_deref(), Some("no library"));
    assert!(!next.ready);
    service.handle(gob_voice::VoiceEvent::Ready);
    let next = service.apply(VoiceStatus::default());
    assert!(next.ready && next.error.is_none());
}

#[test]
fn latency_window() {
    let t0 = Instant::now();
    let mut w = LatencyWindow::new(t0);
    w.add(Duration::from_micros(100));
    w.add(Duration::from_micros(300));
    assert_eq!(
        w.finish(t0 + Duration::from_millis(500)),
        None,
        "not over yet"
    );
    assert_eq!(
        w.finish(t0 + LATENCY_WINDOW),
        Some(Latency {
            average: Duration::from_micros(200),
            max: Duration::from_micros(300),
        })
    );
    assert_eq!(
        w.finish(t0 + 2 * LATENCY_WINDOW),
        None,
        "a window without events"
    );
}
