//! Assigns a speaker to each utterance while the meeting is still running.
//!
//! Diarization used to run only after a recording finished. That left every
//! 30-second chunk with no speaker information on it, so the readable
//! conversation could not exist until the end, and a summary of a long meeting
//! had to be built from text that never said who was talking.
//!
//! The obvious reading of "do it live" — cluster each chunk on its own — is
//! worse than doing nothing, because a speaker found in chunk 1 and the same
//! speaker found in chunk 40 would be different people as far as the data is
//! concerned. Identity has to be *global* even when the decision is made
//! locally. So this keeps a running registry: each new utterance is compared
//! against the speakers heard so far and either joins one or opens a new one.
//!
//! What that costs, stated plainly: an online decision sees less evidence than
//! a global one, so it is less accurate. Early utterances are assigned when the
//! registry is nearly empty and the scatter estimate is weakest. That is why
//! the post-hoc pass still runs at the end and is allowed to overrule this —
//! live assignment is for watching the meeting happen, and the global pass is
//! what the summary is built from.

use super::cluster::{feature_distance, MAX_DISCOVERED_SPEAKERS};
use super::features::VoiceFeatures;

/// Distance below which a new utterance joins an existing speaker before the
/// registry has enough evidence to estimate its own scatter.
///
/// Only used for the first few utterances. It is the clusterer's own absolute
/// noise floor rather than a second constant, so the live and global passes
/// start from the same idea of what "too close to be a different person" means.
const COLD_START_THRESHOLD: f32 = 0.12;

/// Observations needed before the registry trusts its own scatter estimate over
/// the cold-start threshold.
const OBSERVATIONS_BEFORE_SELF_CALIBRATION: usize = 4;

/// How much further than the measured within-speaker scatter an utterance must
/// sit before it opens a new speaker. Matches the global clusterer's multiple,
/// so the two passes disagree because of evidence rather than because of tuning.
const WITHIN_SPEAKER_MULTIPLE: f32 = 3.0;

/// One speaker the registry has heard, and the running mean of their voice.
#[derive(Debug, Clone)]
struct KnownSpeaker {
    centroid: Vec<f32>,
    utterance_count: usize,
    /// Summed microphone share, so the local user can be identified by
    /// comparison once the meeting has run.
    mic_share_total: f32,
    mic_share_count: usize,
}

impl KnownSpeaker {
    /// Folds a new utterance into the running mean.
    ///
    /// A mean rather than every vector kept: the registry runs inside the
    /// recording worker, where holding one vector per utterance for a
    /// three-hour meeting is memory the recorder should not be spending.
    fn absorb(&mut self, vector: &[f32], mic_share: Option<f32>) {
        let n = self.utterance_count as f32;
        for (c, &v) in self.centroid.iter_mut().zip(vector.iter()) {
            *c = (*c * n + v) / (n + 1.0);
        }
        self.utterance_count += 1;
        if let Some(share) = mic_share {
            self.mic_share_total += share;
            self.mic_share_count += 1;
        }
    }

    fn mean_mic_share(&self) -> Option<f32> {
        (self.mic_share_count > 0).then(|| self.mic_share_total / self.mic_share_count as f32)
    }
}

/// What the registry decided about one utterance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiveAssignment {
    /// Zero-based speaker index, in the order voices were first heard.
    pub speaker: usize,
    /// Distance to that speaker's running centre. Large values on an assignment
    /// mean the registry placed it for want of a better option.
    pub distance: f32,
    /// True when this utterance opened a speaker rather than joining one.
    pub is_new_speaker: bool,
}

/// A speaker registry that grows as a meeting is recorded.
#[derive(Debug, Default)]
pub struct IncrementalDiarizer {
    speakers: Vec<KnownSpeaker>,
    /// Distances from utterances to the speaker they joined. The registry's own
    /// read of how far one voice sits from itself on this recording, through
    /// this microphone.
    within_distances: Vec<f32>,
}

