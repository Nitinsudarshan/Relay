//! Entity and Fact data models for central extraction and resolution.

use serde::{Deserialize, Serialize};

/// Supported entity categories across Relay knowledge sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityCategory {
    Person,
    Organization,
    Project,
    Product,
    Technology,
    Location,
    Date,
    Url,
    Identifier,
}

impl EntityCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::Organization => "organization",
            Self::Project => "project",
            Self::Product => "product",
            Self::Technology => "technology",
            Self::Location => "location",
            Self::Date => "date",
            Self::Url => "url",
            Self::Identifier => "identifier",
        }
    }

    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "person" | "people" => Some(Self::Person),
            "organization" | "org" => Some(Self::Organization),
            "project" => Some(Self::Project),
            "product" => Some(Self::Product),
            "technology" | "tech" => Some(Self::Technology),
            "location" | "place" => Some(Self::Location),
            "date" | "time" => Some(Self::Date),
            "url" | "link" => Some(Self::Url),
            "identifier" | "id" => Some(Self::Identifier),
            _ => None,
        }
    }
}

/// A raw entity or fact extracted from a specific source with direct evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractedEntity {
    pub id: String,
    pub name: String,
    pub category: EntityCategory,
    pub source_id: String,
    /// The sentence or phrase in the source that justifies this extraction.
    pub evidence: String,
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    #[serde(default = "default_occurrences")]
    pub occurrences: usize,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

fn default_confidence() -> f32 {
    1.0
}

fn default_occurrences() -> usize {
    1
}

impl ExtractedEntity {
    pub fn new(
        name: impl Into<String>,
        category: EntityCategory,
        source_id: impl Into<String>,
        evidence: impl Into<String>,
    ) -> Self {
        let n = name.into();
        let sid = source_id.into();
        let id = format!("ent_{}_{}_{}", sid, category.as_str(), slugify(&n));
        Self {
            id,
            name: n,
            category,
            source_id: sid,
            evidence: evidence.into(),
            confidence: 1.0,
            occurrences: 1,
            metadata: None,
        }
    }

    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }
}

/// A single mention linking an entity back to source evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityMention {
    pub source_id: String,
    pub evidence: String,
    pub confidence: f32,
    #[serde(default)]
    pub timestamp: Option<String>,
}

/// A canonical, resolved entity uniting multiple mentions, aliases, and identifiers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedEntity {
    pub id: String,
    pub canonical_name: String,
    pub category: EntityCategory,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub source_identifiers: Vec<String>,
    #[serde(default)]
    pub urls: Vec<String>,
    pub confidence: f32,
    #[serde(default)]
    pub mentions: Vec<EntityMention>,
}

/// Basic slug generator for stable ID creation.
pub fn slugify(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}
