//! Memory layer module.
//!
//! Provides long-term, maintainable, provenance-grounded memory with explicit
//! lifecycles.

pub mod formation;
pub mod model;
pub mod store;

pub use formation::{CandidateMemory, FormationAction, MemoryFormationOutcome, MemoryFormationService};
pub use model::{EpistemicState, MemoryItem, MemoryProvenance, MemoryStatus, MemoryType};
pub use store::MemoryStore;

