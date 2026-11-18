//! Settings. Hard-coded defaults for now (with env overrides for testing);
//! the settings window will persist these later.

use std::path::PathBuf;

pub struct Config {
    /// Where draft.txt, archive.txt and vocab.txt live.
    pub draft_dir: PathBuf,
    pub model: PathBuf,
    pub language: String,
}

pub const DEFAULT_MODEL: &str = "ggml-large-v3-turbo-q5_0.bin";

impl Config {
    pub fn load() -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        let docs = dirs::document_dir().unwrap_or_else(|| home.join("Documents"));
        let data = dirs::data_dir().unwrap_or_else(|| home.join(".local/share"));
        let env_path = |key: &str| std::env::var_os(key).map(PathBuf::from);
        Self {
            draft_dir: env_path("DICTATOR_DIR").unwrap_or_else(|| docs.join("Dictator")),
            model: env_path("DICTATOR_MODEL")
                .unwrap_or_else(|| data.join("dictator/models").join(DEFAULT_MODEL)),
            language: std::env::var("DICTATOR_LANG").unwrap_or_else(|_| "en".into()),
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
