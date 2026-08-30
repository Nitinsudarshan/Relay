//! The phrase buffer between the LLM stream and TTS.
//!
//! Tokens must never reach a TTS engine — synthesising "the" alone
//! produces the wrong prosody and one process spawn per word. Whole
//! answers must not either, or time-to-first-audio is the cost of the
//! entire generation. Sentences are the right unit, and this is what
//! turns a token stream into them.
//!
//! This is the single mechanism that lets Piper — a batch engine that
//! writes a WAV file and exits — behave like a streaming one:
//! time-to-first-audio becomes the cost of the *first sentence*.

/// Below this, a fragment is not worth a separate synthesis: "Yes." on its
/// own gets glued to the sentence after it.
const MIN_PHRASE_CHARS: usize = 16;

/// Past this, wait for a clause boundary rather than a full stop — a
/// model that writes a 90-word sentence should not stall the audio.
const SOFT_MAX_CHARS: usize = 140;

/// Past this, emit at the next whitespace regardless. Bounds the worst
/// case for a model that never punctuates.
const HARD_MAX_CHARS: usize = 260;

/// Abbreviations whose trailing period is not a sentence boundary.
const ABBREVIATIONS: &[&str] = &[
    "mr", "mrs", "ms", "dr", "prof", "sr", "jr", "st", "vs", "etc", "eg", "ie", "approx", "no",
    "fig", "inc", "ltd", "co", "dept", "est", "min", "max", "vol", "cf", "al",
];

/// Accumulates streamed text and releases complete, speakable phrases.
#[derive(Debug, Default)]
pub struct PhraseBuffer {
    pending: String,
}

