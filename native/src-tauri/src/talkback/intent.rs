//! Intent routing for a Talkback turn.
//!
//! Runs before anything else, and deliberately without a model. Two
//! reasons: an LLM round-trip to decide *what kind of question this is*
//! costs the whole latency budget before retrieval even starts, and the
//! one behaviour Talkback must never get wrong — answering a
//! personal-memory question from general model knowledge — is exactly the
//! kind of judgement you want pinned down by tests rather than sampling.
//!
//! The seam for a model-driven router (or real tool-calling) is
//! [`route`]'s signature: swap the body, keep the enum.

use serde::{Deserialize, Serialize};

/// What the user's turn is asking Talkback to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Intent {
    /// "What did I say about…", "did we decide…", "do you remember…".
    /// Answerable **only** from Relay's own data.
    PersonalMemory,
    /// "Start recording this as a voice note."
    StartVoiceNote,
    /// "Stop voice note."
    StopVoiceNote,
    /// "Turn this into a Scribble."
    CreateScribble,
    /// "Where did you get that?"
    ShowSources,
    /// Anything else. Retrieval still runs — the user's own context
    /// improves a general answer — but the model may also draw on what it
    /// knows. The default, because an unclassifiable turn must never fall
    /// into a stricter or more privileged mode by accident.
    #[default]
    General,
}

impl Intent {
    /// Whether a turn with this intent must be grounded in retrieved
    /// context or refused.
    pub fn requires_grounding(self) -> bool {
        matches!(self, Intent::PersonalMemory | Intent::ShowSources)
    }

    /// Whether this intent is executed by a tool rather than answered by
    /// the model.
    pub fn is_action(self) -> bool {
        matches!(
            self,
            Intent::StartVoiceNote | Intent::StopVoiceNote | Intent::CreateScribble
        )
    }
}

/// Phrases that mean "recall something from my own history".
///
/// Every one of these was chosen because the alternative — answering from
/// model knowledge — produces a confident, plausible, wrong answer about
/// the user's own life. That failure is worse than "I don't know".
const MEMORY_MARKERS: &[&str] = &[
    "what did i",
    "what did we",
    "what did he",
    "what did she",
    "what did they",
    "did i say",
    "did we say",
    "did we decide",
    "did we agree",
    "what was decided",
    "what did we decide",
    "do you remember",
    "do i have",
    "remind me what",
    "what happened in",
    "what happened at",
    "what happened during",
    "what was the last",
    "when did i",
    "when did we",
    "my notes",
    "my scribbles",
    "my voice notes",
    "my meetings",
    "last time",
    "in my vault",
    "have i ever",
    "did i ever",
    "what have i said",
    "catch me up",
    "summarize my",
    "summarise my",
    "who said",
    "what did the meeting",
];

const START_VOICE_NOTE_MARKERS: &[&str] = &[
    "start recording this as a voice note",
    "record this as a voice note",
    "start a voice note",
    "start voice note",
    "take a voice note",
    "record a voice note",
    "new voice note",
    "capture this as a voice note",
];

const STOP_VOICE_NOTE_MARKERS: &[&str] = &[
    "stop voice note",
    "stop the voice note",
    "end voice note",
    "end the voice note",
    "finish voice note",
    "save the voice note",
    "stop recording the voice note",
];

const SCRIBBLE_MARKERS: &[&str] = &[
    "turn this into a scribble",
    "make this a scribble",
    "save this as a scribble",
    "create a scribble",
    "make a scribble",
    "save that as a scribble",
    "turn that into a scribble",
    "scribble this",
];

const SOURCE_MARKERS: &[&str] = &[
    "where did you get that",
    "where did that come from",
    "what is your source",
    "what's your source",
    "which note",
    "show me the source",
    "show sources",
    "cite that",
    "how do you know that",
];

/// Words that mean the user is asking about a *time*, not just a topic.
/// A hit here makes the engine attach a `since` filter, because
/// similarity alone will happily return last March for "this week".
const TEMPORAL_MARKERS: &[(&str, i64)] = &[
    ("today", 1),
    ("yesterday", 2),
    ("this week", 7),
    ("last week", 14),
    ("past week", 7),
    ("recently", 30),
    ("lately", 30),
    ("this month", 31),
    ("last month", 62),
    ("this quarter", 92),
];

/// The routing decision for one turn.
#[derive(Debug, Clone, PartialEq)]
pub struct Routed {
    pub intent: Intent,
    /// How far back retrieval should look, in days, when the turn named a
    /// time window. `None` means no temporal constraint.
    pub lookback_days: Option<i64>,
}

fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch.is_alphanumeric() || ch.is_whitespace() || ch == '\'' {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(' ');
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn matches_any(haystack: &str, markers: &[&str]) -> bool {
    markers.iter().any(|m| haystack.contains(m))
}

/// Classifies a turn.
///
/// Order matters: explicit actions are checked before memory markers so
/// "save what we just decided as a scribble" creates a Scribble instead of
/// being answered as a recall question.
pub fn route(text: &str) -> Routed {
    let normalized = normalize(text);

    let lookback_days = TEMPORAL_MARKERS
        .iter()
        .filter(|(marker, _)| normalized.contains(marker))
        .map(|(_, days)| *days)
        .min();

    let intent = if matches_any(&normalized, STOP_VOICE_NOTE_MARKERS) {
        Intent::StopVoiceNote
    } else if matches_any(&normalized, START_VOICE_NOTE_MARKERS) {
        Intent::StartVoiceNote
    } else if matches_any(&normalized, SCRIBBLE_MARKERS) {
        Intent::CreateScribble
    } else if matches_any(&normalized, SOURCE_MARKERS) {
        Intent::ShowSources
    } else if matches_any(&normalized, MEMORY_MARKERS) {
        Intent::PersonalMemory
    } else {
        Intent::General
    };

    Routed {
        intent,
        lookback_days,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn intent_of(text: &str) -> Intent {
        route(text).intent
    }

    #[test]
    fn personal_memory_phrasings_all_route_to_memory() {
        for question in [
            "What did I say about the pricing model?",
            "What did we decide in the pricing review?",
            "Do you remember the procurement objection?",
            "What happened in Tuesday's standup?",
            "What was the last thing I said about hiring?",
            "Who said we should ship in March?",
            "Catch me up on the infra work",
            "Summarize my notes on onboarding",
            "Have I ever written about retention?",
            "Remind me what the deadline was",
        ] {
            assert_eq!(
                intent_of(question),
                Intent::PersonalMemory,
                "misrouted: {question}"
            );
        }
    }

    #[test]
    fn general_questions_are_not_forced_into_memory() {
        for question in [
            "What is a vector database?",
            "How do I write a Rust trait?",
            "Explain CRDTs simply",
            "What's the weather like in general terms",
        ] {
            assert_eq!(intent_of(question), Intent::General, "misrouted: {question}");
        }
    }

    #[test]
    fn voice_note_start_and_stop_are_distinguished() {
        assert_eq!(
            intent_of("Start recording this as a voice note"),
            Intent::StartVoiceNote
        );
        assert_eq!(intent_of("Take a voice note"), Intent::StartVoiceNote);
        assert_eq!(intent_of("Stop voice note"), Intent::StopVoiceNote);
        assert_eq!(intent_of("End the voice note please"), Intent::StopVoiceNote);
    }

    #[test]
    fn scribble_creation_beats_the_memory_reading() {
        // Contains "what we decided" but the ask is an action.
        assert_eq!(
            intent_of("Turn this into a scribble"),
            Intent::CreateScribble
        );
        assert_eq!(
            intent_of("Save that as a scribble, what we decided about pricing"),
            Intent::CreateScribble
        );
    }

    #[test]
    fn provenance_questions_route_to_sources() {
        assert_eq!(intent_of("Where did you get that?"), Intent::ShowSources);
        assert_eq!(intent_of("What's your source?"), Intent::ShowSources);
        assert_eq!(intent_of("How do you know that?"), Intent::ShowSources);
    }

    #[test]
    fn grounding_is_required_only_where_it_must_be() {
        assert!(Intent::PersonalMemory.requires_grounding());
        assert!(Intent::ShowSources.requires_grounding());
        assert!(!Intent::General.requires_grounding());
        assert!(!Intent::CreateScribble.requires_grounding());
    }

    #[test]
    fn actions_are_flagged_as_actions() {
        assert!(Intent::StartVoiceNote.is_action());
        assert!(Intent::StopVoiceNote.is_action());
        assert!(Intent::CreateScribble.is_action());
        assert!(!Intent::PersonalMemory.is_action());
        assert!(!Intent::General.is_action());
    }

    #[test]
    fn temporal_markers_produce_a_lookback() {
        assert_eq!(route("What did I say today?").lookback_days, Some(1));
        assert_eq!(route("What did we decide last week?").lookback_days, Some(14));
        assert_eq!(route("What have I said recently?").lookback_days, Some(30));
        assert_eq!(route("What did I say about pricing?").lookback_days, None);
    }

    #[test]
    fn the_tightest_window_wins_when_two_are_named() {
        assert_eq!(
            route("Between today and this month, what did we decide?").lookback_days,
            Some(1)
        );
    }

    #[test]
    fn punctuation_and_case_do_not_change_routing() {
        assert_eq!(
            intent_of("  WHAT DID WE DECIDE...?!  "),
            Intent::PersonalMemory
        );
        assert_eq!(
            intent_of("what's your source"),
            Intent::ShowSources,
            "apostrophes must survive normalization"
        );
    }

    #[test]
    fn empty_input_is_general_not_a_panic() {
        assert_eq!(intent_of(""), Intent::General);
        assert_eq!(intent_of("   "), Intent::General);
    }
}
