pub mod capture;
/// Rung 4 of the speaker-identification ladder: separating recorded audio into
/// distinct voices. Reads the recorder's chunk WAVs, never writes them.
pub mod diarize;
/// Window-level meeting detection (Win32 EnumWindows on Windows, no-op elsewhere).
/// Pure signal — creates no records, has no side effects.
pub mod detection;
pub mod engine;
pub mod live_stt;
/// Derived meeting intelligence: normalization, speakers, conversation,
/// structured extraction, summaries. Reads the recorder's artifacts, never
/// writes them.
pub mod processing;
/// Background loop that resolves calendar + window signals into reminder-queue
/// state transitions and dispatches native OS notifications.
pub mod reminder_engine;
/// Meeting reminder state machine: Pending → Fired → {Snoozed|Dismissed|Actioned|Expired}.
/// Keyed by (id, ReminderKind) so reminders never overwrite each other.
pub mod reminders;
/// Runnable checks for the pipeline's failure modes, for the Diagnostics page.
/// Proves on the user's machine what the unit tests prove on CI.
pub mod selftest;
pub mod session_store;
/// Tells speech apart from what Whisper emits when there is no speech.
/// Read by both audio clocks and by the diagnostics surface.
pub mod transcript_health;
pub mod types;
pub mod worker;

pub use engine::MeetingsV2Engine;
pub use processing::{MeetingProcessing, MeetingProcessor, ProcessingOptions};
pub use reminders::{ActiveMeetingRecording, ReminderEvent, ReminderKind, ReminderQueue};
pub use session_store::SessionStore;
pub use diarize::{Diarization, DiarizationReport, VoiceAssignment};
pub use selftest::{MeetingSelfTestReport, SelfTestCheck};
pub use transcript_health::{HallucinationReason, SpeechProfile, TranscriptRejection};
pub use types::{
    AudioLevels, LiveTranscriptUpdate, MeetingDiagnostics, MeetingNotes, MeetingSession,
    MeetingState, TranscriptSegment, TranscriptSegmentStatus,
};

#[cfg(test)]
pub mod intelligence_tests;
#[cfg(test)]
pub mod intelligence_adversarial_tests;
#[cfg(test)]
pub mod intelligence_golden_meeting_tests;
