//! Tells speech apart from what Whisper produces when there is no speech.
//!
//! Whisper is a sequence model with no way to say "nothing was said". Handed
//! thirty seconds of room tone, a fan, or a muted call, it emits the highest
//! probability continuation it knows — which, for a model trained on captioned
//! video, is subtitle boilerplate: "Thank you.", "Thanks for watching.",
//! "Subtitles by the Amara.org community". Once it starts, the decoder's own
//! output conditions the rest of the window, so a single such token becomes a
//! loop that fills the whole chunk.
//!
//! Two defences live here, and they are deliberately separate:
//!
//! 1. **Before the decode** — [`profile_speech`] measures how much of a chunk
//!    is actually voiced, at 20 ms resolution and against the chunk's *own*
//!    noise floor. A chunk-wide RMS mean cannot do this: constant hiss at
//!    0.006 RMS clears any fixed threshold for the full thirty seconds while
//!    containing no speech at all, which is exactly how a meeting ends up with
//!    240 seconds of "Thank you."
//!
//! 2. **After the decode** — [`assess`] rejects text that no plausible speech
//!    could have produced: an adjacent phrase loop, subtitle filler over audio
//!    with no voice in it, Whisper's own no-speech probability, or more words
//!    than the voiced time could physically hold.
//!
//! Nothing here rewrites a transcript. A rejection replaces the text with
//! nothing and records what was discarded and why, so the failure is visible
//! in the artifact rather than mistaken for something a person said.

use serde::{Deserialize, Serialize};

/// Frame length for the voiced/unvoiced decision. Short enough that a single
/// word registers, long enough that one loud sample cannot.
const VAD_FRAME_MS: f64 = 20.0;

/// Absolute RMS floor below which a frame is silence whatever the noise floor
/// says. Matches the recorder's own audibility gate so the two stages agree
/// about what "audible" means.
pub const ABSOLUTE_SILENCE_RMS: f32 = 0.004;

/// How far above the chunk's own noise floor a frame must sit to count as
/// voiced — roughly 9.5 dB.
///
/// This factor, not the absolute floor, is what rejects steady background
/// noise. Hiss is flat: every frame sits at the floor, so no frame clears
/// `floor * 3` and the chunk correctly reads as silent. Speech is peaky and
/// clears it easily.
const NOISE_FLOOR_SNR: f32 = 3.0;

/// Percentile of frame energies taken as the noise floor. The quietest tenth of
/// a chunk is room tone in nearly any recording that contains speech at all.
const NOISE_FLOOR_PERCENTILE: f64 = 0.10;

/// Minimum voiced audio a chunk must contain before it is worth decoding.
///
/// Below this there is nothing for Whisper to transcribe, so a decode can only
/// invent. Half a second is under one short word — the gate is there to catch
/// silence, not to trim speech.
pub const MIN_VOICED_SECONDS: f64 = 0.5;

/// Longest phrase, in words, the loop detector looks for.
const MAX_LOOP_PHRASE_WORDS: usize = 8;

/// How many adjacent repeats of one phrase constitute a loop.
const LOOP_MIN_REPEATS: usize = 3;

/// Fraction of a segment's words the loop must cover before the segment is
/// called a loop rather than speech that happens to contain repetition.
///
/// Set high on purpose. "we we we should ship it we should ship it we should
/// ship it tomorrow morning" is a decoder stutter around real content: the loop
/// covers two thirds of it, and the right treatment is the normalizer
/// collapsing the repetition and keeping the sentence — not throwing the
/// sentence away. Only a segment that is *almost entirely* one repeated phrase
/// has no content to protect.
const LOOP_MIN_COVERAGE: f64 = 0.85;

/// Coverage at which a tail is refused as an initial prompt.
///
/// Lower than [`LOOP_MIN_COVERAGE`] because the two decisions are not
/// symmetric: refusing to carry a tail costs one chunk a little cross-boundary
/// context, while carrying a partial loop forward primes the next decode to
/// continue it. The reported failure is what that asymmetry looks like when it
/// is ignored.
const PROMPT_MAX_LOOP_COVERAGE: f64 = 0.5;

