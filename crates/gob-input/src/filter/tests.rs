use super::*;
use crate::{Calibration, DeviceInfo};

fn device() -> DeviceState {
    DeviceState::new(DeviceInfo {
        guid: "dev".into(),
        axes: vec![1],
        buttons: 1,
        hats: 1,
        ..Default::default()
    })
}

fn filtered(jitter: u16, debounce: u16) -> Filtered {
    let mut f = Filtered::new(&device());
    let mut cal = Calibrations::new();
    let c = Calibration {
        jitter,
        ..Default::default()
    };
    cal.insert(("dev".into(), 1), c);
    f.configure(&cal, debounce);
    f
}

fn axis(f: &mut Filtered, v: f32, at: Instant) -> Vec<InputEvent> {
    let mut s = device();
    s.axes[0] = v;
    let mut sent = Vec::new();
    f.update(&s, at, &mut |e| sent.push(e));
    sent
}

#[test]
fn a_new_layout_brings_its_buttons_through() {
    let mut f = filtered(0, 0);
    let mut s = device();
    assert!(!f.follow_layout(&s));
    s.info.buttons = 3;
    s.buttons = vec![false, false, true];
    assert!(f.follow_layout(&s));
    assert_eq!(f.out.info.buttons, 3);
    let mut sent = Vec::new();
    assert!(f.update(&s, Instant::now(), &mut |e| sent.push(e)));
    assert_eq!(f.out.buttons, vec![false, false, true]);
    assert!(matches!(
        sent.as_slice(),
        [InputEvent::Button {
            id: 3,
            pressed: true,
            ..
        }]
    ));
}

#[test]
fn jitter_below_the_threshold_is_ignored() {
    // 1 % of the travel = 0.02.
    let mut f = filtered(10, 0);
    let t = Instant::now();
    assert_eq!(axis(&mut f, 0.5, t).len(), 1);
    for v in [0.51, 0.49, 0.515, 0.5] {
        assert!(axis(&mut f, v, t).is_empty(), "{v} is jitter");
    }
    assert_eq!(axis(&mut f, 0.53, t).len(), 1, "a real move passes");
    assert_eq!(f.out.axes[0], 0.53);
    // The ends and the center are reached even from closer than the threshold.
    axis(&mut f, 0.99, t);
    assert_eq!(axis(&mut f, 1.0, t).len(), 1);
    axis(&mut f, 0.01, t);
    assert_eq!(axis(&mut f, 0.0, t).len(), 1);
}

#[test]
fn no_threshold_passes_every_change() {
    let mut f = filtered(0, 0);
    let t = Instant::now();
    assert_eq!(axis(&mut f, 0.001, t).len(), 1);
    assert!(axis(&mut f, 0.0012, t).is_empty(), "below the noise");
}

#[test]
fn debounce_keeps_the_first_change_and_sends_the_final_state() {
    let mut f = filtered(0, 20);
    let t = Instant::now();
    let press = |pressed| Buffered::Button { id: 1, pressed };
    // A bouncing press: the first edge passes at once, the bounces do not.
    assert!(f.event(press(true), t).is_some());
    assert!(
        f.event(press(false), t + Duration::from_millis(2))
            .is_none()
    );
    assert!(f.event(press(true), t + Duration::from_millis(4)).is_none());
    assert_eq!(f.next_deadline(), Some(t + Duration::from_millis(20)));

    // Let go during the hold: the release follows once it ends.
    let mut s = device();
    let mut sent = Vec::new();
    f.update(&s, t + Duration::from_millis(10), &mut |e| sent.push(e));
    assert!(sent.is_empty());
    f.update(&s, t + Duration::from_millis(20), &mut |e| sent.push(e));
    let release = InputEvent::Button {
        device: "dev".into(),
        id: 1,
        pressed: false,
    };
    assert_eq!(sent, vec![release]);

    // Without debounce every edge passes.
    let mut f = filtered(0, 0);
    s.buttons[0] = true;
    assert!(f.event(press(true), t).is_some());
    assert!(f.event(press(false), t).is_some());
    assert_eq!(f.next_deadline(), None);
}
