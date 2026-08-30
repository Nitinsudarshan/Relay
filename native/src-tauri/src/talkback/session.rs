//! The conversational session.
//!
//! **Ephemeral by default** (`docs/talkback/ARCHITECTURE.md` §1): a
//! Talkback conversation lives in memory for as long as it is switched on
//! and is then gone. Nothing here writes to the vault. Turning a
//! conversation into knowledge is an explicit user action, executed by
//! `tools.rs` through the *existing* Scribble and Voice Note persistence.
//!
//! What the session is actually for is reference resolution: "that",
//! "the second option", "what you just said" only mean anything against
//! the turns before them.

use super::intent::Intent;
use super::retrieval::ContextItem;
use serde::{Deserialize, Serialize};

/// How many turns are kept. Roughly ten exchanges — enough for "the
/// second option" to still resolve, short enough that history never eats
/// the context budget retrieval needs.
pub const MAX_TURNS: usize = 20;

/// How many recent turns are replayed to the model as conversation
/// history. Deliberately smaller than `MAX_TURNS`: the rest stays
/// available for provenance questions without being re-sent every turn.
pub const HISTORY_TURNS: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Agent,
}

/// One utterance in the conversation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TalkbackTurn {
    pub turn_id: String,
    pub role: Role,
    pub text: String,
    pub timestamp: String,
    /// Provenance for an agent turn; always empty for a user turn.
    #[serde(default)]
    pub sources: Vec<ContextItem>,
    #[serde(default)]
    pub intent: Option<Intent>,
    /// True when the turn arrived as text rather than speech. Kept so the
    /// UI can render the two differently and so latency metrics are not
    /// polluted by typed turns.
    #[serde(default)]
    pub typed: bool,
}

/// An in-memory conversation. Never serialized to disk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TalkbackSession {
    pub session_id: String,
    pub started_at: String,
    pub turns: Vec<TalkbackTurn>,
    /// Set while a "record this as a voice note" capture is running, so a
    /// later "stop voice note" knows what it is stopping.
    #[serde(default)]
    pub voice_note_buffer: Option<String>,
}

impl TalkbackSession {
    pub fn new() -> Self {
        Self {
            session_id: format!("talkback_{}", uuid::Uuid::new_v4()),
            started_at: chrono::Utc::now().to_rfc3339(),
            turns: Vec::new(),
            voice_note_buffer: None,
        }
    }

    /// Appends a turn, evicting the oldest once `MAX_TURNS` is reached.
    pub fn push(&mut self, turn: TalkbackTurn) {
        self.turns.push(turn);
        if self.turns.len() > MAX_TURNS {
            let overflow = self.turns.len() - MAX_TURNS;
            self.turns.drain(0..overflow);
        }
    }

    pub fn push_user(&mut self, text: &str, intent: Intent, typed: bool) -> String {
        let turn_id = format!("turn_{}", uuid::Uuid::new_v4());
        self.push(TalkbackTurn {
            turn_id: turn_id.clone(),
            role: Role::User,
            text: text.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            sources: Vec::new(),
            intent: Some(intent),
            typed,
        });
        turn_id
    }

    pub fn push_agent(&mut self, text: &str, sources: Vec<ContextItem>) -> String {
        let turn_id = format!("turn_{}", uuid::Uuid::new_v4());
        self.push(TalkbackTurn {
            turn_id: turn_id.clone(),
            role: Role::Agent,
            text: text.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            sources,
            intent: None,
            typed: false,
        });
        turn_id
    }

    /// The most recent turns, oldest first, for replay as conversation
    /// history.
    pub fn recent(&self, count: usize) -> &[TalkbackTurn] {
        let start = self.turns.len().saturating_sub(count);
        &self.turns[start..]
    }

    /// Sources cited by the most recent agent turn — the answer to
    /// "where did you get that?" without another retrieval pass.
    pub fn last_sources(&self) -> &[ContextItem] {
        self.turns
            .iter()
            .rev()
            .find(|t| t.role == Role::Agent && !t.sources.is_empty())
            .map(|t| t.sources.as_slice())
            .unwrap_or(&[])
    }

