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

/// Production default multilingual Whisper model (ggml-small.bin, 244M params).
/// Provides reliable English/Hindi code-switching, robust language separation,
/// and low zero-cost latency on local CPU.
pub const DEFAULT_MODEL_FILENAME: &str = "ggml-small.bin";
pub const DEFAULT_MODEL_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin";

/// High-speed multilingual Whisper model for fast universal dictation (ggml-base.bin, 39M params).
/// Provides ~3x lower latency (~0.8s vs ~2.4s) on CPU for conversational dictation.
pub const FAST_MODEL_FILENAME: &str = "ggml-base.bin";
pub const FAST_MODEL_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin";

/// Checks if a configured model path represents a legacy default model
/// (e.g. `ggml-tiny.en.bin`) so Relay can seamlessly promote to `ggml-small.bin`.
pub fn is_legacy_default_model(path: &Path) -> bool {
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        name == "ggml-tiny.en.bin"
    } else {
        false
    }
}

/// Serializes downloads: `start_capture` fires one of these in the
/// background to get a head start, and the transcription step fires
/// another right after — without this, both could race to write the same
/// temp file at once instead of the second simply finding the first's
/// finished download already in place.
static DOWNLOAD_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Fetches a model from `url` into `models_dir` if it does not already exist.
async fn ensure_model_file(models_dir: &Path, filename: &str, url: &str) -> Result<PathBuf, SttError> {
    let target = models_dir.join(filename);
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
        "Downloading Whisper model {} to {}",
        filename,
        target.display()
    );

    let response = reqwest::get(url)
        .await
        .map_err(|e| SttError::ModelLoadFailed {
            path: url.to_string(),
            message: format!("Failed to download model: {}", e),
        })?;

    if !response.status().is_success() {
        return Err(SttError::ModelLoadFailed {
            path: url.to_string(),
            message: format!("HTTP {} from model server", response.status()),
        });
    }

    let bytes = response.bytes().await.map_err(|e| SttError::ModelLoadFailed {
        path: url.to_string(),
        message: format!("Failed to read response body: {}", e),
    })?;

    let tmp_path = target.with_extension("bin.part");
    std::fs::write(&tmp_path, &bytes).map_err(|e| SttError::ModelLoadFailed {
        path: tmp_path.display().to_string(),
        message: e.to_string(),
    })?;
    std::fs::rename(&tmp_path, &target).map_err(|e| SttError::ModelLoadFailed {
        path: target.display().to_string(),
        message: e.to_string(),
    })?;

    tracing::info!("Whisper model ready at {}", target.display());
    Ok(target)
}

/// If no Whisper model is configured, fetches the production small default model into `models_dir` and returns its path.
pub async fn ensure_default_model(models_dir: &Path) -> Result<PathBuf, SttError> {
    ensure_model_file(models_dir, DEFAULT_MODEL_FILENAME, DEFAULT_MODEL_URL).await
}

/// Ensures the fast base model is available for Universal Dictation Fast profile.
pub async fn ensure_fast_model(models_dir: &Path) -> Result<PathBuf, SttError> {
    ensure_model_file(models_dir, FAST_MODEL_FILENAME, FAST_MODEL_URL).await
}

/// Resolves the effective model path for Universal Dictation based on the user's Dictation quality preference.
/// In `Fast` mode, uses `ggml-base.bin` (~0.8s latency); in `Accurate` mode, uses `ggml-small.bin` (~2.4s latency).
pub async fn resolve_dictation_model_path(
    models_dir: &Path,
    stt_settings: &crate::settings::SttSettings,
) -> Option<String> {
    match stt_settings.dictation_quality {
        crate::settings::DictationSttQuality::Fast => {
            let fast_path = models_dir.join(FAST_MODEL_FILENAME);
            if fast_path.exists() {
                Some(fast_path.to_string_lossy().to_string())
            } else {
                match ensure_fast_model(models_dir).await {
                    Ok(p) => Some(p.to_string_lossy().to_string()),
                    Err(e) => {
                        tracing::warn!("Could not ensure fast Whisper model ({}): Falling back to default", e);
                        let default_path = models_dir.join(DEFAULT_MODEL_FILENAME);
                        if default_path.exists() {
                            Some(default_path.to_string_lossy().to_string())
                        } else {
                            stt_settings.whisper_model_path.clone()
                        }
                    }
                }
            }
        }
        crate::settings::DictationSttQuality::Accurate => {
            if let Some(ref path) = stt_settings.whisper_model_path {
                let p = Path::new(path);
                if p.exists() {
                    Some(path.clone())
                } else {
                    let default_path = models_dir.join(DEFAULT_MODEL_FILENAME);
                    if default_path.exists() {
                        Some(default_path.to_string_lossy().to_string())
                    } else {
                        Some(path.clone())
                    }
                }
            } else {
                let default_path = models_dir.join(DEFAULT_MODEL_FILENAME);
                if default_path.exists() {
                    Some(default_path.to_string_lossy().to_string())
                } else {
                    match ensure_default_model(models_dir).await {
                        Ok(p) => Some(p.to_string_lossy().to_string()),
                        Err(_) => None,
                    }
                }
            }
        }
    }
}

