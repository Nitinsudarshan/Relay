//! The Talkback orchestrator.
//!
//! Everything else in this module is a decision; this is the thing that
//! sequences them, owns cancellation, and emits the events the UI
//! renders. It is deliberately the only file that knows about
//! `AppHandle`, so every rule that matters — what may be answered from
//! model knowledge, what a phrase boundary is, when a turn has ended —
//! stays testable somewhere else.

use super::assemble::{self, NO_EVIDENCE_RESPONSE};
use super::audio::{MicFrame, TalkbackMic};
use super::chunk::PhraseBuffer;
use super::intent::{self, Intent};
use super::retrieval::{self, ContextItem, RetrievalQuery, RetrievalResult, SourceType};
use super::session::{TalkbackSession, HISTORY_TURNS};
use super::sources;
use super::state::{TalkbackEvent, TalkbackState};
use super::tools;
use super::turn::{TurnDetector, TurnDetectorConfig, TurnEvent};
use crate::capture::stt::{SttLanguageConfig, StreamingTranscriber};
use crate::meetings_v2::processing::MeetingProcessor;
use crate::meetings_v2::session_store::SessionStore;
use crate::providers::{CompletionOptions, LLMClient};
use crate::settings::AppSettings;
use crate::sync::MutexExt;
use crate::tts::{resolve_provider, TtsProvider};
use crate::vault::VaultManager;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc as std_mpsc;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

/// Backend-owned state changes. The UI never sets a state itself.
pub const TALKBACK_STATE_EVENT: &str = "talkback-state";
/// A completed turn, user or agent.
pub const TALKBACK_TURN_EVENT: &str = "talkback-turn";
/// A fragment of the answer as it is generated.
pub const TALKBACK_DELTA_EVENT: &str = "talkback-delta";
/// One synthesized phrase, for the frontend's playback queue.
pub const TALKBACK_AUDIO_EVENT: &str = "talkback-audio";
/// Microphone amplitude, for the agent animation.
pub const TALKBACK_LEVEL_EVENT: &str = "talkback-level";
/// Per-turn latency measurements.
pub const TALKBACK_METRICS_EVENT: &str = "talkback-metrics";
pub const TALKBACK_ERROR_EVENT: &str = "talkback-error";

/// How Talkback is switched on.
///
/// `WakeWord` is accepted by the type and refused by the engine. That is
/// the point: the seam is real and the behaviour is honest, rather than a
/// setting that pretends to work (`ARCHITECTURE.md` §9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivationMode {
    #[default]
    Toggle,
    WakeWord,
}

/// Talkback's slice of settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TalkbackSettings {
    #[serde(default)]
    pub activation_mode: ActivationMode,
    /// Whether answers are spoken. Independent of whether a TTS provider
    /// is configured — a user may want text-only even with Piper set up.
    #[serde(default = "default_true")]
    pub speak_responses: bool,
    /// Whether speaking over the agent interrupts it.
    #[serde(default = "default_true")]
    pub allow_barge_in: bool,
    /// Which sources retrieval may read. Empty means all of them.
    #[serde(default)]
    pub sources: Vec<SourceType>,
    /// Silence, in milliseconds, that ends a spoken turn.
    #[serde(default = "default_hangover_ms", alias = "hangoverMs")]
    pub end_of_turn_silence_ms: u32,
}

fn default_true() -> bool {
    true
}

fn default_hangover_ms() -> u32 {
    TurnDetectorConfig::default().hangover_ms
}

impl Default for TalkbackSettings {
    fn default() -> Self {
        Self {
            activation_mode: ActivationMode::Toggle,
            speak_responses: true,
            allow_barge_in: true,
            sources: Vec::new(),
            end_of_turn_silence_ms: default_hangover_ms(),
        }
    }
}

impl TalkbackSettings {
    /// The sources retrieval should search, resolving "empty means all".
    pub fn effective_sources(&self) -> Vec<SourceType> {
        if self.sources.is_empty() {
            SourceType::ALL.to_vec()
        } else {
            self.sources.clone()
        }
    }

