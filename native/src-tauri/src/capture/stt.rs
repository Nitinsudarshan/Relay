#[cfg(feature = "whisper-local")]
use std::sync::{Arc, Mutex};
use thiserror::Error;

#[cfg(feature = "whisper-local")]
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

#[derive(Error, Debug)]
pub enum SttError {
    #[error("No local Whisper model configured. Set a GGML model path (e.g. ggml-base.en.bin) in Provider Settings.")]
    ModelNotConfigured,

    #[error("Failed to load Whisper model at {path}: {message}")]
    ModelLoadFailed { path: String, message: String },

    #[error("Whisper transcription failed: {0}")]
    TranscriptionFailed(String),
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
