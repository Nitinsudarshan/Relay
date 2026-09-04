//! Action dispatching and execution layer.
//!
//! Separates intent from concrete action and validates confirmation requirements
//! prior to mutating executions, with strict idempotency and audit logging.

use super::audit::{ActionAuditLogger, ActionAuditRecord};
use super::idempotency::IdempotencyStore;
use super::model::{ActionStatus, UniversalAction};
use super::registry::{ActionExecutionContext, ActionRegistry};
use crate::vault::VaultManager;

pub struct ActionDispatcher;

impl ActionDispatcher {
    /// Attempts to execute an action, respecting confirmation constraints,
    /// checking idempotency, and recording an audit trail.
    pub fn execute(
        action: &mut UniversalAction,
        confirmed: bool,
        vault: Option<&VaultManager>,
    ) -> Result<serde_json::Value, String> {
        let registry = ActionRegistry::new();
        let handler = registry.find_handler(&action.action_type)
            .ok_or_else(|| format!("Action '{}' is unsupported/not-implemented", action.action_type.as_str()))?;

        // 1. Check idempotency cache if vault is available
        if let Some(v) = vault {
            let idemp_store = IdempotencyStore::new(&v.vault_dir());
            if let Some(cached) = idemp_store.get_cached_result(&action.id) {
                action.mark_completed(cached.clone());
                return Ok(cached);
            }
        }

        // 2. Validate action input
        handler.validate(action)?;

        // 3. Enforce confirmation requirement in code
        let requires_conf = handler.requires_confirmation(action) || action.requires_confirmation;
        if requires_conf && !confirmed {
            action.status = ActionStatus::RequiresConfirmation;
            return Err(format!(
                "Action '{}' on target '{}' requires explicit confirmation before execution.",
                action.action_type.as_str(),
                action.target
            ));
        }

        action.status = ActionStatus::Executing;
        let ctx = ActionExecutionContext { vault };
        let execution_result = handler.execute(action, &ctx);

        // 4. Audit logging & idempotency recording
        if let Some(v) = vault {
            let audit_logger = ActionAuditLogger::new(&v.vault_dir());
            let (status_str, res_val, err_str) = match &execution_result {
                Ok(val) => ("completed", Some(val.clone()), None),
                Err(err) => ("failed", None, Some(err.clone())),
            };
            let audit_record = ActionAuditRecord {
                action_id: action.id.clone(),
                action_type: action.action_type.as_str().to_string(),
                target: action.target.clone(),
                parameters: action.parameters.clone(),
                requires_confirmation: requires_conf,
                confirmed,
                status: status_str.to_string(),
                result: res_val,
                error: err_str,
                timestamp: chrono::Utc::now().to_rfc3339(),
            };
            let _ = audit_logger.log_record(&audit_record);

            if let Ok(ref val) = execution_result {
                let idemp_store = IdempotencyStore::new(&v.vault_dir());
                let _ = idemp_store.record_result(&action.id, val.clone());
            }
        }

        match execution_result {
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
    use crate::actions::model::ActionType;

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

        // Unconfirmed execution fails
        let res = ActionDispatcher::execute(&mut act, false, None);
        assert!(res.is_err());
        assert_eq!(act.status, ActionStatus::RequiresConfirmation);

        // Confirmed execution proceeds and creates genuine side-effect
        let temp_dir = std::env::temp_dir().join(format!("relay_test_act_{}", uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let vault = VaultManager::new(temp_dir.clone());
        let res2 = ActionDispatcher::execute(&mut act, true, Some(&vault));
        assert!(res2.is_ok());
        assert_eq!(act.status, ActionStatus::Completed);

        // Idempotency: repeating with same action ID returns cached result without creating duplicate
        let res3 = ActionDispatcher::execute(&mut act, true, Some(&vault));
        assert!(res3.is_ok());

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_unsupported_action_fails_truthfully() {
        let mut act = UniversalAction::new(
            ActionType::Custom("some_unsupported_action".to_string()),
            "target",
            serde_json::json!({}),
        );
        let res = ActionDispatcher::execute(&mut act, true, None);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("unsupported/not-implemented"));
    }
}