    /// The conversation as plain text, for the Scribble tool.
    ///
    /// This is the *only* path from a conversation to persisted
    /// knowledge, and it runs only when the user asks for it.
    pub fn transcript(&self) -> String {
        self.turns
            .iter()
            .map(|t| {
                let speaker = match t.role {
                    Role::User => "You",
                    Role::Agent => "Relay",
                };
                format!("{}: {}", speaker, t.text.trim())
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// True while a conversational voice-note capture is open.
    pub fn is_capturing_voice_note(&self) -> bool {
        self.voice_note_buffer.is_some()
    }
}

impl Default for TalkbackSession {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::talkback::retrieval::SourceType;

    fn source(id: &str) -> ContextItem {
        ContextItem {
            source_type: SourceType::Scribble,
            source_id: id.to_string(),
            title: "Pricing".to_string(),
            timestamp: "2026-08-01T00:00:00Z".to_string(),
            relevance: 1.0,
            excerpt: "flat seat licence".to_string(),
            detail: None,
            expanded: false,
        }
    }

    #[test]
    fn turns_accumulate_in_order() {
        let mut session = TalkbackSession::new();
        session.push_user("what did we decide", Intent::PersonalMemory, true);
        session.push_agent("A flat seat licence.", vec![source("s1")]);
        assert_eq!(session.turns.len(), 2);
        assert_eq!(session.turns[0].role, Role::User);
        assert_eq!(session.turns[1].role, Role::Agent);
        assert!(session.turns[0].typed);
    }

    #[test]
    fn history_is_capped_and_evicts_the_oldest() {
        let mut session = TalkbackSession::new();
        for i in 0..(MAX_TURNS + 5) {
            session.push_user(&format!("turn {i}"), Intent::General, true);
        }
        assert_eq!(session.turns.len(), MAX_TURNS);
        assert_eq!(session.turns[0].text, "turn 5");
    }

    #[test]
    fn recent_returns_the_tail_oldest_first() {
        let mut session = TalkbackSession::new();
        for i in 0..10 {
            session.push_user(&format!("turn {i}"), Intent::General, true);
        }
        let recent = session.recent(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].text, "turn 7");
        assert_eq!(recent[2].text, "turn 9");
    }

    #[test]
    fn recent_handles_a_shorter_conversation_than_requested() {
        let mut session = TalkbackSession::new();
        session.push_user("only one", Intent::General, true);
        assert_eq!(session.recent(10).len(), 1);
        assert!(TalkbackSession::new().recent(10).is_empty());
    }

    #[test]
    fn last_sources_finds_the_most_recent_cited_answer() {
        let mut session = TalkbackSession::new();
        session.push_agent("first", vec![source("older")]);
        session.push_user("and?", Intent::General, true);
        session.push_agent("second", vec![source("newer")]);
        assert_eq!(session.last_sources()[0].source_id, "newer");
    }

    #[test]
    fn last_sources_skips_uncited_answers() {
        let mut session = TalkbackSession::new();
        session.push_agent("cited", vec![source("s1")]);
        session.push_agent("uncited", vec![]);
        assert_eq!(session.last_sources()[0].source_id, "s1");
    }

    #[test]
    fn last_sources_is_empty_for_a_fresh_session() {
        assert!(TalkbackSession::new().last_sources().is_empty());
    }

    #[test]
    fn transcript_labels_both_speakers() {
        let mut session = TalkbackSession::new();
        session.push_user("what did we decide", Intent::PersonalMemory, true);
        session.push_agent("A flat seat licence.", vec![]);
        let transcript = session.transcript();
        assert!(transcript.contains("You: what did we decide"));
        assert!(transcript.contains("Relay: A flat seat licence."));
    }

    #[test]
    fn a_new_session_holds_no_voice_note_capture() {
        let session = TalkbackSession::new();
        assert!(!session.is_capturing_voice_note());
        assert!(session.turns.is_empty());
        assert!(session.session_id.starts_with("talkback_"));
    }

    #[test]
    fn sessions_get_distinct_ids() {
        assert_ne!(
            TalkbackSession::new().session_id,
            TalkbackSession::new().session_id
        );
    }
}
