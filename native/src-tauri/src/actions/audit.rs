//! Action Audit Trail.
//!
//! Records every action execution attempt, parameters, confirmation status, and outcome
//! in append-only JSONL format.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionAuditRecord {
    pub action_id: String,
    pub action_type: String,
    pub target: String,
    pub parameters: serde_json::Value,
    pub requires_confirmation: bool,
    pub confirmed: bool,
    pub status: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub timestamp: String,
}

pub struct ActionAuditLogger {
    log_path: PathBuf,
}

impl ActionAuditLogger {
    pub fn new(vault_dir: &Path) -> Self {
        let dir = vault_dir.join("actions");
        let _ = std::fs::create_dir_all(&dir);
        let log_path = dir.join("audit_log.jsonl");
        Self { log_path }
    }

    pub fn log_record(&self, record: &ActionAuditRecord) -> Result<(), String> {
        let mut line = serde_json::to_string(record).map_err(|e| e.to_string())?;
        line.push('\n');

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .map_err(|e| e.to_string())?;

        file.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
        Ok(())
    }
}
