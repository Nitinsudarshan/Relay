pub mod evaluation;
pub mod stt;

use crate::sync::MutexExt;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::mpsc as std_mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;

use tauri::{AppHandle, Emitter};

pub use evaluation::{
    build_diagnostic_snapshot, calculate_accuracy, classify_stt_failure, evaluate_audio_buffer,
    get_curated_corpus, run_benchmark_matrix_on_sample, AccuracyMetrics, BenchmarkAggregate,
    BenchmarkReport, CorpusItem, EvalConfigVariant, EvaluationResult, SttDiagnosticSnapshot,
    SttFailureCategory, SttFailureDiagnostic,
};
pub use stt::{SttEngine, SttError, SttLanguageConfig};

const TARGET_SAMPLE_RATE: u32 = 16_000;

/// Absolute floor below which nothing ever counts as speech, regardless of
/// how the adaptive noise floor below calibrates — guards against a
/// pathological calibration (e.g. a session that starts in near-total
/// silence, floor near 0) making the gate too sensitive.
const AUDIO_DETECTED_THRESHOLD: f32 = 0.02;

/// How long the mic must be measurably above the *effective* threshold
/// (see [`NOISE_FLOOR_SPEECH_MARGIN`]) — cumulatively, not in one burst —
/// before a session counts as `had_audio`. A single loud callback isn't
/// proof of speech: real speech sustains energy across a syllable, a noise
/// spike (a keystroke, a chair creak) doesn't.
const AUDIO_DETECTED_MIN_DURATION_MS: u64 = 200;

/// How much of the start of a session is spent purely measuring the
/// ambient noise floor (fan noise, room hum, mic self-noise, Windows
/// "microphone enhancement" processing) before any speech detection runs at
/// all. A *fixed* absolute RMS threshold can't tell "the room is just noisy"
/// apart from "someone is speaking" — a noisy-but-silent room can easily
/// sit continuously above a static threshold, which would defeat the whole
/// had_audio gate. Calibrating per-session and requiring speech to clear
/// the *measured* floor by a margin (below) is what actually distinguishes
/// the two.
const NOISE_FLOOR_CALIBRATION_MS: u64 = 300;

/// How far above the calibrated noise floor a chunk must be to count as
/// (possible) speech rather than ambient noise.
const NOISE_FLOOR_SPEECH_MARGIN: f32 = 0.035;

#[derive(Error, Debug)]
pub enum CaptureError {
    #[error("Audio capture device error: {0}")]
    DeviceError(String),

    #[error("IO error handling WAV file: {0}")]
    IoError(#[from] std::io::Error),

    #[error("WAV encoder error: {0}")]
    WavError(String),

    #[error("No active recording session")]
    NoActiveSession,

    #[error("A recording session is already active")]
    SessionAlreadyActive,
}

// TEMP: dictation latency instrumentation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CaptureTimingMetrics {
    pub thread_stop_ms: u128,
    pub resample_ms: u128,
    pub vad_ms: u128,
    pub wav_write_ms: u128,
    pub total_stop_ms: u128,
}

/// Raw captured audio, resampled to 16kHz mono, ready to hand to an STT engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedAudio {
    pub session_id: String,
    pub mode: String,
    pub audio_path: String,
    #[serde(skip)]
    pub samples: Vec<f32>,
    pub duration_seconds: f32,
    pub original_duration_seconds: f32,
    /// Whether the session measured speech-level energy sustained above the
    /// calibrated ambient noise floor — the authoritative "did the user
    /// actually say something" signal. `recordingStarted` never implies
    /// this; it is only ever set by real, sustained, above-ambient input
    /// arriving during capture. Callers must treat a session with
    /// `had_audio: false` as having nothing to transcribe.
    pub had_audio: bool,
    pub audio_stats: AudioStats,
    pub vad_result: VadResult,
    // TEMP: dictation latency instrumentation
    #[serde(default)]
    pub timing_metrics: Option<CaptureTimingMetrics>,
}

/// Per-session state for telling speech apart from ambient noise: spends
/// the first [`NOISE_FLOOR_CALIBRATION_MS`] purely measuring the room/mic's
/// baseline level, then only counts a chunk towards `had_audio` once it
/// clears that measured floor by [`NOISE_FLOOR_SPEECH_MARGIN`] — a fixed
/// absolute threshold can't otherwise tell a sustained-but-silent noisy
/// room apart from someone actually speaking.
#[derive(Debug, Default)]
struct AudioDetectionState {
    /// Duration-weighted sum of levels seen during calibration (level × how
    /// many sample-frames it covered), so callbacks with different buffer
    /// sizes contribute proportionally to the eventual average.
    calibration_level_duration_sum: f64,
    calibration_frames: u32,
    calibration_done: bool,
    noise_floor: f32,
    frames_above_threshold: u32,
}

pub struct AudioRecorder {
    active_session: Arc<Mutex<Option<ActiveSession>>>,
}

struct ActiveSession {
    session_id: String,
    mode: String,
    file_path: PathBuf,
    start_time: std::time::Instant,
    stop_tx: std_mpsc::Sender<()>,
    done_rx: std_mpsc::Receiver<Result<CaptureThreadResult, String>>,
}

struct CaptureThreadResult {
    samples: Vec<f32>,
    input_rate: u32,
    had_audio: bool,
}

