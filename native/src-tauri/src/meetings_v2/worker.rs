use super::capture::AudioChunk;
use super::session_store::SessionStore;
use super::types::{TranscriptSegment, TranscriptSegmentStatus};
use crate::capture::stt::{SttEngine, SttLanguageConfig, WhisperDecodingConfig};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc as std_mpsc;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

pub struct TranscriptionWorker {
    join_handle: Option<std::thread::JoinHandle<()>>,
    stop_flag: Arc<AtomicBool>,
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
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();

        // Enforce explicit language (defaulting to "en") so Whisper never
        // hallucinates random language codes (e.g. "kn", "la") or token repetition loops on background hiss
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
                let is_stopped = stop_flag_clone.load(Ordering::SeqCst);

                tracing::info!(
                    "Worker: Received Chunk #{} for session {} ({} samples, {:.1}s - {:.1}s)",
                    chunk_idx,
                    session_id,
                    sample_count,
                    start_s,
                    end_s
                );

                // 1. Persist audio chunk to disk immediately (audio is source of truth!)
                let write_res = store.write_chunk_wav(&session_id, chunk_idx, &chunk.samples, 16_000);
                if let Err(ref e) = write_res {
                    tracing::error!("Worker: Failed to write chunk WAV #{}: {}", chunk_idx, e);
                }

                // Update session chunk count in metadata
                if let Ok(mut session) = store.get_session(&session_id) {
                    session.chunk_count = chunk_idx + 1;
                    session.duration_seconds = end_s;
                    session.total_audio_bytes += (sample_count * 2) as u64;
                    session.updated_at = chrono::Utc::now().to_rfc3339();
                    let _ = store.save_session(&session);
                }

                // 2. Incremental Transcription
                let sum_sq: f32 = chunk.samples.iter().map(|&s| s * s).sum();
                let rms = (sum_sq / chunk.samples.len().max(1) as f32).sqrt();

                let (text, status) = if sample_count < 16_000 / 2 || rms < 0.005 || (is_stopped && rms < 0.01) {
                    // Less than 0.5s of audio or silence — treat as empty immediately (< 1ms)
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
                            tracing::warn!("Worker: STT transcription error on chunk #{}: {}", chunk_idx, e);
                            (String::new(), TranscriptSegmentStatus::Failed)
                        }
                    }
                };

                // 3. Persist transcript segment to JSONL
                let segment = TranscriptSegment {
                    chunk_index: chunk_idx,
                    start_time_s: start_s,
                    end_time_s: end_s,
                    text: text.clone(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                    status,
                };

                if let Err(e) = store.append_transcript_segment(&session_id, &segment) {
                    tracing::error!("Worker: Failed to append transcript segment #{}: {}", chunk_idx, e);
                }

                // Update session transcript segment count
                if let Ok(mut session) = store.get_session(&session_id) {
                    session.transcript_segment_count += 1;
                    session.updated_at = chrono::Utc::now().to_rfc3339();
                    let _ = store.save_session(&session);
                }

                // 4. Broadcast live transcript update to frontend
                if let Some(ref a) = app {
                    let _ = a.emit("meeting-transcript-segment", &segment);
                }
            }

            tracing::info!("Worker: All chunks processed for session {}.", session_id);
        });

        Self {
            join_handle: Some(handle),
            stop_flag,
        }
    }

    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }

    pub fn join(&mut self) {
        self.stop();
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}
