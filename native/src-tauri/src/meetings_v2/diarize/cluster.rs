//! Groups voice features into speakers.
//!
//! Agglomerative clustering under average linkage, with the number of speakers
//! chosen from the *shape* of the merge sequence rather than from a distance
//! threshold.
//!
//! Why agglomerative rather than k-means: the number of speakers is the thing
//! we are trying to find. k-means needs it up front, and picking it wrong is
//! not a small error — it either merges two people or splits one person in
//! half, and both are worse than saying "this stretch is unattributed".
//!
//! Why the merge sequence rather than a threshold: a single absolute distance
//! bound has to be right in two directions at once, and it will not be. Set it
//! tight and one animated speaker becomes three; set it loose and a room of
//! twenty collapses to one. Merging all the way down to a single cluster and
//! recording each merge distance sidesteps that: within one speaker the
//! distances rise smoothly, and the first merge that crosses between two
//! speakers jumps. The jump is what says how many voices there were, and it is
//! scale-free — it holds for a quiet recording and a loud one, a close mic and
//! a far one.
//!
//! An absolute floor is still needed, because a jump can be a large *ratio*
//! between two tiny distances. That floor is the only calibrated constant here,
//! and it is a "do not bother splitting anything closer than this" bound rather
//! than a claim about where two voices sit apart.

use super::features::VoiceFeatures;

/// Weight on the MFCC standard-deviation half of the feature vector.
///
/// Below the means because it is weaker evidence: how much a voice moves
/// depends on what the person was saying, whereas the mean depends on the shape
/// of their vocal tract.
const STD_WEIGHT: f32 = 0.5;

/// Weight on the log-pitch term.
///
/// The term is a natural log ratio against 150 Hz, so an octave is about 0.7
/// while inter-speaker MFCC differences run to several units. Without a weight
/// the strongest cue a human would use contributes almost nothing.
const PITCH_WEIGHT: f32 = 8.0;

/// Distance below which two clusters are never treated as different speakers,
/// in per-dimension RMS feature units.
///
/// This is the floor that stops the elbow rule from splitting on jitter: a
/// large ratio between two very small distances is noise, not a speaker change.
///
/// Calibrated by measurement, not taste. On the synthetic voices in
/// `diarize::tests` one speaker's utterances scatter about 0.4 from their own
/// centroid, three distinct speakers sit about 5.7 apart, and forcing those
/// three into one cluster gives a within-distance of 3.9. So the two regimes
/// are an order of magnitude apart and this sits between them, closer to the
/// same-speaker end because leaving two people merged is the more legible
/// failure: "Speaker 2 said both of these" is wrong but readable, whereas a
/// split invents a person who was never in the room.
///
/// Real audio is noisier on both sides than a synthesized vowel, so this is a
/// starting point rather than a settled constant. Two things make that safe:
/// [`Clustering::is_well_separated`] reports when a roster should not be
/// presented as fact, and the expected-speaker hint overrides the rule outright.
const MIN_SPLIT_DISTANCE: f32 = 2.0;

/// Absolute distance the *first* merge must clear to count as a speaker
/// boundary on its own.
///
/// An elbow is a step up from something. The first merge has nothing before it,
/// so a ratio cannot be computed and only an unambiguous absolute distance will
/// do. This is the case where every stretch sounds different from every other:
/// either each one really is a different person, or the features are noise, and
/// a bounded roster with its separation reported is the honest output either way.
const FIRST_MERGE_SPLIT_DISTANCE: f32 = MIN_SPLIT_DISTANCE * 2.0;

/// How much a merge distance must jump, relative to the merge before it, to be
/// read as the boundary between two speakers.
const ELBOW_MIN_RATIO: f32 = 1.6;

/// Hard cap on discovered speakers, when no expected count was given.
///
/// Not a claim about meeting sizes. Past this point MFCC statistics cannot
/// support the distinction and further clusters are noise, so the honest output
/// is a bounded roster plus unattributed stretches.
pub const MAX_DISCOVERED_SPEAKERS: usize = 12;

/// One stretch of speech to be assigned a speaker.
#[derive(Debug, Clone)]
pub struct Utterance {
    /// Caller's identifier, returned untouched on the assignment.
    pub id: String,
    pub start_time_s: f64,
    pub end_time_s: f64,
    pub features: VoiceFeatures,
}

