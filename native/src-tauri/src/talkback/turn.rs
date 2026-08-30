//! Streaming turn detection.
//!
//! Relay already has a VAD (`capture::VadConfig`), and it is the wrong
//! tool here: it runs *after* a recording finishes and trims silence off
//! a completed buffer. Talkback needs the opposite — a decision, frame by
//! frame, about whether the user has started and whether they have
//! stopped, while the audio is still arriving. That is why this exists
//! rather than a call into `capture`.
//!
//! It is also why `capture`'s VAD is left completely untouched: the
//! dictation and meeting paths depend on its current behaviour, and
//! Talkback's timing requirements are not theirs.
//!
//! ## Why energy, and what would replace it
//!
//! This is an energy detector with an adaptive noise floor and a
//! hangover timer — the same shape as the meeting live clock's speech
//! flag, which works. `RESEARCH.md` §B evaluated two better options:
//! Silero VAD (MIT, ONNX, ~1 ms per frame) and Pipecat's Smart Turn v3
//! (open weights, semantic end-of-turn, <60 ms CPU). Neither ships in V1
//! because both need an ONNX runtime in the build, and `ort` has an
//! unresolved Windows packaging risk (System32 shadows its DLL) that has
//! to be proven on the target platform first. [`TurnDetector::push`] is
//! the seam: same signature, better decision.

use serde::{Deserialize, Serialize};

/// What one frame of audio told us about the turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TurnEvent {
    /// Nothing changed.
    None,
    /// Sustained speech began.
    SpeechStart,
    /// Speech ended and the turn should be submitted.
    SpeechEnd,
    /// The turn ran past the cap and is being submitted mid-thought,
    /// rather than growing without bound.
    MaxDurationReached,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TurnDetectorConfig {
    /// Time spent measuring the room before any detection runs. A fixed
    /// threshold cannot tell a noisy room from a talking one; this is the
    /// same reasoning as `capture::NOISE_FLOOR_CALIBRATION_MS`.
    pub calibration_ms: u32,
    /// How far above the measured floor a frame must sit to count as
    /// speech.
    pub speech_margin: f32,
    /// Floor below which nothing is ever speech, whatever calibration
    /// decided. Guards a calibration that ran in total silence.
    pub absolute_floor: f32,
    /// Sustained speech required before a turn starts. Stops a keystroke
    /// or a chair creak opening one.
    pub min_speech_ms: u32,
    /// Silence required to end a turn.
    ///
    /// The single most consequential number in conversational feel. Too
    /// short and the agent interrupts a thinking pause; too long and
    /// every reply feels sluggish. 700 ms sits above a normal
    /// mid-sentence pause and below the point where the delay is the
    /// thing you notice. A semantic turn detector is what removes the
    /// tradeoff — see the module docs.
    pub hangover_ms: u32,
    /// Hard cap on one turn.
    pub max_turn_ms: u32,
    /// Multiplier applied to the speech threshold while the agent is
    /// speaking.
    ///
    /// Relay has no acoustic echo cancellation, so on laptop speakers the
    /// microphone hears the agent. Raising the bar during playback means
    /// a barge-in needs to be clearly louder than the agent's own voice.
    /// This is a mitigation, not a fix; headphones are the fix, and
    /// `KNOWN LIMITATIONS` says so.
    pub echo_guard_multiplier: f32,
}

impl Default for TurnDetectorConfig {
    fn default() -> Self {
        Self {
            calibration_ms: 300,
            speech_margin: 0.035,
            absolute_floor: 0.02,
            min_speech_ms: 200,
            hangover_ms: 700,
            max_turn_ms: 30_000,
            echo_guard_multiplier: 2.5,
        }
    }
}

/// Frame-by-frame turn detection over microphone RMS.
#[derive(Debug)]
pub struct TurnDetector {
    config: TurnDetectorConfig,
    calibration_sum: f64,
    calibration_ms_seen: u32,
    calibrated: bool,
    noise_floor: f32,
    in_speech: bool,
    speech_ms: u32,
    silence_ms: u32,
    turn_ms: u32,
    echo_guard: bool,
}

impl TurnDetector {
    pub fn new(config: TurnDetectorConfig) -> Self {
        Self {
            config,
            calibration_sum: 0.0,
            calibration_ms_seen: 0,
            calibrated: false,
            noise_floor: 0.0,
            in_speech: false,
            speech_ms: 0,
            silence_ms: 0,
            turn_ms: 0,
            echo_guard: false,
        }
    }

    /// Raises the speech threshold while the agent is speaking.
    pub fn set_echo_guard(&mut self, active: bool) {
        self.echo_guard = active;
    }