impl Default for AudioRecorder {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            active_session: Arc::new(Mutex::new(None)),
        }
    }

    pub fn is_active(&self) -> bool {
        self.active_session.lock_or_recover().is_some()
    }

    /// The `mode` of the in-progress session, if any — lets callers tell a
    /// hotkey-owned ("dictation") session apart from a UI-owned one
    /// ("meeting"/"scribble"/"chat") without a separate ownership field.
    pub fn active_mode(&self) -> Option<String> {
        self.active_session
            .lock_or_recover()
            .as_ref()
            .map(|s| s.mode.clone())
    }

    pub fn start(
        &self,
        mode: &str,
        output_dir: &Path,
        app: Option<AppHandle>,
    ) -> Result<String, CaptureError> {
        let mut session = self.active_session.lock_or_recover();
        if session.is_some() {
            return Err(CaptureError::SessionAlreadyActive);
        }

        std::fs::create_dir_all(output_dir)?;
        let session_id = uuid::Uuid::new_v4().to_string();
        let file_name = format!("{}_{}.wav", mode, session_id);
        let file_path = output_dir.join(file_name);

        let (stop_tx, stop_rx) = std_mpsc::channel();
        let (done_tx, done_rx) = std_mpsc::channel();
        spawn_capture_thread(stop_rx, done_tx, app);

        *session = Some(ActiveSession {
            session_id: session_id.clone(),
            mode: mode.to_string(),
            file_path,
            start_time: std::time::Instant::now(),
            stop_tx,
            done_rx,
        });

        tracing::info!(
            "Started audio capture session {} in mode {}",
            session_id,
            mode
        );
        Ok(session_id)
    }

    pub async fn stop(&self) -> Result<CapturedAudio, CaptureError> {
        // TEMP: dictation latency instrumentation
        let t_stop_start = std::time::Instant::now();

        let session = {
            let mut guard = self.active_session.lock_or_recover();
            guard.take().ok_or(CaptureError::NoActiveSession)?
        };

        let duration = session.start_time.elapsed().as_secs_f32();
        let _ = session.stop_tx.send(());

        let capture_result = session
            .done_rx
            .recv_timeout(Duration::from_secs(10))
            .map_err(|e| {
                CaptureError::DeviceError(format!("capture thread did not respond: {}", e))
            })?
            .map_err(CaptureError::DeviceError)?;

        // TEMP: dictation latency instrumentation
        let t_thread_done = std::time::Instant::now();

        let mono_16k = resample_to_16k_mono(&capture_result.samples, capture_result.input_rate);

        // TEMP: dictation latency instrumentation
        let t_resampled = std::time::Instant::now();

        // Apply Voice Activity Detection (VAD) boundary trimming
        let vad_config = VadConfig::default();
        let (processed_samples, vad_result) = vad_config.process(&mono_16k, TARGET_SAMPLE_RATE);

        // TEMP: dictation latency instrumentation
        let t_vad_done = std::time::Instant::now();

        let had_audio = capture_result.had_audio && vad_result.speech_detected;
        let final_samples = if had_audio {
            processed_samples
        } else {
            Vec::new()
        };

        write_wav(&session.file_path, if final_samples.is_empty() { &mono_16k } else { &final_samples })?;

        // TEMP: dictation latency instrumentation
        let t_wav_done = std::time::Instant::now();

        let stats = AudioStats::compute(&final_samples, TARGET_SAMPLE_RATE, 1);
        tracing::info!(
            "Stopped audio capture session {}, duration: {:.2}s -> {:.2}s ({:.1}% silence removed), samples: {}, RMS: {:.4}, peak: {:.4}, had_audio: {}",
            session.session_id,
            duration,
            stats.duration_seconds,
            vad_result.silence_removed_percent,
            stats.sample_count,
            stats.rms,
            stats.peak_amplitude,
            had_audio
        );

        // TEMP: dictation latency instrumentation
        let timing_metrics = CaptureTimingMetrics {
            thread_stop_ms: t_thread_done.duration_since(t_stop_start).as_millis(),
            resample_ms: t_resampled.duration_since(t_thread_done).as_millis(),
            vad_ms: t_vad_done.duration_since(t_resampled).as_millis(),
            wav_write_ms: t_wav_done.duration_since(t_vad_done).as_millis(),
            total_stop_ms: t_stop_start.elapsed().as_millis(),
        };

        Ok(CapturedAudio {
            session_id: session.session_id,
            mode: session.mode,
            audio_path: session.file_path.to_string_lossy().to_string(),
            samples: final_samples,
            duration_seconds: stats.duration_seconds,
            original_duration_seconds: duration,
            had_audio,
            audio_stats: stats,
            vad_result,
            timing_metrics: Some(timing_metrics),
        })
    }
}

fn spawn_capture_thread(
    stop_rx: std_mpsc::Receiver<()>,
    done_tx: std_mpsc::Sender<Result<CaptureThreadResult, String>>,
    app: Option<AppHandle>,
) {
    std::thread::spawn(move || {
        let result = (|| -> Result<CaptureThreadResult, String> {
            let host = cpal::default_host();
            let device = host
                .default_input_device()
                .ok_or_else(|| "No input (microphone) device available".to_string())?;
            let config = device
                .default_input_config()
                .map_err(|e| format!("Could not read default input config: {}", e))?;

            let sample_rate = config.sample_rate().0;
            let channels = config.channels() as usize;
            let samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
            let samples_cb = samples.clone();
            let err_fn =
                |err: cpal::StreamError| tracing::error!("cpal input stream error: {}", err);

            let last_emit = Arc::new(Mutex::new(std::time::Instant::now()));
            let smoothed_level = Arc::new(Mutex::new(0.0_f32));
            let detection = Arc::new(Mutex::new(AudioDetectionState::default()));

            let stream = match config.sample_format() {
                cpal::SampleFormat::F32 => {
                    let app_ref = app.clone();
                    let emit_ref = last_emit.clone();
                    let level_ref = smoothed_level.clone();
                    let detection_ref = detection.clone();
                    device.build_input_stream(
                        &config.into(),
                        move |data: &[f32], _| {
                            push_mono_with_level(
                                &samples_cb, data, channels, |s| s, &app_ref, &emit_ref,
                                &level_ref, &detection_ref, sample_rate,
                            )
                        },
                        err_fn,
                        None,
                    )
                }
                cpal::SampleFormat::I16 => {
                    let app_ref = app.clone();
                    let emit_ref = last_emit.clone();
                    let level_ref = smoothed_level.clone();
                    let detection_ref = detection.clone();
                    device.build_input_stream(
                        &config.into(),
                        move |data: &[i16], _| {
                            push_mono_with_level(
                                &samples_cb,
                                data,
                                channels,
                                |s| s as f32 / i16::MAX as f32,
                                &app_ref,
                                &emit_ref,
                                &level_ref,
                                &detection_ref,
                                sample_rate,
                            )
                        },
                        err_fn,
                        None,
                    )
                }
                cpal::SampleFormat::U16 => {
                    let app_ref = app.clone();
                    let emit_ref = last_emit.clone();
                    let level_ref = smoothed_level.clone();
                    let detection_ref = detection.clone();
                    device.build_input_stream(
                        &config.into(),
                        move |data: &[u16], _| {
                            push_mono_with_level(
                                &samples_cb,
                                data,
                                channels,
                                |s| (s as f32 - u16::MAX as f32 / 2.0) / (u16::MAX as f32 / 2.0),
                                &app_ref,
                                &emit_ref,
                                &level_ref,
                                &detection_ref,
                                sample_rate,
                            )
                        },
                        err_fn,
                        None,
                    )
                }
                other => return Err(format!("Unsupported input sample format: {:?}", other)),
            }
            .map_err(|e| format!("Failed to build input stream: {}", e))?;

            stream
                .play()
                .map_err(|e| format!("Failed to start input stream: {}", e))?;

            let _ = stop_rx.recv();
            drop(stream);

            let final_samples = samples.lock_or_recover().clone();
            // Sustained (not momentary) energy above the *measured* ambient
            // floor — not a fixed absolute threshold — is what counts as
            // speech. See AudioDetectionState and the constants above for
            // why: a fixed threshold can't tell a noisy-but-silent room
            // apart from someone actually talking, and a single loud
            // callback isn't proof of speech either.
            let final_state = detection.lock_or_recover();
            let min_frames_above =
                (sample_rate as u64 * AUDIO_DETECTED_MIN_DURATION_MS / 1000) as u32;
            let audio_detected = final_state.frames_above_threshold >= min_frames_above;
            tracing::debug!(
                "Audio detection: noise_floor={:.4}, frames_above={}, min_required={}, had_audio={}",
                final_state.noise_floor,
                final_state.frames_above_threshold,
                min_frames_above,
                audio_detected
            );
            Ok(CaptureThreadResult {
                samples: final_samples,
                input_rate: sample_rate,
                had_audio: audio_detected,
            })
        })();

        let _ = done_tx.send(result);
    });
}

