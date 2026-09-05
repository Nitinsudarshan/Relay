//! Speaker attribution and the speaker registry.
//!
//! Two rungs of the ladder in `Meeting-rules/meeting_speaker_identification.md`
//! are implemented, and they compose rather than compete:
//!
//! * **Rung 1 — channel.** Microphone input is the local user, system audio is
//!   everyone else. Free, certain for "me", and always on. On its own it can
//!   only ever produce two speakers, which is why a 44-minute meeting with
//!   twenty people in it used to show one chip reading "Speaker 1".
//! * **Rung 4 — diarization.** `meetings_v2::diarize` clusters the recorded
//!   audio into distinct voices. Where a cluster covers the remote side, it
//!   splits that single anonymous bucket into `Speaker 1`, `Speaker 2`, …
//!
//! Rung 1 wins wherever the two disagree about the *local* user, because the
//! channel is direct evidence and a cluster is an inference. Everywhere else
//! diarization refines what the channel could only bucket.
//!
//! Three invariants hold everything else together:
//!
//! 1. **Ids are never display names.** `speaker_1` is the identifier;
//!    "Pranjali" is a label the user can change at any time. Renaming touches
//!    the registry and nothing else, so no transcript is ever rewritten.
//! 2. **Ambiguity is preserved, not resolved.** Where neither the channel nor a
//!    cluster can say who spoke, the segment keeps `speaker_id = None`.
//! 3. **A cluster is never presented as a certainty.** `SpeakerOrigin` records
//!    which rung found each speaker, so the UI can distinguish a channel fact
//!    from an acoustic inference from a name a person typed.

use super::model::{
    NormalizedSegment, SegmentChannel, Speaker, SpeakerOrigin, SPEAKER_ID_ME, SPEAKER_ID_REMOTE,
};
use crate::calendar::CalendarAttendee;
use crate::meetings_v2::diarize::self_voice::SelfVoiceAnchor;
use crate::meetings_v2::diarize::Diarization;
use crate::meetings_v2::types::{
    SpeakerAssignment, SpeakerAssignmentMethod, SpeakerCandidateScore, SpeakerConfidenceLevel,
    SpeakerEvidence,
};
use std::collections::HashMap;

/// Whether speaker attribution should run at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeakerIdentificationMode {
    /// Channel-based attribution (rung 1).
    Automatic,
    /// No attribution; every turn is unattributed.
    Off,
}

/// The stable id for a diarization cluster that is not the local user.
///
/// Cluster 0 maps to [`SPEAKER_ID_REMOTE`] (`speaker_1`) so a meeting that was
/// attributed by channel alone, and is then diarized, keeps its first remote
/// speaker's id — and therefore any name the user already gave them.
pub fn remote_speaker_id(index: usize) -> String {
    if index == 0 {
        SPEAKER_ID_REMOTE.to_string()
    } else {
        format!("speaker_{}", index + 1)
    }
}

/// Input parameters for multi-evidence speaker attribution.
pub struct AttributionInput<'a> {
    pub existing: &'a [Speaker],
    pub mode: SpeakerIdentificationMode,
    pub diarization: Option<&'a Diarization>,
    pub self_voice: Option<&'a SelfVoiceAnchor>,
    pub calendar_attendees: &'a [CalendarAttendee],
    pub assume_in_person: bool,
}

impl<'a> AttributionInput<'a> {
    pub fn new(existing: &'a [Speaker], mode: SpeakerIdentificationMode) -> Self {
        Self {
            existing,
            mode,
            diarization: None,
            self_voice: None,
            calendar_attendees: &[],
            assume_in_person: false,
        }
    }
}

/// Assigns `speaker_id` on each segment from its channel, and returns the
/// registry of speakers that actually contributed.
///
/// `existing` carries user-assigned display names forward across regeneration —
/// re-running the pipeline must never silently discard a rename.
pub fn attribute_speakers(
    segments: &mut [NormalizedSegment],
    existing: &[Speaker],
    mode: SpeakerIdentificationMode,
) -> Vec<Speaker> {
    attribute_speakers_with_voices(segments, existing, mode, None)
}

/// Assigns speakers using the channel and, when one is supplied, a diarization
/// run. Backward-compatible wrapper around `attribute_speakers_with_evidence`.
pub fn attribute_speakers_with_voices(
    segments: &mut [NormalizedSegment],
    existing: &[Speaker],
    mode: SpeakerIdentificationMode,
    diarization: Option<&Diarization>,
) -> Vec<Speaker> {
    let input = AttributionInput {
        existing,
        mode,
        diarization,
        self_voice: None,
        calendar_attendees: &[],
        assume_in_person: false,
    };
    attribute_speakers_with_evidence(segments, input).0
}

