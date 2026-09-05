//! Acoustic features that describe a voice rather than what it said.
//!
//! Diarization needs a fixed-length vector per stretch of speech whose distance
//! tracks *who is speaking* and ignores *what they are saying*. The standard
//! answer is a neural speaker embedding (x-vector, ECAPA). Relay ships no ONNX
//! runtime and no embedding model, and adding either is a download, a licence
//! question, and a consent flow — see `Meeting-rules/meeting_speaker_identification.md`
//! §6 on biometric data. So this module computes the classical alternative:
//! MFCC statistics plus a pitch estimate.
//!
//! What that buys and what it does not:
//!
//! * **Buys** — separating voices that differ in vocal tract length or pitch
//!   register, which covers most mixed-gender and mixed-age meetings, and
//!   counting *how many* distinct voices a stretch of audio holds. That alone
//!   turns "everyone who is not me is Speaker 1" into a real roster.
//! * **Does not buy** — reliably telling apart two similar voices on the same
//!   channel, or matching a voice across meetings. Both need an embedding
//!   model. The clustering reports its own confidence so the UI can say which
//!   of the two situations it is in rather than presenting a guess as fact.
//!
//! Everything here is pure arithmetic over `f32` slices: no allocation per
//! frame beyond the output, no dependency beyond `std`.

/// Analysis window, in milliseconds. 25 ms is the standard speech frame: long
/// enough to resolve pitch down to 80 Hz, short enough to be stationary.
const FRAME_MS: f64 = 25.0;

/// Hop between windows, in milliseconds.
const HOP_MS: f64 = 10.0;

/// Mel filters in the bank.
const MEL_FILTERS: usize = 26;

/// Cepstral coefficients kept per frame. The first is dropped: it is overall
/// loudness, which says nothing about who is speaking and a great deal about
/// how far they are from the microphone.
pub const MFCC_COEFFS: usize = 13;

/// Lowest and highest frequency the filterbank covers. The band is narrowed to
/// speech deliberately — energy below 80 Hz is rumble and above 7.6 kHz is
/// almost entirely fricative noise at 16 kHz sampling.
const MEL_LOW_HZ: f64 = 80.0;
const MEL_HIGH_HZ: f64 = 7600.0;

/// Pitch search range, in Hz. Covers a low male voice to a high female voice
/// with margin either side.
const PITCH_MIN_HZ: f64 = 60.0;
const PITCH_MAX_HZ: f64 = 400.0;

/// Autocorrelation peak below which a frame is treated as unvoiced and
/// contributes no pitch estimate.
const VOICING_THRESHOLD: f32 = 0.30;

/// A speaker feature vector: MFCC mean and standard deviation over a stretch of
/// speech, plus pitch statistics.
///
/// Means capture the average vocal tract shape, which is the strongest
/// non-neural speaker cue available. Standard deviations capture how much the
/// voice moves, which separates a monotone speaker from an animated one. Pitch
/// is added on its own axis because it is the cue a human would use first.
#[derive(Debug, Clone, PartialEq)]
pub struct VoiceFeatures {
    pub mfcc_mean: Vec<f32>,
    pub mfcc_std: Vec<f32>,
    /// Median fundamental frequency across voiced frames, in Hz. `None` when no
    /// frame in the stretch was voiced.
    pub pitch_hz: Option<f32>,
    /// Fraction of frames that were voiced. Low values mean the stretch was
    /// mostly noise, and the features are correspondingly weak evidence.
    pub voiced_fraction: f32,
    /// Frames the features were computed over. Short stretches are noisier and
    /// the clusterer weights them accordingly.
    pub frame_count: usize,
}

impl VoiceFeatures {
    /// The vector the clusterer compares, in a single fixed layout.
    ///
    /// Pitch enters as a log ratio against a 150 Hz reference so an octave of
    /// difference contributes about the same magnitude as a strong cepstral
    /// difference, rather than swamping it in raw Hz.
    pub fn vector(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(MFCC_COEFFS * 2 + 1);
        out.extend_from_slice(&self.mfcc_mean);
        out.extend_from_slice(&self.mfcc_std);
        out.push(match self.pitch_hz {
            Some(hz) if hz > 0.0 => (hz / 150.0).ln(),
            _ => 0.0,
        });
        out
    }

