//! Microphones through cpal: the list of input devices and a capture stream that delivers 16 kHz
//! mono samples, whatever the device records. The stream exists only while it is held, so the
//! microphone is closed the rest of the time.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};

use crate::Error;

/// The sample rate the recognizer gets.
pub const SAMPLE_RATE: u32 = 16_000;

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
    name: String,
}

impl Capture {
    /// Opens the microphone `id` (from [`microphones`]; `None` = the default one) and calls `sink`
    /// with 16 kHz mono samples on the audio thread; `sink` must not block.
    pub fn open(
        id: Option<&str>,
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
        let mixer = Mixer::new(
            config.channels.into(),
            config.sample_rate,
            level.clone(),
            sink,
        );
        let stream = match supported.sample_format() {
            SampleFormat::F32 => build::<f32>(&device, config, mixer),
            SampleFormat::I16 => build::<i16>(&device, config, mixer),
            SampleFormat::I32 => build::<i32>(&device, config, mixer),
            SampleFormat::U16 => build::<u16>(&device, config, mixer),
            SampleFormat::U8 => build::<u8>(&device, config, mixer),
            SampleFormat::F64 => build::<f64>(&device, config, mixer),
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
            name: device.to_string(),
        })
    }

    /// The microphone's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Peak of the last block, 0.0..=1.0.
    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }
}

fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    mut mixer: Mixer<impl FnMut(&[i16]) + Send + 'static>,
) -> Result<cpal::Stream, Error>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    device
        .build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| mixer.push(data),
            |e| log::warn!("microphone: {e}"),
            None,
        )
        .map_err(|e| Error::Audio(e.to_string()))
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
    sink: F,
}

impl<F: FnMut(&[i16])> Mixer<F> {
    fn new(channels: usize, rate: u32, level: Arc<AtomicU32>, sink: F) -> Self {
        Self {
            channels: channels.max(1),
            step: f64::from(rate) / f64::from(SAMPLE_RATE),
            pos: 1.0,
            last: 0.0,
            out: Vec::new(),
            level,
            sink,
        }
    }

    fn push<T: Sample>(&mut self, data: &[T])
    where
        f32: FromSample<T>,
    {
        self.out.clear();
        let mut peak = 0.0f32;
        for frame in data.chunks_exact(self.channels) {
            let sum: f32 = frame.iter().map(|s| f32::from_sample(*s)).sum();
            let sample = sum / self.channels as f32;
            peak = peak.max(sample.abs());
            // Output samples that fall between the previous input sample and this one.
            while self.pos <= 1.0 {
                let v = self.last + (sample - self.last) * self.pos as f32;
                self.out
                    .push((v.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16);
                self.pos += self.step;
            }
            self.pos -= 1.0;
            self.last = sample;
        }
        self.level.store(peak.min(1.0).to_bits(), Ordering::Relaxed);
        if !self.out.is_empty() {
            (self.sink)(&self.out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mix(channels: usize, rate: u32, data: &[f32]) -> (Vec<i16>, f32) {
        let level = Arc::new(AtomicU32::new(0));
        let mut got = Vec::new();
        let mut mixer = Mixer::new(channels, rate, level.clone(), |s: &[i16]| {
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
}
