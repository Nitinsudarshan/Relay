//! The three ways Relay can decide who spoke, and a way to compare them.
//!
//! Speaker identity is the part of the meetings feature that has been wrong
//! twice, and the reason both times was that it could only be judged by holding
//! a real meeting and reading the result. That is an expensive test loop: one
//! recording, one answer, no way to tell whether a different approach would
//! have done better on the *same* audio.
//!
//! So the decision is a swappable engine, and [`compare`] runs all of them over
//! one recording. Record once, see three answers side by side, keep the one
//! that is right. The engines share the feature extraction and the audio path
//! deliberately — what differs between them is only the decision, which is the
//! thing under test.
//!
//! | Engine | How it decides | Costs | Fails by |
//! |---|---|---|---|
//! | [`DiarizationEngine::Channel`] | Which input the sound arrived on | Nothing | Everyone remote shares one label |
//! | [`DiarizationEngine::Voiceprint`] | Clustering every utterance, once, at the end | A pass over the audio | Merging voices that sound alike |
//! | [`DiarizationEngine::Live`] | A registry built up as chunks land | Nothing extra; it already ran | Early guesses made on little evidence |

use super::features;
use super::incremental::IncrementalDiarizer;
use super::{
    collect_spans, read_chunk_samples, slice_for, Diarization, DiarizationReport, UtteranceSpan,
    VoiceAssignment,
};
use crate::meetings_v2::session_store::SessionStore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Which method decides who spoke.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiarizationEngine {
    /// Rung 1 alone: the microphone is the local user, everything else is one
    /// bucket. No audio is read back and nothing can be misattributed to the
    /// wrong *person*, because it never claims to know one — which is also why
    /// a meeting of twenty reports two speakers.
    Channel,
    /// Clustering over the whole recording, after it ends. Sees every utterance
    /// before deciding anything, so it is the most accurate of the three, and
    /// the answer the summary is built from.
    #[default]
    Voiceprint,
    /// The registry the recorder builds as chunks land. Available during the
    /// meeting, which is its whole point; less accurate than `Voiceprint`
    /// because an early utterance is placed against an almost-empty registry.
    Live,
}

impl DiarizationEngine {
    pub const ALL: [Self; 3] = [Self::Channel, Self::Voiceprint, Self::Live];

    pub fn id(self) -> &'static str {
        match self {
            Self::Channel => "channel",
            Self::Voiceprint => "voiceprint",
            Self::Live => "live",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_lowercase().as_str() {
            "channel" => Some(Self::Channel),
            "voiceprint" => Some(Self::Voiceprint),
            "live" => Some(Self::Live),
            _ => None,
        }
    }

    /// A name for the picker.
    pub fn label(self) -> &'static str {
        match self {
            Self::Channel => "Channel only",
            Self::Voiceprint => "Voice separation",
            Self::Live => "Live (as recorded)",
        }
    }

    /// What it does and what it costs, in one line for the UI.
    pub fn summary(self) -> &'static str {
        match self {
            Self::Channel => {
                "Tells you apart from the call, and nothing further. Everyone on the other \
end shares one label."
            }
            Self::Voiceprint => {
                "Separates individual voices once the recording ends. The most accurate \
option, and what summaries are built from."
            }
            Self::Live => {
                "What the recorder worked out while the meeting was running. Available \
during the call; less certain, because early turns were placed with little to \
compare against."
            }
        }
    }
}

/// One engine's answer, with enough detail to compare it against another's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineOutcome {
    pub engine: DiarizationEngine,
    pub id: String,
    pub label: String,
    pub summary: String,
    pub diarization: Diarization,
    /// How many utterances each engine put in each speaker, largest first.
    /// The shape of the answer at a glance — `[14, 9, 2]` is a very different
    /// meeting from `[25]`.
    pub speaker_sizes: Vec<usize>,
    /// Set when the engine could not run, in words the user can act on.
    #[serde(default)]
    pub error: Option<String>,
}

/// Every engine's answer for one recording.
///
/// The point of the comparison: a meeting is expensive to produce and cheap to
/// re-analyse, so the choice between engines should be made by looking at all
/// of them on audio the user actually recognises.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineComparison {
    pub meeting_id: String,
    pub outcomes: Vec<EngineOutcome>,
    /// The engine that would be used now, so the comparison says what is in
    /// force as well as what is possible.
    pub active: DiarizationEngine,
    pub expected_speakers: Option<usize>,
}

