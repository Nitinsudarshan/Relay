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
// The rules themselves now live in `capture`, where dictation and Talkback
// can reach them. Re-exported so this module's public surface — and the
// rule names written into every segment — read unchanged.
pub use crate::capture::text_normalize::{
    normalize_segment_text, RULE_ASR_TAGS, RULE_FILLERS, RULE_GLOSSARY, RULE_REPEATED_PHRASES,
    RULE_REPEATED_WORDS, RULE_SENTENCE_BOUNDARIES, RULE_WHITESPACE,
};
use std::collections::BTreeMap;

/// A raw transcript segment as the normalizer receives it. Mirrors the fields
/// of `TranscriptSegment` that normalization is allowed to see, keeping this
/// module free of any dependency on the recorder's types.
#[derive(Debug, Clone)]
pub struct RawSegmentInput {
    pub chunk_index: usize,
    /// Which utterance within the chunk this is, when the recorder resolved the
    /// chunk into utterances. `None` means the input covers the whole chunk —
    /// a transcript recorded before v2.5, or a chunk Whisper returned no timed
    /// spans for.
    pub utterance_index: Option<usize>,
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

/// The stable segment id for one utterance within a chunk.
///
/// Both indices are immutable properties of the raw transcript, so the id
/// survives regeneration exactly as the chunk-level id does.
pub fn utterance_segment_id(chunk_index: usize, utterance_index: usize) -> String {
    format!("seg_{:05}_{:03}", chunk_index, utterance_index)
}

/// The id for a raw input, whichever granularity it carries.
pub fn raw_segment_id(raw: &RawSegmentInput) -> String {
    match raw.utterance_index {
        Some(utterance_index) => utterance_segment_id(raw.chunk_index, utterance_index),
        None => segment_id(raw.chunk_index),
    }
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
            id: raw_segment_id(raw),
            chunk_index: raw.chunk_index,
            utterance_index: raw.utterance_index,
            start_time_s: raw.start_time_s,
            end_time_s: raw.end_time_s,
            text: outcome.text,
            raw_text: raw.text.clone(),
            channel,
            speaker_id: None,
            applied_rules: outcome.applied_rules,
        });
    }

    // Chronological, and stable within a chunk. `utterance_index` is the tie
    // break rather than the timestamp, because two utterances can share a
    // rounded start.
    segments.sort_by(|a, b| {
        a.chunk_index
            .cmp(&b.chunk_index)
            .then(a.utterance_index.cmp(&b.utterance_index))
    });
    let output_char_count = segments.iter().map(|s| s.text.len()).sum();

    NormalizedTranscript {
        segments,
        rule_hits,
        source_char_count,
        output_char_count,
        dropped_segment_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(chunk_index: usize, text: &str) -> RawSegmentInput {
        RawSegmentInput {
            chunk_index,
            utterance_index: None,
            start_time_s: chunk_index as f64 * 30.0,
            end_time_s: (chunk_index + 1) as f64 * 30.0,
            text: text.to_string(),
            mic_had_audio: true,
            sys_had_audio: false,
        }
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
}