/// Attribute speakers using all available evidence: channel, diarization clusters,
/// meeting-local self-voice anchor, calendar candidates, and in-person constraints.
/// Returns both the updated speaker registry and structured speaker assignments per utterance.
pub fn attribute_speakers_with_evidence(
    segments: &mut [NormalizedSegment],
    input: AttributionInput<'_>,
) -> (Vec<Speaker>, Vec<SpeakerAssignment>) {
    if input.mode == SpeakerIdentificationMode::Off {
        for segment in segments.iter_mut() {
            segment.speaker_id = None;
        }
        return (input.existing.to_vec(), Vec::new());
    }

    let clusters = input
        .diarization
        .filter(|d| d.report.cluster_count > 0)
        .map(|d| d.cluster_map())
        .unwrap_or_default();

    // If assume_in_person is set, room mic audio must not be treated as proof of local user "Me".
    let local_clusters = if input.assume_in_person {
        Vec::new()
    } else {
        input
            .diarization
            .and_then(|d| d.report.local_cluster)
            .map(|index| vec![index])
            .unwrap_or_else(|| local_user_clusters(segments, &clusters))
    };

    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut assignments: Vec<SpeakerAssignment> = Vec::with_capacity(segments.len());

    for i in 0..segments.len() {
        let cluster = clusters.get(segments[i].id.as_str()).copied();
        let prev_speaker = if i > 0 {
            segments[i - 1].speaker_id.as_deref()
        } else {
            None
        };
        let next_speaker = if i + 1 < segments.len() {
            let next_cluster = clusters.get(segments[i + 1].id.as_str()).copied();
            if segments[i + 1].channel == SegmentChannel::Mic && !input.assume_in_person {
                Some(SPEAKER_ID_ME)
            } else if let Some(nc) = next_cluster {
                if local_clusters.contains(&nc) {
                    Some(SPEAKER_ID_ME)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let per_seg_sim = input
            .diarization
            .and_then(|d| d.self_voice_similarity_for(segments[i].id.as_str()));

        let (resolved, method, confidence, confidence_level, evidence) = resolve_segment_with_evidence(
            segments[i].channel,
            cluster,
            &local_clusters,
            input.assume_in_person,
            input.self_voice,
            per_seg_sim,
            &segments[i].text,
            input.calendar_attendees,
            prev_speaker,
            next_speaker,
        );

        if let Some(ref id) = resolved {
            *counts.entry(id.clone()).or_insert(0) += 1;
            assignments.push(SpeakerAssignment {
                utterance_id: segments[i].id.clone(),
                speaker_id: id.clone(),
                confidence,
                confidence_level,
                method,
                evidence,
            });
        }
        segments[i].speaker_id = resolved;
    }

    let speakers = build_roster(
        &counts,
        &local_clusters,
        &clusters,
        input.existing,
        input.diarization,
    );

    (speakers, assignments)
}

const SHORT_INTERJECTIONS: &[&str] = &[
    "yes", "no", "okay", "ok", "haan", "yeah", "yep", "right", "hmm", "sure", "nah", "nahi", "accha", "theek",
];

fn is_short_interjection(text: &str) -> bool {
    let words: Vec<&str> = text
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphabetic()))
        .filter(|w| !w.is_empty())
        .collect();
    if words.is_empty() || words.len() > 2 {
        return false;
    }
    words.iter().all(|w| SHORT_INTERJECTIONS.contains(&w.to_lowercase().as_str()))
}

/// Resolves a segment into speaker id, assignment method, confidence, confidence level, and evidence.
#[allow(clippy::too_many_arguments)]
fn resolve_segment_with_evidence(
    channel: SegmentChannel,
    cluster: Option<usize>,
    local_clusters: &[usize],
    assume_in_person: bool,
    self_voice: Option<&SelfVoiceAnchor>,
    self_voice_sim: Option<f32>,
    text: &str,
    calendar_attendees: &[CalendarAttendee],
    prev_speaker: Option<&str>,
    next_speaker: Option<&str>,
) -> (
    Option<String>,
    SpeakerAssignmentMethod,
    f32,
    Option<SpeakerConfidenceLevel>,
    SpeakerEvidence,
) {
    let lower_text = text.to_lowercase();
    let calendar_names: Vec<String> = calendar_attendees
        .iter()
        .map(|a| {
            if !a.name.is_empty() {
                a.name.clone()
            } else {
                a.email.clone().unwrap_or_default()
            }
        })
        .collect();
    let calendar_str = if calendar_names.is_empty() {
        None
    } else {
        Some(calendar_names.join(", "))
    };

    // Short interjections ("yes", "no", "okay", "haan"):
    // Acoustic features for a short isolated token are insufficient for reliable clustering.
    // If channel is mixed or unknown, do not guess a remote speaker cluster.
    if is_short_interjection(text) && matches!(channel, SegmentChannel::Mixed | SegmentChannel::Unknown) {
        let is_self_match = if let (Some(anchor), Some(c)) = (self_voice, cluster) {
            anchor.has_samples() && local_clusters.contains(&c)
        } else {
            false
        };

        if !is_self_match {
            return (
                None,
                SpeakerAssignmentMethod::Unknown,
                0.0,
                Some(SpeakerConfidenceLevel::Unknown),
                SpeakerEvidence {
                    channel: Some(channel.as_str().to_string()),
                    cluster_id: cluster,
                    similarity: None,
                    notes: Some("Short interjection with insufficient channel/acoustic evidence left unresolved".to_string()),
                    calendar_candidate: calendar_str,
                    contextual_mention: None,
                    temporal_consistency: None,
                    candidate_scores: Vec::new(),
                },
            );
        }
    }

    // 1. In-person meeting: Room mic is NOT assumed to be local user
    if assume_in_person {
        if let Some(c) = cluster {
            let spk_id = format!("speaker_{}", c + 1);
            let mut candidate_scores = Vec::new();
            candidate_scores.push(SpeakerCandidateScore {
                speaker_id: spk_id.clone(),
                acoustic_similarity: None,
                cluster_consistency: 0.85,
                channel_evidence: 0.0,
                contextual_evidence: 0.0,
                calendar_evidence: 0.0,
                temporal_consistency: if prev_speaker == Some(&spk_id) { 0.10 } else { 0.0 },
                contradiction_penalty: 0.0,
                final_confidence: 0.75,
            });

            // Contradiction check: Room mic cannot be "Me"
            candidate_scores.push(SpeakerCandidateScore {
                speaker_id: SPEAKER_ID_ME.to_string(),
                acoustic_similarity: None,
                cluster_consistency: 0.0,
                channel_evidence: 0.0,
                contextual_evidence: 0.0,
                calendar_evidence: 0.0,
                temporal_consistency: 0.0,
                contradiction_penalty: 1.0,
                final_confidence: 0.0,
            });

            let evidence = SpeakerEvidence {
                channel: Some("room_mic".to_string()),
                cluster_id: Some(c),
                similarity: None,
                notes: Some("In-person diarization cluster".to_string()),
                calendar_candidate: calendar_str,
                contextual_mention: None,
                temporal_consistency: if prev_speaker == Some(&spk_id) {
                    Some("Turn continuity".to_string())
                } else if next_speaker == prev_speaker && prev_speaker.is_some() {
                    Some("Conversational interruption preserved".to_string())
                } else {
                    Some("Speaker transition".to_string())
                },
                candidate_scores,
            };
            return (
                Some(spk_id),
                SpeakerAssignmentMethod::Diarization,
                0.75,
                Some(SpeakerConfidenceLevel::Likely),
                evidence,
            );
        }

        // Without cluster in-person, audio is unattributed to prevent false certainty
        return (
            None,
            SpeakerAssignmentMethod::Channel,
            0.0,
            Some(SpeakerConfidenceLevel::Unknown),
            SpeakerEvidence {
                channel: Some("room_mic".to_string()),
                cluster_id: None,
                similarity: None,
                notes: Some("In-person room mic without distinct cluster".to_string()),
                calendar_candidate: calendar_str,
                contextual_mention: None,
                temporal_consistency: None,
                candidate_scores: Vec::new(),
            },
        );
    }

    // 2. Multi-Signal Evidence Fusion: Channel + Diarization Cluster + Self-Voice Anchor + Temporal
    let is_local_cluster = cluster.map(|c| local_clusters.contains(&c)).unwrap_or(false);
    let remote_spk_id = cluster
        .filter(|&c| !local_clusters.contains(&c))
        .map(|c| remote_speaker_id(remote_index(c, local_clusters)))
        .unwrap_or_else(|| "speaker_1".to_string());

    // Acoustic similarity from per-segment anchor match or cluster/fallback
    let (me_acoustic_sim, remote_acoustic_sim) = match self_voice_sim {
        Some(sim) => (Some(sim), Some((1.0 - sim).clamp(0.0, 1.0))),
        None => match (self_voice, cluster) {
            (Some(anchor), Some(_)) if anchor.has_samples() => {
                if is_local_cluster {
                    (Some(0.90f32), Some(0.15f32))
                } else {
                    (Some(0.20f32), Some(0.85f32))
                }
            }
            (Some(anchor), None) if anchor.has_samples() => {
                if channel == SegmentChannel::Mic {
                    (Some(0.75f32), Some(0.25f32))
                } else if channel == SegmentChannel::System {
                    (Some(0.20f32), Some(0.80f32))
                } else {
                    (Some(0.50f32), Some(0.50f32))
                }
            }
            _ => (None, None),
        },
    };

    // Candidate Me scoring
    let me_channel_ev = match channel {
        SegmentChannel::Mic => 0.85f32,
        SegmentChannel::Mixed => 0.40f32,
        SegmentChannel::System => 0.10f32,
        SegmentChannel::Unknown => 0.20f32,
    };
    let me_cluster_ev = if is_local_cluster {
        0.85f32
    } else if me_acoustic_sim.is_some_and(|s| s >= 0.55) {
        0.80f32
    } else if cluster.is_some() {
        0.15f32
    } else {
        0.40f32
    };
    let me_temporal = if prev_speaker == Some(SPEAKER_ID_ME) { 0.10f32 } else { 0.0f32 };

    // Contradiction penalties for Me:
    // If mic channel is claimed, but self-voice or cluster says remote (acoustic leakage / remote speech):
    let me_contradiction = if (channel == SegmentChannel::Mic && me_acoustic_sim.is_some_and(|s| s < 0.40))
        || (channel == SegmentChannel::System && !is_local_cluster)
    {
        0.60f32
    } else {
        0.0f32
    };

    let me_confidence = (0.35 * me_channel_ev
        + 0.35 * me_cluster_ev
        + 0.30 * me_acoustic_sim.unwrap_or(me_channel_ev)
        + me_temporal
        - me_contradiction)
        .clamp(0.0, 1.0);

    // Candidate Remote scoring
    let remote_channel_ev = match channel {
        SegmentChannel::System => 0.85f32,
        SegmentChannel::Mixed => 0.40f32,
        SegmentChannel::Mic => 0.10f32,
        SegmentChannel::Unknown => 0.40f32,
    };
    let remote_cluster_ev = if !is_local_cluster && cluster.is_some() {
        if channel == SegmentChannel::System {
            0.85f32
        } else if me_acoustic_sim.is_some_and(|s| s >= 0.55) {
            0.15f32
        } else {
            0.85f32
        }
    } else if is_local_cluster {
        0.15f32
    } else {
        0.40f32
    };
    let remote_temporal = if prev_speaker == Some(&remote_spk_id) { 0.10f32 } else { 0.0f32 };
    let remote_contradiction = if (is_local_cluster || me_acoustic_sim.is_some_and(|s| s >= 0.55))
        && matches!(channel, SegmentChannel::Mic | SegmentChannel::Mixed)
    {
        0.60f32
    } else {
        0.0f32
    };

    let remote_confidence = (0.35 * remote_channel_ev
        + 0.35 * remote_cluster_ev
        + 0.30 * remote_acoustic_sim.unwrap_or(remote_channel_ev)
        + remote_temporal
        - remote_contradiction)
        .clamp(0.0, 1.0);

    let mut candidate_scores = Vec::new();
    candidate_scores.push(SpeakerCandidateScore {
        speaker_id: SPEAKER_ID_ME.to_string(),
        acoustic_similarity: me_acoustic_sim,
        cluster_consistency: me_cluster_ev,
        channel_evidence: me_channel_ev,
        contextual_evidence: 0.0,
        calendar_evidence: 0.0,
        temporal_consistency: me_temporal,
        contradiction_penalty: me_contradiction,
        final_confidence: me_confidence,
    });
    candidate_scores.push(SpeakerCandidateScore {
        speaker_id: remote_spk_id.clone(),
        acoustic_similarity: remote_acoustic_sim,
        cluster_consistency: remote_cluster_ev,
        channel_evidence: remote_channel_ev,
        contextual_evidence: 0.0,
        calendar_evidence: 0.0,
        temporal_consistency: remote_temporal,
        contradiction_penalty: remote_contradiction,
        final_confidence: remote_confidence,
    });

    // Calendar candidate notes if mentioned
    for cal_name in &calendar_names {
        if lower_text.contains(&cal_name.to_lowercase()) {
            candidate_scores.push(SpeakerCandidateScore {
                speaker_id: cal_name.clone(),
                acoustic_similarity: None,
                cluster_consistency: 0.30,
                channel_evidence: 0.0,
                contextual_evidence: 0.30,
                calendar_evidence: 0.20,
                temporal_consistency: 0.0,
                contradiction_penalty: 0.0,
                final_confidence: 0.25,
            });
        }
    }

    let evidence = SpeakerEvidence {
        channel: Some(channel.as_str().to_string()),
        cluster_id: cluster,
        similarity: me_acoustic_sim,
        notes: Some("Fused multi-signal evidence attribution".to_string()),
        calendar_candidate: calendar_str,
        contextual_mention: None,
        temporal_consistency: if prev_speaker == Some(SPEAKER_ID_ME) || prev_speaker == Some(&remote_spk_id) {
            Some("Turn continuity".to_string())
        } else if next_speaker == prev_speaker && prev_speaker.is_some() {
            Some("Conversational interruption preserved".to_string())
        } else {
            Some("Turn transition".to_string())
        },
        candidate_scores,
    };

    // Margin-based decision
    let margin = 0.15f32;
    if me_confidence >= 0.50 && me_confidence - remote_confidence >= margin {
        let method = if me_acoustic_sim.is_some_and(|s| s >= 0.80) {
            SpeakerAssignmentMethod::SelfVoiceAnchor
        } else if cluster.is_some() {
            SpeakerAssignmentMethod::Diarization
        } else {
            SpeakerAssignmentMethod::Channel
        };
        let level = if cluster.is_none() && me_acoustic_sim.is_none() {
            SpeakerConfidenceLevel::Unresolved
        } else if me_confidence >= 0.85 {
            SpeakerConfidenceLevel::High
        } else {
            SpeakerConfidenceLevel::Likely
        };
        (Some(SPEAKER_ID_ME.to_string()), method, me_confidence, Some(level), evidence)
    } else if remote_confidence >= 0.50 && remote_confidence - me_confidence >= margin {
        let method = if cluster.is_some() {
            SpeakerAssignmentMethod::Diarization
        } else {
            SpeakerAssignmentMethod::Channel
        };
        let level = if cluster.is_none() {
            SpeakerConfidenceLevel::Unresolved
        } else if remote_confidence >= 0.85 {
            SpeakerConfidenceLevel::High
        } else {
            SpeakerConfidenceLevel::Likely
        };
        (Some(remote_spk_id), method, remote_confidence, Some(level), evidence)
    } else {
        // Ambiguous / tie: abstain from confident wrong attribution
        (
            None,
            SpeakerAssignmentMethod::Channel,
            me_confidence.max(remote_confidence),
            Some(SpeakerConfidenceLevel::Unresolved),
            evidence,
        )
    }
}

/// Merges `source_speaker_id` into `target_speaker_id`.
///
/// All segments and assignments pointing to `source_speaker_id` are remapped to
/// `target_speaker_id`. The raw transcript is never modified.
pub fn merge_speakers(
    speakers: &mut Vec<Speaker>,
    segments: &mut [NormalizedSegment],
    assignments: &mut [SpeakerAssignment],
    source_speaker_id: &str,
    target_speaker_id: &str,
    new_display_name: Option<&str>,
) -> Result<(), String> {
    if source_speaker_id == target_speaker_id {
        return Ok(());
    }

    let source_idx = speakers
        .iter()
        .position(|s| s.id == source_speaker_id)
        .ok_or_else(|| format!("Source speaker '{}' not found in roster", source_speaker_id))?;
    let target_idx = speakers
        .iter()
        .position(|s| s.id == target_speaker_id)
        .ok_or_else(|| format!("Target speaker '{}' not found in roster", target_speaker_id))?;

    let source_count = speakers[source_idx].segment_count;
    speakers[target_idx].segment_count += source_count;
    speakers[target_idx].origin = SpeakerOrigin::Manual;
    if let Some(name) = new_display_name.filter(|n| !n.trim().is_empty()) {
        speakers[target_idx].display_name = Some(name.to_string());
    }

    // Remap segments
    for segment in segments.iter_mut() {
        if segment.speaker_id.as_deref() == Some(source_speaker_id) {
            segment.speaker_id = Some(target_speaker_id.to_string());
        }
    }

    // Remap assignments
    for assignment in assignments.iter_mut() {
        if assignment.speaker_id == source_speaker_id {
            assignment.speaker_id = target_speaker_id.to_string();
            assignment.method = SpeakerAssignmentMethod::Manual;
            assignment.evidence.notes = Some(format!("Merged from {}", source_speaker_id));
        }
    }

    // Remove source speaker from active roster
    speakers.remove(source_idx);

    Ok(())
}

/// The speaker one segment resolves to, given its channel and its cluster.
#[allow(dead_code)]
fn resolve_segment_speaker(
    channel: SegmentChannel,
    cluster: Option<usize>,
    local_clusters: &[usize],
) -> Option<String> {
    // Rung 1 first, and it is not overridable: microphone-only audio is the
    // person holding the microphone.
    if channel == SegmentChannel::Mic {
        return Some(SPEAKER_ID_ME.to_string());
    }

    match cluster {
        Some(c) if local_clusters.contains(&c) => Some(SPEAKER_ID_ME.to_string()),
        Some(c) => Some(remote_speaker_id(remote_index(c, local_clusters))),
        // No cluster: fall back to what the channel alone can say.
        None => channel.implied_speaker_id().map(|id| id.to_string()),
    }
}

/// Clusters that coincide with microphone-only audio more often than not.
///
/// The fallback reading, used when a diarization run could not decide which
/// voice is the local user's — or when there was no run at all. It needs a
/// genuinely microphone-exclusive utterance to exist, which is why it cannot be
/// the primary: with speakers rather than headphones every utterance registers
/// both sources and this finds nothing.
fn local_user_clusters(segments: &[NormalizedSegment], clusters: &HashMap<&str, usize>) -> Vec<usize> {
    let mut mic_hits: HashMap<usize, usize> = HashMap::new();
    let mut other_hits: HashMap<usize, usize> = HashMap::new();

    for segment in segments {
        let Some(&cluster) = clusters.get(segment.id.as_str()) else {
            continue;
        };
        match segment.channel {
            SegmentChannel::Mic => *mic_hits.entry(cluster).or_insert(0) += 1,
            SegmentChannel::System => *other_hits.entry(cluster).or_insert(0) += 1,
            // A mixed or unknown chunk is evidence for neither side.
            _ => {}
        }
    }

    let mut local: Vec<usize> = mic_hits
        .into_iter()
        .filter(|(cluster, mic)| *mic > other_hits.get(cluster).copied().unwrap_or(0))
        .map(|(cluster, _)| cluster)
        .collect();
    local.sort_unstable();
    local
}

/// Position of a remote cluster among the remote clusters, so ids stay
/// contiguous when one cluster is the local user's.
fn remote_index(cluster: usize, local_clusters: &[usize]) -> usize {
    cluster - local_clusters.iter().filter(|&&l| l < cluster).count()
}

/// Builds the registry from the segment counts, preserving names.
fn build_roster(
    counts: &HashMap<String, usize>,
    local_clusters: &[usize],
    clusters: &HashMap<&str, usize>,
    existing: &[Speaker],
    diarization: Option<&Diarization>,
) -> Vec<Speaker> {
    let diarized = diarization.is_some_and(|d| d.report.cluster_count > 0);
    let mut speakers: Vec<Speaker> = Vec::new();

    if let Some(&count) = counts.get(SPEAKER_ID_ME) {
        speakers.push(build_speaker(
            SPEAKER_ID_ME,
            "Me",
            SegmentChannel::Mic,
            true,
            count,
            // "Me" is a channel fact whether or not diarization ran.
            SpeakerOrigin::Channel,
            existing,
        ));
    }

    // Remote speakers, in cluster order, so `Speaker 1` is the first remote
    // voice heard rather than whichever id sorts first as a string.
    let mut remote_ids: Vec<(usize, String)> = if diarized {
        let mut distinct: Vec<usize> = clusters
            .values()
            .copied()
            .filter(|c| !local_clusters.contains(c))
            .collect();
        distinct.sort_unstable();
        distinct.dedup();
        distinct
            .into_iter()
            .map(|c| {
                let index = remote_index(c, local_clusters);
                (index, remote_speaker_id(index))
            })
            .collect()
    } else {
        vec![(0, SPEAKER_ID_REMOTE.to_string())]
    };
    remote_ids.sort_by_key(|(index, _)| *index);

    for (index, id) in remote_ids {
        let Some(&count) = counts.get(&id) else {
            continue;
        };
        speakers.push(build_speaker(
            &id,
            &format!("Speaker {}", index + 1),
            SegmentChannel::System,
            false,
            count,
            if diarized {
                SpeakerOrigin::Diarization
            } else {
                SpeakerOrigin::Channel
            },
            existing,
        ));
    }

    // Ensure any speaker present in segment counts is represented in the roster
    for (id, &count) in counts {
        if id != SPEAKER_ID_ME && !speakers.iter().any(|s| &s.id == id) {
            let label = if id == SPEAKER_ID_REMOTE {
                "Speaker 1".to_string()
            } else {
                format!("Speaker {}", speakers.len() + 1)
            };
            speakers.push(build_speaker(
                id,
                &label,
                SegmentChannel::System,
                false,
                count,
                if diarized {
                    SpeakerOrigin::Diarization
                } else {
                    SpeakerOrigin::Channel
                },
                existing,
            ));
        }
    }

    // A speaker the user has named but who did not speak in this run (e.g. the
    // mic was muted) is retained rather than dropped, so the name is not lost.
    for prior in existing {
        if !speakers.iter().any(|s| s.id == prior.id) {
            let mut carried = prior.clone();
            carried.segment_count = 0;
            speakers.push(carried);
        }
    }

    speakers
}

#[allow(clippy::too_many_arguments)]
fn build_speaker(
    id: &str,
    fallback_label: &str,
    channel: SegmentChannel,
    is_local_user: bool,
    segment_count: usize,
    found_by: SpeakerOrigin,
    existing: &[Speaker],
) -> Speaker {
    let prior = existing.iter().find(|s| s.id == id);
    Speaker {
        id: id.to_string(),
        display_name: prior.and_then(|s| s.display_name.clone()),
        fallback_label: fallback_label.to_string(),
        // A manual name is a stronger claim than whichever rung found the
        // speaker, so it is preserved on re-attribution.
        origin: match prior.map(|s| s.origin) {
            Some(SpeakerOrigin::Manual) => SpeakerOrigin::Manual,
            _ => found_by,
        },
        channel,
        is_local_user,
        segment_count,
    }
}

/// Renames a speaker in the registry.
///
/// Returns `Err` for an unknown id rather than creating a speaker who never
/// spoke. An empty or whitespace name clears the override, restoring the
/// `Speaker N` fallback.
pub fn rename_speaker(
    speakers: &mut [Speaker],
    speaker_id: &str,
    display_name: Option<&str>,
) -> Result<(), String> {
    let speaker = speakers
        .iter_mut()
        .find(|s| s.id == speaker_id)
        .ok_or_else(|| format!("Unknown speaker {}", speaker_id))?;

    match display_name.map(str::trim).filter(|n| !n.is_empty()) {
        Some(name) => {
            speaker.display_name = Some(name.to_string());
            speaker.origin = SpeakerOrigin::Manual;
        }
        None => {
            speaker.display_name = None;
            // Reverting to the fallback also reverts the claim about how this
            // speaker was identified.
            speaker.origin = match speaker.channel {
                SegmentChannel::Mic | SegmentChannel::System => SpeakerOrigin::Channel,
                _ => SpeakerOrigin::Diarization,
            };
        }
    }

    Ok(())
}

/// Resolves a speaker id to the name to display, without inventing one.
/// Unknown or absent ids render as "Unknown speaker".
pub fn resolve_label<'a>(speakers: &'a [Speaker], speaker_id: Option<&str>) -> &'a str {
    match speaker_id {
        Some(id) => speakers
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.label())
            .unwrap_or("Unknown speaker"),
        None => "Unknown speaker",
    }
}