use crate::settings::LanguageSettings;

/// Resolved speech-to-text language configuration passed to the STT engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SttLanguageConfig {
    /// Language code passed to Whisper (`Some("en")`, `Some("hi")`, or `None` for auto-detect).
    pub whisper_language: Option<String>,
    /// Whether translation is enabled (`false` for dictation to preserve verbatim speech).
    pub translate: bool,
}

impl SttLanguageConfig {
    /// Resolves the optimal Whisper STT language configuration from user preferences.
    ///
    /// Rules:
    /// 1. `translate` is always `false` to preserve original spoken words (e.g. never translating Hindi to English).
    /// 2. If `primary_dictation_language` is "auto" or empty -> `None` (Whisper auto-detect).
    /// 3. If user has multiple spoken languages (e.g. ["en", "hi"]) -> `None` (auto-detect) so Whisper is NOT
    ///    hard-locked to a single language. This allows mixed-language / code-switching recognition without forcing
    ///    non-English words through an English-only acoustic filter or causing hallucinations.
    /// 4. If user has exactly one spoken language (e.g. ["en"] or ["hi"]) matching primary -> `Some(lang)` to
    ///    eliminate detection latency and avoid short-audio misclassification.
    /// 5. `output_script` ("latin" vs "native") is purely orthography and does NOT alter STT language selection.
    pub fn from_settings(settings: &LanguageSettings) -> Self {
        let primary = settings.primary_dictation_language.trim().to_lowercase();
        let spoken: Vec<String> = settings
            .spoken_languages
            .iter()
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        // Deduplicate spoken languages while preserving ordering
        let mut unique_spoken = Vec::new();
        for s in spoken {
            if !unique_spoken.contains(&s) {
                unique_spoken.push(s);
            }
        }

        let whisper_language = if primary == "auto" || primary.is_empty() {
            None
        } else if unique_spoken.len() > 1 {
            // Bilingual / multilingual user profile (e.g. English + Hindi, or Hinglish).
            // Do not hard-lock Whisper to a single language or pass non-ISO tokens.
            None
        } else if unique_spoken.len() == 1 && unique_spoken[0] == primary {
            // Unambiguous single-language profile.
            Some(primary)
        } else if unique_spoken.is_empty() {
            Some(primary)
        } else {
            None
        };

        Self {
            whisper_language,
            translate: false,
        }
    }
}

use serde::{Deserialize, Serialize};

/// Sampling strategies supported by Whisper decoding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SttSamplingStrategy {
    Greedy { best_of: i32 },
    BeamSearch { beam_size: i32, patience: f32 },
}

/// Centralized Whisper decoding configuration for experiments and production.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WhisperDecodingConfig {
    pub strategy: SttSamplingStrategy,
    pub temperature: f32,
    pub temperature_inc: f32,
    pub initial_prompt: Option<String>,
    pub suppress_blank: bool,
    pub no_speech_thold: f32,
    pub entropy_thold: f32,
    pub logprob_thold: f32,
    pub print_special: bool,
    pub print_timestamps: bool,
    /// Worker threads for this decode. `None` uses every available core.
    /// Meetings pin the durable clock below the core count so the live
    /// clock is never starved of CPU by a 30-second chunk decode.
    pub n_threads: Option<i32>,
}

impl Default for WhisperDecodingConfig {
    fn default() -> Self {
        Self::baseline()
    }
}

impl WhisperDecodingConfig {
    /// Baseline configuration representing production behavior:
    /// - Greedy decoding with best_of = 1
    /// - Temperature = 0.0, temperature_inc = 0.2
    /// - No initial prompt
    /// - suppress_blank = true, print_special = false
    pub fn baseline() -> Self {
        Self {
            strategy: SttSamplingStrategy::Greedy { best_of: 1 },
            temperature: 0.0,
            temperature_inc: 0.2,
            initial_prompt: None,
            suppress_blank: true,
            no_speech_thold: 0.6,
            entropy_thold: 2.4,
            logprob_thold: -1.0,
            print_special: false,
            print_timestamps: false,
            n_threads: None,
        }
    }