/// Whisper's no-speech probability above which a decode is not speech.
/// Deliberately high: this rejects on the model's own certainty, and a
/// confident sentence over quiet audio is caught by the other rules instead.
const NO_SPEECH_REJECT: f32 = 0.85;

/// Voiced seconds below which subtitle filler is hallucination rather than
/// something a person said.
///
/// Deliberately an absolute bound and not a ratio of the window. A 30-second
/// chunk holding two seconds of speech and the words "Thank you." is far more
/// likely to be someone thanking someone than a hallucination, and rejecting
/// it deletes something that was said — which is the worse error of the two,
/// because a stray polite phrase costs the summary nothing while lost speech is
/// unrecoverable. Under a second of voice there was nothing there to say it.
const FILLER_MAX_VOICED_SECONDS: f64 = 1.0;

/// Voiced ratio below which filler is suspect, used only together with the
/// model's own doubt.
const FILLER_MAX_VOICED_RATIO: f64 = 0.35;

/// No-speech probability at which filler is rejected even though the voiced
/// time could have held it — the model itself doubting the span.
const FILLER_NO_SPEECH_HINT: f32 = 0.5;

/// Words per voiced second above which the text cannot have been spoken.
/// Fast conversational speech is 2–4; auctioneers reach 6.
const MAX_WORDS_PER_VOICED_SECOND: f64 = 8.0;

/// Longest discarded text kept for inspection. Enough to recognise the failure,
/// not enough for a three-thousand-word loop to bloat the transcript.
const DISCARDED_TEXT_LIMIT: usize = 320;

/// Subtitle boilerplate Whisper emits over non-speech.
///
/// Every entry is a *whole utterance* Whisper produces from silence, music, or
/// tone — never a substring test, so "thank you for taking that on" is
/// untouched. Compared after lowercasing and stripping punctuation.
const FILLER_PHRASES: &[&str] = &[
    "thank you",
    "thanks",
    "thank you very much",
    "thank you so much",
    "thanks for watching",
    "thank you for watching",
    "thanks for listening",
    "thank you for listening",
    "please subscribe",
    "subscribe to my channel",
    "like and subscribe",
    "dont forget to subscribe",
    "subtitles by the amaraorg community",
    "subtitles by the amara org community",
    "amaraorg",
    "transcription by castingwords",
    "transcribed by hiroshi",
    "copyright",
    "all rights reserved",
    "the end",
    "bye",
    "bye bye",
    "goodbye",
    "okay",
    "ok",
    "yeah",
    "mm",
    "mmm",
    "hmm",
    "uh",
    "um",
    "you",
    "so",
    "music",
    "applause",
    "silence",
    "laughter",
    "blank audio",
    "inaudible",
    "foreign",
];

/// How much of a chunk of audio is actually voice.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SpeechProfile {
    /// Seconds of audio that cleared both the absolute floor and the noise
    /// floor.
    pub voiced_seconds: f64,
    pub total_seconds: f64,
    /// Loudest single sample in the chunk.
    pub peak_amplitude: f32,
    /// RMS across the whole chunk — the measurement the pre-v2.6 gate used
    /// alone, kept because it is what makes the failure legible in diagnostics.
    pub rms: f32,
    /// The chunk's own noise floor, as frame RMS.
    pub noise_floor_rms: f32,
}

impl SpeechProfile {
    /// Fraction of the chunk that was voiced, `0.0..=1.0`.
    pub fn voiced_ratio(&self) -> f64 {
        if self.total_seconds <= 0.0 {
            return 0.0;
        }
        (self.voiced_seconds / self.total_seconds).clamp(0.0, 1.0)
    }

    /// Whether this chunk contains enough voice to be worth decoding.
    pub fn is_worth_decoding(&self) -> bool {
        self.voiced_seconds >= MIN_VOICED_SECONDS
    }
}

