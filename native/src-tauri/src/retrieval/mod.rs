//! Unified Retrieval module.
//!
//! Provides a single query abstraction across all Relay knowledge sources.

pub mod model;
pub mod service;

pub use model::{
    RetrievalFilter, RetrievalProvenance, RetrievalQuery, RetrievalResult, RetrievalSourceType,
    RetrievedItem, TimeFilter,
};
pub use service::{extract_snippet, tokenize, UnifiedRetrievalService};
