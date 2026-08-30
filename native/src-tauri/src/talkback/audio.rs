//! Talkback's own microphone stream.
//!
//! Deliberately separate from `capture::AudioRecorder` and from
//! `meetings_v2::capture::DualAudioCapture`, for reasons that are about
//! timing rather than tidiness:
//!
//! * `AudioRecorder` is a start/stop session that hands back one finished
//!   buffer. Talkback needs frames while the user is still talking.
//! * `DualAudioCapture` is the meeting recorder — dual-source, writing
//!   30-second durable chunks, with crash recovery hanging off its clock.
//!   Reaching into it for a conversational feature is exactly the kind of
//!   change `docs/talkback/ARCHITECTURE.md` §11 rules out.
//!
//! What is *not* duplicated: the resampler
//! ([`crate::capture::resample_to_16k_mono`]) and the transcriber
//! ([`crate::capture::stt::StreamingTranscriber`]) are reused unchanged.
//!
//! The microphone stream is created on enable and dropped on disable.
//! "Talkback off" means the OS-level stream does not exist, not that a
//! running stream is being ignored (`ARCHITECTURE.md` §10).

use crate::capture::resample_to_16k_mono;
use crate::sync::MutexExt;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc as std_mpsc;
use std::sync::{Arc, Mutex};
use thiserror::Error;

/// Whisper's input rate, and the rate everything downstream assumes.
pub const TARGET_SAMPLE_RATE: u32 = 16_000;

/// Frame length handed to the turn detector.
///
/// 100 ms is a compromise the numbers pick for us: barge-in wants to be
/// under ~150 ms end-to-end (`RESEARCH.md` §A), so the detector cannot
/// wait longer than that for its next decision, and anything much shorter
/// makes RMS noisy enough to trip on consonants.
pub const FRAME_MS: u32 = 100;
pub const SAMPLES_PER_FRAME: usize = (TARGET_SAMPLE_RATE as usize * FRAME_MS as usize) / 1000;

#[derive(Error, Debug)]
pub enum TalkbackAudioError {
    #[error("No microphone available: {0}")]
    NoDevice(String),

    #[error("Microphone configuration failed: {0}")]
    Config(String),

    #[error("Microphone stream failed: {0}")]
    Stream(String),
}

/// One frame of microphone audio, already 16 kHz mono.
#[derive(Debug, Clone)]
pub struct MicFrame {
    pub samples: Vec<f32>,
    pub rms: f32,
}

/// Root-mean-square level of a frame. The single number the turn detector
/// and the UI's amplitude animation both run on.
pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f32 = samples
        .iter()
        .map(|s| if s.is_finite() { s * s } else { 0.0 })
        .sum();
    (sum / samples.len() as f32).sqrt()
}

/// Splits an arbitrary-length buffer into fixed frames, returning the
/// frames and whatever remainder did not fill one.
///
/// cpal delivers whatever the OS gives it — 441 samples here, 1024 there
/// — while the detector needs a steady clock. Pure, so the framing is
/// testable without a sound card.
pub fn split_frames(carry: &mut Vec<f32>, incoming: &[f32]) -> Vec<MicFrame> {
    carry.extend_from_slice(incoming);
    let mut frames = Vec::new();
    while carry.len() >= SAMPLES_PER_FRAME {
        let samples: Vec<f32> = carry.drain(..SAMPLES_PER_FRAME).collect();
        let level = rms(&samples);
        frames.push(MicFrame {
            samples,
            rms: level,
        });
    }
    frames
}

/// Holds the live microphone stream for a Talkback session.
///
/// Dropping this closes the stream — which is the whole privacy contract,
/// so it is deliberately not `Clone` and not shareable.
pub struct TalkbackMic {
    stop: Arc<AtomicBool>,
    // Kept alive purely so the stream is not dropped. cpal's `Stream` is
    // `!Send`, so it lives on its own thread and this is the handle that
    // tells that thread to let go.
    worker: Option<std::thread::JoinHandle<()>>,
}

