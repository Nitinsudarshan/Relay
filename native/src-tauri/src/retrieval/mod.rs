//! Unified Retrieval module.
//!
//! Provides a single query abstraction across all Relay knowledge sources.

pub mod model;
pub mod providers;
pub mod service;

pub use model::{
    CandidateItem, Explainability, MatchType, RetrievalFilter, RetrievalProvenance, RetrievalQuery,
    RetrievalResult, RetrievalSourceType, RetrievedItem, TimeFilter,
};
pub use providers::{
    CandidateProvider, DerivedDataProvider, MeetingProvider, MemoryProvider, RelationshipProvider,
    VaultProvider,
};
pub use service::{extract_snippet, tokenize, UnifiedRetrievalService};

#[cfg(test)]
mod acceptance_tests;


