//! Meeting-local self-voice anchoring and calibrated multi-metric verification.
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
use super::embedding::{cosine_similarity, l2_normalize, AcousticSpectralEmbeddingProvider, SpeakerEmbeddingProvider};
use super::features::{self, VoiceFeatures};
use serde::{Deserialize, Serialize};

/// Minimum duration in seconds for an individual utterance to contribute to the anchor reference.
pub const MIN_ANCHOR_SAMPLE_SECONDS: f64 = 1.2;

/// Minimum number of distinct samples required to establish a valid anchor reference.
pub const MIN_ANCHOR_SAMPLES: usize = 2;

/// Minimum total voiced seconds required across all anchor samples.
pub const MIN_TOTAL_ANCHOR_SECONDS: f64 = 2.5;

/// Maximum distance in classical feature space to count as a possible match.
pub const SELF_VOICE_MATCH_THRESHOLD: f32 = 0.65;

/// Calibrated confidence classification for self-voice identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SelfVoiceConfidence {
    /// Strong acoustic match, wide margin over competitors, solid reference.
    High,
    /// Reasonable acoustic similarity; candidate evidence retained.
    Medium,
    /// Marginal similarity; insufficient for identity assignment.
    Low,
    /// Contradictory acoustics, low SNR, or in-person mode; abstain completely.
    Abstain,
}

/// Detailed multi-metric self-voice decision report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelfVoiceDecision {
    /// Whether the candidate is accepted as the self-voice.
    pub is_match: bool,
    /// Calibrated confidence level.
    pub confidence: SelfVoiceConfidence,
    /// Numeric similarity score in 0.0..=1.0.
    pub candidate_similarity: f32,
    /// Runner up similarity across other clusters or reference voices.
    pub runner_up_similarity: Option<f32>,
    /// Margin between candidate and runner up.
    pub margin: f32,
    /// Quality metric of the reference anchor in 0.0..=1.0.
    pub reference_quality: f32,
    /// Total voiced duration in seconds of the anchor reference.
    pub reference_duration_s: f64,
    /// Number of distinct samples forming the anchor.
    pub reference_samples: usize,
    /// Channel confidence for the microphone stream.
    pub channel_confidence: f32,
}

/// A meeting-local reference acoustic model of the user speaking into their microphone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelfVoiceAnchor {
    /// Averaged classical feature vector across accepted anchor samples.
    pub mean_vector: Vec<f32>,
    /// Normalized embedding vector from the embedding provider.
    #[serde(default)]
    pub mean_embedding: Option<Vec<f32>>,
    /// Number of speech samples incorporated.
    pub sample_count: usize,
    /// Total duration in seconds of speech incorporated into the reference.
    pub total_seconds: f64,
    /// Estimated reference quality (SNR and voiced consistency).
    #[serde(default = "default_quality")]
    pub reference_quality: f32,
}

fn default_quality() -> f32 {
    0.85
}