    /// Configuration for Experiment B: Greedy with best_of = 3.
    pub fn experiment_best_of(best_of: i32) -> Self {
        let mut cfg = Self::baseline();
        cfg.strategy = SttSamplingStrategy::Greedy { best_of };
        cfg
    }

    /// Configuration for Experiment C: Temperature fallback with custom initial temperature and increment.
    pub fn experiment_temperature(initial_temp: f32, temp_inc: f32) -> Self {
        let mut cfg = Self::baseline();
        cfg.temperature = initial_temp;
        cfg.temperature_inc = temp_inc;
        cfg
    }

    /// Configuration for Experiment D: Technical vocabulary initial prompt.
    pub fn experiment_prompt(prompt: &str) -> Self {
        let mut cfg = Self::baseline();
        cfg.initial_prompt = Some(prompt.to_string());
        cfg
    }

    /// Configuration for Experiment F: Tuned hallucination/no-speech thresholds.
    pub fn experiment_thresholds(no_speech_thold: f32, entropy_thold: f32, logprob_thold: f32) -> Self {
        let mut cfg = Self::baseline();
        cfg.no_speech_thold = no_speech_thold;
        cfg.entropy_thold = entropy_thold;
        cfg.logprob_thold = logprob_thold;
        cfg
    }

    /// Resolves the effective production decoding configuration from persisted user settings.
    /// If domain vocabulary prompt is disabled (default), returns the clean baseline configuration (initial_prompt = None).
    pub fn from_settings(stt_settings: &crate::settings::SttSettings) -> Self {
        let mut cfg = Self::baseline();
        if stt_settings.enable_initial_prompt {
            if let Some(ref prompt) = stt_settings.custom_initial_prompt {
                let trimmed = prompt.trim();
                if !trimmed.is_empty() {
                    cfg.initial_prompt = Some(trimmed.to_string());
                }
            }
        }
        cfg
    }

    /// Resolves the effective decoding configuration specifically for Universal Dictation.
    /// Uses user-configured thread override if provided, or clamps thread allocation between 1 and 12
    /// to saturate available physical/logical cores without exceeding logical core boundaries.
    pub fn for_dictation(stt_settings: &crate::settings::SttSettings) -> Self {
        let mut cfg = Self::from_settings(stt_settings);
        if let Some(threads) = stt_settings.dictation_threads {
            cfg.n_threads = Some(threads.clamp(1, 64));
        } else {
            let threads = std::thread::available_parallelism()
                .map(|n| n.get() as i32)
                .unwrap_or(4)
                .clamp(1, 12);
            cfg.n_threads = Some(threads);
        }
        cfg
    }
}

