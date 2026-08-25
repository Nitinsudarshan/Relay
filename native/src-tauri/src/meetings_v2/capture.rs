use super::types::AudioLevels;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc as std_mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

pub const TARGET_SAMPLE_RATE: u32 = 16_000;
pub const CHUNK_DURATION_SECS: f64 = 30.0;
pub const SAMPLES_PER_CHUNK: usize = (TARGET_SAMPLE_RATE as f64 * CHUNK_DURATION_SECS) as usize;

pub const LIVE_FRAME_DURATION_SECS: f64 = 1.5;
pub const SAMPLES_PER_LIVE_FRAME: usize = (TARGET_SAMPLE_RATE as f64 * LIVE_FRAME_DURATION_SECS) as usize; // 24,000 samples
pub const LIVE_OVERLAP_SAMPLES: usize = (TARGET_SAMPLE_RATE as f64 * 0.25) as usize; // 4,000 samples (250ms)

const LEVEL_SMOOTHING_ALPHA: f32 = 0.35;

pub struct AudioChunk {
    pub session_id: String,
    pub chunk_index: usize,
    pub start_time_s: f64,
    pub end_time_s: f64,
    pub samples: Vec<f32>,
    pub mic_had_audio: bool,
    pub sys_had_audio: bool,
}

pub struct LiveAudioFrame {
    pub session_id: String,
    pub frame_index: usize,
    pub start_time_s: f64,
    pub end_time_s: f64,
    pub samples: Vec<f32>,
    pub capture_instant: std::time::Instant,
}

pub struct DualAudioCapture {
    _session_id: String,
    mic_active: Arc<AtomicBool>,
    sys_active: Arc<AtomicBool>,
    stop_tx: Option<std_mpsc::Sender<()>>,
    join_handle: Option<std::thread::JoinHandle<()>>,
}

impl DualAudioCapture {
    pub fn start(
        session_id: String,
        chunk_tx: std_mpsc::Sender<AudioChunk>,
        live_tx: Option<std_mpsc::SyncSender<LiveAudioFrame>>,
        app: Option<AppHandle>,
    ) -> Result<Self, String> {
        let (stop_tx, stop_rx) = std_mpsc::channel();
        let mic_active = Arc::new(AtomicBool::new(false));
        let sys_active = Arc::new(AtomicBool::new(false));

        let mic_active_clone = mic_active.clone();
        let sys_active_clone = sys_active.clone();
        let sid_clone = session_id.clone();

        let join_handle = std::thread::spawn(move || {
            run_dual_capture_loop(
                sid_clone,
                stop_rx,
                chunk_tx,
                live_tx,
                mic_active_clone,
                sys_active_clone,
                app,
            );
        });

        Ok(Self {
            _session_id: session_id,
            mic_active,
            sys_active,
            stop_tx: Some(stop_tx),
            join_handle: Some(join_handle),
        })
    }

    pub fn stop(&mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }

    pub fn is_mic_active(&self) -> bool {
        self.mic_active.load(Ordering::SeqCst)
    }

    pub fn is_sys_active(&self) -> bool {
        self.sys_active.load(Ordering::SeqCst)
    }
}

impl Drop for DualAudioCapture {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Compute RMS scaled matching the Dictation reference engine
fn compute_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|&s| s * s).sum();
    let mean_sq = sum_sq / samples.len() as f32;
    (mean_sq.sqrt() * 5.0).clamp(0.0, 1.0)
}

/// Linear sample-rate conversion to 16 kHz
fn resample_linear(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate || input.is_empty() {
        return input.to_vec();
    }

    let ratio = to_rate as f64 / from_rate as f64;
    let output_len = ((input.len() as f64) * ratio).round() as usize;
    let mut output = Vec::with_capacity(output_len);

    for i in 0..output_len {
        let src_idx = (i as f64) / ratio;
        let idx_floor = src_idx.floor() as usize;
        let idx_ceil = (idx_floor + 1).min(input.len() - 1);
        let frac = (src_idx - idx_floor as f64) as f32;

        let sample = if idx_floor < input.len() {
            input[idx_floor] * (1.0 - frac) + input[idx_ceil] * frac
        } else {
            0.0
        };
        output.push(sample);
    }
    output
}

/// Soft-saturation audio mixer combining microphone and system audio without harsh clipping
#[inline]
fn soft_mix(mic: f32, sys: f32) -> f32 {
    let sum = mic + (sys * 0.9);
    if sum > 1.0 {
        1.0 - (-(sum - 1.0)).exp() * 0.5
    } else if sum < -1.0 {
        -1.0 + (sum + 1.0).exp() * 0.5
    } else {
        sum
    }
}