impl IncrementalDiarizer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn speaker_count(&self) -> usize {
        self.speakers.len()
    }

    /// The threshold in force right now.
    ///
    /// Cold-start until the registry has seen enough same-speaker distances to
    /// measure the recording, then derived from it — the same self-calibration
    /// the global clusterer does, computed from what has been heard so far.
    pub fn threshold(&self) -> f32 {
        if self.within_distances.len() < OBSERVATIONS_BEFORE_SELF_CALIBRATION {
            return COLD_START_THRESHOLD;
        }
        let mut sorted = self.within_distances.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = sorted[sorted.len() / 2];
        (median * WITHIN_SPEAKER_MULTIPLE).max(COLD_START_THRESHOLD)
    }

    /// Places one utterance, opening a new speaker if it matches nobody.
    ///
    /// Returns `None` when the utterance carries too little voice to place —
    /// the same bar the global pass applies, so a span that is unattributed
    /// after the meeting is also unattributed during it.
    pub fn assign(
        &mut self,
        features: &VoiceFeatures,
        mic_share: Option<f32>,
    ) -> Option<LiveAssignment> {
        if !features.is_usable() {
            return None;
        }
        let vector = weighted(features);

        let nearest = self
            .speakers
            .iter()
            .enumerate()
            .map(|(index, speaker)| (index, super::cluster::distance(&vector, &speaker.centroid)))
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let threshold = self.threshold();
        match nearest {
            Some((index, distance)) if distance <= threshold => {
                self.speakers[index].absorb(&vector, mic_share);
                self.within_distances.push(distance);
                Some(LiveAssignment {
                    speaker: index,
                    distance,
                    is_new_speaker: false,
                })
            }
            // Past the cap, the least wrong answer is the closest speaker.
            // Opening a thirteenth is a claim the features cannot support, and
            // leaving the span blank loses it from the conversation entirely.
            Some((index, distance)) if self.speakers.len() >= MAX_DISCOVERED_SPEAKERS => {
                self.speakers[index].absorb(&vector, mic_share);
                Some(LiveAssignment {
                    speaker: index,
                    distance,
                    is_new_speaker: false,
                })
            }
            _ => {
                let speaker = self.speakers.len();
                self.speakers.push(KnownSpeaker {
                    centroid: vector,
                    utterance_count: 1,
                    mic_share_total: mic_share.unwrap_or(0.0),
                    mic_share_count: usize::from(mic_share.is_some()),
                });
                Some(LiveAssignment {
                    speaker,
                    distance: 0.0,
                    is_new_speaker: true,
                })
            }
        }
    }

    /// The speaker most likely to be the person using this machine.
    ///
    /// Decided by comparison, not by a threshold, which is the whole point:
    /// with speakers rather than headphones no utterance is ever cleanly
    /// microphone-only, so a threshold is never crossed and the local user
    /// never gets identified. Relative microphone share always has an answer.
    ///
    /// Returns `None` when no speaker is clearly ahead of the rest, because a
    /// coin flip about which voice is the user's is worse than saying nothing.
    pub fn local_speaker(&self) -> Option<usize> {
        let mut ranked: Vec<(usize, f32)> = self
            .speakers
            .iter()
            .enumerate()
            .filter_map(|(index, speaker)| speaker.mean_mic_share().map(|share| (index, share)))
            .collect();
        if ranked.is_empty() {
            return None;
        }
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let (best_index, best_share) = ranked[0];
        // The microphone has to actually be the dominant source for that
        // speaker, or this is the quietest remote participant rather than the
        // person in the room.
        if best_share < 0.5 {
            return None;
        }
        match ranked.get(1) {
            Some(&(_, runner_up)) if best_share - runner_up < 0.15 => None,
            _ => Some(best_index),
        }
    }
}

/// The clusterer's weighting, so live and global distances are the same units.
fn weighted(features: &VoiceFeatures) -> Vec<f32> {
    // Routed through the public distance helper's own weighting by constructing
    // the comparison the same way the clusterer does.
    super::cluster::weighted_vector(features)
}

