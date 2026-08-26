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

/// Cadence at which new audio is handed to the live (Clock B) transcriber.
///
/// Frames are contiguous and non-overlapping: the live worker keeps its own
/// rolling window, so overlapping frames here would only duplicate speech.
pub const LIVE_FRAME_DURATION_SECS: f64 = 1.0;
pub const SAMPLES_PER_LIVE_FRAME: usize =
    (TARGET_SAMPLE_RATE as f64 * LIVE_FRAME_DURATION_SECS) as usize;

/// How far the two capture streams are allowed to diverge before the mixer
/// stops waiting for the lagging one. WASAPI loopback delivers no callbacks at
/// all while nothing is playing, so waiting for exact lockstep would stall the
/// recording entirely; this bounds misalignment instead of accumulating it.
const MAX_STREAM_LAG_SAMPLES: usize = (TARGET_SAMPLE_RATE as f64 * 0.25) as usize;

/// RMS above which a source counts as audible rather than merely connected.
const AUDIBLE_RMS_THRESHOLD: f32 = 0.004;

const LEVEL_SMOOTHING_ALPHA: f32 = 0.35;
const LEVEL_EMIT_INTERVAL: Duration = Duration::from_millis(40);
const MIXER_TICK: Duration = Duration::from_millis(20);

pub struct AudioChunk {
    pub session_id: String,
    pub chunk_index: usize,
    pub start_time_s: f64,
    pub end_time_s: f64,
    pub samples: Vec<f32>,
    /// Whether each source was audible *within this chunk*, measured from the
    /// samples that went into it rather than from an instantaneous meter read.
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
    /// Set on the first frame after a pause, telling the live worker to commit
    /// whatever it was accumulating rather than splicing across the gap.
    pub discontinuity: bool,
    /// Frame-level energy gate: `false` means this second was silence.
    pub is_speech: bool,
}

/// Outcome of binding the capture devices, resolved before `start` returns.
struct CaptureInit {
    mic_bound: bool,
    sys_bound: bool,
}

pub struct DualAudioCapture {
    _session_id: String,
    mic_active: Arc<AtomicBool>,
    sys_active: Arc<AtomicBool>,
    mic_heard: Arc<AtomicBool>,
    sys_heard: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    warning: Option<String>,
    stop_tx: Option<std_mpsc::Sender<()>>,
    join_handle: Option<std::thread::JoinHandle<()>>,
}

