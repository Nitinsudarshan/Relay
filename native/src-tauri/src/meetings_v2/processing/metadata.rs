//! The meeting's own facts: who was there, when, for how long, and how much of
//! it survived transcription.
//!
//! This is separate from `MeetingFacts` on purpose. Facts are what a model read
//! out of the transcript and can be wrong; everything here is counted or
//! measured, and is the header a person needs before reading anything derived.
//! A summary of a 44-minute meeting is a different object from a summary of
//! four minutes of it, and the reader is entitled to know which one they have.
//!
//! It is also what makes a summary shareable. A summary pasted into a message
//! with no date, no participants and no duration is a wall of claims with no
//! provenance; the same summary under this header is a document.

use super::conversation::format_timestamp;
use super::model::{Conversation, NormalizedTranscript, Speaker, SpeakerOrigin};
use super::names::{NameEvidence, NameFindings};
use crate::meetings_v2::types::{
    DirectiveKind, MeetingNotes, MeetingSession, TranscriptSegment, TranscriptSegmentStatus,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// How a participant came to be on the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ParticipantOrigin {
    /// The local user, established by the microphone channel.
    LocalUser,
    /// A voice the capture channel separated. One bucket for everyone remote.
    Channel,
    /// A voice acoustic separation isolated.
    Diarization,
    /// Named themselves in the meeting.
    SelfIntroduced,
    /// Named by somebody else, and not matched to a voice.
    Mentioned,
    /// The user said they were there.
    Stated,
}

impl ParticipantOrigin {
    /// Whether this participant actually contributed audio.
    pub fn spoke(self) -> bool {
        matches!(
            self,
            Self::LocalUser | Self::Channel | Self::Diarization | Self::SelfIntroduced
        )
    }

    /// A short phrase for the participant chip's tooltip.
    pub fn describe(self) -> &'static str {
        match self {
            Self::LocalUser => "you, from the microphone",
            Self::Channel => "everyone on the call, told apart by capture channel only",
            Self::Diarization => "a distinct voice in the recording",
            Self::SelfIntroduced => "introduced themselves in the meeting",
            Self::Mentioned => "named in the meeting, not matched to a voice",
            Self::Stated => "you said they were here",
        }
    }
}

/// One person the meeting involved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Participant {
    /// The speaker this is, when they have a voice in the recording.
    #[serde(default)]
    pub speaker_id: Option<String>,
    /// What to show. Never invented: either a name somebody supplied or a
    /// `Speaker N` label.
    pub label: String,
    /// True when `label` is a real name rather than a positional label.
    pub is_named: bool,
    /// Whether the name is confirmed by a person, or only inferred.
    pub is_confirmed: bool,
    pub origin: ParticipantOrigin,
    #[serde(default)]
    pub is_local_user: bool,
    /// Seconds of the meeting attributed to them.
    #[serde(default)]
    pub speaking_seconds: f64,
    #[serde(default)]
    pub turn_count: usize,
    /// Their share of the attributed talking time, `0.0..=1.0`.
    #[serde(default)]
    pub share_of_talk: f32,
}

/// What became of every recorded chunk.
///
/// Surfaced on the meeting rather than buried in a log because it is the number
/// that explains a thin summary. Nine rejected chunks is four and a half
/// minutes of a meeting that never reached the model, and the honest thing is
/// to say so on the meeting itself.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TranscriptHealth {
    pub chunk_count: usize,
    /// Chunks that produced usable speech.
    pub decoded_chunk_count: usize,
    /// Chunks that held no speech, so were never decoded.
    pub empty_chunk_count: usize,
    /// Chunks decoded into something that was not speech and thrown away.
    pub rejected_chunk_count: usize,
    /// Chunks whose decode errored.
    pub failed_chunk_count: usize,
    /// Voiced seconds measured across the recording.
    pub voiced_seconds: f64,
    /// Seconds of recording the rejected chunks covered.
    pub rejected_seconds: f64,
    /// Rejections by reason key.
    #[serde(default)]
    pub rejection_reasons: BTreeMap<String, usize>,
    /// Spans withheld at read time because they were recorded before the
    /// recorder screened for hallucination.
    #[serde(default)]
    pub withheld_on_read: BTreeMap<String, usize>,
    #[serde(default)]
    pub withheld_word_count: usize,
}

