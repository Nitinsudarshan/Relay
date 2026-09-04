//! Universal Action Layer.
//!
//! Provides a structured contract for action definition, confirmation gating,
//! and safe execution across Relay capabilities.

pub mod dispatcher;
pub mod model;

pub use dispatcher::ActionDispatcher;
pub use model::{ActionStatus, ActionType, UniversalAction};
