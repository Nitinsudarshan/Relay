//! The lifecycle every analysis shares.
//!
//! # The distinction this type exists to hold
//!
//! Before this module, an analysis had two outcomes: a value, or a log line
//! and a deterministic substitute. That collapses three genuinely different
//! situations into one, and the collapse is what lets a vault fill up with
//! confident-sounding nothing:
//!
//! ```text
//! the model answered, and the source supports the answer   → Succeeded
//! the model answered, and the source did not support it    → InsufficientEvidence
//! no model answered, or its answer was unusable            → Failed
//! ```
//!
//! "Open Issues: none" and "Open Issues: not captured" are not the same claim,
//! and neither is "the analysis broke". A caller that cannot tell them apart
//! cannot render them apart, which is how a partial capture ends up asserting
//! that a repository has no issues.

use serde::{Deserialize, Serialize};

use super::prompts::PromptId;
use super::source::{SourceCoverage, SourceDescriptor, SourceSubtype, SourceType};

/// What kind of understanding an analysis is being asked for.
///
/// Deliberately separate from the source type. The same summary analysis runs
/// over a document, a capture and a scribble; the same source supports several
/// different analyses. Pairing them is the prompt registry's job, not this
/// enum's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisType {
    /// Short prose: what is this source about?
    Summary,
    /// Structured source-specific understanding worth retaining.
    Context,
    /// Topics, entities, questions — the knowledge metadata enrichment.
    Enrichment,
    /// A named structured extraction over the source.
    Extraction,
    /// Categorization or domain taxonomy tagging.
    Classification,
    /// Deep source analysis or evaluation.
    Analysis,
    /// Generated speech or video transcript.
    Transcript,
}

impl AnalysisType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Summary => "summary",
            Self::Context => "context",
            Self::Enrichment => "enrichment",
            Self::Extraction => "extraction",
            Self::Classification => "classification",
            Self::Analysis => "analysis",
            Self::Transcript => "transcript",
        }
    }
}

/// Where an analysis ended up.
///
/// `Requested` and `Running` are here because the type is also the shape a UI
/// polls; nothing persists in those states today, and a stored record in one
/// of them means a process died mid-analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisStatus {
    Requested,
    Running,
    /// A model answered and the answer validated against the contract.
    Succeeded,
    /// The analysis ran correctly and the source did not carry what was asked
    /// for. A successful outcome, and emphatically not an error: the result is
    /// the honest statement that the evidence was not there.
    InsufficientEvidence,
    /// No usable answer: the provider was unreachable, returned filler, or
    /// returned something that could not be validated.
    Failed,
    Cancelled,
}

impl AnalysisStatus {
    /// Whether the result carries usable derived content.
    ///
    /// `InsufficientEvidence` counts: "Relay looked and the evidence was not
    /// there" is a finding worth storing and showing.
    pub fn is_usable(&self) -> bool {
        matches!(self, Self::Succeeded | Self::InsufficientEvidence)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::InsufficientEvidence => "insufficient_evidence",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

/// Why an analysis failed, in the terms a caller can act on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum AnalysisFailure {
    /// No model answered. Includes the client substituting heuristic filler,
    /// because for analysis that is the same thing as silence.
    NoCompletion(String),
    /// The model answered with something that is not the requested format.
    Unparseable(String),
    /// The model answered in the right format with contents the contract
    /// rejects.
    ValidationFailed(String),
    /// The source had nothing to analyse.
    EmptySource,
    /// A prompt was requested that does not apply to this source type.
    PromptNotApplicable(String),
}

impl std::fmt::Display for AnalysisFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoCompletion(m) => write!(f, "no completion available: {m}"),
            Self::Unparseable(m) => write!(f, "response could not be parsed: {m}"),
            Self::ValidationFailed(m) => write!(f, "response failed validation: {m}"),
            Self::EmptySource => write!(f, "the source has no content to analyse"),
            Self::PromptNotApplicable(m) => write!(f, "prompt does not apply to this source: {m}"),
        }
    }
}

