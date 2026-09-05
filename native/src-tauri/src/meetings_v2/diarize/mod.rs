//! Rung 4 of `Meeting-rules/meeting_speaker_identification.md`: separating the
//! audio into distinct voices.
//!
//! Before this existed, speaker attribution had exactly two outcomes —
//! microphone input was the local user, everything else was `Speaker 1`. That
//! is correct as far as it goes and it is rung 1, but it means a meeting with
//! twenty people in it reports two speakers, and every action item owned by
//! anyone other than the local user resolves to the same anonymous bucket. The
//! screenshot that prompted this work shows a 44-minute meeting whose entire
//! remote side is one chip reading "Speaker 1".
//!
//! What this module does: reads the chunk WAVs the recorder already wrote,
//! cuts them at the utterance boundaries Whisper already reported, computes a
//! voice feature vector per utterance, and clusters those vectors. Nothing is
//! re-recorded and nothing is re-transcribed.
//!
//! Three properties it is responsible for holding:
//!
//! * **It never rewrites a transcript.** Attribution is a separate layer, keyed
//!   by utterance id, so it can be re-run, corrected, or discarded without
//!   touching `transcript.jsonl`.
//! * **It creates no biometric data.** Features live for the duration of one
//!   call and are never persisted or matched across meetings. The speaker
//!   library in §6 of the rules — which *would* create biometric data — is
//!   deliberately still absent.
//! * **It reports its own confidence.** A marginal split is returned as
//!   marginal rather than presented as fact, because two similar voices on one
//!   channel are beyond what MFCC statistics can resolve.

pub mod benchmarks;
pub mod cluster;
pub mod embedding;
/// The three ways Relay can decide who spoke, and a way to compare them on one
/// recording rather than by holding another meeting.
pub mod engine;
pub mod features;
/// Speaker assignment while a meeting is still being recorded.
pub mod incremental;
pub mod self_voice;
/// Synthetic voices that behave like real ones. Test-only, and the reason the
/// first calibration of this module was wrong.
#[cfg(test)]
pub mod fixtures;

pub use benchmarks::{BenchmarkMetrics, BenchmarkReport, run_full_benchmark_suite};
pub use embedding::{
    AcousticSpectralEmbeddingProvider, DynamicSpeakerEmbeddingProvider, SpeakerEmbedding,
    SpeakerEmbeddingProvider,
};
pub use self_voice::SelfVoiceAnchor;

use super::session_store::SessionStore;
use super::types::{TranscriptSegment, TranscriptSegmentStatus};
use hound::WavReader;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Shortest utterance worth extracting features from, in seconds.
///
/// Below this the MFCC statistics are dominated by whichever phoneme happened
/// to be in the window rather than by the voice, and a wrong attribution is
/// worse than none.
const MIN_UTTERANCE_SECONDS: f64 = 0.8;

/// Share of an utterance's energy that must come from the microphone before a
/// cluster can be the local user.
///
/// Calibrated to 0.30 to account for laptop speaker leakage into the microphone,
/// quiet microphone capture, or distant user positioning while still requiring
/// a distinct margin over the remote speaker.
const LOCAL_MIC_SHARE_MINIMUM: f32 = 0.30;

/// How far ahead of the next cluster the local user's must sit.
const LOCAL_MIC_SHARE_MARGIN: f32 = 0.10;

/// How a diarization run turned out, kept alongside the derived model so the UI
/// can explain the roster it is showing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiarizationReport {
    /// Distinct voices found. Zero means the audio supported no attribution at
    /// all, which is different from a failure.
    pub cluster_count: usize,
    /// Utterances that were clustered.
    pub placed_count: usize,
    /// Utterances with too little voice to place.
    pub unplaced_count: usize,
    /// Utterances skipped because their audio was missing or too short.
    pub skipped_count: usize,
    /// Which cluster is the person using this machine, when the recording says.
    ///
    /// Decided by comparing microphone share *between* clusters rather than by
    /// testing any one utterance against a threshold. That distinction is the
    /// fix for a reported failure: with speakers rather than headphones the
    /// microphone picks up the remote party, so no utterance is ever cleanly
    /// microphone-only, no threshold is ever crossed, and the user's own voice
    /// came back labelled `Speaker 1`. A comparison always has an answer.
    ///
    /// `None` when no cluster stands out — an in-person meeting through one
    /// microphone, or a recording where the user never spoke.
    #[serde(default)]
    pub local_cluster: Option<usize>,
    /// True when the clusters are further from each other than their members
    /// are from their own centre. False means the roster is provisional and the
    /// UI must say so.
    pub well_separated: bool,
    pub mean_within_distance: f32,
    pub min_between_distance: f32,
    /// Speakers heard exactly once. A person who spoke once is real, and so is
    /// a stray utterance that looked like one; the number is surfaced so the UI
    /// can say "heard once" rather than Relay deciding which it was.
    #[serde(default)]
    pub singleton_speaker_count: usize,
    /// How well the roster actually describes the recording, as a mean
    /// silhouette in `-1.0..=1.0`.
    ///
    /// The number the speaker count was decided on, and therefore the number to
    /// look at when a roster is wrong. Above 0.7 the groups are clearly
    /// separate voices; between 0.5 and 0.7 the split is worth making and worth
    /// checking; below 0.5 no split is made at all. Zero means one speaker,
    /// where a silhouette is undefined.
    #[serde(default)]
    pub silhouette: f32,
    /// The hint the run was given, if any.
    pub expected_speakers: Option<usize>,
    pub duration_ms: u64,
    /// Which embedding provider characterises speakers.
    #[serde(default)]
    pub embedding_provider: Option<String>,
    /// Whether the engine fell back from a primary neural model to the acoustic floor.
    #[serde(default)]
    pub fallback_used: bool,
    /// Time spent computing speaker embeddings, in milliseconds.
    #[serde(default)]
    pub embedding_duration_ms: u64,
}

