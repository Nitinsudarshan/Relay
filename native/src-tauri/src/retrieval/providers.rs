//! Candidate Providers for Relay's Unified Retrieval Layer.
//!
//! Feeds normalized CandidateItems into the deterministic scoring and ranking engine.

use super::model::{CandidateItem, RetrievalProvenance, RetrievalQuery, RetrievalSourceType};
use crate::capture::web::context::{ConversationContext, RepositoryContext};
use crate::meetings_v2::processing::MeetingProcessor;
use crate::meetings_v2::session_store::SessionStore;
use crate::memory::{EpistemicState, MemoryStatus, MemoryStore};
use crate::pipeline::analysis::DerivedPayload;
use crate::relationships::RelationshipStore;
use crate::vault::{VaultManager, VOICE_NOTE_TYPE};

/// Trait implemented by all knowledge stores feeding candidates into Unified Retrieval.
pub trait CandidateProvider: Send + Sync {
    fn source_type(&self) -> RetrievalSourceType;
    fn gather_candidates(&self, query: &RetrievalQuery) -> Vec<CandidateItem>;
}

/// Provider for Vault sources: Scribbles, Voice Notes, Imported Files, and Web Captures.
pub struct VaultProvider<'a> {
    pub vault: &'a VaultManager,
}

impl<'a> VaultProvider<'a> {
    pub fn new(vault: &'a VaultManager) -> Self {
        Self { vault }
    }

    pub fn gather_all(&self, query: &RetrievalQuery) -> Vec<CandidateItem> {
        let mut items = Vec::new();
        let allowed = |st: RetrievalSourceType| -> bool {
            query.filter.source_types.is_empty() || query.filter.source_types.contains(&st)
        };

        // 1. Scribbles
        if allowed(RetrievalSourceType::Scribble) {
            if let Ok(scribbles) = self.vault.list_scribbles() {
                for s in scribbles {
                    let mut topics = s.tags.clone();
                    topics.extend(s.topics.clone());
                    let body = match &s.summary {
                        Some(sum) if !sum.trim().is_empty() => format!("{}\n\n{}", sum.trim(), s.content),
                        _ => s.content.clone(),
                    };
                    let provenance = RetrievalProvenance::new(&s.id, RetrievalSourceType::Scribble);
                    items.push(CandidateItem {
                        id: s.id.clone(),
                        source_type: RetrievalSourceType::Scribble,
                        title: s.title.clone(),
                        content: body,
                        timestamp: Some(s.created_at.clone()),
                        topics,
                        entity_refs: Vec::new(),
                        provenance,
                        metadata: serde_json::json!({ "char_count": s.content.len() }),
                    });
                }
            }
        }

        // 2. Voice Notes
        if allowed(RetrievalSourceType::VoiceNote) {
            if let Ok(notes) = self.vault.list_notes() {
                for n in notes {
                    if n.note_type == VOICE_NOTE_TYPE {
                        let provenance = RetrievalProvenance::new(&n.id, RetrievalSourceType::VoiceNote);
                        items.push(CandidateItem {
                            id: n.id.clone(),
                            source_type: RetrievalSourceType::VoiceNote,
                            title: n.title.clone(),
                            content: n.content.clone(),
                            timestamp: Some(n.created_at.clone()),
                            topics: n.tags.clone(),
                            entity_refs: Vec::new(),
                            provenance,
                            metadata: serde_json::json!({ "note_type": n.note_type }),
                        });
                    }
                }
            }
        }

        // 3. Vault Files & Web Captures
        let check_files = allowed(RetrievalSourceType::File);
        let check_captures = allowed(RetrievalSourceType::Capture);
        if check_files || check_captures {
            let mut all_files = Vec::new();
            if check_files {
                if let Ok(files) = self.vault.list_vault_files() {
                    all_files.extend(files);
                }
            }
            if check_captures {
                if let Ok(captures) = self.vault.list_captures() {
                    all_files.extend(captures);
                }
            }

            for f in all_files {
                let is_capture = f.is_capture();
                let st = if is_capture {
                    RetrievalSourceType::Capture
                } else {
                    RetrievalSourceType::File
                };

                if !allowed(st) {
                    continue;
                }

                let mut topics = f.tags.clone();
                topics.extend(f.topics.clone());
                let title = if is_capture {
                    f.capture
                        .as_ref()
                        .map(|c| c.page_title.as_str())
                        .unwrap_or(&f.original_filename)
                } else {
                    &f.original_filename
                };

                let body = match &f.summary {
                    Some(sum) if !sum.trim().is_empty() => format!("{}\n\n{}", sum.trim(), f.content),
                    _ => f.content.clone(),
                };

                let mut prov = RetrievalProvenance::new(&f.id, st);
                if let Some(c) = &f.capture {
                    prov = prov.with_origin(&c.url).with_capture(&f.id);
                } else if !f.last_known_source_path.is_empty() {
                    prov = prov.with_origin(&f.last_known_source_path);
                }

                items.push(CandidateItem {
                    id: f.id.clone(),
                    source_type: st,
                    title: title.to_string(),
                    content: body,
                    timestamp: Some(f.created_at.clone()),
                    topics,
                    entity_refs: Vec::new(),
                    provenance: prov,
                    metadata: serde_json::json!({ "original_filename": f.original_filename }),
                });
            }
        }

        items
    }
}

