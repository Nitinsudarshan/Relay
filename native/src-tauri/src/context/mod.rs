//! Context packs and standard context assembly pipeline.

pub mod assembly;
pub mod pack;

pub use assembly::{ContextAssemblyRequest, ContextAssemblyService};
pub use pack::{ContextPack, ContextPackItem, ContextPackType};
