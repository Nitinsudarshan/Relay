//! The summary length policy.
//!
//! A five-minute check-in and a two-hour planning session are not the same
//! writing problem, and the old fixed per-mode word cap treated them as though
//! they were. It was simultaneously too tight and too loose: a ninety-minute
//! meeting could not be summarized in 550 words, so the model's prose was
//! rejected and the user was handed a deterministic bullet dump; a four-minute
//! call had 550 words of room and nothing stopped it padding into them.
//!
//! The budget here is derived from the meeting instead. It is computed once,
//! **told to the model** in Stage B's prompt, and then used by the validator —
//! so the model is given the target it is judged against rather than only
//! punished for missing it.
//!
//! Mode is a ratio and a ceiling over that budget, not an absolute size. That
//! is what keeps "Detailed" from meaning "long": on a short meeting Detailed
//! still produces a short summary, because there is nothing else to say.

use super::model::SummaryMode;
use serde::{Deserialize, Serialize};

/// Words per minute assumed when converting a transcript's length back into a
/// meeting duration. Deliberately conservative for conversational speech;
/// used only to pick a topic band, never to state a duration to the user.
const WORDS_PER_MINUTE: usize = 130;

/// The fraction of the transcript a summary may occupy, per mode.
///
/// Binding only on short meetings, which is the half that matters: a 200-word
/// transcript cannot honestly yield a 400-word summary, and this is what stops
/// it.
fn transcript_ratio(mode: SummaryMode) -> f64 {
    match mode {
        SummaryMode::Concise => 0.30,
        SummaryMode::Standard => 0.45,
        SummaryMode::Detailed => 0.60,
    }
}

/// The smallest useful summary for a mode. Below this the output stops being a
/// summary and starts being a label.
fn floor_words(mode: SummaryMode) -> usize {
    match mode {
        SummaryMode::Concise => 70,
        SummaryMode::Standard => 100,
        SummaryMode::Detailed => 140,
    }
}

/// How much longer than its budget a summary may run before the prose is
/// rejected rather than merely flagged.
///
/// Overshooting slightly is a style problem; the summary is still correct and
/// still worth reading. Rejecting it would replace good prose with a fact dump,
/// which is a much worse outcome than forty extra words.
pub const OVER_LENGTH_REJECT_FACTOR: f64 = 1.4;

/// A meeting's own summary budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SummaryBudget {
    pub mode: SummaryMode,
    /// Words in the normalized transcript — the surviving content, after
    /// deterministic cleanup has already removed the artifacts.
    pub transcript_words: usize,
    /// What the model is asked to aim for.
    pub target_words: usize,
    /// Past this the summary is flagged; past `OVER_LENGTH_REJECT_FACTOR` times
    /// this it is rejected and repaired.
    pub max_words: usize,
    /// How many distinct topics the Discussion section should carry. Encodes
    /// the length table in `Meeting-rules/meeting_transcript_summary.md` §6.
    pub min_topics: usize,
    pub max_topics: usize,
}

impl SummaryBudget {
    /// The point at which prose is rejected instead of flagged.
    pub fn reject_above_words(&self) -> usize {
        (self.max_words as f64 * OVER_LENGTH_REJECT_FACTOR).round() as usize
    }

    /// Below this a summary is too thin to be a useful record of a meeting this
    /// size. Never applied to meetings that genuinely had little in them.
    pub fn thin_below_words(&self) -> usize {
        (self.target_words / 5).max(8)
    }
}

/// The topic band for a transcript of this length, from §6's duration table.
///
/// Judged on surviving words rather than wall-clock duration, exactly as §6
/// requires: ninety minutes that were sixty minutes of small talk get the band
/// their real content earns, not the one the clock suggests.
fn topic_band(transcript_words: usize) -> (usize, usize) {
    let minutes = transcript_words / WORDS_PER_MINUTE;
    match minutes {
        0..=9 => (1, 2),
        10..=29 => (2, 4),
        30..=59 => (3, 6),
        _ => (4, 7),
    }
}

/// Computes the budget for one meeting.
pub fn summary_budget(transcript_words: usize, mode: SummaryMode) -> SummaryBudget {
    // Proportional to the meeting, floored so a very short one still gets a
    // usable record, and capped by the mode's ceiling. The floor is deliberately
    // allowed to exceed a tiny transcript: a 40-word recording summarized in 20
    // words is not a summary, and the headings alone cost more than that.
    let max_words = (transcript_words as f64 * transcript_ratio(mode))
        .round()
        .max(floor_words(mode) as f64) as usize;
    let max_words = max_words.min(mode.max_words());

    let (mut min_topics, mut max_topics) = topic_band(transcript_words);
    match mode {
        // Concise narrows the band rather than shortening every topic: fewer
        // things said properly beats every topic said in half a line.
        SummaryMode::Concise => max_topics = max_topics.min(min_topics + 1),
        SummaryMode::Standard => {}
        SummaryMode::Detailed => min_topics = (min_topics + 1).min(max_topics),
    }

    SummaryBudget {
        mode,
        transcript_words,
        target_words: ((max_words as f64) * 0.7).round() as usize,
        max_words,
        min_topics,
        max_topics,
    }
}

