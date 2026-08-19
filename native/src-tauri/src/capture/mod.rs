pub mod stt;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::mpsc as std_mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;

use tauri::{AppHandle, Emitter};

pub use stt::{SttEngine, SttError};

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

/// Raw captured audio, resampled to 16kHz mono, ready to hand to an STT engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedAudio {
    pub session_id: String,
    pub mode: String,
    pub audio_path: String,
    #[serde(skip)]
    pub samples: Vec<f32>,
    pub duration_seconds: f32,
    /// Whether the session measured speech-level energy sustained above the
    /// calibrated ambient noise floor — the authoritative "did the user
    /// actually say something" signal. `recordingStarted` never implies
    /// this; it is only ever set by real, sustained, above-ambient input
    /// arriving during capture. Callers must treat a session with
    /// `had_audio: false` as having nothing to transcribe.
    pub had_audio: bool,
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
        self.active_session.lock().unwrap().is_some()
    }

    /// The `mode` of the in-progress session, if any — lets callers tell a
    /// hotkey-owned ("dictation") session apart from a UI-owned one
    /// ("meeting"/"scribble"/"chat") without a separate ownership field.
    pub fn active_mode(&self) -> Option<String> {
        self.active_session
            .lock()
            .unwrap()
            .as_ref()
            .map(|s| s.mode.clone())
    }

    pub fn start(
        &self,
        mode: &str,
        output_dir: &Path,
        app: Option<AppHandle>,
    ) -> Result<String, CaptureError> {
        let mut session = self.active_session.lock().unwrap();
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
        let session = {
            let mut guard = self.active_session.lock().unwrap();
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

        let mono_16k = resample_to_16k_mono(&capture_result.samples, capture_result.input_rate);
        write_wav(&session.file_path, &mono_16k)?;

        tracing::info!(
            "Stopped audio capture session {}, duration: {:.2}s, samples: {}, had_audio: {}",
            session.session_id,
            duration,
            mono_16k.len(),
            capture_result.had_audio
        );

        Ok(CapturedAudio {
            session_id: session.session_id,
            mode: session.mode,
            audio_path: session.file_path.to_string_lossy().to_string(),
            samples: mono_16k,
            duration_seconds: duration,
            had_audio: capture_result.had_audio,
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

            let final_samples = samples.lock().unwrap().clone();
            // Sustained (not momentary) energy above the *measured* ambient
            // floor — not a fixed absolute threshold — is what counts as
            // speech. See AudioDetectionState and the constants above for
            // why: a fixed threshold can't tell a noisy-but-silent room
            // apart from someone actually talking, and a single loud
            // callback isn't proof of speech either.
            let final_state = detection.lock().unwrap();
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
        let mut level_guard = smoothed_level.lock().unwrap();
        *level_guard += (raw_level - *level_guard) * LEVEL_SMOOTHING_ALPHA;
        *level_guard
    };

    if let Some(ref a) = app {
        let mut last_guard = last_emit.lock().unwrap();
        if last_guard.elapsed() >= Duration::from_millis(40) {
            *last_guard = std::time::Instant::now();
            let _ = a.emit("capture-level", serde_json::json!({ "level": smoothed }));
        }
    }

    let mut guard = buf.lock().unwrap();
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
    let mut state = detection.lock().unwrap();

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
fn resample_to_16k_mono(samples: &[f32], input_rate: u32) -> Vec<f32> {
    if samples.is_empty() || input_rate == TARGET_SAMPLE_RATE {
        return samples.to_vec();
    }

    let ratio = input_rate as f64 / TARGET_SAMPLE_RATE as f64;
    let out_len = (samples.len() as f64 / ratio).floor() as usize;
    let mut out = Vec::with_capacity(out_len);

    for i in 0..out_len {
        let src_pos = i as f64 * ratio;
        let idx = src_pos as usize;
        let frac = (src_pos - idx as f64) as f32;
        let s0 = samples.get(idx).copied().unwrap_or(0.0);
        let s1 = samples.get(idx + 1).copied().unwrap_or(s0);
        out.push(s0 + (s1 - s0) * frac);
    }

    out
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
        // ~1 second of near-zero level callbacks.
        let levels = vec![0.0_f32; 50];
        let state = run_session(&levels);
        assert!(!had_audio(&state));
    }

    #[test]
    fn sustained_ambient_noise_does_not_trigger_had_audio() {
        // Regression test: a noisy-but-silent room (fan/keyboard/room hum,
        // mic AGC) sitting continuously above the old fixed 0.02 threshold
        // must not be mistaken for speech, because it never rises above the
        // floor the calibration window measures from that exact same noise.
        let ambient = 0.03_f32;
        let levels = vec![ambient; 50]; // ~1s, well past calibration + detection window
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
        let speech = ambient + NOISE_FLOOR_SPEECH_MARGIN + 0.05; // clearly above floor+margin
        // Calibration window (300ms) of ambient, then well over 200ms of speech.
        let mut levels = vec![ambient; 15]; // 15 * 20ms = 300ms
        levels.extend(vec![speech; 15]); // another 300ms of speech
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
        let spike = 0.5_f32; // loud, but only one callback (20ms) worth
        let mut levels = vec![ambient; 15]; // calibration
        levels.push(spike); // a single 20ms loud callback — well under 200ms
        levels.extend(vec![ambient; 10]);
        let state = run_session(&levels);
        assert!(
            !had_audio(&state),
            "a single brief loud callback must not be enough to trigger had_audio"
        );
    }
}
