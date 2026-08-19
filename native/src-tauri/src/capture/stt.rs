use std::path::{Path, PathBuf};
#[cfg(feature = "whisper-local")]
use std::sync::{Arc, Mutex};
use thiserror::Error;

#[cfg(feature = "whisper-local")]
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

#[derive(Error, Debug)]
pub enum SttError {
    #[error("Set Whisper model path in Provider Settings.")]
    ModelNotConfigured,

    #[error("Failed to load Whisper model at {path}: {message}")]
    ModelLoadFailed { path: String, message: String },

    #[error("Whisper transcription failed: {0}")]
    TranscriptionFailed(String),
}

/// A small English model good enough to prove dictation works end to end
/// without the user having to go find and download one themselves — they
/// can still point Settings at a bigger/multilingual model any time.
const DEFAULT_MODEL_FILENAME: &str = "ggml-tiny.en.bin";
const DEFAULT_MODEL_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin";

/// Serializes downloads: `start_capture` fires one of these in the
/// background to get a head start, and the transcription step fires
/// another right after — without this, both could race to write the same
/// temp file at once instead of the second simply finding the first's
/// finished download already in place.
static DOWNLOAD_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// If no Whisper model is configured, fetches a small default one into
/// `models_dir` and returns its path — so "set a GGML model path" isn't a
/// prerequisite the user has to satisfy by hand before dictation works at
/// all. A no-op (just returns the existing path) once it's already there.
pub async fn ensure_default_model(models_dir: &Path) -> Result<PathBuf, SttError> {
    let target = models_dir.join(DEFAULT_MODEL_FILENAME);
    if target.exists() {
        return Ok(target);
    }

    let _guard = DOWNLOAD_LOCK.lock().await;
    if target.exists() {
        return Ok(target);
    }

    std::fs::create_dir_all(models_dir).map_err(|e| SttError::ModelLoadFailed {
        path: target.display().to_string(),
        message: e.to_string(),
    })?;

    tracing::info!(
        "No Whisper model configured — downloading the default one to {}",
        target.display()
    );

    let response = reqwest::get(DEFAULT_MODEL_URL)
        .await
        .map_err(|e| SttError::ModelLoadFailed {
            path: DEFAULT_MODEL_URL.to_string(),
            message: e.to_string(),
        })?;

    if !response.status().is_success() {
        return Err(SttError::ModelLoadFailed {
            path: DEFAULT_MODEL_URL.to_string(),
            message: format!("HTTP {}", response.status()),
        });
    }

    let bytes = response.bytes().await.map_err(|e| SttError::ModelLoadFailed {
        path: DEFAULT_MODEL_URL.to_string(),
        message: e.to_string(),
    })?;

    // Download to a temp file and rename into place, so a crash or a
    // second concurrent call never leaves (or reads) a half-written model.
    let tmp_path = target.with_extension("bin.part");
    std::fs::write(&tmp_path, &bytes).map_err(|e| SttError::ModelLoadFailed {
        path: tmp_path.display().to_string(),
        message: e.to_string(),
    })?;
    std::fs::rename(&tmp_path, &target).map_err(|e| SttError::ModelLoadFailed {
        path: target.display().to_string(),
        message: e.to_string(),
    })?;

    tracing::info!("Default Whisper model ready at {}", target.display());
    Ok(target)
}

/// Local, zero-cost speech-to-text via whisper.cpp (through whisper-rs).
#[derive(Clone)]
pub struct SttEngine {
    #[cfg(feature = "whisper-local")]
    loaded: Arc<Mutex<Option<(String, WhisperContext)>>>,
}

impl Default for SttEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SttEngine {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "whisper-local")]
            loaded: Arc::new(Mutex::new(None)),
        }
    }

    /// Transcribe 16kHz mono f32 PCM samples using the model at `model_path`.
    pub fn transcribe(
        &self,
        model_path: Option<&str>,
        samples_16k_mono: &[f32],
    ) -> Result<String, SttError> {
        #[cfg(not(feature = "whisper-local"))]
        {
            let _ = (model_path, samples_16k_mono);
            Err(SttError::ModelNotConfigured)
        }

        #[cfg(feature = "whisper-local")]
        {
            let model_path = model_path.ok_or(SttError::ModelNotConfigured)?;
            if model_path.trim().is_empty() {
                return Err(SttError::ModelNotConfigured);
            }
            if samples_16k_mono.is_empty() {
                return Ok(String::new());
            }

            let mut guard = self.loaded.lock().unwrap();
            let needs_reload = match guard.as_ref() {
                Some((loaded_path, _)) => loaded_path != model_path,
                None => true,
            };

            if needs_reload {
                tracing::info!("Loading Whisper model from {}", model_path);
                let ctx =
                    WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
                        .map_err(|e| SttError::ModelLoadFailed {
                            path: model_path.to_string(),
                            message: e.to_string(),
                        })?;
                *guard = Some((model_path.to_string(), ctx));
            }

            let (_, ctx) = guard
                .as_ref()
                .expect("model was just loaded or already present");
            let mut state = ctx
                .create_state()
                .map_err(|e| SttError::TranscriptionFailed(e.to_string()))?;

            let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            params.set_language(Some("en"));
            params.set_translate(false);
            params.set_print_special(false);
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);
            params.set_suppress_blank(true);
            params.set_n_threads(num_cpus());

            state
                .full(params, samples_16k_mono)
                .map_err(|e| SttError::TranscriptionFailed(e.to_string()))?;

            let num_segments = state
                .full_n_segments()
                .map_err(|e| SttError::TranscriptionFailed(e.to_string()))?;

            let mut text = String::new();
            for i in 0..num_segments {
                let segment = state
                    .full_get_segment_text(i)
                    .map_err(|e| SttError::TranscriptionFailed(e.to_string()))?;
                text.push_str(&segment);
            }

            Ok(text.trim().to_string())
        }
    }
}

#[cfg(feature = "whisper-local")]
fn num_cpus() -> std::ffi::c_int {
    std::thread::available_parallelism()
        .map(|n| n.get() as std::ffi::c_int)
        .unwrap_or(4)
        .clamp(1, 8)
}
