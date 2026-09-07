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
//! # Not migrated yet
//!
//! The meetings pipeline (`meetings_v2::processing`) still runs its own
//! `MeetingLlm`, but this module now models what it needs:
//! [`contract::AnalysisStage`] describes a multi-pass analysis, and
//! [`prompts::PromptBody::Computed`] admits a prompt whose instructions are
//! built per call — which is what a meeting summary's are, since they
//! interpolate the length budget, the depth mode, the extension and the user's
//! language. `PromptId::MeetingFacts` and `PromptId::MeetingSummary` are
//! registered, with their own output contracts and per-stage sampling.
//!
//! [`service::AnalysisService`] now holds `providers::Completer` rather than a
//! concrete client, so a caller with a scripted stand-in can exercise its
//! failure paths — which is what the meeting suites need in order to move.
//! What is left is the swap itself: `MeetingLlm` also carries
//! `prompt_budget_chars`, used to decide how many extraction passes a long
//! transcript needs, and that has no equivalent here yet. Destabilising a
//! working pipeline for architectural symmetry is still a bad trade, and §38
//! still says so.
//!
//! The one thing that was genuinely duplicated — the heuristic-filler marker —
//! now comes from `providers` for both.

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
pub use service::{context_request, parse_json_response, AnalysisService};
pub use source::{
    SourceCoverage, SourceDescriptor, SourceSubtype, SourceTrust, SourceType,
};
