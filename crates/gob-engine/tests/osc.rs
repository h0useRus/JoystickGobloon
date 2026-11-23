//! OSC inputs and the send-osc action.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use gob_core::{InputType, NewAction, OSC_GUID, OscArg, OscArgKind, OscKind, Profile};
use gob_engine::{Engine, Output};

#[derive(Debug, Clone, PartialEq)]
enum Out {
    Axis(u8, f32),
    Button(u8, bool),
    Osc(String, u16, gob_osc::Message),
}

#[derive(Default)]
struct Recorder(Vec<Out>);

impl Output for Recorder {
    fn axis(&mut self, _device: u32, axis: u8, value: f32) {
        self.0
            .push(Out::Axis(axis, (value * 1000.0).round() / 1000.0));
    }
    fn button(&mut self, _device: u32, button: u8, pressed: bool) {
        self.0.push(Out::Button(button, pressed));
    }
    fn hat(&mut self, _device: u32, _hat: u8, _direction: Option<u16>) {}
    fn send_osc(&mut self, host: &str, port: u16, message: &gob_osc::Message) {
        self.0.push(Out::Osc(host.into(), port, message.clone()));
    }
}

/// `/fire` (button) presses vJoy button 1, `/throttle` (axis, 0..127) moves vJoy axis 3, and
/// `/fire` also sends `/deck/text "Fire" <value>` to Companion on press and release.
fn profile() -> (Profile, String) {
    let mut p = Profile::default();
    let fire = p.add_osc_input("/fire", OscKind::Button).unwrap();
    let throttle = p.add_osc_input("/throttle", OscKind::Axis).unwrap();
    p.set_osc_range(throttle, 0.0, 127.0);
    let root = p.bind(OSC_GUID, "button", &fire.to_string(), "Default");
    p.add_child(
        &root,
        "actions",
        gob_core::Action::new(&NewAction::MapToVjoy {
            input_type: InputType::Button,
            device: 1,
            input_id: 1,
        }),
    )
    .unwrap();
    let send = p
        .add_child(&root, "actions", gob_core::Action::new(&NewAction::SendOsc))
        .unwrap();
    {
        let a = p.action_mut(&send).unwrap();
        a.set("address", "string", "/deck/text");
        a.set("activation-mode", "string", "both");
    }
    p.set_osc_args(
        &send,
        &[
            OscArg::new(OscArgKind::Text, "Fire"),
            OscArg::new(OscArgKind::Input, ""),
            OscArg::new(OscArgKind::Int, "2,6"),
        ],
    );
    let root = p.bind(OSC_GUID, "axis", &throttle.to_string(), "Default");
    p.add_child(
        &root,
        "actions",
        gob_core::Action::new(&NewAction::MapToVjoy {
            input_type: InputType::Axis,
            device: 1,
            input_id: 3,
        }),
    )
    .unwrap();
    (p, send)
}

fn sent(value: f32) -> Out {
    Out::Osc(
        gob_core::OSC_SEND_HOST.into(),
        gob_core::OSC_SEND_PORT,
        gob_osc::Message::new(
            "/deck/text",
            vec![
                gob_osc::Arg::Str("Fire".into()),
                gob_osc::Arg::Float(value),
                gob_osc::Arg::Int(3),
            ],
        ),
    )
}

#[test]
fn osc_messages_move_axes_and_press_buttons() {
    let (p, _) = profile();
    let mut e = Engine::new(&p, &HashMap::new());
    let (mut out, t0) = (Recorder::default(), Instant::now());
    e.osc("/throttle", Some(127.0), t0, &mut out);
    e.osc("/throttle", Some(31.75), t0, &mut out);
    e.osc("/throttle", None, t0, &mut out);
    assert_eq!(out.0, [Out::Axis(3, 1.0), Out::Axis(3, -0.5)]);
    out.0.clear();
    e.osc("/fire", Some(1.0), t0, &mut out);
    e.osc("/fire", Some(0.0), t0, &mut out);
    assert_eq!(
        out.0,
        [
            Out::Button(1, true),
            sent(1.0),
            Out::Button(1, false),
            sent(0.0)
        ]
    );
    out.0.clear();
    e.osc("/unknown", Some(1.0), t0, &mut out);
    e.osc("/Fire", Some(1.0), t0, &mut out);
    assert!(out.0.is_empty(), "addresses match exactly");
}

#[test]
fn a_button_message_without_a_number_is_a_short_press() {
    let (p, _) = profile();
    let mut e = Engine::new(&p, &HashMap::new());
    let (mut out, t0) = (Recorder::default(), Instant::now());
    let ms = |n| t0 + Duration::from_millis(n);
    e.osc("/fire", None, t0, &mut out);
    assert_eq!(out.0[0], Out::Button(1, true));
    assert_eq!(e.next_deadline(), Some(ms(gob_core::OSC_PULSE_MS)));
    e.osc("/fire", None, ms(50), &mut out);
    e.tick(ms(120), &mut out);
    assert_eq!(out.0.len(), 2, "again while down: held on, no second press");
    e.tick(ms(160), &mut out);
    assert_eq!(out.0[2], Out::Button(1, false));
    // A press without a number, then a release with one: the release counts, no timer left.
    e.osc("/fire", None, ms(200), &mut out);
    e.osc("/fire", Some(0.0), ms(210), &mut out);
    assert_eq!(e.next_deadline(), None);
    assert_eq!(out.0[6], Out::Button(1, false));
}

#[test]
fn a_replaced_engine_releases_its_short_presses() {
    let (p, _) = profile();
    let mut e = Engine::new(&p, &HashMap::new());
    let (mut out, t0) = (Recorder::default(), Instant::now());
    e.osc("/fire", None, t0, &mut out);
    out.0.clear();
    e.stop_osc(t0, &mut out);
    assert_eq!(out.0, [Out::Button(1, false), sent(0.0)]);
    assert_eq!(e.next_deadline(), None);
}

#[test]
fn send_osc_needs_an_address() {
    let (mut p, send) = profile();
    p.action_mut(&send).unwrap().set("address", "string", "/");
    let mut e = Engine::new(&p, &HashMap::new());
    let mut out = Recorder::default();
    e.osc("/fire", Some(1.0), Instant::now(), &mut out);
    assert_eq!(out.0, [Out::Button(1, true)]);
}