impl TranscriptHealth {
    /// Whether anything is worth telling the user about.
    pub fn has_losses(&self) -> bool {
        self.rejected_chunk_count > 0
            || self.failed_chunk_count > 0
            || !self.withheld_on_read.is_empty()
    }

    /// A one-line explanation, or `None` when the transcript is clean.
    pub fn describe(&self) -> Option<String> {
        if !self.has_losses() {
            return None;
        }
        let mut parts: Vec<String> = Vec::new();
        if self.rejected_chunk_count > 0 {
            parts.push(format!(
                "{} {} of audio produced no usable speech and was discarded",
                format_timestamp(self.rejected_seconds),
                if self.rejected_chunk_count == 1 {
                    "chunk"
                } else {
                    "of recording"
                }
            ));
        }
        if self.failed_chunk_count > 0 {
            parts.push(format!(
                "{} {} failed to transcribe",
                self.failed_chunk_count,
                if self.failed_chunk_count == 1 {
                    "chunk"
                } else {
                    "chunks"
                }
            ));
        }
        let withheld: usize = self.withheld_on_read.values().sum();
        if withheld > 0 {
            parts.push(format!(
                "{withheld} spans recorded before hallucination screening existed were withheld \
({} words)",
                self.withheld_word_count
            ));
        }
        Some(parts.join("; "))
    }
}

/// Everything about a meeting that is counted rather than inferred.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeetingMetadata {
    pub title: String,
    /// `YYYY-MM-DD`, from when the recording started.
    pub date_iso: String,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub ended_at: Option<String>,
    pub duration_seconds: f64,
    #[serde(default)]
    pub paused_seconds: f64,
    /// People who contributed audio.
    pub speaking_participant_count: usize,
    /// Everyone the meeting involved, including people who were named but never
    /// heard.
    pub participants: Vec<Participant>,
    pub chunk_count: usize,
    pub word_count: usize,
    pub turn_count: usize,
    pub health: TranscriptHealth,
    /// How speakers were told apart, for the header's provenance line.
    pub speaker_method: SpeakerMethod,
}

/// What established the speaker roster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SpeakerMethod {
    /// No attribution ran.
    None,
    /// Capture channel only: the local user and one bucket for everyone else.
    Channel,
    /// Individual voices were separated acoustically.
    Diarization,
}

impl MeetingMetadata {
    /// The header a shared summary opens with.
    ///
    /// Deliberately terse and factual. Every line is something counted, so
    /// nothing here can be wrong in the way a generated sentence can.
    pub fn to_markdown(&self) -> String {
        let mut out = format!("# {}\n\n", self.title);

        let mut facts: Vec<String> = vec![
            format!("**Date** {}", self.date_iso),
            format!("**Duration** {}", format_timestamp(self.duration_seconds)),
        ];
        if let Some(time) = self.local_time() {
            facts.insert(1, format!("**Started** {time}"));
        }
        if self.paused_seconds > 1.0 {
            facts.push(format!(
                "**Paused** {}",
                format_timestamp(self.paused_seconds)
            ));
        }
        facts.push(format!("**Words** {}", self.word_count));
        out.push_str(&facts.join(" · "));
        out.push_str("\n\n");

        if !self.participants.is_empty() {
            out.push_str(&format!(
                "**Participants ({})** ",
                self.participants.len()
            ));
            let names: Vec<String> = self
                .participants
                .iter()
                .map(|p| match (p.is_named, p.origin.spoke()) {
                    (_, false) => format!("{} (mentioned)", p.label),
                    (false, true) => format!("{} (unnamed)", p.label),
                    (true, true) => p.label.clone(),
                })
                .collect();
            out.push_str(&names.join(", "));
            out.push_str("\n\n");
        }

        if let Some(note) = self.health.describe() {
            out.push_str(&format!("> Transcript note: {note}.\n\n"));
        }

        out
    }

    /// Local clock time the recording started, when it is known.
    fn local_time(&self) -> Option<String> {
        let started = self.started_at.as_deref()?;
        let parsed = chrono::DateTime::parse_from_rfc3339(started).ok()?;
        Some(
            parsed
                .with_timezone(&chrono::Local)
                .format("%-I:%M %p")
                .to_string(),
        )
    }
}