/// One analysis to run.
///
/// Borrowed from the source and content the caller already holds. The prompt is
/// named by id rather than passed as a string, which is what makes
/// "which prompt produced this?" answerable after the fact.
#[derive(Debug, Clone)]
pub struct AnalysisRequest<'a> {
    pub source_id: &'a str,
    pub source_type: SourceType,
    pub source_subtype: SourceSubtype,
    pub analysis_type: AnalysisType,
    pub prompt_id: PromptId,
    /// Overrides the prompt's own sampling when a caller needs different
    /// behaviour for one call. Normally `None` — the prompt knows what it needs.
    pub options: Option<crate::providers::CompletionOptions>,
}

impl<'a> AnalysisRequest<'a> {
    pub fn new(
        source: &SourceDescriptor<'a>,
        analysis_type: AnalysisType,
        prompt_id: PromptId,
    ) -> Self {
        Self {
            source_id: source.id,
            source_type: source.source_type,
            source_subtype: source.subtype,
            analysis_type,
            prompt_id,
            options: None,
        }
    }

    pub fn with_options(mut self, options: crate::providers::CompletionOptions) -> Self {
        self.options = Some(options);
        self
    }
}

/// What an analysis produced, and everything needed to judge how far to trust
/// it.
///
/// The metadata fields are not bookkeeping. `prompt_version` is what stops old
/// derived data from claiming it came from a prompt written after it (§21);
/// `model` is the model that actually answered rather than the provider that
/// was configured; `deterministic` says a fallback wrote this, so nothing
/// downstream has to guess from the shape of the content.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnalysisMetadata {
    pub analysis_type: AnalysisType,
    pub status: AnalysisStatus,
    pub prompt_id: String,
    pub prompt_version: u32,
    /// The provider family that served the request, e.g. `ollama`.
    #[serde(default)]
    pub provider: Option<String>,
    /// The model that actually answered, as the provider reported it — not the
    /// configured provider name. `None` when no model answered.
    #[serde(default)]
    pub model: Option<String>,
    /// True when a deterministic fallback produced this rather than a model.
    #[serde(default)]
    pub deterministic: bool,
    /// Coverage of the source at the time of analysis, so a context generated
    /// from a partial capture still says so after the fact.
    #[serde(default)]
    pub source_coverage: Option<String>,
    pub generated_at: String,
    #[serde(default)]
    pub prompt_tokens: Option<usize>,
    #[serde(default)]
    pub completion_tokens: Option<usize>,
    /// Present only for `Failed`. Records what went wrong so a retry has
    /// something to go on and the UI can say more than "analysis failed".
    #[serde(default)]
    pub failure: Option<AnalysisFailure>,
}

impl AnalysisMetadata {
    pub fn is_usable(&self) -> bool {
        self.status.is_usable()
    }
}

/// A completed analysis: its payload, and the metadata that qualifies it.
#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisResult<T> {
    pub source_id: String,
    pub metadata: AnalysisMetadata,
    /// `None` when the analysis failed. `Some` for both `Succeeded` and
    /// `InsufficientEvidence` — an insufficient-evidence result still carries
    /// the structure, with its unavailable parts marked as unavailable.
    pub payload: Option<T>,
}

impl<T> AnalysisResult<T> {
    pub fn is_usable(&self) -> bool {
        self.metadata.is_usable()
    }

    pub fn failure(&self) -> Option<&AnalysisFailure> {
        self.metadata.failure.as_ref()
    }

    /// Applies a function to the payload, keeping the metadata intact.
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> AnalysisResult<U> {
        AnalysisResult {
            source_id: self.source_id,
            metadata: self.metadata,
            payload: self.payload.map(f),
        }
    }
}

/// Builds the metadata for a result. Kept here rather than in the service so
/// the deterministic paths, which never touch a provider, record the same
/// shape as the model-backed ones.
pub struct MetadataBuilder {
    analysis_type: AnalysisType,
    prompt_id: String,
    prompt_version: u32,
    coverage: Option<SourceCoverage>,
}

impl MetadataBuilder {
    pub fn new(analysis_type: AnalysisType, prompt_id: PromptId, prompt_version: u32) -> Self {
        Self {
            analysis_type,
            prompt_id: prompt_id.as_str().to_string(),
            prompt_version,
            coverage: None,
        }
    }

    pub fn with_coverage(mut self, coverage: SourceCoverage) -> Self {
        self.coverage = Some(coverage);
        self
    }

