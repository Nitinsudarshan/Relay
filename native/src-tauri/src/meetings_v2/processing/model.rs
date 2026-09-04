//! The canonical derived model for a processed meeting.
//!
//! Everything in this file is *derived* data: it is computed from the raw
//! transcript and can be thrown away and recomputed at any time. It is
//! persisted to `processing.json`, deliberately separate from `session.json`
//! and `transcript.jsonl`, which are the recorder's source artifacts and are
//! never written by the processing pipeline.
//!
//! The raw transcript is the only source of truth for what was said. Every
//! derived object here therefore carries the segment ids it came from, so
//! "why did Relay think this was an action item?" is answerable from the data
//! alone.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Bumped whenever the shape or semantics of derived output change enough that
/// a previously generated `processing.json` should be regarded as stale.
///
/// v2: action items are qualified by `processing::qualify` before they are
/// persisted, and the summary artifact records what became of the model's draft
/// separately from whether the stage succeeded. Facts extracted under v1 carry
/// action items that never passed the gate.
///
/// v3: normalized segments are one per *utterance* rather than one per
/// 30-second chunk, so segment ids gained an utterance suffix and speaker
/// attribution resolves per utterance. Facts extracted under v1 or v2 cite
/// chunk-level segment ids and carry owners that were demoted to `Unassigned`
/// because chunk-level channel data could not resolve them.
///
/// v4: facts carry the three things a summary needs in order to be a memory
/// rather than a list — the *reason* behind a decision, the kind of claim a key
/// point is (a proposal is not a decision), and the risks and blockers raised.
/// Facts extracted under v1–v3 have no rationale, classify every point as plain
/// discussion, and carry no risks, so a summary regenerated from them is thinner
/// than one regenerated after a forced re-extraction.
pub const PROCESSING_VERSION: u32 = 4;

/// Identifies the `Meeting-rules/` revision the prompts encode. Recorded on
/// every derived artifact so a quality change six months from now can be
/// attributed to a rules change rather than a model change.
pub const RULES_VERSION: &str = "meeting-rules-2026-08-meeting-memory";

/// The local user's stable speaker id. Resolved from the microphone channel,
/// which is the one attribution that needs no model and no diarization.
pub const SPEAKER_ID_ME: &str = "speaker_me";

/// Stable id for the "everyone else" bucket produced by system-audio-only
/// stretches. Diarization can later split this into `speaker_2`, `speaker_3`,
/// … without any other id changing.
pub const SPEAKER_ID_REMOTE: &str = "speaker_1";

/// Which capture channel a transcript segment came from.
///
/// Derived from the per-chunk energy flags the recorder already measures.
/// `Mixed` means both the microphone and system audio were audible within the
/// same 30-second chunk, so the channel says nothing about who spoke —
/// deliberately not resolved to a speaker rather than guessed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SegmentChannel {
    Mic,
    System,
    Mixed,
    Unknown,
}

impl SegmentChannel {
    /// Resolves the channel flags recorded on a raw transcript segment.
    pub fn from_flags(mic_had_audio: bool, sys_had_audio: bool) -> Self {
        match (mic_had_audio, sys_had_audio) {
            (true, false) => Self::Mic,
            (false, true) => Self::System,
            (true, true) => Self::Mixed,
            (false, false) => Self::Unknown,
        }
    }

    /// The speaker this channel unambiguously implies, if any.
    pub fn implied_speaker_id(self) -> Option<&'static str> {
        match self {
            Self::Mic => Some(SPEAKER_ID_ME),
            Self::System => Some(SPEAKER_ID_REMOTE),
            Self::Mixed | Self::Unknown => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mic => "mic",
            Self::System => "system",
            Self::Mixed => "mixed",
            Self::Unknown => "unknown",
        }
    }
}

/// How a speaker's identity was established.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SpeakerOrigin {
    /// Rung 1: microphone stream vs system stream.
    Channel,
    /// Meeting-local self-voice acoustic reference.
    SelfVoiceAnchor,
    /// Rung 4: a diarization cluster.
    Diarization,
    /// Calendar attendee candidate match.
    Calendar,
    /// Contextual speech inference (self-introduction).
    ContextualInference,
    /// Rung 6: the user named this speaker.
    Manual,
}