/// Measures voiced time in a chunk of 16 kHz mono audio.
///
/// The noise floor is taken from the chunk itself, which is what makes this
/// robust to a hot microphone, a noisy room, or a laptop fan: all of them raise
/// the floor together with the frames, so the *contrast* that marks speech is
/// what is measured rather than absolute loudness.
pub fn profile_speech(samples: &[f32], sample_rate: u32) -> SpeechProfile {
    let sample_rate = sample_rate.max(1);
    let total_seconds = samples.len() as f64 / sample_rate as f64;
    if samples.is_empty() {
        return SpeechProfile {
            voiced_seconds: 0.0,
            total_seconds: 0.0,
            peak_amplitude: 0.0,
            rms: 0.0,
            noise_floor_rms: 0.0,
        };
    }

    let frame_len = ((sample_rate as f64 * VAD_FRAME_MS / 1000.0) as usize).max(1);
    let frame_seconds = frame_len as f64 / sample_rate as f64;

    let mut frame_rms: Vec<f32> = Vec::with_capacity(samples.len() / frame_len + 1);
    for frame in samples.chunks(frame_len) {
        let sum_sq: f32 = frame.iter().map(|&s| s * s).sum();
        frame_rms.push((sum_sq / frame.len() as f32).sqrt());
    }

    let peak_amplitude = samples.iter().fold(0.0f32, |acc, &s| acc.max(s.abs()));
    let total_sum_sq: f32 = samples.iter().map(|&s| s * s).sum();
    let rms = (total_sum_sq / samples.len() as f32).sqrt();

    let mut sorted = frame_rms.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let floor_index = ((sorted.len() as f64 * NOISE_FLOOR_PERCENTILE) as usize)
        .min(sorted.len().saturating_sub(1));
    let noise_floor_rms = sorted[floor_index];

    let threshold = (noise_floor_rms * NOISE_FLOOR_SNR).max(ABSOLUTE_SILENCE_RMS);
    let voiced_frames = frame_rms.iter().filter(|&&r| r > threshold).count();

    SpeechProfile {
        voiced_seconds: voiced_frames as f64 * frame_seconds,
        total_seconds,
        peak_amplitude,
        rms,
        noise_floor_rms,
    }
}

/// Why decoded text was rejected as something other than speech.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HallucinationReason {
    /// One phrase repeated back to back for most of the segment — the shape a
    /// Whisper decoder loop takes.
    RepetitionLoop { phrase: String, repeats: usize },
    /// Subtitle boilerplate over audio that contained no voice.
    FillerOverSilence {
        phrase: String,
        voiced_seconds: f64,
        voiced_ratio: f64,
    },
    /// Whisper's own no-speech probability for the span.
    NoSpeech { probability: f32 },
    /// More words than the measured voiced time could hold.
    ImplausibleRate {
        words: usize,
        voiced_seconds: f64,
        words_per_second: f64,
    },
}

impl HallucinationReason {
    /// A single line for a log or a diagnostics row.
    pub fn describe(&self) -> String {
        match self {
            Self::RepetitionLoop { phrase, repeats } => {
                format!("decoder loop: \"{phrase}\" repeated {repeats} times")
            }
            Self::FillerOverSilence {
                phrase,
                voiced_seconds,
                voiced_ratio,
            } => format!(
                "subtitle filler \"{phrase}\" over {voiced_seconds:.1}s of voice ({:.0}% of the span)",
                voiced_ratio * 100.0
            ),
            Self::NoSpeech { probability } => {
                format!("Whisper reported no speech (p={probability:.2})")
            }
            Self::ImplausibleRate {
                words,
                voiced_seconds,
                words_per_second,
            } => format!(
                "{words} words over {voiced_seconds:.1}s of voice ({words_per_second:.1} words/s)"
            ),
        }
    }

    /// A short, stable key for counting reasons in diagnostics.
    pub fn key(&self) -> &'static str {
        match self {
            Self::RepetitionLoop { .. } => "repetition_loop",
            Self::FillerOverSilence { .. } => "filler_over_silence",
            Self::NoSpeech { .. } => "no_speech",
            Self::ImplausibleRate { .. } => "implausible_rate",
        }
    }
}

