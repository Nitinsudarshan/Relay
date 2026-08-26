use super::capture::{AudioChunk, DualAudioCapture, LiveAudioFrame};
use super::live_stt::{live_thread_count, LiveSttWorker};
use super::session_store::SessionStore;
use super::types::{MeetingDiagnostics, MeetingSession, MeetingState};
use super::worker::TranscriptionWorker;
use crate::capture::stt::{SttEngine, SttLanguageConfig, WhisperDecodingConfig};
use std::path::{Path, PathBuf};
use std::sync::mpsc as std_mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

/// Live audio frames buffered for the live clock before frames start dropping.
const LIVE_QUEUE_DEPTH: usize = 8;

pub struct ActiveSessionContext {
    pub session: MeetingSession,
    pub capture: DualAudioCapture,
    pub worker: TranscriptionWorker,
    pub live_stt: LiveSttWorker,
    pub start_instant: Instant,
    /// When the current pause began, if the session is paused.
    pub paused_at: Option<Instant>,
    /// Total time spent paused across the session.
    pub paused_total: Duration,
}

impl ActiveSessionContext {
    fn paused_duration(&self) -> Duration {
        match self.paused_at {
            Some(at) => self.paused_total + at.elapsed(),
            None => self.paused_total,
        }
    }

    /// Recording time excluding paused intervals. This — not the wall clock
    /// since `started_at` — is what the UI must show, otherwise the timer keeps
    /// counting through pauses.
    fn recorded_seconds(&self) -> f64 {
        self.start_instant
            .elapsed()
            .saturating_sub(self.paused_duration())
            .as_secs_f64()
    }

    /// The session as it stands right now, with live values filled in.
    fn snapshot(&self) -> MeetingSession {
        let mut s = self.session.clone();
        s.duration_seconds = self.recorded_seconds();
        s.paused_seconds = self.paused_duration().as_secs_f64();
        s.mic_active = self.capture.is_mic_active();
        s.sys_audio_active = self.capture.is_sys_active();
        s.mic_heard = s.mic_heard || self.capture.mic_heard();
        s.sys_audio_heard = s.sys_audio_heard || self.capture.sys_heard();
        s
    }
}

pub struct MeetingsV2Engine {
    store: Arc<SessionStore>,
    /// The session that owns the capture devices and workers.
    active_session: Arc<Mutex<Option<ActiveSessionContext>>>,
    /// Snapshot of a session that has been stopped but is still finalizing.
    ///
    /// Holding this separately is what makes stop idempotent and keeps the
    /// recording pill showing "finalizing" instead of blinking back to idle:
    /// the capture context is already gone, but the session is not yet done.
    finalizing_session: Arc<Mutex<Option<MeetingSession>>>,
    stt: SttEngine,
}

impl MeetingsV2Engine {
    pub fn new(vault_dir: PathBuf, stt: SttEngine) -> Self {
        let store = Arc::new(SessionStore::new(vault_dir));
        Self {
            store,
            active_session: Arc::new(Mutex::new(None)),
            finalizing_session: Arc::new(Mutex::new(None)),
            stt,
        }
    }

    pub fn store(&self) -> Arc<SessionStore> {
        self.store.clone()
    }

    pub fn is_recording(&self) -> bool {
        let guard = self.active_session.lock().unwrap();
        guard.as_ref().map_or(false, |ctx| {
            matches!(
                ctx.session.state,
                MeetingState::Recording | MeetingState::Paused
            )
        })
    }

    /// The session the UI should be showing, whether it is recording, paused,
    /// or still finalizing after a stop.
    pub fn get_active_session(&self) -> Option<MeetingSession> {
        if let Some(ctx) = self.active_session.lock().unwrap().as_ref() {
            return Some(ctx.snapshot());
        }
        self.finalizing_session.lock().unwrap().clone()
    }