/// A participant in the meeting.
///
/// `id` is the only identifier that anything else may reference. `display_name`
/// is presentation, is user-editable, and is resolved at read time — renaming a
/// speaker must never rewrite a transcript or a conversation turn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Speaker {
    pub id: String,
    /// `None` until the user names this speaker. Renderers fall back to
    /// `fallback_label`.
    #[serde(default)]
    pub display_name: Option<String>,
    /// What to call this speaker before anyone has named them — "Me",
    /// "Speaker 1". Stored rather than derived so the label a summary was
    /// generated against stays stable.
    pub fallback_label: String,
    pub origin: SpeakerOrigin,
    pub channel: SegmentChannel,
    /// True for the local user, whose commitments are the ones that matter most
    /// when assigning action items.
    #[serde(default)]
    pub is_local_user: bool,
    #[serde(default)]
    pub segment_count: usize,
}

impl Speaker {
    /// The name to show. Never invents a human name.
    pub fn label(&self) -> &str {
        match self.display_name.as_deref() {
            Some(name) if !name.trim().is_empty() => name,
            _ => &self.fallback_label,
        }
    }
}

/// A raw transcript segment after deterministic cleanup.
///
/// `raw_text` is retained alongside `text` so the effect of normalization is
/// inspectable without reopening `transcript.jsonl`, and so a normalization bug
/// can be diagnosed from the derived artifact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NormalizedSegment {
    /// `seg_<chunk_index>`, stable across regeneration because it is derived
    /// from the immutable chunk index.
    pub id: String,
    pub chunk_index: usize,
    /// Which utterance within the chunk this segment is, when the recorder
    /// resolved the chunk into utterances. `None` for a whole-chunk segment.
    #[serde(default)]
    pub utterance_index: Option<usize>,
    pub start_time_s: f64,
    pub end_time_s: f64,
    /// The cleaned text. Meaning-preserving: normalization may repair, it may
    /// not invent.
    pub text: String,
    pub raw_text: String,
    pub channel: SegmentChannel,
    /// `None` where the channel is ambiguous. Never guessed.
    #[serde(default)]
    pub speaker_id: Option<String>,
    /// Names of the normalization rules that changed this segment, for
    /// debugging STT and glossary quality.
    #[serde(default)]
    pub applied_rules: Vec<String>,
}

/// The normalized transcript — the canonical human-readable transcript, and the
/// input to every stage after it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NormalizedTranscript {
    pub segments: Vec<NormalizedSegment>,
    /// How many segments each rule changed. A glossary rule that never fires,
    /// or a dedup rule that fires on every segment, both show up here.
    #[serde(default)]
    pub rule_hits: BTreeMap<String, usize>,
    pub source_char_count: usize,
    pub output_char_count: usize,
    /// Raw segments dropped because they normalized to nothing.
    #[serde(default)]
    pub dropped_segment_count: usize,
}

impl NormalizedTranscript {
    /// The normalized transcript as one block of prose.
    pub fn plain_text(&self) -> String {
        self.segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn word_count(&self) -> usize {
        self.segments
            .iter()
            .map(|s| s.text.split_whitespace().count())
            .sum()
    }
}

/// One speaker's contiguous stretch of the conversation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationTurn {
    pub id: String,
    /// `None` where attribution was ambiguous; renderers say so rather than
    /// picking a speaker.
    #[serde(default)]
    pub speaker_id: Option<String>,
    pub start_time_s: f64,
    pub end_time_s: f64,
    pub text: String,
    /// The normalized segments merged into this turn.
    pub segment_ids: Vec<String>,
    /// Confidence of attribution for this turn.
    #[serde(default)]
    pub confidence: Option<f32>,
}

/// Canonical speaker turn alias for conversational unit.
pub type SpeakerTurn = ConversationTurn;

/// The speaker-labelled, chronological, readable transcript.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Conversation {
    pub turns: Vec<ConversationTurn>,
    /// Turns whose speaker could not be resolved. Surfaced so the UI can be
    /// honest about how much of the conversation is attributed.
    #[serde(default)]
    pub unattributed_turn_count: usize,
}

/// Who owns an action item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OwnerType {
    /// The local user committed to it in their own voice.
    Me,
    /// A specific identified speaker.
    Speaker,
    /// Named in the transcript but not matchable to a speaker (e.g. someone who
    /// was not in the call).
    External,
    /// Stated as a group commitment ("we'll").
    Group,
    /// Ownership was not established. The honest default.
    Unassigned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActionItemStatus {
    Open,
    Done,
}

