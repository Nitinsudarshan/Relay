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

use super::embedding::{cosine_similarity, SpeakerEmbedding};
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

/// Mean silhouette below which a recording is treated as holding one voice.
///
/// The clusterer chooses how many speakers there were by scoring each candidate
/// partition and keeping the best, rather than by testing merge distances
/// against a threshold. Two earlier designs did the latter and both failed, in
/// instructive ways:
///
/// * **An absolute distance floor.** Calibrated on synthetic voices an octave
///   apart, it sat at 2.0 while real voices separate at 0.55–1.47 — above every
///   distance real speech produces, so nothing ever split and a meeting of
///   twenty came back as one speaker.
/// * **A floor derived from the cheapest merges, plus an elbow ratio.** Better,
///   and still wrong in both directions. Measured through the actual recording
///   path, one person's utterances span 0.031–0.422 across a meeting — four
///   times the floor the cheapest merges suggested — so a single speaker split
///   into three. And the elbow gate rejects the correct answer for two similar
///   voices, whose crossing is only a 1.37x step.
///
/// The property that distinguishes a real speaker boundary from a wide-but-
/// single voice is not the size of any distance. It is whether the resulting
/// groups are *tighter internally than they are far apart* — which is what a
/// silhouette measures, and it is scale-free, so it needs no calibration
/// against a microphone, a room, or a codec.
///
/// Set at 0.7 — the conventional reading of "strong structure" — and that
/// choice encodes a limitation worth stating plainly rather than a tuning
/// preference. Measured on the fixtures:
///
/// | recording | best score |
/// |---|---|
/// | one voice across a meeting | 0.52–0.57 |
/// | **two deliberately similar voices** | **0.54** |
/// | two ordinary voices | 0.89 |
/// | three ordinary voices | 0.90 |
///
/// One voice that wanders and two voices that resemble each other score the
/// same. No threshold separates them, because with cepstral features they are
/// not distinguishable — that is a property of the features, not of where the
/// bar is put, and moving the bar only chooses which mistake to make.
///
/// So the bar is placed to make the safer one. Merging two similar voices reads
/// as "Speaker 2 said both of these": wrong, legible, and recoverable — the
/// expected-speaker count forces the split when the user knows better.
/// Splitting one person in two invents somebody who was never in the room, puts
/// their name on commitments, and gives the user nothing to correct. A neural
/// speaker embedding is what actually resolves the ambiguity; until then this
/// fails toward the answer a person can fix.
const MIN_SILHOUETTE_TO_SPLIT: f32 = 0.70;

/// Silhouette above which a roster is presented as fact rather than as a
/// provisional reading.
///
/// Ordinary voices score around 0.89 once separated, so a partition between
/// this and [`MIN_SILHOUETTE_TO_SPLIT`] sits below what a clean recording
/// produces. The split is worth making and worth checking, and the UI says so
/// rather than presenting the roster plainly.
const CONFIDENT_SILHOUETTE: f32 = 0.80;

/// Hard cap on discovered speakers, when no expected count was given.
///
/// A bound on the search, not a claim about meeting sizes: it stops the scan
/// from considering a partition per utterance in a long recording. Set at
/// twenty because meetings of that size are the case this module was rebuilt
/// for, and a cap that bites in an ordinary meeting is a cap in the wrong
/// place. Quality at any given count is reported by the silhouette rather than
/// asserted by this number.
pub const MAX_DISCOVERED_SPEAKERS: usize = 20;

/// One stretch of speech to be assigned a speaker.
#[derive(Debug, Clone)]
pub struct Utterance {
    /// Caller's identifier, returned untouched on the assignment.
    pub id: String,
    pub start_time_s: f64,
    pub end_time_s: f64,
    pub features: VoiceFeatures,
    pub embedding: Option<SpeakerEmbedding>,
}

impl Utterance {
    pub fn new(id: String, start_time_s: f64, end_time_s: f64, features: VoiceFeatures) -> Self {
        Self {
            id,
            start_time_s,
            end_time_s,
            features,
            embedding: None,
        }
    }