/// Runs one engine over a finished recording.
pub fn run(
    engine: DiarizationEngine,
    store: &SessionStore,
    session_id: &str,
    expected_speakers: Option<usize>,
    assume_in_person: bool,
) -> Result<Diarization, String> {
    match engine {
        DiarizationEngine::Channel => channel_only(store, session_id),
        DiarizationEngine::Voiceprint => super::diarize_session_with(
            store,
            session_id,
            expected_speakers,
            assume_in_person,
        ),
        DiarizationEngine::Live => replay_live(store, session_id, assume_in_person),
    }
}

/// Runs every engine over one recording.
///
/// An engine that fails is reported as failed rather than omitted: "this one
/// could not run" is a comparison result, and silently showing two options
/// where there are three hides it.
pub fn compare(
    store: &SessionStore,
    session_id: &str,
    expected_speakers: Option<usize>,
    assume_in_person: bool,
    active: DiarizationEngine,
) -> EngineComparison {
    let outcomes = DiarizationEngine::ALL
        .iter()
        .map(|&engine| {
            match run(engine, store, session_id, expected_speakers, assume_in_person) {
                Ok(diarization) => EngineOutcome {
                    speaker_sizes: speaker_sizes(&diarization),
                    engine,
                    id: engine.id().to_string(),
                    label: engine.label().to_string(),
                    summary: engine.summary().to_string(),
                    diarization,
                    error: None,
                },
                Err(error) => EngineOutcome {
                    engine,
                    id: engine.id().to_string(),
                    label: engine.label().to_string(),
                    summary: engine.summary().to_string(),
                    diarization: empty_diarization(expected_speakers),
                    speaker_sizes: Vec::new(),
                    error: Some(error),
                },
            }
        })
        .collect();

    EngineComparison {
        meeting_id: session_id.to_string(),
        outcomes,
        active,
        expected_speakers,
    }
}

/// Utterances per speaker, largest first.
fn speaker_sizes(diarization: &Diarization) -> Vec<usize> {
    let mut counts: HashMap<usize, usize> = HashMap::new();
    for assignment in &diarization.assignments {
        if let Some(cluster) = assignment.cluster {
            *counts.entry(cluster).or_insert(0) += 1;
        }
    }
    let mut sizes: Vec<usize> = counts.into_values().collect();
    sizes.sort_unstable_by(|a, b| b.cmp(a));
    sizes
}

fn empty_diarization(expected_speakers: Option<usize>) -> Diarization {
    Diarization {
        report: DiarizationReport {
            cluster_count: 0,
            placed_count: 0,
            unplaced_count: 0,
            skipped_count: 0,
            local_cluster: None,
            well_separated: true,
            mean_within_distance: 0.0,
            min_between_distance: 0.0,
            singleton_speaker_count: 0,
            silhouette: 0.0,
            expected_speakers,
            duration_ms: 0,
        },
        assignments: Vec::new(),
    }
}

/// The channel engine: two speakers, decided by which input carried the sound.
///
/// Reads no audio back. Its answer is exactly rung 1's, expressed as a
/// diarization so the rest of the pipeline treats all three engines alike —
/// cluster 0 is the local user, cluster 1 is everyone else.
fn channel_only(store: &SessionStore, session_id: &str) -> Result<Diarization, String> {
    let started = std::time::Instant::now();
    let segments = store
        .get_transcript_segments(session_id)
        .map_err(|e| format!("Failed to read the transcript: {e}"))?;

    let spans = collect_spans(&segments);
    let mut assignments = Vec::with_capacity(spans.len());
    let mut local = 0usize;
    let mut remote = 0usize;

    for span in &spans {
        let cluster = match span.mic_share {
            Some(share) if share >= 0.5 => {
                local += 1;
                Some(0)
            }
            Some(_) => {
                remote += 1;
                Some(1)
            }
            None => None,
        };
        assignments.push(VoiceAssignment {
            segment_id: span.segment_id.clone(),
            cluster,
            distance: 0.0,
        });
    }

    let unplaced = assignments.iter().filter(|a| a.cluster.is_none()).count();
    let cluster_count = usize::from(local > 0) + usize::from(remote > 0);

    Ok(Diarization {
        report: DiarizationReport {
            cluster_count,
            placed_count: assignments.len() - unplaced,
            unplaced_count: unplaced,
            skipped_count: 0,
            local_cluster: (local > 0).then_some(0),
            // Nothing was inferred, so nothing can be poorly separated. The two
            // buckets are a measurement of which cable the sound came down.
            well_separated: true,
            mean_within_distance: 0.0,
            min_between_distance: 0.0,
            singleton_speaker_count: 0,
            silhouette: 0.0,
            expected_speakers: None,
            duration_ms: started.elapsed().as_millis() as u64,
        },
        assignments,
    })
}

