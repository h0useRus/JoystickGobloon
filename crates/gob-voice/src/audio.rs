//! Microphones through cpal: the list of input devices and a capture stream that delivers 16 kHz
//! mono samples, whatever the device records. The stream exists only while it is held, so the
//! microphone is closed the rest of the time. A software gain (the input volume, like a voice chat
//! app's slider) scales the samples before the recognizer and the level meter see them.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};

use crate::Error;

/// The sample rate the recognizer gets.
pub const SAMPLE_RATE: u32 = 16_000;

/// The highest input gain (400 %): enough for a quiet microphone the OS does not boost.
pub const MAX_GAIN: f32 = 4.0;

/// No audio for this long from an open microphone: it is gone. A device that disappears (a
/// headset switched off or out of range) may just stop delivering, without an error.
pub const STALL: Duration = Duration::from_secs(2);

/// How long a freshly opened microphone may take to deliver its first audio (a Bluetooth headset
/// switching to its hands-free profile takes seconds) before it counts as stalled.
pub const STARTUP: Duration = Duration::from_secs(10);

/// An input device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Microphone {
    /// Stable across restarts where the OS allows; what settings and profiles store.
    pub id: String,
    pub name: String,
    pub default: bool,
}

/// The input devices of the default audio host, the default one first.
pub fn microphones() -> Vec<Microphone> {
    let host = cpal::default_host();
    let default = host.default_input_device().and_then(|d| d.id().ok());
    let Ok(devices) = host.input_devices() else {
        return Vec::new();
    };
    let mut list: Vec<Microphone> = devices
        .filter_map(|d| {
            let id = d.id().ok()?;
            Some(Microphone {
                default: default.as_ref() == Some(&id),
                id: id.to_string(),
                name: d.to_string(),
            })
        })
        .collect();
    list.sort_by_key(|m| !m.default);
    list
}

/// An open microphone. Dropping it closes the device.
pub struct Capture {
    _stream: cpal::Stream,
    level: Arc<AtomicU32>,
    gain: Arc<AtomicU32>,
    health: Arc<Health>,
    name: String,
}

/// Whether the stream still delivers, written on the audio thread.
struct Health {
    opened: Instant,
    /// When audio last came in, in milliseconds since `opened` plus one; 0 = not yet.
    last: AtomicU64,
    /// The stream reported an error it does not recover from.
    failed: AtomicBool,
}

impl Health {
    fn now(&self) -> u64 {
        u64::try_from(self.opened.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

impl Capture {
    /// Opens the microphone `id` (from [`microphones`]; `None` = the default one) with the input
    /// `gain` (1.0 = as recorded, see [`Capture::set_gain`]) and calls `sink` with 16 kHz mono
    /// samples on the audio thread; `sink` must not block.
    pub fn open(
        id: Option<&str>,
        gain: f32,
        sink: impl FnMut(&[i16]) + Send + 'static,
    ) -> Result<Self, Error> {
        let host = cpal::default_host();
        let device = match id {
            Some(id) => {
                let id = id.parse().map_err(|e| Error::Audio(format!("{e:?}")))?;
                host.device_by_id(&id)
            }
            None => host.default_input_device(),
        }
        .ok_or_else(|| Error::Audio("no such microphone".into()))?;
        let supported = device
            .default_input_config()
            .map_err(|e| Error::Audio(e.to_string()))?;
        let config = supported.config();
        let level = Arc::new(AtomicU32::new(0));
        let gain = Arc::new(AtomicU32::new(clamp_gain(gain).to_bits()));
        let mixer = Mixer::new(
            config.channels.into(),
            config.sample_rate,
            level.clone(),
            gain.clone(),
            sink,
        );
        let health = Arc::new(Health {
            opened: Instant::now(),
            last: AtomicU64::new(0),
            failed: AtomicBool::new(false),
        });
        let h = health.clone();
        let stream = match supported.sample_format() {
            SampleFormat::F32 => build::<f32>(&device, config, mixer, h),
            SampleFormat::I16 => build::<i16>(&device, config, mixer, h),
            SampleFormat::I32 => build::<i32>(&device, config, mixer, h),
            SampleFormat::U16 => build::<u16>(&device, config, mixer, h),
            SampleFormat::U8 => build::<u8>(&device, config, mixer, h),
            SampleFormat::F64 => build::<f64>(&device, config, mixer, h),
            other => return Err(Error::Audio(format!("sample format {other}"))),
        }?;
        stream.play().map_err(|e| Error::Audio(e.to_string()))?;
        log::info!(
            "microphone open: {device}, {} Hz, {} channels, {}",
            config.sample_rate,
            config.channels,
            supported.sample_format()
        );
        Ok(Self {
            _stream: stream,
            level,
            gain,
            health,
            name: device.to_string(),
        })
    }

    /// The microphone's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The microphone stopped working (disconnected, or the system switched it off): it gives no
    /// more audio and has to be opened again. Also true when the stream has delivered nothing for
    /// [`STALL`], or nothing at all within [`STARTUP`] of opening.
    pub fn lost(&self) -> bool {
        let h = &self.health;
        h.failed.load(Ordering::Relaxed) || stalled(h.now(), h.last.load(Ordering::Relaxed))
    }

    /// Peak of the last block after the gain, 0.0..=1.0 (1.0 = clipping).
    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }

    pub fn gain(&self) -> f32 {
        f32::from_bits(self.gain.load(Ordering::Relaxed))
    }

    /// Changes the input gain while the microphone is open: 0.0 = silent, 1.0 = as recorded, up
    /// to [`MAX_GAIN`]; louder samples are clipped.
    pub fn set_gain(&self, gain: f32) {
        self.gain
            .store(clamp_gain(gain).to_bits(), Ordering::Relaxed);
    }
}

/// No audio for too long: `now` and `last` as in [`Health`] (`last` 0 = none yet).
fn stalled(now: u64, last: u64) -> bool {
    let (since, limit) = match last {
        0 => (now, STARTUP),
        last => (now.saturating_sub(last - 1), STALL),
    };
    u128::from(since) > limit.as_millis()
}

fn clamp_gain(gain: f32) -> f32 {
    if gain.is_nan() {
        1.0
    } else {
        gain.clamp(0.0, MAX_GAIN)
    }
}

fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    mut mixer: Mixer<impl FnMut(&[i16]) + Send + 'static>,
    health: Arc<Health>,
) -> Result<cpal::Stream, Error>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let errors = health.clone();
    device
        .build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                health.last.store(health.now() + 1, Ordering::Relaxed);
                mixer.push(data);
            },
            move |e| {
                if fatal(e.kind()) {
                    errors.failed.store(true, Ordering::Relaxed);
                    log::warn!("microphone: {e} (closed)");
                } else {
                    log::debug!("microphone: {e}");
                }
            },
            None,
        )
        .map_err(|e| Error::Audio(e.to_string()))
}

