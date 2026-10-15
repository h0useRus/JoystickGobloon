//! Cursor motion from map-to-mouse actions (Gremlin's MouseMotionManager):
//! every contribution adds a velocity, summed and integrated every 10 ms with
//! the sub-pixel remainder carried over.

use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};

/// Motion update interval (Gremlin's default 100 Hz).
pub const STEP: Duration = Duration::from_millis(10);
/// At most this many steps are integrated after a stall, so the cursor never jumps.
const MAX_STEPS_BEHIND: u32 = 5;

/// Unit vector for a compass heading (0 = up, 90 = right), y growing downwards.
pub fn compass(degrees: f32) -> (f32, f32) {
    let rad = (degrees - 90.0).to_radians();
    (rad.cos(), rad.sin())
}

#[derive(Debug, Clone)]
enum Velocity {
    Constant((f32, f32)),
    /// Speeds up from min to max while held.
    Accelerating {
        direction: (f32, f32),
        speed: f32,
        max: f32,
        acceleration: f32,
    },
}

#[derive(Debug, Clone)]
struct Source<T> {
    velocity: Velocity,
    tag: T,
}

/// Motion contributions keyed by `K`, each carrying a tag `T` (e.g. the mode it started in).
#[derive(Debug)]
pub struct Motion<K, T> {
    sources: HashMap<K, Source<T>>,
    residual: (f32, f32),
    last: Option<Instant>,
}

impl<K: Eq + Hash + Clone, T: Clone> Default for Motion<K, T> {
    fn default() -> Self {
        Self {
            sources: HashMap::new(),
            residual: (0.0, 0.0),
            last: None,
        }
    }
}

impl<K: Eq + Hash + Clone, T: Clone> Motion<K, T> {
    /// Constant velocity in px/s; zero removes the contribution.
    pub fn set_velocity(&mut self, key: K, velocity: (f32, f32), tag: T, now: Instant) {
        if velocity.0.hypot(velocity.1) < 1e-6 {
            self.clear(&key);
            return;
        }
        self.start(now);
        self.sources.insert(
            key,
            Source {
                velocity: Velocity::Constant(velocity),
                tag,
            },
        );
    }

    /// Accelerating contribution; an existing one keeps its speed and only turns.
    #[allow(clippy::too_many_arguments)]
    pub fn set_accelerated(
        &mut self,
        key: K,
        direction: (f32, f32),
        min: f32,
        max: f32,
        time_to_max: f32,
        tag: T,
        now: Instant,
    ) {
        if let Some(Source {
            velocity: Velocity::Accelerating { direction: d, .. },
            ..
        }) = self.sources.get_mut(&key)
        {
            *d = direction;
            return;
        }
        let (min, max) = if max < min { (max, min) } else { (min, max) };
        let acceleration = if time_to_max < 0.01 {
            1e6
        } else {
            (max - min) / time_to_max
        };
        self.start(now);
        self.sources.insert(
            key,
            Source {
                velocity: Velocity::Accelerating {
                    direction,
                    speed: min,
                    max,
                    acceleration,
                },
                tag,
            },
        );
    }

    pub fn clear(&mut self, key: &K) {
        self.sources.remove(key);
        if self.sources.is_empty() {
            self.last = None;
            self.residual = (0.0, 0.0);
        }
    }

    /// Drops contributions for which `keep` returns false.
    pub fn retain(&mut self, mut keep: impl FnMut(&K, &T) -> bool) {
        let keys: Vec<K> = self
            .sources
            .iter()
            .filter(|(k, s)| !keep(k, &s.tag))
            .map(|(k, _)| k.clone())
            .collect();
        for k in keys {
            self.clear(&k);
        }
    }

    pub fn next_deadline(&self) -> Option<Instant> {
        self.last.map(|t| t + STEP)
    }

    /// Whole-pixel cursor movement since the last step, or `None` when not due yet.
    pub fn step(&mut self, now: Instant) -> Option<(i32, i32)> {
        let last = self.last?;
        let elapsed = now.saturating_duration_since(last);
        if elapsed < STEP {
            return None;
        }
        let dt = elapsed.min(STEP * MAX_STEPS_BEHIND).as_secs_f32();
        let mut total = (0.0, 0.0);
        for s in self.sources.values_mut() {
            let v = match &mut s.velocity {
                Velocity::Constant(v) => *v,
                Velocity::Accelerating {
                    direction,
                    speed,
                    max,
                    acceleration,
                } => {
                    let v = (direction.0 * *speed, direction.1 * *speed);
                    *speed = (*speed + *acceleration * dt).min(*max);
                    v
                }
            };
            total.0 += v.0;
            total.1 += v.1;
        }
        self.last = Some(now);
        self.residual.0 += total.0 * dt;
        self.residual.1 += total.1 * dt;
        let px = (self.residual.0.trunc(), self.residual.1.trunc());
        self.residual.0 -= px.0;
        self.residual.1 -= px.1;
        Some((px.0 as i32, px.1 as i32))
    }

    fn start(&mut self, now: Instant) {
        if self.sources.is_empty() {
            self.last = Some(now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compass_directions() {
        let up = compass(0.0);
        let right = compass(90.0);
        assert!(up.0.abs() < 1e-6 && (up.1 + 1.0).abs() < 1e-6);
        assert!((right.0 - 1.0).abs() < 1e-6 && right.1.abs() < 1e-6);
    }

    #[test]
    fn constant_and_accelerating_motion() {
        let t0 = Instant::now();
        let mut m: Motion<u8, ()> = Motion::default();
        m.set_velocity(1, (100.0, 0.0), (), t0);
        assert_eq!(m.step(t0 + Duration::from_millis(5)), None, "not due yet");
        assert_eq!(m.step(t0 + STEP), Some((1, 0)));
        assert_eq!(
            m.step(t0 + Duration::from_secs(10)),
            Some((5, 0)),
            "stalls integrate at most 5 steps"
        );

        let mut a: Motion<u8, ()> = Motion::default();
        a.set_accelerated(1, (0.0, 1.0), 100.0, 300.0, 1.0, (), t0);
        let mut moved = 0;
        for i in 1..=200 {
            moved += a.step(t0 + STEP * i).unwrap().1;
        }
        // 2 s: ramps 100 -> 300 px/s over the first second (~200 px), then 300 px.
        assert!((490..=505).contains(&moved), "{moved}");
    }
}
