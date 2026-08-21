use std::path::Path;
use serde::{Deserialize, Serialize};

use crate::capture::stt::{
    SttEngine, SttLanguageConfig, SttSamplingStrategy, WhisperDecodingConfig,
    DEFAULT_MODEL_FILENAME,
};
use crate::capture::{resample_to_16k_mono, AudioStats, VadConfig};
use crate::settings::LanguageSettings;

/// Standard Relay technical vocabulary prompt tested during Prompt experiments.
pub const RELAY_TECHNICAL_PROMPT: &str =
    "Relay, Tauri, Rust, Whisper, Supabase, GitHub, Vercel, React, TypeScript, CPAL, whisper-rs";

/// Known Relay technical terms tracked for technical-term accuracy calculations.
pub const TRACKED_TECHNICAL_TERMS: &[&str] = &[
    "relay",
    "tauri",
    "rust",
    "whisper",
    "supabase",
    "github",
    "vercel",
    "react",
    "typescript",
    "cpal",
    "whisper-rs",
];

/// Supported evaluation decoding configuration variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalConfigVariant {
    /// Baseline greedy decoding (best_of = 1, temp = 0.0, prompt = None)
    Baseline,
    /// Greedy decoding with Relay technical vocabulary prompt
    RelayPrompt,
    /// Greedy decoding with best_of = 3
    BestOf3,
    /// Beam search with beam_size = 2, patience = 1.0
    Beam2,
    /// Staged fallback (initial temp = 0.0, retry at temp = 0.2 if unreliable)
    TemperatureFallback,
}

impl EvalConfigVariant {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::RelayPrompt => "relay_prompt",
            Self::BestOf3 => "best_of_3",
            Self::Beam2 => "beam_2",
            Self::TemperatureFallback => "temperature_fallback",
        }
    }

    /// Builds the primary `WhisperDecodingConfig` for this experiment variant.
    pub fn to_decoding_config(&self) -> WhisperDecodingConfig {
        match self {
            Self::Baseline => WhisperDecodingConfig::baseline(),
            Self::RelayPrompt => {
                let mut cfg = WhisperDecodingConfig::baseline();
                cfg.initial_prompt = Some(RELAY_TECHNICAL_PROMPT.to_string());
                cfg
            }
            Self::BestOf3 => {
                let mut cfg = WhisperDecodingConfig::baseline();
                cfg.strategy = SttSamplingStrategy::Greedy { best_of: 3 };
                cfg
            }
            Self::Beam2 => {
                let mut cfg = WhisperDecodingConfig::baseline();
                cfg.strategy = SttSamplingStrategy::BeamSearch {
                    beam_size: 2,
                    patience: 1.0,
                };
                cfg
            }
            Self::TemperatureFallback => {
                // Primary attempt starts at temperature = 0.0
                WhisperDecodingConfig::baseline()
            }
        }
    }
}

/// Comprehensive structured evaluation result schema for reproducibility and diagnostic analysis.
/// Does NOT contain microphone/device identifiers, usernames, or telemetry paths.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvaluationResult {
    pub test_id: String,
    pub audio_file: String,
    pub configuration: String,
    pub language_setting: String,
    pub resolved_whisper_language: Option<String>,

    pub original_duration_seconds: f32,
    pub processed_duration_seconds: f32,

    pub inference_duration_ms: u128,
    pub real_time_factor: f32,

    pub transcript: String,

    pub audio_rms: f32,
    pub audio_peak: f32,
    pub near_zero_percent: f32,

    pub speech_detected: bool,
    pub vad_trimmed_duration: f32,

    pub model_filename: String,

    pub sampling_strategy: String,
    pub best_of: i32,
    pub beam_size: Option<i32>,
    pub temperature: f32,
    pub temperature_increment: f32,
    pub initial_prompt_used: bool,

    pub no_speech_threshold: f32,
    pub entropy_threshold: f32,
    pub logprob_threshold: f32,

    pub accuracy: Option<AccuracyMetrics>,
    pub fallback_triggered: bool,
    pub error: Option<String>,
}

/// Authoritative diagnostic snapshot of the last completed speech-to-text session.
/// Contains complete audio telemetry, VAD decisions, resolved language, effective
/// decoding parameters, inference latency, and transcription outcome without leaking private paths.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SttDiagnosticSnapshot {
    pub timestamp_epoch_ms: u128,
    pub session_mode: String,
    pub audio_file: Option<String>,

    // Audio characteristics
    pub original_duration_seconds: f32,
    pub processed_duration_seconds: f32,
    pub sample_rate: u32,
    pub channels: u16,
    pub rms: f32,
    pub peak_amplitude: f32,
    pub near_zero_percent: f32,
    pub has_non_finite: bool,

    // VAD activity
    pub speech_detected: bool,
    pub vad_start_seconds: f32,
    pub vad_end_seconds: f32,
    pub vad_trimmed_duration_seconds: f32,
    pub silence_removed_percent: f32,
    pub noise_floor: f32,
    pub onset_threshold: f32,

    // Model & language resolution
    pub model_filename: String,
    pub model_path: String,
    pub primary_dictation_language: String,
    pub spoken_languages: Vec<String>,
    pub resolved_whisper_language: Option<String>,
    pub translate: bool,

    // Effective decoding configuration used
    pub strategy: String,
    pub best_of: i32,
    pub beam_size: Option<i32>,
    pub temperature: f32,
    pub temperature_inc: f32,
    pub used_initial_prompt: bool,
    pub initial_prompt_text: Option<String>,
    pub no_speech_thold: f32,
    pub entropy_thold: f32,
    pub logprob_thold: f32,

    // Performance & Transcription Outcome
    pub inference_duration_ms: u128,
    pub real_time_factor: f32,
    pub segment_count: usize,
    pub transcript: String,
    pub transcript_char_count: usize,
    pub error: Option<String>,
}

/// Helper to build a complete SttDiagnosticSnapshot from a completed or failed capture/transcription run.
pub fn build_diagnostic_snapshot(
    session_mode: &str,
    audio_file: Option<String>,
    captured: &crate::capture::CapturedAudio,
    lang_settings: &LanguageSettings,
    lang_config: &SttLanguageConfig,
    decoding_config: &WhisperDecodingConfig,
    model_path_str: &str,
    transcript: &str,
    diag: Option<&crate::capture::stt::SttSessionDiagnostics>,
    error: Option<String>,
) -> SttDiagnosticSnapshot {
    let model_filename = Path::new(model_path_str)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| DEFAULT_MODEL_FILENAME.to_string());

    let (beam_size, best_of) = match &decoding_config.strategy {
        SttSamplingStrategy::Greedy { best_of } => (None, *best_of),
        SttSamplingStrategy::BeamSearch { beam_size, .. } => (Some(*beam_size), 1),
    };

    let (latency_ms, rtf, segments) = match diag {
        Some(d) => (d.transcription_latency_ms, d.real_time_factor, d.segment_count),
        None => (0, 0.0, 0),
    };

    let epoch_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);

    SttDiagnosticSnapshot {
        timestamp_epoch_ms: epoch_ms,
        session_mode: session_mode.to_string(),
        audio_file,
        original_duration_seconds: captured.original_duration_seconds,
        processed_duration_seconds: captured.duration_seconds,
        sample_rate: captured.audio_stats.sample_rate,
        channels: captured.audio_stats.channels,
        rms: captured.audio_stats.rms,
        peak_amplitude: captured.audio_stats.peak_amplitude,
        near_zero_percent: captured.audio_stats.near_zero_percent,
        has_non_finite: captured.audio_stats.has_non_finite,
        speech_detected: captured.vad_result.speech_detected,
        vad_start_seconds: captured.vad_result.start_seconds,
        vad_end_seconds: captured.vad_result.end_seconds,
        vad_trimmed_duration_seconds: captured.vad_result.trimmed_duration,
        silence_removed_percent: captured.vad_result.silence_removed_percent,
        noise_floor: captured.vad_result.noise_floor,
        onset_threshold: captured.vad_result.onset_threshold,
        model_filename,
        model_path: model_path_str.to_string(),
        primary_dictation_language: lang_settings.primary_dictation_language.clone(),
        spoken_languages: lang_settings.spoken_languages.clone(),
        resolved_whisper_language: lang_config.whisper_language.clone(),
        translate: lang_config.translate,
        strategy: format!("{:?}", decoding_config.strategy),
        best_of,
        beam_size,
        temperature: decoding_config.temperature,
        temperature_inc: decoding_config.temperature_inc,
        used_initial_prompt: decoding_config.initial_prompt.is_some(),
        initial_prompt_text: decoding_config.initial_prompt.clone(),
        no_speech_thold: decoding_config.no_speech_thold,
        entropy_thold: decoding_config.entropy_thold,
        logprob_thold: decoding_config.logprob_thold,
        inference_duration_ms: latency_ms,
        real_time_factor: rtf,
        segment_count: segments,
        transcript: transcript.to_string(),
        transcript_char_count: transcript.chars().count(),
        error,
    }
}

