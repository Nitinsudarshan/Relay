//! Universal Action Layer.
//!
//! Provides a structured contract for action definition, confirmation gating,
//! and safe execution across Relay capabilities.

pub mod audit;
pub mod dispatcher;
pub mod idempotency;
pub mod model;
pub mod registry;

pub use audit::{ActionAuditLogger, ActionAuditRecord};
pub use dispatcher::ActionDispatcher;
pub use idempotency::IdempotencyStore;
pub use model::{ActionStatus, ActionType, UniversalAction};
pub use registry::{ActionExecutionContext, ActionHandler, ActionRegistry};

