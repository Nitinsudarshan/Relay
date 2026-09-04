//! Universal Action contract and execution models.

use serde::{Deserialize, Serialize};

/// Supported action types across Relay capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    OpenUrl,
    OpenSource,
    CreateNote,
    CreateTask,
    SaveCapture,
    CopyContent,
    #[serde(untagged)]
    Custom(String),
}

impl ActionType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::OpenUrl => "open_url",
            Self::OpenSource => "open_source",
            Self::CreateNote => "create_note",
            Self::CreateTask => "create_task",
            Self::SaveCapture => "save_capture",
            Self::CopyContent => "copy_content",
            Self::Custom(s) => s.as_str(),
        }
    }

    /// Whether this action is mutating or has external consequences requiring confirmation.
    pub fn is_mutating(&self) -> bool {
        match self {
            Self::OpenUrl | Self::OpenSource => false,
            Self::CreateNote | Self::CreateTask | Self::SaveCapture => true,
            Self::CopyContent => false,
            Self::Custom(_) => true,
        }
    }
}

/// Execution status of a universal action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionStatus {
    Pending,
    RequiresConfirmation,
    Confirmed,
    Executing,
    Completed,
    Failed,
    Cancelled,
}

/// A structured, verifiable action contract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UniversalAction {
    pub id: String,
    pub action_type: ActionType,
    #[serde(default)]
    pub intent: Option<String>,
    pub target: String,
    #[serde(default)]
    pub parameters: serde_json::Value,
    #[serde(default)]
    pub source_context: Option<String>,
    pub requires_confirmation: bool,
    pub status: ActionStatus,
    #[serde(default)]
    pub result: Option<serde_json::Value>,
    #[serde(default)]
    pub error_message: Option<String>,
    #[serde(default)]
    pub provenance: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub executed_at: Option<String>,
}

impl UniversalAction {
    pub fn new(
        action_type: ActionType,
        target: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        let req_conf = action_type.is_mutating();
        let status = if req_conf {
            ActionStatus::RequiresConfirmation
        } else {
            ActionStatus::Pending
        };

        Self {
            id: format!("act_{}_{}", action_type.as_str(), uuid::Uuid::new_v4()),
            action_type,
            intent: None,
            target: target.into(),
            parameters,
            source_context: None,
            requires_confirmation: req_conf,
            status,
            result: None,
            error_message: None,
            provenance: None,
            created_at: now,
            executed_at: None,
        }
    }

    pub fn with_intent(mut self, intent: impl Into<String>) -> Self {
        self.intent = Some(intent.into());
        self
    }

    pub fn with_provenance(mut self, provenance: impl Into<String>) -> Self {
        self.provenance = Some(provenance.into());
        self
    }

    pub fn with_source_context(mut self, ctx: impl Into<String>) -> Self {
        self.source_context = Some(ctx.into());
        self
    }

    pub fn mark_completed(&mut self, result: serde_json::Value) {
        self.status = ActionStatus::Completed;
        self.result = Some(result);
        self.executed_at = Some(chrono::Utc::now().to_rfc3339());
    }

    pub fn mark_failed(&mut self, error: impl Into<String>) {
        self.status = ActionStatus::Failed;
        self.error_message = Some(error.into());
        self.executed_at = Some(chrono::Utc::now().to_rfc3339());
    }
}
