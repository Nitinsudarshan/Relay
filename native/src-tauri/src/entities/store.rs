//! Persistent store and index for canonical resolved entities.
//!
//! Stores entities and mentions in atomic JSON storage, supporting fast candidate
//! lookup during retrieval and context assembly.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use super::model::{EntityCategory, ResolvedEntity};

pub struct EntityStore {
    storage_path: PathBuf,
    entities: RwLock<Vec<ResolvedEntity>>,
}

impl EntityStore {
    /// Creates or opens an EntityStore at the given vault directory.
    pub fn new(vault_dir: &Path) -> Self {
        let entities_dir = vault_dir.join("entities");
        let _ = fs::create_dir_all(&entities_dir);
        let storage_path = entities_dir.join("index.json");

        let mut loaded = Vec::new();
        if storage_path.exists() {
            if let Ok(data) = fs::read_to_string(&storage_path) {
                if let Ok(records) = serde_json::from_str::<Vec<ResolvedEntity>>(&data) {
                    loaded = records;
                } else {
                    tracing::warn!("Entities index was malformed; recovering from clean state");
                }
            }
        }

        Self {
            storage_path,
            entities: RwLock::new(loaded),
        }
    }

    /// Persists current state to disk atomically.
    fn persist(&self) -> Result<(), String> {
        let entities = self.entities.read().map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(&*entities).map_err(|e| e.to_string())?;
        let tmp_path = self.storage_path.with_extension("tmp");
        fs::write(&tmp_path, json.as_bytes()).map_err(|e| e.to_string())?;
        fs::rename(&tmp_path, &self.storage_path).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Stores or merges a single resolved entity into the store.
    pub fn store_entity(&self, entity: ResolvedEntity) -> Result<(), String> {
        let mut list = self.entities.write().map_err(|e| e.to_string())?;

        // Check if an existing entity shares ID or canonical name + category
        let existing_idx = list.iter().position(|e| {
            e.id == entity.id
                || (e.category == entity.category
                    && e.canonical_name.to_lowercase() == entity.canonical_name.to_lowercase())
        });

        if let Some(idx) = existing_idx {
            let existing = &mut list[idx];
            for a in entity.aliases {
                if !existing.aliases.contains(&a) && existing.canonical_name != a {
                    existing.aliases.push(a);
                }
            }
            for sid in entity.source_identifiers {
                if !existing.source_identifiers.contains(&sid) {
                    existing.source_identifiers.push(sid);
                }
            }
            for u in entity.urls {
                if !existing.urls.contains(&u) {
                    existing.urls.push(u);
                }
            }
            for m in entity.mentions {
                if !existing.mentions.contains(&m) {
                    existing.mentions.push(m);
                }
            }
        } else {
            list.push(entity);
        }

        drop(list);
        self.persist()?;
        Ok(())
    }

    /// Stores a batch of resolved entities.
    pub fn store_entities(&self, entities: &[ResolvedEntity]) -> Result<(), String> {
        for ent in entities {
            self.store_entity(ent.clone())?;
        }
        Ok(())
    }

    /// Retrieves an entity by its ID.
    pub fn get_entity(&self, id: &str) -> Option<ResolvedEntity> {
        let list = self.entities.read().unwrap_or_else(|e| e.into_inner());
        list.iter().find(|e| e.id == id).cloned()
    }

    /// Finds an entity by canonical name or alias.
    pub fn find_by_name(&self, name: &str) -> Option<ResolvedEntity> {
        let lower = name.trim().to_lowercase();
        let list = self.entities.read().unwrap_or_else(|e| e.into_inner());
        list.iter()
            .find(|e| {
                e.canonical_name.to_lowercase() == lower
                    || e.aliases.iter().any(|a| a.to_lowercase() == lower)
            })
            .cloned()
    }

    /// Finds an entity by repo identifier or URL.
    pub fn find_by_identifier(&self, identifier: &str) -> Option<ResolvedEntity> {
        let lower = identifier.trim().to_lowercase();
        let list = self.entities.read().unwrap_or_else(|e| e.into_inner());
        list.iter()
            .find(|e| {
                e.source_identifiers.iter().any(|sid| sid.to_lowercase() == lower)
                    || e.urls.iter().any(|u| u.to_lowercase() == lower)
            })
            .cloned()
    }

    /// Lists all entities in the store.
    pub fn list_all(&self) -> Vec<ResolvedEntity> {
        let list = self.entities.read().unwrap_or_else(|e| e.into_inner());
        list.clone()
    }

    /// Lists entities matching a given category.
    pub fn list_by_category(&self, category: EntityCategory) -> Vec<ResolvedEntity> {
        let list = self.entities.read().unwrap_or_else(|e| e.into_inner());
        list.iter().filter(|e| e.category == category).cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::model::EntityMention;

    #[test]
    fn test_entity_store_atomic_persistence_and_merge() {
        let temp_dir = std::env::temp_dir().join(format!("relay_ent_store_{}", uuid::Uuid::new_v4()));
        let _ = fs::create_dir_all(&temp_dir);

        let store = EntityStore::new(&temp_dir);
        let mention1 = EntityMention {
            source_id: "src_1".to_string(),
            evidence: "Orca is an agent workflow platform.".to_string(),
            confidence: 0.95,
            timestamp: None,
        };

        let ent1 = ResolvedEntity {
            id: "ent_orca".to_string(),
            canonical_name: "Orca".to_string(),
            category: EntityCategory::Project,
            aliases: vec!["Orca Engine".to_string()],
            source_identifiers: vec!["stablyai/orca".to_string()],
            urls: vec!["https://github.com/stablyai/orca".to_string()],
            confidence: 0.95,
            mentions: vec![mention1],
        };

        store.store_entity(ent1).unwrap();

        // Verify retrieval by identifier
        let found = store.find_by_identifier("stablyai/orca");
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.canonical_name, "Orca");

        // Merge second mention
        let mention2 = EntityMention {
            source_id: "src_2".to_string(),
            evidence: "Using Orca for parallel workers.".to_string(),
            confidence: 0.90,
            timestamp: None,
        };
        let ent2 = ResolvedEntity {
            id: "ent_orca_2".to_string(),
            canonical_name: "Orca".to_string(),
            category: EntityCategory::Project,
            aliases: vec!["Orca Workflows".to_string()],
            source_identifiers: vec![],
            urls: vec![],
            confidence: 0.90,
            mentions: vec![mention2],
        };
        store.store_entity(ent2).unwrap();

        let updated = store.find_by_name("Orca").unwrap();
        assert_eq!(updated.mentions.len(), 2);
        assert!(updated.aliases.contains(&"Orca Workflows".to_string()));

        let _ = fs::remove_dir_all(temp_dir);
    }
}
