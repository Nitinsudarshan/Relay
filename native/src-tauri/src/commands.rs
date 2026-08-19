use crate::capture::{AudioRecorder, SttEngine};
use crate::hotkeys;
use crate::mcp::McpRouter;
use crate::pipeline::{PipelineEngine, ProcessedPipelineResult};
use crate::providers::{LLMClient, OllamaStatus, ProviderType};
use crate::settings::{AppSettings, HotkeySettings};
use crate::triggers::{TriggerConfig, TriggerEngine};
use crate::vault::{KanbanCard, VaultManager};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};

/// Broadcast to every window whenever the shared microphone session starts
/// or stops, so any surface (main window, floating pill, indicator) can
/// reflect the true backend state instead of guessing from its own clicks.
pub const CAPTURE_STATE_EVENT: &str = "capture-state-changed";

#[derive(Debug, Clone, Serialize)]
pub struct CaptureStatus {
    pub active: bool,
    pub mode: Option<String>,
}

pub fn emit_capture_state(app: &AppHandle, recorder: &AudioRecorder) {
    let status = CaptureStatus {
        active: recorder.is_active(),
        mode: recorder.active_mode(),
    };
    let _ = app.emit(CAPTURE_STATE_EVENT, status);
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

pub struct AppState {
    pub recorder: AudioRecorder,
    pub vault: VaultManager,
    pub config_dir: PathBuf,
    pub settings: Mutex<AppSettings>,
    pub stt: SttEngine,
}

impl AppState {
    fn settings_path(&self) -> PathBuf {
        self.config_dir.join("settings.json")
    }
}

#[tauri::command]
pub async fn get_capture_status(state: State<'_, AppState>) -> Result<CaptureStatus, CommandError> {
    Ok(CaptureStatus {
        active: state.recorder.is_active(),
        mode: state.recorder.active_mode(),
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

#[tauri::command]
pub async fn set_pill_visible(
    app: AppHandle,
    visible: bool,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    crate::overlay::ensure_pill_window(&app, visible);
    let _ = app.emit("pill-visibility-changed", visible);

    let mut settings = state.settings.lock().unwrap();
    settings.ui.show_floating_pill = visible;
    settings
        .save(&state.settings_path())
        .map_err(|e| CommandError::new("CONFIG_SAVE_FAILED", &e.to_string()))
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
        .start(&mode, &audio_dir)
        .map_err(|e| CommandError::new("CAPTURE_FAILED", &e.to_string()));
    emit_capture_state(&app, &state.recorder);

    if result.is_ok() {
        // Kick this off now, in parallel with the user talking, rather
        // than waiting until they stop and the LLM call is imminent — by
        // the time transcription finishes, a local Ollama that needed
        // starting has had real time to come up.
        let provider = state.settings.lock().unwrap().provider.clone();
        if matches!(provider.active_provider, ProviderType::Ollama) {
            tauri::async_runtime::spawn(async move {
                crate::providers::ensure_ollama_ready(&provider.ollama_host, &provider.ollama_model).await;
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

#[tauri::command]
pub async fn stop_capture(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ProcessedPipelineResult, CommandError> {
    let captured = state
        .recorder
        .stop()
        .await
        .map_err(|e| CommandError::new("CAPTURE_STOP_FAILED", &e.to_string()));
    emit_capture_state(&app, &state.recorder);
    let captured = captured?;

    let result = process_captured_audio(&state, captured).await;
    if let Ok(processed) = &result {
        let _ = app.emit(CAPTURE_PROCESSED_EVENT, processed);
    }
    result
}

async fn process_captured_audio(
    state: &State<'_, AppState>,
    captured: crate::capture::CapturedAudio,
) -> Result<ProcessedPipelineResult, CommandError> {
    let settings = state.settings.lock().unwrap().clone();
    let stt = state.stt.clone();
    let samples = captured.samples.clone();
    let model_path = settings.stt.whisper_model_path.clone();

    let transcript =
        tokio::task::spawn_blocking(move || stt.transcribe(model_path.as_deref(), &samples))
            .await
            .map_err(|e| CommandError::new("STT_TASK_FAILED", &e.to_string()))?
            .map_err(|e| CommandError::new("STT_FAILED", &e.to_string()))?;

    // Trigger phrases only make sense for meeting/scribble capture, not for
    // an in-app chat question — a question that happens to contain "remind
    // me" shouldn't hijack the answer into firing an action.
    if captured.mode != "chat" {
        let triggers_path = state.config_dir.join("triggers.json");
        let triggers = TriggerEngine::load_triggers(&triggers_path)
            .unwrap_or_else(|_| TriggerEngine::default_triggers());

        if let Some(trigger_match) = TriggerEngine::match_transcript(&transcript, &triggers) {
            let mcp_res = McpRouter::dispatch_action(
                &trigger_match.action_type,
                &trigger_match.target_tool,
                &trigger_match.extracted_text,
            )
            .await
            .map_err(|e| CommandError::new("MCP_EXECUTION_FAILED", &e.to_string()))?;

            return Ok(ProcessedPipelineResult {
                mode: "trigger".to_string(),
                transcript,
                note_id: None,
                kanban_cards_created: 0,
                output_markdown: mcp_res.result_summary,
                sources: Vec::new(),
                spoken_audio_base64: None,
            });
        }
    }

    let llm = LLMClient::new(settings.provider.clone());

    match captured.mode.as_str() {
        "meeting" => PipelineEngine::process_meeting(&llm, &state.vault, &transcript)
            .await
            .map_err(|e| CommandError::new("PIPELINE_ERROR", &e.to_string())),
        "chat" => crate::pipeline::process_chat(&llm, &state.vault, &settings.tts, &transcript)
            .await
            .map_err(|e| CommandError::new("PIPELINE_ERROR", &e.to_string())),
        _ => PipelineEngine::process_scribble(&llm, &state.vault, &transcript)
            .await
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
    settings: AppSettings,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    settings
        .save(&state.settings_path())
        .map_err(|e| CommandError::new("CONFIG_SAVE_FAILED", &e.to_string()))?;
    *state.settings.lock().unwrap() = settings;
    Ok(())
}