/// A commitment made in the meeting that has to happen after it ends.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionItem {
    pub id: String,
    pub description: String,
    pub owner_type: OwnerType,
    /// Set only for `Me` and `Speaker`. The rendered name is resolved from the
    /// speaker registry at read time, so a rename updates this item's display
    /// without regenerating anything.
    #[serde(default)]
    pub owner_speaker_id: Option<String>,
    /// A name for owners that are not speakers (`External`). Never a
    /// substitute for an unresolved speaker.
    #[serde(default)]
    pub owner_label: Option<String>,
    /// ISO `YYYY-MM-DD`, and only when a date was actually spoken. Never
    /// inferred from "soon" or "next week" without an anchor.
    #[serde(default)]
    pub deadline: Option<String>,
    #[serde(default = "default_action_status")]
    pub status: ActionItemStatus,
    /// Normalized segments this was read out of.
    #[serde(default)]
    pub source_segment_ids: Vec<String>,
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    /// The Kanban card this item was pushed to, if it has been.
    ///
    /// Recorded so the same commitment cannot reach the board twice, and so the
    /// meeting can show which of its to-dos have already left the app. `None`
    /// means it has not been pushed.
    #[serde(default)]
    pub kanban_card_id: Option<String>,
}

fn default_action_status() -> ActionItemStatus {
    ActionItemStatus::Open
}

fn default_confidence() -> f32 {
    0.5
}

/// Something the meeting settled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Decision {
    pub id: String,
    pub statement: String,
    /// Why it was settled this way, when the meeting said so.
    ///
    /// The single most valuable field on a decision and the one a summary is
    /// most likely to lose. "Move the launch to Monday" is a note; "move the
    /// launch to Monday because the payment integration still has three
    /// blocking bugs" is a memory — six weeks later it is the reason, not the
    /// date, that someone needs. `None` when the meeting stated no reason;
    /// never filled in with a plausible one.
    #[serde(default)]
    pub rationale: Option<String>,
    /// Speaker id of whoever decided, when that is known.
    #[serde(default)]
    pub decided_by_speaker_id: Option<String>,
    #[serde(default)]
    pub source_segment_ids: Vec<String>,
    #[serde(default = "default_confidence")]
    pub confidence: f32,
}

/// A subject that occupied a sustained stretch of the meeting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Topic {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub segment_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EntityKind {
    Person,
    Organization,
    Product,
    Project,
    Technology,
    Other,
}

/// A named thing referenced in the meeting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    pub id: String,
    pub name: String,
    pub kind: EntityKind,
    #[serde(default)]
    pub segment_ids: Vec<String>,
}

/// What kind of exposure a risk describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RiskKind {
    /// Something that could go wrong.
    #[default]
    Risk,
    /// Something already stopping work.
    Blocker,
    /// Progress waits on someone or something outside the room.
    Dependency,
    /// A constraint the meeting has to work inside.
    Constraint,
}

impl RiskKind {
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_lowercase().replace(['_', '-', ' '], "").as_str() {
            "blocker" | "blocked" | "blocking" => Self::Blocker,
            "dependency" | "dependent" => Self::Dependency,
            "constraint" | "limitation" => Self::Constraint,
            _ => Self::Risk,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Risk => "Risk",
            Self::Blocker => "Blocker",
            Self::Dependency => "Dependency",
            Self::Constraint => "Constraint",
        }
    }
}

/// A risk, blocker, dependency, or constraint the meeting actually raised.
///
/// Deliberately a separate collection rather than a flavour of key point: a
/// blocker is the thing a reader scans for first, and burying it in prose is
/// how it gets missed. Never inferred — a discussion that merely sounds
/// serious is not a risk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Risk {
    pub id: String,
    pub statement: String,
    pub kind: RiskKind,
    /// Who raised it, when the transcript makes that clear.
    #[serde(default)]
    pub raised_by_speaker_id: Option<String>,
    #[serde(default)]
    pub source_segment_ids: Vec<String>,
}

/// Something raised and left unresolved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenQuestion {
    pub id: String,
    pub question: String,
    #[serde(default)]
    pub source_segment_ids: Vec<String>,
}