impl DualAudioCapture {
    /// Starts dual capture, returning only once the audio devices have actually
    /// been bound so a failure surfaces as an error instead of a silent
    /// recording that reports success.
    pub fn start(
        session_id: String,
        chunk_tx: std_mpsc::Sender<AudioChunk>,
        live_tx: Option<std_mpsc::SyncSender<LiveAudioFrame>>,
        app: Option<AppHandle>,
    ) -> Result<Self, String> {
        let (stop_tx, stop_rx) = std_mpsc::channel();
        let (init_tx, init_rx) = std_mpsc::channel::<Result<CaptureInit, String>>();

        let mic_active = Arc::new(AtomicBool::new(false));
        let sys_active = Arc::new(AtomicBool::new(false));
        let mic_heard = Arc::new(AtomicBool::new(false));
        let sys_heard = Arc::new(AtomicBool::new(false));
        let paused = Arc::new(AtomicBool::new(false));

        let ctx = CaptureLoopContext {
            session_id: session_id.clone(),
            mic_active: mic_active.clone(),
            sys_active: sys_active.clone(),
            mic_heard: mic_heard.clone(),
            sys_heard: sys_heard.clone(),
            paused: paused.clone(),
            app,
        };

        let join_handle = std::thread::spawn(move || {
            run_dual_capture_loop(ctx, stop_rx, chunk_tx, live_tx, init_tx);
        });

        let init = init_rx
            .recv_timeout(Duration::from_secs(10))
            .map_err(|_| "Audio capture did not start within 10s".to_string())?;

        let init = match init {
            Ok(init) => init,
            Err(e) => {
                let _ = join_handle.join();
                return Err(e);
            }
        };

        let warning = match (init.mic_bound, init.sys_bound) {
            (true, true) => None,
            (true, false) => Some(
                "System audio (loopback) capture unavailable — only the microphone is being recorded."
                    .to_string(),
            ),
            (false, true) => Some(
                "Microphone capture unavailable — only system audio is being recorded.".to_string(),
            ),
            (false, false) => None, // unreachable: reported as Err above
        };

        if let Some(ref w) = warning {
            tracing::warn!("DualCapture: {}", w);
        }

        Ok(Self {
            _session_id: session_id,
            mic_active,
            sys_active,
            mic_heard,
            sys_heard,
            paused,
            warning,
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

    pub fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    pub fn warning(&self) -> Option<String> {
        self.warning.clone()
    }

    pub fn is_mic_active(&self) -> bool {
        self.mic_active.load(Ordering::SeqCst)
    }

    pub fn is_sys_active(&self) -> bool {
        self.sys_active.load(Ordering::SeqCst)
    }

    pub fn mic_heard(&self) -> bool {
        self.mic_heard.load(Ordering::SeqCst)
    }

    pub fn sys_heard(&self) -> bool {
        self.sys_heard.load(Ordering::SeqCst)
    }
}

impl Drop for DualAudioCapture {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Scales an RMS reading into the 0..1 range the Dictation meter draws.
fn meter_level(rms: f32) -> f32 {
    (rms * 5.0).clamp(0.0, 1.0)
}

/// True root-mean-square, unscaled — used for every audibility decision.
fn raw_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|&s| s * s).sum();
    rms_from_sum_sq(sum_sq, samples.len())
}

fn rms_from_sum_sq(sum_sq: f32, count: usize) -> f32 {
    if count == 0 {
        return 0.0;
    }
    (sum_sq / count as f32).sqrt()
}

/// Linear sample-rate conversion to 16 kHz.
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

/// Soft-saturating mix of microphone and system audio.
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

/// Decides how many samples to consume from each FIFO this tick.
///
/// With both streams live the mixer consumes them in exact lockstep, so
/// `mic[t]` is only ever mixed with `sys[t]`. Consuming `max` instead — and
/// zero-padding whichever stream is momentarily behind — would mix real
/// samples against padding and then mix the late arrivals against *future*
/// audio from the other stream, drifting further apart for the rest of the
/// meeting.
///
/// A stream that has gone quiet at the device level (loopback with nothing
/// playing) delivers no callbacks at all, so strict lockstep would stall. Once
/// the other stream is more than [`MAX_STREAM_LAG_SAMPLES`] ahead, the mixer
/// advances anyway and pads the silent side, capping misalignment at that lag
/// rather than letting it accumulate.
fn plan_drain(mic_avail: usize, sys_avail: usize, has_mic: bool, has_sys: bool) -> usize {
    match (has_mic, has_sys) {
        (true, true) => {
            let lockstep = mic_avail.min(sys_avail);
            if lockstep > 0 {
                lockstep
            } else {
                let ahead = mic_avail.max(sys_avail);
                ahead.saturating_sub(MAX_STREAM_LAG_SAMPLES)
            }
        }
        (true, false) => mic_avail,
        (false, true) => sys_avail,
        (false, false) => 0,
    }
}

struct CaptureLoopContext {
    session_id: String,
    mic_active: Arc<AtomicBool>,
    sys_active: Arc<AtomicBool>,
    mic_heard: Arc<AtomicBool>,
    sys_heard: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    app: Option<AppHandle>,
}

/// Accumulator for one pending output slice (durable chunk or live frame).
struct SliceAccumulator {
    samples: Vec<f32>,
    mic_sum_sq: f32,
    sys_sum_sq: f32,
}

impl SliceAccumulator {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            samples: Vec::with_capacity(capacity),
            mic_sum_sq: 0.0,
            sys_sum_sq: 0.0,
        }
    }

