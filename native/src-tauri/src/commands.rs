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
    /// What the last dictation put into a field, and where.
    ///
    /// Held so the cleanup offered afterwards can select exactly that text and
    /// replace it. Cleared on the next dictation, because the offer is only
    /// ever about the most recent one.
    pub last_dictation: Mutex<Option<LastDictation>>,
    /// The loopback listener the Relay browser extension posts captures to.
    /// `None` whenever capture is switched off, which is the default.
    pub capture_bridge: Mutex<Option<crate::capture::web::bridge::BridgeHandle>>,
    pub memory_store: Arc<crate::memory::MemoryStore>,
    pub relationship_store: Arc<crate::relationships::RelationshipStore>,
    pub entity_store: Arc<crate::entities::EntityStore>,
}

impl AppState {
    fn settings_path(&self) -> PathBuf {
        self.config_dir.join("settings.json")
    }
}

/// The dictation a cleanup may still replace.
#[derive(Debug, Clone)]
pub struct LastDictation {
    /// Exactly the text that was injected — not the transcript before
    /// normalization, because what has to be selected back is what landed.
    pub text: String,
    /// Where it landed. A cleanup that cannot prove the caret is still in the
    /// same place does not touch the field.
    pub focus: Option<crate::hotkeys::injection::TargetFocusContext>,
}

pub fn record_stt_diagnostics(
    app: &AppHandle,
    state: &AppState,
    snapshot: crate::capture::SttDiagnosticSnapshot,
) {
    // The full snapshot stays in memory for the panel showing the last run.
    // A reduced, transcript-free record is retained on disk, because the
    // questions worth asking of this data — are input levels low enough to
    // want a gain stage, does pinning the language starve decodes — are about
    // the distribution across runs and cannot be answered from the last one.
    crate::capture::decode_history::append(
        &state.config_dir,
        &crate::capture::decode_history::DecodeRecord::from_snapshot(&snapshot),
    );

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

/// What the retained decode history says.
///
/// Deliberately a summary rather than the rows: the decision this exists to
/// support is "are levels low" or "does pinning starve decodes", and a caller
/// that has to compute percentiles itself will compute them differently from
/// the next caller.
#[tauri::command]
pub async fn get_stt_decode_summary(
    state: State<'_, AppState>,
) -> Result<crate::capture::decode_history::DecodeSummary, CommandError> {
    let records = crate::capture::decode_history::load(&state.config_dir);
    Ok(crate::capture::decode_history::summarize(&records))
}

/// Proposes a cleanup of dictated text, with the diff that makes it reviewable.
///
/// Returns a proposal rather than replacing anything. The decision to accept it
/// is the user's, made against the diff — which is the entire reason this layer
/// is allowed to exist (Decision 65 for where it may not be used, and
/// `capture::rewrite` for why a diff rather than a more careful prompt).
#[tauri::command]
pub async fn rewrite_dictation(
    state: State<'_, AppState>,
    text: String,
    style: Option<String>,
) -> Result<crate::capture::rewrite::RewriteProposal, CommandError> {
    let (provider, configured, enabled) = {
        let settings = state.settings.lock_or_recover();
        (
            settings.provider.clone(),
            settings.stt.cleanup_style.clone(),
            settings.stt.text_transform,
        )
    };

    // The setting decides, not the caller. The pill hides the button when this
    // is off, and gating only there would leave `text_transform` as a
    // preference nothing enforces — which is how a setting comes to mean
    // whatever the newest call site assumed.
    if !enabled {
        return Ok(crate::capture::rewrite::RewriteProposal::unchanged(text));
    }
    let style = crate::capture::rewrite::CleanupStyle::from_setting(
        style.as_deref().unwrap_or(&configured),
    );
    let client = crate::providers::LLMClient::new(provider);
    Ok(crate::capture::rewrite::propose(&client, &text, style).await)
}

/// Whether a cleanup can still be offered for the last dictation, and of what.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CleanupTarget {
    pub available: bool,
    pub text: String,
}

/// The dictation a cleanup would act on, if any.
#[tauri::command]
pub async fn get_cleanup_target(
    state: State<'_, AppState>,
) -> Result<CleanupTarget, CommandError> {
    let guard = state.last_dictation.lock_or_recover();
    Ok(match guard.as_ref() {
        Some(last) => CleanupTarget {
            available: !last.text.trim().is_empty(),
            text: last.text.clone(),
        },
        None => CleanupTarget {
            available: false,
            text: String::new(),
        },
    })
}

