use crate::capture::{AudioRecorder, SttEngine};
use crate::hotkeys;
use crate::mcp::McpRouter;
use crate::pipeline::{PipelineEngine, ProcessedPipelineResult};
use crate::providers::{LLMClient, OllamaStatus, ProviderType};
use crate::settings::{AppSettings, HotkeySettings, PillPosition};
use crate::triggers::{TriggerConfig, TriggerEngine};
use crate::vault::{KanbanCard, VaultManager};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

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

#[tauri::command]
pub async fn set_pill_visible(
    app: AppHandle,
    visible: bool,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    let position = state.settings.lock().unwrap().ui.pill_position;
    crate::overlay::ensure_pill_window(&app, visible, position);
    let _ = app.emit("pill-visibility-changed", visible);

    let mut settings = state.settings.lock().unwrap();
    settings.ui.show_floating_pill = visible;
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

    // Nobody should have to go find and download a GGML file themselves
    // before dictation produces any text at all — if Settings doesn't have
    // one configured, fetch a small default and remember it for next time.
    let model_path = match settings
        .stt
        .whisper_model_path
        .clone()
        .filter(|p| !p.trim().is_empty())
    {
        Some(configured) => Some(configured),
        None => {
            let models_dir = state.config_dir.join("models");
            match crate::capture::stt::ensure_default_model(&models_dir).await {
                Ok(path) => {
                    let path_str = path.to_string_lossy().to_string();
                    let mut guard = state.settings.lock().unwrap();
                    guard.stt.whisper_model_path = Some(path_str.clone());
                    let _ = guard.save(&state.settings_path());
                    Some(path_str)
                }
                Err(e) => {
                    tracing::warn!("Could not auto-provision a default Whisper model: {}", e);
                    None
                }
            }
        }
    };

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
    let configured = state
        .settings
        .lock()
        .unwrap()
        .stt
        .whisper_model_path
        .clone()
        .filter(|p| !p.trim().is_empty());
    if let Some(path) = configured {
        if std::path::Path::new(&path).exists() {
            return Ok(SttModelStatus::Ready { path });
        }
    }

    let models_dir = state.config_dir.join("models");
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
    settings: AppSettings,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    settings
        .save(&state.settings_path())
        .map_err(|e| CommandError::new("CONFIG_SAVE_FAILED", &e.to_string()))?;
    *state.settings.lock().unwrap() = settings;
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

