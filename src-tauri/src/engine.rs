//! The engine: owns the mic, the model and the draft, and reacts to commands.
//!
//! It runs on its own thread and is driven by messages, so hotkeys, the CLI and
//! the UI can all send commands without locking anything. Transcription runs on
//! a second worker thread so you can start the next clip while one is processing.
//! The engine knows nothing about Tauri; it reports back through [`Host`].

use crate::audio::{Recorder, SAMPLE_RATE};
use crate::config::Config;
use crate::draft::Draft;
use crate::stt::Transcriber;
use crate::text::{self, Outcome};
use anyhow::Result;
use serde::Serialize;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Model not loaded, mic untouchable.
    Off,
    Loading,
    Ready,
    Recording,
    Transcribing,
}

/// Everything the UI needs to draw itself.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub status: Status,
    /// The end of the draft (the overlay only shows the last few lines).
    pub text: String,
    pub words: usize,
    /// One-off message for this update ("Copied 120 words"), empty if none.
    pub msg: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    On,
    Off,
    /// On if off, off if on.
    Power,
    Start,
    Stop,
    /// Start if not recording, stop if recording.
    Toggle,
    /// Stop recording and throw the audio away.
    Cancel,
    Undo,
    /// Copy the draft to the clipboard and clear it.
    Copy,
}

impl Command {
    pub const NAMES: &str = "on, off, power, start, stop, toggle, cancel, undo, copy";

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "on" => Self::On,
            "off" => Self::Off,
            "power" => Self::Power,
            "start" => Self::Start,
            "stop" => Self::Stop,
            "toggle" => Self::Toggle,
            "cancel" => Self::Cancel,
            "undo" => Self::Undo,
            "copy" => Self::Copy,
            _ => return None,
        })
    }
}

/// How the engine talks to the outside world (the Tauri app, or a test).
pub trait Host: Send + 'static {
    fn update(&self, snapshot: &Snapshot);
    fn copy_to_clipboard(&self, text: &str) -> Result<()>;
}

#[derive(Clone)]
pub struct Engine {
    tx: Sender<Msg>,
}

impl Engine {
    pub fn spawn(config: Config, host: impl Host) -> Result<Self> {
        let draft = Draft::open(config.draft_dir.clone())?;
        let (tx, rx) = mpsc::channel();
        let jobs = spawn_worker(tx.clone());
        let actor_tx = tx.clone();
        thread::Builder::new().name("engine".into()).spawn(move || {
            Actor {
                config: Arc::new(config),
                host,
                draft,
                recorder: Recorder::new(),
                model: None,
                want_on: false,
                loading: false,
                pending: 0,
                msg: String::new(),
                tx: actor_tx,
                jobs,
            }
            .run(rx)
        })?;
        Ok(Self { tx })
    }

    pub fn send(&self, command: Command) {
        let _ = self.tx.send(Msg::Command(command));
    }
}

enum Msg {
    Command(Command),
    Loaded(Result<Transcriber>),
    Transcribed(Result<String>),
}

struct Job {
    model: Arc<Transcriber>,
    audio: Vec<f32>,
    config: Arc<Config>,
}

fn spawn_worker(results: Sender<Msg>) -> Sender<Job> {
    let (tx, rx) = mpsc::channel::<Job>();
    thread::Builder::new()
        .name("transcribe".into())
        .spawn(move || {
            for job in rx {
                let text = job.model.transcribe(&job.audio, &job.config.language, &job.config.vocab());
                // The job holds a reference to the model, so turning dictation off
                // mid-transcription frees it only once this clip is done.
                drop(job);
                if results.send(Msg::Transcribed(text)).is_err() {
                    break;
                }
            }
        })
        .expect("spawn transcription thread");
    tx
}

/// Clips shorter than this are almost always an accidental double press.
const MIN_CLIP_SECS: f32 = 0.3;
/// How much of the draft's end to send to the UI.
const TAIL_CHARS: usize = 2000;

struct Actor<H: Host> {
    config: Arc<Config>,
    host: H,
    draft: Draft,
    recorder: Recorder,
    model: Option<Arc<Transcriber>>,
    /// The user asked for "on". Stays true while the model is loading.
    want_on: bool,
    loading: bool,
    /// Clips sent to the worker and not back yet.
    pending: usize,
    msg: String,
    tx: Sender<Msg>,
    jobs: Sender<Job>,
}

impl<H: Host> Actor<H> {
    fn run(mut self, rx: Receiver<Msg>) {
        self.publish();
        for msg in rx {
            match msg {
                Msg::Command(c) => self.command(c),
                Msg::Loaded(result) => self.loaded(result),
                Msg::Transcribed(result) => self.transcribed(result),
            }
            self.publish();
        }
    }

