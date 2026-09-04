//! Assembles a meeting into one document somebody else can read.
//!
//! A generated summary on its own is a wall of claims with no provenance:
//! pasted into a message it does not say which meeting it is about, when it
//! happened, who was in it, or how much of the recording it was written from.
//! This puts the counted header from `metadata` in front of it, and offers the
//! parts a reader may or may not want — to-dos, decisions, the full
//! conversation — as explicit choices rather than a fixed format.
//!
//! Two rules hold here, and both exist because this text leaves the app:
//!
//! * **Nothing is invented.** Every line is either the metadata header, prose
//!   that was already generated and validated, or transcript text. This module
//!   composes; it does not write.
//! * **A degraded meeting says so.** If chunks were rejected, or the roster is
//!   channel-only, or the summary was rendered without a model, the document
//!   states it. A shared summary that hides its own limitations is worse than
//!   no shared summary, because the reader cannot know to ask.

use super::conversation::render_conversation_markdown;
use super::metadata::{format_duration, MeetingMetadata, SpeakerMethod};
use super::model::{
    ActionItemStatus, Conversation, MeetingFacts, OwnerType, Speaker, SummaryArtifact,
};
use super::speakers::resolve_label;
use serde::{Deserialize, Serialize};

/// Which parts of a meeting to include.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareOptions {
    /// The generated prose. Off produces a header-and-to-dos handout.
    pub summary: bool,
    pub action_items: bool,
    pub decisions: bool,
    /// The full speaker-labelled conversation. Long, and off by default: this
    /// is the part that turns a one-page summary into forty pages.
    pub conversation: bool,
    /// The user's own notes. Off by default because notes are often private
    /// working material rather than something meant to be circulated.
    pub notes: bool,
}

impl Default for ShareOptions {
    fn default() -> Self {
        Self {
            summary: true,
            action_items: true,
            decisions: true,
            conversation: false,
            notes: false,
        }
    }
}

/// Everything a shared document is composed from.
pub struct ShareInput<'a> {
    pub metadata: &'a MeetingMetadata,
    pub summary: Option<&'a SummaryArtifact>,
    pub facts: Option<&'a MeetingFacts>,
    pub conversation: Option<&'a Conversation>,
    pub speakers: &'a [Speaker],
    /// Prose the user wrote, already folded into one string by the caller.
    pub notes: &'a str,
}

/// Renders a meeting as a Markdown document.
pub fn render(input: &ShareInput<'_>, options: ShareOptions) -> String {
    let mut out = input.metadata.to_markdown();

    if let Some(caveat) = caveat(input) {
        out.push_str(&format!("> {caveat}\n\n"));
    }

    if options.summary {
        if let Some(summary) = input.summary {
            out.push_str(summary.markdown.trim());
            out.push_str("\n\n");
        }
    }

    if let Some(facts) = input.facts {
        if options.action_items && !facts.action_items.is_empty() {
            out.push_str("## To-dos\n\n");
            for item in &facts.action_items {
                let done = item.status == ActionItemStatus::Done;
                let owner = owner_label(input.speakers, item);
                let deadline = item
                    .deadline
                    .as_deref()
                    .map(|d| format!(" — due {d}"))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "- [{}] {}{}{}\n",
                    if done { "x" } else { " " },
                    item.description.trim(),
                    owner,
                    deadline
                ));
            }
            out.push('\n');
        }

        if options.decisions && !facts.decisions.is_empty() {
            out.push_str("## Decisions\n\n");
            for decision in &facts.decisions {
                out.push_str(&format!("- {}", decision.statement.trim()));
                // The rationale is the half of a decision that is worth
                // anything six weeks later, so it is never dropped when present.
                if let Some(reason) = decision.rationale.as_deref().map(str::trim) {
                    if !reason.is_empty() {
                        out.push_str(&format!(" — {reason}"));
                    }
                }
                out.push('\n');
            }
            out.push('\n');
        }
    }

    if options.notes && !input.notes.trim().is_empty() {
        out.push_str("## Notes\n\n");
        out.push_str(input.notes.trim());
        out.push_str("\n\n");
    }

    if options.conversation {
        if let Some(conversation) = input.conversation.filter(|c| !c.turns.is_empty()) {
            out.push_str("## Conversation\n\n");
            out.push_str(&render_conversation_markdown(conversation, input.speakers));
            out.push_str("\n\n");
        }
    }

    out.trim_end().to_string()
}

