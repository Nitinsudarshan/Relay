//! Builds the conversation transcript — the readable, speaker-labelled view.
//!
//! This is not the raw transcript and not a second copy of it. It is a
//! projection of the normalized transcript grouped by speaker, and it is
//! deliberately under-formatted: chronological, speaker-labelled, sentence
//! grouped, timestamped at the turn. No headings, no bullets, no editorializing.
//!
//! Speaker *ids* are stored on turns; names are resolved when rendered. That is
//! what makes a rename update this view without regenerating anything.

use super::model::{Conversation, ConversationTurn, NormalizedSegment, Speaker};
use super::speakers::resolve_label;

/// Word count past which a continuous stretch from one speaker is broken into a
/// new turn.
///
/// Attribution is per 30-second chunk, so an uninterrupted speaker produces one
/// turn per *meeting*, not one per thought — twelve minutes of talking arrives
/// as a single wall of text. Breaking it at a segment boundary keeps the same
/// speaker id and invents no speaker change; it only stops the view from being
/// unreadable. Set high enough that ordinary back-and-forth is untouched.
const MAX_TURN_WORDS: usize = 180;

/// Groups normalized segments into speaker turns.
///
/// Consecutive segments sharing a speaker id merge into one turn, including
/// consecutive *unattributed* segments, which merge into a single "Unknown
/// speaker" turn rather than one per 30-second chunk.
///
/// Grouping is deliberately conservative in both directions. It never splits on
/// anything but a real speaker change or [`MAX_TURN_WORDS`], so no turn boundary
/// here implies a speaker change that the data does not support; and it never
/// rewrites, reorders, or merges across a speaker change.
pub fn build_conversation(segments: &[NormalizedSegment]) -> Conversation {
    let mut turns: Vec<ConversationTurn> = Vec::new();

    for segment in segments {
        if segment.text.trim().is_empty() {
            continue;
        }

        let continues_previous = turns.last().is_some_and(|turn| {
            turn.speaker_id == segment.speaker_id
                && turn.text.split_whitespace().count() < MAX_TURN_WORDS
        });

        if continues_previous {
            let turn = turns
                .last_mut()
                .expect("continues_previous implies a last turn");
            turn.text.push(' ');
            turn.text.push_str(segment.text.trim());
            turn.end_time_s = segment.end_time_s;
            turn.segment_ids.push(segment.id.clone());
        } else {
            turns.push(ConversationTurn {
                id: format!("turn_{:05}", turns.len()),
                speaker_id: segment.speaker_id.clone(),
                start_time_s: segment.start_time_s,
                end_time_s: segment.end_time_s,
                text: segment.text.trim().to_string(),
                segment_ids: vec![segment.id.clone()],
            });
        }
    }

    let unattributed_turn_count = turns.iter().filter(|t| t.speaker_id.is_none()).count();

    Conversation {
        turns,
        unattributed_turn_count,
    }
}

/// Renders the conversation as Markdown, resolving speaker names at render time.
///
/// Used for the Scribble export and for the transcript block handed to the
/// extraction stage. The UI renders from the structured turns instead, so a
/// rename is reflected without calling this again.
pub fn render_conversation_markdown(conversation: &Conversation, speakers: &[Speaker]) -> String {
    let mut out = String::new();
    for turn in &conversation.turns {
        let label = resolve_label(speakers, turn.speaker_id.as_deref());
        out.push_str(&format!(
            "**{}** ({}):\n{}\n\n",
            label,
            format_timestamp(turn.start_time_s),
            turn.text
        ));
    }
    out.trim_end().to_string()
}

/// Renders the transcript for a model prompt: speaker label, timestamp, and the
/// segment id, so the model can cite the segment an item came from and
/// provenance survives into the structured facts.
pub fn render_for_extraction(segments: &[NormalizedSegment], speakers: &[Speaker]) -> String {
    let mut out = String::new();
    for segment in segments {
        let label = resolve_label(speakers, segment.speaker_id.as_deref());
        out.push_str(&format!(
            "[{}] {} ({}): {}\n",
            segment.id,
            label,
            format_timestamp(segment.start_time_s),
            segment.text
        ));
    }
    out
}

