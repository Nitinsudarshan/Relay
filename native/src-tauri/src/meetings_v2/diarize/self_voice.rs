//! Meeting-local self-voice anchoring.
//!
//! Builds a reference voice profile for the local user from confident, sufficiently long
//! microphone-channel speech samples recorded during the current meeting.
//!
//! This reference is used to identify short local-user interjections (e.g. "yes", "agreed")
//! that are too short for independent clustering, without creating any persistent cross-meeting
//! biometric data.
//!
//! In-person meetings sharing one microphone disable this mechanism entirely, because
//! channel identity does not apply in a single-microphone room.

use super::cluster::distance;
use super::features::{self, VoiceFeatures};
use serde::{Deserialize, Serialize};

/// Minimum duration in seconds for an individual utterance to contribute to the anchor reference.
pub const MIN_ANCHOR_SAMPLE_SECONDS: f64 = 1.2;

/// Minimum number of distinct samples required to establish a valid anchor reference.
pub const MIN_ANCHOR_SAMPLES: usize = 2;

/// Minimum total voiced seconds required across all anchor samples.
pub const MIN_TOTAL_ANCHOR_SECONDS: f64 = 2.5;

/// Maximum distance to count as a confident self-voice match.
/// Same-speaker distance in Relay's calibrated acoustic space is 0.031 – 0.422.
pub const SELF_VOICE_MATCH_THRESHOLD: f32 = 0.65;

/// A meeting-local reference acoustic model of the user speaking into their microphone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelfVoiceAnchor {
    /// Averaged feature vector across accepted anchor samples.
    pub mean_vector: Vec<f32>,
    /// Number of speech samples incorporated.
    pub sample_count: usize,
    /// Total duration in seconds of speech incorporated into the reference.
    pub total_seconds: f64,
}

impl SelfVoiceAnchor {
    /// Builds a self-voice anchor from a collection of candidate audio slices and durations.
    ///
    /// Slices must be 16 kHz mono PCM samples from confident microphone-only speech.
    /// Returns `None` if insufficient speech was available to build a reliable reference.
    pub fn build_from_samples(samples: &[(&[f32], f64)], assume_in_person: bool) -> Option<Self> {
        if assume_in_person || samples.is_empty() {
            return None;
        }

        let mut qualifying_vectors: Vec<Vec<f32>> = Vec::new();
        let mut total_duration = 0.0;

        for &(audio, duration_s) in samples {
            if duration_s < MIN_ANCHOR_SAMPLE_SECONDS {
                continue;
            }

            if let Some(feat) = features::extract(audio, 16_000) {
                if feat.is_usable() {
                    qualifying_vectors.push(feat.vector());
                    total_duration += duration_s;
                }
            }
        }

        if qualifying_vectors.len() < MIN_ANCHOR_SAMPLES || total_duration < MIN_TOTAL_ANCHOR_SECONDS {
            return None;
        }

        let vector_len = qualifying_vectors[0].len();
        let count = qualifying_vectors.len() as f32;
        let mut mean_vector = vec![0.0f32; vector_len];

        for vec in &qualifying_vectors {
            for (i, val) in vec.iter().enumerate() {
                mean_vector[i] += *val;
            }
        }

        for val in &mut mean_vector {
            *val /= count;
        }

        Some(Self {
            mean_vector,
            sample_count: qualifying_vectors.len(),
            total_seconds: total_duration,
        })
    }

    /// Measures the acoustic distance between candidate features and the self-voice anchor.
    /// Returns `Some((is_match, distance))` where distance < `SELF_VOICE_MATCH_THRESHOLD` indicates a match.
    pub fn compare(&self, candidate: &VoiceFeatures) -> Option<(bool, f32)> {
        let cand_vec = candidate.vector();
        if cand_vec.len() != self.mean_vector.len() {
            return None;
        }

        let dist = distance(&self.mean_vector, &cand_vec);
        Some((dist <= SELF_VOICE_MATCH_THRESHOLD, dist))
    }

    /// Compares raw 16 kHz audio samples directly against the anchor.
    pub fn compare_samples(&self, audio: &[f32]) -> Option<(bool, f32)> {
        let feat = features::extract(audio, 16_000)?;
        self.compare(&feat)
    }

    /// Whether this anchor contains at least one accepted speech sample.
    pub fn has_samples(&self) -> bool {
        self.sample_count > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generate_synthetic_voice(freq: f32, duration_s: f64) -> Vec<f32> {
        let samples_count = (duration_s * 16_000.0) as usize;
        (0..samples_count)
            .map(|i| {
                let t = i as f32 / 16_000.0;
                0.3 * (2.0 * std::f32::consts::PI * freq * t).sin()
                    + 0.15 * (2.0 * std::f32::consts::PI * freq * 2.0 * t).sin()
                    + 0.08 * (2.0 * std::f32::consts::PI * freq * 3.0 * t).sin()
            })
            .collect()
    }

    #[test]
    fn anchor_requires_multiple_long_samples() {
        // Too short duration
        let short_samples = generate_synthetic_voice(140.0, 0.5);
        assert!(SelfVoiceAnchor::build_from_samples(&[(&short_samples, 0.5)], false).is_none());

        // Single sample is not enough
        let sample1 = generate_synthetic_voice(140.0, 1.5);
        assert!(SelfVoiceAnchor::build_from_samples(&[(&sample1, 1.5)], false).is_none());

        // In-person flag disables anchor
        let sample2 = generate_synthetic_voice(140.0, 1.5);
        assert!(SelfVoiceAnchor::build_from_samples(&[(&sample1, 1.5), (&sample2, 1.5)], true).is_none());

        // Valid anchor
        let anchor = SelfVoiceAnchor::build_from_samples(&[(&sample1, 1.5), (&sample2, 1.5)], false)
            .expect("should build anchor from 2 qualifying samples");
        assert_eq!(anchor.sample_count, 2);
        assert!(anchor.total_seconds >= 3.0);
    }

    #[test]
    fn anchor_matches_same_voice_and_rejects_different_voice() {
        let sample1 = generate_synthetic_voice(150.0, 1.5);
        let sample2 = generate_synthetic_voice(150.0, 1.5);
        let anchor = SelfVoiceAnchor::build_from_samples(&[(&sample1, 1.5), (&sample2, 1.5)], false)
            .expect("anchor builds");

        // Same speaker saying short 0.9s "yes"
        let short_same = generate_synthetic_voice(150.0, 0.9);
        let (matches_same, dist_same) = anchor.compare_samples(&short_same).expect("can compare");
        assert!(matches_same, "same speaker should match anchor, dist: {dist_same}");

        // Different speaker with pitch 280 Hz
        let diff = generate_synthetic_voice(280.0, 1.2);
        let (matches_diff, dist_diff) = anchor.compare_samples(&diff).expect("can compare");
        assert!(!matches_diff, "different speaker should not match anchor, dist: {dist_diff}");
        assert!(dist_diff > dist_same);
    }
}
