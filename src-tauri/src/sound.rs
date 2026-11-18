//! Start/stop chimes, synthesized on the fly (no sound files to ship).

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Chime {
    /// Two rising notes: "listening".
    Start,
    /// Two falling notes: "got it".
    Stop,
}

impl Chime {
    fn notes(self) -> [f32; 2] {
        match self {
            Chime::Start => [660.0, 880.0],
            Chime::Stop => [880.0, 660.0],
        }
    }
}

const NOTE_SECS: f32 = 0.07;
const VOLUME: f32 = 0.18;

/// Plays a chime without blocking. Errors (no speakers, etc.) are ignored:
/// a missing chime shouldn't break dictation.
pub fn play(chime: Chime) {
    std::thread::spawn(move || {
        if let Err(e) = play_blocking(chime) {
            eprintln!("chime: {e:#}");
        }
    });
}

fn play_blocking(chime: Chime) -> anyhow::Result<()> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or_else(|| anyhow::anyhow!("no output device"))?;
    let config = device.default_output_config()?;
    let rate = config.sample_rate() as f32;
    let channels = config.channels() as usize;
    let samples = tone(chime.notes(), rate);
    let len = samples.len();

    let stream = match config.sample_format() {
        SampleFormat::F32 => build::<f32>(&device, config.into(), channels, samples),
        SampleFormat::I16 => build::<i16>(&device, config.into(), channels, samples),
        SampleFormat::U16 => build::<u16>(&device, config.into(), channels, samples),
        SampleFormat::I32 => build::<i32>(&device, config.into(), channels, samples),
        other => anyhow::bail!("unsupported output format {other}"),
    }?;
    stream.play()?;
    // Let it play out, plus a little slack for the output buffer.
    std::thread::sleep(Duration::from_secs_f32(len as f32 / rate) + Duration::from_millis(80));
    Ok(())
}

fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    channels: usize,
    samples: Vec<f32>,
) -> anyhow::Result<cpal::Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let mut pos = 0;
    let stream = device.build_output_stream(
        config,
        move |out: &mut [T], _: &_| {
            for frame in out.chunks_mut(channels) {
                let s = samples.get(pos).copied().unwrap_or(0.0);
                pos += 1;
                frame.fill(T::from_sample(s));
            }
        },
        |err| eprintln!("chime stream error: {err}"),
        None,
    )?;
    Ok(stream)
}

/// Mono samples for a sequence of short sine notes with soft edges (no clicks).
fn tone(notes: [f32; 2], rate: f32) -> Vec<f32> {
    let n = (NOTE_SECS * rate) as usize;
    let fade = (0.008 * rate) as usize;
    notes
        .iter()
        .flat_map(|&freq| {
            (0..n).map(move |i| {
                let envelope = (i.min(n - 1 - i) as f32 / fade as f32).min(1.0);
                let t = i as f32 / rate;
                (t * freq * std::f32::consts::TAU).sin() * envelope * VOLUME
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tone_is_bounded_and_starts_and_ends_silent() {
        let t = tone([660.0, 880.0], 48_000.0);
        assert_eq!(t.len(), 2 * (NOTE_SECS * 48_000.0) as usize);
        assert!(t.iter().all(|s| s.abs() <= VOLUME));
        assert_eq!(t[0], 0.0);
        assert!(t.last().unwrap().abs() < 0.01);
    }
}