    /// Resolves the language for one recording.
    ///
    /// A per-meeting choice wins over the global profile, and `"auto"` means
    /// auto-detect. `translate` is always off: translating Hinglish to English
    /// and then summarizing loses more than summarizing across languages does.
    fn resolve_language(
        requested: Option<&str>,
        global: &SttLanguageConfig,
    ) -> (SttLanguageConfig, Option<String>) {
        match requested.map(str::trim).filter(|r| !r.is_empty()) {
            Some(code) if code.eq_ignore_ascii_case("auto") => (
                SttLanguageConfig {
                    whisper_language: None,
                    translate: false,
                },
                Some("auto".to_string()),
            ),
            Some(code) => {
                let code = code.to_lowercase();
                (
                    SttLanguageConfig {
                        whisper_language: Some(code.clone()),
                        translate: false,
                    },
                    Some(code),
                )
            }
            None => (
                SttLanguageConfig {
                    whisper_language: global.whisper_language.clone(),
                    translate: false,
                },
                global
                    .whisper_language
                    .clone()
                    .or_else(|| Some("auto".to_string())),
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn start_session(
        &self,
        title: Option<String>,
        language: Option<String>,
        models_dir: &Path,
        whisper_model_path: Option<String>,
        language_config: SttLanguageConfig,
        decoding_config: WhisperDecodingConfig,
        app: Option<AppHandle>,
    ) -> Result<MeetingSession, String> {
        let mut guard = self.active_session.lock().unwrap();
        if guard.is_some() {
            return Err("A meeting recording session is already active".to_string());
        }
        if self.finalizing_session.lock().unwrap().is_some() {
            return Err("The previous meeting is still finalizing — try again in a moment".to_string());
        }

        let session_id = format!("meet_{}", uuid::Uuid::new_v4());
        let (language_config, resolved_language) =
            Self::resolve_language(language.as_deref(), &language_config);

        let mut session = MeetingSession::new(session_id.clone(), title);
        session.state = MeetingState::Starting;
        session.language = resolved_language;
        self.store.init_session(&session)?;

        let resolved_model_path = match whisper_model_path.filter(|p| !p.trim().is_empty()) {
            Some(p) => Some(PathBuf::from(p)),
            None => {
                let default_path = models_dir.join(crate::capture::stt::DEFAULT_MODEL_FILENAME);
                if default_path.exists() {
                    Some(default_path)
                } else {
                    None
                }
            }
        };

        // CLOCK A: durable 30 s chunks. Unbounded — durable audio is never dropped.
        let (chunk_tx, chunk_rx) = std_mpsc::channel::<AudioChunk>();
        // CLOCK B: live frames. Bounded, so a slow decode drops live frames
        // rather than stalling capture.
        let (live_tx, live_rx) = std_mpsc::sync_channel::<LiveAudioFrame>(LIVE_QUEUE_DEPTH);

        // The two clocks run concurrently on their own Whisper contexts, so
        // split the cores between them rather than letting each claim all of
        // them and fight.
        let mut durable_decoding_config = decoding_config;
        durable_decoding_config.n_threads = Some(durable_thread_count());
        // Temperature fallback stays enabled here: when a decode trips the
        // entropy or log-probability threshold, whisper.cpp retries the window
        // hotter, which is what breaks a loop instead of recording it. The live
        // clock disables it for latency, and can afford to — the durable
        // transcript is the one that has to be right.
        durable_decoding_config.no_context = true;

        let worker = TranscriptionWorker::spawn(
            session_id.clone(),
            chunk_rx,
            self.store.clone(),
            self.stt.clone(),
            resolved_model_path.clone(),
            language_config.clone(),
            durable_decoding_config,
            app.clone(),
        );

        let live_stt = LiveSttWorker::spawn(
            session_id.clone(),
            live_rx,
            resolved_model_path,
            language_config.clone(),
            app.clone(),
        );

        // Capture resolves its devices before returning, so a failure to record
        // surfaces here instead of producing a silent session.
        let capture = match DualAudioCapture::start(
            session_id.clone(),
            chunk_tx,
            Some(live_tx),
            app.clone(),
        ) {
            Ok(capture) => capture,
            Err(e) => {
                let mut worker = worker;
                let mut live_stt = live_stt;
                worker.join();
                live_stt.stop();
                live_stt.join();
                let _ = self.store.update_session(&session_id, |s| {
                    s.state = MeetingState::Error;
                    s.error_message = Some(e.clone());
                });
                return Err(e);
            }
        };

        session.state = MeetingState::Recording;
        session.mic_active = capture.is_mic_active();
        session.sys_audio_active = capture.is_sys_active();
        session.capture_warning = capture.warning();

        let persisted = self
            .store
            .update_session(&session_id, |s| {
                s.state = session.state;
                s.mic_active = session.mic_active;
                s.sys_audio_active = session.sys_audio_active;
                s.capture_warning = session.capture_warning.clone();
            })
            .unwrap_or_else(|_| session.clone());

        emit_state(&app, &persisted);

        *guard = Some(ActiveSessionContext {
            session: persisted.clone(),
            capture,
            worker,
            live_stt,
            start_instant: Instant::now(),
            paused_at: None,
            paused_total: Duration::ZERO,
        });

        tracing::info!("MeetingsV2: successfully started session {}", session_id);
        Ok(persisted)
    }

    /// Pauses capture. Audio arriving while paused is discarded, so the
    /// recording resumes contiguously rather than containing the gap.
    pub fn pause_session(
        &self,
        session_id: Option<String>,
        app: Option<AppHandle>,
    ) -> Result<MeetingSession, String> {
        let mut guard = self.active_session.lock().unwrap();
        let ctx = guard
            .as_mut()
            .ok_or_else(|| "No active meeting recording session to pause".to_string())?;
        fence(&ctx.session.id, session_id.as_deref())?;

        if ctx.paused_at.is_some() {
            return Ok(ctx.snapshot());
        }

        ctx.capture.pause();
        ctx.paused_at = Some(Instant::now());
        ctx.session.state = MeetingState::Paused;

        let snapshot = ctx.snapshot();
        let _ = self.store.update_session(&snapshot.id, |s| {
            s.state = MeetingState::Paused;
            s.duration_seconds = snapshot.duration_seconds;
            s.paused_seconds = snapshot.paused_seconds;
        });
        emit_state(&app, &snapshot);
        tracing::info!("MeetingsV2: paused session {}", snapshot.id);
        Ok(snapshot)
    }

    pub fn resume_session(
        &self,
        session_id: Option<String>,
        app: Option<AppHandle>,
    ) -> Result<MeetingSession, String> {
        let mut guard = self.active_session.lock().unwrap();
        let ctx = guard
            .as_mut()
            .ok_or_else(|| "No active meeting recording session to resume".to_string())?;
        fence(&ctx.session.id, session_id.as_deref())?;

        let paused_at = match ctx.paused_at.take() {
            Some(at) => at,
            None => return Ok(ctx.snapshot()),
        };

        ctx.paused_total += paused_at.elapsed();
        ctx.capture.resume();
        ctx.session.state = MeetingState::Recording;

        let snapshot = ctx.snapshot();
        let _ = self.store.update_session(&snapshot.id, |s| {
            s.state = MeetingState::Recording;
            s.paused_seconds = snapshot.paused_seconds;
        });
        emit_state(&app, &snapshot);
        tracing::info!("MeetingsV2: resumed session {}", snapshot.id);
        Ok(snapshot)
    }

    /// Stops the active session and finalizes it.
    ///
    /// Idempotent and fenced: concurrent stops (the pill and the meetings page
    /// both have a stop button, and a session can also end from a
    /// non-interactive path) collapse into one teardown, and a stop naming a
    /// session that is no longer current is rejected rather than tearing down
    /// whatever happens to be recording now.
    pub async fn stop_session(
        &self,
        session_id: Option<String>,
        app: Option<AppHandle>,
    ) -> Result<MeetingSession, String> {
        let ctx = {
            let mut guard = self.active_session.lock().unwrap();
            match guard.take() {
                Some(ctx) => {
                    if let Err(e) = fence(&ctx.session.id, session_id.as_deref()) {
                        *guard = Some(ctx);
                        return Err(e);
                    }
                    ctx
                }
                None => {
                    // A stop is already in flight: report its state instead of
                    // surfacing a spurious "nothing to stop" error.
                    if let Some(finalizing) = self.finalizing_session.lock().unwrap().clone() {
                        return Ok(finalizing);
                    }
                    return Err("No active meeting recording session to stop".to_string());
                }
            }
        };

        let session_id = ctx.session.id.clone();
        let recorded_seconds = ctx.recorded_seconds();
        let paused_seconds = ctx.paused_duration().as_secs_f64();
        let mic_heard = ctx.capture.mic_heard();
        let sys_heard = ctx.capture.sys_heard();

        let stopping = self
            .store
            .update_session(&session_id, |s| {
                s.state = MeetingState::Stopping;
                s.duration_seconds = recorded_seconds;
                s.paused_seconds = paused_seconds;
            })
            .unwrap_or_else(|_| ctx.session.clone());
        *self.finalizing_session.lock().unwrap() = Some(stopping.clone());
        emit_state(&app, &stopping);

        tracing::info!(
            "MeetingsV2: stopping capture streams for session {}...",
            session_id
        );

        // Teardown blocks on audio threads and on draining however many chunks
        // are still queued for transcription, so it must not run on an async
        // executor thread.
        let drain_result = tokio::task::spawn_blocking(move || {
            let mut ctx = ctx;
            // Stop capture first: it flushes the tail of the recording and drops
            // the channel senders, which is what tells the workers to finish.
            ctx.capture.stop();
            // The live clock's remaining frames are disposable; the durable
            // clock's are not.
            ctx.live_stt.stop();
            ctx.worker.join();
            ctx.live_stt.join();
        })
        .await;

        if let Err(e) = drain_result {
            tracing::error!("MeetingsV2: teardown task failed for {}: {}", session_id, e);
        }

        let finalizing = self
            .store
            .update_session(&session_id, |s| s.state = MeetingState::Finalizing)
            .unwrap_or_else(|_| stopping.clone());
        *self.finalizing_session.lock().unwrap() = Some(finalizing.clone());
        emit_state(&app, &finalizing);

        let store = self.store.clone();
        let finalize_id = session_id.clone();
        let final_session = tokio::task::spawn_blocking(move || {
            let chunk_count = store.list_chunk_files(&finalize_id).unwrap_or_default().len();

            let final_session = store.update_session(&finalize_id, |s| {
                s.state = MeetingState::Completed;
                s.ended_at = Some(chrono::Utc::now().to_rfc3339());
                s.duration_seconds = recorded_seconds;
                s.paused_seconds = paused_seconds;
                s.chunk_count = chunk_count;
                s.pending_transcription_chunks = 0;
                s.mic_heard = s.mic_heard || mic_heard;
                s.sys_audio_heard = s.sys_audio_heard || sys_heard;
                s.mic_active = false;
                s.sys_audio_active = false;
            })?;

            let _ = store.merge_chunks_to_full_audio(&finalize_id);
            let transcript_text = store
                .get_full_transcript_text(&finalize_id)
                .unwrap_or_default();
            let _ = store.generate_markdown_note(&final_session, &transcript_text);

            Ok::<MeetingSession, String>(final_session)
        })
        .await
        .map_err(|e| format!("Finalization task failed: {}", e))?;

        *self.finalizing_session.lock().unwrap() = None;

        match final_session {
            Ok(final_session) => {
                emit_state(&app, &final_session);
                tracing::info!("MeetingsV2: session {} completed successfully.", session_id);
                Ok(final_session)
            }
            Err(e) => {
                tracing::error!("MeetingsV2: failed to finalize {}: {}", session_id, e);
                // The UI must still leave the recording state even when
                // finalization fails, or the pill stays up forever.
                let mut fallback = finalizing;
                fallback.state = MeetingState::Error;
                fallback.error_message = Some(e.clone());
                emit_state(&app, &fallback);
                Err(e)
            }
        }
    }

    pub fn get_diagnostics(&self) -> Result<Vec<MeetingDiagnostics>, String> {
        let mut diags = Vec::new();

        if let Some(active) = self.get_active_session() {
            diags.push(MeetingDiagnostics {
                session_id: active.id.clone(),
                state: active.state,
                duration_seconds: active.duration_seconds,
                last_audio_saved_at: Some(active.updated_at.clone()),
                chunk_count: active.chunk_count,
                total_audio_bytes: active.total_audio_bytes,
                last_transcription_at: Some(active.updated_at.clone()),
                transcript_segment_count: active.transcript_segment_count,
                pending_transcription_chunks: active.pending_transcription_chunks,
                mic_active: active.mic_active,
                sys_audio_active: active.sys_audio_active,
                mic_heard: active.mic_heard,
                sys_audio_heard: active.sys_audio_heard,
                mic_rms: 0.0,
                sys_rms: 0.0,
                error: active.error_message,
            });
        }

        Ok(diags)
    }

    /// Startup recovery: reconciles sessions that were interrupted mid-recording
    /// or mid-finalization.
    pub fn recover_interrupted_sessions(&self) -> Result<Vec<MeetingSession>, String> {
        let interrupted = self.store.scan_interrupted_sessions()?;
        let mut recovered_list = Vec::new();

        for session in interrupted {
            tracing::warn!(
                "MeetingsV2 recovery: reconciling interrupted session {}",
                session.id
            );

            // The chunk files on disk are authoritative for what was recorded.
            let _ = self.store.merge_chunks_to_full_audio(&session.id);
            let chunk_count = self
                .store
                .list_chunk_files(&session.id)
                .unwrap_or_default()
                .len();
            let transcript_text = self
                .store
                .get_full_transcript_text(&session.id)
                .unwrap_or_default();

            let recovered = self.store.update_session(&session.id, |s| {
                s.state = MeetingState::Recovered;
                s.chunk_count = chunk_count;
                s.pending_transcription_chunks = 0;
                s.mic_active = false;
                s.sys_audio_active = false;
            })?;

            let _ = self
                .store
                .generate_markdown_note(&recovered, &transcript_text);
            recovered_list.push(recovered);
        }

        Ok(recovered_list)
    }
}

/// Rejects an operation aimed at a session that is no longer the current one.
///
/// Callbacks and UI surfaces can outlive the session they were created for; a
/// stale stop must never tear down a newer recording. `None` means the caller
/// did not name a session and accepts whatever is current.
fn fence(active_id: &str, requested_id: Option<&str>) -> Result<(), String> {
    match requested_id {
        Some(requested) if requested != active_id => Err(format!(
            "Stale meeting session {} — {} is the active recording",
            requested, active_id
        )),
        _ => Ok(()),
    }
}

/// Threads for the durable clock: whatever the live clock is not using.
fn durable_thread_count() -> i32 {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get() as i32)
        .unwrap_or(4);
    (cores - live_thread_count()).clamp(1, 8)
}

fn emit_state(app: &Option<AppHandle>, session: &MeetingSession) {
    if let Some(a) = app {
        let _ = a.emit("meeting-session-state-changed", session);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stale_session_id_cannot_stop_the_current_recording() {
        assert!(fence("meet_new", Some("meet_old")).is_err());
        assert!(fence("meet_new", Some("meet_new")).is_ok());
    }

    #[test]
    fn an_unnamed_stop_targets_whatever_is_current() {
        assert!(fence("meet_new", None).is_ok());
    }

    fn global(lang: Option<&str>) -> SttLanguageConfig {
        SttLanguageConfig {
            whisper_language: lang.map(str::to_string),
            translate: false,
        }
    }

    #[test]
    fn a_per_meeting_language_overrides_the_global_profile() {
        let (config, stored) = MeetingsV2Engine::resolve_language(Some("hi"), &global(Some("en")));
        assert_eq!(config.whisper_language.as_deref(), Some("hi"));
        assert_eq!(stored.as_deref(), Some("hi"));
    }

    #[test]
    fn auto_means_auto_detect_not_english() {
        let (config, stored) = MeetingsV2Engine::resolve_language(Some("auto"), &global(Some("en")));
        assert_eq!(
            config.whisper_language, None,
            "auto must reach Whisper as auto-detect"
        );
        assert_eq!(stored.as_deref(), Some("auto"));
    }

    #[test]
    fn without_a_per_meeting_choice_the_global_profile_is_used() {
        let (config, stored) = MeetingsV2Engine::resolve_language(None, &global(Some("en")));
        assert_eq!(config.whisper_language.as_deref(), Some("en"));
        assert_eq!(stored.as_deref(), Some("en"));

        let (config, stored) = MeetingsV2Engine::resolve_language(None, &global(None));
        assert_eq!(config.whisper_language, None);
        assert_eq!(stored.as_deref(), Some("auto"));
    }

    #[test]
    fn translation_is_never_enabled() {
        // Translating Hinglish to English and then summarizing loses more than
        // summarizing across languages does.
        for requested in [Some("hi"), Some("auto"), None] {
            let (config, _) = MeetingsV2Engine::resolve_language(requested, &global(Some("en")));
            assert!(!config.translate);
        }
    }

    #[test]
    fn the_two_clocks_do_not_oversubscribe_the_cpu() {
        let total = std::thread::available_parallelism()
            .map(|n| n.get() as i32)
            .unwrap_or(4);
        assert!(durable_thread_count() >= 1);
        assert!(live_thread_count() >= 1);
        assert!(
            durable_thread_count() + live_thread_count() <= total.max(2),
            "durable + live threads must fit the machine"
        );
    }
}
