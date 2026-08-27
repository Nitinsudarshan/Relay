//! Turning a meeting's action items into tasks.
//!
//! An action item is already the right shape for a task — it carries an owner, a
//! deadline, a status, and the transcript segments it was read out of. What it
//! did not have before v2.5 was anywhere to go: the only exit from a meeting was
//! a Scribble, which produces a note rather than a to-do.
//!
//! This module owns the mapping and nothing else. It produces a
//! [`MeetingTaskDraft`], deliberately *not* a `KanbanCard`: the processing
//! pipeline reads the recorder's artifacts and writes `processing.json`, and
//! giving it a dependency on the vault's storage types would make the derived
//! layer responsible for where a task ends up. The command layer converts a
//! draft into a card and saves it.

use super::model::{ActionItem, ActionItemStatus, MeetingFacts, OwnerType, Speaker};
use super::speakers::resolve_label;

/// Longest task title before it is trimmed.
///
/// A commitment that reads as one line on a board is more useful than one that
/// wraps three times; the full text stays on the description.
const MAX_TITLE_CHARS: usize = 90;

/// A task as the meeting describes it, before anything decides where it lives.
#[derive(Debug, Clone, PartialEq)]
pub struct MeetingTaskDraft {
    /// The action item this came from, so the push can be recorded back onto it.
    pub action_item_id: String,
    pub title: String,
    /// The resolved owner name, or `"Unassigned"`. Never a speaker id.
    pub assignee: String,
    /// `"todo"` or `"done"`, mirroring the action item's own status.
    pub status: &'static str,
    pub priority: &'static str,
    pub due_date: Option<String>,
    /// Full text plus where it came from.
    pub description: String,
}

/// The name to put on a task for an action item's owner.
///
/// Mirrors how the summary renders ownership, so a task and the summary it came
/// from never disagree about who owns the work. An owner the speaker registry
/// cannot resolve becomes `Unassigned` rather than a speaker id leaking onto a
/// board.
pub fn owner_display_name(item: &ActionItem, speakers: &[Speaker]) -> String {
    match item.owner_type {
        OwnerType::Me => resolve_label(speakers, Some(super::model::SPEAKER_ID_ME)).to_string(),
        OwnerType::Speaker => match item.owner_speaker_id.as_deref() {
            Some(id) => {
                let label = resolve_label(speakers, Some(id));
                if label == "Unknown speaker" {
                    "Unassigned".to_string()
                } else {
                    label.to_string()
                }
            }
            None => "Unassigned".to_string(),
        },
        OwnerType::External => item
            .owner_label
            .as_deref()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .unwrap_or("Unassigned")
            .to_string(),
        OwnerType::Group => "Group".to_string(),
        OwnerType::Unassigned => "Unassigned".to_string(),
    }
}

/// Trims a commitment to a board-sized title without cutting a word in half.
fn task_title(description: &str) -> String {
    let text = description.trim();
    if text.chars().count() <= MAX_TITLE_CHARS {
        return text.to_string();
    }

    let cut: String = text.chars().take(MAX_TITLE_CHARS).collect();
    match cut.rfind(char::is_whitespace) {
        Some(space) if space > MAX_TITLE_CHARS / 2 => format!("{}…", cut[..space].trim_end()),
        _ => format!("{}…", cut.trim_end()),
    }
}

/// Priority for a task, from what the meeting actually established.
///
/// Only two inputs are trustworthy here: whether a date was spoken, and whether
/// the extractor was confident. Anything more would be inventing urgency the
/// meeting never expressed.
fn task_priority(item: &ActionItem) -> &'static str {
    if item.deadline.is_some() {
        "high"
    } else if item.confidence >= 0.7 {
        "medium"
    } else {
        "low"
    }
}

/// Builds the task description: the full commitment, then its provenance.
fn task_description(item: &ActionItem, meeting_title: &str, meeting_date: Option<&str>) -> String {
    let mut out = item.description.trim().to_string();
    out.push_str("\n\n---\n");
    out.push_str(&format!("From meeting: {}", meeting_title.trim()));
    if let Some(date) = meeting_date.map(str::trim).filter(|d| !d.is_empty()) {
        out.push_str(&format!("\nRecorded: {date}"));
    }
    if !item.source_segment_ids.is_empty() {
        out.push_str(&format!(
            "\nTranscript segments: {}",
            item.source_segment_ids.join(", ")
        ));
    }
    out
}

