//! The shared foundation every analysis runs on.
//!
//! ```text
//! SOURCE            source::SourceDescriptor    — what is this, and how much of it does Relay have?
//!   ↓
//! NORMALISE         content::CanonicalContent   — the analysis-facing shape every normalizer agrees on
//!   ↓
//! ANALYSE           service::AnalysisService    — prompt → boundary → provider → parse → validate
//!   ↓                 contract::AnalysisRequest / AnalysisResult
//!   ↓                 prompts::PromptDefinition — identified, versioned, applicability-checked
//!   ↓
//! DERIVED DATA      derived::DerivedData        — keyed by source id, never mistaken for the source
//! ```
//!
//! # What this module does not do
//!
//! It carries no source-specific meaning. It does not know what a repository
//! stack is, what a decision is, or which fields a conversation context has.
//! Those live with the types that define them — `capture::web::context` for the
//! two context schemas, `pipeline::enrichment` for the canonical two — and this
//! module supplies the mechanics they all needed and each had reimplemented.
//!
//! The measure of success is not that a `SourceDescriptor` exists. It is that
//! adding a new analysis means writing a prompt, a payload type and a builder,
//! rather than another capture → normalize → prompt → LLM → parse → persist →
//! provenance pipeline.
//!
//! # Everything runs here now
//!
//! The meetings pipeline was the last holdout and has moved. Both of its
//! stages go through [`service::AnalysisService`]:
//! [`contract::AnalysisStage`] names the two passes,
//! [`prompts::PromptBody::Computed`] carries instructions that cannot be
//! constants — a meeting summary's interpolate the length budget, the depth
//! mode, the extension and the user's language — and `PromptId::MeetingFacts`
//! and `PromptId::MeetingSummary` supply the identity, version, output
//! contract and sampling that the pipeline used to decide for itself.
//!
//! The last blocker was a number. `MeetingLlm` carried `prompt_budget_chars`,
//! which extraction uses to decide how many passes a long transcript needs, and
//! nothing here could answer it. [`service::AnalysisService::prompt_budget_chars`]
//! answers it now, and answers it better: per prompt, from the window less that
//! prompt's own output allowance, rather than from one constant that assumed
//! every analysis writes the same amount back.
//!
//! What stayed in `meetings_v2::processing` is what should have: the repair
//! loop, the deterministic floor, and the qualification pass. Those are the
//! pipeline's own judgement about a meeting, not mechanics anyone else needed.
//! §38 was right that destabilising a working pipeline for symmetry is a bad
//! trade — the trade was worth making only once the symmetry was real, and the
//! test that proves the chunk size did not move is the receipt.
//!
//! Two things that were genuinely duplicated are now single: the
//! heuristic-filler marker, which comes from `providers` for both, and the
//! definition of a parseable structured answer, which is
//! [`service::parse_json_response`] for both.

pub mod content;
pub mod contract;
pub mod derived;
pub mod prompts;
pub mod service;
pub mod source;

pub use content::{ArtifactKind, CanonicalContent, ContentArtifact, ContentSegment};
pub use contract::{
    AnalysisFailure, AnalysisMetadata, AnalysisRequest, AnalysisResult, AnalysisStage,
    AnalysisStatus,
    AnalysisType, MetadataBuilder,
};
pub use derived::{DerivedData, DerivedPayload, DerivedType};
pub use prompts::{context_prompt_for, OutputContract, PromptBody, PromptDefinition, PromptId};
pub use service::{
    context_request, parse_json_response, prompt_budget_chars_for, provider_name,
    AnalysisService,
};
pub use source::{
    SourceCoverage, SourceDescriptor, SourceSubtype, SourceTrust, SourceType,
};
