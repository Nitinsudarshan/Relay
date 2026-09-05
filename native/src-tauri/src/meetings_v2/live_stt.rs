use super::capture::{LiveAudioFrame, TARGET_SAMPLE_RATE};
use super::transcript_health::{self, DecodeEvidence};
use super::types::LiveTranscriptUpdate;
use crate::capture::stt::{SttLanguageConfig, StreamingTranscriber};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc as std_mpsc;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

/// Longest an utterance may grow before it is committed regardless of silence.
/// Also bounds the cost of each decode, since the whole window is re-decoded
/// every tick.
const MAX_UTTERANCE_SECS: f64 = 12.0;
const MAX_UTTERANCE_SAMPLES: usize = (TARGET_SAMPLE_RATE as f64 * MAX_UTTERANCE_SECS) as usize;

/// Consecutive silent frames (1 s each) that close an utterance.
const SILENT_FRAMES_TO_COMMIT: usize = 1;

/// Threads used for live decoding. Deliberately below the core count so the
/// durable clock's 30-second decodes and the rest of the app still get CPU.
pub fn live_thread_count() -> i32 {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get() as i32)
        .unwrap_or(4);
    (cores / 2).clamp(1, 4)
}

/// The live transcription clock (Clock B).
///
/// Rather than transcribing fixed overlapping windows and stitching the results
/// with word-level de-duplication, this keeps a rolling *utterance* buffer:
/// every tick it re-decodes everything accumulated since the last commit and
/// emits the result as a partial update under a stable segment id. The UI
/// replaces the partial as it improves; when a silence boundary or the window
/// cap arrives, the same text is emitted once more as final and the buffer
/// resets.
///
/// This design exists because the previous one lost text outright. Windows of
/// 1.5 s routinely end on a lone timestamp token, which makes whisper.cpp
/// discard the entire window ("single timestamp ending - skip entire chunk"),
/// so the live stream simply went quiet for seconds at a time. Growing windows
/// plus `single_segment` decoding avoid that failure mode, and no de-duplication
/// heuristic is needed because consecutive updates replace rather than append.
pub struct LiveSttWorker {
    join_handle: Option<std::thread::JoinHandle<()>>,
    stop_flag: Arc<AtomicBool>,
}