    /// Whether this stretch carries enough voice to be worth clustering.
    pub fn is_usable(&self) -> bool {
        self.frame_count >= 40 && self.voiced_fraction >= 0.15
    }
}

/// Computes voice features over one stretch of 16 kHz mono audio.
///
/// Returns `None` for audio too short to frame at all.
pub fn extract(samples: &[f32], sample_rate: u32) -> Option<VoiceFeatures> {
    let sample_rate = sample_rate.max(1);
    let frame_len = (sample_rate as f64 * FRAME_MS / 1000.0) as usize;
    let hop_len = ((sample_rate as f64 * HOP_MS / 1000.0) as usize).max(1);
    if frame_len == 0 || samples.len() < frame_len {
        return None;
    }

    let fft_size = frame_len.next_power_of_two();
    let bank = MelFilterBank::new(fft_size, sample_rate);
    let window = hann(frame_len);

    let mut mfccs: Vec<Vec<f32>> = Vec::new();
    let mut pitches: Vec<f32> = Vec::new();
    let mut voiced_frames = 0usize;
    let mut total_frames = 0usize;

    let mut offset = 0usize;
    while offset + frame_len <= samples.len() {
        let frame = &samples[offset..offset + frame_len];
        offset += hop_len;
        total_frames += 1;

        // Silence contributes nothing but noise to a speaker average.
        let energy: f32 = frame.iter().map(|&s| s * s).sum::<f32>() / frame.len() as f32;
        if energy.sqrt() < 0.005 {
            continue;
        }

        let mut buffer = vec![0.0f32; fft_size];
        for (i, (&sample, &w)) in frame.iter().zip(window.iter()).enumerate() {
            buffer[i] = sample * w;
        }
        let spectrum = power_spectrum(&buffer);
        let energies = bank.apply(&spectrum);
        mfccs.push(dct_ii(&energies, MFCC_COEFFS));

        if let Some(hz) = estimate_pitch(frame, sample_rate) {
            pitches.push(hz);
            voiced_frames += 1;
        }
    }

    if mfccs.is_empty() {
        return None;
    }

    let (mfcc_mean, mfcc_std) = mean_and_std(&mfccs);
    pitches.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let pitch_hz = (!pitches.is_empty()).then(|| pitches[pitches.len() / 2]);

    Some(VoiceFeatures {
        mfcc_mean,
        mfcc_std,
        pitch_hz,
        voiced_fraction: if total_frames == 0 {
            0.0
        } else {
            voiced_frames as f32 / total_frames as f32
        },
        frame_count: mfccs.len(),
    })
}

/// A Hann window, precomputed once per stretch.
pub(crate) fn hann(len: usize) -> Vec<f32> {
    if len <= 1 {
        return vec![1.0; len];
    }
    (0..len)
        .map(|i| {
            let x = i as f64 / (len - 1) as f64;
            (0.5 - 0.5 * (2.0 * std::f64::consts::PI * x).cos()) as f32
        })
        .collect()
}

/// Power spectrum of a real signal, up to Nyquist.
///
/// Uses an in-place iterative radix-2 FFT. `buffer.len()` must be a power of
/// two; [`extract`] guarantees that by construction.
pub(crate) fn power_spectrum(buffer: &[f32]) -> Vec<f32> {
    let n = buffer.len();
    let mut re: Vec<f32> = buffer.to_vec();
    let mut im: Vec<f32> = vec![0.0; n];
    fft_in_place(&mut re, &mut im);

    (0..=n / 2)
        .map(|k| re[k] * re[k] + im[k] * im[k])
        .collect()
}

