//! Synthetic voices that behave like real ones.
//!
//! This module exists because the fixtures it replaces were the reason
//! diarization shipped broken. Those voices were an octave apart with formants
//! scaled by 55%, which put two "different speakers" about 5.7 apart in feature
//! space — so a split threshold of 2.0 looked comfortably calibrated and was in
//! fact above every distance real speech produces. Measured against the
//! fixtures here, two genuinely different people sit at 0.55–1.34 and the same
//! person at 0.02–0.10. The old threshold could not split anything real.
//!
//! What makes a fixture realistic, and why each part matters:
//!
//! * **Conversational pitch spread, not an octave.** Adults in one meeting run
//!   roughly 100–200 Hz. Voices an octave apart are a different problem.
//! * **Overlapping formants.** Vowel resonances differ between speakers by a
//!   few hundred Hz, not by half the spectrum.
//! * **One shared recording chain.** The same room noise floor is added to
//!   every voice, because in a real meeting it is the same microphone. This is
//!   the common component that makes every voice look alike, and a clusterer
//!   that only works without it does not work.
//! * **Pitch drift and a syllabic envelope.** A steady tone is not speech, and
//!   feature variance across a turn is part of what identifies a speaker.
//!
//! These are still not human speech, and passing here is not proof of working
//! on a real recording. They are a floor: an engine that cannot separate these
//! cannot possibly separate people.

use super::cluster;
use super::features;

pub const RATE: u32 = 16_000;

/// A speaker's voice, as a set of resonances rather than a scale factor.
#[derive(Debug, Clone, Copy)]
pub struct VoiceProfile {
    pub name: &'static str,
    /// Fundamental frequency, in Hz.
    pub f0: f32,
    /// First three formants — the resonances that make a vowel and, between
    /// speakers, most of what distinguishes one voice from another.
    pub f1: f32,
    pub f2: f32,
    pub f3: f32,
}

/// Three adults in one meeting: a low, a mid and a high voice, all within the
/// range a single conversation actually spans.
pub const THREE_SPEAKERS: [VoiceProfile; 3] = [
    VoiceProfile { name: "A", f0: 118.0, f1: 620.0, f2: 1180.0, f3: 2500.0 },
    VoiceProfile { name: "B", f0: 142.0, f1: 700.0, f2: 1300.0, f3: 2600.0 },
    VoiceProfile { name: "C", f0: 196.0, f1: 800.0, f2: 1450.0, f3: 2750.0 },
];

/// Two speakers who are genuinely hard to tell apart: close pitch, close
/// formants. The case where an honest engine should either merge them or
/// report the split as uncertain, and must never be confidently wrong.
pub const TWO_SIMILAR_SPEAKERS: [VoiceProfile; 2] = [
    VoiceProfile { name: "D", f0: 132.0, f1: 660.0, f2: 1240.0, f3: 2540.0 },
    VoiceProfile { name: "E", f0: 138.0, f1: 685.0, f2: 1275.0, f3: 2570.0 },
];

/// `count` voices spread widely enough to be genuinely tellable apart.
///
/// Spacing matters more than the count. Sixteen voices crammed into the adult
/// range sit as close to each other as [`TWO_SIMILAR_SPEAKERS`] do, and merging
/// them is the *correct* answer — so a fixture like that tests nothing about
/// capacity and everything about the similarity limit. These are spread across
/// the full range with the formants varied on their own axis rather than
/// tracking pitch, which is what keeps neighbours distinguishable.
///
/// Practically limited to a handful: telling many voices apart from cepstral
/// statistics alone is beyond what these features do, and a fixture claiming
/// otherwise would be testing a capability Relay does not have.
pub fn distinct_voices(count: usize) -> Vec<VoiceProfile> {
    const NAMES: &[&str] = &["A", "B", "C", "D", "E", "F", "G", "H"];
    let count = count.min(NAMES.len());
    (0..count)
        .map(|i| {
            let step = i as f32 / (count.max(2) - 1) as f32;
            // Formants swing on a different phase from pitch, so a low-pitched
            // voice is not automatically a low-formant one.
            let swing = (step * std::f32::consts::PI).sin();
            VoiceProfile {
                name: NAMES[i],
                f0: 95.0 + step * 145.0,
                f1: 560.0 + swing * 340.0,
                f2: 1100.0 + step * 620.0 - swing * 180.0,
                f3: 2350.0 + swing * 520.0,
            }
        })
        .collect()
}