/// What kind of claim a key point is.
///
/// The distinction exists to keep the four categories from collapsing into each
/// other. "We could launch Friday" and "let's launch Monday" are different
/// facts about a meeting, and a schema with only one slot for both is an
/// invitation to file the first as the second. Giving a proposal somewhere
/// honest to live is what stops it being promoted into `decisions`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum KeyPointKind {
    /// Something explained, reported, or established. The default.
    #[default]
    Discussion,
    /// Floated but not settled — "we could", "what if we".
    Proposal,
    /// Advocated by someone, but not adopted by the meeting.
    Recommendation,
    /// A material difference of position that affected the outcome.
    Disagreement,
    /// A cost knowingly accepted in exchange for something else.
    Tradeoff,
}

impl KeyPointKind {
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_lowercase().replace(['_', '-', ' '], "").as_str() {
            "proposal" | "proposed" | "suggestion" => Self::Proposal,
            "recommendation" | "recommended" => Self::Recommendation,
            "disagreement" | "disagreed" | "conflict" => Self::Disagreement,
            "tradeoff" => Self::Tradeoff,
            _ => Self::Discussion,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Discussion => "discussion",
            Self::Proposal => "proposal",
            Self::Recommendation => "recommendation",
            Self::Disagreement => "disagreement",
            Self::Tradeoff => "tradeoff",
        }
    }
}

/// A substantive discussion point — what a reader who missed the meeting needs
/// in order to follow it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyPoint {
    pub id: String,
    pub text: String,
    /// Discussion unless the meeting made it something more specific. Carried
    /// into Stage B so the prose can say "was proposed" where the meeting only
    /// proposed, and "was agreed" only where it agreed.
    #[serde(default)]
    pub kind: KeyPointKind,
    #[serde(default)]
    pub topic_id: Option<String>,
    #[serde(default)]
    pub source_segment_ids: Vec<String>,
}

/// Lightweight classification, used for grouping and for finding related
/// meetings. Deliberately a small closed set rather than free text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MeetingType {
    Scrum,
    OneOnOne,
    ProjectReview,
    ClientMeeting,
    Planning,
    Interview,
    General,
}

impl MeetingType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Scrum => "Scrum",
            Self::OneOnOne => "1:1",
            Self::ProjectReview => "Project Review",
            Self::ClientMeeting => "Client Meeting",
            Self::Planning => "Planning",
            Self::Interview => "Interview",
            Self::General => "General",
        }
    }

    /// Parses a model-supplied type string. Unrecognized values become
    /// `General` rather than inventing a category.
    pub fn parse(raw: &str) -> Self {
        let key: String = raw
            .trim()
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect();
        match key.as_str() {
            "scrum" | "standup" | "dailyscrum" | "dailystandup" => Self::Scrum,
            "oneonone" | "11" | "1on1" | "onetoone" => Self::OneOnOne,
            "projectreview" | "review" => Self::ProjectReview,
            "clientmeeting" | "client" => Self::ClientMeeting,
            "planning" | "sprintplanning" => Self::Planning,
            "interview" => Self::Interview,
            _ => Self::General,
        }
    }
}

/// The structured intermediate representation — Stage A's output, and the
/// single source every derived view is projected from.
///
/// Prose generation reads this, not the transcript, so no model is ever asked
/// to comprehend, extract, and write at the same time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeetingFacts {
    pub title: String,
    pub meeting_type: MeetingType,
    #[serde(default)]
    pub key_points: Vec<KeyPoint>,
    #[serde(default)]
    pub topics: Vec<Topic>,
    #[serde(default)]
    pub decisions: Vec<Decision>,
    #[serde(default)]
    pub action_items: Vec<ActionItem>,
    #[serde(default)]
    pub open_questions: Vec<OpenQuestion>,
    /// Risks, blockers, dependencies, and constraints the meeting raised.
    #[serde(default)]
    pub risks: Vec<Risk>,
    #[serde(default)]
    pub entities: Vec<Entity>,
    /// Speaker ids that actually contributed, for the related-meetings signal.
    #[serde(default)]
    pub speaker_ids: Vec<String>,
    /// True when these facts came from the deterministic extractor because no
    /// model was reachable. Surfaced so a thin summary is explainable.
    #[serde(default)]
    pub deterministic: bool,
}

/// How long and how detailed a generated summary should be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SummaryMode {
    Concise,
    #[default]
    Standard,
    Detailed,
}

