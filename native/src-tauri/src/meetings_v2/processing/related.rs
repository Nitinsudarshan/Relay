//! Related meetings, from relational metadata rather than a graph database.
//!
//! Two meetings are related when they share the structured metadata the pipeline
//! already extracts: type, topics, entities, and participants. Title similarity
//! is one weak signal among several and deliberately cannot carry a match on its
//! own — two meetings both called "Daily Standup" may have nothing in common,
//! which is exactly the failure a title-only implementation produces.
//!
//! No graph store is introduced. These are `meeting → topic`, `meeting → entity`,
//! `meeting → speaker`, `meeting → type` relations computed over the derived
//! artifacts on demand. If the data proves useful, a real index can replace this
//! without changing the model.

use super::model::{MeetingFacts, MeetingType};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Relative weight of each signal. Topics and entities dominate because they are
/// about subject matter; type and participants are context; the title is a hint.
const WEIGHT_TOPICS: f32 = 0.35;
const WEIGHT_ENTITIES: f32 = 0.25;
const WEIGHT_TYPE: f32 = 0.15;
const WEIGHT_SPEAKERS: f32 = 0.15;
const WEIGHT_TITLE: f32 = 0.10;

/// Below this, a "relation" is noise.
const MIN_SCORE: f32 = 0.12;

/// One meeting as the related-meetings search sees it.
#[derive(Debug, Clone)]
pub struct MeetingIndexEntry {
    pub meeting_id: String,
    pub title: String,
    pub created_at: String,
    pub meeting_type: MeetingType,
    pub topics: Vec<String>,
    pub entities: Vec<String>,
    pub speaker_labels: Vec<String>,
}

impl MeetingIndexEntry {
    /// Builds an index entry from a meeting's derived facts.
    ///
    /// `speaker_labels` uses resolved display names rather than ids, so a
    /// renamed speaker still matches across meetings — ids are per-meeting, a
    /// name is the thing two meetings can actually share.
    pub fn from_facts(
        meeting_id: &str,
        title: &str,
        created_at: &str,
        facts: &MeetingFacts,
        speaker_labels: Vec<String>,
    ) -> Self {
        Self {
            meeting_id: meeting_id.to_string(),
            title: title.to_string(),
            created_at: created_at.to_string(),
            meeting_type: facts.meeting_type,
            topics: facts.topics.iter().map(|t| t.label.clone()).collect(),
            entities: facts.entities.iter().map(|e| e.name.clone()).collect(),
            speaker_labels,
        }
    }
}

/// Which signals contributed to a match, so the UI can say *why* two meetings
/// are related rather than presenting an unexplained score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelatedSignals {
    pub shared_topics: Vec<String>,
    pub shared_entities: Vec<String>,
    pub shared_speakers: Vec<String>,
    pub same_meeting_type: bool,
    pub title_similarity: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelatedMeeting {
    pub meeting_id: String,
    pub title: String,
    pub created_at: String,
    pub meeting_type: MeetingType,
    pub score: f32,
    pub signals: RelatedSignals,
}

/// Ranks `candidates` by how related they are to `subject`.
///
/// The subject itself is excluded. Results are ordered by score, then by recency
/// so a tie between two standups prefers the more recent one.
pub fn find_related(
    subject: &MeetingIndexEntry,
    candidates: &[MeetingIndexEntry],
    limit: usize,
) -> Vec<RelatedMeeting> {
    let mut scored: Vec<RelatedMeeting> = candidates
        .iter()
        .filter(|candidate| candidate.meeting_id != subject.meeting_id)
        .filter_map(|candidate| score_pair(subject, candidate))
        .collect();

    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.created_at.cmp(&a.created_at))
    });
    scored.truncate(limit);
    scored
}

