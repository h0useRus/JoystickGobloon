use super::*;

fn mix(channels: usize, rate: u32, data: &[f32]) -> (Vec<i16>, f32) {
    mix_with_gain(channels, rate, 1.0, data)
}

fn mix_with_gain(channels: usize, rate: u32, gain: f32, data: &[f32]) -> (Vec<i16>, f32) {
    let level = Arc::new(AtomicU32::new(0));
    let gain = Arc::new(AtomicU32::new(gain.to_bits()));
    let mut got = Vec::new();
    let mut mixer = Mixer::new(channels, rate, level.clone(), gain, |s: &[i16]| {
        got.extend_from_slice(s)
    });
    mixer.push(data);
    drop(mixer);
    (got, f32::from_bits(level.load(Ordering::Relaxed)))
}

#[test]
fn resamples_to_16_khz() {
    let (out, _) = mix(1, 48_000, &[0.0; 4800]);
    assert_eq!(out.len(), 1600, "100 ms");
    let (out, _) = mix(1, 44_100, &[0.0; 4410]);
    assert!((1599..=1601).contains(&out.len()), "{}", out.len());
    let (out, _) = mix(1, 16_000, &[0.5; 160]);
    assert_eq!(out.len(), 160);
    assert!(
        out[1..].iter().all(|&s| s == 16383),
        "same rate keeps samples"
    );
}

#[test]
fn mixes_channels_and_measures_the_peak() {
    let (out, level) = mix(2, 16_000, &[0.5, -0.5, 0.25, 0.75, 1.0, 1.0]);
    assert_eq!(out.len(), 3);
    assert_eq!(out[1], 16383, "average of 0.25 and 0.75");
    assert_eq!(level, 1.0);
}

#[test]
fn gain_scales_samples_and_level() {
    let (out, level) = mix_with_gain(1, 16_000, 2.0, &[0.25; 16]);
    assert_eq!(out[1], 16383, "0.25 at 200 %");
    assert_eq!(level, 0.5, "the meter shows the louder signal");
    let (out, level) = mix_with_gain(2, 16_000, 0.5, &[0.5, 0.5, 0.5, 0.5]);
    assert_eq!(out[1], 8191, "50 % of the channels' average");
    assert_eq!(level, 0.25);
    let (out, level) = mix_with_gain(1, 16_000, 0.0, &[0.9; 16]);
    assert!(out.iter().all(|&s| s == 0), "0 % mutes");
    assert_eq!(level, 0.0);
}

#[test]
fn loud_gain_clips_instead_of_wrapping() {
    let (out, level) = mix_with_gain(1, 16_000, MAX_GAIN, &[0.5, -0.5, 0.5, -0.5]);
    assert_eq!(&out[1..], &[-i16::MAX, i16::MAX, -i16::MAX]);
    assert_eq!(level, 1.0, "clipping shows as full scale");
}

#[test]
fn gain_changes_while_open() {
    let level = Arc::new(AtomicU32::new(0));
    let gain = Arc::new(AtomicU32::new(1.0f32.to_bits()));
    let mut got = Vec::new();
    let mut mixer = Mixer::new(1, 16_000, level, gain.clone(), |s: &[i16]| {
        got.push(s[s.len() - 1])
    });
    mixer.push(&[0.25f32; 4]);
    gain.store(3.0f32.to_bits(), Ordering::Relaxed);
    mixer.push(&[0.25f32; 4]);
    drop(mixer);
    assert_eq!(got, [8191, 24575]);
}

#[test]
fn only_lasting_errors_close_the_stream() {
    assert!(fatal(cpal::ErrorKind::DeviceNotAvailable));
    assert!(fatal(cpal::ErrorKind::StreamInvalidated));
    assert!(!fatal(cpal::ErrorKind::Xrun));
    assert!(
        !fatal(cpal::ErrorKind::DeviceChanged),
        "rerouted, still running"
    );
}

#[test]
fn a_slow_start_is_not_a_stall() {
    let ms = |d: Duration| d.as_millis() as u64;
    assert!(!stalled(ms(STALL) * 2, 0), "first audio may take a while");
    assert!(stalled(ms(STARTUP) + 1, 0), "but not forever");
    assert!(!stalled(10_000, 9_000), "audio a second ago");
    assert!(
        stalled(10_000 + ms(STALL) + 1, 10_001),
        "silent longer than STALL"
    );
}

#[test]
fn gain_stays_in_range() {
    assert_eq!(clamp_gain(-1.0), 0.0);
    assert_eq!(clamp_gain(10.0), MAX_GAIN);
    assert_eq!(clamp_gain(f32::NAN), 1.0);
    assert_eq!(clamp_gain(1.5), 1.5);
}