impl SummaryMode {
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_lowercase().as_str() {
            "concise" => Self::Concise,
            "detailed" => Self::Detailed,
            _ => Self::Standard,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Concise => "Concise",
            Self::Standard => "Standard",
            Self::Detailed => "Detailed",
        }
    }

    /// The ceiling this mode allows a summary of *any* meeting.
    ///
    /// A ceiling, not a target. What a given meeting is actually allowed is
    /// computed by `processing::length` from that meeting's own transcript, and
    /// only ever binds below this. The distinction matters: as a fixed cap these
    /// numbers rejected legitimate summaries of long meetings and left short
    /// ones room to pad, because the same number cannot be right for a
    /// four-minute call and a two-hour planning session.
    pub fn max_words(self) -> usize {
        match self {
            Self::Concise => 280,
            Self::Standard => 650,
            Self::Detailed => 1_200,
        }
    }
}

/// A named presentation treatment applied on top of a summary mode.
///
/// An extension changes instructions and layout only. It never bypasses
/// extraction, so two extensions of the same meeting cannot disagree about
/// what was decided.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeetingExtension {
    pub id: String,
    pub name: String,
    /// Appended to the Stage B instructions.
    pub instructions: String,
    /// True for the extensions Relay ships; those cannot be deleted.
    #[serde(default)]
    pub builtin: bool,
}

/// Severity of a validator finding. `Error` means the artifact is not fit to
/// show and the deterministic renderer is used instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IssueSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationIssue {
    /// Stable machine-readable code, e.g. `SUMMARY_TOO_LONG`.
    pub code: String,
    pub severity: IssueSeverity,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ValidationReport {
    pub passed: bool,
    #[serde(default)]
    pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    pub fn ok() -> Self {
        Self {
            passed: true,
            issues: Vec::new(),
        }
    }

    pub fn from_issues(issues: Vec<ValidationIssue>) -> Self {
        let passed = !issues.iter().any(|i| i.severity == IssueSeverity::Error);
        Self { passed, issues }
    }

    pub fn has_errors(&self) -> bool {
        self.issues
            .iter()
            .any(|i| i.severity == IssueSeverity::Error)
    }
}

/// What became of the model's proposed prose.
///
/// Separated from the summary stage's own outcome because they are different
/// facts about a run: a rejected model draft followed by a valid deterministic
/// render is a **successful** summary stage that happens to have rejected the
/// model. Conflating the two is what made the UI say "Summary unavailable" over
/// a summary that was sitting right there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProviderOutputStatus {
    /// No model was asked — the transcript was too short, or none was configured.
    #[default]
    NotAttempted,
    /// A model answered and its prose is what the user is reading.
    Accepted,
    /// A model answered and the validator refused the answer.
    Rejected,
    /// A model was asked and could not answer.
    Unavailable,
}

impl ProviderOutputStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::NotAttempted => "not attempted",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Unavailable => "unavailable",
        }
    }
}

/// How the prose the user is reading came to exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SummarySource {
    /// A model wrote it and the validator accepted it.
    #[default]
    Model,
    /// Rendered from model-extracted facts without a model writing the prose.
    /// Comprehension was a model's; presentation was not.
    DeterministicPresentation,
    /// Rendered from cue-extracted facts. No model was involved at any stage,
    /// and the points are lifted from the transcript rather than understood.
    DeterministicExtraction,
}

impl SummarySource {
    pub fn is_deterministic(self) -> bool {
        !matches!(self, Self::Model)
    }

    /// What to tell the user about where this text came from. Never calls
    /// deterministic output an AI summary.
    pub fn provenance(self) -> &'static str {
        match self {
            Self::Model => "Written by a language model from the extracted facts.",
            Self::DeterministicPresentation => {
                "Written without a language model, from facts a model extracted."
            }
            Self::DeterministicExtraction => {
                "Written without a language model. The points are taken from the transcript \
rather than summarized."
            }
        }
    }
}