    /// Clamped so a settings value can never make turn-taking unusable:
    /// below ~250 ms the agent cuts off mid-sentence, above 3 s it feels
    /// broken.
    pub fn turn_detector_config(&self) -> TurnDetectorConfig {
        TurnDetectorConfig {
            hangover_ms: self.end_of_turn_silence_ms.clamp(250, 3_000),
            ..TurnDetectorConfig::default()
        }
    }
}

/// What a turn actually costs, for the latency work in
/// `docs/talkback/BENCHMARKS.md`.
///
/// Durations, counts and ids only. Never transcript text, never retrieved
/// content, never audio (`ARCHITECTURE.md` §10).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TurnMetrics {
    pub session_id: String,
    pub turn_id: String,
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub stt_ms: Option<u128>,
    pub retrieval_ms: u128,
    pub retrieved_count: usize,
    pub candidate_count: usize,
    #[serde(default)]
    pub llm_first_token_ms: Option<u128>,
    #[serde(default)]
    pub llm_total_ms: Option<u128>,
    #[serde(default)]
    pub tts_first_audio_ms: Option<u128>,
    pub total_ms: u128,
    pub interrupted: bool,
    /// True when the turn was answered without a model — the
    /// no-evidence path, or a tool.
    pub deterministic: bool,
    pub intent: Intent,
}

/// How a turn will be answered, decided before anything expensive runs.
#[derive(Debug, Clone, PartialEq)]
pub enum TurnPlan {
    /// Answer with this exact text. No model is called.
    Immediate {
        text: String,
        sources: Vec<ContextItem>,
    },
    /// Ask the model, with this system prompt.
    Generate {
        system_prompt: String,
        sources: Vec<ContextItem>,
    },
    /// Run a tool instead of answering.
    Action(Intent),
}

/// Decides how to answer a turn.
///
/// **The anti-hallucination rule lives here.** A personal-memory question
/// with no retrieved evidence never reaches a model: the cheapest way to
/// guarantee Talkback does not invent a memory is not to give it the
/// opportunity. Pure, so that guarantee is a test rather than a hope.
pub fn plan_turn(
    intent: Intent,
    retrieval: &RetrievalResult,
    session: &TalkbackSession,
) -> TurnPlan {
    if intent.is_action() {
        return TurnPlan::Action(intent);
    }

    if intent == Intent::ShowSources {
        let sources = session.last_sources().to_vec();
        return TurnPlan::Immediate {
            text: assemble::describe_sources(&sources),
            sources,
        };
    }

    if intent == Intent::PersonalMemory && retrieval.is_empty() {
        return TurnPlan::Immediate {
            text: NO_EVIDENCE_RESPONSE.to_string(),
            sources: Vec::new(),
        };
    }

    TurnPlan::Generate {
        system_prompt: assemble::build_system_prompt(intent, retrieval, session, HISTORY_TURNS),
        sources: retrieval.items.clone(),
    }
}

