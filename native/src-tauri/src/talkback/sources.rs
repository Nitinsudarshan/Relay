//! Projecting Relay's existing stores into retrievable candidates.
//!
//! This is the only I/O in the retrieval path, and the only file that
//! knows a `Scribble` from a `MeetingFacts`. Everything downstream sees
//! [`CandidateDoc`]s.
//!
//! Talkback creates **no storage of its own** (`ARCHITECTURE.md` §12).
//! Adding a future source — files, calendar, email — means adding a
//! projector here, not a database.

use super::retrieval::{CandidateDoc, SourceType};
use crate::meetings_v2::processing::model::MeetingFacts;
use crate::meetings_v2::processing::MeetingProcessor;
use crate::meetings_v2::session_store::SessionStore;
use crate::meetings_v2::types::MeetingSession;
use crate::vault::{Scribble, VaultManager, VaultNote, VOICE_NOTE_TYPE};

/// How much of a long document is worth carrying into ranking.
///
/// Scoring reads the whole body, and a two-hour meeting transcript is
/// hundreds of kilobytes. Capping keeps a full-vault scan bounded; the
/// excerpt selector then picks the relevant window out of whatever
/// survives.
const MAX_BODY_CHARS: usize = 20_000;

fn cap(body: &str) -> String {
    if body.chars().count() <= MAX_BODY_CHARS {
        body.to_string()
    } else {
        body.chars().take(MAX_BODY_CHARS).collect()
    }
}

/// A Voice Note as a candidate. Verbatim dictation — high recall, which
/// is why `SourceType::weight` scores it lowest.
pub fn voice_note_candidate(note: &VaultNote) -> CandidateDoc {
    CandidateDoc::new(
        SourceType::VoiceNote,
        &note.id,
        &note.title,
        &cap(&note.content),
    )
    .with_timestamp(&note.created_at)
    .with_topics(note.tags.clone())
}

/// A Scribble as a candidate, carrying its topics, entities and links so
/// one-hop expansion has something to walk.
pub fn scribble_candidate(scribble: &Scribble) -> CandidateDoc {
    let body = match &scribble.summary {
        // The summary is the author's own compression; leading with it
        // means a matched Scribble contributes its point rather than its
        // opening sentence.
        Some(summary) if !summary.trim().is_empty() => {
            format!("{}\n\n{}", summary.trim(), scribble.content)
        }
        _ => scribble.content.clone(),
    };
    CandidateDoc::new(SourceType::Scribble, &scribble.id, &scribble.title, &cap(&body))
        .with_timestamp(&scribble.created_at)
        .with_topics(scribble.topics.clone())
        .with_entities(scribble.entities.clone())
        .with_related(
            scribble
                .relationships
                .iter()
                .map(|r| r.target_id.clone())
                .collect(),
        )
}

/// A meeting's generated summary as a candidate.
///
/// Returns `None` for a meeting with no summary — an empty candidate
/// would dilute IDF across the corpus for no possible match.
pub fn meeting_candidate(
    session: &MeetingSession,
    summary_markdown: Option<&str>,
) -> Option<CandidateDoc> {
    let body = summary_markdown
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or(session.summary.as_deref())?;
    Some(
        CandidateDoc::new(SourceType::Meeting, &session.id, &session.title, &cap(body))
            .with_timestamp(session.ended_at.as_deref().unwrap_or(&session.created_at))
            .with_detail("summary"),
    )
}

/// A meeting's derived intelligence as a candidate.
///
/// This is the highest-weighted source in Talkback, and the reason is the
/// whole Granola/Notion lesson from `RESEARCH.md` §A: a decision plus its
/// rationale answers "what did we decide and why" directly, where the
/// transcript only contains the raw material for that answer.
///
/// Rendered as flat prose rather than JSON because the ranker scores text
/// and the model reads text; the structure has already done its job by
/// deciding what is worth including.
pub fn meeting_facts_candidate(
    session: &MeetingSession,
    facts: &MeetingFacts,
) -> Option<CandidateDoc> {
    let mut lines: Vec<String> = Vec::new();

    for decision in &facts.decisions {
        match &decision.rationale {
            Some(why) if !why.trim().is_empty() => {
                lines.push(format!("Decision: {} — because {}", decision.statement, why))
            }
            _ => lines.push(format!("Decision: {}", decision.statement)),
        }
    }
    for item in &facts.action_items {
        let owner = item
            .owner_label
            .as_deref()
            .or(item.owner_speaker_id.as_deref())
            .unwrap_or("unassigned");
        let deadline = item
            .deadline
            .as_deref()
            .map(|d| format!(" by {d}"))
            .unwrap_or_default();
        lines.push(format!(
            "Action item: {} ({}{})",
            item.description, owner, deadline
        ));
    }
    for point in &facts.key_points {
        lines.push(format!("{}: {}", point.kind.label(), point.text));
    }
    for risk in &facts.risks {
        lines.push(format!("{}: {}", risk.kind.label(), risk.statement));
    }
    for question in &facts.open_questions {
        lines.push(format!("Open question: {}", question.question));
    }

    if lines.is_empty() {
        return None;
    }

    let title = if facts.title.trim().is_empty() {
        session.title.clone()
    } else {
        facts.title.clone()
    };
    let detail = if !facts.decisions.is_empty() {
        "decisions"
    } else if !facts.action_items.is_empty() {
        "action items"
    } else {
        "key points"
    };

    Some(
        CandidateDoc::new(
            SourceType::MeetingFacts,
            &session.id,
            &title,
            &cap(&lines.join("\n")),
        )
        .with_timestamp(session.ended_at.as_deref().unwrap_or(&session.created_at))
        .with_topics(facts.topics.iter().map(|t| t.label.clone()).collect())
        .with_entities(facts.entities.iter().map(|e| e.name.clone()).collect())
        .with_detail(detail),
    )
}

