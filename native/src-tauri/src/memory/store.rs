//! Memory storage and lifecycle management.
//!
//! Provides non-destructive lifecycle transitions: create, update, supersede,
//! archive, mark known-false, and soft-delete.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use super::model::{EpistemicState, MemoryItem, MemoryProvenance, MemoryStatus, MemoryType};

pub struct MemoryStore {
    storage_path: PathBuf,
    items: RwLock<Vec<MemoryItem>>,
}

impl MemoryStore {
    pub fn new(vault_dir: &Path) -> Self {
        let memory_dir = vault_dir.join("memory");
        let _ = fs::create_dir_all(&memory_dir);
        let storage_path = memory_dir.join("index.json");

        let mut loaded = Vec::new();
        if storage_path.exists() {
            if let Ok(data) = fs::read_to_string(&storage_path) {
                if let Ok(records) = serde_json::from_str::<Vec<MemoryItem>>(&data) {
                    loaded = records;
                }
            }
        }

        Self {
            storage_path,
            items: RwLock::new(loaded),
        }
    }

    fn persist(&self) -> Result<(), String> {
        let items = self.items.read().map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(&*items).map_err(|e| e.to_string())?;
        fs::write(&self.storage_path, json).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Stores a new active memory.
    pub fn create_memory(&self, item: MemoryItem) -> Result<MemoryItem, String> {
        let mut items = self.items.write().map_err(|e| e.to_string())?;
        items.push(item.clone());
        drop(items);
        self.persist()?;
        Ok(item)
    }

    /// Retrieves a memory by ID.
    pub fn get_memory(&self, id: &str) -> Option<MemoryItem> {
        let items = self.items.read().unwrap_or_else(|e| e.into_inner());
        items.iter().find(|m| m.id == id).cloned()
    }

    /// In-place update of memory content and evidence without changing identity.
    pub fn update_memory(
        &self,
        id: &str,
        new_content: &str,
        additional_provenance: Option<MemoryProvenance>,
    ) -> Result<MemoryItem, String> {
        let mut items = self.items.write().map_err(|e| e.to_string())?;
        let idx = items
            .iter()
            .position(|m| m.id == id)
            .ok_or_else(|| format!("Memory {} not found", id))?;

        let now = chrono::Utc::now().to_rfc3339();
        let item = &mut items[idx];
        item.content = new_content.to_string();
        item.updated_at = now;
        if let Some(prov) = additional_provenance {
            item.provenance.push(prov);
        }

        let updated = item.clone();
        drop(items);
        self.persist()?;
        Ok(updated)
    }

    /// Supersedes an existing memory with a new memory, preserving full lineage and history.
    /// Returns (superseded_old_item, new_active_item).
    pub fn supersede_memory(
        &self,
        old_id: &str,
        new_content: &str,
        provenance: MemoryProvenance,
    ) -> Result<(MemoryItem, MemoryItem), String> {
        let mut items = self.items.write().map_err(|e| e.to_string())?;
        let idx = items
            .iter()
            .position(|m| m.id == old_id)
            .ok_or_else(|| format!("Memory {} not found", old_id))?;

        let old_item = &mut items[idx];
        let new_item = old_item.supersede(new_content, provenance);
        let old_clone = old_item.clone();
        let new_clone = new_item.clone();

        items.push(new_item);
        drop(items);
        self.persist()?;
        Ok((old_clone, new_clone))
    }

    /// Archives a memory (retained in history, but inactive for general queries).
    pub fn archive_memory(&self, id: &str) -> Result<MemoryItem, String> {
        let mut items = self.items.write().map_err(|e| e.to_string())?;
        let idx = items
            .iter()
            .position(|m| m.id == id)
            .ok_or_else(|| format!("Memory {} not found", id))?;

        let item = &mut items[idx];
        item.status = MemoryStatus::Archived;
        item.updated_at = chrono::Utc::now().to_rfc3339();
        let updated = item.clone();
        drop(items);
        self.persist()?;
        Ok(updated)
    }

    /// Marks a memory as known false backed by contradicting counter-evidence.
    pub fn mark_known_false(
        &self,
        id: &str,
        counter_evidence: MemoryProvenance,
    ) -> Result<MemoryItem, String> {
        let mut items = self.items.write().map_err(|e| e.to_string())?;
        let idx = items
            .iter()
            .position(|m| m.id == id)
            .ok_or_else(|| format!("Memory {} not found", id))?;

        let item = &mut items[idx];
        item.epistemic_state = EpistemicState::KnownFalse;
        item.status = MemoryStatus::Archived;
        item.provenance.push(counter_evidence);
        item.updated_at = chrono::Utc::now().to_rfc3339();
        let updated = item.clone();
        drop(items);
        self.persist()?;
        Ok(updated)
    }

    /// Soft-deletes a memory, preserving the record with status = Deleted.
    pub fn delete_memory(&self, id: &str) -> Result<bool, String> {
        let mut items = self.items.write().map_err(|e| e.to_string())?;
        if let Some(item) = items.iter_mut().find(|m| m.id == id) {
            item.status = MemoryStatus::Deleted;
            item.updated_at = chrono::Utc::now().to_rfc3339();
            drop(items);
            self.persist()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Lists all currently active memories, optionally filtered by type.
    pub fn list_active(&self, memory_type: Option<MemoryType>) -> Vec<MemoryItem> {
        let items = self.items.read().unwrap_or_else(|e| e.into_inner());
        items
            .iter()
            .filter(|m| m.status == MemoryStatus::Active && m.epistemic_state == EpistemicState::Current)
            .filter(|m| memory_type.map(|t| m.memory_type == t).unwrap_or(true))
            .cloned()
            .collect()
    }

    /// Queries all historical versions in the supersedes lineage of a memory ID.
    pub fn get_lineage(&self, start_id: &str) -> Vec<MemoryItem> {
        let items = self.items.read().unwrap_or_else(|e| e.into_inner());
        let mut chain = Vec::new();

        // 1. Find root by walking supersedes_id backwards
        let mut root_id = start_id.to_string();
        while let Some(prev) = items.iter().find(|m| m.id == root_id).and_then(|m| m.supersedes_id.as_ref()) {
            root_id = prev.clone();
        }

        // 2. Walk forward from root via superseded_by
        let mut curr_id = Some(root_id);
        while let Some(id) = curr_id {
            if let Some(item) = items.iter().find(|m| m.id == id) {
                chain.push(item.clone());
                curr_id = item.superseded_by.clone();
            } else {
                break;
            }
        }

        chain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_provenance() -> MemoryProvenance {
        MemoryProvenance {
            source_id: "src_1".to_string(),
            source_type: "conversation".to_string(),
            evidence: "User stated preference for short summaries.".to_string(),
            confidence: 0.95,
            extracted_by: "deterministic".to_string(),
        }
    }

    #[test]
    fn test_memory_lifecycle_and_supersedes() {
        let temp_dir = std::env::temp_dir().join(format!("relay_test_mem_{}", uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let store = MemoryStore::new(&temp_dir);

        let mem = MemoryItem::new(
            MemoryType::Preference,
            "summary_length",
            "User prefers concise 2-sentence summaries.",
            sample_provenance(),
        );
        let created = store.create_memory(mem).unwrap();
        assert_eq!(created.status, MemoryStatus::Active);

        // Active list contains it
        let active = store.list_active(Some(MemoryType::Preference));
        assert_eq!(active.len(), 1);

        // Newer evidence arrives: User now wants detailed technical summaries!
        let new_prov = MemoryProvenance {
            source_id: "src_2".to_string(),
            source_type: "conversation".to_string(),
            evidence: "User says: give me deep technical breakdowns now.".to_string(),
            confidence: 0.98,
            extracted_by: "deterministic".to_string(),
        };

        let (old, new_mem) = store
            .supersede_memory(&created.id, "User prefers detailed technical breakdowns.", new_prov)
            .unwrap();

        assert_eq!(old.status, MemoryStatus::Superseded);
        assert_eq!(old.epistemic_state, EpistemicState::NoLongerCurrent);
        assert_eq!(old.superseded_by, Some(new_mem.id.clone()));

        assert_eq!(new_mem.status, MemoryStatus::Active);
        assert_eq!(new_mem.supersedes_id, Some(created.id.clone()));

        // Active list only returns the new one!
        let active = store.list_active(Some(MemoryType::Preference));
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, new_mem.id);

        // Lineage returns both in order: old -> new
        let lineage = store.get_lineage(&new_mem.id);
        assert_eq!(lineage.len(), 2);
        assert_eq!(lineage[0].id, created.id);
        assert_eq!(lineage[1].id, new_mem.id);

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_mark_known_false() {
        let temp_dir = std::env::temp_dir().join(format!("relay_test_mem_kf_{}", uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let store = MemoryStore::new(&temp_dir);

        let mem = MemoryItem::new(
            MemoryType::Fact,
            "orca_db",
            "Orca uses MongoDB as primary database.",
            sample_provenance(),
        );
        let created = store.create_memory(mem).unwrap();

        let counter = MemoryProvenance {
            source_id: "readme_1".to_string(),
            source_type: "repository".to_string(),
            evidence: "Orca uses SQLite only; MongoDB was removed in v0.2.".to_string(),
            confidence: 1.0,
            extracted_by: "user".to_string(),
        };

        let refuted = store.mark_known_false(&created.id, counter).unwrap();
        assert_eq!(refuted.epistemic_state, EpistemicState::KnownFalse);
        assert_eq!(refuted.status, MemoryStatus::Archived);
        assert!(store.list_active(Some(MemoryType::Fact)).is_empty());

        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