/// Everything the metadata is assembled from.
pub struct MetadataInput<'a> {
    pub session: &'a MeetingSession,
    pub raw_segments: &'a [TranscriptSegment],
    pub normalized: Option<&'a NormalizedTranscript>,
    pub conversation: Option<&'a Conversation>,
    pub speakers: &'a [Speaker],
    pub names: &'a NameFindings,
    pub notes: &'a MeetingNotes,
    pub diarized: bool,
    /// Spans withheld at read time, by reason key.
    pub withheld_on_read: BTreeMap<String, usize>,
    pub withheld_word_count: usize,
}

/// Builds a meeting's metadata.
pub fn build(mut input: MetadataInput<'_>) -> MeetingMetadata {
    let session = input.session;
    let health = build_health(
        input.raw_segments,
        std::mem::take(&mut input.withheld_on_read),
        input.withheld_word_count,
    );
    let participants = build_participants(&input);

    MeetingMetadata {
        title: session.title.clone(),
        date_iso: session
            .started_at
            .as_deref()
            .unwrap_or(session.created_at.as_str())
            .split('T')
            .next()
            .unwrap_or("")
            .to_string(),
        started_at: session.started_at.clone(),
        ended_at: session.ended_at.clone(),
        duration_seconds: session.duration_seconds,
        paused_seconds: session.paused_seconds,
        speaking_participant_count: participants.iter().filter(|p| p.origin.spoke()).count(),
        participants,
        chunk_count: session.chunk_count,
        word_count: input
            .normalized
            .map(|n| n.word_count())
            .unwrap_or(session.word_count),
        turn_count: input.conversation.map(|c| c.turns.len()).unwrap_or(0),
        health,
        speaker_method: if input.speakers.is_empty() {
            SpeakerMethod::None
        } else if input.diarized {
            SpeakerMethod::Diarization
        } else {
            SpeakerMethod::Channel
        },
    }
}

fn build_health(
    segments: &[TranscriptSegment],
    withheld_on_read: BTreeMap<String, usize>,
    withheld_word_count: usize,
) -> TranscriptHealth {
    let mut health = TranscriptHealth {
        chunk_count: segments.len(),
        withheld_on_read,
        withheld_word_count,
        ..TranscriptHealth::default()
    };

    for segment in segments {
        match segment.status {
            TranscriptSegmentStatus::Success => health.decoded_chunk_count += 1,
            TranscriptSegmentStatus::Empty => health.empty_chunk_count += 1,
            TranscriptSegmentStatus::Failed => health.failed_chunk_count += 1,
            TranscriptSegmentStatus::Rejected => {
                health.rejected_chunk_count += 1;
                health.rejected_seconds += (segment.end_time_s - segment.start_time_s).max(0.0);
            }
        }
        if let Some(rejection) = segment.rejection.as_ref() {
            *health
                .rejection_reasons
                .entry(rejection.reason.key().to_string())
                .or_insert(0) += 1;
        }
        if let Some(profile) = segment.speech {
            health.voiced_seconds += profile.voiced_seconds;
        }
    }

    health
}