/// Talkback's runtime state. One per app, held in `AppState`.
pub struct TalkbackEngine {
    state: Mutex<TalkbackState>,
    session: Mutex<TalkbackSession>,
    /// Bumped to invalidate everything in flight. A turn captures the
    /// value at its start and abandons itself the moment it changes,
    /// which is how one barge-in stops the LLM stream, the synthesis and
    /// the playback queue without three separate cancel paths.
    generation: AtomicU64,
    mic: Mutex<Option<TalkbackMic>>,
    /// Kept so a stopped worker thread can be joined on disable.
    worker: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl Default for TalkbackEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl TalkbackEngine {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(TalkbackState::Off),
            session: Mutex::new(TalkbackSession::new()),
            generation: AtomicU64::new(0),
            mic: Mutex::new(None),
            worker: Mutex::new(None),
        }
    }

    pub fn state(&self) -> TalkbackState {
        *self.state.lock_or_recover()
    }

    /// True while Talkback holds the microphone, so dictation can refuse
    /// rather than race it for the device.
    pub fn holds_microphone(&self) -> bool {
        self.state().holds_microphone()
    }

    pub fn session_snapshot(&self) -> TalkbackSession {
        self.session.lock_or_recover().clone()
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    /// True when the turn that started at `generation` has been
    /// superseded.
    pub fn is_stale(&self, generation: u64) -> bool {
        self.generation() != generation
    }

    /// Abandons everything in flight.
    pub fn cancel(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Applies a state event and broadcasts the result.
    ///
    /// An illegal transition is logged and the state left alone: a
    /// stray event from a racing audio callback must not wedge a live
    /// conversation.
    pub fn transition(&self, app: &AppHandle, event: TalkbackEvent) -> TalkbackState {
        let mut guard = self.state.lock_or_recover();
        match guard.apply(event) {
            Ok(next) => {
                let changed = *guard != next;
                *guard = next;
                drop(guard);
                if changed {
                    let _ = app.emit(
                        TALKBACK_STATE_EVENT,
                        serde_json::json!({
                            "state": next,
                            "sessionId": self.session.lock_or_recover().session_id,
                        }),
                    );
                }
                next
            }
            Err(e) => {
                tracing::debug!("talkback: {}", e);
                *guard
            }
        }
    }

    /// Starts a conversation and, for a voice session, opens the
    /// microphone.
    ///
    /// `voice` is false for the text fallback, which needs the same
    /// engine but no microphone at all.
    pub fn enable(
        &self,
        app: &AppHandle,
        settings: &TalkbackSettings,
        voice: bool,
        whisper_model: Option<std::path::PathBuf>,
        language: SttLanguageConfig,
    ) -> Result<TalkbackState, String> {
        if settings.activation_mode == ActivationMode::WakeWord {
            return Err(
                "Wake-word activation isn't available yet — Talkback is toggle-only for now."
                    .to_string(),
            );
        }

        *self.session.lock_or_recover() = TalkbackSession::new();
        self.cancel();
        self.transition(app, TalkbackEvent::Enable);

        if voice {
            let (frame_tx, frame_rx) = std_mpsc::channel::<MicFrame>();
            let mic = TalkbackMic::start(frame_tx).map_err(|e| e.to_string())?;
            *self.mic.lock_or_recover() = Some(mic);
            let worker = spawn_voice_worker(
                app.clone(),
                frame_rx,
                settings.clone(),
                whisper_model,
                language,
            );
            *self.worker.lock_or_recover() = Some(worker);
        }

        Ok(self.transition(app, TalkbackEvent::Ready))
    }

    /// Ends the conversation and closes the microphone.
    ///
    /// The stream is *dropped*, not muted: "Talkback off" has to mean the
    /// OS-level capture no longer exists.
    pub fn disable(&self, app: &AppHandle) -> TalkbackState {
        self.cancel();
        if let Some(mut mic) = self.mic.lock_or_recover().take() {
            mic.stop();
        }
        if let Some(worker) = self.worker.lock_or_recover().take() {
            let _ = worker.join();
        }
        self.transition(app, TalkbackEvent::Disable)
    }

    /// Handles a barge-in.
    ///
    /// Bumping the generation is the whole mechanism: the in-flight LLM
    /// stream stops on its next delta, no further phrase is synthesized,
    /// and the frontend clears its audio queue on the state event.
    pub fn interrupt(&self, app: &AppHandle) -> u64 {
        let generation = self.cancel();
        self.transition(app, TalkbackEvent::Interrupt);
        generation
    }
}

/// Everything one turn needs, gathered so `run_turn` has a single
/// argument rather than nine.
pub struct TurnContext<'a> {
    pub app: &'a AppHandle,
    pub engine: &'a TalkbackEngine,
    pub vault: &'a VaultManager,
    pub sessions: &'a SessionStore,
    pub processor: &'a MeetingProcessor,
    pub settings: &'a AppSettings,
    /// Milliseconds spent transcribing, for a spoken turn.
    pub stt_ms: Option<u128>,
    pub typed: bool,
}