fn score_pair(
    subject: &MeetingIndexEntry,
    candidate: &MeetingIndexEntry,
) -> Option<RelatedMeeting> {
    let shared_topics = shared_labels(&subject.topics, &candidate.topics);
    let shared_entities = shared_labels(&subject.entities, &candidate.entities);
    let shared_speakers = shared_labels(&subject.speaker_labels, &candidate.speaker_labels);
    let same_meeting_type = subject.meeting_type == candidate.meeting_type;
    let title_similarity = token_similarity(&subject.title, &candidate.title);

    let topic_score = jaccard(&subject.topics, &candidate.topics);
    let entity_score = jaccard(&subject.entities, &candidate.entities);
    let speaker_score = jaccard(&subject.speaker_labels, &candidate.speaker_labels);

    let score = topic_score * WEIGHT_TOPICS
        + entity_score * WEIGHT_ENTITIES
        + if same_meeting_type { WEIGHT_TYPE } else { 0.0 }
        + speaker_score * WEIGHT_SPEAKERS
        + title_similarity * WEIGHT_TITLE;

    // A shared type and a similar title, with no subject matter in common, is
    // the "two unrelated standups" case. Require at least one substantive
    // signal before calling it a relation.
    let has_substance = !shared_topics.is_empty() || !shared_entities.is_empty();
    if !has_substance || score < MIN_SCORE {
        return None;
    }

    Some(RelatedMeeting {
        meeting_id: candidate.meeting_id.clone(),
        title: candidate.title.clone(),
        created_at: candidate.created_at.clone(),
        meeting_type: candidate.meeting_type,
        score: (score * 100.0).round() / 100.0,
        signals: RelatedSignals {
            shared_topics,
            shared_entities,
            shared_speakers,
            same_meeting_type,
            title_similarity: (title_similarity * 100.0).round() / 100.0,
        },
    })
}

/// Labels present in both lists, compared case-insensitively but reported in the
/// subject's own casing.
fn shared_labels(left: &[String], right: &[String]) -> Vec<String> {
    let right_keys: HashSet<String> = right.iter().map(|r| r.trim().to_lowercase()).collect();
    let mut seen = HashSet::new();
    left.iter()
        .filter(|l| {
            let key = l.trim().to_lowercase();
            !key.is_empty() && right_keys.contains(&key) && seen.insert(key)
        })
        .cloned()
        .collect()
}

fn jaccard(left: &[String], right: &[String]) -> f32 {
    let l: HashSet<String> = left
        .iter()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    let r: HashSet<String> = right
        .iter()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    if l.is_empty() || r.is_empty() {
        return 0.0;
    }
    let intersection = l.intersection(&r).count() as f32;
    let union = l.union(&r).count() as f32;
    intersection / union
}

