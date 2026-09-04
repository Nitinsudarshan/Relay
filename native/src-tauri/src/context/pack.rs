//! Reusable, bounded Context Packs.
//!
//! A Context Pack is a task-specific projection of Relay knowledge combining
//! source content, summaries, entities, memories, and relationships.

use serde::{Deserialize, Serialize};

use crate::entities::ResolvedEntity;
use crate::memory::MemoryItem;
use crate::relationships::RelationshipRecord;

/// The target domain/type of a Context Pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextPackType {
    Repository,
    Meeting,
    Project,
    Conversation,
    Document,
    General,
}

impl ContextPackType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Repository => "repository",
            Self::Meeting => "meeting",
            Self::Project => "project",
            Self::Conversation => "conversation",
            Self::Document => "document",
            Self::General => "general",
        }
    }
}

/// A discrete item included in a Context Pack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextPackItem {
    pub id: String,
    pub source_id: String,
    pub item_type: String, // "evidence", "summary", "structured_context", "memory"
    pub title: String,
    pub content: String,
    pub is_external: bool,
    pub provenance: String,
}

/// A bounded, provenance-retaining projection of Relay knowledge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextPack {
    pub id: String,
    pub pack_type: ContextPackType,
    pub query: String,
    #[serde(default)]
    pub intent: Option<String>,
    #[serde(default)]
    pub items: Vec<ContextPackItem>,
    #[serde(default)]
    pub entities: Vec<ResolvedEntity>,
    #[serde(default)]
    pub memories: Vec<MemoryItem>,
    #[serde(default)]
    pub relationships: Vec<RelationshipRecord>,
    pub char_budget: usize,
    pub total_chars: usize,
    pub created_at: String,
}

impl ContextPack {
    pub fn new(
        pack_type: ContextPackType,
        query: impl Into<String>,
        char_budget: usize,
    ) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        let q = query.into();
        let id = format!("pack_{}_{}", pack_type.as_str(), uuid::Uuid::new_v4());
        Self {
            id,
            pack_type,
            query: q,
            intent: None,
            items: Vec::new(),
            entities: Vec::new(),
            memories: Vec::new(),
            relationships: Vec::new(),
            char_budget,
            total_chars: 0,
            created_at: now,
        }
    }

    /// Appends an item if it fits within the remaining character budget.
    pub fn try_add_item(&mut self, item: ContextPackItem) -> bool {
        let item_len = item.content.len();
        if self.total_chars + item_len > self.char_budget && !self.items.is_empty() {
            return false;
        }
        self.total_chars += item_len;
        self.items.push(item);
        true
    }

    /// Formats this context pack as an LLM prompt block with provenance and external attribution.
    pub fn to_prompt_context(&self) -> String {
        let mut out = String::new();
        out.push_str("=== RELAY CONTEXT PACK ===\n");
        out.push_str(&format!("Target: {}\n", self.pack_type.as_str()));
        out.push_str(&format!("Query: {}\n\n", self.query));

        // 1. Active Memories
        if !self.memories.is_empty() {
            out.push_str("--- RELEVANT USER MEMORY ---\n");
            for mem in &self.memories {
                out.push_str(&format!("- [{}] {}: {}\n", mem.memory_type.as_str(), mem.subject, mem.content));
            }
            out.push('\n');
        }

        // 2. Resolved Entities
        if !self.entities.is_empty() {
            out.push_str("--- KNOWN ENTITIES ---\n");
            for ent in &self.entities {
                let alias_str = if ent.aliases.is_empty() {
                    String::new()
                } else {
                    format!(" (aliases: {})", ent.aliases.join(", "))
                };
                out.push_str(&format!("- {} [{}{}]\n", ent.canonical_name, ent.category.as_str(), alias_str));
            }
            out.push('\n');
        }

        // 3. Grounded Evidence & Summaries
        if !self.items.is_empty() {
            out.push_str("--- RETRIEVED EVIDENCE & SOURCES ---\n");
            for (idx, item) in self.items.iter().enumerate() {
                let ext_flag = if item.is_external { " [EXTERNAL]" } else { "" };
                out.push_str(&format!(
                    "[{}] {}{} (from: {})\n{}\n\n",
                    idx + 1,
                    item.title,
                    ext_flag,
                    item.provenance,
                    item.content.trim()
                ));
            }
        }

        out
    }

    /// Formats a concise speech-oriented context block for Talkback.
    pub fn to_talkback_context(&self) -> String {
        let mut out = String::new();
        for item in &self.items {
            let label = if item.is_external { "Captured web evidence" } else { "Your Relay note" };
            out.push_str(&format!("{}: {}\n{}\n\n", label, item.title, item.content.trim()));
        }
        for mem in &self.memories {
            out.push_str(&format!("Remembered {}: {}\n", mem.subject, mem.content.trim()));
        }
        out
    }
}