fn run_dual_capture_loop(
    session_id: String,
    stop_rx: std_mpsc::Receiver<()>,
    chunk_tx: std_mpsc::Sender<AudioChunk>,
    live_tx: Option<std_mpsc::SyncSender<LiveAudioFrame>>,
    mic_active: Arc<AtomicBool>,
    sys_active: Arc<AtomicBool>,
    app: Option<AppHandle>,
) {
    let host = cpal::default_host();

    // 1. Separate FIFO Queues for temporal sample-by-sample alignment
    let mic_fifo: Arc<Mutex<VecDeque<f32>>> = Arc::new(Mutex::new(VecDeque::with_capacity(32_000)));
    let sys_fifo: Arc<Mutex<VecDeque<f32>>> = Arc::new(Mutex::new(VecDeque::with_capacity(32_000)));

    let mic_rms_cell = Arc::new(Mutex::new(0.0_f32));
    let sys_rms_cell = Arc::new(Mutex::new(0.0_f32));
    let last_emit = Arc::new(Mutex::new(std::time::Instant::now()));

    // 2. Setup Microphone Stream
    let mut _mic_stream = None;
    if let Some(mic_device) = host.default_input_device() {
        if let Ok(config) = mic_device.default_input_config() {
            let sample_rate = config.sample_rate().0;
            let channels = config.channels() as usize;
            let fifo_ref = mic_fifo.clone();
            let rms_ref = mic_rms_cell.clone();
            let other_rms_ref = sys_rms_cell.clone();
            let flag_ref = mic_active.clone();
            let app_ref = app.clone();
            let emit_ref = last_emit.clone();

            let err_fn = |err| tracing::error!("DualCapture: Microphone CPAL error: {}", err);
            let stream_res = match config.sample_format() {
                cpal::SampleFormat::F32 => mic_device.build_input_stream(
                    &config.into(),
                    move |data: &[f32], _| {
                        process_stream_frames(
                            data,
                            channels,
                            sample_rate,
                            |s| s,
                            &fifo_ref,
                            &rms_ref,
                            &flag_ref,
                            &app_ref,
                            &emit_ref,
                            &other_rms_ref,
                            true,
                        );
                    },
                    err_fn,
                    None,
                ),
                cpal::SampleFormat::I16 => mic_device.build_input_stream(
                    &config.into(),
                    move |data: &[i16], _| {
                        process_stream_frames(
                            data,
                            channels,
                            sample_rate,
                            |s| s as f32 / i16::MAX as f32,
                            &fifo_ref,
                            &rms_ref,
                            &flag_ref,
                            &app_ref,
                            &emit_ref,
                            &other_rms_ref,
                            true,
                        );
                    },
                    err_fn,
                    None,
                ),
                cpal::SampleFormat::U16 => mic_device.build_input_stream(
                    &config.into(),
                    move |data: &[u16], _| {
                        process_stream_frames(
                            data,
                            channels,
                            sample_rate,
                            |s| (s as f32 - u16::MAX as f32 / 2.0) / (u16::MAX as f32 / 2.0),
                            &fifo_ref,
                            &rms_ref,
                            &flag_ref,
                            &app_ref,
                            &emit_ref,
                            &other_rms_ref,
                            true,
                        );
                    },
                    err_fn,
                    None,
                ),
                _ => Err(cpal::BuildStreamError::DeviceNotAvailable),
            };

            if let Ok(stream) = stream_res {
                if stream.play().is_ok() {
                    _mic_stream = Some(stream);
                    mic_active.store(true, Ordering::SeqCst);
                    tracing::info!("DualCapture: Microphone stream active ({channels}ch @ {sample_rate}Hz)");
                }
            }
        }
    }

    // 3. Setup System Audio Loopback Stream (WASAPI default output device)
    let mut _sys_stream = None;
    if let Some(out_device) = host.default_output_device() {
        let config_res = out_device
            .default_input_config()
            .or_else(|_| out_device.default_output_config());

        if let Ok(config) = config_res {
            let sample_rate = config.sample_rate().0;
            let channels = config.channels() as usize;
            let fifo_ref = sys_fifo.clone();
            let rms_ref = sys_rms_cell.clone();
            let other_rms_ref = mic_rms_cell.clone();
            let flag_ref = sys_active.clone();
            let app_ref = app.clone();
            let emit_ref = last_emit.clone();

            let err_fn = |err| tracing::warn!("DualCapture: Loopback CPAL error: {}", err);
            let stream_res = match config.sample_format() {
                cpal::SampleFormat::F32 => out_device.build_input_stream(
                    &config.into(),
                    move |data: &[f32], _| {
                        process_stream_frames(
                            data,
                            channels,
                            sample_rate,
                            |s| s,
                            &fifo_ref,
                            &rms_ref,
                            &flag_ref,
                            &app_ref,
                            &emit_ref,
                            &other_rms_ref,
                            false,
                        );
                    },
                    err_fn,
                    None,
                ),
                cpal::SampleFormat::I16 => out_device.build_input_stream(
                    &config.into(),
                    move |data: &[i16], _| {
                        process_stream_frames(
                            data,
                            channels,
                            sample_rate,
                            |s| s as f32 / i16::MAX as f32,
                            &fifo_ref,
                            &rms_ref,
                            &flag_ref,
                            &app_ref,
                            &emit_ref,
                            &other_rms_ref,
                            false,
                        );
                    },
                    err_fn,
                    None,
                ),
                cpal::SampleFormat::U16 => out_device.build_input_stream(
                    &config.into(),
                    move |data: &[u16], _| {
                        process_stream_frames(
                            data,
                            channels,
                            sample_rate,
                            |s| (s as f32 - u16::MAX as f32 / 2.0) / (u16::MAX as f32 / 2.0),
                            &fifo_ref,
                            &rms_ref,
                            &flag_ref,
                            &app_ref,
                            &emit_ref,
                            &other_rms_ref,
                            false,
                        );
                    },
                    err_fn,
                    None,
                ),
                _ => Err(cpal::BuildStreamError::DeviceNotAvailable),
            };

            if let Ok(stream) = stream_res {
                if stream.play().is_ok() {
                    _sys_stream = Some(stream);
                    sys_active.store(true, Ordering::SeqCst);
                    tracing::info!("DualCapture: System audio loopback stream active ({channels}ch @ {sample_rate}Hz)");
                }
            }
        }
    }

    // 4. Synchronized Temporal Audio Mixer & Chunk Slicer Loop
    let mut mixed_durable_accum: Vec<f32> = Vec::with_capacity(SAMPLES_PER_CHUNK + 16_000);
    let mut mixed_live_accum: Vec<f32> = Vec::with_capacity(SAMPLES_PER_LIVE_FRAME + 16_000);

    let mut chunk_index = 0;
    let mut live_frame_index = 0;
    let mut elapsed_samples = 0usize;
    let mut live_elapsed_samples = 0usize;

    loop {
        // Check for stop signal (non-blocking)
        let is_stopping = stop_rx.try_recv().is_ok();

        // Drain and mix available samples from both FIFOs temporally in lockstep
        {
            let mut mic_guard = mic_fifo.lock().unwrap();
            let mut sys_guard = sys_fifo.lock().unwrap();

            let mic_avail = mic_guard.len();
            let sys_avail = sys_guard.len();

            let has_mic_device = _mic_stream.is_some();
            let has_sys_device = _sys_stream.is_some();

            let count_to_drain = if has_mic_device && has_sys_device {
                // If both hardware streams are bound, mix up to the max available (pad underflowing stream)
                mic_avail.max(sys_avail)
            } else if has_mic_device {
                mic_avail
            } else {
                sys_avail
            };

            if count_to_drain > 0 {
                mixed_durable_accum.reserve(count_to_drain);
                mixed_live_accum.reserve(count_to_drain);

                for _ in 0..count_to_drain {
                    let m = mic_guard.pop_front().unwrap_or(0.0);
                    let s = sys_guard.pop_front().unwrap_or(0.0);
                    let sample = soft_mix(m, s);

                    mixed_durable_accum.push(sample);
                    mixed_live_accum.push(sample);
                }
            }
        }

        // --- CLOCK B: Low-Latency Live STT Feed (~1.5s windows) ---
        if let Some(ref l_tx) = live_tx {
            while mixed_live_accum.len() >= SAMPLES_PER_LIVE_FRAME {
                let frame_samples = mixed_live_accum[0..SAMPLES_PER_LIVE_FRAME].to_vec();
                let start_time_s = live_elapsed_samples as f64 / TARGET_SAMPLE_RATE as f64;
                let step_samples = SAMPLES_PER_LIVE_FRAME - LIVE_OVERLAP_SAMPLES;
                live_elapsed_samples += step_samples;
                let end_time_s = (live_elapsed_samples + LIVE_OVERLAP_SAMPLES) as f64 / TARGET_SAMPLE_RATE as f64;

                // Advance by (duration - overlap) to provide continuity to the next window
                mixed_live_accum.drain(0..step_samples);

                let frame = LiveAudioFrame {
                    session_id: session_id.clone(),
                    frame_index: live_frame_index,
                    start_time_s,
                    end_time_s,
                    samples: frame_samples,
                    capture_instant: std::time::Instant::now(),
                };

                // Non-blocking send — never blocks capture even if STT is slow
                match l_tx.try_send(frame) {
                    Ok(_) => {}
                    Err(std_mpsc::TrySendError::Full(_)) => {
                        tracing::warn!("DualCapture: Live STT queue full — dropped live frame (durable recording intact)");
                    }
                    Err(std_mpsc::TrySendError::Disconnected(_)) => {}
                }

                live_frame_index += 1;
            }
        }

        // --- CLOCK A: Durable Recording Slicer (30-second WAV chunks) ---
        while mixed_durable_accum.len() >= SAMPLES_PER_CHUNK {
            let chunk_samples: Vec<f32> = mixed_durable_accum.drain(0..SAMPLES_PER_CHUNK).collect();
            let start_time_s = elapsed_samples as f64 / TARGET_SAMPLE_RATE as f64;
            elapsed_samples += chunk_samples.len();
            let end_time_s = elapsed_samples as f64 / TARGET_SAMPLE_RATE as f64;

            let mic_had = *mic_rms_cell.lock().unwrap() > 0.004;
            let sys_had = *sys_rms_cell.lock().unwrap() > 0.004;

            let chunk = AudioChunk {
                session_id: session_id.clone(),
                chunk_index,
                start_time_s,
                end_time_s,
                samples: chunk_samples,
                mic_had_audio: mic_had,
                sys_had_audio: sys_had,
            };

            let _ = chunk_tx.send(chunk);
            chunk_index += 1;
        }

        if is_stopping {
            break;
        }

        std::thread::sleep(Duration::from_millis(20));
    }

    // Flush any remaining partial audio chunk for durable recording
    if !mixed_durable_accum.is_empty() {
        let start_time_s = elapsed_samples as f64 / TARGET_SAMPLE_RATE as f64;
        elapsed_samples += mixed_durable_accum.len();
        let end_time_s = elapsed_samples as f64 / TARGET_SAMPLE_RATE as f64;

        let chunk = AudioChunk {
            session_id: session_id.clone(),
            chunk_index,
            start_time_s,
            end_time_s,
            samples: mixed_durable_accum,
            mic_had_audio: true,
            sys_had_audio: true,
        };

        let _ = chunk_tx.send(chunk);
    }

    mic_active.store(false, Ordering::SeqCst);
    sys_active.store(false, Ordering::SeqCst);
    tracing::info!("DualCapture: Synchronized temporal capture loop exited cleanly.");
}