fn compute_rms_f32(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|&s| s * s).sum();
    let mean_sq = sum_sq / samples.len() as f32;
    (mean_sq.sqrt() * 3.5).clamp(0.0, 1.0)
}

/// One-pole low-pass filter blending the previous smoothed level with the
/// current raw one — turns per-callback RMS spikiness into the steady
/// rise/fall the waveform bars are meant to show, without needing to buffer
/// or delay anything.
const LEVEL_SMOOTHING_ALPHA: f32 = 0.35;

#[allow(clippy::too_many_arguments)]
fn push_mono_with_level<T: Copy>(
    buf: &Arc<Mutex<Vec<f32>>>,
    data: &[T],
    channels: usize,
    to_f32: impl Fn(T) -> f32,
    app: &Option<AppHandle>,
    last_emit: &Arc<Mutex<std::time::Instant>>,
    smoothed_level: &Arc<Mutex<f32>>,
    detection: &Arc<Mutex<AudioDetectionState>>,
    sample_rate: u32,
) {
    let mut chunk = Vec::with_capacity(data.len() / channels.max(1));
    if channels > 1 {
        for frame in data.chunks(channels) {
            let sum: f32 = frame.iter().map(|s| to_f32(*s)).sum();
            chunk.push(sum / channels as f32);
        }
    } else {
        chunk.extend(data.iter().map(|s| to_f32(*s)));
    }

    // Computed unconditionally (not just when an emit is due) so audio
    // detection never depends on emit throttling or on an `AppHandle` being
    // present — "did real input arrive" must hold regardless of whether
    // anything is listening for level events.
    let raw_level = compute_rms_f32(&chunk);
    update_audio_detection(detection, raw_level, chunk.len() as u32, sample_rate);

    let smoothed = {
        let mut level_guard = smoothed_level.lock_or_recover();
        *level_guard += (raw_level - *level_guard) * LEVEL_SMOOTHING_ALPHA;
        *level_guard
    };

    if let Some(ref a) = app {
        let mut last_guard = last_emit.lock_or_recover();
        if last_guard.elapsed() >= Duration::from_millis(40) {
            *last_guard = std::time::Instant::now();
            let _ = a.emit("capture-level", serde_json::json!({ "level": smoothed }));
        }
    }

    let mut guard = buf.lock_or_recover();
    guard.extend(chunk);
}

/// Spends the first [`NOISE_FLOOR_CALIBRATION_MS`] of a session measuring
/// the ambient level with no detection running, then treats any chunk that
/// clears `max(AUDIO_DETECTED_THRESHOLD, noise_floor + NOISE_FLOOR_SPEECH_MARGIN)`
/// as (possible) speech, accumulating how many sample-frames of it there
/// were — [`AudioRecorder::stop`] later requires that to add up to at least
/// [`AUDIO_DETECTED_MIN_DURATION_MS`] before calling the session `had_audio`.
fn update_audio_detection(
    detection: &Arc<Mutex<AudioDetectionState>>,
    raw_level: f32,
    frame_count: u32,
    sample_rate: u32,
) {
    let mut state = detection.lock_or_recover();

    if !state.calibration_done {
        state.calibration_level_duration_sum += raw_level as f64 * frame_count as f64;
        state.calibration_frames += frame_count;

        let calibration_min_frames =
            (sample_rate as u64 * NOISE_FLOOR_CALIBRATION_MS / 1000) as u32;
        if state.calibration_frames < calibration_min_frames {
            return;
        }

        state.noise_floor =
            (state.calibration_level_duration_sum / state.calibration_frames as f64) as f32;
        state.calibration_done = true;
        return;
    }

    let effective_threshold =
        (state.noise_floor + NOISE_FLOOR_SPEECH_MARGIN).max(AUDIO_DETECTED_THRESHOLD);
    if raw_level >= effective_threshold {
        state.frames_above_threshold += frame_count;
    }
}