/// A stream error after which no more audio comes: the device is gone (`DeviceNotAvailable`) or
/// the stream has to be built again (`StreamInvalidated`: WASAPI reports a change of the default
/// device this way for a stream on the default one). `DeviceChanged` means the stream was rerouted
/// and keeps running.
fn fatal(kind: cpal::ErrorKind) -> bool {
    use cpal::ErrorKind as K;
    !matches!(
        kind,
        K::Xrun | K::DeviceChanged | K::DeviceBusy | K::RealtimeDenied
    )
}

/// Mixes the channels down to mono and resamples to [`SAMPLE_RATE`] (linear: speech needs no
/// better, and recognition models take 16 kHz).
struct Mixer<F> {
    channels: usize,
    /// Input samples per output sample.
    step: f64,
    /// Position of the next output sample after `last` (0.0), in input samples.
    pos: f64,
    last: f32,
    out: Vec<i16>,
    level: Arc<AtomicU32>,
    gain: Arc<AtomicU32>,
    sink: F,
}

impl<F: FnMut(&[i16])> Mixer<F> {
    fn new(
        channels: usize,
        rate: u32,
        level: Arc<AtomicU32>,
        gain: Arc<AtomicU32>,
        sink: F,
    ) -> Self {
        Self {
            channels: channels.max(1),
            step: f64::from(rate) / f64::from(SAMPLE_RATE),
            pos: 1.0,
            last: 0.0,
            out: Vec::new(),
            level,
            gain,
            sink,
        }
    }

    fn push<T: Sample>(&mut self, data: &[T])
    where
        f32: FromSample<T>,
    {
        self.out.clear();
        let gain = f32::from_bits(self.gain.load(Ordering::Relaxed)) / self.channels as f32;
        let mut peak = 0.0f32;
        for frame in data.chunks_exact(self.channels) {
            let sum: f32 = frame.iter().map(|s| f32::from_sample(*s)).sum();
            let sample = (sum * gain).clamp(-1.0, 1.0);
            peak = peak.max(sample.abs());
            // Output samples that fall between the previous input sample and this one.
            while self.pos <= 1.0 {
                let v = self.last + (sample - self.last) * self.pos as f32;
                self.out.push((v * f32::from(i16::MAX)) as i16);
                self.pos += self.step;
            }
            self.pos -= 1.0;
            self.last = sample;
        }
        self.level.store(peak.to_bits(), Ordering::Relaxed);
        if !self.out.is_empty() {
            (self.sink)(&self.out);
        }
    }
}

#[cfg(test)]
mod tests;
