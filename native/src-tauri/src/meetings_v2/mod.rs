pub mod capture;
pub mod engine;
pub mod live_stt;
/// Derived meeting intelligence: normalization, speakers, conversation,
/// structured extraction, summaries. Reads the recorder's artifacts, never
/// writes them.
pub mod processing;
pub mod session_store;
pub mod types;
pub mod worker;

pub use engine::MeetingsV2Engine;
pub use processing::{MeetingProcessing, MeetingProcessor, ProcessingOptions};
pub use session_store::SessionStore;
pub use types::{
    AudioLevels, LiveTranscriptUpdate, MeetingDiagnostics, MeetingSession, MeetingState,
    TranscriptSegment, TranscriptSegmentStatus,
};