    fn push(&mut self, mixed: f32, mic: f32, sys: f32) {
        self.samples.push(mixed);
        self.mic_sum_sq += mic * mic;
        self.sys_sum_sq += sys * sys;
    }

    /// Removes the leading `count` samples, scaling the energy accumulators by
    /// the fraction retained. Exact per-sample energy bookkeeping is not worth
    /// a second buffer here — these values only drive audibility flags.
    fn take_front(&mut self, count: usize) -> (Vec<f32>, f32, f32) {
        let total = self.samples.len();
        let taken: Vec<f32> = self.samples.drain(0..count.min(total)).collect();
        let fraction = if total == 0 {
            0.0
        } else {
            taken.len() as f32 / total as f32
        };
        let mic = self.mic_sum_sq * fraction;
        let sys = self.sys_sum_sq * fraction;
        self.mic_sum_sq -= mic;
        self.sys_sum_sq -= sys;
        (taken, mic, sys)
    }

    fn drain_all(&mut self) -> (Vec<f32>, f32, f32) {
        let samples = std::mem::take(&mut self.samples);
        let mic = std::mem::replace(&mut self.mic_sum_sq, 0.0);
        let sys = std::mem::replace(&mut self.sys_sum_sq, 0.0);
        (samples, mic, sys)
    }

    fn len(&self) -> usize {
        self.samples.len()
    }
}