/// The one line a reader needs in order to know how far to trust the rest.
///
/// Returns `None` for a meeting with nothing to disclose. Present caveats are
/// joined into a single sentence rather than a list, because a stack of
/// warnings above a summary reads as boilerplate and gets skipped.
fn caveat(input: &ShareInput<'_>) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();

    if let Some(note) = input.metadata.health.describe() {
        parts.push(note);
    }
    if input.metadata.speaker_method == SpeakerMethod::Channel
        && input.metadata.speaking_participant_count > 1
    {
        parts.push(
            "speakers were told apart by capture channel only, so everyone other than the \
recording user shares one label".to_string(),
        );
    }
    if input.summary.is_some_and(|s| s.deterministic) {
        parts.push(
            "this summary was assembled from the transcript without a language model".to_string(),
        );
    }
    if let Some(conversation) = input.conversation {
        if conversation.unattributed_turn_count > 0 {
            parts.push(format!(
                "{} stretches could not be attributed to anyone",
                conversation.unattributed_turn_count
            ));
        }
    }

    if parts.is_empty() {
        return None;
    }
    Some(format!("{}.", parts.join("; ")))
}

/// The owner suffix on a to-do line, or nothing.
fn owner_label(
    speakers: &[Speaker],
    item: &super::model::ActionItem,
) -> String {
    match item.owner_type {
        OwnerType::Me | OwnerType::Speaker => match item.owner_speaker_id.as_deref() {
            Some(id) => format!(" ({})", resolve_label(speakers, Some(id))),
            None => String::new(),
        },
        OwnerType::External => item
            .owner_label
            .as_deref()
            .map(|name| format!(" ({name})"))
            .unwrap_or_default(),
        OwnerType::Group => " (the group)".to_string(),
        OwnerType::Unassigned => String::new(),
    }
}

/// A filename for a saved copy: `2026-09-04-placement-review.md`.
pub fn suggested_filename(metadata: &MeetingMetadata) -> String {
    let slug: String = metadata
        .title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let slug = slug
        .split('-')
        .filter(|p| !p.is_empty())
        .take(8)
        .collect::<Vec<_>>()
        .join("-");
    let date = if metadata.date_iso.is_empty() {
        "meeting".to_string()
    } else {
        metadata.date_iso.clone()
    };
    if slug.is_empty() {
        format!("{date}-meeting.md")
    } else {
        format!("{date}-{slug}.md")
    }
}

/// A one-line description of what a share will contain, for the button's label.
pub fn describe(options: ShareOptions) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if options.summary {
        parts.push("summary");
    }
    if options.action_items {
        parts.push("to-dos");
    }
    if options.decisions {
        parts.push("decisions");
    }
    if options.notes {
        parts.push("your notes");
    }
    if options.conversation {
        parts.push("full conversation");
    }
    if parts.is_empty() {
        return "header only".to_string();
    }
    parts.join(", ")
}