/// The participant list: everyone with a voice, then everyone only named.
fn build_participants(input: &MetadataInput<'_>) -> Vec<Participant> {
    // Talking time per speaker, from the conversation turns where there is one
    // and from the normalized segments otherwise.
    let mut seconds: BTreeMap<String, f64> = BTreeMap::new();
    let mut turns: BTreeMap<String, usize> = BTreeMap::new();

    match input.conversation {
        Some(conversation) => {
            for turn in &conversation.turns {
                if let Some(id) = turn.speaker_id.as_deref() {
                    *seconds.entry(id.to_string()).or_insert(0.0) +=
                        (turn.end_time_s - turn.start_time_s).max(0.0);
                    *turns.entry(id.to_string()).or_insert(0) += 1;
                }
            }
        }
        None => {
            if let Some(normalized) = input.normalized {
                for segment in &normalized.segments {
                    if let Some(id) = segment.speaker_id.as_deref() {
                        *seconds.entry(id.to_string()).or_insert(0.0) +=
                            (segment.end_time_s - segment.start_time_s).max(0.0);
                        *turns.entry(id.to_string()).or_insert(0) += 1;
                    }
                }
            }
        }
    }

    let total: f64 = seconds.values().sum();
    let mut participants: Vec<Participant> = Vec::new();

    for speaker in input.speakers {
        let spoken = seconds.get(&speaker.id).copied().unwrap_or(0.0);
        // A name the transcript offered, used only where the user has not.
        let inferred = input.names.self_introductions.get(&speaker.id);
        let label = match speaker.display_name.as_deref().filter(|n| !n.trim().is_empty()) {
            Some(name) => name.to_string(),
            None => match inferred {
                Some(candidate) => candidate.name.clone(),
                None => speaker.fallback_label.clone(),
            },
        };
        let is_named = speaker
            .display_name
            .as_deref()
            .is_some_and(|n| !n.trim().is_empty())
            || inferred.is_some();

        participants.push(Participant {
            speaker_id: Some(speaker.id.clone()),
            label,
            is_named,
            is_confirmed: speaker.origin == SpeakerOrigin::Manual,
            origin: if speaker.is_local_user {
                ParticipantOrigin::LocalUser
            } else if inferred.is_some_and(|c| c.evidence == NameEvidence::SelfIntroduction) {
                ParticipantOrigin::SelfIntroduced
            } else if speaker.origin == SpeakerOrigin::Diarization {
                ParticipantOrigin::Diarization
            } else {
                ParticipantOrigin::Channel
            },
            is_local_user: speaker.is_local_user,
            speaking_seconds: spoken,
            turn_count: turns.get(&speaker.id).copied().unwrap_or(0),
            share_of_talk: if total > 0.0 {
                (spoken / total) as f32
            } else {
                0.0
            },
        });
    }

    // Loudest first among those who spoke, so the list reads as the meeting did.
    participants.sort_by(|a, b| {
        b.speaking_seconds
            .partial_cmp(&a.speaking_seconds)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut known: Vec<String> = participants.iter().map(|p| p.label.to_lowercase()).collect();

    // People the user said were there.
    for directive in input.notes.directives_of(DirectiveKind::Participant) {
        let name = directive.value.trim();
        if name.is_empty() || known.contains(&name.to_lowercase()) {
            continue;
        }
        known.push(name.to_lowercase());
        participants.push(Participant {
            speaker_id: None,
            label: name.to_string(),
            is_named: true,
            // The user typed it, so it is as confirmed as a name gets.
            is_confirmed: true,
            origin: ParticipantOrigin::Stated,
            is_local_user: false,
            speaking_seconds: 0.0,
            turn_count: 0,
            share_of_talk: 0.0,
        });
    }

    // People the meeting named but nobody matched to a voice.
    for candidate in &input.names.mentioned {
        let name = candidate.name.trim();
        if name.is_empty() || known.contains(&name.to_lowercase()) {
            continue;
        }
        known.push(name.to_lowercase());
        participants.push(Participant {
            speaker_id: None,
            label: name.to_string(),
            is_named: true,
            is_confirmed: false,
            origin: ParticipantOrigin::Mentioned,
            is_local_user: false,
            speaking_seconds: 0.0,
            turn_count: 0,
            share_of_talk: 0.0,
        });
    }

    participants
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meetings_v2::processing::model::{ConversationTurn, SegmentChannel, SPEAKER_ID_ME};
    use crate::meetings_v2::processing::names::NameCandidate;
    use crate::meetings_v2::transcript_health::{HallucinationReason, SpeechProfile};
    use crate::meetings_v2::transcript_health::TranscriptRejection;
    use crate::meetings_v2::types::{MeetingDirective, MeetingState};

    fn session() -> MeetingSession {
        let mut session = MeetingSession::new("meet_meta".into(), Some("Placement review".into()));
        session.state = MeetingState::Completed;
        session.started_at = Some("2026-09-04T09:39:00+00:00".into());
        session.ended_at = Some("2026-09-04T10:23:43+00:00".into());
        session.duration_seconds = 2683.0;
        session.chunk_count = 90;
        session.word_count = 5661;
        session
    }

    fn speaker(id: &str, label: &str, name: Option<&str>, local: bool, origin: SpeakerOrigin) -> Speaker {
        Speaker {
            id: id.into(),
            display_name: name.map(str::to_string),
            fallback_label: label.into(),
            origin,
            channel: if local {
                SegmentChannel::Mic
            } else {
                SegmentChannel::System
            },
            is_local_user: local,
            segment_count: 4,
        }
    }

    fn turn(id: &str, speaker_id: Option<&str>, start: f64, end: f64) -> ConversationTurn {
        ConversationTurn {
            id: id.into(),
            speaker_id: speaker_id.map(str::to_string),
            start_time_s: start,
            end_time_s: end,
            text: "words".into(),
            segment_ids: vec![],
        }
    }

    fn raw(chunk: usize, status: TranscriptSegmentStatus) -> TranscriptSegment {
        TranscriptSegment {
            chunk_index: chunk,
            start_time_s: chunk as f64 * 30.0,
            end_time_s: (chunk + 1) as f64 * 30.0,
            text: if status == TranscriptSegmentStatus::Success {
                "some words".into()
            } else {
                String::new()
            },
            created_at: "2026-09-04T09:39:00Z".into(),
            status,
            mic_had_audio: true,
            sys_had_audio: false,
            utterances: Vec::new(),
            speech: Some(SpeechProfile {
                voiced_seconds: if status == TranscriptSegmentStatus::Success {
                    22.0
                } else {
                    0.0
                },
                total_seconds: 30.0,
                peak_amplitude: 0.5,
                rms: 0.06,
                noise_floor_rms: 0.002,
            }),
            rejection: (status == TranscriptSegmentStatus::Rejected).then(|| {
                TranscriptRejection {
                    reason: HallucinationReason::RepetitionLoop {
                        phrase: "thank you".into(),
                        repeats: 73,
                    },
                    discarded_text: "Thank you. Thank you.".into(),
                    truncated: true,
                    discarded_word_count: 146,
                }
            }),
        }
    }

    fn input<'a>(
        session: &'a MeetingSession,
        raws: &'a [TranscriptSegment],
        conversation: &'a Conversation,
        speakers: &'a [Speaker],
        names: &'a NameFindings,
        notes: &'a MeetingNotes,
        diarized: bool,
    ) -> MetadataInput<'a> {
        MetadataInput {
            session,
            raw_segments: raws,
            normalized: None,
            conversation: Some(conversation),
            speakers,
            names,
            notes,
            diarized,
            withheld_on_read: BTreeMap::new(),
            withheld_word_count: 0,
        }
    }

    #[test]
    fn the_header_carries_what_the_screenshot_was_missing() {
        // The reported header showed date, duration, chunks and words, and
        // nothing about who was there.
        let session = session();
        let raws: Vec<TranscriptSegment> = (0..3)
            .map(|i| raw(i, TranscriptSegmentStatus::Success))
            .collect();
        let conversation = Conversation {
            turns: vec![
                turn("t0", Some(SPEAKER_ID_ME), 0.0, 400.0),
                turn("t1", Some("speaker_1"), 400.0, 900.0),
                turn("t2", Some("speaker_2"), 900.0, 1100.0),
            ],
            unattributed_turn_count: 0,
        };
        let speakers = vec![
            speaker(SPEAKER_ID_ME, "Me", None, true, SpeakerOrigin::Channel),
            speaker("speaker_1", "Speaker 1", Some("Pranjali"), false, SpeakerOrigin::Manual),
            speaker("speaker_2", "Speaker 2", None, false, SpeakerOrigin::Diarization),
        ];
        let names = NameFindings::default();
        let notes = MeetingNotes::default();

        let metadata = build(input(
            &session,
            &raws,
            &conversation,
            &speakers,
            &names,
            &notes,
            true,
        ));

        assert_eq!(metadata.title, "Placement review");
        assert_eq!(metadata.date_iso, "2026-09-04");
        assert_eq!(metadata.duration_seconds, 2683.0);
        assert_eq!(metadata.speaking_participant_count, 3);
        assert_eq!(metadata.turn_count, 3);
        assert_eq!(metadata.speaker_method, SpeakerMethod::Diarization);

        let markdown = metadata.to_markdown();
        assert!(markdown.contains("# Placement review"));
        assert!(markdown.contains("2026-09-04"));
        assert!(markdown.contains("44:43"), "{markdown}");
        assert!(markdown.contains("Participants (3)"));
        assert!(markdown.contains("Pranjali"));
        assert!(markdown.contains("Speaker 2 (unnamed)"), "{markdown}");
    }

    #[test]
    fn talking_time_is_measured_and_shares_add_up() {
        let session = session();
        let conversation = Conversation {
            turns: vec![
                turn("t0", Some(SPEAKER_ID_ME), 0.0, 300.0),
                turn("t1", Some("speaker_1"), 300.0, 400.0),
                turn("t2", None, 400.0, 500.0),
            ],
            unattributed_turn_count: 1,
        };
        let speakers = vec![
            speaker(SPEAKER_ID_ME, "Me", None, true, SpeakerOrigin::Channel),
            speaker("speaker_1", "Speaker 1", None, false, SpeakerOrigin::Channel),
        ];
        let names = NameFindings::default();
        let notes = MeetingNotes::default();
        let metadata = build(input(
            &session,
            &[],
            &conversation,
            &speakers,
            &names,
            &notes,
            false,
        ));

        assert_eq!(metadata.participants[0].label, "Me");
        assert_eq!(metadata.participants[0].speaking_seconds, 300.0);
        assert!((metadata.participants[0].share_of_talk - 0.75).abs() < 1e-5);
        assert!((metadata.participants[1].share_of_talk - 0.25).abs() < 1e-5);
        // The unattributed turn belongs to nobody and inflates nobody's share.
        let total: f32 = metadata.participants.iter().map(|p| p.share_of_talk).sum();
        assert!((total - 1.0).abs() < 1e-4, "shares summed to {total}");
    }

    #[test]
    fn a_name_the_meeting_offered_labels_an_unnamed_speaker_without_confirming_it() {
        let session = session();
        let conversation = Conversation {
            turns: vec![turn("t0", Some("speaker_1"), 0.0, 60.0)],
            unattributed_turn_count: 0,
        };
        let speakers = vec![speaker(
            "speaker_1",
            "Speaker 1",
            None,
            false,
            SpeakerOrigin::Diarization,
        )];
        let mut names = NameFindings::default();
        names.self_introductions.insert(
            "speaker_1".into(),
            NameCandidate {
                name: "Pranjali".into(),
                evidence: NameEvidence::SelfIntroduction,
                speaker_id: Some("speaker_1".into()),
                source_segment_ids: vec!["seg_00000_000".into()],
                mentions: 1,
            },
        );
        let notes = MeetingNotes::default();

        let metadata = build(input(
            &session,
            &[],
            &conversation,
            &speakers,
            &names,
            &notes,
            true,
        ));

        let p = &metadata.participants[0];
        assert_eq!(p.label, "Pranjali");
        assert!(p.is_named);
        assert!(
            !p.is_confirmed,
            "an inferred name must be visibly unconfirmed"
        );
        assert_eq!(p.origin, ParticipantOrigin::SelfIntroduced);
    }

    #[test]
    fn a_user_supplied_name_beats_one_the_transcript_offered() {
        let session = session();
        let conversation = Conversation {
            turns: vec![turn("t0", Some("speaker_1"), 0.0, 60.0)],
            unattributed_turn_count: 0,
        };
        let speakers = vec![speaker(
            "speaker_1",
            "Speaker 1",
            Some("Pranjali Sharma"),
            false,
            SpeakerOrigin::Manual,
        )];
        let mut names = NameFindings::default();
        names.self_introductions.insert(
            "speaker_1".into(),
            NameCandidate {
                name: "Pranj".into(),
                evidence: NameEvidence::SelfIntroduction,
                speaker_id: Some("speaker_1".into()),
                source_segment_ids: vec![],
                mentions: 1,
            },
        );
        let notes = MeetingNotes::default();
        let metadata = build(input(
            &session,
            &[],
            &conversation,
            &speakers,
            &names,
            &notes,
            true,
        ));

        assert_eq!(metadata.participants[0].label, "Pranjali Sharma");
        assert!(metadata.participants[0].is_confirmed);
    }

    #[test]
    fn people_who_were_named_but_never_heard_are_still_participants() {
        let session = session();
        let conversation = Conversation {
            turns: vec![turn("t0", Some(SPEAKER_ID_ME), 0.0, 60.0)],
            unattributed_turn_count: 0,
        };
        let speakers = vec![speaker(SPEAKER_ID_ME, "Me", None, true, SpeakerOrigin::Channel)];
        let mut names = NameFindings::default();
        names.mentioned.push(NameCandidate {
            name: "Ayush".into(),
            evidence: NameEvidence::DirectAddress,
            speaker_id: None,
            source_segment_ids: vec!["seg_00002_000".into()],
            mentions: 3,
        });
        let mut notes = MeetingNotes::default();
        notes.directives.push(
            MeetingDirective::new(DirectiveKind::Participant, None, "Rahul").unwrap(),
        );

        let metadata = build(input(
            &session,
            &[],
            &conversation,
            &speakers,
            &names,
            &notes,
            false,
        ));

        let labels: Vec<&str> = metadata
            .participants
            .iter()
            .map(|p| p.label.as_str())
            .collect();
        assert_eq!(labels, vec!["Me", "Rahul", "Ayush"]);
        assert_eq!(
            metadata.speaking_participant_count, 1,
            "someone who was named is not someone who spoke"
        );
        let rahul = &metadata.participants[1];
        assert_eq!(rahul.origin, ParticipantOrigin::Stated);
        assert!(rahul.is_confirmed, "the user typed this one");
        assert_eq!(metadata.participants[2].origin, ParticipantOrigin::Mentioned);
        assert!(!metadata.participants[2].is_confirmed);
    }

    #[test]
    fn a_stated_participant_is_not_listed_twice_when_they_also_spoke() {
        let session = session();
        let conversation = Conversation {
            turns: vec![turn("t0", Some("speaker_1"), 0.0, 60.0)],
            unattributed_turn_count: 0,
        };
        let speakers = vec![speaker(
            "speaker_1",
            "Speaker 1",
            Some("Pranjali"),
            false,
            SpeakerOrigin::Manual,
        )];
        let names = NameFindings::default();
        let mut notes = MeetingNotes::default();
        notes.directives.push(
            MeetingDirective::new(DirectiveKind::Participant, None, "pranjali").unwrap(),
        );

        let metadata = build(input(
            &session,
            &[],
            &conversation,
            &speakers,
            &names,
            &notes,
            true,
        ));
        assert_eq!(metadata.participants.len(), 1);
    }

    #[test]
    fn health_counts_every_outcome_and_explains_the_losses() {
        let session = session();
        let raws = vec![
            raw(0, TranscriptSegmentStatus::Success),
            raw(1, TranscriptSegmentStatus::Empty),
            raw(2, TranscriptSegmentStatus::Rejected),
            raw(3, TranscriptSegmentStatus::Rejected),
            raw(4, TranscriptSegmentStatus::Failed),
        ];
        let conversation = Conversation {
            turns: Vec::new(),
            unattributed_turn_count: 0,
        };
        let names = NameFindings::default();
        let notes = MeetingNotes::default();
        let metadata = build(input(
            &session,
            &raws,
            &conversation,
            &[],
            &names,
            &notes,
            false,
        ));

        let health = &metadata.health;
        assert_eq!(health.chunk_count, 5);
        assert_eq!(health.decoded_chunk_count, 1);
        assert_eq!(health.empty_chunk_count, 1);
        assert_eq!(health.rejected_chunk_count, 2);
        assert_eq!(health.failed_chunk_count, 1);
        assert_eq!(health.rejected_seconds, 60.0);
        assert_eq!(health.rejection_reasons.get("repetition_loop"), Some(&2));
        assert!(health.has_losses());

        let described = health.describe().unwrap();
        assert!(described.contains("1:00"), "{described}");
        assert!(described.contains("failed to transcribe"), "{described}");
        assert!(metadata.to_markdown().contains("Transcript note:"));
    }

    #[test]
    fn a_clean_transcript_says_nothing_about_its_health() {
        let session = session();
        let raws: Vec<TranscriptSegment> = (0..4)
            .map(|i| raw(i, TranscriptSegmentStatus::Success))
            .collect();
        let conversation = Conversation {
            turns: Vec::new(),
            unattributed_turn_count: 0,
        };
        let names = NameFindings::default();
        let notes = MeetingNotes::default();
        let metadata = build(input(
            &session,
            &raws,
            &conversation,
            &[],
            &names,
            &notes,
            false,
        ));

        assert!(!metadata.health.has_losses());
        assert_eq!(metadata.health.describe(), None);
        assert!(!metadata.to_markdown().contains("Transcript note:"));
        assert_eq!(metadata.speaker_method, SpeakerMethod::None);
    }

    #[test]
    fn withheld_legacy_spans_are_reported_as_such() {
        let session = session();
        let conversation = Conversation {
            turns: Vec::new(),
            unattributed_turn_count: 0,
        };
        let names = NameFindings::default();
        let notes = MeetingNotes::default();
        let mut withheld = BTreeMap::new();
        withheld.insert("repetition_loop".to_string(), 9usize);

        let metadata = build(MetadataInput {
            session: &session,
            raw_segments: &[],
            normalized: None,
            conversation: Some(&conversation),
            speakers: &[],
            names: &names,
            notes: &notes,
            diarized: false,
            withheld_on_read: withheld,
            withheld_word_count: 1314,
        });

        let described = metadata.health.describe().unwrap();
        assert!(described.contains("9 spans"), "{described}");
        assert!(described.contains("1314 words"), "{described}");
    }
}
