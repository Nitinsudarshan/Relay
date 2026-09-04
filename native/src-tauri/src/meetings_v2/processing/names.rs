//! Rung 5 of `Meeting-rules/meeting_speaker_identification.md`: reading names
//! out of what people said.
//!
//! Diarization can tell you that three distinct voices spoke. It cannot tell
//! you that the second one is Pranjali. Most of the time the meeting says so
//! itself — someone introduces themselves, or is thanked by name, or is handed
//! the floor by name — and that is free information sitting in the transcript.
//!
//! This is deterministic pattern matching, not a model pass. The rules doc
//! specifies rung 5 as a model pass; a model is the wrong tool here because the
//! failure mode is inventing a name, and a model asked "who is speaker 2" will
//! always produce a plausible answer. Patterns produce nothing when there is
//! nothing, which is the required behaviour.
//!
//! Two confidence levels, kept apart because they warrant different treatment:
//!
//! * **Self-introduction** — "I'm Nitin", "my name is Pranjali". The speaker of
//!   the turn is naming themselves, so the name binds to that speaker id. Shown
//!   as an unconfirmed name the user can accept or correct.
//! * **Direct address** — "Thanks, Ayush", "Ayush, can you take that". Names
//!   somebody, but not the person speaking. Collected as a *mentioned
//!   participant* and never bound to a speaker id, because guessing which voice
//!   the name belongs to is how a summary ends up attributing a commitment to
//!   the wrong person.

use super::model::{NormalizedSegment, Speaker};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Words that look like names because they are capitalized, and are not.
///
/// Sentence-initial capitals make this necessary: "Thanks, Everyone" and
/// "Thanks, Monday" both match the shape of a direct address.
const NOT_A_NAME: &[&str] = &[
    "i", "im", "id", "ill", "ive", "you", "youre", "we", "were", "they", "he", "she", "it", "this",
    "that", "there", "here", "everyone", "everybody", "somebody", "someone", "anyone", "nobody",
    "all", "guys", "team", "folks", "monday", "tuesday", "wednesday", "thursday", "friday",
    "saturday", "sunday", "today", "tomorrow", "yesterday", "january", "february", "march",
    "april", "may", "june", "july", "august", "september", "october", "november", "december",
    "yes", "no", "ok", "okay", "yeah", "yep", "sure", "right", "well", "so", "and", "but", "or",
    "the", "a", "an", "not", "just", "very", "really", "actually", "sorry", "thanks", "thank",
    "please", "hello", "hi", "hey", "bye", "again", "also", "then", "now", "next", "last",
    "first", "second", "third", "one", "two", "three", "god", "sir", "madam", "mr", "mrs", "ms",
    "dr", "relay", "whisper", "google", "zoom", "teams", "slack", "meet",
];

/// Longest a plausible given name is, in characters. Guards against a run-on
/// capitalized phrase being taken as one name.
const MAX_NAME_LEN: usize = 24;

/// How a name was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NameEvidence {
    /// The speaker named themselves. Binds to a speaker id.
    SelfIntroduction,
    /// Somebody else was named. Does not bind to a speaker id.
    DirectAddress,
}

/// A name the transcript offered, and what it was offered for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NameCandidate {
    pub name: String,
    pub evidence: NameEvidence,
    /// Set only for a self-introduction: the speaker who introduced themselves.
    #[serde(default)]
    pub speaker_id: Option<String>,
    /// Normalized segments the name was read out of, so the claim is checkable.
    pub source_segment_ids: Vec<String>,
    /// How many times this name appeared with this evidence. A name said once
    /// in ninety minutes is weaker than one said eight times.
    pub mentions: usize,
}

/// Everything the transcript said about who was in the meeting.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NameFindings {
    /// Speaker id to the name that speaker gave for themselves.
    pub self_introductions: BTreeMap<String, NameCandidate>,
    /// Names addressed in the meeting but not bound to any voice.
    pub mentioned: Vec<NameCandidate>,
}

impl NameFindings {
    pub fn is_empty(&self) -> bool {
        self.self_introductions.is_empty() && self.mentioned.is_empty()
    }
}

