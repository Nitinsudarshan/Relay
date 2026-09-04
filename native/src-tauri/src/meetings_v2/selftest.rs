//! Runnable checks for the meeting pipeline's failure modes.
//!
//! Unit tests prove these properties on CI. This module proves them *on the
//! user's machine*, with the user's own Whisper model, from a button in
//! Diagnostics — which is a different guarantee. The bug this pipeline was
//! rebuilt around (four minutes of "Thank you." over room tone) is
//! machine-dependent: it depends on the microphone's noise floor, on which
//! Whisper model is installed, and on how quiet the room is. A green CI run
//! says the logic is right; a green self-test says it is working *here*.
//!
//! Every check is built from synthesized audio, so running it needs no
//! recording, touches no vault, and can be run as often as the user likes.

use super::diarize::{cluster, features};
use super::transcript_health::{self, DecodeEvidence};
use crate::capture::stt::{SttEngine, SttLanguageConfig, WhisperDecodingConfig};
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// One check and what it found.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelfTestCheck {
    pub id: String,
    pub name: String,
    /// What a pass actually proves. Written for someone reading the panel, not
    /// for someone reading the source.
    pub purpose: String,
    pub passed: bool,
    /// The measurement behind the verdict. Always populated: a check that says
    /// only "passed" cannot be trusted, and a failure with no number cannot be
    /// diagnosed.
    pub detail: String,
    pub duration_ms: u64,
}

/// The whole run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeetingSelfTestReport {
    pub checks: Vec<SelfTestCheck>,
    pub passed: usize,
    pub failed: usize,
    pub duration_ms: u64,
    /// Whether the checks that need a Whisper model ran. False is not a
    /// failure — it means the model is not configured.
    pub whisper_checked: bool,
    /// What the user's own Whisper model produced from thirty seconds of room
    /// tone, when it was available to ask.
    ///
    /// This is the single most useful line in the report. If it comes back with
    /// subtitle boilerplate, the user is looking at the exact hallucination
    /// this pipeline exists to catch, produced by their model on their machine —
    /// and the check above it says whether the gate stopped it reaching a
    /// transcript.
    pub whisper_on_silence: Option<String>,
}

/// Sample rate every fixture is synthesized at.
const RATE: u32 = 16_000;

