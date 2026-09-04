//! Entities module for central fact extraction and conservative entity resolution.

pub mod extractor;
pub mod model;
pub mod resolution;

pub use extractor::EntityExtractor;
pub use model::{EntityCategory, EntityMention, ExtractedEntity, ResolvedEntity};
pub use resolution::EntityResolver;