/// Gathers every candidate Talkback can search.
///
/// Reads the vault and meeting stores directly rather than through a
/// cache: the corpus is a personal one (hundreds to low thousands of
/// documents), a cache would need invalidating on every capture from four
/// different surfaces, and a stale answer about your own notes is worse
/// than a slower one. If a real corpus ever outgrows this, the fix is
/// embeddings and an index — not a cache in front of a linear scan.
///
/// A failing store is logged and skipped rather than failing the turn:
/// losing meetings should not cost the user their Scribbles too.
pub fn gather_candidates(
    vault: &VaultManager,
    sessions: &SessionStore,
    processor: &MeetingProcessor,
    wanted: &[SourceType],
) -> Vec<CandidateDoc> {
    let mut candidates = Vec::new();

    if wanted.contains(&SourceType::VoiceNote) {
        match vault.list_notes_by_type(VOICE_NOTE_TYPE) {
            Ok(notes) => candidates.extend(notes.iter().map(voice_note_candidate)),
            Err(e) => tracing::warn!("talkback: voice notes unavailable for retrieval: {}", e),
        }
    }

    if wanted.contains(&SourceType::Scribble) {
        match vault.list_scribbles() {
            Ok(scribbles) => candidates.extend(scribbles.iter().map(scribble_candidate)),
            Err(e) => tracing::warn!("talkback: scribbles unavailable for retrieval: {}", e),
        }
    }

    let needs_meetings =
        wanted.contains(&SourceType::Meeting) || wanted.contains(&SourceType::MeetingFacts);
    if needs_meetings {
        match sessions.list_sessions() {
            Ok(meetings) => {
                for session in &meetings {
                    let processing = processor.get(&session.id);
                    if wanted.contains(&SourceType::MeetingFacts) {
                        if let Some(facts) = processing.as_ref().and_then(|p| p.facts.as_ref()) {
                            if let Some(candidate) = meeting_facts_candidate(session, facts) {
                                candidates.push(candidate);
                            }
                        }
                    }
                    if wanted.contains(&SourceType::Meeting) {
                        let summary = processing
                            .as_ref()
                            .and_then(|p| p.summary.as_ref())
                            .map(|s| s.markdown.as_str());
                        if let Some(candidate) = meeting_candidate(session, summary) {
                            candidates.push(candidate);
                        }
                    }
                }
            }
            Err(e) => tracing::warn!("talkback: meetings unavailable for retrieval: {}", e),
        }
    }

    candidates
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meetings_v2::processing::model::{
        ActionItem, ActionItemStatus, Decision, KeyPoint, KeyPointKind, MeetingType, OwnerType,
        Topic,
    };
    use crate::vault::{ScribbleRelationship, VaultNote};

    fn session() -> MeetingSession {
        let mut s = MeetingSession::new("meeting_1".to_string(), Some("Pricing review".into()));
        s.created_at = "2026-08-01T09:00:00Z".to_string();
        s.ended_at = Some("2026-08-01T10:00:00Z".to_string());
        s
    }

    fn facts() -> MeetingFacts {
        MeetingFacts {
            title: "Pricing review".to_string(),
            meeting_type: MeetingType::General,
            key_points: vec![KeyPoint {
                id: "k1".into(),
                text: "Procurement wants an annual number".into(),
                kind: KeyPointKind::Discussion,
                topic_id: None,
                source_segment_ids: vec![],
            }],
            topics: vec![Topic {
                id: "t1".into(),
                label: "pricing".into(),
                segment_ids: vec![],
            }],
            decisions: vec![Decision {
                id: "d1".into(),
                statement: "Ship the flat seat licence".into(),
                rationale: Some("procurement predictability outweighs upside".into()),
                decided_by_speaker_id: None,
                source_segment_ids: vec![],
                confidence: 0.9,
            }],
            action_items: vec![ActionItem {
                id: "a1".into(),
                description: "Draft the pricing page".into(),
                owner_type: OwnerType::Me,
                owner_speaker_id: None,
                owner_label: Some("Nitin".into()),
                deadline: Some("2026-08-15".into()),
                status: ActionItemStatus::Open,
                source_segment_ids: vec![],
                confidence: 0.8,
                kanban_card_id: None,
            }],
            open_questions: vec![],
            risks: vec![],
            entities: vec![],
            speaker_ids: vec![],
            deterministic: false,
        }
    }

    #[test]
    fn a_decisions_rationale_survives_into_the_candidate() {
        let candidate = meeting_facts_candidate(&session(), &facts()).unwrap();
        assert!(
            candidate
                .body
                .contains("Ship the flat seat licence — because procurement predictability"),
            "the rationale is the part a memory question actually needs: {}",
            candidate.body
        );
    }

    #[test]
    fn action_items_carry_owner_and_deadline() {
        let candidate = meeting_facts_candidate(&session(), &facts()).unwrap();
        assert!(candidate
            .body
            .contains("Action item: Draft the pricing page (Nitin by 2026-08-15)"));
    }

    #[test]
    fn facts_are_timestamped_from_when_the_meeting_ended() {
        let candidate = meeting_facts_candidate(&session(), &facts()).unwrap();
        assert_eq!(candidate.timestamp, "2026-08-01T10:00:00Z");
        assert_eq!(candidate.source_type, SourceType::MeetingFacts);
        assert_eq!(candidate.detail.as_deref(), Some("decisions"));
    }

    #[test]
    fn facts_topics_become_searchable_tags() {
        let candidate = meeting_facts_candidate(&session(), &facts()).unwrap();
        assert_eq!(candidate.topics, vec!["pricing".to_string()]);
    }

    #[test]
    fn empty_facts_produce_no_candidate() {
        let empty = MeetingFacts {
            title: "Nothing".into(),
            meeting_type: MeetingType::General,
            key_points: vec![],
            topics: vec![],
            decisions: vec![],
            action_items: vec![],
            open_questions: vec![],
            risks: vec![],
            entities: vec![],
            speaker_ids: vec![],
            deterministic: false,
        };
        assert!(meeting_facts_candidate(&session(), &empty).is_none());
    }

    #[test]
    fn a_meeting_with_no_summary_produces_no_candidate() {
        assert!(meeting_candidate(&session(), None).is_none());
        assert!(meeting_candidate(&session(), Some("   ")).is_none());
    }

    #[test]
    fn a_meeting_summary_becomes_a_candidate() {
        let candidate = meeting_candidate(&session(), Some("We chose the flat licence.")).unwrap();
        assert_eq!(candidate.source_type, SourceType::Meeting);
        assert_eq!(candidate.source_id, "meeting_1");
        assert_eq!(candidate.body, "We chose the flat licence.");
    }

    #[test]
    fn a_scribbles_summary_leads_its_body() {
        let mut scribble = Scribble::new_text("A long rambling body about pricing.", Some("Pricing"));
        scribble.summary = Some("Flat licence wins".to_string());
        let candidate = scribble_candidate(&scribble);
        assert!(candidate.body.starts_with("Flat licence wins"));
        assert!(candidate.body.contains("A long rambling body"));
    }

    #[test]
    fn scribble_relationships_become_expansion_edges() {
        let mut scribble = Scribble::new_text("body", Some("Pricing"));
        scribble.relationships = vec![ScribbleRelationship {
            id: "r1".into(),
            target_id: "scribble_other".into(),
            relationship_type: "RELATED_TO".into(),
            confidence: 1.0,
            source: "user".into(),
        }];
        let candidate = scribble_candidate(&scribble);
        assert_eq!(candidate.related_ids, vec!["scribble_other".to_string()]);
    }

    #[test]
    fn a_voice_note_becomes_a_candidate_with_its_transcript() {
        let note = VaultNote::new_voice_note("so the pricing thing, flat licence I think");
        let candidate = voice_note_candidate(&note);
        assert_eq!(candidate.source_type, SourceType::VoiceNote);
        assert_eq!(candidate.body, "so the pricing thing, flat licence I think");
        assert!(!candidate.timestamp.is_empty());
    }

    #[test]
    fn oversized_bodies_are_capped() {
        let huge = "word ".repeat(40_000);
        let mut scribble = Scribble::new_text(&huge, Some("Big"));
        scribble.summary = None;
        let candidate = scribble_candidate(&scribble);
        assert!(candidate.body.chars().count() <= MAX_BODY_CHARS);
    }
}
