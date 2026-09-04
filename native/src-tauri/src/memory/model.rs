//! Explicit, provenance-aware Memory Layer models.

use serde::{Deserialize, Serialize};

/// Supported memory categories Relay learns and maintains over time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    Fact,
    Preference,
    Decision,
    ProjectContext,
    Relationship,
    Instruction,
}

impl MemoryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Fact => "fact",
            Self::Preference => "preference",
            Self::Decision => "decision",
            Self::ProjectContext => "project_context",
            Self::Relationship => "relationship",
            Self::Instruction => "instruction",
        }
    }
}

/// Lifecycle status of a memory item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryStatus {
    /// Active memory considered current and valid.
    Active,
    /// Superseded by a newer, more current memory.
    Superseded,
    /// Archived memory retained for history but not prioritized in active recall.
    Archived,
    /// Soft-deleted memory.
    Deleted,
}

/// Epistemic state distinguishing absence of evidence from known falsehoods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EpistemicState {
    /// Actively believed to be true based on evidence.
    Current,
    /// Was true previously, but has since been replaced or expired.
    NoLongerCurrent,
    /// Directly contradicted or refuted by evidence.
    KnownFalse,
    /// Tentative or unverified inference.
    Unverified,
}

/// Evidence and provenance backing why Relay holds a memory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryProvenance {
    pub source_id: String,
    pub source_type: String,
    pub evidence: String,
    pub confidence: f32,
    pub extracted_by: String, // e.g. "user", "deterministic_extractor", "analysis"
}

/// An explicit, versioned, provenance-grounded memory record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryItem {
    pub id: String,
    pub memory_type: MemoryType,
    pub subject: String,
    pub content: String,
    pub status: MemoryStatus,
    pub epistemic_state: EpistemicState,
    pub confidence: f32,
    #[serde(default)]
    pub provenance: Vec<MemoryProvenance>,
    #[serde(default)]
    pub superseded_by: Option<String>,
    #[serde(default)]
    pub supersedes_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

impl MemoryItem {
    pub fn new(
        memory_type: MemoryType,
        subject: impl Into<String>,
        content: impl Into<String>,
        provenance: MemoryProvenance,
    ) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        let sub = subject.into();
        let id = format!("mem_{}_{}", memory_type.as_str(), uuid::Uuid::new_v4());
        Self {
            id,
            memory_type,
            subject: sub,
            content: content.into(),
            status: MemoryStatus::Active,
            epistemic_state: EpistemicState::Current,
            confidence: provenance.confidence,
            provenance: vec![provenance],
            superseded_by: None,
            supersedes_id: None,
            created_at: now.clone(),
            updated_at: now,
            metadata: None,
        }
    }

    /// Creates a successor memory that supersedes this memory.
    pub fn supersede(
        &mut self,
        new_content: impl Into<String>,
        provenance: MemoryProvenance,
    ) -> MemoryItem {
        let now = chrono::Utc::now().to_rfc3339();
        let new_id = format!("mem_{}_{}", self.memory_type.as_str(), uuid::Uuid::new_v4());

        // Update self as superseded
        self.status = MemoryStatus::Superseded;
        self.epistemic_state = EpistemicState::NoLongerCurrent;
        self.superseded_by = Some(new_id.clone());
        self.updated_at = now.clone();

        MemoryItem {
            id: new_id,
            memory_type: self.memory_type,
            subject: self.subject.clone(),
            content: new_content.into(),
            status: MemoryStatus::Active,
            epistemic_state: EpistemicState::Current,
            confidence: provenance.confidence,
            provenance: vec![provenance],
            superseded_by: None,
            supersedes_id: Some(self.id.clone()),
            created_at: now.clone(),
            updated_at: now,
            metadata: None,
        }
    }
}