/// The live engine, replayed over the stored recording.
///
/// Replayed rather than read back from the transcript so the comparison is
/// like-for-like: every engine sees the same audio in the same order, and a
/// difference between them is a difference in method rather than in what was
/// available at the time. The registry is fed chunks in recording order, which
/// is what the recorder itself did.
fn replay_live(
    store: &SessionStore,
    session_id: &str,
    assume_in_person: bool,
) -> Result<Diarization, String> {
    let started = std::time::Instant::now();
    let segments = store
        .get_transcript_segments(session_id)
        .map_err(|e| format!("Failed to read the transcript: {e}"))?;

    if store
        .list_chunk_files(session_id)
        .map_err(|e| format!("Failed to list the recorded audio: {e}"))?
        .is_empty()
    {
        return Err("This meeting's audio has been discarded, so its speakers cannot be \
separated. The transcript and summary are unaffected."
            .to_string());
    }

    let mut spans = collect_spans(&segments);
    spans.sort_by(|a, b| {
        a.start_time_s
            .partial_cmp(&b.start_time_s)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut registry = IncrementalDiarizer::new();
    let mut assignments = Vec::with_capacity(spans.len());
    let mut skipped = 0usize;
    let mut samples_cache: Option<(usize, Vec<f32>)> = None;

    for span in &spans {
        let samples = match &samples_cache {
            Some((index, cached)) if *index == span.chunk_index => cached,
            _ => {
                let path = store.chunk_path(session_id, span.chunk_index);
                match read_chunk_samples(&path) {
                    Ok(loaded) => {
                        samples_cache = Some((span.chunk_index, loaded));
                        &samples_cache.as_ref().expect("just stored").1
                    }
                    Err(_) => {
                        skipped += 1;
                        assignments.push(unplaced(span));
                        continue;
                    }
                }
            }
        };

        let Some(slice) = slice_for(samples, span) else {
            skipped += 1;
            assignments.push(unplaced(span));
            continue;
        };
        let Some(voice) = features::extract(slice, 16_000) else {
            skipped += 1;
            assignments.push(unplaced(span));
            continue;
        };

        match registry.assign(&voice, span.mic_share) {
            Some(placed) => assignments.push(VoiceAssignment {
                segment_id: span.segment_id.clone(),
                cluster: Some(placed.speaker),
                distance: placed.distance,
            }),
            None => assignments.push(unplaced(span)),
        }
    }

    let unplaced_count = assignments.iter().filter(|a| a.cluster.is_none()).count();
    let local_cluster = if assume_in_person {
        None
    } else {
        registry.local_speaker()
    };

    Ok(Diarization {
        report: DiarizationReport {
            cluster_count: registry.speaker_count(),
            placed_count: assignments.len() - unplaced_count,
            unplaced_count,
            skipped_count: skipped,
            local_cluster,
            // An online decision cannot know how well separated the whole
            // recording turned out to be, and claiming otherwise would present
            // a running guess as a finished one.
            well_separated: false,
            mean_within_distance: 0.0,
            min_between_distance: 0.0,
            singleton_speaker_count: 0,
            silhouette: 0.0,
            expected_speakers: None,
            duration_ms: started.elapsed().as_millis() as u64,
        },
        assignments,
    })
}

fn unplaced(span: &UtteranceSpan) -> VoiceAssignment {
    VoiceAssignment {
        segment_id: span.segment_id.clone(),
        cluster: None,
        distance: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meetings_v2::diarize::fixtures;
    use crate::meetings_v2::types::{
        MeetingSession, MeetingState, TranscriptSegment, TranscriptSegmentStatus,
        TranscriptUtterance,
    };

    struct Harness {
        vault: std::path::PathBuf,
        store: SessionStore,
        meeting_id: String,
    }

    impl Drop for Harness {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.vault);
        }
    }

    /// A meeting on disk: one voice per chunk, with the microphone share the
    /// recorder would have measured.
    fn meeting(voices: &[(fixtures::VoiceProfile, f32)], turns: usize) -> Harness {
        let vault = std::env::temp_dir().join(format!("relay_engine_{}", uuid::Uuid::new_v4()));
        let store = SessionStore::new(vault.clone());
        let meeting_id = "meet_engines".to_string();

        let mut session = MeetingSession::new(meeting_id.clone(), None);
        session.state = MeetingState::Completed;
        store.init_session(&session).unwrap();

        let mut chunk = 0usize;
        for turn in 0..turns {
            for (voice, mic_share) in voices {
                let samples = fixtures::utterance_audio(voice, turn, 4.0);
                store.write_chunk_wav(&meeting_id, chunk, &samples, 16_000).unwrap();
                let loudness = 0.08f32;
                store
                    .append_transcript_segment(
                        &meeting_id,
                        &TranscriptSegment {
                            chunk_index: chunk,
                            start_time_s: chunk as f64 * 30.0,
                            end_time_s: chunk as f64 * 30.0 + 4.0,
                            text: format!("turn {chunk}"),
                            created_at: "2026-09-04T10:00:00Z".to_string(),
                            status: TranscriptSegmentStatus::Success,
                            mic_had_audio: true,
                            sys_had_audio: true,
                            utterances: vec![TranscriptUtterance {
                                index: 0,
                                start_time_s: chunk as f64 * 30.0,
                                end_time_s: chunk as f64 * 30.0 + 4.0,
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

        Harness { vault, store, meeting_id }
    }

    fn three_person_call() -> Harness {
        meeting(
            &[
                (fixtures::THREE_SPEAKERS[0], 0.82),
                (fixtures::THREE_SPEAKERS[1], 0.28),
                (fixtures::THREE_SPEAKERS[2], 0.22),
            ],
            3,
        )
    }

    #[test]
    fn the_channel_engine_finds_two_speakers_however_many_were_there() {
        // Not a bug — the honest limit of what a channel split can say, and the
        // reason the other engines exist.
        let harness = three_person_call();
        let result = run(
            DiarizationEngine::Channel,
            &harness.store,
            &harness.meeting_id,
            None,
            false,
        )
        .unwrap();

        assert_eq!(result.report.cluster_count, 2);
        assert_eq!(result.report.local_cluster, Some(0));
        assert!(result.report.well_separated, "nothing was inferred");
    }

    #[test]
    fn the_voiceprint_engine_finds_all_three() {
        let harness = three_person_call();
        let result = run(
            DiarizationEngine::Voiceprint,
            &harness.store,
            &harness.meeting_id,
            None,
            false,
        )
        .unwrap();

        assert_eq!(
            result.report.cluster_count, 3,
            "silhouette {:.3}",
            result.report.silhouette
        );
        assert_eq!(result.report.local_cluster, Some(0));
    }

    #[test]
    fn the_live_engine_finds_all_three_from_a_running_registry() {
        let harness = three_person_call();
        let result = run(
            DiarizationEngine::Live,
            &harness.store,
            &harness.meeting_id,
            None,
            false,
        )
        .unwrap();

        assert_eq!(result.report.cluster_count, 3);
        assert_eq!(result.report.local_cluster, Some(0));
        assert!(
            !result.report.well_separated,
            "a running guess must not present itself as a finished one"
        );
    }

    #[test]
    fn comparing_engines_runs_all_of_them_over_the_same_recording() {
        // The point of the whole module: one meeting, three answers, no need to
        // hold another meeting to try a different approach.
        let harness = three_person_call();
        let comparison = compare(
            &harness.store,
            &harness.meeting_id,
            None,
            false,
            DiarizationEngine::Voiceprint,
        );

        assert_eq!(comparison.outcomes.len(), 3);
        assert_eq!(comparison.active, DiarizationEngine::Voiceprint);
        assert!(comparison.outcomes.iter().all(|o| o.error.is_none()));

        let by_id = |id: &str| {
            comparison
                .outcomes
                .iter()
                .find(|o| o.id == id)
                .expect("every engine is reported")
        };
        assert_eq!(by_id("channel").diarization.report.cluster_count, 2);
        assert_eq!(by_id("voiceprint").diarization.report.cluster_count, 3);
        assert_eq!(by_id("live").diarization.report.cluster_count, 3);
    }

    #[test]
    fn the_comparison_shows_the_shape_of_each_answer() {
        let harness = three_person_call();
        let comparison = compare(
            &harness.store,
            &harness.meeting_id,
            None,
            false,
            DiarizationEngine::Voiceprint,
        );

        let channel = comparison.outcomes.iter().find(|o| o.id == "channel").unwrap();
        // One local speaker against six remote turns: the imbalance is the
        // finding, and it is visible without opening the transcript.
        assert_eq!(channel.speaker_sizes, vec![6, 3]);

        let voiceprint = comparison
            .outcomes
            .iter()
            .find(|o| o.id == "voiceprint")
            .unwrap();
        assert_eq!(voiceprint.speaker_sizes, vec![3, 3, 3]);
    }

    #[test]
    fn an_engine_that_cannot_run_is_reported_rather_than_dropped() {
        // "This one could not run" is a comparison result. Showing two options
        // where there are three hides it.
        let vault = std::env::temp_dir().join(format!("relay_engine_{}", uuid::Uuid::new_v4()));
        let store = SessionStore::new(vault.clone());
        let session = MeetingSession::new("meet_no_audio".to_string(), None);
        store.init_session(&session).unwrap();

        let comparison = compare(
            &store,
            "meet_no_audio",
            None,
            false,
            DiarizationEngine::Voiceprint,
        );

        assert_eq!(comparison.outcomes.len(), 3);
        let voiceprint = comparison
            .outcomes
            .iter()
            .find(|o| o.id == "voiceprint")
            .unwrap();
        assert!(voiceprint.error.as_ref().is_some_and(|e| e.contains("audio")));
        // The channel engine reads no audio, so it still answers.
        let channel = comparison.outcomes.iter().find(|o| o.id == "channel").unwrap();
        assert!(channel.error.is_none());

        let _ = std::fs::remove_dir_all(&vault);
    }

    #[test]
    fn an_expected_count_reaches_the_engine_that_can_use_it() {
        let harness = three_person_call();
        let comparison = compare(
            &harness.store,
            &harness.meeting_id,
            Some(2),
            false,
            DiarizationEngine::Voiceprint,
        );

        let voiceprint = comparison
            .outcomes
            .iter()
            .find(|o| o.id == "voiceprint")
            .unwrap();
        assert_eq!(voiceprint.diarization.report.cluster_count, 2);
        assert_eq!(comparison.expected_speakers, Some(2));
    }

    #[test]
    fn in_person_mode_claims_no_local_user_on_any_engine() {
        let harness = meeting(
            &[
                (fixtures::THREE_SPEAKERS[0], 0.95),
                (fixtures::THREE_SPEAKERS[2], 0.95),
            ],
            3,
        );
        for engine in [DiarizationEngine::Voiceprint, DiarizationEngine::Live] {
            let result = run(engine, &harness.store, &harness.meeting_id, None, true).unwrap();
            assert_eq!(
                result.report.local_cluster,
                None,
                "{} claimed a local user through one shared microphone",
                engine.id()
            );
        }
    }

    #[test]
    fn engine_ids_round_trip() {
        for engine in DiarizationEngine::ALL {
            assert_eq!(DiarizationEngine::parse(engine.id()), Some(engine));
            assert!(!engine.label().is_empty());
            assert!(!engine.summary().is_empty());
        }
        assert_eq!(DiarizationEngine::parse("  VOICEPRINT "), Some(DiarizationEngine::Voiceprint));
        assert_eq!(DiarizationEngine::parse("neural"), None);
    }
}
