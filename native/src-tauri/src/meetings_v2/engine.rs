use super::capture::{AudioChunk, DualAudioCapture, LiveAudioFrame};
use super::live_stt::LiveSttWorker;
use super::session_store::SessionStore;
use super::types::{MeetingDiagnostics, MeetingSession, MeetingState};
use super::worker::TranscriptionWorker;
use crate::capture::stt::{SttEngine, SttLanguageConfig, WhisperDecodingConfig};
use std::path::{Path, PathBuf};
use std::sync::mpsc as std_mpsc;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

pub struct ActiveSessionContext {
    pub session: MeetingSession,
    pub capture: DualAudioCapture,
    pub worker: TranscriptionWorker,
    pub live_stt: LiveSttWorker,
    pub start_instant: std::time::Instant,
}

pub struct MeetingsV2Engine {
    store: Arc<SessionStore>,
    active_session: Arc<Mutex<Option<ActiveSessionContext>>>,
    stt: SttEngine,
}

impl MeetingsV2Engine {
    pub fn new(vault_dir: PathBuf, stt: SttEngine) -> Self {
        let store = Arc::new(SessionStore::new(vault_dir));
        Self {
            store,
            active_session: Arc::new(Mutex::new(None)),
            stt,
        }
    }

    pub fn store(&self) -> Arc<SessionStore> {
        self.store.clone()
    }

    pub fn is_recording(&self) -> bool {
        let guard = self.active_session.lock().unwrap();
        guard.as_ref().map_or(false, |ctx| ctx.session.state == MeetingState::Recording)
    }

    pub fn get_active_session(&self) -> Option<MeetingSession> {
        let guard = self.active_session.lock().unwrap();
        guard.as_ref().map(|ctx| {
            let mut s = ctx.session.clone();
            s.duration_seconds = ctx.start_instant.elapsed().as_secs_f64();
            s.mic_active = ctx.capture.is_mic_active();
            s.sys_audio_active = ctx.capture.is_sys_active();
            s
        })
    }

    pub fn start_session(
        &self,
        title: Option<String>,
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

        let session_id = format!("meet_{}", uuid::Uuid::new_v4());
        let mut session = MeetingSession::new(session_id.clone(), title);
        session.state = MeetingState::Starting;

        // Initialize session on disk
        self.store.init_session(&session)?;

        // Ensure default Whisper model if needed
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

        // 1. Channel for CLOCK A: 30s durable audio chunks
        let (chunk_tx, chunk_rx) = std_mpsc::channel::<AudioChunk>();

        // 2. Bounded channel for CLOCK B: Low-latency live STT frames (~1.5s windows)
        let (live_tx, live_rx) = std_mpsc::sync_channel::<LiveAudioFrame>(8);

        // Fast decoding config for responsive inference
        let mut fast_decoding_config = decoding_config;
        fast_decoding_config.temperature_inc = 0.0;

        // Spawn CLOCK A worker (Durable 30s recording & persistence)
        let worker = TranscriptionWorker::spawn(
            session_id.clone(),
            chunk_rx,
            self.store.clone(),
            self.stt.clone(),
            resolved_model_path.clone(),
            language_config.clone(),
            fast_decoding_config.clone(),
            app.clone(),
        );

        // Spawn CLOCK B worker (Low-latency ~1.5s live transcription)
        let live_stt = LiveSttWorker::spawn(
            session_id.clone(),
            live_rx,
            self.stt.clone(),
            resolved_model_path,
            language_config,
            fast_decoding_config,
            app.clone(),
        );

        // Start dual audio capture broadcasting to both clocks
        let capture = DualAudioCapture::start(session_id.clone(), chunk_tx, Some(live_tx), app.clone())?;

        session.state = MeetingState::Recording;
        session.mic_active = capture.is_mic_active();
        session.sys_audio_active = capture.is_sys_active();
        let _ = self.store.save_session(&session);

        if let Some(ref a) = app {
            let _ = a.emit("meeting-session-state-changed", &session);
        }

        let ctx = ActiveSessionContext {
            session: session.clone(),
            capture,
            worker,
            live_stt,
            start_instant: std::time::Instant::now(),
        };

        *guard = Some(ctx);
        tracing::info!("MeetingsV2: Successfully started session {}", session_id);
        Ok(session)
    }

    pub async fn stop_session(&self, app: Option<AppHandle>) -> Result<MeetingSession, String> {
        let mut ctx = {
            let mut guard = self.active_session.lock().unwrap();
            guard.take().ok_or_else(|| "No active meeting recording session to stop".to_string())?
        };

        let session_id = ctx.session.id.clone();
        ctx.session.state = MeetingState::Stopping;
        let _ = self.store.save_session(&ctx.session);

        if let Some(ref a) = app {
            let _ = a.emit("meeting-session-state-changed", &ctx.session);
        }

        tracing::info!("MeetingsV2: Stopping capture streams for session {}...", session_id);
        // 1. Signal workers and capture loop to stop and flush
        ctx.live_stt.stop();
        ctx.worker.stop();
        ctx.capture.stop();

        ctx.session.state = MeetingState::Finalizing;
        let _ = self.store.save_session(&ctx.session);
        if let Some(ref a) = app {
            let _ = a.emit("meeting-session-state-changed", &ctx.session);
        }

        tracing::info!("MeetingsV2: Waiting for workers to drain for session {}...", session_id);
        // 2. Wait for workers to cleanly finish
        ctx.live_stt.join();
        ctx.worker.join();

        // 3. Finalize session metadata and files
        let mut final_session = self.store.get_session(&session_id)?;
        final_session.state = MeetingState::Completed;
        final_session.ended_at = Some(chrono::Utc::now().to_rfc3339());
        final_session.duration_seconds = ctx.start_instant.elapsed().as_secs_f64();
        final_session.updated_at = chrono::Utc::now().to_rfc3339();

        let chunks = self.store.list_chunk_files(&session_id).unwrap_or_default();
        final_session.chunk_count = chunks.len();

        // Merge chunk WAVs to audio_full.wav
        let _ = self.store.merge_chunks_to_full_audio(&session_id);

        // Generate final Markdown note
        let transcript_text = self.store.get_full_transcript_text(&session_id).unwrap_or_default();
        let _ = self.store.generate_markdown_note(&final_session, &transcript_text);
        let _ = self.store.save_session(&final_session);

        if let Some(ref a) = app {
            let _ = a.emit("meeting-session-state-changed", &final_session);
        }

        tracing::info!("MeetingsV2: Session {} completed successfully.", session_id);
        Ok(final_session)
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
                mic_rms: 0.0,
                sys_rms: 0.0,
                error: active.error_message,
            });
        }

        Ok(diags)
    }

    /// Startup recovery method: reconciles interrupted sessions on launch.
    pub fn recover_interrupted_sessions(&self) -> Result<Vec<MeetingSession>, String> {
        let interrupted = self.store.scan_interrupted_sessions()?;
        let mut recovered_list = Vec::new();

        for mut session in interrupted {
            tracing::warn!("MeetingsV2 Recovery: Reconciling interrupted session {}", session.id);

            // Re-assemble full audio from whatever chunks exist on disk
            let _ = self.store.merge_chunks_to_full_audio(&session.id);

            // Read available transcript
            let transcript_text = self.store.get_full_transcript_text(&session.id).unwrap_or_default();

            session.state = MeetingState::Recovered;
            session.updated_at = chrono::Utc::now().to_rfc3339();

            let _ = self.store.generate_markdown_note(&session, &transcript_text);
            let _ = self.store.save_session(&session);

            recovered_list.push(session);
        }

        Ok(recovered_list)
    }
}