/// How an apply ended, in the terms the pill has to render.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CleanupApplied {
    /// The field now holds the cleaned text.
    Replaced,
    /// The caret is no longer where the dictation landed, so nothing was
    /// touched. The cleaned text is on the clipboard instead.
    Moved { message: String },
    Failed { message: String },
}

/// Replaces the last dictation with its cleaned form.
///
/// Selects exactly what Relay injected and types over it. Three things make
/// that safe enough to do to somebody's document:
///
/// * The selection is counted in cursor steps, not characters
///   (`text_normalize::cursor_steps`), so a Devanagari cluster is one step and
///   the selection cannot run past what was written.
/// * The focus context is compared against the one captured at injection. If
///   the user has moved to another window or tab, nothing is selected and
///   nothing is typed.
/// * The target is consumed. A second apply has nothing to act on, so a
///   double-press cannot delete a second helping of the user's text.
#[tauri::command]
pub async fn apply_dictation_cleanup(
    state: State<'_, AppState>,
    cleaned: String,
) -> Result<CleanupApplied, CommandError> {
    use crate::hotkeys::injection;

    // Taken, not read: whatever happens next, this dictation is no longer a
    // thing a later press may replace.
    let Some(last) = state.last_dictation.lock_or_recover().take() else {
        return Ok(CleanupApplied::Failed {
            message: "There is no recent dictation to clean up.".to_string(),
        });
    };

    let still_there = injection::focus_unchanged(
        last.focus.as_ref(),
        injection::capture_target_focus_context().as_ref(),
    );

    if !still_there {
        let _ = injection::copy_to_clipboard(&cleaned);
        return Ok(CleanupApplied::Moved {
            message: "You've moved since dictating, so nothing was changed. The cleaned text is on the clipboard.".to_string(),
        });
    }

    let steps = crate::capture::text_normalize::cursor_steps(&last.text);
    if let Err(e) = injection::select_previous(steps) {
        let _ = injection::copy_to_clipboard(&cleaned);
        return Ok(CleanupApplied::Failed {
            message: format!("Could not select the dictated text ({e}). It is on the clipboard."),
        });
    }

    let method = state.settings.lock_or_recover().clipboard.injection_method;
    match injection::inject_text(&cleaned, method) {
        Ok(()) => Ok(CleanupApplied::Replaced),
        Err(e) => {
            // The selection is still live and the replacement did not land.
            // The clipboard is the recovery path, and saying so is the whole
            // of the contract the injection branches already keep.
            let _ = injection::copy_to_clipboard(&cleaned);
            Ok(CleanupApplied::Failed {
                message: format!("Could not type the cleaned text ({e}). It is on the clipboard."),
            })
        }
    }
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

    let language_config = crate::capture::SttLanguageConfig::from_settings(&settings.language, crate::capture::stt::SttWindow::ShortForm);
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

    // The deterministic cleanup meetings have always had, now on this path
    // too: bracketed ASR tags removed, decoder stutters collapsed, isolated
    // fillers dropped, and the user's own dictionary applied by edit distance.
    // No model runs, so nothing here can invent a word that was not spoken.
    //
    // The `Dictated` profile leaves sentence boundaries alone. This text is
    // going into whatever field has focus, and a period Relay appended is a
    // period the user has to delete.
    let transcript = crate::capture::text_normalize::normalize_text(
        &transcript,
        crate::capture::text_normalize::Vocabulary::new(
            &settings.dictionary,
            &settings.vocabulary_corrections,
        ),
        crate::capture::text_normalize::TextProfile::Dictated,
    )
    .text;

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
    // whether it succeeds.
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

/// A correction that was applied, and what it takes to undo it.
#[derive(Debug, Clone, Serialize)]
pub struct PhraseCorrectionResult {
    pub note: VaultNote,
    /// The correction as it was written to the note's history. Undo is
    /// `undo_voice_note_correction`, which reverses this record — which is why
    /// no versioning system is needed for it.
    pub record: crate::vault::CorrectionRecord,
    /// True when the phrase was also added to the learned vocabulary.
    pub learned: bool,
}

/// Corrects one selected phrase inside a Voice Note.
///
/// A deterministic range edit, saved through the same `update_note_content`
/// the full editor uses — so this is a second way into the existing
/// persistence, not a second persistence. No model is called: the user has
/// already said what the text should be.
///
/// `start` and `end` are character offsets into the note's current content,
/// and `original` is what the caller believes is there. The edit refuses
/// rather than applying stale offsets to changed content.
///
/// `learn` is opt-in per correction. Most corrections are ordinary edits —
/// "Thursday" to "Tuesday" is not vocabulary — so nothing is added to the
/// learned list unless the user ticks the box.
// A Tauri command's parameters are the IPC payload's fields — they are flat by
// construction, and grouping them would change the frontend-facing contract.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn correct_voice_note_phrase(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    start: usize,
    end: usize,
    original: String,
    replacement: String,
    learn: Option<bool>,
) -> Result<PhraseCorrectionResult, CommandError> {
    if replacement.trim().is_empty() {
        return Err(CommandError::new(
            "CORRECTION_EMPTY",
            "A correction needs a replacement.",
        ));
    }

    let note = state
        .vault
        .get_note(&id)
        .map_err(|e| CommandError::new("VAULT_READ_FAILED", &e.to_string()))?;

    let corrected =
        crate::vault::correction::replace_range(&note.content, start, end, &original, &replacement)
            .map_err(|e| CommandError::new("CORRECTION_FAILED", &e.to_string()))?;

    let note = state
        .vault
        .update_note_content(&id, &corrected)
        .map_err(|e| CommandError::new("VAULT_UPDATE_FAILED", &e.to_string()))?;

    // Learning first, so the record can say truthfully whether it happened.
    let learned = if learn.unwrap_or(false) {
        let mut settings = state.settings.lock_or_recover();
        let added = settings.learn_correction(&original, &replacement);
        if added {
            let _ = settings.save(&state.settings_path());
            let updated = settings.clone();
            drop(settings);
            // The Settings window is a separate window holding its own copy,
            // and `save_settings` writes that copy whole. Without this it
            // would overwrite the rule the moment anything else there is
            // changed.
            let _ = app.emit("settings-changed", &updated);
        }
        added
    } else {
        false
    };

    let record = crate::vault::CorrectionRecord::new(&id, &original, &replacement, start, learned);
    // A history that cannot be written costs the undo, not the correction the
    // user already sees applied — so it is logged rather than surfaced.
    if let Err(e) = state.vault.record_correction(&record) {
        tracing::warn!("Could not record correction history for {id}: {e}");
    }

    Ok(PhraseCorrectionResult {
        note,
        record,
        learned,
    })
}

