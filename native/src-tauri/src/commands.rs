use crate::capture::{AudioRecorder, SttEngine};
use crate::hotkeys;
use crate::pipeline::{PipelineEngine, ProcessedPipelineResult};
use crate::providers::{LLMClient, OllamaStatus, ProviderType};
use crate::settings::{AppSettings, HotkeySettings, PillPosition};
use crate::triggers::{TriggerConfig, TriggerEngine};
use crate::vault::{
    GraphFilter, KanbanCard, KnowledgeGraphData, KnowledgeSearchResult,
    Scribble, ScribbleRelationship, TrashItem, VaultManager, VaultNote,
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
    let mut guard = state.last_stt_diagnostics.lock().unwrap();
    *guard = Some(snapshot.clone());
    let _ = app.emit(STT_DIAGNOSTICS_EVENT, &snapshot);
}

#[tauri::command]
pub async fn get_last_stt_diagnostics(
    state: State<'_, AppState>,
) -> Result<Option<crate::capture::SttDiagnosticSnapshot>, CommandError> {
    let guard = state.last_stt_diagnostics.lock().unwrap();
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
    hotkeys::apply_hotkeys(&app, &hotkeys.show_hide_hotkey, &hotkeys.dictation_hotkey)
        .map_err(|e| CommandError::new("HOTKEY_REGISTER_FAILED", &e))?;

    let mut settings = state.settings.lock().unwrap();
    settings.hotkeys = hotkeys;
    settings
        .save(&state.settings_path())
        .map_err(|e| CommandError::new("CONFIG_SAVE_FAILED", &e.to_string()))
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
    let mut settings = state.settings.lock().unwrap();
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
    let position = state.settings.lock().unwrap().ui.pill_position;
    crate::overlay::set_expanded(&app, expanded, position);
    Ok(())
}

