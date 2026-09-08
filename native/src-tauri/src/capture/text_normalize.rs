//! The deterministic text-cleanup rules, shared by every surface that turns
//! speech into text.
//!
//! These were written for meeting transcripts and were reachable only from the
//! meeting pipeline, so dictation and Talkback shipped raw Whisper output while
//! meetings got glossary repair, repetition collapsing and sentence casing.
//! Nothing here is meeting-specific: a decoder stutter is a decoder stutter
//! wherever it is decoded, and a mistranscribed project name is worth fixing in
//! a dictated sentence too.
//!
//! Every rule is deterministic and reports itself. No model runs here, so
//! nothing in this module can invent a word that was not spoken — which is what
//! makes it safe to apply everywhere and without asking. The rules that *can*
//! change meaning belong to the opt-in rewrite layer, not to this one.
//!
//! Each function names the rule it applied, so a misbehaving rule is visible in
//! the artifact rather than only in the output.

/// Rule names, recorded per segment and counted per transcript so a misbehaving
/// rule is visible without re-running anything.
pub const RULE_ASR_TAGS: &str = "asr_tags_removed";
pub const RULE_WHITESPACE: &str = "whitespace_normalized";
pub const RULE_REPEATED_WORDS: &str = "repeated_words_collapsed";
pub const RULE_REPEATED_PHRASES: &str = "repeated_phrases_collapsed";
pub const RULE_FILLERS: &str = "isolated_fillers_removed";
pub const RULE_GLOSSARY: &str = "glossary_terms_corrected";
pub const RULE_LEARNED_CORRECTIONS: &str = "learned_corrections_applied";
pub const RULE_SENTENCE_BOUNDARIES: &str = "sentence_boundaries_repaired";

/// Standalone filler tokens. Removed only when they stand alone as a whole
/// token, never as a substring — "um" must not eat the "um" in "umbrella", and
/// "so" is not in this list because it routinely carries meaning.
const ISOLATED_FILLERS: &[&str] = &[
    "um", "umm", "uh", "uhh", "erm", "ah", "ahh", "eh", "hmm", "mmm", "mm", "uhm", "er",
];

/// Longest repeated phrase length (in words) the loop detector looks for.
/// Whisper's repetition loops can span full sentences (up to 16 words);
/// searching up to 16 catches full sentence loops while preserving single-occurrence speech.
const MAX_PHRASE_REPEAT_LEN: usize = 16;

/// Minimum token length before a glossary term is matched by edit distance
/// rather than exactly. Short tokens are far too easy to "correct" into
/// something the speaker never said.
const MIN_FUZZY_GLOSSARY_LEN: usize = 6;

pub struct SegmentOutcome {
    pub text: String,
    pub applied_rules: Vec<String>,
}

/// Whether `c` belongs inside a word rather than between two words.
///
/// Unicode's `Alphabetic` property — which is what `char::is_alphanumeric`
/// consults — covers Devanagari vowel signs, so `दूंगी` tokenizes as one word.
/// It does *not* cover the virama, which suppresses a consonant's inherent
/// vowel and is therefore word-**internal**: it is how `क्या` and `फॉर्म` are
/// spelled. Splitting on it turns every consonant cluster into two words, and
/// consonant clusters are most Hindi words.
///
/// That mattered concretely: the action-item gate matches whole words against
/// its lexicons, so a Hindi commitment could not have matched even with the
/// vocabulary added, because the words it was matching against were fragments.
///
/// The zero-width joiners are here for the same reason — they control ligature
/// shaping within a word and carry no boundary.
pub fn is_word_internal(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '\u{094D}' | '\u{093C}' | '\u{200C}' | '\u{200D}')
}

/// The words this normalizer knows about.
///
/// A struct rather than two more parameters because the two halves answer
/// different questions and are easy to transpose: `glossary` is a list of
/// canonical *words* matched by token and edit distance, and `corrections` are
/// whole-*phrase* repairs of forms the recognizer keeps producing. Priming and
/// repair are not the same mechanism and neither replaces the other.
#[derive(Debug, Clone, Copy, Default)]
pub struct Vocabulary<'a> {
    pub glossary: &'a [String],
    pub corrections: &'a [crate::settings::VocabularyCorrection],
}

impl<'a> Vocabulary<'a> {
    pub fn new(
        glossary: &'a [String],
        corrections: &'a [crate::settings::VocabularyCorrection],
    ) -> Self {
        Self {
            glossary,
            corrections,
        }
    }

    /// For a caller that has no learned corrections to apply.
    pub fn glossary_only(glossary: &'a [String]) -> Self {
        Self {
            glossary,
            corrections: &[],
        }
    }
}