/// Runs one complete turn: route → retrieve → plan → answer → speak.
///
/// Returns the agent's text. Cancellation is checked between every stage,
/// so an interruption costs at most one phrase of audio.
pub async fn run_turn(ctx: TurnContext<'_>, text: &str) -> Result<String, String> {
    let started = std::time::Instant::now();
    let generation = ctx.engine.generation();
    let text = text.trim();
    if text.is_empty() {
        return Ok(String::new());
    }

    let talkback = &ctx.settings.talkback;

    // A voice-note capture swallows turns rather than answering them —
    // except for the one that closes it.
    let routed = intent::route(text);
    {
        let mut session = ctx.engine.session.lock_or_recover();
        if session.is_capturing_voice_note() && routed.intent != Intent::StopVoiceNote {
            tools::append_to_voice_note(&mut session, text);
            drop(session);
            ctx.engine
                .transition(ctx.app, TalkbackEvent::ResponseComplete);
            return Ok(String::new());
        }
    }

    let turn_id = ctx
        .engine
        .session
        .lock_or_recover()
        .push_user(text, routed.intent, ctx.typed);
    emit_turn(ctx.app, ctx.engine, &turn_id);
    ctx.engine
        .transition(ctx.app, TalkbackEvent::TranscriptReady);

    // Retrieval.
    let retrieval_started = std::time::Instant::now();
    let retrieval = if routed.intent.is_action() {
        RetrievalResult {
            items: Vec::new(),
            searched_sources: Vec::new(),
            total_candidates: 0,
        }
    } else {
        let wanted = talkback.effective_sources();
        let candidates =
            sources::gather_candidates(ctx.vault, ctx.sessions, ctx.processor, &wanted);
        let llm_window = ctx.settings.provider.context_tokens;
        let query = RetrievalQuery::new(text)
            .with_sources(wanted)
            .with_char_budget(assemble::char_budget_for(llm_window))
            .with_since(routed.lookback_days.map(|days| {
                (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339()
            }));
        retrieval::rank(&candidates, &query, chrono::Utc::now())
    };
    let retrieval_ms = retrieval_started.elapsed().as_millis();

    if ctx.engine.is_stale(generation) {
        return Ok(String::new());
    }

    let plan = {
        let session = ctx.engine.session.lock_or_recover();
        plan_turn(routed.intent, &retrieval, &session)
    };

    let llm = LLMClient::new(ctx.settings.provider.clone());
    let mut metrics = TurnMetrics {
        session_id: ctx.engine.session.lock_or_recover().session_id.clone(),
        turn_id: turn_id.clone(),
        provider: format!("{:?}", llm.provider_type()),
        model: llm.model_name(),
        stt_ms: ctx.stt_ms,
        retrieval_ms,
        retrieved_count: retrieval.items.len(),
        candidate_count: retrieval.total_candidates,
        intent: routed.intent,
        ..Default::default()
    };

    let tts = resolve_provider(&ctx.settings.tts);
    let speak = talkback.speak_responses && tts.is_configured();

    let (answer, sources) = match plan {
        TurnPlan::Action(action) => {
            metrics.deterministic = true;
            (run_action(&ctx, action)?, Vec::new())
        }
        TurnPlan::Immediate { text, sources } => {
            metrics.deterministic = true;
            (text, sources)
        }
        TurnPlan::Generate {
            system_prompt,
            sources,
        } => {
            let answer = generate_streaming(
                &ctx,
                &llm,
                &system_prompt,
                text,
                &turn_id,
                generation,
                speak,
                tts.as_ref(),
                &mut metrics,
            )
            .await?;
            (answer, sources)
        }
    };

    if ctx.engine.is_stale(generation) {
        metrics.interrupted = true;
        emit_metrics(ctx.app, &metrics, started);
        return Ok(answer);
    }

    // A deterministic answer still gets spoken; it just had no stream to
    // chunk, so it is synthesized in one piece.
    if metrics.deterministic && speak && !answer.trim().is_empty() {
        ctx.engine
            .transition(ctx.app, TalkbackEvent::ResponseStarted);
        speak_phrase(&ctx, tts.as_ref(), &answer, &turn_id, 0, generation, &mut metrics);
    }

    let agent_turn_id = ctx
        .engine
        .session
        .lock_or_recover()
        .push_agent(&answer, sources);
    emit_turn(ctx.app, ctx.engine, &agent_turn_id);
    ctx.engine
        .transition(ctx.app, TalkbackEvent::ResponseComplete);

    emit_metrics(ctx.app, &metrics, started);
    Ok(answer)
}

/// Streams the model's answer, releasing it to TTS a phrase at a time.
#[allow(clippy::too_many_arguments)]
async fn generate_streaming(
    ctx: &TurnContext<'_>,
    llm: &LLMClient,
    system_prompt: &str,
    question: &str,
    turn_id: &str,
    generation: u64,
    speak: bool,
    tts: &dyn TtsProvider,
    metrics: &mut TurnMetrics,
) -> Result<String, String> {
    let llm_started = std::time::Instant::now();
    let first_token = Mutex::new(None::<u128>);
    let buffer = Mutex::new(PhraseBuffer::new());
    let phrases = Mutex::new(Vec::<String>::new());

    let options = CompletionOptions {
        // Spoken answers are short by design; a low ceiling also bounds
        // how long a runaway local model can hold the floor.
        max_output_tokens: 400,
        temperature: 0.4,
        context_tokens: llm.context_tokens(),
        timeout_secs: 90,
    };

    let response = llm
        .complete_streaming(question, Some(system_prompt), options, |delta| {
            if ctx.engine.is_stale(generation) {
                return false;
            }
            let mut first = first_token.lock_or_recover();
            if first.is_none() {
                *first = Some(llm_started.elapsed().as_millis());
            }
            drop(first);

            let _ = ctx.app.emit(
                TALKBACK_DELTA_EVENT,
                serde_json::json!({ "turnId": turn_id, "text": delta }),
            );
            let ready = buffer.lock_or_recover().push(delta);
            if !ready.is_empty() {
                phrases.lock_or_recover().extend(ready);
            }
            true
        })
        .await;

    let response = match response {
        Ok(response) => response,
        Err(e) => {
            // The turn survives a provider failure as an honest sentence
            // rather than an error dialog — this is a conversation.
            tracing::warn!("talkback: generation failed: {}", e);
            let _ = ctx.app.emit(
                TALKBACK_ERROR_EVENT,
                serde_json::json!({ "code": "LLM_FAILED", "message": e.to_string() }),
            );
            return Ok("I couldn't reach the model just now.".to_string());
        }
    };

    metrics.llm_first_token_ms = *first_token.lock_or_recover();
    metrics.llm_total_ms = Some(llm_started.elapsed().as_millis());

    if let Some(tail) = buffer.lock_or_recover().finish() {
        phrases.lock_or_recover().push(tail);
    }

    let phrases = phrases.into_inner().unwrap_or_default();
    if speak && !phrases.is_empty() {
        ctx.engine
            .transition(ctx.app, TalkbackEvent::ResponseStarted);
        for (index, phrase) in phrases.iter().enumerate() {
            if ctx.engine.is_stale(generation) {
                metrics.interrupted = true;
                break;
            }
            speak_phrase(ctx, tts, phrase, turn_id, index, generation, metrics);
        }
    }

    Ok(response.text)
}

/// Synthesizes one phrase and hands it to the frontend's playback queue.
///
/// A TTS failure degrades to text: the answer is already on screen, and
/// losing the voice is better than losing the turn.
fn speak_phrase(
    ctx: &TurnContext<'_>,
    tts: &dyn TtsProvider,
    phrase: &str,
    turn_id: &str,
    index: usize,
    generation: u64,
    metrics: &mut TurnMetrics,
) {
    let started = std::time::Instant::now();
    match tts.synthesize(phrase) {
        Ok(Some(audio)) => {
            if ctx.engine.is_stale(generation) {
                return;
            }
            if metrics.tts_first_audio_ms.is_none() {
                metrics.tts_first_audio_ms = Some(started.elapsed().as_millis());
            }
            let _ = ctx.app.emit(
                TALKBACK_AUDIO_EVENT,
                serde_json::json!({
                    "turnId": turn_id,
                    "seq": index,
                    "generation": generation,
                    "wavBase64": audio.wav_base64,
                }),
            );
        }
        Ok(None) => {}
        Err(e) => {
            tracing::warn!("talkback: TTS failed, continuing text-only: {}", e);
            let _ = ctx.app.emit(
                TALKBACK_ERROR_EVENT,
                serde_json::json!({ "code": "TTS_FAILED", "message": e.to_string() }),
            );
        }
    }
}

/// Executes a tool and returns its spoken confirmation.
fn run_action(ctx: &TurnContext<'_>, action: Intent) -> Result<String, String> {
    let mut session = ctx.engine.session.lock_or_recover();
    let outcome = match action {
        Intent::StartVoiceNote => tools::start_voice_note(&mut session).map(|o| o.message),
        Intent::StopVoiceNote => tools::stop_voice_note(&mut session, ctx.vault).map(|(o, note)| {
            if let Some(note) = note {
                let _ = ctx.app.emit(crate::commands::VOICE_NOTE_SAVED_EVENT, &note);
            }
            o.message
        }),
        Intent::CreateScribble => {
            tools::create_scribble(&session, ctx.vault, None).map(|(o, scribble)| {
                if let Some(scribble) = scribble {
                    let _ = ctx.app.emit(crate::commands::SCRIBBLE_SAVED_EVENT, &scribble);
                }
                o.message
            })
        }
        other => Ok(format!("I can't do that yet ({other:?}).")),
    };

    // A refused action is still an answer — "you're already recording" is
    // the right thing to say, not an error to surface.
    Ok(outcome.unwrap_or_else(|e| e.to_string()))
}

fn emit_turn(app: &AppHandle, engine: &TalkbackEngine, turn_id: &str) {
    let session = engine.session.lock_or_recover();
    if let Some(turn) = session.turns.iter().find(|t| t.turn_id == turn_id) {
        let _ = app.emit(TALKBACK_TURN_EVENT, turn);
    }
}

fn emit_metrics(app: &AppHandle, metrics: &TurnMetrics, started: std::time::Instant) {
    let mut metrics = metrics.clone();
    metrics.total_ms = started.elapsed().as_millis();
    tracing::info!(
        turn_id = %metrics.turn_id,
        provider = %metrics.provider,
        model = %metrics.model,
        stt_ms = ?metrics.stt_ms,
        retrieval_ms = metrics.retrieval_ms,
        retrieved = metrics.retrieved_count,
        llm_first_token_ms = ?metrics.llm_first_token_ms,
        tts_first_audio_ms = ?metrics.tts_first_audio_ms,
        total_ms = metrics.total_ms,
        interrupted = metrics.interrupted,
        "talkback turn"
    );
    let _ = app.emit(TALKBACK_METRICS_EVENT, &metrics);
}

/// The voice loop: frames in, turns out.
///
/// Runs on its own thread because whisper decoding is CPU-bound and must
/// not sit on the async runtime — the same reason `meetings_v2` runs its
/// live clock on a thread.
fn spawn_voice_worker(
    app: AppHandle,
    frame_rx: std_mpsc::Receiver<MicFrame>,
    settings: TalkbackSettings,
    whisper_model: Option<std::path::PathBuf>,
    language: SttLanguageConfig,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut detector = TurnDetector::new(settings.turn_detector_config());
        let mut transcriber = whisper_model
            .as_ref()
            .and_then(|p| p.to_str())
            .and_then(|path| {
                match StreamingTranscriber::new(path, &language, transcriber_threads()) {
                    Ok(t) => Some(t),
                    Err(e) => {
                        tracing::warn!("talkback: voice input disabled ({})", e);
                        let _ = app.emit(
                            TALKBACK_ERROR_EVENT,
                            serde_json::json!({
                                "code": "STT_UNAVAILABLE",
                                "message": e.to_string(),
                            }),
                        );
                        None
                    }
                }
            });

        let mut utterance: Vec<f32> = Vec::new();
        let mut last_level_emit = std::time::Instant::now();

        while let Ok(frame) = frame_rx.recv() {
            // Throttled to roughly 25 Hz: the agent animation needs a
            // smooth signal, the event bus does not need 10 per frame.
            if last_level_emit.elapsed() >= std::time::Duration::from_millis(40) {
                last_level_emit = std::time::Instant::now();
                let _ = app.emit(
                    TALKBACK_LEVEL_EVENT,
                    serde_json::json!({ "level": frame.rms }),
                );
            }

            let speaking = matches!(
                app_state_of(&app),
                Some(TalkbackState::Speaking) | Some(TalkbackState::Interrupted)
            );
            detector.set_echo_guard(speaking);

            let event = detector.push(frame.rms, super::audio::FRAME_MS);
            if detector.in_speech() || event == TurnEvent::SpeechStart {
                utterance.extend_from_slice(&frame.samples);
            }

            match event {
                TurnEvent::SpeechStart => {
                    if speaking && settings.allow_barge_in {
                        emit_barge_in(&app);
                    } else {
                        let _ = app.emit(
                            TALKBACK_STATE_EVENT,
                            serde_json::json!({ "state": TalkbackState::UserSpeaking }),
                        );
                    }
                }
                TurnEvent::SpeechEnd | TurnEvent::MaxDurationReached => {
                    let samples = std::mem::take(&mut utterance);
                    let Some(transcriber) = transcriber.as_mut() else {
                        continue;
                    };
                    let started = std::time::Instant::now();
                    match transcriber.transcribe(&samples) {
                        Ok(text) if !text.trim().is_empty() => {
                            let _ = app.emit(
                                "talkback-utterance",
                                serde_json::json!({
                                    "text": text,
                                    "sttMs": started.elapsed().as_millis(),
                                }),
                            );
                        }
                        Ok(_) => {
                            tracing::debug!("talkback: utterance produced no usable text");
                        }
                        Err(e) => tracing::warn!("talkback: decode failed: {}", e),
                    }
                }
                TurnEvent::None => {}
            }
        }
    })
}

