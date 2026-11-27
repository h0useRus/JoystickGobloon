use super::*;

fn bytes<R: Record, const N: usize>(r: &R) -> [u8; N] {
    assert_eq!(R::SIZE, N);
    let mut out = [0xAA; N];
    r.write(&mut out);
    out
}

#[test]
fn header_layout() {
    let h = Header::new(Code::SetHidden, 0x0102_0304);
    let b: [u8; 12] = bytes(&h);
    assert_eq!(b, [b'G', b'O', b'B', b'S', 0x02, 0x01, 1, 0, 4, 3, 2, 1]);
    assert_eq!(Header::read(&b), None, "over the payload limit");
    let h = Header::new(Code::Hello, 36);
    let b: [u8; 12] = bytes(&h);
    assert_eq!(Header::read(&b), Some(h));
    assert_eq!(Header::read(&b[..11]), None);
    let mut bad = b;
    bad[0] = b'X';
    assert_eq!(Header::read(&bad), None, "no magic");
}

#[test]
fn hello_layout() {
    let h = Hello::new("0.11.0");
    let b: [u8; 36] = bytes(&h);
    assert_eq!(&b[..10], &[1, 0, 0, 0, b'0', b'.', b'1', b'1', b'.', b'0']);
    assert!(b[10..].iter().all(|&x| x == 0));
    assert_eq!(Hello::read(&b), Some(h));
    assert_eq!(Hello::read(&b[..35]), None);

    let r = HelloReply {
        protocol: PROTOCOL,
        reserved: 0,
        capabilities: caps::HIDE_EVERYWHERE | caps::VIRTUAL_JOYSTICKS,
        service_version: Text::new("1.2.3"),
    };
    let b: [u8; 40] = bytes(&r);
    assert_eq!(&b[..9], &[1, 0, 0, 0, 0b1100, 0, 0, 0, b'1']);
    assert_eq!(HelloReply::read(&b), Some(r));
    assert_eq!(r.service_version.as_str(), "1.2.3");
}

#[test]
fn error_layout() {
    let b: [u8; 4] = bytes(&ErrorReply {
        code: ErrorCode::Busy,
    });
    assert_eq!(b, [6, 0, 0, 0]);
    assert_eq!(
        ErrorReply::read(&[9, 0, 0, 0]).unwrap().code,
        ErrorCode::Failed
    );
}

#[test]
fn hiding_layout() {
    let h = Hidden {
        id: Text::new("1f4b044f-0003-0000-0011-223344556677"),
        name: Text::new("T.16000M"),
        vendor: 0x044f,
        product: 0xb10a,
        present: 1,
        reserved: [0; 3],
    };
    let b: [u8; 144] = bytes(&h);
    assert_eq!(&b[..4], b"1f4b");
    assert_eq!(&b[40..48], b"T.16000M");
    assert_eq!(&b[136..141], &[0x4f, 0x04, 0x0a, 0xb1, 1]);
    assert_eq!(Hidden::read(&b), Some(h));

    let set = SetHidden {
        hidden: 1,
        reserved: [0; 3],
        device: h,
    };
    let b: [u8; 148] = bytes(&set);
    assert_eq!(&b[..5], &[1, 0, 0, 0, b'1']);
    assert_eq!(SetHidden::read(&b), Some(set));
    let b: [u8; 4] = bytes(&SetHideEnabled {
        enabled: 1,
        reserved: [0; 3],
    });
    assert_eq!(b, [1, 0, 0, 0]);

    let mut out = [0; 8 + 2 * 144];
    assert_eq!(write_hide_state(true, &[h, h], &mut out), Some(out.len()));
    assert_eq!(&out[..8], &[1, 0, 0, 0, 2, 0, 0, 0]);
    let (state, records) = hidden_records(&out).unwrap();
    assert_eq!(state.enabled, 1);
    assert_eq!(records.collect::<Vec<_>>(), [h, h]);
    assert!(
        hidden_records(&out[..8 + 144]).is_none(),
        "one record missing"
    );
    assert_eq!(write_hide_state(true, &[h, h, h], &mut out), None);
}

