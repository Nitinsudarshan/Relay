//! Standard Context Assembly Service.
//!
//! Provides the single shared pipeline used by UI, Talkback, MCP, and pipelines
//! to assemble bounded, provenance-preserving Context Packs.

use super::pack::{ContextPack, ContextPackItem, ContextPackType};
use crate::entities::{EntityExtractor, EntityResolver, EntityStore};
use crate::meetings_v2::processing::MeetingProcessor;
use crate::meetings_v2::session_store::SessionStore;
use crate::memory::MemoryStore;
use crate::relationships::RelationshipStore;
use crate::retrieval::{RetrievalFilter, RetrievalQuery, UnifiedRetrievalService};
use crate::vault::VaultManager;

pub struct ContextAssemblyRequest {
    pub query: String,
    pub intent: Option<String>,
    pub pack_type: Option<ContextPackType>,
    pub char_budget: Option<usize>,
    pub filter: Option<RetrievalFilter>,
}

impl ContextAssemblyRequest {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            intent: None,
            pack_type: None,
            char_budget: None,
            filter: None,
        }
    }

    pub fn with_pack_type(mut self, pack_type: ContextPackType) -> Self {
        self.pack_type = Some(pack_type);
        self
    }

    pub fn with_char_budget(mut self, budget: usize) -> Self {
        self.char_budget = Some(budget);
        self
    }

    pub fn with_filter(mut self, filter: RetrievalFilter) -> Self {
        self.filter = Some(filter);
        self
    }
}

pub struct ContextAssemblyService;

impl ContextAssemblyService {
    /// Assembles a complete Context Pack across all knowledge dimensions.
    pub fn assemble(
        vault: &VaultManager,
        memory_store: Option<&MemoryStore>,
        relationship_store: Option<&RelationshipStore>,
        request: &ContextAssemblyRequest,
    ) -> ContextPack {
        Self::assemble_full(vault, memory_store, relationship_store, None, None, None, request)
    }

