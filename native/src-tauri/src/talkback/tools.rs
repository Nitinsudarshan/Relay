//! The action layer.
//!
//! Talkback can create a Voice Note and a Scribble, and look things up.
//! It does all three through the **existing** persistence — there is no
//! Talkback note type, no Talkback Scribble schema, no Talkback vector
//! store (`ARCHITECTURE.md` §12). A Scribble made by talking is
//! indistinguishable from one made by typing, except for its
//! `source_metadata`.
//!
//! ## What is deliberately absent
//!
//! No tool here sends, deletes, or modifies anything outside Relay. Four
//! read/create tools ship; `send_email`, `delete_*` and calendar writes
//! are not merely unimplemented but excluded by design until there is a
//! confirmation step to protect them.

use super::retrieval::ContextItem;
use super::session::TalkbackSession;
use crate::vault::{Scribble, VaultManager, VaultNote};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ToolError {
    #[error("Vault operation failed: {0}")]
    Vault(String),

    #[error("{0}")]
    NotApplicable(String),
}

/// The tools Talkback is allowed to run.
///
/// An enum rather than a string registry because the set is small,
/// closed, and safety-relevant: adding a destructive tool should require
/// editing this type and everything that matches on it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tool", rename_all = "snake_case")]
pub enum Tool {
    /// Open a conversational Voice Note capture.
    CreateVoiceNote,
    /// Close it and persist what was said.
    StopVoiceNote,
    /// Turn the conversation so far into a Scribble.
    CreateScribble,
    /// Retrieve from Relay's own knowledge. Runs implicitly on most
    /// turns; named here because the model may also ask for it.
    SearchMemory { query: String },
}

/// What a tool did, in the words Talkback will say out loud.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolOutcome {
    /// The spoken confirmation. Deterministic — a model should not be
    /// given the chance to say "saved" about something that failed.
    pub message: String,
    /// The vault id created, when one was.
    #[serde(default)]
    pub created_id: Option<String>,
    #[serde(default)]
    pub sources: Vec<ContextItem>,
}

impl ToolOutcome {
    fn spoken(message: &str) -> Self {
        Self {
            message: message.to_string(),
            created_id: None,
            sources: Vec::new(),
        }
    }
}

/// Opens a conversational Voice Note capture.
///
/// The capture itself is the ordinary microphone path; this only marks
/// the session so the next turns are collected rather than answered.
pub fn start_voice_note(session: &mut TalkbackSession) -> Result<ToolOutcome, ToolError> {
    if session.is_capturing_voice_note() {
        return Err(ToolError::NotApplicable(
            "You're already recording a voice note. Say \"stop voice note\" when you're done."
                .to_string(),
        ));
    }
    session.voice_note_buffer = Some(String::new());
    Ok(ToolOutcome::spoken(
        "Recording a voice note. Say \"stop voice note\" when you're done.",
    ))
}

/// Appends a spoken turn to the open Voice Note capture.
///
/// Returns `true` when the text was absorbed, which is how the engine
/// knows not to answer that turn.
pub fn append_to_voice_note(session: &mut TalkbackSession, text: &str) -> bool {
    let Some(buffer) = session.voice_note_buffer.as_mut() else {
        return false;
    };
    if !buffer.is_empty() {
        buffer.push(' ');
    }
    buffer.push_str(text.trim());
    true
}

/// Closes the capture and persists it as an ordinary Voice Note.
///
/// Uses `VaultNote::new_voice_note` — the same constructor the dictation
/// hotkey uses — so a Voice Note dictated through Talkback is byte-for-byte
/// the same kind of thing as one dictated through the pill. It is
/// explicitly **not** a Scribble: those are structured, LLM-cleaned
/// artifacts, and a dictation history that had been rewritten would stop
/// being a truthful record.
pub fn stop_voice_note(
    session: &mut TalkbackSession,
    vault: &VaultManager,
) -> Result<(ToolOutcome, Option<VaultNote>), ToolError> {
    let Some(buffer) = session.voice_note_buffer.take() else {
        return Err(ToolError::NotApplicable(
            "There's no voice note recording right now.".to_string(),
        ));
    };

    let transcript = buffer.trim();
    if transcript.is_empty() {
        return Ok((
            ToolOutcome::spoken("I didn't catch anything, so I didn't save a voice note."),
            None,
        ));
    }

    let note = VaultNote::new_voice_note(transcript);
    vault
        .save_note(&note)
        .map_err(|e| ToolError::Vault(e.to_string()))?;

    Ok((
        ToolOutcome {
            message: "Saved that as a voice note.".to_string(),
            created_id: Some(note.id.clone()),
            sources: Vec::new(),
        },
        Some(note),
    ))
}

