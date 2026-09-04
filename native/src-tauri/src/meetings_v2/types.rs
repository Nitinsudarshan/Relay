use serde::{Deserialize, Serialize};

/// Authoritative state of a Meeting recording session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[derive(Default)]
pub enum MeetingState {
    #[default]
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
    /// Chunks whose decode was thrown away as something other than speech.
    ///
    /// Surfaced on the session rather than left in the transcript because it is
    /// the number that explains a thin summary: nine rejected chunks is four
    /// minutes of the meeting that never reached the model, and the user is
    /// entitled to see that rather than wonder why the notes are short.
    #[serde(default)]
    pub rejected_chunk_count: usize,
    /// Total voiced seconds measured across every chunk. Compared against
    /// `duration_seconds` this is the meeting's talk-to-silence ratio.
    #[serde(default)]
    pub voiced_seconds: f64,
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
            rejected_chunk_count: 0,
            voiced_seconds: 0.0,
            error_message: None,
        }
    }
}

/// What a directive is telling Relay.
///
/// Every kind here is something the user knows and the recording does not, and
/// each one changes the pipeline's behaviour in a specific way. That is the
/// test for belonging in this list: a kind that only ends up in a prompt is a
/// [`DirectiveKind::Note`], not a kind of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DirectiveKind {
    /// "Speaker 2 is Pranjali." Renames a speaker in the registry.
    SpeakerName,
    /// "Ayush was on this call." Adds a participant, whether or not they spoke.
    Participant,
    /// "It is LanceDB, not Lance TV." Adds a term to this meeting's glossary,
    /// which the normalizer applies to the derived transcript.
    Term,
    /// What the meeting was for. Read as context, never as evidence that
    /// something was decided.
    Agenda,
    /// Anything else worth remembering. The escape hatch, and what the old free
    /// paragraph box became.
    Note,
}

impl DirectiveKind {
    /// Whether this kind needs a subject as well as a value.
    ///
    /// `SpeakerName` needs to know *which* speaker, and `Term` needs to know
    /// what was misheard. The rest are a single value.
    pub fn needs_subject(self) -> bool {
        matches!(self, Self::SpeakerName | Self::Term)
    }
}

/// One short, typed instruction a person gave about a meeting.
///
/// This replaces the paragraph box as the *primary* way of correcting a
/// meeting. A paragraph is the wrong shape for "the recogniser heard my name
/// as Nithin": the user has to write a sentence, and the pipeline has to hope a
/// model notices it and acts on it. A directive is read by the stage that can
/// actually act on it — the registry, the glossary, the participant list — so a
/// name correction takes effect without a model being involved at all.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeetingDirective {
    pub id: String,
    pub kind: DirectiveKind,
    /// For `SpeakerName`, which speaker (an id, a display name, or a
    /// `Speaker N` label). For `Term`, the misheard spelling. Unused otherwise.
    #[serde(default)]
    pub subject: Option<String>,
    /// For `SpeakerName`, the person's name. For `Term`, the correct spelling.
    /// Otherwise the whole content of the directive.
    pub value: String,
    pub created_at: String,
}