fn run_dual_capture_loop(
    ctx: CaptureLoopContext,
    stop_rx: std_mpsc::Receiver<()>,
    chunk_tx: std_mpsc::Sender<AudioChunk>,
    live_tx: Option<std_mpsc::SyncSender<LiveAudioFrame>>,
    init_tx: std_mpsc::Sender<Result<CaptureInit, String>>,
) {
    let CaptureLoopContext {
        session_id,
        mic_active,
        sys_active,
        mic_heard,
        sys_heard,
        paused,
        app,
    } = ctx;

    let host = cpal::default_host();

    // Separate FIFOs so the two streams can be consumed in temporal lockstep.
    let mic_fifo: Arc<Mutex<VecDeque<f32>>> = Arc::new(Mutex::new(VecDeque::with_capacity(32_000)));
    let sys_fifo: Arc<Mutex<VecDeque<f32>>> = Arc::new(Mutex::new(VecDeque::with_capacity(32_000)));

    let _mic_stream = build_input_stream(
        host.default_input_device(),
        false,
        &mic_fifo,
        &mic_active,
    );
    let _sys_stream = build_input_stream(
        host.default_output_device(),
        true,
        &sys_fifo,
        &sys_active,
    );

    let has_mic = _mic_stream.is_some();
    let has_sys = _sys_stream.is_some();

    if !has_mic && !has_sys {
        let _ = init_tx.send(Err(
            "No microphone or system audio device could be opened for recording".to_string(),
        ));
        return;
    }

    let _ = init_tx.send(Ok(CaptureInit {
        mic_bound: has_mic,
        sys_bound: has_sys,
    }));

    let mut durable = SliceAccumulator::with_capacity(SAMPLES_PER_CHUNK + 16_000);
    let mut live = SliceAccumulator::with_capacity(SAMPLES_PER_LIVE_FRAME + 16_000);

    let mut chunk_index = 0usize;
    let mut live_frame_index = 0usize;
    let mut elapsed_samples = 0usize;
    let mut live_elapsed_samples = 0usize;

    let mut mic_level = 0.0_f32;
    let mut sys_level = 0.0_f32;
    let mut last_level_emit = std::time::Instant::now();
    let mut was_paused = false;
    let mut pending_discontinuity = false;

    loop {
        let is_stopping = stop_rx.try_recv().is_ok();
        let is_paused = paused.load(Ordering::SeqCst);

        if is_paused {
            // Discard incoming audio rather than letting it queue: the recording
            // must resume contiguously, not replay the paused interval.
            drop_fifo_contents(&mic_fifo);
            drop_fifo_contents(&sys_fifo);
            if !was_paused {
                was_paused = true;
                pending_discontinuity = true;
                mic_level = 0.0;
                sys_level = 0.0;
                emit_levels(&app, 0.0, 0.0);
                last_level_emit = std::time::Instant::now();
            }
            if is_stopping {
                break;
            }
            std::thread::sleep(MIXER_TICK);
            continue;
        }
        was_paused = false;

        // --- Drain both FIFOs in lockstep and mix ---
        let (drained, mic_block_sq, sys_block_sq) = {
            let mut mic_guard = mic_fifo.lock().unwrap();
            let mut sys_guard = sys_fifo.lock().unwrap();
            let count = plan_drain(mic_guard.len(), sys_guard.len(), has_mic, has_sys);

            let mut mic_sq = 0.0_f32;
            let mut sys_sq = 0.0_f32;
            if count > 0 {
                durable.samples.reserve(count);
                live.samples.reserve(count);
                for _ in 0..count {
                    let m = mic_guard.pop_front().unwrap_or(0.0);
                    let s = sys_guard.pop_front().unwrap_or(0.0);
                    let mixed = soft_mix(m, s);
                    mic_sq += m * m;
                    sys_sq += s * s;
                    durable.push(mixed, m, s);
                    live.push(mixed, m, s);
                }
            }
            (count, mic_sq, sys_sq)
        };

        // --- Level meters, computed off the audio callback thread ---
        // Each meter tracks its own source, so the two waveforms stay
        // independent readings rather than two copies of the mix.
        if drained > 0 {
            let mic_rms = meter_level(rms_from_sum_sq(mic_block_sq, drained));
            let sys_rms = meter_level(rms_from_sum_sq(sys_block_sq, drained));
            mic_level += (mic_rms - mic_level) * LEVEL_SMOOTHING_ALPHA;
            sys_level += (sys_rms - sys_level) * LEVEL_SMOOTHING_ALPHA;
        }
        if last_level_emit.elapsed() >= LEVEL_EMIT_INTERVAL {
            last_level_emit = std::time::Instant::now();
            emit_levels(&app, mic_level, sys_level);
        }

        // --- CLOCK B: hand contiguous 1 s frames to the live transcriber ---
        if let Some(ref l_tx) = live_tx {
            while live.len() >= SAMPLES_PER_LIVE_FRAME {
                let (samples, mic_energy, sys_energy) = live.take_front(SAMPLES_PER_LIVE_FRAME);
                let start_time_s = live_elapsed_samples as f64 / TARGET_SAMPLE_RATE as f64;
                live_elapsed_samples += samples.len();
                let end_time_s = live_elapsed_samples as f64 / TARGET_SAMPLE_RATE as f64;

                let count = samples.len();
                let is_speech = raw_rms(&samples) > AUDIBLE_RMS_THRESHOLD;
                mark_heard(&mic_heard, mic_energy, count);
                mark_heard(&sys_heard, sys_energy, count);

                let frame = LiveAudioFrame {
                    session_id: session_id.clone(),
                    frame_index: live_frame_index,
                    start_time_s,
                    end_time_s,
                    samples,
                    capture_instant: std::time::Instant::now(),
                    discontinuity: pending_discontinuity,
                    is_speech,
                };
                pending_discontinuity = false;
                live_frame_index += 1;

                // Never block capture on the live clock.
                match l_tx.try_send(frame) {
                    Ok(_) => {}
                    Err(std_mpsc::TrySendError::Full(_)) => {
                        tracing::warn!(
                            "DualCapture: live STT queue full — dropped live frame (durable recording intact)"
                        );
                    }
                    Err(std_mpsc::TrySendError::Disconnected(_)) => {}
                }
            }
        }

        // --- CLOCK A: slice durable 30 s WAV chunks ---
        while durable.len() >= SAMPLES_PER_CHUNK {
            let (samples, mic_energy, sys_energy) = durable.take_front(SAMPLES_PER_CHUNK);
            emit_chunk(
                &chunk_tx,
                &session_id,
                &mut chunk_index,
                &mut elapsed_samples,
                samples,
                mic_energy,
                sys_energy,
            );
        }

        if is_stopping {
            break;
        }

        std::thread::sleep(MIXER_TICK);
    }

    // Final drain: mix and persist whatever is still buffered so the tail of the
    // meeting is not lost.
    {
        let mut mic_guard = mic_fifo.lock().unwrap();
        let mut sys_guard = sys_fifo.lock().unwrap();
        let remaining = mic_guard.len().max(sys_guard.len());
        for _ in 0..remaining {
            let m = mic_guard.pop_front().unwrap_or(0.0);
            let s = sys_guard.pop_front().unwrap_or(0.0);
            durable.push(soft_mix(m, s), m, s);
        }
    }

    while durable.len() >= SAMPLES_PER_CHUNK {
        let (samples, mic_energy, sys_energy) = durable.take_front(SAMPLES_PER_CHUNK);
        emit_chunk(
            &chunk_tx,
            &session_id,
            &mut chunk_index,
            &mut elapsed_samples,
            samples,
            mic_energy,
            sys_energy,
        );
    }

    if durable.len() > 0 {
        let (samples, mic_energy, sys_energy) = durable.drain_all();
        emit_chunk(
            &chunk_tx,
            &session_id,
            &mut chunk_index,
            &mut elapsed_samples,
            samples,
            mic_energy,
            sys_energy,
        );
    }

    mic_active.store(false, Ordering::SeqCst);
    sys_active.store(false, Ordering::SeqCst);
    emit_levels(&app, 0.0, 0.0);
    tracing::info!("DualCapture: synchronized temporal capture loop exited cleanly.");
}

