//! Correcting a phrase inside a saved note.
//!
//! The whole operation is a deterministic range edit. No model is involved and
//! none should be: the user has already said what the text should be, and
//! regenerating a note through an LLM to perform a substitution would risk
//! changing everything they did not ask about.
//!
//! # Why a range and not a search
//!
//! "Replace the selected occurrence" and "replace every occurrence" are
//! different operations, and a note that says "I opened super base, then super
//! base crashed" makes the difference obvious. The caller sends the character
//! range the user actually selected, so the second occurrence is untouched.
//!
//! # Why the expected text travels with it
//!
//! An offset is only meaningful against the exact content it was measured on.
//! If the note changed between the selection and the save — the full editor was
//! used in another window, a merge landed — those offsets now point somewhere
//! else, and applying them would corrupt text at random. The caller sends what
//! it believes is there and the edit refuses if reality disagrees.
//!
//! # Markdown
//!
//! Nothing here parses Markdown, which is what keeps it safe. The note is
//! stored as Markdown text and the edit is a substring replacement within that
//! text, so a heading stays a heading and a list stays a list — the syntax is
//! never converted to a document model and back.
//!
//! # Why the history is a range and not a snapshot
//!
//! [`CorrectionRecord`] stores the two phrases and where they were, not the
//! note before and after. A range edit is its own inverse — put `original`
//! back where `replacement` now sits — so reversal needs nothing more, and
//! storing whole copies of the note per correction would be a versioning
//! system, which this deliberately is not.
//!
//! Reversing through the range also fails safe. A snapshot undo applied after
//! the full editor touched the note would throw that edit away; reversing the
//! range asks whether `replacement` is still where it was left and refuses
//! when it is not.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Why a correction could not be applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorrectionError {
    /// The range falls outside the note, or splits a character.
    OutOfRange { start: usize, end: usize, len: usize },
    /// The note no longer reads the way the caller believed.
    Stale { expected: String, found: String },
    /// The range selects nothing.
    Empty,
}

impl fmt::Display for CorrectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfRange { start, end, len } => write!(
                f,
                "selection {start}..{end} is outside this note (length {len})"
            ),
            Self::Stale { expected, found } => write!(
                f,
                "this note has changed since the text was selected — expected {expected:?}, found {found:?}"
            ),
            Self::Empty => write!(f, "nothing was selected"),
        }
    }
}

/// Replaces `content[start..end]` with `replacement`, having checked that the
/// range still holds `expected`.
///
/// Offsets are **character** indices, not bytes: they come from a browser
/// selection, where `"मैं".length` and Rust's `str` byte length disagree, and
/// resolving that here is the difference between a correct edit and one that
/// panics on the user's own language.
pub fn replace_range(
    content: &str,
    start: usize,
    end: usize,
    expected: &str,
    replacement: &str,
) -> Result<String, CorrectionError> {
    if end <= start {
        return Err(CorrectionError::Empty);
    }

    let char_count = content.chars().count();
    if end > char_count {
        return Err(CorrectionError::OutOfRange {
            start,
            end,
            len: char_count,
        });
    }

    let byte_start = char_to_byte(content, start);
    let byte_end = char_to_byte(content, end);
    let found = &content[byte_start..byte_end];
    if found != expected {
        return Err(CorrectionError::Stale {
            expected: expected.to_string(),
            found: found.to_string(),
        });
    }

    let mut out = String::with_capacity(content.len() - found.len() + replacement.len());
    out.push_str(&content[..byte_start]);
    out.push_str(replacement);
    out.push_str(&content[byte_end..]);
    Ok(out)
}

/// The byte offset of the `index`-th character.
fn char_to_byte(text: &str, index: usize) -> usize {
    text.char_indices()
        .nth(index)
        .map(|(byte, _)| byte)
        .unwrap_or(text.len())
}