#[allow(clippy::too_many_arguments)]
fn process_stream_frames<T: Copy>(
    data: &[T],
    channels: usize,
    sample_rate: u32,
    to_f32: impl Fn(T) -> f32,
    fifo: &Arc<Mutex<VecDeque<f32>>>,
    rms_cell: &Arc<Mutex<f32>>,
    active_flag: &Arc<AtomicBool>,
    app: &Option<AppHandle>,
    last_emit: &Arc<Mutex<std::time::Instant>>,
    other_rms_cell: &Arc<Mutex<f32>>,
    is_mic: bool,
) {
    if data.is_empty() {
        return;
    }

    active_flag.store(true, Ordering::Relaxed);

    // 1. Convert to mono
    let mut mono = Vec::with_capacity(data.len() / channels.max(1));
    if channels > 1 {
        for frame in data.chunks(channels) {
            let sum: f32 = frame.iter().map(|s| to_f32(*s)).sum();
            mono.push(sum / channels as f32);
        }
    } else {
        mono.extend(data.iter().map(|s| to_f32(*s)));
    }

    // 2. Measure RMS
    let raw_rms = compute_rms(&mono);
    let smoothed = {
        let mut guard = rms_cell.lock().unwrap();
        *guard += (raw_rms - *guard) * LEVEL_SMOOTHING_ALPHA;
        *guard
    };

    // 3. Emit live level event directly from audio callback (throttled ~35ms)
    if let Some(ref a) = app {
        let mut last = last_emit.lock().unwrap();
        if last.elapsed() >= Duration::from_millis(35) {
            *last = std::time::Instant::now();
            let other_l = *other_rms_cell.lock().unwrap();
            let payload = if is_mic {
                AudioLevels {
                    mic_level: smoothed,
                    sys_level: other_l,
                }
            } else {
                AudioLevels {
                    mic_level: other_l,
                    sys_level: smoothed,
                }
            };
            let _ = a.emit("meeting-audio-levels", &payload);
        }
    }

    // 4. Resample to 16kHz
    let resampled = resample_linear(&mono, sample_rate, TARGET_SAMPLE_RATE);

    // 5. Push resampled 16kHz frames into dedicated stream FIFO queue
    let mut guard = fifo.lock().unwrap();
    guard.extend(resampled);
}