/// `mm:ss`, or `h:mm:ss` past an hour.
pub fn format_timestamp(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as u64;
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let secs = total % 60;
    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, minutes, secs)
    } else {
        format!("{}:{:02}", minutes, secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meetings_v2::processing::model::{
        SegmentChannel, SpeakerOrigin, SPEAKER_ID_ME, SPEAKER_ID_REMOTE,
    };
    use crate::meetings_v2::processing::normalize::{normalize_transcript, RawSegmentInput};
    use crate::meetings_v2::processing::speakers::{
        attribute_speakers, rename_speaker, SpeakerIdentificationMode,
    };

    fn raw(chunk_index: usize, text: &str, mic: bool, sys: bool) -> RawSegmentInput {
        RawSegmentInput {
            chunk_index,
            start_time_s: chunk_index as f64 * 30.0,
            end_time_s: (chunk_index + 1) as f64 * 30.0,
            text: text.to_string(),
            mic_had_audio: mic,
            sys_had_audio: sys,
        }
    }

    fn attributed() -> (Vec<NormalizedSegment>, Vec<Speaker>) {
        let raws = vec![
            raw(
                0,
                "so yeah um I think we should probably do this tomorrow",
                true,
                false,
            ),
            raw(1, "and I will draft the plan tonight", true, false),
            raw(2, "Agreed I'll take care of it", false, true),
        ];
        let mut segments = normalize_transcript(&raws, &[]).segments;
        let speakers = attribute_speakers(&mut segments, &[], SpeakerIdentificationMode::Automatic);
        (segments, speakers)
    }

    #[test]
    fn consecutive_segments_from_one_speaker_become_one_turn() {
        let (segments, _) = attributed();
        let conversation = build_conversation(&segments);

        assert_eq!(conversation.turns.len(), 2);
        assert_eq!(
            conversation.turns[0].speaker_id.as_deref(),
            Some(SPEAKER_ID_ME)
        );
        assert_eq!(conversation.turns[0].segment_ids.len(), 2);
        assert_eq!(conversation.turns[0].start_time_s, 0.0);
        assert_eq!(conversation.turns[0].end_time_s, 60.0);
        assert_eq!(
            conversation.turns[1].speaker_id.as_deref(),
            Some(SPEAKER_ID_REMOTE)
        );
    }

    #[test]
    fn the_conversation_reads_differently_from_the_raw_transcript() {
        let (segments, speakers) = attributed();
        let conversation = build_conversation(&segments);
        let rendered = render_conversation_markdown(&conversation, &speakers);

        // The raw text had "so yeah um I think..."; the conversation does not.
        assert!(!rendered.contains(" um "));
        assert!(rendered.contains("**Me**"));
        assert!(rendered.contains("**Speaker 1**"));
        assert!(rendered.contains("I think we should probably do this tomorrow."));
    }

    #[test]
    fn renaming_a_speaker_changes_the_conversation_without_touching_the_turns() {
        let (segments, mut speakers) = attributed();
        let conversation = build_conversation(&segments);

        let before = render_conversation_markdown(&conversation, &speakers);
        assert!(before.contains("**Speaker 1**"));

        rename_speaker(&mut speakers, SPEAKER_ID_REMOTE, Some("Pranjali")).unwrap();

        // Same conversation object, new render.
        let after = render_conversation_markdown(&conversation, &speakers);
        assert!(after.contains("**Pranjali**"));
        assert!(!after.contains("**Speaker 1**"));
        assert_eq!(
            conversation.turns[1].speaker_id.as_deref(),
            Some(SPEAKER_ID_REMOTE),
            "the turn still references the id, not the name"
        );
    }

    #[test]
    fn unattributed_turns_are_counted_and_labelled_honestly() {
        let raws = vec![raw(0, "we were both talking here", true, true)];
        let mut segments = normalize_transcript(&raws, &[]).segments;
        let speakers = attribute_speakers(&mut segments, &[], SpeakerIdentificationMode::Automatic);

        let conversation = build_conversation(&segments);
        assert_eq!(conversation.unattributed_turn_count, 1);
        assert!(
            render_conversation_markdown(&conversation, &speakers).contains("**Unknown speaker**")
        );
    }

    #[test]
    fn extraction_rendering_carries_segment_ids_for_provenance() {
        let (segments, speakers) = attributed();
        let rendered = render_for_extraction(&segments, &speakers);
        assert!(rendered.contains("[seg_00000] Me (0:00):"));
        assert!(rendered.contains("[seg_00002] Speaker 1 (1:00):"));
    }

    #[test]
    fn timestamps_cross_the_hour_correctly() {
        assert_eq!(format_timestamp(0.0), "0:00");
        assert_eq!(format_timestamp(65.4), "1:05");
        assert_eq!(format_timestamp(3661.0), "1:01:01");
    }

    #[test]
    fn a_speaker_with_no_registry_entry_does_not_crash_rendering() {
        let segments = vec![NormalizedSegment {
            id: "seg_00000".into(),
            chunk_index: 0,
            start_time_s: 0.0,
            end_time_s: 30.0,
            text: "Orphaned segment.".into(),
            raw_text: "orphaned segment".into(),
            channel: SegmentChannel::Unknown,
            speaker_id: Some("speaker_42".into()),
            applied_rules: Vec::new(),
        }];
        let speakers = vec![Speaker {
            id: SPEAKER_ID_ME.into(),
            display_name: None,
            fallback_label: "Me".into(),
            origin: SpeakerOrigin::Channel,
            channel: SegmentChannel::Mic,
            is_local_user: true,
            segment_count: 0,
        }];
        let conversation = build_conversation(&segments);
        assert!(
            render_conversation_markdown(&conversation, &speakers).contains("**Unknown speaker**")
        );
    }
}

#[cfg(test)]
mod turn_length_tests {
    use super::*;
    use crate::meetings_v2::processing::model::SPEAKER_ID_ME;
    use crate::meetings_v2::processing::normalize::{normalize_transcript, RawSegmentInput};
    use crate::meetings_v2::processing::speakers::{
        attribute_speakers, SpeakerIdentificationMode,
    };

    #[test]
    fn one_speaker_talking_for_a_long_time_does_not_become_one_wall_of_text() {
        let line = "and then we looked at the migration strategy and what it means for the local \
vault and the sync layer in some detail before moving on";
        let raws: Vec<RawSegmentInput> = (0..12)
            .map(|i| RawSegmentInput {
                chunk_index: i,
                start_time_s: i as f64 * 30.0,
                end_time_s: (i + 1) as f64 * 30.0,
                text: line.to_string(),
                mic_had_audio: true,
                sys_had_audio: false,
            })
            .collect();

        let mut segments = normalize_transcript(&raws, &[]).segments;
        let speakers = attribute_speakers(&mut segments, &[], SpeakerIdentificationMode::Automatic);
        let conversation = build_conversation(&segments);

        assert!(
            conversation.turns.len() > 1,
            "twelve minutes of one voice is not one readable turn"
        );
        for turn in &conversation.turns {
            assert_eq!(
                turn.speaker_id.as_deref(),
                Some(SPEAKER_ID_ME),
                "breaking a long turn must not invent a speaker change"
            );
        }
        // Every segment is still accounted for exactly once, in order.
        let ids: Vec<&String> = conversation
            .turns
            .iter()
            .flat_map(|t| t.segment_ids.iter())
            .collect();
        assert_eq!(ids.len(), segments.len());
        assert!(ids.windows(2).all(|w| w[0] < w[1]));

        let rendered = render_conversation_markdown(&conversation, &speakers);
        assert!(!rendered.contains("**Unknown speaker**"));
    }

    #[test]
    fn ordinary_back_and_forth_is_not_split() {
        let raws = vec![
            RawSegmentInput {
                chunk_index: 0,
                start_time_s: 0.0,
                end_time_s: 30.0,
                text: "so I think we should ship on Friday".into(),
                mic_had_audio: true,
                sys_had_audio: false,
            },
            RawSegmentInput {
                chunk_index: 1,
                start_time_s: 30.0,
                end_time_s: 60.0,
                text: "and I will write the changelog tonight".into(),
                mic_had_audio: true,
                sys_had_audio: false,
            },
        ];
        let mut segments = normalize_transcript(&raws, &[]).segments;
        attribute_speakers(&mut segments, &[], SpeakerIdentificationMode::Automatic);
        assert_eq!(build_conversation(&segments).turns.len(), 1);
    }
}
