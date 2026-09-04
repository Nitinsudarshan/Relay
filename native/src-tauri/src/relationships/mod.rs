//! Relationships module for explicit link topology across Relay objects.

pub mod model;
pub mod store;

pub use model::{RelationshipRecord, RelationshipType};
pub use store::RelationshipStore;