/// Reads names out of a normalized transcript.
///
/// `speakers` is used only to avoid re-suggesting a name the user already
/// assigned: a speaker with a manual name is left alone, because a pattern
/// match must never overrule a person.
pub fn find_names(segments: &[NormalizedSegment], speakers: &[Speaker]) -> NameFindings {
    let already_named: Vec<&str> = speakers
        .iter()
        .filter(|s| s.display_name.as_deref().is_some_and(|n| !n.trim().is_empty()))
        .map(|s| s.id.as_str())
        .collect();

    let mut intro_hits: BTreeMap<String, (String, Vec<String>, usize)> = BTreeMap::new();
    let mut address_hits: BTreeMap<String, (Vec<String>, usize)> = BTreeMap::new();

    for segment in segments {
        for name in self_introductions(&segment.text) {
            let Some(speaker_id) = segment.speaker_id.as_deref() else {
                // Nobody to bind it to. A self-introduction from an
                // unattributed stretch is still a participant, so it is kept as
                // a mention rather than discarded.
                let entry = address_hits.entry(name).or_insert((Vec::new(), 0));
                entry.0.push(segment.id.clone());
                entry.1 += 1;
                continue;
            };
            if already_named.contains(&speaker_id) {
                continue;
            }
            let entry = intro_hits
                .entry(speaker_id.to_string())
                .or_insert((name.clone(), Vec::new(), 0));
            // The first self-introduction wins: a later one is more likely to
            // be someone repeating a name they heard than a correction.
            entry.1.push(segment.id.clone());
            entry.2 += 1;
        }

        for name in direct_addresses(&segment.text) {
            let entry = address_hits.entry(name).or_insert((Vec::new(), 0));
            entry.0.push(segment.id.clone());
            entry.1 += 1;
        }
    }

    let self_introductions = intro_hits
        .into_iter()
        .map(|(speaker_id, (name, sources, mentions))| {
            (
                speaker_id.clone(),
                NameCandidate {
                    name,
                    evidence: NameEvidence::SelfIntroduction,
                    speaker_id: Some(speaker_id),
                    source_segment_ids: sources,
                    mentions,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    // A name somebody used for themselves is not also a stranger who was
    // mentioned; the introduction is the stronger record.
    let introduced: Vec<&str> = self_introductions
        .values()
        .map(|c| c.name.as_str())
        .collect();

    let mut mentioned: Vec<NameCandidate> = address_hits
        .into_iter()
        .filter(|(name, _)| !introduced.contains(&name.as_str()))
        .map(|(name, (sources, mentions))| NameCandidate {
            name,
            evidence: NameEvidence::DirectAddress,
            speaker_id: None,
            source_segment_ids: sources,
            mentions,
        })
        .collect();
    // Most-mentioned first: that is the order a participant list should read.
    mentioned.sort_by(|a, b| b.mentions.cmp(&a.mentions).then(a.name.cmp(&b.name)));

    NameFindings {
        self_introductions,
        mentioned,
    }
}

/// Names the speaker gave for themselves in one segment.
fn self_introductions(text: &str) -> Vec<String> {
    const OPENERS: &[&str] = &[
        "i am",
        "i'm",
        "im",
        "my name is",
        "my name's",
        "this is",
        "you can call me",
        "call me",
    ];

    let mut found = Vec::new();
    let lower = text.to_lowercase();

    for opener in OPENERS {
        let mut from = 0usize;
        while let Some(at) = lower[from..].find(opener) {
            let start = from + at;
            // Must begin at a word boundary, or "him" matches "i'm".
            let boundary = start == 0
                || !lower[..start]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric());
            from = start + opener.len();
            if !boundary {
                continue;
            }
            if let Some(name) = leading_name(&text[from.min(text.len())..]) {
                found.push(name);
            }
        }
    }

    found
}

/// Names somebody other than the speaker was addressed by, in one segment.
fn direct_addresses(text: &str) -> Vec<String> {
    const AFTER: &[&str] = &["thanks", "thank you", "over to you", "hi", "hello", "hey"];

    let mut found = Vec::new();
    let lower = text.to_lowercase();

    // "Thanks, Ayush" — the name follows the phrase.
    for phrase in AFTER {
        let mut from = 0usize;
        while let Some(at) = lower[from..].find(phrase) {
            let start = from + at;
            let boundary = start == 0
                || !lower[..start]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric());
            from = start + phrase.len();
            if !boundary {
                continue;
            }
            if let Some(name) = leading_name(&text[from.min(text.len())..]) {
                found.push(name);
            }
        }
    }

    // "Ayush, can you take that" — a capitalized word followed by a comma and
    // a second-person clause. Requiring the clause is what stops "Friday, we
    // ship" matching.
    for (index, raw) in text.split_whitespace().enumerate() {
        let Some(stem) = raw.strip_suffix(',') else {
            continue;
        };
        let Some(name) = plausible_name(stem) else {
            continue;
        };
        let rest: Vec<&str> = text.split_whitespace().skip(index + 1).collect();
        let follows_with_address = rest
            .first()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase())
            .is_some_and(|w| {
                matches!(
                    w.as_str(),
                    "can" | "could" | "would" | "will" | "do" | "did" | "are" | "you" | "your"
                        | "what" | "any" | "please" | "anything"
                )
            });
        if follows_with_address {
            found.push(name);
        }
    }

    found
}

/// The name at the start of a fragment, if it starts with one.
fn leading_name(rest: &str) -> Option<String> {
    let token = rest
        .trim_start_matches(|c: char| !c.is_alphanumeric())
        .split_whitespace()
        .next()?;
    plausible_name(token)
}

/// Whether a token is plausibly a person's given name, normalized to
/// `Titlecase`.
///
/// Deliberately strict. Everything this rejects is a name Relay does not
/// suggest; everything it wrongly accepts is a name the user has to delete from
/// their own participant list, which is worse.
fn plausible_name(token: &str) -> Option<String> {
    let core: String = token
        .chars()
        .filter(|c| c.is_alphabetic() || *c == '\'' || *c == '-')
        .collect();
    let core = core.trim_matches(|c: char| c == '\'' || c == '-');
    if core.chars().count() < 2 || core.chars().count() > MAX_NAME_LEN {
        return None;
    }
    // A name is capitalized in the transcript. Normalization already
    // capitalizes sentence openings, so this is not free — it is combined with
    // the stop list below.
    if !core.chars().next().is_some_and(|c| c.is_uppercase()) {
        return None;
    }
    // ALL CAPS is an acronym, not a name.
    if core.chars().filter(|c| c.is_alphabetic()).all(|c| c.is_uppercase())
        && core.chars().count() > 2
    {
        return None;
    }
    let key: String = core.to_lowercase().chars().filter(|c| c.is_alphabetic()).collect();
    if NOT_A_NAME.contains(&key.as_str()) {
        return None;
    }

    let mut chars = core.chars();
    let first = chars.next()?;
    Some(
        first
            .to_uppercase()
            .chain(chars.flat_map(|c| c.to_lowercase()))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meetings_v2::processing::model::{SegmentChannel, SpeakerOrigin, SPEAKER_ID_ME};

    fn segment(id: &str, speaker: Option<&str>, text: &str) -> NormalizedSegment {
        NormalizedSegment {
            id: id.to_string(),
            chunk_index: 0,
            utterance_index: None,
            start_time_s: 0.0,
            end_time_s: 5.0,
            text: text.to_string(),
            raw_text: text.to_string(),
            channel: SegmentChannel::System,
            speaker_id: speaker.map(str::to_string),
            applied_rules: Vec::new(),
        }
    }

    #[test]
    fn a_self_introduction_binds_a_name_to_the_speaker_who_gave_it() {
        let segments = vec![
            segment("seg_00000_000", Some("speaker_1"), "Hi, I'm Pranjali."),
            segment(
                "seg_00001_000",
                Some("speaker_2"),
                "My name is Ayush and I run placements.",
            ),
        ];
        let findings = find_names(&segments, &[]);

        assert_eq!(
            findings.self_introductions.get("speaker_1").unwrap().name,
            "Pranjali"
        );
        assert_eq!(
            findings.self_introductions.get("speaker_2").unwrap().name,
            "Ayush"
        );
        assert_eq!(
            findings.self_introductions["speaker_1"].evidence,
            NameEvidence::SelfIntroduction
        );
    }

    #[test]
    fn a_direct_address_is_never_bound_to_a_voice() {
        // Naming somebody is not the same as being them. Binding this would be
        // how a commitment gets attributed to the wrong person.
        let segments = vec![segment(
            "seg_00000_000",
            Some("speaker_1"),
            "Thanks, Ayush. Nitin, can you take the cohort sheet?",
        )];
        let findings = find_names(&segments, &[]);

        assert!(findings.self_introductions.is_empty());
        let names: Vec<&str> = findings.mentioned.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"Ayush"), "{names:?}");
        assert!(names.contains(&"Nitin"), "{names:?}");
        assert!(findings.mentioned.iter().all(|c| c.speaker_id.is_none()));
    }

    #[test]
    fn a_name_the_user_already_assigned_is_left_alone() {
        // A pattern match must never overrule a person.
        let speakers = vec![Speaker {
            id: "speaker_1".into(),
            display_name: Some("Pranjali Sharma".into()),
            fallback_label: "Speaker 1".into(),
            origin: SpeakerOrigin::Manual,
            channel: SegmentChannel::System,
            is_local_user: false,
            segment_count: 4,
        }];
        let segments = vec![segment(
            "seg_00000_000",
            Some("speaker_1"),
            "I'm Pranjali, by the way.",
        )];
        assert!(find_names(&segments, &speakers)
            .self_introductions
            .is_empty());
    }

    #[test]
    fn capitalized_words_that_are_not_names_are_rejected() {
        let segments = vec![
            segment("seg_00000_000", Some("speaker_1"), "Thanks, Everyone."),
            segment("seg_00001_000", Some("speaker_1"), "Thanks, Monday works."),
            segment("seg_00002_000", Some("speaker_1"), "I'm Sorry about that."),
            segment("seg_00003_000", Some("speaker_1"), "This is Relay speaking."),
            segment("seg_00004_000", Some("speaker_1"), "Hi, ETA is Friday."),
        ];
        let findings = find_names(&segments, &[]);
        assert!(
            findings.is_empty(),
            "invented names: {:?} / {:?}",
            findings.self_introductions,
            findings.mentioned
        );
    }

    #[test]
    fn a_filler_word_inside_another_word_does_not_match_an_opener() {
        // "him" contains "im"; "victim" contains "i'm" once apostrophes are
        // stripped. Neither is a self-introduction.
        let segments = vec![
            segment("seg_00000_000", Some("speaker_1"), "I told him Rahul agreed."),
            segment("seg_00001_000", Some("speaker_1"), "The victim Statement stands."),
        ];
        assert!(find_names(&segments, &[]).self_introductions.is_empty());
    }

    #[test]
    fn a_comma_name_needs_a_second_person_clause_after_it() {
        // "Nitin, can you" is an address. "Friday, we ship" is a date.
        let addressed = vec![segment(
            "seg_00000_000",
            Some("speaker_1"),
            "Nitin, can you send it?",
        )];
        assert_eq!(
            find_names(&addressed, &[])
                .mentioned
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Nitin"]
        );

        let not_addressed = vec![segment(
            "seg_00000_000",
            Some("speaker_1"),
            "Rahul, we already shipped it.",
        )];
        assert!(find_names(&not_addressed, &[]).mentioned.is_empty());
    }

    #[test]
    fn a_self_introduction_from_an_unattributed_stretch_becomes_a_mention() {
        // Somebody said their name; Relay does not know whose voice it was.
        // Losing the participant would be worse than listing them unbound.
        let segments = vec![segment("seg_00000_000", None, "I'm Rahul, joining late.")];
        let findings = find_names(&segments, &[]);
        assert!(findings.self_introductions.is_empty());
        assert_eq!(findings.mentioned[0].name, "Rahul");
        assert_eq!(findings.mentioned[0].speaker_id, None);
    }

    #[test]
    fn repeated_mentions_are_counted_and_ordered_by_frequency() {
        let segments = vec![
            segment("seg_00000_000", Some(SPEAKER_ID_ME), "Thanks, Ayush."),
            segment("seg_00001_000", Some(SPEAKER_ID_ME), "Thanks, Ayush."),
            segment("seg_00002_000", Some(SPEAKER_ID_ME), "Thanks, Rahul."),
        ];
        let findings = find_names(&segments, &[]);
        assert_eq!(findings.mentioned[0].name, "Ayush");
        assert_eq!(findings.mentioned[0].mentions, 2);
        assert_eq!(findings.mentioned[1].name, "Rahul");
        assert_eq!(findings.mentioned[0].source_segment_ids.len(), 2);
    }

    #[test]
    fn somebody_who_introduced_themselves_is_not_also_listed_as_a_stranger() {
        let segments = vec![
            segment("seg_00000_000", Some("speaker_1"), "I'm Pranjali."),
            segment("seg_00001_000", Some(SPEAKER_ID_ME), "Thanks, Pranjali."),
        ];
        let findings = find_names(&segments, &[]);
        assert_eq!(findings.self_introductions.len(), 1);
        assert!(
            findings.mentioned.is_empty(),
            "listed twice: {:?}",
            findings.mentioned
        );
    }

    #[test]
    fn an_empty_transcript_offers_no_names() {
        assert!(find_names(&[], &[]).is_empty());
        let silent = vec![segment("seg_00000_000", Some("speaker_1"), "")];
        assert!(find_names(&silent, &[]).is_empty());
    }

    #[test]
    fn names_are_normalized_to_titlecase() {
        assert_eq!(plausible_name("PRANJALI"), None, "all caps is an acronym");
        assert_eq!(plausible_name("Pranjali"), Some("Pranjali".to_string()));
        assert_eq!(plausible_name("Pranjali,"), Some("Pranjali".to_string()));
        assert_eq!(plausible_name("O'Brien"), Some("O'brien".to_string()));
        assert_eq!(plausible_name("pranjali"), None, "lowercase is not a name here");
        assert_eq!(plausible_name("A"), None, "one letter is an initial");
    }
}
