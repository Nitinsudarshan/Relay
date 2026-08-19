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
    done_rx: std_mpsc::Receiver<Result<(Vec<f32>, u32), String>>,
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

        let (raw_samples, input_rate) = session
            .done_rx
            .recv_timeout(Duration::from_secs(10))
            .map_err(|e| {
                CaptureError::DeviceError(format!("capture thread did not respond: {}", e))
            })?
            .map_err(CaptureError::DeviceError)?;

        let mono_16k = resample_to_16k_mono(&raw_samples, input_rate);
        write_wav(&session.file_path, &mono_16k)?;

        tracing::info!(
            "Stopped audio capture session {}, duration: {:.2}s, samples: {}",
            session.session_id,
            duration,
            mono_16k.len()
        );

        Ok(CapturedAudio {
            session_id: session.session_id,
            mode: session.mode,
            audio_path: session.file_path.to_string_lossy().to_string(),
            samples: mono_16k,
            duration_seconds: duration,
        })
    }
}

fn spawn_capture_thread(
    stop_rx: std_mpsc::Receiver<()>,
    done_tx: std_mpsc::Sender<Result<(Vec<f32>, u32), String>>,
    app: Option<AppHandle>,
) {
    std::thread::spawn(move || {
        let result = (|| -> Result<(Vec<f32>, u32), String> {
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

            let stream = match config.sample_format() {
                cpal::SampleFormat::F32 => {
                    let app_ref = app.clone();
                    let emit_ref = last_emit.clone();
                    device.build_input_stream(
                        &config.into(),
                        move |data: &[f32], _| {
                            push_mono_with_level(&samples_cb, data, channels, |s| s, &app_ref, &emit_ref)
                        },
                        err_fn,
                        None,
                    )
                }
                cpal::SampleFormat::I16 => {
                    let app_ref = app.clone();
                    let emit_ref = last_emit.clone();
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
                            )
                        },
                        err_fn,
                        None,
                    )
                }
                cpal::SampleFormat::U16 => {
                    let app_ref = app.clone();
                    let emit_ref = last_emit.clone();
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
            Ok((final_samples, sample_rate))
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

fn push_mono_with_level<T: Copy>(
    buf: &Arc<Mutex<Vec<f32>>>,
    data: &[T],
    channels: usize,
    to_f32: impl Fn(T) -> f32,
    app: &Option<AppHandle>,
    last_emit: &Arc<Mutex<std::time::Instant>>,
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

    if let Some(ref a) = app {
        let mut last_guard = last_emit.lock().unwrap();
        if last_guard.elapsed() >= Duration::from_millis(40) {
            *last_guard = std::time::Instant::now();
            let level = compute_rms_f32(&chunk);
            let _ = a.emit("capture-level", serde_json::json!({ "level": level }));
        }
    }

    let mut guard = buf.lock().unwrap();
    guard.extend(chunk);
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