/// Distance between two feature sets, exposed for callers comparing a live
/// assignment against a global one.
pub fn distance_between(a: &VoiceFeatures, b: &VoiceFeatures) -> f32 {
    feature_distance(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meetings_v2::diarize::fixtures;

    fn features_for(voice: &fixtures::VoiceProfile, turn: usize) -> VoiceFeatures {
        fixtures::utterance(voice, turn, 0.0, 3.0).features
    }

    #[test]
    fn one_voice_across_a_meeting_stays_one_speaker() {
        let mut registry = IncrementalDiarizer::new();
        for turn in 0..6 {
            registry.assign(&features_for(&fixtures::THREE_SPEAKERS[0], turn), Some(0.9));
        }
        assert_eq!(registry.speaker_count(), 1);
    }

    #[test]
    fn three_voices_are_found_as_they_arrive() {
        // The property the user asked for: by the time chunk 3 has landed, the
        // recording already knows there are three people in the meeting.
        let mut registry = IncrementalDiarizer::new();
        let mut assignments = Vec::new();
        for turn in 0..3 {
            for voice in &fixtures::THREE_SPEAKERS {
                let a = registry
                    .assign(&features_for(voice, turn), Some(0.2))
                    .expect("a fixture voice is placeable");
                assignments.push((voice.name, a.speaker));
            }
        }

        assert_eq!(registry.speaker_count(), 3, "{assignments:?}");
        // Every turn of one voice landed on the same speaker.
        for voice in &fixtures::THREE_SPEAKERS {
            let seen: Vec<usize> = assignments
                .iter()
                .filter(|(name, _)| *name == voice.name)
                .map(|(_, s)| *s)
                .collect();
            assert!(
                seen.windows(2).all(|w| w[0] == w[1]),
                "voice {} was split across {seen:?}",
                voice.name
            );
        }
    }

    #[test]
    fn speakers_are_numbered_in_the_order_they_were_first_heard() {
        let mut registry = IncrementalDiarizer::new();
        let first = registry
            .assign(&features_for(&fixtures::THREE_SPEAKERS[1], 0), None)
            .unwrap();
        let second = registry
            .assign(&features_for(&fixtures::THREE_SPEAKERS[0], 0), None)
            .unwrap();
        assert_eq!(first.speaker, 0);
        assert!(first.is_new_speaker);
        assert_eq!(second.speaker, 1);
        assert!(second.is_new_speaker);
    }

    #[test]
    fn a_returning_voice_joins_rather_than_opening_a_new_speaker() {
        let mut registry = IncrementalDiarizer::new();
        registry.assign(&features_for(&fixtures::THREE_SPEAKERS[0], 0), None);
        registry.assign(&features_for(&fixtures::THREE_SPEAKERS[1], 0), None);
        let back = registry
            .assign(&features_for(&fixtures::THREE_SPEAKERS[0], 2), None)
            .unwrap();

        assert_eq!(back.speaker, 0);
        assert!(!back.is_new_speaker);
        assert_eq!(registry.speaker_count(), 2);
    }

    #[test]
    fn a_span_with_no_voice_in_it_is_not_placed() {
        let mut registry = IncrementalDiarizer::new();
        let empty = VoiceFeatures {
            mfcc_mean: vec![0.0; 13],
            mfcc_std: vec![0.0; 13],
            pitch_hz: None,
            voiced_fraction: 0.0,
            frame_count: 3,
        };
        assert_eq!(registry.assign(&empty, Some(0.9)), None);
        assert_eq!(registry.speaker_count(), 0);
    }

    #[test]
    fn the_threshold_calibrates_itself_once_there_is_evidence() {
        let mut registry = IncrementalDiarizer::new();
        assert_eq!(registry.threshold(), COLD_START_THRESHOLD);

        // Six turns of one voice give the registry six same-speaker distances.
        for turn in 0..6 {
            registry.assign(&features_for(&fixtures::THREE_SPEAKERS[0], turn), None);
        }
        assert!(
            registry.within_distances.len() >= OBSERVATIONS_BEFORE_SELF_CALIBRATION,
            "the registry must accumulate evidence as it goes"
        );
        // Never below the cold-start floor, however tight the recording.
        assert!(registry.threshold() >= COLD_START_THRESHOLD);
    }

    #[test]
    fn the_local_user_is_the_voice_the_microphone_heard_most() {
        // The reported failure: on speakers rather than headphones nothing is
        // ever cleanly microphone-only, so the user's own voice was labelled
        // Speaker 1. Comparison always has an answer where a threshold does not.
        let mut registry = IncrementalDiarizer::new();
        for turn in 0..3 {
            // The local user: mic clearly dominant, but far from exclusive.
            registry.assign(&features_for(&fixtures::THREE_SPEAKERS[0], turn), Some(0.78));
            // Two people arriving through the call.
            registry.assign(&features_for(&fixtures::THREE_SPEAKERS[1], turn), Some(0.30));
            registry.assign(&features_for(&fixtures::THREE_SPEAKERS[2], turn), Some(0.25));
        }

        assert_eq!(registry.speaker_count(), 3);
        assert_eq!(registry.local_speaker(), Some(0));
    }

    #[test]
    fn nobody_is_called_the_local_user_when_no_voice_stands_out() {
        // An in-person meeting through one microphone: every voice has the same
        // share, and guessing which is the user is worse than saying nothing.
        let mut registry = IncrementalDiarizer::new();
        for turn in 0..3 {
            for voice in &fixtures::THREE_SPEAKERS {
                registry.assign(&features_for(voice, turn), Some(0.95));
            }
        }
        assert_eq!(registry.local_speaker(), None);
    }

    #[test]
    fn a_meeting_where_the_microphone_never_dominates_has_no_local_user() {
        // Everything arrived through the call — a recording of a playback, or a
        // muted microphone. There is no local speaker to find.
        let mut registry = IncrementalDiarizer::new();
        for turn in 0..3 {
            registry.assign(&features_for(&fixtures::THREE_SPEAKERS[0], turn), Some(0.20));
            registry.assign(&features_for(&fixtures::THREE_SPEAKERS[2], turn), Some(0.10));
        }
        assert_eq!(registry.local_speaker(), None);
    }

    #[test]
    fn the_registry_stops_opening_speakers_at_the_cap() {
        let mut registry = IncrementalDiarizer::new();
        // Deliberately far-apart synthetic vectors, one per assignment.
        for i in 0..(MAX_DISCOVERED_SPEAKERS + 5) {
            let features = VoiceFeatures {
                mfcc_mean: (0..13).map(|d| (i * 20 + d * 3) as f32).collect(),
                mfcc_std: vec![1.0; 13],
                pitch_hz: Some(100.0 + i as f32 * 25.0),
                voiced_fraction: 0.8,
                frame_count: 200,
            };
            registry.assign(&features, None);
        }
        assert_eq!(registry.speaker_count(), MAX_DISCOVERED_SPEAKERS);
    }
}