/// Diagnostic metadata recorded for an STT transcription session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SttSessionDiagnostics {
    pub model_path: String,
    pub audio_duration_seconds: f32,
    pub whisper_language: Option<String>,
    pub decoding_strategy: String,
    pub temperature: f32,
    pub temperature_inc: f32,
    pub best_of: i32,
    pub used_initial_prompt: bool,
    pub transcription_latency_ms: u128,
    pub real_time_factor: f32,
    pub segment_count: usize,
    pub is_empty: bool,
    pub transcript_char_count: usize,
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

    /// Transcribe using default baseline decoding configuration.
    pub fn transcribe(
        &self,
        model_path: Option<&str>,
        samples_16k_mono: &[f32],
        language_config: &SttLanguageConfig,
    ) -> Result<String, SttError> {
        let (text, _) = self.transcribe_with_config(
            model_path,
            samples_16k_mono,
            language_config,
            &WhisperDecodingConfig::default(),
        )?;
        Ok(text)
    }

    /// Transcribe with explicit Whisper decoding configuration and return diagnostic metrics.
    pub fn transcribe_with_config(
        &self,
        model_path: Option<&str>,
        samples_16k_mono: &[f32],
        language_config: &SttLanguageConfig,
        decoding_config: &WhisperDecodingConfig,
    ) -> Result<(String, SttSessionDiagnostics), SttError> {
        #[cfg(not(feature = "whisper-local"))]
        {
            let _ = (model_path, samples_16k_mono, language_config, decoding_config);
            Err(SttError::ModelNotConfigured)
        }

        #[cfg(feature = "whisper-local")]
        {
            let model_path = model_path.ok_or(SttError::ModelNotConfigured)?;
            if model_path.trim().is_empty() {
                return Err(SttError::ModelNotConfigured);
            }
            let audio_dur = samples_16k_mono.len() as f32 / 16000.0;
            if samples_16k_mono.is_empty() {
                let diag = SttSessionDiagnostics {
                    model_path: model_path.to_string(),
                    audio_duration_seconds: 0.0,
                    whisper_language: language_config.whisper_language.clone(),
                    decoding_strategy: format!("{:?}", decoding_config.strategy),
                    temperature: decoding_config.temperature,
                    temperature_inc: decoding_config.temperature_inc,
                    best_of: match decoding_config.strategy {
                        SttSamplingStrategy::Greedy { best_of } => best_of,
                        _ => 1,
                    },
                    used_initial_prompt: decoding_config.initial_prompt.is_some(),
                    transcription_latency_ms: 0,
                    real_time_factor: 0.0,
                    segment_count: 0,
                    is_empty: true,
                    transcript_char_count: 0,
                };
                return Ok((String::new(), diag));
            }

            // TEMP: whisper internal latency diagnostics
            let t_whisper_start = std::time::Instant::now();

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

            // TEMP: whisper internal latency diagnostics (create_state)
            let t_create_state_start = std::time::Instant::now();
            let mut state = ctx
                .create_state()
                .map_err(|e| SttError::TranscriptionFailed(e.to_string()))?;
            let t_create_state_end = std::time::Instant::now();

            let strategy = match &decoding_config.strategy {
                SttSamplingStrategy::Greedy { best_of } => {
                    SamplingStrategy::Greedy { best_of: *best_of }
                }
                SttSamplingStrategy::BeamSearch {
                    beam_size,
                    patience,
                } => SamplingStrategy::BeamSearch {
                    beam_size: *beam_size,
                    patience: *patience,
                },
            };

            let mut params = FullParams::new(strategy);
            params.set_language(language_config.whisper_language.as_deref());
            params.set_translate(language_config.translate);
            params.set_print_special(decoding_config.print_special);
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(decoding_config.print_timestamps);
            params.set_suppress_blank(decoding_config.suppress_blank);
            params.set_temperature(decoding_config.temperature);
            params.set_temperature_inc(decoding_config.temperature_inc);
            params.set_no_speech_thold(decoding_config.no_speech_thold);
            params.set_entropy_thold(decoding_config.entropy_thold);
            params.set_logprob_thold(decoding_config.logprob_thold);
            if let Some(ref prompt) = decoding_config.initial_prompt {
                params.set_initial_prompt(prompt);
            }
            params.set_n_threads(decoding_config.n_threads.unwrap_or_else(num_cpus));

            // TEMP: whisper internal latency diagnostics (state_full)
            let t_state_full_start = std::time::Instant::now();
            state
                .full(params, samples_16k_mono)
                .map_err(|e| SttError::TranscriptionFailed(e.to_string()))?;
            let t_state_full_end = std::time::Instant::now();

            let elapsed_ms = t_state_full_end.duration_since(t_state_full_start).as_millis();

            let mut text = String::new();
            let mut segment_count = 0;
            for segment in state.as_iter() {
                segment_count += 1;
                let segment_text = segment
                    .to_str_lossy()
                    .map_err(|e| SttError::TranscriptionFailed(e.to_string()))?;
                text.push_str(&segment_text);
            }

            let trimmed_text = text.trim().to_string();
            let rtf = if audio_dur > 0.0 {
                (elapsed_ms as f32 / 1000.0) / audio_dur
            } else {
                0.0
            };

            let diag = SttSessionDiagnostics {
                model_path: model_path.to_string(),
                audio_duration_seconds: audio_dur,
                whisper_language: language_config.whisper_language.clone(),
                decoding_strategy: format!("{:?}", decoding_config.strategy),
                temperature: decoding_config.temperature,
                temperature_inc: decoding_config.temperature_inc,
                best_of: match decoding_config.strategy {
                    SttSamplingStrategy::Greedy { best_of } => best_of,
                    _ => 1,
                },
                used_initial_prompt: decoding_config.initial_prompt.is_some(),
                transcription_latency_ms: elapsed_ms,
                real_time_factor: rtf,
                segment_count,
                is_empty: trimmed_text.is_empty(),
                transcript_char_count: trimmed_text.chars().count(),
            };

            // TEMP: whisper internal latency diagnostics (timing summary)
            let t_whisper_end = std::time::Instant::now();
            let create_state_ms = t_create_state_end.duration_since(t_create_state_start).as_millis();
            let state_full_ms = t_state_full_end.duration_since(t_state_full_start).as_millis();
            let whisper_total_ms = t_whisper_end.duration_since(t_whisper_start).as_millis();
            let other_ms = whisper_total_ms.saturating_sub(create_state_ms + state_full_ms);

            println!("\n==================================================");
            println!("WHISPER_INTERNAL_LATENCY");
            println!("create_state: {} ms", create_state_ms);
            println!("language_detection: 0 ms (embedded within state_full auto-detect pass)");
            println!("state_full: {} ms", state_full_ms);
            println!("other: {} ms", other_ms);
            println!("whisper_total: {} ms", whisper_total_ms);
            println!("==================================================\n");

            tracing::debug!(
                "Whisper STT finished: audio={:.2}s, latency={}ms, RTF={:.2}, lang={:?}, segments={}, chars={}",
                diag.audio_duration_seconds,
                diag.transcription_latency_ms,
                diag.real_time_factor,
                diag.whisper_language,
                diag.segment_count,
                diag.transcript_char_count
            );

            Ok((trimmed_text, diag))
        }
    }
}

