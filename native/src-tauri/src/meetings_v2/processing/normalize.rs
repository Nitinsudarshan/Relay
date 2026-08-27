//! Deterministic transcript normalization — no model involved.
//!
//! This is the cheap stage that runs before anything expensive, and it exists
//! because feeding raw Whisper output straight to a summarizer makes the model
//! spend its attention on decoder loops and missing punctuation instead of on
//! the meeting.
//!
//! Every rule here is **meaning-preserving**. Normalization may repair
//! punctuation, collapse a stutter the decoder emitted twice, fix the casing of
//! a known glossary term, and drop a bracketed ASR tag. It may not add a
//! sentence, a name, a number, a decision, or a hedge. If a rule cannot be
//! stated as "the speaker said this, written correctly", it does not belong in
//! this file.
//!
//! The raw text is carried through on every segment (`NormalizedSegment::raw_text`)
//! and `transcript.jsonl` is opened read-only, so the effect of any rule here is
//! always reversible by inspection.

use super::model::{NormalizedSegment, NormalizedTranscript, SegmentChannel};
use std::collections::BTreeMap;

/// Rule names, recorded per segment and counted per transcript so a misbehaving
/// rule is visible without re-running anything.
pub const RULE_ASR_TAGS: &str = "asr_tags_removed";
pub const RULE_WHITESPACE: &str = "whitespace_normalized";
pub const RULE_REPEATED_WORDS: &str = "repeated_words_collapsed";
pub const RULE_REPEATED_PHRASES: &str = "repeated_phrases_collapsed";
pub const RULE_FILLERS: &str = "isolated_fillers_removed";
pub const RULE_GLOSSARY: &str = "glossary_terms_corrected";
pub const RULE_SENTENCE_BOUNDARIES: &str = "sentence_boundaries_repaired";

/// Standalone filler tokens. Removed only when they stand alone as a whole
/// token, never as a substring — "um" must not eat the "um" in "umbrella", and
/// "so" is not in this list because it routinely carries meaning.
const ISOLATED_FILLERS: &[&str] = &[
    "um", "umm", "uh", "uhh", "erm", "ah", "ahh", "eh", "hmm", "mmm", "mm", "uhm", "er",
];

/// Longest repeated phrase length (in words) the loop detector looks for.
/// Whisper's repetition loops are short; searching further mostly finds
/// legitimate repetition for emphasis.
const MAX_PHRASE_REPEAT_LEN: usize = 6;

/// Minimum token length before a glossary term is matched by edit distance
/// rather than exactly. Short tokens are far too easy to "correct" into
/// something the speaker never said.
const MIN_FUZZY_GLOSSARY_LEN: usize = 6;

/// A raw transcript segment as the normalizer receives it. Mirrors the fields
/// of `TranscriptSegment` that normalization is allowed to see, keeping this
/// module free of any dependency on the recorder's types.
#[derive(Debug, Clone)]
pub struct RawSegmentInput {
    pub chunk_index: usize,
    pub start_time_s: f64,
    pub end_time_s: f64,
    pub text: String,
    pub mic_had_audio: bool,
    pub sys_had_audio: bool,
}

/// The stable segment id for a chunk. Derived from the immutable chunk index so
/// it survives regeneration and can be cited by action items and decisions.
pub fn segment_id(chunk_index: usize) -> String {
    format!("seg_{:05}", chunk_index)
}

/// Normalizes a whole raw transcript.
///
/// `glossary` is the user's dictionary (Settings › Dictionary). Terms are
/// matched case-insensitively and rewritten to the glossary's own casing, which
/// is how "relay" and "lance db" become "Relay" and "LanceDB" without a model.
pub fn normalize_transcript(
    raw_segments: &[RawSegmentInput],
    glossary: &[String],
) -> NormalizedTranscript {
    let mut segments = Vec::with_capacity(raw_segments.len());
    let mut rule_hits: BTreeMap<String, usize> = BTreeMap::new();
    let mut source_char_count = 0usize;
    let mut dropped_segment_count = 0usize;

    for raw in raw_segments {
        source_char_count += raw.text.len();

        let outcome = normalize_segment_text(&raw.text, glossary);
        if outcome.text.trim().is_empty() {
            // A segment that normalizes to nothing was silence, a bracketed tag,
            // or pure filler. Dropping it from the *derived* transcript leaves
            // the raw line untouched on disk.
            dropped_segment_count += 1;
            continue;
        }

        for rule in &outcome.applied_rules {
            *rule_hits.entry(rule.clone()).or_insert(0) += 1;
        }

        let channel = SegmentChannel::from_flags(raw.mic_had_audio, raw.sys_had_audio);
        segments.push(NormalizedSegment {
            id: segment_id(raw.chunk_index),
            chunk_index: raw.chunk_index,
            start_time_s: raw.start_time_s,
            end_time_s: raw.end_time_s,
            text: outcome.text,
            raw_text: raw.text.clone(),
            channel,
            speaker_id: None,
            applied_rules: outcome.applied_rules,
        });
    }

    segments.sort_by_key(|s| s.chunk_index);
    let output_char_count = segments.iter().map(|s| s.text.len()).sum();

    NormalizedTranscript {
        segments,
        rule_hits,
        source_char_count,
        output_char_count,
        dropped_segment_count,
    }
}