/// Naive linear-interpolation resampler. Good enough for speech-to-text
/// input (which itself works on 16kHz mono); not a mastering-quality resampler.
/// Automatically sanitizes non-finite values (NaN / Inf) and clamps output to [-1.0, 1.0].
pub fn resample_to_16k_mono(samples: &[f32], input_rate: u32) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }

    let sanitize = |val: f32| -> f32 {
        if val.is_finite() {
            val.clamp(-1.0, 1.0)
        } else {
            0.0
        }
    };

    if input_rate == TARGET_SAMPLE_RATE {
        return samples.iter().copied().map(sanitize).collect();
    }

    let ratio = input_rate as f64 / TARGET_SAMPLE_RATE as f64;
    let out_len = (samples.len() as f64 / ratio).floor() as usize;
    let mut out = Vec::with_capacity(out_len);

    for i in 0..out_len {
        let src_pos = i as f64 * ratio;
        let idx = src_pos as usize;
        let frac = (src_pos - idx as f64) as f32;
        let s0 = samples.get(idx).copied().map(sanitize).unwrap_or(0.0);
        let s1 = samples.get(idx + 1).copied().map(sanitize).unwrap_or(s0);
        out.push(s0 + (s1 - s0) * frac);
    }

    out
}

/// Comprehensive audio measurement statistics for STT observability.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioStats {
    pub sample_count: usize,
    pub duration_seconds: f32,
    pub sample_rate: u32,
    pub channels: u16,
    pub rms: f32,
    pub peak_amplitude: f32,
    pub near_zero_percent: f32,
    pub near_clipping_percent: f32,
    pub has_non_finite: bool,
    pub non_finite_count: usize,
}

impl AudioStats {
    /// Computes audio statistics over a slice of PCM floating-point samples.
    pub fn compute(samples: &[f32], sample_rate: u32, channels: u16) -> Self {
        if samples.is_empty() {
            return Self {
                sample_count: 0,
                duration_seconds: 0.0,
                sample_rate,
                channels,
                rms: 0.0,
                peak_amplitude: 0.0,
                near_zero_percent: 100.0,
                near_clipping_percent: 0.0,
                has_non_finite: false,
                non_finite_count: 0,
            };
        }

        let mut non_finite_count = 0;
        let mut sum_sq = 0.0_f64;
        let mut peak = 0.0_f32;
        let mut near_zero_count = 0;
        let mut near_clipping_count = 0;

        for &s in samples {
            if !s.is_finite() {
                non_finite_count += 1;
                near_zero_count += 1;
                continue;
            }
            let abs_s = s.abs();
            if abs_s > peak {
                peak = abs_s;
            }
            if abs_s < 0.001 {
                near_zero_count += 1;
            }
            if abs_s >= 0.98 {
                near_clipping_count += 1;
            }
            sum_sq += (s as f64) * (s as f64);
        }

        let total = samples.len() as f64;
        let rms = ((sum_sq / total).sqrt() as f32).clamp(0.0, 1.0);
        let near_zero_percent = ((near_zero_count as f64 / total) * 100.0) as f32;
        let near_clipping_percent = ((near_clipping_count as f64 / total) * 100.0) as f32;

        Self {
            sample_count: samples.len(),
            duration_seconds: samples.len() as f32 / sample_rate.max(1) as f32,
            sample_rate,
            channels,
            rms,
            peak_amplitude: peak,
            near_zero_percent,
            near_clipping_percent,
            has_non_finite: non_finite_count > 0,
            non_finite_count,
        }
    }
}

/// Reads a WAV file from disk and computes its audio statistics.
pub fn analyze_wav_file(path: &Path) -> Result<AudioStats, CaptureError> {
    let mut reader =
        hound::WavReader::open(path).map_err(|e| CaptureError::WavError(e.to_string()))?;
    let spec = reader.spec();
    let raw_samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().filter_map(|s| s.ok()).collect(),
        hound::SampleFormat::Int => {
            let max_val = match spec.bits_per_sample {
                16 => i16::MAX as f32,
                24 => 8_388_607.0_f32,
                32 => i32::MAX as f32,
                8 => i8::MAX as f32,
                _ => i16::MAX as f32,
            };
            reader
                .samples::<i32>()
                .filter_map(|s| s.ok())
                .map(|s| s as f32 / max_val)
                .collect()
        }
    };
    Ok(AudioStats::compute(
        &raw_samples,
        spec.sample_rate,
        spec.channels,
    ))
}

/// Voice Activity Detection (VAD) configuration for speech boundary trimming.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VadConfig {
    pub enabled: bool,
    pub frame_ms: usize,
    pub min_speech_duration_ms: usize,
    pub min_silence_duration_ms: usize,
    pub pad_before_ms: usize,
    pub pad_after_ms: usize,
    pub speech_margin: f32,
    pub min_energy_threshold: f32,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            frame_ms: 20,
            min_speech_duration_ms: 80,
            min_silence_duration_ms: 300,
            pad_before_ms: 250,
            pad_after_ms: 250,
            speech_margin: 0.008,
            min_energy_threshold: 0.006,
        }
    }
}

/// Diagnostic metadata and results from a VAD boundary detection run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VadResult {
    pub speech_detected: bool,
    pub start_sample: usize,
    pub end_sample: usize,
    pub start_seconds: f32,
    pub end_seconds: f32,
    pub original_duration: f32,
    pub trimmed_duration: f32,
    pub silence_removed_seconds: f32,
    pub silence_removed_percent: f32,
    pub noise_floor: f32,
    pub onset_threshold: f32,
}

