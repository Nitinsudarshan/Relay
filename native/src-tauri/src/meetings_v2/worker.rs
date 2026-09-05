use super::capture::{resolve_utterance_channel, utterance_channel_energy, AudioChunk};
use super::diarize::features as voice_features;
use super::diarize::incremental::IncrementalDiarizer;
use super::session_store::SessionStore;
use super::transcript_health::{
    self, DecodeEvidence, SpeechProfile, TranscriptRejection, MIN_VOICED_SECONDS,
};
use super::types::{TranscriptSegment, TranscriptSegmentStatus, TranscriptUtterance};
use crate::capture::stt::{
    join_utterance_text, SttEngine, SttLanguageConfig, SttUtterance, WhisperDecodingConfig,
};
use std::path::PathBuf;
use std::sync::mpsc as std_mpsc;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

/// Shortest chunk worth decoding at all, in samples. Half a second; below this
/// there is not a word to find.
const MIN_DECODABLE_SAMPLES: usize = 16_000 / 2;

/// Shortest utterance worth identifying a speaker from, in seconds.
///
/// Below this the cepstral statistics describe whichever phoneme happened to
/// fall in the window rather than the voice, and a wrong speaker is worse than
/// none — it puts words in somebody's mouth in the conversation view.
const MIN_LIVE_UTTERANCE_SECONDS: f64 = 0.8;

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

/// The tail of a chunk's text to carry into the next decode, or nothing.
///
/// Returns `None` whenever the text is not safe to prompt with. This is the
/// other half of the loop fix: chunk state is already discarded between
/// decodes, but the *prompt* is not, and Whisper reads it as preceding speech.
/// Prompting chunk n+1 with "Thank you. Thank you." makes the same continuation
/// overwhelmingly likely, which is how one bad chunk became nine consecutive
/// ones in the failure this replaces.
fn carry_forward(text: &str) -> Option<String> {
    let tail = context_tail(text);
    transcript_health::is_safe_as_prompt(&tail).then_some(tail)
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
    voices: Option<&mut IncrementalDiarizer>,
) -> Vec<TranscriptUtterance> {
    let chunk_duration_s = (chunk.end_time_s - chunk.start_time_s).max(0.0);
    let mut voices = voices;

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

            let (mic_rms, sys_rms) =
                utterance_channel_energy(&chunk.channel_track, start_offset_s, end_offset_s);

            // Who spoke, decided now rather than after the meeting. The chunk
            // carries the answer, so the readable conversation and the summary
            // are built from text that already says who said it.
            let live_speaker = voices.as_deref_mut().and_then(|registry| {
                let slice = utterance_samples(&chunk.samples, start_offset_s, end_offset_s)?;
                let features = voice_features::extract(slice, 16_000)?;
                let total = mic_rms + sys_rms;
                let mic_share = (total > 0.0).then(|| mic_rms / total);
                registry.assign(&features, mic_share).map(|a| a.speaker)
            });

            TranscriptUtterance {
                index,
                start_time_s: chunk.start_time_s + start_offset_s,
                end_time_s: chunk.start_time_s + end_offset_s,
                text: utterance.text.clone(),
                mic_had_audio,
                sys_had_audio,
                no_speech_prob: utterance.no_speech_prob,
                mic_rms,
                sys_rms,
                live_speaker,
            }
        })
        .collect()
}

/// The samples covering one utterance within its chunk.
///
/// `None` when the span falls outside the audio actually captured, or is too
/// short to characterise a voice from — the same bound the post-hoc pass uses,
/// so a span left unattributed after the meeting is also unattributed during it.
fn utterance_samples(samples: &[f32], start_s: f64, end_s: f64) -> Option<&[f32]> {
    let start = (start_s * 16_000.0) as usize;
    let end = ((end_s * 16_000.0) as usize).min(samples.len());
    if start >= end {
        return None;
    }
    let slice = &samples[start..end];
    (slice.len() as f64 / 16_000.0 >= MIN_LIVE_UTTERANCE_SECONDS).then_some(slice)
}