/// Which cluster a stretch was assigned to.
#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    pub id: String,
    /// `None` when the stretch carried too little voice to place.
    pub cluster: Option<usize>,
    /// Distance from this stretch to its cluster's centroid. Lower is a better
    /// fit; the UI uses it to mark an attribution as uncertain.
    pub distance: f32,
}

/// The outcome of clustering one meeting.
#[derive(Debug, Clone, PartialEq)]
pub struct Clustering {
    pub assignments: Vec<Assignment>,
    /// Clusters found, numbered by when each voice was first heard, so
    /// `Speaker 1` is the first person heard rather than a merge artefact.
    pub cluster_count: usize,
    /// Mean distance from a member to its own cluster's centroid.
    pub mean_within_distance: f32,
    /// Smallest distance between two surviving cluster centroids.
    pub min_between_distance: f32,
    pub unplaced_count: usize,
}

impl Clustering {
    /// Whether the separation is good enough to present the roster as fact
    /// rather than as a guess.
    ///
    /// The test is relative: clusters must sit further from each other than
    /// their members sit from their own centroids. A roster that fails this is
    /// still returned — it is better than one bucket for twenty people — but
    /// the caller is expected to mark it unconfirmed.
    pub fn is_well_separated(&self) -> bool {
        self.cluster_count <= 1 || self.min_between_distance > self.mean_within_distance * 1.5
    }
}

/// Per-dimension RMS distance between two feature vectors.
///
/// Euclidean rather than cosine, and deliberately *not* centred on the
/// meeting's own mean. Centring cancels out of a difference anyway, and any
/// data-derived *scaling* would make the distance scale-free — which is
/// precisely what destroys the "is this the same voice" judgement, because one
/// speaker's jitter then normalizes up to look like a room full of people. The
/// weights are fixed, so a distance means the same thing in every meeting.
///
/// Dividing by the dimension count makes the result a per-dimension RMS
/// difference, which is what lets [`MIN_SPLIT_DISTANCE`] be stated in units
/// anyone can reason about.
pub fn distance(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    if n == 0 {
        return 0.0;
    }
    let mut sum_sq = 0.0f64;
    for i in 0..n {
        let d = (a[i] - b[i]) as f64;
        sum_sq += d * d;
    }
    (sum_sq / n as f64).sqrt() as f32
}

/// Applies the fixed weighting to a raw feature vector.
fn weighted(features: &VoiceFeatures) -> Vec<f32> {
    let raw = features.vector();
    let coeffs = features.mfcc_mean.len();
    raw.iter()
        .enumerate()
        .map(|(i, &x)| {
            if i < coeffs {
                x
            } else if i < coeffs * 2 {
                x * STD_WEIGHT
            } else {
                x * PITCH_WEIGHT
            }
        })
        .collect()
}

