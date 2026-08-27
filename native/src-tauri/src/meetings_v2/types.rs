use serde::{Deserialize, Serialize};

/// Authoritative state of a Meeting recording session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MeetingState {
    Idle,
    Starting,
    Recording,
    Paused,
    Stopping,
    Finalizing,
    Completed,
    Interrupted,
    Recovered,
    Error,
}

impl Default for MeetingState {
    fn default() -> Self {
        Self::Idle
    }
}

/// Metadata representation of a Meeting V2 recording session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingSession {
    pub id: String,
    pub title: String,
    pub state: MeetingState,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub duration_seconds: f64,
    pub chunk_count: usize,
    pub mic_active: bool,
    pub sys_audio_active: bool,
    /// Whether each source was ever actually *audible*, as opposed to merely
    /// bound to a device. A muted mic or a silent loopback device is `active`
    /// but not `heard`; only `heard` says the source contributed audio.
    #[serde(default)]
    pub mic_heard: bool,
    #[serde(default)]
    pub sys_audio_heard: bool,
    /// Wall-clock seconds spent paused, already excluded from `duration_seconds`.
    #[serde(default)]
    pub paused_seconds: f64,
    /// Set when capture came up degraded (e.g. no loopback device) so the UI
    /// can say so instead of implying both sources were captured.
    #[serde(default)]
    pub capture_warning: Option<String>,
    pub total_audio_bytes: u64,
    pub transcript_segment_count: usize,
    #[serde(default)]
    pub word_count: usize,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub action_items: Vec<String>,
    pub pending_transcription_chunks: usize,
    pub error_message: Option<String>,
}

impl MeetingSession {
    pub fn new(id: String, title: Option<String>) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        let default_title = format!(
            "Meeting — {}",
            chrono::Local::now().format("%b %d, %Y %I:%M %p")
        );

        Self {
            id,
            title: title.unwrap_or(default_title),
            state: MeetingState::Starting,
            created_at: now.clone(),
            updated_at: now.clone(),
            started_at: Some(now),
            ended_at: None,
            duration_seconds: 0.0,
            chunk_count: 0,
            mic_active: false,
            sys_audio_active: false,
            mic_heard: false,
            sys_audio_heard: false,
            paused_seconds: 0.0,
            capture_warning: None,
            total_audio_bytes: 0,
            transcript_segment_count: 0,
            word_count: 0,
            summary: None,
            action_items: Vec::new(),
            pending_transcription_chunks: 0,
            error_message: None,
        }
    }
}

/// A single incremental transcript segment derived from an audio chunk.
///
/// This is the **raw** transcript record: one line of `transcript.jsonl`, written
/// once by the durable transcription worker and never rewritten. Derived text
/// (normalized, conversation, summary) lives in `processing.json` instead — see
/// `meetings_v2::processing`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegment {
    pub chunk_index: usize,
    pub start_time_s: f64,
    pub end_time_s: f64,
    pub text: String,
    pub created_at: String,
    pub status: TranscriptSegmentStatus,
    /// Whether each capture source was audible within this chunk, copied from
    /// the `AudioChunk` the segment came from.
    ///
    /// This is rung 1 of `Meeting-rules/meeting_speaker_identification.md`: the
    /// microphone is the local user, system audio is everyone else. The recorder
    /// already measures both values per chunk; persisting them here is what makes
    /// speaker attribution possible at all without diarization. Both default to
    /// `false` so transcripts written before this field existed deserialize
    /// unchanged, correctly reading as "channel unknown" rather than as silence.
    #[serde(default)]
    pub mic_had_audio: bool,
    #[serde(default)]
    pub sys_had_audio: bool,
    /// Whisper's own utterance spans within this chunk, each already resolved to
    /// a channel from the chunk's per-second energy track.
    ///
    /// This is what lifts attribution off the 30-second chunk. The chunk-level
    /// booleans above stay as the roll-up and as the fallback for transcripts
    /// written before v2.5, which deserialize with an empty list and are still
    /// read exactly as they were.
    #[serde(default)]
    pub utterances: Vec<TranscriptUtterance>,
}

/// One utterance inside a chunk, with the channel that was audible while it was
/// spoken.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranscriptUtterance {
    /// Index within the chunk, so an id can be derived without a counter.
    pub index: usize,
    /// Absolute session time, not an offset within the chunk.
    pub start_time_s: f64,
    pub end_time_s: f64,
    pub text: String,
    /// Measured over exactly this utterance's span of the channel track.
    pub mic_had_audio: bool,
    pub sys_had_audio: bool,
    /// Whisper's own no-speech probability for the span. Kept for diagnostics:
    /// a high value on a confidently-transcribed sentence is the signature of a
    /// decoder hallucination over silence.
    #[serde(default)]
    pub no_speech_prob: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TranscriptSegmentStatus {
    Success,
    Empty,
    Failed,
}

/// A low-latency live transcript update emitted during recording.
///
/// Updates are keyed by `segment_id`, which is stable for the whole of one
/// utterance: successive updates for the same id *replace* each other as the
/// utterance grows, and the last one carries `is_final = true`. Consumers
/// should key on `segment_id` and never append blindly, or growing partials
/// will read as duplicated speech.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveTranscriptUpdate {
    pub segment_id: String,
    pub session_id: String,
    /// Monotonic index of the utterance this update belongs to.
    pub utterance_index: usize,
    pub start_time_s: f64,
    pub end_time_s: f64,
    pub text: String,
    /// `false` while the utterance is still growing, `true` once committed
    /// (a silence boundary, the window cap, or end of session).
    pub is_final: bool,
    pub latency_ms: u64,
}

/// Real-time live audio energy levels broadcasted to the recording overlay.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AudioLevels {
    pub mic_level: f32,
    pub sys_level: f32,
}

/// Comprehensive diagnostics for a meeting recording session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingDiagnostics {
    pub session_id: String,
    pub state: MeetingState,
    pub duration_seconds: f64,
    pub last_audio_saved_at: Option<String>,
    pub chunk_count: usize,
    pub total_audio_bytes: u64,
    pub last_transcription_at: Option<String>,
    pub transcript_segment_count: usize,
    pub pending_transcription_chunks: usize,
    pub mic_active: bool,
    pub sys_audio_active: bool,
    pub mic_heard: bool,
    pub sys_audio_heard: bool,
    pub mic_rms: f32,
    pub sys_rms: f32,
    pub error: Option<String>,
}