/// Matches a model-supplied owner string to a speaker.
///
/// Accepts a speaker id, a current display name, or a fallback label — case
/// insensitively. Anything else returns `None`, which is what keeps an invented
/// name out of an action item's owner field.
pub fn match_speaker<'a>(speakers: &'a [Speaker], candidate: &str) -> Option<&'a Speaker> {
    let needle = candidate.trim().to_lowercase();
    if needle.is_empty() {
        return None;
    }

    speakers.iter().find(|s| {
        s.id.to_lowercase() == needle
            || s.fallback_label.to_lowercase() == needle
            || s.display_name
                .as_deref()
                .is_some_and(|n| n.trim().to_lowercase() == needle)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meetings_v2::processing::normalize::{normalize_transcript, RawSegmentInput};

    fn raw(chunk_index: usize, text: &str, mic: bool, sys: bool) -> RawSegmentInput {
        RawSegmentInput {
            chunk_index,
            utterance_index: None,
            start_time_s: chunk_index as f64 * 30.0,
            end_time_s: (chunk_index + 1) as f64 * 30.0,
            text: text.to_string(),
            mic_had_audio: mic,
            sys_had_audio: sys,
        }
    }

    fn fixture() -> Vec<NormalizedSegment> {
        let raws = vec![
            raw(0, "I will send the document tomorrow", true, false),
            raw(1, "Agreed I can take care of the deployment", false, true),
            raw(2, "Both of us were talking over each other", true, true),
        ];
        normalize_transcript(&raws, &[]).segments
    }

    #[test]
    fn channels_attribute_only_the_unambiguous_segments() {
        let mut segments = fixture();
        let speakers = attribute_speakers(&mut segments, &[], SpeakerIdentificationMode::Automatic);

        assert_eq!(segments[0].speaker_id.as_deref(), Some(SPEAKER_ID_ME));
        assert_eq!(segments[1].speaker_id.as_deref(), Some(SPEAKER_ID_REMOTE));
        assert_eq!(
            segments[2].speaker_id, None,
            "a chunk with both channels audible must not be attributed"
        );

        assert_eq!(speakers.len(), 2);
        assert!(speakers
            .iter()
            .any(|s| s.id == SPEAKER_ID_ME && s.is_local_user));
        assert_eq!(
            speakers
                .iter()
                .find(|s| s.id == SPEAKER_ID_REMOTE)
                .unwrap()
                .label(),
            "Speaker 1"
        );
    }

    #[test]
    fn unknown_speakers_keep_their_speaker_n_labels() {
        let mut segments = fixture();
        let speakers = attribute_speakers(&mut segments, &[], SpeakerIdentificationMode::Automatic);
        let remote = speakers.iter().find(|s| s.id == SPEAKER_ID_REMOTE).unwrap();
        assert_eq!(remote.display_name, None);
        assert_eq!(remote.label(), "Speaker 1");
    }

    #[test]
    fn renaming_changes_the_label_but_not_the_id() {
        let mut segments = fixture();
        let mut speakers =
            attribute_speakers(&mut segments, &[], SpeakerIdentificationMode::Automatic);

        rename_speaker(&mut speakers, SPEAKER_ID_REMOTE, Some("Pranjali")).unwrap();

        let remote = speakers.iter().find(|s| s.id == SPEAKER_ID_REMOTE).unwrap();
        assert_eq!(remote.id, SPEAKER_ID_REMOTE, "the id must never change");
        assert_eq!(remote.label(), "Pranjali");
        assert_eq!(remote.origin, SpeakerOrigin::Manual);

        // The segments still reference the id, never the name.
        assert_eq!(segments[1].speaker_id.as_deref(), Some(SPEAKER_ID_REMOTE));
    }

    #[test]
    fn a_rename_survives_re_attribution() {
        let mut segments = fixture();
        let mut speakers =
            attribute_speakers(&mut segments, &[], SpeakerIdentificationMode::Automatic);
        rename_speaker(&mut speakers, SPEAKER_ID_REMOTE, Some("Pranjali")).unwrap();

        // Regeneration must not discard the user's work.
        let mut segments2 = fixture();
        let speakers2 = attribute_speakers(
            &mut segments2,
            &speakers,
            SpeakerIdentificationMode::Automatic,
        );
        assert_eq!(
            speakers2
                .iter()
                .find(|s| s.id == SPEAKER_ID_REMOTE)
                .unwrap()
                .label(),
            "Pranjali"
        );
    }

    #[test]
    fn clearing_a_name_restores_the_fallback() {
        let mut segments = fixture();
        let mut speakers =
            attribute_speakers(&mut segments, &[], SpeakerIdentificationMode::Automatic);
        rename_speaker(&mut speakers, SPEAKER_ID_REMOTE, Some("Pranjali")).unwrap();
        rename_speaker(&mut speakers, SPEAKER_ID_REMOTE, Some("   ")).unwrap();

        let remote = speakers.iter().find(|s| s.id == SPEAKER_ID_REMOTE).unwrap();
        assert_eq!(remote.label(), "Speaker 1");
        assert_eq!(remote.origin, SpeakerOrigin::Channel);
    }

    #[test]
    fn renaming_an_unknown_speaker_is_rejected_rather_than_inventing_one() {
        let mut speakers = Vec::new();
        assert!(rename_speaker(&mut speakers, "speaker_9", Some("Ghost")).is_err());
        assert!(speakers.is_empty());
    }

    // -----------------------------------------------------------------------
    // Rung 4 — diarization
    //
    // The reported failure: a 44-minute meeting with twenty people showed one
    // remote speaker, because channel attribution has only two buckets.
    // -----------------------------------------------------------------------

    use crate::meetings_v2::diarize::{Diarization, DiarizationReport, VoiceAssignment};

    fn diarization(clusters: &[(&str, Option<usize>)], cluster_count: usize) -> Diarization {
        Diarization {
            report: DiarizationReport {
                cluster_count,
                placed_count: clusters.iter().filter(|(_, c)| c.is_some()).count(),
                unplaced_count: clusters.iter().filter(|(_, c)| c.is_none()).count(),
                skipped_count: 0,
                local_cluster: None,
                well_separated: true,
                mean_within_distance: 0.2,
                min_between_distance: 1.4,
                singleton_speaker_count: 0,
                silhouette: 0.81,
                expected_speakers: None,
                duration_ms: 40,
                embedding_provider: None,
                fallback_used: false,
                embedding_duration_ms: 0,
            },
            assignments: clusters
                .iter()
                .map(|(id, cluster)| VoiceAssignment {
                    segment_id: (*id).to_string(),
                    cluster: *cluster,
                    distance: 0.2,
                })
                .collect(),
            self_voice_anchor: None,
            self_voice_similarities: HashMap::new(),
        }
    }

    /// Four remote turns from three different voices, plus one of the user's.
    fn conference_call() -> Vec<NormalizedSegment> {
        let raws = vec![
            raw(0, "Right, shall we start with the placement numbers", true, false),
            raw(1, "We closed forty-one this month", false, true),
            raw(2, "That is ahead of where we were in July", false, true),
            raw(3, "I can pull the cohort breakdown before Thursday", false, true),
            raw(4, "And I will circulate the sheet after that", false, true),
        ];
        normalize_transcript(&raws, &[]).segments
    }

    #[test]
    fn diarization_splits_the_remote_bucket_into_a_real_roster() {
        let mut segments = conference_call();
        // Three distinct remote voices across the four system-audio turns.
        let voices = diarization(
            &[
                ("seg_00000", Some(0)),
                ("seg_00001", Some(1)),
                ("seg_00002", Some(2)),
                ("seg_00003", Some(3)),
                ("seg_00004", Some(3)),
            ],
            4,
        );

        let roster = attribute_speakers_with_voices(
            &mut segments,
            &[],
            SpeakerIdentificationMode::Automatic,
            Some(&voices),
        );

        assert_eq!(
            roster.len(),
            4,
            "one local user and three remote voices, not one bucket: {:?}",
            roster.iter().map(|s| s.label()).collect::<Vec<_>>()
        );
        let remote: Vec<&str> = roster
            .iter()
            .filter(|s| !s.is_local_user)
            .map(|s| s.label())
            .collect();
        assert_eq!(remote, vec!["Speaker 1", "Speaker 2", "Speaker 3"]);
    }

    #[test]
    fn without_diarization_the_roster_is_still_the_two_bucket_answer() {
        // Rung 1 alone. Unchanged behaviour, and the reason rung 4 exists.
        let mut segments = conference_call();
        let roster = attribute_speakers(&mut segments, &[], SpeakerIdentificationMode::Automatic);
        assert_eq!(roster.len(), 2);
        assert_eq!(
            roster
                .iter()
                .filter(|s| !s.is_local_user)
                .map(|s| s.label())
                .collect::<Vec<_>>(),
            vec!["Speaker 1"]
        );
    }

    #[test]
    fn the_microphone_channel_is_never_overridden_by_a_cluster() {
        // The channel is what the audio device reported; a cluster is an
        // inference about it. Where they disagree about the local user, the
        // device wins.
        let mut segments = conference_call();
        let voices = diarization(
            &[
                // Cluster 2 is mostly remote, and the run put the user's own
                // turn in it. That must not make the user a remote speaker.
                ("seg_00000", Some(2)),
                ("seg_00001", Some(2)),
                ("seg_00002", Some(2)),
                ("seg_00003", Some(1)),
                ("seg_00004", Some(1)),
            ],
            3,
        );

        attribute_speakers_with_voices(
            &mut segments,
            &[],
            SpeakerIdentificationMode::Automatic,
            Some(&voices),
        );
        assert_eq!(segments[0].speaker_id.as_deref(), Some(SPEAKER_ID_ME));
    }

    #[test]
    fn the_local_users_own_cluster_does_not_also_become_a_remote_speaker() {
        // The user talks on mic and is also heard through the loopback, so
        // their voice forms a cluster. Without intersecting the cluster against
        // the channel, they appear twice.
        let raws = vec![
            raw(0, "Let me share what I found", true, false),
            raw(1, "So the migration is nearly done", true, false),
            raw(2, "Sounds good, ship it", false, true),
        ];
        let mut segments = normalize_transcript(&raws, &[]).segments;
        let voices = diarization(
            &[
                ("seg_00000", Some(0)),
                ("seg_00001", Some(0)),
                ("seg_00002", Some(1)),
            ],
            2,
        );

        let roster = attribute_speakers_with_voices(
            &mut segments,
            &[],
            SpeakerIdentificationMode::Automatic,
            Some(&voices),
        );

        assert_eq!(roster.len(), 2, "{:?}", roster.iter().map(|s| s.label()).collect::<Vec<_>>());
        assert_eq!(roster.iter().filter(|s| s.is_local_user).count(), 1);
        assert_eq!(
            roster
                .iter()
                .filter(|s| !s.is_local_user)
                .map(|s| s.label())
                .collect::<Vec<_>>(),
            vec!["Speaker 1"]
        );
    }

    #[test]
    fn diarization_attributes_a_mixed_chunk_the_channel_had_to_leave_blank() {
        let raws = vec![raw(0, "We were both talking over each other", true, true)];
        let mut segments = normalize_transcript(&raws, &[]).segments;

        // The channel alone leaves this unattributed.
        attribute_speakers(&mut segments, &[], SpeakerIdentificationMode::Automatic);
        assert_eq!(segments[0].speaker_id, None);

        let mut segments = normalize_transcript(&raws, &[]).segments;
        let voices = diarization(&[("seg_00000", Some(0))], 1);
        attribute_speakers_with_voices(
            &mut segments,
            &[],
            SpeakerIdentificationMode::Automatic,
            Some(&voices),
        );
        assert_eq!(segments[0].speaker_id.as_deref(), Some(SPEAKER_ID_REMOTE));
    }

    #[test]
    fn an_unplaced_stretch_falls_back_to_the_channel_rather_than_guessing() {
        let raws = vec![
            raw(0, "Someone spoke here but too briefly to place", true, true),
            raw(1, "This one came through the call", false, true),
        ];
        let mut segments = normalize_transcript(&raws, &[]).segments;
        let voices = diarization(&[("seg_00000", None), ("seg_00001", Some(0))], 1);

        attribute_speakers_with_voices(
            &mut segments,
            &[],
            SpeakerIdentificationMode::Automatic,
            Some(&voices),
        );

        assert_eq!(segments[0].speaker_id, None, "a mixed chunk with no cluster stays honest");
        assert_eq!(segments[1].speaker_id.as_deref(), Some(SPEAKER_ID_REMOTE));
    }

    #[test]
    fn a_diarized_speaker_records_that_a_cluster_found_them() {
        let mut segments = conference_call();
        let voices = diarization(
            &[
                ("seg_00000", Some(0)),
                ("seg_00001", Some(1)),
                ("seg_00002", Some(1)),
                ("seg_00003", Some(2)),
                ("seg_00004", Some(2)),
            ],
            3,
        );
        let roster = attribute_speakers_with_voices(
            &mut segments,
            &[],
            SpeakerIdentificationMode::Automatic,
            Some(&voices),
        );

        let me = roster.iter().find(|s| s.is_local_user).unwrap();
        assert_eq!(
            me.origin,
            SpeakerOrigin::Channel,
            "\"Me\" is a channel fact, not an acoustic inference"
        );
        for remote in roster.iter().filter(|s| !s.is_local_user) {
            assert_eq!(remote.origin, SpeakerOrigin::Diarization);
        }
    }

    #[test]
    fn a_rename_survives_diarization_being_run_afterwards() {
        // The user named Speaker 1 before diarization existed. Running it must
        // not lose that name — which is why cluster 0 maps to `speaker_1`.
        let mut segments = conference_call();
        let mut roster =
            attribute_speakers(&mut segments, &[], SpeakerIdentificationMode::Automatic);
        rename_speaker(&mut roster, SPEAKER_ID_REMOTE, Some("Pranjali")).unwrap();

        let mut segments = conference_call();
        let voices = diarization(
            &[
                ("seg_00000", Some(0)),
                ("seg_00001", Some(1)),
                ("seg_00002", Some(1)),
                ("seg_00003", Some(2)),
                ("seg_00004", Some(2)),
            ],
            3,
        );
        let after = attribute_speakers_with_voices(
            &mut segments,
            &roster,
            SpeakerIdentificationMode::Automatic,
            Some(&voices),
        );

        let named = after.iter().find(|s| s.id == SPEAKER_ID_REMOTE).unwrap();
        assert_eq!(named.label(), "Pranjali");
        assert_eq!(named.origin, SpeakerOrigin::Manual);
    }

    #[test]
    fn a_diarization_that_found_nothing_falls_back_to_the_channel() {
        let mut segments = conference_call();
        let empty = diarization(&[], 0);
        let roster = attribute_speakers_with_voices(
            &mut segments,
            &[],
            SpeakerIdentificationMode::Automatic,
            Some(&empty),
        );
        assert_eq!(roster.len(), 2);
        assert_eq!(segments[1].speaker_id.as_deref(), Some(SPEAKER_ID_REMOTE));
    }

    #[test]
    fn remote_ids_stay_contiguous_when_a_cluster_belongs_to_the_local_user() {
        // Cluster 1 is the user's. The remaining clusters must be Speaker 1 and
        // Speaker 2, with no gap where cluster 1 used to be.
        let raws = vec![
            raw(0, "My own voice on the microphone", true, false),
            raw(1, "The first remote voice", false, true),
            raw(2, "A second remote voice", false, true),
        ];
        let mut segments = normalize_transcript(&raws, &[]).segments;
        let voices = diarization(
            &[
                ("seg_00000", Some(1)),
                ("seg_00001", Some(0)),
                ("seg_00002", Some(2)),
            ],
            3,
        );
        let roster = attribute_speakers_with_voices(
            &mut segments,
            &[],
            SpeakerIdentificationMode::Automatic,
            Some(&voices),
        );

        let labels: Vec<&str> = roster
            .iter()
            .filter(|s| !s.is_local_user)
            .map(|s| s.label())
            .collect();
        assert_eq!(labels, vec!["Speaker 1", "Speaker 2"]);
        let ids: Vec<&str> = roster
            .iter()
            .filter(|s| !s.is_local_user)
            .map(|s| s.id.as_str())
            .collect();
        assert_eq!(ids, vec!["speaker_1", "speaker_2"]);
    }

    #[test]
    fn diarization_is_ignored_when_identification_is_off() {
        let mut segments = conference_call();
        let voices = diarization(&[("seg_00001", Some(1))], 2);
        attribute_speakers_with_voices(
            &mut segments,
            &[],
            SpeakerIdentificationMode::Off,
            Some(&voices),
        );
        assert!(segments.iter().all(|s| s.speaker_id.is_none()));
    }

    #[test]
    fn the_local_user_is_identified_from_relative_microphone_share() {
        // The reported failure, end to end: a call taken on speakers, where
        // every utterance registers both sources and nothing is ever cleanly
        // microphone-only. The channel alone finds no local user and the
        // roster came back as Speaker 1, Speaker 2, Speaker 3 with no "Me".
        let raws = vec![
            raw(0, "Shall we start with the placement numbers", true, true),
            raw(1, "We closed forty-one this month", true, true),
            raw(2, "That is ahead of where we were in July", true, true),
        ];
        let mut segments = normalize_transcript(&raws, &[]).segments;

        let mut voices = diarization(
            &[
                ("seg_00000", Some(0)),
                ("seg_00001", Some(1)),
                ("seg_00002", Some(2)),
            ],
            3,
        );
        // The run compared microphone share and found cluster 0 dominant.
        voices.report.local_cluster = Some(0);

        let roster = attribute_speakers_with_voices(
            &mut segments,
            &[],
            SpeakerIdentificationMode::Automatic,
            Some(&voices),
        );

        assert_eq!(segments[0].speaker_id.as_deref(), Some(SPEAKER_ID_ME));
        assert_eq!(
            roster.iter().filter(|s| s.is_local_user).count(),
            1,
            "exactly one speaker is the person using this machine: {:?}",
            roster.iter().map(|s| s.label()).collect::<Vec<_>>()
        );
        assert_eq!(
            roster
                .iter()
                .filter(|s| !s.is_local_user)
                .map(|s| s.label())
                .collect::<Vec<_>>(),
            vec!["Speaker 1", "Speaker 2"]
        );
    }

    #[test]
    fn a_run_that_could_not_decide_falls_back_to_the_channel() {
        // In-person, or a meeting the user never spoke in. The channel reading
        // still applies where a microphone-exclusive utterance exists.
        let raws = vec![
            raw(0, "My own voice on the microphone alone", true, false),
            raw(1, "Somebody arriving through the call", false, true),
        ];
        let mut segments = normalize_transcript(&raws, &[]).segments;

        let mut voices = diarization(&[("seg_00000", Some(0)), ("seg_00001", Some(1))], 2);
        voices.report.local_cluster = None;

        attribute_speakers_with_voices(
            &mut segments,
            &[],
            SpeakerIdentificationMode::Automatic,
            Some(&voices),
        );

        assert_eq!(segments[0].speaker_id.as_deref(), Some(SPEAKER_ID_ME));
        assert_eq!(segments[1].speaker_id.as_deref(), Some(SPEAKER_ID_REMOTE));
    }

    #[test]
    fn the_local_user_never_also_appears_as_a_remote_speaker() {
        let raws = vec![
            raw(0, "Me talking", true, true),
            raw(1, "Me again", true, true),
            raw(2, "Somebody else", true, true),
        ];
        let mut segments = normalize_transcript(&raws, &[]).segments;
        let mut voices = diarization(
            &[
                ("seg_00000", Some(0)),
                ("seg_00001", Some(0)),
                ("seg_00002", Some(1)),
            ],
            2,
        );
        voices.report.local_cluster = Some(0);

        let roster = attribute_speakers_with_voices(
            &mut segments,
            &[],
            SpeakerIdentificationMode::Automatic,
            Some(&voices),
        );

        assert_eq!(roster.len(), 2);
        assert_eq!(roster.iter().filter(|s| s.is_local_user).count(), 1);
        assert_eq!(segments[2].speaker_id.as_deref(), Some(SPEAKER_ID_REMOTE));
    }

    #[test]
    fn attribution_off_leaves_every_segment_unattributed() {
        let mut segments = fixture();
        attribute_speakers(&mut segments, &[], SpeakerIdentificationMode::Off);
        assert!(segments.iter().all(|s| s.speaker_id.is_none()));
    }

    #[test]
    fn owner_matching_accepts_ids_labels_and_names_but_nothing_invented() {
        let mut segments = fixture();
        let mut speakers =
            attribute_speakers(&mut segments, &[], SpeakerIdentificationMode::Automatic);
        rename_speaker(&mut speakers, SPEAKER_ID_REMOTE, Some("Pranjali")).unwrap();

        assert_eq!(
            match_speaker(&speakers, "speaker_1").unwrap().id,
            SPEAKER_ID_REMOTE
        );
        assert_eq!(
            match_speaker(&speakers, "Pranjali").unwrap().id,
            SPEAKER_ID_REMOTE
        );
        assert_eq!(
            match_speaker(&speakers, "  me  ").unwrap().id,
            SPEAKER_ID_ME
        );
        assert!(
            match_speaker(&speakers, "Someone Who Was Never Here").is_none(),
            "a name the meeting never established must not resolve to a speaker"
        );
    }

    #[test]
    fn an_unresolved_speaker_renders_honestly() {
        let speakers: Vec<Speaker> = Vec::new();
        assert_eq!(resolve_label(&speakers, None), "Unknown speaker");
        assert_eq!(
            resolve_label(&speakers, Some("speaker_4")),
            "Unknown speaker"
        );
    }
}
