use super::capture::AudioChunk;
use super::session_store::SessionStore;
use super::types::{TranscriptSegment, TranscriptSegmentStatus, TranscriptUtterance};
use crate::capture::stt::{SttEngine, SttLanguageConfig, SttSegment, WhisperDecodingConfig};
use std::path::PathBuf;
use std::sync::mpsc as std_mpsc;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

/// Below this RMS a chunk is treated as silence and never sent to Whisper.
const SILENCE_RMS_THRESHOLD: f32 = 0.005;

/// Rebases whisper's chunk-relative timings onto the session timeline and drops
/// utterances the decoder itself is not confident about.
///
/// Two signals, both of which Whisper produces and most pipelines throw away: a
/// high no-speech probability means the decoder found words where there was no
/// speech, and a low mean log-probability means it was guessing. Either is a
/// hallucination or a loop. Dropping them here is cheaper and more reliable
/// than asking a model downstream to ignore them.
fn filter_utterances(
    segments: &[SttSegment],
    chunk_start_s: f64,
    max_no_speech_prob: f32,
    min_avg_logprob: f32,
) -> (Vec<TranscriptUtterance>, usize) {
    let offset_ms = (chunk_start_s * 1000.0).round() as u64;
    let mut kept = Vec::with_capacity(segments.len());
    let mut dropped = 0;

    for segment in segments {
        if segment.text.trim().is_empty() {
            continue;
        }
        let unreliable = segment.no_speech_prob > max_no_speech_prob
            || segment.avg_logprob < min_avg_logprob;
        if unreliable {
            tracing::debug!(
                "Worker: dropping utterance (no_speech={:.2}, avg_logprob={:.2}): {:?}",
                segment.no_speech_prob,
                segment.avg_logprob,
                segment.text
            );
            dropped += 1;
            continue;
        }
        kept.push(TranscriptUtterance {
            start_ms: offset_ms + segment.start_ms,
            end_ms: offset_ms + segment.end_ms,
            text: segment.text.clone(),
            no_speech_prob: segment.no_speech_prob,
            avg_logprob: segment.avg_logprob,
        });
    }

    (kept, dropped)
}

fn mean(values: impl Iterator<Item = f32>) -> Option<f32> {
    let mut sum = 0.0;
    let mut count = 0;
    for v in values {
        sum += v;
        count += 1;
    }
    (count > 0).then(|| sum / count as f32)
}

/// The durable recording clock (Clock A).
///
/// Persists every audio chunk before transcribing it, so the audio on disk is
/// never contingent on transcription succeeding. The worker runs until the
/// capture thread drops its sender and the queue is empty — it is deliberately
/// *not* interruptible, because everything still queued at stop time is audio
/// the user already recorded.
pub struct TranscriptionWorker {
    join_handle: Option<std::thread::JoinHandle<()>>,
}

