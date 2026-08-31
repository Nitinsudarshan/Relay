//! The speech pipeline: phrases in, ordered audio out, while the model is
//! still writing.
//!
//! This is the file that makes the architecture in
//! `docs/talkback/ARCHITECTURE.md` §8 true rather than aspirational.
//! Collecting phrases during the stream and synthesizing them afterwards
//! — which is what Talkback did before this module existed — is not
//! streaming TTS. It only *looks* like it, because the LLM API streams.
//! Time-to-first-audio was still the whole generation plus the first
//! synthesis.
//!
//! ```text
//!  LLM stream ─┬─ "sentence one." ──┐
//!              │                    │   bounded queue
//!              ├─ "sentence two." ──┤   (backpressure, never unbounded)
//!              │                    ▼
//!              │            synthesis worker ── one at a time, in order
//!              │                    │
//!              └─ still generating  └──→ audio event ──→ WebView playback
//! ```
//!
//! Three properties this has to hold, and the mechanism for each:
//!
//! * **Ordered.** A single worker thread consumes the queue, so phrase
//!   *n* is always emitted before *n+1*. No sorting, no sequence gaps.
//! * **Exactly once.** Phrases are moved through a channel; nothing can
//!   read one twice, and nothing drops one except cancellation.
//! * **Cancellable.** Every stage checks the turn's generation — before
//!   dequeuing, inside the synthesis itself (the Piper child is killed),
//!   and before emitting. A superseded turn produces no further audio.

use crate::tts::{TtsError, TtsProvider};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc as std_mpsc;
use std::sync::Arc;

/// Queue depth between the LLM stream and synthesis.
///
/// Talkback caps generation at 400 output tokens, which is at most ~15
/// sentences, so this is never reached in practice — it exists so a
/// runaway model cannot grow an unbounded backlog of audio nobody will
/// hear. When it *is* reached the producer blocks, which is correct
/// backpressure: there is no point generating speech faster than it can
/// be spoken.
pub const QUEUE_DEPTH: usize = 24;

/// One synthesized phrase, ready for the frontend's playback queue.
#[derive(Debug, Clone, PartialEq)]
pub struct SpeechChunk {
    pub turn_id: String,
    /// Position within the turn. The frontend plays strictly in this order.
    pub seq: usize,
    /// The engine's cancellation counter at synthesis time.
    pub generation: u64,
    pub wav_base64: String,
    /// The text of the phrase that was synthesized.
    pub text: String,
}

/// Where synthesized audio goes.
///
/// A trait so the pipeline's ordering and cancellation behaviour can be
/// tested against a recording sink, with no Tauri app handle and no
/// WebView in sight.
pub trait SpeechSink: Send + Sync {
    fn emit_audio(&self, chunk: SpeechChunk);
    /// A synthesis failure the user should know about. Cancellation never
    /// reaches here — talking over the agent is not an error.
    fn emit_error(&self, code: &str, message: &str);
    /// Called once, when the first phrase of a turn is emitted, so the
    /// engine can move to SPEAKING at the moment audio actually exists.
    fn on_first_audio(&self) {}
}

/// What a turn's speech cost, gathered by the worker and read once it
/// finishes.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SpeechOutcome {
    /// Phrases actually emitted as audio.
    pub spoken: usize,
    /// Milliseconds spent inside the *first* synthesis call. Distinct
    /// from the engine's `tts_first_audio_ms`, which is measured from the
    /// start of the turn.
    pub first_synthesis_ms: Option<u128>,
    /// Total time spent synthesizing, for throughput accounting.
    pub total_synthesis_ms: u128,
    /// True when a failure was permanent enough to stop trying.
    pub disabled: bool,
}

/// Shared counters the worker writes and the owner reads.
#[derive(Default)]
struct SharedOutcome {
    spoken: AtomicUsize,
    first_synthesis_ms: AtomicU64,
    has_first: AtomicBool,
    total_synthesis_ms: AtomicU64,
    disabled: AtomicBool,
}