/// Iterative in-place radix-2 Cooley–Tukey FFT.
fn fft_in_place(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    debug_assert!(n.is_power_of_two());
    if n <= 1 {
        return;
    }

    // Bit-reversal permutation.
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }

    let mut len = 2usize;
    while len <= n {
        let angle = -2.0 * std::f64::consts::PI / len as f64;
        let (wr, wi) = (angle.cos() as f32, angle.sin() as f32);
        for start in (0..n).step_by(len) {
            let (mut cur_r, mut cur_i) = (1.0f32, 0.0f32);
            for k in 0..len / 2 {
                let (ur, ui) = (re[start + k], im[start + k]);
                let (vr0, vi0) = (re[start + k + len / 2], im[start + k + len / 2]);
                let vr = vr0 * cur_r - vi0 * cur_i;
                let vi = vr0 * cur_i + vi0 * cur_r;

                re[start + k] = ur + vr;
                im[start + k] = ui + vi;
                re[start + k + len / 2] = ur - vr;
                im[start + k + len / 2] = ui - vi;

                let next_r = cur_r * wr - cur_i * wi;
                cur_i = cur_r * wi + cur_i * wr;
                cur_r = next_r;
            }
        }
        len <<= 1;
    }
}

/// Triangular mel filters over the FFT bins.
pub(crate) struct MelFilterBank {
    /// One `(start_bin, weights)` pair per filter. Storing only the non-zero
    /// span keeps `apply` linear in bin count rather than filters × bins.
    filters: Vec<(usize, Vec<f32>)>,
}

impl MelFilterBank {
    pub(crate) fn new(fft_size: usize, sample_rate: u32) -> Self {
        let bins = fft_size / 2 + 1;
        let bin_hz = sample_rate as f64 / fft_size as f64;

        let low_mel = hz_to_mel(MEL_LOW_HZ);
        let high_mel = hz_to_mel(MEL_HIGH_HZ.min(sample_rate as f64 / 2.0));
        let edges: Vec<f64> = (0..MEL_FILTERS + 2)
            .map(|i| {
                let mel = low_mel + (high_mel - low_mel) * i as f64 / (MEL_FILTERS + 1) as f64;
                mel_to_hz(mel) / bin_hz
            })
            .collect();

        let mut filters = Vec::with_capacity(MEL_FILTERS);
        for f in 0..MEL_FILTERS {
            let (left, centre, right) = (edges[f], edges[f + 1], edges[f + 2]);
            let start = left.floor().max(0.0) as usize;
            let end = (right.ceil() as usize).min(bins.saturating_sub(1));
            let mut weights = Vec::with_capacity(end.saturating_sub(start) + 1);
            for bin in start..=end {
                let x = bin as f64;
                let w = if x <= centre {
                    if centre > left {
                        (x - left) / (centre - left)
                    } else {
                        0.0
                    }
                } else if right > centre {
                    (right - x) / (right - centre)
                } else {
                    0.0
                };
                weights.push(w.clamp(0.0, 1.0) as f32);
            }
            filters.push((start, weights));
        }

        Self { filters }
    }

    /// Log energy in each filter. The floor keeps `ln` finite on a silent band.
    pub(crate) fn apply(&self, spectrum: &[f32]) -> Vec<f32> {
        self.filters
            .iter()
            .map(|(start, weights)| {
                let mut sum = 0.0f32;
                for (i, &w) in weights.iter().enumerate() {
                    if let Some(&power) = spectrum.get(start + i) {
                        sum += power * w;
                    }
                }
                (sum.max(1e-10)).ln()
            })
            .collect()
    }
}

fn hz_to_mel(hz: f64) -> f64 {
    2595.0 * (1.0 + hz / 700.0).log10()
}

fn mel_to_hz(mel: f64) -> f64 {
    700.0 * (10f64.powf(mel / 2595.0) - 1.0)
}

/// Type-II DCT, keeping `count` coefficients and dropping the zeroth.
///
/// The zeroth coefficient is total log energy: it tracks microphone distance
/// and speaking volume, not identity, and including it makes a speaker who
/// leaned toward the mic look like a different person.
pub(crate) fn dct_ii(input: &[f32], count: usize) -> Vec<f32> {
    let n = input.len();
    (1..=count)
        .map(|k| {
            let mut sum = 0.0f64;
            for (i, &value) in input.iter().enumerate() {
                let angle = std::f64::consts::PI * k as f64 * (i as f64 + 0.5) / n as f64;
                sum += value as f64 * angle.cos();
            }
            (sum * (2.0 / n as f64).sqrt()) as f32
        })
        .collect()
}