impl Default for DiarizationReport {
    fn default() -> Self {
        Self {
            cluster_count: 0,
            placed_count: 0,
            unplaced_count: 0,
            skipped_count: 0,
            local_cluster: None,
            well_separated: false,
            mean_within_distance: 0.0,
            min_between_distance: 0.0,
            singleton_speaker_count: 0,
            silhouette: 0.0,
            expected_speakers: None,
            duration_ms: 0,
            embedding_provider: None,
            fallback_used: false,
            embedding_duration_ms: 0,
        }
    }
}

/// The per-utterance result of a diarization run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VoiceAssignment {
    /// `seg_<chunk>_<utterance>`, matching the normalized transcript's ids.
    pub segment_id: String,
    /// Zero-based cluster index, ordered by when each voice was first heard.
    /// `None` where the audio could not place this utterance.
    pub cluster: Option<usize>,
    /// Distance from the cluster centre. Higher is a weaker fit.
    pub distance: f32,
}

/// Everything one diarization run produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diarization {
    pub report: DiarizationReport,
    pub assignments: Vec<VoiceAssignment>,
    #[serde(default)]
    pub self_voice_anchor: Option<SelfVoiceAnchor>,
}

impl Diarization {
    /// Cluster index for a segment id, if one was assigned.
    pub fn cluster_for(&self, segment_id: &str) -> Option<usize> {
        self.assignments
            .iter()
            .find(|a| a.segment_id == segment_id)
            .and_then(|a| a.cluster)
    }

    /// A map from segment id to cluster, for callers assigning a whole
    /// transcript at once.
    pub fn cluster_map(&self) -> HashMap<&str, usize> {
        self.assignments
            .iter()
            .filter_map(|a| a.cluster.map(|c| (a.segment_id.as_str(), c)))
            .collect()
    }
}

/// Runs diarization over a finished recording.
///
/// Post-hoc by design, per §3.2 of the rules: the recording must have ended,
/// because clustering needs to see the whole meeting before it can say how many
/// voices were in it. Returns `Err` only when the audio is gone — an audio-less
/// meeting is a state the UI must state plainly rather than a silent no-op.
pub fn diarize_session(
    store: &SessionStore,
    session_id: &str,
    expected_speakers: Option<usize>,
) -> Result<Diarization, String> {
    diarize_session_with(store, session_id, expected_speakers, false)
}