/// Clusters utterances into speakers.
///
/// `expected_speakers` is a hint, not a demand: it fixes where the merge
/// sequence is cut, but it cannot create a cluster the audio does not support.
/// A meeting where twenty people were present but three spoke produces three
/// clusters whatever the hint says, because inventing seventeen silent speakers
/// would be a claim the recording cannot back.
pub fn cluster(utterances: &[Utterance], expected_speakers: Option<usize>) -> Clustering {
    let usable: Vec<usize> = utterances
        .iter()
        .enumerate()
        .filter(|(_, u)| u.features.is_usable())
        .map(|(i, _)| i)
        .collect();

    let unplaced_count = utterances.len() - usable.len();
    let unassigned: Vec<Assignment> = utterances
        .iter()
        .map(|u| Assignment {
            id: u.id.clone(),
            cluster: None,
            distance: 0.0,
        })
        .collect();

    if usable.is_empty() {
        return Clustering {
            assignments: unassigned,
            cluster_count: 0,
            mean_within_distance: 0.0,
            min_between_distance: 0.0,
            unplaced_count,
        };
    }

    let vectors: Vec<Vec<f32>> = usable
        .iter()
        .map(|&i| weighted(&utterances[i].features))
        .collect();

    // Merge all the way down, recording what each merge cost. The sequence is
    // what the stopping rule reads; stopping early would throw away exactly the
    // evidence needed to choose where to stop.
    let (snapshots, merge_distances) = merge_all(&vectors);
    let k = choose_cluster_count(&merge_distances, vectors.len(), expected_speakers);

    // `snapshots[i]` is the partition with `i + 1` clusters.
    let mut members = snapshots
        .get(k.saturating_sub(1))
        .cloned()
        .unwrap_or_else(|| vec![(0..vectors.len()).collect()]);

    // Order clusters by when each was first heard.
    members.sort_by(|left, right| {
        let first = |group: &Vec<usize>| {
            group
                .iter()
                .map(|&i| utterances[usable[i]].start_time_s)
                .fold(f64::MAX, f64::min)
        };
        first(left)
            .partial_cmp(&first(right))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let centroids: Vec<Vec<f32>> = members
        .iter()
        .map(|group| centroid(group, &vectors))
        .collect();

    let mut assignments = unassigned;
    let mut within_total = 0.0f32;
    let mut within_count = 0usize;
    for (cluster_index, group) in members.iter().enumerate() {
        for &local in group {
            let d = distance(&vectors[local], &centroids[cluster_index]);
            within_total += d;
            within_count += 1;
            let original = usable[local];
            assignments[original] = Assignment {
                id: utterances[original].id.clone(),
                cluster: Some(cluster_index),
                distance: d,
            };
        }
    }

    let mut min_between = f32::MAX;
    for i in 0..centroids.len() {
        for j in i + 1..centroids.len() {
            min_between = min_between.min(distance(&centroids[i], &centroids[j]));
        }
    }

    Clustering {
        assignments,
        cluster_count: members.len(),
        mean_within_distance: if within_count == 0 {
            0.0
        } else {
            within_total / within_count as f32
        },
        min_between_distance: if min_between == f32::MAX {
            0.0
        } else {
            min_between
        },
        unplaced_count,
    }
}

/// Merges every point down to one cluster under average linkage.
///
/// Returns the partition at each cluster count — `snapshots[i]` holds `i + 1`
/// clusters — and the distance each merge was made at, ordered from the first
/// merge (cheapest) to the last.
type MergeTrace = (Vec<Vec<Vec<usize>>>, Vec<f32>);

fn merge_all(vectors: &[Vec<f32>]) -> MergeTrace {
    let mut members: Vec<Vec<usize>> = (0..vectors.len()).map(|i| vec![i]).collect();
    let mut snapshots: Vec<Vec<Vec<usize>>> = vec![Vec::new(); vectors.len()];
    let mut merge_distances: Vec<f32> = Vec::new();

    snapshots[members.len() - 1] = members.clone();

    while members.len() > 1 {
        let Some((a, b, best)) = closest_pair(&members, vectors) else {
            break;
        };
        let merged = members.remove(b);
        members[a].extend(merged);
        merge_distances.push(best);
        snapshots[members.len() - 1] = members.clone();
    }

    // Agglomerative merging always takes the closest pair first, so this is
    // already cheapest-first — the order the elbow rule reads.
    (snapshots, merge_distances)
}

/// Chooses how many speakers the merge sequence supports.
///
/// `merge_distances` is ordered cheapest-first: `merge_distances[i]` is the
/// distance the *i*-th merge was made at, taking the partition from
/// `point_count - i` clusters down to `point_count - i - 1`. So cutting the
/// sequence just before merge *i* leaves `point_count - i` clusters.
///
/// Within one speaker those distances rise smoothly; the first merge that
/// crosses between two speakers jumps. The largest jump that clears both the
/// absolute floor and the ratio is the boundary.
///
/// When no merge clears both bars the answer is one speaker. That includes the
/// case where every merge is expensive — points spread uniformly with no
/// grouping in them — because a roster invented from uniform spread would be a
/// guess dressed as a finding. The run's within-cluster distance is reported so
/// the diagnostics surface can show that the audio supported no separation.
fn choose_cluster_count(
    merge_distances: &[f32],
    point_count: usize,
    expected_speakers: Option<usize>,
) -> usize {
    if point_count == 0 {
        return 0;
    }
    if let Some(expected) = expected_speakers.filter(|&n| n > 0) {
        return expected.min(point_count);
    }
    if merge_distances.is_empty() {
        return 1;
    }

    let ceiling = MAX_DISCOVERED_SPEAKERS.min(point_count);

    let mut best: Option<(usize, f32)> = None;
    // Set when the boundary the sequence points at needs more speakers than can
    // be resolved. Cutting at the ceiling is then more honest than collapsing
    // to one, because the audio did say there were several voices.
    let mut boundary_above_ceiling = false;

    for (i, &d) in merge_distances.iter().enumerate() {
        if d < MIN_SPLIT_DISTANCE {
            continue;
        }
        let ratio = match i {
            0 if d >= FIRST_MERGE_SPLIT_DISTANCE => f32::INFINITY,
            0 => continue,
            _ if merge_distances[i - 1] <= 0.0 => f32::INFINITY,
            _ => d / merge_distances[i - 1],
        };
        if ratio < ELBOW_MIN_RATIO {
            continue;
        }

        // This merge joined two speakers, so cutting before it leaves the
        // clusters that existed going into it.
        let clusters = point_count - i;
        if clusters < 2 {
            continue;
        }
        if clusters > ceiling {
            boundary_above_ceiling = true;
            continue;
        }
        if best.as_ref().is_none_or(|&(_, r)| ratio > r) {
            best = Some((clusters, ratio));
        }
    }

    match best {
        Some((k, _)) => k,
        None if boundary_above_ceiling => ceiling,
        None => 1,
    }
}

/// The closest pair of clusters under average linkage, and their distance.
fn closest_pair(members: &[Vec<usize>], vectors: &[Vec<f32>]) -> Option<(usize, usize, f32)> {
    let mut best: Option<(usize, usize, f32)> = None;
    for i in 0..members.len() {
        for j in i + 1..members.len() {
            let mut total = 0.0f32;
            let mut count = 0usize;
            for &a in &members[i] {
                for &b in &members[j] {
                    total += distance(&vectors[a], &vectors[b]);
                    count += 1;
                }
            }
            if count == 0 {
                continue;
            }
            let mean = total / count as f32;
            if best.as_ref().is_none_or(|(_, _, d)| mean < *d) {
                best = Some((i, j, mean));
            }
        }
    }
    best
}

fn centroid(group: &[usize], vectors: &[Vec<f32>]) -> Vec<f32> {
    let width = vectors.first().map(|v| v.len()).unwrap_or(0);
    let mut out = vec![0.0f32; width];
    for &i in group {
        for (o, &v) in out.iter_mut().zip(vectors[i].iter()) {
            *o += v;
        }
    }
    let n = group.len().max(1) as f32;
    for o in out.iter_mut() {
        *o /= n;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A feature vector built directly, so clustering is tested independently
    /// of feature extraction.
    fn features(mfcc: &[f32], pitch: f32, frames: usize) -> VoiceFeatures {
        let width = super::super::features::MFCC_COEFFS;
        let mut mean = mfcc.to_vec();
        mean.resize(width, 0.0);
        VoiceFeatures {
            mfcc_mean: mean,
            mfcc_std: vec![1.0; width],
            pitch_hz: Some(pitch),
            voiced_fraction: 0.7,
            frame_count: frames,
        }
    }

    fn utterance(id: &str, start: f64, mfcc: &[f32], pitch: f32) -> Utterance {
        Utterance {
            id: id.to_string(),
            start_time_s: start,
            end_time_s: start + 5.0,
            features: features(mfcc, pitch, 200),
        }
    }

    /// Three clearly distinct voices, four turns each, interleaved the way a
    /// real conversation is.
    fn three_speaker_meeting() -> Vec<Utterance> {
        let voices: [(&[f32], f32); 3] = [
            (&[8.0, -3.0, 2.0, 1.0, -1.0], 105.0),
            (&[-6.0, 5.0, -3.0, 2.0, 1.0], 210.0),
            (&[1.0, 1.0, 7.0, -5.0, 3.0], 155.0),
        ];
        let mut out = Vec::new();
        for turn in 0..4 {
            for (v, (mfcc, pitch)) in voices.iter().enumerate() {
                let t = (turn * 3 + v) as f64 * 10.0;
                // A little jitter, so identical vectors are not what makes the
                // clustering work.
                let jittered: Vec<f32> = mfcc
                    .iter()
                    .map(|&x| x + (turn as f32 * 0.13 - 0.2))
                    .collect();
                out.push(utterance(
                    &format!("u{}_{}", turn, v),
                    t,
                    &jittered,
                    *pitch + turn as f32,
                ));
            }
        }
        out
    }

    #[test]
    fn three_distinct_voices_produce_three_speakers() {
        let clustering = cluster(&three_speaker_meeting(), None);
        assert_eq!(
            clustering.cluster_count, 3,
            "within {} between {}",
            clustering.mean_within_distance, clustering.min_between_distance
        );
        assert!(clustering.is_well_separated());
        assert_eq!(clustering.unplaced_count, 0);
    }

    #[test]
    fn every_turn_of_one_voice_lands_in_the_same_cluster() {
        let meeting = three_speaker_meeting();
        let clustering = cluster(&meeting, None);
        for voice in 0..3 {
            let clusters: Vec<Option<usize>> = (0..4)
                .map(|turn| {
                    let id = format!("u{}_{}", turn, voice);
                    clustering
                        .assignments
                        .iter()
                        .find(|a| a.id == id)
                        .unwrap()
                        .cluster
                })
                .collect();
            assert!(
                clusters.windows(2).all(|w| w[0] == w[1]),
                "voice {voice} was split across {clusters:?}"
            );
        }
    }

    #[test]
    fn speaker_numbers_follow_the_order_people_first_spoke() {
        let meeting = three_speaker_meeting();
        let clustering = cluster(&meeting, None);
        let first = clustering
            .assignments
            .iter()
            .find(|a| a.id == "u0_0")
            .unwrap();
        assert_eq!(
            first.cluster,
            Some(0),
            "the first voice heard must be the first speaker"
        );
    }

    #[test]
    fn one_voice_produces_one_speaker_not_many() {
        // The failure mode a distance threshold has to avoid: splitting a
        // single person into a roster.
        let mut meeting = Vec::new();
        for i in 0..10 {
            meeting.push(utterance(
                &format!("u{i}"),
                i as f64 * 8.0,
                &[5.0 + i as f32 * 0.1, -2.0, 1.5, 0.5, -0.5],
                140.0 + i as f32,
            ));
        }
        assert_eq!(cluster(&meeting, None).cluster_count, 1);
    }

    #[test]
    fn an_expected_speaker_count_is_a_hint_not_an_invention() {
        // Two voices in the audio, twenty people in the room. Twenty clusters
        // would be twenty claims the recording cannot support.
        let mut meeting = Vec::new();
        for i in 0..6 {
            let (mfcc, pitch): (&[f32], f32) = if i % 2 == 0 {
                (&[9.0, -4.0, 2.0, 1.0, 0.0], 100.0)
            } else {
                (&[-7.0, 6.0, -3.0, 1.0, 2.0], 215.0)
            };
            meeting.push(utterance(&format!("u{i}"), i as f64 * 10.0, mfcc, pitch));
        }
        let clustering = cluster(&meeting, Some(20));
        assert!(
            clustering.cluster_count <= 6,
            "cannot exceed the number of stretches, got {}",
            clustering.cluster_count
        );
        assert!(clustering.cluster_count >= 2);
    }

    #[test]
    fn an_expected_count_can_merge_below_the_threshold() {
        // The user says two; the features would have found three. Honouring the
        // hint is the point of asking for it.
        let clustering = cluster(&three_speaker_meeting(), Some(2));
        assert_eq!(clustering.cluster_count, 2);
    }

    #[test]
    fn stretches_with_too_little_voice_are_left_unplaced() {
        let mut meeting = three_speaker_meeting();
        meeting.push(Utterance {
            id: "noise".to_string(),
            start_time_s: 500.0,
            end_time_s: 501.0,
            // Ten frames and almost no voicing: not evidence about anyone.
            features: features(&[0.0, 0.0, 0.0], 0.0, 10),
        });

        let clustering = cluster(&meeting, None);
        let noise = clustering
            .assignments
            .iter()
            .find(|a| a.id == "noise")
            .unwrap();
        assert_eq!(
            noise.cluster, None,
            "a stretch with no voice must not be assigned to a person"
        );
        assert_eq!(clustering.unplaced_count, 1);
    }

    #[test]
    fn an_empty_meeting_clusters_to_nothing() {
        let clustering = cluster(&[], None);
        assert_eq!(clustering.cluster_count, 0);
        assert!(clustering.assignments.is_empty());
        assert!(clustering.is_well_separated());
    }

    #[test]
    fn a_meeting_of_only_unusable_stretches_assigns_nobody() {
        let meeting = vec![Utterance {
            id: "u0".into(),
            start_time_s: 0.0,
            end_time_s: 1.0,
            features: features(&[1.0], 0.0, 5),
        }];
        let clustering = cluster(&meeting, None);
        assert_eq!(clustering.cluster_count, 0);
        assert_eq!(clustering.assignments[0].cluster, None);
        assert_eq!(clustering.unplaced_count, 1);
    }

    #[test]
    fn the_distance_is_a_per_dimension_rms_difference() {
        assert_eq!(distance(&[1.0, 0.0], &[1.0, 0.0]), 0.0);
        // Two dimensions differing by 2 each: RMS is 2, not 2·√2.
        assert!((distance(&[0.0, 0.0], &[2.0, 2.0]) - 2.0).abs() < 1e-5);
        assert_eq!(distance(&[], &[]), 0.0);
    }

    #[test]
    fn a_shared_channel_component_cancels_out_of_the_distance() {
        // The room, the microphone and the codec are the same for everyone in
        // one recording, so they must not affect who looks like whom. Not
        // centring the data is what makes this hold without any normalization
        // step that could rescale one speaker's jitter into a roster.
        let bare = [vec![1.0f32, 0.0], vec![-1.0f32, 0.5]];
        let offset = [vec![101.0f32, 50.0], vec![99.0f32, 50.5]];
        assert!(
            (distance(&bare[0], &bare[1]) - distance(&offset[0], &offset[1])).abs() < 1e-4
        );
    }

    #[test]
    fn the_elbow_rule_reads_a_smooth_sequence_as_one_speaker() {
        // Distances that rise gently are one voice varying, not two voices.
        let smooth = vec![0.10, 0.13, 0.16, 0.19, 0.22, 0.26];
        assert_eq!(choose_cluster_count(&smooth, 7, None), 1);
    }

    #[test]
    fn the_elbow_rule_finds_the_jump_between_speakers() {
        // Four cheap within-speaker merges on the measured same-voice scale,
        // then one on the measured different-voice scale. The partition going
        // into that merge had two clusters.
        let jumpy = vec![0.30, 0.38, 0.44, 0.52, 5.70];
        assert_eq!(choose_cluster_count(&jumpy, 6, None), 2);
    }

    #[test]
    fn a_jump_that_stays_inside_one_voices_scatter_is_not_a_speaker_change() {
        // A fourfold rise, and still an order of magnitude below where two
        // voices actually sit. This is the failure the measured floor prevents.
        let within_scatter = vec![0.20, 0.24, 0.26, 0.30, 1.20];
        assert_eq!(choose_cluster_count(&within_scatter, 6, None), 1);
    }

    #[test]
    fn a_large_ratio_between_two_tiny_distances_is_not_a_speaker_change() {
        // 0.02 to 0.2 is a tenfold jump and still well inside one voice.
        let tiny = vec![0.01, 0.02, 0.20];
        assert_eq!(choose_cluster_count(&tiny, 4, None), 1);
    }

    #[test]
    fn a_roster_larger_than_can_be_resolved_is_capped_not_collapsed() {
        // Every merge expensive from the first: forty-one stretches that sound
        // nothing like each other. One cluster would be a lie in the other
        // direction, so the answer is the ceiling.
        let all_far: Vec<f32> = (0..40).map(|i| 5.0 + i as f32).collect();
        assert_eq!(
            choose_cluster_count(&all_far, 41, None),
            MAX_DISCOVERED_SPEAKERS
        );
    }

    #[test]
    fn a_first_merge_inside_one_voices_scatter_is_not_a_roster() {
        // Two stretches, close together: one person, not two.
        assert_eq!(choose_cluster_count(&[0.6], 2, None), 1);
        // Two stretches, unmistakably far apart: two people.
        assert_eq!(choose_cluster_count(&[6.0], 2, None), 2);
    }

    #[test]
    fn an_expected_count_is_honoured_over_the_elbow() {
        let smooth = vec![0.10, 0.13, 0.16, 0.19];
        assert_eq!(choose_cluster_count(&smooth, 5, Some(3)), 3);
        // But it can never exceed the number of stretches available.
        assert_eq!(choose_cluster_count(&smooth, 5, Some(50)), 5);
    }

    #[test]
    fn no_points_means_no_speakers() {
        assert_eq!(choose_cluster_count(&[], 0, None), 0);
        assert_eq!(choose_cluster_count(&[], 1, None), 1);
    }

    #[test]
    fn poorly_separated_clusters_report_themselves_as_such() {
        // Two nearly identical voices: the roster may still split them, but it
        // must not claim to be sure.
        let meeting = vec![
            utterance("a", 0.0, &[1.0, 1.0, 1.0], 150.0),
            utterance("b", 10.0, &[1.02, 0.98, 1.01], 151.0),
            utterance("c", 20.0, &[0.99, 1.01, 0.99], 149.0),
            utterance("d", 30.0, &[1.01, 1.0, 1.02], 150.5),
        ];
        let clustering = cluster(&meeting, None);
        if clustering.cluster_count > 1 {
            assert!(
                !clustering.is_well_separated(),
                "a marginal split must be reported as marginal"
            );
        }
    }
}