impl PhraseBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds a delta from the LLM stream, returning any phrases that are
    /// now complete. Usually empty; occasionally one; rarely more.
    pub fn push(&mut self, delta: &str) -> Vec<String> {
        self.pending.push_str(delta);
        let mut out = Vec::new();
        while let Some(split_at) = self.find_split() {
            let rest = self.pending.split_off(split_at);
            let phrase = std::mem::replace(&mut self.pending, rest);
            let phrase = phrase.trim().to_string();
            if !phrase.is_empty() {
                out.push(phrase);
            }
        }
        out
    }

    /// Flushes whatever is left when the stream ends.
    pub fn finish(&mut self) -> Option<String> {
        let remainder = std::mem::take(&mut self.pending).trim().to_string();
        if remainder.is_empty() {
            None
        } else {
            Some(remainder)
        }
    }

    /// True when nothing is buffered.
    pub fn is_empty(&self) -> bool {
        self.pending.trim().is_empty()
    }

    /// Byte index just past the end of a releasable phrase, if there is
    /// one.
    fn find_split(&self) -> Option<usize> {
        let chars: Vec<(usize, char)> = self.pending.char_indices().collect();
        if chars.is_empty() {
            return None;
        }

        let mut chars_seen = 0_usize;
        let mut last_clause_break: Option<usize> = None;
        let mut last_space: Option<usize> = None;

        for (position, (byte_index, ch)) in chars.iter().enumerate() {
            chars_seen += 1;
            let next = chars.get(position + 1).map(|(_, c)| *c);
            let end_of_char = byte_index + ch.len_utf8();

            if ch.is_whitespace() {
                last_space = Some(*byte_index);
            }

            // A newline always ends a phrase — a model emitting a bullet
            // list should not have its items run together.
            if *ch == '\n' && chars_seen >= MIN_PHRASE_CHARS {
                return Some(end_of_char);
            }

            // `।` is the Devanagari danda. Relay's STT is configured for
            // English/Hindi code-switching, so a Hindi answer that ended
            // only at danda would otherwise never chunk — one enormous
            // phrase, and no streaming audio at all.
            let is_terminator = matches!(ch, '.' | '!' | '?' | '।' | '॥' | '。' | '？' | '！');
            if is_terminator
                && chars_seen >= MIN_PHRASE_CHARS
                && self.is_sentence_end(&chars, position, next)
            {
                // Absorb a closing quote or bracket so it is not stranded
                // at the head of the next phrase.
                let mut end = end_of_char;
                if let Some(c) = next {
                    if matches!(c, '"' | '\'' | ')' | ']' | '”' | '’') {
                        end += c.len_utf8();
                    }
                }
                return Some(end);
            }

            if matches!(ch, ',' | ';' | ':' | '—') {
                last_clause_break = Some(end_of_char);
            }

            if chars_seen >= SOFT_MAX_CHARS {
                if let Some(brk) = last_clause_break {
                    return Some(brk);
                }
            }

            if chars_seen >= HARD_MAX_CHARS {
                return Some(last_space.map(|s| s + 1).unwrap_or(end_of_char));
            }
        }

        None
    }

    /// Distinguishes a real full stop from a decimal point, an ellipsis,
    /// an abbreviation, or a period the stream has not finished yet.
    fn is_sentence_end(
        &self,
        chars: &[(usize, char)],
        position: usize,
        next: Option<char>,
    ) -> bool {
        let ch = chars[position].1;

        if ch == '.' {
            let prev = position.checked_sub(1).map(|i| chars[i].1);

            // "3.14" — a decimal, not a stop.
            if let (Some(p), Some(n)) = (prev, next) {
                if p.is_ascii_digit() && n.is_ascii_digit() {
                    return false;
                }
            }

            // "..." — wait for the last one.
            if next == Some('.') || prev == Some('.') {
                return false;
            }

            // "Dr." / "e.g." — read the word before the period.
            let mut word = String::new();
            for i in (0..position).rev() {
                let c = chars[i].1;
                if c.is_alphanumeric() {
                    word.insert(0, c.to_ascii_lowercase());
                } else if c == '.' {
                    // Keep walking through an inner period so "e.g" is
                    // seen as "eg" rather than as "g".
                    continue;
                } else {
                    break;
                }
            }
            if ABBREVIATIONS.contains(&word.as_str()) {
                return false;
            }
        }

        // A terminator at the very end of the buffer might still be
        // mid-token; wait for whatever follows unless the buffer is
        // already long enough that waiting hurts more than splitting.
        match next {
            None => chars.len() >= SOFT_MAX_CHARS,
            Some(c) => c.is_whitespace() || matches!(c, '"' | '\'' | ')' | ']' | '”' | '’'),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feeds text one character at a time, the worst case a token stream
    /// can produce, and collects everything released.
    fn stream_char_by_char(text: &str) -> Vec<String> {
        let mut buffer = PhraseBuffer::new();
        let mut out = Vec::new();
        for ch in text.chars() {
            out.extend(buffer.push(&ch.to_string()));
        }
        if let Some(rest) = buffer.finish() {
            out.push(rest);
        }
        out
    }

    /// The cost of turning a token stream into speakable phrases. Must be
    /// negligible next to synthesis, or the buffer would be adding latency
    /// to the thing it exists to reduce.
    #[test]
    #[ignore = "benchmark, not a correctness test"]
    fn phrase_buffer_throughput() {
        let answer = "You settled on the flat seat licence. Procurement wanted an annual \
                      number they could budget against, and that outweighed the upside from \
                      usage-based pricing. The decision was recorded in Tuesday's review. ";
        let mut buffer = PhraseBuffer::new();
        let started = std::time::Instant::now();
        let mut phrases = 0_usize;
        const RUNS: usize = 2_000;
        for _ in 0..RUNS {
            for ch in answer.chars() {
                phrases += buffer.push(&ch.to_string()).len();
            }
        }
        let elapsed = started.elapsed();
        let chars = RUNS * answer.chars().count();
        println!(
            "phrase buffer: {} chars -> {:.2} ms total ({:.4} us/char), {} phrases",
            chars,
            elapsed.as_secs_f64() * 1000.0,
            elapsed.as_secs_f64() * 1_000_000.0 / chars as f64,
            phrases
        );
    }

    #[test]
    fn releases_whole_sentences() {
        let phrases =
            stream_char_by_char("We shipped the flat licence. Procurement approved it. Done.");
        assert_eq!(
            phrases,
            vec![
                "We shipped the flat licence.",
                "Procurement approved it.",
                "Done."
            ]
        );
    }

    #[test]
    fn never_releases_a_bare_token() {
        let mut buffer = PhraseBuffer::new();
        for token in ["The", " pricing", " model", " is"] {
            assert!(
                buffer.push(token).is_empty(),
                "released mid-sentence on {token}"
            );
        }
    }

    #[test]
    fn a_decimal_point_is_not_a_sentence_end() {
        let phrases = stream_char_by_char("The rate landed at 3.14 percent after review.");
        assert_eq!(phrases, vec!["The rate landed at 3.14 percent after review."]);
    }

    #[test]
    fn abbreviations_do_not_split() {
        let phrases = stream_char_by_char("Dr. Patel signed off on the change yesterday.");
        assert_eq!(
            phrases,
            vec!["Dr. Patel signed off on the change yesterday."]
        );
    }

    #[test]
    fn ellipsis_does_not_split_three_ways() {
        let phrases = stream_char_by_char("Well... it depends on procurement entirely.");
        assert_eq!(phrases.len(), 1, "got {:?}", phrases);
    }

    #[test]
    fn a_short_fragment_is_glued_to_the_next_sentence() {
        let phrases = stream_char_by_char("Yes. Procurement approved the flat licence.");
        assert_eq!(
            phrases,
            vec!["Yes. Procurement approved the flat licence."],
            "a two-word fragment is not worth its own synthesis"
        );
    }

    #[test]
    fn questions_and_exclamations_split() {
        let phrases = stream_char_by_char("Did procurement approve it? They did, yesterday!");
        assert_eq!(
            phrases,
            vec!["Did procurement approve it?", "They did, yesterday!"]
        );
    }

    #[test]
    fn a_long_unpunctuated_run_splits_at_a_clause_boundary() {
        let text = "we talked about pricing and procurement and the seat licence and the annual \
                    number they can budget against, which is the thing that actually matters here";
        let phrases = stream_char_by_char(text);
        assert!(phrases.len() >= 2, "never split: {:?}", phrases);
        assert!(phrases[0].ends_with(','), "split off a clause: {:?}", phrases[0]);
    }

    #[test]
    fn a_run_with_no_punctuation_at_all_still_splits() {
        let text = "word ".repeat(120);
        let phrases = stream_char_by_char(&text);
        assert!(phrases.len() > 1, "hard cap never fired");
        for phrase in &phrases {
            assert!(
                phrase.chars().count() <= HARD_MAX_CHARS + 4,
                "phrase overshot the hard cap: {} chars",
                phrase.chars().count()
            );
        }
    }

    #[test]
    fn newlines_end_a_phrase() {
        let phrases = stream_char_by_char("First the pricing point\nThen the procurement point\n");
        assert_eq!(
            phrases,
            vec!["First the pricing point", "Then the procurement point"]
        );
    }

    #[test]
    fn closing_quotes_stay_with_their_sentence() {
        let phrases = stream_char_by_char("She said \"we ship it Friday.\" Everyone agreed on that.");
        assert_eq!(phrases.len(), 2, "got {:?}", phrases);
        assert!(phrases[0].ends_with('"'), "got {:?}", phrases[0]);
    }

    #[test]
    fn finish_flushes_an_unterminated_tail() {
        let mut buffer = PhraseBuffer::new();
        buffer.push("This answer was cut off mid");
        assert_eq!(
            buffer.finish().as_deref(),
            Some("This answer was cut off mid")
        );
        assert!(buffer.finish().is_none(), "finish must not repeat itself");
    }

    #[test]
    fn empty_and_whitespace_input_release_nothing() {
        let mut buffer = PhraseBuffer::new();
        assert!(buffer.push("").is_empty());
        assert!(buffer.push("   \n  ").is_empty());
        assert!(buffer.is_empty());
        assert!(buffer.finish().is_none());
    }

    #[test]
    fn chunked_and_char_by_char_streams_agree() {
        let text = "Procurement approved the flat licence. The annual number is what they need. \
                    We ship in March.";
        let per_char = stream_char_by_char(text);

        let mut buffer = PhraseBuffer::new();
        let mut per_chunk = Vec::new();
        for chunk in text.as_bytes().chunks(7) {
            per_chunk.extend(buffer.push(std::str::from_utf8(chunk).unwrap()));
        }
        if let Some(rest) = buffer.finish() {
            per_chunk.push(rest);
        }
        assert_eq!(per_char, per_chunk);
    }

    #[test]
    fn multibyte_text_does_not_split_a_character() {
        let phrases = stream_char_by_char(
            "हमने फ्लैट लाइसेंस भेजा है। खरीद टीम सहमत हो गई है। बहुत बढ़िया रहा।",
        );
        assert!(phrases.len() >= 2, "got {:?}", phrases);
        for phrase in &phrases {
            assert!(!phrase.is_empty());
        }
    }
}