/// Runs diarization, optionally treating the meeting as in-person.
///
/// `assume_in_person` says everybody shared one microphone. It disables the
/// local-user inference, because the channel split that identifies the person
/// at this machine is meaningless when every voice arrives through the same
/// input — and a guess there mislabels whoever it lands on.
pub fn diarize_session_with(
    store: &SessionStore,
    session_id: &str,
    expected_speakers: Option<usize>,
    assume_in_person: bool,
) -> Result<Diarization, String> {
    let started = std::time::Instant::now();

    let segments = store
        .get_transcript_segments(session_id)
        .map_err(|e| format!("Failed to read the transcript: {e}"))?;

    let chunk_files = store
        .list_chunk_files(session_id)
        .map_err(|e| format!("Failed to list the recorded audio: {e}"))?;
    if chunk_files.is_empty() {
        return Err(
            "This meeting's audio has been discarded, so its speakers cannot be separated. \
The transcript and summary are unaffected."
                .to_string(),
        );
    }

    let spans = collect_spans(&segments);
    let mut utterances: Vec<cluster::Utterance> = Vec::new();
    let mut mic_shares: HashMap<String, Option<f32>> = HashMap::new();
    let mut skipped = 0usize;

    // One WAV read per chunk, however many utterances it holds. Reading per
    // utterance would reopen the same file up to a dozen times.
    let mut by_chunk: HashMap<usize, Vec<&UtteranceSpan>> = HashMap::new();
    for span in &spans {
        by_chunk.entry(span.chunk_index).or_default().push(span);
    }

    let mut chunk_indices: Vec<usize> = by_chunk.keys().copied().collect();
    chunk_indices.sort_unstable();

    let embedding_provider = DynamicSpeakerEmbeddingProvider::new();
    let mut embedding_duration_ms = 0u64;
    let mut fallback_used = false;

    let mut anchor_samples: Vec<(Vec<f32>, f64)> = Vec::new();

    for chunk_index in chunk_indices {
        let path = store.chunk_path(session_id, chunk_index);
        let samples = match read_chunk_samples(&path) {
            Ok(samples) => samples,
            Err(e) => {
                tracing::debug!("diarize: chunk #{} unreadable ({}); skipping", chunk_index, e);
                skipped += by_chunk.get(&chunk_index).map(|v| v.len()).unwrap_or(0);
                continue;
            }
        };

        for span in by_chunk.get(&chunk_index).into_iter().flatten() {
            let Some(slice) = slice_for(&samples, span) else {
                skipped += 1;
                continue;
            };

            // Collect anchor samples from confident microphone speech if not in-person
            if !assume_in_person && span.duration_s >= self_voice::MIN_ANCHOR_SAMPLE_SECONDS {
                if let Some(share) = span.mic_share {
                    if share >= 0.60 {
                        anchor_samples.push((slice.to_vec(), span.duration_s));
                    }
                }
            }

            match features::extract(slice, 16_000) {
                Some(f) => {
                    let emb_start = std::time::Instant::now();
                    let embedding = match embedding_provider.embed_with_status(slice, 16_000) {
                        Ok((emb, used_fb)) => {
                            if used_fb {
                                fallback_used = true;
                            }
                            Some(emb)
                        }
                        Err(_) => None,
                    };
                    embedding_duration_ms += emb_start.elapsed().as_millis() as u64;

                    mic_shares.insert(span.segment_id.clone(), span.mic_share);
                    utterances.push(cluster::Utterance {
                        id: span.segment_id.clone(),
                        start_time_s: span.start_time_s,
                        end_time_s: span.end_time_s,
                        features: f,
                        embedding,
                    });
                }
                None => skipped += 1,
            }
        }
    }

    let self_voice_anchor = if !assume_in_person && !anchor_samples.is_empty() {
        let sample_refs: Vec<(&[f32], f64)> = anchor_samples
            .iter()
            .map(|(s, d)| (s.as_slice(), *d))
            .collect();
        SelfVoiceAnchor::build_from_samples(&sample_refs, assume_in_person)
    } else {
        None
    };

    let clustering = cluster::cluster(&utterances, expected_speakers);
    let placed = clustering
        .assignments
        .iter()
        .filter(|a| a.cluster.is_some())
        .count();
    let local_cluster = if assume_in_person {
        // One microphone carrying everybody. There is no channel evidence to
        // separate the person holding it from the people around them, and
        // picking the loudest would be a guess dressed as a finding.
        None
    } else {
        let from_mic = local_cluster_from_mic_share(&clustering, &mic_shares);
        if from_mic.is_some() {
            from_mic
        } else if let Some(ref anchor) = self_voice_anchor {
            local_cluster_from_anchor(anchor, &clustering, &utterances)
        } else {
            None
        }
    };

    let report = DiarizationReport {
        cluster_count: clustering.cluster_count,
        placed_count: placed,
        unplaced_count: clustering.unplaced_count,
        skipped_count: skipped,
        local_cluster,
        well_separated: clustering.is_well_separated(),
        mean_within_distance: clustering.mean_within_distance,
        min_between_distance: clustering.min_between_distance,
        singleton_speaker_count: clustering.singleton_cluster_count,
        silhouette: clustering.silhouette,
        expected_speakers,
        duration_ms: started.elapsed().as_millis() as u64,
        embedding_provider: Some(embedding_provider.name().to_string()),
        fallback_used,
        embedding_duration_ms,
    };

    tracing::info!(
        session_id = %session_id,
        clusters = report.cluster_count,
        placed = report.placed_count,
        unplaced = report.unplaced_count,
        skipped = report.skipped_count,
        well_separated = report.well_separated,
        duration_ms = report.duration_ms,
        "diarize: clustering complete"
    );

    Ok(Diarization {
        report,
        assignments: clustering
            .assignments
            .into_iter()
            .map(|a| VoiceAssignment {
                segment_id: a.id,
                cluster: a.cluster,
                distance: a.distance,
            })
            .collect(),
        self_voice_anchor,
    })
}