/// Utterances Whisper returned, minus the ones that are not speech.
///
/// Filtering happens per utterance rather than per chunk because Whisper
/// routinely decodes a real sentence and then pads the rest of a 30-second
/// window with filler. Dropping the padding keeps the sentence.
fn keep_spoken_utterances(
    decoded: &[SttUtterance],
    profile: &SpeechProfile,
) -> (Vec<SttUtterance>, usize) {
    let mut kept = Vec::with_capacity(decoded.len());
    let mut dropped = 0usize;

    for utterance in decoded {
        let span = (utterance.end_s - utterance.start_s).max(0.0);
        // An utterance's own span is the audio it claims to cover, so the
        // voiced time available to it is bounded by the chunk's voiced time.
        let evidence = DecodeEvidence {
            voiced_seconds: profile.voiced_seconds.min(span.max(0.001)),
            total_seconds: span.max(0.001),
            mean_no_speech_prob: utterance.no_speech_prob,
        };
        match transcript_health::assess(&utterance.text, evidence) {
            Some(reason) => {
                dropped += 1;
                tracing::debug!(
                    "Worker: dropped utterance [{:.1}s-{:.1}s]: {}",
                    utterance.start_s,
                    utterance.end_s,
                    reason.describe()
                );
            }
            None => kept.push(utterance.clone()),
        }
    }

    (kept, dropped)
}

/// What one chunk's decode resolved to.
struct DecodeOutcome {
    text: String,
    utterances: Vec<TranscriptUtterance>,
    status: TranscriptSegmentStatus,
    rejection: Option<TranscriptRejection>,
    /// Text to prompt the next chunk with. `None` breaks the chain.
    carry: Option<String>,
}

impl DecodeOutcome {
    fn empty() -> Self {
        Self {
            text: String::new(),
            utterances: Vec::new(),
            status: TranscriptSegmentStatus::Empty,
            rejection: None,
            carry: None,
        }
    }

    fn failed() -> Self {
        Self {
            text: String::new(),
            utterances: Vec::new(),
            status: TranscriptSegmentStatus::Failed,
            rejection: None,
            carry: None,
        }
    }

    fn rejected(rejection: TranscriptRejection) -> Self {
        Self {
            text: String::new(),
            utterances: Vec::new(),
            status: TranscriptSegmentStatus::Rejected,
            rejection: Some(rejection),
            carry: None,
        }
    }
}

/// Turns Whisper's raw output for one chunk into the segment that gets stored.
///
/// Split out from the worker loop so the whole decision — what counts as
/// speech, what is thrown away, and what is carried into the next decode — is
/// testable without an audio device or a Whisper model.
fn resolve_decode(
    decoded: &[SttUtterance],
    chunk: &AudioChunk,
    profile: &SpeechProfile,
    voices: Option<&mut IncrementalDiarizer>,
) -> DecodeOutcome {
    let (kept, dropped) = keep_spoken_utterances(decoded, profile);
    if dropped > 0 {
        tracing::info!(
            "Worker: chunk #{} dropped {}/{} utterances as non-speech",
            chunk.chunk_index,
            dropped,
            decoded.len()
        );
    }

    let joined = join_utterance_text(&kept);
    if joined.is_empty() {
        // Everything Whisper produced was filler. If it produced anything at
        // all, that is a rejection to record, not an empty chunk.
        let discarded = join_utterance_text(decoded);
        if discarded.is_empty() {
            return DecodeOutcome::empty();
        }
        let mean_no_speech = mean_no_speech_prob(decoded);
        let reason = transcript_health::assess(
            &discarded,
            DecodeEvidence {
                voiced_seconds: profile.voiced_seconds,
                total_seconds: profile.total_seconds,
                mean_no_speech_prob: mean_no_speech,
            },
        )
        .unwrap_or(transcript_health::HallucinationReason::NoSpeech {
            probability: mean_no_speech,
        });
        return DecodeOutcome::rejected(transcript_health::rejection(reason, &discarded));
    }

    // The chunk as a whole is assessed again: individually plausible utterances
    // can still form a loop across the window, which is the shape the reported
    // failure actually took.
    let evidence = DecodeEvidence {
        voiced_seconds: profile.voiced_seconds,
        total_seconds: profile.total_seconds,
        mean_no_speech_prob: mean_no_speech_prob(&kept),
    };
    if let Some(reason) = transcript_health::assess(&joined, evidence) {
        return DecodeOutcome::rejected(transcript_health::rejection(reason, &joined));
    }

    DecodeOutcome {
        carry: carry_forward(&joined),
        utterances: attribute_utterances(&kept, chunk, voices),
        text: joined,
        status: TranscriptSegmentStatus::Success,
        rejection: None,
    }
}