/// Marks a Scribble as having come from a Talkback conversation.
///
/// Uses the existing `source_metadata` free-form field rather than a new
/// column, so nothing downstream — the graph, enrichment, merge, the
/// Obsidian export — has to learn about Talkback to keep working.
pub const SCRIBBLE_SOURCE: &str = "talkback";

/// Turns the conversation into a Scribble.
///
/// The whole exchange is the content, because in a conversation the
/// user's questions carry as much intent as the answers. Enrichment
/// (title, topics, entities, relationships) is the existing pipeline's
/// job — this leaves `enrichment_status` at the `pending` its constructor
/// sets, exactly as a typed Scribble does.
pub fn create_scribble(
    session: &TalkbackSession,
    vault: &VaultManager,
    title: Option<&str>,
) -> Result<(ToolOutcome, Option<Scribble>), ToolError> {
    let transcript = session.transcript();
    if transcript.trim().is_empty() {
        return Ok((
            ToolOutcome::spoken("There's nothing in this conversation to save yet."),
            None,
        ));
    }

    let mut scribble = Scribble::new_text(&transcript, title);
    scribble.source_metadata = serde_json::json!({
        "source": SCRIBBLE_SOURCE,
        "session_id": session.session_id,
        "turn_count": session.turns.len(),
    });

    vault
        .save_scribble(&scribble)
        .map_err(|e| ToolError::Vault(e.to_string()))?;

    Ok((
        ToolOutcome {
            message: "Saved this conversation as a Scribble.".to_string(),
            created_id: Some(scribble.id.clone()),
            sources: Vec::new(),
        },
        Some(scribble),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::talkback::intent::Intent;

    /// A vault rooted in a fresh temp directory, matching the pattern the
    /// rest of the crate's vault tests use (no extra dev-dependency).
    fn temp_vault() -> (VaultManager, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("relay_talkback_test_{}", uuid::Uuid::new_v4()));
        let vault = VaultManager::new(dir.clone());
        vault.init().expect("vault init");
        (vault, dir)
    }

    fn conversation() -> TalkbackSession {
        let mut session = TalkbackSession::new();
        session.push_user("what did we decide about pricing", Intent::PersonalMemory, true);
        session.push_agent("You settled on a flat seat licence.", vec![]);
        session
    }

    #[test]
    fn a_voice_note_capture_opens_and_absorbs_turns() {
        let mut session = TalkbackSession::new();
        let outcome = start_voice_note(&mut session).unwrap();
        assert!(outcome.message.contains("Recording a voice note"));
        assert!(session.is_capturing_voice_note());

        assert!(append_to_voice_note(&mut session, "first thought"));
        assert!(append_to_voice_note(&mut session, "second thought"));
        assert_eq!(
            session.voice_note_buffer.as_deref(),
            Some("first thought second thought")
        );
    }

    #[test]
    fn turns_are_not_absorbed_when_no_capture_is_open() {
        let mut session = TalkbackSession::new();
        assert!(!append_to_voice_note(&mut session, "just a question"));
    }

    #[test]
    fn opening_a_second_capture_is_refused_with_an_explanation() {
        let mut session = TalkbackSession::new();
        start_voice_note(&mut session).unwrap();
        let err = start_voice_note(&mut session).unwrap_err();
        assert!(matches!(err, ToolError::NotApplicable(_)));
        assert!(err.to_string().contains("already recording"));
    }

    #[test]
    fn stopping_persists_a_voice_note_not_a_scribble() {
        let (vault, _dir) = temp_vault();
        let mut session = TalkbackSession::new();
        start_voice_note(&mut session).unwrap();
        append_to_voice_note(&mut session, "remember to renew the domain");

        let (outcome, note) = stop_voice_note(&mut session, &vault).unwrap();
        assert_eq!(outcome.message, "Saved that as a voice note.");
        let note = note.expect("a note was created");
        assert_eq!(note.note_type, crate::vault::VOICE_NOTE_TYPE);
        assert_eq!(note.content, "remember to renew the domain");
        assert!(!session.is_capturing_voice_note());

        // Persisted, and visible to the same listing the Voice Note page uses.
        let stored = vault
            .list_notes_by_type(crate::vault::VOICE_NOTE_TYPE)
            .unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].content, "remember to renew the domain");
        assert!(vault.list_scribbles().unwrap().is_empty(), "must not create a Scribble");
    }

    #[test]
    fn the_transcript_is_stored_verbatim() {
        let (vault, _dir) = temp_vault();
        let mut session = TalkbackSession::new();
        start_voice_note(&mut session).unwrap();
        append_to_voice_note(&mut session, "um so the thing is, we probably want it");

        let (_, note) = stop_voice_note(&mut session, &vault).unwrap();
        assert_eq!(
            note.unwrap().content,
            "um so the thing is, we probably want it",
            "a Voice Note is a truthful dictation history, never a rewrite"
        );
    }

    #[test]
    fn stopping_an_empty_capture_saves_nothing_and_says_so() {
        let (vault, _dir) = temp_vault();
        let mut session = TalkbackSession::new();
        start_voice_note(&mut session).unwrap();

        let (outcome, note) = stop_voice_note(&mut session, &vault).unwrap();
        assert!(note.is_none());
        assert!(outcome.message.contains("didn't catch anything"));
        assert!(vault
            .list_notes_by_type(crate::vault::VOICE_NOTE_TYPE)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn stopping_without_a_capture_is_refused() {
        let (vault, _dir) = temp_vault();
        let mut session = TalkbackSession::new();
        let err = stop_voice_note(&mut session, &vault).unwrap_err();
        assert!(err.to_string().contains("no voice note recording"));
    }

    #[test]
    fn a_scribble_captures_the_whole_exchange() {
        let (vault, _dir) = temp_vault();
        let session = conversation();

        let (outcome, scribble) = create_scribble(&session, &vault, None).unwrap();
        assert_eq!(outcome.message, "Saved this conversation as a Scribble.");
        let scribble = scribble.expect("a scribble was created");
        assert!(scribble.content.contains("You: what did we decide about pricing"));
        assert!(scribble.content.contains("Relay: You settled on a flat seat licence."));
        assert_eq!(outcome.created_id.as_deref(), Some(scribble.id.as_str()));
    }

    #[test]
    fn a_talkback_scribble_carries_its_provenance() {
        let (vault, _dir) = temp_vault();
        let session = conversation();
        let (_, scribble) = create_scribble(&session, &vault, None).unwrap();
        let scribble = scribble.unwrap();
        assert_eq!(scribble.source_metadata["source"], SCRIBBLE_SOURCE);
        assert_eq!(scribble.source_metadata["session_id"], session.session_id);
        assert_eq!(scribble.source_metadata["turn_count"], 2);
    }

    #[test]
    fn a_talkback_scribble_uses_the_existing_schema_and_enrichment_queue() {
        let (vault, _dir) = temp_vault();
        let (_, scribble) = create_scribble(&conversation(), &vault, None).unwrap();
        let scribble = scribble.unwrap();
        assert!(scribble.id.starts_with("scribble_"));
        assert_eq!(scribble.status, "active");
        assert_eq!(
            scribble.ai_metadata.enrichment_status, "pending",
            "the existing enrichment pipeline must pick this up unchanged"
        );

        let stored = vault.get_scribble(&scribble.id).unwrap();
        assert_eq!(stored.id, scribble.id);
    }

    #[test]
    fn an_explicit_title_is_honoured() {
        let (vault, _dir) = temp_vault();
        let (_, scribble) = create_scribble(&conversation(), &vault, Some("Pricing call")).unwrap();
        assert_eq!(scribble.unwrap().title, "Pricing call");
    }

    #[test]
    fn an_empty_conversation_produces_no_scribble() {
        let (vault, _dir) = temp_vault();
        let session = TalkbackSession::new();
        let (outcome, scribble) = create_scribble(&session, &vault, None).unwrap();
        assert!(scribble.is_none());
        assert!(outcome.message.contains("nothing in this conversation"));
        assert!(vault.list_scribbles().unwrap().is_empty());
    }
}