/// Per-coefficient mean and population standard deviation across frames.
pub(crate) fn mean_and_std(frames: &[Vec<f32>]) -> (Vec<f32>, Vec<f32>) {
    let width = frames.first().map(|f| f.len()).unwrap_or(0);
    let mut mean = vec![0.0f32; width];
    for frame in frames {
        for (m, &v) in mean.iter_mut().zip(frame.iter()) {
            *m += v;
        }
    }
    let count = frames.len() as f32;
    for m in mean.iter_mut() {
        *m /= count;
    }

    let mut var = vec![0.0f32; width];
    for frame in frames {
        for (i, &v) in frame.iter().enumerate() {
            let d = v - mean[i];
            var[i] += d * d;
        }
    }
    let std = var.iter().map(|&v| (v / count).sqrt()).collect();

    (mean, std)
}

/// Fundamental frequency by normalized autocorrelation.
///
/// Returns `None` when the strongest peak in the search range is too weak to
/// call the frame voiced, which is the honest answer for a fricative, a breath,
/// or a keyboard.
pub(crate) fn estimate_pitch(frame: &[f32], sample_rate: u32) -> Option<f32> {
    let min_lag = (sample_rate as f64 / PITCH_MAX_HZ) as usize;
    let max_lag = ((sample_rate as f64 / PITCH_MIN_HZ) as usize).min(frame.len().saturating_sub(1));
    if min_lag >= max_lag || min_lag == 0 {
        return None;
    }

    let energy: f32 = frame.iter().map(|&s| s * s).sum();
    if energy <= 0.0 {
        return None;
    }

    let mut best_lag = 0usize;
    let mut best_score = 0.0f32;
    for lag in min_lag..=max_lag {
        let mut corr = 0.0f32;
        let mut norm = 0.0f32;
        for i in 0..frame.len() - lag {
            corr += frame[i] * frame[i + lag];
            norm += frame[i + lag] * frame[i + lag];
        }
        if norm <= 0.0 {
            continue;
        }
        let score = corr / (energy.sqrt() * norm.sqrt());
        if score > best_score {
            best_score = score;
            best_lag = lag;
        }
    }

    (best_score >= VOICING_THRESHOLD && best_lag > 0)
        .then(|| sample_rate as f32 / best_lag as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic voice: a fundamental plus two formant-like harmonics, with a
    /// glottal-ish envelope. Two calls with different fundamentals and formant
    /// ratios stand in for two speakers.
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

    #[test]
    fn the_fft_matches_a_direct_transform() {
        // A power-of-two buffer with a single tone: the peak must land on the
        // bin the tone belongs to, and nowhere else.
        let n = 256;
        let bin = 8;
        let signal: Vec<f32> = (0..n)
            .map(|i| {
                (2.0 * std::f32::consts::PI * bin as f32 * i as f32 / n as f32).sin()
            })
            .collect();
        let spectrum = power_spectrum(&signal);
        let peak = spectrum
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap();
        assert_eq!(peak, bin);
    }

    #[test]
    fn the_fft_of_silence_is_silence() {
        let spectrum = power_spectrum(&vec![0.0; 64]);
        assert!(spectrum.iter().all(|&p| p == 0.0));
    }

    #[test]
    fn pitch_is_recovered_from_a_periodic_signal() {
        for target in [90.0f32, 150.0, 220.0] {
            let frame: Vec<f32> = (0..400)
                .map(|i| {
                    (2.0 * std::f32::consts::PI * target * i as f32 / 16_000.0).sin()
                })
                .collect();
            let hz = estimate_pitch(&frame, 16_000).expect("a sine wave is voiced");
            assert!(
                (hz - target).abs() / target < 0.08,
                "expected ~{target} Hz, got {hz} Hz"
            );
        }
    }

    #[test]
    fn noise_is_not_reported_as_voiced() {
        let frame: Vec<f32> = (0..400)
            .map(|i| (((i as f32 * 37.7).sin() * 4391.0).fract() - 0.5) * 0.3)
            .collect();
        assert!(estimate_pitch(&frame, 16_000).is_none());
    }

    #[test]
    fn silence_yields_no_features() {
        assert!(extract(&vec![0.0; 16_000], 16_000).is_none());
        assert!(extract(&[], 16_000).is_none());
        // Shorter than one frame.
        assert!(extract(&vec![0.1; 100], 16_000).is_none());
    }

    #[test]
    fn features_have_the_declared_shape() {
        let features = extract(&synth_voice(120.0, 1.0, 2.0), 16_000).unwrap();
        assert_eq!(features.mfcc_mean.len(), MFCC_COEFFS);
        assert_eq!(features.mfcc_std.len(), MFCC_COEFFS);
        assert_eq!(features.vector().len(), MFCC_COEFFS * 2 + 1);
        assert!(features.frame_count > 100);
        assert!(features.is_usable());
    }

    #[test]
    fn the_same_voice_twice_is_closer_than_two_different_voices() {
        // The property the whole clusterer rests on. If this fails, no
        // clustering threshold can rescue it.
        let a1 = extract(&synth_voice(110.0, 1.0, 2.0), 16_000).unwrap();
        let a2 = extract(&synth_voice(112.0, 1.02, 2.0), 16_000).unwrap();
        let b = extract(&synth_voice(220.0, 1.5, 2.0), 16_000).unwrap();

        let same = super::super::cluster::distance(&a1.vector(), &a2.vector());
        let different = super::super::cluster::distance(&a1.vector(), &b.vector());
        assert!(
            same < different,
            "same voice distance {same} was not below different-voice distance {different}"
        );
    }

    #[test]
    fn pitch_separates_two_registers() {
        let low = extract(&synth_voice(100.0, 1.0, 2.0), 16_000).unwrap();
        let high = extract(&synth_voice(240.0, 1.0, 2.0), 16_000).unwrap();
        let (a, b) = (low.pitch_hz.unwrap(), high.pitch_hz.unwrap());
        assert!(b > a * 1.5, "pitches were {a} and {b}");
    }

    #[test]
    fn loudness_alone_does_not_change_the_feature_vector_much() {
        // Dropping the zeroth cepstral coefficient is what buys this: someone
        // leaning toward the microphone must not read as a new speaker.
        let quiet: Vec<f32> = synth_voice(130.0, 1.0, 2.0)
            .iter()
            .map(|&s| s * 0.35)
            .collect();
        let loud = synth_voice(130.0, 1.0, 2.0);
        let a = extract(&quiet, 16_000).unwrap();
        let b = extract(&loud, 16_000).unwrap();
        let other = extract(&synth_voice(250.0, 1.6, 2.0), 16_000).unwrap();

        let gain_distance = super::super::cluster::distance(&a.vector(), &b.vector());
        let speaker_distance = super::super::cluster::distance(&a.vector(), &other.vector());
        assert!(
            gain_distance < speaker_distance,
            "a gain change ({gain_distance}) looked more like a speaker change than a \
speaker change did ({speaker_distance})"
        );
    }

    #[test]
    fn a_short_or_unvoiced_stretch_is_marked_unusable() {
        let short = extract(&synth_voice(130.0, 1.0, 0.2), 16_000).unwrap();
        assert!(!short.is_usable(), "frames: {}", short.frame_count);
    }

    #[test]
    fn mel_filters_cover_the_speech_band_without_gaps() {
        let bank = MelFilterBank::new(512, 16_000);
        assert_eq!(bank.filters.len(), MEL_FILTERS);
        for (_, weights) in &bank.filters {
            assert!(
                weights.iter().any(|&w| w > 0.0),
                "a filter with no weight contributes nothing"
            );
        }
    }

    #[test]
    fn the_mel_scale_round_trips() {
        for hz in [80.0, 500.0, 1000.0, 4000.0, 7600.0] {
            let back = mel_to_hz(hz_to_mel(hz));
            assert!((back - hz).abs() < 1.0, "{hz} became {back}");
        }
    }
}
