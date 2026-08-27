//! Unit tests for the action-item gate.
//!
//! Every rejected phrase in here is one that a real meeting produced and that
//! the pipeline turned into a task. They are kept verbatim rather than
//! paraphrased, because the point of this file is that *these* sentences never
//! come back.

use super::*;
use crate::meetings_v2::processing::model::{
    ActionItemStatus, SegmentChannel, SPEAKER_ID_ME, SPEAKER_ID_REMOTE,
};

fn segment(index: usize, text: &str, speaker: Option<&str>) -> NormalizedSegment {
    NormalizedSegment {
        id: format!("seg_{:05}", index),
        chunk_index: index,
        start_time_s: index as f64 * 30.0,
        end_time_s: (index + 1) as f64 * 30.0,
        text: text.to_string(),
        raw_text: text.to_string(),
        channel: match speaker {
            Some(SPEAKER_ID_ME) => SegmentChannel::Mic,
            Some(_) => SegmentChannel::System,
            None => SegmentChannel::Mixed,
        },
        speaker_id: speaker.map(str::to_string),
        applied_rules: Vec::new(),
    }
}

fn candidate(description: &str, owner: OwnerType, segment_ids: &[&str]) -> ActionItem {
    ActionItem {
        id: "action_x".to_string(),
        description: description.to_string(),
        owner_type: owner,
        owner_speaker_id: match owner {
            OwnerType::Me => Some(SPEAKER_ID_ME.to_string()),
            OwnerType::Speaker => Some(SPEAKER_ID_REMOTE.to_string()),
            _ => None,
        },
        owner_label: None,
        deadline: None,
        status: ActionItemStatus::Open,
        source_segment_ids: segment_ids.iter().map(|s| s.to_string()).collect(),
        confidence: 0.8,
    }
}

/// Runs the gate over one candidate whose evidence is the sentence itself,
/// which is the shape the cue-based extractor produces.
fn verdict(sentence: &str, owner: OwnerType) -> Option<RejectionReason> {
    let segments = vec![segment(0, sentence, Some(SPEAKER_ID_ME))];
    let candidates = vec![candidate(sentence, owner, &["seg_00000"])];
    let (_, report) = qualify_action_items(candidates, &segments);
    report.diagnostics[0].rejection_reason
}

// ---------------------------------------------------------------------------
// Gate 1 — durability
// ---------------------------------------------------------------------------

#[test]
fn meeting_mechanics_never_become_tasks() {
    // Every one of these shipped to a user as an action item.
    for sentence in [
        "I'll just be back in a minute so give me a second.",
        "I will project my screen so you can follow along.",
        "Let me just check the ID for this booking.",
        "I'll stop sharing now and hand it over.",
        "I will take you through the pointers one by one.",
        "Yes I'll quickly check with Ayush to join the call.",
        "I'll share my screen and we can look at it together.",
        "We are taking notes in the meeting so we will update her.",
        "I'll speak first and then hand over to the team.",
    ] {
        assert_eq!(
            verdict(sentence, OwnerType::Me),
            Some(RejectionReason::MeetingMechanic),
            "should have been rejected as mechanics: {}",
            sentence
        );
    }
}

#[test]
fn demo_narration_never_becomes_a_task() {
    for sentence in [
        "Now I'll click here and change the role to member.",
        "I will move it to approved on this screen so you can see the flow.",
        "Let me show you the dashboard and I'll scroll down to the reports.",
        "I'll switch to the other tab now and refresh the page.",
    ] {
        assert!(
            matches!(
                verdict(sentence, OwnerType::Me),
                Some(RejectionReason::DemoNarration) | Some(RejectionReason::MeetingMechanic)
            ),
            "should have been rejected as demo narration: {}",
            sentence
        );
    }
}

#[test]
fn a_real_commitment_that_shares_a_verb_with_a_demo_still_survives() {
    // "switch" and "share" are demo verbs only when something is being pointed
    // at. Real work using the same verbs must not be collateral damage.
    assert_eq!(
        verdict(
            "I'll switch the mail provider over to Gmail SMTP after the call.",
            OwnerType::Me
        ),
        None
    );
    assert_eq!(
        verdict("I'll share the required email list with PNC.", OwnerType::Me),
        None
    );
}

// ---------------------------------------------------------------------------
// Gates 2 and 3
// ---------------------------------------------------------------------------