/// Identifies the local user's cluster by matching against the meeting-local self-voice anchor.
fn local_cluster_from_anchor(
    anchor: &SelfVoiceAnchor,
    clustering: &cluster::Clustering,
    utterances: &[cluster::Utterance],
) -> Option<usize> {
    if !anchor.has_samples() || clustering.cluster_count == 0 {
        return None;
    }

    let mut cluster_scores: HashMap<usize, (f32, usize)> = HashMap::new();
    for assignment in &clustering.assignments {
        let Some(cluster_idx) = assignment.cluster else {
            continue;
        };
        let Some(utt) = utterances.iter().find(|u| u.id == assignment.id) else {
            continue;
        };

        let sim = if let Some(ref emb) = utt.embedding {
            anchor.compare_embedding(&emb.vector).unwrap_or(0.0)
        } else if let Some((_, dist)) = anchor.compare(&utt.features) {
            (1.0 - (dist / 1.5)).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let entry = cluster_scores.entry(cluster_idx).or_insert((0.0, 0));
        entry.0 += sim;
        entry.1 += 1;
    }

    let mut ranked: Vec<(usize, f32)> = cluster_scores
        .into_iter()
        .filter(|(_, (_, count))| *count > 0)
        .map(|(cluster_idx, (sum, count))| (cluster_idx, sum / count as f32))
        .collect();

    if ranked.is_empty() {
        return None;
    }
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let (best_index, best_sim) = ranked[0];
    if best_sim < 0.65 {
        return None;
    }
    match ranked.get(1) {
        Some(&(_, runner_up)) if best_sim - runner_up < 0.08 => None,
        _ => Some(best_index),
    }
}

/// The cluster whose utterances the microphone heard most.
///
/// Returns `None` unless one cluster is both microphone-dominant in absolute
/// terms and clearly ahead of the next — a coin flip about which voice belongs
/// to the user is worse than leaving it unattributed, because a wrong "Me"
/// attaches the user's name to somebody else's commitments.
fn local_cluster_from_mic_share(
    clustering: &cluster::Clustering,
    mic_shares: &HashMap<String, Option<f32>>,
) -> Option<usize> {
    let mut totals: HashMap<usize, (f32, usize)> = HashMap::new();
    for assignment in &clustering.assignments {
        let Some(index) = assignment.cluster else {
            continue;
        };
        let Some(Some(share)) = mic_shares.get(&assignment.id) else {
            continue;
        };
        let entry = totals.entry(index).or_insert((0.0, 0));
        entry.0 += share;
        entry.1 += 1;
    }

    let mut ranked: Vec<(usize, f32)> = totals
        .into_iter()
        .map(|(index, (sum, count))| (index, sum / count as f32))
        .collect();
    if ranked.is_empty() {
        return None;
    }
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let (best_index, best_share) = ranked[0];
    if best_share < LOCAL_MIC_SHARE_MINIMUM {
        return None;
    }
    match ranked.get(1) {
        Some(&(_, runner_up)) if best_share - runner_up < LOCAL_MIC_SHARE_MARGIN => None,
        _ => Some(best_index),
    }
}

/// One stretch of audio to characterise, located within its chunk.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UtteranceSpan {
    segment_id: String,
    chunk_index: usize,
    /// Session-clock bounds, as the transcript records them.
    start_time_s: f64,
    end_time_s: f64,
    /// Offset within the chunk's own audio.
    offset_in_chunk_s: f64,
    duration_s: f64,
    /// Share of this span's energy that came from the microphone, when the
    /// recorder measured it. The signal that identifies the local user.
    mic_share: Option<f32>,
}

/// Builds the list of spans to characterise from the raw transcript.
///
/// Uses Whisper's utterance timings where the recorder resolved them, and falls
/// back to the whole chunk where it did not — a pre-v2.5 transcript, or a chunk
/// Whisper returned no timed spans for. Rejected and empty chunks are skipped:
/// there is no voice in them to attribute.
pub(crate) fn collect_spans(segments: &[TranscriptSegment]) -> Vec<UtteranceSpan> {
    let mut spans = Vec::new();

    for segment in segments {
        if segment.status != TranscriptSegmentStatus::Success {
            continue;
        }

        if segment.utterances.is_empty() {
            let duration = segment.end_time_s - segment.start_time_s;
            if duration < MIN_UTTERANCE_SECONDS || segment.text.trim().is_empty() {
                continue;
            }
            spans.push(UtteranceSpan {
                segment_id: format!("seg_{:05}", segment.chunk_index),
                chunk_index: segment.chunk_index,
                start_time_s: segment.start_time_s,
                end_time_s: segment.end_time_s,
                offset_in_chunk_s: 0.0,
                duration_s: duration,
                mic_share: None,
            });
            continue;
        }

        for utterance in &segment.utterances {
            let duration = utterance.end_time_s - utterance.start_time_s;
            if duration < MIN_UTTERANCE_SECONDS || utterance.text.trim().is_empty() {
                continue;
            }
            spans.push(UtteranceSpan {
                segment_id: format!("seg_{:05}_{:03}", segment.chunk_index, utterance.index),
                chunk_index: segment.chunk_index,
                start_time_s: utterance.start_time_s,
                end_time_s: utterance.end_time_s,
                offset_in_chunk_s: (utterance.start_time_s - segment.start_time_s).max(0.0),
                duration_s: duration,
                mic_share: utterance.mic_share(),
            });
        }
    }

    spans
}

/// The samples covering one span, or `None` when the span falls outside the
/// audio that was actually written.
pub(crate) fn slice_for<'a>(samples: &'a [f32], span: &UtteranceSpan) -> Option<&'a [f32]> {
    let start = (span.offset_in_chunk_s * 16_000.0) as usize;
    let len = (span.duration_s * 16_000.0) as usize;
    if start >= samples.len() || len == 0 {
        return None;
    }
    let end = (start + len).min(samples.len());
    let slice = &samples[start..end];
    (slice.len() as f64 / 16_000.0 >= MIN_UTTERANCE_SECONDS).then_some(slice)
}