/// Reverses the most recent phrase correction on a Voice Note.
///
/// Puts the original phrase back where the replacement now sits, rather than
/// restoring a snapshot of the whole note: a snapshot undo applied after the
/// full editor touched the note would throw that edit away. If the replacement
/// is no longer there, this refuses.
#[tauri::command]
pub async fn undo_voice_note_correction(
    state: State<'_, AppState>,
    id: String,
) -> Result<VaultNote, CommandError> {
    let record = state
        .vault
        .correction_history(&id)
        .map_err(|e| CommandError::new("VAULT_READ_FAILED", &e.to_string()))?
        .pop()
        .ok_or_else(|| {
            CommandError::new("NO_CORRECTION_TO_UNDO", "This note has no correction to undo.")
        })?;

    let note = state
        .vault
        .get_note(&id)
        .map_err(|e| CommandError::new("VAULT_READ_FAILED", &e.to_string()))?;

    let restored = record
        .reverse(&note.content)
        .map_err(|e| CommandError::new("UNDO_FAILED", &e.to_string()))?;

    let note = state
        .vault
        .update_note_content(&id, &restored)
        .map_err(|e| CommandError::new("VAULT_UPDATE_FAILED", &e.to_string()))?;

    // Only once the note is safely written: a correction that is still applied
    // must stay in the history, or the next undo reverses the wrong edit.
    let _ = state.vault.pop_correction(&id);

    // A learned rule is deliberately left in place. It is a standing statement
    // about the phrase, made on purpose through a separate checkbox, and
    // Settings › Dictionary is where it is turned off or removed.
    Ok(note)
}