/// Whisper's no-speech probability across a decode, weighted by span length.
///
/// Weighting matters: a one-word span with a high probability should not
/// condemn twenty seconds of confident speech beside it.
fn mean_no_speech_prob(utterances: &[SttUtterance]) -> f32 {
    let mut weighted = 0.0f64;
    let mut total = 0.0f64;
    for utterance in utterances {
        let span = (utterance.end_s - utterance.start_s).max(0.01);
        weighted += utterance.no_speech_prob as f64 * span;
        total += span;
    }
    if total <= 0.0 {
        return 0.0;
    }
    (weighted / total) as f32
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
    // The worker's collaborators, handed over once at spawn. A config struct
    // here would be a struct with one construction site and one read site.
    #[allow(clippy::too_many_arguments)]
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
        // Respect the user's language configuration directly. When None, Whisper's
        // native language auto-detection operates across chunks, preserving
        // multilingual speech, Hindi, and code-switched Hinglish without forcing
        // English or triggering translation.
        let mut effective_lang_config = language_config;
        // Invariant: Meeting transcription NEVER translates.
        effective_lang_config.translate = false;

        let handle = std::thread::spawn(move || {
            let model_path_str = whisper_model_path.as_ref().and_then(|p| p.to_str());

            // The standing vocabulary prompt, kept aside so each chunk can be
            // prompted with it *plus* the tail of the previous chunk.
            let vocabulary_prompt = decoding_config.initial_prompt.clone();
            let mut previous_tail = String::new();

            // The speaker registry for this recording. It grows as chunks land,
            // so by the third chunk the meeting already knows how many people
            // are in it rather than waiting for the recording to end.
            let mut voices = IncrementalDiarizer::new();

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

                // 2. Decide whether there is speech here at all.
                //
                //    This is measured at 20 ms resolution against the chunk's own
                //    noise floor, not as one RMS mean across thirty seconds. A
                //    chunk-wide mean cannot tell a fan from a conversation: steady
                //    hiss at 0.006 RMS clears any fixed threshold for the whole
                //    window while containing no voice, and Whisper handed such a
                //    window emits subtitle boilerplate — which is how a real
                //    meeting ended up with four minutes of "Thank you."
                let profile = transcript_health::profile_speech(&chunk.samples, 16_000);

                let outcome = if sample_count < MIN_DECODABLE_SAMPLES
                    || !profile.is_worth_decoding()
                {
                    tracing::info!(
                        "Worker: chunk #{} not decoded — {:.2}s voiced of {:.1}s (floor {:.4}, \
peak {:.3}); below the {:.1}s gate",
                        chunk_idx,
                        profile.voiced_seconds,
                        profile.total_seconds,
                        profile.noise_floor_rms,
                        profile.peak_amplitude,
                        MIN_VOICED_SECONDS
                    );
                    DecodeOutcome::empty()
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
                            resolve_decode(&decoded, &chunk, &profile, Some(&mut voices))
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Worker: STT transcription error on chunk #{}: {}",
                                chunk_idx,
                                e
                            );
                            DecodeOutcome::failed()
                        }
                    }
                };

                if let Some(rejection) = outcome.rejection.as_ref() {
                    tracing::warn!(
                        "Worker: chunk #{} rejected as non-speech — {} ({} words discarded)",
                        chunk_idx,
                        rejection.reason.describe(),
                        rejection.discarded_word_count
                    );
                }

                // Anything that is not clean speech breaks the context chain.
                // Carrying a gap, a failure, or a loop forward is what turns one
                // bad chunk into a run of them.
                previous_tail = outcome.carry.clone().unwrap_or_default();

                let text = outcome.text;

                // 3. Persist the transcript segment.
                let segment = TranscriptSegment {
                    chunk_index: chunk_idx,
                    start_time_s: start_s,
                    end_time_s: end_s,
                    text: text.clone(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                    status: outcome.status,
                    // Already measured on the chunk; carrying it onto the segment
                    // is what lets the processing pipeline attribute speakers by
                    // channel without any extra work or a second audio pass.
                    mic_had_audio: chunk.mic_had_audio,
                    sys_had_audio: chunk.sys_had_audio,
                    utterances: outcome.utterances,
                    speech: Some(profile),
                    rejection: outcome.rejection,
                };

                if let Err(e) = store.append_transcript_segment(&session_id, &segment) {
                    tracing::error!(
                        "Worker: failed to append transcript segment #{}: {}",
                        chunk_idx,
                        e
                    );
                }

                let segment_words = text.split_whitespace().count();
                let rejected = segment.status == TranscriptSegmentStatus::Rejected;
                let voiced_seconds = profile.voiced_seconds;
                let live_speaker_count = voices.speaker_count();
                let _ = store.update_session(&session_id, |session| {
                    session.transcript_segment_count += 1;
                    session.word_count += segment_words;
                    session.live_speaker_count = live_speaker_count;
                    if rejected {
                        session.rejected_chunk_count += 1;
                    }
                    session.voiced_seconds += voiced_seconds;
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

    /// A profile describing audio that really did contain speech.
    fn voiced(seconds: f64) -> SpeechProfile {
        SpeechProfile {
            voiced_seconds: seconds,
            total_seconds: 30.0,
            peak_amplitude: 0.6,
            rms: 0.08,
            noise_floor_rms: 0.002,
        }
    }

    /// A profile describing thirty seconds of room tone.
    fn near_silent() -> SpeechProfile {
        SpeechProfile {
            voiced_seconds: 0.0,
            total_seconds: 30.0,
            peak_amplitude: 0.03,
            rms: 0.006,
            noise_floor_rms: 0.0055,
        }
    }

    #[test]
    fn the_reported_failure_is_rejected_rather_than_stored_as_speech() {
        // Chunk 11 of the reported meeting: 30 seconds of room tone that
        // Whisper filled with subtitle boilerplate.
        let chunk = chunk_with_track(track(&[(QUIET, QUIET); 30]), 330.0, 30.0);
        let decoded: Vec<SttUtterance> = (0..15)
            .map(|i| {
                SttUtterance {
                    start_s: i as f64 * 2.0,
                    end_s: i as f64 * 2.0 + 2.0,
                    text: "Thank you.".to_string(),
                    no_speech_prob: 0.55,
                }
            })
            .collect();

        let outcome = resolve_decode(&decoded, &chunk, &near_silent(), None);

        assert_eq!(outcome.status, TranscriptSegmentStatus::Rejected);
        assert!(outcome.text.is_empty(), "rejected text must not be stored as speech");
        assert!(outcome.utterances.is_empty());
        let rejection = outcome.rejection.expect("a rejection must be recorded");
        assert!(
            rejection.discarded_text.contains("Thank you"),
            "the discarded text is the evidence the rejection was right"
        );
        assert!(rejection.discarded_word_count >= 30);
    }

    #[test]
    fn a_rejected_chunk_never_prompts_the_next_one() {
        let chunk = chunk_with_track(track(&[(QUIET, QUIET); 30]), 330.0, 30.0);
        let decoded = vec![utterance(0.0, 30.0, "Thank you. Thank you. Thank you. Thank you.")];
        let outcome = resolve_decode(&decoded, &chunk, &near_silent(), None);
        assert_eq!(
            outcome.carry, None,
            "carrying a loop forward is what turned one bad chunk into nine"
        );
    }

    #[test]
    fn real_speech_survives_and_is_carried_forward() {
        let chunk = chunk_with_track(track(&[(LOUD, QUIET); 30]), 0.0, 30.0);
        let decoded = vec![
            utterance(0.0, 6.0, "So the placement numbers came in at forty-one this month."),
            utterance(6.0, 12.0, "That is ahead of the plan we set in July."),
        ];

        let outcome = resolve_decode(&decoded, &chunk, &voiced(11.0), None);

        assert_eq!(outcome.status, TranscriptSegmentStatus::Success);
        assert!(outcome.text.contains("forty-one"));
        assert_eq!(outcome.utterances.len(), 2);
        assert!(outcome.carry.is_some(), "clean speech must prompt the next chunk");
    }

    #[test]
    fn filler_padding_is_dropped_without_losing_the_sentence_beside_it() {
        // Whisper's habitual shape: one real sentence, then boilerplate for the
        // rest of the window. Rejecting the whole chunk would lose the sentence.
        let chunk = chunk_with_track(track(&[(LOUD, QUIET); 30]), 0.0, 30.0);
        let mut decoded = vec![utterance(
            0.0,
            5.0,
            "Pranjali will send the placement sheet by Thursday.",
        )];
        for i in 0..8 {
            decoded.push(SttUtterance {
                start_s: 6.0 + i as f64 * 3.0,
                end_s: 9.0 + i as f64 * 3.0,
                text: "Thank you.".to_string(),
                no_speech_prob: 0.9,
            });
        }

        let outcome = resolve_decode(&decoded, &chunk, &voiced(5.0), None);

        assert_eq!(outcome.status, TranscriptSegmentStatus::Success);
        assert!(outcome.text.contains("placement sheet"));
        assert!(
            !outcome.text.contains("Thank you"),
            "text was {:?}",
            outcome.text
        );
        assert_eq!(outcome.utterances.len(), 1);
    }

    #[test]
    fn a_decode_that_produced_nothing_at_all_is_empty_not_rejected() {
        let chunk = chunk_with_track(track(&[(QUIET, QUIET); 30]), 0.0, 30.0);
        let outcome = resolve_decode(&[], &chunk, &near_silent(), None);
        assert_eq!(outcome.status, TranscriptSegmentStatus::Empty);
        assert!(outcome.rejection.is_none());
    }

    #[test]
    fn the_no_speech_probability_is_weighted_by_how_long_each_span_was() {
        let spans = vec![
            SttUtterance {
                start_s: 0.0,
                end_s: 20.0,
                text: "twenty seconds of confident speech".into(),
                no_speech_prob: 0.02,
            },
            SttUtterance {
                start_s: 20.0,
                end_s: 20.5,
                text: "hm".into(),
                no_speech_prob: 0.95,
            },
        ];
        let mean = mean_no_speech_prob(&spans);
        assert!(
            mean < 0.1,
            "half a second of doubt must not condemn twenty seconds of speech; got {mean}"
        );
        assert_eq!(mean_no_speech_prob(&[]), 0.0);
    }

    #[test]
    fn a_polite_closing_thank_you_over_real_speech_is_not_thrown_away() {
        let chunk = chunk_with_track(track(&[(LOUD, QUIET); 30]), 0.0, 30.0);
        let decoded = vec![utterance(0.0, 2.0, "Thank you.")];
        let outcome = resolve_decode(&decoded, &chunk, &voiced(1.8), None);
        assert_eq!(outcome.status, TranscriptSegmentStatus::Success);
        assert_eq!(outcome.text, "Thank you.");
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

        let attributed = attribute_utterances(&decoded, &chunk, None);

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
        let attributed = attribute_utterances(&[utterance(0.5, 1.5, "somewhere in the middle")], &chunk, None);
        assert_eq!(attributed[0].start_time_s, 120.5);
        assert_eq!(attributed[0].end_time_s, 121.5);
        assert_eq!(attributed[0].index, 0);
    }

    #[test]
    fn a_span_whisper_reports_past_the_audio_is_clamped() {
        let chunk = chunk_with_track(track(&[(LOUD, QUIET)]), 0.0, 1.0);
        let attributed = attribute_utterances(&[utterance(0.0, 30.0, "overrun")], &chunk, None);
        assert_eq!(attributed[0].end_time_s, 1.0);
    }

    #[test]
    fn a_chunk_without_a_track_falls_back_to_the_chunk_flags() {
        // A transcript recorded before v2.5 carries no channel track.
        let mut chunk = chunk_with_track(Vec::new(), 0.0, 30.0);
        chunk.mic_had_audio = true;
        chunk.sys_had_audio = false;

        let attributed = attribute_utterances(&[utterance(0.0, 3.0, "legacy chunk")], &chunk, None);
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
        let attributed = attribute_utterances(&[utterance(0.0, 1.0, "quiet but decoded")], &chunk, None);
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
