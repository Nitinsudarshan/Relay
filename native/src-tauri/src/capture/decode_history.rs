//! A retained record of what each Whisper decode actually did.
//!
//! `record_stt_diagnostics` kept the *last* decode, in memory, and dropped it
//! on restart. That is enough to debug the run in front of you and not enough
//! to answer any question about behaviour over time — which is what two open
//! decisions both turn out to need:
//!
//! * **Is a gain stage worth it?** Only if input levels are actually low. That
//!   is a claim about the distribution of `rms` and `peak_amplitude` across
//!   real recordings, not about the last one.
//! * **Does language detection beat pinning to the configured language?** Only
//!   measurable by seeing how pinned decodes fare — and a decode pinned to the
//!   wrong language does not error, it returns a short transcript. Words per
//!   second is what exposes it.
//!
//! Both were listed as blocked on "measure first". Nothing was retaining the
//! measurements.
//!
//! # What is kept, and what is deliberately not
//!
//! Counts and parameters. Never the transcript, and never the initial prompt —
//! the first is the user's speech and the second carries their vocabulary and
//! their colleagues' names. `meetings_v2::processing::qualify` already draws
//! this line for its own diagnostics ("counts only … the half that is safe to
//! persist"); this is the same line in the same place.
//!
//! The consequence worth stating: this file can be read, attached to a bug
//! report, or synced by whatever backs up the config directory, and it still
//! contains nothing anybody said.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::evaluation::SttDiagnosticSnapshot;

/// How many decodes to keep. A few hundred spans weeks of ordinary use and
/// bounds the file at roughly a megabyte.
pub const MAX_RECORDS: usize = 500;

/// One decode, reduced to what is safe to keep.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DecodeRecord {
    pub timestamp_epoch_ms: u128,
    /// `dictation`, `meeting`, and so on — the surface, so the two can be
    /// compared rather than averaged together.
    pub session_mode: String,

    // What the microphone delivered. The AGC question.
    pub audio_seconds: f32,
    pub rms: f32,
    pub peak_amplitude: f32,
    pub near_zero_percent: f32,
    pub noise_floor: f32,
    pub speech_detected: bool,

    // What language resolution decided. The LID question.
    pub primary_language: String,
    pub spoken_languages: Vec<String>,
    /// `None` means Whisper auto-detected; `Some` means it was pinned.
    pub resolved_language: Option<String>,

    // What ran.
    pub model_filename: String,
    pub strategy: String,
    pub beam_size: Option<i32>,

    // What came back — counts only, never the text.
    pub segment_count: usize,
    pub word_count: usize,
    pub char_count: usize,
    /// The starvation signal. Conversation runs 2–4; a decode pinned to the
    /// wrong language collapses towards zero without ever reporting an error.
    pub words_per_second: f32,
    pub real_time_factor: f32,
    pub inference_duration_ms: u128,
    pub failed: bool,
}

impl DecodeRecord {
    /// Reduces a full snapshot to the retainable part.
    pub fn from_snapshot(snapshot: &SttDiagnosticSnapshot) -> Self {
        let word_count = snapshot.transcript.split_whitespace().count();
        let seconds = snapshot.processed_duration_seconds.max(0.0);
        Self {
            timestamp_epoch_ms: snapshot.timestamp_epoch_ms,
            session_mode: snapshot.session_mode.clone(),
            audio_seconds: snapshot.original_duration_seconds,
            rms: snapshot.rms,
            peak_amplitude: snapshot.peak_amplitude,
            near_zero_percent: snapshot.near_zero_percent,
            noise_floor: snapshot.noise_floor,
            speech_detected: snapshot.speech_detected,
            primary_language: snapshot.primary_dictation_language.clone(),
            spoken_languages: snapshot.spoken_languages.clone(),
            resolved_language: snapshot.resolved_whisper_language.clone(),
            model_filename: snapshot.model_filename.clone(),
            strategy: snapshot.strategy.clone(),
            beam_size: snapshot.beam_size,
            segment_count: snapshot.segment_count,
            word_count,
            char_count: snapshot.transcript_char_count,
            words_per_second: if seconds > 0.0 {
                word_count as f32 / seconds
            } else {
                0.0
            },
            real_time_factor: snapshot.real_time_factor,
            inference_duration_ms: snapshot.inference_duration_ms,
            failed: snapshot.error.is_some(),
        }
    }
}

