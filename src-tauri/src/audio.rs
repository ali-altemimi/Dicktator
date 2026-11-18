//! Microphone recording. The mic is only opened while recording: `start` opens
//! an input stream and `stop` closes it again and hands back the audio.

use anyhow::{Context, Result, anyhow};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};
use std::sync::{Arc, Mutex};

/// Whisper wants 16 kHz mono.
pub const SAMPLE_RATE: u32 = 16_000;

pub struct Recorder {
    active: Option<Active>,
}

struct Active {
    // Dropping the stream closes the mic. cpal streams aren't Send on every
    // platform, so a Recorder must stay on the thread that created it.
    _stream: cpal::Stream,
    samples: Arc<Mutex<Vec<f32>>>, // mono, at `rate`
    rate: u32,
}

impl Recorder {
    pub fn new() -> Self {
        Self { active: None }
    }

    pub fn is_recording(&self) -> bool {
        self.active.is_some()
    }

    pub fn start(&mut self) -> Result<()> {
        if self.active.is_some() {
            return Ok(());
        }
        let device = cpal::default_host()
            .default_input_device()
            .ok_or_else(|| anyhow!("No microphone found"))?;
        let config = device.default_input_config().context("Couldn't read mic settings")?;
        let rate = config.sample_rate();
        let channels = config.channels() as usize;
        let samples = Arc::new(Mutex::new(Vec::with_capacity(rate as usize * 30)));

        let stream = match config.sample_format() {
            SampleFormat::F32 => build::<f32>(&device, config.into(), channels, &samples),
            SampleFormat::I16 => build::<i16>(&device, config.into(), channels, &samples),
            SampleFormat::I32 => build::<i32>(&device, config.into(), channels, &samples),
            SampleFormat::U16 => build::<u16>(&device, config.into(), channels, &samples),
            other => return Err(anyhow!("Unsupported mic sample format {other}")),
        }?;
        stream.play().context("Couldn't start the mic")?;
        self.active = Some(Active { _stream: stream, samples, rate });
        Ok(())
    }

    /// Stops recording and returns the audio as 16 kHz mono, or None if we
    /// weren't recording.
    pub fn stop(&mut self) -> Option<Vec<f32>> {
        let active = self.active.take()?;
        drop(active._stream); // close the mic before doing anything else
        let samples = std::mem::take(&mut *active.samples.lock().unwrap());
        Some(resample(&samples, active.rate, SAMPLE_RATE))
    }
}

fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    channels: usize,
    out: &Arc<Mutex<Vec<f32>>>,
) -> Result<cpal::Stream>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let out = out.clone();
    let stream = device.build_input_stream(
        config,
        move |data: &[T], _: &_| {
            let mut out = out.lock().unwrap();
            // Downmix to mono by averaging the channels of each frame.
            out.extend(data.chunks(channels).map(|frame| {
                frame.iter().map(|&s| f32::from_sample(s)).sum::<f32>() / channels as f32
            }));
        },
        |err| eprintln!("mic stream error: {err}"),
        None,
    )?;
    Ok(stream)
}

/// Linear-interpolation resampler. Plenty for speech going into Whisper.
pub fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() {
        return input.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let len = (input.len() as f64 / ratio) as usize;
    (0..len)
        .map(|i| {
            let pos = i as f64 * ratio;
            let idx = pos as usize;
            let frac = (pos - idx as f64) as f32;
            let a = input[idx];
            let b = *input.get(idx + 1).unwrap_or(&a);
            a + (b - a) * frac
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_lengths_and_values() {
        let input: Vec<f32> = (0..48_000).map(|i| i as f32).collect();
        let out = resample(&input, 48_000, 16_000);
        assert_eq!(out.len(), 16_000);
        assert_eq!(out[1], 3.0); // every third sample
        assert_eq!(resample(&[1.0, 2.0], 16_000, 16_000), vec![1.0, 2.0]);
    }
}