fn mark_heard(flag: &Arc<AtomicBool>, sum_sq: f32, count: usize) {
    if !flag.load(Ordering::Relaxed) && rms_from_sum_sq(sum_sq, count) > AUDIBLE_RMS_THRESHOLD {
        flag.store(true, Ordering::SeqCst);
    }
}

fn drop_fifo_contents(fifo: &Arc<Mutex<VecDeque<f32>>>) {
    if let Ok(mut guard) = fifo.lock() {
        guard.clear();
    }
}

fn emit_levels(app: &Option<AppHandle>, mic_level: f32, sys_level: f32) {
    if let Some(a) = app {
        let _ = a.emit(
            "meeting-audio-levels",
            &AudioLevels {
                mic_level,
                sys_level,
            },
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_chunk(
    chunk_tx: &std_mpsc::Sender<AudioChunk>,
    session_id: &str,
    chunk_index: &mut usize,
    elapsed_samples: &mut usize,
    samples: Vec<f32>,
    mic_energy: f32,
    sys_energy: f32,
) {
    let count = samples.len();
    let start_time_s = *elapsed_samples as f64 / TARGET_SAMPLE_RATE as f64;
    *elapsed_samples += count;
    let end_time_s = *elapsed_samples as f64 / TARGET_SAMPLE_RATE as f64;

    let chunk = AudioChunk {
        session_id: session_id.to_string(),
        chunk_index: *chunk_index,
        start_time_s,
        end_time_s,
        samples,
        mic_had_audio: rms_from_sum_sq(mic_energy, count) > AUDIBLE_RMS_THRESHOLD,
        sys_had_audio: rms_from_sum_sq(sys_energy, count) > AUDIBLE_RMS_THRESHOLD,
    };

    let _ = chunk_tx.send(chunk);
    *chunk_index += 1;
}

/// Binds one capture stream, returning `None` if the device cannot be opened.
///
/// `loopback` selects the system-audio path, where the "input" is the default
/// *output* device captured in loopback mode.
fn build_input_stream(
    device: Option<cpal::Device>,
    loopback: bool,
    fifo: &Arc<Mutex<VecDeque<f32>>>,
    active_flag: &Arc<AtomicBool>,
) -> Option<cpal::Stream> {
    let label = if loopback { "system audio" } else { "microphone" };
    let device = match device {
        Some(d) => d,
        None => {
            tracing::warn!("DualCapture: no {} device available", label);
            return None;
        }
    };

    let config = if loopback {
        device
            .default_input_config()
            .or_else(|_| device.default_output_config())
    } else {
        device.default_input_config()
    };
    let config = match config {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("DualCapture: no usable {} config: {}", label, e);
            return None;
        }
    };

    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let sample_format = config.sample_format();
    let stream_config: cpal::StreamConfig = config.into();

    // A stream that errors out is no longer capturing; reflect that in state
    // rather than reporting the source as active for the rest of the meeting.
    let err_flag = active_flag.clone();
    let err_label = label.to_string();
    let err_fn = move |err| {
        tracing::error!("DualCapture: {} stream error: {}", err_label, err);
        err_flag.store(false, Ordering::SeqCst);
    };

    macro_rules! build {
        ($sample:ty, $convert:expr) => {{
            let fifo_ref = fifo.clone();
            let flag_ref = active_flag.clone();
            device.build_input_stream(
                &stream_config,
                move |data: &[$sample], _| {
                    process_stream_frames(
                        data,
                        channels,
                        sample_rate,
                        $convert,
                        &fifo_ref,
                        &flag_ref,
                    );
                },
                err_fn,
                None,
            )
        }};
    }

    let stream_res = match sample_format {
        cpal::SampleFormat::F32 => build!(f32, |s: f32| s),
        cpal::SampleFormat::I16 => build!(i16, |s: i16| s as f32 / i16::MAX as f32),
        cpal::SampleFormat::U16 => {
            build!(u16, |s: u16| (s as f32 - u16::MAX as f32 / 2.0)
                / (u16::MAX as f32 / 2.0))
        }
        other => {
            tracing::warn!(
                "DualCapture: unsupported {} sample format {:?}",
                label,
                other
            );
            return None;
        }
    };

    let stream = match stream_res {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("DualCapture: failed to build {} stream: {}", label, e);
            return None;
        }
    };

    if let Err(e) = stream.play() {
        tracing::warn!("DualCapture: failed to start {} stream: {}", label, e);
        return None;
    }

    active_flag.store(true, Ordering::SeqCst);
    tracing::info!(
        "DualCapture: {} stream active ({}ch @ {}Hz)",
        label,
        channels,
        sample_rate
    );
    Some(stream)
}

/// Audio callback: downmix, resample, enqueue. Nothing else belongs here —
/// metering and event emission happen on the mixer thread so this stays cheap
/// and non-blocking.
fn process_stream_frames<T: Copy>(
    data: &[T],
    channels: usize,
    sample_rate: u32,
    to_f32: impl Fn(T) -> f32,
    fifo: &Arc<Mutex<VecDeque<f32>>>,
    active_flag: &Arc<AtomicBool>,
) {
    if data.is_empty() {
        return;
    }

    active_flag.store(true, Ordering::Relaxed);

    let mut mono = Vec::with_capacity(data.len() / channels.max(1));
    if channels > 1 {
        for frame in data.chunks(channels) {
            let sum: f32 = frame.iter().map(|s| to_f32(*s)).sum();
            mono.push(sum / channels as f32);
        }
    } else {
        mono.extend(data.iter().map(|s| to_f32(*s)));
    }

    let resampled = resample_linear(&mono, sample_rate, TARGET_SAMPLE_RATE);

    if let Ok(mut guard) = fifo.lock() {
        guard.extend(resampled);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lockstep_drain_consumes_equal_counts_from_both_streams() {
        // Both streams live: never consume more than the slower one has, so
        // mic[t] is only ever mixed with sys[t].
        assert_eq!(plan_drain(4_000, 3_000, true, true), 3_000);
        assert_eq!(plan_drain(3_000, 4_000, true, true), 3_000);
        assert_eq!(plan_drain(0, 4_000, true, true), 4_000 - MAX_STREAM_LAG_SAMPLES);
    }

    #[test]
    fn a_silent_stream_does_not_stall_the_recording() {
        // WASAPI loopback delivers nothing at all while no audio plays. Once the
        // mic runs further ahead than the lag allowance, the mixer advances.
        let mic_ahead = MAX_STREAM_LAG_SAMPLES * 3;
        let drained = plan_drain(mic_ahead, 0, true, true);
        assert!(drained > 0, "recording must not stall on a silent loopback");
        assert_eq!(drained, mic_ahead - MAX_STREAM_LAG_SAMPLES);

        // Misalignment stays bounded by the allowance rather than accumulating.
        assert!(mic_ahead - drained <= MAX_STREAM_LAG_SAMPLES);
    }

    #[test]
    fn within_the_lag_allowance_the_mixer_waits_instead_of_padding() {
        assert_eq!(plan_drain(MAX_STREAM_LAG_SAMPLES / 2, 0, true, true), 0);
    }

    #[test]
    fn single_device_sessions_drain_only_that_device() {
        assert_eq!(plan_drain(1_000, 0, true, false), 1_000);
        assert_eq!(plan_drain(0, 1_000, false, true), 1_000);
        assert_eq!(plan_drain(1_000, 1_000, false, false), 0);
    }

    #[test]
    fn accumulator_reports_per_source_audibility() {
        let mut acc = SliceAccumulator::with_capacity(16);
        for _ in 0..100 {
            acc.push(0.5, 0.5, 0.0); // mic audible, system silent
        }
        let (samples, mic_energy, sys_energy) = acc.drain_all();
        assert_eq!(samples.len(), 100);
        assert!(rms_from_sum_sq(mic_energy, 100) > AUDIBLE_RMS_THRESHOLD);
        assert!(rms_from_sum_sq(sys_energy, 100) <= AUDIBLE_RMS_THRESHOLD);
    }

    #[test]
    fn accumulator_take_front_splits_energy_with_the_samples() {
        let mut acc = SliceAccumulator::with_capacity(16);
        for _ in 0..200 {
            acc.push(0.4, 0.4, 0.2);
        }
        let (taken, mic_energy, _) = acc.take_front(100);
        assert_eq!(taken.len(), 100);
        assert_eq!(acc.len(), 100);
        // Half the samples carry roughly half the energy, and the rest stays.
        assert!(rms_from_sum_sq(mic_energy, 100) > AUDIBLE_RMS_THRESHOLD);
        assert!(rms_from_sum_sq(acc.mic_sum_sq, 100) > AUDIBLE_RMS_THRESHOLD);
    }

    #[test]
    fn meter_level_saturates_at_full_scale() {
        assert_eq!(meter_level(0.0), 0.0);
        assert!(meter_level(0.1) > 0.0 && meter_level(0.1) <= 1.0);
        assert_eq!(meter_level(5.0), 1.0);
    }

    #[test]
    fn soft_mix_never_exceeds_full_scale() {
        assert!(soft_mix(1.0, 1.0) <= 1.0);
        assert!(soft_mix(-1.0, -1.0) >= -1.0);
        assert_eq!(soft_mix(0.0, 0.0), 0.0);
    }

    #[test]
    fn resampling_halves_sample_count_from_32k_to_16k() {
        let input = vec![0.25_f32; 3_200];
        let out = resample_linear(&input, 32_000, TARGET_SAMPLE_RATE);
        assert_eq!(out.len(), 1_600);
        assert!(out.iter().all(|s| (*s - 0.25).abs() < 1e-6));
    }
}