/// Where the history lives, under the config directory Relay already owns.
pub fn history_path(config_dir: &Path) -> PathBuf {
    config_dir.join("diagnostics").join("stt-decodes.jsonl")
}

/// Appends one decode, trimming to [`MAX_RECORDS`].
///
/// Best-effort by design: diagnostics must never be able to fail a capture.
/// A write that does not land costs one row of a chart.
pub fn append(config_dir: &Path, record: &DecodeRecord) {
    let path = history_path(config_dir);
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            tracing::debug!("stt decode history: cannot create {}: {}", parent.display(), e);
            return;
        }
    }

    let mut records = load(config_dir);
    records.push(record.clone());
    let surplus = records.len().saturating_sub(MAX_RECORDS);
    records.drain(..surplus);

    let mut out = String::with_capacity(records.len() * 320);
    for record in &records {
        match serde_json::to_string(record) {
            Ok(line) => {
                out.push_str(&line);
                out.push('\n');
            }
            Err(e) => tracing::debug!("stt decode history: unserializable record: {}", e),
        }
    }
    if let Err(e) = std::fs::write(&path, out) {
        tracing::debug!("stt decode history: cannot write {}: {}", path.display(), e);
    }
}

/// Reads the history, skipping any line that cannot be parsed.
///
/// A record written by an older version is skipped rather than failing the
/// read: this is a diagnostic log, and losing one row to a schema change must
/// not lose the other four hundred.
pub fn load(config_dir: &Path) -> Vec<DecodeRecord> {
    let path = history_path(config_dir);
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    contents
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<DecodeRecord>(line).ok())
        .collect()
}

/// What the history says, in the terms the two open decisions are posed in.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct DecodeSummary {
    pub decodes: usize,
    pub failed: usize,

    /// Input level, as percentiles rather than a mean: a gain stage is worth
    /// building for the quiet tail, and a mean hides it behind the loud ones.
    pub rms_p10: f32,
    pub rms_median: f32,
    pub peak_p10: f32,
    pub peak_median: f32,
    /// Decodes whose peak never reached a quarter of full scale. The AGC
    /// question in one number: if this is near zero, a gain stage is risk
    /// without benefit.
    pub quiet_decodes: usize,

    /// How many decodes were pinned to a language versus auto-detected.
    pub pinned_decodes: usize,
    pub auto_detected_decodes: usize,
    /// Words per second, median, split by whether the language was pinned.
    /// A pinned decode that collapses here is the failure the LID detector
    /// exists to prevent, and this is the comparison that would justify it.
    pub pinned_words_per_second_median: f32,
    pub auto_words_per_second_median: f32,
    /// Decodes that produced speech but under one word per second — the shape
    /// of the transcript that started this whole line of work.
    pub starved_decodes: usize,
}

/// A peak below this never got near full scale. Chosen as an obvious "quiet"
/// mark rather than a tuned threshold: this counts candidates for a decision,
/// it does not make one.
const QUIET_PEAK: f32 = 0.25;

/// Below this, a decode produced speech at a rate no conversation runs at.
const STARVED_WORDS_PER_SECOND: f32 = 1.0;

