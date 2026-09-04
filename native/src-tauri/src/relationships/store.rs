//! Persistence and query store for relationships between Relay objects.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use super::model::{RelationshipRecord, RelationshipType};

/// In-memory indexed store backed by lightweight JSON storage.
pub struct RelationshipStore {
    storage_path: PathBuf,
    records: RwLock<Vec<RelationshipRecord>>,
}

impl RelationshipStore {
    /// Creates or opens a RelationshipStore at the given directory.
    pub fn new(vault_dir: &Path) -> Self {
        let storage_dir = vault_dir.join("relationships");
        let _ = fs::create_dir_all(&storage_dir);
        let storage_path = storage_dir.join("index.json");

        let mut loaded = Vec::new();
        if storage_path.exists() {
            if let Ok(data) = fs::read_to_string(&storage_path) {
                if let Ok(records) = serde_json::from_str::<Vec<RelationshipRecord>>(&data) {
                    loaded = records;
                }
            }
        }

        Self {
            storage_path,
            records: RwLock::new(loaded),
        }
    }

    /// Persists current in-memory state to disk.
    fn persist(&self) -> Result<(), String> {
        let records = self.records.read().map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(&*records).map_err(|e| e.to_string())?;
        fs::write(&self.storage_path, json).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Adds or updates a relationship, ensuring no supersedes cycle is introduced.
    pub fn add_relationship(&self, rel: RelationshipRecord) -> Result<(), String> {
        let mut records = self.records.write().map_err(|e| e.to_string())?;

        // Detect supersedes cycle if adding a supersedes link
        if rel.relationship_type == RelationshipType::Supersedes {
            let mut curr = rel.target_id.as_str();
            let mut visited = std::collections::HashSet::new();
            visited.insert(rel.source_id.as_str());

            while let Some(next_link) = records
                .iter()
                .find(|r| r.relationship_type == RelationshipType::Supersedes && r.source_id == curr)
            {
                if visited.contains(next_link.target_id.as_str()) {
                    return Err(format!(
                        "Cycle detected in supersedes chain: {} -> {}",
                        rel.source_id, rel.target_id
                    ));
                }
                visited.insert(curr);
                curr = next_link.target_id.as_str();
            }
        }

        // Deduplicate: replace existing with same ID or same (source, target, type)
        records.retain(|r| {
            !(r.id == rel.id
                || (r.source_id == rel.source_id
                    && r.target_id == rel.target_id
                    && r.relationship_type == rel.relationship_type))
        });

        records.push(rel);
        drop(records);
        self.persist()?;
        Ok(())
    }

    /// Retrieves all relationships where the object is the source.
    pub fn get_relationships_for_source(&self, source_id: &str) -> Vec<RelationshipRecord> {
        let records = self.records.read().unwrap_or_else(|e| e.into_inner());
        records
            .iter()
            .filter(|r| r.source_id == source_id)
            .cloned()
            .collect()
    }

    /// Retrieves all relationships where the object is the target.
    pub fn get_relationships_for_target(&self, target_id: &str) -> Vec<RelationshipRecord> {
        let records = self.records.read().unwrap_or_else(|e| e.into_inner());
        records
            .iter()
            .filter(|r| r.target_id == target_id)
            .cloned()
            .collect()
    }

    /// Finds relationships connecting two objects in either direction.
    pub fn find_relationships_between(&self, id_a: &str, id_b: &str) -> Vec<RelationshipRecord> {
        let records = self.records.read().unwrap_or_else(|e| e.into_inner());
        records
            .iter()
            .filter(|r| {
                (r.source_id == id_a && r.target_id == id_b)
                    || (r.source_id == id_b && r.target_id == id_a)
            })
            .cloned()
            .collect()
    }

    /// Deletes a relationship by ID.
    pub fn delete_relationship(&self, id: &str) -> Result<bool, String> {
        let mut records = self.records.write().map_err(|e| e.to_string())?;
        let prev_len = records.len();
        records.retain(|r| r.id != id);
        let removed = records.len() < prev_len;
        drop(records);
        if removed {
            self.persist()?;
        }
        Ok(removed)
    }

    /// Returns all stored relationships.
    pub fn list_all(&self) -> Vec<RelationshipRecord> {
        let records = self.records.read().unwrap_or_else(|e| e.into_inner());
        records.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_crud_and_query() {
        let temp_dir = std::env::temp_dir().join(format!("relay_test_rel_{}", uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let store = RelationshipStore::new(&temp_dir);

        let rel1 = RelationshipRecord::new("summary_1", "source_1", RelationshipType::Summarizes).unwrap();
        let rel2 = RelationshipRecord::new("source_1", "repo_1", RelationshipType::BelongsTo).unwrap();

        assert!(store.add_relationship(rel1.clone()).is_ok());
        assert!(store.add_relationship(rel2.clone()).is_ok());

        let from_source = store.get_relationships_for_source("summary_1");
        assert_eq!(from_source.len(), 1);
        assert_eq!(from_source[0].target_id, "source_1");

        let for_target = store.get_relationships_for_target("source_1");
        assert_eq!(for_target.len(), 1);
        assert_eq!(for_target[0].source_id, "summary_1");

        let between = store.find_relationships_between("repo_1", "source_1");
        assert_eq!(between.len(), 1);
        assert_eq!(between[0].relationship_type, RelationshipType::BelongsTo);

        assert!(store.delete_relationship(&rel1.id).unwrap());
        assert_eq!(store.get_relationships_for_source("summary_1").len(), 0);

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_detect_supersedes_cycle() {
        let temp_dir = std::env::temp_dir().join(format!("relay_test_rel_cyc_{}", uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let store = RelationshipStore::new(&temp_dir);

        let rel1 = RelationshipRecord::new("mem_b", "mem_a", RelationshipType::Supersedes).unwrap();
        let rel2 = RelationshipRecord::new("mem_c", "mem_b", RelationshipType::Supersedes).unwrap();
        let cycle = RelationshipRecord::new("mem_a", "mem_c", RelationshipType::Supersedes).unwrap();

        assert!(store.add_relationship(rel1).is_ok());
        assert!(store.add_relationship(rel2).is_ok());
        assert!(store.add_relationship(cycle).is_err());

        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
