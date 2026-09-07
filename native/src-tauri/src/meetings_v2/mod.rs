pub mod capture;
/// Rung 4 of the speaker-identification ladder: separating recorded audio into
/// distinct voices. Reads the recorder's chunk WAVs, never writes them.
pub mod diarize;
pub mod engine;
pub mod live_stt;
/// Derived meeting intelligence: normalization, speakers, conversation,
/// structured extraction, summaries. Reads the recorder's artifacts, never
/// writes them.
pub mod processing;
/// Telling somebody a meeting is about to happen, and letting them act on it.
/// Reads the calendar and the desktop; starts nothing itself.
pub mod reminders;
/// Runnable checks for the pipeline's failure modes, for the Diagnostics page.
/// Proves on the user's machine what the unit tests prove on CI.
pub mod selftest;
pub mod session_store;
/// Tells speech apart from what Whisper emits when there is no speech.
/// Read by both audio clocks and by the diagnostics surface.
/// The speech gate and hallucination screen, which now live in `capture`
/// because they are not meeting-specific: dictation, Talkback and the live
/// clock decode the same Whisper and produce the same subtitle filler.
///
/// Re-exported under the old name so this module's own call sites read
/// unchanged, and because `meetings_v2` is where the rules were derived and
/// where the reported failures came from.
pub use crate::capture::speech_health as transcript_health;
pub mod types;
pub mod worker;

pub use engine::MeetingsV2Engine;
pub use reminders::{MeetingReminderPayload, NotificationService, ReminderQueue};
pub use processing::{MeetingProcessing, MeetingProcessor, ProcessingOptions};
pub use reminders::{ReminderEvent, ReminderKind};
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