impl VadConfig {
    /// Detects speech boundaries in a 16kHz mono audio buffer and returns trimmed/padded samples.
    pub fn process(&self, samples: &[f32], sample_rate: u32) -> (Vec<f32>, VadResult) {
        let total_samples = samples.len();
        let total_duration = total_samples as f32 / sample_rate.max(1) as f32;

        if !self.enabled || samples.is_empty() {
            let res = VadResult {
                speech_detected: !samples.is_empty(),
                start_sample: 0,
                end_sample: total_samples,
                start_seconds: 0.0,
                end_seconds: total_duration,
                original_duration: total_duration,
                trimmed_duration: total_duration,
                silence_removed_seconds: 0.0,
                silence_removed_percent: 0.0,
                noise_floor: 0.0,
                onset_threshold: 0.0,
            };
            return (samples.to_vec(), res);
        }

        let frame_samples = (sample_rate as usize * self.frame_ms) / 1000;
        if frame_samples == 0 || total_samples < frame_samples {
            let res = VadResult {
                speech_detected: false,
                start_sample: 0,
                end_sample: 0,
                start_seconds: 0.0,
                end_seconds: 0.0,
                original_duration: total_duration,
                trimmed_duration: 0.0,
                silence_removed_seconds: total_duration,
                silence_removed_percent: 100.0,
                noise_floor: 0.0,
                onset_threshold: self.min_energy_threshold,
            };
            return (Vec::new(), res);
        }

        // 1. Calculate per-frame RMS
        let num_frames = total_samples / frame_samples;
        let mut frame_rms: Vec<f32> = Vec::with_capacity(num_frames);
        for i in 0..num_frames {
            let start = i * frame_samples;
            let end = start + frame_samples;
            let frame = &samples[start..end];
            let sum_sq: f32 = frame.iter().map(|&s| s * s).sum();
            let rms = (sum_sq / frame_samples as f32).sqrt();
            frame_rms.push(rms);
        }

        // 2. Estimate adaptive noise floor (lowest 20% of frames)
        let mut sorted_rms = frame_rms.clone();
        sorted_rms.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let noise_sample_count = ((num_frames as f32 * 0.2).ceil() as usize).max(1);
        let noise_floor =
            sorted_rms[..noise_sample_count].iter().sum::<f32>() / noise_sample_count as f32;

        let onset_threshold = (noise_floor + self.speech_margin).max(self.min_energy_threshold);
        let hold_threshold =
            (noise_floor + self.speech_margin * 0.6).max(self.min_energy_threshold * 0.75);

        let min_speech_frames =
            ((self.min_speech_duration_ms as f32 / self.frame_ms as f32).ceil() as usize).max(1);
        let min_silence_frames =
            ((self.min_silence_duration_ms as f32 / self.frame_ms as f32).ceil() as usize).max(1);

        // 3. Detect speech regions with onset and hangover hysteresis
        let mut speech_regions: Vec<(usize, usize)> = Vec::new();
        let mut in_speech = false;
        let mut current_start = 0;
        let mut consecutive_above = 0;
        let mut consecutive_below = 0;
        let mut last_active_frame = 0;

        for (i, &rms) in frame_rms.iter().enumerate() {
            if !in_speech {
                if rms >= onset_threshold {
                    consecutive_above += 1;
                    if consecutive_above >= min_speech_frames {
                        in_speech = true;
                        current_start = i.saturating_sub(consecutive_above - 1);
                        last_active_frame = i;
                        consecutive_below = 0;
                    }
                } else {
                    consecutive_above = 0;
                }
            } else if rms >= hold_threshold {
                last_active_frame = i;
                consecutive_below = 0;
            } else {
                consecutive_below += 1;
                if consecutive_below >= min_silence_frames {
                    in_speech = false;
                    speech_regions.push((current_start, last_active_frame));
                    consecutive_above = 0;
                    consecutive_below = 0;
                }
            }
        }

        if in_speech {
            speech_regions.push((current_start, last_active_frame));
        }

        if speech_regions.is_empty() {
            let res = VadResult {
                speech_detected: false,
                start_sample: 0,
                end_sample: 0,
                start_seconds: 0.0,
                end_seconds: 0.0,
                original_duration: total_duration,
                trimmed_duration: 0.0,
                silence_removed_seconds: total_duration,
                silence_removed_percent: 100.0,
                noise_floor,
                onset_threshold,
            };
            return (Vec::new(), res);
        }

        // 4. Determine outer speech envelope with pre/post padding
        let first_speech_frame = speech_regions.first().unwrap().0;
        let last_speech_frame = speech_regions.last().unwrap().1;

        let pad_before_samples = (sample_rate as usize * self.pad_before_ms) / 1000;
        let pad_after_samples = (sample_rate as usize * self.pad_after_ms) / 1000;

        let raw_start_sample = first_speech_frame * frame_samples;
        let raw_end_sample = ((last_speech_frame + 1) * frame_samples).min(total_samples);

        let start_sample = raw_start_sample.saturating_sub(pad_before_samples);
        let end_sample = (raw_end_sample + pad_after_samples).min(total_samples);

        let trimmed = samples[start_sample..end_sample].to_vec();
        let trimmed_duration = trimmed.len() as f32 / sample_rate as f32;
        let silence_removed = (total_duration - trimmed_duration).max(0.0);
        let silence_percent = if total_duration > 0.0 {
            (silence_removed / total_duration) * 100.0
        } else {
            0.0
        };

        let result = VadResult {
            speech_detected: true,
            start_sample,
            end_sample,
            start_seconds: start_sample as f32 / sample_rate as f32,
            end_seconds: end_sample as f32 / sample_rate as f32,
            original_duration: total_duration,
            trimmed_duration,
            silence_removed_seconds: silence_removed,
            silence_removed_percent: silence_percent,
            noise_floor,
            onset_threshold,
        };

        (trimmed, result)
    }
}