#[tauri::command]
pub async fn set_pill_window_mode(
    app: AppHandle,
    mode: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    let position = state.settings.lock().unwrap().ui.pill_position;
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
        let provider = state.settings.lock().unwrap().provider.clone();
        if matches!(provider.active_provider, ProviderType::Ollama) {
            tauri::async_runtime::spawn(async move {
                crate::providers::ensure_ollama_ready(&provider.ollama_host, &provider.ollama_model).await;
            });
        }

        let has_model = state
            .settings
            .lock()
            .unwrap()
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
            // trigger-matching or the note/kanban/chat pipeline on nothing.
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
    let settings = state.settings.lock().unwrap().clone();
    let stt = state.stt.clone();
    let samples = captured.samples.clone();

    // Nobody should have to go find and download a GGML file themselves
    let models_dir = state.config_dir.join("models");
    let model_path = match settings
        .stt
        .whisper_model_path
        .clone()
        .filter(|p| !p.trim().is_empty())
    {
        Some(configured) => {
            let path = std::path::Path::new(&configured);
            if crate::capture::stt::is_legacy_default_model(path) {
                // Promote legacy default model (e.g. ggml-base.bin) to production ggml-small.bin
                match crate::capture::stt::ensure_default_model(&models_dir).await {
                    Ok(small_path) => {
                        let path_str = small_path.to_string_lossy().to_string();
                        let mut guard = state.settings.lock().unwrap();
                        guard.stt.whisper_model_path = Some(path_str.clone());
                        let _ = guard.save(&state.settings_path());
                        Some(path_str)
                    }
                    Err(_) => Some(configured),
                }
            } else {
                Some(configured)
            }
        }
        None => {
            match crate::capture::stt::ensure_default_model(&models_dir).await {
                Ok(path) => {
                    let path_str = path.to_string_lossy().to_string();
                    let mut guard = state.settings.lock().unwrap();
                    guard.stt.whisper_model_path = Some(path_str.clone());
                    let _ = guard.save(&state.settings_path());
                    Some(path_str)
                }
                Err(e) => {
                    tracing::warn!("Could not auto-provision production Whisper model: {}", e);
                    None
                }
            }
        }
    };

    let language_config = crate::capture::SttLanguageConfig::from_settings(&settings.language);
    let decoding_config = crate::capture::stt::WhisperDecodingConfig::from_settings(&settings.stt);

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
    // reach the note/kanban/chat pipeline.
    if transcript.trim().is_empty() {
        return Ok(None);
    }

    // Every successful, non-empty transcript becomes a Voice Note — this
    // must not depend on which mode-specific pipeline runs next, or on
    // whether it succeeds. "chat" is Voice Chat (a deferred, unrelated
    // feature per docs/decisions.md Decision 34) answering a spoken
    // question, not a dictation to keep — excluded so voice chat queries
    // don't clutter the Voice Note history.
    if captured.mode != "chat" {
        save_voice_note(app, &state.vault, &transcript);
    }

    let llm = LLMClient::new(settings.provider.clone());

    match captured.mode.as_str() {
        "chat" => crate::pipeline::process_chat(&llm, &state.vault, &settings.tts, &transcript)
            .await
            .map(Some)
            .map_err(|e| CommandError::new("PIPELINE_ERROR", &e.to_string())),
        _ => PipelineEngine::process_scribble(&llm, &state.vault, &transcript)
            .await
            .map(Some)
            .map_err(|e| CommandError::new("PIPELINE_ERROR", &e.to_string())),
    }
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

pub const SCRIBBLE_SAVED_EVENT: &str = "scribble-saved";
pub const SCRIBBLE_ENRICHED_EVENT: &str = "scribble-enriched";

pub fn spawn_scribble_enrichment(
    app: AppHandle,
    state: &AppState,
    scribble_id: String,
) {
    let settings = state.settings.lock().unwrap().clone();
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
    let settings = state.settings.lock().unwrap().clone();
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
    let settings = state.settings.lock().unwrap().clone();
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
    let configured = state.settings.lock().unwrap().vault.directory.is_some();
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
    let mut settings = state.settings.lock().unwrap();
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
        .lock()
        .unwrap()
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
            let mut settings = state.settings.lock().unwrap();
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
    let settings = state.settings.lock().unwrap().clone();
    if !matches!(settings.provider.active_provider, ProviderType::Ollama) {
        return Ok(OllamaStatus::Running);
    }
    Ok(crate::providers::ensure_ollama_ready(&settings.provider.ollama_host, &settings.provider.ollama_model).await)
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, CommandError> {
    Ok(state.settings.lock().unwrap().clone())
}

#[tauri::command]
pub async fn save_settings(
    app: AppHandle,
    settings: AppSettings,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    settings
        .save(&state.settings_path())
        .map_err(|e| CommandError::new("CONFIG_SAVE_FAILED", &e.to_string()))?;
    *state.settings.lock().unwrap() = settings.clone();
    let _ = app.emit("settings-changed", &settings);
    Ok(())
}

#[tauri::command]
pub async fn open_settings_window(app: AppHandle) -> Result<(), CommandError> {
    if let Some(window) = app.get_webview_window(crate::hotkeys::MAIN_WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
        let _ = app.emit("navigate-tab", "settings");
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

        if trimmed.starts_with("## [") {
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

            let rest = &trimmed[4..];
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

        if trimmed.starts_with("### ") {
            if let Some(entry) = current_entry.as_mut() {
                if entry.title.is_empty() {
                    entry.title = trimmed[4..].trim().to_string();
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
    if content.starts_with("**") {
        if let Some(end_bold) = content[2..].find("**") {
            let bold_text = &content[2..2 + end_bold];
            let after_bold = &content[2 + end_bold + 2..];
            let text = after_bold.trim_start_matches(':').trim().to_string();

            if let Some(open_p) = bold_text.find('(') {
                if let Some(close_p) = bold_text.find(')') {
                    let cat = bold_text[..open_p].trim().to_string();
                    let dom_raw = bold_text[open_p + 1..close_p].trim().trim_matches('`');
                    let dom = if dom_raw.contains('/') || dom_raw.contains('\\') {
                        dom_raw.rsplit_once(|c| c == '/' || c == '\\').map(|(_, f)| f).unwrap_or(dom_raw)
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
    let settings = state.settings.lock().unwrap().clone();
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
        .map(|p| p.split(['/', '\\']).last().unwrap_or(p))
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
    let settings = state.settings.lock().unwrap().clone();
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
    let settings = state.settings.lock().unwrap().clone();
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
    let settings = state.settings.lock().unwrap().clone();
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

    let settings = state.settings.lock().unwrap().clone();
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

    let settings = state.settings.lock().unwrap().clone();
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
    let mut settings = state.settings.lock().unwrap();
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
    let mut settings = state.settings.lock().unwrap();
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
    let settings = state.settings.lock().unwrap().clone();
    let language_config = crate::capture::SttLanguageConfig::from_settings(&settings.language);
    let decoding_config = crate::capture::stt::WhisperDecodingConfig::from_settings(&settings.stt);
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

    result.map_err(|e: String| CommandError::new("STOP_MEETING_FAILED", &e))
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

#[tauri::command]
pub async fn summarize_meeting_v2(
    app: AppHandle,
    session_id: String,
    state: State<'_, AppState>,
) -> Result<crate::meetings_v2::MeetingSession, CommandError> {
    let settings = state.settings.lock().unwrap().clone();
    let llm = LLMClient::new(settings.provider);

    let updated = crate::pipeline::summarize_meeting(&llm, &state.meetings_v2.store(), &session_id)
        .await
        .map_err(|e| CommandError::new("SUMMARIZE_MEETING_FAILED", &e))?;

    let _ = app.emit("meeting-session-state-changed", &updated);
    Ok(updated)
}





