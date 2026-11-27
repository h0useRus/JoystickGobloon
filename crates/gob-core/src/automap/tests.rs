use super::*;
use crate::VirtualOutput;

#[test]
fn axes_keep_their_slot_and_fill_the_rest_in_order() {
    // X, Y, Rz, Slider onto a joystick with X, Y, Z, Rx.
    let t = axis_targets(&[1, 2, 6, 7], &[1, 2, 3, 4]);
    assert_eq!(
        t,
        vec![(1, Some(1)), (2, Some(2)), (6, Some(3)), (7, Some(4))]
    );
    // More axes than the joystick has: the last ones stay unmapped.
    let t = axis_targets(&[1, 2, 3], &[1, 2]);
    assert_eq!(t, vec![(1, Some(1)), (2, Some(2)), (3, None)]);
}

#[test]
fn automap_binds_what_fits_and_reports_the_rest() {
    let from = Layout {
        axes: vec![1, 2, 3],
        buttons: 40,
        hats: 2,
    };
    let to = Layout {
        axes: vec![1, 2],
        buttons: 32,
        hats: 4,
    };
    let mut p = Profile::default();
    let mut used = Layout::default();
    p.automap("stick", "Stick", &from, 2, &to, &mut used, "Default");
    assert_eq!(p.devices.len(), 1);
    // 2 axes + 32 buttons + 2 hats.
    assert_eq!(p.inputs.len(), 36);
    let usage = p.output_usage(None);
    let vjoy = |input_type, id| VirtualOutput::Vjoy {
        device: 2,
        input_type,
        id,
    };
    assert!(usage.contains_key(&vjoy(InputType::Button, 32)));
    assert!(!usage.contains_key(&vjoy(InputType::Button, 33)));
    assert!(usage.contains_key(&vjoy(InputType::Hat, 2)));
    assert!(usage.values().all(|u| u.len() == 1), "one input per output");
    assert_eq!(p.main_source(2), Some("Stick"));
    assert_eq!(p.main_source(1), None, "nothing writes to device 1");

    let missing = from.missing_in(&to);
    assert_eq!(
        missing,
        Layout {
            axes: vec![3],
            buttons: 8,
            hats: 0,
        }
    );
    assert!(to.missing_in(&to).is_empty());
    assert!(to.without(&used).buttons == 0 && to.without(&used).axes.is_empty());
}

/// The output each input of `device` maps to, as (input type, input, output).
fn targets(p: &Profile, device: &str) -> Vec<(InputType, u32, u32)> {
    p.output_usage(None)
        .into_iter()
        .flat_map(|(o, users)| users.into_iter().map(move |u| (o.clone(), u.binding)))
        .filter(|(_, b)| p.inputs[*b].device_id == device)
        .filter_map(|(o, b)| match o {
            VirtualOutput::Vjoy { input_type, id, .. } => {
                Some((input_type, p.inputs[b].input_id.parse().ok()?, id))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn controllers_sharing_a_joystick_fill_it_in_order() {
    // Two 2-axis, 20-button boxes and a third that only partly fits, onto one joystick
    // with 6 axes, 50 buttons and 1 hat.
    let small = Layout {
        axes: vec![1, 2],
        buttons: 20,
        hats: 1,
    };
    let to = Layout {
        axes: vec![1, 2, 3, 4, 5, 6],
        buttons: 50,
        hats: 1,
    };
    let mut p = Profile::default();
    let mut used = Layout::default();
    let missing: Vec<Layout> = ["a", "b", "c"]
        .into_iter()
        .map(|d| p.automap(d, d, &small, 1, &to, &mut used, "Default"))
        .collect();
    let a = targets(&p, "a");
    let b = targets(&p, "b");
    let c = targets(&p, "c");
    assert!(a.contains(&(InputType::Axis, 1, 1)) && a.contains(&(InputType::Axis, 2, 2)));
    assert!(a.contains(&(InputType::Button, 20, 20)));
    assert!(a.contains(&(InputType::Hat, 1, 1)));
    assert!(b.contains(&(InputType::Axis, 1, 3)) && b.contains(&(InputType::Axis, 2, 4)));
    assert!(b.contains(&(InputType::Button, 1, 21)));
    assert!(b.contains(&(InputType::Button, 20, 40)));
    assert!(
        !b.iter().any(|(t, _, _)| *t == InputType::Hat),
        "the hat is taken"
    );
    assert!(c.contains(&(InputType::Axis, 1, 5)) && c.contains(&(InputType::Axis, 2, 6)));
    assert!(c.contains(&(InputType::Button, 10, 50)));
    assert!(!c.contains(&(InputType::Button, 11, 51)), "no room past 50");
    assert!(p.output_usage(None).values().all(|u| u.len() == 1));
    // What the wizard warns about: the same as `take` without a profile.
    assert!(missing[0].is_empty());
    assert_eq!(
        missing[1],
        Layout {
            axes: vec![],
            buttons: 0,
            hats: 1
        }
    );
    assert_eq!(
        missing[2],
        Layout {
            axes: vec![],
            buttons: 10,
            hats: 1
        }
    );
    let mut dry = Layout::default();
    let dry_missing: Vec<Layout> = (0..3).map(|_| small.take(&to, &mut dry)).collect();
    assert_eq!(dry_missing, missing);
    assert_eq!(dry, used);
}
