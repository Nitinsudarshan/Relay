use crate::capture::AudioRecorder;
use crate::mcp::McpRouter;
use crate::pipeline::{PipelineEngine, ProcessedPipelineResult};
use crate::providers::{LLMClient, ProviderConfig};
use crate::triggers::{TriggerConfig, TriggerEngine};
use crate::vault::{KanbanCard, VaultManager};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::State;

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
}

#[tauri::command]
pub async fn start_capture(
    mode: String,
    state: State<'_, AppState>,
) -> Result<String, CommandError> {
    if mode.is_empty() {
        return Err(CommandError::new("INVALID_INPUT", "Capture mode cannot be empty"));
    }

    let audio_dir = state.config_dir.join("audio");
    state
        .recorder
        .start(&mode, &audio_dir)
        .map_err(|e| CommandError::new("CAPTURE_FAILED", &e.to_string()))
}

#[tauri::command]
pub async fn stop_capture(
    state: State<'_, AppState>,
) -> Result<ProcessedPipelineResult, CommandError> {
    let capture_res = state
        .recorder
        .stop()
        .await
        .map_err(|e| CommandError::new("CAPTURE_STOP_FAILED", &e.to_string()))?;

    let triggers_path = state.config_dir.join("triggers.json");
    let triggers = TriggerEngine::load_triggers(&triggers_path)
        .unwrap_or_else(|_| TriggerEngine::default_triggers());

    if let Some(trigger_match) = TriggerEngine::match_transcript(&capture_res.transcript, &triggers) {
        let mcp_res = McpRouter::dispatch_action(
            &trigger_match.action_type,
            &trigger_match.target_tool,
            &trigger_match.extracted_text,
        )
        .await
        .map_err(|e| CommandError::new("MCP_EXECUTION_FAILED", &e.to_string()))?;

        return Ok(ProcessedPipelineResult {
            mode: "trigger".to_string(),
            transcript: capture_res.transcript,
            note_id: None,
            kanban_cards_created: 0,
            output_markdown: mcp_res.result_summary,
        });
    }

    let provider_config = ProviderConfig::default();
    let llm = LLMClient::new(provider_config);

    if capture_res.mode == "meeting" {
        PipelineEngine::process_meeting(&llm, &state.vault, &capture_res.transcript)
            .await
            .map_err(|e| CommandError::new("PIPELINE_ERROR", &e.to_string()))
    } else {
        PipelineEngine::process_scribble(&llm, &state.vault, &capture_res.transcript)
            .await
            .map_err(|e| CommandError::new("PIPELINE_ERROR", &e.to_string()))
    }
}

#[tauri::command]
pub async fn get_kanban_cards(
    state: State<'_, AppState>,
) -> Result<Vec<KanbanCard>, CommandError> {
    state
        .vault
        .list_kanban_cards()
        .map_err(|e| CommandError::new("VAULT_READ_FAILED", &e.to_string()))
}

#[tauri::command]
pub async fn get_triggers(
    state: State<'_, AppState>,
) -> Result<Vec<TriggerConfig>, CommandError> {
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
