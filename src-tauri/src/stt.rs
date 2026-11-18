//! Speech-to-text with whisper.cpp (via whisper-rs). Runs on the GPU through
//! Vulkan (Windows/Linux) or Metal (macOS), falling back to the CPU.

use crate::audio::SAMPLE_RATE;
use anyhow::{Context, Result};
use std::path::Path;
use std::sync::Once;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Clips quieter than this (RMS) are treated as silence. Whisper tends to
/// invent text ("Thank you.") when fed near-silence, so don't even try.
const SILENCE_RMS: f32 = 0.004;
/// Segments Whisper itself thinks are probably not speech.
const NO_SPEECH_PROB: f32 = 0.6;

pub struct Transcriber {
    ctx: WhisperContext,
}

impl Transcriber {
    pub fn load(model: &Path) -> Result<Self> {
        static QUIET: Once = Once::new();
        // Route whisper.cpp's very chatty logging into the (unused) `log` facade.
        QUIET.call_once(whisper_rs::install_logging_hooks);

        check_gpu_runtime()?;
        let path = model.to_str().context("Model path isn't valid UTF-8")?;
        let ctx = WhisperContext::new_with_params(path, WhisperContextParameters::default())
            .with_context(|| format!("Couldn't load model {}", model.display()))?;
        let this = Self { ctx };
        // Warm up with a throwaway clip. The first run on a machine compiles the
        // GPU shaders (~20 s on Vulkan, cached by the driver afterwards); do it
        // now, while the UI says "Starting up", not on the user's first clip.
        this.run(&vec![0.0; SAMPLE_RATE as usize * 11 / 10], "en", "")?;
        Ok(this)
    }

    /// Transcribes 16 kHz mono audio. `prompt` biases spelling of names/jargon.
    pub fn transcribe(&self, audio: &[f32], language: &str, prompt: &str) -> Result<String> {
        if rms(audio) < SILENCE_RMS {
            return Ok(String::new());
        }
        self.run(audio, language, prompt)
    }

    fn run(&self, audio: &[f32], language: &str, prompt: &str) -> Result<String> {
        // whisper.cpp refuses clips under a second; pad short ones with silence.
        let min = SAMPLE_RATE as usize + SAMPLE_RATE as usize / 10;
        let mut padded;
        let audio = if audio.len() < min {
            padded = audio.to_vec();
            padded.resize(min, 0.0);
            &padded[..]
        } else {
            audio
        };

        let mut params = FullParams::new(SamplingStrategy::BeamSearch { beam_size: 5, patience: -1.0 });
        params.set_language(Some(language));
        params.set_n_threads(4);
        params.set_no_context(true); // each clip stands alone
        params.set_suppress_blank(true);
        params.set_no_speech_thold(NO_SPEECH_PROB);
        if !prompt.is_empty() {
            params.set_initial_prompt(prompt);
        }
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        let mut state = self.ctx.create_state()?;
        state.full(params, audio)?;
        let text = state
            .as_iter()
            .filter(|seg| seg.no_speech_probability() < NO_SPEECH_PROB)
            .filter_map(|seg| seg.to_str_lossy().ok().map(|s| s.trim().to_owned()))
            .collect::<Vec<_>>()
            .join(" ");
        Ok(text)
    }
}

/// Windows: vulkan-1.dll is delay-loaded (see build.rs), so check it exists
/// before whisper.cpp first calls into it. If it's missing, the delay-load
/// would crash the app instead of failing cleanly.
#[cfg(windows)]
fn check_gpu_runtime() -> Result<()> {
    use windows::Win32::Foundation::FreeLibrary;
    use windows::Win32::System::LibraryLoader::LoadLibraryW;
    match unsafe { LoadLibraryW(windows::core::w!("vulkan-1.dll")) } {
        Ok(module) => {
            let _ = unsafe { FreeLibrary(module) };
            Ok(())
        }
        Err(_) => anyhow::bail!(
            "Your graphics driver doesn't support Vulkan, which Dictator needs. \
             Update your GPU driver (NVIDIA, AMD or Intel) and try again"
        ),
    }
}

#[cfg(not(windows))]
fn check_gpu_runtime() -> Result<()> {
    Ok(())
}

fn rms(audio: &[f32]) -> f32 {
    if audio.is_empty() {
        return 0.0;
    }
    (audio.iter().map(|s| s * s).sum::<f32>() / audio.len() as f32).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs the real model and a 16 kHz mono f32 clip, so it's opt-in:
    ///   espeak-ng -w /tmp/s.wav "some text"
    ///   ffmpeg -i /tmp/s.wav -f f32le -ac 1 -ar 16000 /tmp/s.f32
    ///   DICTATOR_TEST_CLIP=/tmp/s.f32 cargo test -- --ignored --nocapture
    #[test]
    #[ignore]
    fn transcribes_a_real_clip() {
        let clip = std::env::var("DICTATOR_TEST_CLIP").expect("set DICTATOR_TEST_CLIP");
        let bytes = std::fs::read(clip).unwrap();
        let audio: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
            .collect();

        let t = std::time::Instant::now();
        let model = Transcriber::load(&crate::config::Config::from_settings(&Default::default()).model).unwrap();
        println!("load: {:.2?}", t.elapsed());
        for _ in 0..2 {
            let t = std::time::Instant::now();
            let text = model.transcribe(&audio, "en", "").unwrap();
            println!("transcribe: {:.2?} -> {text:?}", t.elapsed());
            assert!(text.to_lowercase().contains("memory"));
        }
        assert_eq!(model.transcribe(&vec![0.0; 32_000], "en", "").unwrap(), "");
    }
}
