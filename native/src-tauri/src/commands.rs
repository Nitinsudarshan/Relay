use crate::sync::MutexExt;
use crate::capture::{AudioRecorder, SttEngine};
use crate::hotkeys;
use crate::pipeline::{PipelineEngine, ProcessedPipelineResult};
use crate::providers::{LLMClient, OllamaStatus, ProviderType};
use crate::settings::{AppSettings, HotkeySettings, PillPosition};
use crate::triggers::{TriggerConfig, TriggerEngine};
use crate::vault::{
    GraphFilter, KanbanCard, KnowledgeGraphData, KnowledgeSearchResult,
    Scribble, ScribbleRelationship, TrashItem, VaultFile, VaultManager, VaultNote,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

/// Broadcast to every window whenever the shared microphone session starts
/// or stops, so any surface (main window, floating pill, indicator) can
/// reflect the true backend state instead of guessing from its own clicks.
pub const CAPTURE_STATE_EVENT: &str = "capture-state-changed";

#[derive(Debug, Clone, Serialize)]
pub struct CaptureStatus {
    pub active: bool,
    pub mode: Option<String>,
    pub status: String,
    pub message: Option<String>,
}

pub fn emit_capture_state(app: &AppHandle, recorder: &AudioRecorder) {
    let mode = recorder.active_mode();
    let active = recorder.is_active();
    let status = if active { "LISTENING" } else { "IDLE" };
    let payload = CaptureStatus {
        active,
        mode,
        status: status.to_string(),
        message: None,
    };
    let _ = app.emit(CAPTURE_STATE_EVENT, payload);
}

pub fn emit_capture_status_event(
    app: &AppHandle,
    active: bool,
    mode: Option<String>,
    status: &str,
    message: Option<String>,
) {
    let payload = CaptureStatus {
        active,
        mode,
        status: status.to_string(),
        message,
    };
    let _ = app.emit(CAPTURE_STATE_EVENT, payload);
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl CommandError {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.to_string(),
            message: message.to_string(),
        }
    }
}

pub const STT_DIAGNOSTICS_EVENT: &str = "stt-diagnostics-updated";

/// Tauri's resource directory, when the build has one.
///
/// Used only to look for a bundled Piper. Nothing bundles one today; the
/// lookup exists so that packaging one later is a packaging change rather
/// than a code change (`tts::discovery`).
fn resource_dir(app: &AppHandle) -> Option<PathBuf> {
    use tauri::Manager;
    app.path().resource_dir().ok()
}

pub struct AppState {
    pub recorder: AudioRecorder,
    pub vault: VaultManager,
    /// The process-relative vault path Relay used before Vault Directory
    /// Location was configurable — the "Use Default Relay Vault" choice in
    /// first-time setup, and the fallback whenever nothing is configured.
    pub default_vault_dir: PathBuf,
    pub config_dir: PathBuf,
    pub settings: Mutex<AppSettings>,
    pub stt: SttEngine,
    pub last_stt_diagnostics: Mutex<Option<crate::capture::SttDiagnosticSnapshot>>,
    pub meetings_v2: Arc<crate::meetings_v2::MeetingsV2Engine>,
    /// Derived meeting intelligence. Shares the recorder's session directory but
    /// only reads from it — everything it produces goes to `processing.json`.
    pub meeting_processor: Arc<crate::meetings_v2::MeetingProcessor>,
    /// The conversational surface over everything above. Owns its own
    /// microphone stream and its own ephemeral session; owns no storage.
    pub talkback: Arc<crate::talkback::TalkbackEngine>,
    /// Where Relay keeps its local voice installation. Resolved once at
    /// startup from the OS application-data directory, because
    /// `config_dir` is process-relative and a packaged Windows app cannot
    /// rely on it (see `tts::discovery::default_tts_root`).
    pub tts_root: PathBuf,
    /// Set while a voice install is running; cleared when it ends.
    /// Cancellation flips it, which every download and every stage polls.
    pub voice_install: Arc<VoiceInstall>,
    /// The loopback listener the Relay browser extension posts captures to.
    /// `None` whenever capture is switched off, which is the default.
    pub capture_bridge: Mutex<Option<crate::capture::web::bridge::BridgeHandle>>,
    pub memory_store: Arc<crate::memory::MemoryStore>,
    pub relationship_store: Arc<crate::relationships::RelationshipStore>,
}

/// The state of an in-flight voice setup.
///
/// One at a time: two concurrent installs would race over the same
/// staging root and the same destination files.
#[derive(Default)]
pub struct VoiceInstall {
    running: std::sync::atomic::AtomicBool,
    cancelled: std::sync::atomic::AtomicBool,
}

impl VoiceInstall {
    /// Claims the install slot, or reports that one is already running.
    fn begin(&self) -> bool {
        use std::sync::atomic::Ordering;
        if self.running.swap(true, Ordering::SeqCst) {
            return false;
        }
        self.cancelled.store(false, Ordering::SeqCst);
        true
    }

    fn end(&self) {
        use std::sync::atomic::Ordering;
        self.running.store(false, Ordering::SeqCst);
        self.cancelled.store(false, Ordering::SeqCst);
    }