#[test]
fn vague_intentions_without_a_deliverable_are_dropped() {
    for sentence in [
        "Some of the things I will jump in wherever needed.",
        "I'll look into it and we will take it up.",
        "We will handle it between us somehow.",
    ] {
        assert!(
            matches!(
                verdict(sentence, OwnerType::Unassigned),
                Some(RejectionReason::NoDeliverable) | Some(RejectionReason::BrokenFragment)
            ),
            "should have been rejected for having no deliverable: {} ({:?})",
            sentence,
            verdict(sentence, OwnerType::Unassigned)
        );
    }
}

#[test]
fn hypotheticals_are_dropped() {
    for sentence in [
        "We could send them a summary email at some point.",
        "Maybe we will add a city dropdown in version two.",
        "It would be nice to send the tracker every week.",
    ] {
        assert_eq!(
            verdict(sentence, OwnerType::Unassigned),
            Some(RejectionReason::Hypothetical),
            "should have been rejected as hypothetical: {}",
            sentence
        );
    }
}

#[test]
fn work_already_done_is_dropped() {
    for sentence in [
        "I already sent the configuration to the platform team.",
        "We have already updated the employee guide this morning.",
    ] {
        assert_eq!(
            verdict(sentence, OwnerType::Me),
            Some(RejectionReason::AlreadyCompleted),
            "should have been rejected as completed: {}",
            sentence
        );
    }
}

#[test]
fn an_observation_is_not_a_commitment() {
    // A deliverable verb and an object, but nobody undertook anything.
    assert_eq!(
        verdict(
            "They are asking us to send the vendor report every month.",
            OwnerType::Unassigned
        ),
        Some(RejectionReason::NoCommitment)
    );
}

#[test]
fn assignment_plus_acceptance_qualifies() {
    // Rules §4.2 — the accepter owns it, and the acceptance is in a different
    // sentence from the request.
    let segments = vec![segment(
        0,
        "Can you review the employee guide before Friday? Sure, we'll go through \
the employee guide and send the corrections.",
        Some(SPEAKER_ID_REMOTE),
    )];
    let candidates = vec![candidate(
        "Review the employee guide and send corrections",
        OwnerType::Speaker,
        &["seg_00000"],
    )];
    let (retained, report) = qualify_action_items(candidates, &segments);
    assert_eq!(retained.len(), 1, "diagnostics: {:?}", report.diagnostics);
}

// ---------------------------------------------------------------------------
// Structural rejections
// ---------------------------------------------------------------------------

#[test]
fn a_decoder_loop_is_never_a_task() {
    let looped = "I will pay the firm to fill the form. ".repeat(9);
    let segments = vec![segment(0, &looped, Some(SPEAKER_ID_ME))];
    let candidates = vec![candidate(&looped, OwnerType::Me, &["seg_00000"])];
    let (retained, report) = qualify_action_items(candidates, &segments);
    assert!(retained.is_empty());
    assert_eq!(
        report.diagnostics[0].rejection_reason,
        Some(RejectionReason::DecoderLoop)
    );
}

#[test]
fn a_collided_fragment_is_discarded_rather_than_repaired() {
    assert_eq!(
        verdict(
            "There are few features that we will the specialty IUC has also joined in.",
            OwnerType::Unassigned
        ),
        Some(RejectionReason::BrokenFragment)
    );
}

#[test]
fn a_candidate_with_no_transcript_evidence_is_discarded() {
    let segments = vec![segment(0, "We talked about the release.", Some(SPEAKER_ID_ME))];
    let candidates = vec![candidate("Send the release notes", OwnerType::Me, &[])];
    let (retained, report) = qualify_action_items(candidates, &segments);
    assert!(retained.is_empty(), "an unsourced action item is unprovable");
    assert_eq!(
        report.diagnostics[0].rejection_reason,
        Some(RejectionReason::NoEvidence)
    );
}

// ---------------------------------------------------------------------------
// Owners
// ---------------------------------------------------------------------------

#[test]
fn an_owner_the_channel_cannot_support_is_demoted_not_guessed() {
    // Both channels were live for this chunk, so nothing in the data says who
    // spoke. Unassigned is the correct answer.
    let segments = vec![segment(
        0,
        "I'll send the required email list to the vendor tomorrow.",
        None,
    )];
    let candidates = vec![candidate(
        "Send the required email list",
        OwnerType::Me,
        &["seg_00000"],
    )];
    let (retained, report) = qualify_action_items(candidates, &segments);

    assert_eq!(retained.len(), 1);
    assert_eq!(retained[0].owner_type, OwnerType::Unassigned);
    assert!(retained[0].owner_speaker_id.is_none());
    assert_eq!(report.counts.owners_downgraded, 1);
    assert_eq!(report.counts.unassigned, 1);
}

