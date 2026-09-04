use super::types::AudioLevels;
use crate::sync::MutexExt;
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

/// Resolution of the per-source channel energy track carried on every chunk —
/// one bucket per second at the target rate.
///
/// The mixer already measures both sources sample by sample in order to set
/// `mic_had_audio` / `sys_had_audio`; before v2.5 it summed that measurement
/// across the whole 30-second chunk and kept one boolean per source. In a real
/// two-way conversation almost every 30-second window contains both sources, so
/// chunk-level attribution resolved to "unknown" for the majority of segments
/// and every action item read out of them lost its owner.
///
/// Bucketing the same measurement per second costs ~30 float pairs per chunk and
/// lets attribution resolve at roughly sentence granularity once Whisper's own
/// segment timestamps are matched against it. Both [`SAMPLES_PER_CHUNK`] and
/// [`SAMPLES_PER_LIVE_FRAME`] are exact multiples of this, so slicing never
/// splits a bucket in the steady state.
const SAMPLES_PER_ENERGY_BUCKET: usize = TARGET_SAMPLE_RATE as usize;

const LEVEL_SMOOTHING_ALPHA: f32 = 0.35;
const LEVEL_EMIT_INTERVAL: Duration = Duration::from_millis(40);
const MIXER_TICK: Duration = Duration::from_millis(20);

/// One second of per-source loudness, measured on the samples that were mixed.
///
/// Kept as RMS rather than a boolean so a later stage can pick its own
/// threshold, and so a marginal second is distinguishable from a silent one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelEnergy {
    /// Offset of this bucket from the start of its chunk, in seconds.
    pub offset_s: f64,
    pub duration_s: f64,
    pub mic_rms: f32,
    pub sys_rms: f32,
}

pub struct AudioChunk {
    pub session_id: String,
    pub chunk_index: usize,
    pub start_time_s: f64,
    pub end_time_s: f64,
    pub samples: Vec<f32>,
    /// Whether each source was audible *within this chunk*, measured from the
    /// samples that went into it rather than from an instantaneous meter read.
    ///
    /// Retained unchanged: these are the chunk-wide roll-up of
    /// [`AudioChunk::channel_track`] and every existing consumer still reads
    /// them.
    pub mic_had_audio: bool,
    pub sys_had_audio: bool,
    /// The same measurement at one-second resolution, oldest first. Empty only
    /// if the chunk carried no samples.
    pub channel_track: Vec<ChannelEnergy>,
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
/// The decibel window the level meter maps onto bar height.
///
/// Conversational speech sits around -34 dBFS RMS. The floor is set below a
/// quiet room and the ceiling below full scale, so a normal speaking voice lands
/// near the middle of the bar rather than against either end.
const METER_FLOOR_DB: f32 = -55.0;
const METER_CEILING_DB: f32 = -15.0;

/// Maps an RMS reading onto `0.0..=1.0` for the recording pill's waveform.
///
/// Display only — every audibility decision uses [`raw_rms`] directly, so this
/// curve can be tuned for the eye without touching what the recorder considers
/// audible.
///
/// A meter is a perceptual instrument, not a linear one. Scaling RMS linearly
/// (the previous `rms * 5.0`) put speech at 0.1–0.4 of full scale: bars that
/// moved by two or three pixels and read as a flat line. Mapping a decibel
/// window instead puts speech where it can actually be seen.
fn meter_level(rms: f32) -> f32 {
    if rms <= AUDIBLE_RMS_THRESHOLD {
        // Below what the rest of the pipeline already calls silence, the meter
        // reads flat rather than animating the room tone.
        return 0.0;
    }
    let db = 20.0 * rms.log10();
    ((db - METER_FLOOR_DB) / (METER_CEILING_DB - METER_FLOOR_DB)).clamp(0.0, 1.0)
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

/// One bucket of the channel energy track, still accumulating.
#[derive(Debug, Clone, Copy, Default)]
struct EnergyBucket {
    mic_sum_sq: f32,
    sys_sum_sq: f32,
    count: usize,
}

impl EnergyBucket {
    /// Splits off the leading `count` samples' worth of energy, apportioning the
    /// sums by the fraction taken. Only ever called on a bucket a slice boundary
    /// lands inside — which, given both slice sizes are exact multiples of
    /// [`SAMPLES_PER_ENERGY_BUCKET`], happens only on the final drain.
    fn split_front(&mut self, count: usize) -> EnergyBucket {
        let take = count.min(self.count);
        let fraction = if self.count == 0 {
            0.0
        } else {
            take as f32 / self.count as f32
        };
        let front = EnergyBucket {
            mic_sum_sq: self.mic_sum_sq * fraction,
            sys_sum_sq: self.sys_sum_sq * fraction,
            count: take,
        };
        self.mic_sum_sq -= front.mic_sum_sq;
        self.sys_sum_sq -= front.sys_sum_sq;
        self.count -= take;
        front
    }
}

/// Accumulator for one pending output slice (durable chunk or live frame).
///
/// Channel energy is bucketed per second rather than summed across the whole
/// slice, so a slice can report *when* each source was audible instead of only
/// whether it ever was. The chunk-wide booleans are recovered by summing the
/// buckets, which is arithmetically identical to the single running total this
/// held before v2.5.
struct SliceAccumulator {
    samples: Vec<f32>,
    buckets: VecDeque<EnergyBucket>,
}

impl SliceAccumulator {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            samples: Vec::with_capacity(capacity),
            buckets: VecDeque::new(),
        }
    }