/// Replaces phrases the user has taught Relay to repair.
///
/// Case-insensitive and bounded by word edges, so "super base" in "super
/// basement" is left alone. The replacement is written exactly as the user
/// typed it — the whole point is that they chose the capitalisation.
///
/// This cannot make Whisper *hear* differently; it repairs what Whisper wrote.
/// The dictionary handles the other half by priming the recognizer before it
/// guesses.
pub fn apply_learned_corrections(
    text: &str,
    corrections: &[crate::settings::VocabularyCorrection],
) -> String {
    let mut out = text.to_string();
    for correction in corrections
        .iter()
        .filter(|c| c.enabled && c.is_meaningful())
    {
        out = replace_phrase_ignoring_case(&out, correction.source.trim(), &correction.replacement);
    }
    out
}

/// Every whole-word occurrence of `needle`, replaced.
fn replace_phrase_ignoring_case(haystack: &str, needle: &str, replacement: &str) -> String {
    if needle.is_empty() {
        return haystack.to_string();
    }
    let lower_hay = haystack.to_lowercase();
    let lower_needle = needle.to_lowercase();

    let mut out = String::with_capacity(haystack.len());
    let mut cursor = 0usize;
    // Lowercasing can change byte lengths for some scripts, which would make
    // offsets from the lowered string wrong against the original. Fall back to
    // leaving the text alone rather than slicing at a bad index.
    if lower_hay.len() != haystack.len() {
        return haystack.to_string();
    }

    while let Some(found) = lower_hay[cursor..].find(&lower_needle) {
        let start = cursor + found;
        let end = start + lower_needle.len();
        let bounded = !preceded_by_word_char(haystack, start) && !followed_by_word_char(haystack, end);
        out.push_str(&haystack[cursor..start]);
        if bounded {
            out.push_str(replacement);
        } else {
            out.push_str(&haystack[start..end]);
        }
        cursor = end;
    }
    out.push_str(&haystack[cursor..]);
    out
}

fn preceded_by_word_char(text: &str, index: usize) -> bool {
    text[..index].chars().next_back().is_some_and(is_word_internal)
}

fn followed_by_word_char(text: &str, index: usize) -> bool {
    text[index..].chars().next().is_some_and(is_word_internal)
}

/// How many cursor steps this text occupies — grapheme clusters, not chars.
///
/// Exists for one purpose: walking a cursor back over text Relay itself
/// injected, so the dictation cleanup can select what it wrote and replace it.
/// The count has to match what an arrow key does, and an arrow key moves by
/// cluster.
///
/// Getting this wrong is not a cosmetic bug. `क्या` is three chars and one
/// cursor step; counting chars would select two steps too many and delete
/// whatever the user had written before the dictation — on the script Relay is
/// most used for. Over-counting destroys their text, so the rule below errs
/// towards treating a mark as part of the preceding cluster.
///
/// An approximation of UAX #29, deliberately, rather than a dependency: what is
/// needed is combining marks, and the Indic blocks put theirs at consistent
/// offsets. It is exact for Latin, correct for the Indic scripts Relay sees,
/// and conservative everywhere else.
pub fn cursor_steps(text: &str) -> usize {
    let mut steps = 0usize;
    for c in text.chars() {
        if !continues_cluster(c) {
            steps += 1;
        }
    }
    steps
}

/// Whether this char attaches to the cluster before it rather than starting one.
fn continues_cluster(c: char) -> bool {
    let code = c as u32;

    // Joiners and variation selectors bind what surrounds them.
    if matches!(code, 0x200C | 0x200D | 0xFE00..=0xFE0F) {
        return true;
    }
    // General combining marks: Latin/Greek/Cyrillic diacritics and enclosing
    // marks.
    if matches!(code, 0x0300..=0x036F | 0x0483..=0x0489 | 0x20D0..=0x20F0) {
        return true;
    }
    // Hebrew points and Arabic marks.
    if matches!(code, 0x0591..=0x05C7 | 0x0610..=0x061A | 0x064B..=0x065F | 0x0670) {
        return true;
    }

    // The Indic blocks, which is the case that actually matters here. Each is
    // 0x80 wide from Devanagari at 0x0900, and each puts its signs, matras,
    // virama and nukta at the same offsets within the block.
    if (0x0900..0x0E00).contains(&code) {
        let offset = code % 0x80;
        return matches!(offset, 0x00..=0x03 | 0x3A..=0x4F | 0x51..=0x57 | 0x62..=0x63);
    }

    false
}