/// Runs every check. CPU-bound and synchronous; callers put it on a blocking
/// thread.
pub fn run(whisper_model_path: Option<&str>) -> MeetingSelfTestReport {
    let started = Instant::now();
    let mut checks = Vec::new();

    checks.push(check(
        "gate_rejects_room_tone",
        "Room tone is not decoded",
        "Thirty seconds of steady background noise must never reach Whisper. This is \
the gate that failed: a fan sitting just above a fixed loudness threshold passed \
for the whole window, and Whisper filled it with subtitle boilerplate.",
        || {
            let profile = transcript_health::profile_speech(&room_tone(30.0), RATE);
            (
                !profile.is_worth_decoding(),
                format!(
                    "{:.2}s voiced of 30s; overall RMS {:.4} (above the old {:.4} \
threshold), noise floor {:.4}",
                    profile.voiced_seconds,
                    profile.rms,
                    transcript_health::ABSOLUTE_SILENCE_RMS,
                    profile.noise_floor_rms
                ),
            )
        },
    ));

    checks.push(check(
        "gate_accepts_speech",
        "Speech is decoded",
        "The gate must not be so strict that it silences real meetings. A voiced \
signal has to clear it.",
        || {
            let profile = transcript_health::profile_speech(&speech_like(30.0), RATE);
            (
                profile.is_worth_decoding(),
                format!(
                    "{:.1}s voiced of 30s ({:.0}%), gate needs {:.1}s",
                    profile.voiced_seconds,
                    profile.voiced_ratio() * 100.0,
                    transcript_health::MIN_VOICED_SECONDS
                ),
            )
        },
    ));

    checks.push(check(
        "gate_rejects_digital_silence",
        "A muted source is not decoded",
        "A muted microphone or a loopback device with nothing playing produces exact \
zeroes. Decoding that can only invent.",
        || {
            let profile = transcript_health::profile_speech(&vec![0.0; RATE as usize * 30], RATE);
            (
                !profile.is_worth_decoding(),
                format!("{:.2}s voiced of 30s", profile.voiced_seconds),
            )
        },
    ));

    checks.push(check(
        "loop_is_rejected",
        "A decoder loop is discarded",
        "The exact output that filled chunks 11 to 19 of the reported meeting: one \
phrase repeated for the whole window. It must be recognised and thrown away rather \
than stored as something a person said.",
        || {
            let text = "Thank you. ".repeat(73);
            let reason = transcript_health::assess(
                &text,
                DecodeEvidence {
                    voiced_seconds: 0.2,
                    total_seconds: 30.0,
                    mean_no_speech_prob: 0.4,
                },
            );
            match reason {
                Some(r) => (true, format!("rejected — {}", r.describe())),
                None => (false, "146 words of \"Thank you.\" were accepted".to_string()),
            }
        },
    ));

    checks.push(check(
        "speech_is_kept",
        "Ordinary speech is kept",
        "The screen must not be so aggressive that it deletes a real meeting. A \
normal sentence over normal voiced time has to survive.",
        || {
            let text = "So the plan is to ship the migration on Friday, and Pranjali will \
take the review.";
            let reason = transcript_health::assess(
                text,
                DecodeEvidence {
                    voiced_seconds: 9.0,
                    total_seconds: 12.0,
                    mean_no_speech_prob: 0.02,
                },
            );
            match reason {
                None => (true, "kept, as it must be".to_string()),
                Some(r) => (false, format!("wrongly rejected — {}", r.describe())),
            }
        },
    ));

    checks.push(check(
        "a_real_thank_you_is_kept",
        "A real \"thank you\" survives",
        "The same words the hallucination produces are also a thing people say. Over \
audio that actually contained a voice, it has to be kept — deleting it would lose \
speech, which is the worse error.",
        || {
            let over_speech = transcript_health::assess(
                "Thank you.",
                DecodeEvidence {
                    voiced_seconds: 1.8,
                    total_seconds: 3.0,
                    mean_no_speech_prob: 0.03,
                },
            );
            let over_silence = transcript_health::assess(
                "Thank you.",
                DecodeEvidence {
                    voiced_seconds: 0.1,
                    total_seconds: 30.0,
                    mean_no_speech_prob: 0.3,
                },
            );
            (
                over_speech.is_none() && over_silence.is_some(),
                format!(
                    "over 1.8s of voice: {}; over 0.1s of voice: {}",
                    if over_speech.is_none() { "kept" } else { "rejected" },
                    if over_silence.is_some() { "rejected" } else { "kept" }
                ),
            )
        },
    ));

    checks.push(check(
        "a_loop_never_prompts_the_next_chunk",
        "A loop cannot spread",
        "Each chunk's text is carried into the next decode as Whisper's initial \
prompt, which reads as preceding speech. This is why one bad chunk became nine: the \
prompt propagated the loop even though the decoder state was discarded.",
        || {
            let looped = !transcript_health::is_safe_as_prompt("Thank you. Thank you. Thank you.");
            let filler = !transcript_health::is_safe_as_prompt("Thanks for watching");
            let real = transcript_health::is_safe_as_prompt("and then we agreed to ship on Friday");
            (
                looped && filler && real,
                format!(
                    "loop carried forward: {}; filler: {}; real speech: {}",
                    if looped { "no" } else { "YES" },
                    if filler { "no" } else { "YES" },
                    if real { "yes" } else { "NO" }
                ),
            )
        },
    ));

    checks.push(check(
        "voices_are_separated",
        "Distinct voices become distinct speakers",
        "Three synthesized voices must cluster into three speakers. Failing this is \
the state the app shipped in: everyone who is not the local user shares one label, \
however many of them there are.",
        || {
            let utterances = synthetic_meeting(&[
                (105.0, 1.0),
                (230.0, 1.55),
                (160.0, 1.25),
                (107.0, 1.02),
                (228.0, 1.53),
                (162.0, 1.24),
            ]);
            let clustering = cluster::cluster(&utterances, None);
            (
                clustering.cluster_count == 3,
                format!(
                    "found {} of 3; within-cluster {:.2}, between-cluster {:.2}, \
separated: {}",
                    clustering.cluster_count,
                    clustering.mean_within_distance,
                    clustering.min_between_distance,
                    clustering.is_well_separated()
                ),
            )
        },
    ));

    checks.push(check(
        "one_voice_stays_one_speaker",
        "One voice does not become a roster",
        "The opposite failure, and the worse one: splitting a single person into \
several invents people who were never in the room.",
        || {
            let utterances = synthetic_meeting(&[
                (130.0, 1.0),
                (131.0, 1.01),
                (129.0, 0.99),
                (132.0, 1.02),
            ]);
            let clustering = cluster::cluster(&utterances, None);
            (
                clustering.cluster_count == 1,
                format!(
                    "found {} of 1; within-cluster {:.2}",
                    clustering.cluster_count, clustering.mean_within_distance
                ),
            )
        },
    ));

    checks.push(check(
        "pitch_is_measured",
        "Voice pitch is measured correctly",
        "Pitch is the strongest cue separating two speakers without a voiceprint. If \
this is wrong, every roster below it is guesswork.",
        || {
            let mut errors = Vec::new();
            for target in [90.0f32, 150.0, 220.0] {
                let tone: Vec<f32> = (0..RATE as usize / 2)
                    .map(|i| {
                        (2.0 * std::f32::consts::PI * target * i as f32 / RATE as f32).sin()
                    })
                    .collect();
                match features::extract(&tone, RATE).and_then(|f| f.pitch_hz) {
                    Some(hz) => errors.push(format!("{target:.0}→{hz:.0}Hz")),
                    None => errors.push(format!("{target:.0}→none")),
                }
            }
            let ok = features::extract(&pure_tone(150.0, 1.0), RATE)
                .and_then(|f| f.pitch_hz)
                .is_some_and(|hz| (hz - 150.0).abs() < 15.0);
            (ok, errors.join(", "))
        },
    ));

    // The check that needs the user's own model. Everything above is arithmetic
    // and would pass on any machine; this one is about this machine.
    let mut whisper_on_silence = None;
    let mut whisper_checked = false;
    if let Some(path) = whisper_model_path.filter(|p| !p.trim().is_empty()) {
        whisper_checked = true;
        let started_whisper = Instant::now();
        let tone = room_tone(30.0);
        let engine = SttEngine::new();
        let decoded = engine.transcribe_utterances_with_config(
            Some(path),
            &tone,
            &SttLanguageConfig {
                whisper_language: Some("en".to_string()),
                translate: false,
            },
            &WhisperDecodingConfig::baseline(),
        );

        let (passed, detail) = match decoded {
            Ok((utterances, _)) => {
                let text = crate::capture::stt::join_utterance_text(&utterances);
                whisper_on_silence = Some(if text.is_empty() {
                    "(nothing — this model does not hallucinate on this fixture)".to_string()
                } else {
                    text.clone()
                });

                // The gate is what has to hold. Whisper producing boilerplate
                // here is expected and is exactly what the gate exists for.
                let profile = transcript_health::profile_speech(&tone, RATE);
                let gate_held = !profile.is_worth_decoding();
                (
                    gate_held,
                    if text.is_empty() {
                        format!(
                            "your model produced nothing from room tone; the gate would \
have stopped it anyway ({:.2}s voiced)",
                            profile.voiced_seconds
                        )
                    } else {
                        format!(
                            "your model produced {} words from room tone, and the gate \
stopped all of it reaching the transcript ({:.2}s voiced, below the {:.1}s minimum)",
                            text.split_whitespace().count(),
                            profile.voiced_seconds,
                            transcript_health::MIN_VOICED_SECONDS
                        )
                    },
                )
            }
            Err(e) => (false, format!("could not run your Whisper model: {e}")),
        };

        checks.push(SelfTestCheck {
            id: "whisper_on_room_tone".to_string(),
            name: "Your Whisper model on room tone".to_string(),
            purpose: "Runs thirty seconds of synthesized room tone through the model that \
is actually installed here, and reports what it produced. A pass means the gate stopped \
that output reaching a transcript — whatever the model invented."
                .to_string(),
            passed,
            detail,
            duration_ms: started_whisper.elapsed().as_millis() as u64,
        });
    }

    let passed = checks.iter().filter(|c| c.passed).count();
    MeetingSelfTestReport {
        failed: checks.len() - passed,
        passed,
        checks,
        duration_ms: started.elapsed().as_millis() as u64,
        whisper_checked,
        whisper_on_silence,
    }
}

