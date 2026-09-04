//! Applies the typed instructions a user attached to a meeting.
//!
//! A directive exists because the user knows something the recording does not,
//! and each kind is read by the stage that can act on it rather than by a
//! model. `Term` directives join the normalization glossary; `Participant` and
//! `Agenda` are read where the participant list and the model context are
//! assembled. This module handles the one that has to reach into the speaker
//! registry: `SpeakerName`.
//!
//! The reason a name correction is not simply prose in the notes box: a
//! sentence like "the recogniser heard my name as Nithin" only works if a model
//! notices it and chooses to act on it, and a summary is not where a name lives
//! anyway — the registry is. A directive renames the speaker, so every derived
//! view picks the name up at read time and no prose has to be regenerated.

use super::model::{Speaker, SpeakerOrigin};
use crate::meetings_v2::types::{DirectiveKind, MeetingDirective, MeetingNotes};
use serde::{Deserialize, Serialize};

/// A directive that could not be applied, and why.
///
/// Reported rather than swallowed: a user who typed "Speaker 4 is Ayush" into a
/// meeting with three speakers needs to be told, or they will assume the
/// correction took and that Relay ignored it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnresolvedDirective {
    pub directive_id: String,
    pub kind: DirectiveKind,
    /// What the directive said, in one line.
    pub summary: String,
    pub reason: String,
}

/// Applies every `SpeakerName` directive to the roster.
///
/// Matching accepts a speaker id (`speaker_2`), a fallback label
/// (`Speaker 2`), or a name already assigned — the same three forms the rest of
/// the pipeline accepts for an owner, so a user can write whichever they can
/// see in the UI. Anything else is returned unresolved.
pub fn apply_speaker_names(
    speakers: &mut [Speaker],
    notes: &MeetingNotes,
) -> Vec<UnresolvedDirective> {
    let mut unresolved = Vec::new();

    for directive in notes.directives_of(DirectiveKind::SpeakerName) {
        let Some(subject) = directive.subject.as_deref().map(str::trim).filter(|s| !s.is_empty())
        else {
            unresolved.push(unresolved_for(
                directive,
                "this correction does not say which speaker it is about",
            ));
            continue;
        };

        match find_speaker(speakers, subject) {
            Some(index) => {
                speakers[index].display_name = Some(directive.value.trim().to_string());
                // The user said so, which outranks any rung that found them.
                speakers[index].origin = SpeakerOrigin::Manual;
            }
            None => unresolved.push(unresolved_for(
                directive,
                &format!("there is no \"{subject}\" in this meeting"),
            )),
        }
    }

    unresolved
}

fn unresolved_for(directive: &MeetingDirective, reason: &str) -> UnresolvedDirective {
    UnresolvedDirective {
        directive_id: directive.id.clone(),
        kind: directive.kind,
        summary: directive.describe(),
        reason: reason.to_string(),
    }
}

