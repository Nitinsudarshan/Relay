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
            pending_transcription_chunks: 0,
            error_message: None,
        }
    }
}

/// A single incremental transcript segment derived from an audio chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegment {
    pub chunk_index: usize,
    pub start_time_s: f64,
    pub end_time_s: f64,
    pub text: String,
    pub created_at: String,
    pub status: TranscriptSegmentStatus,
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