    /// True once the room has been measured.
    pub fn is_calibrated(&self) -> bool {
        self.calibrated
    }

    pub fn noise_floor(&self) -> f32 {
        self.noise_floor
    }

    /// The level a frame must clear right now to count as speech.
    pub fn effective_threshold(&self) -> f32 {
        let base = (self.noise_floor + self.config.speech_margin).max(self.config.absolute_floor);
        if self.echo_guard {
            base * self.config.echo_guard_multiplier
        } else {
            base
        }
    }

    /// True while a turn is open.
    pub fn in_speech(&self) -> bool {
        self.in_speech
    }

    /// Clears turn state without discarding the calibrated noise floor —
    /// the room has not changed just because a turn ended.
    pub fn reset_turn(&mut self) {
        self.in_speech = false;
        self.speech_ms = 0;
        self.silence_ms = 0;
        self.turn_ms = 0;
    }

    /// Feeds one frame's RMS level and its duration.
    ///
    /// **The seam.** Replacing the body with a Silero or Smart Turn
    /// inference — same inputs, same events — is the whole upgrade path.
    pub fn push(&mut self, rms: f32, frame_ms: u32) -> TurnEvent {
        if !self.calibrated {
            self.calibration_sum += rms as f64 * frame_ms as f64;
            self.calibration_ms_seen += frame_ms;
            if self.calibration_ms_seen < self.config.calibration_ms {
                return TurnEvent::None;
            }
            self.noise_floor =
                (self.calibration_sum / self.calibration_ms_seen.max(1) as f64) as f32;
            self.calibrated = true;
            return TurnEvent::None;
        }

        let is_speech = rms >= self.effective_threshold();

        if self.in_speech {
            self.turn_ms += frame_ms;
            if is_speech {
                self.silence_ms = 0;
            } else {
                self.silence_ms += frame_ms;
                if self.silence_ms >= self.config.hangover_ms {
                    self.reset_turn();
                    return TurnEvent::SpeechEnd;
                }
            }
            if self.turn_ms >= self.config.max_turn_ms {
                self.reset_turn();
                return TurnEvent::MaxDurationReached;
            }
            return TurnEvent::None;
        }

        if is_speech {
            self.speech_ms += frame_ms;
            if self.speech_ms >= self.config.min_speech_ms {
                self.in_speech = true;
                self.silence_ms = 0;
                self.turn_ms = self.speech_ms;
                return TurnEvent::SpeechStart;
            }
        } else {
            // Speech has to be sustained, not cumulative across a minute
            // of keyboard noise.
            self.speech_ms = 0;
        }
        TurnEvent::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME_MS: u32 = 100;

    fn detector() -> TurnDetector {
        TurnDetector::new(TurnDetectorConfig::default())
    }

    /// Feeds `count` frames at `rms`, returning every non-`None` event.
    fn feed(detector: &mut TurnDetector, rms: f32, count: usize) -> Vec<TurnEvent> {
        (0..count)
            .map(|_| detector.push(rms, FRAME_MS))
            .filter(|e| *e != TurnEvent::None)
            .collect()
    }

    /// Runs calibration on a quiet room.
    fn calibrate(detector: &mut TurnDetector, level: f32) {
        assert!(feed(detector, level, 5).is_empty());
        assert!(detector.is_calibrated());
    }

    #[test]
    fn nothing_is_detected_during_calibration() {
        let mut d = detector();
        // Loud enough to be speech, but the room is still being measured.
        assert!(feed(&mut d, 0.5, 2).is_empty());
        assert!(!d.is_calibrated());
    }

    #[test]
    fn calibration_measures_the_room() {
        let mut d = detector();
        calibrate(&mut d, 0.01);
        assert!((d.noise_floor() - 0.01).abs() < 0.001, "{}", d.noise_floor());
    }

    #[test]
    fn a_full_turn_starts_and_ends() {
        let mut d = detector();
        calibrate(&mut d, 0.005);

        assert_eq!(feed(&mut d, 0.3, 2), vec![TurnEvent::SpeechStart]);
        assert!(d.in_speech());
        assert!(feed(&mut d, 0.3, 5).is_empty(), "still speaking");
        assert_eq!(feed(&mut d, 0.001, 7), vec![TurnEvent::SpeechEnd]);
        assert!(!d.in_speech());
    }

    #[test]
    fn a_single_loud_frame_is_not_a_turn() {
        let mut d = detector();
        calibrate(&mut d, 0.005);
        // One 100ms spike, below the 200ms sustained requirement.
        assert!(feed(&mut d, 0.9, 1).is_empty());
        assert!(feed(&mut d, 0.001, 3).is_empty());
        assert!(!d.in_speech());
    }

    #[test]
    fn intermittent_noise_never_accumulates_into_a_turn() {
        let mut d = detector();
        calibrate(&mut d, 0.005);
        for _ in 0..30 {
            assert!(feed(&mut d, 0.9, 1).is_empty(), "a keystroke opened a turn");
            assert!(feed(&mut d, 0.001, 1).is_empty());
        }
        assert!(!d.in_speech());
    }

    #[test]
    fn a_mid_sentence_pause_does_not_end_the_turn() {
        let mut d = detector();
        calibrate(&mut d, 0.005);
        feed(&mut d, 0.3, 2);
        assert!(d.in_speech());

        // 500ms of thinking, below the 700ms hangover.
        assert!(feed(&mut d, 0.001, 5).is_empty());
        assert!(d.in_speech(), "a thinking pause must not submit the turn");

        assert!(feed(&mut d, 0.3, 3).is_empty());
        assert_eq!(feed(&mut d, 0.001, 7), vec![TurnEvent::SpeechEnd]);
    }

    #[test]
    fn a_noisy_room_does_not_hold_the_turn_open_forever() {
        let mut d = TurnDetector::new(TurnDetectorConfig::default());
        // Calibrate in a room that is continuously noisy.
        calibrate(&mut d, 0.08);
        // The same continuous noise must now read as silence, because the
        // threshold moved with it.
        assert!(feed(&mut d, 0.08, 3).is_empty(), "ambient noise opened a turn");
    }

    #[test]
    fn a_runaway_turn_is_capped() {
        let mut d = TurnDetector::new(TurnDetectorConfig {
            max_turn_ms: 1_000,
            ..Default::default()
        });
        calibrate(&mut d, 0.005);
        // 200ms to open the turn, then 800ms more to reach the cap.
        let events = feed(&mut d, 0.3, 10);
        assert_eq!(events, vec![TurnEvent::SpeechStart, TurnEvent::MaxDurationReached]);
        assert!(!d.in_speech());
    }

    #[test]
    fn someone_who_never_stops_gets_successive_capped_turns() {
        // Not a bug: a monologue has to reach the model in pieces rather
        // than growing one buffer until whisper chokes on it.
        let mut d = TurnDetector::new(TurnDetectorConfig {
            max_turn_ms: 1_000,
            ..Default::default()
        });
        calibrate(&mut d, 0.005);
        let events = feed(&mut d, 0.3, 30);
        assert_eq!(
            events
                .iter()
                .filter(|e| **e == TurnEvent::MaxDurationReached)
                .count(),
            3
        );
    }

    #[test]
    fn the_echo_guard_raises_the_bar_during_playback() {
        let mut d = detector();
        calibrate(&mut d, 0.005);
        let quiet_threshold = d.effective_threshold();

        d.set_echo_guard(true);
        assert!(d.effective_threshold() > quiet_threshold);

        // The agent's own voice, at a level that would otherwise be a turn.
        assert!(
            feed(&mut d, quiet_threshold * 1.5, 5).is_empty(),
            "the agent interrupted itself"
        );

        // A genuinely loud interruption still gets through.
        assert_eq!(feed(&mut d, 0.9, 2), vec![TurnEvent::SpeechStart]);
    }

    #[test]
    fn clearing_the_echo_guard_restores_the_normal_threshold() {
        let mut d = detector();
        calibrate(&mut d, 0.005);
        let normal = d.effective_threshold();
        d.set_echo_guard(true);
        d.set_echo_guard(false);
        assert!((d.effective_threshold() - normal).abs() < f32::EPSILON);
    }

    #[test]
    fn resetting_a_turn_keeps_the_measured_room() {
        let mut d = detector();
        calibrate(&mut d, 0.02);
        let floor = d.noise_floor();
        feed(&mut d, 0.3, 3);
        d.reset_turn();
        assert!(!d.in_speech());
        assert!(d.is_calibrated());
        assert_eq!(d.noise_floor(), floor);
    }

    #[test]
    fn the_absolute_floor_survives_a_silent_calibration() {
        let mut d = detector();
        calibrate(&mut d, 0.0);
        assert!(
            d.effective_threshold() >= TurnDetectorConfig::default().absolute_floor,
            "a room calibrated at zero must not make everything speech"
        );
    }

    #[test]
    fn back_to_back_turns_both_fire() {
        let mut d = detector();
        calibrate(&mut d, 0.005);
        for _ in 0..3 {
            assert_eq!(feed(&mut d, 0.3, 2), vec![TurnEvent::SpeechStart]);
            assert_eq!(feed(&mut d, 0.001, 7), vec![TurnEvent::SpeechEnd]);
        }
    }
}