/// Index of the speaker a subject string refers to.
fn find_speaker(speakers: &[Speaker], subject: &str) -> Option<usize> {
    let needle = subject.trim().to_lowercase();
    speakers.iter().position(|s| {
        s.id.to_lowercase() == needle
            || s.fallback_label.to_lowercase() == needle
            || s.display_name
                .as_deref()
                .is_some_and(|n| n.trim().to_lowercase() == needle)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meetings_v2::processing::model::{SegmentChannel, SPEAKER_ID_ME};

    fn speaker(id: &str, label: &str, name: Option<&str>) -> Speaker {
        Speaker {
            id: id.into(),
            display_name: name.map(str::to_string),
            fallback_label: label.into(),
            origin: SpeakerOrigin::Diarization,
            channel: SegmentChannel::System,
            is_local_user: false,
            segment_count: 3,
        }
    }

    fn notes(entries: &[(&str, &str)]) -> MeetingNotes {
        let mut notes = MeetingNotes::default();
        for (subject, value) in entries {
            notes.directives.push(
                MeetingDirective::new(DirectiveKind::SpeakerName, Some(subject), value).unwrap(),
            );
        }
        notes
    }

    #[test]
    fn a_correction_can_name_a_speaker_by_label_id_or_existing_name() {
        let mut roster = vec![
            speaker("speaker_1", "Speaker 1", None),
            speaker("speaker_2", "Speaker 2", None),
            speaker("speaker_3", "Speaker 3", Some("Nithin")),
        ];
        let notes = notes(&[
            ("Speaker 1", "Pranjali"),
            ("speaker_2", "Ayush"),
            ("Nithin", "Nitin"),
        ]);

        assert!(apply_speaker_names(&mut roster, &notes).is_empty());
        assert_eq!(roster[0].label(), "Pranjali");
        assert_eq!(roster[1].label(), "Ayush");
        assert_eq!(roster[2].label(), "Nitin");
    }

    #[test]
    fn a_correction_marks_the_speaker_as_named_by_a_person() {
        let mut roster = vec![speaker("speaker_1", "Speaker 1", None)];
        apply_speaker_names(&mut roster, &notes(&[("Speaker 1", "Pranjali")]));
        assert_eq!(
            roster[0].origin,
            SpeakerOrigin::Manual,
            "a name a person typed is confirmed, not inferred"
        );
    }

    #[test]
    fn matching_ignores_case_and_surrounding_space() {
        let mut roster = vec![speaker(SPEAKER_ID_ME, "Me", None)];
        apply_speaker_names(&mut roster, &notes(&[("  me  ", "Nitin")]));
        assert_eq!(roster[0].label(), "Nitin");
    }

    #[test]
    fn a_correction_for_a_speaker_who_is_not_here_is_reported_not_swallowed() {
        // The user must be told, or they will assume it worked.
        let mut roster = vec![speaker("speaker_1", "Speaker 1", None)];
        let unresolved = apply_speaker_names(&mut roster, &notes(&[("Speaker 4", "Ayush")]));

        assert_eq!(unresolved.len(), 1);
        assert!(unresolved[0].reason.contains("Speaker 4"));
        assert_eq!(unresolved[0].summary, "Speaker 4 is Ayush");
        assert_eq!(unresolved[0].kind, DirectiveKind::SpeakerName);
        assert_eq!(roster[0].display_name, None);
    }

    #[test]
    fn later_corrections_win_over_earlier_ones() {
        // The user changed their mind; the last thing they typed is what they
        // meant.
        let mut roster = vec![speaker("speaker_1", "Speaker 1", None)];
        apply_speaker_names(
            &mut roster,
            &notes(&[("Speaker 1", "Pranjali"), ("Speaker 1", "Pranjali Sharma")]),
        );
        assert_eq!(roster[0].label(), "Pranjali Sharma");
    }

    #[test]
    fn other_kinds_of_directive_are_left_to_the_stages_that_read_them() {
        let mut roster = vec![speaker("speaker_1", "Speaker 1", None)];
        let mut notes = MeetingNotes::default();
        notes.directives.push(
            MeetingDirective::new(DirectiveKind::Term, Some("Lance TV"), "LanceDB").unwrap(),
        );
        notes
            .directives
            .push(MeetingDirective::new(DirectiveKind::Participant, None, "Rahul").unwrap());
        notes
            .directives
            .push(MeetingDirective::new(DirectiveKind::Note, None, "the vault rewrite is blocked").unwrap());

        assert!(apply_speaker_names(&mut roster, &notes).is_empty());
        assert_eq!(roster[0].display_name, None);
        assert_eq!(notes.glossary_terms(), vec!["LanceDB"]);
        assert!(notes.during_for_model().contains("vault rewrite"));
    }

    #[test]
    fn an_empty_roster_reports_every_correction_as_unresolved() {
        let mut roster: Vec<Speaker> = Vec::new();
        let unresolved = apply_speaker_names(&mut roster, &notes(&[("Speaker 1", "Pranjali")]));
        assert_eq!(unresolved.len(), 1);
    }

    #[test]
    fn a_directive_cannot_be_created_without_the_parts_its_kind_needs() {
        // The guard belongs on construction: an unusable directive must never
        // reach storage in the first place.
        assert!(MeetingDirective::new(DirectiveKind::SpeakerName, None, "Pranjali").is_none());
        assert!(MeetingDirective::new(DirectiveKind::SpeakerName, Some("  "), "Pranjali").is_none());
        assert!(MeetingDirective::new(DirectiveKind::Term, None, "LanceDB").is_none());
        assert!(MeetingDirective::new(DirectiveKind::Note, None, "   ").is_none());
        assert!(MeetingDirective::new(DirectiveKind::Note, None, "remember this").is_some());
    }
}