    fn push(&mut self, mixed: f32, mic: f32, sys: f32) {
        self.samples.push(mixed);
        if self
            .buckets
            .back()
            .is_none_or(|b| b.count >= SAMPLES_PER_ENERGY_BUCKET)
        {
            self.buckets.push_back(EnergyBucket::default());
        }
        let bucket = self
            .buckets
            .back_mut()
            .expect("a bucket was just ensured to exist");
        bucket.mic_sum_sq += mic * mic;
        bucket.sys_sum_sq += sys * sys;
        bucket.count += 1;
    }

    /// Removes the leading `count` samples together with the energy buckets
    /// covering them.
    fn take_front(&mut self, count: usize) -> (Vec<f32>, Vec<EnergyBucket>) {
        let total = self.samples.len();
        let taken: Vec<f32> = self.samples.drain(0..count.min(total)).collect();

        let mut remaining = taken.len();
        let mut buckets = Vec::new();
        while remaining > 0 {
            let Some(front) = self.buckets.front_mut() else {
                break;
            };
            if front.count <= remaining {
                remaining -= front.count;
                buckets.push(self.buckets.pop_front().expect("front was just borrowed"));
            } else {
                buckets.push(front.split_front(remaining));
                remaining = 0;
            }
        }
        (taken, buckets)
    }

    fn drain_all(&mut self) -> (Vec<f32>, Vec<EnergyBucket>) {
        let samples = std::mem::take(&mut self.samples);
        let buckets = std::mem::take(&mut self.buckets).into_iter().collect();
        (samples, buckets)
    }

    fn len(&self) -> usize {
        self.samples.len()
    }
}

/// Converts accumulated buckets into the chunk's channel track plus the
/// chunk-wide audibility flags.
fn resolve_channel_track(buckets: &[EnergyBucket]) -> (Vec<ChannelEnergy>, bool, bool) {
    let mut offset_s = 0.0_f64;
    let mut mic_total = 0.0_f32;
    let mut sys_total = 0.0_f32;
    let mut total_samples = 0usize;
    let mut track = Vec::with_capacity(buckets.len());

    for bucket in buckets {
        if bucket.count == 0 {
            continue;
        }
        let duration_s = bucket.count as f64 / TARGET_SAMPLE_RATE as f64;
        track.push(ChannelEnergy {
            offset_s,
            duration_s,
            mic_rms: rms_from_sum_sq(bucket.mic_sum_sq, bucket.count),
            sys_rms: rms_from_sum_sq(bucket.sys_sum_sq, bucket.count),
        });
        offset_s += duration_s;
        mic_total += bucket.mic_sum_sq;
        sys_total += bucket.sys_sum_sq;
        total_samples += bucket.count;
    }

    let mic_had_audio = rms_from_sum_sq(mic_total, total_samples) > AUDIBLE_RMS_THRESHOLD;
    let sys_had_audio = rms_from_sum_sq(sys_total, total_samples) > AUDIBLE_RMS_THRESHOLD;
    (track, mic_had_audio, sys_had_audio)
}

