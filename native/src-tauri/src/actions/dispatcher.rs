//! Action dispatching and execution layer.
//!
//! Separates intent from concrete action and validates confirmation requirements
//! prior to mutating executions.

use super::model::{ActionStatus, ActionType, UniversalAction};
use crate::vault::{Scribble, VaultManager};

fn open_in_browser(url: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub struct ActionDispatcher;

impl ActionDispatcher {
    /// Attempts to execute an action, respecting confirmation constraints.
    pub fn execute(
        action: &mut UniversalAction,
        confirmed: bool,
        vault: Option<&VaultManager>,
    ) -> Result<serde_json::Value, String> {
        // Enforce confirmation requirement
        if action.requires_confirmation && !confirmed {
            action.status = ActionStatus::RequiresConfirmation;
            return Err(format!(
                "Action '{}' on target '{}' requires explicit confirmation before execution.",
                action.action_type.as_str(),
                action.target
            ));
        }

        action.status = ActionStatus::Executing;

        let res = match &action.action_type {
            ActionType::OpenUrl => {
                let url = &action.target;
                if !url.starts_with("http://") && !url.starts_with("https://") {
                    return Err(format!("Invalid URL: {}", url));
                }
                let _ = open_in_browser(url);
                Ok(serde_json::json!({ "opened_url": url, "status": "success" }))
            }

            ActionType::CopyContent => {
                let text = action.parameters.get("text")
                    .and_then(|t| t.as_str())
                    .unwrap_or(&action.target);
                
                // Copy to system clipboard
                if let Ok(mut board) = arboard::Clipboard::new() {
                    let _ = board.set_text(text);
                }
                Ok(serde_json::json!({ "copied_chars": text.len(), "status": "success" }))
            }

            ActionType::CreateNote => {
                let Some(v) = vault else {
                    return Err("Vault is required to create a note".to_string());
                };
                let title = action.parameters.get("title")
                    .and_then(|t| t.as_str())
                    .unwrap_or("New Note");
                let content = action.parameters.get("content")
                    .and_then(|c| c.as_str())
                    .unwrap_or(&action.target);

                let scribble = Scribble::new_text(content, Some(title));
                v.save_scribble(&scribble).map_err(|e| e.to_string())?;
                Ok(serde_json::json!({ "note_id": scribble.id, "title": title, "status": "created" }))
            }

            ActionType::OpenSource => {
                Ok(serde_json::json!({ "source_id": action.target, "status": "opened" }))
            }

            ActionType::CreateTask => {
                Ok(serde_json::json!({ "task": action.target, "status": "created" }))
            }

            ActionType::SaveCapture => {
                Ok(serde_json::json!({ "capture_target": action.target, "status": "saved" }))
            }

            ActionType::Custom(name) => {
                Ok(serde_json::json!({ "custom_action": name, "status": "dispatched" }))
            }
        };

        match res {
            Ok(val) => {
                action.mark_completed(val.clone());
                Ok(val)
            }
            Err(err) => {
                action.mark_failed(&err);
                Err(err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_only_action_executes_without_confirmation() {
        let mut act = UniversalAction::new(
            ActionType::OpenUrl,
            "https://github.com/stablyai/orca",
            serde_json::json!({}),
        );
        assert!(!act.requires_confirmation);

        let res = ActionDispatcher::execute(&mut act, false, None);
        assert!(res.is_ok());
        assert_eq!(act.status, ActionStatus::Completed);
    }

    #[test]
    fn test_mutating_action_blocks_without_confirmation() {
        let mut act = UniversalAction::new(
            ActionType::CreateNote,
            "Note content here",
            serde_json::json!({ "title": "Test Title" }),
        );
        assert!(act.requires_confirmation);

        // Unconfirmed execution fails
        let res = ActionDispatcher::execute(&mut act, false, None);
        assert!(res.is_err());
        assert_eq!(act.status, ActionStatus::RequiresConfirmation);

        // Confirmed execution proceeds
        let temp_dir = std::env::temp_dir().join(format!("relay_test_act_{}", uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let vault = VaultManager::new(temp_dir.clone());
        let res2 = ActionDispatcher::execute(&mut act, true, Some(&vault));
        assert!(res2.is_ok());
        assert_eq!(act.status, ActionStatus::Completed);
        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