/// Which rules a surface wants.
///
/// The chain is shared; this is the one place it legitimately differs, and it
/// differs because the *destination* differs rather than because the surfaces
/// disagree about what good text is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextProfile {
    /// A recorded transcript segment. The full chain, including sentence
    /// casing and a terminal period: a stored segment is read later, on its
    /// own, and needs to read as a sentence.
    Transcript,
    /// Text about to be injected into whatever field has focus.
    ///
    /// Everything except sentence-boundary repair. The user may be mid-sentence,
    /// filling a form field, or continuing after text that is already there —
    /// and a period Relay added is a period they have to delete. Capitalising
    /// the first letter is wrong for the same reason.
    Dictated,
}

impl TextProfile {
    fn repairs_sentence_boundaries(self) -> bool {
        matches!(self, Self::Transcript)
    }
}

/// Applies the full rule chain to one transcript segment's text.
pub fn normalize_segment_text(raw: &str, glossary: &[String]) -> SegmentOutcome {
    normalize_text(raw, Vocabulary::glossary_only(glossary), TextProfile::Transcript)
}

/// As [`normalize_segment_text`], for a caller that also has learned
/// corrections. Kept separate so the meeting pipeline's many call sites do not
/// all have to grow a parameter they mostly pass empty.
pub fn normalize_segment_text_with(raw: &str, vocabulary: Vocabulary<'_>) -> SegmentOutcome {
    normalize_text(raw, vocabulary, TextProfile::Transcript)
}

/// Applies the rule chain for `profile`.
pub fn normalize_text(
    raw: &str,
    vocabulary: Vocabulary<'_>,
    profile: TextProfile,
) -> SegmentOutcome {
    let mut applied = Vec::new();

    let stripped = strip_bracketed_tags(raw);
    if stripped != raw {
        applied.push(RULE_ASR_TAGS.to_string());
    }

    let collapsed_ws = collapse_whitespace(&stripped);
    if collapsed_ws != stripped.trim() {
        applied.push(RULE_WHITESPACE.to_string());
    }

    let deduped_words = collapse_repeated_words(&collapsed_ws);
    if deduped_words != collapsed_ws {
        applied.push(RULE_REPEATED_WORDS.to_string());
    }

    let deduped_phrases = collapse_repeated_phrases(&deduped_words);
    if deduped_phrases != deduped_words {
        applied.push(RULE_REPEATED_PHRASES.to_string());
    }

    let defillered = remove_isolated_fillers(&deduped_phrases);
    if defillered != deduped_phrases {
        applied.push(RULE_FILLERS.to_string());
    }

    // Before the glossary, deliberately. A learned correction is the user's
    // own statement about a specific phrase; the glossary is a token-level
    // guess by edit distance. The specific rule should win, and running it
    // first also means the glossary sees the corrected form.
    let corrected = apply_learned_corrections(&defillered, vocabulary.corrections);
    if corrected != defillered {
        applied.push(RULE_LEARNED_CORRECTIONS.to_string());
    }

    let glossed = apply_glossary(&corrected, vocabulary.glossary);
    if glossed != corrected {
        applied.push(RULE_GLOSSARY.to_string());
    }

    let repaired = if profile.repairs_sentence_boundaries() {
        let repaired = repair_sentence_boundaries(&glossed);
        if repaired != glossed {
            applied.push(RULE_SENTENCE_BOUNDARIES.to_string());
        }
        repaired
    } else {
        glossed
    };

    SegmentOutcome {
        text: repaired,
        applied_rules: applied,
    }
}

/// Removes `[bracketed]` and `(parenthesized)` ASR annotations such as
/// `[BLANK_AUDIO]`, `[inaudible]`, `(music)`.
///
/// Unterminated openers are treated as running to the end of the segment, which
/// is the common Whisper failure (`[BLANK_AUDIO` with no closer).
fn strip_bracketed_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth_square = 0usize;
    let mut depth_round = 0usize;

    for c in text.chars() {
        match c {
            '[' => depth_square += 1,
            ']' => depth_square = depth_square.saturating_sub(1),
            '(' => depth_round += 1,
            ')' => depth_round = depth_round.saturating_sub(1),
            _ if depth_square == 0 && depth_round == 0 => out.push(c),
            _ => {}
        }
    }

    out
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Returns true if the token is a conversational affirmation or emphasis word
/// where saying it twice ("yes, yes", "haan haan", "theek theek") is natural human speech.
fn is_conversational_affirmation(key: &str) -> bool {
    matches!(
        key,
        "yes" | "no" | "haan" | "sure" | "right" | "theek" | "ok" | "okay" | "yeah"
    )
}