/// A generated human-facing summary, with everything needed to explain why it
/// reads the way it does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SummaryArtifact {
    pub markdown: String,
    pub mode: SummaryMode,
    pub extension_id: String,
    pub generated_at: String,
    pub provider: String,
    pub model: String,
    pub processing_version: u32,
    pub rules_version: String,
    /// True when the prose was rendered deterministically from facts because no
    /// model was reachable, or because the model's output failed validation.
    #[serde(default)]
    pub deterministic: bool,
    /// Where the prose came from, and — for deterministic prose — whether the
    /// *facts* behind it were understood by a model or lifted from the
    /// transcript. `deterministic` alone cannot tell those apart.
    #[serde(default)]
    pub source: SummarySource,
    /// True when the first draft failed validation and a corrected one was
    /// requested. Recorded because "the model needed a second try" and "the
    /// model could not do it" are different quality signals, and the rate of the
    /// first is what says whether the contract is clear enough.
    #[serde(default)]
    pub repair_attempted: bool,
    /// The word ceiling this meeting's own length allowed, so a summary that
    /// looks short or long can be judged against what it was asked for.
    #[serde(default)]
    pub length_budget_words: Option<usize>,
    /// What became of the model's draft, independently of whether this summary
    /// stage succeeded.
    #[serde(default)]
    pub provider_output_status: ProviderOutputStatus,
    /// True when the deterministic renderer produced the text being shown.
    #[serde(default)]
    pub fallback_used: bool,
    /// The issues that caused a model draft to be rejected. Kept as the record
    /// of why this summary reads the way it does — deliberately *not* merged
    /// into `validation`, which describes only the prose actually shown.
    #[serde(default)]
    pub rejected_issues: Vec<ValidationIssue>,
    /// Set when a speaker was renamed after this summary was written. The prose
    /// still carries the old label; action items and the conversation resolve
    /// live, so only the prose is stale.
    #[serde(default)]
    pub speaker_names_stale: bool,
    /// The verdict on the prose the user is reading. Nothing else.
    #[serde(default)]
    pub validation: ValidationReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StageStatus {
    #[default]
    NotRun,
    Running,
    Success,
    Failed,
    /// Deliberately not run — e.g. the conversation transcript is switched off
    /// in settings. Distinct from `Failed` so the UI does not offer a retry for
    /// something the user turned off.
    Skipped,
}

/// Per-stage bookkeeping. This is what makes a failed meeting diagnosable
/// without reading source: which stages ran, which model, how long, and what
/// the validator said.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct StageState {
    pub status: StageStatus,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub finished_at: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub input_chars: Option<usize>,
    #[serde(default)]
    pub output_chars: Option<usize>,
    #[serde(default)]
    pub validation: Option<ValidationReport>,
    /// Set on the extraction stage: how many action-item candidates there were
    /// and what happened to them. Counts only — no candidate text, so this is
    /// safe to persist and to log.
    #[serde(default)]
    pub action_diagnostics: Option<super::qualify::ActionDiagnostics>,
}

impl StageState {
    pub fn skipped(reason: &str) -> Self {
        Self {
            status: StageStatus::Skipped,
            error: Some(reason.to_string()),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct StageStates {
    #[serde(default)]
    pub normalization: StageState,
    #[serde(default)]
    pub speakers: StageState,
    #[serde(default)]
    pub conversation: StageState,
    #[serde(default)]
    pub extraction: StageState,
    #[serde(default)]
    pub summary: StageState,
}

/// Overall processing state, rolled up from the individual stages. Never
/// conflated with `MeetingState`, which is about recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProcessingStatus {
    #[default]
    NotStarted,
    Running,
    /// Everything that was asked for succeeded.
    Ready,
    /// Some stages succeeded and some did not. The meeting is usable.
    Partial,
    /// Nothing usable was produced.
    Failed,
}

/// Where a meeting's exported Scribble lives, so the meeting can link to it
/// instead of duplicating it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScribbleRef {
    pub scribble_id: String,
    pub created_at: String,
    pub title: String,
}

/// The complete derived artifact for one meeting: `processing.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeetingProcessing {
    pub meeting_id: String,
    pub processing_version: u32,
    pub rules_version: String,
    pub updated_at: String,
    pub status: ProcessingStatus,
    #[serde(default)]
    pub stages: StageStates,
    #[serde(default)]
    pub normalized: Option<NormalizedTranscript>,
    #[serde(default)]
    pub speakers: Vec<Speaker>,
    /// The acoustic speaker separation the roster was built from, when one ran.
    ///
    /// Kept so the UI can say how a speaker was found and how confident the
    /// separation was, and so `prepare` need not re-read every chunk WAV on
    /// each open. `None` means attribution used the capture channel alone.
    #[serde(default)]
    pub diarization: Option<crate::meetings_v2::diarize::Diarization>,
    #[serde(default)]
    pub conversation: Option<Conversation>,
    /// The meeting's counted facts — participants, timing, transcript health.
    ///
    /// Distinct from `facts` below, which is what a model read out of the
    /// transcript and can be wrong. Everything here is counted or measured.
    #[serde(default)]
    pub metadata: Option<crate::meetings_v2::processing::metadata::MeetingMetadata>,
    /// Names the transcript itself offered (rung 5).
    ///
    /// Kept apart from the roster: these label a participant without claiming
    /// the user confirmed them, so `Speaker 2` never silently becomes a name
    /// nobody approved.
    #[serde(default)]
    pub names: Option<crate::meetings_v2::processing::names::NameFindings>,
    /// Instructions the user gave that could not be applied — a name correction
    /// naming a speaker this meeting does not have, say. Surfaced rather than
    /// swallowed, or the user assumes the correction took.
    #[serde(default)]
    pub unresolved_directives:
        Vec<crate::meetings_v2::processing::directives::UnresolvedDirective>,
    #[serde(default)]
    pub facts: Option<MeetingFacts>,
    #[serde(default)]
    pub summary: Option<SummaryArtifact>,
    #[serde(default)]
    pub scribble_ref: Option<ScribbleRef>,
    /// Explicit provenance mapping attributing each utterance to identity evidence.
    #[serde(default)]
    pub speaker_assignments: Vec<crate::meetings_v2::types::SpeakerAssignment>,
}