    pub fn with_embedding(mut self, embedding: SpeakerEmbedding) -> Self {
        self.embedding = Some(embedding);
        self
    }
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
    /// Clusters holding exactly one utterance.
    ///
    /// Reported because a single stray utterance is the shape a false speaker
    /// takes: agglomerative merging joins everything else cheaply and leaves
    /// the outlier to the end, where it can clear the split gate on its own. A
    /// person who spoke once is also real, so this is surfaced rather than
    /// suppressed — the UI can say "heard once" instead of Relay guessing.
    pub singleton_cluster_count: usize,
    /// How well the chosen partition actually describes the recording, as a
    /// mean silhouette in `-1.0..=1.0`.
    ///
    /// This is the number the speaker count was decided on, so it is the number
    /// to look at when a roster is wrong. Zero means the answer was one
    /// speaker, where a silhouette is undefined.
    pub silhouette: f32,
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
        if self.cluster_count <= 1 {
            // Nothing was split, so there is no separation to be wrong about.
            return true;
        }
        if self.singleton_cluster_count > 0 && self.cluster_count > 2 {
            // A roster resting on an utterance heard once is not a confident
            // roster, however cleanly the rest of the partition scores.
            return false;
        }
        self.silhouette >= CONFIDENT_SILHOUETTE
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
/// difference, so a distance can be compared against the within-speaker scatter
/// of the same recording in the same units.
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

/// Distance between two voices, in the same units the clusterer uses.
///
/// Exposed because every consumer that has to decide "is this the same person"
/// — the incremental engine, the fixtures, the diagnostics — must measure it
/// exactly as the clusterer does, or their thresholds mean different things.
pub fn feature_distance(a: &VoiceFeatures, b: &VoiceFeatures) -> f32 {
    distance(&weighted(a), &weighted(b))
}

/// The weighted comparison vector for a voice.
///
/// Public so the incremental registry compares in exactly the same space the
/// global clusterer does. Two passes measuring in different units would make
/// their thresholds incomparable and their disagreements unreadable.
pub fn weighted_vector(features: &VoiceFeatures) -> Vec<f32> {
    weighted(features)
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
    let use_embeddings = !utterances.is_empty()
        && utterances.iter().all(|u| {
            u.embedding
                .as_ref()
                .is_some_and(|e| e.provider != "acoustic-spectral-v2")
        });

    let is_usable_fn = |u: &Utterance| -> bool {
        if use_embeddings {
            u.embedding.as_ref().is_some_and(|e| e.quality >= 0.15)
        } else {
            u.features.is_usable()
        }
    };

    let usable: Vec<usize> = utterances
        .iter()
        .enumerate()
        .filter(|(_, u)| is_usable_fn(u))
        .map(|(i, _)| i)
        .collect();

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
            unplaced_count: utterances.len(),
            singleton_cluster_count: 0,
            silhouette: 0.0,
        };
    }

    // Core selection: utterances with duration >= 1.2s provide the stable cluster centroids.
    // Short interjections (< 1.2s) will be projected onto established centroids to prevent
    // premature merge distortion or false singletons.
    let core_usable: Vec<usize> = usable
        .iter()
        .copied()
        .filter(|&i| (utterances[i].end_time_s - utterances[i].start_time_s) >= 1.2)
        .collect();

    let cluster_usable = if core_usable.len() >= 2 {
        core_usable
    } else {
        usable.clone()
    };

    let vectors: Vec<Vec<f32>> = if use_embeddings {
        cluster_usable
            .iter()
            .map(|&i| utterances[i].embedding.as_ref().unwrap().vector.clone())
            .collect()
    } else {
        cluster_usable
            .iter()
            .map(|&i| weighted(&utterances[i].features))
            .collect()
    };

    let dist_fn = |a: &[f32], b: &[f32]| -> f32 {
        if use_embeddings {
            (1.0 - cosine_similarity(a, b)).max(0.0)
        } else {
            distance(a, b)
        }
    };

    let distances = distance_matrix_with(&vectors, dist_fn);
    let (snapshots, _) = merge_all_with(&vectors, dist_fn);
    let chosen = choose_cluster_count(&snapshots, &distances, vectors.len(), expected_speakers);

    let mut members = snapshots
        .get(chosen.k.saturating_sub(1))
        .cloned()
        .unwrap_or_else(|| vec![(0..vectors.len()).collect()]);

    members.sort_by(|left, right| {
        let first = |group: &Vec<usize>| {
            group
                .iter()
                .map(|&i| utterances[cluster_usable[i]].start_time_s)
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
            let d = dist_fn(&vectors[local], &centroids[cluster_index]);
            within_total += d;
            within_count += 1;
            let original = cluster_usable[local];
            assignments[original] = Assignment {
                id: utterances[original].id.clone(),
                cluster: Some(cluster_index),
                distance: d,
            };
        }
    }

    // Two-stage projection for short utterances:
    for &idx in &usable {
        if assignments[idx].cluster.is_none() && !centroids.is_empty() {
            let u_vec = if use_embeddings {
                utterances[idx].embedding.as_ref().map(|e| e.vector.clone())
            } else {
                Some(weighted(&utterances[idx].features))
            };

            if let Some(ref v) = u_vec {
                let mut best_cluster = None;
                let mut min_d = f32::MAX;
                for (c_idx, c_vec) in centroids.iter().enumerate() {
                    let d = dist_fn(v, c_vec);
                    if d < min_d {
                        min_d = d;
                        best_cluster = Some(c_idx);
                    }
                }

                let max_threshold = if use_embeddings { 0.35 } else { 0.65 };
                if min_d <= max_threshold {
                    if let Some(c) = best_cluster {
                        assignments[idx] = Assignment {
                            id: utterances[idx].id.clone(),
                            cluster: Some(c),
                            distance: min_d,
                        };
                        within_total += min_d;
                        within_count += 1;
                    }
                }
            }
        }
    }

    let mut min_between = f32::MAX;
    for i in 0..centroids.len() {
        for j in i + 1..centroids.len() {
            min_between = min_between.min(dist_fn(&centroids[i], &centroids[j]));
        }
    }

    let final_placed = assignments.iter().filter(|a| a.cluster.is_some()).count();
    let final_unplaced = utterances.len().saturating_sub(final_placed);

    Clustering {
        singleton_cluster_count: members.iter().filter(|g| g.len() == 1).count(),
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
        unplaced_count: final_unplaced,
        silhouette: chosen.silhouette,
    }
}