struct SegmentOutcome {
    text: String,
    applied_rules: Vec<String>,
}

/// Applies the full rule chain to one segment's text.
fn normalize_segment_text(raw: &str, glossary: &[String]) -> SegmentOutcome {
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

    let glossed = apply_glossary(&defillered, glossary);
    if glossed != defillered {
        applied.push(RULE_GLOSSARY.to_string());
    }

    let repaired = repair_sentence_boundaries(&glossed);
    if repaired != glossed {
        applied.push(RULE_SENTENCE_BOUNDARIES.to_string());
    }

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

/// Collapses immediate word repetition: "the the the plan" → "the plan".
///
/// Comparison ignores case and trailing punctuation so "Plan. plan" is caught,
/// and the *first* occurrence is kept so its punctuation survives.
fn collapse_repeated_words(text: &str) -> String {
    let words: Vec<&str> = text.split(' ').filter(|w| !w.is_empty()).collect();
    let mut out: Vec<&str> = Vec::with_capacity(words.len());

    for word in words {
        let matches_previous = out.last().is_some_and(|prev| {
            comparison_key(prev) == comparison_key(word) && !comparison_key(word).is_empty()
        });
        if !matches_previous {
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
mod tests {
    use super::*;

    fn seg(chunk_index: usize, text: &str) -> RawSegmentInput {
        RawSegmentInput {
            chunk_index,
            start_time_s: chunk_index as f64 * 30.0,
            end_time_s: (chunk_index + 1) as f64 * 30.0,
            text: text.to_string(),
            mic_had_audio: true,
            sys_had_audio: false,
        }
    }

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
    fn normalization_records_what_it_changed_and_keeps_the_raw_text() {
        let raws = vec![seg(0, "[BLANK_AUDIO] um the the plan is is ready")];
        let normalized = normalize_transcript(&raws, &[]);

        assert_eq!(normalized.segments.len(), 1);
        let s = &normalized.segments[0];
        assert_eq!(s.text, "The plan is ready.");
        assert_eq!(
            s.raw_text, "[BLANK_AUDIO] um the the plan is is ready",
            "the raw text must be carried through untouched"
        );
        assert_eq!(s.id, "seg_00000");
        assert!(s.applied_rules.contains(&RULE_ASR_TAGS.to_string()));
        assert!(s.applied_rules.contains(&RULE_FILLERS.to_string()));
        assert!(s.applied_rules.contains(&RULE_REPEATED_WORDS.to_string()));
        assert_eq!(normalized.rule_hits.get(RULE_ASR_TAGS), Some(&1));
    }

    #[test]
    fn segments_that_normalize_to_nothing_are_dropped_not_kept_empty() {
        let raws = vec![seg(0, "[BLANK_AUDIO]"), seg(1, "Real content here")];
        let normalized = normalize_transcript(&raws, &[]);
        assert_eq!(normalized.segments.len(), 1);
        assert_eq!(normalized.dropped_segment_count, 1);
        assert_eq!(normalized.segments[0].chunk_index, 1);
    }

    #[test]
    fn channel_flags_become_segment_channels() {
        let mut mic_only = seg(0, "I will send it");
        mic_only.mic_had_audio = true;
        mic_only.sys_had_audio = false;

        let mut both = seg(1, "Sounds good to me");
        both.mic_had_audio = true;
        both.sys_had_audio = true;

        let normalized = normalize_transcript(&[mic_only, both], &[]);
        assert_eq!(normalized.segments[0].channel, SegmentChannel::Mic);
        assert_eq!(normalized.segments[1].channel, SegmentChannel::Mixed);
    }

    #[test]
    fn normalization_never_grows_the_transcript() {
        // A guard against a rule that "repairs" by elaborating. Cleanup can only
        // ever remove content or adjust punctuation.
        let raws = vec![
            seg(0, "um so the the thing is is we we need to ship"),
            seg(1, "[BLANK_AUDIO]"),
            seg(2, "yeah agreed lets do it tomorrow"),
        ];
        let normalized = normalize_transcript(&raws, &[]);
        let grown = normalized.output_char_count as i64 - normalized.source_char_count as i64;
        assert!(
            grown <= normalized.segments.len() as i64,
            "normalization added {} chars, more than the one terminal period per segment it is allowed",
            grown
        );
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