impl MeetingProcessing {
    pub fn new(meeting_id: &str) -> Self {
        Self {
            meeting_id: meeting_id.to_string(),
            processing_version: PROCESSING_VERSION,
            rules_version: RULES_VERSION.to_string(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            status: ProcessingStatus::NotStarted,
            stages: StageStates::default(),
            normalized: None,
            speakers: Vec::new(),
            diarization: None,
            conversation: None,
            metadata: None,
            names: None,
            unresolved_directives: Vec::new(),
            facts: None,
            summary: None,
            scribble_ref: None,
            speaker_assignments: Vec::new(),
        }
    }

    /// Recomputes `status` from the stage states.
    ///
    /// A meeting counts as `Partial` — not `Failed` — as long as any stage
    /// produced something, because the raw transcript and whatever was derived
    /// remain usable. Only a run where nothing succeeded is `Failed`.
    /// When a valid summary is present (including deterministic fallback), the meeting
    /// is treated as ready rather than failed.
    pub fn recompute_status(&mut self) {
        let stages = [
            &self.stages.normalization,
            &self.stages.speakers,
            &self.stages.conversation,
            &self.stages.extraction,
            &self.stages.summary,
        ];

        let any_running = stages.iter().any(|s| s.status == StageStatus::Running);
        let succeeded = stages
            .iter()
            .filter(|s| s.status == StageStatus::Success)
            .count();
        let failed = stages
            .iter()
            .filter(|s| s.status == StageStatus::Failed)
            .count();
        let attempted = stages
            .iter()
            .filter(|s| s.status != StageStatus::NotRun)
            .count();

        // If a valid summary exists (e.g. deterministic summary), the summary is ready
        // and should not cause the whole meeting to read as degraded/partial.
        let summary_ready = self.summary.is_some();
        let effective_failed = if summary_ready && self.stages.summary.status == StageStatus::Failed {
            failed.saturating_sub(1)
        } else {
            failed
        };

        self.status = if any_running {
            ProcessingStatus::Running
        } else if attempted == 0 {
            ProcessingStatus::NotStarted
        } else if effective_failed == 0 {
            ProcessingStatus::Ready
        } else if succeeded > 0 || summary_ready {
            ProcessingStatus::Partial
        } else {
            ProcessingStatus::Failed
        };
    }

    /// Looks up a speaker's presentation name without inventing one.
    pub fn speaker_label(&self, speaker_id: &str) -> Option<&str> {
        self.speakers
            .iter()
            .find(|s| s.id == speaker_id)
            .map(|s| s.label())
    }
}

/// One line of `processing_log.jsonl` — the observability record for a single
/// stage run. Deliberately carries sizes and outcomes, never transcript text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessingLogEntry {
    pub meeting_id: String,
    pub stage: String,
    pub status: String,
    pub at: String,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub input_chars: Option<usize>,
    #[serde(default)]
    pub output_chars: Option<usize>,
    #[serde(default)]
    pub validator_passed: Option<bool>,
    #[serde(default)]
    pub validator_issue_codes: Vec<String>,
    #[serde(default)]
    pub error: Option<String>,
    /// Extraction only. Counts, never candidate text — the privacy guarantee on
    /// this log is that it explains a run without quoting the meeting.
    #[serde(default)]
    pub action_diagnostics: Option<super::qualify::ActionDiagnostics>,
    /// Summary only. What became of the model's draft, so a fallback is
    /// distinguishable from a failure without reading `processing.json`.
    #[serde(default)]
    pub provider_output_status: Option<String>,
    #[serde(default)]
    pub fallback_used: Option<bool>,
    pub processing_version: u32,
    pub rules_version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_resolves_only_unambiguous_speakers() {
        assert_eq!(
            SegmentChannel::from_flags(true, false).implied_speaker_id(),
            Some(SPEAKER_ID_ME)
        );
        assert_eq!(
            SegmentChannel::from_flags(false, true).implied_speaker_id(),
            Some(SPEAKER_ID_REMOTE)
        );
        // Both channels audible in the same chunk says nothing about who spoke.
        assert_eq!(
            SegmentChannel::from_flags(true, true).implied_speaker_id(),
            None
        );
        // Old transcripts have no channel data at all.
        assert_eq!(
            SegmentChannel::from_flags(false, false).implied_speaker_id(),
            None
        );
    }