type MergeTrace = (Vec<Vec<Vec<usize>>>, Vec<f32>);

#[cfg(test)]
fn merge_all(vectors: &[Vec<f32>]) -> MergeTrace {
    merge_all_with(vectors, distance)
}

fn merge_all_with<F>(vectors: &[Vec<f32>], dist_fn: F) -> MergeTrace
where
    F: Fn(&[f32], &[f32]) -> f32 + Copy,
{
    let mut members: Vec<Vec<usize>> = (0..vectors.len()).map(|i| vec![i]).collect();
    let mut snapshots: Vec<Vec<Vec<usize>>> = vec![Vec::new(); vectors.len()];
    let mut merge_distances: Vec<f32> = Vec::new();

    snapshots[members.len() - 1] = members.clone();

    while members.len() > 1 {
        let Some((a, b, best)) = closest_pair_with(&members, vectors, dist_fn) else {
            break;
        };
        let merged = members.remove(b);
        members[a].extend(merged);
        merge_distances.push(best);
        snapshots[members.len() - 1] = members.clone();
    }

    (snapshots, merge_distances)
}

fn closest_pair_with<F>(
    members: &[Vec<usize>],
    vectors: &[Vec<f32>],
    dist_fn: F,
) -> Option<(usize, usize, f32)>
where
    F: Fn(&[f32], &[f32]) -> f32 + Copy,
{
    let mut best_a = 0usize;
    let mut best_b = 0usize;
    let mut best_dist = f32::MAX;

    for i in 0..members.len() {
        for j in i + 1..members.len() {
            let d = average_linkage_distance_with(&members[i], &members[j], vectors, dist_fn);
            if d < best_dist {
                best_dist = d;
                best_a = i;
                best_b = j;
            }
        }
    }

    (best_dist != f32::MAX).then_some((best_a, best_b, best_dist))
}

