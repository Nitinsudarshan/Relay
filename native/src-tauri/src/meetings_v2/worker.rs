use super::capture::{resolve_utterance_channel, AudioChunk};
use super::session_store::SessionStore;
use super::types::{TranscriptSegment, TranscriptSegmentStatus, TranscriptUtterance};
use crate::capture::stt::{
    join_utterance_text, SttEngine, SttLanguageConfig, SttUtterance, WhisperDecodingConfig,
};
use std::path::PathBuf;
use std::sync::mpsc as std_mpsc;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

/// Below this RMS a chunk is treated as silence and never sent to Whisper.
const SILENCE_RMS_THRESHOLD: f32 = 0.005;

/// How much of the previous chunk's text is carried into the next chunk's
/// decode as context.
///
/// Every chunk is decoded on a fresh Whisper state, which is what keeps a
/// decoder loop in one chunk from poisoning the rest of the meeting. The cost is
/// that a sentence spanning a 30-second boundary is decoded as two blind halves.
/// Passing the tail of the previous chunk as the initial prompt restores the
/// context without restoring the shared state: a loop cannot propagate, because
/// the prompt is capped and the state is still discarded.
const CONTEXT_CARRY_CHARS: usize = 180;

/// Builds the initial prompt for one chunk: the standing vocabulary prompt,
/// followed by the tail of what was said just before.
///
/// Whisper treats the prompt as preceding text rather than as instructions, so
/// vocabulary and prior speech belong in the same string in that order.
fn chunk_initial_prompt(vocabulary: Option<&str>, previous_tail: &str) -> Option<String> {
    let vocabulary = vocabulary.map(str::trim).filter(|v| !v.is_empty());
    let previous_tail = previous_tail.trim();

    match (vocabulary, previous_tail.is_empty()) {
        (None, true) => None,
        (None, false) => Some(previous_tail.to_string()),
        (Some(v), true) => Some(v.to_string()),
        (Some(v), false) => Some(format!("{v}. {previous_tail}")),
    }
}

/// The trailing `CONTEXT_CARRY_CHARS` of a chunk's text, cut at a word boundary.
fn context_tail(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.len() <= CONTEXT_CARRY_CHARS {
        return trimmed.to_string();
    }
    // Cut on a char boundary first, then advance to the next whitespace so the
    // carried context never begins mid-word.
    let mut cut = trimmed.len() - CONTEXT_CARRY_CHARS;
    while cut < trimmed.len() && !trimmed.is_char_boundary(cut) {
        cut += 1;
    }
    let tail = &trimmed[cut..];
    match tail.find(char::is_whitespace) {
        Some(space) => tail[space..].trim_start().to_string(),
        None => tail.to_string(),
    }
}

