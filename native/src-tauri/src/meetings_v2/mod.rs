pub mod capture;
pub mod engine;
pub mod live_stt;
/// Derived meeting intelligence: normalization, speakers, conversation,
/// structured extraction, summaries. Reads the recorder's artifacts, never
/// writes them.
pub mod processing;
pub mod session_store;
/// Tells speech apart from what Whisper emits when there is no speech.
/// Read by both audio clocks and by the diagnostics surface.
pub mod transcript_health;
pub mod types;
pub mod worker;

pub use engine::MeetingsV2Engine;
pub use processing::{MeetingProcessing, MeetingProcessor, ProcessingOptions};
pub use session_store::SessionStore;
pub use transcript_health::{HallucinationReason, SpeechProfile, TranscriptRejection};
pub use types::{
    AudioLevels, LiveTranscriptUpdate, MeetingDiagnostics, MeetingNotes, MeetingSession,
    MeetingState, TranscriptSegment, TranscriptSegmentStatus,
};