/// The budget as an instruction the model can act on.
///
/// Phrased as a target with a hard ceiling, and paired with the one rule that
/// makes shortening safe: length is cut by dropping repetition, never by
/// dropping a decision or a commitment.
pub fn budget_guidance(budget: &SummaryBudget) -> String {
    let topics = if budget.min_topics == budget.max_topics {
        format!("{} topic", budget.min_topics)
    } else {
        format!("{} to {} topics", budget.min_topics, budget.max_topics)
    };

    format!(
        "LENGTH — this meeting, not a fixed size\n\
The transcript holds about {transcript} words of real content. Aim for about \
{target} words overall and never exceed {max}. Organize the discussion into \
{topics}; merge topics that recur in different parts of the meeting rather than \
repeating them.\n\
Shorten by removing repetition, conversational framing, and detail that changed \
nothing — never by dropping a decision, a commitment, an owner, a deadline, or \
an unresolved question. A short meeting gets a short summary; do not pad toward \
the ceiling.",
        transcript = budget.transcript_words,
        target = budget.target_words,
        max = budget.max_words,
        topics = topics,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_meeting_cannot_produce_a_long_summary() {
        // The old fixed cap allowed 550 words of "summary" for a two-minute call.
        let budget = summary_budget(250, SummaryMode::Standard);
        assert!(
            budget.max_words < 150,
            "a 250-word transcript got a {}-word budget",
            budget.max_words
        );
        assert!(budget.max_words >= 100, "but still enough to be useful");
    }

    #[test]
    fn a_long_meeting_gets_room_the_fixed_cap_refused() {
        // 9,000 words is roughly 70 minutes. Under the old 550-word Standard cap
        // a legitimate 600-word summary was rejected outright and replaced with
        // the deterministic renderer.
        let budget = summary_budget(9_000, SummaryMode::Standard);
        assert!(budget.max_words >= 600, "got {}", budget.max_words);
        assert_eq!(budget.max_words, SummaryMode::Standard.max_words());
    }

    #[test]
    fn detailed_is_not_a_synonym_for_long() {
        let short_detailed = summary_budget(300, SummaryMode::Detailed);
        let long_concise = summary_budget(9_000, SummaryMode::Concise);
        assert!(
            short_detailed.max_words < long_concise.max_words,
            "a detailed summary of a tiny meeting must stay smaller than a \
concise summary of a long one"
        );
    }

    #[test]
    fn modes_stay_ordered_at_every_meeting_size() {
        for words in [120, 400, 1_500, 5_000, 20_000] {
            let concise = summary_budget(words, SummaryMode::Concise);
            let standard = summary_budget(words, SummaryMode::Standard);
            let detailed = summary_budget(words, SummaryMode::Detailed);
            assert!(
                concise.max_words <= standard.max_words
                    && standard.max_words <= detailed.max_words,
                "modes out of order at {} words",
                words
            );
        }
    }

    #[test]
    fn the_topic_band_follows_the_rules_table() {
        // ~5 min, ~20 min, ~45 min, ~90 min at 130 wpm.
        assert_eq!(topic_band(650), (1, 2));
        assert_eq!(topic_band(2_600), (2, 4));
        assert_eq!(topic_band(5_850), (3, 6));
        assert_eq!(topic_band(11_700), (4, 7));
    }

    #[test]
    fn concise_narrows_the_topic_band_and_detailed_raises_its_floor() {
        let concise = summary_budget(11_700, SummaryMode::Concise);
        let detailed = summary_budget(11_700, SummaryMode::Detailed);
        assert_eq!((concise.min_topics, concise.max_topics), (4, 5));
        assert_eq!((detailed.min_topics, detailed.max_topics), (5, 7));
    }

    #[test]
    fn the_guidance_names_the_numbers_the_validator_will_use() {
        let budget = summary_budget(4_000, SummaryMode::Standard);
        let guidance = budget_guidance(&budget);
        assert!(guidance.contains(&budget.max_words.to_string()));
        assert!(guidance.contains(&budget.target_words.to_string()));
        assert!(
            guidance.contains("never by dropping a decision"),
            "shortening must never be allowed to cost a decision"
        );
    }

    #[test]
    fn rejection_leaves_room_for_a_slight_overrun() {
        let budget = summary_budget(4_000, SummaryMode::Standard);
        assert!(budget.reject_above_words() > budget.max_words);
        // Forty words over budget is a style problem, not grounds for throwing
        // away the whole summary.
        assert!(budget.reject_above_words() >= budget.max_words + 40);
    }

    #[test]
    fn an_empty_transcript_still_yields_a_sane_budget() {
        let budget = summary_budget(0, SummaryMode::Standard);
        assert!(budget.max_words > 0);
        assert!(budget.target_words > 0);
        assert_eq!(budget.min_topics, 1);
    }
}
