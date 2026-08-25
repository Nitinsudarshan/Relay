use super::capture::LiveAudioFrame;
use super::types::LiveTranscriptUpdate;
use crate::capture::stt::{SttEngine, SttLanguageConfig, WhisperDecodingConfig};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc as std_mpsc;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

pub struct LiveSttWorker {
    join_handle: Option<std::thread::JoinHandle<()>>,
    stop_flag: Arc<AtomicBool>,
}

impl LiveSttWorker {
    pub fn spawn(
        session_id: String,
        live_rx: std_mpsc::Receiver<LiveAudioFrame>,
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
            let mut last_committed_words: Vec<String> = Vec::new();

            while let Ok(frame) = live_rx.recv() {
                if stop_flag_clone.load(Ordering::Relaxed) {
                    break;
                }

                let frame_idx = frame.frame_index;
                let sample_count = frame.samples.len();
                let start_s = frame.start_time_s;
                let end_s = frame.end_time_s;
                let capture_instant = frame.capture_instant;

                // 1. Check RMS Energy (Silence gating < 1ms)
                let sum_sq: f32 = frame.samples.iter().map(|&s| s * s).sum();
                let rms = (sum_sq / sample_count.max(1) as f32).sqrt();

                if sample_count < 16_000 / 2 || rms < 0.005 {
                    // Silence or insufficient audio — skip heavy STT
                    continue;
                }

                // 2. Perform Fast Single-Pass Whisper Inference
                let stt_start = std::time::Instant::now();
                let transcription_res = stt.transcribe_with_config(
                    model_path_str,
                    &frame.samples,
                    &effective_lang_config,
                    &decoding_config,
                );
                let stt_duration_ms = stt_start.elapsed().as_millis();

                if let Ok((raw_text, _diag)) = transcription_res {
                    let cleaned = raw_text.trim();
                    if cleaned.is_empty() {
                        continue;
                    }

                    // 3. Overlap Deduplication
                    let words: Vec<String> = cleaned
                        .split_whitespace()
                        .map(|w| w.to_string())
                        .collect();

                    if words.is_empty() {
                        continue;
                    }

                    // Strip any prefix words that match the tail of the previous window
                    let mut unique_words = words.clone();
                    if !last_committed_words.is_empty() {
                        let max_check = unique_words.len().min(last_committed_words.len()).min(5);
                        for overlap_len in (1..=max_check).rev() {
                            let tail_slice = &last_committed_words[last_committed_words.len() - overlap_len..];
                            let head_slice = &unique_words[0..overlap_len];
                            if tail_slice.iter().map(|s| s.to_lowercase()).eq(head_slice.iter().map(|s| s.to_lowercase())) {
                                unique_words.drain(0..overlap_len);
                                break;
                            }
                        }
                    }

                    if unique_words.is_empty() {
                        continue;
                    }

                    let final_text = unique_words.join(" ");
                    last_committed_words = words;

                    let total_latency_ms = capture_instant.elapsed().as_millis() as u64;

                    tracing::info!(
                        "[LiveSTT] Frame #{} [{:.1}s - {:.1}s] (STT: {}ms, Latency: {}ms): \"{}\"",
                        frame_idx,
                        start_s,
                        end_s,
                        stt_duration_ms,
                        total_latency_ms,
                        final_text
                    );

                    // 4. Emit Low-Latency Live Segment Update to Frontend
                    let update = LiveTranscriptUpdate {
                        segment_id: format!("{}_{}", session_id, frame_idx),
                        session_id: session_id.clone(),
                        start_time_s: start_s,
                        end_time_s: end_s,
                        text: final_text,
                        is_final: true,
                        latency_ms: total_latency_ms,
                    };

                    if let Some(ref a) = app {
                        let _ = a.emit("meeting-live-transcript", &update);
                    }
                }
            }

            tracing::info!("LiveSttWorker: Exited cleanly for session {}.", session_id);
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