/// Provider for Derived Data: RepositoryContext, ConversationContext, and Summaries.
pub struct DerivedDataProvider<'a> {
    pub vault: &'a VaultManager,
}

impl<'a> DerivedDataProvider<'a> {
    pub fn new(vault: &'a VaultManager) -> Self {
        Self { vault }
    }

    pub fn gather(&self, query: &RetrievalQuery) -> Vec<CandidateItem> {
        let allowed = query.filter.source_types.is_empty()
            || query.filter.source_types.contains(&RetrievalSourceType::DerivedArtifact);
        if !allowed {
            return Vec::new();
        }

        let mut items = Vec::new();
        let mut all_files = Vec::new();
        if let Ok(files) = self.vault.list_vault_files() {
            all_files.extend(files);
        }
        if let Ok(caps) = self.vault.list_captures() {
            all_files.extend(caps);
        }

        for f in all_files {
                // Check for Context derived data
                if let Ok(Some(derived_ctx)) = self.vault.get_derived_data(&f.id, crate::pipeline::analysis::DerivedType::Context) {
                    let (title, content) = match &derived_ctx.payload {
                        DerivedPayload::Structured(val) => {
                            if let Ok(repo_ctx) = serde_json::from_value::<RepositoryContext>(val.clone()) {
                                let title = format!("{} [Repository Context]", repo_ctx.repository_name);
                                let mut text = String::new();
                                text.push_str(&format!("Objective: {}\n", repo_ctx.objective));
                                if !repo_ctx.stack.is_empty() {
                                    text.push_str(&format!("Stack: {}\n", repo_ctx.stack.join(", ")));
                                }
                                if !repo_ctx.features.is_empty() {
                                    text.push_str(&format!("Features / Ecosystem: {}\n", repo_ctx.features.join(", ")));
                                }
                                if !repo_ctx.user_base.is_empty() {
                                    text.push_str(&format!("User Base: {}\n", repo_ctx.user_base.join(", ")));
                                }
                                if let Some(ref lic) = repo_ctx.licensing {
                                    text.push_str(&format!("Licensing: {}\n", lic));
                                }
                                (title, text)
                            } else if let Ok(conv_ctx) = serde_json::from_value::<ConversationContext>(val.clone()) {
                                let title = format!("{} [Conversation Context]", conv_ctx.title);
                                let mut text = String::new();
                                text.push_str(&format!("Objective: {}\n", conv_ctx.objective));
                                text.push_str(&format!("State: {}\n", conv_ctx.current_state));
                                for d in &conv_ctx.decisions {
                                    text.push_str(&format!("Decision: {}\n", d.decision));
                                }
                                for r in &conv_ctx.requirements {
                                    text.push_str(&format!("Requirement: {}\n", r.statement));
                                }
                                for c in &conv_ctx.constraints {
                                    text.push_str(&format!("Constraint: {}\n", c.statement));
                                }
                                (title, text)
                            } else {
                                (format!("{} Context", f.original_filename), val.to_string())
                            }
                        }
                        DerivedPayload::Text(text) => {
                            (format!("{} Context", f.original_filename), text.clone())
                        }
                    };

                    if !content.trim().is_empty() {
                        let mut prov = RetrievalProvenance::new(&f.id, RetrievalSourceType::DerivedArtifact)
                            .with_derived(&derived_ctx.id);
                        if let Some(c) = &f.capture {
                            prov = prov.with_origin(&c.url).with_capture(&f.id);
                        }

                        items.push(CandidateItem {
                            id: format!("derived_ctx_{}", f.id),
                            source_type: RetrievalSourceType::DerivedArtifact,
                            title,
                            content,
                            timestamp: Some(derived_ctx.created_at.clone()),
                            topics: vec!["context".to_string(), "repository".to_string()],
                            entity_refs: Vec::new(),
                            provenance: prov,
                            metadata: serde_json::json!({
                                "source_file_id": f.id,
                                "derived_type": "context",
                                "version": derived_ctx.version,
                            }),
                        });
                    }
                }

                // Check for Summary derived data
                if let Ok(Some(derived_sum)) = self.vault.get_derived_data(&f.id, crate::pipeline::analysis::DerivedType::Summary) {
                    let sum_text = match &derived_sum.payload {
                        DerivedPayload::Text(t) => t.clone(),
                        DerivedPayload::Structured(v) => v.to_string(),
                    };

                    if !sum_text.trim().is_empty() {
                        let prov = RetrievalProvenance::new(&f.id, RetrievalSourceType::DerivedArtifact)
                            .with_derived(&derived_sum.id);

                        items.push(CandidateItem {
                            id: format!("derived_sum_{}", f.id),
                            source_type: RetrievalSourceType::DerivedArtifact,
                            title: format!("{} [Summary]", f.original_filename),
                            content: sum_text,
                            timestamp: Some(derived_sum.created_at.clone()),
                            topics: vec!["summary".to_string()],
                            entity_refs: Vec::new(),
                            provenance: prov,
                            metadata: serde_json::json!({
                                "source_file_id": f.id,
                                "derived_type": "summary",
                            }),
                        });
                    }
                }
            }
        items
    }
}