impl MeetingDirective {
    /// Builds a directive, generating its id. Returns `None` when the content
    /// is empty or a required subject is missing, so an empty row can never be
    /// stored.
    pub fn new(kind: DirectiveKind, subject: Option<&str>, value: &str) -> Option<Self> {
        let value = value.trim();
        let subject = subject.map(str::trim).filter(|s| !s.is_empty());
        if value.is_empty() {
            return None;
        }
        if kind.needs_subject() && subject.is_none() {
            return None;
        }
        Some(Self {
            id: format!("dir_{}", uuid::Uuid::new_v4().simple()),
            kind,
            subject: subject.map(str::to_string),
            value: value.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// One line, for a shared summary or a log.
    pub fn describe(&self) -> String {
        match (self.kind, self.subject.as_deref()) {
            (DirectiveKind::SpeakerName, Some(subject)) => {
                format!("{subject} is {}", self.value)
            }
            (DirectiveKind::Term, Some(subject)) => {
                format!("\"{subject}\" should read \"{}\"", self.value)
            }
            _ => self.value.clone(),
        }
    }
}

/// Notes a person wrote about a meeting.
///
/// A **source** artifact, not derived: nothing in the processing pipeline may
/// write it, and regenerating a summary never touches it. It sits beside
/// `session.json` and `transcript.jsonl` for the same reason those do — it is
/// something a human produced, and the pipeline's job is to read it.
///
/// Three fields, answering different questions.
///
/// `directives` are short typed instructions — a name correction, a
/// participant, a misheard term. They are the primary surface, because most of
/// what a person wants to tell Relay about a meeting is a correction of a
/// specific thing, and a correction expressed as a sentence in a paragraph
/// depends on a model noticing it. A directive is read by the stage that can
/// act on it.
///
/// `during` is prose the user captured while the meeting was happening, and is
/// still the cheapest quality signal Relay has: three bullets a person typed
/// outrank any amount of prompt tuning at saying which part of ninety minutes
/// mattered. `before` is a rare enrichment — an agenda or a set of questions
/// written in advance — and the pipeline is built so that its absence changes
/// nothing at all.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MeetingNotes {
    /// Short typed instructions. Read by the stage each one concerns.
    #[serde(default)]
    pub directives: Vec<MeetingDirective>,
    /// Written during or after the meeting. Markdown, as the user typed it.
    #[serde(default)]
    pub during: String,
    /// Written before the meeting, if anything was. Roughly one meeting in a
    /// hundred; never required, never a pipeline stage, never a summary section.
    #[serde(default)]
    pub before: String,
    #[serde(default)]
    pub updated_at: Option<String>,
}

impl MeetingNotes {
    /// True when there is nothing here worth sending to a model.
    pub fn is_empty(&self) -> bool {
        self.directives.is_empty()
            && self.during.trim().is_empty()
            && self.before.trim().is_empty()
    }

    pub fn has_during(&self) -> bool {
        !self.during.trim().is_empty()
    }

    pub fn has_before(&self) -> bool {
        !self.before.trim().is_empty()
    }

    /// Directives of one kind, in the order they were added.
    pub fn directives_of(&self, kind: DirectiveKind) -> Vec<&MeetingDirective> {
        self.directives.iter().filter(|d| d.kind == kind).collect()
    }

    /// The prose a model is shown as "notes taken during the meeting".
    ///
    /// Free-text directives are folded in here rather than given a section of
    /// their own: to a summarizer, "remember that the vault rewrite is blocked"
    /// typed as a directive and the same sentence typed in the paragraph box
    /// are the same kind of evidence, and splitting them would only invite the
    /// model to weigh one above the other.
    pub fn during_for_model(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.has_during() {
            parts.push(self.during.trim().to_string());
        }
        for note in self.directives_of(DirectiveKind::Note) {
            parts.push(format!("- {}", note.value.trim()));
        }
        parts.join("\n")
    }

    /// The prose a model is shown as "written before the meeting".
    pub fn before_for_model(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.has_before() {
            parts.push(self.before.trim().to_string());
        }
        for agenda in self.directives_of(DirectiveKind::Agenda) {
            parts.push(format!("- {}", agenda.value.trim()));
        }
        parts.join("\n")
    }

    /// Terms the user says were misheard, as glossary entries.
    pub fn glossary_terms(&self) -> Vec<String> {
        self.directives_of(DirectiveKind::Term)
            .iter()
            .map(|d| d.value.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect()
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
    /// How much of this chunk's audio was actually voice, measured at 20 ms
    /// resolution against the chunk's own noise floor.
    ///
    /// Recorded on every segment because it is the measurement that decides
    /// whether the chunk was decoded at all, and therefore the first thing
    /// worth looking at when a transcript is thinner or stranger than the
    /// meeting was. `None` for transcripts written before v2.6.
    #[serde(default)]
    pub speech: Option<crate::meetings_v2::transcript_health::SpeechProfile>,
    /// Set when a decode was rejected as something other than speech, carrying
    /// the reason and the text that was discarded.
    ///
    /// Present exactly when `status` is [`TranscriptSegmentStatus::Rejected`].
    /// The text is kept so the rejection is auditable from the artifact — a
    /// transcript that silently drops a chunk is not a diagnostic source.
    #[serde(default)]
    pub rejection: Option<crate::meetings_v2::transcript_health::TranscriptRejection>,
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
    /// The chunk contained no speech, so it was never decoded.
    Empty,
    Failed,
    /// The chunk was decoded and the result was not speech — a decoder loop,
    /// subtitle filler over silence, or text the voiced time could not hold.
    ///
    /// Deliberately distinct from `Empty`: `Empty` means the recorder heard
    /// nothing, `Rejected` means Whisper produced something and it was thrown
    /// away. Conflating them hides the failure that this change exists to make
    /// visible.
    Rejected,
}

impl TranscriptSegmentStatus {
    /// Whether this segment contributes text to the derived transcript.
    pub fn has_text(self) -> bool {
        matches!(self, Self::Success)
    }
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
