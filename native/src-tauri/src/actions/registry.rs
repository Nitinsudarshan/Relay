//! Action Handler Registry.
//!
//! Provides modular action definition with strict input validation, security checks,
//! and truthful execution producing genuine side-effects.

use std::sync::Arc;
use serde_json::json;

use super::model::{ActionType, UniversalAction};
use crate::vault::{Scribble, VaultManager};

/// Execution context providing access to Vault and system services.
pub struct ActionExecutionContext<'a> {
    pub vault: Option<&'a VaultManager>,
}

/// Trait defining a validated, truthful action handler.
pub trait ActionHandler: Send + Sync {
    fn action_type(&self) -> ActionType;
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn requires_confirmation(&self, action: &UniversalAction) -> bool;
    fn validate(&self, action: &UniversalAction) -> Result<(), String>;
    fn execute(&self, action: &UniversalAction, ctx: &ActionExecutionContext) -> Result<serde_json::Value, String>;
}

/// Handler for OpenUrl: opens validated HTTP/HTTPS URLs in the default browser.
pub struct OpenUrlHandler;

impl ActionHandler for OpenUrlHandler {
    fn action_type(&self) -> ActionType {
        ActionType::OpenUrl
    }

    fn name(&self) -> &'static str {
        "open_url"
    }

    fn description(&self) -> &'static str {
        "Opens an external HTTP/HTTPS URL in the default browser."
    }

    fn requires_confirmation(&self, _action: &UniversalAction) -> bool {
        false
    }

    fn validate(&self, action: &UniversalAction) -> Result<(), String> {
        let url = action.target.trim();
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(format!(
                "Security rejection: URL '{}' must begin with http:// or https://",
                url
            ));
        }
        if url.contains('\n') || url.contains('\r') || url.contains('\"') {
            return Err("Security rejection: URL contains forbidden control characters".to_string());
        }
        Ok(())
    }

    fn execute(&self, action: &UniversalAction, _ctx: &ActionExecutionContext) -> Result<serde_json::Value, String> {
        let url = action.target.trim();
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("rundll32")
                .args(["url.dll,FileProtocolHandler", url])
                .spawn()
                .map_err(|e| format!("Failed to open URL: {}", e))?;
        }
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg(url)
                .spawn()
                .map_err(|e| format!("Failed to open URL: {}", e))?;
        }
        #[cfg(target_os = "linux")]
        {
            std::process::Command::new("xdg-open")
                .arg(url)
                .spawn()
                .map_err(|e| format!("Failed to open URL: {}", e))?;
        }
        Ok(json!({ "opened_url": url, "status": "opened" }))
    }
}

/// Handler for CopyContent: copies text to the operating system clipboard.
pub struct CopyContentHandler;

impl ActionHandler for CopyContentHandler {
    fn action_type(&self) -> ActionType {
        ActionType::CopyContent
    }

    fn name(&self) -> &'static str {
        "copy_content"
    }

    fn description(&self) -> &'static str {
        "Copies provided content to the system clipboard."
    }

    fn requires_confirmation(&self, _action: &UniversalAction) -> bool {
        false
    }

    fn validate(&self, action: &UniversalAction) -> Result<(), String> {
        let text = action.parameters.get("text")
            .and_then(|t| t.as_str())
            .unwrap_or(&action.target);
        if text.is_empty() {
            return Err("Cannot copy empty content to clipboard".to_string());
        }
        Ok(())
    }

    fn execute(&self, action: &UniversalAction, _ctx: &ActionExecutionContext) -> Result<serde_json::Value, String> {
        let text = action.parameters.get("text")
            .and_then(|t| t.as_str())
            .unwrap_or(&action.target);

        let mut board = arboard::Clipboard::new()
            .map_err(|e| format!("Clipboard access failed: {}", e))?;
        board.set_text(text)
            .map_err(|e| format!("Failed to copy text to clipboard: {}", e))?;

        Ok(json!({ "copied_chars": text.len(), "status": "copied" }))
    }
}

/// Handler for CreateNote: creates a real Scribble record in the Vault.
pub struct CreateNoteHandler;

impl ActionHandler for CreateNoteHandler {
    fn action_type(&self) -> ActionType {
        ActionType::CreateNote
    }

    fn name(&self) -> &'static str {
        "create_note"
    }

    fn description(&self) -> &'static str {
        "Creates a persistent note/scribble in the Vault."
    }

    fn requires_confirmation(&self, _action: &UniversalAction) -> bool {
        true
    }

    fn validate(&self, action: &UniversalAction) -> Result<(), String> {
        let content = action.parameters.get("content")
            .and_then(|c| c.as_str())
            .unwrap_or(&action.target);
        if content.trim().is_empty() {
            return Err("Note content cannot be empty".to_string());
        }
        Ok(())
    }

    fn execute(&self, action: &UniversalAction, ctx: &ActionExecutionContext) -> Result<serde_json::Value, String> {
        let vault = ctx.vault.ok_or_else(|| "Vault is required to create a note".to_string())?;
        let title = action.parameters.get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("New Note");
        let content = action.parameters.get("content")
            .and_then(|c| c.as_str())
            .unwrap_or(&action.target);

        let scribble = Scribble::new_text(content, Some(title));
        vault.save_scribble(&scribble).map_err(|e| e.to_string())?;

        Ok(json!({
            "note_id": scribble.id,
            "title": title,
            "status": "created"
        }))
    }
}