#[test]
fn open_node_layout() {
    let r = OpenNode {
        path: Text::new("/dev/hidraw3"),
    };
    let b: [u8; 64] = bytes(&r);
    assert_eq!(&b[..13], b"/dev/hidraw3\0");
    assert_eq!(OpenNode::read(&b), Some(r));
}

#[test]
fn virtual_layout() {
    let j = Joystick {
        id: 2,
        buttons: 95,
        axes: 0b1100_0011,
        hats: 4,
        flags: Joystick::DISCRETE_HATS,
        owner: Owner::You as u8,
        product: 0xBE02,
        name: Text::new("Gobloon Stick"),
    };
    let b: [u8; 92] = bytes(&j);
    assert_eq!(
        &b[..14],
        [
            2,
            0,
            0,
            0,
            95,
            0,
            0b1100_0011,
            4,
            1,
            1,
            0x02,
            0xBE,
            b'G',
            b'o'
        ]
    );
    assert_eq!(b[91], 0);
    assert_eq!(Joystick::read(&b), Some(j));
    let g = Gamepad {
        id: 3,
        owner: Owner::Other as u8,
        reserved: [0; 3],
    };
    let b: [u8; 8] = bytes(&g);
    assert_eq!(b, [3, 0, 0, 0, 2, 0, 0, 0]);

    let mut out = [0; 8 + 2 * 92 + 8];
    assert_eq!(
        write_virtual_state(&[j, j], &[g], &mut out),
        Some(out.len())
    );
    assert_eq!(&out[..8], &[2, 0, 0, 0, 1, 0, 0, 0]);
    let (js, gs) = virtual_records(&out).unwrap();
    assert_eq!(js.collect::<Vec<_>>(), [j, j]);
    assert_eq!(gs.collect::<Vec<_>>(), [g]);
    assert!(
        virtual_records(&out[..out.len() - 1]).is_none(),
        "gamepad cut off"
    );

    let a = ApplyVirtual {
        kind: ChangeKind::SetJoystick as u8,
        reserved: [0; 3],
        joystick: j,
    };
    let b: [u8; 96] = bytes(&a);
    assert_eq!(&b[..5], &[1, 0, 0, 0, 2]);
    assert_eq!(ApplyVirtual::read(&b), Some(a));
    assert_eq!(ChangeKind::from_u8(9), None);
    assert_eq!(DeviceKind::from_u8(3), Some(DeviceKind::Mouse));
    assert_eq!(DeviceKind::from_u8(4), None);
    let r = VirtualRef {
        kind: DeviceKind::Gamepad as u8,
        reserved: [0; 3],
        id: 4,
    };
    let b: [u8; 8] = bytes(&r);
    assert_eq!(b, [1, 0, 0, 0, 4, 0, 0, 0]);
    assert_eq!(Owner::from_u8(7), Owner::Free);
}

#[test]
fn codes_are_unique_and_round_trip() {
    for (i, c) in Code::ALL.iter().enumerate() {
        assert_eq!(Code::from_u16(*c as u16), Some(*c));
        assert!(Code::ALL[i + 1..].iter().all(|o| *o as u16 != *c as u16));
    }
    assert_eq!(Code::from_u16(0x7777), None);
}

#[test]
fn text_cuts_at_characters() {
    assert_eq!(Text::<4>::new("abcdef").as_str(), "abcd");
    // "é" is two bytes: it does not fit in the last one.
    assert_eq!(Text::<4>::new("abcé").as_str(), "abc");
    assert_eq!(Text::<4>::new("").as_str(), "");
    assert_eq!(Text::<4>([b'a', 0xFF, 0, 0]).as_str(), "a");
}