fn average_linkage_distance_with<F>(
    a: &[usize],
    b: &[usize],
    vectors: &[Vec<f32>],
    dist_fn: F,
) -> f32
where
    F: Fn(&[f32], &[f32]) -> f32 + Copy,
{
    if a.is_empty() || b.is_empty() {
        return f32::MAX;
    }
    let mut total = 0.0f64;
    for &i in a {
        for &j in b {
            total += dist_fn(&vectors[i], &vectors[j]) as f64;
        }
    }
    (total / (a.len() * b.len()) as f64) as f32
}

fn distance_matrix_with<F>(vectors: &[Vec<f32>], dist_fn: F) -> Vec<Vec<f32>>
where
    F: Fn(&[f32], &[f32]) -> f32 + Copy,
{
    let n = vectors.len();
    let mut matrix = vec![vec![0.0f32; n]; n];
    for i in 0..n {
        for j in i + 1..n {
            let d = dist_fn(&vectors[i], &vectors[j]);
            matrix[i][j] = d;
            matrix[j][i] = d;
        }
    }
    matrix
}

/// Mean silhouette of one partition, over a precomputed distance matrix.
///
/// For each point: `a` is its mean distance to the rest of its own cluster and
/// `b` the mean distance to the nearest other cluster; the score is
/// `(b - a) / max(a, b)`, which is 1 when a point sits squarely inside a
/// well-separated group and 0 or below when it does not belong where it is.
///
/// A point alone in its cluster scores 0 by convention, and that convention is
/// load-bearing here: it is what stops one stray utterance from being promoted
/// to a speaker, because doing so drags the whole partition's score down rather
/// than leaving it untouched.
fn mean_silhouette(members: &[Vec<usize>], distances: &[Vec<f32>]) -> f32 {
    if members.len() < 2 {
        return 0.0;
    }

    let mut total = 0.0f64;
    let mut counted = 0usize;

    for (own_index, own) in members.iter().enumerate() {
        for &point in own {
            if own.len() <= 1 {
                counted += 1;
                continue;
            }

            let a: f32 = own
                .iter()
                .filter(|&&other| other != point)
                .map(|&other| distances[point][other])
                .sum::<f32>()
                / (own.len() - 1) as f32;

            let b = members
                .iter()
                .enumerate()
                .filter(|(index, group)| *index != own_index && !group.is_empty())
                .map(|(_, group)| {
                    group.iter().map(|&other| distances[point][other]).sum::<f32>()
                        / group.len() as f32
                })
                .fold(f32::MAX, f32::min);

            if b == f32::MAX {
                counted += 1;
                continue;
            }
            let denominator = a.max(b);
            if denominator > 0.0 {
                total += ((b - a) / denominator) as f64;
            }
            counted += 1;
        }
    }

    if counted == 0 {
        return 0.0;
    }
    (total / counted as f64) as f32
}

/// Every pairwise distance, computed once.
#[allow(dead_code)]
fn distance_matrix(vectors: &[Vec<f32>]) -> Vec<Vec<f32>> {
    let n = vectors.len();
    let mut matrix = vec![vec![0.0f32; n]; n];
    for i in 0..n {
        for j in i + 1..n {
            let d = distance(&vectors[i], &vectors[j]);
            matrix[i][j] = d;
            matrix[j][i] = d;
        }
    }
    matrix
}

/// How many speakers the recording supports, and how clearly.
struct SpeakerCount {
    k: usize,
    /// Mean silhouette of the chosen partition. Zero when the answer is one
    /// speaker, where a silhouette is undefined.
    silhouette: f32,
}