impl TranscriptionWorker {
    pub fn spawn(
        session_id: String,
        chunk_rx: std_mpsc::Receiver<AudioChunk>,
        store: Arc<SessionStore>,
        stt: SttEngine,
        whisper_model_path: Option<PathBuf>,
        language_config: SttLanguageConfig,
        decoding_config: WhisperDecodingConfig,
        app: Option<AppHandle>,
    ) -> Self {
        // The language resolved for this session is used as-is, including
        // auto-detect. Forcing English here used to be the guard against
        // decoder loops and hallucinated language codes, but it also pushed
        // Hindi and Hinglish audio through an English acoustic filter, which
        // costs more than it saves. Loops are now handled where they belong:
        // `no_context` in the decoder plus per-utterance confidence filtering
        // below.
        let effective_lang_config = language_config;

        let handle = std::thread::spawn(move || {
            let model_path_str = whisper_model_path.as_ref().and_then(|p| p.to_str());

            while let Ok(chunk) = chunk_rx.recv() {
                let chunk_idx = chunk.chunk_index;
                let sample_count = chunk.samples.len();
                let start_s = chunk.start_time_s;
                let end_s = chunk.end_time_s;

                tracing::info!(
                    "Worker: received chunk #{} for session {} ({} samples, {:.1}s - {:.1}s)",
                    chunk_idx,
                    session_id,
                    sample_count,
                    start_s,
                    end_s
                );

                // 1. Persist audio first — audio is the source of truth.
                if let Err(e) =
                    store.write_chunk_wav(&session_id, chunk_idx, &chunk.samples, 16_000)
                {
                    tracing::error!("Worker: failed to write chunk WAV #{}: {}", chunk_idx, e);
                }

                let _ = store.update_session(&session_id, |session| {
                    session.chunk_count = chunk_idx + 1;
                    session.duration_seconds = end_s;
                    session.total_audio_bytes += (sample_count * 2) as u64;
                    if chunk.mic_had_audio {
                        session.mic_heard = true;
                    }
                    if chunk.sys_had_audio {
                        session.sys_audio_heard = true;
                    }
                    session.pending_transcription_chunks = session
                        .chunk_count
                        .saturating_sub(session.transcript_segment_count);
                });

                // 2. Transcribe. Silence is rejected on energy alone rather than
                //    spending a full decode to produce nothing.
                let sum_sq: f32 = chunk.samples.iter().map(|&s| s * s).sum();
                let rms = (sum_sq / sample_count.max(1) as f32).sqrt();

                let (text, status, utterances, dropped) =
                    if sample_count < 16_000 / 2 || rms < SILENCE_RMS_THRESHOLD {
                        (String::new(), TranscriptSegmentStatus::Empty, Vec::new(), 0)
                    } else {
                        match stt.transcribe_segments_with_config(
                            model_path_str,
                            &chunk.samples,
                            &effective_lang_config,
                            &decoding_config,
                        ) {
                            Ok((segments, _diag)) => {
                                let (kept, dropped) = filter_utterances(
                                    &segments,
                                    start_s,
                                    decoding_config.no_speech_thold,
                                    decoding_config.logprob_thold,
                                );
                                let joined = kept
                                    .iter()
                                    .map(|u| u.text.as_str())
                                    .collect::<Vec<_>>()
                                    .join(" ")
                                    .trim()
                                    .to_string();
                                if dropped > 0 {
                                    tracing::info!(
                                        "Worker: dropped {} low-confidence utterance(s) from chunk #{}",
                                        dropped,
                                        chunk_idx
                                    );
                                }
                                if joined.is_empty() {
                                    (String::new(), TranscriptSegmentStatus::Empty, kept, dropped)
                                } else {
                                    (joined, TranscriptSegmentStatus::Success, kept, dropped)
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Worker: STT transcription error on chunk #{}: {}",
                                    chunk_idx,
                                    e
                                );
                                (String::new(), TranscriptSegmentStatus::Failed, Vec::new(), 0)
                            }
                        }
                    };

                // 3. Persist the transcript segment.
                let segment = TranscriptSegment {
                    chunk_index: chunk_idx,
                    start_time_s: start_s,
                    end_time_s: end_s,
                    text: text.clone(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                    status,
                    avg_logprob: mean(utterances.iter().map(|u| u.avg_logprob)),
                    no_speech_prob: mean(utterances.iter().map(|u| u.no_speech_prob)),
                    utterances,
                    dropped_utterances: dropped,
                };

                if let Err(e) = store.append_transcript_segment(&session_id, &segment) {
                    tracing::error!(
                        "Worker: failed to append transcript segment #{}: {}",
                        chunk_idx,
                        e
                    );
                }

                let segment_words = text.split_whitespace().count();
                let _ = store.update_session(&session_id, |session| {
                    session.transcript_segment_count += 1;
                    session.word_count += segment_words;
                    session.pending_transcription_chunks = session
                        .chunk_count
                        .saturating_sub(session.transcript_segment_count);
                });

                // 4. Broadcast to the UI.
                if let Some(ref a) = app {
                    let _ = a.emit("meeting-transcript-segment", &segment);
                }
            }

            tracing::info!("Worker: all chunks processed for session {}.", session_id);
        });

        Self {
            join_handle: Some(handle),
        }
    }

    /// Waits for the queue to drain. Returns once every recorded chunk has been
    /// written and transcribed.
    pub fn join(&mut self) {
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(text: &str, no_speech_prob: f32, avg_logprob: f32) -> SttSegment {
        SttSegment {
            start_ms: 0,
            end_ms: 2_000,
            text: text.to_string(),
            no_speech_prob,
            avg_logprob,
        }
    }

    #[test]
    fn a_confident_utterance_is_kept_and_rebased_onto_the_session_timeline() {
        let mut s = segment("We agreed the split is fifty-fifty.", 0.05, -0.3);
        s.start_ms = 1_500;
        s.end_ms = 4_000;

        // Chunk five starts at 150 s, so this utterance is at 151.5 s.
        let (kept, dropped) = filter_utterances(&[s], 150.0, 0.6, -1.0);
        assert_eq!(dropped, 0);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].start_ms, 151_500);
        assert_eq!(kept[0].end_ms, 154_000);
    }

    #[test]
    fn an_utterance_over_the_no_speech_threshold_is_dropped() {
        // Words decoded where there was no speech: the classic hallucination.
        let (kept, dropped) =
            filter_utterances(&[segment("Thank you for watching!", 0.92, -0.2)], 0.0, 0.6, -1.0);
        assert!(kept.is_empty());
        assert_eq!(dropped, 1);
    }

    #[test]
    fn an_utterance_below_the_logprob_threshold_is_dropped() {
        let (kept, dropped) =
            filter_utterances(&[segment("mumbled fragment", 0.1, -2.4)], 0.0, 0.6, -1.0);
        assert!(kept.is_empty());
        assert_eq!(dropped, 1);
    }

    #[test]
    fn filtering_keeps_the_good_utterances_from_a_mixed_chunk() {
        let segments = [
            segment("The placement data came company-wise.", 0.04, -0.25),
            segment("Thanks for watching, please subscribe.", 0.95, -0.1),
            segment("We want candidate-level updates weekly.", 0.06, -0.4),
        ];
        let (kept, dropped) = filter_utterances(&segments, 0.0, 0.6, -1.0);
        assert_eq!(dropped, 1);
        assert_eq!(kept.len(), 2);
        assert!(kept[1].text.contains("weekly"));
    }

    #[test]
    fn empty_utterances_are_neither_kept_nor_counted_as_dropped() {
        let (kept, dropped) = filter_utterances(&[segment("   ", 0.1, -0.2)], 0.0, 0.6, -1.0);
        assert!(kept.is_empty());
        assert_eq!(dropped, 0, "whitespace is not a dropped utterance");
    }

    #[test]
    fn mean_of_nothing_is_absent_not_zero() {
        assert_eq!(mean(std::iter::empty()), None);
        assert_eq!(mean([1.0_f32, 3.0].into_iter()), Some(2.0));
    }
}