/// Attributes each decoded utterance to a channel using the chunk's per-second
/// energy track, and rebases its timing onto the session clock.
///
/// Whisper reports utterance bounds relative to the audio it was given, which is
/// exactly this chunk, so the same offsets index the channel track directly.
///
/// When the chunk carries no track — a transcript recorded before v2.5, or a
/// chunk with no samples — every utterance inherits the chunk-wide flags. That
/// is no worse than the pre-v2.5 behaviour and never claims a resolution the
/// data cannot support.
fn attribute_utterances(
    utterances: &[SttUtterance],
    chunk: &AudioChunk,
) -> Vec<TranscriptUtterance> {
    let chunk_duration_s = (chunk.end_time_s - chunk.start_time_s).max(0.0);

    utterances
        .iter()
        .enumerate()
        .map(|(index, utterance)| {
            // Whisper can report a span slightly past the audio it was handed.
            let start_offset_s = utterance.start_s.clamp(0.0, chunk_duration_s);
            let end_offset_s = utterance.end_s.clamp(start_offset_s, chunk_duration_s);

            let (mic_had_audio, sys_had_audio) = if chunk.channel_track.is_empty() {
                (chunk.mic_had_audio, chunk.sys_had_audio)
            } else {
                let resolved =
                    resolve_utterance_channel(&chunk.channel_track, start_offset_s, end_offset_s);
                // A zero-length or silent span tells us nothing; fall back rather
                // than reporting silence over speech Whisper did decode.
                if resolved == (false, false) {
                    (chunk.mic_had_audio, chunk.sys_had_audio)
                } else {
                    resolved
                }
            };

            TranscriptUtterance {
                index,
                start_time_s: chunk.start_time_s + start_offset_s,
                end_time_s: chunk.start_time_s + end_offset_s,
                text: utterance.text.clone(),
                mic_had_audio,
                sys_had_audio,
                no_speech_prob: utterance.no_speech_prob,
            }
        })
        .collect()
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

            // The standing vocabulary prompt, kept aside so each chunk can be
            // prompted with it *plus* the tail of the previous chunk.
            let vocabulary_prompt = decoding_config.initial_prompt.clone();
            let mut previous_tail = String::new();

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

                let (text, utterances, status) = if sample_count < 16_000 / 2
                    || rms < SILENCE_RMS_THRESHOLD
                {
                    // Silence breaks the context chain: carrying text across a
                    // gap would prompt the next chunk with speech that did not
                    // immediately precede it.
                    previous_tail.clear();
                    (String::new(), Vec::new(), TranscriptSegmentStatus::Empty)
                } else {
                    let mut chunk_config = decoding_config.clone();
                    chunk_config.initial_prompt =
                        chunk_initial_prompt(vocabulary_prompt.as_deref(), &previous_tail);

                    match stt.transcribe_utterances_with_config(
                        model_path_str,
                        &chunk.samples,
                        &effective_lang_config,
                        &chunk_config,
                    ) {
                        Ok((decoded, _diag)) => {
                            let joined = join_utterance_text(&decoded);
                            if joined.is_empty() {
                                previous_tail.clear();
                                (String::new(), Vec::new(), TranscriptSegmentStatus::Empty)
                            } else {
                                previous_tail = context_tail(&joined);
                                let attributed = attribute_utterances(&decoded, &chunk);
                                (joined, attributed, TranscriptSegmentStatus::Success)
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Worker: STT transcription error on chunk #{}: {}",
                                chunk_idx,
                                e
                            );
                            previous_tail.clear();
                            (String::new(), Vec::new(), TranscriptSegmentStatus::Failed)
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
                    // Already measured on the chunk; carrying it onto the segment
                    // is what lets the processing pipeline attribute speakers by
                    // channel without any extra work or a second audio pass.
                    mic_had_audio: chunk.mic_had_audio,
                    sys_had_audio: chunk.sys_had_audio,
                    utterances,
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
    use crate::meetings_v2::capture::ChannelEnergy;

    const LOUD: f32 = 0.20;
    const QUIET: f32 = 0.001;

    fn chunk_with_track(track: Vec<ChannelEnergy>, start_time_s: f64, duration_s: f64) -> AudioChunk {
        let mic_had_audio = track.iter().any(|b| b.mic_rms > 0.01);
        let sys_had_audio = track.iter().any(|b| b.sys_rms > 0.01);
        AudioChunk {
            session_id: "meet_test".to_string(),
            chunk_index: 0,
            start_time_s,
            end_time_s: start_time_s + duration_s,
            samples: Vec::new(),
            mic_had_audio,
            sys_had_audio,
            channel_track: track,
        }
    }

    fn track(seconds: &[(f32, f32)]) -> Vec<ChannelEnergy> {
        seconds
            .iter()
            .enumerate()
            .map(|(i, &(mic_rms, sys_rms))| ChannelEnergy {
                offset_s: i as f64,
                duration_s: 1.0,
                mic_rms,
                sys_rms,
            })
            .collect()
    }

    fn utterance(start_s: f64, end_s: f64, text: &str) -> SttUtterance {
        SttUtterance {
            start_s,
            end_s,
            text: text.to_string(),
            no_speech_prob: 0.01,
        }
    }

    #[test]
    fn each_utterance_gets_the_channel_that_was_live_while_it_was_spoken() {
        let chunk = chunk_with_track(
            track(&[(LOUD, QUIET), (LOUD, QUIET), (QUIET, LOUD), (QUIET, LOUD)]),
            30.0,
            4.0,
        );
        let decoded = vec![
            utterance(0.0, 2.0, "I'll send the deck tomorrow."),
            utterance(2.0, 4.0, "Great, thanks."),
        ];

        let attributed = attribute_utterances(&decoded, &chunk);

        assert_eq!(attributed.len(), 2);
        assert_eq!(
            (attributed[0].mic_had_audio, attributed[0].sys_had_audio),
            (true, false)
        );
        assert_eq!(
            (attributed[1].mic_had_audio, attributed[1].sys_had_audio),
            (false, true)
        );
        // The chunk-wide flags alone could not have told these apart.
        assert!(chunk.mic_had_audio && chunk.sys_had_audio);
    }

    #[test]
    fn utterance_timings_are_rebased_onto_the_session_clock() {
        let chunk = chunk_with_track(track(&[(LOUD, QUIET), (LOUD, QUIET)]), 120.0, 2.0);
        let attributed = attribute_utterances(&[utterance(0.5, 1.5, "somewhere in the middle")], &chunk);
        assert_eq!(attributed[0].start_time_s, 120.5);
        assert_eq!(attributed[0].end_time_s, 121.5);
        assert_eq!(attributed[0].index, 0);
    }

    #[test]
    fn a_span_whisper_reports_past_the_audio_is_clamped() {
        let chunk = chunk_with_track(track(&[(LOUD, QUIET)]), 0.0, 1.0);
        let attributed = attribute_utterances(&[utterance(0.0, 30.0, "overrun")], &chunk);
        assert_eq!(attributed[0].end_time_s, 1.0);
    }

    #[test]
    fn a_chunk_without_a_track_falls_back_to_the_chunk_flags() {
        // A transcript recorded before v2.5 carries no channel track.
        let mut chunk = chunk_with_track(Vec::new(), 0.0, 30.0);
        chunk.mic_had_audio = true;
        chunk.sys_had_audio = false;

        let attributed = attribute_utterances(&[utterance(0.0, 3.0, "legacy chunk")], &chunk);
        assert_eq!(
            (attributed[0].mic_had_audio, attributed[0].sys_had_audio),
            (true, false)
        );
    }

    #[test]
    fn a_silent_span_falls_back_rather_than_claiming_silence_over_decoded_speech() {
        // Whisper decoded words here, so reporting "no channel" would be worse
        // than reporting the chunk's own flags.
        let chunk = chunk_with_track(track(&[(QUIET, QUIET), (LOUD, QUIET)]), 0.0, 2.0);
        let attributed = attribute_utterances(&[utterance(0.0, 1.0, "quiet but decoded")], &chunk);
        assert_eq!(
            (attributed[0].mic_had_audio, attributed[0].sys_had_audio),
            (true, false)
        );
    }

    #[test]
    fn the_initial_prompt_carries_vocabulary_then_recent_speech() {
        assert_eq!(chunk_initial_prompt(None, ""), None);
        assert_eq!(
            chunk_initial_prompt(Some("Relay, Supabase"), ""),
            Some("Relay, Supabase".to_string())
        );
        assert_eq!(
            chunk_initial_prompt(None, "and then we agreed to"),
            Some("and then we agreed to".to_string())
        );
        assert_eq!(
            chunk_initial_prompt(Some("Relay, Supabase"), "and then we agreed to"),
            Some("Relay, Supabase. and then we agreed to".to_string())
        );
        // Whitespace-only inputs are treated as absent.
        assert_eq!(chunk_initial_prompt(Some("   "), "  "), None);
    }

    #[test]
    fn the_carried_tail_is_bounded_and_never_starts_mid_word() {
        let short = "a short chunk";
        assert_eq!(context_tail(short), short);

        let long = "word ".repeat(200);
        let tail = context_tail(&long);
        assert!(tail.len() <= CONTEXT_CARRY_CHARS);
        assert!(tail.starts_with("word"), "tail was {tail:?}");
    }

    #[test]
    fn the_carried_tail_handles_multibyte_text() {
        // A cut landing inside a multi-byte character must not panic.
        let long = "मैं कल तक भेज दूंगा ".repeat(40);
        let tail = context_tail(&long);
        assert!(!tail.is_empty());
        assert!(long.contains(tail.trim()));
    }
}