pub fn summarize(records: &[DecodeRecord]) -> DecodeSummary {
    let mut summary = DecodeSummary {
        decodes: records.len(),
        ..Default::default()
    };
    if records.is_empty() {
        return summary;
    }

    summary.failed = records.iter().filter(|r| r.failed).count();
    summary.rms_p10 = percentile(&values(records, |r| r.rms), 0.10);
    summary.rms_median = percentile(&values(records, |r| r.rms), 0.50);
    summary.peak_p10 = percentile(&values(records, |r| r.peak_amplitude), 0.10);
    summary.peak_median = percentile(&values(records, |r| r.peak_amplitude), 0.50);
    summary.quiet_decodes = records
        .iter()
        .filter(|r| r.speech_detected && r.peak_amplitude < QUIET_PEAK)
        .count();

    // Only decodes that heard something can say anything about language: a
    // silent one is not evidence that pinning went wrong.
    let voiced: Vec<&DecodeRecord> = records
        .iter()
        .filter(|r| r.speech_detected && !r.failed)
        .collect();
    let (pinned, auto): (Vec<&&DecodeRecord>, Vec<&&DecodeRecord>) =
        voiced.iter().partition(|r| r.resolved_language.is_some());

    summary.pinned_decodes = pinned.len();
    summary.auto_detected_decodes = auto.len();
    summary.pinned_words_per_second_median =
        percentile(&sorted(pinned.iter().map(|r| r.words_per_second)), 0.50);
    summary.auto_words_per_second_median =
        percentile(&sorted(auto.iter().map(|r| r.words_per_second)), 0.50);
    summary.starved_decodes = voiced
        .iter()
        .filter(|r| r.words_per_second < STARVED_WORDS_PER_SECOND)
        .count();

    summary
}

fn values(records: &[DecodeRecord], pick: impl Fn(&DecodeRecord) -> f32) -> Vec<f32> {
    sorted(records.iter().map(pick))
}

fn sorted(values: impl Iterator<Item = f32>) -> Vec<f32> {
    let mut out: Vec<f32> = values.filter(|v| v.is_finite()).collect();
    out.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    out
}

