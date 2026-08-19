use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TriggerError {
    #[error("Trigger IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Trigger not found: {0}")]
    NotFound(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TriggerConfig {
    pub id: String,
    pub phrase: String,
    pub action_type: String, // "mcp_calendar", "local_reminder", "mcp_notion", "mcp_gdrive"
    pub target_tool: String,
    pub parameters: serde_json::Value,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TriggerMatch {
    pub trigger_id: String,
    pub phrase: String,
    pub action_type: String,
    pub target_tool: String,
    pub extracted_text: String,
}

pub struct TriggerEngine;

impl TriggerEngine {
    pub fn default_triggers() -> Vec<TriggerConfig> {
        vec![
            TriggerConfig {
                id: "trig_001".to_string(),
                phrase: "schedule meeting".to_string(),
                action_type: "mcp_calendar".to_string(),
                target_tool: "google_calendar_create_event".to_string(),
                parameters: serde_json::json!({ "calendar_id": "primary" }),
                enabled: true,
            },
            TriggerConfig {
                id: "trig_002".to_string(),
                phrase: "remind me to".to_string(),
                action_type: "local_reminder".to_string(),
                target_tool: "os_notification".to_string(),
                parameters: serde_json::json!({}),
                enabled: true,
            },
        ]
    }

    pub fn load_triggers(config_path: &Path) -> Result<Vec<TriggerConfig>, TriggerError> {
        if !config_path.exists() {
            let defaults = Self::default_triggers();
            Self::save_triggers(config_path, &defaults)?;
            return Ok(defaults);
        }

        let content = fs::read_to_string(config_path)?;
        let triggers: Vec<TriggerConfig> = serde_json::from_str(&content)?;
        Ok(triggers)
    }

    pub fn save_triggers(
        config_path: &Path,
        triggers: &[TriggerConfig],
    ) -> Result<(), TriggerError> {
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(triggers)?;
        fs::write(config_path, content)?;
        tracing::info!(
            "Saved {} trigger configurations to {:?}",
            triggers.len(),
            config_path
        );
        Ok(())
    }

    pub fn match_transcript<'a>(
        transcript: &'a str,
        triggers: &'a [TriggerConfig],
    ) -> Option<TriggerMatch> {
        let clean_transcript = transcript.to_lowercase();

        for trigger in triggers {
            if !trigger.enabled {
                continue;
            }

            let phrase_clean = trigger.phrase.to_lowercase();
            if clean_transcript.contains(&phrase_clean) {
                let remainder = clean_transcript
                    .split(&phrase_clean)
                    .nth(1)
                    .unwrap_or("")
                    .trim()
                    .to_string();

                return Some(TriggerMatch {
                    trigger_id: trigger.id.clone(),
                    phrase: trigger.phrase.clone(),
                    action_type: trigger.action_type.clone(),
                    target_tool: trigger.target_tool.clone(),
                    extracted_text: if remainder.is_empty() {
                        transcript.to_string()
                    } else {
                        remainder
                    },
                });
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_triggers() {
        let defaults = TriggerEngine::default_triggers();
        assert_eq!(defaults.len(), 2);
        assert_eq!(defaults[0].phrase, "schedule meeting");
    }

    #[test]
    fn test_match_transcript() {
        let triggers = TriggerEngine::default_triggers();
        let match_res = TriggerEngine::match_transcript(
            "Please remind me to submit architecture plan",
            &triggers,
        );
        assert!(match_res.is_some());
        let m = match_res.unwrap();
        assert_eq!(m.trigger_id, "trig_002");
        assert_eq!(m.action_type, "local_reminder");
        assert_eq!(m.extracted_text, "submit architecture plan");
    }
}