/// A rejected decode, recorded on the transcript segment in place of text.
///
/// Kept rather than dropped so the raw transcript stays the diagnostic source
/// it claims to be: the discarded text is what proves the rejection was right.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranscriptRejection {
    pub reason: HallucinationReason,
    /// The text that was thrown away, truncated for storage.
    pub discarded_text: String,
    /// True when `discarded_text` is a prefix of what was actually discarded.
    #[serde(default)]
    pub truncated: bool,
    pub discarded_word_count: usize,
}

/// Everything the assessment is allowed to look at.
#[derive(Debug, Clone, Copy)]
pub struct DecodeEvidence {
    /// Voiced time measured on the audio that was decoded.
    pub voiced_seconds: f64,
    pub total_seconds: f64,
    /// Whisper's no-speech probability, averaged across the decode's spans.
    pub mean_no_speech_prob: f32,
}

impl DecodeEvidence {
    pub fn voiced_ratio(&self) -> f64 {
        if self.total_seconds <= 0.0 {
            return 0.0;
        }
        (self.voiced_seconds / self.total_seconds).clamp(0.0, 1.0)
    }
}

/// Decides whether decoded text is speech.
///
/// Returns `None` when the text is trustworthy. The order of the rules is the
/// order of their confidence: a loop is a loop regardless of the audio, whereas
/// filler needs the audio to agree before a real "Thank you." is thrown away.
pub fn assess(text: &str, evidence: DecodeEvidence) -> Option<HallucinationReason> {
    let words = word_count(text);
    if words == 0 {
        return None;
    }

    if let Some(loop_hit) = dominant_repeat(text) {
        if loop_hit.repeats >= LOOP_MIN_REPEATS
            && loop_hit.covered_words as f64 / words as f64 >= LOOP_MIN_COVERAGE
        {
            return Some(HallucinationReason::RepetitionLoop {
                phrase: loop_hit.phrase,
                repeats: loop_hit.repeats,
            });
        }
    }

    if evidence.mean_no_speech_prob >= NO_SPEECH_REJECT {
        return Some(HallucinationReason::NoSpeech {
            probability: evidence.mean_no_speech_prob,
        });
    }

    let voiced_ratio = evidence.voiced_ratio();
    let too_little_voice = evidence.voiced_seconds < FILLER_MAX_VOICED_SECONDS;
    let model_doubts_it = evidence.mean_no_speech_prob >= FILLER_NO_SPEECH_HINT
        && voiced_ratio < FILLER_MAX_VOICED_RATIO;
    if too_little_voice || model_doubts_it {
        if let Some(phrase) = filler_only_phrase(text) {
            return Some(HallucinationReason::FillerOverSilence {
                phrase,
                voiced_seconds: evidence.voiced_seconds,
                voiced_ratio,
            });
        }
    }

    if evidence.voiced_seconds > 0.0 {
        let rate = words as f64 / evidence.voiced_seconds;
        if rate > MAX_WORDS_PER_VOICED_SECOND {
            return Some(HallucinationReason::ImplausibleRate {
                words,
                voiced_seconds: evidence.voiced_seconds,
                words_per_second: rate,
            });
        }
    }

    None
}

/// Builds the record kept in place of rejected text.
pub fn rejection(reason: HallucinationReason, discarded: &str) -> TranscriptRejection {
    let trimmed = discarded.trim();
    let (kept, truncated) = if trimmed.chars().count() > DISCARDED_TEXT_LIMIT {
        let cut: String = trimmed.chars().take(DISCARDED_TEXT_LIMIT).collect();
        (cut, true)
    } else {
        (trimmed.to_string(), false)
    };

    TranscriptRejection {
        reason,
        discarded_text: kept,
        truncated,
        discarded_word_count: word_count(trimmed),
    }
}

/// The longest adjacent phrase repetition in a piece of text.
struct RepeatHit {
    phrase: String,
    repeats: usize,
    covered_words: usize,
}