/// Chooses how many speakers there were by scoring each candidate partition.
///
/// Walks every cut of the merge tree from two clusters up to the ceiling,
/// scores each, and keeps the best. A partition that scores below
/// [`MIN_SILHOUETTE_TO_SPLIT`] is not describing separate voices, so the answer
/// is one speaker — which is how a single person talking for an hour stays one
/// person even though their voice wanders.
fn choose_cluster_count(
    snapshots: &[Vec<Vec<usize>>],
    distances: &[Vec<f32>],
    point_count: usize,
    expected_speakers: Option<usize>,
) -> SpeakerCount {
    if point_count == 0 {
        return SpeakerCount { k: 0, silhouette: 0.0 };
    }
    if let Some(expected) = expected_speakers.filter(|&n| n > 0) {
        let k = expected.min(point_count);
        let silhouette = snapshots
            .get(k.saturating_sub(1))
            .map(|members| mean_silhouette(members, distances))
            .unwrap_or(0.0);
        return SpeakerCount { k, silhouette };
    }

    let ceiling = MAX_DISCOVERED_SPEAKERS.min(point_count);
    let mut scored: Vec<SpeakerCount> = Vec::new();

    for k in 2..=ceiling {
        let Some(members) = snapshots.get(k - 1) else {
            continue;
        };
        if members.len() != k {
            continue;
        }
        scored.push(SpeakerCount {
            k,
            silhouette: mean_silhouette(members, distances),
        });
    }

    let Some(best) = scored
        .iter()
        .max_by(|a, b| {
            a.silhouette
                .partial_cmp(&b.silhouette)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|found| SpeakerCount {
            k: found.k,
            silhouette: found.silhouette,
        })
    else {
        return SpeakerCount { k: 1, silhouette: 0.0 };
    };

    if best.silhouette >= MIN_SILHOUETTE_TO_SPLIT {
        return best;
    }

    // Scoring below the bar normally means there is one voice here. It means
    // something else when the best score sits at the ceiling and is still
    // rising: the recording holds more distinct voices than the search was
    // allowed to consider, and every partition it *could* consider merges some
    // of them. Collapsing to one speaker there would be the failure this module
    // was rebuilt to fix, reappearing through the cap.
    let still_rising = best.k == ceiling
        && scored
            .iter()
            .find(|c| c.k + 1 == ceiling)
            .is_some_and(|previous| best.silhouette > previous.silhouette);

    if still_rising {
        return best;
    }

    SpeakerCount { k: 1, silhouette: 0.0 }
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
    use crate::meetings_v2::diarize::fixtures;

    // -----------------------------------------------------------------------
    // Against voices that behave like real ones.
    //
    // These are the tests that would have caught the shipped bug. The suite
    // they replace used synthetic voices an octave apart, which every
    // threshold separates; a meeting of twenty real people reported one
    // speaker and nothing here went red.
    // -----------------------------------------------------------------------

    #[test]
    fn three_real_voices_are_separated_into_three_speakers() {
        let meeting = fixtures::interleaved_meeting(&fixtures::THREE_SPEAKERS, 3);
        let clustering = cluster(&meeting, None);

        assert_eq!(
            clustering.cluster_count, 3,
            "silhouette {:.3}, within {:.3}, between {:.3}",
            clustering.silhouette,
            clustering.mean_within_distance,
            clustering.min_between_distance
        );
        assert!(
            fixtures::partition_matches(&clustering.assignments, &fixtures::truth(&meeting)),
            "every turn of one voice must land together"
        );
        assert!(clustering.is_well_separated());
        assert_eq!(clustering.singleton_cluster_count, 0);
    }

    #[test]
    fn one_real_voice_stays_one_speaker() {
        // The failure in the other direction, and the worse one: a split
        // invents somebody who was never in the room.
        let meeting = fixtures::interleaved_meeting(&fixtures::THREE_SPEAKERS[..1], 6);
        let clustering = cluster(&meeting, None);
        assert_eq!(
            clustering.cluster_count, 1,
            "silhouette {:.3}",
            clustering.silhouette
        );
    }

    #[test]
    fn two_similar_voices_merge_rather_than_risk_inventing_a_speaker() {
        // A limitation, recorded as a test so it cannot quietly change.
        //
        // Two voices this close score the same as one voice that wanders across
        // a meeting, so no threshold tells them apart with cepstral features.
        // Given the choice, Relay merges: "Speaker 2 said both of these" is
        // wrong but legible and the user can force the split, whereas inventing
        // a person puts their name on somebody else's commitments.
        let meeting = fixtures::interleaved_meeting(&fixtures::TWO_SIMILAR_SPEAKERS, 3);
        let clustering = cluster(&meeting, None);
        assert_eq!(
            clustering.cluster_count, 1,
            "silhouette {:.3}",
            clustering.silhouette
        );
    }

    #[test]
    fn the_expected_count_recovers_a_split_the_audio_could_not_justify() {
        // The escape hatch that makes the merge above acceptable: a user who
        // knows there were two people says so, and gets two.
        let meeting = fixtures::interleaved_meeting(&fixtures::TWO_SIMILAR_SPEAKERS, 3);
        let clustering = cluster(&meeting, Some(2));

        assert_eq!(clustering.cluster_count, 2);
        assert!(
            fixtures::partition_matches(&clustering.assignments, &fixtures::truth(&meeting)),
            "and the split it produces is the correct one"
        );
        assert!(
            !clustering.is_well_separated(),
            "a split the audio did not justify on its own must not claim confidence"
        );
    }

    #[test]
    fn a_clean_split_is_presented_as_fact_and_a_forced_one_is_not() {
        let distinct = cluster(
            &fixtures::interleaved_meeting(&fixtures::THREE_SPEAKERS, 3),
            None,
        );
        assert!(
            distinct.is_well_separated(),
            "silhouette {:.3}",
            distinct.silhouette
        );

        let forced = cluster(
            &fixtures::interleaved_meeting(&fixtures::TWO_SIMILAR_SPEAKERS, 3),
            Some(2),
        );
        assert!(!forced.is_well_separated());
    }

    #[test]
    fn a_two_person_call_is_separated() {
        // The commonest meeting there is, and the one the shipped build turned
        // into a single "Speaker 1".
        let meeting = fixtures::interleaved_meeting(&fixtures::THREE_SPEAKERS[..2], 4);
        let clustering = cluster(&meeting, None);
        assert_eq!(clustering.cluster_count, 2);
        assert!(fixtures::partition_matches(
            &clustering.assignments,
            &fixtures::truth(&meeting)
        ));
    }

    #[test]
    fn a_speaker_who_spoke_once_is_found_but_reported_as_thin_evidence() {
        let mut meeting = fixtures::interleaved_meeting(&fixtures::THREE_SPEAKERS[..2], 3);
        meeting.push(fixtures::utterance(&fixtures::THREE_SPEAKERS[2], 0, 100.0, 3.0));

        let clustering = cluster(&meeting, None);
        assert_eq!(clustering.cluster_count, 3);
        assert_eq!(clustering.singleton_cluster_count, 1);
        assert!(
            !clustering.is_well_separated(),
            "a roster resting on one utterance is not a confident roster"
        );
    }

    #[test]
    fn speaker_numbers_follow_the_order_people_first_spoke() {
        let meeting = fixtures::interleaved_meeting(&fixtures::THREE_SPEAKERS, 2);
        let clustering = cluster(&meeting, None);
        let first = clustering
            .assignments
            .iter()
            .find(|a| a.id == "A0")
            .unwrap();
        assert_eq!(first.cluster, Some(0));
    }

    #[test]
    fn an_expected_count_is_honoured_over_what_the_audio_suggests() {
        let meeting = fixtures::interleaved_meeting(&fixtures::THREE_SPEAKERS, 3);
        assert_eq!(cluster(&meeting, Some(2)).cluster_count, 2);
        assert_eq!(cluster(&meeting, Some(3)).cluster_count, 3);
    }

    #[test]
    fn an_expected_count_cannot_invent_a_speaker_the_audio_lacks() {
        // Twenty in the room, three on the recording, is still three.
        let meeting = fixtures::interleaved_meeting(&fixtures::THREE_SPEAKERS, 2);
        let clustering = cluster(&meeting, Some(20));
        assert!(clustering.cluster_count <= meeting.len());
    }

    // -----------------------------------------------------------------------
    // Choosing the speaker count.
    //
    // The criterion is scale-free, so these test it on the fixtures rather than
    // on hand-written distance sequences. Two previous designs passed
    // hand-written sequences and failed on real recordings, which is the
    // argument for testing it this way.
    // -----------------------------------------------------------------------

    #[test]
    fn one_voice_that_wanders_across_a_meeting_stays_one_voice() {
        // Measured through the recording path, one person's utterances span
        // 0.031 to 0.422 over a meeting — four times what the cheapest merges
        // suggest. The design this replaces read that spread as three speakers.
        let mut meeting = Vec::new();
        for turn in 0..8 {
            meeting.push(fixtures::utterance(
                &fixtures::THREE_SPEAKERS[0],
                turn,
                turn as f64 * 10.0,
                3.0,
            ));
        }
        let clustering = cluster(&meeting, None);
        assert_eq!(
            clustering.cluster_count, 1,
            "silhouette {:.3}",
            clustering.silhouette
        );
    }

    #[test]
    fn the_silhouette_is_reported_so_a_wrong_roster_is_diagnosable() {
        let distinct = cluster(
            &fixtures::interleaved_meeting(&fixtures::THREE_SPEAKERS, 3),
            None,
        );
        assert!(
            distinct.silhouette >= CONFIDENT_SILHOUETTE,
            "three clearly different voices should score strongly, got {:.3}",
            distinct.silhouette
        );

        let single = cluster(
            &fixtures::interleaved_meeting(&fixtures::THREE_SPEAKERS[..1], 6),
            None,
        );
        assert_eq!(
            single.silhouette, 0.0,
            "a silhouette is undefined for one cluster and must not be invented"
        );
    }

    #[test]
    fn a_partition_that_describes_nothing_is_rejected_in_favour_of_one_speaker() {
        // Direct test of the gate: scoring below the bar means the split is not
        // describing separate voices, whatever the distances happen to be.
        let meeting = fixtures::interleaved_meeting(&fixtures::THREE_SPEAKERS[..1], 6);
        let clustering = cluster(&meeting, None);
        assert_eq!(clustering.cluster_count, 1);
        assert!(clustering.is_well_separated());
    }

    #[test]
    fn an_expected_count_is_honoured_even_when_it_scores_poorly() {
        // The user said two. Overruling them because the audio disagrees would
        // make the setting pointless.
        let meeting = fixtures::interleaved_meeting(&fixtures::THREE_SPEAKERS[..1], 6);
        assert_eq!(cluster(&meeting, Some(2)).cluster_count, 2);
    }

    #[test]
    fn a_meeting_of_several_distinct_voices_finds_all_of_them() {
        let voices = fixtures::distinct_voices(6);
        let meeting = fixtures::interleaved_meeting(&voices, 3);
        let clustering = cluster(&meeting, None);

        assert_eq!(
            clustering.cluster_count,
            voices.len(),
            "silhouette {:.3}",
            clustering.silhouette
        );
        assert!(fixtures::partition_matches(
            &clustering.assignments,
            &fixtures::truth(&meeting)
        ));
    }

    #[test]
    fn more_voices_than_the_search_can_consider_are_capped_not_collapsed() {
        // The ceiling must not reintroduce the failure this module was rebuilt
        // to fix. When the best partition sits at the ceiling and is still
        // improving, the recording holds more voices than the search was
        // allowed to look for — which is the opposite of holding one.
        //
        // Driven from vectors through the real merge tree rather than from
        // fixture audio, because synthesizing twenty-odd voices this feature
        // space can tell apart is not possible, and a fixture pretending
        // otherwise would be testing a capability Relay does not have.
        let group_count = MAX_DISCOVERED_SPEAKERS + 3;
        let mut vectors: Vec<Vec<f32>> = Vec::new();
        for group in 0..group_count {
            // Two near-identical points per group, groups far apart.
            for nudge in [0.0f32, 0.02] {
                let mut v = vec![0.0f32; 8];
                v[group % 8] = group as f32 * 10.0 + nudge;
                v[(group + 3) % 8] = group as f32 * 4.0;
                vectors.push(v);
            }
        }

        let distances = distance_matrix(&vectors);
        let (snapshots, _) = merge_all(&vectors);
        let chosen = choose_cluster_count(&snapshots, &distances, vectors.len(), None);

        assert!(
            chosen.k > 1,
            "a recording of many distinct voices must not read as one (k={}, \
silhouette {:.3})",
            chosen.k,
            chosen.silhouette
        );
        assert!(chosen.k <= MAX_DISCOVERED_SPEAKERS, "got {}", chosen.k);
    }

    #[test]
    fn the_silhouette_of_a_single_cluster_partition_is_zero() {
        let distances = vec![vec![0.0, 0.2], vec![0.2, 0.0]];
        assert_eq!(mean_silhouette(&[vec![0, 1]], &distances), 0.0);
    }

    #[test]
    fn a_singleton_cluster_drags_the_score_down_rather_than_being_free() {
        // The convention that stops one stray utterance becoming a speaker.
        let distances = vec![
            vec![0.0, 0.1, 0.1, 1.0],
            vec![0.1, 0.0, 0.1, 1.0],
            vec![0.1, 0.1, 0.0, 1.0],
            vec![1.0, 1.0, 1.0, 0.0],
        ];
        let together = mean_silhouette(&[vec![0, 1, 2], vec![3]], &distances);
        let split_evenly = mean_silhouette(&[vec![0, 1], vec![2, 3]], &distances);
        assert!(
            together < 1.0,
            "the singleton must not score a free 1.0, got {together:.3}"
        );
        assert!(together > split_evenly);
    }

    #[test]
    fn the_distance_matrix_is_symmetric_with_a_zero_diagonal() {
        let vectors = vec![vec![1.0, 2.0], vec![3.0, 1.0], vec![0.0, 0.0]];
        let matrix = distance_matrix(&vectors);
        for (i, row) in matrix.iter().enumerate() {
            assert_eq!(row[i], 0.0);
            for (j, &value) in row.iter().enumerate() {
                assert_eq!(value, matrix[j][i]);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Distance and degenerate inputs.
    // -----------------------------------------------------------------------

    #[test]
    fn the_distance_is_a_per_dimension_rms_difference() {
        assert_eq!(distance(&[1.0, 0.0], &[1.0, 0.0]), 0.0);
        assert!((distance(&[0.0, 0.0], &[2.0, 2.0]) - 2.0).abs() < 1e-5);
        assert_eq!(distance(&[], &[]), 0.0);
    }

    #[test]
    fn a_shared_channel_component_cancels_out_of_the_distance() {
        // The room, the microphone and the codec are the same for everyone in
        // one recording, so they must not affect who looks like whom.
        let bare = [vec![1.0f32, 0.0], vec![-1.0f32, 0.5]];
        let offset = [vec![101.0f32, 50.0], vec![99.0f32, 50.5]];
        assert!((distance(&bare[0], &bare[1]) - distance(&offset[0], &offset[1])).abs() < 1e-4);
    }

    #[test]
    fn an_empty_meeting_clusters_to_nothing() {
        let clustering = cluster(&[], None);
        assert_eq!(clustering.cluster_count, 0);
        assert!(clustering.assignments.is_empty());
        assert!(clustering.is_well_separated());
    }

    #[test]
    fn a_stretch_with_too_little_voice_is_left_unplaced() {
        let mut meeting = fixtures::interleaved_meeting(&fixtures::THREE_SPEAKERS, 2);
        meeting.push(Utterance {
            id: "noise".into(),
            start_time_s: 500.0,
            end_time_s: 501.0,
            features: VoiceFeatures {
                mfcc_mean: vec![0.0; 13],
                mfcc_std: vec![0.0; 13],
                pitch_hz: None,
                voiced_fraction: 0.0,
                frame_count: 5,
            },
            embedding: None,
        });

        let clustering = cluster(&meeting, None);
        let noise = clustering.assignments.iter().find(|a| a.id == "noise").unwrap();
        assert_eq!(noise.cluster, None);
        assert_eq!(clustering.unplaced_count, 1);
    }
}