    fn command(&mut self, command: Command) {
        match command {
            Command::On => self.turn_on(),
            Command::Off => self.turn_off(),
            Command::Power if self.want_on => self.turn_off(),
            Command::Power => self.turn_on(),
            Command::Start => self.start(),
            Command::Stop => self.stop(),
            Command::Toggle if self.recorder.is_recording() => self.stop(),
            Command::Toggle => self.start(),
            Command::Cancel => {
                self.recorder.stop();
            }
            Command::Undo => self.undo(),
            Command::Copy => self.copy(),
        }
    }

    fn turn_on(&mut self) {
        self.want_on = true;
        if self.model.is_some() || self.loading {
            return;
        }
        self.loading = true;
        let (tx, path) = (self.tx.clone(), self.config.model.clone());
        thread::spawn(move || {
            let _ = tx.send(Msg::Loaded(Transcriber::load(&path)));
        });
    }

    fn turn_off(&mut self) {
        self.want_on = false;
        self.recorder.stop(); // discard anything mid-recording
        self.model = None; // frees VRAM (once any in-flight clip finishes)
        self.msg = "Off. Mic closed, model unloaded".into();
    }

    fn loaded(&mut self, result: Result<Transcriber>) {
        self.loading = false;
        match result {
            // Turned off again while loading: just drop it.
            Ok(_) if !self.want_on => {}
            Ok(model) => {
                self.model = Some(Arc::new(model));
                self.msg = "Ready".into();
            }
            Err(e) => {
                self.want_on = false;
                self.msg = format!("{e:#}");
            }
        }
    }

    fn start(&mut self) {
        if self.model.is_none() {
            self.msg = if self.loading { "Still loading…" } else { "Dictator is off" }.into();
            return;
        }
        if let Err(e) = self.recorder.start() {
            self.msg = format!("{e:#}");
        }
    }

    fn stop(&mut self) {
        let Some(audio) = self.recorder.stop() else { return };
        let Some(model) = self.model.clone() else { return };
        if (audio.len() as f32) < MIN_CLIP_SECS * SAMPLE_RATE as f32 {
            self.msg = "Too short, ignored".into();
            return;
        }
        self.pending += 1;
        let _ = self.jobs.send(Job { model, audio, config: self.config.clone() });
    }

    fn transcribed(&mut self, result: Result<String>) {
        self.pending -= 1;
        match result.map(|raw| text::process(&raw)) {
            Ok(Outcome::Append(text)) => {
                if let Err(e) = self.draft.append(&text) {
                    self.msg = format!("Couldn't save: {e}");
                }
            }
            Ok(Outcome::Undo) => self.undo(),
            Ok(Outcome::Nothing) => self.msg = "Heard nothing".into(),
            Err(e) => self.msg = format!("Transcription failed: {e:#}"),
        }
    }

    fn undo(&mut self) {
        self.msg = match self.draft.undo() {
            Ok(true) => "Removed last chunk".into(),
            Ok(false) => "Nothing to undo".into(),
            Err(e) => format!("Undo failed: {e}"),
        };
    }

    fn copy(&mut self) {
        self.msg = match self.try_copy() {
            Ok(Some(n)) => format!("Copied {n} words, paste with Ctrl+V"),
            Ok(None) => "Nothing to copy".into(),
            Err(e) => format!("Copy failed: {e:#}"),
        };
    }

    /// Returns the number of words copied, or None if the draft was empty.
    fn try_copy(&mut self) -> Result<Option<usize>> {
        let text = self.draft.text()?;
        if text.trim().is_empty() {
            return Ok(None);
        }
        // Clipboard first: only clear the draft once the text is safely copied.
        self.host.copy_to_clipboard(&text)?;
        self.draft.take()?;
        Ok(Some(word_count(&text)))
    }

    fn status(&self) -> Status {
        if self.recorder.is_recording() {
            Status::Recording
        } else if self.pending > 0 {
            Status::Transcribing
        } else if self.loading && self.want_on {
            // (If turned off mid-load we already report Off; the load result
            // gets dropped when it arrives.)
            Status::Loading
        } else if self.model.is_some() {
            Status::Ready
        } else {
            Status::Off
        }
    }

    fn publish(&mut self) {
        let text = self.draft.text().unwrap_or_default();
        let tail_start = text
            .char_indices()
            .rev()
            .nth(TAIL_CHARS)
            .map_or(0, |(i, _)| i);
        self.host.update(&Snapshot {
            status: self.status(),
            words: word_count(&text),
            text: text[tail_start..].to_owned(),
            msg: std::mem::take(&mut self.msg),
        });
    }
}

fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}