/// Renders one turn of a voice.
///
/// `turn` varies the pitch and phase slightly, standing in for the fact that
/// nobody says two sentences identically.
pub fn utterance_audio(voice: &VoiceProfile, turn: usize, seconds: f64) -> Vec<f32> {
    let n = (RATE as f64 * seconds) as usize;
    let seed = turn as f32 * 1.1;
    let f0 = voice.f0 + turn as f32 * 2.0;

    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let tau = 2.0 * std::f32::consts::PI;

            // Pitch drifts through a phrase, as real intonation does.
            let f0t = f0 * (1.0 + 0.06 * (tau * 1.7 * t + seed).sin());

            // A glottal source: harmonics of the fundamental falling off as 1/n.
            let mut source = 0.0f32;
            for harmonic in 1..=12 {
                source += (tau * f0t * harmonic as f32 * t).sin() / harmonic as f32;
            }

            // Formants shape the source into something vowel-like. These, more
            // than the pitch, are what a cepstral feature actually measures.
            let shaped = source * 0.35
                + 0.30 * (tau * voice.f1 * t).sin()
                + 0.20 * (tau * voice.f2 * t).sin()
                + 0.12 * (tau * voice.f3 * t).sin();

            // Syllables, and the room everyone shares.
            let envelope = (0.55 + 0.45 * (tau * 3.4 * t + seed).sin()).max(0.0);
            let room = (((i as f32 * 7.13 + seed).sin() * 4391.0).fract() - 0.5) * 0.012;

            shaped * envelope * 0.22 + room
        })
        .collect()
}

/// One clusterable utterance from a voice.
pub fn utterance(
    voice: &VoiceProfile,
    turn: usize,
    start_time_s: f64,
    seconds: f64,
) -> cluster::Utterance {
    let audio = utterance_audio(voice, turn, seconds);
    let features = features::extract(&audio, RATE)
        .unwrap_or_else(|| panic!("fixture {} turn {turn} produced no features", voice.name));
    cluster::Utterance {
        id: format!("{}{}", voice.name, turn),
        start_time_s,
        end_time_s: start_time_s + seconds,
        features,
        embedding: None,
    }
}

/// A meeting where each voice takes `turns_each` turns, interleaved the way a
/// conversation actually runs rather than grouped by speaker.
pub fn interleaved_meeting(voices: &[VoiceProfile], turns_each: usize) -> Vec<cluster::Utterance> {
    let mut out = Vec::new();
    for turn in 0..turns_each {
        for (index, voice) in voices.iter().enumerate() {
            let position = (turn * voices.len() + index) as f64;
            out.push(utterance(voice, turn, position * 10.0, 3.0));
        }
    }
    out
}

/// The speaker each fixture utterance actually came from, for scoring.
pub fn truth(utterances: &[cluster::Utterance]) -> Vec<String> {
    utterances
        .iter()
        .map(|u| u.id.chars().take(1).collect())
        .collect()
}