/// How much louder one source must be than the other before the quieter one is
/// treated as leakage rather than as a second speaker.
///
/// Device-level capture has no acoustic isolation: with speakers rather than
/// headphones, the microphone picks up the remote party, and every utterance
/// would otherwise register both channels as audible and resolve to no speaker
/// at all. Requiring roughly a 10 dB margin rejects that bleed without
/// guessing — it encodes a physical property of the room, not an inference about
/// who was talking. Genuine crosstalk fails the margin in both directions and is
/// correctly left unresolved.
const CHANNEL_DOMINANCE_RATIO: f32 = 3.0;

/// Which sources were audible during `[start_offset_s, end_offset_s)` of a chunk.
///
/// Offsets are relative to the start of the chunk, matching both the channel
/// track's own offsets and the timestamps Whisper reports for a decode.
///
/// Loudness is averaged over the overlapping buckets in proportion to how much
/// of each falls inside the span, so a bucket straddling a speaker change
/// contributes to both neighbours in proportion rather than marking both of them
/// ambiguous. `(false, false)` means the span was silent or fell outside the
/// track; the caller decides what to do with that.
/// Mean loudness of each source across one utterance's span.
///
/// Returned alongside the audible/not verdict because the verdict alone cannot
/// answer "whose voice is this": with speakers rather than headphones both
/// sources register on nearly every utterance, and only their *relative*
/// loudness separates the person at this machine from the people on the call.
pub fn utterance_channel_energy(
    track: &[ChannelEnergy],
    start_offset_s: f64,
    end_offset_s: f64,
) -> (f32, f32) {
    let mut mic_weighted = 0.0_f64;
    let mut sys_weighted = 0.0_f64;
    let mut weight = 0.0_f64;

    for bucket in track {
        let bucket_end = bucket.offset_s + bucket.duration_s;
        let overlap = end_offset_s.min(bucket_end) - start_offset_s.max(bucket.offset_s);
        if overlap <= 0.0 {
            continue;
        }
        mic_weighted += bucket.mic_rms as f64 * overlap;
        sys_weighted += bucket.sys_rms as f64 * overlap;
        weight += overlap;
    }

    if weight <= 0.0 {
        return (0.0, 0.0);
    }
    ((mic_weighted / weight) as f32, (sys_weighted / weight) as f32)
}

pub fn resolve_utterance_channel(
    track: &[ChannelEnergy],
    start_offset_s: f64,
    end_offset_s: f64,
) -> (bool, bool) {
    let mut mic_weighted = 0.0_f64;
    let mut sys_weighted = 0.0_f64;
    let mut weight = 0.0_f64;

    for bucket in track {
        let bucket_end = bucket.offset_s + bucket.duration_s;
        let overlap = end_offset_s.min(bucket_end) - start_offset_s.max(bucket.offset_s);
        if overlap <= 0.0 {
            continue;
        }
        mic_weighted += bucket.mic_rms as f64 * overlap;
        sys_weighted += bucket.sys_rms as f64 * overlap;
        weight += overlap;
    }

    if weight <= 0.0 {
        return (false, false);
    }

    let mic_mean = (mic_weighted / weight) as f32;
    let sys_mean = (sys_weighted / weight) as f32;
    let mut mic_audible = mic_mean > AUDIBLE_RMS_THRESHOLD;
    let mut sys_audible = sys_mean > AUDIBLE_RMS_THRESHOLD;

    if mic_audible && sys_audible {
        if mic_mean >= sys_mean * CHANNEL_DOMINANCE_RATIO {
            sys_audible = false;
        } else if sys_mean >= mic_mean * CHANNEL_DOMINANCE_RATIO {
            mic_audible = false;
        }
    }

    (mic_audible, sys_audible)
}