/// Adds a phrase to the user's dictionary, from wherever they found it.
///
/// The same list Settings › Dictionary edits, reached from a Voice Note: a
/// spelling that is already right and that Relay should keep getting right,
/// used to prime the recognizer before it guesses. Distinct from a learned
/// correction, which repairs a guess already made.
#[tauri::command]
pub async fn add_dictionary_word(
    app: AppHandle,
    state: State<'_, AppState>,
    word: String,
) -> Result<Vec<String>, CommandError> {
    if word.trim().is_empty() {
        return Err(CommandError::new(
            "DICTIONARY_WORD_EMPTY",
            "A dictionary entry needs a word.",
        ));
    }

    let mut settings = state.settings.lock_or_recover();
    if !settings.add_dictionary_word(&word) {
        // Already known. Saying so is the honest answer, and rewriting the
        // file to change nothing is not.
        return Ok(settings.dictionary.clone());
    }
    settings
        .save(&state.settings_path())
        .map_err(|e| CommandError::new("CONFIG_SAVE_FAILED", &e.to_string()))?;

    let updated = settings.clone();
    drop(settings);
    // Same reason as above: the Settings window holds its own copy.
    let _ = app.emit("settings-changed", &updated);
    Ok(updated.dictionary)
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

/// Downloads a managed Whisper model the user asked for.
///
/// Separate from [`ensure_stt_model_ready`], which fetches the default so a
/// first capture works at all. This one is a deliberate choice: the accuracy
/// ceiling is ~1.6 GB and costs real decode time, so it is never fetched
/// implicitly and never becomes the active model as a side effect of being
/// downloaded. Selecting it stays the user's separate act.
///
/// Reports a failed download as `Failed` rather than as a command error, the
/// same way `ensure_stt_model_ready` does, so Settings can render the reason
/// instead of a toast with a stack in it.
#[tauri::command]
pub async fn download_stt_model(
    state: State<'_, AppState>,
    filename: String,
) -> Result<SttModelStatus, CommandError> {
    let models_dir = state.config_dir.join("models");
    match crate::capture::stt::ensure_managed_model(&models_dir, &filename).await {
        Ok(path) => Ok(SttModelStatus::Ready {
            path: path.to_string_lossy().to_string(),
        }),
        Err(e) => Ok(SttModelStatus::Failed {
            message: e.to_string(),
        }),
    }
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
    crate::capture::device::set_preference(&settings.audio_input);
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

    // Same reason, for the OS launch entry. `start_minimized` has no
    // counterpart here on purpose — it is a decision taken at launch, and the
    // switch says so.
    crate::startup::reconcile_launch_at_login(&app, settings.startup.launch_at_login);

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

fn picked_path(
    picked: Option<tauri_plugin_dialog::FilePath>,
) -> Result<Option<String>, CommandError> {
    match picked {
        Some(file_path) => file_path
            .into_path()
            .map(|p| Some(p.to_string_lossy().to_string()))
            .map_err(|e| CommandError::new("DIALOG_PATH_INVALID", &e.to_string())),
        None => Ok(None),
    }
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
    Ok(crate::retrieval::UnifiedRetrievalService::search_with_memory(
        &state.vault,
        Some(&state.memory_store),
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

    Ok(crate::context::ContextAssemblyService::assemble_full(
        &state.vault,
        Some(&state.memory_store),
        Some(&state.relationship_store),
        Some(&state.entity_store),
        &req,
    ))
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KnowledgeTelemetrySnapshot {
    pub total_memories: usize,
    pub active_memories: usize,
    pub total_entities: usize,
    pub total_relationships: usize,
    pub total_scribbles: usize,
    pub total_notes: usize,
    pub total_files: usize,
    pub total_captures: usize,
}

#[tauri::command]
pub async fn get_knowledge_telemetry(
    state: State<'_, AppState>,
) -> Result<KnowledgeTelemetrySnapshot, CommandError> {
    let active_mems = state.memory_store.list_active(None).len();
    let all_rels = state.relationship_store.list_all().len();
    let all_ents = state.entity_store.list_all().len();
    let scribbles = state.vault.list_scribbles().map(|v| v.len()).unwrap_or(0);
    let notes = state.vault.list_notes().map(|v| v.len()).unwrap_or(0);
    let (files, caps) = state.vault.list_vault_files()
        .map(|all| {
            let caps = all.iter().filter(|f| f.is_capture()).count();
            let files = all.len() - caps;
            (files, caps)
        })
        .unwrap_or((0, 0));

    Ok(KnowledgeTelemetrySnapshot {
        total_memories: active_mems,
        active_memories: active_mems,
        total_entities: all_ents,
        total_relationships: all_rels,
        total_scribbles: scribbles,
        total_notes: notes,
        total_files: files,
        total_captures: caps,
    })
}

#[tauri::command]
pub async fn form_memory_candidate(
    candidate: crate::memory::CandidateMemory,
    state: State<'_, AppState>,
) -> Result<crate::memory::MemoryFormationOutcome, CommandError> {
    crate::memory::MemoryFormationService::process_candidate(&state.memory_store, candidate)
        .map_err(|e| CommandError::new("MEMORY_ERROR", &e))
}

#[tauri::command]
pub async fn list_entities(
    category: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<crate::entities::ResolvedEntity>, CommandError> {
    if let Some(cat_str) = category {
        if let Some(cat) = crate::entities::EntityCategory::from_str_opt(&cat_str) {
            return Ok(state.entity_store.list_by_category(cat));
        }
    }
    Ok(state.entity_store.list_all())
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