/// One applied correction, kept so it can be reversed and seen.
///
/// The four things the record has to answer are what was wrong, what it became,
/// when, and whether the user also asked Relay to learn it — that last one
/// being the difference between an ordinary edit and a standing rule, and not
/// recoverable from the note itself afterwards.
///
/// `start` is a character offset, matching [`replace_range`]. It is what makes
/// [`reverse`](CorrectionRecord::reverse) possible without storing a copy of
/// the note.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CorrectionRecord {
    pub id: String,
    pub note_id: String,
    /// The phrase as the recognizer wrote it.
    pub original: String,
    /// The phrase as the user says it should read.
    pub replacement: String,
    /// Character offset of the replacement within the note it was applied to.
    pub start: usize,
    pub corrected_at: String,
    /// Whether the user ticked "Teach Relay this correction". A learned
    /// correction also exists as a `settings::VocabularyCorrection`; this is
    /// the record that *this* note is where it came from.
    pub learned: bool,
}

impl CorrectionRecord {
    pub fn new(note_id: &str, original: &str, replacement: &str, start: usize, learned: bool) -> Self {
        Self {
            id: format!("corr_{}", uuid::Uuid::new_v4()),
            note_id: note_id.to_string(),
            original: original.to_string(),
            replacement: replacement.to_string(),
            start,
            corrected_at: chrono::Utc::now().to_rfc3339(),
            learned,
        }
    }