    pub fn cancel(&self) {
        use std::sync::atomic::Ordering;
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl AppState {
    fn settings_path(&self) -> PathBuf {
        self.config_dir.join("settings.json")
    }
}

pub fn record_stt_diagnostics(
    app: &AppHandle,
    state: &AppState,
    snapshot: crate::capture::SttDiagnosticSnapshot,
) {
    let mut guard = state.last_stt_diagnostics.lock_or_recover();
    *guard = Some(snapshot.clone());
    let _ = app.emit(STT_DIAGNOSTICS_EVENT, &snapshot);
}

#[tauri::command]
pub async fn get_last_stt_diagnostics(
    state: State<'_, AppState>,
) -> Result<Option<crate::capture::SttDiagnosticSnapshot>, CommandError> {
    let guard = state.last_stt_diagnostics.lock_or_recover();
    Ok(guard.clone())
}

#[tauri::command]
pub async fn get_capture_status(state: State<'_, AppState>) -> Result<CaptureStatus, CommandError> {
    let active = state.recorder.is_active();
    let mode = state.recorder.active_mode();
    let status = if active { "LISTENING" } else { "IDLE" };
    Ok(CaptureStatus {
        active,
        mode,
        status: status.to_string(),
        message: None,
    })
}

#[tauri::command]
pub async fn update_hotkeys(
    app: AppHandle,
    hotkeys: HotkeySettings,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    hotkeys::apply_hotkeys(
        &app,
        &hotkeys.show_hide_hotkey,
        &hotkeys.dictation_hotkey,
        &hotkeys.capture_hotkey,
    )
    .map_err(|e| CommandError::new("HOTKEY_REGISTER_FAILED", &e))?;

    let mut settings = state.settings.lock_or_recover();
    settings.hotkeys = hotkeys;
    settings
        .save(&state.settings_path())
        .map_err(|e| CommandError::new("CONFIG_SAVE_FAILED", &e.to_string()))?;
    let updated = settings.clone();
    drop(settings);

    let _ = app.emit("settings-changed", &updated);
    Ok(())
}

/// Where the floating pill anchors on screen. Re-anchors immediately using
/// a freshly computed monitor/work-area, at whatever size (resting or
/// expanded) it currently is.
#[tauri::command]
pub async fn set_pill_position(
    app: AppHandle,
    position: PillPosition,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    let mut settings = state.settings.lock_or_recover();
    settings.ui.pill_position = position;
    settings
        .save(&state.settings_path())
        .map_err(|e| CommandError::new("CONFIG_SAVE_FAILED", &e.to_string()))?;
    drop(settings);

    crate::overlay::reposition_pill(&app, position);
    let _ = app.emit("pill-position-changed", position);
    Ok(())
}

/// The pill's own RESTING/EXPANDED presentation state is frontend-owned
/// (it's driven by hover and by the capture phase, not by the shared
/// capture session truth), but window geometry can only be changed from
/// Rust — this is the actuator the frontend calls whenever that state
/// flips, so the native window always tightly matches what's actually
/// visible (never a bigger invisible hit-region than the pill itself).
#[tauri::command]
pub async fn set_pill_expanded(
    app: AppHandle,
    expanded: bool,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    let position = state.settings.lock_or_recover().ui.pill_position;
    crate::overlay::set_expanded(&app, expanded, position);
    Ok(())
}

#[tauri::command]
pub async fn set_pill_window_mode(
    app: AppHandle,
    mode: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    let position = state.settings.lock_or_recover().ui.pill_position;
    crate::overlay::set_pill_window_geometry(&app, &mode, position);
    Ok(())
}

#[tauri::command]
pub async fn start_capture(
    app: AppHandle,
    mode: String,
    state: State<'_, AppState>,
) -> Result<String, CommandError> {
    if mode.is_empty() {
        return Err(CommandError::new(
            "INVALID_INPUT",
            "Capture mode cannot be empty",
        ));
    }

    // The universal dictation hotkey (Ctrl+Space, held) and the in-app
    // Click-to-dictate button share one `AudioRecorder` — the microphone can
    // only feed one session at a time. Surface this as a distinct,
    // actionable error instead of the generic "already active" message so
    // the UI can explain *why* rather than looking broken.
    if state.recorder.active_mode().as_deref() == Some("dictation") {
        return Err(CommandError::new(
            "DICTATION_HOTKEY_ACTIVE",
            "Universal dictation is currently recording (hotkey held down). Release it first.",
        ));
    }

    let audio_dir = state.config_dir.join("audio");
    let result = state
        .recorder
        .start(&mode, &audio_dir, Some(app.clone()))
        .map_err(|e| CommandError::new("CAPTURE_FAILED", &e.to_string()));
    emit_capture_state(&app, &state.recorder);

    if result.is_ok() {
        // Kick these off now, in parallel with the user talking, rather
        // than waiting until they stop and need them immediately — by the
        // time transcription runs, a local Ollama that needed starting (or
        // a default Whisper model that needed downloading) has had real
        // time to come up.
        let provider = state.settings.lock_or_recover().provider.clone();
        if matches!(provider.active_provider, ProviderType::Ollama) {
            tauri::async_runtime::spawn(async move {
                crate::providers::ensure_ollama_ready(&provider.ollama_host, &provider.ollama_model).await;
            });
        }

        let has_model = state
            .settings
            .lock_or_recover()
            .stt
            .whisper_model_path
            .as_ref()
            .is_some_and(|p| !p.trim().is_empty());
        if !has_model {
            let models_dir = state.config_dir.join("models");
            tauri::async_runtime::spawn(async move {
                let _ = crate::capture::stt::ensure_default_model(&models_dir).await;
            });
        }
    }

    result
}

/// Broadcast whenever a capture session (from any surface — the in-app
/// pill, the floating overlay pill, or the universal dictation hotkey)
/// finishes processing, so every window can refresh its own view of the
/// vault/Kanban board without polling.
pub const CAPTURE_PROCESSED_EVENT: &str = "capture-processed";

/// Broadcast whenever a Voice Note is persisted to the vault, so the Voice
/// Note page can prepend it to Transcript History and refresh its stats
/// without polling or requiring a restart.
pub const VOICE_NOTE_SAVED_EVENT: &str = "voice-note-saved";

/// Persists `transcript` as a Voice Note and notifies every window. This is
/// the single funnel both the global dictation hotkey
/// (`hotkeys::stop_dictation_session`) and click-to-talk
/// (`process_captured_audio`) route every successful, non-empty transcript
/// through — regardless of whether OS text injection also happens for it —
/// so one recording can never produce more than one Voice Note. Callers
/// must already have guarded against an empty/whitespace-only transcript.
/// Failure to write is logged, not surfaced — a Voice Note write failure
/// must never interrupt dictation/injection, which have already succeeded
/// by the time this is called.
pub fn save_voice_note(app: &AppHandle, vault: &VaultManager, transcript: &str) {
    let note = VaultNote::new_voice_note(transcript);
    match vault.save_note(&note) {
        Ok(_) => {
            let _ = app.emit(VOICE_NOTE_SAVED_EVENT, &note);
        }
        Err(e) => {
            tracing::error!("Failed to save voice note: {}", e);
        }
    }
}

/// Stops the active capture session and, only if the microphone actually
/// picked up audio (see [`crate::capture::CapturedAudio::had_audio`]),
/// transcribes and processes it. Returns `Ok(None)` — never a fabricated
/// `ProcessedPipelineResult` — when nothing was captured, so callers can
/// tell "recorded silence" apart from "recorded and processed speech"
/// instead of assuming every stop produced a result.
#[tauri::command]
pub async fn stop_capture(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<ProcessedPipelineResult>, CommandError> {
    let captured = state
        .recorder
        .stop()
        .await
        .map_err(|e| CommandError::new("CAPTURE_STOP_FAILED", &e.to_string()));
    emit_capture_state(&app, &state.recorder);
    let captured = captured?;

    if !captured.had_audio {
        tracing::info!("[Dictation] Recording stopped with no audio input");
        emit_capture_status_event(&app, false, Some(captured.mode), "NO_SPEECH", None);
        return Ok(None);
    }

    emit_capture_status_event(&app, false, Some(captured.mode.clone()), "TRANSCRIBING", None);
    let result = process_captured_audio(&app, &state, captured).await;
    match &result {
        Ok(Some(processed)) => {
            let _ = app.emit(CAPTURE_PROCESSED_EVENT, processed);
        }
        Ok(None) => {
            // had_audio was true (real, sustained energy was captured) but
            // Whisper still produced no usable text — most commonly a short
            // hallucination (e.g. "Hello.") on a marginal recording that
            // whisper.cpp's own confidence/no-speech heuristics rejected
            // internally, leaving an empty transcript. Must not run
            // trigger-matching or the note/kanban pipeline on nothing.
            tracing::info!("[Dictation] Transcription produced no usable text");
            emit_capture_status_event(&app, false, None, "NO_SPEECH", None);
        }
        Err(_) => {}
    }
    result
}

async fn process_captured_audio(
    app: &AppHandle,
    state: &State<'_, AppState>,
    captured: crate::capture::CapturedAudio,
) -> Result<Option<ProcessedPipelineResult>, CommandError> {
    let settings = state.settings.lock_or_recover().clone();
    let stt = state.stt.clone();
    let samples = captured.samples.clone();

    // Use the shared Capture STT Profile (Fast / ggml-base.bin vs Accurate / ggml-small.bin)
    let models_dir = state.config_dir.join("models");
    let model_path = crate::capture::stt::resolve_dictation_model_path(&models_dir, &settings.stt).await;

    let language_config = crate::capture::SttLanguageConfig::from_settings(&settings.language);
    let mut decoding_config = crate::capture::stt::WhisperDecodingConfig::for_dictation(&settings.stt);
    if let Some(prompt) = settings.build_stt_prompt() {
        decoding_config.initial_prompt = Some(prompt);
    }

    let mp_clone = model_path.clone();
    let lang_clone = language_config.clone();
    let dec_clone = decoding_config.clone();

    let (transcript, diag, err) = tokio::task::spawn_blocking(move || {
        match stt.transcribe_with_config(
            mp_clone.as_deref(),
            &samples,
            &lang_clone,
            &dec_clone,
        ) {
            Ok((t, d)) => (t, Some(d), None),
            Err(e) => (String::new(), None, Some(e.to_string())),
        }
    })
    .await
    .map_err(|e| CommandError::new("STT_TASK_FAILED", &e.to_string()))?;

    let model_str = model_path.as_deref().unwrap_or(crate::capture::stt::DEFAULT_MODEL_FILENAME);
    let snapshot = crate::capture::build_diagnostic_snapshot(
        &captured.mode,
        Some(captured.audio_path.clone()),
        &captured,
        &settings.language,
        &language_config,
        &decoding_config,
        model_str,
        &transcript,
        diag.as_ref(),
        err.clone(),
    );
    record_stt_diagnostics(app, state, snapshot);

    if let Some(err_msg) = err {
        return Err(CommandError::new("STT_FAILED", &err_msg));
    }

    // had_audio only proves the mic measured sustained energy — Whisper can
    // still land on nothing (most commonly a short hallucination that its
    // own internal confidence/no-speech heuristics then reject, leaving an
    // empty result) on a marginal recording. An empty transcript must never
    // reach the note/kanban pipeline.
    if transcript.trim().is_empty() {
        return Ok(None);
    }

    // Expand snippets if trigger words were dictated
    let expanded = settings.expand_snippets(&transcript);
    let transcript = if !expanded.trim().is_empty() { expanded } else { transcript };

    // Every successful, non-empty transcript becomes a Voice Note — this
    // must not depend on which mode-specific pipeline runs next, or on
    // whether it succeeds. Talkback does not come through here at all: it
    // owns its own microphone stream and only persists a Voice Note when
    // the user explicitly asks for one.
    save_voice_note(app, &state.vault, &transcript);

    match captured.mode.as_str() {
        "voice_note" => Ok(Some(ProcessedPipelineResult {
            mode: "voice_note".to_string(),
            transcript: transcript.clone(),
            note_id: None,
            kanban_cards_created: 0,
            output_markdown: transcript,
            sources: Vec::new(),
            spoken_audio_base64: None,
        })),
        "scribble" => {
            let llm = LLMClient::new(settings.provider.clone());
            PipelineEngine::process_scribble(&llm, &state.vault, &transcript)
                .await
                .map(Some)
                .map_err(|e| CommandError::new("PIPELINE_ERROR", &e.to_string()))
        }
        _ => Ok(Some(ProcessedPipelineResult {
            mode: captured.mode.clone(),
            transcript: transcript.clone(),
            note_id: None,
            kanban_cards_created: 0,
            output_markdown: transcript,
            sources: Vec::new(),
            spoken_audio_base64: None,
        })),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDeviceInfo {
    pub name: String,
    pub is_default: bool,
}


#[tauri::command]
pub async fn get_audio_devices() -> Result<Vec<AudioDeviceInfo>, CommandError> {
    use cpal::traits::{DeviceTrait, HostTrait};
    let host = cpal::default_host();
    let default_device_name = host.default_input_device().and_then(|d| d.name().ok());

    let mut devices = Vec::new();
    if let Ok(input_devices) = host.input_devices() {
        for device in input_devices {
            if let Ok(name) = device.name() {
                let is_default = default_device_name.as_deref() == Some(&name);
                devices.push(AudioDeviceInfo {
                    name,
                    is_default,
                });
            }
        }
    }
    Ok(devices)
}

#[tauri::command]
pub async fn get_kanban_cards(state: State<'_, AppState>) -> Result<Vec<KanbanCard>, CommandError> {
    state
        .vault
        .list_kanban_cards()
        .map_err(|e| CommandError::new("VAULT_READ_FAILED", &e.to_string()))
}

/// All Voice Notes in the vault, newest first — the Transcript History the
/// Voice Note page renders and computes its stats from.
#[tauri::command]
pub async fn get_voice_notes(state: State<'_, AppState>) -> Result<Vec<VaultNote>, CommandError> {
    state
        .vault
        .list_notes_by_type(crate::vault::VOICE_NOTE_TYPE)
        .map_err(|e| CommandError::new("VAULT_READ_FAILED", &e.to_string()))
}

#[tauri::command]
pub async fn update_voice_note(
    id: String,
    content: String,
    state: State<'_, AppState>,
) -> Result<VaultNote, CommandError> {
    state
        .vault
        .update_note_content(&id, &content)
        .map_err(|e| CommandError::new("VAULT_UPDATE_FAILED", &e.to_string()))
}

#[tauri::command]
pub async fn delete_voice_note(
    id: String,
    state: State<'_, AppState>,
) -> Result<TrashItem, CommandError> {
    state
        .vault
        .move_to_trash("voice_note", &id)
        .map_err(|e| CommandError::new("VAULT_DELETE_FAILED", &e.to_string()))
}

#[tauri::command]
pub async fn delete_voice_notes(
    ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<usize, CommandError> {
    let mut count = 0;
    for id in ids {
        if state.vault.move_to_trash("voice_note", &id).is_ok() {
            count += 1;
        }
    }
    Ok(count)
}


#[tauri::command]
pub async fn merge_voice_notes(
    app: AppHandle,
    primary_id: String,
    secondary_id: String,
    state: State<'_, AppState>,
) -> Result<VaultNote, CommandError> {
    let merged = state
        .vault
        .merge_notes(&primary_id, &secondary_id)
        .map_err(|e| CommandError::new("VAULT_MERGE_FAILED", &e.to_string()))?;

    // Synchronize and re-enrich any existing Scribbles derived from the merged Voice Notes
    if let Ok(affected_scribble_ids) = state.vault.sync_scribbles_for_voice_note_merge(&primary_id, &secondary_id) {
        for scribble_id in affected_scribble_ids {
            if let Ok(scribble) = state.vault.get_scribble(&scribble_id) {
                let _ = app.emit(SCRIBBLE_SAVED_EVENT, &scribble);
                spawn_scribble_enrichment(app.clone(), &state, scribble_id);
            }
        }
    }

    Ok(merged)
}

#[derive(Serialize, Deserialize)]
pub struct UnmergeVoiceNotesResponse {
    pub primary: VaultNote,
    pub secondary: VaultNote,
}

#[tauri::command]
pub async fn unmerge_voice_note(
    id: String,
    state: State<'_, AppState>,
) -> Result<UnmergeVoiceNotesResponse, CommandError> {
    let result = state
        .vault
        .unmerge_notes(&id)
        .map_err(|e| CommandError::new("VAULT_UNMERGE_FAILED", &e.to_string()))?;

    Ok(UnmergeVoiceNotesResponse {
        primary: result.primary,
        secondary: result.secondary,
    })
}

pub const SCRIBBLE_SAVED_EVENT: &str = "scribble-saved";
pub const SCRIBBLE_ENRICHED_EVENT: &str = "scribble-enriched";

pub fn spawn_scribble_enrichment(
    app: AppHandle,
    state: &AppState,
    scribble_id: String,
) {
    let settings = state.settings.lock_or_recover().clone();
    let llm = LLMClient::new(settings.provider);
    let vault_dir = state.vault.vault_dir();

    tauri::async_runtime::spawn(async move {
        let vault = VaultManager::new(vault_dir);
        match crate::pipeline::enrich_scribble(&llm, &vault, &scribble_id).await {
            Ok(enriched) => {
                let _ = app.emit(SCRIBBLE_ENRICHED_EVENT, &enriched);
            }
            Err(e) => {
                tracing::warn!("Async scribble enrichment failed for {}: {}", scribble_id, e);
            }
        }
    });
}

#[tauri::command]
pub async fn get_scribbles(state: State<'_, AppState>) -> Result<Vec<Scribble>, CommandError> {
    state
        .vault
        .list_scribbles()
        .map_err(|e| CommandError::new("VAULT_READ_FAILED", &e.to_string()))
}

#[tauri::command]
pub async fn get_scribble(id: String, state: State<'_, AppState>) -> Result<Scribble, CommandError> {
    state
        .vault
        .get_scribble(&id)
        .map_err(|e| CommandError::new("VAULT_READ_FAILED", &e.to_string()))
}

// A Tauri command's parameters are the IPC payload's fields — they are flat by
// construction, and grouping them would change the frontend-facing contract.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn create_scribble(
    app: AppHandle,
    content: String,
    title: Option<String>,
    source_type: Option<String>,
    source_metadata: Option<serde_json::Value>,
    tags: Option<Vec<String>>,
    topics: Option<Vec<String>>,
    state: State<'_, AppState>,
) -> Result<Scribble, CommandError> {
    let mut scribble = Scribble::new_text(&content, title.as_deref());
    if let Some(st) = source_type {
        scribble.source_type = st;
    }
    if let Some(sm) = source_metadata {
        scribble.source_metadata = sm;
    }
    if let Some(tg) = tags {
        scribble.tags = tg;
    }
    if let Some(tp) = topics {
        scribble.topics = tp;
    }

    state
        .vault
        .save_scribble(&scribble)
        .map_err(|e| CommandError::new("VAULT_SAVE_FAILED", &e.to_string()))?;

    let _ = app.emit(SCRIBBLE_SAVED_EVENT, &scribble);
    spawn_scribble_enrichment(app, &state, scribble.id.clone());

    Ok(scribble)
}

#[tauri::command]
pub async fn promote_voice_note_to_scribble(
    app: AppHandle,
    voice_note_id: String,
    custom_title: Option<String>,
    state: State<'_, AppState>,
) -> Result<Scribble, CommandError> {
    let voice_note = state
        .vault
        .get_note(&voice_note_id)
        .map_err(|e| CommandError::new("NOTE_NOT_FOUND", &e.to_string()))?;

    let scribble = Scribble::from_voice_note(&voice_note.id, &voice_note.content, custom_title.as_deref());
    state
        .vault
        .save_scribble(&scribble)
        .map_err(|e| CommandError::new("VAULT_SAVE_FAILED", &e.to_string()))?;

    let _ = app.emit(SCRIBBLE_SAVED_EVENT, &scribble);
    spawn_scribble_enrichment(app, &state, scribble.id.clone());

    Ok(scribble)
}

#[tauri::command]
pub async fn create_file_scribble(
    app: AppHandle,
    filename: String,
    content: String,
    mime_type: Option<String>,
    size_bytes: Option<u64>,
    state: State<'_, AppState>,
) -> Result<Scribble, CommandError> {
    let scribble = Scribble::from_file(&filename, &content, mime_type.as_deref(), size_bytes);
    state
        .vault
        .save_scribble(&scribble)
        .map_err(|e| CommandError::new("VAULT_SAVE_FAILED", &e.to_string()))?;

    let _ = app.emit(SCRIBBLE_SAVED_EVENT, &scribble);
    spawn_scribble_enrichment(app, &state, scribble.id.clone());

    Ok(scribble)
}

#[tauri::command]
pub async fn update_scribble(
    app: AppHandle,
    scribble: Scribble,
    state: State<'_, AppState>,
) -> Result<Scribble, CommandError> {
    let updated = state
        .vault
        .update_scribble(&scribble)
        .map_err(|e| CommandError::new("VAULT_UPDATE_FAILED", &e.to_string()))?;

    let _ = app.emit(SCRIBBLE_SAVED_EVENT, &updated);
    Ok(updated)
}

#[tauri::command]
pub async fn delete_scribble(
    id: String,
    state: State<'_, AppState>,
) -> Result<TrashItem, CommandError> {
    state
        .vault
        .move_to_trash("scribble", &id)
        .map_err(|e| CommandError::new("VAULT_DELETE_FAILED", &e.to_string()))
}

#[tauri::command]
pub async fn merge_scribbles(
    app: AppHandle,
    source_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<Scribble, CommandError> {
    let merged = state
        .vault
        .merge_scribbles(&source_ids)
        .map_err(|e| CommandError::new("VAULT_MERGE_FAILED", &e.to_string()))?;

    let _ = app.emit(SCRIBBLE_SAVED_EVENT, &merged);
    spawn_scribble_enrichment(app, &state, merged.id.clone());

    Ok(merged)
}

#[tauri::command]
pub async fn import_vault_file(
    source_path: String,
    state: State<'_, AppState>,
) -> Result<VaultFile, CommandError> {
    let path = std::path::Path::new(&source_path);
    state
        .vault
        .import_vault_file(path)
        .map_err(|e| CommandError::new("IMPORT_FAILED", &e.to_string()))
}

#[tauri::command]
pub async fn import_vault_file_bytes(
    filename: String,
    bytes: Vec<u8>,
    source_path: Option<String>,
    state: State<'_, AppState>,
) -> Result<VaultFile, CommandError> {
    state
        .vault
        .import_vault_file_bytes(&filename, &bytes, source_path.as_deref())
        .map_err(|e| CommandError::new("IMPORT_FAILED", &e.to_string()))
}

#[tauri::command]
pub async fn get_vault_files(
    state: State<'_, AppState>,
) -> Result<Vec<VaultFile>, CommandError> {
    state
        .vault
        .list_vault_files()
        .map_err(|e| CommandError::new("LIST_FILES_FAILED", &e.to_string()))
}

#[tauri::command]
pub async fn get_vault_file(
    id: String,
    state: State<'_, AppState>,
) -> Result<VaultFile, CommandError> {
    state
        .vault
        .get_vault_file(&id)
        .map_err(|e| CommandError::new("FILE_NOT_FOUND", &e.to_string()))
}

#[tauri::command]
pub async fn analyze_vault_file(
    id: String,
    state: State<'_, AppState>,
) -> Result<VaultFile, CommandError> {
    let _ = state.vault.reprocess_vault_file(&id);
    let settings = state.settings.lock_or_recover().clone();
    let llm = LLMClient::new(settings.provider);
    crate::pipeline::enrich_vault_file(&llm, &state.vault, &id)
        .await
        .map_err(|e| CommandError::new("ANALYZE_FAILED", &e))
}

#[tauri::command]
pub async fn summarize_vault_file(
    id: String,
    state: State<'_, AppState>,
) -> Result<VaultFile, CommandError> {
    let _ = state.vault.reprocess_vault_file(&id);
    let settings = state.settings.lock_or_recover().clone();
    let llm = LLMClient::new(settings.provider);
    crate::pipeline::summarize_vault_file(&llm, &state.vault, &id)
        .await
        .map_err(|e| CommandError::new("SUMMARIZE_FAILED", &e))
}

#[tauri::command]
pub async fn enrich_vault_file(
    id: String,
    state: State<'_, AppState>,
) -> Result<VaultFile, CommandError> {
    analyze_vault_file(id, state).await
}

#[tauri::command]
pub async fn update_vault_file_tags(
    id: String,
    tags: Vec<String>,
    topics: Vec<String>,
    entities: Vec<String>,
    state: State<'_, AppState>,
) -> Result<VaultFile, CommandError> {
    let mut file = state
        .vault
        .get_vault_file(&id)
        .map_err(|e| CommandError::new("FILE_NOT_FOUND", &e.to_string()))?;

    file.tags = tags;
    file.topics = topics;
    file.entities = entities;
    file.updated_at = chrono::Utc::now().to_rfc3339();

    state
        .vault
        .save_vault_file(&file)
        .map_err(|e| CommandError::new("SAVE_FAILED", &e.to_string()))?;

    Ok(file)
}

#[tauri::command]
pub async fn create_scribble_from_vault_file(
    app: AppHandle,
    id: String,
    state: State<'_, AppState>,
) -> Result<Scribble, CommandError> {
    let scribble = state
        .vault
        .create_scribble_from_file(&id)
        .map_err(|e| CommandError::new("CREATE_SCRIBBLE_FAILED", &e.to_string()))?;

    let _ = app.emit(SCRIBBLE_SAVED_EVENT, &scribble);
    spawn_scribble_enrichment(app, &state, scribble.id.clone());

    Ok(scribble)
}

#[tauri::command]
pub async fn reprocess_vault_file(
    id: String,
    state: State<'_, AppState>,
) -> Result<VaultFile, CommandError> {
    analyze_vault_file(id, state).await
}

#[tauri::command]
pub async fn delete_vault_file(
    id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    state
        .vault
        .delete_vault_file(&id)
        .map_err(|e| CommandError::new("DELETE_FAILED", &e.to_string()))
}

#[tauri::command]
pub async fn open_vault_file_location(
    id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    let file = state
        .vault
        .get_vault_file(&id)
        .map_err(|e| CommandError::new("FILE_NOT_FOUND", &e.to_string()))?;

    let full_path = state.vault.vault_dir().join(&file.vault_path);
    let target_dir = if full_path.exists() {
        full_path.parent().unwrap_or(&full_path).to_path_buf()
    } else {
        state.vault.vault_dir().join("files").join(&id).join("original")
    };

    if target_dir.exists() {
        #[cfg(target_os = "windows")]
        {
            let path_buf = std::fs::canonicalize(&target_dir).unwrap_or(target_dir);
            let path_str = path_buf.to_string_lossy().replace('/', "\\");
            let clean_path = path_str.trim_start_matches(r"\\?\");
            let _ = std::process::Command::new("explorer")
                .arg(clean_path)
                .spawn();
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn get_trash_items(state: State<'_, AppState>) -> Result<Vec<TrashItem>, CommandError> {
    state
        .vault
        .get_trash_items()
        .map_err(|e| CommandError::new("TRASH_READ_FAILED", &e.to_string()))
}

#[tauri::command]
pub async fn restore_trash_item(
    trash_id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    state
        .vault
        .restore_trash_item(&trash_id)
        .map_err(|e| CommandError::new("TRASH_RESTORE_FAILED", &e.to_string()))
}

#[tauri::command]
pub async fn delete_trash_item_permanently(
    trash_id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    state
        .vault
        .delete_trash_item_permanently(&trash_id)
        .map_err(|e| CommandError::new("TRASH_DELETE_FAILED", &e.to_string()))
}

#[tauri::command]
pub async fn empty_trash(state: State<'_, AppState>) -> Result<usize, CommandError> {
    state
        .vault
        .empty_trash()
        .map_err(|e| CommandError::new("TRASH_EMPTY_FAILED", &e.to_string()))
}

#[tauri::command]
pub async fn add_scribble_relationship(
    app: AppHandle,
    source_id: String,
    relationship: ScribbleRelationship,
    state: State<'_, AppState>,
) -> Result<Scribble, CommandError> {
    let updated = state
        .vault
        .add_scribble_relationship(&source_id, relationship)
        .map_err(|e| CommandError::new("VAULT_UPDATE_FAILED", &e.to_string()))?;

    let _ = app.emit(SCRIBBLE_SAVED_EVENT, &updated);
    Ok(updated)
}

#[tauri::command]
pub async fn remove_scribble_relationship(
    app: AppHandle,
    source_id: String,
    relationship_id: String,
    state: State<'_, AppState>,
) -> Result<Scribble, CommandError> {
    let updated = state
        .vault
        .remove_scribble_relationship(&source_id, &relationship_id)
        .map_err(|e| CommandError::new("VAULT_UPDATE_FAILED", &e.to_string()))?;

    let _ = app.emit(SCRIBBLE_SAVED_EVENT, &updated);
    Ok(updated)
}

#[tauri::command]
pub async fn search_knowledge(
    query: String,
    state: State<'_, AppState>,
) -> Result<KnowledgeSearchResult, CommandError> {
    state
        .vault
        .search_knowledge(&query)
        .map_err(|e| CommandError::new("VAULT_SEARCH_FAILED", &e.to_string()))
}

#[tauri::command]
pub async fn get_knowledge_graph(
    filter: Option<GraphFilter>,
    state: State<'_, AppState>,
) -> Result<KnowledgeGraphData, CommandError> {
    state
        .vault
        .get_knowledge_graph(filter.as_ref())
        .map_err(|e| CommandError::new("GRAPH_BUILD_FAILED", &e.to_string()))
}

#[tauri::command]
pub async fn trigger_enrich_scribble(
    app: AppHandle,
    id: String,
    state: State<'_, AppState>,
) -> Result<Scribble, CommandError> {
    let settings = state.settings.lock_or_recover().clone();
    let llm = LLMClient::new(settings.provider);

    let enriched = crate::pipeline::enrich_scribble(&llm, &state.vault, &id)
        .await
        .map_err(|e| CommandError::new("ENRICH_FAILED", &e))?;

    let _ = app.emit(SCRIBBLE_ENRICHED_EVENT, &enriched);
    let _ = app.emit(SCRIBBLE_SAVED_EVENT, &enriched);
    Ok(enriched)
}

#[tauri::command]
pub async fn summarize_scribble(
    app: AppHandle,
    id: String,
    state: State<'_, AppState>,
) -> Result<Scribble, CommandError> {
    let settings = state.settings.lock_or_recover().clone();
    let llm = LLMClient::new(settings.provider);

    let updated = crate::pipeline::summarize_scribble(&llm, &state.vault, &id)
        .await
        .map_err(|e| CommandError::new("SUMMARIZE_FAILED", &e))?;

    let _ = app.emit(SCRIBBLE_SAVED_EVENT, &updated);
    Ok(updated)
}

#[derive(Debug, Clone, Serialize)]
pub struct VaultLocationInfo {
    /// Absolute path currently in use, whether from an explicit user choice
    /// or the process-relative default.
    pub path: String,
    /// The process-relative default path — what "Use Default Relay Vault"
    /// would set `path` to.
    pub default_path: String,
    /// Whether the user has explicitly chosen/confirmed a location (Voice
    /// Note first-time setup, or Settings) — distinct from "currently using
    /// the unconfirmed default".
    pub configured: bool,
    /// Whether `path` currently exists (or can be created) and is usable.
    pub accessible: bool,
}

/// Reports where Relay's vault currently lives, so the Voice Note page can
/// decide whether to show first-time setup, a "can't access your folder"
/// recovery state, or the normal history view.
#[tauri::command]
pub async fn get_vault_location(
    state: State<'_, AppState>,
) -> Result<VaultLocationInfo, CommandError> {
    let configured = state.settings.lock_or_recover().vault.directory.is_some();
    let path = state.vault.vault_dir();
    // Reuses `VaultManager::init` (already called by every read/write path)
    // as the accessibility probe, rather than duplicating filesystem-
    // permission-checking logic.
    let accessible = state.vault.init().is_ok();
    Ok(VaultLocationInfo {
        path: path.to_string_lossy().to_string(),
        default_path: state.default_vault_dir.to_string_lossy().to_string(),
        configured,
        accessible,
    })
}

/// Opens the native OS folder picker and returns the chosen path, or `None`
/// if the user cancelled. Runs on a blocking task since the dialog blocks
/// its calling thread until the user responds.
#[tauri::command]
pub async fn choose_vault_folder(app: AppHandle) -> Result<Option<String>, CommandError> {
    let selected =
        tauri::async_runtime::spawn_blocking(move || app.dialog().file().blocking_pick_folder())
            .await
            .map_err(|e| CommandError::new("DIALOG_TASK_FAILED", &e.to_string()))?;

    match selected {
        Some(file_path) => file_path
            .into_path()
            .map(|p| Some(p.to_string_lossy().to_string()))
            .map_err(|e| CommandError::new("DIALOG_PATH_INVALID", &e.to_string())),
        None => Ok(None),
    }
}

/// Validates `path`, repoints the live vault at it (no restart needed —
/// future Voice Notes, and any other vault reads/writes, immediately use
/// it), and persists it to the existing Vault Directory Location setting.
/// Never moves, migrates, or deletes whatever is at the old location.
#[tauri::command]
pub async fn set_vault_location(
    path: String,
    state: State<'_, AppState>,
) -> Result<VaultLocationInfo, CommandError> {
    let new_dir = PathBuf::from(&path);
    let probe = VaultManager::new(new_dir.clone());
    probe
        .init()
        .map_err(|e| CommandError::new("VAULT_PATH_INVALID", &e.to_string()))?;

    // Persist before repointing the live vault — if the settings write
    // fails, the running app must keep using the old (still-working)
    // location rather than silently diverging from what's on disk.
    let mut settings = state.settings.lock_or_recover();
    settings.vault.directory = Some(path.clone());
    settings
        .save(&state.settings_path())
        .map_err(|e| CommandError::new("CONFIG_SAVE_FAILED", &e.to_string()))?;
    drop(settings);

    state.vault.set_vault_dir(new_dir);

    Ok(VaultLocationInfo {
        path,
        default_path: state.default_vault_dir.to_string_lossy().to_string(),
        configured: true,
        accessible: true,
    })
}

#[tauri::command]
pub async fn get_triggers(state: State<'_, AppState>) -> Result<Vec<TriggerConfig>, CommandError> {
    let path = state.config_dir.join("triggers.json");
    TriggerEngine::load_triggers(&path)
        .map_err(|e| CommandError::new("CONFIG_READ_FAILED", &e.to_string()))
}

#[tauri::command]
pub async fn save_triggers(
    triggers: Vec<TriggerConfig>,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    let path = state.config_dir.join("triggers.json");
    TriggerEngine::save_triggers(&path, &triggers)
        .map_err(|e| CommandError::new("CONFIG_SAVE_FAILED", &e.to_string()))
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SttModelStatus {
    Ready { path: String },
    Failed { message: String },
}

/// Removes "go find and download a GGML model yourself" as a prerequisite:
/// downloads the default one now if nothing is configured, and reports
/// where things stand so Settings can show real status instead of the
/// user finding out only when a capture silently fails.
#[tauri::command]
pub async fn ensure_stt_model_ready(state: State<'_, AppState>) -> Result<SttModelStatus, CommandError> {
    let models_dir = state.config_dir.join("models");
    let configured = state
        .settings
        .lock_or_recover()
        .stt
        .whisper_model_path
        .clone()
        .filter(|p| !p.trim().is_empty());

    if let Some(ref path_str) = configured {
        let path = std::path::Path::new(path_str);
        if path.exists() {
            if !crate::capture::stt::is_legacy_default_model(path) {
                // User has an explicit custom model that exists
                return Ok(SttModelStatus::Ready {
                    path: path_str.clone(),
                });
            }
            // If it's a legacy default model (e.g. ggml-base.bin), proceed to ensure production ggml-small.bin
        } else {
            return Ok(SttModelStatus::Failed {
                message: format!(
                    "Configured model path not found: '{}'. Expected production model: '{}'.",
                    path_str,
                    crate::capture::stt::DEFAULT_MODEL_FILENAME
                ),
            });
        }
    }

    match crate::capture::stt::ensure_default_model(&models_dir).await {
        Ok(path) => {
            let path_str = path.to_string_lossy().to_string();
            let mut settings = state.settings.lock_or_recover();
            settings.stt.whisper_model_path = Some(path_str.clone());
            let _ = settings.save(&state.settings_path());
            Ok(SttModelStatus::Ready { path: path_str })
        }
        Err(e) => Ok(SttModelStatus::Failed {
            message: e.to_string(),
        }),
    }
}

/// Removes the "install and manually start Ollama" step for local mode:
/// starts it and pulls the configured model if needed. A no-op for the
/// Cloud API path, which is unaffected.
#[tauri::command]
pub async fn ensure_local_llm_ready(state: State<'_, AppState>) -> Result<OllamaStatus, CommandError> {
    let settings = state.settings.lock_or_recover().clone();
    if !matches!(settings.provider.active_provider, ProviderType::Ollama) {
        return Ok(OllamaStatus::Running);
    }
    Ok(crate::providers::ensure_ollama_ready(&settings.provider.ollama_host, &settings.provider.ollama_model).await)
}

#[tauri::command]
pub async fn get_available_llm_models(
    host: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<crate::providers::OllamaModelDetails>, CommandError> {
    let host = host.unwrap_or_else(|| {
        state.settings.lock_or_recover().provider.ollama_host.clone()
    });
    crate::providers::list_installed_models(&host)
        .await
        .map_err(|e| CommandError::new("OLLAMA_QUERY_FAILED", &e))
}

#[tauri::command]
pub async fn test_llm_prompt(
    host: Option<String>,
    model: String,
    prompt: Option<String>,
    state: State<'_, AppState>,
) -> Result<crate::providers::OllamaPromptTestResult, CommandError> {
    let host = host.unwrap_or_else(|| {
        state.settings.lock_or_recover().provider.ollama_host.clone()
    });
    let prompt = prompt.unwrap_or_else(|| "Hello! Reply with 'Relay AI ready' in under 5 words.".to_string());
    Ok(crate::providers::test_ollama_prompt(&host, &model, &prompt).await)
}

#[tauri::command]
pub async fn get_available_stt_models(
    state: State<'_, AppState>,
) -> Result<crate::capture::stt::SttModelsOverview, CommandError> {
    let models_dir = state.config_dir.join("models");
    let stt_settings = state.settings.lock_or_recover().stt.clone();
    Ok(crate::capture::stt::get_stt_models_overview(&models_dir, &stt_settings))
}

#[tauri::command]
pub async fn test_stt_model(
    model_path: String,
) -> Result<crate::capture::stt::SttModelTestResult, CommandError> {
    Ok(crate::capture::stt::test_stt_model_file(&model_path))
}

#[tauri::command]
pub async fn copy_to_clipboard(text: String) -> Result<(), CommandError> {
    crate::hotkeys::injection::copy_to_clipboard(&text)
        .map_err(|e| CommandError::new("CLIPBOARD_COPY_FAILED", &e.to_string()))
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, CommandError> {
    Ok(state.settings.lock_or_recover().clone())
}

#[tauri::command]
pub async fn save_settings(
    app: AppHandle,
    settings: AppSettings,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    // Capture is configured through its own commands, never through this one.
    // Carrying the stored section over means a settings object that predates
    // it — or one from a frontend that never read it — cannot switch capture
    // off and throw away the pairing token as a side effect.
    let stored_capture = state.settings.lock_or_recover().capture.clone();
    let settings = settings.preserving_capture(&stored_capture);

    settings
        .save(&state.settings_path())
        .map_err(|e| CommandError::new("CONFIG_SAVE_FAILED", &e.to_string()))?;
    state
        .recorder
        .set_keep_warm_duration(settings.audio_input.parse_keep_warm_duration());
    *state.settings.lock_or_recover() = settings.clone();

    // Re-register hotkeys dynamically with the OS immediately
    let _ = hotkeys::apply_hotkeys(
        &app,
        &settings.hotkeys.show_hide_hotkey,
        &settings.hotkeys.dictation_hotkey,
        &settings.hotkeys.capture_hotkey,
    );

    // The bridge's lifetime follows the setting: turning capture off closes
    // the socket immediately rather than at the next launch.
    apply_capture_bridge(&app, &state);

    let _ = app.emit("settings-changed", &settings);
    Ok(())
}

#[tauri::command]
pub async fn open_settings_window(
    app: AppHandle,
    section: Option<String>,
) -> Result<(), CommandError> {
    if let Some(window) = app.get_webview_window(crate::hotkeys::MAIN_WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
        let payload = serde_json::json!({
            "tab": "settings",
            "section": section,
        });
        let _ = app.emit("navigate-tab", payload);
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangelogItem {
    pub category: String,
    pub domain: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangelogEntry {
    pub version: String,
    pub date: String,
    pub release_type: String,
    pub title: String,
    pub tags: Vec<String>,
    pub domains: Vec<String>,
    pub items: Vec<ChangelogItem>,
}

#[tauri::command]
pub async fn get_app_version() -> Result<String, CommandError> {
    let version = include_str!("../../../VERSION").trim().to_string();
    Ok(version)
}

#[tauri::command]
pub async fn get_changelog() -> Result<Vec<ChangelogEntry>, CommandError> {
    let raw = include_str!("../../../CHANGELOG.md");
    Ok(parse_changelog_markdown(raw))
}

fn parse_changelog_markdown(md: &str) -> Vec<ChangelogEntry> {
    let mut entries = Vec::new();
    let mut current_entry: Option<ChangelogEntry> = None;
    let mut current_item: Option<ChangelogItem> = None;

    for line in md.lines() {
        let trimmed = line.trim();

        if let Some(rest) = trimmed.strip_prefix("## [") {
            if let Some(mut item) = current_item.take() {
                if let Some(entry) = current_entry.as_mut() {
                    item.text = item.text.trim().to_string();
                    if !item.text.is_empty() {
                        entry.items.push(item);
                    }
                }
            }
            if let Some(entry) = current_entry.take() {
                entries.push(entry);
            }

            let (ver, date) = if let Some(close_idx) = rest.find(']') {
                let v = &rest[..close_idx];
                let d = if let Some(dash_idx) = rest.find(" - ") {
                    rest[dash_idx + 3..].trim()
                } else {
                    ""
                };
                (v, d)
            } else {
                ("", "")
            };

            let release_type = if ver.ends_with(".0.0") || ver == "0.1.0" {
                "major"
            } else if ver.ends_with(".0") {
                "minor"
            } else {
                "patch"
            };

            current_entry = Some(ChangelogEntry {
                version: ver.to_string(),
                date: date.to_string(),
                release_type: release_type.to_string(),
                title: String::new(),
                tags: Vec::new(),
                domains: Vec::new(),
                items: Vec::new(),
            });
            continue;
        }

        if let Some(heading) = trimmed.strip_prefix("### ") {
            if let Some(entry) = current_entry.as_mut() {
                if entry.title.is_empty() {
                    entry.title = heading.trim().to_string();
                }
            }
            continue;
        }

        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            if let Some(mut item) = current_item.take() {
                if let Some(entry) = current_entry.as_mut() {
                    item.text = item.text.trim().to_string();
                    if !item.text.is_empty() {
                        entry.items.push(item);
                    }
                }
            }

            let bullet_content = trimmed[2..].trim();
            let (category, domain, text) = parse_bullet_line(bullet_content);

            if let Some(entry) = current_entry.as_mut() {
                if !category.is_empty() && !entry.tags.contains(&category) {
                    entry.tags.push(category.clone());
                }
                if !domain.is_empty() && !entry.domains.contains(&domain) {
                    entry.domains.push(domain.clone());
                }
            }

            current_item = Some(ChangelogItem {
                category,
                domain,
                text,
            });
            continue;
        }

        if let Some(item) = current_item.as_mut() {
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                item.text.push(' ');
                item.text.push_str(trimmed);
            }
        }
    }

    if let Some(mut item) = current_item.take() {
        if let Some(entry) = current_entry.as_mut() {
            item.text = item.text.trim().to_string();
            if !item.text.is_empty() {
                entry.items.push(item);
            }
        }
    }
    if let Some(entry) = current_entry.take() {
        entries.push(entry);
    }

    entries
}

fn parse_bullet_line(content: &str) -> (String, String, String) {
    if let Some(after_open) = content.strip_prefix("**") {
        if let Some(end_bold) = after_open.find("**") {
            let bold_text = &after_open[..end_bold];
            let after_bold = &after_open[end_bold + 2..];
            let text = after_bold.trim_start_matches(':').trim().to_string();

            if let Some(open_p) = bold_text.find('(') {
                if let Some(close_p) = bold_text.find(')') {
                    let cat = bold_text[..open_p].trim().to_string();
                    let dom_raw = bold_text[open_p + 1..close_p].trim().trim_matches('`');
                    let dom = if dom_raw.contains('/') || dom_raw.contains('\\') {
                        dom_raw.rsplit_once(['/', '\\']).map(|(_, f)| f).unwrap_or(dom_raw)
                    } else {
                        dom_raw
                    };
                    return (cat, dom.to_string(), text);
                }
            }

            return (bold_text.to_string(), "Core".to_string(), text);
        }
    }

    ("General".to_string(), "Relay".to_string(), content.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SttDiagnosticResult {
    pub model_used: String,
    pub auto_transcript: String,
    pub auto_duration_ms: u64,
    pub hindi_locked_transcript: String,
    pub hindi_locked_duration_ms: u64,
    pub english_locked_transcript: String,
    pub english_locked_duration_ms: u64,
}

/// Diagnostic development helper: runs an existing recorded WAV through Auto (None),
/// Hindi-locked ("hi"), and English-locked ("en") STT configurations to compare raw model emissions.
#[tauri::command]
pub async fn diagnose_stt_variants(
    wav_path: String,
    custom_model_path: Option<String>,
    state: State<'_, AppState>,
) -> Result<SttDiagnosticResult, CommandError> {
    let settings = state.settings.lock_or_recover().clone();
    let stt = state.stt.clone();
    let model_path = custom_model_path
        .filter(|p| !p.trim().is_empty())
        .or_else(|| settings.stt.whisper_model_path.clone());

    let samples = tokio::task::spawn_blocking(move || -> Result<Vec<f32>, String> {
        let mut reader = hound::WavReader::open(&wav_path)
            .map_err(|e| format!("Failed to open WAV: {}", e))?;
        let spec = reader.spec();
        let raw_samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Float => reader.samples::<f32>().filter_map(|s| s.ok()).collect(),
            hound::SampleFormat::Int => reader
                .samples::<i16>()
                .filter_map(|s| s.ok())
                .map(|s| s as f32 / i16::MAX as f32)
                .collect(),
        };
        Ok(raw_samples)
    })
    .await
    .map_err(|e| CommandError::new("IO_ERROR", &e.to_string()))?
    .map_err(|e| CommandError::new("WAV_ERROR", &e))?;

    let model_label = model_path
        .as_deref()
        .map(|p| p.split(['/', '\\']).next_back().unwrap_or(p))
        .unwrap_or("default")
        .to_string();

    let stt1 = stt.clone();
    let mp1 = model_path.clone();
    let s1 = samples.clone();
    let (auto_res, auto_duration_ms) = tokio::task::spawn_blocking(move || {
        let start = std::time::Instant::now();
        let cfg = crate::capture::SttLanguageConfig {
            whisper_language: None,
            translate: false,
        };
        let res = stt1.transcribe(mp1.as_deref(), &s1, &cfg).unwrap_or_default();
        (res, start.elapsed().as_millis() as u64)
    })
    .await
    .map_err(|e| CommandError::new("STT_FAILED", &e.to_string()))?;

    let stt2 = stt.clone();
    let mp2 = model_path.clone();
    let s2 = samples.clone();
    let (hi_res, hindi_locked_duration_ms) = tokio::task::spawn_blocking(move || {
        let start = std::time::Instant::now();
        let cfg = crate::capture::SttLanguageConfig {
            whisper_language: Some("hi".to_string()),
            translate: false,
        };
        let res = stt2.transcribe(mp2.as_deref(), &s2, &cfg).unwrap_or_default();
        (res, start.elapsed().as_millis() as u64)
    })
    .await
    .map_err(|e| CommandError::new("STT_FAILED", &e.to_string()))?;

    let stt3 = stt.clone();
    let mp3 = model_path.clone();
    let s3 = samples.clone();
    let (en_res, english_locked_duration_ms) = tokio::task::spawn_blocking(move || {
        let start = std::time::Instant::now();
        let cfg = crate::capture::SttLanguageConfig {
            whisper_language: Some("en".to_string()),
            translate: false,
        };
        let res = stt3.transcribe(mp3.as_deref(), &s3, &cfg).unwrap_or_default();
        (res, start.elapsed().as_millis() as u64)
    })
    .await
    .map_err(|e| CommandError::new("STT_FAILED", &e.to_string()))?;

    Ok(SttDiagnosticResult {
        model_used: model_label,
        auto_transcript: auto_res,
        auto_duration_ms,
        hindi_locked_transcript: hi_res,
        hindi_locked_duration_ms,
        english_locked_transcript: en_res,
        english_locked_duration_ms,
    })
}

/// Evaluates a recorded WAV file against a specific STT decoding configuration variant
/// (e.g. baseline, relay_prompt, best_of_3, beam_2, temperature_fallback) using the Phase 5 harness.
#[tauri::command]
pub async fn run_stt_evaluation(
    wav_path: String,
    variant: String,
    reference_text: Option<String>,
    custom_model_path: Option<String>,
    state: State<'_, AppState>,
) -> Result<crate::capture::EvaluationResult, CommandError> {
    let settings = state.settings.lock_or_recover().clone();
    let stt = state.stt.clone();
    let model_path = custom_model_path
        .filter(|p| !p.trim().is_empty())
        .or_else(|| settings.stt.whisper_model_path.clone());

    let (samples, sample_rate) = tokio::task::spawn_blocking(move || -> Result<(Vec<f32>, u32), String> {
        let mut reader = hound::WavReader::open(&wav_path)
            .map_err(|e| format!("Failed to open WAV: {}", e))?;
        let spec = reader.spec();
        let raw_samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Float => reader.samples::<f32>().filter_map(|s| s.ok()).collect(),
            hound::SampleFormat::Int => reader
                .samples::<i16>()
                .filter_map(|s| s.ok())
                .map(|s| s as f32 / i16::MAX as f32)
                .collect(),
        };
        Ok((raw_samples, spec.sample_rate))
    })
    .await
    .map_err(|e| CommandError::new("IO_ERROR", &e.to_string()))?
    .map_err(|e| CommandError::new("WAV_ERROR", &e))?;

    let eval_variant = match variant.to_lowercase().as_str() {
        "relay_prompt" | "prompt" => crate::capture::EvalConfigVariant::RelayPrompt,
        "best_of_3" | "best_of" => crate::capture::EvalConfigVariant::BestOf3,
        "beam_2" | "beam" => crate::capture::EvalConfigVariant::Beam2,
        "temperature_fallback" | "fallback" => crate::capture::EvalConfigVariant::TemperatureFallback,
        _ => crate::capture::EvalConfigVariant::Baseline,
    };

    let result = tokio::task::spawn_blocking(move || {
        crate::capture::evaluate_audio_buffer(
            "manual_eval",
            "eval_audio.wav",
            &samples,
            sample_rate,
            eval_variant,
            &settings.language,
            model_path.as_deref(),
            reference_text.as_deref(),
            &stt,
        )
    })
    .await
    .map_err(|e| CommandError::new("EVAL_ERROR", &e.to_string()))?;

    Ok(result)
}

/// Retrieves the full 35-item curated evaluation corpus manifest for UI test-bench selection.
#[tauri::command]
pub async fn get_stt_corpus() -> Result<Vec<crate::capture::CorpusItem>, CommandError> {
    Ok(crate::capture::get_curated_corpus())
}

#[tauri::command]
pub async fn get_relay_profile(
    state: State<'_, AppState>,
) -> Result<crate::identity::RelayProfile, CommandError> {
    Ok(crate::identity::load_relay_profile(&state.config_dir))
}

#[tauri::command]
pub async fn update_profile_display_name(
    app: tauri::AppHandle,
    display_name: String,
    state: State<'_, AppState>,
) -> Result<crate::identity::RelayProfile, CommandError> {
    let profile = crate::identity::update_profile_display_name(&state.config_dir, &display_name)
        .map_err(|e| CommandError::new("UPDATE_NAME_FAILED", &e))?;

    let _ = app.emit("profile-changed", &profile);
    Ok(profile)
}

#[tauri::command]
pub async fn complete_profile_onboarding(
    app: tauri::AppHandle,
    display_name: String,
    account_mode: Option<crate::identity::AccountMode>,
    state: State<'_, AppState>,
) -> Result<crate::identity::RelayProfile, CommandError> {
    let profile = crate::identity::complete_profile_onboarding(
        &state.config_dir,
        &display_name,
        account_mode,
    )
    .map_err(|e| CommandError::new("ONBOARDING_FAILED", &e))?;

    // Also mark first_run_completed in settings
    if let Ok(mut settings) = state.settings.lock() {
        settings.diagnostics.first_run_completed = true;
        let _ = settings.save(&state.config_dir.join("settings.json"));
    }

    let _ = app.emit("profile-changed", &profile);
    Ok(profile)
}

#[tauri::command]
pub async fn get_developer_settings(
    state: State<'_, AppState>,
) -> Result<crate::developer::DeveloperSettings, CommandError> {
    Ok(crate::developer::load_developer_settings(&state.config_dir))
}

#[tauri::command]
pub async fn set_developer_force_onboarding(
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<crate::developer::DeveloperSettings, CommandError> {
    crate::developer::set_force_onboarding(&state.config_dir, enabled)
        .map_err(|e| CommandError::new("DEV_SETTINGS_FAILED", &e))
}

#[tauri::command]
pub async fn set_developer_notification_surface_mode(
    mode: crate::developer::NotificationSurfaceMode,
    state: State<'_, AppState>,
) -> Result<crate::developer::DeveloperSettings, CommandError> {
    crate::developer::set_notification_surface_mode(&state.config_dir, mode)
        .map_err(|e| CommandError::new("DEV_SETTINGS_FAILED", &e))
}

#[tauri::command]
pub async fn get_account_state(
    state: State<'_, AppState>,
) -> Result<crate::identity::RelayAccount, CommandError> {
    Ok(crate::identity::load_relay_account(&state.config_dir))
}

#[tauri::command]
pub async fn start_google_sign_in(
    app: tauri::AppHandle,
    custom_client_id: Option<String>,
    custom_client_secret: Option<String>,
    state: State<'_, AppState>,
) -> Result<crate::identity::RelayAccount, CommandError> {
    let settings = state.settings.lock_or_recover().clone();
    let supabase_url = settings.cloud.supabase_url;
    let supabase_anon = settings.cloud.supabase_anon_key;

    let account = crate::identity::sign_in_with_google(
        &state.config_dir,
        custom_client_id,
        custom_client_secret,
        supabase_url,
        supabase_anon,
    )
    .await
    .map_err(|e| CommandError::new("AUTH_FAILED", &e))?;

    let _ = app.emit("account-changed", &account);

    // Report diagnostics event if enabled
    let settings = state.settings.lock_or_recover().clone();
    let inst = crate::identity::get_or_create_installation_info(
        &state.config_dir,
        env!("CARGO_PKG_VERSION"),
    );
    crate::diagnostics::DiagnosticsService::report_event(
        settings.diagnostics.allow_anonymous_diagnostics,
        &inst.installation_id,
        account.user_id.as_deref(),
        env!("CARGO_PKG_VERSION"),
        "account_sign_in",
        std::collections::HashMap::new(),
    );

    Ok(account)
}

#[tauri::command]
pub async fn sign_out_account(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<crate::identity::RelayAccount, CommandError> {
    let account = crate::identity::sign_out_account(&state.config_dir)
        .map_err(|e| CommandError::new("SIGNOUT_FAILED", &e))?;

    let _ = app.emit("account-changed", &account);

    let settings = state.settings.lock_or_recover().clone();
    let inst = crate::identity::get_or_create_installation_info(
        &state.config_dir,
        env!("CARGO_PKG_VERSION"),
    );
    crate::diagnostics::DiagnosticsService::report_event(
        settings.diagnostics.allow_anonymous_diagnostics,
        &inst.installation_id,
        None,
        env!("CARGO_PKG_VERSION"),
        "account_sign_out",
        std::collections::HashMap::new(),
    );

    Ok(account)
}

#[tauri::command]
pub async fn delete_relay_account(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<crate::identity::RelayAccount, CommandError> {
    let account = crate::identity::delete_relay_account(&state.config_dir)
        .await
        .map_err(|e| CommandError::new("DELETE_ACCOUNT_FAILED", &e))?;

    let _ = app.emit("account-changed", &account);

    let settings = state.settings.lock_or_recover().clone();
    let inst = crate::identity::get_or_create_installation_info(
        &state.config_dir,
        env!("CARGO_PKG_VERSION"),
    );
    crate::diagnostics::DiagnosticsService::report_event(
        settings.diagnostics.allow_anonymous_diagnostics,
        &inst.installation_id,
        None,
        env!("CARGO_PKG_VERSION"),
        "account_deleted",
        std::collections::HashMap::new(),
    );

    Ok(account)
}

#[tauri::command]
pub async fn get_installation_info(
    state: State<'_, AppState>,
) -> Result<crate::identity::InstallationInfo, CommandError> {
    Ok(crate::identity::get_or_create_installation_info(
        &state.config_dir,
        env!("CARGO_PKG_VERSION"),
    ))
}

#[tauri::command]
pub async fn check_for_app_updates(
    _state: State<'_, AppState>,
) -> Result<crate::updates::UpdateInfo, CommandError> {
    let current_ver = env!("CARGO_PKG_VERSION");
    Ok(crate::updates::UpdateService::check_for_updates(current_ver).await)
}

#[tauri::command]
pub async fn set_diagnostics_consent(
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<AppSettings, CommandError> {
    let mut settings = state.settings.lock_or_recover();
    settings.diagnostics.allow_anonymous_diagnostics = enabled;
    settings
        .save(&state.config_dir.join("settings.json"))
        .map_err(|e| CommandError::new("SAVE_FAILED", &e.to_string()))?;
    Ok(settings.clone())
}

#[tauri::command]
pub async fn complete_first_run(
    state: State<'_, AppState>,
) -> Result<AppSettings, CommandError> {
    let mut settings = state.settings.lock_or_recover();
    settings.diagnostics.first_run_completed = true;
    settings
        .save(&state.config_dir.join("settings.json"))
        .map_err(|e| CommandError::new("SAVE_FAILED", &e.to_string()))?;
    Ok(settings.clone())
}

// =========================================================================
// MEETINGS V2 TAURI COMMANDS
// =========================================================================

#[tauri::command]
pub async fn start_meeting_v2(
    title: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<crate::meetings_v2::MeetingSession, CommandError> {
    let settings = state.settings.lock_or_recover().clone();
    let language_config = crate::capture::SttLanguageConfig::from_settings(&settings.language);
    let mut decoding_config = crate::capture::stt::WhisperDecodingConfig::from_settings(&settings.stt);

    // Hand the recognizer the vocabulary before it guesses, rather than
    // repairing its guess afterwards.
    //
    // `normalize::apply_glossary` already rewrites known terms by edit distance,
    // but that can only fix a near-miss — it cannot recover a project name or a
    // participant's name that Whisper never produced anything close to. The
    // dictionary is the same list either way, so seeding it here costs nothing
    // and is strictly more capable. `build_stt_prompt` also folds in
    // Settings › STT's own custom prompt, so an explicitly configured prompt is
    // still honoured.
    if decoding_config.initial_prompt.is_none() {
        decoding_config.initial_prompt = settings.build_stt_prompt();
    }

    let models_dir = state.config_dir.join("models");
    let whisper_model_path = settings.stt.whisper_model_path;

    let session = state
        .meetings_v2
        .start_session(
            title,
            &models_dir,
            whisper_model_path,
            language_config,
            decoding_config,
            Some(app.clone()),
        )
        .map_err(|e: String| CommandError::new("START_MEETING_FAILED", &e))?;

    // Bring the overlay up only once recording is actually under way, then
    // re-announce the session: the overlay's webview may not have existed when
    // `start_session` emitted, and a pill that missed the start event would show
    // a stale or zero timer.
    crate::overlay::ensure_meeting_overlay(&app, true);
    let _ = app.emit("meeting-session-state-changed", &session);

    Ok(session)
}

#[tauri::command]
pub async fn stop_meeting_v2(
    session_id: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<crate::meetings_v2::MeetingSession, CommandError> {
    let result = state
        .meetings_v2
        .stop_session(session_id, Some(app.clone()))
        .await;

    // The overlay comes down once the session is genuinely finished, so the
    // pill can show "finalizing" while chunks drain instead of vanishing while
    // work is still in flight.
    if state.meetings_v2.get_active_session().is_none() {
        crate::overlay::hide_meeting_overlay(&app);
    }

    // The recording is now safely on disk. Derived processing starts here, in the
    // background, and the meeting is already openable — nothing below can affect
    // the recording, the chunks, or the raw transcript.
    if let Ok(session) = result.as_ref() {
        spawn_meeting_processing(app.clone(), &state, session.id.clone());
    }

    result.map_err(|e: String| CommandError::new("STOP_MEETING_FAILED", &e))
}

/// Kicks off derived processing for a finished meeting without blocking the
/// caller.
///
/// The deterministic stages always run — they are cheap and need no model. A
/// summary follows only if the user has left auto-generation on. Every failure
/// path here is logged and dropped: the meeting, its audio, and its raw
/// transcript are already durable, and none of them depends on this succeeding.
pub fn spawn_meeting_processing(app: AppHandle, state: &AppState, meeting_id: String) {
    let (options, provider, auto_summary) = {
        let settings = state.settings.lock_or_recover();
        (
            meeting_processing_options(&settings, None, None),
            settings.provider.clone(),
            settings.meetings.auto_generate_summary,
        )
    };
    let processor = state.meeting_processor.clone();

    tauri::async_runtime::spawn(async move {
        match processor.prepare(&meeting_id, &options) {
            Ok(processing) => {
                let _ = app.emit(MEETING_PROCESSING_EVENT, &processing);
            }
            Err(e) => {
                tracing::warn!(
                    meeting_id = %meeting_id,
                    "meeting_processing: deterministic stages failed: {}",
                    e
                );
                // Without a normalized transcript there is nothing to summarize,
                // so stop here rather than failing again downstream.
                return;
            }
        }

        if !auto_summary {
            return;
        }

        let llm = crate::meetings_v2::processing::llm::ProviderLlm::new(provider);
        match processor
            .generate_summary(&meeting_id, &llm, &options, false)
            .await
        {
            Ok(processing) => {
                let _ = app.emit(MEETING_PROCESSING_EVENT, &processing);
            }
            Err(e) => tracing::warn!(
                meeting_id = %meeting_id,
                "meeting_processing: automatic summary failed: {}",
                e
            ),
        }
    });
}

#[tauri::command]
pub async fn pause_meeting_v2(
    session_id: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<crate::meetings_v2::MeetingSession, CommandError> {
    state
        .meetings_v2
        .pause_session(session_id, Some(app))
        .map_err(|e: String| CommandError::new("PAUSE_MEETING_FAILED", &e))
}

#[tauri::command]
pub async fn resume_meeting_v2(
    session_id: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<crate::meetings_v2::MeetingSession, CommandError> {
    state
        .meetings_v2
        .resume_session(session_id, Some(app))
        .map_err(|e: String| CommandError::new("RESUME_MEETING_FAILED", &e))
}

#[tauri::command]
pub async fn get_active_meeting_v2(
    state: State<'_, AppState>,
) -> Result<Option<crate::meetings_v2::MeetingSession>, CommandError> {
    Ok(state.meetings_v2.get_active_session())
}

#[tauri::command]
pub async fn list_meetings_v2(
    state: State<'_, AppState>,
) -> Result<Vec<crate::meetings_v2::MeetingSession>, CommandError> {
    state
        .meetings_v2
        .store()
        .list_sessions()
        .map_err(|e: String| CommandError::new("LIST_MEETINGS_FAILED", &e))
}

#[tauri::command]
pub async fn get_meeting_v2(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<crate::meetings_v2::MeetingSession, CommandError> {
    state
        .meetings_v2
        .store()
        .get_session(&session_id)
        .map_err(|e: String| CommandError::new("GET_MEETING_FAILED", &e))
}

#[tauri::command]
pub async fn get_meeting_v2_transcript(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<crate::meetings_v2::TranscriptSegment>, CommandError> {
    state
        .meetings_v2
        .store()
        .get_transcript_segments(&session_id)
        .map_err(|e: String| CommandError::new("GET_TRANSCRIPT_FAILED", &e))
}

#[tauri::command]
pub async fn get_meeting_v2_diagnostics(
    state: State<'_, AppState>,
) -> Result<Vec<crate::meetings_v2::MeetingDiagnostics>, CommandError> {
    state
        .meetings_v2
        .get_diagnostics()
        .map_err(|e: String| CommandError::new("DIAGNOSTICS_FAILED", &e))
}

#[tauri::command]
pub async fn delete_meeting_v2(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<TrashItem, CommandError> {
    state
        .vault
        .move_to_trash("meeting", &session_id)
        .map_err(|e| CommandError::new("DELETE_MEETING_FAILED", &e.to_string()))
}

// ---------------------------------------------------------------------------
// MEETING PROCESSING (derived intelligence)
//
// These commands are thin: they validate input, translate settings into
// `ProcessingOptions`, and hand off to `meetings_v2::processing`. None of them
// touches the recorder, the audio chunks, or the raw transcript for writing.
// ---------------------------------------------------------------------------

/// Event emitted whenever a meeting's derived data changes, so any open view can
/// refresh without polling.
pub const MEETING_PROCESSING_EVENT: &str = "meeting-processing-updated";

/// Builds the processing options in force right now from user settings.
fn meeting_processing_options(
    settings: &AppSettings,
    summary_mode: Option<&str>,
    extension_id: Option<String>,
) -> crate::meetings_v2::ProcessingOptions {
    use crate::meetings_v2::processing::model::{MeetingExtension, SummaryMode};
    use crate::meetings_v2::processing::speakers::SpeakerIdentificationMode;
    use crate::settings::{DefaultSummaryMode, SpeakerIdentification};

    let default_mode = match settings.meetings.default_summary_mode {
        DefaultSummaryMode::Concise => SummaryMode::Concise,
        DefaultSummaryMode::Standard => SummaryMode::Standard,
        DefaultSummaryMode::Detailed => SummaryMode::Detailed,
    };

    crate::meetings_v2::ProcessingOptions {
        // The dictation dictionary doubles as the normalization glossary: these
        // are exactly the terms the user has already told Relay it mishears.
        glossary: settings.dictionary.clone(),
        generate_conversation: settings.meetings.generate_conversation_transcript,
        speaker_identification: match settings.meetings.speaker_identification {
            SpeakerIdentification::Automatic => SpeakerIdentificationMode::Automatic,
            SpeakerIdentification::Off => SpeakerIdentificationMode::Off,
        },
        diarize_speakers: settings.meetings.identify_individual_speakers,
        expected_speakers: settings.meetings.expected_speakers.filter(|&n| n > 0),
        diarization_engine: settings.meetings.diarization_engine,
        assume_in_person: settings.meetings.meetings_are_in_person,
        summary_mode: summary_mode.map(SummaryMode::parse).unwrap_or(default_mode),
        extension_id: extension_id
            .filter(|id| !id.trim().is_empty())
            .unwrap_or_else(|| settings.meetings.default_extension_id.clone()),
        user_extensions: settings
            .meetings
            .extensions
            .iter()
            .map(|e| MeetingExtension {
                id: e.id.clone(),
                name: e.name.clone(),
                instructions: e.instructions.clone(),
                builtin: false,
            })
            .collect(),
        user_instructions: Some(settings.meetings.summary_instructions.clone())
            .filter(|i| !i.trim().is_empty()),
    }
}

/// A meeting's derived data, or `None` if it has never been processed.
#[tauri::command]
pub async fn get_meeting_v2_processing(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<Option<crate::meetings_v2::MeetingProcessing>, CommandError> {
    if session_id.trim().is_empty() {
        return Err(CommandError::new("INVALID_MEETING_ID", "A meeting id is required"));
    }
    Ok(state.meeting_processor.get(&session_id))
}

/// Runs the deterministic stages — normalize, attribute speakers, build the
/// conversation. No model, no network.
#[tauri::command]
pub async fn prepare_meeting_v2(
    app: AppHandle,
    session_id: String,
    state: State<'_, AppState>,
) -> Result<crate::meetings_v2::MeetingProcessing, CommandError> {
    if session_id.trim().is_empty() {
        return Err(CommandError::new("INVALID_MEETING_ID", "A meeting id is required"));
    }
    let options = {
        let settings = state.settings.lock_or_recover();
        meeting_processing_options(&settings, None, None)
    };

    let processing = state
        .meeting_processor
        .prepare(&session_id, &options)
        .map_err(|e| CommandError::new("PREPARE_MEETING_FAILED", &e))?;

    let _ = app.emit(MEETING_PROCESSING_EVENT, &processing);
    Ok(processing)
}

/// Generates (or regenerates) a meeting summary through the canonical pipeline.
///
/// `mode` and `extensionId` override the defaults for this run only. `force`
/// re-runs structured extraction as well as prose; without it, changing the mode
/// or extension re-renders from the facts already extracted.
#[tauri::command]
pub async fn generate_meeting_v2_summary(
    app: AppHandle,
    session_id: String,
    mode: Option<String>,
    extension_id: Option<String>,
    force: Option<bool>,
    state: State<'_, AppState>,
) -> Result<crate::meetings_v2::MeetingProcessing, CommandError> {
    if session_id.trim().is_empty() {
        return Err(CommandError::new("INVALID_MEETING_ID", "A meeting id is required"));
    }

    let (options, provider) = {
        let settings = state.settings.lock_or_recover();
        (
            meeting_processing_options(&settings, mode.as_deref(), extension_id),
            settings.provider.clone(),
        )
    };

    let llm = crate::meetings_v2::processing::llm::ProviderLlm::new(provider);
    let processing = state
        .meeting_processor
        .generate_summary(&session_id, &llm, &options, force.unwrap_or(false))
        .await
        .map_err(|e| CommandError::new("GENERATE_MEETING_SUMMARY_FAILED", &e))?;

    let _ = app.emit(MEETING_PROCESSING_EVENT, &processing);
    Ok(processing)
}

/// Kept for compatibility with the existing "Generate Summary" call site. Routes
/// to the canonical pipeline rather than the retired single-call prompt.
#[tauri::command]
pub async fn summarize_meeting_v2(
    app: AppHandle,
    session_id: String,
    state: State<'_, AppState>,
) -> Result<crate::meetings_v2::MeetingProcessing, CommandError> {
    generate_meeting_v2_summary(app, session_id, None, None, None, state).await
}

/// Reads a meeting's user-written notes.
///
/// A meeting with no notes returns empty ones rather than an error: not having
/// written any is the normal case, and the UI should show an empty editor, not a
/// failure.
#[tauri::command]
pub async fn get_meeting_v2_notes(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<crate::meetings_v2::MeetingNotes, CommandError> {
    if session_id.trim().is_empty() {
        return Err(CommandError::new("INVALID_MEETING_ID", "A meeting id is required"));
    }
    state
        .meetings_v2
        .store()
        .get_notes(&session_id)
        .map_err(|e| CommandError::new("READ_MEETING_NOTES_FAILED", &e))
}

/// Saves a meeting's user-written notes.
///
/// Notes are a **source** artifact. Saving them writes `notes.json` beside
/// `session.json` and touches nothing derived: no summary is regenerated, no
/// facts are invalidated, and the next summary the user asks for reads them.
/// That separation is what makes it safe to type into this field during a
/// meeting.
#[tauri::command]
pub async fn save_meeting_v2_notes(
    session_id: String,
    during: Option<String>,
    before: Option<String>,
    state: State<'_, AppState>,
) -> Result<crate::meetings_v2::MeetingNotes, CommandError> {
    if session_id.trim().is_empty() {
        return Err(CommandError::new("INVALID_MEETING_ID", "A meeting id is required"));
    }

    let sessions = state.meetings_v2.store();
    let existing = sessions.get_notes(&session_id).unwrap_or_default();
    let notes = crate::meetings_v2::MeetingNotes {
        directives: existing.directives,
        during: during.unwrap_or(existing.during),
        before: before.unwrap_or(existing.before),
        updated_at: None,
    };

    sessions
        .save_notes(&session_id, &notes)
        .map_err(|e| CommandError::new("SAVE_MEETING_NOTES_FAILED", &e))
}

/// Adds one typed directive to a meeting's notes.
///
/// Separate from `save_meeting_v2_notes` because a directive is not a text
/// edit: it is an instruction with a kind, and the pipeline stage that acts on
/// it depends on that kind. Adding one re-prepares the meeting so the
/// correction takes effect immediately — a name correction the user has to hit
/// "regenerate" to see is a name correction they will assume did not work.
#[tauri::command]
pub async fn add_meeting_v2_directive(
    app: AppHandle,
    session_id: String,
    kind: String,
    subject: Option<String>,
    value: String,
    state: State<'_, AppState>,
) -> Result<crate::meetings_v2::MeetingNotes, CommandError> {
    use crate::meetings_v2::types::{DirectiveKind, MeetingDirective};

    if session_id.trim().is_empty() {
        return Err(CommandError::new("INVALID_MEETING_ID", "A meeting id is required"));
    }

    let kind = match kind.trim().to_uppercase().as_str() {
        "SPEAKER_NAME" => DirectiveKind::SpeakerName,
        "PARTICIPANT" => DirectiveKind::Participant,
        "TERM" => DirectiveKind::Term,
        "AGENDA" => DirectiveKind::Agenda,
        "NOTE" => DirectiveKind::Note,
        other => {
            return Err(CommandError::new(
                "INVALID_DIRECTIVE_KIND",
                &format!("{other} is not a kind of directive"),
            ))
        }
    };

    let directive = MeetingDirective::new(kind, subject.as_deref(), &value).ok_or_else(|| {
        CommandError::new(
            "INVALID_DIRECTIVE",
            if kind.needs_subject() {
                "This kind of note needs both a subject and a value"
            } else {
                "This note is empty"
            },
        )
    })?;

    let sessions = state.meetings_v2.store();
    let mut notes = sessions.get_notes(&session_id).unwrap_or_default();
    notes.directives.push(directive);
    let saved = sessions
        .save_notes(&session_id, &notes)
        .map_err(|e| CommandError::new("SAVE_MEETING_NOTES_FAILED", &e))?;

    reprepare_after_directive_change(&app, &state, &session_id);
    Ok(saved)
}

/// Removes one directive by id. Unknown ids are a no-op, not an error: the row
/// the user wanted gone is gone either way.
#[tauri::command]
pub async fn remove_meeting_v2_directive(
    app: AppHandle,
    session_id: String,
    directive_id: String,
    state: State<'_, AppState>,
) -> Result<crate::meetings_v2::MeetingNotes, CommandError> {
    if session_id.trim().is_empty() || directive_id.trim().is_empty() {
        return Err(CommandError::new(
            "INVALID_DIRECTIVE",
            "A meeting id and a directive id are required",
        ));
    }

    let sessions = state.meetings_v2.store();
    let mut notes = sessions.get_notes(&session_id).unwrap_or_default();
    notes.directives.retain(|d| d.id != directive_id);
    let saved = sessions
        .save_notes(&session_id, &notes)
        .map_err(|e| CommandError::new("SAVE_MEETING_NOTES_FAILED", &e))?;

    reprepare_after_directive_change(&app, &state, &session_id);
    Ok(saved)
}

/// Re-runs the deterministic stages so a directive's effect is visible at once.
///
/// Best effort: a failure here leaves the directive saved and the derived data
/// as it was, which is recoverable. Failing the save because re-preparation
/// failed would lose the user's correction, which is not.
fn reprepare_after_directive_change(app: &AppHandle, state: &AppState, session_id: &str) {
    let options = {
        let settings = state.settings.lock_or_recover();
        meeting_processing_options(&settings, None, None)
    };
    match state.meeting_processor.prepare(session_id, &options) {
        Ok(processing) => {
            let _ = app.emit(MEETING_PROCESSING_EVENT, &processing);
        }
        Err(e) => tracing::info!(
            meeting_id = %session_id,
            "meeting_processing: directive saved but re-preparation failed ({}); \
the correction applies on the next run",
            e
        ),
    }
}

/// Runs the meeting pipeline's self-checks against synthesized fixtures.
///
/// The point of running these here, rather than trusting CI, is that the
/// failure they cover is machine-dependent: it turns on this microphone's noise
/// floor and this installed Whisper model. Where a model is configured the run
/// also asks it to transcribe thirty seconds of room tone and reports what came
/// back, so the user can see the hallucination for themselves and see that the
/// gate stopped it.
///
/// Reads and writes nothing: no recording, no vault access, no settings change.
#[tauri::command]
pub async fn run_meeting_pipeline_selftest(
    state: State<'_, AppState>,
) -> Result<crate::meetings_v2::MeetingSelfTestReport, CommandError> {
    // The same resolution a real recording uses, so the report is about the
    // model this machine actually records with.
    let model_path = {
        let settings = state.settings.lock_or_recover();
        crate::capture::stt::resolve_meeting_model_path(
            &state.config_dir.join("models"),
            settings.stt.whisper_model_path.as_deref(),
        )
    };

    // Whisper inference is CPU-bound and the fixtures are thirty seconds long.
    tauri::async_runtime::spawn_blocking(move || {
        crate::meetings_v2::selftest::run(model_path.as_ref().and_then(|p| p.to_str()))
    })
    .await
    .map_err(|e| CommandError::new("MEETING_SELFTEST_FAILED", &e.to_string()))
}

/// Runs every speaker-separation method over one recording and reports each.
///
/// The answer to "which of these actually works" without holding three
/// meetings to find out. Reads the stored audio and the transcript; writes
/// nothing, so running it can never make a meeting worse.
#[tauri::command]
pub async fn compare_meeting_v2_speaker_engines(
    session_id: String,
    expected_speakers: Option<usize>,
    state: State<'_, AppState>,
) -> Result<crate::meetings_v2::diarize::engine::EngineComparison, CommandError> {
    if session_id.trim().is_empty() {
        return Err(CommandError::new("INVALID_MEETING_ID", "A meeting id is required"));
    }

    let mut options = {
        let settings = state.settings.lock_or_recover();
        meeting_processing_options(&settings, None, None)
    };
    if let Some(count) = expected_speakers.filter(|&n| n > 0) {
        options.expected_speakers = Some(count);
    }

    let processor = state.meeting_processor.clone();
    // Three passes over the recorded audio: CPU-bound, and not the async
    // runtime's work.
    tauri::async_runtime::spawn_blocking(move || processor.compare_engines(&session_id, &options))
        .await
        .map_err(|e| CommandError::new("COMPARE_ENGINES_FAILED", &e.to_string()))
}

/// A meeting's transcript health: what became of every recorded chunk.
///
/// Separate from the derived data because it answers a different question —
/// not "what did this meeting decide" but "how much of this meeting is even
/// here". A summary that reads thin is explained by this number and by nothing
/// else in the app.
#[tauri::command]
pub async fn get_meeting_v2_transcript_health(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<crate::meetings_v2::processing::metadata::TranscriptHealth, CommandError> {
    if session_id.trim().is_empty() {
        return Err(CommandError::new("INVALID_MEETING_ID", "A meeting id is required"));
    }

    state
        .meeting_processor
        .transcript_health(&session_id)
        .map_err(|e| CommandError::new("READ_TRANSCRIPT_HEALTH_FAILED", &e))
}

/// Renders a meeting as one Markdown document, for sharing.
///
/// The header is counted rather than generated — date, duration, participants,
/// and what became of the recording — so a summary that leaves the app carries
/// its own provenance instead of arriving as a wall of unattributed claims.
#[tauri::command]
pub async fn share_meeting_v2(
    session_id: String,
    include_summary: Option<bool>,
    include_action_items: Option<bool>,
    include_decisions: Option<bool>,
    include_conversation: Option<bool>,
    include_notes: Option<bool>,
    state: State<'_, AppState>,
) -> Result<crate::meetings_v2::processing::SharedDocument, CommandError> {
    use crate::meetings_v2::processing::share::ShareOptions;

    if session_id.trim().is_empty() {
        return Err(CommandError::new("INVALID_MEETING_ID", "A meeting id is required"));
    }

    let defaults = ShareOptions::default();
    let options = ShareOptions {
        summary: include_summary.unwrap_or(defaults.summary),
        action_items: include_action_items.unwrap_or(defaults.action_items),
        decisions: include_decisions.unwrap_or(defaults.decisions),
        conversation: include_conversation.unwrap_or(defaults.conversation),
        notes: include_notes.unwrap_or(defaults.notes),
    };

    state
        .meeting_processor
        .share_document(&session_id, options)
        .map_err(|e| CommandError::new("SHARE_MEETING_FAILED", &e))
}

/// Renames a speaker. Updates the registry only — the raw transcript is
/// untouched and every derived view resolves the new name on read.
#[tauri::command]
pub async fn rename_meeting_v2_speaker(
    app: AppHandle,
    session_id: String,
    speaker_id: String,
    display_name: Option<String>,
    state: State<'_, AppState>,
) -> Result<crate::meetings_v2::MeetingProcessing, CommandError> {
    if session_id.trim().is_empty() || speaker_id.trim().is_empty() {
        return Err(CommandError::new(
            "INVALID_SPEAKER_RENAME",
            "A meeting id and a speaker id are required",
        ));
    }

    let processing = state
        .meeting_processor
        .rename_speaker(&session_id, &speaker_id, display_name.as_deref())
        .map_err(|e| CommandError::new("RENAME_SPEAKER_FAILED", &e))?;

    let _ = app.emit(MEETING_PROCESSING_EVENT, &processing);
    Ok(processing)
}

/// Separates the recorded audio into distinct voices, then re-attributes.
///
/// §3 of `Meeting-rules/meeting_speaker_identification.md` requires this to be
/// a command the user can invoke rather than only a background step: people
/// usually decide they need speakers once they have read the notes. It runs
/// post-hoc over the stored chunk WAVs and never touches the raw transcript.
#[tauri::command]
pub async fn identify_meeting_v2_speakers(
    app: AppHandle,
    session_id: String,
    expected_speakers: Option<usize>,
    state: State<'_, AppState>,
) -> Result<crate::meetings_v2::MeetingProcessing, CommandError> {
    if session_id.trim().is_empty() {
        return Err(CommandError::new("INVALID_MEETING_ID", "A meeting id is required"));
    }

    let mut options = {
        let settings = state.settings.lock_or_recover();
        meeting_processing_options(&settings, None, None)
    };
    options.diarize_speakers = true;
    // An explicit request carries its own hint; the stored default is only a
    // default.
    if let Some(count) = expected_speakers.filter(|&n| n > 0) {
        options.expected_speakers = Some(count);
    }

    let processor = state.meeting_processor.clone();
    // Reading and characterising every chunk WAV is CPU-bound, so it does not
    // belong on the async runtime's worker threads.
    let processing = tauri::async_runtime::spawn_blocking(move || {
        processor.identify_speakers(&session_id, &options)
    })
    .await
    .map_err(|e| CommandError::new("IDENTIFY_SPEAKERS_FAILED", &e.to_string()))?
    .map_err(|e| CommandError::new("IDENTIFY_SPEAKERS_FAILED", &e))?;

    let _ = app.emit(MEETING_PROCESSING_EVENT, &processing);
    Ok(processing)
}

/// Persists an action item's checked state.
#[tauri::command]
pub async fn set_meeting_v2_action_item_status(
    app: AppHandle,
    session_id: String,
    action_item_id: String,
    done: bool,
    state: State<'_, AppState>,
) -> Result<crate::meetings_v2::MeetingProcessing, CommandError> {
    use crate::meetings_v2::processing::model::ActionItemStatus;

    if session_id.trim().is_empty() || action_item_id.trim().is_empty() {
        return Err(CommandError::new(
            "INVALID_ACTION_ITEM",
            "A meeting id and an action item id are required",
        ));
    }

    let status = if done {
        ActionItemStatus::Done
    } else {
        ActionItemStatus::Open
    };

    let processing = state
        .meeting_processor
        .set_action_item_status(&session_id, &action_item_id, status)
        .map_err(|e| CommandError::new("SET_ACTION_ITEM_STATUS_FAILED", &e))?;

    let _ = app.emit(MEETING_PROCESSING_EVENT, &processing);
    Ok(processing)
}

/// Meetings related to this one, by shared topics, entities, participants, and
/// type.
#[tauri::command]
pub async fn get_meeting_v2_related(
    session_id: String,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<crate::meetings_v2::processing::related::RelatedMeeting>, CommandError> {
    if session_id.trim().is_empty() {
        return Err(CommandError::new("INVALID_MEETING_ID", "A meeting id is required"));
    }
    state
        .meeting_processor
        .related(&session_id, limit.unwrap_or(5).clamp(1, 25))
        .map_err(|e| CommandError::new("GET_RELATED_MEETINGS_FAILED", &e))
}

/// The per-stage processing log, for diagnosing a meeting that did not process
/// cleanly without reading source or log files.
#[tauri::command]
pub async fn get_meeting_v2_processing_log(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<crate::meetings_v2::processing::model::ProcessingLogEntry>, CommandError> {
    if session_id.trim().is_empty() {
        return Err(CommandError::new("INVALID_MEETING_ID", "A meeting id is required"));
    }
    Ok(state.meeting_processor.log(&session_id))
}

/// Every available summary extension — the shipped ones plus the user's.
#[tauri::command]
pub async fn get_meeting_v2_extensions(
    state: State<'_, AppState>,
) -> Result<Vec<crate::meetings_v2::processing::model::MeetingExtension>, CommandError> {
    use crate::meetings_v2::processing::model::MeetingExtension;

    let settings = state.settings.lock_or_recover().clone();
    let user_defined: Vec<MeetingExtension> = settings
        .meetings
        .extensions
        .iter()
        .map(|e| MeetingExtension {
            id: e.id.clone(),
            name: e.name.clone(),
            instructions: e.instructions.clone(),
            builtin: false,
        })
        .collect();

    Ok(crate::meetings_v2::processing::modes::resolve_extensions(
        &user_defined,
    ))
}

/// A compact per-meeting view for the meetings list: the extracted title,
/// processing status, type, and outstanding task count.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MeetingProcessingIndexEntry {
    pub meeting_id: String,
    /// The extracted title, when one exists. The list falls back to the
    /// recorder's own title, which this deliberately does not overwrite.
    pub title: Option<String>,
    pub status: crate::meetings_v2::processing::model::ProcessingStatus,
    pub meeting_type: Option<String>,
    pub has_summary: bool,
    pub open_action_item_count: usize,
    pub action_item_count: usize,
}

/// The processing index for every processed meeting, in one call, so the list
/// view does not fan out per row.
#[tauri::command]
pub async fn list_meeting_v2_processing(
    state: State<'_, AppState>,
) -> Result<Vec<MeetingProcessingIndexEntry>, CommandError> {
    use crate::meetings_v2::processing::model::ActionItemStatus;

    let sessions = state
        .meetings_v2
        .store()
        .list_sessions()
        .map_err(|e| CommandError::new("LIST_MEETINGS_FAILED", &e))?;

    Ok(sessions
        .into_iter()
        .filter_map(|session| {
            let processing = state.meeting_processor.get(&session.id)?;
            let facts = processing.facts.as_ref();
            Some(MeetingProcessingIndexEntry {
                meeting_id: session.id,
                title: facts.map(|f| f.title.clone()),
                status: processing.status,
                meeting_type: facts.map(|f| f.meeting_type.label().to_string()),
                has_summary: processing.summary.is_some(),
                open_action_item_count: facts
                    .map(|f| {
                        f.action_items
                            .iter()
                            .filter(|a| a.status == ActionItemStatus::Open)
                            .count()
                    })
                    .unwrap_or(0),
                action_item_count: facts.map(|f| f.action_items.len()).unwrap_or(0),
            })
        })
        .collect())
}

/// Grows or shrinks the meeting pill for its hovered state.
///
/// Frontend-owned presentation, backend-owned geometry — the same split the
/// dictation pill uses. The window is resized rather than left permanently large
/// because a transparent margin would still swallow clicks for the whole
/// meeting.
#[tauri::command]
pub async fn set_meeting_overlay_expanded(
    app: AppHandle,
    expanded: bool,
) -> Result<(), CommandError> {
    crate::overlay::set_meeting_overlay_expanded(&app, expanded);
    Ok(())
}

/// One action item that reached (or failed to reach) a Kanban board.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MeetingTaskPushResult {
    pub action_item_id: String,
    pub kanban_card_id: Option<String>,
    pub title: String,
    pub assignee: String,
    /// Set only when this item could not be added.
    pub error: Option<String>,
}

/// Adds a meeting's action items to the Kanban board as tasks.
///
/// With `action_item_id` set, adds exactly that item; without it, adds every
/// action item that is not already on the board. Pressing "add all" twice is
/// therefore safe — the second press adds only what is new.
///
/// The mapping itself lives in `processing::tasks`, so what lands on a card is
/// decided in one tested place rather than here.
#[tauri::command]
pub async fn push_meeting_v2_action_items_to_kanban(
    app: AppHandle,
    session_id: String,
    action_item_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<MeetingTaskPushResult>, CommandError> {
    use crate::meetings_v2::processing::tasks::{draft_from_action_item, pending_drafts};

    if session_id.trim().is_empty() {
        return Err(CommandError::new("INVALID_MEETING_ID", "A meeting id is required"));
    }

    let session = state
        .meetings_v2
        .store()
        .get_session(&session_id)
        .map_err(|e| CommandError::new("MEETING_NOT_FOUND", &e))?;

    let processing = state.meeting_processor.get(&session_id).ok_or_else(|| {
        CommandError::new(
            "MEETING_NOT_PROCESSED",
            "Process this meeting before adding its to-dos as tasks",
        )
    })?;

    let facts = processing.facts.as_ref().ok_or_else(|| {
        CommandError::new(
            "MEETING_NOT_PROCESSED",
            "This meeting has no extracted to-dos yet",
        )
    })?;

    let meeting_title = facts.title.trim();
    let meeting_title = if meeting_title.is_empty() {
        session.title.as_str()
    } else {
        meeting_title
    };
    let meeting_date = session.started_at.as_deref();

    let drafts = match action_item_id.as_deref().map(str::trim).filter(|id| !id.is_empty()) {
        Some(id) => {
            let item = facts
                .action_items
                .iter()
                .find(|i| i.id == id)
                .ok_or_else(|| {
                    CommandError::new("UNKNOWN_ACTION_ITEM", "That to-do is no longer in this meeting")
                })?;
            if item.kanban_card_id.is_some() {
                return Err(CommandError::new(
                    "ALREADY_A_TASK",
                    "This to-do is already on the board",
                ));
            }
            vec![draft_from_action_item(
                item,
                &processing.speakers,
                meeting_title,
                meeting_date,
            )]
        }
        None => pending_drafts(facts, &processing.speakers, meeting_title, meeting_date),
    };

    let mut results = Vec::with_capacity(drafts.len());
    for draft in drafts {
        let card = KanbanCard {
            id: format!("card_{}", uuid::Uuid::new_v4()),
            title: draft.title.clone(),
            assignee: draft.assignee.clone(),
            status: draft.status.to_string(),
            priority: draft.priority.to_string(),
            due_date: draft.due_date.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            description: draft.description.clone(),
            source_note_id: Some(session_id.clone()),
        };

        // Save the card first: a card without provenance is recoverable, but a
        // to-do marked as pushed with no card behind it is not.
        if let Err(e) = state.vault.save_kanban_card(&card) {
            results.push(MeetingTaskPushResult {
                action_item_id: draft.action_item_id,
                kanban_card_id: None,
                title: draft.title,
                assignee: draft.assignee,
                error: Some(e.to_string()),
            });
            continue;
        }

        if let Err(e) = state.meeting_processor.record_action_item_task(
            &session_id,
            &draft.action_item_id,
            &card.id,
        ) {
            tracing::warn!(
                meeting_id = %session_id,
                action_item_id = %draft.action_item_id,
                "meeting_tasks: card saved but the to-do could not be marked as pushed: {}",
                e
            );
        }

        results.push(MeetingTaskPushResult {
            action_item_id: draft.action_item_id,
            kanban_card_id: Some(card.id),
            title: draft.title,
            assignee: draft.assignee,
            error: None,
        });
    }

    if let Some(processing) = state.meeting_processor.get(&session_id) {
        let _ = app.emit(MEETING_PROCESSING_EVENT, &processing);
    }
    let _ = app.emit("kanban-cards-changed", ());

    Ok(results)
}

/// Converts a meeting into a Scribble.
///
/// Reuses the existing Scribble infrastructure rather than adding a second
/// implementation: same `Scribble` type, same vault, same saved event. The
/// Scribble references the meeting as its source instead of duplicating it, and
/// the meeting records the Scribble it produced.
#[tauri::command]
pub async fn promote_meeting_v2_to_scribble(
    app: AppHandle,
    session_id: String,
    custom_title: Option<String>,
    include_conversation: Option<bool>,
    state: State<'_, AppState>,
) -> Result<Scribble, CommandError> {
    use crate::meetings_v2::processing::model::ScribbleRef;

    if session_id.trim().is_empty() {
        return Err(CommandError::new("INVALID_MEETING_ID", "A meeting id is required"));
    }

    let session = state
        .meetings_v2
        .store()
        .get_session(&session_id)
        .map_err(|e| CommandError::new("MEETING_NOT_FOUND", &e))?;

    let processing = state.meeting_processor.get(&session_id).ok_or_else(|| {
        CommandError::new(
            "MEETING_NOT_PROCESSED",
            "Process this meeting before turning it into a Scribble",
        )
    })?;

    let content = crate::meetings_v2::processing::render_scribble_markdown(
        &processing,
        &session.title,
        include_conversation.unwrap_or(false),
    );

    let title = custom_title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .or_else(|| processing.facts.as_ref().map(|f| f.title.clone()))
        .unwrap_or_else(|| session.title.clone());

    let scribble = Scribble::from_meeting(
        &session_id,
        &session.title,
        &content,
        &title,
        processing
            .facts
            .as_ref()
            .map(|f| f.topics.iter().map(|t| t.label.clone()).collect())
            .unwrap_or_default(),
        processing
            .facts
            .as_ref()
            .map(|f| f.entities.iter().map(|e| e.name.clone()).collect())
            .unwrap_or_default(),
    );

    state
        .vault
        .save_scribble(&scribble)
        .map_err(|e| CommandError::new("VAULT_SAVE_FAILED", &e.to_string()))?;

    // Provenance in both directions: the Scribble names its source meeting, and
    // the meeting names the Scribble it produced.
    let updated = state.meeting_processor.record_scribble(
        &session_id,
        ScribbleRef {
            scribble_id: scribble.id.clone(),
            created_at: scribble.created_at.clone(),
            title: scribble.title.clone(),
        },
    );
    match updated {
        Ok(processing) => {
            let _ = app.emit(MEETING_PROCESSING_EVENT, &processing);
        }
        Err(e) => tracing::warn!(
            meeting_id = %session_id,
            "meeting_processing: could not record the Scribble reference: {}",
            e
        ),
    }

    let _ = app.emit(SCRIBBLE_SAVED_EVENT, &scribble);
    Ok(scribble)
}






// ── Talkback ────────────────────────────────────────────────────────────
//
// The conversational surface. Commands here stay thin per
// `rules/api-conventions.md`: they resolve settings, hand off to
// `talkback::engine`, and map its errors. Nothing decides anything.

/// Turns Talkback on.
///
/// `voice` false is the text fallback — the same engine, same retrieval,
/// same session, no microphone. That is what makes RAG and provider
/// behaviour testable without a sound card, and what keeps Talkback
/// usable when TTS or STT is unconfigured.
#[tauri::command]
pub async fn start_talkback(
    app: AppHandle,
    voice: bool,
    state: State<'_, AppState>,
) -> Result<crate::talkback::TalkbackState, CommandError> {
    // Dictation and Talkback share one microphone. Refusing with a
    // specific code beats two capture sessions fighting over the device.
    if voice && state.recorder.is_active() {
        return Err(CommandError::new(
            "CAPTURE_ACTIVE",
            "Relay is already recording. Stop dictation before starting Talkback.",
        ));
    }

    let settings = state.settings.lock_or_recover().clone();
    let language = crate::capture::SttLanguageConfig::from_settings(&settings.language);
    let models_dir = state.config_dir.join("models");
    let model_path = if voice {
        crate::capture::stt::resolve_dictation_model_path(&models_dir, &settings.stt)
            .await
            .map(PathBuf::from)
    } else {
        None
    };

    state
        .talkback
        .enable(&app, &settings.talkback, voice, model_path, language)
        .map_err(|e| CommandError::new("TALKBACK_START_FAILED", &e))
}

/// Turns Talkback off, closing the microphone stream rather than muting it.
#[tauri::command]
pub async fn stop_talkback(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<crate::talkback::TalkbackState, CommandError> {
    Ok(state.talkback.disable(&app))
}

#[tauri::command]
pub async fn get_talkback_state(
    state: State<'_, AppState>,
) -> Result<crate::talkback::TalkbackState, CommandError> {
    Ok(state.talkback.state())
}

#[tauri::command]
pub async fn get_talkback_session(
    state: State<'_, AppState>,
) -> Result<crate::talkback::TalkbackSession, CommandError> {
    Ok(state.talkback.session_snapshot())
}

/// Submits one turn — typed, or the text of a spoken utterance.
///
/// Both paths run the identical engine; `stt_ms` is the only difference,
/// and it exists so latency numbers are not polluted by typed turns.
#[tauri::command]
pub async fn submit_talkback_turn(
    app: AppHandle,
    text: String,
    typed: bool,
    stt_ms: Option<u64>,
    state: State<'_, AppState>,
) -> Result<String, CommandError> {
    if text.trim().is_empty() {
        return Err(CommandError::new("INVALID_INPUT", "Turn text is empty"));
    }
    if state.talkback.state() == crate::talkback::TalkbackState::Off {
        return Err(CommandError::new(
            "TALKBACK_OFF",
            "Talkback is off. Switch it on before sending a turn.",
        ));
    }

    let settings = state.settings.lock_or_recover().clone();
    let ctx = crate::talkback::TurnContext {
        app: &app,
        engine: &state.talkback,
        vault: &state.vault,
        sessions: &state.meetings_v2.store(),
        processor: &state.meeting_processor,
        settings: &settings,
        stt_ms: stt_ms.map(u128::from),
        typed,
        tts_root: &state.tts_root,
        resource_dir: resource_dir(&app),
    };

    crate::talkback::engine::run_turn(ctx, &text)
        .await
        .map_err(|e| CommandError::new("TALKBACK_TURN_FAILED", &e))
}

/// Barge-in: stop speaking and listen.
///
/// Called by the frontend when the user clicks "stop", and by the voice
/// worker when it hears speech over the agent.
#[tauri::command]
pub async fn interrupt_talkback(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<crate::talkback::TalkbackState, CommandError> {
    // A manual stop has no follow-up speech, so the conversation returns
    // to listening rather than waiting in INTERRUPTED for words that are
    // not coming.
    state.talkback.interrupt(&app, true);
    Ok(state.talkback.state())
}

/// Retrieval on its own, with no model involved.
///
/// The debugging and evaluation entry point: it answers "what would
/// Talkback have been given?" without spending a generation, which is how
/// retrieval quality gets judged rather than guessed.
#[tauri::command]
pub async fn search_talkback_context(
    query: String,
    state: State<'_, AppState>,
) -> Result<crate::talkback::RetrievalResult, CommandError> {
    if query.trim().is_empty() {
        return Err(CommandError::new("INVALID_INPUT", "Query is empty"));
    }
    let settings = state.settings.lock_or_recover().clone();
    let wanted = settings.talkback.effective_sources();
    let candidates = crate::talkback::sources::gather_candidates(
        &state.vault,
        &state.meetings_v2.store(),
        &state.meeting_processor,
        &wanted,
    );
    let budget = crate::talkback::assemble::char_budget_for(settings.provider.context_tokens);
    let request = crate::talkback::RetrievalQuery::new(&query)
        .with_sources(wanted)
        .with_char_budget(budget);
    Ok(crate::talkback::retrieval::rank(
        &candidates,
        &request,
        chrono::Utc::now(),
    ))
}

// ── Local voice (TTS) setup ─────────────────────────────────────────────
//
// "Configure `piper_binary_path` in settings.json" is not a product.
// These commands back the Settings › Talkback voice card, which is the
// answer to "I installed Relay — how do I make Talkback speak?".

/// Everything the voice settings UI needs, in one filesystem pass.
#[tauri::command]
pub async fn get_tts_status(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<crate::tts::TtsStatus, CommandError> {
    let settings = state.settings.lock_or_recover().clone();
    Ok(crate::tts::status(
        &settings.tts,
        &state.tts_root,
        resource_dir(&app).as_deref(),
    ))
}

/// Opens the OS file picker for a Piper executable.
#[tauri::command]
pub async fn browse_for_piper_binary(app: AppHandle) -> Result<Option<String>, CommandError> {
    // Filtered to the executable extension on Windows; on Unix a Piper
    // build has no extension at all, so no filter is offered there.
    let picked = tauri::async_runtime::spawn_blocking(move || {
        let dialog = app.dialog().file().set_title("Select the Piper program");
        if cfg!(windows) {
            dialog.add_filter("Program", &["exe"]).blocking_pick_file()
        } else {
            dialog.blocking_pick_file()
        }
    })
    .await
    .map_err(|e| CommandError::new("DIALOG_TASK_FAILED", &e.to_string()))?;

    picked_path(picked)
}

/// Opens the OS file picker for a Piper voice model.
#[tauri::command]
pub async fn browse_for_piper_voice(app: AppHandle) -> Result<Option<String>, CommandError> {
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Select a Piper voice model")
            .add_filter("Piper voice", &["onnx"])
            .blocking_pick_file()
    })
    .await
    .map_err(|e| CommandError::new("DIALOG_TASK_FAILED", &e.to_string()))?;

    picked_path(picked)
}

fn picked_path(
    picked: Option<tauri_plugin_dialog::FilePath>,
) -> Result<Option<String>, CommandError> {
    match picked {
        Some(path) => path
            .into_path()
            .map(|p| Some(p.to_string_lossy().to_string()))
            .map_err(|e| CommandError::new("DIALOG_PATH_INVALID", &e.to_string())),
        None => Ok(None),
    }
}

/// Persists a voice configuration and reports what it resolved to.
///
/// Validation happens here rather than in the UI so a hand-edited
/// settings file gets the same treatment as a browsed one. An empty
/// string clears the field, which is how a user returns to Relay's
/// automatic discovery.
#[tauri::command]
pub async fn set_tts_configuration(
    app: AppHandle,
    binary_path: Option<String>,
    voice_path: Option<String>,
    state: State<'_, AppState>,
) -> Result<crate::tts::TtsStatus, CommandError> {
    let settings_path = state.settings_path();
    let updated = {
        let mut settings = state.settings.lock_or_recover();
        if let Some(binary) = binary_path {
            settings.tts.piper_binary_path = normalize_setting(binary);
        }
        if let Some(voice) = voice_path {
            settings.tts.piper_voice_path = normalize_setting(voice);
        }
        settings
            .save(&settings_path)
            .map_err(|e| CommandError::new("SETTINGS_SAVE_FAILED", &e.to_string()))?;
        settings.clone()
    };

    // Every other settings surface listens for this, so the Talkback page
    // learns the voice is ready without polling.
    let _ = app.emit("settings-changed", &updated);

    Ok(crate::tts::status(
        &updated.tts,
        &state.tts_root,
        resource_dir(&app).as_deref(),
    ))
}

/// A blank field means "unset", not "a path that happens to be empty".
fn normalize_setting(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Speaks a test sentence and returns the audio for the UI to play.
///
/// The one honest way to answer "will this voice work?" — it exercises
/// the same provider, binary and model a real turn would, so a
/// configuration that passes here cannot fail differently in conversation.
#[tauri::command]
pub async fn test_tts_voice(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, CommandError> {
    let settings = state.settings.lock_or_recover().clone();
    let tts_root = state.tts_root.clone();
    let resources = resource_dir(&app);

    // Synthesis spawns a process and waits on it; that belongs on a
    // blocking thread, not on the async runtime.
    let audio = tauri::async_runtime::spawn_blocking(move || {
        let provider =
            crate::tts::resolve_provider(&settings.tts, &tts_root, resources.as_deref());
        provider.synthesize(crate::tts::TEST_PHRASE)
    })
    .await
    .map_err(|e| CommandError::new("TTS_TASK_FAILED", &e.to_string()))?;

    match audio {
        Ok(Some(audio)) => Ok(audio.wav_base64),
        Ok(None) => Err(CommandError::new(
            "TTS_NOT_CONFIGURED",
            "Local voice isn't set up yet.",
        )),
        Err(e) => Err(CommandError::new("TTS_TEST_FAILED", &e.to_string())),
    }
}

/// Creates Relay's managed voice folders and returns the voices one, so
/// the UI can tell the user exactly where to drop files — and so the
/// folder exists when they go looking for it.
#[tauri::command]
pub async fn prepare_tts_folders(state: State<'_, AppState>) -> Result<String, CommandError> {
    let voices = crate::tts::discovery::managed_voices_dir(&state.tts_root);
    let piper = crate::tts::discovery::managed_piper_dir(&state.tts_root);
    for dir in [&piper, &voices] {
        std::fs::create_dir_all(dir)
            .map_err(|e| CommandError::new("TTS_FOLDER_FAILED", &e.to_string()))?;
    }
    Ok(voices.to_string_lossy().to_string())
}

/// Progress from an in-flight voice setup.
pub const VOICE_INSTALL_PROGRESS_EVENT: &str = "voice-install-progress";

/// Downloads and installs a local voice, end to end.
///
/// `voice_id` names an entry in Relay's own catalogue
/// (`tts::manifest`) — never a URL. The frontend cannot ask Relay to
/// download something the catalogue does not list, which is the point.
///
/// Runs on a blocking task: it streams downloads, hashes them, unpacks an
/// archive and spawns a process, none of which belongs on the async
/// runtime. Progress arrives as `voice-install-progress` events.
#[tauri::command]
pub async fn install_local_voice(
    app: AppHandle,
    voice_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<crate::tts::TtsStatus, CommandError> {
    if !state.voice_install.begin() {
        return Err(CommandError::new(
            "INSTALL_IN_PROGRESS",
            "Voice setup is already running.",
        ));
    }

    let manifest = match crate::tts::manifest::VoiceManifest::load() {
        Ok(manifest) => manifest,
        Err(e) => {
            state.voice_install.end();
            tracing::error!("tts: voice catalogue unusable: {}", e);
            return Err(CommandError::new(
                "NOT_PROVISIONED",
                "Automatic voice setup isn't available in this build of Relay.",
            ));
        }
    };

    // Default to the recommended voice, so first-run setup never makes
    // the user choose one.
    let voice_id = match voice_id.filter(|v| !v.trim().is_empty()) {
        Some(id) => id,
        None => match manifest.recommended_voice() {
            Some(voice) => voice.id.clone(),
            None => {
                state.voice_install.end();
                return Err(CommandError::new(
                    "NOT_PROVISIONED",
                    "Relay has no recommended voice to install.",
                ));
            }
        },
    };

    let tts_root = state.tts_root.clone();
    let install_state = state.voice_install.clone();
    let progress_app = app.clone();

    let outcome = tauri::async_runtime::spawn_blocking({
        let install_state = install_state.clone();
        let voice_id = voice_id.clone();
        move || {
            crate::tts::installer::install(crate::tts::installer::InstallRequest {
                manifest: &manifest,
                voice_id: &voice_id,
                tts_root: &tts_root,
                platform: crate::tts::manifest::current_platform(),
                arch: crate::tts::manifest::current_arch(),
                on_progress: &move |progress| {
                    let _ = progress_app.emit(VOICE_INSTALL_PROGRESS_EVENT, &progress);
                },
                is_cancelled: &move || install_state.is_cancelled(),
            })
        }
    })
    .await;

    state.voice_install.end();

    let outcome = match outcome {
        Ok(Ok(outcome)) => outcome,
        Ok(Err(e)) => {
            // The detail goes to the log; the user gets the sentence.
            tracing::warn!("tts: voice setup failed ({}): {}", e.code(), e.detail());
            return Err(CommandError::new(e.code(), &e.to_string()));
        }
        Err(e) => {
            tracing::error!("tts: voice setup task failed: {}", e);
            return Err(CommandError::new(
                "IO",
                "Voice setup couldn't be completed. Please try again.",
            ));
        }
    };

    // Persist what was installed, so the provider resolves it on the very
    // next turn without a restart.
    let settings_path = state.settings_path();
    let updated = {
        let mut settings = state.settings.lock_or_recover();
        settings.tts.piper_binary_path =
            Some(outcome.binary_path.to_string_lossy().to_string());
        settings.tts.piper_voice_path = Some(outcome.voice_path.to_string_lossy().to_string());
        settings
            .save(&settings_path)
            .map_err(|e| CommandError::new("SETTINGS_SAVE_FAILED", &e.to_string()))?;
        settings.clone()
    };
    let _ = app.emit("settings-changed", &updated);

    tracing::info!(
        voice = %outcome.voice_id,
        engine_version = %outcome.runtime_version,
        reused_engine = outcome.reused_runtime,
        "tts: local voice installed"
    );

    Ok(crate::tts::status(
        &updated.tts,
        &state.tts_root,
        resource_dir(&app).as_deref(),
    ))
}

/// Abandons an in-flight voice setup.
///
/// The install polls this between chunks, so cancelling stops mid-download
/// rather than at the end of a file, and staging is removed on the way out.
#[tauri::command]
pub async fn cancel_voice_install(state: State<'_, AppState>) -> Result<(), CommandError> {
    state.voice_install.cancel();
    Ok(())
}

#[cfg(test)]
mod voice_install_tests {
    use super::VoiceInstall;
    use std::sync::Arc;

    #[test]
    fn only_one_install_can_hold_the_slot() {
        let install = VoiceInstall::default();
        assert!(install.begin(), "the first caller takes the slot");
        assert!(!install.begin(), "a second click must not start a second download");
        install.end();
        assert!(install.begin(), "the slot is free again once the first ends");
    }

    #[test]
    fn a_cancel_does_not_poison_the_next_attempt() {
        let install = VoiceInstall::default();
        assert!(install.begin());
        install.cancel();
        assert!(install.is_cancelled());
        install.end();

        // Retrying after a cancelled setup is the common case, and it must
        // not be cancelled before it starts.
        assert!(install.begin());
        assert!(!install.is_cancelled());
    }

    #[test]
    fn a_cancel_arriving_with_nothing_running_is_cleared_by_the_next_begin() {
        // The UI can fire Cancel as the install is finishing; the flag must
        // not survive into the next run.
        let install = VoiceInstall::default();
        install.cancel();
        assert!(install.begin());
        assert!(!install.is_cancelled());
    }

    #[test]
    fn cancellation_is_visible_from_the_worker_thread() {
        let install = Arc::new(VoiceInstall::default());
        assert!(install.begin());

        let worker = install.clone();
        let handle = std::thread::spawn(move || {
            while !worker.is_cancelled() {
                std::thread::yield_now();
            }
            true
        });

        install.cancel();
        assert!(handle.join().unwrap());
        assert!(install.is_running(), "cancelling does not release the slot");
        install.end();
        assert!(!install.is_running());
    }
}

// ---------------------------------------------------------------------------
// Web capture
// ---------------------------------------------------------------------------

/// Progress for one capture, broadcast so any surface can show it. The stages
/// are `SAVING`, `SAVED`, `ANALYSING`, `ANALYSED`, `FAILED`.
pub const CAPTURE_PROGRESS_EVENT: &str = "capture-progress";

#[derive(Debug, Clone, Serialize)]
pub struct CaptureProgress {
    pub stage: String,
    pub capture_id: Option<String>,
    pub title: Option<String>,
    pub application: Option<String>,
    pub message: Option<String>,
}

/// What the pairing UI needs to show, and what the extension needs to connect.
#[derive(Debug, Clone, Serialize)]
pub struct CaptureBridgeStatus {
    pub enabled: bool,
    /// Whether a listener is actually bound right now. Differs from `enabled`
    /// when the port could not be bound at all.
    pub running: bool,
    /// The port actually in use, which may differ from the configured one if
    /// that port was taken.
    pub port: u16,
    pub configured_port: u16,
    /// The pairing secret, shown so the user can paste it into the extension.
    /// `None` until capture has been enabled for the first time.
    pub pairing_token: Option<String>,
    pub protocol_version: u32,
    pub analyze_on_capture: bool,
    pub capture_hotkey: String,
    pub last_error: Option<String>,
}

fn emit_capture_progress(app: &AppHandle, progress: CaptureProgress) {
    let _ = app.emit(CAPTURE_PROGRESS_EVENT, progress);
}

/// Stores one capture payload and, if configured, kicks off analysis.
///
/// Storage and interpretation are separated here on purpose: the artifact is
/// durable the moment `ingest` returns, and the analysis pass runs afterwards
/// on a background task whose failure is logged and never propagated.
fn accept_capture(app: &AppHandle, bytes: &[u8]) -> (u16, String) {
    let state = app.state::<AppState>();
    emit_capture_progress(
        app,
        CaptureProgress {
            stage: "SAVING".to_string(),
            capture_id: None,
            title: None,
            application: None,
            message: None,
        },
    );

    match crate::capture::web::ingest(&state.vault, bytes) {
        Ok(artifact) => {
            let application = artifact.capture.as_ref().map(|c| c.application.clone());
            emit_capture_progress(
                app,
                CaptureProgress {
                    stage: "SAVED".to_string(),
                    capture_id: Some(artifact.id.clone()),
                    title: Some(artifact.original_filename.clone()),
                    application: application.clone(),
                    message: None,
                },
            );

            let analyze = state.settings.lock_or_recover().capture.analyze_on_capture;
            if analyze {
                spawn_capture_analysis(app.clone(), artifact.id.clone());
            }

            let body = serde_json::json!({
                "ok": true,
                "id": artifact.id,
                "title": artifact.original_filename,
                "capture_type": artifact.capture.as_ref().map(|c| c.capture_type.clone()),
                "application": application,
                "notes": artifact.capture.as_ref().map(|c| c.notes.clone()).unwrap_or_default(),
            });
            (200, body.to_string())
        }
        Err(e) => {
            // The message is the user-facing one from `WebCaptureError`; the
            // payload itself is never logged, because it is page content.
            tracing::warn!("[Capture] Rejected a capture: {}", e);
            emit_capture_progress(
                app,
                CaptureProgress {
                    stage: "FAILED".to_string(),
                    capture_id: None,
                    title: None,
                    application: None,
                    message: Some(e.to_string()),
                },
            );
            let status = match e {
                crate::capture::web::WebCaptureError::PayloadTooLarge(_) => 413,
                crate::capture::web::WebCaptureError::EmptyCapture => 422,
                crate::capture::web::WebCaptureError::Vault(_) => 500,
                _ => 400,
            };
            (
                status,
                crate::capture::web::bridge::error_body("CAPTURE_REJECTED", &e.to_string()),
            )
        }
    }
}

/// Runs Relay's existing analysis contract over a stored capture.
///
/// Reuses `enrich_vault_file` rather than introducing a capture-specific
/// prompt: a capture is a Vault artifact, and it should be summarized and
/// tagged by exactly the same rules as an imported document.
fn spawn_capture_analysis(app: AppHandle, capture_id: String) {
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        emit_capture_progress(
            &app,
            CaptureProgress {
                stage: "ANALYSING".to_string(),
                capture_id: Some(capture_id.clone()),
                title: None,
                application: None,
                message: None,
            },
        );

        let settings = state.settings.lock_or_recover().clone();
        let llm = LLMClient::new(settings.provider);
        match crate::pipeline::enrich_vault_file(&llm, &state.vault, &capture_id).await {
            Ok(_) => emit_capture_progress(
                &app,
                CaptureProgress {
                    stage: "ANALYSED".to_string(),
                    capture_id: Some(capture_id),
                    title: None,
                    application: None,
                    message: None,
                },
            ),
            Err(e) => {
                // A failed analysis is a missing summary, not a lost capture.
                tracing::warn!("[Capture] Analysis failed for {}: {}", capture_id, e);
                emit_capture_progress(
                    &app,
                    CaptureProgress {
                        stage: "ANALYSED".to_string(),
                        capture_id: Some(capture_id),
                        title: None,
                        application: None,
                        message: Some(
                            "Saved, but Relay could not analyse it. The capture is intact — try \
                             Analyse again from the capture."
                                .to_string(),
                        ),
                    },
                );
            }
        }
    });
}

/// Starts or stops the bridge so that it matches the current settings.
///
/// Called at startup and after every settings save, so "enabled" in Settings
/// and "a socket is open" can never drift apart.
pub fn apply_capture_bridge(app: &AppHandle, state: &AppState) {
    let (enabled, port, token) = {
        let settings = state.settings.lock_or_recover();
        (
            settings.capture.bridge_enabled,
            settings.capture.bridge_port,
            settings.capture.pairing_token.clone(),
        )
    };

    let mut bridge = state.capture_bridge.lock_or_recover();
    if let Some(existing) = bridge.take() {
        existing.stop();
    }

    if !enabled {
        return;
    }

    let Some(token) = token else {
        tracing::warn!("[Capture] Bridge enabled without a pairing token; not starting");
        return;
    };

    let handler_app = app.clone();
    match crate::capture::web::bridge::start(port, token, move |bytes| {
        accept_capture(&handler_app, bytes)
    }) {
        Ok(handle) => *bridge = Some(handle),
        Err(e) => tracing::error!("[Capture] Bridge failed to start: {}", e),
    }
}

fn bridge_status(state: &AppState, last_error: Option<String>) -> CaptureBridgeStatus {
    let settings = state.settings.lock_or_recover().clone();
    let bridge = state.capture_bridge.lock_or_recover();
    let running = bridge.as_ref().is_some_and(|b| b.is_running());
    CaptureBridgeStatus {
        enabled: settings.capture.bridge_enabled,
        running,
        port: bridge
            .as_ref()
            .map(|b| b.port)
            .unwrap_or(settings.capture.bridge_port),
        configured_port: settings.capture.bridge_port,
        pairing_token: settings.capture.pairing_token.clone(),
        protocol_version: crate::capture::web::PROTOCOL_VERSION,
        analyze_on_capture: settings.capture.analyze_on_capture,
        capture_hotkey: settings.hotkeys.capture_hotkey.clone(),
        last_error,
    }
}

#[tauri::command]
pub async fn get_capture_bridge_status(
    state: State<'_, AppState>,
) -> Result<CaptureBridgeStatus, CommandError> {
    Ok(bridge_status(&state, None))
}

/// Turns the capture bridge on or off, generating a pairing token the first
/// time it is switched on.
#[tauri::command]
pub async fn set_capture_bridge_enabled(
    app: AppHandle,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<CaptureBridgeStatus, CommandError> {
    {
        let mut settings = state.settings.lock_or_recover();
        settings.capture.bridge_enabled = enabled;
        if enabled && settings.capture.pairing_token.is_none() {
            settings.capture.pairing_token =
                Some(crate::capture::web::bridge::generate_token());
        }
        settings
            .save(&state.settings_path())
            .map_err(|e| CommandError::new("CONFIG_SAVE_FAILED", &e.to_string()))?;
    }

    apply_capture_bridge(&app, &state);
    let status = bridge_status(&state, None);
    if enabled && !status.running {
        return Err(CommandError::new(
            "CAPTURE_BRIDGE_START_FAILED",
            "Relay could not open a local port for capture. Another program may be using it — \
             try a different port in Capture settings.",
        ));
    }
    Ok(status)
}

/// Issues a new pairing token, which immediately invalidates every browser
/// that was paired with the old one.
#[tauri::command]
pub async fn regenerate_capture_pairing_token(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<CaptureBridgeStatus, CommandError> {
    {
        let mut settings = state.settings.lock_or_recover();
        settings.capture.pairing_token = Some(crate::capture::web::bridge::generate_token());
        settings
            .save(&state.settings_path())
            .map_err(|e| CommandError::new("CONFIG_SAVE_FAILED", &e.to_string()))?;
    }
    apply_capture_bridge(&app, &state);
    Ok(bridge_status(&state, None))
}

/// Sets the preferred loopback port and rebinds.
#[tauri::command]
pub async fn set_capture_bridge_port(
    app: AppHandle,
    port: u16,
    state: State<'_, AppState>,
) -> Result<CaptureBridgeStatus, CommandError> {
    if port < 1024 {
        return Err(CommandError::new(
            "INVALID_PORT",
            "Choose a port above 1023 — lower ports are reserved by the operating system.",
        ));
    }
    {
        let mut settings = state.settings.lock_or_recover();
        settings.capture.bridge_port = port;
        settings
            .save(&state.settings_path())
            .map_err(|e| CommandError::new("CONFIG_SAVE_FAILED", &e.to_string()))?;
    }
    apply_capture_bridge(&app, &state);
    Ok(bridge_status(&state, None))
}

/// Whether Relay analyses each capture as soon as it lands.
#[tauri::command]
pub async fn set_capture_analyze_on_capture(
    app: AppHandle,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<CaptureBridgeStatus, CommandError> {
    {
        let mut settings = state.settings.lock_or_recover();
        settings.capture.analyze_on_capture = enabled;
        settings
            .save(&state.settings_path())
            .map_err(|e| CommandError::new("CONFIG_SAVE_FAILED", &e.to_string()))?;
        let updated = settings.clone();
        drop(settings);
        let _ = app.emit("settings-changed", &updated);
    }
    Ok(bridge_status(&state, None))
}

#[tauri::command]
pub async fn get_captures(state: State<'_, AppState>) -> Result<Vec<VaultFile>, CommandError> {
    state
        .vault
        .list_captures()
        .map_err(|e| CommandError::new("LIST_CAPTURES_FAILED", &e.to_string()))
}

#[tauri::command]
pub async fn get_capture(
    id: String,
    state: State<'_, AppState>,
) -> Result<VaultFile, CommandError> {
    let artifact = state
        .vault
        .get_vault_file(&id)
        .map_err(|e| CommandError::new("CAPTURE_NOT_FOUND", &e.to_string()))?;
    if !artifact.is_capture() {
        return Err(CommandError::new(
            "CAPTURE_NOT_FOUND",
            "That artifact is not a capture.",
        ));
    }
    Ok(artifact)
}

/// Returns the untouched structured payload behind a capture — what the page
/// actually said, before normalization made it readable.
#[tauri::command]
pub async fn get_capture_payload(
    id: String,
    state: State<'_, AppState>,
) -> Result<crate::capture::web::WebCapturePayload, CommandError> {
    state
        .vault
        .get_capture_payload(&id)
        .map_err(|e| CommandError::new("CAPTURE_PAYLOAD_UNAVAILABLE", &e.to_string()))
}

/// Rebuilds a capture's markdown from its stored payload.
#[tauri::command]
pub async fn renormalize_capture(
    id: String,
    state: State<'_, AppState>,
) -> Result<VaultFile, CommandError> {
    state
        .vault
        .renormalize_capture(&id)
        .map_err(|e| CommandError::new("RENORMALIZE_FAILED", &e.to_string()))
}

#[tauri::command]
pub async fn delete_capture(id: String, state: State<'_, AppState>) -> Result<(), CommandError> {
    state
        .vault
        .delete_vault_file(&id)
        .map_err(|e| CommandError::new("DELETE_FAILED", &e.to_string()))
}

/// Ingests a capture payload from inside the app rather than over the bridge.
///
/// Same validation, sanitization and storage as a bridged capture — this is
/// the seam a future non-browser capture source plugs into, and the path an
/// end-to-end test drives without opening a socket.
#[tauri::command]
pub async fn import_web_capture(
    app: AppHandle,
    payload_json: String,
    state: State<'_, AppState>,
) -> Result<VaultFile, CommandError> {
    if payload_json.len() > crate::capture::web::MAX_PAYLOAD_BYTES {
        return Err(CommandError::new(
            "PAYLOAD_TOO_LARGE",
            "That capture is larger than Relay's capture size limit.",
        ));
    }

    let artifact = crate::capture::web::ingest(&state.vault, payload_json.as_bytes())
        .map_err(|e| CommandError::new("CAPTURE_REJECTED", &e.to_string()))?;

    if state.settings.lock_or_recover().capture.analyze_on_capture {
        spawn_capture_analysis(app, artifact.id.clone());
    }
    Ok(artifact)
}

/// Retrieves the derived structured context for a capture, if already analyzed.
#[tauri::command]
pub async fn get_capture_context(
    id: String,
    state: State<'_, AppState>,
) -> Result<Option<crate::capture::web::SourceContext>, CommandError> {
    state
        .vault
        .get_capture_context(&id)
        .map_err(|e| CommandError::new("CONTEXT_UNAVAILABLE", &e.to_string()))
}

/// Analyzes a captured source to extract structured work context.
#[tauri::command]
pub async fn analyze_capture_context(
    id: String,
    state: State<'_, AppState>,
) -> Result<crate::capture::web::SourceContext, CommandError> {
    let file = state
        .vault
        .get_vault_file(&id)
        .map_err(|e| CommandError::new("CAPTURE_NOT_FOUND", &e.to_string()))?;

    let payload = state
        .vault
        .get_capture_payload(&id)
        .map_err(|e| CommandError::new("CAPTURE_PAYLOAD_UNAVAILABLE", &e.to_string()))?;

    let settings = state.settings.lock_or_recover().clone();
    let llm = LLMClient::new(settings.provider);

    // Which analysis runs is decided from the classification capture already
    // derived from the URL and stored on the artifact — not re-derived here by
    // testing the URL for a substring, which matched
    // `https://evil.example/?ref=github.com` and treated every GitHub issue and
    // pull request as a repository.
    let context = crate::capture::web::context::extract_source_context(Some(&llm), &file, &payload).await;

    state
        .vault
        .save_capture_context(&id, &context)
        .map_err(|e| CommandError::new("SAVE_CONTEXT_FAILED", &e.to_string()))?;

    Ok(context)
}

/// Opens the native OS file picker for an AI conversation export archive (.zip or .json).
#[tauri::command]
pub async fn pick_ai_conversation_export_file(app: AppHandle) -> Result<Option<String>, CommandError> {
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Select AI Conversation Export")
            .add_filter("AI Conversation Export (.zip, .json)", &["zip", "json"])
            .blocking_pick_file()
    })
    .await
    .map_err(|e| CommandError::new("DIALOG_TASK_FAILED", &e.to_string()))?;

    picked_path(picked)
}

/// Inspects an exported AI conversation archive (.zip or .json) from ChatGPT or Claude.
#[tauri::command]
pub async fn inspect_ai_conversation_export(
    path: String,
    state: State<'_, AppState>,
) -> Result<crate::capture::web::importer::ExportInspection, CommandError> {
    let p = std::path::PathBuf::from(&path);
    if !p.exists() {
        return Err(CommandError::new("FILE_NOT_FOUND", "Selected export file does not exist"));
    }
    crate::capture::web::importer::inspect_export_file(&p, &state.vault)
}

/// Inspects exported AI conversation archive bytes staged directly from drag-and-drop.
#[tauri::command]
pub async fn inspect_ai_conversation_export_bytes(
    filename: String,
    bytes: Vec<u8>,
    state: State<'_, AppState>,
) -> Result<crate::capture::web::importer::ExportInspection, CommandError> {
    let temp_dir = std::env::temp_dir().join("relay_import_staging");
    let _ = std::fs::create_dir_all(&temp_dir);
    let temp_file = temp_dir.join(format!("{}_{}", uuid::Uuid::new_v4(), filename));
    std::fs::write(&temp_file, &bytes)
        .map_err(|e| CommandError::new("TEMP_FILE_WRITE_FAILED", &e.to_string()))?;

    let res = crate::capture::web::importer::inspect_export_file(&temp_file, &state.vault);
    let _ = std::fs::remove_file(temp_file);
    res
}

/// Imports a chosen conversation from an AI export archive into Relay's vault.
#[tauri::command]
pub async fn import_ai_conversation_export(
    path: String,
    conversation_id: String,
    duplicate_mode: Option<String>,
    state: State<'_, AppState>,
) -> Result<crate::vault::VaultFile, CommandError> {
    let p = std::path::PathBuf::from(&path);
    if !p.exists() {
        return Err(CommandError::new("FILE_NOT_FOUND", "Selected export file does not exist"));
    }
    let settings = state.settings.lock_or_recover().clone();
    crate::capture::web::importer::import_export_conversation(
        &p,
        &conversation_id,
        duplicate_mode.as_deref(),
        &state.vault,
        &settings,
    )
    .await
}

/// Imports a chosen conversation from staged export bytes into Relay's vault.
#[tauri::command]
pub async fn import_ai_conversation_export_bytes(
    filename: String,
    bytes: Vec<u8>,
    conversation_id: String,
    duplicate_mode: Option<String>,
    state: State<'_, AppState>,
) -> Result<crate::vault::VaultFile, CommandError> {
    let temp_dir = std::env::temp_dir().join("relay_import_staging");
    let _ = std::fs::create_dir_all(&temp_dir);
    let temp_file = temp_dir.join(format!("{}_{}", uuid::Uuid::new_v4(), filename));
    std::fs::write(&temp_file, &bytes)
        .map_err(|e| CommandError::new("TEMP_FILE_WRITE_FAILED", &e.to_string()))?;

    let settings = state.settings.lock_or_recover().clone();
    let res = crate::capture::web::importer::import_export_conversation(
        &temp_file,
        &conversation_id,
        duplicate_mode.as_deref(),
        &state.vault,
        &settings,
    )
    .await;

    let _ = std::fs::remove_file(temp_file);
    res
}

/// Validates that a string is a safe HTTP or HTTPS URL before delegating to the OS.
pub fn validate_external_url(raw: &str) -> Result<String, &'static str> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("URL cannot be empty.");
    }
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return Err("Only HTTP and HTTPS URLs can be opened.");
    }
    if trimmed.chars().any(|c| c.is_control()) {
        return Err("URL contains invalid control characters.");
    }
    Ok(trimmed.to_string())
}

/// Opens a validated HTTP or HTTPS URL in the user's default OS browser.
#[tauri::command]
pub async fn open_external_url(url: String) -> Result<(), CommandError> {
    let validated = validate_external_url(&url)
        .map_err(|e| CommandError::new("INVALID_URL", e))?;

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", &validated])
            .spawn()
            .map_err(|e| CommandError::new("OPEN_URL_FAILED", &e.to_string()))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&validated)
            .spawn()
            .map_err(|e| CommandError::new("OPEN_URL_FAILED", &e.to_string()))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&validated)
            .spawn()
            .map_err(|e| CommandError::new("OPEN_URL_FAILED", &e.to_string()))?;
    }

    Ok(())
}

// ── Foundation Roadmap 11-20 Commands ────────────────────────────────────────

#[tauri::command]
pub async fn unified_retrieve(
    query: crate::retrieval::RetrievalQuery,
    state: State<'_, AppState>,
) -> Result<crate::retrieval::RetrievalResult, CommandError> {
    Ok(crate::retrieval::UnifiedRetrievalService::search(
        &state.vault,
        Some(&state.meetings_v2.store()),
        Some(&state.meeting_processor),
        &query,
    ))
}

#[tauri::command]
pub async fn assemble_context_pack(
    query: String,
    pack_type: Option<String>,
    char_budget: Option<usize>,
    state: State<'_, AppState>,
) -> Result<crate::context::ContextPack, CommandError> {
    let pt = pack_type.map(|t| match t.to_lowercase().as_str() {
        "repository" => crate::context::ContextPackType::Repository,
        "meeting" => crate::context::ContextPackType::Meeting,
        "project" => crate::context::ContextPackType::Project,
        "conversation" => crate::context::ContextPackType::Conversation,
        "document" => crate::context::ContextPackType::Document,
        _ => crate::context::ContextPackType::General,
    });

    let mut req = crate::context::ContextAssemblyRequest::new(&query);
    if let Some(t) = pt {
        req = req.with_pack_type(t);
    }
    if let Some(b) = char_budget {
        req = req.with_char_budget(b);
    }

    Ok(crate::context::ContextAssemblyService::assemble(
        &state.vault,
        Some(&state.memory_store),
        Some(&state.relationship_store),
        &req,
    ))
}

#[tauri::command]
pub async fn list_memories(
    memory_type: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<crate::memory::MemoryItem>, CommandError> {
    let mt = memory_type.and_then(|t| match t.to_lowercase().as_str() {
        "fact" => Some(crate::memory::MemoryType::Fact),
        "preference" => Some(crate::memory::MemoryType::Preference),
        "decision" => Some(crate::memory::MemoryType::Decision),
        "project_context" => Some(crate::memory::MemoryType::ProjectContext),
        "relationship" => Some(crate::memory::MemoryType::Relationship),
        "instruction" => Some(crate::memory::MemoryType::Instruction),
        _ => None,
    });
    Ok(state.memory_store.list_active(mt))
}

#[tauri::command]
pub async fn create_memory(
    memory_type: String,
    subject: String,
    content: String,
    source_id: String,
    evidence: String,
    state: State<'_, AppState>,
) -> Result<crate::memory::MemoryItem, CommandError> {
    let mt = match memory_type.to_lowercase().as_str() {
        "preference" => crate::memory::MemoryType::Preference,
        "decision" => crate::memory::MemoryType::Decision,
        "project_context" => crate::memory::MemoryType::ProjectContext,
        "relationship" => crate::memory::MemoryType::Relationship,
        "instruction" => crate::memory::MemoryType::Instruction,
        _ => crate::memory::MemoryType::Fact,
    };
    let prov = crate::memory::MemoryProvenance {
        source_id,
        source_type: "manual".to_string(),
        evidence,
        confidence: 1.0,
        extracted_by: "user".to_string(),
    };
    let item = crate::memory::MemoryItem::new(mt, subject, content, prov);
    state.memory_store.create_memory(item).map_err(|e| CommandError::new("MEMORY_ERROR", &e))
}

#[tauri::command]
pub async fn supersede_memory(
    old_id: String,
    new_content: String,
    source_id: String,
    evidence: String,
    state: State<'_, AppState>,
) -> Result<crate::memory::MemoryItem, CommandError> {
    let prov = crate::memory::MemoryProvenance {
        source_id,
        source_type: "update".to_string(),
        evidence,
        confidence: 1.0,
        extracted_by: "user".to_string(),
    };
    let (_old, new_mem) = state
        .memory_store
        .supersede_memory(&old_id, &new_content, prov)
        .map_err(|e| CommandError::new("MEMORY_ERROR", &e))?;
    Ok(new_mem)
}

#[tauri::command]
pub async fn extract_and_resolve_entities(
    source_id: String,
    content: String,
) -> Result<Vec<crate::entities::ResolvedEntity>, CommandError> {
    let extracted = crate::entities::EntityExtractor::extract_deterministic(&source_id, &content);
    Ok(crate::entities::EntityResolver::resolve(&extracted))
}

#[tauri::command]
pub async fn dispatch_universal_action(
    mut action: crate::actions::UniversalAction,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, CommandError> {
    crate::actions::ActionDispatcher::execute(&mut action, confirmed, Some(&state.vault))
        .map_err(|e| CommandError::new("ACTION_ERROR", &e))
}

#[tauri::command]
pub async fn list_relationships(
    source_id: Option<String>,
    target_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<crate::relationships::RelationshipRecord>, CommandError> {
    if let Some(s) = source_id {
        Ok(state.relationship_store.get_relationships_for_source(&s))
    } else if let Some(t) = target_id {
        Ok(state.relationship_store.get_relationships_for_target(&t))
    } else {
        Ok(state.relationship_store.list_all())
    }
}

#[tauri::command]
pub async fn add_relationship(
    source_id: String,
    target_id: String,
    relationship_type: String,
    state: State<'_, AppState>,
) -> Result<crate::relationships::RelationshipRecord, CommandError> {
    let rt = crate::relationships::RelationshipType::from_str_opt(&relationship_type)
        .ok_or_else(|| CommandError::new("INVALID_INPUT", &format!("Unknown relationship type: {}", relationship_type)))?;
    let rel = crate::relationships::RelationshipRecord::new(source_id, target_id, rt)
        .map_err(|e| CommandError::new("INVALID_INPUT", &e))?;
    state.relationship_store.add_relationship(rel.clone())
        .map_err(|e| CommandError::new("RELATIONSHIP_ERROR", &e))?;
    Ok(rel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_external_url_accepts_valid_http_and_https_urls() {
        assert_eq!(
            validate_external_url("https://github.com/stablyai/orca").unwrap(),
            "https://github.com/stablyai/orca"
        );
        assert_eq!(
            validate_external_url("  http://localhost:3000/path?query=1#hash  ").unwrap(),
            "http://localhost:3000/path?query=1#hash"
        );
    }

    #[test]
    fn validate_external_url_refuses_unsafe_schemes_and_control_characters() {
        assert!(validate_external_url("").is_err());
        assert!(validate_external_url("   ").is_err());
        assert!(validate_external_url("file:///C:/secrets.txt").is_err());
        assert!(validate_external_url("javascript:alert(1)").is_err());
        assert!(validate_external_url("data:text/html,hello").is_err());
        assert!(validate_external_url("https://example.com\n\revil").is_err());
    }
}