impl LiveSttWorker {
    pub fn spawn(
        session_id: String,
        live_rx: std_mpsc::Receiver<LiveAudioFrame>,
        whisper_model_path: Option<PathBuf>,
        language_config: SttLanguageConfig,
        app: Option<AppHandle>,
    ) -> Self {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();

        let mut effective_lang_config = language_config;
        effective_lang_config.translate = false;

        let handle = std::thread::spawn(move || {
            run_live_loop(
                session_id,
                live_rx,
                whisper_model_path,
                effective_lang_config,
                app,
                stop_flag_clone,
            );
        });

        Self {
            join_handle: Some(handle),
            stop_flag,
        }
    }

    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }

    pub fn join(&mut self) {
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

/// The utterance currently being accumulated and re-decoded.
struct Utterance {
    index: usize,
    samples: Vec<f32>,
    start_time_s: f64,
    end_time_s: f64,
    has_speech: bool,
    silent_frames: usize,
    last_text: String,
}

impl Utterance {
    fn new(index: usize) -> Self {
        Self {
            index,
            samples: Vec::with_capacity(MAX_UTTERANCE_SAMPLES),
            start_time_s: 0.0,
            end_time_s: 0.0,
            has_speech: false,
            silent_frames: 0,
            last_text: String::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    fn reset(&mut self) {
        self.index += 1;
        self.samples.clear();
        self.has_speech = false;
        self.silent_frames = 0;
        self.last_text.clear();
    }

    fn should_commit(&self) -> bool {
        self.silent_frames >= SILENT_FRAMES_TO_COMMIT || self.samples.len() >= MAX_UTTERANCE_SAMPLES
    }
}

fn run_live_loop(
    session_id: String,
    live_rx: std_mpsc::Receiver<LiveAudioFrame>,
    whisper_model_path: Option<PathBuf>,
    language_config: SttLanguageConfig,
    app: Option<AppHandle>,
    stop_flag: Arc<AtomicBool>,
) {
    let mut transcriber = match whisper_model_path
        .as_ref()
        .and_then(|p| p.to_str())
        .map(|path| StreamingTranscriber::new(path, &language_config, live_thread_count()))
    {
        Some(Ok(t)) => t,
        Some(Err(e)) => {
            tracing::warn!(
                "LiveSttWorker: live transcription disabled for {} ({}). Durable recording is unaffected.",
                session_id,
                e
            );
            drain_without_transcribing(live_rx);
            return;
        }
        None => {
            tracing::warn!(
                "LiveSttWorker: no Whisper model configured — live transcription disabled for {}. Durable recording is unaffected.",
                session_id
            );
            drain_without_transcribing(live_rx);
            return;
        }
    };

    let mut utterance = Utterance::new(0);
    let mut latest_capture = std::time::Instant::now();

    // Block for the next frame, then take everything else already queued.
    // Catching up in one decode is what keeps latency equal to a single
    // inference instead of growing with the queue depth. The loop ends when
    // capture stops and drops the sender.
    while let Ok(first) = live_rx.recv() {
        let mut frames = vec![first];
        while let Ok(frame) = live_rx.try_recv() {
            frames.push(frame);
        }

        for frame in frames {
            // A pause boundary must not splice two halves of different moments
            // into one utterance.
            if frame.discontinuity && !utterance.is_empty() {
                commit(&app, &session_id, &mut utterance, latest_capture);
            }

            if utterance.is_empty() {
                utterance.start_time_s = frame.start_time_s;
            }
            utterance.end_time_s = frame.end_time_s;
            latest_capture = frame.capture_instant;

            if frame.is_speech {
                utterance.has_speech = true;
                utterance.silent_frames = 0;
            } else {
                utterance.silent_frames += 1;
            }
            utterance.samples.extend_from_slice(&frame.samples);
        }

        if stop_flag.load(Ordering::Relaxed) {
            break;
        }

        // Silence never reaches Whisper: it would burn a full decode to produce
        // nothing, or worse, hallucinate on background noise.
        //
        // The frame flags alone are not enough. They compare each second against
        // a fixed RMS floor, so steady room tone sitting just above it marks
        // every frame as speech and the whole utterance is decoded — the same
        // failure the durable clock had. Measuring against the buffer's own
        // noise floor is what tells a fan from a voice.
        let profile = transcript_health::profile_speech(&utterance.samples, TARGET_SAMPLE_RATE);
        if !utterance.has_speech || !profile.is_worth_decoding() {
            utterance.samples.clear();
            utterance.has_speech = false;
            utterance.silent_frames = 0;
            continue;
        }

        let text = match transcriber.transcribe(&utterance.samples) {
            Ok(text) => text,
            Err(e) => {
                tracing::warn!("LiveSttWorker: live decode failed: {}", e);
                String::new()
            }
        };

        let commit_now = utterance.should_commit();

        // The live stream is what the user watches while talking, so a
        // hallucination here is the most visible failure Relay has. It is also
        // the cheapest to catch: the same assessment the durable clock runs.
        let text = match transcript_health::assess(
            &text,
            DecodeEvidence {
                voiced_seconds: profile.voiced_seconds,
                total_seconds: profile.total_seconds,
                mean_no_speech_prob: 0.0,
            },
        ) {
            Some(reason) => {
                tracing::debug!(
                    "LiveSttWorker: discarded live decode — {}",
                    reason.describe()
                );
                String::new()
            }
            None => text,
        };

        if !text.is_empty() {
            utterance.last_text = text;
            emit_update(&app, &session_id, &utterance, commit_now, latest_capture);
        } else if commit_now && !utterance.last_text.is_empty() {
            // Keep the best text we had rather than dropping the utterance
            // because the final decode came back empty.
            emit_update(&app, &session_id, &utterance, true, latest_capture);
        }

        if commit_now {
            utterance.reset();
        }
    }

    // Flush whatever was in flight so the last thing said still appears.
    if utterance.has_speech && !utterance.is_empty() {
        let profile = transcript_health::profile_speech(&utterance.samples, TARGET_SAMPLE_RATE);
        if profile.is_worth_decoding() {
            if let Ok(text) = transcriber.transcribe(&utterance.samples) {
                let evidence = DecodeEvidence {
                    voiced_seconds: profile.voiced_seconds,
                    total_seconds: profile.total_seconds,
                    mean_no_speech_prob: 0.0,
                };
                if !text.is_empty() && transcript_health::assess(&text, evidence).is_none() {
                    utterance.last_text = text;
                }
            }
        }
        commit(&app, &session_id, &mut utterance, latest_capture);
    }

    tracing::info!("LiveSttWorker: exited cleanly for session {}.", session_id);
}

fn commit(
    app: &Option<AppHandle>,
    session_id: &str,
    utterance: &mut Utterance,
    latest_capture: std::time::Instant,
) {
    if !utterance.last_text.is_empty() {
        emit_update(app, session_id, utterance, true, latest_capture);
    }
    utterance.reset();
}

fn emit_update(
    app: &Option<AppHandle>,
    session_id: &str,
    utterance: &Utterance,
    is_final: bool,
    latest_capture: std::time::Instant,
) {
    let latency_ms = latest_capture.elapsed().as_millis() as u64;
    let update = LiveTranscriptUpdate {
        segment_id: format!("{}_u{}", session_id, utterance.index),
        session_id: session_id.to_string(),
        utterance_index: utterance.index,
        start_time_s: utterance.start_time_s,
        end_time_s: utterance.end_time_s,
        text: utterance.last_text.clone(),
        is_final,
        latency_ms,
    };

    tracing::debug!(
        "[LiveSTT] u{} [{:.1}s - {:.1}s] final={} latency={}ms: \"{}\"",
        update.utterance_index,
        update.start_time_s,
        update.end_time_s,
        is_final,
        latency_ms,
        update.text
    );

    if let Some(a) = app {
        let _ = a.emit("meeting-live-transcript", &update);
    }
}

/// Keeps the capture side unblocked when live transcription is unavailable.
fn drain_without_transcribing(live_rx: std_mpsc::Receiver<LiveAudioFrame>) {
    while live_rx.recv().is_ok() {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(index: usize, is_speech: bool, discontinuity: bool) -> LiveAudioFrame {
        LiveAudioFrame {
            session_id: "meet_test".to_string(),
            frame_index: index,
            start_time_s: index as f64,
            end_time_s: index as f64 + 1.0,
            samples: vec![if is_speech { 0.3 } else { 0.0 }; 16_000],
            capture_instant: std::time::Instant::now(),
            discontinuity,
            is_speech,
        }
    }

    #[test]
    fn an_utterance_commits_after_a_silent_frame() {
        let mut u = Utterance::new(0);
        let speech = frame(0, true, false);
        u.samples.extend_from_slice(&speech.samples);
        u.has_speech = true;
        assert!(!u.should_commit(), "speech alone must not close an utterance");

        u.silent_frames = SILENT_FRAMES_TO_COMMIT;
        assert!(u.should_commit(), "trailing silence closes the utterance");
    }

    #[test]
    fn an_utterance_commits_at_the_window_cap() {
        let mut u = Utterance::new(0);
        u.has_speech = true;
        u.samples = vec![0.2; MAX_UTTERANCE_SAMPLES];
        assert!(u.should_commit(), "an unbroken monologue must still commit");
    }

    #[test]
    fn reset_advances_the_segment_id_so_updates_never_collide() {
        let mut u = Utterance::new(0);
        u.samples = vec![0.1; 10];
        u.last_text = "hello".to_string();
        u.has_speech = true;
        u.reset();

        assert_eq!(u.index, 1);
        assert!(u.is_empty());
        assert!(!u.has_speech);
        assert!(u.last_text.is_empty());
        assert_eq!(u.silent_frames, 0);
    }

    #[test]
    fn live_decoding_leaves_cpu_for_the_durable_clock() {
        let threads = live_thread_count();
        assert!((1..=4).contains(&threads));
    }

    #[test]
    fn frames_are_contiguous_and_carry_discontinuity_across_a_pause() {
        let a = frame(0, true, false);
        let b = frame(1, true, true);
        assert_eq!(a.end_time_s, b.start_time_s);
        assert!(b.discontinuity);
    }
}