/// Feeds phrases to a background synthesis worker.
///
/// Created when a turn starts speaking, dropped when it ends. Dropping
/// closes the queue, which is what tells the worker to finish and exit —
/// there is no separate shutdown flag to get wrong.
pub struct SpeechPipeline {
    tx: Option<std_mpsc::SyncSender<(usize, String)>>,
    worker: Option<std::thread::JoinHandle<()>>,
    outcome: Arc<SharedOutcome>,
    next_seq: usize,
}

impl SpeechPipeline {
    /// Starts a synthesis worker for one turn.
    ///
    /// `is_stale` is the cancellation predicate — the engine's generation
    /// check. It is consulted before dequeuing, during synthesis, and
    /// before emitting, so a barge-in costs at most the tail of one
    /// in-flight phrase.
    pub fn start(
        turn_id: String,
        generation: u64,
        tts: Arc<dyn TtsProvider>,
        sink: Arc<dyn SpeechSink>,
        is_stale: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> Self {
        let (tx, rx) = std_mpsc::sync_channel::<(usize, String)>(QUEUE_DEPTH);
        let outcome = Arc::new(SharedOutcome::default());
        let worker_outcome = outcome.clone();

        // A thread, not a tokio task: Piper synthesis is a blocking child
        // process, and parking a runtime worker on it would stall
        // unrelated async work — including the LLM stream feeding this
        // very queue.
        let worker = std::thread::spawn(move || {
            // Latched on a permanent failure (missing binary, unloadable
            // model) so Relay does not spawn a process per sentence to
            // produce the same error each time.
            let mut disabled = false;

            for (seq, phrase) in rx {
                if disabled || is_stale() {
                    continue;
                }

                let started = std::time::Instant::now();
                let result = tts.synthesize_cancellable(&phrase, &|| is_stale());
                let elapsed = started.elapsed().as_millis();
                worker_outcome
                    .total_synthesis_ms
                    .fetch_add(elapsed as u64, Ordering::Relaxed);

                match result {
                    Ok(Some(audio)) => {
                        // Re-checked after synthesis: the user may have
                        // started talking while this phrase was being
                        // produced, and stale audio must never play.
                        if is_stale() {
                            continue;
                        }
                        if !worker_outcome.has_first.swap(true, Ordering::SeqCst) {
                            worker_outcome
                                .first_synthesis_ms
                                .store(elapsed as u64, Ordering::SeqCst);
                            sink.on_first_audio();
                        }
                        worker_outcome.spoken.fetch_add(1, Ordering::Relaxed);
                        sink.emit_audio(SpeechChunk {
                            turn_id: turn_id.clone(),
                            seq,
                            generation,
                            wav_base64: audio.wav_base64,
                            text: phrase,
                        });
                    }
                    // Not configured. Nothing to say, nothing to report.
                    Ok(None) => {}
                    // A normal conversational event, not a failure.
                    Err(TtsError::Cancelled) => continue,
                    Err(error) => {
                        tracing::warn!("talkback: TTS failed, continuing text-only: {}", error);
                        sink.emit_error("TTS_FAILED", &error.to_string());
                        if error.is_permanent() {
                            disabled = true;
                            worker_outcome.disabled.store(true, Ordering::SeqCst);
                            tracing::warn!(
                                "talkback: voice disabled for this turn — the configuration \
                                 needs fixing, not retrying"
                            );
                        }
                    }
                }
            }
        });

        Self {
            tx: Some(tx),
            worker: Some(worker),
            outcome,
            next_seq: 0,
        }
    }

    /// Queues a phrase for synthesis and returns immediately.
    ///
    /// Called from inside the LLM stream callback, which is why it must
    /// not do any work itself: the point of this module is that
    /// generation continues while synthesis happens.
    ///
    /// Returns `false` once the pipeline has been closed or the worker has
    /// gone, so the caller can stop feeding it.
    pub fn push(&mut self, phrase: &str) -> bool {
        let phrase = phrase.trim();
        if phrase.is_empty() {
            return true;
        }
        let Some(tx) = self.tx.as_ref() else {
            return false;
        };
        let seq = self.next_seq;
        // A full queue blocks, which is deliberate backpressure. With a
        // 400-token generation cap the depth is never reached, so this is
        // a guard rather than a hot path.
        match tx.send((seq, phrase.to_string())) {
            Ok(()) => {
                self.next_seq += 1;
                true
            }
            Err(_) => false,
        }
    }

    /// How many phrases have been queued so far.
    pub fn queued(&self) -> usize {
        self.next_seq
    }

    /// Closes the queue and waits for synthesis to drain.
    ///
    /// "Drain" respects cancellation: a stale turn's worker skips its
    /// remaining phrases rather than synthesizing them to throw away, so
    /// this returns promptly after a barge-in.
    pub fn finish(mut self) -> SpeechOutcome {
        self.close_and_join();
        SpeechOutcome {
            spoken: self.outcome.spoken.load(Ordering::SeqCst),
            first_synthesis_ms: self
                .outcome
                .has_first
                .load(Ordering::SeqCst)
                .then(|| self.outcome.first_synthesis_ms.load(Ordering::SeqCst) as u128),
            total_synthesis_ms: self.outcome.total_synthesis_ms.load(Ordering::SeqCst) as u128,
            disabled: self.outcome.disabled.load(Ordering::SeqCst),
        }
    }

    fn close_and_join(&mut self) {
        // Dropping the sender is the shutdown signal: the worker's `for`
        // loop ends when the channel closes.
        self.tx.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for SpeechPipeline {
    /// Guarantees the worker is joined even if the turn returns early —
    /// an error path, an early cancellation, or a panic. Without this a
    /// detached worker could emit audio for a turn that no longer exists.
    fn drop(&mut self) {
        self.close_and_join();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tts::{TtsAudio, TtsCapabilities};
    use std::sync::Mutex;

    /// A provider that records what it was asked to say and can be made
    /// slow, cancellable, or broken.
    struct FakeTts {
        calls: Mutex<Vec<String>>,
        delay_ms: u64,
        failure: Option<fn() -> TtsError>,
        configured: bool,
    }

    impl FakeTts {
        fn ok() -> Arc<Self> {
            Arc::new(Self {
                calls: Mutex::new(Vec::new()),
                delay_ms: 0,
                failure: None,
                configured: true,
            })
        }
        fn slow(delay_ms: u64) -> Arc<Self> {
            Arc::new(Self {
                calls: Mutex::new(Vec::new()),
                delay_ms,
                failure: None,
                configured: true,
            })
        }
        fn failing(failure: fn() -> TtsError) -> Arc<Self> {
            Arc::new(Self {
                calls: Mutex::new(Vec::new()),
                delay_ms: 0,
                failure: Some(failure),
                configured: true,
            })
        }
        fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl TtsProvider for FakeTts {
        fn name(&self) -> &'static str {
            "fake"
        }
        fn capabilities(&self) -> TtsCapabilities {
            TtsCapabilities {
                intra_utterance_streaming: false,
                cancellable: true,
                voices: false,
            }
        }
        fn is_configured(&self) -> bool {
            self.configured
        }
        fn synthesize_cancellable(
            &self,
            text: &str,
            is_cancelled: &dyn Fn() -> bool,
        ) -> Result<Option<TtsAudio>, TtsError> {
            self.calls.lock().unwrap().push(text.to_string());
            // Poll like the real provider does, so cancellation mid-call
            // is exercised rather than assumed.
            let deadline =
                std::time::Instant::now() + std::time::Duration::from_millis(self.delay_ms);
            while std::time::Instant::now() < deadline {
                if is_cancelled() {
                    return Err(TtsError::Cancelled);
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            if let Some(make_error) = self.failure {
                return Err(make_error());
            }
            Ok(Some(TtsAudio {
                wav_base64: format!("wav:{text}"),
                char_count: text.chars().count(),
            }))
        }
    }

    #[derive(Default)]
    struct RecordingSink {
        chunks: Mutex<Vec<SpeechChunk>>,
        errors: Mutex<Vec<(String, String)>>,
        first_audio_calls: AtomicUsize,
    }

    impl RecordingSink {
        fn chunks(&self) -> Vec<SpeechChunk> {
            self.chunks.lock().unwrap().clone()
        }
        fn spoken_text(&self) -> Vec<String> {
            self.chunks()
                .into_iter()
                .map(|c| c.wav_base64.replace("wav:", ""))
                .collect()
        }
        fn errors(&self) -> Vec<(String, String)> {
            self.errors.lock().unwrap().clone()
        }
    }

    impl SpeechSink for RecordingSink {
        fn emit_audio(&self, chunk: SpeechChunk) {
            self.chunks.lock().unwrap().push(chunk);
        }
        fn emit_error(&self, code: &str, message: &str) {
            self.errors
                .lock()
                .unwrap()
                .push((code.to_string(), message.to_string()));
        }
        fn on_first_audio(&self) {
            self.first_audio_calls.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn never_stale() -> Arc<dyn Fn() -> bool + Send + Sync> {
        Arc::new(|| false)
    }

    /// Quantifies the overlap this module exists to create.
    ///
    /// A *simulation*, not a Piper measurement: it models a model that
    /// emits a sentence every 300 ms and a synthesizer that takes 250 ms,
    /// and compares time-to-first-audio through the pipeline against the
    /// batch approach it replaced (collect every phrase, then synthesize).
    /// The scheduling being measured is real; the component timings are
    /// stand-ins, and `docs/talkback/BENCHMARKS.md` says so.
    ///
    /// ```text
    /// cargo test --release overlap_saving -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "benchmark, not a correctness test"]
    fn overlap_saving() {
        const SENTENCES: usize = 5;
        const GENERATION_MS: u64 = 300;
        const SYNTHESIS_MS: u64 = 250;

        let tts = FakeTts::slow(SYNTHESIS_MS);
        let sink = Arc::new(RecordingSink::default());
        let started = std::time::Instant::now();
        let mut pipeline =
            SpeechPipeline::start("t".into(), 1, tts.clone(), sink.clone(), never_stale());

        let mut first_audio_at = None;
        for i in 0..SENTENCES {
            std::thread::sleep(std::time::Duration::from_millis(GENERATION_MS));
            pipeline.push(&format!("Sentence {i}."));
            if first_audio_at.is_none() && !sink.chunks().is_empty() {
                first_audio_at = Some(started.elapsed());
            }
        }
        // Catch the first chunk if it landed during the last generation gap.
        while first_audio_at.is_none() && started.elapsed().as_millis() < 5_000 {
            if !sink.chunks().is_empty() {
                first_audio_at = Some(started.elapsed());
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        pipeline.finish();

        let overlapped = first_audio_at.expect("audio").as_millis();
        // What the batch path cost: the whole generation, then one synthesis.
        let batched = (SENTENCES as u64 * GENERATION_MS + SYNTHESIS_MS) as u128;

        println!(
            "first audio: overlapped {overlapped} ms vs batched {batched} ms \
             ({} sentences, {GENERATION_MS} ms/sentence, {SYNTHESIS_MS} ms synthesis)",
            SENTENCES
        );
        assert!(
            overlapped < batched,
            "the pipeline gained nothing over batching"
        );
    }

    #[test]
    fn phrases_are_spoken_in_order() {
        let tts = FakeTts::ok();
        let sink = Arc::new(RecordingSink::default());
        let mut pipeline = SpeechPipeline::start(
            "turn_1".into(),
            1,
            tts.clone(),
            sink.clone(),
            never_stale(),
        );

        for phrase in ["One.", "Two.", "Three."] {
            assert!(pipeline.push(phrase));
        }
        let outcome = pipeline.finish();

        assert_eq!(outcome.spoken, 3);
        assert_eq!(sink.spoken_text(), vec!["One.", "Two.", "Three."]);
        assert_eq!(
            sink.chunks().iter().map(|c| c.seq).collect::<Vec<_>>(),
            vec![0, 1, 2],
            "sequence numbers must be dense and ascending"
        );
    }

    #[test]
    fn synthesis_overlaps_generation_rather_than_following_it() {
        // The whole point of this module. Each synthesis takes 60ms; if
        // the pipeline waited for all pushes before starting, the first
        // audio could not exist until after the last push.
        let tts = FakeTts::slow(60);
        let sink = Arc::new(RecordingSink::default());
        let mut pipeline = SpeechPipeline::start(
            "turn_1".into(),
            1,
            tts.clone(),
            sink.clone(),
            never_stale(),
        );

        pipeline.push("First sentence.");

        // Stand in for the model still generating.
        let mut first_audio_seen_during_generation = false;
        for _ in 0..40 {
            std::thread::sleep(std::time::Duration::from_millis(5));
            if !sink.chunks().is_empty() {
                first_audio_seen_during_generation = true;
                break;
            }
        }
        assert!(
            first_audio_seen_during_generation,
            "the first sentence must be audible before the answer is finished"
        );

        pipeline.push("Second sentence.");
        let outcome = pipeline.finish();
        assert_eq!(outcome.spoken, 2);
    }

    #[test]
    fn no_phrase_is_synthesized_twice() {
        let tts = FakeTts::ok();
        let sink = Arc::new(RecordingSink::default());
        let mut pipeline =
            SpeechPipeline::start("t".into(), 1, tts.clone(), sink.clone(), never_stale());

        for phrase in ["A sentence.", "A sentence.", "Another one."] {
            pipeline.push(phrase);
        }
        pipeline.finish();

        // Identical text twice is legitimate; three calls for three
        // pushes is the invariant.
        assert_eq!(tts.calls().len(), 3);
        assert_eq!(sink.chunks().len(), 3);
    }

    #[test]
    fn no_phrase_is_dropped_during_normal_operation() {
        let tts = FakeTts::ok();
        let sink = Arc::new(RecordingSink::default());
        let mut pipeline =
            SpeechPipeline::start("t".into(), 1, tts.clone(), sink.clone(), never_stale());

        let phrases: Vec<String> = (0..20).map(|i| format!("Sentence {i}.")).collect();
        for phrase in &phrases {
            pipeline.push(phrase);
        }
        let outcome = pipeline.finish();

        assert_eq!(outcome.spoken, phrases.len());
        assert_eq!(sink.spoken_text(), phrases);
    }

    #[test]
    fn empty_and_whitespace_phrases_are_not_queued() {
        let tts = FakeTts::ok();
        let sink = Arc::new(RecordingSink::default());
        let mut pipeline =
            SpeechPipeline::start("t".into(), 1, tts.clone(), sink.clone(), never_stale());

        pipeline.push("");
        pipeline.push("   \n ");
        pipeline.push("Real.");
        pipeline.finish();

        assert_eq!(tts.calls(), vec!["Real."]);
        assert_eq!(
            sink.chunks()[0].seq,
            0,
            "skipped phrases must not leave a gap the player waits on"
        );
    }

    #[test]
    fn cancellation_stops_queued_phrases_from_being_synthesized() {
        let stale = Arc::new(AtomicBool::new(false));
        let flag = stale.clone();
        let tts = FakeTts::slow(40);
        let sink = Arc::new(RecordingSink::default());
        let mut pipeline = SpeechPipeline::start(
            "t".into(),
            1,
            tts.clone(),
            sink.clone(),
            Arc::new(move || flag.load(Ordering::SeqCst)),
        );

        for i in 0..10 {
            pipeline.push(&format!("Sentence {i}."));
        }
        // Barge in almost immediately.
        std::thread::sleep(std::time::Duration::from_millis(20));
        stale.store(true, Ordering::SeqCst);

        let started = std::time::Instant::now();
        let outcome = pipeline.finish();

        assert!(
            outcome.spoken < 10,
            "everything was spoken despite the interruption"
        );
        assert!(
            started.elapsed() < std::time::Duration::from_millis(500),
            "cancellation waited for the whole queue: {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn audio_synthesized_before_a_barge_in_is_not_emitted_after_it() {
        let stale = Arc::new(AtomicBool::new(false));
        let flag = stale.clone();
        let tts = FakeTts::slow(80);
        let sink = Arc::new(RecordingSink::default());
        let mut pipeline = SpeechPipeline::start(
            "t".into(),
            1,
            tts.clone(),
            sink.clone(),
            Arc::new(move || flag.load(Ordering::SeqCst)),
        );

        pipeline.push("A sentence being synthesized.");
        std::thread::sleep(std::time::Duration::from_millis(10));
        stale.store(true, Ordering::SeqCst);
        let outcome = pipeline.finish();

        assert_eq!(outcome.spoken, 0);
        assert!(
            sink.chunks().is_empty(),
            "stale audio reached playback after the user interrupted"
        );
        assert!(
            sink.errors().is_empty(),
            "cancellation is a conversational event, not an error to show"
        );
    }

    #[test]
    fn a_turn_cancelled_before_it_starts_speaks_nothing() {
        let tts = FakeTts::ok();
        let sink = Arc::new(RecordingSink::default());
        let mut pipeline = SpeechPipeline::start(
            "t".into(),
            1,
            tts.clone(),
            sink.clone(),
            Arc::new(|| true),
        );

        pipeline.push("Never spoken.");
        let outcome = pipeline.finish();

        assert_eq!(outcome.spoken, 0);
        assert!(tts.calls().is_empty(), "a stale turn must not spawn Piper");
    }

    #[test]
    fn a_permanent_failure_stops_further_synthesis_attempts() {
        // An unloadable model would otherwise produce one failed process
        // spawn per sentence, for every sentence, forever.
        let tts = FakeTts::failing(|| TtsError::SynthesisFailed("model config not found".into()));
        let sink = Arc::new(RecordingSink::default());
        let mut pipeline =
            SpeechPipeline::start("t".into(), 1, tts.clone(), sink.clone(), never_stale());

        for i in 0..6 {
            pipeline.push(&format!("Sentence {i}."));
        }
        let outcome = pipeline.finish();

        assert!(outcome.disabled);
        assert_eq!(tts.calls().len(), 1, "it kept retrying a broken configuration");
        assert_eq!(sink.errors().len(), 1, "and kept telling the user about it");
    }

    #[test]
    fn a_transient_failure_does_not_disable_the_voice() {
        let tts = FakeTts::failing(|| TtsError::SynthesisFailed("terminated by signal".into()));
        let sink = Arc::new(RecordingSink::default());
        let mut pipeline =
            SpeechPipeline::start("t".into(), 1, tts.clone(), sink.clone(), never_stale());

        for i in 0..3 {
            pipeline.push(&format!("Sentence {i}."));
        }
        let outcome = pipeline.finish();

        assert!(!outcome.disabled);
        assert_eq!(tts.calls().len(), 3, "a one-off failure must not latch off");
    }

    #[test]
    fn an_unconfigured_provider_produces_silence_not_errors() {
        struct Silent;
        impl TtsProvider for Silent {
            fn name(&self) -> &'static str {
                "none"
            }
            fn capabilities(&self) -> TtsCapabilities {
                TtsCapabilities {
                    intra_utterance_streaming: false,
                    cancellable: true,
                    voices: false,
                }
            }
            fn is_configured(&self) -> bool {
                false
            }
            fn synthesize_cancellable(
                &self,
                _: &str,
                _: &dyn Fn() -> bool,
            ) -> Result<Option<TtsAudio>, TtsError> {
                Ok(None)
            }
        }

        let sink = Arc::new(RecordingSink::default());
        let mut pipeline =
            SpeechPipeline::start("t".into(), 1, Arc::new(Silent), sink.clone(), never_stale());
        pipeline.push("Anything.");
        let outcome = pipeline.finish();

        assert_eq!(outcome.spoken, 0);
        assert!(sink.errors().is_empty());
    }

    #[test]
    fn first_audio_is_signalled_exactly_once_per_turn() {
        let tts = FakeTts::ok();
        let sink = Arc::new(RecordingSink::default());
        let mut pipeline =
            SpeechPipeline::start("t".into(), 1, tts.clone(), sink.clone(), never_stale());

        for phrase in ["One.", "Two.", "Three."] {
            pipeline.push(phrase);
        }
        pipeline.finish();

        assert_eq!(
            sink.first_audio_calls.load(Ordering::SeqCst),
            1,
            "SPEAKING must be entered once, not per phrase"
        );
    }

    #[test]
    fn first_audio_is_never_signalled_when_nothing_is_spoken() {
        let tts = FakeTts::failing(|| TtsError::SynthesisFailed("model config not found".into()));
        let sink = Arc::new(RecordingSink::default());
        let mut pipeline =
            SpeechPipeline::start("t".into(), 1, tts.clone(), sink.clone(), never_stale());
        pipeline.push("One.");
        pipeline.finish();

        assert_eq!(sink.first_audio_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn synthesis_timing_is_recorded() {
        let tts = FakeTts::slow(30);
        let sink = Arc::new(RecordingSink::default());
        let mut pipeline =
            SpeechPipeline::start("t".into(), 1, tts.clone(), sink.clone(), never_stale());
        pipeline.push("One.");
        pipeline.push("Two.");
        let outcome = pipeline.finish();

        let first = outcome.first_synthesis_ms.expect("a first synthesis");
        assert!(first >= 25, "implausibly fast: {first}ms");
        assert!(
            outcome.total_synthesis_ms >= first,
            "the total must include the first"
        );
    }

    #[test]
    fn the_chunk_carries_the_turn_and_generation_it_belongs_to() {
        let tts = FakeTts::ok();
        let sink = Arc::new(RecordingSink::default());
        let mut pipeline = SpeechPipeline::start(
            "turn_42".into(),
            7,
            tts.clone(),
            sink.clone(),
            never_stale(),
        );
        pipeline.push("One.");
        pipeline.finish();

        let chunk = &sink.chunks()[0];
        assert_eq!(chunk.turn_id, "turn_42");
        assert_eq!(chunk.generation, 7, "the player gates stale audio on this");
    }

    #[test]
    fn dropping_the_pipeline_joins_its_worker() {
        // An early return must not leave a detached worker emitting audio
        // into a turn that has already ended.
        let tts = FakeTts::slow(20);
        let sink = Arc::new(RecordingSink::default());
        {
            let mut pipeline =
                SpeechPipeline::start("t".into(), 1, tts.clone(), sink.clone(), never_stale());
            pipeline.push("One.");
            pipeline.push("Two.");
        }
        // If Drop did not join, the count could still be rising here.
        let settled = sink.chunks().len();
        std::thread::sleep(std::time::Duration::from_millis(80));
        assert_eq!(sink.chunks().len(), settled);
        assert_eq!(settled, 2);
    }

    #[test]
    fn pushing_after_finishing_is_refused_rather_than_panicking() {
        let tts = FakeTts::ok();
        let sink = Arc::new(RecordingSink::default());
        let mut pipeline =
            SpeechPipeline::start("t".into(), 1, tts.clone(), sink.clone(), never_stale());
        pipeline.push("One.");
        pipeline.close_and_join();
        assert!(!pipeline.push("Two."));
    }

    #[test]
    fn queued_counts_only_real_phrases() {
        let tts = FakeTts::ok();
        let sink = Arc::new(RecordingSink::default());
        let mut pipeline =
            SpeechPipeline::start("t".into(), 1, tts.clone(), sink.clone(), never_stale());
        pipeline.push("One.");
        pipeline.push("  ");
        pipeline.push("Two.");
        assert_eq!(pipeline.queued(), 2);
        pipeline.finish();
    }

    #[test]
    fn the_queue_is_bounded_and_applies_backpressure() {
        // A slow synthesis with more phrases than the queue holds must
        // block the producer rather than growing without limit.
        let tts = FakeTts::slow(15);
        let sink = Arc::new(RecordingSink::default());
        let mut pipeline =
            SpeechPipeline::start("t".into(), 1, tts.clone(), sink.clone(), never_stale());

        let total = QUEUE_DEPTH * 2;
        for i in 0..total {
            assert!(pipeline.push(&format!("Sentence {i}.")));
        }
        let outcome = pipeline.finish();
        assert_eq!(
            outcome.spoken, total,
            "backpressure must slow the producer, never drop a phrase"
        );
    }
}