/// Rough duration of a conversation export, for warning about its size.
pub fn conversation_length_hint(conversation: Option<&Conversation>) -> Option<String> {
    let conversation = conversation?;
    if conversation.turns.is_empty() {
        return None;
    }
    let words: usize = conversation
        .turns
        .iter()
        .map(|t| t.text.split_whitespace().count())
        .sum();
    let span = conversation
        .turns
        .last()
        .map(|t| t.end_time_s)
        .unwrap_or(0.0);
    Some(format!(
        "{} turns, {} words, {}",
        conversation.turns.len(),
        words,
        format_duration(span)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::metadata::{Participant, ParticipantOrigin, TranscriptHealth};
    use super::super::model::{
        ActionItem, ConversationTurn, Decision, MeetingType, SegmentChannel, SpeakerOrigin,
        SummaryMode, SummarySource, ProviderOutputStatus, SPEAKER_ID_ME,
    };

    fn metadata(health: TranscriptHealth, method: SpeakerMethod) -> MeetingMetadata {
        MeetingMetadata {
            title: "Placement review".into(),
            date_iso: "2026-09-04".into(),
            started_at: Some("2026-09-04T09:39:00+00:00".into()),
            ended_at: Some("2026-09-04T10:23:43+00:00".into()),
            duration_seconds: 2683.0,
            paused_seconds: 0.0,
            speaking_participant_count: 2,
            participants: vec![
                Participant {
                    speaker_id: Some(SPEAKER_ID_ME.into()),
                    label: "Nitin".into(),
                    is_named: true,
                    is_confirmed: true,
                    origin: ParticipantOrigin::LocalUser,
                    is_local_user: true,
                    speaking_seconds: 900.0,
                    turn_count: 12,
                    share_of_talk: 0.6,
                },
                Participant {
                    speaker_id: Some("speaker_1".into()),
                    label: "Pranjali".into(),
                    is_named: true,
                    is_confirmed: true,
                    origin: ParticipantOrigin::Diarization,
                    is_local_user: false,
                    speaking_seconds: 600.0,
                    turn_count: 9,
                    share_of_talk: 0.4,
                },
            ],
            chunk_count: 90,
            word_count: 5661,
            turn_count: 21,
            health,
            speaker_method: method,
        }
    }

    fn summary(deterministic: bool) -> SummaryArtifact {
        SummaryArtifact {
            markdown: "## Overview\n\nPlacements closed at forty-one this month.".into(),
            mode: SummaryMode::Concise,
            extension_id: "default".into(),
            generated_at: "2026-09-04T10:30:00Z".into(),
            provider: "ollama".into(),
            model: "llama3.2:latest".into(),
            processing_version: 4,
            rules_version: "test".into(),
            deterministic,
            source: SummarySource::default(),
            repair_attempted: false,
            length_budget_words: Some(300),
            provider_output_status: ProviderOutputStatus::default(),
            fallback_used: deterministic,
            rejected_issues: Vec::new(),
            speaker_names_stale: false,
            validation: Default::default(),
        }
    }

    fn facts() -> MeetingFacts {
        MeetingFacts {
            title: "Placement review".into(),
            meeting_type: MeetingType::ProjectReview,
            key_points: Vec::new(),
            topics: Vec::new(),
            open_questions: Vec::new(),
            risks: Vec::new(),
            entities: Vec::new(),
            speaker_ids: Vec::new(),
            deterministic: false,
            action_items: vec![
                ActionItem {
                    id: "act_1".into(),
                    description: "Send the cohort breakdown".into(),
                    owner_type: OwnerType::Speaker,
                    owner_speaker_id: Some("speaker_1".into()),
                    owner_label: None,
                    deadline: Some("2026-09-11".into()),
                    status: ActionItemStatus::Open,
                    source_segment_ids: vec!["seg_00003_000".into()],
                    confidence: 0.9,
                    kanban_card_id: None,
                },
                ActionItem {
                    id: "act_2".into(),
                    description: "Book the review room".into(),
                    owner_type: OwnerType::Group,
                    owner_speaker_id: None,
                    owner_label: None,
                    deadline: None,
                    status: ActionItemStatus::Done,
                    source_segment_ids: vec![],
                    confidence: 0.7,
                    kanban_card_id: None,
                },
            ],
            decisions: vec![Decision {
                id: "dec_1".into(),
                statement: "Move the launch to Monday".into(),
                rationale: Some("the payment integration still has blocking bugs".into()),
                decided_by_speaker_id: Some(SPEAKER_ID_ME.into()),
                source_segment_ids: vec![],
                confidence: 0.9,
            }],
        }
    }

    fn speakers() -> Vec<Speaker> {
        vec![
            Speaker {
                id: SPEAKER_ID_ME.into(),
                display_name: Some("Nitin".into()),
                fallback_label: "Me".into(),
                origin: SpeakerOrigin::Manual,
                channel: SegmentChannel::Mic,
                is_local_user: true,
                segment_count: 12,
            },
            Speaker {
                id: "speaker_1".into(),
                display_name: Some("Pranjali".into()),
                fallback_label: "Speaker 1".into(),
                origin: SpeakerOrigin::Manual,
                channel: SegmentChannel::System,
                is_local_user: false,
                segment_count: 9,
            },
        ]
    }

    fn conversation() -> Conversation {
        Conversation {
            turns: vec![
                ConversationTurn {
                    id: "turn_00000".into(),
                    speaker_id: Some(SPEAKER_ID_ME.into()),
                    start_time_s: 0.0,
                    end_time_s: 20.0,
                    text: "Shall we start with the placement numbers?".into(),
                    segment_ids: vec!["seg_00000_000".into()],
                },
                ConversationTurn {
                    id: "turn_00001".into(),
                    speaker_id: Some("speaker_1".into()),
                    start_time_s: 20.0,
                    end_time_s: 45.0,
                    text: "We closed forty-one this month.".into(),
                    segment_ids: vec!["seg_00000_001".into()],
                },
            ],
            unattributed_turn_count: 0,
        }
    }

    fn input<'a>(
        metadata: &'a MeetingMetadata,
        summary: Option<&'a SummaryArtifact>,
        facts: Option<&'a MeetingFacts>,
        conversation: Option<&'a Conversation>,
        speakers: &'a [Speaker],
        notes: &'a str,
    ) -> ShareInput<'a> {
        ShareInput {
            metadata,
            summary,
            facts,
            conversation,
            speakers,
            notes,
        }
    }

    #[test]
    fn a_shared_summary_carries_its_own_provenance() {
        let meta = metadata(TranscriptHealth::default(), SpeakerMethod::Diarization);
        let summary = summary(false);
        let facts = facts();
        let speakers = speakers();
        let doc = render(
            &input(&meta, Some(&summary), Some(&facts), None, &speakers, ""),
            ShareOptions::default(),
        );

        assert!(doc.starts_with("# Placement review"));
        assert!(doc.contains("2026-09-04"));
        assert!(doc.contains("44m 43s"));
        assert!(doc.contains("Participants (2)"));
        assert!(doc.contains("Nitin"));
        assert!(doc.contains("Pranjali"));
        assert!(doc.contains("Placements closed at forty-one"));
    }

    #[test]
    fn to_dos_carry_their_owner_and_deadline() {
        let meta = metadata(TranscriptHealth::default(), SpeakerMethod::Diarization);
        let facts = facts();
        let speakers = speakers();
        let doc = render(
            &input(&meta, None, Some(&facts), None, &speakers, ""),
            ShareOptions::default(),
        );

        assert!(doc.contains("- [ ] Send the cohort breakdown (Pranjali) — due 2026-09-11"));
        assert!(doc.contains("- [x] Book the review room (the group)"));
    }

    #[test]
    fn a_decisions_rationale_is_never_dropped() {
        // The reason, not the date, is what somebody needs six weeks later.
        let meta = metadata(TranscriptHealth::default(), SpeakerMethod::Diarization);
        let facts = facts();
        let speakers = speakers();
        let doc = render(
            &input(&meta, None, Some(&facts), None, &speakers, ""),
            ShareOptions::default(),
        );
        assert!(doc.contains("Move the launch to Monday — the payment integration still has blocking bugs"));
    }

    #[test]
    fn a_degraded_meeting_discloses_it_in_the_document() {
        let health = TranscriptHealth {
            chunk_count: 90,
            decoded_chunk_count: 81,
            rejected_chunk_count: 9,
            rejected_seconds: 270.0,
            ..TranscriptHealth::default()
        };
        let meta = metadata(health, SpeakerMethod::Channel);
        let summary = summary(true);
        let speakers = speakers();
        let doc = render(
            &input(&meta, Some(&summary), None, None, &speakers, ""),
            ShareOptions::default(),
        );

        assert!(doc.contains("no usable speech"), "{doc}");
        assert!(doc.contains("capture channel only"), "{doc}");
        assert!(doc.contains("without a language model"), "{doc}");
    }

    #[test]
    fn a_clean_meeting_carries_no_caveat() {
        let meta = metadata(TranscriptHealth::default(), SpeakerMethod::Diarization);
        let summary = summary(false);
        let speakers = speakers();
        let doc = render(
            &input(&meta, Some(&summary), None, None, &speakers, ""),
            ShareOptions::default(),
        );
        assert!(!doc.contains("capture channel only"));
        assert!(!doc.contains("without a language model"));
    }

    #[test]
    fn the_conversation_is_included_only_when_asked_for() {
        let meta = metadata(TranscriptHealth::default(), SpeakerMethod::Diarization);
        let conversation = conversation();
        let speakers = speakers();

        let without = render(
            &input(&meta, None, None, Some(&conversation), &speakers, ""),
            ShareOptions::default(),
        );
        assert!(!without.contains("## Conversation"));

        let with = render(
            &input(&meta, None, None, Some(&conversation), &speakers, ""),
            ShareOptions {
                conversation: true,
                ..ShareOptions::default()
            },
        );
        assert!(with.contains("## Conversation"));
        assert!(with.contains("**Pranjali**"));
        assert!(with.contains("We closed forty-one this month."));
    }

    #[test]
    fn private_notes_are_left_out_unless_the_user_includes_them() {
        let meta = metadata(TranscriptHealth::default(), SpeakerMethod::Diarization);
        let speakers = speakers();
        let notes = "ask about the funding gap before committing";

        let without = render(
            &input(&meta, None, None, None, &speakers, notes),
            ShareOptions::default(),
        );
        assert!(
            !without.contains("funding gap"),
            "notes are working material and must not leave by default"
        );

        let with = render(
            &input(&meta, None, None, None, &speakers, notes),
            ShareOptions {
                notes: true,
                ..ShareOptions::default()
            },
        );
        assert!(with.contains("## Notes"));
        assert!(with.contains("funding gap"));
    }

    #[test]
    fn a_meeting_with_nothing_derived_still_produces_a_readable_header() {
        let meta = metadata(TranscriptHealth::default(), SpeakerMethod::None);
        let doc = render(
            &input(&meta, None, None, None, &[], ""),
            ShareOptions::default(),
        );
        assert!(doc.starts_with("# Placement review"));
        assert!(!doc.contains("## To-dos"));
        assert!(!doc.ends_with('\n'), "trailing whitespace is not content");
    }

    #[test]
    fn unattributed_stretches_are_disclosed() {
        let meta = metadata(TranscriptHealth::default(), SpeakerMethod::Diarization);
        let mut conversation = conversation();
        conversation.unattributed_turn_count = 3;
        let speakers = speakers();
        let doc = render(
            &input(&meta, None, None, Some(&conversation), &speakers, ""),
            ShareOptions::default(),
        );
        assert!(doc.contains("3 stretches could not be attributed"), "{doc}");
    }

    #[test]
    fn the_filename_is_dated_and_slugged() {
        let meta = metadata(TranscriptHealth::default(), SpeakerMethod::Diarization);
        assert_eq!(suggested_filename(&meta), "2026-09-04-placement-review.md");

        let mut untitled = metadata(TranscriptHealth::default(), SpeakerMethod::None);
        untitled.title = "!!!".into();
        assert_eq!(suggested_filename(&untitled), "2026-09-04-meeting.md");

        let mut undated = metadata(TranscriptHealth::default(), SpeakerMethod::None);
        undated.date_iso = String::new();
        assert_eq!(suggested_filename(&undated), "meeting-placement-review.md");
    }

    #[test]
    fn the_share_description_lists_what_is_going_out() {
        assert_eq!(
            describe(ShareOptions::default()),
            "summary, to-dos, decisions"
        );
        assert_eq!(
            describe(ShareOptions {
                summary: false,
                action_items: false,
                decisions: false,
                conversation: false,
                notes: false,
            }),
            "header only"
        );
        assert!(describe(ShareOptions {
            conversation: true,
            notes: true,
            ..ShareOptions::default()
        })
        .contains("full conversation"));
    }

    #[test]
    fn the_conversation_hint_says_how_big_the_export_would_be() {
        let conversation = conversation();
        let hint = conversation_length_hint(Some(&conversation)).unwrap();
        assert!(hint.contains("2 turns"), "{hint}");
        assert!(hint.contains("words"), "{hint}");
        assert_eq!(conversation_length_hint(None), None);
        assert_eq!(
            conversation_length_hint(Some(&Conversation {
                turns: Vec::new(),
                unattributed_turn_count: 0
            })),
            None
        );
    }

}
