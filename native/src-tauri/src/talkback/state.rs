//! The Talkback state machine.
//!
//! Backend-owned, and the frontend renders it rather than inventing it —
//! the same rule the capture pill follows (`docs/decisions.md`
//! Decision 18). Keeping it here, as a total function over an event enum,
//! means an illegal transition is a failing test instead of a UI that
//! shows "listening" while the microphone is closed.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Every state a Talkback session can be in.
///
/// `INTERRUPTED` is a real state rather than a flag on `SPEAKING`: the
/// moment a barge-in is detected, playback must already have been told to
/// stop, and the UI must be able to show that it heard you before the new
/// turn's audio has been captured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TalkbackState {
    #[default]
    Off,
    Starting,
    Listening,
    UserSpeaking,
    Transcribing,
    Thinking,
    Speaking,
    Interrupted,
    Error,
}

impl TalkbackState {
    /// True while Talkback holds the microphone. Used by `commands.rs` to
    /// refuse a dictation capture rather than let two streams fight over
    /// the device.
    pub fn holds_microphone(self) -> bool {
        !matches!(self, TalkbackState::Off | TalkbackState::Error)
    }
}

/// Things that happen to a Talkback session. The state machine is driven
/// only by these — nothing sets a state directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TalkbackEvent {
    /// User switched Talkback on.
    Enable,
    /// The microphone (or, for text-only turns, the engine) came up.
    Ready,
    /// Turn detection saw speech begin.
    SpeechStarted,
    /// Turn detection saw the turn end.
    SpeechEnded,
    /// A turn's text is available — from STT, or typed.
    TranscriptReady,
    /// The first phrase of the answer is ready to speak.
    ResponseStarted,
    /// Playback of the whole answer finished.
    ResponseComplete,
    /// The user started speaking while the agent was speaking.
    Interrupt,
    /// A component failed in a way the turn cannot recover from.
    Fail,
    /// User switched Talkback off.
    Disable,
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("Talkback cannot go from {from:?} to {event:?}")]
pub struct InvalidTransition {
    pub from: TalkbackState,
    pub event: TalkbackEvent,
}