impl SelfVoiceAnchor {
    /// Builds a self-voice anchor from a collection of candidate audio slices and durations.
    ///
    /// Slices must be 16 kHz mono PCM samples from confident microphone-only speech.
    /// Returns `None` if insufficient speech was available to build a reliable reference or if in-person.
    pub fn build_from_samples(samples: &[(&[f32], f64)], assume_in_person: bool) -> Option<Self> {
        if assume_in_person || samples.is_empty() {
            return None;
        }

        let provider = AcousticSpectralEmbeddingProvider::new();
        let mut qualifying_vectors: Vec<Vec<f32>> = Vec::new();
        let mut qualifying_embeddings: Vec<Vec<f32>> = Vec::new();
        let mut total_duration = 0.0;
        let mut quality_accum = 0.0f32;

        for &(audio, duration_s) in samples {
            if duration_s < MIN_ANCHOR_SAMPLE_SECONDS {
                continue;
            }

            if let Some(feat) = features::extract(audio, 16_000) {
                if feat.is_usable() {
                    qualifying_vectors.push(feat.vector());
                    total_duration += duration_s;
                    quality_accum += feat.voiced_fraction;

                    if let Ok(emb) = provider.embed(audio, 16_000) {
                        qualifying_embeddings.push(emb.vector);
                    }
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

        let mean_embedding = if !qualifying_embeddings.is_empty() {
            let emb_dim = qualifying_embeddings[0].len();
            let mut mean_emb = vec![0.0f32; emb_dim];
            for emb in &qualifying_embeddings {
                for (i, val) in emb.iter().enumerate() {
                    mean_emb[i] += *val;
                }
            }
            l2_normalize(&mut mean_emb);
            Some(mean_emb)
        } else {
            None
        };

        let reference_quality = (quality_accum / count).clamp(0.1, 1.0);

        Some(Self {
            mean_vector,
            mean_embedding,
            sample_count: qualifying_vectors.len(),
            total_seconds: total_duration,
            reference_quality,
        })
    }

    /// Calibrates a full multi-metric identity decision for a candidate audio slice.
    pub fn evaluate_candidate(
        &self,
        audio: &[f32],
        channel_confidence: f32,
        runner_up_sim: Option<f32>,
    ) -> SelfVoiceDecision {
        let provider = AcousticSpectralEmbeddingProvider::new();
        let candidate_sim = if let (Some(ref anchor_emb), Ok(cand_emb)) = (
            &self.mean_embedding,
            provider.embed(audio, 16_000),
        ) {
            cosine_similarity(anchor_emb, &cand_emb.vector).max(0.0)
        } else if let Some((_, dist)) = self.compare_samples(audio) {
            (1.0 - (dist / 1.5)).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let margin = if let Some(ru) = runner_up_sim {
            (candidate_sim - ru).max(0.0)
        } else {
            (candidate_sim - 0.50).max(0.0)
        };

        // Decision calibration matrix
        let confidence = if candidate_sim >= 0.82
            && margin >= 0.15
            && self.sample_count >= MIN_ANCHOR_SAMPLES
            && self.total_seconds >= MIN_TOTAL_ANCHOR_SECONDS
            && channel_confidence >= 0.80
        {
            SelfVoiceConfidence::High
        } else if candidate_sim >= 0.70 && margin >= 0.08 && channel_confidence >= 0.50 {
            SelfVoiceConfidence::Medium
        } else if candidate_sim >= 0.55 {
            SelfVoiceConfidence::Low
        } else {
            SelfVoiceConfidence::Abstain
        };

        let is_match = matches!(confidence, SelfVoiceConfidence::High | SelfVoiceConfidence::Medium);

        SelfVoiceDecision {
            is_match,
            confidence,
            candidate_similarity: candidate_sim,
            runner_up_similarity: runner_up_sim,
            margin,
            reference_quality: self.reference_quality,
            reference_duration_s: self.total_seconds,
            reference_samples: self.sample_count,
            channel_confidence,
        }
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

    /// Compares a normalized speaker embedding directly against the anchor embedding.
    pub fn compare_embedding(&self, emb: &[f32]) -> Option<f32> {
        self.mean_embedding
            .as_ref()
            .map(|anchor_emb| cosine_similarity(anchor_emb, emb))
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
        let short_samples = generate_synthetic_voice(140.0, 0.5);
        assert!(SelfVoiceAnchor::build_from_samples(&[(&short_samples, 0.5)], false).is_none());

        let sample1 = generate_synthetic_voice(140.0, 1.5);
        assert!(SelfVoiceAnchor::build_from_samples(&[(&sample1, 1.5)], false).is_none());

        let sample2 = generate_synthetic_voice(140.0, 1.5);
        assert!(SelfVoiceAnchor::build_from_samples(&[(&sample1, 1.5), (&sample2, 1.5)], true).is_none());

        let anchor = SelfVoiceAnchor::build_from_samples(&[(&sample1, 1.5), (&sample2, 1.5)], false)
            .expect("should build anchor from 2 qualifying samples");
        assert_eq!(anchor.sample_count, 2);
        assert!(anchor.total_seconds >= 3.0);
        assert!(anchor.mean_embedding.is_some());
    }

    #[test]
    fn anchor_calibrated_decision_distinguishes_high_confidence_from_abstain() {
        let sample1 = generate_synthetic_voice(140.0, 1.5);
        let sample2 = generate_synthetic_voice(140.0, 1.5);
        let anchor = SelfVoiceAnchor::build_from_samples(&[(&sample1, 1.5), (&sample2, 1.5)], false)
            .expect("valid anchor");

        // Same voice short candidate ("Yes")
        let same_voice_short = generate_synthetic_voice(140.0, 0.9);
        let decision_high = anchor.evaluate_candidate(&same_voice_short, 1.0, Some(0.60));
        assert!(decision_high.is_match);
        assert_eq!(decision_high.confidence, SelfVoiceConfidence::High);
        assert!(decision_high.margin > 0.15);

        // Competitor / different voice
        let different_voice = generate_synthetic_voice(280.0, 1.0);
        let decision_diff = anchor.evaluate_candidate(&different_voice, 0.5, Some(0.80));
        assert!(!decision_diff.is_match);
        assert!(matches!(
            decision_diff.confidence,
            SelfVoiceConfidence::Low | SelfVoiceConfidence::Abstain
        ));
    }
}