    #[test]
    fn a_speaker_falls_back_to_its_label_rather_than_inventing_a_name() {
        let mut speaker = Speaker {
            id: SPEAKER_ID_REMOTE.to_string(),
            display_name: None,
            fallback_label: "Speaker 1".to_string(),
            origin: SpeakerOrigin::Channel,
            channel: SegmentChannel::System,
            is_local_user: false,
            segment_count: 3,
        };
        assert_eq!(speaker.label(), "Speaker 1");

        speaker.display_name = Some("   ".to_string());
        assert_eq!(speaker.label(), "Speaker 1", "whitespace is not a name");

        speaker.display_name = Some("Pranjali".to_string());
        assert_eq!(speaker.label(), "Pranjali");
    }

    #[test]
    fn a_partly_failed_run_stays_usable_rather_than_failed() {
        let mut processing = MeetingProcessing::new("meet_1");
        processing.stages.normalization.status = StageStatus::Success;
        processing.stages.summary.status = StageStatus::Failed;
        processing.recompute_status();
        assert_eq!(processing.status, ProcessingStatus::Partial);

        processing.stages.normalization.status = StageStatus::Failed;
        processing.recompute_status();
        assert_eq!(processing.status, ProcessingStatus::Failed);

        processing.stages.normalization.status = StageStatus::Success;
        processing.stages.summary.status = StageStatus::Success;
        processing.recompute_status();
        assert_eq!(processing.status, ProcessingStatus::Ready);
    }

    #[test]
    fn a_skipped_stage_is_not_a_failure() {
        let mut processing = MeetingProcessing::new("meet_1");
        processing.stages.normalization.status = StageStatus::Success;
        processing.stages.conversation = StageState::skipped("disabled in settings");
        processing.recompute_status();
        assert_eq!(processing.status, ProcessingStatus::Ready);
    }

    #[test]
    fn unknown_meeting_types_do_not_become_new_categories() {
        assert_eq!(MeetingType::parse("Daily Scrum"), MeetingType::Scrum);
        assert_eq!(MeetingType::parse("1:1"), MeetingType::OneOnOne);
        assert_eq!(MeetingType::parse("Sprint Planning"), MeetingType::Planning);
        assert_eq!(
            MeetingType::parse("Quarterly Vibes Alignment"),
            MeetingType::General
        );
    }

    #[test]
    fn validation_fails_only_on_errors() {
        let warnings = ValidationReport::from_issues(vec![ValidationIssue {
            code: "X".into(),
            severity: IssueSeverity::Warning,
            message: "m".into(),
        }]);
        assert!(warnings.passed);

        let errors = ValidationReport::from_issues(vec![ValidationIssue {
            code: "Y".into(),
            severity: IssueSeverity::Error,
            message: "m".into(),
        }]);
        assert!(!errors.passed);
        assert!(errors.has_errors());
    }
}
