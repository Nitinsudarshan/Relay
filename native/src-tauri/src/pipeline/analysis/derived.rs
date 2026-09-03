//! Derived data: what analysis produced, kept apart from what the source is.
//!
//! # The relationship this makes explicit
//!
//! ```text
//! SOURCE (a VaultFile — immutable evidence)
//!   ├── DerivedData { derived_type: Summary }
//!   ├── DerivedData { derived_type: Context }
//!   └── DerivedData { derived_type: Enrichment }
//! ```
//!
//! Before this, a summary was a field *on* the source record, which made three
//! things impossible: knowing which model produced it, knowing whether it
//! succeeded or fell back, and re-analysing without rewriting the artifact that
//! is supposed to be the immutable record of what was captured.
//!
//! # One record, typed payloads
//!
//! There is deliberately no `SummaryRecord` / `ContextRecord` / `ExtractionRecord`
//! split. They would differ only by the type of one field, and three storage
//! paths is three migrations the next time anything changes. What varies is
//! [`DerivedPayload`]; everything around it is shared.
//!
//! # Regeneration policy
//!
//! Re-analysis **replaces** the derived record for that `(source_id,
//! derived_type)` pair. Relay keeps the latest derived representation, not a
//! history — this matches how `context.json` already behaved, and §12 asks for
//! the choice to be documented and applied consistently rather than left
//! implicit. The source is never touched, so re-analysis can always be redone.

use serde::{Deserialize, Serialize};

use super::contract::{AnalysisMetadata, AnalysisType};

/// What kind of derived artifact this is.
///
/// The wire names are stored on disk and are part of the file layout
/// (`derived/<derived_type>.json`), so they are stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DerivedType {
    Summary,
    Context,
    Enrichment,
    Extraction,
}

impl DerivedType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Summary => "summary",
            Self::Context => "context",
            Self::Enrichment => "enrichment",
            Self::Extraction => "extraction",
        }
    }

    pub fn from_analysis(analysis_type: AnalysisType) -> Self {
        match analysis_type {
            AnalysisType::Summary => Self::Summary,
            AnalysisType::Context => Self::Context,
            AnalysisType::Enrichment => Self::Enrichment,
            AnalysisType::Extraction => Self::Extraction,
        }
    }
}

/// The content of a derived artifact.
///
/// `Text` for a summary, `Structured` for anything with a schema. Untagged on
/// the wire would be ambiguous, so it carries its own tag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "payload_kind", content = "payload", rename_all = "snake_case")]
pub enum DerivedPayload {
    Text(String),
    Structured(serde_json::Value),
}

impl DerivedPayload {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(t) => Some(t),
            Self::Structured(_) => None,
        }
    }

    pub fn as_structured(&self) -> Option<&serde_json::Value> {
        match self {
            Self::Structured(v) => Some(v),
            Self::Text(_) => None,
        }
    }

    /// Deserializes a structured payload into its semantic type.
    pub fn parse_structured<T: serde::de::DeserializeOwned>(&self) -> Option<T> {
        serde_json::from_value(self.as_structured()?.clone()).ok()
    }
}

/// One derived artifact and its full provenance.
///
/// `source_id` is the whole point: every derived record names the source it
/// came from, and nothing here is ever mistaken for source truth.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DerivedData {
    /// Deterministic from `(source_id, derived_type)` — the same analysis
    /// re-run overwrites its own record rather than accumulating orphans.
    pub id: String,
    /// The artifact this was derived from.
    pub source_id: String,
    pub derived_type: DerivedType,
    /// Bumped each time this record is regenerated. Not a history — the
    /// previous payload is gone — but it makes "has this been re-analysed?"
    /// answerable, and a stale UI can tell it is stale.
    #[serde(default = "default_version")]
    pub version: u32,
    pub created_at: String,
    pub updated_at: String,
    /// Status, prompt id and version, model, and whether a fallback wrote it.
    pub analysis: AnalysisMetadata,
    pub payload: DerivedPayload,
}

fn default_version() -> u32 {
    1
}

impl DerivedData {
    pub fn id_for(source_id: &str, derived_type: DerivedType) -> String {
        format!("{}::{}", source_id, derived_type.as_str())
    }