impl TalkbackState {
    /// Applies `event`, returning the next state or an error naming the
    /// illegal move.
    ///
    /// `Disable` and `Fail` are accepted from anywhere: switching off and
    /// falling over must always be possible, or a wedged session has no
    /// way out.
    pub fn apply(self, event: TalkbackEvent) -> Result<TalkbackState, InvalidTransition> {
        use TalkbackEvent as E;
        use TalkbackState as S;

        match (self, event) {
            (_, E::Disable) => Ok(S::Off),
            (_, E::Fail) => Ok(S::Error),

            (S::Off, E::Enable) => Ok(S::Starting),
            (S::Error, E::Enable) => Ok(S::Starting),
            (S::Starting, E::Ready) => Ok(S::Listening),

            (S::Listening, E::SpeechStarted) => Ok(S::UserSpeaking),
            (S::UserSpeaking, E::SpeechEnded) => Ok(S::Transcribing),
            // A typed turn skips capture entirely and still has to reach
            // THINKING, which is why TranscriptReady is legal from
            // LISTENING as well as from TRANSCRIBING.
            (S::Listening, E::TranscriptReady) => Ok(S::Thinking),
            (S::Transcribing, E::TranscriptReady) => Ok(S::Thinking),
            (S::Interrupted, E::TranscriptReady) => Ok(S::Thinking),

            (S::Thinking, E::ResponseStarted) => Ok(S::Speaking),
            // A turn with no spoken output (TTS unconfigured, or an
            // action that only needs a confirmation line) goes straight
            // back to listening.
            (S::Thinking, E::ResponseComplete) => Ok(S::Listening),
            (S::Speaking, E::ResponseComplete) => Ok(S::Listening),

            (S::Speaking, E::Interrupt) => Ok(S::Interrupted),
            (S::Interrupted, E::SpeechStarted) => Ok(S::UserSpeaking),
            (S::Interrupted, E::SpeechEnded) => Ok(S::Transcribing),
            // Nothing usable followed the barge-in — fall back to
            // listening rather than stranding the session.
            (S::Interrupted, E::ResponseComplete) => Ok(S::Listening),

            // Idempotent re-assertions of the state we are already in are
            // not errors: two capture callbacks can both report speech.
            (S::UserSpeaking, E::SpeechStarted) => Ok(S::UserSpeaking),
            (S::Listening, E::ResponseComplete) => Ok(S::Listening),
            (S::Starting, E::Enable) => Ok(S::Starting),

            (from, event) => Err(InvalidTransition { from, event }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TalkbackEvent as E;
    use super::TalkbackState as S;
    use super::*;

    /// Drives a sequence of events, asserting each lands where expected.
    fn drive(start: S, steps: &[(E, S)]) {
        let mut state = start;
        for (event, expected) in steps {
            state = state
                .apply(*event)
                .unwrap_or_else(|e| panic!("unexpected rejection: {e}"));
            assert_eq!(state, *expected, "after {:?}", event);
        }
    }

    #[test]
    fn the_happy_voice_loop() {
        drive(
            S::Off,
            &[
                (E::Enable, S::Starting),
                (E::Ready, S::Listening),
                (E::SpeechStarted, S::UserSpeaking),
                (E::SpeechEnded, S::Transcribing),
                (E::TranscriptReady, S::Thinking),
                (E::ResponseStarted, S::Speaking),
                (E::ResponseComplete, S::Listening),
            ],
        );
    }

    #[test]
    fn barge_in_reaches_a_new_turn() {
        drive(
            S::Speaking,
            &[
                (E::Interrupt, S::Interrupted),
                (E::SpeechStarted, S::UserSpeaking),
                (E::SpeechEnded, S::Transcribing),
                (E::TranscriptReady, S::Thinking),
            ],
        );
    }

    #[test]
    fn a_typed_turn_skips_capture() {
        drive(
            S::Listening,
            &[
                (E::TranscriptReady, S::Thinking),
                (E::ResponseStarted, S::Speaking),
                (E::ResponseComplete, S::Listening),
            ],
        );
    }

    #[test]
    fn a_silent_answer_returns_to_listening_without_speaking() {
        assert_eq!(S::Thinking.apply(E::ResponseComplete), Ok(S::Listening));
    }

    #[test]
    fn disable_wins_from_every_state() {
        for state in [
            S::Off,
            S::Starting,
            S::Listening,
            S::UserSpeaking,
            S::Transcribing,
            S::Thinking,
            S::Speaking,
            S::Interrupted,
            S::Error,
        ] {
            assert_eq!(state.apply(E::Disable), Ok(S::Off), "from {:?}", state);
        }
    }

    #[test]
    fn failure_wins_from_every_state() {
        for state in [S::Listening, S::Thinking, S::Speaking, S::Interrupted] {
            assert_eq!(state.apply(E::Fail), Ok(S::Error), "from {:?}", state);
        }
    }

    #[test]
    fn error_can_be_re_enabled() {
        assert_eq!(S::Error.apply(E::Enable), Ok(S::Starting));
    }

    #[test]
    fn illegal_transitions_are_rejected_not_silently_allowed() {
        assert_eq!(
            S::Off.apply(E::SpeechStarted),
            Err(InvalidTransition {
                from: S::Off,
                event: E::SpeechStarted
            })
        );
        assert!(S::Listening.apply(E::ResponseStarted).is_err());
        assert!(S::Speaking.apply(E::SpeechEnded).is_err());
        assert!(S::Off.apply(E::Ready).is_err());
    }

    #[test]
    fn repeated_speech_start_is_idempotent() {
        assert_eq!(S::UserSpeaking.apply(E::SpeechStarted), Ok(S::UserSpeaking));
    }

    #[test]
    fn microphone_ownership_matches_the_visible_state() {
        assert!(!S::Off.holds_microphone());
        assert!(!S::Error.holds_microphone());
        for state in [
            S::Starting,
            S::Listening,
            S::UserSpeaking,
            S::Transcribing,
            S::Thinking,
            S::Speaking,
            S::Interrupted,
        ] {
            assert!(state.holds_microphone(), "{:?} should hold the mic", state);
        }
    }

    #[test]
    fn default_is_off() {
        assert_eq!(TalkbackState::default(), S::Off);
    }
}