    /// Puts the original phrase back, if the replacement is still where this
    /// record left it.
    ///
    /// Refuses otherwise rather than guessing: the note having moved on is the
    /// case where an undo would destroy work the user did after correcting.
    pub fn reverse(&self, content: &str) -> Result<String, CorrectionError> {
        let end = self.start + self.replacement.chars().count();
        replace_range(content, self.start, end, &self.replacement, &self.original)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The character range of the first occurrence of `needle`.
    fn range_of(text: &str, needle: &str) -> (usize, usize) {
        let byte = text.find(needle).expect("needle present");
        let start = text[..byte].chars().count();
        (start, start + needle.chars().count())
    }

    fn correct(text: &str, needle: &str, replacement: &str) -> String {
        let (start, end) = range_of(text, needle);
        replace_range(text, start, end, needle, replacement).expect("applies")
    }

    #[test]
    fn a_single_word_is_replaced() {
        assert_eq!(correct("I tested ollama today", "ollama", "Ollama"), "I tested Ollama today");
    }

    #[test]
    fn a_multi_word_phrase_is_replaced() {
        assert_eq!(
            correct("I was testing super base yesterday.", "super base", "Supabase"),
            "I was testing Supabase yesterday."
        );
    }

    #[test]
    fn only_the_selected_occurrence_changes() {
        // The requirement that makes this a range edit rather than a search.
        let text = "I was testing super base yesterday and then opened super base again.";
        assert_eq!(
            correct(text, "super base", "Supabase"),
            "I was testing Supabase yesterday and then opened super base again."
        );
    }

    #[test]
    fn the_second_occurrence_can_be_chosen_instead() {
        let text = "opened super base, then super base crashed";
        let first = text.find("super base").unwrap();
        let byte = text[first + 1..].find("super base").unwrap() + first + 1;
        let start = text[..byte].chars().count();
        let out = replace_range(text, start, start + 10, "super base", "Supabase").unwrap();
        assert_eq!(out, "opened super base, then Supabase crashed");
    }

    #[test]
    fn a_phrase_at_the_very_start_is_replaced() {
        assert_eq!(correct("super base is fast", "super base", "Supabase"), "Supabase is fast");
    }

    #[test]
    fn a_phrase_at_the_very_end_is_replaced() {
        assert_eq!(correct("we should try super base", "super base", "Supabase"), "we should try Supabase");
    }

    #[test]
    fn punctuation_inside_the_selection_is_respected() {
        assert_eq!(correct("call it lance-db, please", "lance-db,", "LanceDB,"), "call it LanceDB, please");
    }

    #[test]
    fn markdown_structure_survives() {
        // Nothing here parses Markdown, which is precisely why the syntax is
        // safe: a heading is text, and only the selected span changes.
        let note = "# Standup\n\n- tested super base\n- **bold** and `code`\n\n> quoted\n";
        let out = correct(note, "super base", "Supabase");
        assert_eq!(out, "# Standup\n\n- tested Supabase\n- **bold** and `code`\n\n> quoted\n");
    }

    #[test]
    fn a_selection_inside_inline_code_does_not_break_the_fence() {
        let note = "run `ollama serve` first";
        assert_eq!(correct(note, "ollama", "Ollama"), "run `Ollama serve` first");
    }

    #[test]
    fn a_stale_offset_is_refused_rather_than_corrupting_the_note() {
        // The note was edited elsewhere between selection and save. Applying
        // the old offsets would replace text at random.
        let err = replace_range("a completely different note", 0, 10, "super base", "Supabase");
        assert!(matches!(err, Err(CorrectionError::Stale { .. })), "{err:?}");
    }

    #[test]
    fn a_range_past_the_end_is_refused() {
        let err = replace_range("short", 0, 99, "short", "long");
        assert!(matches!(err, Err(CorrectionError::OutOfRange { .. })), "{err:?}");
    }

    #[test]
    fn an_empty_selection_is_refused() {
        assert_eq!(replace_range("text", 2, 2, "", "x"), Err(CorrectionError::Empty));
    }

    #[test]
    fn offsets_are_characters_so_devanagari_does_not_panic() {
        // A browser selection counts UTF-16 code units over characters, not
        // Rust bytes. Treating these offsets as bytes would slice mid-character
        // and panic on the user's own language.
        let note = "मैं super base चला रहा हूं";
        let (start, end) = range_of(note, "super base");
        let out = replace_range(note, start, end, "super base", "Supabase").unwrap();
        assert_eq!(out, "मैं Supabase चला रहा हूं");
    }

    #[test]
    fn a_devanagari_phrase_can_itself_be_corrected() {
        let note = "कल मीटिंग है";
        let (start, end) = range_of(note, "मीटिंग");
        let out = replace_range(note, start, end, "मीटिंग", "meeting").unwrap();
        assert_eq!(out, "कल meeting है");
    }

    /// A record, and the correction it describes, applied together — which is
    /// how the command uses them.
    fn apply_and_record(note: &str, needle: &str, replacement: &str, learned: bool) -> (String, CorrectionRecord) {
        let (start, end) = range_of(note, needle);
        let corrected = replace_range(note, start, end, needle, replacement).expect("applies");
        (
            corrected,
            CorrectionRecord::new("note_1", needle, replacement, start, learned),
        )
    }

    #[test]
    fn reversing_a_record_restores_the_previous_content() {
        let before = "I was testing super base yesterday.";
        let (after, record) = apply_and_record(before, "super base", "Supabase", false);
        assert_eq!(record.reverse(&after).unwrap(), before);
    }

    #[test]
    fn reversing_touches_only_the_occurrence_that_was_corrected() {
        let before = "opened super base, then super base crashed";
        let (after, record) = apply_and_record(before, "super base", "Supabase", false);
        assert_eq!(after, "opened Supabase, then super base crashed");
        assert_eq!(record.reverse(&after).unwrap(), before);
    }

    #[test]
    fn reversing_after_the_note_moved_on_is_refused_rather_than_destroying_the_edit() {
        // The full editor rewrote the note after the correction. A snapshot
        // undo would throw that rewrite away; reversing the range asks first.
        let (_, record) = apply_and_record("I was testing super base yesterday.", "super base", "Supabase", false);
        let rewritten = "Completely different notes now.";
        assert!(
            matches!(record.reverse(rewritten), Err(CorrectionError::Stale { .. })),
            "a moved-on note must refuse the reversal"
        );
    }

    #[test]
    fn a_record_remembers_whether_the_correction_was_taught() {
        // Not recoverable from the note afterwards, and it is the difference
        // between an ordinary edit and a standing rule.
        let (_, taught) = apply_and_record("tested super base", "super base", "Supabase", true);
        let (_, ordinary) = apply_and_record("meeting on Thursday", "Thursday", "Tuesday", false);
        assert!(taught.learned);
        assert!(!ordinary.learned);
        assert!(!taught.corrected_at.is_empty(), "a timestamp is part of the record");
        assert_ne!(taught.id, ordinary.id, "records are individually addressable");
    }

    #[test]
    fn reversing_a_multibyte_correction_uses_character_offsets() {
        let before = "मैं super base चला रहा हूं";
        let (after, record) = apply_and_record(before, "super base", "Supabase", false);
        assert_eq!(after, "मैं Supabase चला रहा हूं");
        assert_eq!(record.reverse(&after).unwrap(), before);
    }
}