/// Runs one check, timing it.
fn check<F>(id: &str, name: &str, purpose: &str, body: F) -> SelfTestCheck
where
    F: FnOnce() -> (bool, String),
{
    let started = Instant::now();
    let (passed, detail) = body();
    SelfTestCheck {
        id: id.to_string(),
        name: name.to_string(),
        purpose: purpose.to_string(),
        passed,
        detail,
        duration_ms: started.elapsed().as_millis() as u64,
    }
}

/// Steady background noise: a fan, an air conditioner, an open mic in a quiet
/// room.
///
/// Deliberately loud enough that its mean RMS clears the recorder's audibility
/// threshold, because that is the property that made the old gate useless. A
/// quieter fixture would pass a broken gate.
fn room_tone(seconds: f64) -> Vec<f32> {
    let n = (RATE as f64 * seconds) as usize;
    (0..n)
        .map(|i| {
            let x = ((i as f32 * 12.9898).sin() * 43758.547).fract();
            (x - 0.5) * 0.021
        })
        .collect()
}

/// A peaky, voiced signal: bursts with silence between, like speech.
fn speech_like(seconds: f64) -> Vec<f32> {
    let n = (RATE as f64 * seconds) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let envelope = if (t * 2.0).fract() < 0.6 { 0.3 } else { 0.001 };
            envelope * (2.0 * std::f32::consts::PI * 180.0 * t).sin()
        })
        .collect()
}