#[test]
fn an_owner_the_channel_supports_is_kept() {
    let segments = vec![segment(
        0,
        "I'll send the required email list to the vendor tomorrow.",
        Some(SPEAKER_ID_ME),
    )];
    let candidates = vec![candidate(
        "Send the required email list",
        OwnerType::Me,
        &["seg_00000"],
    )];
    let (retained, report) = qualify_action_items(candidates, &segments);
    assert_eq!(retained[0].owner_type, OwnerType::Me);
    assert_eq!(report.counts.owners_downgraded, 0);
}

// ---------------------------------------------------------------------------
// Deduplication
// ---------------------------------------------------------------------------

#[test]
fn restatements_of_one_commitment_collapse_into_the_richest_version() {
    let segments = vec![
        segment(
            0,
            "I'll send the mail list to them.",
            Some(SPEAKER_ID_ME),
        ),
        segment(
            1,
            "I'll send you the list of mails that need to go out.",
            Some(SPEAKER_ID_ME),
        ),
        segment(
            2,
            "Right, I'll share the required email list by tomorrow.",
            Some(SPEAKER_ID_ME),
        ),
    ];
    let mut richest = candidate(
        "Share the required email list",
        OwnerType::Me,
        &["seg_00002"],
    );
    richest.deadline = Some("2026-08-28".to_string());

    let candidates = vec![
        candidate("Send the mail list", OwnerType::Me, &["seg_00000"]),
        candidate(
            "Send the list of mails that need to go out",
            OwnerType::Me,
            &["seg_00001"],
        ),
        richest,
    ];

    let (retained, report) = qualify_action_items(candidates, &segments);
    assert_eq!(retained.len(), 1, "retained: {:?}", retained);
    assert_eq!(report.counts.deduplicated, 2);
    assert_eq!(
        retained[0].deadline.as_deref(),
        Some("2026-08-28"),
        "the version carrying a date is the richer one"
    );
    assert_eq!(
        retained[0].source_segment_ids.len(),
        3,
        "provenance from every restatement is kept"
    );
}

#[test]
fn a_different_act_on_the_same_object_is_not_merged_away() {
    let segments = vec![
        segment(0, "I'll send the employee guide to Pranjal.", Some(SPEAKER_ID_ME)),
        segment(
            1,
            "I'll review the employee guide for discrepancies.",
            Some(SPEAKER_ID_ME),
        ),
    ];
    let candidates = vec![
        candidate("Send the employee guide", OwnerType::Me, &["seg_00000"]),
        candidate("Review the employee guide", OwnerType::Me, &["seg_00001"]),
    ];
    let (retained, _) = qualify_action_items(candidates, &segments);
    assert_eq!(retained.len(), 2, "sending and reviewing are different work");
}

#[test]
fn two_owners_are_never_merged_into_one_task() {
    let segments = vec![
        segment(0, "I'll send the tracker link.", Some(SPEAKER_ID_ME)),
        segment(1, "I'll send the tracker link as well.", Some(SPEAKER_ID_REMOTE)),
    ];
    let candidates = vec![
        candidate("Send the tracker link", OwnerType::Me, &["seg_00000"]),
        candidate("Send the tracker link", OwnerType::Speaker, &["seg_00001"]),
    ];
    let (retained, _) = qualify_action_items(candidates, &segments);
    assert_eq!(retained.len(), 2);
}

// ---------------------------------------------------------------------------
// The cap
// ---------------------------------------------------------------------------

#[test]
fn the_cap_is_a_ceiling_and_the_list_is_never_padded_to_it() {
    let segments: Vec<NormalizedSegment> = (0..3)
        .map(|i| {
            segment(
                i,
                "I'll send the migration plan to the platform team.",
                Some(SPEAKER_ID_ME),
            )
        })
        .collect();
    let candidates = vec![
        candidate("Send the migration plan", OwnerType::Me, &["seg_00000"]),
        candidate("Review the rollback script", OwnerType::Me, &["seg_00001"]),
    ];
    let (retained, _) = qualify_action_items(candidates, &segments);
    assert!(
        retained.len() <= 2,
        "two qualifying candidates must not become fifteen"
    );
}

