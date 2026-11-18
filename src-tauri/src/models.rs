//! Speech models: which ones we offer, and downloading them with progress.
//!
//! Models are whisper.cpp's quantized ggml files from Hugging Face. Sizes and
//! SHA-256s are pinned here, so a download is checked against known-good
//! values, not against whatever the server says.

use crate::config::Config;
use anyhow::{Context, Result, bail};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;

pub struct Model {
    pub id: &'static str,
    pub file: &'static str,
    pub label: &'static str,
    pub note: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
}

const BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

/// Multilingual models only, so the language setting always works.
pub const MODELS: &[Model] = &[
    Model {
        id: "turbo",
        file: "ggml-large-v3-turbo-q5_0.bin",
        label: "Large v3 Turbo",
        note: "Best accuracy. Recommended with a dedicated GPU (about 1 GB of VRAM).",
        bytes: 574_041_195,
        sha256: "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2",
    },
    Model {
        id: "small",
        file: "ggml-small-q5_1.bin",
        label: "Small",
        note: "Good accuracy, lighter. For older or integrated GPUs.",
        bytes: 190_085_487,
        sha256: "ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb",
    },
    Model {
        id: "base",
        file: "ggml-base-q5_1.bin",
        label: "Base",
        note: "Fast but makes more mistakes. For laptops without a usable GPU.",
        bytes: 59_707_625,
        sha256: "422f1ae452ade6f30a004d7e5c6a43195e4433bc370bf23fac9cc591f01a8898",
    },
];

pub fn find(id: &str) -> Option<&'static Model> {
    MODELS.iter().find(|m| m.id == id)
}

pub fn dir() -> PathBuf {
    Config::default_model().parent().map(PathBuf::from).unwrap_or_default()
}

impl Model {
    pub fn path(&self) -> PathBuf {
        dir().join(self.file)
    }
}

/// Progress, sent to the UI as the `model-download` event.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub id: &'static str,
    pub done: u64,
    pub total: u64,
}

/// Downloads `model` into the models folder, calling `progress` a few times a
/// second. Writes to a `.part` file and only renames it once the size and
/// SHA-256 check out, so a cut-off download never looks like a real model.
/// Stops early (with an error) if `cancel` becomes true.
pub async fn download(
    model: &'static Model,
    cancel: &AtomicBool,
    progress: impl Fn(Progress),
) -> Result<PathBuf> {
    let dest = model.path();
    let part = dest.with_extension("bin.part");
    tokio::fs::create_dir_all(dir()).await.context("Couldn't create the models folder")?;

    let mut response = reqwest::get(format!("{BASE_URL}/{}", model.file))
        .await
        .context("Couldn't reach Hugging Face")?
        .error_for_status()
        .context("Download failed")?;
    let mut file = tokio::fs::File::create(&part).await.context("Couldn't write the model file")?;
    let mut hasher = Sha256::new();
    let mut done = 0u64;
    let mut last_report = Instant::now();

    let result: Result<()> = async {
        while let Some(chunk) = response.chunk().await.context("Download interrupted")? {
            if cancel.load(Ordering::Relaxed) {
                bail!("Download cancelled");
            }
            hasher.update(&chunk);
            file.write_all(&chunk).await.context("Couldn't write the model file")?;
            done += chunk.len() as u64;
            if last_report.elapsed() > Duration::from_millis(200) {
                last_report = Instant::now();
                progress(Progress { id: model.id, done, total: model.bytes });
            }
        }
        file.flush().await?;
        if done != model.bytes {
            bail!("Download incomplete ({done} of {} bytes)", model.bytes);
        }
        let hash = format!("{:x}", hasher.finalize());
        if hash != model.sha256 {
            bail!("Downloaded file is corrupt (checksum mismatch)");
        }
        Ok(())
    }
    .await;

    drop(file);
    if let Err(e) = result {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(e);
    }
    tokio::fs::rename(&part, &dest).await.context("Couldn't save the model file")?;
    progress(Progress { id: model.id, done, total: model.bytes });
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_model_is_offered() {
        let default = Config::default_model();
        let name = default.file_name().unwrap().to_str().unwrap();
        assert!(MODELS.iter().any(|m| m.file == name));
    }

    #[test]
    fn hashes_look_like_sha256() {
        for m in MODELS {
            assert_eq!(m.sha256.len(), 64, "{}", m.id);
            assert!(m.sha256.chars().all(|c| c.is_ascii_hexdigit()));
        }
    }

    /// Downloads the smallest model for real (60 MB): opt-in.
    ///   cargo test download_for_real -- --ignored --nocapture
    #[test]
    #[ignore]
    fn download_for_real() {
        let model = find("base").unwrap();
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let path = rt
            .block_on(download(model, &AtomicBool::new(false), |p| {
                println!("{:.0}%", p.done as f64 / p.total as f64 * 100.0)
            }))
            .unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().len(), model.bytes);
    }
}