/// Whether a clustering recovered the fixture's speakers exactly.
///
/// Compares the *partition*, not the labels: cluster numbering is arbitrary, so
/// what matters is that two utterances share a cluster exactly when they share
/// a speaker.
pub fn partition_matches(assignments: &[cluster::Assignment], truth: &[String]) -> bool {
    for i in 0..assignments.len() {
        for j in i + 1..assignments.len() {
            let same_truth = truth[i] == truth[j];
            let same_cluster = match (assignments[i].cluster, assignments[j].cluster) {
                (Some(a), Some(b)) => a == b,
                _ => return false,
            };
            if same_truth != same_cluster {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The measurement the whole calibration rests on, pinned so a change to
    /// the features cannot silently move it.
    #[test]
    fn different_voices_sit_far_apart_and_one_voice_sits_close() {
        let meeting = interleaved_meeting(&THREE_SPEAKERS, 3);
        let truth = truth(&meeting);

        let mut same_speaker: Vec<f32> = Vec::new();
        let mut different_speaker: Vec<f32> = Vec::new();

        for i in 0..meeting.len() {
            for j in i + 1..meeting.len() {
                let d = cluster::feature_distance(&meeting[i].features, &meeting[j].features);
                if truth[i] == truth[j] {
                    same_speaker.push(d);
                } else {
                    different_speaker.push(d);
                }
            }
        }

        let worst_same = same_speaker.iter().cloned().fold(0.0f32, f32::max);
        let best_different = different_speaker
            .iter()
            .cloned()
            .fold(f32::MAX, f32::min);

        assert!(
            worst_same < best_different,
            "the two populations overlap: worst same-speaker {worst_same:.3}, \
closest different-speaker {best_different:.3}"
        );
        // The margin is what a relative threshold has to work with. If this
        // narrows, the features got worse, whatever the clusterer then does.
        assert!(
            best_different > worst_same * 3.0,
            "margin too thin to cluster on: same ≤ {worst_same:.3}, different ≥ \
{best_different:.3}"
        );
    }

    #[test]
    fn a_fixture_voice_is_loud_enough_to_be_decoded_at_all() {
        // A fixture quieter than the speech gate would test nothing downstream.
        let audio = utterance_audio(&THREE_SPEAKERS[0], 0, 3.0);
        let profile = crate::meetings_v2::transcript_health::profile_speech(&audio, RATE);
        assert!(profile.is_worth_decoding(), "{profile:?}");
    }

    #[test]
    fn similar_voices_are_closer_than_dissimilar_ones() {
        // The hard case has to actually be hard, or a test using it proves
        // nothing about uncertainty reporting.
        let similar = interleaved_meeting(&TWO_SIMILAR_SPEAKERS, 2);
        let distinct = interleaved_meeting(&THREE_SPEAKERS[..2], 2);

        let gap = |m: &[cluster::Utterance]| {
            let mut worst = f32::MAX;
            for i in 0..m.len() {
                for j in i + 1..m.len() {
                    if m[i].id.chars().next() != m[j].id.chars().next() {
                        worst = worst.min(cluster::feature_distance(
                            &m[i].features,
                            &m[j].features,
                        ));
                    }
                }
            }
            worst
        };

        assert!(
            gap(&similar) < gap(&distinct),
            "similar {:.3} was not closer than distinct {:.3}",
            gap(&similar),
            gap(&distinct)
        );
    }

    #[test]
    fn the_truth_helper_describes_the_fixture_it_was_built_from() {
        let meeting = interleaved_meeting(&THREE_SPEAKERS, 2);
        assert_eq!(meeting.len(), 6);
        assert_eq!(truth(&meeting), vec!["A", "B", "C", "A", "B", "C"]);
    }

    #[test]
    fn partition_matching_ignores_cluster_numbering_but_not_grouping() {
        let truth = vec!["A".to_string(), "A".to_string(), "B".to_string()];
        let assign = |a: usize, b: usize, c: usize| {
            vec![
                cluster::Assignment { id: "0".into(), cluster: Some(a), distance: 0.0 },
                cluster::Assignment { id: "1".into(), cluster: Some(b), distance: 0.0 },
                cluster::Assignment { id: "2".into(), cluster: Some(c), distance: 0.0 },
            ]
        };

        // Same grouping, different numbers.
        assert!(partition_matches(&assign(0, 0, 1), &truth));
        assert!(partition_matches(&assign(7, 7, 2), &truth));
        // Wrong grouping.
        assert!(!partition_matches(&assign(0, 1, 1), &truth));
        // An unplaced utterance is not a match.
        assert!(!partition_matches(
            &[
                cluster::Assignment { id: "0".into(), cluster: None, distance: 0.0 },
                cluster::Assignment { id: "1".into(), cluster: Some(0), distance: 0.0 },
                cluster::Assignment { id: "2".into(), cluster: Some(1), distance: 0.0 },
            ],
            &truth
        ));
    }
}