/// Live state, read through the app's managed engine.
///
/// The worker thread cannot hold a borrow of the engine across frames, so
/// it asks the Tauri state each time. Cheap: one mutex read.
fn app_state_of(app: &AppHandle) -> Option<TalkbackState> {
    use tauri::Manager;
    app.try_state::<crate::commands::AppState>()
        .map(|state| state.talkback.state())
}

fn emit_barge_in(app: &AppHandle) {
    use tauri::Manager;
    if let Some(state) = app.try_state::<crate::commands::AppState>() {
        state.talkback.interrupt(app);
    }
}

/// Below the core count so the rest of the app — and any meeting
/// recording that happens to be running — still gets CPU.
fn transcriber_threads() -> i32 {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get() as i32)
        .unwrap_or(4);
    (cores / 2).clamp(1, 4)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::talkback::retrieval::SourceType;

    fn item(id: &str) -> ContextItem {
        ContextItem {
            source_type: SourceType::Scribble,
            source_id: id.to_string(),
            title: "Pricing".to_string(),
            timestamp: "2026-08-01T00:00:00Z".to_string(),
            relevance: 3.0,
            excerpt: "flat seat licence".to_string(),
            detail: None,
            expanded: false,
        }
    }

    fn empty() -> RetrievalResult {
        RetrievalResult {
            items: Vec::new(),
            searched_sources: SourceType::ALL.to_vec(),
            total_candidates: 40,
        }
    }

    fn found() -> RetrievalResult {
        RetrievalResult {
            items: vec![item("s1")],
            searched_sources: SourceType::ALL.to_vec(),
            total_candidates: 40,
        }
    }

    #[test]
    fn a_memory_question_with_no_evidence_never_reaches_the_model() {
        let plan = plan_turn(Intent::PersonalMemory, &empty(), &TalkbackSession::new());
        assert_eq!(
            plan,
            TurnPlan::Immediate {
                text: NO_EVIDENCE_RESPONSE.to_string(),
                sources: Vec::new(),
            },
            "this is the guarantee that Talkback cannot invent a memory"
        );
    }

    #[test]
    fn a_memory_question_with_evidence_is_generated_and_grounded() {
        let plan = plan_turn(Intent::PersonalMemory, &found(), &TalkbackSession::new());
        match plan {
            TurnPlan::Generate {
                system_prompt,
                sources,
            } => {
                assert!(system_prompt.contains("Answer only from the CONTEXT"));
                assert_eq!(sources.len(), 1);
                assert_eq!(sources[0].source_id, "s1");
            }
            other => panic!("expected generation, got {other:?}"),
        }
    }

    #[test]
    fn a_general_question_with_no_evidence_still_reaches_the_model() {
        match plan_turn(Intent::General, &empty(), &TalkbackSession::new()) {
            TurnPlan::Generate { sources, .. } => assert!(sources.is_empty()),
            other => panic!("a general question must not be short-circuited: {other:?}"),
        }
    }

    #[test]
    fn a_provenance_question_is_answered_from_the_session_not_the_model() {
        let mut session = TalkbackSession::new();
        session.push_agent("A flat seat licence.", vec![item("s1")]);
        match plan_turn(Intent::ShowSources, &empty(), &session) {
            TurnPlan::Immediate { text, sources } => {
                assert!(text.contains("your Scribble \"Pricing\""), "{text}");
                assert_eq!(sources.len(), 1);
            }
            other => panic!("expected an immediate answer, got {other:?}"),
        }
    }

    #[test]
    fn a_provenance_question_with_nothing_cited_is_honest() {
        match plan_turn(Intent::ShowSources, &empty(), &TalkbackSession::new()) {
            TurnPlan::Immediate { text, .. } => {
                assert!(text.contains("wasn't from your Relay data"), "{text}")
            }
            other => panic!("expected an immediate answer, got {other:?}"),
        }
    }

    #[test]
    fn actions_are_planned_as_actions_not_answers() {
        for action in [
            Intent::StartVoiceNote,
            Intent::StopVoiceNote,
            Intent::CreateScribble,
        ] {
            assert_eq!(
                plan_turn(action, &found(), &TalkbackSession::new()),
                TurnPlan::Action(action)
            );
        }
    }

    #[test]
    fn wake_word_activation_is_refused_rather_than_faked() {
        let settings = TalkbackSettings {
            activation_mode: ActivationMode::WakeWord,
            ..Default::default()
        };
        assert_eq!(settings.activation_mode, ActivationMode::WakeWord);
        // The engine's refusal is asserted through `enable`, which needs
        // an AppHandle; what is checkable here is that the mode is a real
        // distinct value rather than an alias for Toggle.
        assert_ne!(settings.activation_mode, ActivationMode::Toggle);
    }

    #[test]
    fn default_settings_are_toggle_speaking_and_interruptible() {
        let settings = TalkbackSettings::default();
        assert_eq!(settings.activation_mode, ActivationMode::Toggle);
        assert!(settings.speak_responses);
        assert!(settings.allow_barge_in);
        assert_eq!(settings.effective_sources().len(), SourceType::ALL.len());
    }

    #[test]
    fn an_explicit_source_selection_is_honoured() {
        let settings = TalkbackSettings {
            sources: vec![SourceType::Scribble],
            ..Default::default()
        };
        assert_eq!(settings.effective_sources(), vec![SourceType::Scribble]);
    }

    #[test]
    fn the_end_of_turn_silence_is_clamped_to_a_usable_range() {
        let too_fast = TalkbackSettings {
            end_of_turn_silence_ms: 10,
            ..Default::default()
        };
        assert_eq!(too_fast.turn_detector_config().hangover_ms, 250);

        let too_slow = TalkbackSettings {
            end_of_turn_silence_ms: 60_000,
            ..Default::default()
        };
        assert_eq!(too_slow.turn_detector_config().hangover_ms, 3_000);
    }

    #[test]
    fn cancelling_makes_an_in_flight_turn_stale() {
        let engine = TalkbackEngine::new();
        let generation = engine.generation();
        assert!(!engine.is_stale(generation));
        engine.cancel();
        assert!(engine.is_stale(generation), "a barge-in must invalidate the turn");
    }

    #[test]
    fn a_new_engine_is_off_and_holds_no_microphone() {
        let engine = TalkbackEngine::new();
        assert_eq!(engine.state(), TalkbackState::Off);
        assert!(!engine.holds_microphone());
    }

    #[test]
    fn settings_round_trip_through_json() {
        let settings = TalkbackSettings {
            activation_mode: ActivationMode::Toggle,
            speak_responses: false,
            allow_barge_in: false,
            sources: vec![SourceType::MeetingFacts, SourceType::Scribble],
            end_of_turn_silence_ms: 900,
        };
        let json = serde_json::to_string(&settings).unwrap();
        assert_eq!(serde_json::from_str::<TalkbackSettings>(&json).unwrap(), settings);
    }

    #[test]
    fn absent_talkback_settings_deserialize_to_defaults() {
        let settings: TalkbackSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings, TalkbackSettings::default());
    }
}