/// Maps one action item to a task draft.
pub fn draft_from_action_item(
    item: &ActionItem,
    speakers: &[Speaker],
    meeting_title: &str,
    meeting_date: Option<&str>,
) -> MeetingTaskDraft {
    MeetingTaskDraft {
        action_item_id: item.id.clone(),
        title: task_title(&item.description),
        assignee: owner_display_name(item, speakers),
        status: match item.status {
            ActionItemStatus::Done => "done",
            ActionItemStatus::Open => "todo",
        },
        priority: task_priority(item),
        due_date: item.deadline.clone(),
        description: task_description(item, meeting_title, meeting_date),
    }
}

/// Every action item in a meeting that has not already been pushed to a board.
///
/// Skipping the already-pushed ones is what makes "add all to-dos" safe to press
/// twice: a second press adds only what is new, rather than duplicating the
/// board.
pub fn pending_drafts(
    facts: &MeetingFacts,
    speakers: &[Speaker],
    meeting_title: &str,
    meeting_date: Option<&str>,
) -> Vec<MeetingTaskDraft> {
    facts
        .action_items
        .iter()
        .filter(|item| item.kanban_card_id.is_none())
        .map(|item| draft_from_action_item(item, speakers, meeting_title, meeting_date))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meetings_v2::processing::model::{
        SpeakerOrigin, SPEAKER_ID_ME, SPEAKER_ID_REMOTE,
    };
    use crate::meetings_v2::processing::model::{MeetingType, SegmentChannel};

    fn speaker(id: &str, display_name: Option<&str>, fallback: &str) -> Speaker {
        Speaker {
            id: id.to_string(),
            display_name: display_name.map(str::to_string),
            fallback_label: fallback.to_string(),
            channel: SegmentChannel::Mic,
            is_local_user: id == SPEAKER_ID_ME,
            origin: SpeakerOrigin::Channel,
            segment_count: 3,
        }
    }

    fn item(description: &str, owner_type: OwnerType) -> ActionItem {
        ActionItem {
            id: "act_1".to_string(),
            description: description.to_string(),
            owner_type,
            owner_speaker_id: None,
            owner_label: None,
            deadline: None,
            status: ActionItemStatus::Open,
            source_segment_ids: vec!["seg_00002_001".to_string()],
            confidence: 0.8,
            kanban_card_id: None,
        }
    }

    #[test]
    fn a_renamed_speaker_becomes_the_task_assignee() {
        let speakers = vec![
            speaker(SPEAKER_ID_ME, None, "Me"),
            speaker(SPEAKER_ID_REMOTE, Some("Pranjali"), "Speaker 1"),
        ];
        let mut owned = item("Send the revised deck", OwnerType::Speaker);
        owned.owner_speaker_id = Some(SPEAKER_ID_REMOTE.to_string());

        let draft = draft_from_action_item(&owned, &speakers, "Sprint review", None);
        assert_eq!(draft.assignee, "Pranjali");
    }

    #[test]
    fn an_unresolvable_speaker_is_unassigned_rather_than_an_id_on_the_board() {
        let speakers = vec![speaker(SPEAKER_ID_ME, None, "Me")];
        let mut owned = item("Send the revised deck", OwnerType::Speaker);
        owned.owner_speaker_id = Some("speaker_7".to_string());

        let draft = draft_from_action_item(&owned, &speakers, "Sprint review", None);
        assert_eq!(draft.assignee, "Unassigned");
    }

    #[test]
    fn owner_types_map_to_names_a_person_would_recognize() {
        let speakers = vec![speaker(SPEAKER_ID_ME, Some("Nitin"), "Me")];

        let mine = item("Write the migration", OwnerType::Me);
        assert_eq!(owner_display_name(&mine, &speakers), "Nitin");

        let group = item("Write the migration", OwnerType::Group);
        assert_eq!(owner_display_name(&group, &speakers), "Group");

        let none = item("Write the migration", OwnerType::Unassigned);
        assert_eq!(owner_display_name(&none, &speakers), "Unassigned");

        let mut external = item("Write the migration", OwnerType::External);
        external.owner_label = Some("Ravi".to_string());
        assert_eq!(owner_display_name(&external, &speakers), "Ravi");

        let mut blank_external = item("Write the migration", OwnerType::External);
        blank_external.owner_label = Some("   ".to_string());
        assert_eq!(owner_display_name(&blank_external, &speakers), "Unassigned");
    }

    #[test]
    fn a_long_commitment_is_trimmed_on_a_word_boundary_and_kept_in_full_below() {
        let long = "Send the revised onboarding deck to the partnerships team and \
follow up with the finance folks about the invoice that was raised last quarter";
        let draft = draft_from_action_item(&item(long, OwnerType::Me), &[], "Weekly sync", None);

        assert!(draft.title.chars().count() <= MAX_TITLE_CHARS + 1);
        assert!(draft.title.ends_with('…'));
        assert!(!draft.title.contains("  "));
        // Nothing is lost — the description carries the whole commitment.
        assert!(draft.description.starts_with(long));
    }

    #[test]
    fn a_short_commitment_is_used_verbatim() {
        let draft =
            draft_from_action_item(&item("Send the deck", OwnerType::Me), &[], "Sync", None);
        assert_eq!(draft.title, "Send the deck");
        assert!(!draft.title.ends_with('…'));
    }

    #[test]
    fn the_description_records_where_the_task_came_from() {
        let draft = draft_from_action_item(
            &item("Send the deck", OwnerType::Me),
            &[],
            "Sprint review",
            Some("2026-08-27"),
        );
        assert!(draft.description.contains("From meeting: Sprint review"));
        assert!(draft.description.contains("Recorded: 2026-08-27"));
        assert!(draft.description.contains("seg_00002_001"));
    }

    #[test]
    fn a_spoken_deadline_becomes_the_due_date_and_raises_priority() {
        let mut dated = item("Send the deck", OwnerType::Me);
        dated.deadline = Some("2026-08-28".to_string());

        let draft = draft_from_action_item(&dated, &[], "Sync", None);
        assert_eq!(draft.due_date.as_deref(), Some("2026-08-28"));
        assert_eq!(draft.priority, "high");

        // Without a date, priority follows extraction confidence only.
        let undated = draft_from_action_item(&item("Send the deck", OwnerType::Me), &[], "Sync", None);
        assert_eq!(undated.due_date, None);
        assert_eq!(undated.priority, "medium");

        let mut unsure = item("Send the deck", OwnerType::Me);
        unsure.confidence = 0.4;
        assert_eq!(draft_from_action_item(&unsure, &[], "Sync", None).priority, "low");
    }

    #[test]
    fn a_ticked_action_item_arrives_on_the_board_already_done() {
        let mut done = item("Send the deck", OwnerType::Me);
        done.status = ActionItemStatus::Done;
        assert_eq!(draft_from_action_item(&done, &[], "Sync", None).status, "done");
    }

    fn empty_facts() -> MeetingFacts {
        MeetingFacts {
            title: "Sync".to_string(),
            meeting_type: MeetingType::General,
            key_points: Vec::new(),
            topics: Vec::new(),
            decisions: Vec::new(),
            action_items: Vec::new(),
            open_questions: Vec::new(),
            entities: Vec::new(),
            speaker_ids: Vec::new(),
            deterministic: false,
        }
    }

    #[test]
    fn pushing_twice_adds_only_what_is_new() {
        let mut facts = empty_facts();
        let mut already = item("Send the deck", OwnerType::Me);
        already.id = "act_pushed".to_string();
        already.kanban_card_id = Some("card_abc".to_string());
        let mut fresh = item("Book the room", OwnerType::Me);
        fresh.id = "act_fresh".to_string();
        facts.action_items = vec![already, fresh];

        let drafts = pending_drafts(&facts, &[], "Sync", None);
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].action_item_id, "act_fresh");
    }
}