/// Provider for Memory candidates.
pub struct MemoryProvider<'a> {
    pub memory_store: &'a MemoryStore,
}

impl<'a> MemoryProvider<'a> {
    pub fn new(memory_store: &'a MemoryStore) -> Self {
        Self { memory_store }
    }

    pub fn gather(&self, query: &RetrievalQuery) -> Vec<CandidateItem> {
        let allowed = query.filter.source_types.is_empty()
            || query.filter.source_types.contains(&RetrievalSourceType::Memory);
        if !allowed {
            return Vec::new();
        }

        let mut items = Vec::new();
        let memories = self.memory_store.list_active(None);
        for m in memories {
            // Strictly exclude non-current, superseded, deleted, or known-false memories
            if m.status != MemoryStatus::Active || m.epistemic_state != EpistemicState::Current {
                continue;
            }

            let primary_prov = m.provenance.first();
            let mut prov = RetrievalProvenance::new(&m.id, RetrievalSourceType::Memory);
            if let Some(p) = primary_prov {
                prov = prov.with_evidence(&p.evidence);
            }

            items.push(CandidateItem {
                id: m.id.clone(),
                source_type: RetrievalSourceType::Memory,
                title: format!("{} [{}]", m.subject, m.memory_type.as_str()),
                content: m.content.clone(),
                timestamp: Some(m.created_at.clone()),
                topics: vec![m.memory_type.as_str().to_string(), m.subject.clone()],
                entity_refs: Vec::new(),
                provenance: prov,
                metadata: serde_json::json!({
                    "memory_type": m.memory_type.as_str(),
                    "confidence": m.confidence,
                }),
            });
        }
        items
    }
}

/// Provider for Meetings from SessionStore and MeetingProcessor.
pub struct MeetingProvider<'a> {
    pub session_store: Option<&'a SessionStore>,
    pub _meeting_processor: Option<&'a MeetingProcessor>,
}

impl<'a> MeetingProvider<'a> {
    pub fn new(
        session_store: Option<&'a SessionStore>,
        meeting_processor: Option<&'a MeetingProcessor>,
    ) -> Self {
        Self {
            session_store,
            _meeting_processor: meeting_processor,
        }
    }

    pub fn gather(&self, query: &RetrievalQuery) -> Vec<CandidateItem> {
        let allowed = query.filter.source_types.is_empty()
            || query.filter.source_types.contains(&RetrievalSourceType::Meeting);
        if !allowed {
            return Vec::new();
        }

        let mut items = Vec::new();
        if let Some(store) = self.session_store {
            if let Ok(sessions) = store.list_sessions() {
                for s in sessions {
                    let title = if s.title.is_empty() {
                        "Untitled Meeting".to_string()
                    } else {
                        s.title.clone()
                    };

                    let mut body = s.summary.clone().unwrap_or_default();
                    if !s.action_items.is_empty() {
                        body.push_str("\n\nAction Items:\n");
                        for item in &s.action_items {
                            body.push_str(&format!("- {}\n", item));
                        }
                    }
                    if body.trim().is_empty() {
                        body = format!("Meeting session {}", s.id);
                    }

                    let prov = RetrievalProvenance::new(&s.id, RetrievalSourceType::Meeting);
                    items.push(CandidateItem {
                        id: s.id.clone(),
                        source_type: RetrievalSourceType::Meeting,
                        title,
                        content: body,
                        timestamp: Some(s.created_at.clone()),
                        topics: vec!["meeting".to_string()],
                        entity_refs: Vec::new(),
                        provenance: prov,
                        metadata: serde_json::json!({ "action_items": s.action_items }),
                    });
                }
            }
        }
        items
    }
}

/// Provider for Relationships and Entity graph expansions.
pub struct RelationshipProvider<'a> {
    pub relationship_store: &'a RelationshipStore,
}

impl<'a> RelationshipProvider<'a> {
    pub fn new(relationship_store: &'a RelationshipStore) -> Self {
        Self { relationship_store }
    }

    pub fn expand_for_sources(&self, source_ids: &[String]) -> Vec<crate::relationships::RelationshipRecord> {
        let mut expanded = Vec::new();
        for sid in source_ids {
            let rels = self.relationship_store.get_relationships_for_source(sid);
            for r in rels {
                if !expanded.contains(&r) {
                    expanded.push(r);
                }
            }
        }
        expanded
    }
}
