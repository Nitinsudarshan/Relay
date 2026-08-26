//! Stage 0: deterministic transcript normalization.
//!
//! Raw ASR segments in, cleaned turns plus a diagnostics report out. No model
//! is involved and none should be: stripping bracketed tags, collapsing decoder
//! loops, and dropping filler are jobs a small quantized model does
//! inconsistently, at the cost of the context it needs for actual reasoning.
//!
//! Two invariants shape the API (see `CLAUDE.md`):
//!
//! - **ASR output is immutable.** Nothing here edits the stored transcript.
//!   The output is a derived layer, and every turn carries the ids of the
//!   source segments it came from, so it can be re-run or discarded freely.
//! - **The result is keyed to time**, not to string offsets, so downstream
//!   claims can cite `start_ms`/`end_ms` evidence spans.
//!
//! Specified by `Meeting-rules/meeting_transcript_summary.md` §4 and
//! `Meeting-rules/meeting_action_items_tasks.md` §6.

use super::glossary::Glossary;
use super::types::Channel;
use serde::{Deserialize, Serialize};

/// Inner text of a bracketed span that marks it as an ASR artifact rather than
/// speech. Matched as a substring against the lowercased span.
const ARTIFACT_MARKERS: &[&str] = &[
    "blank_audio",
    "blank audio",
    "no audio",
    "inaudible",
    "unintelligible",
    "indistinct",
    "silence",
    "no speech",
    "music",
    "laugh",
    "chuckl",
    "cough",
    "sneez",
    "sigh",
    "throat",
    "applause",
    "phone ringing",
    "ringing",
    "water running",
    "typing",
    "keyboard",
    "noise",
    "crosstalk",
    "cross-talk",
    "speaking in",
    "speaks in",
    "foreign language",
    "non-english",
    "beep",
    "static",
    "footsteps",
    "breathing",
    "background",
    "pause",
];

/// Filler openings removed at the start of a clause. Deliberately short: only
/// phrases that carry no meaning anywhere they appear. Bare "so", "well",
/// "like", and "actually" are excluded on purpose — each can be load-bearing.
const CLAUSE_FILLERS: &[&str] = &[
    "so basically",
    "yeah so",
    "okay so",
    "ok so",
    "alright so",
    "right so",
    "i mean",
    "you know",
    "um",
    "uh",
    "uhm",
    "erm",
    "hmm",
    "mhm",
    "mm",
];

#[derive(Debug, Clone)]
pub struct NormalizerConfig {
    /// Consecutive repetitions of a phrase that constitute a decoder loop.
    /// Three is the threshold the rules files specify.
    pub min_loop_repeats: usize,
    /// Longest phrase, in words, checked for loop repetition.
    pub max_loop_phrase_words: usize,
    /// Silence at or above this becomes a structural break between turns
    /// instead of being carried as text.
    pub gap_break_ms: u64,
    pub strip_fillers: bool,
}

impl Default for NormalizerConfig {
    fn default() -> Self {
        Self {
            min_loop_repeats: 3,
            max_loop_phrase_words: 8,
            gap_break_ms: 2_500,
            strip_fillers: true,
        }
    }
}

/// One raw ASR segment entering the normalizer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSegment {
    /// Stable id in the source transcript (Relay uses the chunk index).
    pub id: usize,
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    #[serde(default)]
    pub channel: Channel,
    /// Speaker label, when attribution has already run. Turns never merge
    /// across different speakers.
    #[serde(default)]
    pub speaker: Option<String>,
}

impl SourceSegment {
    /// Builds the input for a session recorded before channel splitting, where
    /// every segment is the mixed track.
    pub fn from_mixed(id: usize, start_s: f64, end_s: f64, text: &str) -> Self {
        Self {
            id,
            start_ms: (start_s * 1000.0).round() as u64,
            end_ms: (end_s * 1000.0).round() as u64,
            text: text.to_string(),
            channel: Channel::Mixed,
            speaker: None,
        }
    }
}