    pub fn new(
        source_id: &str,
        derived_type: DerivedType,
        analysis: AnalysisMetadata,
        payload: DerivedPayload,
    ) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: Self::id_for(source_id, derived_type),
            source_id: source_id.to_string(),
            derived_type,
            version: 1,
            created_at: now.clone(),
            updated_at: now,
            analysis,
            payload,
        }
    }

    /// Replaces this record's content, keeping its identity and creation time.
    ///
    /// This is what "re-analyse" does: the same source, a new derived version.
    /// Never a new source, and never a second record for the same pair.
    pub fn supersede(&self, analysis: AnalysisMetadata, payload: DerivedPayload) -> Self {
        Self {
            id: self.id.clone(),
            source_id: self.source_id.clone(),
            derived_type: self.derived_type,
            version: self.version.saturating_add(1),
            created_at: self.created_at.clone(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            analysis,
            payload,
        }
    }

    pub fn is_usable(&self) -> bool {
        self.analysis.is_usable()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::analysis::contract::{AnalysisFailure, MetadataBuilder};
    use crate::pipeline::analysis::prompts::PromptId;

    fn metadata() -> AnalysisMetadata {
        MetadataBuilder::new(AnalysisType::Summary, PromptId::Summary, 1)
            .deterministic(AnalysisFailure::NoCompletion("offline".to_string()))
    }

    /// §53.5 — every derived artifact names its source, and the three kinds a
    /// source can have all point back at the same one.
    #[test]
    fn every_derived_kind_references_the_same_source() {
        let summary = DerivedData::new(
            "cap_1",
            DerivedType::Summary,
            metadata(),
            DerivedPayload::Text("A summary".to_string()),
        );
        let context = DerivedData::new(
            "cap_1",
            DerivedType::Context,
            metadata(),
            DerivedPayload::Structured(serde_json::json!({"objective": "x"})),
        );
        let enrichment = DerivedData::new(
            "cap_1",
            DerivedType::Enrichment,
            metadata(),
            DerivedPayload::Structured(serde_json::json!({"topics": []})),
        );

        for derived in [&summary, &context, &enrichment] {
            assert_eq!(derived.source_id, "cap_1");
        }
        // Distinct records, one per kind, no collisions.
        assert_ne!(summary.id, context.id);
        assert_ne!(context.id, enrichment.id);
        assert_eq!(summary.id, "cap_1::summary");
    }

    /// §12 — re-analysis produces a new derived version against the same
    /// source, not a second source and not a second record.
    #[test]
    fn re_analysis_supersedes_in_place_and_keeps_the_source() {
        let first = DerivedData::new(
            "cap_1",
            DerivedType::Context,
            metadata(),
            DerivedPayload::Text("first".to_string()),
        );
        let second = first.supersede(metadata(), DerivedPayload::Text("second".to_string()));

        assert_eq!(second.id, first.id, "identity is stable across re-analysis");
        assert_eq!(second.source_id, first.source_id);
        assert_eq!(second.created_at, first.created_at);
        assert_eq!(second.version, 2);
        assert_eq!(second.payload.as_text(), Some("second"));
    }

    #[test]
    fn a_derived_record_round_trips_through_json() {
        let derived = DerivedData::new(
            "cap_1",
            DerivedType::Context,
            metadata(),
            DerivedPayload::Structured(serde_json::json!({"objective": "ship it"})),
        );
        let raw = serde_json::to_string_pretty(&derived).unwrap();
        let back: DerivedData = serde_json::from_str(&raw).unwrap();
        assert_eq!(derived, back);
        assert_eq!(
            back.payload.as_structured().unwrap()["objective"],
            serde_json::json!("ship it")
        );
    }

    /// A record written by a fallback must not read back as a model's work.
    #[test]
    fn a_fallback_record_reports_that_no_model_answered() {
        let derived = DerivedData::new(
            "cap_1",
            DerivedType::Summary,
            metadata(),
            DerivedPayload::Text("deterministic summary".to_string()),
        );
        assert!(derived.analysis.deterministic);
        assert!(derived.analysis.model.is_none());
        // Still usable — an honest fallback is worth showing, flagged as one.
        assert!(derived.is_usable());
    }
}