/// Finds the adjacent phrase repetition covering the most words.
///
/// Scans phrase lengths from one word up to [`MAX_LOOP_PHRASE_WORDS`]. For each
/// starting position it counts how many times the phrase immediately repeats,
/// and keeps whichever run covers the most of the text. That is what
/// distinguishes "we should ship it, we should ship it, we should ship it" from
/// a sentence that merely reuses a word.
fn dominant_repeat(text: &str) -> Option<RepeatHit> {
    let words: Vec<String> = text
        .split_whitespace()
        .map(normalize_word)
        .filter(|w| !w.is_empty())
        .collect();
    if words.len() < 2 {
        return None;
    }

    let mut best: Option<RepeatHit> = None;

    for len in 1..=MAX_LOOP_PHRASE_WORDS.min(words.len() / 2) {
        let mut start = 0usize;
        while start + len * 2 <= words.len() {
            let phrase = &words[start..start + len];
            let mut repeats = 1usize;
            while start + len * (repeats + 1) <= words.len()
                && &words[start + len * repeats..start + len * (repeats + 1)] == phrase
            {
                repeats += 1;
            }

            if repeats > 1 {
                let covered = repeats * len;
                if best.as_ref().is_none_or(|b| covered > b.covered_words) {
                    best = Some(RepeatHit {
                        phrase: phrase.join(" "),
                        repeats,
                        covered_words: covered,
                    });
                }
                start += len * repeats;
            } else {
                start += 1;
            }
        }
    }

    best
}

