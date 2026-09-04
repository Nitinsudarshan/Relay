//! Data model for Relay's Unified Retrieval Layer.
//!
//! Provides a unified query contract across all Relay sources (notes, scribbles,
//! files, web captures, meetings, memories, and derived artifacts) with full
//! provenance tracing.

use serde::{Deserialize, Serialize};

/// Source types known to Unified Retrieval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalSourceType {
    VoiceNote,
    Scribble,
    Meeting,
    MeetingFacts,
    File,
    Capture,
    Memory,
    DerivedArtifact,
    Entity,
}

impl RetrievalSourceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::VoiceNote => "voice_note",
            Self::Scribble => "scribble",
            Self::Meeting => "meeting",
            Self::MeetingFacts => "meeting_facts",
            Self::File => "file",
            Self::Capture => "capture",
            Self::Memory => "memory",
            Self::DerivedArtifact => "derived_artifact",
            Self::Entity => "entity",
        }
    }

    /// Whether this source is material acquired externally from web/browser.
    pub fn is_external(&self) -> bool {
        matches!(self, Self::Capture)
    }

    /// Priority/weight multiplier for ranking.
    pub fn default_weight(&self) -> f32 {
        match self {
            Self::Memory => 1.30,
            Self::DerivedArtifact => 1.25,
            Self::MeetingFacts => 1.20,
            Self::Entity => 1.15,
            Self::Scribble => 1.10,
            Self::Capture => 1.05,
            Self::File => 1.05,
            Self::Meeting => 1.00,
            Self::VoiceNote => 0.95,
        }
    }
}

/// Provenance trace linking retrieved content back to its source chain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetrievalProvenance {
    /// ID of the root source (e.g. file ID, capture ID, note ID).
    pub source_id: String,
    /// Type of the root source.
    pub source_type: RetrievalSourceType,
    /// Origin path or URL if known (e.g., captured URL or local file path).
    #[serde(default)]
    pub source_origin: Option<String>,
    /// Intermediate capture ID if this originated from a web capture.
    #[serde(default)]
    pub capture_id: Option<String>,
    /// Intermediate derived artifact ID if retrieved from analysis/summary/extraction.
    #[serde(default)]
    pub derived_id: Option<String>,
    /// Exact excerpt or evidence sentence supporting the retrieval.
    #[serde(default)]
    pub evidence: Option<String>,
}

impl RetrievalProvenance {
    pub fn new(source_id: impl Into<String>, source_type: RetrievalSourceType) -> Self {
        Self {
            source_id: source_id.into(),
            source_type,
            source_origin: None,
            capture_id: None,
            derived_id: None,
            evidence: None,
        }
    }

    pub fn with_origin(mut self, origin: impl Into<String>) -> Self {
        self.source_origin = Some(origin.into());
        self
    }

    pub fn with_capture(mut self, capture_id: impl Into<String>) -> Self {
        self.capture_id = Some(capture_id.into());
        self
    }

    pub fn with_derived(mut self, derived_id: impl Into<String>) -> Self {
        self.derived_id = Some(derived_id.into());
        self
    }

    pub fn with_evidence(mut self, evidence: impl Into<String>) -> Self {
        self.evidence = Some(evidence.into());
        self
    }
}

/// Category of match detected during multi-signal scoring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchType {
    ExactPhrase,
    TitleMatch,
    HeadingMatch,
    TopicMatch,
    EntityMatch,
    DerivedAbstraction,
    TermCoverage,
    RecencyOnly,
}

/// Granular explainability explaining why an item was selected and ranked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Explainability {
    pub matched_terms: Vec<String>,
    pub match_types: Vec<MatchType>,
    pub why: Vec<String>,
    pub base_score: f32,
    pub boosts_applied: Vec<String>,
    pub final_score: f32,
}

/// Normalized candidate passed to the scoring and ranking engine.
#[derive(Debug, Clone, PartialEq)]
pub struct CandidateItem {
    pub id: String,
    pub source_type: RetrievalSourceType,
    pub title: String,
    pub content: String,
    pub timestamp: Option<String>,
    pub topics: Vec<String>,
    pub entity_refs: Vec<String>,
    pub provenance: RetrievalProvenance,
    pub metadata: serde_json::Value,
}

/// Time window filter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TimeFilter {
    pub created_after: Option<String>,
    pub created_before: Option<String>,
}

/// Metadata and facet filtering for retrieval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RetrievalFilter {
    /// Whitelist of allowed source types. If empty, all source types are eligible.
    #[serde(default)]
    pub source_types: Vec<RetrievalSourceType>,
    /// Filter by tags or topics.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Filter by time window.
    #[serde(default)]
    pub time_filter: Option<TimeFilter>,
    /// Filter by specific entity or subject keys.
    #[serde(default)]
    pub entity_keys: Vec<String>,
}

/// A unified retrieval request across Relay knowledge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetrievalQuery {
    pub text: String,
    #[serde(default)]
    pub filter: RetrievalFilter,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub char_budget: Option<usize>,
    #[serde(default)]
    pub include_evidence: bool,
}

impl RetrievalQuery {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            filter: RetrievalFilter::default(),
            limit: Some(20),
            char_budget: None,
            include_evidence: true,
        }
    }

    pub fn with_source_types(mut self, sources: Vec<RetrievalSourceType>) -> Self {
        self.filter.source_types = sources;
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn with_char_budget(mut self, budget: usize) -> Self {
        self.char_budget = Some(budget);
        self
    }
}

/// A single item retrieved by Unified Retrieval with full explainability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetrievedItem {
    pub id: String,
    pub source_type: RetrievalSourceType,
    pub title: String,
    pub content: String,
    pub snippet: String,
    pub score: f32,
    pub timestamp: Option<String>,
    pub provenance: RetrievalProvenance,
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default)]
    pub entity_refs: Vec<String>,
    #[serde(default)]
    pub explainability: Explainability,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// The result returned from Unified Retrieval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetrievalResult {
    pub query: String,
    pub items: Vec<RetrievedItem>,
    pub total_matches: usize,
    pub budget_used: usize,
}