#[test]
fn more_than_fifteen_qualifying_items_are_capped_at_fifteen() {
    let objects = [
        "migration plan",
        "rollback script",
        "release notes",
        "cancellation logic",
        "city dropdown",
        "analytics filter",
        "mail service",
        "chat support hours",
        "employee guide",
        "query tracker",
        "slack channel",
        "email templates",
        "ticket workflow",
        "audit log",
        "billing report",
        "onboarding checklist",
        "vendor contract",
        "status dashboard",
    ];
    let segments: Vec<NormalizedSegment> = objects
        .iter()
        .enumerate()
        .map(|(i, object)| {
            segment(
                i,
                &format!("I'll update the {} after this call.", object),
                Some(SPEAKER_ID_ME),
            )
        })
        .collect();
    let candidates: Vec<ActionItem> = objects
        .iter()
        .enumerate()
        .map(|(i, object)| {
            candidate(
                &format!("Update the {}", object),
                OwnerType::Me,
                &[Box::leak(format!("seg_{:05}", i).into_boxed_str())],
            )
        })
        .collect();

    let (retained, report) = qualify_action_items(candidates, &segments);
    assert_eq!(
        retained.len(),
        MAX_ACTION_ITEMS,
        "rejections: {:?}",
        report.rejection_codes()
    );
    assert_eq!(report.counts.capped, objects.len() - MAX_ACTION_ITEMS);
    assert_eq!(
        report.counts.rejected, 0,
        "\"after this call\" is ordinary phrasing, not demo narration"
    );
}

#[test]
fn an_explicitly_owned_item_outranks_an_unassigned_one_at_the_cap() {
    const OBJECTS: [&str; 18] = [
        "migration plan",
        "rollback script",
        "release notes",
        "cancellation logic",
        "city dropdown",
        "analytics filter",
        "mail service",
        "chat support hours",
        "employee guide",
        "query tracker",
        "slack channel",
        "email templates",
        "ticket workflow",
        "audit log",
        "billing report",
        "onboarding checklist",
        "vendor contract",
        "status dashboard",
    ];

    let mut segments = Vec::new();
    let mut candidates = Vec::new();
    for (index, object) in OBJECTS.iter().enumerate() {
        let owned = index % 2 == 0;
        segments.push(segment(
            index,
            &format!(
                "{} update the {} after the call.",
                if owned { "I'll" } else { "We'll" },
                object
            ),
            Some(SPEAKER_ID_ME),
        ));
        candidates.push(candidate(
            &format!("Update the {}", object),
            if owned {
                OwnerType::Me
            } else {
                OwnerType::Unassigned
            },
            &[Box::leak(format!("seg_{:05}", index).into_boxed_str())],
        ));
    }

    let (retained, report) = qualify_action_items(candidates, &segments);
    assert_eq!(
        retained.len(),
        MAX_ACTION_ITEMS,
        "diagnostics: {:?}",
        report.counts
    );
    let owned = retained
        .iter()
        .filter(|i| i.owner_type == OwnerType::Me)
        .count();
    assert_eq!(owned, 9, "every explicitly owned item survived the cap");
}

// ---------------------------------------------------------------------------
// Diagnostics
// ---------------------------------------------------------------------------

#[test]
fn every_candidate_is_accounted_for() {
    let segments = vec![
        segment(0, "I'll share my screen now.", Some(SPEAKER_ID_ME)),
        segment(
            1,
            "I'll send the required email list tomorrow.",
            Some(SPEAKER_ID_ME),
        ),
    ];
    let candidates = vec![
        candidate("Share my screen", OwnerType::Me, &["seg_00000"]),
        candidate("Send the required email list", OwnerType::Me, &["seg_00001"]),
    ];
    let (retained, report) = qualify_action_items(candidates, &segments);

    assert_eq!(report.counts.candidates, 2);
    assert_eq!(report.counts.retained, 1);
    assert_eq!(retained.len(), 1);
    assert_eq!(report.diagnostics.len(), 2, "one record per candidate");
    assert_eq!(
        report.rejection_codes(),
        vec!["MEETING_MECHANIC"],
        "the rejection says which rule caught it"
    );
    assert!(report.diagnostics.iter().any(|d| d.accepted));
}

#[test]
fn retained_items_keep_meeting_order_not_ranking_order() {
    let segments = vec![
        segment(
            0,
            "I'll update the cancellation logic after this call.",
            Some(SPEAKER_ID_ME),
        ),
        segment(
            1,
            "And I'll send the required email list by tomorrow.",
            Some(SPEAKER_ID_ME),
        ),
    ];
    let mut second = candidate(
        "Send the required email list",
        OwnerType::Me,
        &["seg_00001"],
    );
    second.deadline = Some("2026-08-28".to_string());
    let candidates = vec![
        candidate(
            "Update the cancellation logic",
            OwnerType::Me,
            &["seg_00000"],
        ),
        second,
    ];

    let (retained, _) = qualify_action_items(candidates, &segments);
    assert_eq!(retained.len(), 2);
    assert!(retained[0].description.starts_with("Update"));
    assert_eq!(retained[0].id, "action_0");
    assert_eq!(retained[1].id, "action_1");
}

