//! Settings: what the user can change, saved as JSON in the config directory
//! (~/.config/dictator/settings.json, %APPDATA%\dictator\settings.json).
//!
//! [`Settings`] is the saved form. [`Config`] is the engine's view of it, with
//! defaults filled in and env overrides (DICTATOR_DIR, DICTATOR_MODEL,
//! DICTATOR_LANG) applied, which keeps tests away from your real files.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const DEFAULT_MODEL: &str = "ggml-large-v3-turbo-q5_0.bin";

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HotkeyMode {
    /// Press once to start, again to stop.
    Toggle,
    /// Record while the key is held down.
    Hold,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OverlayPosition {
    Top,
    Bottom,
}

/// Built-in global hotkeys, in tauri-plugin-global-shortcut syntax
/// ("Ctrl+Alt+D"). Empty means unbound.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Hotkeys {
    pub record: String,
    pub power: String,
    pub copy: String,
    pub undo: String,
    pub overlay: String,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            record: "Ctrl+Alt+D".into(),
            power: "Ctrl+Alt+Shift+D".into(),
            copy: "Ctrl+Alt+C".into(),
            undo: "Ctrl+Alt+Z".into(),
            overlay: "Ctrl+Alt+O".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub hotkey_mode: HotkeyMode,
    pub hotkeys: Hotkeys,
    /// Short tones when recording starts and stops.
    pub chimes: bool,
    pub overlay_enabled: bool,
    pub overlay_position: OverlayPosition,
    /// Whisper language code ("en", "de", ...) or "auto".
    pub language: String,
    /// Turn off (unload the model) after this many idle minutes. 0 = never.
    pub auto_off_minutes: u32,
    pub start_on_login: bool,
    /// None = Documents/Dictator.
    pub draft_dir: Option<PathBuf>,
    /// None = the default model in the data directory.
    pub model: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey_mode: HotkeyMode::Toggle,
            hotkeys: Hotkeys::default(),
            chimes: true,
            overlay_enabled: true,
            overlay_position: OverlayPosition::Top,
            language: "en".into(),
            auto_off_minutes: 30,
            start_on_login: false,
            draft_dir: None,
            model: None,
        }
    }
}

impl Settings {
    pub fn path() -> PathBuf {
        dirs::config_dir().unwrap_or_default().join("dictator/settings.json")
    }

    /// Loads saved settings. Missing or unreadable files give the defaults,
    /// and missing fields are filled in, so old files keep working.
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_string_pretty(self).expect("settings serialize");
        // Write then rename, so a crash can't leave a half-written file.
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json)?;
        std::fs::rename(tmp, path)
    }
}

/// What the engine needs, resolved from [`Settings`].
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// Where draft.txt, archive.txt and vocab.txt live.
    pub draft_dir: PathBuf,
    pub model: PathBuf,
    pub language: String,
    pub auto_off: Option<Duration>,
}

impl Config {
    pub fn default_draft_dir() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_default();
        dirs::document_dir().unwrap_or_else(|| home.join("Documents")).join("Dictator")
    }

    pub fn default_model() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_default();
        dirs::data_dir()
            .unwrap_or_else(|| home.join(".local/share"))
            .join("dictator/models")
            .join(DEFAULT_MODEL)
    }

    pub fn from_settings(s: &Settings) -> Self {
        let env_path = |key: &str| std::env::var_os(key).map(PathBuf::from);
        Self {
            draft_dir: env_path("DICTATOR_DIR")
                .or_else(|| s.draft_dir.clone())
                .unwrap_or_else(Self::default_draft_dir),
            model: env_path("DICTATOR_MODEL")
                .or_else(|| s.model.clone())
                .unwrap_or_else(Self::default_model),
            language: std::env::var("DICTATOR_LANG").unwrap_or_else(|_| s.language.clone()),
            auto_off: (s.auto_off_minutes > 0)
                .then(|| Duration::from_secs(u64::from(s.auto_off_minutes) * 60)),
        }
    }

    /// Optional names/jargon to bias recognition: vocab.txt next to the draft.
    /// Read on every clip so edits apply immediately.
    pub fn vocab(&self) -> String {
        std::fs::read_to_string(self.draft_dir.join("vocab.txt"))
            .map(|s| s.trim().to_owned())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_get_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"chimes": false, "hotkeys": {"record": "F8"}}"#).unwrap();
        let s = Settings::load(&path);
        assert!(!s.chimes);
        assert_eq!(s.hotkeys.record, "F8");
        assert_eq!(s.hotkeys.copy, Hotkeys::default().copy);
        assert_eq!(s.language, "en");
    }

    #[test]
    fn save_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/settings.json");
        let s = Settings { hotkey_mode: HotkeyMode::Hold, auto_off_minutes: 5, ..Default::default() };
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path), s);
    }

    #[test]
    fn garbage_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "not json").unwrap();
        assert_eq!(Settings::load(&path), Settings::default());
    }
}