/// Reads a 16-bit mono chunk WAV back into normalized floats.
pub(crate) fn read_chunk_samples(path: &Path) -> Result<Vec<f32>, String> {
    let mut reader = WavReader::open(path).map_err(|e| e.to_string())?;
    Ok(reader
        .samples::<i16>()
        .flatten()
        .map(|s| s as f32 / i16::MAX as f32)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meetings_v2::types::TranscriptUtterance;

    fn utterance(index: usize, start: f64, end: f64, text: &str) -> TranscriptUtterance {
        TranscriptUtterance {
            index,
            start_time_s: start,
            end_time_s: end,
            text: text.to_string(),
            mic_had_audio: true,
            sys_had_audio: false,
            no_speech_prob: 0.01,
            mic_rms: 0.08,
            sys_rms: 0.002,
            live_speaker: None,
        }
    }

    fn segment(
        chunk_index: usize,
        status: TranscriptSegmentStatus,
        text: &str,
        utterances: Vec<TranscriptUtterance>,
    ) -> TranscriptSegment {
        TranscriptSegment {
            chunk_index,
            start_time_s: chunk_index as f64 * 30.0,
            end_time_s: (chunk_index + 1) as f64 * 30.0,
            text: text.to_string(),
            created_at: "2026-09-04T00:00:00Z".to_string(),
            status,
            mic_had_audio: true,
            sys_had_audio: false,
            utterances,
            speech: None,
            rejection: None,
        }
    }

// Probe: how far apart do *realistic* voices sit in the current feature space?
    #[test]
    fn spans_come_from_whispers_own_utterance_timings() {
        let segments = vec![segment(
            2,
            TranscriptSegmentStatus::Success,
            "two things",
            vec![
                utterance(0, 60.0, 64.0, "first thing"),
                utterance(1, 64.0, 70.0, "second thing"),
            ],
        )];
        let spans = collect_spans(&segments);

        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].segment_id, "seg_00002_000");
        assert_eq!(spans[0].offset_in_chunk_s, 0.0);
        assert_eq!(spans[1].segment_id, "seg_00002_001");
        assert_eq!(spans[1].offset_in_chunk_s, 4.0);
        assert_eq!(spans[1].duration_s, 6.0);
    }

    #[test]
    fn a_chunk_without_utterance_timings_becomes_one_span() {
        let segments = vec![segment(
            0,
            TranscriptSegmentStatus::Success,
            "a legacy chunk",
            Vec::new(),
        )];
        let spans = collect_spans(&segments);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].segment_id, "seg_00000");
        assert_eq!(spans[0].duration_s, 30.0);
    }

    #[test]
    fn rejected_and_empty_chunks_contribute_no_spans() {
        let segments = vec![
            segment(0, TranscriptSegmentStatus::Rejected, "", Vec::new()),
            segment(1, TranscriptSegmentStatus::Empty, "", Vec::new()),
            segment(2, TranscriptSegmentStatus::Failed, "", Vec::new()),
        ];
        assert!(collect_spans(&segments).is_empty());
    }

    #[test]
    fn utterances_too_short_to_characterise_are_left_out() {
        let segments = vec![segment(
            0,
            TranscriptSegmentStatus::Success,
            "yes",
            vec![
                utterance(0, 0.0, 0.4, "yes"),
                utterance(1, 1.0, 6.0, "a long enough answer to work with"),
            ],
        )];
        let spans = collect_spans(&segments);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].segment_id, "seg_00000_001");
    }

    #[test]
    fn an_empty_utterance_is_left_out_even_when_long() {
        let segments = vec![segment(
            0,
            TranscriptSegmentStatus::Success,
            " ",
            vec![utterance(0, 0.0, 10.0, "   ")],
        )];
        assert!(collect_spans(&segments).is_empty());
    }

    #[test]
    fn a_span_is_sliced_out_of_its_own_chunks_audio() {
        let samples: Vec<f32> = (0..16_000 * 30).map(|i| i as f32).collect();
        let span = UtteranceSpan {
            segment_id: "seg_00000_000".into(),
            chunk_index: 0,
            start_time_s: 4.0,
            end_time_s: 9.0,
            offset_in_chunk_s: 4.0,
            duration_s: 5.0,
            mic_share: Some(0.9),
        };
        let slice = slice_for(&samples, &span).unwrap();
        assert_eq!(slice.len(), 16_000 * 5);
        assert_eq!(slice[0], (16_000 * 4) as f32);
    }

    #[test]
    fn a_span_past_the_end_of_the_audio_is_declined() {
        let samples = vec![0.1f32; 16_000];
        let span = UtteranceSpan {
            segment_id: "seg_00000_000".into(),
            chunk_index: 0,
            start_time_s: 20.0,
            end_time_s: 25.0,
            offset_in_chunk_s: 20.0,
            duration_s: 5.0,
            mic_share: Some(0.9),
        };
        assert!(slice_for(&samples, &span).is_none());
    }

    #[test]
    fn a_span_truncated_below_the_minimum_is_declined() {
        // The recording stopped mid-utterance: half a second of audio is not
        // enough to say whose voice it was.
        let samples = vec![0.1f32; 16_000 / 2];
        let span = UtteranceSpan {
            segment_id: "seg_00000_000".into(),
            chunk_index: 0,
            start_time_s: 0.0,
            end_time_s: 5.0,
            offset_in_chunk_s: 0.0,
            duration_s: 5.0,
            mic_share: Some(0.9),
        };
        assert!(slice_for(&samples, &span).is_none());
    }

    // -----------------------------------------------------------------------
    // End to end, against real WAVs in a temporary vault.
    //
    // The unit tests above cover span selection and slicing; these cover the
    // path the app actually takes: audio on disk, transcript on disk, roster
    // out. Without them a change to the WAV round-trip or the chunk naming
    // would pass every other test in the module.
    // -----------------------------------------------------------------------

    use crate::meetings_v2::session_store::SessionStore;
    use crate::meetings_v2::types::{MeetingSession, MeetingState};

    /// A synthetic voice, distinct per `f0`/`formant_scale` pair.
    fn synth_voice(f0: f32, formant_scale: f32, seconds: f64) -> Vec<f32> {
        let n = (16_000.0 * seconds) as usize;
        (0..n)
            .map(|i| {
                let t = i as f32 / 16_000.0;
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

    struct VaultHarness {
        vault: std::path::PathBuf,
        store: SessionStore,
        meeting_id: String,
    }

    impl VaultHarness {
        /// Builds a meeting whose chunk *n* holds voice `voices[n]`.
        fn new(voices: &[(f32, f32)]) -> Self {
            let vault =
                std::env::temp_dir().join(format!("relay_test_diarize_{}", uuid::Uuid::new_v4()));
            let store = SessionStore::new(vault.clone());
            let meeting_id = "meet_diarize".to_string();

            let mut session = MeetingSession::new(meeting_id.clone(), None);
            session.state = MeetingState::Completed;
            store.init_session(&session).unwrap();

            for (index, &(f0, formant)) in voices.iter().enumerate() {
                let samples = synth_voice(f0, formant, 6.0);
                store
                    .write_chunk_wav(&meeting_id, index, &samples, 16_000)
                    .unwrap();
                store
                    .append_transcript_segment(
                        &meeting_id,
                        &TranscriptSegment {
                            chunk_index: index,
                            start_time_s: index as f64 * 30.0,
                            end_time_s: index as f64 * 30.0 + 6.0,
                            text: format!("turn {index}"),
                            created_at: "2026-09-04T10:00:00Z".to_string(),
                            status: TranscriptSegmentStatus::Success,
                            mic_had_audio: index == 0,
                            sys_had_audio: index != 0,
                            utterances: vec![utterance(
                                0,
                                index as f64 * 30.0,
                                index as f64 * 30.0 + 6.0,
                                &format!("turn {index}"),
                            )],
                            speech: None,
                            rejection: None,
                        },
                    )
                    .unwrap();
            }

            Self {
                vault,
                store,
                meeting_id,
            }
        }
    }

    impl Drop for VaultHarness {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.vault);
        }
    }

    #[test]
    fn three_voices_on_disk_come_back_as_three_clusters() {
        // Each voice speaks twice, interleaved.
        let harness = VaultHarness::new(&[
            (105.0, 1.0),
            (230.0, 1.55),
            (160.0, 1.25),
            (107.0, 1.02),
            (228.0, 1.53),
            (162.0, 1.24),
        ]);

        let diarization =
            diarize_session(&harness.store, &harness.meeting_id, None).expect("audio is present");

        assert_eq!(
            diarization.report.cluster_count, 3,
            "within {} between {}",
            diarization.report.mean_within_distance, diarization.report.min_between_distance
        );
        assert_eq!(diarization.report.placed_count, 6);
        assert_eq!(diarization.report.unplaced_count, 0);
        assert_eq!(diarization.report.skipped_count, 0);

        // Each voice's two turns must land together.
        let cluster_of = |chunk: usize| {
            diarization
                .cluster_for(&format!("seg_{:05}_000", chunk))
                .expect("every turn was placed")
        };
        assert_eq!(cluster_of(0), cluster_of(3));
        assert_eq!(cluster_of(1), cluster_of(4));
        assert_eq!(cluster_of(2), cluster_of(5));
        assert_ne!(cluster_of(0), cluster_of(1));
        assert_ne!(cluster_of(1), cluster_of(2));
    }

    /// Builds a meeting from realistic voices, with per-utterance channel
    /// energies as the recorder would have measured them.
    fn realistic_vault(
        voices: &[(fixtures::VoiceProfile, f32)],
        turns: usize,
    ) -> VaultHarness {
        let vault =
            std::env::temp_dir().join(format!("relay_test_diarize_{}", uuid::Uuid::new_v4()));
        let store = SessionStore::new(vault.clone());
        let meeting_id = "meet_realistic".to_string();

        let mut session = MeetingSession::new(meeting_id.clone(), None);
        session.state = MeetingState::Completed;
        store.init_session(&session).unwrap();

        let mut chunk = 0usize;
        for turn in 0..turns {
            for (voice, mic_share) in voices {
                let samples = fixtures::utterance_audio(voice, turn, 6.0);
                store
                    .write_chunk_wav(&meeting_id, chunk, &samples, 16_000)
                    .unwrap();
                // The energies the mixer would have recorded for this span.
                let loudness = 0.08f32;
                store
                    .append_transcript_segment(
                        &meeting_id,
                        &TranscriptSegment {
                            chunk_index: chunk,
                            start_time_s: chunk as f64 * 30.0,
                            end_time_s: chunk as f64 * 30.0 + 6.0,
                            text: format!("turn {chunk}"),
                            created_at: "2026-09-04T10:00:00Z".to_string(),
                            status: TranscriptSegmentStatus::Success,
                            // Both sources audible on every utterance, which is
                            // what a call taken on speakers actually looks like.
                            mic_had_audio: true,
                            sys_had_audio: true,
                            utterances: vec![TranscriptUtterance {
                                index: 0,
                                start_time_s: chunk as f64 * 30.0,
                                end_time_s: chunk as f64 * 30.0 + 6.0,
                                text: format!("turn {chunk}"),
                                mic_had_audio: true,
                                sys_had_audio: true,
                                no_speech_prob: 0.01,
                                mic_rms: loudness * mic_share,
                                sys_rms: loudness * (1.0 - mic_share),
                                live_speaker: None,
                            }],
                            speech: None,
                            rejection: None,
                        },
                    )
                    .unwrap();
                chunk += 1;
            }
        }

        VaultHarness { vault, store, meeting_id }
    }

    #[test]
    fn the_reported_failure_is_fixed_end_to_end() {
        // Three real-shaped voices on a call taken over speakers: every
        // utterance registers both sources, which is why the shipped build
        // reported "1 spoke · Speaker 1 100%" for a meeting of three.
        let harness = realistic_vault(
            &[
                (fixtures::THREE_SPEAKERS[0], 0.80), // the local user
                (fixtures::THREE_SPEAKERS[1], 0.30),
                (fixtures::THREE_SPEAKERS[2], 0.25),
            ],
            3,
        );

        let diarization =
            diarize_session(&harness.store, &harness.meeting_id, None).expect("audio is present");

        assert_eq!(
            diarization.report.cluster_count, 3,
            "three people must come back as three speakers — silhouette {:.3}, between {:.3}",
            diarization.report.silhouette,
            diarization.report.min_between_distance
        );
        assert_eq!(
            diarization.report.local_cluster,
            Some(0),
            "the voice the microphone heard most is the person using the machine"
        );
        assert_eq!(diarization.report.unplaced_count, 0);
    }

    #[test]
    fn probe_vault_distances() {
        // Six turns each so the same-speaker population is worth reading.
        let harness = realistic_vault(
            &[
                (fixtures::THREE_SPEAKERS[0], 0.85),
                (fixtures::THREE_SPEAKERS[2], 0.20),
            ],
            6,
        );
        let segments = harness.store.get_transcript_segments(&harness.meeting_id).unwrap();
        let spans = collect_spans(&segments);
        let mut feats: Vec<(usize, crate::meetings_v2::diarize::features::VoiceFeatures)> =
            Vec::new();
        for span in &spans {
            let samples =
                read_chunk_samples(&harness.store.chunk_path(&harness.meeting_id, span.chunk_index))
                    .unwrap();
            let slice = slice_for(&samples, span).unwrap();
            let f = crate::meetings_v2::diarize::features::extract(slice, 16_000).unwrap();
            // Voices alternate chunk by chunk in `realistic_vault`.
            feats.push((span.chunk_index % 2, f));
        }

        let mut same = Vec::new();
        let mut diff = Vec::new();
        for i in 0..feats.len() {
            for j in i + 1..feats.len() {
                let d = crate::meetings_v2::diarize::cluster::feature_distance(
                    &feats[i].1,
                    &feats[j].1,
                );
                if feats[i].0 == feats[j].0 { same.push(d) } else { diff.push(d) }
            }
        }
        let mx = |v: &Vec<f32>| v.iter().cloned().fold(0.0f32, f32::max);
        let mn = |v: &Vec<f32>| v.iter().cloned().fold(f32::MAX, f32::min);
        println!(
            "VAULT PATH: same [{:.3}..{:.3}]  different [{:.3}..{:.3}]",
            mn(&same), mx(&same), mn(&diff), mx(&diff)
        );
    }

    #[test]
    fn probe_vault_clustering() {
        let harness = realistic_vault(
            &[
                (fixtures::THREE_SPEAKERS[0], 0.85),
                (fixtures::THREE_SPEAKERS[2], 0.20),
            ],
            3,
        );
        let d = diarize_session(&harness.store, &harness.meeting_id, None).unwrap();
        println!(
            "clusters={} silhouette={:.4} within={:.4} between={:.4}",
            d.report.cluster_count,
            d.report.silhouette,
            d.report.mean_within_distance,
            d.report.min_between_distance
        );
        for a in &d.assignments {
            println!("  {} -> {:?} d={:.4}", a.segment_id, a.cluster, a.distance);
        }
    }

    #[test]
    fn a_two_person_call_over_speakers_is_separated() {
        let harness = realistic_vault(
            &[
                (fixtures::THREE_SPEAKERS[0], 0.85),
                (fixtures::THREE_SPEAKERS[2], 0.20),
            ],
            3,
        );
        let diarization = diarize_session(&harness.store, &harness.meeting_id, None).unwrap();
        assert_eq!(diarization.report.cluster_count, 2);
        assert_eq!(diarization.report.local_cluster, Some(0));
    }

    #[test]
    fn an_in_person_meeting_claims_no_local_user() {
        // One microphone carrying everybody. Every voice has the same share, so
        // naming one of them "Me" would mislabel whoever it landed on.
        let harness = realistic_vault(
            &[
                (fixtures::THREE_SPEAKERS[0], 0.95),
                (fixtures::THREE_SPEAKERS[1], 0.95),
                (fixtures::THREE_SPEAKERS[2], 0.95),
            ],
            3,
        );

        let automatic = diarize_session(&harness.store, &harness.meeting_id, None).unwrap();
        assert_eq!(
            automatic.report.local_cluster, None,
            "no voice stands out, so none may be claimed as the user's"
        );

        let marked =
            diarize_session_with(&harness.store, &harness.meeting_id, None, true).unwrap();
        assert_eq!(marked.report.local_cluster, None);
        assert_eq!(marked.report.cluster_count, 3, "the voices still separate");
    }

    #[test]
    fn one_voice_on_disk_comes_back_as_one_cluster() {
        let harness = VaultHarness::new(&[
            (130.0, 1.0),
            (131.0, 1.01),
            (129.0, 0.99),
            (132.0, 1.02),
        ]);
        let diarization =
            diarize_session(&harness.store, &harness.meeting_id, None).expect("audio is present");
        assert_eq!(
            diarization.report.cluster_count, 1,
            "one person must not become a roster (within {} between {})",
            diarization.report.mean_within_distance, diarization.report.min_between_distance
        );
    }

    #[test]
    fn a_meeting_whose_audio_is_gone_says_so_rather_than_failing_silently() {
        let vault =
            std::env::temp_dir().join(format!("relay_test_diarize_{}", uuid::Uuid::new_v4()));
        let store = SessionStore::new(vault.clone());
        let session = MeetingSession::new("meet_no_audio".to_string(), None);
        store.init_session(&session).unwrap();

        let err = diarize_session(&store, "meet_no_audio", None).unwrap_err();
        assert!(err.contains("audio"), "unhelpful message: {err}");
        assert!(
            err.contains("unaffected"),
            "the user must be told what still works: {err}"
        );
        let _ = std::fs::remove_dir_all(&vault);
    }

    #[test]
    fn a_hint_of_two_merges_three_voices_into_two() {
        let harness = VaultHarness::new(&[
            (105.0, 1.0),
            (230.0, 1.55),
            (160.0, 1.25),
            (107.0, 1.02),
            (228.0, 1.53),
            (162.0, 1.24),
        ]);
        let diarization = diarize_session(&harness.store, &harness.meeting_id, Some(2))
            .expect("audio is present");
        assert_eq!(diarization.report.cluster_count, 2);
        assert_eq!(diarization.report.expected_speakers, Some(2));
    }

    #[test]
    fn diarization_never_writes_to_the_transcript() {
        let harness = VaultHarness::new(&[(105.0, 1.0), (230.0, 1.55)]);
        let path = harness
            .store
            .session_dir(&harness.meeting_id)
            .join("transcript.jsonl");
        let before = std::fs::read(&path).unwrap();

        diarize_session(&harness.store, &harness.meeting_id, None).unwrap();

        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "attribution is a separate layer; the raw transcript is immutable"
        );
    }


    #[test]
    fn a_diarization_maps_segment_ids_to_clusters() {
        let diarization = Diarization {
            report: DiarizationReport {
                cluster_count: 2,
                placed_count: 2,
                unplaced_count: 1,
                skipped_count: 0,
                local_cluster: Some(0),
                well_separated: true,
                mean_within_distance: 0.1,
                min_between_distance: 0.8,
                singleton_speaker_count: 0,
                silhouette: 0.82,
                expected_speakers: None,
                duration_ms: 12,
                embedding_provider: None,
                fallback_used: false,
                embedding_duration_ms: 0,
            },
            assignments: vec![
                VoiceAssignment {
                    segment_id: "seg_00000_000".into(),
                    cluster: Some(0),
                    distance: 0.05,
                },
                VoiceAssignment {
                    segment_id: "seg_00000_001".into(),
                    cluster: Some(1),
                    distance: 0.11,
                },
                VoiceAssignment {
                    segment_id: "seg_00001_000".into(),
                    cluster: None,
                    distance: 0.0,
                },
            ],
            self_voice_anchor: None,
        };

        assert_eq!(diarization.cluster_for("seg_00000_001"), Some(1));
        assert_eq!(diarization.cluster_for("seg_00001_000"), None);
        assert_eq!(diarization.cluster_for("nonexistent"), None);
        assert_eq!(diarization.cluster_map().len(), 2);
    }
}