/// Overlap of meaningful title words. Stop words are excluded so "the" and "and"
/// do not make two unrelated titles look similar.
fn token_similarity(left: &str, right: &str) -> f32 {
    const STOP_WORDS: &[&str] = &[
        "the",
        "and",
        "for",
        "with",
        "a",
        "an",
        "of",
        "on",
        "to",
        "in",
        "meeting",
        "call",
        "sync",
        "discussion",
        "review",
    ];

    let tokenize = |text: &str| -> HashSet<String> {
        text.split_whitespace()
            .map(|w| {
                w.chars()
                    .filter(|c| c.is_alphanumeric())
                    .collect::<String>()
                    .to_lowercase()
            })
            .filter(|w| w.len() > 2 && !STOP_WORDS.contains(&w.as_str()))
            .collect()
    };

    let l = tokenize(left);
    let r = tokenize(right);
    if l.is_empty() || r.is_empty() {
        return 0.0;
    }
    l.intersection(&r).count() as f32 / l.union(&r).count() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(
        id: &str,
        title: &str,
        date: &str,
        meeting_type: MeetingType,
        topics: &[&str],
        entities: &[&str],
        speakers: &[&str],
    ) -> MeetingIndexEntry {
        MeetingIndexEntry {
            meeting_id: id.to_string(),
            title: title.to_string(),
            created_at: date.to_string(),
            meeting_type,
            topics: topics.iter().map(|s| s.to_string()).collect(),
            entities: entities.iter().map(|s| s.to_string()).collect(),
            speaker_labels: speakers.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn meetings_sharing_subject_matter_are_related() {
        let subject = entry(
            "meet_1",
            "Schema Freeze And Migration",
            "2026-08-27T10:00:00Z",
            MeetingType::Planning,
            &["Data Migration Strategy", "Release Planning"],
            &["Relay", "Supabase"],
            &["Me", "Pranjali"],
        );
        let candidate = entry(
            "meet_2",
            "Migration Follow Up",
            "2026-08-25T10:00:00Z",
            MeetingType::Planning,
            &["Data Migration Strategy"],
            &["Supabase"],
            &["Me", "Pranjali"],
        );

        let related = find_related(&subject, &[candidate], 5);
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].meeting_id, "meet_2");
        assert!(related[0]
            .signals
            .shared_topics
            .contains(&"Data Migration Strategy".to_string()));
        assert!(related[0]
            .signals
            .shared_entities
            .contains(&"Supabase".to_string()));
        assert!(related[0].signals.same_meeting_type);
        assert_eq!(related[0].signals.shared_speakers.len(), 2);
    }

    #[test]
    fn two_identically_titled_standups_about_different_things_are_not_related() {
        // The failure a title-only implementation produces.
        let subject = entry(
            "meet_1",
            "Daily Standup",
            "2026-08-27T10:00:00Z",
            MeetingType::Scrum,
            &["Audio Processing"],
            &["Whisper"],
            &["Me"],
        );
        let candidate = entry(
            "meet_2",
            "Daily Standup",
            "2026-08-26T10:00:00Z",
            MeetingType::Scrum,
            &["Cloud Backend & Supabase"],
            &["Supabase"],
            &["Me"],
        );

        assert!(
            find_related(&subject, &[candidate], 5).is_empty(),
            "a shared title and type is not a relation"
        );
    }

    #[test]
    fn a_recurring_meeting_series_groups_together() {
        let subject = entry(
            "meet_today",
            "Sprint Standup",
            "2026-08-27T10:00:00Z",
            MeetingType::Scrum,
            &["Release Planning"],
            &["Relay"],
            &["Me", "Pranjali"],
        );
        let candidates: Vec<MeetingIndexEntry> =
            ["2026-08-20", "2026-08-21", "2026-08-24", "2026-08-25"]
                .iter()
                .enumerate()
                .map(|(i, date)| {
                    entry(
                        &format!("meet_{}", i),
                        "Sprint Standup",
                        &format!("{}T10:00:00Z", date),
                        MeetingType::Scrum,
                        &["Release Planning"],
                        &["Relay"],
                        &["Me", "Pranjali"],
                    )
                })
                .collect();

        let related = find_related(&subject, &candidates, 10);
        assert_eq!(related.len(), 4);
        // Equal scores break toward the most recent.
        assert_eq!(related[0].created_at, "2026-08-25T10:00:00Z");
        assert_eq!(related[3].created_at, "2026-08-20T10:00:00Z");
    }

    #[test]
    fn the_subject_never_matches_itself() {
        let subject = entry(
            "meet_1",
            "Schema Freeze",
            "2026-08-27T10:00:00Z",
            MeetingType::Planning,
            &["Release Planning"],
            &["Relay"],
            &["Me"],
        );
        assert!(find_related(&subject, std::slice::from_ref(&subject), 5).is_empty());
    }

    #[test]
    fn a_meeting_with_no_extracted_metadata_relates_to_nothing() {
        let bare = entry(
            "meet_1",
            "Untitled",
            "2026-08-27T10:00:00Z",
            MeetingType::General,
            &[],
            &[],
            &[],
        );
        let other = entry(
            "meet_2",
            "Untitled",
            "2026-08-26T10:00:00Z",
            MeetingType::General,
            &[],
            &[],
            &[],
        );
        assert!(find_related(&bare, &[other], 5).is_empty());
    }

    #[test]
    fn results_are_capped_at_the_limit() {
        let subject = entry(
            "meet_subject",
            "Release Planning",
            "2026-08-27T10:00:00Z",
            MeetingType::Planning,
            &["Release Planning"],
            &["Relay"],
            &["Me"],
        );
        let candidates: Vec<MeetingIndexEntry> = (0..10)
            .map(|i| {
                entry(
                    &format!("meet_{}", i),
                    "Release Planning",
                    "2026-08-20T10:00:00Z",
                    MeetingType::Planning,
                    &["Release Planning"],
                    &["Relay"],
                    &["Me"],
                )
            })
            .collect();

        assert_eq!(find_related(&subject, &candidates, 3).len(), 3);
    }

    #[test]
    fn label_matching_ignores_case_but_reports_the_subjects_casing() {
        let shared = shared_labels(
            &["Release Planning".to_string()],
            &["release planning".to_string()],
        );
        assert_eq!(shared, vec!["Release Planning".to_string()]);
    }

    #[test]
    fn title_similarity_ignores_generic_meeting_words() {
        // "Meeting" and "Sync" carry no information about subject.
        assert_eq!(token_similarity("Meeting Sync", "Meeting Sync"), 0.0);
        assert!(token_similarity("Schema Migration Plan", "Schema Migration Review") > 0.5);
    }
}
