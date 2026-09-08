//! Hedged generation — a duplicate request when the first one is slow.
//!
//! The tail, not the average, is what makes a spoken assistant feel broken. A
//! turn that usually answers in under a second and occasionally takes eight is
//! worse than one that always takes two, because the user has already started
//! talking again by then. A hedge bounds that tail: if the first request has
//! produced nothing by [`HEDGE_AFTER`], send a second and speak whichever
//! answers first.
//!
//! # Why this is not on by default for a local model
//!
//! The pattern comes from cloud voice infrastructure, where a hedge reaches
//! *different capacity* — another server, another route — so the second request
//! is genuinely an independent draw from the latency distribution. That is what
//! makes it a hedge rather than a coin flipped twice.
//!
//! Against a local Ollama, it is not independent and cannot help:
//!
//! * Ollama serialises requests per model by default (`OLLAMA_NUM_PARALLEL`),
//!   so the duplicate queues behind the request it was meant to overtake.
//! * Where it *is* configured to run them in parallel, both inferences share
//!   one CPU or GPU, so the hedge slows the request it is racing.
//! * The dominant local tail is a cold model load, and the duplicate waits on
//!   exactly the same load.
//!
//! So [`hedging_helps`] answers no for Ollama. This is not caution about an
//! unmeasured risk — it is that the mechanism has no path to working there.

use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use crate::providers::ProviderType;

/// How long the first request gets before a duplicate is sent.
///
/// Chosen to sit above p50 so the common turn never pays for two requests: a
/// hedge that fires on a typical turn is not bounding the tail, it is doubling
/// the bill and the load. 1.8s is also past the point where a spoken reply
/// stops feeling like an answer and starts feeling like a hang.
pub const HEDGE_AFTER: Duration = Duration::from_millis(1_800);

/// Whether a duplicate request can reach capacity the first one is not already
/// using. See the module docs — for a local model the answer is no.
pub fn hedging_helps(provider: &ProviderType) -> bool {
    match provider {
        ProviderType::Ollama => false,
        ProviderType::CloudOpenAI | ProviderType::CloudGemini | ProviderType::CloudAnthropic => {
            true
        }
    }
}

/// Decides which of two in-flight attempts owns the output.
///
/// The whole correctness problem of hedging in a streaming, speaking pipeline:
/// both requests may start producing tokens, and exactly one of them may be
/// spoken. Whoever produces the first token claims the turn, and the loser is
/// told to stop rather than being allowed to interleave a second voice into the
/// same sentence buffer.
#[derive(Debug, Default)]
pub struct Winner(AtomicU8);

/// Attempt ids. Zero means nobody has claimed the turn yet.
pub const FIRST: u8 = 1;
pub const HEDGE: u8 = 2;

impl Winner {
    pub fn new() -> Self {
        Self(AtomicU8::new(0))
    }

    /// Whether `attempt` may emit. The first caller wins and every later call
    /// from that same attempt keeps winning; the other attempt is refused for
    /// the rest of the turn.
    pub fn claim(&self, attempt: u8) -> bool {
        match self
            .0
            .compare_exchange(0, attempt, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => true,
            Err(existing) => existing == attempt,
        }
    }

    /// Which attempt spoke, or `None` if neither produced a token.
    pub fn settled(&self) -> Option<u8> {
        match self.0.load(Ordering::Acquire) {
            0 => None,
            id => Some(id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_attempt_may_ever_speak() {
        let winner = Winner::new();
        assert!(winner.claim(FIRST), "the first token claims the turn");
        assert!(winner.claim(FIRST), "and keeps it for every later token");
        assert!(!winner.claim(HEDGE), "the loser is refused");
        assert!(!winner.claim(HEDGE), "and stays refused");
        assert_eq!(winner.settled(), Some(FIRST));
    }

    #[test]
    fn the_hedge_may_win() {
        // The case the mechanism exists for: the first request stalled and the
        // duplicate answered.
        let winner = Winner::new();
        assert!(winner.claim(HEDGE));
        assert!(!winner.claim(FIRST));
        assert_eq!(winner.settled(), Some(HEDGE));
    }

    #[test]
    fn nobody_speaks_until_a_token_arrives() {
        assert_eq!(Winner::new().settled(), None);
    }

    #[test]
    fn a_local_model_is_never_hedged() {
        // Not a policy preference — a duplicate cannot reach capacity the first
        // request is not already occupying. See the module docs.
        assert!(!hedging_helps(&ProviderType::Ollama));
        assert!(hedging_helps(&ProviderType::CloudOpenAI));
        assert!(hedging_helps(&ProviderType::CloudGemini));
        assert!(hedging_helps(&ProviderType::CloudAnthropic));
    }

    #[test]
    fn the_hedge_waits_past_a_typical_turn() {
        // A hedge that fires on the common turn is not bounding the tail, it is
        // doubling the bill.
        assert!(HEDGE_AFTER >= Duration::from_millis(1_500));
    }
}
