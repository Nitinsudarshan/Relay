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
}