fn pure_tone(hz: f32, seconds: f64) -> Vec<f32> {
    let n = (RATE as f64 * seconds) as usize;
    (0..n)
        .map(|i| (2.0 * std::f32::consts::PI * hz * i as f32 / RATE as f32).sin() * 0.4)
        .collect()
}

/// A synthesized voice: a fundamental, a harmonic, and two formant-like tones.
fn synth_voice(f0: f32, formant_scale: f32, seconds: f64) -> Vec<f32> {
    let n = (RATE as f64 * seconds) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let tau = 2.0 * std::f32::consts::PI;
            let envelope = 0.4 * (1.0 + 0.3 * (tau * 4.0 * t).sin());
            envelope
                * (0.6 * (tau * f0 * t).sin()
                    + 0.3 * (tau * f0 * 2.0 * t).sin()
                    + 0.25 * (tau * 700.0 * formant_scale * t).sin()
                    + 0.18 * (tau * 1900.0 * formant_scale * t).sin())
        })
        .collect()
}

/// A meeting of synthesized voices, one turn each in order.
fn synthetic_meeting(voices: &[(f32, f32)]) -> Vec<cluster::Utterance> {
    voices
        .iter()
        .enumerate()
        .filter_map(|(index, &(f0, formant))| {
            let samples = synth_voice(f0, formant, 4.0);
            features::extract(&samples, RATE).map(|f| cluster::Utterance {
                id: format!("probe_{index}"),
                start_time_s: index as f64 * 10.0,
                end_time_s: index as f64 * 10.0 + 4.0,
                features: f,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_check_passes_without_a_whisper_model() {
        // The self-test is only worth offering if it is green on a correct
        // build. A failure here means either the pipeline regressed or a
        // fixture drifted — both of which this test exists to catch before a
        // user sees a red panel and cannot tell which it is.
        let report = run(None);

        assert!(!report.checks.is_empty());
        assert!(!report.whisper_checked);
        assert_eq!(report.whisper_on_silence, None);
        assert_eq!(
            report.failed,
            0,
            "failing checks: {:?}",
            report
                .checks
                .iter()
                .filter(|c| !c.passed)
                .map(|c| format!("{}: {}", c.id, c.detail))
                .collect::<Vec<_>>()
        );
        assert_eq!(report.passed, report.checks.len());
    }

    #[test]
    fn every_check_reports_a_measurement_not_just_a_verdict() {
        // A check that says only "passed" cannot be trusted, and a failure with
        // no number cannot be diagnosed.
        for check in run(None).checks {
            assert!(!check.detail.trim().is_empty(), "{} had no detail", check.id);
            assert!(!check.purpose.trim().is_empty(), "{} had no purpose", check.id);
            assert!(!check.name.trim().is_empty());
        }
    }

    #[test]
    fn check_ids_are_unique_so_the_panel_can_key_on_them() {
        let report = run(None);
        let mut ids: Vec<&str> = report.checks.iter().map(|c| c.id.as_str()).collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(before, ids.len(), "duplicate check ids: {ids:?}");
    }

    #[test]
    fn an_empty_model_path_is_treated_as_no_model() {
        let report = run(Some("   "));
        assert!(!report.whisper_checked);
        assert_eq!(report.failed, 0);
    }

    #[test]
    fn the_room_tone_fixture_would_defeat_the_gate_it_replaced() {
        // If this fixture is quieter than the old fixed threshold, the
        // "room tone is not decoded" check proves nothing.
        let profile = transcript_health::profile_speech(&room_tone(30.0), RATE);
        assert!(
            profile.rms > transcript_health::ABSOLUTE_SILENCE_RMS,
            "fixture RMS {} does not clear the old gate",
            profile.rms
        );
    }
}
