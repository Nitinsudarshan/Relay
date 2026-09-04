//! Relationship model connecting Relay sources, captures, and derived artifacts.

use serde::{Deserialize, Serialize};

/// Supported relationship types between Relay knowledge objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipType {
    /// Target is the upstream source from which this artifact was derived.
    DerivedFrom,
    /// Target is the content being summarized by this artifact.
    Summarizes,
    /// Target is being analyzed by this artifact.
    Analyses,
    /// Source cites, links to, or references target.
    References,
    /// Source is a child, component, or part of target container/project.
    BelongsTo,
    /// Source replaces or renders target obsolete.
    Supersedes,
}

impl RelationshipType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DerivedFrom => "derived_from",
            Self::Summarizes => "summarizes",
            Self::Analyses => "analyses",
            Self::References => "references",
            Self::BelongsTo => "belongs_to",
            Self::Supersedes => "supersedes",
        }
    }

    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "derived_from" => Some(Self::DerivedFrom),
            "summarizes" => Some(Self::Summarizes),
            "analyses" | "analyzes" => Some(Self::Analyses),
            "references" => Some(Self::References),
            "belongs_to" => Some(Self::BelongsTo),
            "supersedes" => Some(Self::Supersedes),
            _ => None,
        }
    }
}

/// An explicit edge connecting two Relay objects with provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelationshipRecord {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    pub relationship_type: RelationshipType,
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    pub created_at: String,
    #[serde(default)]
    pub provenance: Option<String>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

fn default_confidence() -> f32 {
    1.0
}

impl RelationshipRecord {
    pub fn new(
        source_id: impl Into<String>,
        target_id: impl Into<String>,
        relationship_type: RelationshipType,
    ) -> Result<Self, String> {
        let s = source_id.into();
        let t = target_id.into();
        if s.trim().is_empty() || t.trim().is_empty() {
            return Err("Source and target IDs cannot be empty".to_string());
        }
        if s == t {
            return Err("Self-referential relationships are not permitted".to_string());
        }

        let now = chrono::Utc::now().to_rfc3339();
        let id = format!("rel_{}_{}_{}", s, relationship_type.as_str(), t);
        Ok(Self {
            id,
            source_id: s,
            target_id: t,
            relationship_type,
            confidence: 1.0,
            created_at: now,
            provenance: None,
            metadata: None,
        })
    }

    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    pub fn with_provenance(mut self, provenance: impl Into<String>) -> Self {
        self.provenance = Some(provenance.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_relationship() {
        let rel = RelationshipRecord::new("summary_1", "capture_1", RelationshipType::Summarizes);
        assert!(rel.is_ok());
        let rel = rel.unwrap();
        assert_eq!(rel.source_id, "summary_1");
        assert_eq!(rel.target_id, "capture_1");
        assert_eq!(rel.relationship_type, RelationshipType::Summarizes);
    }

    #[test]
    fn test_reject_self_reference() {
        let rel = RelationshipRecord::new("node_1", "node_1", RelationshipType::DerivedFrom);
        assert!(rel.is_err());
    }
}