/// Encoder context used for live streaming windows.
///
/// Whisper pads its mel spectrogram to 30 s regardless of input length, so a
/// 2-second window costs a full 30-second encoder pass at the default context
/// of 1500 positions. Clamping the encoder to 768 positions (~15 s of audio)
/// roughly halves that cost — the same trick whisper.cpp's own `stream`
/// example uses — and every window a live stream submits is far shorter than
/// the clamped span.
pub const LIVE_AUDIO_CTX: i32 = 768;

/// A dedicated Whisper context for one low-latency stream.
///
/// Two properties matter and neither is available through [`SttEngine`]:
///
/// 1. **Its own context.** `SttEngine` holds one model behind a mutex that is
///    locked for the whole of `whisper_full`, so a live clock sharing it would
///    serialize behind every 30-second chunk decode (and behind dictation).
/// 2. **A reused state.** `create_state` allocates whisper's KV cache and
///    compute buffers — roughly 330 MB for `ggml-small` — so allocating one
///    per window is far more expensive than the inference itself. This holds a
///    single state for the stream's lifetime.
///
/// Decoding is configured for short windows: one segment, no cross-window
/// prompt carry-over, greedy at temperature 0, and a clamped encoder context.
/// `single_segment` in particular stops whisper from discarding a whole window
/// when the decode ends on a lone timestamp token ("single timestamp ending -
/// skip entire chunk"), which is the common outcome for sub-2-second windows.
pub struct StreamingTranscriber {
    #[cfg(feature = "whisper-local")]
    state: whisper_rs::WhisperState,
    language: Option<String>,
    translate: bool,
    n_threads: i32,
    audio_ctx: i32,
}

impl StreamingTranscriber {
    /// Loads `model_path` into a private context and pre-allocates its state.
    #[cfg(feature = "whisper-local")]
    pub fn new(
        model_path: &str,
        language_config: &SttLanguageConfig,
        n_threads: i32,
    ) -> Result<Self, SttError> {
        if model_path.trim().is_empty() {
            return Err(SttError::ModelNotConfigured);
        }

        let ctx = WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
            .map_err(|e| SttError::ModelLoadFailed {
                path: model_path.to_string(),
                message: e.to_string(),
            })?;
        let state = ctx
            .create_state()
            .map_err(|e| SttError::TranscriptionFailed(e.to_string()))?;

        Ok(Self {
            state,
            language: language_config.whisper_language.clone(),
            translate: language_config.translate,
            n_threads: n_threads.max(1),
            audio_ctx: LIVE_AUDIO_CTX,
        })
    }

    #[cfg(not(feature = "whisper-local"))]
    pub fn new(
        _model_path: &str,
        _language_config: &SttLanguageConfig,
        _n_threads: i32,
    ) -> Result<Self, SttError> {
        Err(SttError::ModelNotConfigured)
    }

    /// Transcribes one window. The window is independent: no state, prompt, or
    /// text carries over from the previous call.
    #[cfg(feature = "whisper-local")]
    pub fn transcribe(&mut self, samples_16k_mono: &[f32]) -> Result<String, SttError> {
        if samples_16k_mono.is_empty() {
            return Ok(String::new());
        }

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(self.language.as_deref());
        params.set_translate(self.translate);
        params.set_n_threads(self.n_threads);
        params.set_audio_ctx(self.audio_ctx);
        params.set_single_segment(true);
        params.set_no_context(true);
        params.set_temperature(0.0);
        params.set_temperature_inc(0.0);
        params.set_suppress_blank(true);
        params.set_token_timestamps(false);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        self.state
            .full(params, samples_16k_mono)
            .map_err(|e| SttError::TranscriptionFailed(e.to_string()))?;

        let mut text = String::new();
        for segment in self.state.as_iter() {
            let segment_text = segment
                .to_str_lossy()
                .map_err(|e| SttError::TranscriptionFailed(e.to_string()))?;
            text.push_str(&segment_text);
        }

        Ok(text.trim().to_string())
    }

