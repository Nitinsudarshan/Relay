//! Action Idempotency Store.
//!
//! Caches execution results by action_id to prevent duplicate mutating executions on retry.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

pub struct IdempotencyStore {
    storage_path: PathBuf,
    cache: RwLock<HashMap<String, serde_json::Value>>,
}

impl IdempotencyStore {
    pub fn new(vault_dir: &Path) -> Self {
        let dir = vault_dir.join("actions");
        let _ = fs::create_dir_all(&dir);
        let storage_path = dir.join("idempotency.json");

        let mut loaded = HashMap::new();
        if storage_path.exists() {
            if let Ok(data) = fs::read_to_string(&storage_path) {
                if let Ok(records) = serde_json::from_str(&data) {
                    loaded = records;
                }
            }
        }

        Self {
            storage_path,
            cache: RwLock::new(loaded),
        }
    }

    fn persist(&self) -> Result<(), String> {
        let cache = self.cache.read().map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(&*cache).map_err(|e| e.to_string())?;
        let tmp = self.storage_path.with_extension("tmp");
        fs::write(&tmp, json.as_bytes()).map_err(|e| e.to_string())?;
        fs::rename(&tmp, &self.storage_path).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_cached_result(&self, action_id: &str) -> Option<serde_json::Value> {
        let cache = self.cache.read().unwrap_or_else(|e| e.into_inner());
        cache.get(action_id).cloned()
    }

    pub fn record_result(&self, action_id: &str, result: serde_json::Value) -> Result<(), String> {
        let mut cache = self.cache.write().map_err(|e| e.to_string())?;
        cache.insert(action_id.to_string(), result);
        drop(cache);
        self.persist()
    }
}