    fn base(&self, status: AnalysisStatus) -> AnalysisMetadata {
        AnalysisMetadata {
            analysis_type: self.analysis_type,
            status,
            prompt_id: self.prompt_id.clone(),
            prompt_version: self.prompt_version,
            provider: None,
            model: None,
            deterministic: false,
            source_coverage: self.coverage.map(|c| c.as_str().to_string()),
            generated_at: chrono::Utc::now().to_rfc3339(),
            prompt_tokens: None,
            completion_tokens: None,
            failure: None,
        }
    }

    /// A model answered and the answer validated.
    pub fn succeeded(&self, provider: &str, response: &crate::providers::LLMResponse) -> AnalysisMetadata {
        AnalysisMetadata {
            provider: Some(provider.to_string()),
            model: Some(response.model.clone()),
            prompt_tokens: response.prompt_tokens,
            completion_tokens: response.completion_tokens,
            ..self.base(AnalysisStatus::Succeeded)
        }
    }

    /// A deterministic fallback produced the payload. Recorded as
    /// `InsufficientEvidence` rather than `Succeeded`: no model read the
    /// source, so whatever came out is pattern-matching, and calling that a
    /// successful analysis is the overstatement this whole module exists to
    /// stop.
    pub fn deterministic(&self, failure: AnalysisFailure) -> AnalysisMetadata {
        AnalysisMetadata {
            deterministic: true,
            failure: Some(failure),
            ..self.base(AnalysisStatus::InsufficientEvidence)
        }
    }

    pub fn failed(&self, failure: AnalysisFailure) -> AnalysisMetadata {
        AnalysisMetadata {
            failure: Some(failure),
            ..self.base(AnalysisStatus::Failed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insufficient_evidence_is_usable_and_failure_is_not() {
        assert!(AnalysisStatus::Succeeded.is_usable());
        assert!(
            AnalysisStatus::InsufficientEvidence.is_usable(),
            "an honest 'the evidence was not there' is a result worth keeping"
        );
        assert!(!AnalysisStatus::Failed.is_usable());
        assert!(!AnalysisStatus::Cancelled.is_usable());
    }

    #[test]
    fn a_deterministic_fallback_never_reports_success() {
        let builder = MetadataBuilder::new(
            AnalysisType::Summary,
            PromptId::Summary,
            3,
        );
        let meta = builder.deterministic(AnalysisFailure::NoCompletion("offline".to_string()));

        assert_eq!(meta.status, AnalysisStatus::InsufficientEvidence);
        assert!(meta.deterministic);
        assert!(meta.model.is_none(), "no model answered, so none may be named");
        assert!(meta.failure.is_some(), "the reason a model did not answer is recorded");
    }

    #[test]
    fn a_succeeded_result_names_the_model_that_answered_not_the_provider_config() {
        let builder = MetadataBuilder::new(AnalysisType::Context, PromptId::ConversationContext, 1);
        let response = crate::providers::LLMResponse {
            text: "{}".to_string(),
            model: "llama3.2:latest".to_string(),
            prompt_tokens: Some(120),
            completion_tokens: Some(40),
        };
        let meta = builder.succeeded("ollama", &response);

        assert_eq!(meta.status, AnalysisStatus::Succeeded);
        assert_eq!(meta.model.as_deref(), Some("llama3.2:latest"));
        assert_eq!(meta.provider.as_deref(), Some("ollama"));
        assert_eq!(meta.prompt_tokens, Some(120));
        assert!(!meta.deterministic);
    }

    #[test]
    fn coverage_at_analysis_time_is_recorded_on_the_result() {
        let meta = MetadataBuilder::new(AnalysisType::Context, PromptId::RepositoryContext, 1)
            .with_coverage(SourceCoverage::Partial)
            .failed(AnalysisFailure::EmptySource);
        assert_eq!(meta.source_coverage.as_deref(), Some("partial"));
    }

    #[test]
    fn metadata_round_trips_through_json() {
        let meta = MetadataBuilder::new(AnalysisType::Summary, PromptId::Summary, 2)
            .deterministic(AnalysisFailure::Unparseable("not json".to_string()));
        let raw = serde_json::to_string(&meta).unwrap();
        let back: AnalysisMetadata = serde_json::from_str(&raw).unwrap();
        assert_eq!(meta, back);
    }
}
