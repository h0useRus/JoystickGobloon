//! Damaged profiles never crash: import either fails with an error or gives a profile the engine
//! builds and runs. Profiles are files people share and edit by hand.

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::{Duration, Instant};

use gob_engine::{Engine, InputKey, InputKind, Output, Value};

struct Null;

impl Output for Null {
    fn axis(&mut self, _: u32, _: u8, _: f32) {}
    fn button(&mut self, _: u32, _: u8, _: bool) {}
    fn hat(&mut self, _: u32, _: u8, _: Option<u16>) {}
}

/// Deterministic pseudo-random numbers (no dependency, same cases on every run).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// One damaged copy: cut short, bytes changed, a piece removed or a piece repeated.
fn damage(text: &str, rng: &mut Rng) -> String {
    let mut b = text.as_bytes().to_vec();
    match rng.below(5) {
        0 => b.truncate(rng.below(b.len())),
        1 => {
            for _ in 0..1 + rng.below(8) {
                let i = rng.below(b.len());
                b[i] = b"<>\"=/ 0-9.ax\n"[rng.below(13)];
            }
        }
        2 => {
            let i = rng.below(b.len());
            let n = rng.below(200).min(b.len() - i);
            b.drain(i..i + n);
        }
        3 => {
            let i = rng.below(b.len());
            let n = rng.below(300).min(b.len() - i);
            let piece = b[i..i + n].to_vec();
            let at = rng.below(b.len());
            b.splice(at..at, piece);
        }
        _ => {
            // A number in an attribute replaced by an extreme one.
            let s = String::from_utf8_lossy(&b).into_owned();
            let numbers = ["-1", "0", "4294967296", "1e39", "NaN", "-0", "99999"];
            let mut out = s.clone();
            if let Some(i) = s.match_indices("=\"").map(|(i, _)| i).nth(rng.below(400)) {
                let end = s[i + 2..].find('"').map_or(s.len(), |e| i + 2 + e);
                out = format!("{}=\"{}{}", &s[..i], numbers[rng.below(numbers.len())], &s[end..]);
            }
            return out;
        }
    }
    String::from_utf8_lossy(&b).into_owned()
}

/// Runs a profile with a few events of every kind on the devices it binds.
fn run(profile: &gob_core::Profile) {
    let mut engine = Engine::new(profile, &HashMap::new());
    let mut out = Null;
    engine.activate(&mut out);
    let start = Instant::now();
    let devices: Vec<String> = profile.inputs.iter().map(|b| b.device_id.clone()).collect();
    let mut t = start;
    for (n, device) in devices.iter().take(8).enumerate() {
        for id in 1..=4u32 {
            t += Duration::from_millis(7);
            let key = |kind| InputKey::new(device, kind, id);
            engine.process(&key(InputKind::Axis), Value::Axis(n as f32 / 4.0 - 1.0), t, &mut out);
            engine.process(&key(InputKind::Button), Value::Button(true), t, &mut out);
            engine.process(&key(InputKind::Hat), Value::Hat(Some(9000)), t, &mut out);
            engine.tick(t + Duration::from_millis(600), &mut out);
            engine.process(&key(InputKind::Button), Value::Button(false), t, &mut out);
            engine.process(&key(InputKind::Hat), Value::Hat(None), t, &mut out);
        }
    }
    engine.tick(t + Duration::from_secs(5), &mut out);
    engine.release_all(&mut out);
}

fn profiles() -> Vec<(String, String)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    let mut found = Vec::new();
    for path in [dir.join("sample_r14.xml")].into_iter().chain(
        std::fs::read_dir(dir.join("local"))
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "xml")),
    ) {
        if let Ok(text) = std::fs::read_to_string(&path) {
            found.push((path.display().to_string(), text));
        }
    }
    found
}

#[test]
fn damaged_profiles_never_crash() {
    let cases = profiles();
    assert!(!cases.is_empty());
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut imported = 0;
    for (name, text) in &cases {
        for i in 0..300 {
            let damaged = damage(text, &mut rng);
            let result = catch_unwind(AssertUnwindSafe(|| {
                if let Ok(profile) = gob_gremlin::from_str(&damaged) {
                    run(&profile);
                    // What was imported saves and imports again.
                    let saved = gob_gremlin::to_string(&profile);
                    gob_gremlin::from_str(&saved).map(|_| ()).map_err(|e| e.to_string())
                } else {
                    Ok(())
                }
            }));
            match result {
                Ok(Ok(())) => imported += 1,
                Ok(Err(e)) => panic!("{name} case {i}: saved profile does not import: {e}"),
                Err(_) => {
                    let path = std::env::temp_dir().join(format!("damaged-{i}.xml"));
                    let _ = std::fs::write(&path, &damaged);
                    panic!("{name} case {i} crashed; input in {}", path.display());
                }
            }
        }
    }
    assert!(imported > 0);
}