impl TalkbackMic {
    /// Opens the default input device and sends frames to `frame_tx`.
    ///
    /// Returns once the device is open, so a failure to acquire the
    /// microphone surfaces as an error the user can see rather than as
    /// silence.
    pub fn start(
        frame_tx: std_mpsc::Sender<MicFrame>,
    ) -> Result<Self, TalkbackAudioError> {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_for_thread = stop.clone();
        let (ready_tx, ready_rx) = std_mpsc::channel::<Result<(), TalkbackAudioError>>();

        let worker = std::thread::spawn(move || {
            let result = run_stream(frame_tx, stop_for_thread);
            // The receiver is gone if `start` already timed out; nothing
            // to do about it but continue to shutdown.
            let _ = ready_tx.send(result);
        });

        match ready_rx.recv_timeout(std::time::Duration::from_secs(5)) {
            // A completed `run_stream` means the stream ended; that is
            // only an error if it ended before it began, which is what
            // an `Err` says.
            Ok(Ok(())) => Ok(Self {
                stop,
                worker: Some(worker),
            }),
            Ok(Err(e)) => Err(e),
            Err(_) => Ok(Self {
                stop,
                worker: Some(worker),
            }),
        }
    }

    /// Closes the microphone stream and waits for the thread to exit.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for TalkbackMic {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Opens the device, runs until stopped, then lets the stream drop.
fn run_stream(
    frame_tx: std_mpsc::Sender<MicFrame>,
    stop: Arc<AtomicBool>,
) -> Result<(), TalkbackAudioError> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| TalkbackAudioError::NoDevice("no default input device".to_string()))?;
    let config = device
        .default_input_config()
        .map_err(|e| TalkbackAudioError::Config(e.to_string()))?;

    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let carry = Arc::new(Mutex::new(Vec::<f32>::new()));

    let stream = build_stream(&device, &config, sample_rate, channels, carry, frame_tx)?;
    stream
        .play()
        .map_err(|e| TalkbackAudioError::Stream(e.to_string()))?;

    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    drop(stream);
    Ok(())
}

fn build_stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    sample_rate: u32,
    channels: usize,
    carry: Arc<Mutex<Vec<f32>>>,
    frame_tx: std_mpsc::Sender<MicFrame>,
) -> Result<cpal::Stream, TalkbackAudioError> {
    let stream_config: cpal::StreamConfig = config.config();
    let on_error = |e| tracing::warn!("talkback: microphone stream error: {}", e);

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &stream_config,
            move |data: &[f32], _: &_| {
                handle_input(data, channels, sample_rate, &carry, &frame_tx, |s| s);
            },
            on_error,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            &stream_config,
            move |data: &[i16], _: &_| {
                handle_input(data, channels, sample_rate, &carry, &frame_tx, |s| {
                    s as f32 / i16::MAX as f32
                });
            },
            on_error,
            None,
        ),
        cpal::SampleFormat::U16 => device.build_input_stream(
            &stream_config,
            move |data: &[u16], _: &_| {
                handle_input(data, channels, sample_rate, &carry, &frame_tx, |s| {
                    (s as f32 / u16::MAX as f32) * 2.0 - 1.0
                });
            },
            on_error,
            None,
        ),
        format => {
            return Err(TalkbackAudioError::Config(format!(
                "unsupported sample format {format:?}"
            )))
        }
    };

    stream.map_err(|e| TalkbackAudioError::Stream(e.to_string()))
}