/// A merged, cleaned run of speech from one speaker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    pub index: usize,
    pub start_ms: u64,
    pub end_ms: u64,
    pub channel: Channel,
    pub speaker: Option<String>,
    pub text: String,
    /// Source segments this turn was built from — the link back to immutable
    /// ASR output.
    pub source_segment_ids: Vec<usize>,
    /// Silence separating this turn from the previous one, when it exceeded the
    /// structural-break threshold.
    #[serde(default)]
    pub gap_before_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ArtifactRemoval {
    pub tag: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopCollapse {
    pub phrase: String,
    pub repetitions: usize,
    pub start_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NormalizationDiagnostics {
    pub segments_in: usize,
    pub turns_out: usize,
    pub chars_in: usize,
    pub chars_out: usize,
    pub artifacts_removed: Vec<ArtifactRemoval>,
    pub loops_collapsed: Vec<LoopCollapse>,
    pub filler_removals: usize,
    pub glossary_corrections: Vec<super::glossary::GlossaryHit>,
    pub structural_breaks: usize,
    pub empty_segments: usize,
}

impl NormalizationDiagnostics {
    /// Total repeated lines discarded as decoder loops.
    pub fn loop_lines_discarded(&self) -> usize {
        self.loops_collapsed
            .iter()
            .map(|l| l.repetitions.saturating_sub(1))
            .sum()
    }

    pub fn artifact_total(&self) -> usize {
        self.artifacts_removed.iter().map(|a| a.count).sum()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedTranscript {
    pub turns: Vec<Turn>,
    pub diagnostics: NormalizationDiagnostics,
}

impl NormalizedTranscript {
    /// Plain speech, one turn per line — the model-facing form when attribution
    /// is unavailable.
    pub fn plain_text(&self) -> String {
        self.turns
            .iter()
            .map(|t| t.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Speech prefixed with a speaker label and a timestamp, so the model can
    /// cite evidence and attribute ownership.
    pub fn labelled_text(&self) -> String {
        self.turns
            .iter()
            .map(|t| {
                let label = t.speaker.clone().unwrap_or_else(|| match t.channel {
                    Channel::Mic => "Me".to_string(),
                    Channel::System => "Them".to_string(),
                    Channel::Mixed => "Speaker".to_string(),
                });
                format!("[{}] {}: {}", format_timestamp(t.start_ms), label, t.text)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn is_empty(&self) -> bool {
        self.turns.iter().all(|t| t.text.trim().is_empty())
    }

    pub fn word_count(&self) -> usize {
        self.turns
            .iter()
            .map(|t| t.text.split_whitespace().count())
            .sum()
    }

    /// The turn covering `ms`, for resolving a model's evidence span back to a
    /// seekable position.
    pub fn turn_at(&self, ms: u64) -> Option<&Turn> {
        self.turns
            .iter()
            .find(|t| ms >= t.start_ms && ms <= t.end_ms)
    }
}

pub fn format_timestamp(ms: u64) -> String {
    let total_secs = ms / 1000;
    let h = total_secs / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    if h > 0 {
        format!("{:02}:{:02}:{:02}", h, m, s)
    } else {
        format!("{:02}:{:02}", m, s)
    }
}

/// Normalizes raw ASR segments into cleaned turns.
///
/// Pure: same input, same output, no I/O, no clock, no model.
pub fn normalize(
    segments: &[SourceSegment],
    config: &NormalizerConfig,
    glossary: Option<&Glossary>,
) -> NormalizedTranscript {
    let mut diagnostics = NormalizationDiagnostics {
        segments_in: segments.len(),
        chars_in: segments.iter().map(|s| s.text.chars().count()).sum(),
        ..Default::default()
    };

    // 1. Clean each segment independently: artifacts, then glossary, then filler.
    struct CleanSegment<'a> {
        source: &'a SourceSegment,
        text: String,
    }

    let mut cleaned: Vec<CleanSegment> = Vec::with_capacity(segments.len());
    for segment in segments {
        let (mut text, removals) = strip_artifacts(&segment.text);
        for removal in removals {
            record_artifact(&mut diagnostics.artifacts_removed, &removal);
        }

        if let Some(glossary) = glossary {
            let (corrected, hits) = glossary.normalize_text(&text);
            text = corrected;
            for hit in hits {
                record_glossary_hit(&mut diagnostics.glossary_corrections, hit);
            }
        }

        if config.strip_fillers {
            let (stripped, removed) = strip_clause_fillers(&text);
            text = stripped;
            diagnostics.filler_removals += removed;
        }

        let text = collapse_whitespace(&text);
        if text.is_empty() {
            diagnostics.empty_segments += 1;
        }
        cleaned.push(CleanSegment {
            source: segment,
            text,
        });
    }

    // 2. Merge into turns. A turn breaks on a speaker change or on silence: an
    //    empty segment is silence, so its duration counts toward the gap rather
    //    than joining two unrelated stretches of speech into one turn.
    let mut turns: Vec<Turn> = Vec::new();
    let mut pending_gap_ms: u64 = 0;

    for entry in &cleaned {
        let segment = entry.source;

        if entry.text.is_empty() {
            pending_gap_ms += segment.end_ms.saturating_sub(segment.start_ms);
            continue;
        }

        let same_speaker = turns.last().is_some_and(|last| {
            last.channel == segment.channel && last.speaker == segment.speaker
        });
        let gap_from_timing = turns
            .last()
            .map(|last| segment.start_ms.saturating_sub(last.end_ms))
            .unwrap_or(0);
        // Both values measure the same silence — the dropped empty segment and
        // the hole its removal leaves in the timeline — so take the larger
        // rather than counting it twice.
        let gap_ms = pending_gap_ms.max(gap_from_timing);
        let is_break = gap_ms >= config.gap_break_ms;

        if same_speaker && !is_break {
            let last = turns.last_mut().expect("same_speaker implies a last turn");
            last.text.push(' ');
            last.text.push_str(&entry.text);
            last.end_ms = segment.end_ms.max(last.end_ms);
            last.source_segment_ids.push(segment.id);
        } else {
            if is_break && !turns.is_empty() {
                diagnostics.structural_breaks += 1;
            }
            turns.push(Turn {
                index: turns.len(),
                start_ms: segment.start_ms,
                end_ms: segment.end_ms,
                channel: segment.channel,
                speaker: segment.speaker.clone(),
                text: entry.text.clone(),
                source_segment_ids: vec![segment.id],
                gap_before_ms: if is_break && gap_ms > 0 {
                    Some(gap_ms)
                } else {
                    None
                },
            });
        }
        pending_gap_ms = 0;
    }

    // 3. Collapse decoder loops. This runs after merging on purpose: Relay
    //    slices audio into fixed 30-second chunks, so a loop of 9–20
    //    repetitions is routinely split across several segments and is only
    //    visible once they are one turn.
    for turn in &mut turns {
        let (collapsed, loops) = collapse_loops(
            &turn.text,
            config.min_loop_repeats,
            config.max_loop_phrase_words,
        );
        turn.text = collapsed;
        for (phrase, repetitions) in loops {
            diagnostics.loops_collapsed.push(LoopCollapse {
                phrase,
                repetitions,
                start_ms: turn.start_ms,
            });
        }
    }

    turns.retain(|t| !t.text.trim().is_empty());
    for (i, turn) in turns.iter_mut().enumerate() {
        turn.index = i;
    }

    diagnostics.turns_out = turns.len();
    diagnostics.chars_out = turns.iter().map(|t| t.text.chars().count()).sum();

    NormalizedTranscript {
        turns,
        diagnostics,
    }
}

fn record_artifact(list: &mut Vec<ArtifactRemoval>, tag: &str) {
    let key = tag.to_lowercase();
    if let Some(existing) = list.iter_mut().find(|a| a.tag == key) {
        existing.count += 1;
    } else {
        list.push(ArtifactRemoval {
            tag: key,
            count: 1,
        });
    }
}

fn record_glossary_hit(list: &mut Vec<super::glossary::GlossaryHit>, hit: super::glossary::GlossaryHit) {
    if let Some(existing) = list
        .iter_mut()
        .find(|h| h.heard == hit.heard && h.canonical == hit.canonical)
    {
        existing.count += hit.count;
    } else {
        list.push(hit);
    }
}

/// Removes bracketed ASR tags, returning the cleaned text and the tags removed.
///
/// Square-bracketed spans go unconditionally — Whisper does not emit `[` in
/// transcribed speech. Parenthesized spans go only when their content looks
/// like an artifact, because a speaker can legitimately be transcribed with
/// parentheses and deleting real speech is worse than leaving a stray tag.
///
/// This replaces an earlier implementation that deleted everything between any
/// `[`/`(` and the next closing bracket, which silently ate the remainder of a
/// segment whenever a bracket was left unclosed.
pub fn strip_artifacts(text: &str) -> (String, Vec<String>) {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut removed = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        let opener = chars[i];
        let closer = match opener {
            '[' => Some(']'),
            '(' => Some(')'),
            _ => None,
        };

        match closer {
            Some(closer) => {
                let close_idx = (i + 1..chars.len()).find(|&j| chars[j] == closer);
                match close_idx {
                    Some(j) => {
                        let inner: String = chars[i + 1..j].iter().collect();
                        if opener == '[' || is_artifact_span(&inner) {
                            removed.push(inner.trim().to_string());
                            i = j + 1;
                            // Leave a space so removal cannot fuse two words.
                            out.push(' ');
                            continue;
                        }
                        // Real parenthetical speech: keep it, brackets and all.
                        out.extend(&chars[i..=j]);
                        i = j + 1;
                    }
                    None => {
                        // Unclosed bracket. Drop the rest only if it reads as a
                        // tag; otherwise drop just the stray bracket.
                        let rest: String = chars[i + 1..].iter().collect();
                        if is_artifact_span(&rest) {
                            removed.push(rest.trim().to_string());
                            i = chars.len();
                            out.push(' ');
                        } else {
                            i += 1;
                        }
                    }
                }
            }
            None => {
                out.push(opener);
                i += 1;
            }
        }
    }

    (collapse_whitespace(&out), removed)
}

fn is_artifact_span(inner: &str) -> bool {
    let lower = inner.trim().to_lowercase();
    if lower.is_empty() {
        return true;
    }
    // An all-caps token like BLANK_AUDIO is a tag regardless of vocabulary.
    let is_screaming = inner
        .chars()
        .filter(|c| c.is_alphabetic())
        .all(|c| c.is_uppercase())
        && inner.chars().any(|c| c.is_alphabetic());
    if is_screaming && lower.split_whitespace().count() <= 4 {
        return true;
    }
    ARTIFACT_MARKERS.iter().any(|m| lower.contains(m))
}

/// Removes filler openings at the start of each clause.
pub fn strip_clause_fillers(text: &str) -> (String, usize) {
    let mut out = String::with_capacity(text.len());
    let mut removed = 0;
    let mut clause_start = true;
    let mut rest = text;

    while !rest.is_empty() {
        if clause_start {
            let trimmed = rest.trim_start();
            let consumed_ws = rest.len() - trimmed.len();
            if let Some(len) = matched_filler(trimmed) {
                let after = trimmed[len..].trim_start();
                // Only drop the filler when real content follows it.
                if !after.is_empty() && after.chars().next().is_some_and(|c| c.is_alphanumeric()) {
                    removed += 1;
                    // Keep the separator: dropping it too would run the previous
                    // clause into this one.
                    if !out.is_empty() && !out.ends_with(' ') {
                        out.push(' ');
                    }
                    rest = after;
                    continue;
                }
            }
            out.push_str(&rest[..consumed_ws]);
            rest = trimmed;
            clause_start = false;
            continue;
        }

        let ch = rest.chars().next().expect("rest is non-empty");
        out.push(ch);
        if matches!(ch, '.' | '?' | '!' | ',' | ';' | ':' | '\n') {
            clause_start = true;
        }
        rest = &rest[ch.len_utf8()..];
    }

    (collapse_whitespace(&out), removed)
}

/// Length in bytes of the filler phrase opening `text`, if any.
fn matched_filler(text: &str) -> Option<usize> {
    let lower = text.to_lowercase();
    for filler in CLAUSE_FILLERS {
        if !lower.starts_with(filler) {
            continue;
        }
        let after = &lower[filler.len()..];
        // Must be a whole word, and may be followed by its own punctuation.
        let mut len = filler.len();
        let boundary_ok = match after.chars().next() {
            None => true,
            Some(c) if c.is_whitespace() => true,
            Some(c) if matches!(c, ',' | '.' | '-' | '…') => {
                len += c.len_utf8();
                true
            }
            Some(_) => false,
        };
        if boundary_ok {
            return Some(len);
        }
    }
    None
}

/// Collapses decoder loops in a single turn.
///
/// Two shapes are handled: whole sentences repeating (the common Whisper loop,
/// which arrives with punctuation) and a short phrase repeating inside one
/// unpunctuated run. Returns the collapsed text and `(phrase, repetitions)` for
/// each loop found.
pub fn collapse_loops(
    text: &str,
    min_repeats: usize,
    max_phrase_words: usize,
) -> (String, Vec<(String, usize)>) {
    let mut loops = Vec::new();

    // Pass 1 — repeated sentences.
    let units = split_sentences(text);
    let mut kept: Vec<String> = Vec::with_capacity(units.len());
    let mut i = 0;
    while i < units.len() {
        let key = normalize_for_compare(&units[i]);
        let mut run = 1;
        while i + run < units.len() && normalize_for_compare(&units[i + run]) == key {
            run += 1;
        }
        if run >= min_repeats && !key.is_empty() {
            // A loop: one instance survives.
            kept.push(units[i].clone());
            loops.push((units[i].trim().to_string(), run));
        } else {
            // Two repeats can be emphasis. Below the threshold, keep them all.
            for unit in &units[i..i + run] {
                kept.push(unit.clone());
            }
        }
        i += run;
    }

    // Pass 2 — a phrase repeating inside one unit, with no punctuation to split on.
    let mut result: Vec<String> = Vec::with_capacity(kept.len());
    for unit in kept {
        let (collapsed, unit_loops) = collapse_phrase_runs(&unit, min_repeats, max_phrase_words);
        for l in unit_loops {
            loops.push(l);
        }
        result.push(collapsed);
    }

    (collapse_whitespace(&result.join(" ")), loops)
}

/// Collapses `A A A` → `A` for any phrase up to `max_phrase_words` long.
fn collapse_phrase_runs(
    unit: &str,
    min_repeats: usize,
    max_phrase_words: usize,
) -> (String, Vec<(String, usize)>) {
    let words: Vec<&str> = unit.split_whitespace().collect();
    if words.len() < min_repeats {
        return (unit.to_string(), Vec::new());
    }

    let normalized: Vec<String> = words.iter().map(|w| normalize_for_compare(w)).collect();
    let mut loops = Vec::new();
    let mut out: Vec<&str> = Vec::with_capacity(words.len());
    let mut i = 0;

    while i < words.len() {
        let mut collapsed = false;
        // Longest phrase first, so "the form the form" is not mistaken for a
        // run of "the".
        let max_len = max_phrase_words.min((words.len() - i) / min_repeats);
        for len in (1..=max_len).rev() {
            if len == 0 {
                continue;
            }
            let mut run = 1;
            while i + (run + 1) * len <= words.len()
                && normalized[i..i + len] == normalized[i + run * len..i + (run + 1) * len]
            {
                run += 1;
            }
            if run >= min_repeats {
                out.extend(&words[i..i + len]);
                loops.push((words[i..i + len].join(" "), run));
                i += run * len;
                collapsed = true;
                break;
            }
        }
        if !collapsed {
            out.push(words[i]);
            i += 1;
        }
    }

    (out.join(" "), loops)
}

fn split_sentences(text: &str) -> Vec<String> {
    let mut units = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch == '\n' {
            if !current.trim().is_empty() {
                units.push(current.trim().to_string());
            }
            current.clear();
            continue;
        }
        current.push(ch);
        if matches!(ch, '.' | '?' | '!') {
            if !current.trim().is_empty() {
                units.push(current.trim().to_string());
            }
            current.clear();
        }
    }
    if !current.trim().is_empty() {
        units.push(current.trim().to_string());
    }
    units
}

/// Comparison form for loop detection: case, punctuation, and spacing removed.
fn normalize_for_compare(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .flat_map(|c| c.to_lowercase())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn collapse_whitespace(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_was_space = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !last_was_space && !out.is_empty() {
                out.push(' ');
            }
            last_was_space = true;
        } else {
            out.push(ch);
            last_was_space = false;
        }
    }
    // Punctuation left stranded by a removed tag.
    let trimmed = out.trim();
    let cleaned: String = {
        let mut s = String::with_capacity(trimmed.len());
        let mut prev_space = false;
        for ch in trimmed.chars() {
            if prev_space && matches!(ch, ',' | '.' | '?' | '!' | ';' | ':') {
                // Drop the space before punctuation.
                s.pop();
            }
            s.push(ch);
            prev_space = ch == ' ';
        }
        s
    };
    cleaned
        .trim()
        .trim_start_matches([',', '.', ';', ':', '-'])
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meetings_v2::glossary::{GlossarySource, GlossaryTerm};

    fn seg(id: usize, text: &str) -> SourceSegment {
        SourceSegment::from_mixed(id, id as f64 * 30.0, (id as f64 + 1.0) * 30.0, text)
    }

    fn run(segments: &[SourceSegment]) -> NormalizedTranscript {
        normalize(segments, &NormalizerConfig::default(), None)
    }

    // ---- artifact stripping ----

    #[test]
    fn strips_every_bracketed_tag_the_rules_files_list() {
        for tag in [
            "[BLANK_AUDIO]",
            "[MUSIC PLAYING]",
            "[NON-ENGLISH SPEECH]",
            "(Silence / No Speech)",
            "(laughing)",
            "(coughing)",
            "(speaking in foreign language)",
            "(speaks in Hindi)",
            "(phone ringing)",
            "(water running)",
            "(upbeat music)",
            "[no audio]",
            "[inaudible]",
        ] {
            let input = format!("{} we agreed on the split {}", tag, tag);
            let (out, removed) = strip_artifacts(&input);
            assert_eq!(out, "we agreed on the split", "tag not stripped: {tag}");
            assert_eq!(removed.len(), 2, "removal not recorded: {tag}");
        }
    }

    #[test]
    fn keeps_a_parenthetical_that_is_real_speech() {
        // The previous implementation deleted everything between any brackets,
        // which took genuine speech with it.
        let (out, removed) = strip_artifacts("the revenue split (fifty-fifty) was agreed");
        assert_eq!(out, "the revenue split (fifty-fifty) was agreed");
        assert!(removed.is_empty());
    }

    #[test]
    fn an_unclosed_tag_does_not_swallow_the_rest_of_the_segment() {
        // A bare "[" used to eat every remaining character in the segment.
        let (out, _) = strip_artifacts("we shipped the change [inaudible");
        assert_eq!(out, "we shipped the change");

        let (out, removed) = strip_artifacts("the value was [ 40 percent higher");
        assert_eq!(
            out, "the value was 40 percent higher",
            "an unclosed bracket with real speech after it must keep the speech"
        );
        assert!(removed.is_empty());
    }

    #[test]
    fn removal_does_not_fuse_the_words_around_it() {
        let (out, _) = strip_artifacts("send the list[BLANK_AUDIO]before Friday");
        assert_eq!(out, "send the list before Friday");
    }

    // ---- decoder loops ----

    #[test]
    fn collapses_a_repeated_sentence_to_one_instance() {
        let loop_text = "I will pay the firm to fill the form. ".repeat(9);
        let (out, loops) = collapse_loops(&loop_text, 3, 8);
        assert_eq!(out, "I will pay the firm to fill the form.");
        assert_eq!(loops.len(), 1);
        assert_eq!(loops[0].1, 9);
    }

    #[test]
    fn collapses_a_repeated_phrase_with_no_punctuation() {
        let (out, loops) = collapse_loops("yeah yeah yeah yeah yeah okay", 3, 8);
        assert_eq!(out, "yeah okay");
        assert_eq!(loops[0].1, 5);
    }

    #[test]
    fn two_repeats_are_emphasis_and_survive() {
        // The rules files set the threshold at three deliberately.
        let text = "Earlier it was less. Earlier it was less.";
        let (out, loops) = collapse_loops(text, 3, 8);
        assert_eq!(out, text);
        assert!(loops.is_empty());
    }

    #[test]
    fn a_loop_split_across_chunk_boundaries_is_still_collapsed() {
        // Relay slices audio every 30 seconds, so a 20-repetition loop arrives
        // as several segments and is only visible once they are one turn.
        let half = "I will pay the firm to fill the form. ".repeat(6);
        let result = run(&[seg(0, &half), seg(1, &half)]);

        assert_eq!(result.turns.len(), 1, "same speaker, no gap: one turn");
        assert_eq!(result.turns[0].text, "I will pay the firm to fill the form.");
        assert_eq!(result.diagnostics.loops_collapsed.len(), 1);
        assert_eq!(result.diagnostics.loops_collapsed[0].repetitions, 12);
        assert_eq!(result.diagnostics.loop_lines_discarded(), 11);
    }

    #[test]
    fn a_longer_phrase_wins_over_a_repeated_word_inside_it() {
        let (out, loops) = collapse_loops("the form the form the form", 3, 8);
        assert_eq!(out, "the form");
        assert_eq!(loops[0].0, "the form");
    }

    // ---- filler ----

    #[test]
    fn removes_filler_only_at_a_clause_start() {
        let (out, removed) = strip_clause_fillers(
            "So basically, um, the main problem is data. You know, we need weekly updates.",
        );
        assert_eq!(out, "the main problem is data. we need weekly updates.");
        assert_eq!(removed, 3);
    }

    #[test]
    fn keeps_words_that_only_look_like_filler() {
        // "so" and "well" carry meaning; only the listed phrases go.
        let (out, removed) = strip_clause_fillers("So the umbrella policy went well.");
        assert_eq!(out, "So the umbrella policy went well.");
        assert_eq!(removed, 0);
    }

    #[test]
    fn filler_alone_in_a_clause_is_left_alone() {
        let (out, _) = strip_clause_fillers("um");
        assert_eq!(out, "um", "removing this would empty the segment for nothing");
    }

    // ---- turns and gaps ----

    #[test]
    fn merges_consecutive_same_speaker_segments_into_one_turn() {
        let result = run(&[seg(0, "First part of the point."), seg(1, "Second part of it.")]);
        assert_eq!(result.turns.len(), 1);
        assert_eq!(
            result.turns[0].text,
            "First part of the point. Second part of it."
        );
        assert_eq!(result.turns[0].source_segment_ids, vec![0, 1]);
        assert_eq!(result.turns[0].start_ms, 0);
        assert_eq!(result.turns[0].end_ms, 60_000);
    }

    #[test]
    fn a_speaker_change_starts_a_new_turn() {
        let mut a = seg(0, "Can you review the employee guide?");
        a.channel = Channel::Mic;
        let mut b = seg(1, "Sure, we will go through this.");
        b.channel = Channel::System;

        let result = run(&[a, b]);
        assert_eq!(result.turns.len(), 2);
        assert_eq!(result.turns[0].channel, Channel::Mic);
        assert_eq!(result.turns[1].channel, Channel::System);
    }

    #[test]
    fn silence_becomes_a_structural_break_not_text() {
        // A silent chunk carries no words; its duration separates the turns
        // instead of joining two unrelated stretches of speech.
        let result = run(&[
            seg(0, "We closed the first topic."),
            seg(1, "[BLANK_AUDIO]"),
            seg(2, "Moving to the second topic."),
        ]);

        assert_eq!(result.turns.len(), 2);
        assert_eq!(result.diagnostics.structural_breaks, 1);
        assert_eq!(result.diagnostics.empty_segments, 1);
        assert_eq!(result.turns[1].gap_before_ms, Some(30_000));
        assert!(!result.plain_text().contains("BLANK"));
    }

    #[test]
    fn turns_keep_evidence_timestamps_for_click_to_seek() {
        let result = run(&[seg(0, "First."), seg(1, "Second.")]);
        let turn = result.turn_at(45_000).expect("45s falls inside the turn");
        assert_eq!(turn.index, 0);
        assert!(result.turn_at(120_000).is_none());
    }

    #[test]
    fn labelled_text_names_the_channel_for_the_todo_pass() {
        let mut a = seg(0, "I will send the list.");
        a.channel = Channel::Mic;
        let result = run(&[a]);
        assert_eq!(result.labelled_text(), "[00:00] Me: I will send the list.");
    }

    #[test]
    fn an_all_silence_recording_normalizes_to_nothing() {
        let result = run(&[seg(0, "[BLANK_AUDIO]"), seg(1, "(Silence / No Speech)")]);
        assert!(result.turns.is_empty());
        assert!(result.is_empty());
        assert_eq!(result.diagnostics.empty_segments, 2);
    }

    #[test]
    fn normalization_never_touches_the_source_segments() {
        let segments = vec![seg(0, "[BLANK_AUDIO] um, the point stands.")];
        let before = segments[0].text.clone();
        let _ = run(&segments);
        assert_eq!(
            segments[0].text, before,
            "ASR output is immutable; normalization is a derived layer"
        );
    }

    // ---- the fixture ----

    fn load_fixture() -> Vec<SourceSegment> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/meetings/hinglish_alumni_placement/transcript.jsonl");
        let raw = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("fixture missing at {}: {}", path.display(), e));

        raw.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|line| {
                let v: serde_json::Value = serde_json::from_str(line).expect("fixture line is JSON");
                SourceSegment::from_mixed(
                    v["chunk_index"].as_u64().unwrap_or(0) as usize,
                    v["start_time_s"].as_f64().unwrap_or(0.0),
                    v["end_time_s"].as_f64().unwrap_or(0.0),
                    v["text"].as_str().unwrap_or(""),
                )
            })
            .collect()
    }

    #[test]
    fn the_fixture_loses_every_loop_and_every_tag() {
        let segments = load_fixture();
        let glossary = crate::meetings_v2::glossary::Glossary::from_terms(vec![
            GlossaryTerm::new("alumni", GlossarySource::Manual),
            GlossaryTerm::new("Coursera", GlossarySource::Manual),
            GlossaryTerm::new("Pay Forward", GlossarySource::Manual),
            GlossaryTerm::new("NavGurukul", GlossarySource::Manual),
        ]);
        let result = normalize(&segments, &NormalizerConfig::default(), Some(&glossary));
        let text = result.plain_text();

        // No artifact survives anywhere in the output.
        for tag in [
            "BLANK_AUDIO",
            "MUSIC PLAYING",
            "speaking in foreign language",
            "inaudible",
            "laughing",
            "[",
            "]",
        ] {
            assert!(!text.contains(tag), "artifact survived: {tag}\n{text}");
        }

        // All four loops collapsed, and each repeated phrase appears once.
        assert_eq!(
            result.diagnostics.loops_collapsed.len(),
            4,
            "expected four loops, got {:?}",
            result.diagnostics.loops_collapsed
        );
        for phrase in [
            "pay the firm to fill the form",
            "Earlier it was less",
            "a lot of examples of this",
        ] {
            assert_eq!(
                text.matches(phrase).count(),
                1,
                "phrase should survive exactly once: {phrase}"
            );
        }

        // The mangled proper nouns were normalized, the good ones kept.
        assert!(text.contains("alumni placement support"), "{text}");
        assert!(text.contains("Coursera certificates"), "{text}");
        assert!(text.contains("NavGurukul team"), "{text}");
        assert!(text.contains("Pay Forward"), "{text}");
        assert!(!text.contains("Aluminium"), "{text}");
        assert!(!text.contains("Nagpur Kul"), "{text}");

        // Real speech and exact figures survive.
        assert!(text.contains("(fifty-fifty)"), "{text}");
        assert!(text.contains("62%"), "{text}");

        // The diagnostics report says what was removed and how much.
        let d = &result.diagnostics;
        assert!(d.artifact_total() >= 5, "{:?}", d.artifacts_removed);
        assert!(d.loop_lines_discarded() >= 35, "{}", d.loop_lines_discarded());
        assert!(d.filler_removals >= 4, "{}", d.filler_removals);
        assert_eq!(d.glossary_corrections.len(), 4, "{:?}", d.glossary_corrections);
        assert!(
            d.chars_out < d.chars_in / 2,
            "the fixture is mostly noise: {} -> {}",
            d.chars_in,
            d.chars_out
        );
    }
}