/// Collapses immediate word repetition: "the the the plan" → "the plan".
///
/// Comparison ignores case and trailing punctuation so "Plan. plan" is caught,
/// and the *first* occurrence is kept so its punctuation survives.
///
/// Conversational affirmations ("yes", "no", "haan", "sure", "right", "theek", "ok", "okay", "yeah")
/// repeated up to 2 times for natural emphasis are preserved, while 3 or more occurrences
/// are collapsed down to 2. Non-conversational words always collapse to 1 occurrence.
fn collapse_repeated_words(text: &str) -> String {
    let words: Vec<&str> = text.split(' ').filter(|w| !w.is_empty()).collect();
    let mut out: Vec<&str> = Vec::with_capacity(words.len());
    let mut current_repeat_count = 0usize;

    for word in words {
        let key = comparison_key(word);
        if key.is_empty() {
            out.push(word);
            current_repeat_count = 0;
            continue;
        }

        let matches_previous = out.last().is_some_and(|prev| {
            comparison_key(prev) == key
        });

        if matches_previous {
            current_repeat_count += 1;
            let is_conversational = is_conversational_affirmation(&key);
            if is_conversational && current_repeat_count <= 2 {
                out.push(word);
            }
        } else {
            current_repeat_count = 1;
            out.push(word);
        }
    }

    out.join(" ")
}

/// Collapses immediately repeated multi-word phrases, the shape a Whisper
/// decoder loop takes: "we should ship it we should ship it we should ship it".
///
/// Only *adjacent* repetition is collapsed, and only when the phrase repeats in
/// full. Legitimate repetition separated by other words is left alone.
fn collapse_repeated_phrases(text: &str) -> String {
    let mut words: Vec<String> = text
        .split(' ')
        .filter(|w| !w.is_empty())
        .map(|w| w.to_string())
        .collect();

    // Longest phrase first: a 6-word loop should be recognized as one
    // repetition, not as three collapses of a 2-word phrase.
    for len in (2..=MAX_PHRASE_REPEAT_LEN).rev() {
        let mut i = 0usize;
        while words.len() >= len * 2 && i + len * 2 <= words.len() {
            let first: Vec<String> = words[i..i + len]
                .iter()
                .map(|w| comparison_key(w))
                .collect();
            let second: Vec<String> = words[i + len..i + len * 2]
                .iter()
                .map(|w| comparison_key(w))
                .collect();

            if first == second && !first.iter().all(|w| w.is_empty()) {
                words.drain(i + len..i + len * 2);
                // Stay at `i` so a phrase repeated three or more times
                // collapses down to one occurrence.
                continue;
            }
            i += 1;
        }
    }

    words.join(" ")
}

/// Drops filler tokens that stand alone. A filler carrying punctuation
/// ("Um, no") loses only the filler; the punctuation is re-normalized by the
/// sentence-boundary pass.
fn remove_isolated_fillers(text: &str) -> String {
    let kept: Vec<&str> = text
        .split(' ')
        .filter(|word| {
            let key = comparison_key(word);
            key.is_empty() || !ISOLATED_FILLERS.contains(&key.as_str())
        })
        .filter(|w| !w.is_empty())
        .collect();

    // Removing a leading filler can leave the segment starting with a comma.
    let joined = kept.join(" ");
    joined
        .trim_start_matches([',', '.', '-', ' '])
        .trim()
        .to_string()
}