/// Total accumulated energy across buckets, for the `*_heard` session flags.
fn bucket_totals(buckets: &[EnergyBucket]) -> (f32, f32, usize) {
    buckets.iter().fold((0.0, 0.0, 0), |(m, s, c), b| {
        (m + b.mic_sum_sq, s + b.sys_sum_sq, c + b.count)
    })
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
            let mut mic_guard = mic_fifo.lock_or_recover();
            let mut sys_guard = sys_fifo.lock_or_recover();
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
                let (samples, buckets) = live.take_front(SAMPLES_PER_LIVE_FRAME);
                let start_time_s = live_elapsed_samples as f64 / TARGET_SAMPLE_RATE as f64;
                live_elapsed_samples += samples.len();
                let end_time_s = live_elapsed_samples as f64 / TARGET_SAMPLE_RATE as f64;

                let is_speech = raw_rms(&samples) > AUDIBLE_RMS_THRESHOLD;
                let (mic_energy, sys_energy, count) = bucket_totals(&buckets);
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
            let (samples, buckets) = durable.take_front(SAMPLES_PER_CHUNK);
            emit_chunk(
                &chunk_tx,
                &session_id,
                &mut chunk_index,
                &mut elapsed_samples,
                samples,
                &buckets,
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
        let mut mic_guard = mic_fifo.lock_or_recover();
        let mut sys_guard = sys_fifo.lock_or_recover();
        let remaining = mic_guard.len().max(sys_guard.len());
        for _ in 0..remaining {
            let m = mic_guard.pop_front().unwrap_or(0.0);
            let s = sys_guard.pop_front().unwrap_or(0.0);
            durable.push(soft_mix(m, s), m, s);
        }
    }

    while durable.len() >= SAMPLES_PER_CHUNK {
        let (samples, buckets) = durable.take_front(SAMPLES_PER_CHUNK);
        emit_chunk(
            &chunk_tx,
            &session_id,
            &mut chunk_index,
            &mut elapsed_samples,
            samples,
            &buckets,
        );
    }

    if durable.len() > 0 {
        let (samples, buckets) = durable.drain_all();
        emit_chunk(
            &chunk_tx,
            &session_id,
            &mut chunk_index,
            &mut elapsed_samples,
            samples,
            &buckets,
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

fn emit_chunk(
    chunk_tx: &std_mpsc::Sender<AudioChunk>,
    session_id: &str,
    chunk_index: &mut usize,
    elapsed_samples: &mut usize,
    samples: Vec<f32>,
    buckets: &[EnergyBucket],
) {
    let count = samples.len();
    let start_time_s = *elapsed_samples as f64 / TARGET_SAMPLE_RATE as f64;
    *elapsed_samples += count;
    let end_time_s = *elapsed_samples as f64 / TARGET_SAMPLE_RATE as f64;

    let (channel_track, mic_had_audio, sys_had_audio) = resolve_channel_track(buckets);

    let chunk = AudioChunk {
        session_id: session_id.to_string(),
        chunk_index: *chunk_index,
        start_time_s,
        end_time_s,
        samples,
        mic_had_audio,
        sys_had_audio,
        channel_track,
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
        let (samples, buckets) = acc.drain_all();
        assert_eq!(samples.len(), 100);
        let (_, mic_had_audio, sys_had_audio) = resolve_channel_track(&buckets);
        assert!(mic_had_audio);
        assert!(!sys_had_audio);
    }

    #[test]
    fn accumulator_take_front_splits_energy_with_the_samples() {
        let mut acc = SliceAccumulator::with_capacity(16);
        for _ in 0..200 {
            acc.push(0.4, 0.4, 0.2);
        }
        let (taken, buckets) = acc.take_front(100);
        assert_eq!(taken.len(), 100);
        assert_eq!(acc.len(), 100);
        // Half the samples carry roughly half the energy, and the rest stays.
        let (mic_energy, _, count) = bucket_totals(&buckets);
        assert_eq!(count, 100);
        assert!(rms_from_sum_sq(mic_energy, count) > AUDIBLE_RMS_THRESHOLD);
        let (_, remaining) = acc.drain_all();
        let (mic_rest, _, rest_count) = bucket_totals(&remaining);
        assert_eq!(rest_count, 100);
        assert!(rms_from_sum_sq(mic_rest, rest_count) > AUDIBLE_RMS_THRESHOLD);
    }

    #[test]
    fn the_channel_track_resolves_one_bucket_per_second() {
        let mut acc = SliceAccumulator::with_capacity(SAMPLES_PER_CHUNK);
        // Three seconds: mic alone, then system alone, then both.
        for _ in 0..SAMPLES_PER_ENERGY_BUCKET {
            acc.push(0.5, 0.5, 0.0);
        }
        for _ in 0..SAMPLES_PER_ENERGY_BUCKET {
            acc.push(0.5, 0.0, 0.5);
        }
        for _ in 0..SAMPLES_PER_ENERGY_BUCKET {
            acc.push(0.5, 0.4, 0.4);
        }

        let (_, buckets) = acc.drain_all();
        let (track, mic_had_audio, sys_had_audio) = resolve_channel_track(&buckets);

        assert_eq!(track.len(), 3);
        // Chunk-wide flags stay what they always were: both sources were heard.
        assert!(mic_had_audio && sys_had_audio);

        // But the track localizes each one, which the flags alone cannot.
        assert!(track[0].mic_rms > AUDIBLE_RMS_THRESHOLD);
        assert!(track[0].sys_rms <= AUDIBLE_RMS_THRESHOLD);
        assert!(track[1].mic_rms <= AUDIBLE_RMS_THRESHOLD);
        assert!(track[1].sys_rms > AUDIBLE_RMS_THRESHOLD);
        assert!(track[2].mic_rms > AUDIBLE_RMS_THRESHOLD);
        assert!(track[2].sys_rms > AUDIBLE_RMS_THRESHOLD);

        // Offsets are contiguous and one second apart.
        assert_eq!(track[0].offset_s, 0.0);
        assert!((track[1].offset_s - 1.0).abs() < 1e-9);
        assert!((track[2].offset_s - 2.0).abs() < 1e-9);
        assert!(track.iter().all(|b| (b.duration_s - 1.0).abs() < 1e-9));
    }

    #[test]
    fn a_chunk_sized_slice_never_splits_a_bucket() {
        let mut acc = SliceAccumulator::with_capacity(SAMPLES_PER_CHUNK);
        for _ in 0..SAMPLES_PER_CHUNK + SAMPLES_PER_ENERGY_BUCKET {
            acc.push(0.3, 0.3, 0.1);
        }
        let (samples, buckets) = acc.take_front(SAMPLES_PER_CHUNK);
        assert_eq!(samples.len(), SAMPLES_PER_CHUNK);
        assert_eq!(buckets.len(), SAMPLES_PER_CHUNK / SAMPLES_PER_ENERGY_BUCKET);
        assert!(buckets
            .iter()
            .all(|b| b.count == SAMPLES_PER_ENERGY_BUCKET));
    }

    /// Builds a channel track from one `(mic_rms, sys_rms)` pair per second.
    fn track(seconds: &[(f32, f32)]) -> Vec<ChannelEnergy> {
        seconds
            .iter()
            .enumerate()
            .map(|(i, &(mic_rms, sys_rms))| ChannelEnergy {
                offset_s: i as f64,
                duration_s: 1.0,
                mic_rms,
                sys_rms,
            })
            .collect()
    }

    const LOUD: f32 = 0.20;
    const QUIET: f32 = 0.001;
    /// Speaker bleed into the microphone: audible, but far below the real source.
    const BLEED: f32 = 0.02;

    #[test]
    fn an_utterance_resolves_to_whichever_source_was_speaking() {
        // Me for three seconds, then them for five.
        let t = track(&[
            (LOUD, QUIET),
            (LOUD, QUIET),
            (LOUD, QUIET),
            (QUIET, LOUD),
            (QUIET, LOUD),
            (QUIET, LOUD),
            (QUIET, LOUD),
            (QUIET, LOUD),
        ]);

        assert_eq!(resolve_utterance_channel(&t, 0.0, 3.0), (true, false));
        assert_eq!(resolve_utterance_channel(&t, 3.0, 8.0), (false, true));
    }

    #[test]
    fn chunk_wide_flags_cannot_make_this_distinction() {
        // The exact case the chunk booleans collapse: both sources were heard
        // during the chunk, so `(true, true)` -> Mixed -> no speaker at all.
        let t = track(&[(LOUD, QUIET), (QUIET, LOUD)]);
        let (mic_any, sys_any) = (
            t.iter().any(|b| b.mic_rms > AUDIBLE_RMS_THRESHOLD),
            t.iter().any(|b| b.sys_rms > AUDIBLE_RMS_THRESHOLD),
        );
        assert_eq!((mic_any, sys_any), (true, true));

        // Per utterance, each second resolves to exactly one source.
        assert_eq!(resolve_utterance_channel(&t, 0.0, 1.0), (true, false));
        assert_eq!(resolve_utterance_channel(&t, 1.0, 2.0), (false, true));
    }

    #[test]
    fn microphone_bleed_from_the_speakers_is_not_a_second_speaker() {
        let t = track(&[(BLEED, LOUD), (BLEED, LOUD)]);
        // The premise of this test: bleed is loud enough to pass the audible
        // gate, so what follows is about channel attribution, not loudness.
        // (Both operands are consts, so clippy sees the comparison as
        // constant — it is, deliberately, and it guards the fixture values.)
        #[allow(clippy::assertions_on_constants)]
        {
            assert!(BLEED > AUDIBLE_RMS_THRESHOLD, "bleed must clear the gate");
        }
        assert_eq!(resolve_utterance_channel(&t, 0.0, 2.0), (false, true));
    }

    #[test]
    fn genuine_crosstalk_stays_unresolved_rather_than_guessed() {
        let t = track(&[(LOUD, LOUD), (LOUD, LOUD)]);
        assert_eq!(resolve_utterance_channel(&t, 0.0, 2.0), (true, true));
    }

    #[test]
    fn a_straddling_bucket_is_outweighed_by_an_utterance_own_seconds() {
        // Three seconds of me, one second of crosstalk, three seconds of them.
        // Each utterance takes half of the crosstalk second and still resolves,
        // because its own seconds dominate the weighted mean.
        let t = track(&[
            (LOUD, QUIET),
            (LOUD, QUIET),
            (LOUD, QUIET),
            (LOUD, LOUD),
            (QUIET, LOUD),
            (QUIET, LOUD),
            (QUIET, LOUD),
        ]);
        assert_eq!(resolve_utterance_channel(&t, 0.0, 3.5), (true, false));
        assert_eq!(resolve_utterance_channel(&t, 3.5, 7.0), (false, true));
    }

    #[test]
    fn a_short_utterance_mostly_inside_crosstalk_stays_unresolved() {
        // The honest limit of the mechanism, asserted rather than hidden: when a
        // straddling second is a large fraction of a short utterance, neither
        // source clears the dominance margin and the utterance keeps no speaker.
        // Consistent with the module's rule that ambiguity is preserved, and the
        // reason `speaker_id` stays `None` rather than being guessed.
        let t = track(&[(LOUD, QUIET), (LOUD, LOUD)]);
        assert_eq!(resolve_utterance_channel(&t, 0.0, 1.5), (true, true));
    }

    #[test]
    fn silence_and_out_of_range_spans_resolve_to_nothing() {
        let t = track(&[(QUIET, QUIET)]);
        assert_eq!(resolve_utterance_channel(&t, 0.0, 1.0), (false, false));
        assert_eq!(resolve_utterance_channel(&t, 5.0, 6.0), (false, false));
        assert_eq!(resolve_utterance_channel(&[], 0.0, 1.0), (false, false));
    }

    #[test]
    fn meter_level_saturates_at_full_scale() {
        assert_eq!(meter_level(0.0), 0.0);
        assert!(meter_level(0.1) > 0.0 && meter_level(0.1) <= 1.0);
        assert_eq!(meter_level(5.0), 1.0);
    }

    #[test]
    fn the_meter_puts_conversational_speech_in_the_visible_middle() {
        // Silence and room tone read flat.
        assert_eq!(meter_level(0.0), 0.0);
        assert_eq!(meter_level(0.001), 0.0, "room tone is not a waveform");

        // A normal speaking voice — around -34 dBFS — must land where a bar is
        // actually legible, not pinned near the floor. This is the regression:
        // the linear meter put this at 0.1.
        let speech = meter_level(0.02);
        assert!(
            (0.35..=0.75).contains(&speech),
            "speech read {speech}, which is not a visible bar"
        );

        // Loud speech is clearly higher again, and nothing exceeds full scale.
        assert!(meter_level(0.1) > speech);
        assert!(meter_level(0.9) <= 1.0);

        // The curve is monotonic across the audible range.
        let mut previous = 0.0;
        for step in 1..=40 {
            let level = meter_level(step as f32 * 0.005);
            assert!(level >= previous, "meter is not monotonic at step {step}");
            previous = level;
        }
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
