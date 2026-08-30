//! Talkback — the conversational interface over everything Relay has
//! captured.
//!
//! ```text
//! CAPTURE  →  UNDERSTAND  →  REMEMBER  →  CONVERSE
//! ```
//!
//! Relay already did the first three. This is the fourth, and the design
//! constraint that shapes every file here is that it must not become a
//! fifth silo:
//!
//! * **No storage of its own.** Voice Notes, Scribbles, Meetings and
//!   MeetingFacts *are* the memory. `sources.rs` projects them;
//!   `retrieval.rs` ranks them. Nothing here writes a new kind of record.
//! * **Provenance survives to the answer.** Every retrieved item keeps
//!   its source type, id, title and timestamp, so "where did you get
//!   that?" has a real answer.
//! * **Personal memory is never answered from model knowledge.** A recall
//!   question with no retrieved evidence is answered honestly *without
//!   calling a model at all* (`engine::plan_turn`).
//! * **Ephemeral by default.** A conversation is not knowledge until the
//!   user says it is (`tools.rs`).
//!
//! The full reasoning, including the architectures rejected and why, is
//! in `docs/talkback/ARCHITECTURE.md` and `docs/talkback/RESEARCH.md`.
//!
//! ## Layout
//!
//! ```text
//!  state.rs      the authoritative state machine
//!  turn.rs       streaming turn detection (energy + hangover)
//!  audio.rs      Talkback's own microphone stream
//!  intent.rs     deterministic intent routing
//!  retrieval.rs  ranking, expansion, dedup, budget   (pure)
//!  sources.rs    vault/meeting projection            (the only I/O)
//!  assemble.rs   prompts, context blocks, provenance (pure)
//!  chunk.rs      phrase buffer for streaming TTS     (pure)
//!  session.rs    ephemeral conversation
//!  tools.rs      Voice Note and Scribble actions
//!  engine.rs     orchestration, cancellation, events
//! ```

pub mod assemble;
pub mod audio;
pub mod chunk;
pub mod engine;
pub mod intent;
pub mod retrieval;
pub mod session;
pub mod sources;
pub mod state;
pub mod tools;
pub mod turn;

pub use engine::{
    ActivationMode, TalkbackEngine, TalkbackSettings, TurnContext, TurnMetrics, TurnPlan,
};
pub use intent::Intent;
pub use retrieval::{ContextItem, RetrievalQuery, RetrievalResult, SourceType};
pub use session::{Role, TalkbackSession, TalkbackTurn};
pub use state::{TalkbackEvent, TalkbackState};