    #[cfg(not(feature = "whisper-local"))]
    pub fn transcribe(&mut self, _samples_16k_mono: &[f32]) -> Result<String, SttError> {
        Err(SttError::ModelNotConfigured)
    }
}

#[cfg(feature = "whisper-local")]
fn num_cpus() -> std::ffi::c_int {
    std::thread::available_parallelism()
        .map(|n| n.get() as std::ffi::c_int)
        .unwrap_or(4)
        .clamp(1, 8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::LanguageSettings;

    #[test]
    fn test_stt_language_config_english_only() {
        let settings = LanguageSettings {
            primary_dictation_language: "en".to_string(),
            spoken_languages: vec!["en".to_string()],
            notes_language: "en".to_string(),
            output_script: "latin".to_string(),
        };
        let config = SttLanguageConfig::from_settings(&settings);
        assert_eq!(config.whisper_language, Some("en".to_string()));
        assert!(!config.translate);
    }

    #[test]
    fn test_stt_language_config_hindi_only() {
        let settings = LanguageSettings {
            primary_dictation_language: "hi".to_string(),
            spoken_languages: vec!["hi".to_string()],
            notes_language: "hi".to_string(),
            output_script: "native".to_string(),
        };
        let config = SttLanguageConfig::from_settings(&settings);
        assert_eq!(config.whisper_language, Some("hi".to_string()));
        assert!(!config.translate);
    }

    #[test]
    fn test_stt_language_config_hinglish_mixed_profile() {
        // Selecting Hinglish in pill: primary="en", spoken=["en", "hi"]
        let settings = LanguageSettings {
            primary_dictation_language: "en".to_string(),
            spoken_languages: vec!["en".to_string(), "hi".to_string()],
            notes_language: "en".to_string(),
            output_script: "latin".to_string(),
        };
        let config = SttLanguageConfig::from_settings(&settings);
        // Must NOT pass "hinglish" or hard-lock to "en"
        assert_eq!(config.whisper_language, None);
        assert!(!config.translate);
    }

    #[test]
    fn test_stt_language_config_hindi_primary_mixed_profile() {
        // Hindi primary + English secondary: primary="hi", spoken=["hi", "en"]
        let settings = LanguageSettings {
            primary_dictation_language: "hi".to_string(),
            spoken_languages: vec!["hi".to_string(), "en".to_string()],
            notes_language: "en".to_string(),
            output_script: "latin".to_string(),
        };
        let config = SttLanguageConfig::from_settings(&settings);
        // Must NOT hard-lock to "hi"
        assert_eq!(config.whisper_language, None);
        assert!(!config.translate);
    }

    #[test]
    fn test_stt_language_config_auto() {
        let settings = LanguageSettings {
            primary_dictation_language: "auto".to_string(),
            spoken_languages: vec!["en".to_string()],
            notes_language: "en".to_string(),
            output_script: "latin".to_string(),
        };
        let config = SttLanguageConfig::from_settings(&settings);
        assert_eq!(config.whisper_language, None);
        assert!(!config.translate);
    }

    #[test]
    fn test_stt_language_config_duplicates_normalized() {
        let settings = LanguageSettings {
            primary_dictation_language: "en".to_string(),
            spoken_languages: vec!["en".to_string(), "en".to_string(), "EN".to_string()],
            notes_language: "en".to_string(),
            output_script: "latin".to_string(),
        };
        let config = SttLanguageConfig::from_settings(&settings);
        assert_eq!(config.whisper_language, Some("en".to_string()));
        assert!(!config.translate);
    }

    #[test]
    fn test_stt_language_config_whitespace_and_case() {
        let settings = LanguageSettings {
            primary_dictation_language: " HI ".to_string(),
            spoken_languages: vec!["  hi  ".to_string()],
            notes_language: "hi".to_string(),
            output_script: "native".to_string(),
        };
        let config = SttLanguageConfig::from_settings(&settings);
        assert_eq!(config.whisper_language, Some("hi".to_string()));
        assert!(!config.translate);
    }

    #[test]
    fn test_stt_language_config_output_script_independence() {
        let mut settings = LanguageSettings {
            primary_dictation_language: "hi".to_string(),
            spoken_languages: vec!["hi".to_string()],
            notes_language: "hi".to_string(),
            output_script: "latin".to_string(),
        };
        let config_latin = SttLanguageConfig::from_settings(&settings);

        settings.output_script = "native".to_string();
        let config_native = SttLanguageConfig::from_settings(&settings);

        // Output script setting must NOT change STT language configuration
        assert_eq!(config_latin, config_native);
        assert_eq!(config_latin.whisper_language, Some("hi".to_string()));
        assert!(!config_latin.translate);
    }

    #[test]
    fn test_whisper_decoding_config_baseline_invariants() {
        let baseline = WhisperDecodingConfig::baseline();
        assert_eq!(baseline.strategy, SttSamplingStrategy::Greedy { best_of: 1 });
        assert_eq!(baseline.temperature, 0.0);
        assert_eq!(baseline.temperature_inc, 0.2);
        assert_eq!(baseline.initial_prompt, None);
        assert!(baseline.suppress_blank);
        assert!(!baseline.print_special);
        assert!(!baseline.print_timestamps);
        assert_eq!(baseline.no_speech_thold, 0.6);
        assert_eq!(baseline.entropy_thold, 2.4);
        assert_eq!(baseline.logprob_thold, -1.0);
    }

    #[test]
    fn test_whisper_decoding_experiment_matrix_constructors() {
        let exp_b = WhisperDecodingConfig::experiment_best_of(3);
        assert_eq!(exp_b.strategy, SttSamplingStrategy::Greedy { best_of: 3 });

        let exp_c = WhisperDecodingConfig::experiment_temperature(0.2, 0.2);
        assert_eq!(exp_c.temperature, 0.2);

        let exp_d = WhisperDecodingConfig::experiment_prompt("Relay, Tauri, Rust");
        assert_eq!(exp_d.initial_prompt, Some("Relay, Tauri, Rust".to_string()));

        let exp_f = WhisperDecodingConfig::experiment_thresholds(0.7, 2.2, -1.2);
        assert_eq!(exp_f.no_speech_thold, 0.7);
        assert_eq!(exp_f.entropy_thold, 2.2);
        assert_eq!(exp_f.logprob_thold, -1.2);
    }

    #[test]
    #[cfg(feature = "whisper-local")]
    fn test_whisper_live_decoding_experiments_if_model_present() {
        let current_dir = std::env::current_dir().unwrap();
        let model_paths = [
            current_dir.join(".relay/config/models/ggml-small.bin"),
            current_dir.join("native/src-tauri/.relay/config/models/ggml-small.bin"),
            current_dir.join(".relay/config/models/ggml-base.bin"),
            current_dir.join("native/src-tauri/.relay/config/models/ggml-base.bin"),
        ];

        let model_path = model_paths.into_iter().find(|p| p.exists());
        if let Some(path) = model_path {
            let model_str = path.to_string_lossy().to_string();
            let engine = SttEngine::new();

            // Find a real test WAV file from .relay/config/audio with duration > 2s
            let audio_dirs = [
                current_dir.join(".relay/config/audio"),
                current_dir.join("native/src-tauri/.relay/config/audio"),
            ];

            let mut test_samples: Option<Vec<f32>> = None;
            for audio_dir in &audio_dirs {
                if audio_dir.exists() {
                    if let Ok(entries) = std::fs::read_dir(audio_dir) {
                        for entry in entries.flatten() {
                            let p = entry.path();
                            if p.extension().and_then(|e| e.to_str()) == Some("wav") {
                                if let Ok(reader) = hound::WavReader::open(&p) {
                                    let spec = reader.spec();
                                    let samples: Vec<f32> = match spec.sample_format {
                                        hound::SampleFormat::Float => reader
                                            .into_samples::<f32>()
                                            .filter_map(|s| s.ok())
                                            .collect(),
                                        hound::SampleFormat::Int => {
                                            let max_val = i16::MAX as f32;
                                            reader
                                                .into_samples::<i32>()
                                                .filter_map(|s| s.ok())
                                                .map(|s| s as f32 / max_val)
                                                .collect()
                                        }
                                    };
                                    if samples.len() >= 32000 && samples.len() <= 160000 {
                                        // 2s to 10s audio
                                        test_samples = Some(samples);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                if test_samples.is_some() {
                    break;
                }
            }

            if let Some(samples) = test_samples {
                let lang_en = SttLanguageConfig {
                    whisper_language: Some("en".to_string()),
                    translate: false,
                };
                let lang_auto = SttLanguageConfig {
                    whisper_language: None,
                    translate: false,
                };

                println!("\n==========================================================================");
                println!("           PHASE 4: WHISPER DECODING EXPERIMENT MATRIX RESULTS            ");
                println!("==========================================================================");

                // Exp A: Baseline Greedy
                let cfg_a = WhisperDecodingConfig::baseline();
                let (res_a, diag_a) = engine
                    .transcribe_with_config(Some(&model_str), &samples, &lang_en, &cfg_a)
                    .unwrap();
                println!(
                    "EXP A (Baseline Greedy): latency={}ms, RTF={:.2}, segments={}, len={}\n  -> \"{}\"",
                    diag_a.transcription_latency_ms, diag_a.real_time_factor, diag_a.segment_count, diag_a.transcript_char_count, res_a
                );

                // Exp B: best_of = 3
                let cfg_b = WhisperDecodingConfig::experiment_best_of(3);
                let (res_b, diag_b) = engine
                    .transcribe_with_config(Some(&model_str), &samples, &lang_en, &cfg_b)
                    .unwrap();
                println!(
                    "EXP B (Greedy best_of=3): latency={}ms, RTF={:.2}, segments={}, len={}\n  -> \"{}\"",
                    diag_b.transcription_latency_ms, diag_b.real_time_factor, diag_b.segment_count, diag_b.transcript_char_count, res_b
                );

                // Exp C: Temperature fallback
                let cfg_c = WhisperDecodingConfig::experiment_temperature(0.2, 0.2);
                let (res_c, diag_c) = engine
                    .transcribe_with_config(Some(&model_str), &samples, &lang_en, &cfg_c)
                    .unwrap();
                println!(
                    "EXP C (Temp Fallback 0.2): latency={}ms, RTF={:.2}, segments={}, len={}\n  -> \"{}\"",
                    diag_c.transcription_latency_ms, diag_c.real_time_factor, diag_c.segment_count, diag_c.transcript_char_count, res_c
                );

                // Exp D: Initial Prompt
                let cfg_d = WhisperDecodingConfig::experiment_prompt("Relay, Whisper, Tauri, Rust, Supabase, GitHub, Vercel, n8n");
                let (res_d, diag_d) = engine
                    .transcribe_with_config(Some(&model_str), &samples, &lang_en, &cfg_d)
                    .unwrap();
                println!(
                    "EXP D (Tech Initial Prompt): latency={}ms, RTF={:.2}, segments={}, len={}\n  -> \"{}\"",
                    diag_d.transcription_latency_ms, diag_d.real_time_factor, diag_d.segment_count, diag_d.transcript_char_count, res_d
                );

                // Exp E: Auto vs Locked Language
                let (res_e, diag_e) = engine
                    .transcribe_with_config(Some(&model_str), &samples, &lang_auto, &cfg_a)
                    .unwrap();
                println!(
                    "EXP E (Auto Language Detect): latency={}ms, RTF={:.2}, segments={}, len={}\n  -> \"{}\"",
                    diag_e.transcription_latency_ms, diag_e.real_time_factor, diag_e.segment_count, diag_e.transcript_char_count, res_e
                );

                // Exp F: Hallucination Suppression Thresholds
                let cfg_f = WhisperDecodingConfig::experiment_thresholds(0.7, 2.2, -0.9);
                let (res_f, diag_f) = engine
                    .transcribe_with_config(Some(&model_str), &samples, &lang_en, &cfg_f)
                    .unwrap();
                println!(
                    "EXP F (Hallucination Thresholds): latency={}ms, RTF={:.2}, segments={}, len={}\n  -> \"{}\"",
                    diag_f.transcription_latency_ms, diag_f.real_time_factor, diag_f.segment_count, diag_f.transcript_char_count, res_f
                );
                println!("==========================================================================\n");

                assert!(!diag_a.is_empty);
            }
        }
    }

    #[test]
    fn test_whisper_decoding_config_for_dictation_thread_bounds() {
        let mut settings = crate::settings::SttSettings::default();

        // 1. Default (no override) clamps within [1, 12]
        let cfg_default = WhisperDecodingConfig::for_dictation(&settings);
        let threads = cfg_default.n_threads.unwrap();
        assert!(threads >= 1 && threads <= 12);

        // 2. Safe user override
        settings.dictation_threads = Some(8);
        let cfg_custom = WhisperDecodingConfig::for_dictation(&settings);
        assert_eq!(cfg_custom.n_threads, Some(8));

        // 3. User override <= 0 clamps to 1
        settings.dictation_threads = Some(0);
        let cfg_zero = WhisperDecodingConfig::for_dictation(&settings);
        assert_eq!(cfg_zero.n_threads, Some(1));

        settings.dictation_threads = Some(-5);
        let cfg_neg = WhisperDecodingConfig::for_dictation(&settings);
        assert_eq!(cfg_neg.n_threads, Some(1));

        // 4. Extreme user override clamps to 64
        settings.dictation_threads = Some(128);
        let cfg_high = WhisperDecodingConfig::for_dictation(&settings);
        assert_eq!(cfg_high.n_threads, Some(64));
    }
}