/// The filler phrase a segment consists entirely of, if it does.
///
/// Splits on sentence punctuation and requires *every* resulting clause to be
/// filler. A segment mixing filler with real speech is real speech.
fn filler_only_phrase(text: &str) -> Option<String> {
    let clauses: Vec<String> = text
        .split(['.', '!', '?', ',', ';', '\n'])
        .map(|clause| {
            clause
                .split_whitespace()
                .map(normalize_word)
                .filter(|w| !w.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|clause| !clause.is_empty())
        .collect();

    if clauses.is_empty() {
        return None;
    }
    if clauses.iter().all(|c| FILLER_PHRASES.contains(&c.as_str())) {
        return Some(clauses[0].clone());
    }
    None
}

/// Lowercased, punctuation-free form of a word, for comparison only.
fn normalize_word(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn word_count(text: &str) -> usize {
    text.split_whitespace().filter(|w| !w.is_empty()).count()
}

/// Whether a piece of text is safe to hand to the next chunk's decode as its
/// initial prompt.
///
/// This is the other half of the loop fix, and the half that let one bad chunk
/// become nine. Whisper treats the initial prompt as preceding text, so
/// prompting chunk *n+1* with "Thank you. Thank you." makes the same
/// continuation overwhelmingly likely again. Discarding the state between
/// chunks does not help when the prompt carries the loop across the boundary.
pub fn is_safe_as_prompt(text: &str) -> bool {
    let words = word_count(text);
    if words == 0 {
        return true;
    }
    if let Some(hit) = dominant_repeat(text) {
        if hit.repeats >= 2 && hit.covered_words as f64 / words as f64 >= PROMPT_MAX_LOOP_COVERAGE {
            return false;
        }
    }
    filler_only_phrase(text).is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 30 seconds of the exact failure the user reported: a fan and nothing
    /// else. RMS clears the old fixed gate; no frame clears the noise floor.
    fn steady_hiss(seconds: f64) -> Vec<f32> {
        let n = (16_000.0 * seconds) as usize;
        (0..n)
            .map(|i| {
                // Deterministic pseudo-noise at ~0.006 RMS — above the recorder's
                // 0.004 audibility gate, which is why the chunk used to be decoded.
                let x = ((i as f32 * 12.9898).sin() * 43758.547).fract();
                (x - 0.5) * 0.021
            })
            .collect()
    }

    fn speech_like(seconds: f64) -> Vec<f32> {
        let n = (16_000.0 * seconds) as usize;
        (0..n)
            .map(|i| {
                let t = i as f32 / 16_000.0;
                // A voiced burst every 500 ms, silence between: peaky, like speech.
                let envelope = if (t * 2.0).fract() < 0.6 { 0.3 } else { 0.001 };
                envelope * (2.0 * std::f32::consts::PI * 180.0 * t).sin()
            })
            .collect()
    }

    #[test]
    fn steady_background_noise_reads_as_silence_however_loud_its_mean() {
        let profile = profile_speech(&steady_hiss(30.0), 16_000);
        assert!(
            profile.rms > ABSOLUTE_SILENCE_RMS,
            "the fixture must clear the old fixed gate, or it is not the bug — rms was {}",
            profile.rms
        );
        assert!(
            !profile.is_worth_decoding(),
            "hiss must not be decoded; voiced_seconds was {}",
            profile.voiced_seconds
        );
    }

    #[test]
    fn speech_clears_the_gate() {
        let profile = profile_speech(&speech_like(30.0), 16_000);
        assert!(profile.is_worth_decoding());
        assert!(profile.voiced_ratio() > 0.3, "ratio {}", profile.voiced_ratio());
    }

    #[test]
    fn an_empty_chunk_is_not_worth_decoding() {
        let profile = profile_speech(&[], 16_000);
        assert_eq!(profile.voiced_seconds, 0.0);
        assert_eq!(profile.voiced_ratio(), 0.0);
        assert!(!profile.is_worth_decoding());
    }

    #[test]
    fn digital_silence_is_not_worth_decoding() {
        let profile = profile_speech(&vec![0.0; 16_000 * 30], 16_000);
        assert!(!profile.is_worth_decoding());
        assert_eq!(profile.noise_floor_rms, 0.0);
    }

    fn evidence(voiced: f64, total: f64, no_speech: f32) -> DecodeEvidence {
        DecodeEvidence {
            voiced_seconds: voiced,
            total_seconds: total,
            mean_no_speech_prob: no_speech,
        }
    }

    #[test]
    fn the_reported_failure_is_rejected() {
        // Verbatim shape of the chunk 11–19 output.
        let text = "Thank you. ".repeat(73);
        let reason = assess(&text, evidence(0.2, 30.0, 0.4))
            .expect("240 seconds of \"Thank you.\" is not speech");
        match reason {
            HallucinationReason::RepetitionLoop { repeats, .. } => {
                assert!(repeats >= LOOP_MIN_REPEATS, "repeats {repeats}")
            }
            other => panic!("expected a repetition loop, got {other:?}"),
        }
    }

    #[test]
    fn a_loop_is_rejected_even_when_the_audio_looks_fine() {
        // The audio evidence is deliberately healthy: a loop is self-evident.
        let text = "we should ship it we should ship it we should ship it we should ship it";
        assert!(assess(text, evidence(20.0, 30.0, 0.01)).is_some());
    }

    #[test]
    fn one_polite_thank_you_over_real_speech_survives() {
        let text = "Thank you, that covers everything I needed for the release.";
        assert_eq!(assess(text, evidence(4.0, 6.0, 0.02)), None);
    }

    #[test]
    fn a_single_thank_you_over_silence_is_rejected() {
        let reason = assess("Thank you.", evidence(0.1, 30.0, 0.3))
            .expect("filler over silence is not speech");
        assert!(matches!(
            reason,
            HallucinationReason::FillerOverSilence { .. }
        ));
    }

    #[test]
    fn a_single_thank_you_over_speech_is_kept() {
        // Same phrase, audio that actually contained a voice. Rejecting this
        // would be deleting something the user said.
        assert_eq!(assess("Thank you.", evidence(1.2, 2.0, 0.05)), None);
    }

    #[test]
    fn a_closing_thank_you_in_a_mostly_quiet_chunk_is_kept() {
        // The end of a call: thirty seconds of window, two seconds of speech.
        // The ratio is low, the model is confident, and someone said it.
        assert_eq!(assess("Thank you.", evidence(1.8, 30.0, 0.02)), None);
    }

    #[test]
    fn filler_the_model_itself_doubts_is_rejected_even_with_voice_in_the_window() {
        let reason = assess("Thank you.", evidence(2.0, 30.0, 0.7))
            .expect("the model doubting a filler span over a quiet window is not speech");
        assert!(matches!(
            reason,
            HallucinationReason::FillerOverSilence { .. }
        ));
    }

    #[test]
    fn whispers_own_no_speech_probability_rejects() {
        let reason = assess("The quarterly numbers look strong.", evidence(9.0, 30.0, 0.94))
            .expect("the model said this was not speech");
        assert!(matches!(reason, HallucinationReason::NoSpeech { .. }));
    }

    #[test]
    fn more_words_than_the_voiced_time_could_hold_is_rejected() {
        let text = (0..120)
            .map(|i| format!("word{i}"))
            .collect::<Vec<_>>()
            .join(" ");
        let reason = assess(&text, evidence(2.0, 30.0, 0.1)).expect("60 words/s is not speech");
        assert!(matches!(
            reason,
            HallucinationReason::ImplausibleRate { .. }
        ));
    }

    #[test]
    fn ordinary_meeting_speech_is_never_rejected() {
        let text = "So the plan is to ship the migration on Friday, and Pranjali will take the \
review. If the vault rewrite slips we push to Monday rather than shipping half of it.";
        assert_eq!(assess(text, evidence(11.0, 14.0, 0.02)), None);
    }

    #[test]
    fn a_decoder_stutter_around_real_content_is_kept_for_the_normalizer_to_clean() {
        // The loop covers two thirds of this, but the sentence it surrounds is
        // real. Rejecting it would delete speech that only needed collapsing.
        let text = "we we we should ship it we should ship it we should ship it tomorrow \
morning at the latest";
        assert_eq!(assess(text, evidence(6.0, 8.0, 0.02)), None);
    }

    #[test]
    fn deliberate_repetition_for_emphasis_survives() {
        let text = "This is important, really important. We cannot ship this without the \
migration finished, tested, and reviewed by someone other than me.";
        assert_eq!(assess(text, evidence(9.0, 12.0, 0.03)), None);
    }

    #[test]
    fn a_looped_tail_is_never_carried_into_the_next_decode() {
        assert!(!is_safe_as_prompt("Thank you. Thank you. Thank you."));
        assert!(!is_safe_as_prompt("Thank you."));
        assert!(!is_safe_as_prompt("Thanks for watching"));
        assert!(is_safe_as_prompt("and then we agreed to ship on Friday"));
        assert!(is_safe_as_prompt(""));
    }

    #[test]
    fn a_rejection_records_what_it_discarded_without_storing_all_of_it() {
        let text = "Thank you. ".repeat(300);
        let record = rejection(
            HallucinationReason::RepetitionLoop {
                phrase: "thank you".into(),
                repeats: 300,
            },
            &text,
        );
        assert!(record.truncated);
        assert!(record.discarded_text.chars().count() <= DISCARDED_TEXT_LIMIT);
        assert_eq!(record.discarded_word_count, 600);
        assert!(record.reason.describe().contains("decoder loop"));
        assert_eq!(record.reason.key(), "repetition_loop");
    }

    #[test]
    fn short_discarded_text_is_kept_whole() {
        let record = rejection(
            HallucinationReason::NoSpeech { probability: 0.9 },
            "  Thank you.  ",
        );
        assert!(!record.truncated);
        assert_eq!(record.discarded_text, "Thank you.");
    }

    #[test]
    fn the_loop_detector_prefers_the_run_that_covers_the_most_words() {
        let hit = dominant_repeat("alpha beta alpha beta alpha beta gamma").unwrap();
        assert_eq!(hit.phrase, "alpha beta");
        assert_eq!(hit.repeats, 3);
        assert_eq!(hit.covered_words, 6);
    }

    #[test]
    fn punctuation_and_casing_do_not_hide_a_loop() {
        let hit = dominant_repeat("Thank you. thank you, THANK YOU!").unwrap();
        assert_eq!(hit.repeats, 3);
    }

    #[test]
    fn empty_and_single_word_text_are_not_loops() {
        assert!(dominant_repeat("").is_none());
        assert!(dominant_repeat("hello").is_none());
        assert_eq!(assess("", evidence(0.0, 30.0, 0.9)), None);
    }
}