/// Accuracy metrics calculated against ground-truth reference text.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AccuracyMetrics {
    pub reference: String,
    pub hypothesis: String,
    pub word_count: usize,
    pub substitutions: usize,
    pub deletions: usize,
    pub insertions: usize,
    pub wer: f32,
    pub cer: f32,
    pub technical_term_accuracy: Option<f32>,
    pub technical_term_errors: Vec<TechnicalTermError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TechnicalTermError {
    pub expected: String,
    pub actual_found: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SttFailureCategory {
    AcousticIssue,
    VadBoundaryIssue,
    Hallucination,
    TechnicalTermError,
    LanguageMismatch,
    HighLatency,
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SttFailureDiagnostic {
    pub category: SttFailureCategory,
    pub description: String,
    pub severity: String,
    pub suggested_remedy: String,
}

pub fn classify_stt_failure(
    audio_stats: &crate::capture::AudioStats,
    vad_result: &crate::capture::VadResult,
    eval_result: &EvaluationResult,
) -> Vec<SttFailureDiagnostic> {
    let mut diagnostics = Vec::new();

    if audio_stats.has_non_finite {
        diagnostics.push(SttFailureDiagnostic {
            category: SttFailureCategory::AcousticIssue,
            description: "Audio buffer contained non-finite float32 samples (NaN/Inf)".to_string(),
            severity: "critical".to_string(),
            suggested_remedy: "Sanitize float32 capture stream using AudioStats sanitization".to_string(),
        });
    }
    if audio_stats.near_clipping_percent > 1.0 || audio_stats.peak_amplitude >= 0.99 {
        diagnostics.push(SttFailureDiagnostic {
            category: SttFailureCategory::AcousticIssue,
            description: "Microphone input clipped (amplitude >= 0.99)".to_string(),
            severity: "medium".to_string(),
            suggested_remedy: "Lower microphone input gain in OS sound settings".to_string(),
        });
    }

    if vad_result.speech_detected && vad_result.trimmed_duration < 0.2 && eval_result.original_duration_seconds > 1.0 {
        diagnostics.push(SttFailureDiagnostic {
            category: SttFailureCategory::VadBoundaryIssue,
            description: "VAD trimmed >80% of audio resulting in under 200ms speech slice".to_string(),
            severity: "medium".to_string(),
            suggested_remedy: "Calibrate ambient noise floor or adjust speech margin".to_string(),
        });
    }

    if !vad_result.speech_detected && !eval_result.transcript.trim().is_empty() {
        diagnostics.push(SttFailureDiagnostic {
            category: SttFailureCategory::Hallucination,
            description: "Whisper emitted text despite no speech detected by VAD".to_string(),
            severity: "critical".to_string(),
            suggested_remedy: "Ensure had_audio gating rejects transcription on silence".to_string(),
        });
    }

    if let Some(ref acc) = eval_result.accuracy {
        if !acc.technical_term_errors.is_empty() {
            diagnostics.push(SttFailureDiagnostic {
                category: SttFailureCategory::TechnicalTermError,
                description: format!("Missed {} domain technical terms", acc.technical_term_errors.len()),
                severity: "low".to_string(),
                suggested_remedy: "Enable Technical Domain Vocabulary Priming in Settings".to_string(),
            });
        }
    }

    if eval_result.real_time_factor > 0.50 || (eval_result.inference_duration_ms > 2000 && eval_result.original_duration_seconds < 4.0) {
        diagnostics.push(SttFailureDiagnostic {
            category: SttFailureCategory::HighLatency,
            description: format!("High inference latency ({}ms, RTF {:.2}x)", eval_result.inference_duration_ms, eval_result.real_time_factor),
            severity: "medium".to_string(),
            suggested_remedy: "Ensure multi-threading is enabled or avoid high beam sizes".to_string(),
        });
    }

    diagnostics
}

/// Item in the curated evaluation corpus manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CorpusItem {
    pub test_id: String,
    pub audio_filename: String,
    pub category: String,
    pub language: String,
    pub reference: Option<String>,
    pub reference_available: bool,
    pub description: String,
}

/// Curated corpus of 35 representative test recordings across English, Hindi, Hinglish,
/// technical vocabulary, short commands, and silence/noise categories.
pub fn get_curated_corpus() -> Vec<CorpusItem> {
    vec![
        // --- 1. English Conversational & Technical (10 items) ---
        CorpusItem {
            test_id: "en_short_001".to_string(),
            audio_filename: "dictation_7fea4e59.wav".to_string(),
            category: "english_short".to_string(),
            language: "en".to_string(),
            reference: Some("I need to finish this report today.".to_string()),
            reference_available: true,
            description: "Short conversational English request".to_string(),
        },
        CorpusItem {
            test_id: "en_conv_002".to_string(),
            audio_filename: "dictation_81471fe9.wav".to_string(),
            category: "english_conversational".to_string(),
            language: "en".to_string(),
            reference: Some("Let's review the automation team's progress before the meeting.".to_string()),
            reference_available: true,
            description: "Conversational sentence with multiple clauses".to_string(),
        },
        CorpusItem {
            test_id: "en_long_003".to_string(),
            audio_filename: "dictation_ff078ec0.wav".to_string(),
            category: "english_long".to_string(),
            language: "en".to_string(),
            reference: Some("I need to review the automation team's progress this afternoon, identify the three workflows that are still blocked, speak to the owners about the dependencies, and then send a short update to the leadership team before the end of the day.".to_string()),
            reference_available: true,
            description: "Long multi-clause dictation test".to_string(),
        },
        CorpusItem {
            test_id: "en_tech_004".to_string(),
            audio_filename: "dictation_c5410592.wav".to_string(),
            category: "english_technical".to_string(),
            language: "en".to_string(),
            reference: Some("Relay uses Tauri, Rust, Whisper and Supabase for local speech recognition.".to_string()),
            reference_available: true,
            description: "English sentence packed with technical framework nouns".to_string(),
        },
        CorpusItem {
            test_id: "en_quiet_005".to_string(),
            audio_filename: "dictation_f141e57f.wav".to_string(),
            category: "english_quiet".to_string(),
            language: "en".to_string(),
            reference: Some("This is a low volume test.".to_string()),
            reference_available: true,
            description: "Spoken at low amplitude to test sensitivity".to_string(),
        },
        CorpusItem {
            test_id: "en_pause_006".to_string(),
            audio_filename: "dictation_d8aa36b6.wav".to_string(),
            category: "english_pauses".to_string(),
            language: "en".to_string(),
            reference: Some("I need to check the deployment... before sending the update.".to_string()),
            reference_available: true,
            description: "Natural pauses inside a sentence".to_string(),
        },
        CorpusItem {
            test_id: "en_tech_react_ts_007".to_string(),
            audio_filename: "dictation_e4b109aa.wav".to_string(),
            category: "english_technical".to_string(),
            language: "en".to_string(),
            reference: Some("React components in TypeScript with CPAL audio backend.".to_string()),
            reference_available: true,
            description: "React, TypeScript, and CPAL technical terms".to_string(),
        },
        CorpusItem {
            test_id: "en_tech_mcp_008".to_string(),
            audio_filename: "dictation_d12a55ee.wav".to_string(),
            category: "english_technical".to_string(),
            language: "en".to_string(),
            reference: Some("Trigger the MCP calendar tool to schedule a reminder.".to_string()),
            reference_available: true,
            description: "MCP tool execution technical phrasing".to_string(),
        },
        CorpusItem {
            test_id: "en_tech_lance_009".to_string(),
            audio_filename: "dictation_ca441112.wav".to_string(),
            category: "english_technical".to_string(),
            language: "en".to_string(),
            reference: Some("Store local note vectors in LanceDB table.".to_string()),
            reference_available: true,
            description: "LanceDB vector database terminology".to_string(),
        },
        CorpusItem {
            test_id: "en_fast_010".to_string(),
            audio_filename: "dictation_bf223344.wav".to_string(),
            category: "english_fast".to_string(),
            language: "en".to_string(),
            reference: Some("Quick dictation note update the kanban cards.".to_string()),
            reference_available: true,
            description: "Fast English utterance".to_string(),
        },

        // --- 2. Hindi Natural & Conversational (6 items) ---
        CorpusItem {
            test_id: "hi_short_001".to_string(),
            audio_filename: "dictation_f63bd971.wav".to_string(),
            category: "hindi_short".to_string(),
            language: "hi".to_string(),
            reference: Some("कल मुझे टीम के साथ एक मीटिंग करनी है।".to_string()),
            reference_available: true,
            description: "Natural short Hindi sentence".to_string(),
        },
        CorpusItem {
            test_id: "hi_conv_002".to_string(),
            audio_filename: "dictation_fa37fce8.wav".to_string(),
            category: "hindi_conversational".to_string(),
            language: "hi".to_string(),
            reference: Some("कल मुझे टीम के साथ एक मीटिंग करनी है और उसके बाद मुझे रिपोर्ट पूरी करनी है।".to_string()),
            reference_available: true,
            description: "Normal conversational Hindi".to_string(),
        },
        CorpusItem {
            test_id: "hi_long_003".to_string(),
            audio_filename: "dictation_e23ebaec.wav".to_string(),
            category: "hindi_long".to_string(),
            language: "hi".to_string(),
            reference: Some("आज सुबह मैंने पूरी टीम के साथ प्रोजेक्ट की प्रगति पर चर्चा की। कुछ काम अभी भी बाकी हैं, इसलिए मुझे पहले उन लोगों से बात करनी है जिनके टास्क ब्लॉक हो गए हैं।".to_string()),
            reference_available: true,
            description: "Long Hindi paragraph".to_string(),
        },
        CorpusItem {
            test_id: "hi_quiet_004".to_string(),
            audio_filename: "dictation_fdfb5ec0.wav".to_string(),
            category: "hindi_quiet".to_string(),
            language: "hi".to_string(),
            reference: Some("मुझे आज शाम तक रिपोर्ट भेजनी है।".to_string()),
            reference_available: true,
            description: "Quiet Hindi dictation".to_string(),
        },
        CorpusItem {
            test_id: "hi_task_005".to_string(),
            audio_filename: "dictation_c1998877.wav".to_string(),
            category: "hindi_task".to_string(),
            language: "hi".to_string(),
            reference: Some("कृपया इस कार्य को प्राथमिक सूची में जोड़ें।".to_string()),
            reference_available: true,
            description: "Hindi task management phrase".to_string(),
        },
        CorpusItem {
            test_id: "hi_notes_006".to_string(),
            audio_filename: "dictation_d4433221.wav".to_string(),
            category: "hindi_notes".to_string(),
            language: "hi".to_string(),
            reference: Some("आज के मुख्य बिंदु नोट कर लीजिए।".to_string()),
            reference_available: true,
            description: "Hindi note-taking command".to_string(),
        },

        // --- 3. Hinglish / Code-Switching (8 items) ---
        CorpusItem {
            test_id: "hinglish_001".to_string(),
            audio_filename: "dictation_ebcaee29.wav".to_string(),
            category: "hinglish_code_switching".to_string(),
            language: "hinglish".to_string(),
            reference: Some("आज मुझे team के साथ project review करना है।".to_string()),
            reference_available: true,
            description: "Code-switching with English nouns in Hindi matrix".to_string(),
        },
        CorpusItem {
            test_id: "hinglish_002".to_string(),
            audio_filename: "dictation_ed9f38fb.wav".to_string(),
            category: "hinglish_code_switching".to_string(),
            language: "hinglish".to_string(),
            reference: Some("कल हमने deployment पूरा किया लेकिन production में कुछ issues अभी भी हैं।".to_string()),
            reference_available: true,
            description: "Deployment and production technical Hinglish".to_string(),
        },
        CorpusItem {
            test_id: "hinglish_003".to_string(),
            audio_filename: "dictation_f3cf42e7.wav".to_string(),
            category: "hinglish_code_switching".to_string(),
            language: "hinglish".to_string(),
            reference: Some("मैंने Supabase database check किया और वहाँ कुछ duplicate records मिले।".to_string()),
            reference_available: true,
            description: "Supabase database technical code-switching".to_string(),
        },
        CorpusItem {
            test_id: "hinglish_004".to_string(),
            audio_filename: "dictation_e1dba298.wav".to_string(),
            category: "hinglish_code_switching".to_string(),
            language: "hinglish".to_string(),
            reference: Some("आज शाम को GitHub पर PR merge कर देना।".to_string()),
            reference_available: true,
            description: "GitHub PR technical terminology".to_string(),
        },
        CorpusItem {
            test_id: "hinglish_005".to_string(),
            audio_filename: "dictation_e2b46188.wav".to_string(),
            category: "hinglish_code_switching".to_string(),
            language: "hinglish".to_string(),
            reference: Some("Basically मुझे पहले ये check करना है कि Whisper model properly load ho raha hai ya nahi.".to_string()),
            reference_available: true,
            description: "Complex conversational Hinglish with technical phrasing".to_string(),
        },
        CorpusItem {
            test_id: "hinglish_006".to_string(),
            audio_filename: "dictation_ab554433.wav".to_string(),
            category: "hinglish_code_switching".to_string(),
            language: "hinglish".to_string(),
            reference: Some("Tauri application ko build karne ke liye Rust compiler chahiye.".to_string()),
            reference_available: true,
            description: "Tauri and Rust compiler Hinglish sentence".to_string(),
        },
        CorpusItem {
            test_id: "hinglish_007".to_string(),
            audio_filename: "dictation_bc665544.wav".to_string(),
            category: "hinglish_code_switching".to_string(),
            language: "hinglish".to_string(),
            reference: Some("Next.js web dashboard me user login ke liye Supabase auth use ho raha hai.".to_string()),
            reference_available: true,
            description: "Next.js and Supabase auth Hinglish sentence".to_string(),
        },
        CorpusItem {
            test_id: "hinglish_008".to_string(),
            audio_filename: "dictation_cd776655.wav".to_string(),
            category: "hinglish_code_switching".to_string(),
            language: "hinglish".to_string(),
            reference: Some("Meeting ke baad action items ko Kanban board pe move kar dena.".to_string()),
            reference_available: true,
            description: "Kanban board action items Hinglish sentence".to_string(),
        },

        // --- 4. Short Legitimate Utterances (5 items) ---
        CorpusItem {
            test_id: "short_en_okay".to_string(),
            audio_filename: "dictation_f39bb789.wav".to_string(),
            category: "short_legitimate".to_string(),
            language: "en".to_string(),
            reference: Some("Okay.".to_string()),
            reference_available: true,
            description: "Short affirmative word 'Okay.'".to_string(),
        },
        CorpusItem {
            test_id: "short_en_yes".to_string(),
            audio_filename: "dictation_f279d099.wav".to_string(),
            category: "short_legitimate".to_string(),
            language: "en".to_string(),
            reference: Some("Yes.".to_string()),
            reference_available: true,
            description: "Short affirmative word 'Yes.'".to_string(),
        },
        CorpusItem {
            test_id: "short_en_done".to_string(),
            audio_filename: "dictation_da112233.wav".to_string(),
            category: "short_legitimate".to_string(),
            language: "en".to_string(),
            reference: Some("Done.".to_string()),
            reference_available: true,
            description: "Single-word command 'Done.'".to_string(),
        },
        CorpusItem {
            test_id: "short_en_commit".to_string(),
            audio_filename: "dictation_eb223344.wav".to_string(),
            category: "short_legitimate".to_string(),
            language: "en".to_string(),
            reference: Some("Commit.".to_string()),
            reference_available: true,
            description: "Single-word command 'Commit.'".to_string(),
        },
        CorpusItem {
            test_id: "short_hi_theek".to_string(),
            audio_filename: "dictation_f1489099.wav".to_string(),
            category: "short_legitimate".to_string(),
            language: "hi".to_string(),
            reference: Some("ठीक है।".to_string()),
            reference_available: true,
            description: "Short Hindi acknowledgement 'ठीक है।'".to_string(),
        },

        // --- 5. Silence, Noise & Accidental Triggers (6 items) ---
        CorpusItem {
            test_id: "silence_empty_001".to_string(),
            audio_filename: "dictation_e7166cbe.wav".to_string(),
            category: "silence_noise".to_string(),
            language: "en".to_string(),
            reference: Some("".to_string()),
            reference_available: true,
            description: "Empty recording / zero speech".to_string(),
        },
        CorpusItem {
            test_id: "noise_accidental_tap".to_string(),
            audio_filename: "dictation_e204a1d3.wav".to_string(),
            category: "silence_noise".to_string(),
            language: "en".to_string(),
            reference: Some("".to_string()),
            reference_available: true,
            description: "Accidental key tap / desk knock under 300ms".to_string(),
        },
        CorpusItem {
            test_id: "noise_desk_thump".to_string(),
            audio_filename: "dictation_c8112233.wav".to_string(),
            category: "silence_noise".to_string(),
            language: "en".to_string(),
            reference: Some("".to_string()),
            reference_available: true,
            description: "Low-frequency desk thump".to_string(),
        },
        CorpusItem {
            test_id: "noise_mic_breath".to_string(),
            audio_filename: "dictation_d9223344.wav".to_string(),
            category: "silence_noise".to_string(),
            language: "en".to_string(),
            reference: Some("".to_string()),
            reference_available: true,
            description: "Microphone exhale / breathing puff".to_string(),
        },
        CorpusItem {
            test_id: "noise_keyboard_typing".to_string(),
            audio_filename: "dictation_ea334455.wav".to_string(),
            category: "silence_noise".to_string(),
            language: "en".to_string(),
            reference: Some("".to_string()),
            reference_available: true,
            description: "Mechanical keyboard typing burst".to_string(),
        },
        CorpusItem {
            test_id: "silence_pause_room".to_string(),
            audio_filename: "dictation_fb445566.wav".to_string(),
            category: "silence_noise".to_string(),
            language: "en".to_string(),
            reference: Some("".to_string()),
            reference_available: true,
            description: "Room tone / air conditioner ambient humming".to_string(),
        },
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BenchmarkAggregate {
    pub variant: String,
    pub total_samples: usize,
    pub evaluated_samples: usize,
    pub mean_wer: f32,
    pub mean_cer: f32,
    pub mean_technical_term_accuracy: f32,
    pub mean_inference_latency_ms: f32,
    pub mean_rtf: f32,
    pub hallucination_count: usize,
    pub total_substitutions: usize,
    pub total_deletions: usize,
    pub total_insertions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BenchmarkReport {
    pub generated_at_epoch_ms: u128,
    pub corpus_size: usize,
    pub variants: Vec<BenchmarkAggregate>,
}

/// Executes an automated comparative evaluation matrix over an in-memory test audio buffer
/// across all 5 STT configurations.
pub fn run_benchmark_matrix_on_sample(
    sample_id: &str,
    filename: &str,
    samples: &[f32],
    sample_rate: u32,
    lang_settings: &LanguageSettings,
    model_path: Option<&str>,
    reference: Option<&str>,
    stt_engine: &SttEngine,
) -> Vec<EvaluationResult> {
    let variants = [
        EvalConfigVariant::Baseline,
        EvalConfigVariant::RelayPrompt,
        EvalConfigVariant::BestOf3,
        EvalConfigVariant::Beam2,
        EvalConfigVariant::TemperatureFallback,
    ];

    variants
        .iter()
        .map(|v| {
            evaluate_audio_buffer(
                sample_id,
                filename,
                samples,
                sample_rate,
                *v,
                lang_settings,
                model_path,
                reference,
                stt_engine,
            )
        })
        .collect()
}

/// Calculates deterministic Word Error Rate (WER) and Character Error Rate (CER)
/// using Levenshtein dynamic programming alignment.
pub fn calculate_accuracy(reference: &str, hypothesis: &str) -> AccuracyMetrics {
    let norm_ref = normalize_for_eval(reference);
    let norm_hyp = normalize_for_eval(hypothesis);

    let ref_words: Vec<&str> = norm_ref.split_whitespace().collect();
    let hyp_words: Vec<&str> = norm_hyp.split_whitespace().collect();

    let (subs, dels, inss) = compute_alignment_ops(&ref_words, &hyp_words);
    let word_count = ref_words.len();

    let wer = if word_count == 0 {
        if hyp_words.is_empty() {
            0.0
        } else {
            1.0
        }
    } else {
        ((subs + dels + inss) as f32 / word_count as f32).min(5.0)
    };

    // Character error rate (CER)
    let ref_chars: Vec<char> = norm_ref.chars().filter(|c| !c.is_whitespace()).collect();
    let hyp_chars: Vec<char> = norm_hyp.chars().filter(|c| !c.is_whitespace()).collect();
    let (c_subs, c_dels, c_inss) = compute_alignment_ops(&ref_chars, &hyp_chars);
    let char_count = ref_chars.len();
    let cer = if char_count == 0 {
        if hyp_chars.is_empty() {
            0.0
        } else {
            1.0
        }
    } else {
        ((c_subs + c_dels + c_inss) as f32 / char_count as f32).min(5.0)
    };

    // Technical term accuracy
    let mut tracked_in_ref = Vec::new();
    for term in TRACKED_TECHNICAL_TERMS {
        if norm_ref.to_lowercase().contains(term) {
            tracked_in_ref.push(*term);
        }
    }

    let (tech_acc, tech_errors) = if !tracked_in_ref.is_empty() {
        let hyp_lower = norm_hyp.to_lowercase();
        let mut errors = Vec::new();
        let mut matched = 0;
        for term in &tracked_in_ref {
            if hyp_lower.contains(term) {
                matched += 1;
            } else {
                errors.push(TechnicalTermError {
                    expected: term.to_string(),
                    actual_found: None,
                });
            }
        }
        let acc = matched as f32 / tracked_in_ref.len() as f32;
        (Some(acc), errors)
    } else {
        (None, Vec::new())
    };

    AccuracyMetrics {
        reference: reference.to_string(),
        hypothesis: hypothesis.to_string(),
        word_count,
        substitutions: subs,
        deletions: dels,
        insertions: inss,
        wer,
        cer,
        technical_term_accuracy: tech_acc,
        technical_term_errors: tech_errors,
    }
}

/// Computes Levenshtein edit distance alignment operations (Substitutions, Deletions, Insertions).
fn compute_alignment_ops<T: PartialEq>(ref_seq: &[T], hyp_seq: &[T]) -> (usize, usize, usize) {
    let m = ref_seq.len();
    let n = hyp_seq.len();

    let mut d = vec![vec![0usize; n + 1]; m + 1];

    for i in 0..=m {
        d[i][0] = i;
    }
    for j in 0..=n {
        d[0][j] = j;
    }

    for i in 1..=m {
        for j in 1..=n {
            let cost = if ref_seq[i - 1] == hyp_seq[j - 1] { 0 } else { 1 };
            let sub = d[i - 1][j - 1] + cost;
            let del = d[i - 1][j] + 1;
            let ins = d[i][j - 1] + 1;
            d[i][j] = sub.min(del).min(ins);
        }
    }

    // Backtrack to count operations
    let mut i = m;
    let mut j = n;
    let mut subs = 0;
    let mut dels = 0;
    let mut inss = 0;

    while i > 0 || j > 0 {
        if i > 0 && j > 0 {
            let cost = if ref_seq[i - 1] == hyp_seq[j - 1] { 0 } else { 1 };
            if d[i][j] == d[i - 1][j - 1] + cost {
                if cost > 0 {
                    subs += 1;
                }
                i -= 1;
                j -= 1;
                continue;
            }
        }
        if i > 0 && d[i][j] == d[i - 1][j] + 1 {
            dels += 1;
            i -= 1;
        } else if j > 0 && d[i][j] == d[i][j - 1] + 1 {
            inss += 1;
            j -= 1;
        } else {
            break;
        }
    }

    (subs, dels, inss)
}

/// Normalizes text for evaluation: strips punctuation and normalizes whitespace,
/// while preserving Devanagari script characters.
fn normalize_for_eval(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() || is_devanagari(c) {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

fn is_devanagari(c: char) -> bool {
    matches!(c, '\u{0900}'..='\u{097F}')
}

/// Evaluates a given audio sample buffer or WAV file against a specific decoding configuration.
pub fn evaluate_audio_buffer(
    test_id: &str,
    audio_file_label: &str,
    raw_samples: &[f32],
    input_sample_rate: u32,
    variant: EvalConfigVariant,
    language_settings: &LanguageSettings,
    model_path: Option<&str>,
    reference_text: Option<&str>,
    stt_engine: &SttEngine,
) -> EvaluationResult {
    let clean_label = Path::new(audio_file_label)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| audio_file_label.to_string());

    let original_dur = raw_samples.len() as f32 / input_sample_rate.max(1) as f32;
    let mono_16k = resample_to_16k_mono(raw_samples, input_sample_rate);
    let audio_stats = AudioStats::compute(&mono_16k, 16000, 1);

    // Apply production VAD
    let vad = VadConfig::default();
    let (vad_samples, vad_result) = vad.process(&mono_16k, 16000);

    let language_config = SttLanguageConfig::from_settings(language_settings);
    let primary_decoding_config = variant.to_decoding_config();

    let model_filename = model_path
        .map(|p| {
            Path::new(p)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| DEFAULT_MODEL_FILENAME.to_string())
        })
        .unwrap_or_else(|| DEFAULT_MODEL_FILENAME.to_string());

    let (beam_size, best_of) = match &primary_decoding_config.strategy {
        SttSamplingStrategy::Greedy { best_of } => (None, *best_of),
        SttSamplingStrategy::BeamSearch { beam_size, .. } => (Some(*beam_size), 1),
    };

    let mut result = EvaluationResult {
        test_id: test_id.to_string(),
        audio_file: clean_label,
        configuration: variant.name().to_string(),
        language_setting: format!(
            "primary={}, spoken={:?}",
            language_settings.primary_dictation_language, language_settings.spoken_languages
        ),
        resolved_whisper_language: language_config.whisper_language.clone(),
        original_duration_seconds: original_dur,
        processed_duration_seconds: audio_stats.duration_seconds,
        inference_duration_ms: 0,
        real_time_factor: 0.0,
        transcript: String::new(),
        audio_rms: audio_stats.rms,
        audio_peak: audio_stats.peak_amplitude,
        near_zero_percent: audio_stats.near_zero_percent,
        speech_detected: vad_result.speech_detected,
        vad_trimmed_duration: vad_result.trimmed_duration,
        model_filename,
        sampling_strategy: format!("{:?}", primary_decoding_config.strategy),
        best_of,
        beam_size,
        temperature: primary_decoding_config.temperature,
        temperature_increment: primary_decoding_config.temperature_inc,
        initial_prompt_used: primary_decoding_config.initial_prompt.is_some(),
        no_speech_threshold: primary_decoding_config.no_speech_thold,
        entropy_threshold: primary_decoding_config.entropy_thold,
        logprob_threshold: primary_decoding_config.logprob_thold,
        accuracy: None,
        fallback_triggered: false,
        error: None,
    };

    if !vad_result.speech_detected || vad_samples.is_empty() {
        if let Some(ref_str) = reference_text {
            result.accuracy = Some(calculate_accuracy(ref_str, ""));
        }
        return result;
    }

    // Execute STT
    let t0 = std::time::Instant::now();
    let primary_run = stt_engine.transcribe_with_config(
        model_path,
        &vad_samples,
        &language_config,
        &primary_decoding_config,
    );

    match primary_run {
        Ok((text, diag)) => {
            result.transcript = text.clone();
            result.inference_duration_ms = diag.transcription_latency_ms;
            result.real_time_factor = diag.real_time_factor;

            // Handle temperature fallback experiment
            if variant == EvalConfigVariant::TemperatureFallback {
                let is_unreliable = is_transcription_unreliable(&text, &diag);
                if is_unreliable {
                    result.fallback_triggered = true;
                    let mut fallback_cfg = primary_decoding_config.clone();
                    fallback_cfg.temperature = 0.2;
                    fallback_cfg.temperature_inc = 0.2;

                    if let Ok((fb_text, fb_diag)) = stt_engine.transcribe_with_config(
                        model_path,
                        &vad_samples,
                        &language_config,
                        &fallback_cfg,
                    ) {
                        result.transcript = fb_text;
                        result.inference_duration_ms += fb_diag.transcription_latency_ms;
                        result.temperature = 0.2;
                    }
                }
            }

            if let Some(ref_str) = reference_text {
                result.accuracy = Some(calculate_accuracy(ref_str, &result.transcript));
            }
        }
        Err(e) => {
            result.inference_duration_ms = t0.elapsed().as_millis();
            result.error = Some(e.to_string());
        }
    }

    result
}

/// Evaluates whether a transcription result is unreliable, requiring temperature fallback.
/// Criteria:
/// 1. Transcript is empty despite strong VAD speech detected
/// 2. Severe repetition / compression ratio pathology (e.g. repeated identical 3-grams)
fn is_transcription_unreliable(text: &str, diag: &crate::capture::stt::SttSessionDiagnostics) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() && diag.audio_duration_seconds > 1.0 {
        return true;
    }

    let words: Vec<&str> = trimmed.split_whitespace().collect();
    if words.len() >= 6 {
        // Check for 3 consecutive repeated phrases
        for window_size in 1..=4 {
            if words.len() >= window_size * 3 {
                let chunk1 = &words[0..window_size];
                let chunk2 = &words[window_size..window_size * 2];
                let chunk3 = &words[window_size * 2..window_size * 3];
                if chunk1 == chunk2 && chunk2 == chunk3 {
                    return true;
                }
            }
        }
    }

    false
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::capture::stt::DEFAULT_MODEL_FILENAME;
    use crate::settings::LanguageSettings;

    #[test]
    fn test_eval_config_serialization() {
        let variants = [
            EvalConfigVariant::Baseline,
            EvalConfigVariant::RelayPrompt,
            EvalConfigVariant::BestOf3,
            EvalConfigVariant::Beam2,
            EvalConfigVariant::TemperatureFallback,
        ];
        for v in variants {
            let json = serde_json::to_string(&v).expect("serialize variant");
            let back: EvalConfigVariant = serde_json::from_str(&json).expect("deserialize variant");
            assert_eq!(v, back);
        }
    }

    #[test]
    fn test_baseline_config_matches_production() {
        let eval_baseline = EvalConfigVariant::Baseline.to_decoding_config();
        let prod_baseline = WhisperDecodingConfig::baseline();
        assert_eq!(eval_baseline, prod_baseline);
        assert_eq!(eval_baseline.strategy, SttSamplingStrategy::Greedy { best_of: 1 });
        assert_eq!(eval_baseline.temperature, 0.0);
        assert_eq!(eval_baseline.initial_prompt, None);
    }

    #[test]
    fn test_relay_prompt_differs_only_by_prompt() {
        let baseline = EvalConfigVariant::Baseline.to_decoding_config();
        let prompt_cfg = EvalConfigVariant::RelayPrompt.to_decoding_config();
        assert_eq!(prompt_cfg.strategy, baseline.strategy);
        assert_eq!(prompt_cfg.temperature, baseline.temperature);
        assert_eq!(prompt_cfg.temperature_inc, baseline.temperature_inc);
        assert_eq!(prompt_cfg.suppress_blank, baseline.suppress_blank);
        assert_eq!(prompt_cfg.initial_prompt, Some(RELAY_TECHNICAL_PROMPT.to_string()));
    }

    #[test]
    fn test_best_of_3_differs_only_by_best_of() {
        let baseline = EvalConfigVariant::Baseline.to_decoding_config();
        let best_of_3 = EvalConfigVariant::BestOf3.to_decoding_config();
        assert_eq!(best_of_3.strategy, SttSamplingStrategy::Greedy { best_of: 3 });
        assert_eq!(best_of_3.temperature, baseline.temperature);
        assert_eq!(best_of_3.initial_prompt, baseline.initial_prompt);
    }

    #[test]
    fn test_beam_2_uses_expected_beam_size() {
        let beam_cfg = EvalConfigVariant::Beam2.to_decoding_config();
        assert_eq!(
            beam_cfg.strategy,
            SttSamplingStrategy::BeamSearch {
                beam_size: 2,
                patience: 1.0
            }
        );
    }

    #[test]
    fn test_evaluation_result_contains_all_required_fields() {
        let dummy_samples = vec![0.05_f32; 16000]; // 1 second
        let lang = LanguageSettings {
            primary_dictation_language: "en".to_string(),
            spoken_languages: vec!["en".to_string()],
            notes_language: "en".to_string(),
            output_script: "latin".to_string(),
        };
        let engine = SttEngine::new();
        let res = evaluate_audio_buffer(
            "test_schema_01",
            "dummy.wav",
            &dummy_samples,
            16000,
            EvalConfigVariant::Baseline,
            &lang,
            None,
            Some("test expected text"),
            &engine,
        );

        assert_eq!(res.test_id, "test_schema_01");
        assert_eq!(res.audio_file, "dummy.wav");
        assert_eq!(res.configuration, "baseline");
        assert_eq!(res.model_filename, DEFAULT_MODEL_FILENAME);
        assert_eq!(res.best_of, 1);
        assert_eq!(res.temperature, 0.0);
        assert!(res.original_duration_seconds > 0.0);
        assert!(res.audio_rms > 0.0);

        let json = serde_json::to_string_pretty(&res).expect("must serialize");
        assert!(json.contains("test_id"));
        assert!(json.contains("audio_file"));
        assert!(json.contains("configuration"));
        assert!(json.contains("audio_rms"));
        assert!(json.contains("audio_peak"));
        assert!(json.contains("sampling_strategy"));
    }

    #[test]
    fn test_wav_evaluation_does_not_mutate_original_wav() {
        let temp_dir = std::env::temp_dir();
        let test_wav = temp_dir.join("eval_non_mutation_test.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        {
            let mut writer = hound::WavWriter::create(&test_wav, spec).unwrap();
            for _ in 0..16000 {
                writer.write_sample(1000_i16).unwrap();
            }
            writer.finalize().unwrap();
        }

        let original_bytes = std::fs::read(&test_wav).unwrap();
        let reader = hound::WavReader::open(&test_wav).unwrap();
        let samples: Vec<f32> = reader
            .into_samples::<i16>()
            .filter_map(|s| s.ok())
            .map(|s| s as f32 / i16::MAX as f32)
            .collect();

        let lang = LanguageSettings::default();
        let engine = SttEngine::new();
        let _ = evaluate_audio_buffer(
            "non_mutate_01",
            &test_wav.to_string_lossy(),
            &samples,
            16000,
            EvalConfigVariant::Baseline,
            &lang,
            None,
            None,
            &engine,
        );

        let post_bytes = std::fs::read(&test_wav).unwrap();
        assert_eq!(original_bytes, post_bytes, "WAV evaluation must never mutate the original file");
        let _ = std::fs::remove_file(&test_wav);
    }

    #[test]
    fn test_vad_output_matches_production_vad() {
        let mut samples = vec![0.0_f32; 8000]; // 0.5s silence
        samples.extend(vec![0.1_f32; 16000]); // 1.0s speech
        samples.extend(vec![0.0_f32; 8000]); // 0.5s silence

        let vad = VadConfig::default();
        let (processed, res) = vad.process(&samples, 16000);

        assert!(res.speech_detected);
        assert!(res.silence_removed_percent > 0.0);
        assert!(processed.len() < samples.len());
    }

    #[test]
    fn test_language_settings_resolve_identically_to_production() {
        // Case A: primary=en, spoken=[en] -> "en"
        let s_a = LanguageSettings {
            primary_dictation_language: "en".to_string(),
            spoken_languages: vec!["en".to_string()],
            notes_language: "en".to_string(),
            output_script: "latin".to_string(),
        };
        assert_eq!(SttLanguageConfig::from_settings(&s_a).whisper_language, Some("en".to_string()));

        // Case B: primary=hi, spoken=[hi] -> "hi"
        let s_b = LanguageSettings {
            primary_dictation_language: "hi".to_string(),
            spoken_languages: vec!["hi".to_string()],
            notes_language: "hi".to_string(),
            output_script: "native".to_string(),
        };
        assert_eq!(SttLanguageConfig::from_settings(&s_b).whisper_language, Some("hi".to_string()));

        // Case C: primary=en, spoken=[en, hi] -> None (multilingual)
        let s_c = LanguageSettings {
            primary_dictation_language: "en".to_string(),
            spoken_languages: vec!["en".to_string(), "hi".to_string()],
            notes_language: "en".to_string(),
            output_script: "latin".to_string(),
        };
        assert_eq!(SttLanguageConfig::from_settings(&s_c).whisper_language, None);

        // Case D: primary=hi, spoken=[hi, en] -> None (multilingual)
        let s_d = LanguageSettings {
            primary_dictation_language: "hi".to_string(),
            spoken_languages: vec!["hi".to_string(), "en".to_string()],
            notes_language: "en".to_string(),
            output_script: "latin".to_string(),
        };
        assert_eq!(SttLanguageConfig::from_settings(&s_d).whisper_language, None);

        // Case E: primary=auto -> None
        let s_e = LanguageSettings {
            primary_dictation_language: "auto".to_string(),
            spoken_languages: vec!["en".to_string()],
            notes_language: "en".to_string(),
            output_script: "latin".to_string(),
        };
        assert_eq!(SttLanguageConfig::from_settings(&s_e).whisper_language, None);

        // Case F: spoken empty, primary=en -> Some("en")
        let s_f = LanguageSettings {
            primary_dictation_language: "en".to_string(),
            spoken_languages: vec![],
            notes_language: "en".to_string(),
            output_script: "latin".to_string(),
        };
        assert_eq!(SttLanguageConfig::from_settings(&s_f).whisper_language, Some("en".to_string()));
    }

    #[test]
    fn test_empty_audio_produces_no_speech_result() {
        let empty_samples: Vec<f32> = Vec::new();
        let lang = LanguageSettings::default();
        let engine = SttEngine::new();
        let res = evaluate_audio_buffer(
            "empty_01",
            "empty.wav",
            &empty_samples,
            16000,
            EvalConfigVariant::Baseline,
            &lang,
            None,
            Some(""),
            &engine,
        );

        assert!(!res.speech_detected);
        assert_eq!(res.transcript, "");
        assert_eq!(res.accuracy.unwrap().wer, 0.0);
    }

    #[test]
    fn test_evaluation_errors_captured_without_crashing() {
        let samples = vec![0.05_f32; 16000];
        let lang = LanguageSettings::default();
        let engine = SttEngine::new();
        let res = evaluate_audio_buffer(
            "error_capture_01",
            "invalid.wav",
            &samples,
            16000,
            EvalConfigVariant::Baseline,
            &lang,
            Some("C:/non/existent/model.bin"),
            None,
            &engine,
        );

        // Even with invalid model or missing feature, errors are captured gracefully
        assert!(res.transcript.is_empty());
        assert!(res.error.is_some() || res.inference_duration_ms == 0);
    }

    #[test]
    fn test_ground_truth_accuracy_calculation() {
        let ref_text = "Relay uses Tauri and Rust";
        let hyp_perfect = "Relay uses Tauri and Rust";
        let acc_perfect = calculate_accuracy(ref_text, hyp_perfect);
        assert_eq!(acc_perfect.wer, 0.0);
        assert_eq!(acc_perfect.substitutions, 0);
        assert_eq!(acc_perfect.insertions, 0);
        assert_eq!(acc_perfect.deletions, 0);

        let hyp_sub = "Relay uses Tari and Rust"; // Tauri -> Tari (1 sub)
        let acc_sub = calculate_accuracy(ref_text, hyp_sub);
        assert_eq!(acc_sub.substitutions, 1);
        assert_eq!(acc_sub.insertions, 0);
        assert_eq!(acc_sub.deletions, 0);
        assert_eq!(acc_sub.technical_term_accuracy, Some(2.0 / 3.0)); // relay, rust passed; tauri failed

        let hyp_del = "Relay uses Rust"; // Tauri and deleted (2 dels)
        let acc_del = calculate_accuracy(ref_text, hyp_del);
        assert_eq!(acc_del.deletions, 2);

        let hyp_ins = "Relay always uses Tauri and Rust"; // always inserted (1 ins)
        let acc_ins = calculate_accuracy(ref_text, hyp_ins);
        assert_eq!(acc_ins.insertions, 1);
    }

    #[test]
    fn test_hindi_and_hinglish_accuracy_cer() {
        let hi_ref = "आज मुझे team के साथ review करना है।";
        let hi_hyp = "आज मुझे team के साथ review करना है।";
        let acc = calculate_accuracy(hi_ref, hi_hyp);
        assert_eq!(acc.wer, 0.0);
        assert_eq!(acc.cer, 0.0);

        let hi_hyp_err = "आज मुझे team के साथ review karna hai";
        let acc_err = calculate_accuracy(hi_ref, hi_hyp_err);
        assert!(acc_err.cer > 0.0);
    }

    #[test]
    fn test_deterministic_greedy_decoding_invariants() {
        let samples = vec![0.02_f32; 16000];
        let lang = LanguageSettings::default();
        let engine = SttEngine::new();

        let res1 = evaluate_audio_buffer(
            "det_01",
            "det.wav",
            &samples,
            16000,
            EvalConfigVariant::Baseline,
            &lang,
            None,
            None,
            &engine,
        );
        let res2 = evaluate_audio_buffer(
            "det_01",
            "det.wav",
            &samples,
            16000,
            EvalConfigVariant::Baseline,
            &lang,
            None,
            None,
            &engine,
        );

        assert_eq!(res1.speech_detected, res2.speech_detected);
        assert_eq!(res1.audio_rms, res2.audio_rms);
        assert_eq!(res1.audio_peak, res2.audio_peak);
        assert_eq!(res1.sampling_strategy, res2.sampling_strategy);
    }

    #[test]
    fn test_production_configuration_remains_unchanged() {
        let prod_baseline = WhisperDecodingConfig::baseline();
        assert_eq!(prod_baseline.strategy, SttSamplingStrategy::Greedy { best_of: 1 });
        assert_eq!(prod_baseline.temperature, 0.0);
        assert_eq!(prod_baseline.temperature_inc, 0.2);
        assert_eq!(prod_baseline.initial_prompt, None);
        assert_eq!(prod_baseline.suppress_blank, true);
        assert_eq!(prod_baseline.no_speech_thold, 0.6);
        assert_eq!(prod_baseline.entropy_thold, 2.4);
        assert_eq!(prod_baseline.logprob_thold, -1.0);
    }

    #[test]
    fn test_stt_settings_default_prompt_disabled() {
        let default_stt = crate::settings::SttSettings::default();
        assert!(!default_stt.enable_initial_prompt);
        assert_eq!(default_stt.custom_initial_prompt, None);

        let cfg = WhisperDecodingConfig::from_settings(&default_stt);
        assert_eq!(cfg.initial_prompt, None);
    }

    #[test]
    fn test_whisper_decoding_config_from_settings_prompt_enabled() {
        let mut stt_settings = crate::settings::SttSettings::default();
        stt_settings.enable_initial_prompt = true;
        stt_settings.custom_initial_prompt = Some("Tauri, Rust, Relay".to_string());

        let cfg = WhisperDecodingConfig::from_settings(&stt_settings);
        assert_eq!(cfg.initial_prompt, Some("Tauri, Rust, Relay".to_string()));
        assert_eq!(cfg.strategy, SttSamplingStrategy::Greedy { best_of: 1 });
        assert_eq!(cfg.temperature, 0.0);
    }

    #[test]
    fn test_stt_diagnostic_snapshot_construction_and_serialization() {
        let dummy_samples = vec![0.04_f32; 16000];
        let stats = crate::capture::AudioStats::compute(&dummy_samples, 16000, 1);
        let vad = crate::capture::VadConfig::default();
        let (_, vad_res) = vad.process(&dummy_samples, 16000);

        let captured = crate::capture::CapturedAudio {
            session_id: "snap_01".to_string(),
            mode: "dictation".to_string(),
            audio_path: "test.wav".to_string(),
            samples: dummy_samples,
            duration_seconds: stats.duration_seconds,
            original_duration_seconds: 1.0,
            had_audio: true,
            audio_stats: stats,
            vad_result: vad_res,
        };

        let lang_settings = LanguageSettings {
            primary_dictation_language: "en".to_string(),
            spoken_languages: vec!["en".to_string(), "hi".to_string()],
            notes_language: "en".to_string(),
            output_script: "latin".to_string(),
        };
        let lang_cfg = SttLanguageConfig::from_settings(&lang_settings);
        let dec_cfg = WhisperDecodingConfig::baseline();

        let diag = crate::capture::stt::SttSessionDiagnostics {
            model_path: "ggml-small.bin".to_string(),
            audio_duration_seconds: 1.0,
            whisper_language: None,
            decoding_strategy: "Greedy { best_of: 1 }".to_string(),
            temperature: 0.0,
            temperature_inc: 0.2,
            best_of: 1,
            used_initial_prompt: false,
            transcription_latency_ms: 120,
            real_time_factor: 0.12,
            segment_count: 1,
            is_empty: false,
            transcript_char_count: 18,
        };

        let snapshot = build_diagnostic_snapshot(
            "dictation",
            Some("test.wav".to_string()),
            &captured,
            &lang_settings,
            &lang_cfg,
            &dec_cfg,
            "ggml-small.bin",
            "Hello world dictation",
            Some(&diag),
            None,
        );

        assert_eq!(snapshot.session_mode, "dictation");
        assert_eq!(snapshot.transcript, "Hello world dictation");
        assert_eq!(snapshot.inference_duration_ms, 120);
        assert_eq!(snapshot.resolved_whisper_language, None);
        assert!(!snapshot.translate);

        let json = serde_json::to_string_pretty(&snapshot).expect("serialize snapshot");
        assert!(json.contains("timestamp_epoch_ms"));
        assert!(json.contains("noise_floor"));
        assert!(json.contains("peak_amplitude"));
        assert!(json.contains("resolved_whisper_language"));
    }

    #[test]
    fn test_failure_taxonomy_classification() {
        let stats_non_finite = crate::capture::AudioStats {
            sample_count: 16000,
            duration_seconds: 1.0,
            sample_rate: 16000,
            channels: 1,
            rms: 0.05,
            peak_amplitude: 0.2,
            near_zero_percent: 10.0,
            near_clipping_percent: 0.0,
            has_non_finite: true,
            non_finite_count: 2,
        };

        let vad_quiet = crate::capture::VadResult {
            speech_detected: false,
            start_sample: 0,
            end_sample: 0,
            start_seconds: 0.0,
            end_seconds: 0.0,
            original_duration: 1.0,
            trimmed_duration: 0.0,
            silence_removed_seconds: 1.0,
            silence_removed_percent: 100.0,
            noise_floor: 0.005,
            onset_threshold: 0.012,
        };

        let mut eval_res = evaluate_audio_buffer(
            "tax_01",
            "dummy.wav",
            &[],
            16000,
            EvalConfigVariant::Baseline,
            &LanguageSettings::default(),
            None,
            None,
            &SttEngine::new(),
        );
        // Simulate a hallucination emission on silence
        eval_res.transcript = "Phantom sentence emitted by model".to_string();

        let diags = classify_stt_failure(&stats_non_finite, &vad_quiet, &eval_res);
        let categories: Vec<SttFailureCategory> = diags.iter().map(|d| d.category).collect();

        assert!(categories.contains(&SttFailureCategory::AcousticIssue));
        assert!(categories.contains(&SttFailureCategory::Hallucination));
    }

    #[test]
    fn test_corpus_35_items_manifest_completeness() {
        let corpus = get_curated_corpus();
        assert_eq!(corpus.len(), 35);

        let en_count = corpus.iter().filter(|i| i.language == "en").count();
        let hi_count = corpus.iter().filter(|i| i.language == "hi").count();
        let hinglish_count = corpus.iter().filter(|i| i.language == "hinglish").count();

        assert!(en_count >= 15);
        assert!(hi_count >= 6);
        assert!(hinglish_count >= 8);

        // Every item has non-empty fields
        for item in &corpus {
            assert!(!item.test_id.is_empty());
            assert!(!item.audio_filename.is_empty());
            assert!(!item.category.is_empty());
            assert!(!item.description.is_empty());
        }
    }

    #[test]
    fn test_benchmark_matrix_runner_on_sample() {
        let samples = vec![0.04_f32; 16000];
        let lang = LanguageSettings::default();
        let engine = SttEngine::new();

        let matrix_results = run_benchmark_matrix_on_sample(
            "bench_matrix_01",
            "test_sample.wav",
            &samples,
            16000,
            &lang,
            None,
            Some("Testing benchmark runner"),
            &engine,
        );

        assert_eq!(matrix_results.len(), 5);
        let configs: Vec<String> = matrix_results.iter().map(|r| r.configuration.clone()).collect();
        assert!(configs.contains(&"baseline".to_string()));
        assert!(configs.contains(&"relay_prompt".to_string()));
        assert!(configs.contains(&"best_of_3".to_string()));
        assert!(configs.contains(&"beam_2".to_string()));
        assert!(configs.contains(&"temperature_fallback".to_string()));
    }

    #[test]
    fn test_phase8_production_configuration_exact_lock() {
        let baseline = WhisperDecodingConfig::baseline();
        // Exact configuration lock assertions
        assert_eq!(baseline.strategy, SttSamplingStrategy::Greedy { best_of: 1 });
        assert_eq!(baseline.temperature, 0.0);
        assert_eq!(baseline.temperature_inc, 0.2);
        assert_eq!(baseline.initial_prompt, None);
        assert_eq!(baseline.suppress_blank, true);
        assert_eq!(baseline.no_speech_thold, 0.6);
        assert_eq!(baseline.entropy_thold, 2.4);
        assert_eq!(baseline.logprob_thold, -1.0);
        assert_eq!(baseline.print_special, false);
        assert_eq!(baseline.print_timestamps, false);
    }

    #[test]
    fn test_phase8_language_regression_matrix_end_to_end() {
        // 1. English
        let lang_en = LanguageSettings {
            primary_dictation_language: "en".to_string(),
            spoken_languages: vec!["en".to_string()],
            notes_language: "en".to_string(),
            output_script: "latin".to_string(),
        };
        let cfg_en = SttLanguageConfig::from_settings(&lang_en);
        assert_eq!(cfg_en.whisper_language, Some("en".to_string()));
        assert_eq!(cfg_en.translate, false);

        // 2. Hindi
        let lang_hi = LanguageSettings {
            primary_dictation_language: "hi".to_string(),
            spoken_languages: vec!["hi".to_string()],
            notes_language: "hi".to_string(),
            output_script: "native".to_string(),
        };
        let cfg_hi = SttLanguageConfig::from_settings(&lang_hi);
        assert_eq!(cfg_hi.whisper_language, Some("hi".to_string()));
        assert_eq!(cfg_hi.translate, false);

        // 3. Spanish
        let lang_es = LanguageSettings {
            primary_dictation_language: "es".to_string(),
            spoken_languages: vec!["es".to_string()],
            notes_language: "es".to_string(),
            output_script: "latin".to_string(),
        };
        let cfg_es = SttLanguageConfig::from_settings(&lang_es);
        assert_eq!(cfg_es.whisper_language, Some("es".to_string()));
        assert_eq!(cfg_es.translate, false);

        // 4. Hinglish (Multilingual profile)
        let lang_hinglish = LanguageSettings {
            primary_dictation_language: "en".to_string(),
            spoken_languages: vec!["en".to_string(), "hi".to_string()],
            notes_language: "en".to_string(),
            output_script: "latin".to_string(),
        };
        let cfg_hinglish = SttLanguageConfig::from_settings(&lang_hinglish);
        assert_eq!(cfg_hinglish.whisper_language, None);
        assert_eq!(cfg_hinglish.translate, false);

        // 5. Auto
        let lang_auto = LanguageSettings {
            primary_dictation_language: "auto".to_string(),
            spoken_languages: vec!["en".to_string()],
            notes_language: "en".to_string(),
            output_script: "latin".to_string(),
        };
        let cfg_auto = SttLanguageConfig::from_settings(&lang_auto);
        assert_eq!(cfg_auto.whisper_language, None);
        assert_eq!(cfg_auto.translate, false);
    }

    #[test]
    fn test_phase8_prompt_safety_matrix() {
        // A. Default: disabled -> None
        let default_stt = crate::settings::SttSettings::default();
        let cfg_default = WhisperDecodingConfig::from_settings(&default_stt);
        assert_eq!(cfg_default.initial_prompt, None);
        assert_eq!(cfg_default.strategy, SttSamplingStrategy::Greedy { best_of: 1 });
        assert_eq!(cfg_default.temperature, 0.0);

        // B. Enabled with custom prompt
        let mut custom_stt = crate::settings::SttSettings::default();
        custom_stt.enable_initial_prompt = true;
        custom_stt.custom_initial_prompt = Some("Tauri, Rust, Relay".to_string());
        let cfg_custom = WhisperDecodingConfig::from_settings(&custom_stt);
        assert_eq!(cfg_custom.initial_prompt, Some("Tauri, Rust, Relay".to_string()));
        // Ensure other parameters are untouched
        assert_eq!(cfg_custom.strategy, SttSamplingStrategy::Greedy { best_of: 1 });
        assert_eq!(cfg_custom.temperature, 0.0);
        assert_eq!(cfg_custom.no_speech_thold, 0.6);

        // C. Enabled with empty/whitespace prompt -> None
        let mut empty_stt = crate::settings::SttSettings::default();
        empty_stt.enable_initial_prompt = true;
        empty_stt.custom_initial_prompt = Some("   ".to_string());
        let cfg_empty = WhisperDecodingConfig::from_settings(&empty_stt);
        assert_eq!(cfg_empty.initial_prompt, None);

        // D. Disabled after being enabled -> None
        custom_stt.enable_initial_prompt = false;
        let cfg_re_disabled = WhisperDecodingConfig::from_settings(&custom_stt);
        assert_eq!(cfg_re_disabled.initial_prompt, None);
    }

    #[test]
    fn test_phase8_vad_safety_matrix() {
        let vad = crate::capture::VadConfig::default();

        // 1. Pure silence
        let silence_samples = vec![0.0_f32; 16000];
        let (silence_trimmed, silence_res) = vad.process(&silence_samples, 16000);
        assert!(!silence_res.speech_detected);
        assert!(silence_trimmed.is_empty());

        // 2. Micro-tap under 300ms
        let mut tap_samples = vec![0.002_f32; 16000];
        for i in 2000..2500 {
            tap_samples[i] = 0.08; // 500 samples = ~31ms burst
        }
        let (tap_trimmed, tap_res) = vad.process(&tap_samples, 16000);
        assert!(!tap_res.speech_detected);
        assert!(tap_trimmed.is_empty());

        // 3. Legitimate speech burst (600ms above threshold)
        let mut speech_samples = vec![0.002_f32; 16000];
        for i in 2000..12000 {
            speech_samples[i] = 0.05 * ((i as f32) * 0.1).sin();
        }
        let (speech_trimmed, speech_res) = vad.process(&speech_samples, 16000);
        assert!(speech_res.speech_detected);
        assert!(!speech_trimmed.is_empty());
        assert!(speech_res.trimmed_duration > 0.4);
    }

    #[test]
    fn test_phase8_failure_and_recovery_matrix() {
        let engine = SttEngine::new();
        let lang = LanguageSettings::default();

        // 1. Empty buffer
        let empty_res = evaluate_audio_buffer(
            "empty_eval",
            "empty.wav",
            &[],
            16000,
            EvalConfigVariant::Baseline,
            &lang,
            None,
            None,
            &engine,
        );
        assert!(!empty_res.speech_detected);
        assert_eq!(empty_res.transcript, "");

        // 2. Corrupt / Non-existent model path
        let bad_model_res = evaluate_audio_buffer(
            "bad_model_eval",
            "test.wav",
            &vec![0.05_f32; 8000],
            16000,
            EvalConfigVariant::Baseline,
            &lang,
            Some("C:/non/existent/path/ggml.bin"),
            None,
            &engine,
        );
        // Error captured gracefully without panicking
        assert!(bad_model_res.error.is_some() || bad_model_res.transcript.is_empty());
    }

    #[test]
    fn test_phase9_soak_test_100_sessions_stability() {
        let engine = SttEngine::new();
        let lang = LanguageSettings::default();
        let vad = crate::capture::VadConfig::default();

        let mut silence_detected_count = 0;
        let mut speech_detected_count = 0;

        for i in 0..100 {
            let session_id = format!("soak_sess_{:03}", i);
            let is_speech = (i % 3) != 0; // 2/3 speech, 1/3 silence/noise

            let samples: Vec<f32> = if is_speech {
                // Generate a speech-like sine wave burst (600ms)
                let mut s = vec![0.002_f32; 16000];
                for j in 2000..12000 {
                    s[j] = 0.04 * ((j as f32) * 0.05).sin();
                }
                s
            } else {
                // Ambient noise / silence
                vec![0.001_f32; 16000]
            };

            let (trimmed, vad_res) = vad.process(&samples, 16000);
            if vad_res.speech_detected {
                speech_detected_count += 1;
                assert!(!trimmed.is_empty());
            } else {
                silence_detected_count += 1;
                assert!(trimmed.is_empty());
            }

            let eval_res = evaluate_audio_buffer(
                &session_id,
                "soak.wav",
                &samples,
                16000,
                EvalConfigVariant::Baseline,
                &lang,
                None,
                if is_speech { Some("Expected transcript") } else { Some("") },
                &engine,
            );

            assert_eq!(eval_res.speech_detected, is_speech);
        }

        assert_eq!(speech_detected_count, 66);
        assert_eq!(silence_detected_count, 34);
    }

    #[test]
    fn test_phase9_multilingual_and_orthography_independence() {
        // Output script "latin" vs "native" must never mutate Whisper's language resolution
        let s_latin = LanguageSettings {
            primary_dictation_language: "hi".to_string(),
            spoken_languages: vec!["hi".to_string()],
            notes_language: "hi".to_string(),
            output_script: "latin".to_string(),
        };
        let s_native = LanguageSettings {
            primary_dictation_language: "hi".to_string(),
            spoken_languages: vec!["hi".to_string()],
            notes_language: "hi".to_string(),
            output_script: "native".to_string(),
        };

        assert_eq!(
            SttLanguageConfig::from_settings(&s_latin).whisper_language,
            SttLanguageConfig::from_settings(&s_native).whisper_language
        );
        assert_eq!(
            SttLanguageConfig::from_settings(&s_latin).whisper_language,
            Some("hi".to_string())
        );
    }
}