/// Downmixes, resamples and frames one cpal callback's worth of audio.
fn handle_input<T: Copy>(
    data: &[T],
    channels: usize,
    sample_rate: u32,
    carry: &Arc<Mutex<Vec<f32>>>,
    frame_tx: &std_mpsc::Sender<MicFrame>,
    to_f32: impl Fn(T) -> f32,
) {
    let mono: Vec<f32> = if channels > 1 {
        data.chunks(channels)
            .map(|frame| frame.iter().map(|s| to_f32(*s)).sum::<f32>() / channels as f32)
            .collect()
    } else {
        data.iter().map(|s| to_f32(*s)).collect()
    };

    let resampled = if sample_rate == TARGET_SAMPLE_RATE {
        mono
    } else {
        resample_to_16k_mono(&mono, sample_rate)
    };

    let mut guard = carry.lock_or_recover();
    for frame in split_frames(&mut guard, &resampled) {
        // A closed receiver means the session ended between callbacks;
        // dropping the frame is correct, and must not panic the audio
        // thread.
        if frame_tx.send(frame).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rms_of_silence_is_zero() {
        assert_eq!(rms(&[0.0; 100]), 0.0);
        assert_eq!(rms(&[]), 0.0);
    }

    #[test]
    fn rms_of_a_constant_signal_is_its_magnitude() {
        assert!((rms(&[0.5; 100]) - 0.5).abs() < 1e-6);
        assert!((rms(&[-0.5; 100]) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn rms_ignores_non_finite_samples_rather_than_producing_nan() {
        let level = rms(&[f32::NAN, f32::INFINITY, 0.5, 0.5]);
        assert!(level.is_finite(), "got {level}");
        assert!(level > 0.0);
    }

    #[test]
    fn frames_are_exactly_one_hundred_milliseconds() {
        let mut carry = Vec::new();
        let frames = split_frames(&mut carry, &vec![0.1; SAMPLES_PER_FRAME * 3]);
        assert_eq!(frames.len(), 3);
        for frame in &frames {
            assert_eq!(frame.samples.len(), SAMPLES_PER_FRAME);
        }
        assert!(carry.is_empty());
    }

    #[test]
    fn a_partial_buffer_is_carried_to_the_next_callback() {
        let mut carry = Vec::new();
        // Two callbacks that individually fall short but together make a
        // frame — the exact thing cpal's variable buffer sizes cause.
        assert!(split_frames(&mut carry, &vec![0.1; SAMPLES_PER_FRAME - 10]).is_empty());
        assert_eq!(carry.len(), SAMPLES_PER_FRAME - 10);

        let frames = split_frames(&mut carry, &[0.1; 20]);
        assert_eq!(frames.len(), 1);
        assert_eq!(carry.len(), 10, "the remainder is kept, not dropped");
    }

    #[test]
    fn odd_buffer_sizes_never_lose_or_duplicate_samples() {
        let mut carry = Vec::new();
        let mut emitted = 0_usize;
        // 441 is a real cpal buffer size at 44.1kHz.
        for _ in 0..50 {
            emitted += split_frames(&mut carry, &vec![0.2; 441])
                .iter()
                .map(|f| f.samples.len())
                .sum::<usize>();
        }
        assert_eq!(emitted + carry.len(), 50 * 441);
    }

    #[test]
    fn frame_level_is_computed_per_frame() {
        let mut carry = Vec::new();
        let mut samples = vec![0.0; SAMPLES_PER_FRAME];
        samples.extend(vec![0.8; SAMPLES_PER_FRAME]);
        let frames = split_frames(&mut carry, &samples);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].rms, 0.0);
        // Loose tolerance on purpose: 1600 f32 additions per frame carry
        // real rounding error, and the level only drives a threshold
        // comparison and an animation.
        assert!((frames[1].rms - 0.8).abs() < 1e-3, "{}", frames[1].rms);
    }

    #[test]
    fn the_frame_size_matches_the_declared_frame_duration() {
        assert_eq!(SAMPLES_PER_FRAME, 1_600);
        assert_eq!(
            SAMPLES_PER_FRAME as u32 * 1000 / TARGET_SAMPLE_RATE,
            FRAME_MS
        );
    }
}