/// Rewrites known glossary terms to their canonical casing.
///
/// Two levels, both conservative: an exact case-insensitive token match, and —
/// for tokens of at least `MIN_FUZZY_GLOSSARY_LEN` characters — a single-edit
/// match, which is what catches Whisper hearing "Supabase" as "Supabass".
/// Anything looser would start inventing words the speaker did not say.
fn apply_glossary(text: &str, glossary: &[String]) -> String {
    if glossary.is_empty() {
        return text.to_string();
    }

    let terms: Vec<(String, &str)> = glossary
        .iter()
        .filter(|t| !t.trim().is_empty())
        .map(|t| (t.trim().to_lowercase(), t.trim()))
        .collect();

    text.split(' ')
        .map(|word| {
            let key = comparison_key(word);
            if key.is_empty() {
                return word.to_string();
            }

            let exact = terms.iter().find(|(lower, _)| *lower == key);
            let matched = match exact {
                Some(hit) => Some(hit),
                None if key.chars().count() >= MIN_FUZZY_GLOSSARY_LEN => terms
                    .iter()
                    .find(|(lower, _)| is_within_one_edit(lower, &key)),
                None => None,
            };

            match matched {
                // Already correct — don't record a spurious rule hit.
                Some((_, canonical)) if *canonical == trimmed_core(word) => word.to_string(),
                Some((_, canonical)) => replace_core(word, canonical),
                None => word.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Capitalizes sentence openings and gives the segment terminal punctuation.
///
/// Adding a final period is the one insertion this module makes, and it adds no
/// information: a 30-second chunk always ends at a wall-clock boundary, never
/// on a decoder-supplied full stop.
fn repair_sentence_boundaries(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut out = String::with_capacity(trimmed.len() + 1);
    let mut at_sentence_start = true;

    let mut chars = trimmed.chars().peekable();
    while let Some(c) = chars.next() {
        if at_sentence_start && c.is_alphabetic() {
            out.extend(c.to_uppercase());
            at_sentence_start = false;
            continue;
        }

        out.push(c);

        if matches!(c, '.' | '!' | '?') {
            // Only treat this as a boundary if whitespace follows, so "e.g."
            // and "3.5" are not re-capitalized mid-token.
            if chars.peek().is_some_and(|n| n.is_whitespace()) {
                at_sentence_start = true;
            }
        }
    }

    let ends_with_terminal = out
        .trim_end()
        .chars()
        .last()
        .is_some_and(|c| matches!(c, '.' | '!' | '?' | ':' | ';' | ','));
    if !ends_with_terminal {
        out.push('.');
    }

    out
}

/// Lowercased, punctuation-free form of a token, for comparisons only. Never
/// written back into the transcript.
fn comparison_key(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphanumeric() || *c == '\'')
        .collect::<String>()
        .to_lowercase()
}

/// The token with leading/trailing punctuation removed.
fn trimmed_core(word: &str) -> &str {
    word.trim_matches(|c: char| !c.is_alphanumeric())
}

/// Substitutes a token's alphanumeric core while keeping the punctuation that
/// surrounded it, so "supabase," becomes "Supabase,".
fn replace_core(word: &str, replacement: &str) -> String {
    let core = trimmed_core(word);
    if core.is_empty() {
        return word.to_string();
    }
    match word.find(core) {
        Some(idx) => {
            let mut out = String::with_capacity(word.len());
            out.push_str(&word[..idx]);
            out.push_str(replacement);
            out.push_str(&word[idx + core.len()..]);
            out
        }
        None => word.to_string(),
    }
}

/// True when `a` and `b` differ by at most one substitution, insertion, or
/// deletion. Bounded and cheap enough to run per token.
fn is_within_one_edit(a: &str, b: &str) -> bool {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();

    let (long, short) = if a.len() >= b.len() {
        (&a, &b)
    } else {
        (&b, &a)
    };
    if long.len() - short.len() > 1 {
        return false;
    }
    if long.len() == short.len() {
        let diffs = long
            .iter()
            .zip(short.iter())
            .filter(|(x, y)| x != y)
            .count();
        return diffs <= 1;
    }

    // Lengths differ by one: check for a single insertion.
    let mut i = 0usize;
    let mut j = 0usize;
    let mut skipped = false;
    while i < long.len() && j < short.len() {
        if long[i] == short[j] {
            i += 1;
            j += 1;
        } else if skipped {
            return false;
        } else {
            skipped = true;
            i += 1;
        }
    }
    true
}


#[cfg(test)]
mod learned_correction_tests {
    use super::*;
    use crate::settings::VocabularyCorrection;

    fn corrections(pairs: &[(&str, &str)]) -> Vec<VocabularyCorrection> {
        pairs
            .iter()
            .map(|(s, r)| VocabularyCorrection::new(s, r))
            .collect()
    }

    #[test]
    fn a_learned_phrase_is_repaired() {
        let list = corrections(&[("super base", "Supabase")]);
        assert_eq!(
            apply_learned_corrections("I was testing super base yesterday", &list),
            "I was testing Supabase yesterday"
        );
    }

    #[test]
    fn every_occurrence_in_later_transcripts_is_repaired() {
        // Unlike a single Voice Note correction, which is scoped to the range
        // the user selected, a *learned* rule is a standing statement about the
        // phrase and applies wherever it appears.
        let list = corrections(&[("super base", "Supabase")]);
        assert_eq!(
            apply_learned_corrections("super base and super base", &list),
            "Supabase and Supabase"
        );
    }

    #[test]
    fn matching_ignores_case_but_the_replacement_does_not() {
        // Whisper's capitalisation of a phrase it got wrong is noise; the
        // user's capitalisation of the fix is the entire point.
        let list = corrections(&[("super base", "Supabase")]);
        assert_eq!(
            apply_learned_corrections("Super Base is fast", &list),
            "Supabase is fast"
        );
    }

    #[test]
    fn a_phrase_inside_a_longer_word_is_left_alone() {
        let list = corrections(&[("lance", "LanceDB")]);
        assert_eq!(
            apply_learned_corrections("a freelancer used lance today", &list),
            "a freelancer used LanceDB today"
        );
    }

    #[test]
    fn a_disabled_correction_stops_being_applied() {
        let mut list = corrections(&[("super base", "Supabase")]);
        list[0].enabled = false;
        assert_eq!(
            apply_learned_corrections("testing super base", &list),
            "testing super base"
        );
    }

    #[test]
    fn a_removed_correction_stops_being_applied() {
        assert_eq!(
            apply_learned_corrections("testing super base", &[]),
            "testing super base"
        );
    }

    #[test]
    fn a_no_op_rule_is_ignored() {
        let list = vec![
            VocabularyCorrection::new("", "Supabase"),
            VocabularyCorrection::new("ollama", "  "),
            VocabularyCorrection::new("Ollama", "ollama"),
        ];
        assert!(!list[0].is_meaningful());
        assert!(!list[1].is_meaningful());
        assert!(!list[2].is_meaningful(), "differing only by case is not a repair");
        assert_eq!(apply_learned_corrections("ollama and Supabase", &list), "ollama and Supabase");
    }

    #[test]
    fn corrections_run_before_the_glossary_so_the_specific_rule_wins() {
        // The glossary would fuzzily pull "super base" nowhere useful; the
        // learned rule is the user's own statement and is applied first, and
        // the glossary then sees the corrected form.
        let glossary = vec!["Supabase".to_string()];
        let list = corrections(&[("super base", "Supabase")]);
        let outcome = normalize_text(
            "we tested super base",
            Vocabulary::new(&glossary, &list),
            TextProfile::Dictated,
        );
        assert_eq!(outcome.text, "we tested Supabase");
        assert!(outcome
            .applied_rules
            .contains(&RULE_LEARNED_CORRECTIONS.to_string()));
    }

    #[test]
    fn no_corrections_means_the_rule_is_not_reported() {
        let outcome = normalize_text(
            "we tested super base",
            Vocabulary::default(),
            TextProfile::Dictated,
        );
        assert!(!outcome
            .applied_rules
            .contains(&RULE_LEARNED_CORRECTIONS.to_string()));
        assert_eq!(outcome.text, "we tested super base");
    }

    #[test]
    fn a_correction_inside_hinglish_applies_and_leaves_the_rest_alone() {
        // Devanagari is caseless, so lowercasing does not move any offsets and
        // the repair lands normally in mixed text — which is the common shape
        // of what Relay actually transcribes.
        let list = corrections(&[("super base", "Supabase")]);
        assert_eq!(
            apply_learned_corrections("मैं super base चला रहा हूं", &list),
            "मैं Supabase चला रहा हूं"
        );
    }

    #[test]
    fn text_whose_length_changes_when_lowercased_is_left_alone() {
        // The guard that stops a bad slice. Turkish dotted capital I lowercases
        // to two code points, so byte offsets taken from the lowered string no
        // longer line up with the original; the pass declines rather than
        // cutting mid-character.
        let text = "\u{0130}stanbul super base";
        assert_ne!(text.to_lowercase().len(), text.len(), "premise of this test");

        let list = corrections(&[("super base", "Supabase")]);
        assert_eq!(
            apply_learned_corrections(text, &list),
            text,
            "declining is correct; corrupting the text is not"
        );
    }
}

#[cfg(test)]
mod cursor_tests {
    use super::cursor_steps;

    #[test]
    fn latin_counts_one_step_per_character() {
        assert_eq!(cursor_steps("hello"), 5);
        assert_eq!(cursor_steps("send it to Pragati"), 18);
        assert_eq!(cursor_steps(""), 0);
    }

    #[test]
    fn a_devanagari_cluster_is_one_step_not_three() {
        // The case that makes this function necessary. Counting chars here
        // would walk the cursor two steps too far and delete text the user
        // wrote before dictating.
        assert_eq!("क्या".chars().count(), 4, "four chars…");
        assert_eq!(cursor_steps("क्या"), 2, "…but क् + या is two cursor steps");

        // A matra binds to its consonant.
        assert_eq!("मैं".chars().count(), 3);
        assert_eq!(cursor_steps("मैं"), 1);

        // And a whole phrase: मैं(1) + space + कल(2) + space + आऊंगा(3), where
        // आऊंगा is आ, ऊ carrying the anusvara, and ग carrying its matra.
        assert_eq!(cursor_steps("मैं कल आऊंगा"), 8);
        assert_eq!("मैं कल आऊंगा".chars().count(), 12, "twelve chars, eight steps");
    }

    #[test]
    fn combining_marks_never_add_a_step() {
        // e + combining acute is one step, however it was typed.
        assert_eq!(cursor_steps("e\u{0301}"), 1);
        assert_eq!(cursor_steps("cafe\u{0301}"), 4);
    }

    #[test]
    fn a_joined_sequence_is_one_step() {
        // Zero-width joiner binds what surrounds it, so a joined pair does not
        // count as two.
        assert_eq!(cursor_steps("\u{0915}\u{200D}\u{0937}"), 2);
    }

    #[test]
    fn the_count_never_exceeds_the_character_count() {
        // The invariant that makes over-selection impossible: whatever the
        // rule decides, it can only ever merge steps, never invent them.
        for text in [
            "hello",
            "क्या हाल है",
            "cafe\u{0301}",
            "mixed मैं text",
            "",
            "   ",
        ] {
            assert!(
                cursor_steps(text) <= text.chars().count(),
                "{text:?} counted {} steps for {} chars",
                cursor_steps(text),
                text.chars().count()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asr_tags_are_removed_including_unterminated_ones() {
        assert_eq!(
            strip_bracketed_tags("[BLANK_AUDIO] We shipped it (music)"),
            " We shipped it "
        );
        assert_eq!(
            strip_bracketed_tags("We shipped it [BLANK_AUDIO"),
            "We shipped it "
        );
    }

    #[test]
    fn repeated_stt_fragments_collapse() {
        // Fixture C: repeated fragments, the classic decoder loop.
        assert_eq!(
            collapse_repeated_words("the the the plan is ready"),
            "the plan is ready"
        );
        assert_eq!(
            collapse_repeated_phrases("we should ship it we should ship it we should ship it"),
            "we should ship it"
        );
        // Long sentence repetition loop (regression for meeting failure)
        assert_eq!(
            collapse_repeated_phrases(
                "If you are schedule for Monday then you can sit here. If you are schedule for Monday then you can sit here."
            ),
            "If you are schedule for Monday then you can sit here."
        );

        // 3-word repetition loop
        assert_eq!(
            collapse_repeated_phrases("we should ship we should ship"),
            "we should ship"
        );

        // 8-word repetition loop
        assert_eq!(
            collapse_repeated_phrases(
                "we need to verify all changes before shipping we need to verify all changes before shipping"
            ),
            "we need to verify all changes before shipping"
        );

        // 16-word repetition loop
        let sixteen_words = "one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen";
        let sixteen_repeated = format!("{sixteen_words} {sixteen_words}");
        assert_eq!(collapse_repeated_phrases(&sixteen_repeated), sixteen_words);
    }

    #[test]
    fn conversational_repetition_is_preserved_up_to_two_times() {
        assert_eq!(
            collapse_repeated_words("Yes, yes, I agree"),
            "Yes, yes, I agree"
        );
        assert_eq!(
            collapse_repeated_words("Haan, haan, theek hai"),
            "Haan, haan, theek hai"
        );
        assert_eq!(
            collapse_repeated_words("No, no, that is wrong"),
            "No, no, that is wrong"
        );
        assert_eq!(
            collapse_repeated_words("Sure, sure, we can do that"),
            "Sure, sure, we can do that"
        );
        assert_eq!(
            collapse_repeated_words("Right, right, exactly"),
            "Right, right, exactly"
        );

        // 3 or more occurrences collapse down to 2
        assert_eq!(
            collapse_repeated_words("Yes, yes, yes, yes, I agree"),
            "Yes, yes, I agree"
        );
        assert_eq!(
            collapse_repeated_words("Haan, haan, haan, samjha"),
            "Haan, haan, samjha"
        );
    }

    #[test]
    fn legitimate_repetition_separated_by_other_words_survives() {
        let input = "ship it today and then ship it tomorrow";
        assert_eq!(collapse_repeated_phrases(input), input);
    }

    #[test]
    fn only_isolated_fillers_are_removed() {
        assert_eq!(
            remove_isolated_fillers("um I think uh we should go"),
            "I think we should go"
        );
        // A filler as a substring must survive.
        assert_eq!(
            remove_isolated_fillers("bring an umbrella"),
            "bring an umbrella"
        );
        // "so" is meaning-bearing and deliberately not a filler.
        assert_eq!(remove_isolated_fillers("so we ship"), "so we ship");
    }

    #[test]
    fn glossary_fixes_casing_and_one_character_mishearings() {
        let glossary = vec![
            "Relay".to_string(),
            "Supabase".to_string(),
            "LanceDB".to_string(),
        ];
        assert_eq!(
            apply_glossary("we use relay daily", &glossary),
            "we use Relay daily"
        );
        // Punctuation around the term is preserved.
        assert_eq!(
            apply_glossary("with supabase,", &glossary),
            "with Supabase,"
        );
        // One-edit mishearing of a long term.
        assert_eq!(
            apply_glossary("try supabass now", &glossary),
            "try Supabase now"
        );
        // Short unrelated words are never fuzzy-matched into a glossary term.
        assert_eq!(apply_glossary("relax now", &glossary), "relax now");
    }

    #[test]
    fn dictated_text_is_cleaned_but_never_punctuated() {
        // The rules that fix what the decoder got wrong still run.
        let outcome = normalize_text(
            "[BLANK_AUDIO] um the the plan is ready",
            Vocabulary::default(),
            TextProfile::Dictated,
        );
        assert_eq!(outcome.text, "the plan is ready");

        // The rule that would add something does not. This text is going into
        // whatever field has focus: a capital and a period Relay invented are
        // two edits the user has to undo.
        assert!(
            !outcome.applied_rules.contains(&RULE_SENTENCE_BOUNDARIES.to_string()),
            "{:?}",
            outcome.applied_rules
        );
        assert!(outcome.applied_rules.contains(&RULE_ASR_TAGS.to_string()));
        assert!(outcome.applied_rules.contains(&RULE_FILLERS.to_string()));
    }

    #[test]
    fn a_transcript_segment_still_gets_full_punctuation() {
        // The other half of the split: a stored segment is read on its own
        // later and has to look like a sentence.
        let outcome = normalize_text("we shipped the release", Vocabulary::default(), TextProfile::Transcript);
        assert_eq!(outcome.text, "We shipped the release.");
        assert!(outcome
            .applied_rules
            .contains(&RULE_SENTENCE_BOUNDARIES.to_string()));
    }

    #[test]
    fn the_profiles_differ_only_in_punctuation() {
        // Everything else must be identical, or the two surfaces have quietly
        // become two pipelines again.
        let raw = "um we we use relay daily [music]";
        let glossary = vec!["Relay".to_string()];
        let dictated = normalize_text(raw, Vocabulary::glossary_only(&glossary), TextProfile::Dictated);
        let transcript = normalize_text(raw, Vocabulary::glossary_only(&glossary), TextProfile::Transcript);

        assert!(dictated.text.contains("Relay"), "{}", dictated.text);
        assert!(transcript.text.contains("Relay"), "{}", transcript.text);
        // The transcript form is the dictated form plus sentence repair.
        assert_eq!(
            transcript.text.trim_end_matches('.').to_lowercase(),
            dictated.text.to_lowercase()
        );
    }

    #[test]
    fn a_virama_does_not_end_a_word() {
        // The property every whole-word lexicon match depends on.
        let split_on = |text: &str| -> Vec<String> {
            text.split(|c: char| !is_word_internal(c))
                .filter(|w| !w.is_empty())
                .map(str::to_string)
                .collect()
        };
        assert_eq!(split_on("क्या"), vec!["क्या"]);
        assert_eq!(split_on("फॉर्म"), vec!["फॉर्म"]);
        assert_eq!(split_on("मैं फॉर्म भर दूंगी"), vec!["मैं", "फॉर्म", "भर", "दूंगी"]);
        // And it still separates actual words.
        assert_eq!(split_on("bhej dungi"), vec!["bhej", "dungi"]);
    }

    #[test]
    fn dictated_devanagari_survives_the_chain() {
        // The rules are script-agnostic, and dictation is where a Hindi user
        // would notice first if they were not.
        let raw = "मैं फॉर्म भर दूंगी";
        let outcome = normalize_text(raw, Vocabulary::default(), TextProfile::Dictated);
        assert_eq!(outcome.text, raw);
    }

    #[test]
    fn punctuation_is_repaired_without_adding_meaning() {
        // Fixture D: poor punctuation.
        assert_eq!(
            repair_sentence_boundaries("we shipped the release"),
            "We shipped the release."
        );
        assert_eq!(
            repair_sentence_boundaries("we shipped it. then we tested"),
            "We shipped it. Then we tested."
        );
        // A decimal must not be read as a sentence boundary.
        assert_eq!(
            repair_sentence_boundaries("version 3.5 is out"),
            "Version 3.5 is out."
        );
        // An existing terminal mark is not doubled.
        assert_eq!(repair_sentence_boundaries("Is it ready?"), "Is it ready?");
    }

    #[test]
    fn edit_distance_stays_within_one() {
        assert!(is_within_one_edit("supabase", "supabass"));
        assert!(is_within_one_edit("lancedb", "lancedb"));
        assert!(is_within_one_edit("relay", "relays"));
        assert!(!is_within_one_edit("relay", "relaxed"));
        assert!(!is_within_one_edit("whisper", "whispered"));
    }
}