fn write_wav(path: &Path, samples_16k_mono: &[f32]) -> Result<(), CaptureError> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: TARGET_SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer =
        hound::WavWriter::create(path, spec).map_err(|e| CaptureError::WavError(e.to_string()))?;
    for &sample in samples_16k_mono {
        let clamped = sample.clamp(-1.0, 1.0);
        writer
            .write_sample((clamped * i16::MAX as f32) as i16)
            .map_err(|e| CaptureError::WavError(e.to_string()))?;
    }
    writer
        .finalize()
        .map_err(|e| CaptureError::WavError(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SAMPLE_RATE: u32 = 16_000;
    /// One cpal callback's worth of frames at a 20ms buffer, a realistic size.
    const TEST_FRAMES_PER_CALLBACK: u32 = TEST_SAMPLE_RATE / 50;

    fn run_session(levels: &[f32]) -> AudioDetectionState {
        let detection = Arc::new(Mutex::new(AudioDetectionState::default()));
        for &level in levels {
            update_audio_detection(&detection, level, TEST_FRAMES_PER_CALLBACK, TEST_SAMPLE_RATE);
        }
        Arc::try_unwrap(detection).unwrap().into_inner().unwrap()
    }

    fn had_audio(state: &AudioDetectionState) -> bool {
        let min_frames_above =
            (TEST_SAMPLE_RATE as u64 * AUDIO_DETECTED_MIN_DURATION_MS / 1000) as u32;
        state.frames_above_threshold >= min_frames_above
    }

    #[test]
    fn true_silence_never_counts_as_had_audio() {
        let levels = vec![0.0_f32; 50];
        let state = run_session(&levels);
        assert!(!had_audio(&state));
    }

    #[test]
    fn sustained_ambient_noise_does_not_trigger_had_audio() {
        let ambient = 0.03_f32;
        let levels = vec![ambient; 50];
        let state = run_session(&levels);
        assert!(
            !had_audio(&state),
            "sustained ambient noise at a constant level must not trigger had_audio; noise_floor={}",
            state.noise_floor
        );
    }

    #[test]
    fn sustained_speech_above_calibrated_floor_triggers_had_audio() {
        let ambient = 0.03_f32;
        let speech = ambient + NOISE_FLOOR_SPEECH_MARGIN + 0.05;
        let mut levels = vec![ambient; 15];
        levels.extend(vec![speech; 15]);
        let state = run_session(&levels);
        assert!(
            had_audio(&state),
            "sustained speech clearly above the calibrated floor must trigger had_audio; noise_floor={}",
            state.noise_floor
        );
    }

    #[test]
    fn brief_spike_shorter_than_min_duration_does_not_trigger_had_audio() {
        let ambient = 0.0_f32;
        let spike = 0.5_f32;
        let mut levels = vec![ambient; 15];
        levels.push(spike);
        levels.extend(vec![ambient; 10]);
        let state = run_session(&levels);
        assert!(
            !had_audio(&state),
            "a single brief loud callback must not be enough to trigger had_audio"
        );
    }

    #[test]
    fn test_audio_stats_silence() {
        let silence = vec![0.0_f32; 16000];
        let stats = AudioStats::compute(&silence, 16000, 1);
        assert_eq!(stats.sample_count, 16000);
        assert_eq!(stats.duration_seconds, 1.0);
        assert_eq!(stats.sample_rate, 16000);
        assert_eq!(stats.channels, 1);
        assert_eq!(stats.rms, 0.0);
        assert_eq!(stats.peak_amplitude, 0.0);
        assert_eq!(stats.near_zero_percent, 100.0);
        assert_eq!(stats.near_clipping_percent, 0.0);
        assert!(!stats.has_non_finite);
    }

    #[test]
    fn test_audio_stats_sine_wave() {
        // 1 second of 440Hz sine wave at peak amplitude 0.5 -> theoretical RMS is 0.5 / sqrt(2) ≈ 0.3535
        let samples: Vec<f32> = (0..16000)
            .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 440.0 * (i as f32 / 16000.0)).sin())
            .collect();
        let stats = AudioStats::compute(&samples, 16000, 1);
        assert_eq!(stats.sample_count, 16000);
        assert_eq!(stats.duration_seconds, 1.0);
        assert!((stats.peak_amplitude - 0.5).abs() < 0.01);
        assert!((stats.rms - 0.3535).abs() < 0.02);
        assert_eq!(stats.near_clipping_percent, 0.0);
        assert!(!stats.has_non_finite);
    }

    #[test]
    fn test_audio_stats_clipping_detection() {
        // Samples near full scale (0.99)
        let samples = vec![0.99_f32; 1000];
        let stats = AudioStats::compute(&samples, 16000, 1);
        assert_eq!(stats.near_clipping_percent, 100.0);
        assert_eq!(stats.peak_amplitude, 0.99);
    }

    #[test]
    fn test_audio_stats_non_finite_sanitization() {
        let samples = vec![0.1, f32::NAN, 0.2, f32::INFINITY, -f32::INFINITY, 0.3];
        let stats = AudioStats::compute(&samples, 16000, 1);
        assert!(stats.has_non_finite);
        assert_eq!(stats.non_finite_count, 3);

        let resampled = resample_to_16k_mono(&samples, 16000);
        for &s in &resampled {
            assert!(s.is_finite());
            assert!((-1.0..=1.0).contains(&s));
        }
    }

    #[test]
    fn test_resampler_identity_at_16k() {
        let input: Vec<f32> = (0..1600).map(|i| (i as f32 / 1600.0) * 0.5).collect();
        let output = resample_to_16k_mono(&input, 16000);
        assert_eq!(output.len(), input.len());
        for (a, b) in input.iter().zip(output.iter()) {
            assert_eq!(*a, *b);
        }
    }

    #[test]
    fn test_resampler_48k_to_16k() {
        // 1 second of audio at 48kHz (48,000 samples) -> should resample to 16,000 samples at 16kHz
        let input: Vec<f32> = (0..48000)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * (i as f32 / 48000.0)).sin() * 0.4)
            .collect();
        let output = resample_to_16k_mono(&input, 48000);
        assert_eq!(output.len(), 16000);
        let in_stats = AudioStats::compute(&input, 48000, 1);
        let out_stats = AudioStats::compute(&output, 16000, 1);
        assert!((in_stats.rms - out_stats.rms).abs() < 0.01);
        assert!((in_stats.peak_amplitude - out_stats.peak_amplitude).abs() < 0.01);
    }

    #[test]
    fn test_resampler_44_1k_to_16k() {
        // 1 second of audio at 44.1kHz (44,100 samples) -> should resample to ~16,000 samples at 16kHz
        let input: Vec<f32> = (0..44100)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * (i as f32 / 44100.0)).sin() * 0.3)
            .collect();
        let output = resample_to_16k_mono(&input, 44100);
        assert_eq!(output.len(), 16000);
        let in_stats = AudioStats::compute(&input, 44100, 1);
        let out_stats = AudioStats::compute(&output, 16000, 1);
        assert!((in_stats.rms - out_stats.rms).abs() < 0.01);
    }

    #[test]
    fn test_resampler_96k_to_16k() {
        let input: Vec<f32> = (0..96000)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * (i as f32 / 96000.0)).sin() * 0.25)
            .collect();
        let output = resample_to_16k_mono(&input, 96000);
        assert_eq!(output.len(), 16000);
    }

    #[test]
    fn test_wav_write_and_analyze_roundtrip() {
        let dir = std::env::temp_dir().join(format!("relay_audio_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let wav_path = dir.join("test_output.wav");

        let samples: Vec<f32> = (0..32000)
            .map(|i| 0.3 * (2.0 * std::f32::consts::PI * 440.0 * (i as f32 / 16000.0)).sin())
            .collect();

        write_wav(&wav_path, &samples).unwrap();
        assert!(wav_path.exists());

        let stats = analyze_wav_file(&wav_path).unwrap();
        assert_eq!(stats.sample_rate, 16000);
        assert_eq!(stats.channels, 1);
        assert_eq!(stats.sample_count, 32000);
        assert!((stats.duration_seconds - 2.0).abs() < 0.01);
        assert!((stats.peak_amplitude - 0.3).abs() < 0.01);
        assert!((stats.rms - (0.3 / std::f32::consts::SQRT_2)).abs() < 0.02);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_analyze_real_recorded_wavs_in_relay_config() {
        // Measure existing real recordings from .relay/config/audio if present
        let current_dir = std::env::current_dir().unwrap();
        let audio_dirs = [
            current_dir.join(".relay/config/audio"),
            current_dir.join("native/src-tauri/.relay/config/audio"),
        ];

        for audio_dir in &audio_dirs {
            if audio_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(audio_dir) {
                    let mut analyzed = 0;
                    let mut total_rms = 0.0_f64;
                    let mut max_peak = 0.0_f32;
                    let mut max_clipping = 0.0_f32;

                    let mut sample_stats = Vec::new();

                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().and_then(|e| e.to_str()) == Some("wav") {
                            if let Ok(stats) = analyze_wav_file(&path) {
                                if stats.sample_count > 0 {
                                    analyzed += 1;
                                    total_rms += stats.rms as f64;
                                    if stats.peak_amplitude > max_peak {
                                        max_peak = stats.peak_amplitude;
                                    }
                                    if stats.near_clipping_percent > max_clipping {
                                        max_clipping = stats.near_clipping_percent;
                                    }
                                    if stats.duration_seconds > 1.0 && sample_stats.len() < 8 {
                                        sample_stats.push((path.file_name().unwrap().to_string_lossy().to_string(), stats));
                                    }
                                }
                            }
                        }
                    }

                    if analyzed > 0 {
                        let avg_rms = (total_rms / analyzed as f64) as f32;
                        println!(
                            "[Real WAV Measurement] Analyzed {} WAV files in {:?}: avg_rms={:.4}, max_peak={:.4}, max_clipping={:.2}%",
                            analyzed, audio_dir, avg_rms, max_peak, max_clipping
                        );
                        for (name, s) in &sample_stats {
                            println!(
                                "  -> file: {}, dur: {:.2}s, rate: {}, ch: {}, RMS: {:.4}, peak: {:.4}, near_zero: {:.1}%, clipping: {:.2}%, non_finite: {}",
                                name, s.duration_seconds, s.sample_rate, s.channels, s.rms, s.peak_amplitude, s.near_zero_percent, s.near_clipping_percent, s.has_non_finite
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_vad_pure_silence() {
        let vad = VadConfig::default();
        let silence = vec![0.0_f32; 48000]; // 3.0 seconds
        let (trimmed, result) = vad.process(&silence, 16000);
        assert!(!result.speech_detected);
        assert_eq!(trimmed.len(), 0);
        assert_eq!(result.trimmed_duration, 0.0);
        assert_eq!(result.silence_removed_percent, 100.0);
    }

    #[test]
    fn test_vad_speech_surrounded_by_silence() {
        let vad = VadConfig::default();
        // 1.5s silence (24k samples) + 2.0s sine speech (32k samples) + 1.5s silence (24k samples) = 5.0s (80k samples)
        let mut audio = vec![0.0005_f32; 24000];
        let speech: Vec<f32> = (0..32000)
            .map(|i| 0.35 * (2.0 * std::f32::consts::PI * 440.0 * (i as f32 / 16000.0)).sin())
            .collect();
        audio.extend_from_slice(&speech);
        audio.extend_from_slice(&vec![0.0005_f32; 24000]);

        let (trimmed, result) = vad.process(&audio, 16000);
        assert!(result.speech_detected);
        // Speech is 2.0s + ~250ms pre-pad + ~250ms post-pad -> ~2.5s (40,000 samples)
        assert!(
            (result.trimmed_duration - 2.5).abs() < 0.2,
            "expected trimmed duration ~2.5s, got {:.2}s",
            result.trimmed_duration
        );
        assert!(
            result.silence_removed_seconds >= 2.0,
            "expected >= 2.0s silence removed, got {:.2}s",
            result.silence_removed_seconds
        );
        assert_eq!(trimmed.len(), result.end_sample - result.start_sample);
    }

    #[test]
    fn test_vad_intra_sentence_pause_preserved() {
        let vad = VadConfig::default();
        // 1.0s speech + 250ms pause (within 350ms hangover) + 1.0s speech
        let mut audio = Vec::new();
        let speech1: Vec<f32> = (0..16000)
            .map(|i| 0.3 * (2.0 * std::f32::consts::PI * 300.0 * (i as f32 / 16000.0)).sin())
            .collect();
        let pause = vec![0.001_f32; 4000]; // 250ms pause
        let speech2: Vec<f32> = (0..16000)
            .map(|i| 0.3 * (2.0 * std::f32::consts::PI * 500.0 * (i as f32 / 16000.0)).sin())
            .collect();

        audio.extend_from_slice(&speech1);
        audio.extend_from_slice(&pause);
        audio.extend_from_slice(&speech2);

        let (trimmed, result) = vad.process(&audio, 16000);
        assert!(result.speech_detected);
        // Total duration is 2.25s; since it starts and ends with speech, padding clamps to full audio
        assert_eq!(trimmed.len(), audio.len());
        assert_eq!(result.trimmed_duration, 2.25);
    }

    #[test]
    fn test_vad_quiet_speech_detected() {
        let vad = VadConfig::default();
        // 0.5s silence + 1.0s quiet speech (RMS ~0.02) + 0.5s silence
        let mut audio = vec![0.001_f32; 8000];
        let quiet_speech: Vec<f32> = (0..16000)
            .map(|i| 0.028 * (2.0 * std::f32::consts::PI * 350.0 * (i as f32 / 16000.0)).sin())
            .collect();
        audio.extend_from_slice(&quiet_speech);
        audio.extend_from_slice(&vec![0.001_f32; 8000]);

        let (_trimmed, result) = vad.process(&audio, 16000);
        assert!(result.speech_detected, "quiet speech with RMS 0.02 must be detected");
    }

    #[test]
    fn test_vad_boundary_clamps() {
        let vad = VadConfig::default();
        // Speech starting at sample 0
        let speech_at_start: Vec<f32> = (0..16000)
            .map(|i| 0.4 * (2.0 * std::f32::consts::PI * 440.0 * (i as f32 / 16000.0)).sin())
            .collect();
        let mut audio1 = speech_at_start;
        audio1.extend_from_slice(&vec![0.0_f32; 16000]);
        let (_trimmed1, res1) = vad.process(&audio1, 16000);
        assert!(res1.speech_detected);
        assert_eq!(res1.start_sample, 0);

        // Speech ending at final sample
        let mut audio2 = vec![0.0_f32; 16000];
        let speech_at_end: Vec<f32> = (0..16000)
            .map(|i| 0.4 * (2.0 * std::f32::consts::PI * 440.0 * (i as f32 / 16000.0)).sin())
            .collect();
        audio2.extend_from_slice(&speech_at_end);
        let (_trimmed2, res2) = vad.process(&audio2, 16000);
        assert!(res2.speech_detected);
        assert_eq!(res2.end_sample, audio2.len());
    }

    #[test]
    fn test_vad_233_recordings_experiment() {
        let vad = VadConfig::default();
        let current_dir = std::env::current_dir().unwrap();
        let audio_dirs = [
            current_dir.join(".relay/config/audio"),
            current_dir.join("native/src-tauri/.relay/config/audio"),
        ];

        for audio_dir in &audio_dirs {
            if audio_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(audio_dir) {
                    let mut total_files = 0;
                    let mut total_orig_dur = 0.0_f64;
                    let mut total_trim_dur = 0.0_f64;
                    let mut total_removed_dur = 0.0_f64;
                    let mut no_speech_count = 0;
                    let mut over_50_pct_removed_count = 0;
                    let mut total_proc_time_micros = 0_u128;
                    let mut no_speech_files = Vec::new();

                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().and_then(|e| e.to_str()) == Some("wav") {
                            if let Ok(reader) = hound::WavReader::open(&path) {
                                let spec = reader.spec();
                                let raw_samples: Vec<f32> = match spec.sample_format {
                                    hound::SampleFormat::Float => {
                                        reader.into_samples::<f32>().filter_map(|s| s.ok()).collect()
                                    }
                                    hound::SampleFormat::Int => {
                                        let max_val = match spec.bits_per_sample {
                                            16 => i16::MAX as f32,
                                            24 => 8_388_607.0_f32,
                                            32 => i32::MAX as f32,
                                            8 => i8::MAX as f32,
                                            _ => i16::MAX as f32,
                                        };
                                        reader
                                            .into_samples::<i32>()
                                            .filter_map(|s| s.ok())
                                            .map(|s| s as f32 / max_val)
                                            .collect()
                                    }
                                };

                                if raw_samples.len() >= 320 {
                                    total_files += 1;
                                    let t0 = std::time::Instant::now();
                                    let (_trimmed, res) = vad.process(&raw_samples, spec.sample_rate);
                                    let elapsed = t0.elapsed().as_micros();
                                    total_proc_time_micros += elapsed;

                                    total_orig_dur += res.original_duration as f64;
                                    total_trim_dur += res.trimmed_duration as f64;
                                    total_removed_dur += res.silence_removed_seconds as f64;

                                    if !res.speech_detected {
                                        no_speech_count += 1;
                                        if no_speech_files.len() < 10 {
                                            no_speech_files.push((
                                                path.file_name().unwrap().to_string_lossy().to_string(),
                                                res.original_duration,
                                                AudioStats::compute(&raw_samples, spec.sample_rate, spec.channels).rms,
                                            ));
                                        }
                                    }
                                    if res.silence_removed_percent > 50.0 {
                                        over_50_pct_removed_count += 1;
                                    }
                                }
                            }
                        }
                    }

                    if total_files > 0 {
                        let avg_orig = total_orig_dur / total_files as f64;
                        let avg_trim = total_trim_dur / total_files as f64;
                        let avg_removed = total_removed_dur / total_files as f64;
                        let avg_reduction_pct = (avg_removed / avg_orig) * 100.0;
                        let avg_proc_ms =
                            (total_proc_time_micros as f64 / total_files as f64) / 1000.0;

                        println!("\n=======================================================");
                        println!("        VAD 233-RECORDING EXPERIMENT RESULTS           ");
                        println!("=======================================================");
                        println!("Total recordings analyzed:       {}", total_files);
                        println!("Average original duration:       {:.2} s", avg_orig);
                        println!("Average trimmed duration:        {:.2} s", avg_trim);
                        println!("Average silence removed:         {:.2} s", avg_removed);
                        println!("Average audio reduction:         {:.1} %", avg_reduction_pct);
                        println!("No speech detected (empty/tap):  {}", no_speech_count);
                        for (fname, dur, rms) in &no_speech_files {
                            println!("   -> {} (dur: {:.2}s, RMS: {:.5})", fname, dur, rms);
                        }
                        println!("Recordings with >50% removed:    {}", over_50_pct_removed_count);
                        println!("Average preprocessing cost:      {:.3} ms / recording", avg_proc_ms);
                        println!("=======================================================\n");

                        assert!(
                            avg_reduction_pct > 5.0,
                            "VAD should remove significant leading/trailing dead air"
                        );
                    }
                }
            }
        }
    }
}