/// Nearest-rank percentile over an already-sorted slice.
fn percentile(sorted: &[f32], fraction: f32) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    let index = ((sorted.len() as f32 - 1.0) * fraction).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(words: &str, seconds: f32) -> SttDiagnosticSnapshot {
        let mut s = SttDiagnosticSnapshot {
            timestamp_epoch_ms: 1,
            session_mode: "dictation".to_string(),
            audio_file: None,
            original_duration_seconds: seconds,
            processed_duration_seconds: seconds,
            sample_rate: 16_000,
            channels: 1,
            rms: 0.05,
            peak_amplitude: 0.4,
            near_zero_percent: 0.0,
            has_non_finite: false,
            speech_detected: true,
            vad_start_seconds: 0.0,
            vad_end_seconds: seconds,
            vad_trimmed_duration_seconds: seconds,
            silence_removed_percent: 0.0,
            noise_floor: 0.001,
            onset_threshold: 0.01,
            model_filename: "ggml-small.bin".to_string(),
            model_path: "/models/ggml-small.bin".to_string(),
            primary_dictation_language: "en".to_string(),
            spoken_languages: vec!["en".to_string()],
            resolved_whisper_language: Some("en".to_string()),
            translate: false,
            strategy: "greedy".to_string(),
            best_of: 1,
            beam_size: None,
            temperature: 0.0,
            temperature_inc: 0.2,
            used_initial_prompt: true,
            initial_prompt_text: Some("Pragati, Sandhya, NavGurukul".to_string()),
            no_speech_thold: 0.6,
            entropy_thold: 2.4,
            logprob_thold: -1.0,
            inference_duration_ms: 900,
            real_time_factor: 0.3,
            segment_count: 2,
            transcript: words.to_string(),
            transcript_char_count: words.len(),
            error: None,
        };
        s.transcript_char_count = s.transcript.len();
        s
    }

    #[test]
    fn a_record_keeps_no_transcript_and_no_prompt() {
        // The property that lets this file be attached to a bug report. The
        // snapshot carries both the user's speech and the vocabulary they
        // configured; neither may survive into something persisted.
        let snapshot = snapshot("the vault rewrite slips to Monday", 4.0);
        let record = DecodeRecord::from_snapshot(&snapshot);

        let json = serde_json::to_string(&record).unwrap();
        assert!(!json.contains("vault rewrite"), "the transcript leaked: {json}");
        assert!(!json.contains("Sandhya"), "the initial prompt leaked: {json}");
        assert!(!json.contains("NavGurukul"));

        // The counts it exists for did survive.
        assert_eq!(record.word_count, 6);
        assert_eq!(record.words_per_second, 1.5);
    }

    #[test]
    fn words_per_second_exposes_a_starved_decode() {
        // The original report: 56 words for 4m55s of audio, no error anywhere.
        // Nothing in the old snapshot made that comparable across runs.
        let words = "word ".repeat(56);
        let record = DecodeRecord::from_snapshot(&snapshot(&words, 295.0));
        assert_eq!(record.word_count, 56);
        assert!(
            record.words_per_second < 0.25,
            "got {}",
            record.words_per_second
        );

        let summary = summarize(&[record]);
        assert_eq!(summary.starved_decodes, 1);
    }

    #[test]
    fn a_zero_length_decode_does_not_divide_by_zero() {
        let record = DecodeRecord::from_snapshot(&snapshot("", 0.0));
        assert_eq!(record.words_per_second, 0.0);
        assert!(record.words_per_second.is_finite());
    }

    #[test]
    fn the_summary_separates_pinned_decodes_from_auto_detected_ones() {
        // The comparison the LID decision needs: pinning is only worth
        // replacing if pinned decodes actually fare worse.
        let mut pinned = DecodeRecord::from_snapshot(&snapshot(&"word ".repeat(10), 100.0));
        pinned.resolved_language = Some("en".to_string());
        let mut auto = DecodeRecord::from_snapshot(&snapshot(&"word ".repeat(300), 100.0));
        auto.resolved_language = None;

        let summary = summarize(&[pinned, auto]);
        assert_eq!(summary.pinned_decodes, 1);
        assert_eq!(summary.auto_detected_decodes, 1);
        assert!(
            summary.pinned_words_per_second_median < summary.auto_words_per_second_median,
            "pinned {} vs auto {}",
            summary.pinned_words_per_second_median,
            summary.auto_words_per_second_median
        );
    }

    #[test]
    fn quiet_input_is_counted_for_the_gain_stage_question() {
        let mut loud = DecodeRecord::from_snapshot(&snapshot("hello there", 2.0));
        loud.peak_amplitude = 0.8;
        let mut quiet = DecodeRecord::from_snapshot(&snapshot("hello there", 2.0));
        quiet.peak_amplitude = 0.05;

        let summary = summarize(&[loud, quiet]);
        assert_eq!(summary.quiet_decodes, 1, "only the quiet one counts");
        assert_eq!(summary.decodes, 2);
    }

    #[test]
    fn silence_is_not_evidence_about_language() {
        // A decode that heard nothing says nothing about whether pinning was
        // wrong, and counting it would make pinning look worse than it is.
        let mut silent = DecodeRecord::from_snapshot(&snapshot("", 5.0));
        silent.speech_detected = false;

        let summary = summarize(&[silent]);
        assert_eq!(summary.decodes, 1);
        assert_eq!(summary.pinned_decodes, 0);
        assert_eq!(summary.starved_decodes, 0);
    }

    #[test]
    fn the_history_round_trips_and_stays_bounded() {
        let dir = std::env::temp_dir().join(format!("relay-decode-history-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        for i in 0..(MAX_RECORDS + 25) {
            let mut record = DecodeRecord::from_snapshot(&snapshot("one two three", 2.0));
            record.timestamp_epoch_ms = i as u128;
            append(&dir, &record);
        }

        let loaded = load(&dir);
        assert_eq!(loaded.len(), MAX_RECORDS, "the file is capped");
        assert_eq!(
            loaded.last().unwrap().timestamp_epoch_ms,
            (MAX_RECORDS + 24) as u128,
            "and it is the newest that survive"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unreadable_row_does_not_lose_the_rest() {
        let dir = std::env::temp_dir().join(format!("relay-decode-bad-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("diagnostics")).unwrap();
        let record = DecodeRecord::from_snapshot(&snapshot("one two", 1.0));
        let good = serde_json::to_string(&record).unwrap();
        std::fs::write(
            history_path(&dir),
            format!("{good}\n{{\"from\":\"an older version\"}}\n{good}\n"),
        )
        .unwrap();

        assert_eq!(load(&dir).len(), 2, "the parseable rows survive");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_empty_history_summarizes_to_zero_rather_than_panicking() {
        let summary = summarize(&[]);
        assert_eq!(summary, DecodeSummary::default());
        assert_eq!(summary.decodes, 0);
    }
}