/// Handler for CreateTask: records a truthful task in persistent Vault task store.
pub struct CreateTaskHandler;

impl ActionHandler for CreateTaskHandler {
    fn action_type(&self) -> ActionType {
        ActionType::CreateTask
    }

    fn name(&self) -> &'static str {
        "create_task"
    }

    fn description(&self) -> &'static str {
        "Creates an actionable task record in the Vault."
    }

    fn requires_confirmation(&self, _action: &UniversalAction) -> bool {
        true
    }

    fn validate(&self, action: &UniversalAction) -> Result<(), String> {
        if action.target.trim().is_empty() {
            return Err("Task description cannot be empty".to_string());
        }
        Ok(())
    }

    fn execute(&self, action: &UniversalAction, ctx: &ActionExecutionContext) -> Result<serde_json::Value, String> {
        let vault = ctx.vault.ok_or_else(|| "Vault is required to persist tasks".to_string())?;
        let tasks_dir = vault.vault_dir().join("tasks");
        std::fs::create_dir_all(&tasks_dir).map_err(|e| e.to_string())?;
        let tasks_file = tasks_dir.join("index.json");

        let mut tasks: Vec<serde_json::Value> = Vec::new();
        if tasks_file.exists() {
            if let Ok(content) = std::fs::read_to_string(&tasks_file) {
                if let Ok(loaded) = serde_json::from_str(&content) {
                    tasks = loaded;
                }
            }
        }

        let task_id = format!("task_{}", uuid::Uuid::new_v4());
        let task_record = json!({
            "id": task_id,
            "title": action.target,
            "parameters": action.parameters,
            "created_at": chrono::Utc::now().to_rfc3339(),
            "status": "pending",
        });

        tasks.push(task_record.clone());
        let json_raw = serde_json::to_string_pretty(&tasks).map_err(|e| e.to_string())?;
        let tmp = tasks_file.with_extension("tmp");
        std::fs::write(&tmp, json_raw.as_bytes()).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &tasks_file).map_err(|e| e.to_string())?;

        Ok(json!({
            "task_id": task_id,
            "title": action.target,
            "status": "created"
        }))
    }
}

/// Handler for OpenSource: resolves existing source in Vault.
pub struct OpenSourceHandler;

impl ActionHandler for OpenSourceHandler {
    fn action_type(&self) -> ActionType {
        ActionType::OpenSource
    }

    fn name(&self) -> &'static str {
        "open_source"
    }

    fn description(&self) -> &'static str {
        "Resolves and opens an existing source item in Relay."
    }

    fn requires_confirmation(&self, _action: &UniversalAction) -> bool {
        false
    }

    fn validate(&self, action: &UniversalAction) -> Result<(), String> {
        if action.target.trim().is_empty() {
            return Err("Target source ID cannot be empty".to_string());
        }
        Ok(())
    }

    fn execute(&self, action: &UniversalAction, ctx: &ActionExecutionContext) -> Result<serde_json::Value, String> {
        let vault = ctx.vault.ok_or_else(|| "Vault is required to resolve sources".to_string())?;
        
        // Verify source exists in files, captures, scribbles, or notes
        let sid = action.target.trim();
        let exists = vault.get_vault_file(sid).is_ok()
            || vault.get_scribble(sid).is_ok()
            || vault.get_note(sid).is_ok();

        if !exists {
            return Err(format!("Source '{}' does not exist in Vault", sid));
        }

        Ok(json!({ "source_id": sid, "status": "resolved" }))
    }
}

/// Registry managing available action handlers.
pub struct ActionRegistry {
    handlers: Vec<Arc<dyn ActionHandler>>,
}

impl ActionRegistry {
    pub fn new() -> Self {
        Self {
            handlers: vec![
                Arc::new(OpenUrlHandler),
                Arc::new(CopyContentHandler),
                Arc::new(CreateNoteHandler),
                Arc::new(CreateTaskHandler),
                Arc::new(OpenSourceHandler),
            ],
        }
    }

    pub fn find_handler(&self, action_type: &ActionType) -> Option<Arc<dyn ActionHandler>> {
        self.handlers.iter().find(|h| &h.action_type() == action_type).cloned()
    }
}

impl Default for ActionRegistry {
    fn default() -> Self {
        Self::new()
    }
}
