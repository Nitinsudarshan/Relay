use super::capture::AudioChunk;
use super::session_store::SessionStore;
use super::types::{TranscriptSegment, TranscriptSegmentStatus};
use crate::capture::stt::{SttEngine, SttLanguageConfig, WhisperDecodingConfig};
use std::path::PathBuf;
use std::sync::mpsc as std_mpsc;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

/// Below this RMS a chunk is treated as silence and never sent to Whisper.
const SILENCE_RMS_THRESHOLD: f32 = 0.005;

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
        // Enforce explicit language (defaulting to "en") so Whisper never
        // hallucinates random language codes (e.g. "kn", "la") or token
        // repetition loops on background hiss.
        let effective_lang_config = if language_config.whisper_language.is_none() {
            SttLanguageConfig {
                whisper_language: Some("en".to_string()),
                translate: false,
            }
        } else {
            language_config
        };

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

                let (text, status) = if sample_count < 16_000 / 2 || rms < SILENCE_RMS_THRESHOLD {
                    (String::new(), TranscriptSegmentStatus::Empty)
                } else {
                    match stt.transcribe_with_config(
                        model_path_str,
                        &chunk.samples,
                        &effective_lang_config,
                        &decoding_config,
                    ) {
                        Ok((t, _diag)) => {
                            let trimmed = t.trim().to_string();
                            if trimmed.is_empty() {
                                (String::new(), TranscriptSegmentStatus::Empty)
                            } else {
                                (trimmed, TranscriptSegmentStatus::Success)
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Worker: STT transcription error on chunk #{}: {}",
                                chunk_idx,
                                e
                            );
                            (String::new(), TranscriptSegmentStatus::Failed)
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