    /// Full assembly entrypoint receiving all stores for deep integration.
    pub fn assemble_full(
        vault: &VaultManager,
        memory_store: Option<&MemoryStore>,
        relationship_store: Option<&RelationshipStore>,
        entity_store: Option<&EntityStore>,
        session_store: Option<&SessionStore>,
        meeting_processor: Option<&MeetingProcessor>,
        request: &ContextAssemblyRequest,
    ) -> ContextPack {
        let budget = request.char_budget.unwrap_or(8_000);
        let pack_type = request.pack_type.unwrap_or(ContextPackType::General);
        let mut pack = ContextPack::new(pack_type, &request.query, budget);
        pack.intent = request.intent.clone();

        // 1. Run Unified Retrieval with all knowledge stores
        let mut ret_query = RetrievalQuery::new(&request.query)
            .with_char_budget(budget);
        if let Some(ref f) = request.filter {
            ret_query.filter = f.clone();
        }

        let ret_result = UnifiedRetrievalService::search_with_memory(
            vault,
            memory_store,
            session_store,
            meeting_processor,
            &ret_query,
        );

        // 2. Domain-Aware Item Prioritization:
        // E.g., for a Repository pack, prioritize DerivedArtifact (RepositoryContext) over raw files.
        let mut sorted_items = ret_result.items;
        match pack_type {
            ContextPackType::Repository => {
                sorted_items.sort_by(|a, b| {
                    let a_is_derived = a.source_type == crate::retrieval::RetrievalSourceType::DerivedArtifact;
                    let b_is_derived = b.source_type == crate::retrieval::RetrievalSourceType::DerivedArtifact;
                    b_is_derived.cmp(&a_is_derived).then_with(|| {
                        b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
                    })
                });
            }
            ContextPackType::Meeting => {
                sorted_items.sort_by(|a, b| {
                    let a_is_mtg = a.source_type == crate::retrieval::RetrievalSourceType::Meeting;
                    let b_is_mtg = b.source_type == crate::retrieval::RetrievalSourceType::Meeting;
                    b_is_mtg.cmp(&a_is_mtg).then_with(|| {
                        b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
                    })
                });
            }
            _ => {}
        }

        // 3. Add retrieved items up to budget
        let mut aggregated_content = String::new();
        let mut added_source_ids = Vec::new();

        for item in sorted_items {
            let prov_str = item.provenance.source_origin
                .unwrap_or_else(|| item.provenance.source_id.clone());

            let pack_item = ContextPackItem {
                id: item.id.clone(),
                source_id: item.provenance.source_id.clone(),
                item_type: item.source_type.as_str().to_string(),
                title: item.title,
                content: item.snippet,
                is_external: item.source_type.is_external(),
                provenance: prov_str,
            };

            aggregated_content.push_str(&pack_item.content);
            aggregated_content.push(' ');
            added_source_ids.push(item.id.clone());

            if !pack.try_add_item(pack_item) {
                break;
            }

            // 4. Graph expansion: collect linked relationships
            if let Some(rel_store) = relationship_store {
                let rels = rel_store.get_relationships_for_source(&item.id);
                for r in rels {
                    if !pack.relationships.contains(&r) {
                        pack.relationships.push(r);
                    }
                }
            }
        }

        // 5. Entities: query EntityStore if available, and extract from aggregated content
        let mut candidate_entities = Vec::new();
        if let Some(estore) = entity_store {
            // Find entities matching query or in store
            let terms = crate::retrieval::tokenize(&request.query);
            for term in terms {
                if let Some(ent) = estore.find_by_name(&term) {
                    if !candidate_entities.contains(&ent) {
                        candidate_entities.push(ent);
                    }
                }
                if let Some(ent) = estore.find_by_identifier(&term) {
                    if !candidate_entities.contains(&ent) {
                        candidate_entities.push(ent);
                    }
                }
            }
        }

        if !aggregated_content.is_empty() {
            let extracted = EntityExtractor::extract_deterministic(&pack.id, &aggregated_content);
            let resolved = EntityResolver::resolve(&extracted);
            for ent in resolved {
                if !candidate_entities.iter().any(|e| e.canonical_name.to_lowercase() == ent.canonical_name.to_lowercase()) {
                    candidate_entities.push(ent);
                }
            }
        }
        pack.entities = candidate_entities;

        // 6. Memory: add active memories from MemoryStore
        if let Some(mem_store) = memory_store {
            let active_memories = mem_store.list_active(None);
            let query_terms = crate::retrieval::tokenize(&request.query);
            for mem in active_memories {
                let mem_text = format!("{} {}", mem.subject, mem.content).to_lowercase();
                let matches = query_terms.is_empty()
                    || query_terms.iter().any(|t| mem_text.contains(t));
                if matches && !pack.memories.iter().any(|m| m.id == mem.id) {
                    pack.memories.push(mem);
                }
            }
        }

        pack
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_assembly_bounded_budget() {
        let temp_dir = std::env::temp_dir().join(format!("relay_test_ctx_{}", uuid::Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let vault = VaultManager::new(temp_dir.clone());
        let mem_store = MemoryStore::new(&temp_dir);
        let rel_store = RelationshipStore::new(&temp_dir);

        // Add a scribble
        let mut scribble = crate::vault::Scribble::new_text(
            "Relay is designed with modular architecture for local intelligence.",
            Some("Relay Design"),
        );
        scribble.id = "sc_1".to_string();
        vault.save_scribble(&scribble).unwrap();

        let req = ContextAssemblyRequest::new("Relay architecture")
            .with_pack_type(ContextPackType::Project)
            .with_char_budget(1000);

        let pack = ContextAssemblyService::assemble(
            &vault,
            Some(&mem_store),
            Some(&rel_store),
            &req,
        );

        assert_eq!(pack.pack_type, ContextPackType::Project);
        assert!(!pack.items.is_empty());
        assert!(pack.total_chars <= 1000);

        let prompt = pack.to_prompt_context();
        assert!(prompt.contains("RELAY CONTEXT PACK"));
        assert!(prompt.contains("Relay Design"));

        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
