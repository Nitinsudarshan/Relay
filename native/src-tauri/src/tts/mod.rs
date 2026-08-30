//! Text-to-speech, behind a provider boundary.
//!
//! Before Talkback this module was one struct that shelled out to Piper.
//! It is now a trait with Piper as the first implementation, because
//! every voice-agent architecture worth copying (Pipecat, LiveKit, and
//! the commercial systems surveyed in `docs/talkback/RESEARCH.md`)
//! converges on the same lesson: the TTS engine is the component most
//! likely to be swapped, and hard-coding one is how a voice product ends
//! up unable to change its voice.
//!
//! Candidate second providers, with their evidence, are in
//! `RESEARCH.md` §B — Kokoro-82M (Apache-2.0 weights, Rust ONNX ports)
//! is the leading one. None ships in V1: the honest order is trait
//! first, benchmark second, dependency third.

use crate::settings::TtsSettings;
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod piper;
pub use piper::PiperProvider;

#[derive(Error, Debug)]
pub enum TtsError {
    #[error("Failed to spawn the TTS process: {0}")]
    SpawnFailed(String),

    #[error("The TTS engine exited with an error: {0}")]
    SynthesisFailed(String),

    #[error("Failed to read synthesized audio: {0}")]
    IoError(#[from] std::io::Error),
}

/// One synthesized utterance, ready to hand to the WebView for playback.
///
/// Base64 WAV rather than raw PCM because playback lives in the frontend,
/// not in Rust. That is a deliberate architectural choice, not laziness:
/// `rodio` — the obvious Rust playback crate — requires `cpal ^0.17`
/// while Relay pins `cpal 0.15`, so adding it would put two WASAPI stacks
/// in one process (`RESEARCH.md` §E.4). The Web Audio API also gives
/// interruption for free, which is the one thing barge-in needs most.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsAudio {
    pub wav_base64: String,
    /// Characters synthesized, for latency accounting.
    pub char_count: usize,
}

/// What a provider can actually do, so callers can degrade honestly
/// instead of assuming.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TtsCapabilities {
    /// True only for engines that emit audio *within* one utterance.
    /// Piper is false: Talkback gets its streaming behaviour from the
    /// phrase buffer (`talkback::chunk`), which is why a batch engine
    /// still produces audio a sentence at a time.
    pub intra_utterance_streaming: bool,
    /// Whether an in-flight synthesis can be abandoned. For an
    /// out-of-process engine this means the child can be killed;
    /// Talkback additionally checks its cancellation token between
    /// phrases, so barge-in is bounded by one short phrase either way.
    pub cancellable: bool,
    /// Whether the provider exposes selectable voices.
    pub voices: bool,
}

/// A voice a provider can speak with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TtsVoice {
    pub id: String,
    pub label: String,
}

/// Everything Talkback needs from a speech engine.
///
/// Implementations must be cheap to construct — one is resolved per turn
/// from settings — and must never panic: a TTS failure degrades to a
/// text-only answer, it does not fail the turn.
pub trait TtsProvider: Send + Sync {
    /// Stable identifier, used in logs and in the settings UI.
    fn name(&self) -> &'static str;

    fn capabilities(&self) -> TtsCapabilities;

    /// False when the provider has no usable configuration. Callers skip
    /// synthesis entirely rather than collecting an error per phrase.
    fn is_configured(&self) -> bool;

    /// Synthesizes one phrase. `Ok(None)` means "not configured" — a
    /// normal, non-error outcome — while `Err` means a configured engine
    /// actually failed.
    fn synthesize(&self, text: &str) -> Result<Option<TtsAudio>, TtsError>;

    fn voices(&self) -> Vec<TtsVoice> {
        Vec::new()
    }
}

/// A provider that speaks nothing, for when TTS is unconfigured.
///
/// Exists so the engine has no `Option<Box<dyn TtsProvider>>` to unwrap
/// and no "if TTS is on" branch: a Talkback turn is always text plus
/// zero-or-more audio phrases.
pub struct NullProvider;

impl TtsProvider for NullProvider {
    fn name(&self) -> &'static str {
        "none"
    }

    fn capabilities(&self) -> TtsCapabilities {
        TtsCapabilities {
            intra_utterance_streaming: false,
            cancellable: true,
            voices: false,
        }
    }

    fn is_configured(&self) -> bool {
        false
    }

    fn synthesize(&self, _text: &str) -> Result<Option<TtsAudio>, TtsError> {
        Ok(None)
    }
}

/// Picks the provider for the current settings.
///
/// One place to add Kokoro (or any future engine) once it has been
/// benchmarked, rather than a provider check at every call site.
pub fn resolve_provider(settings: &TtsSettings) -> Box<dyn TtsProvider> {
    let piper = PiperProvider::from_settings(settings);
    if piper.is_configured() {
        Box::new(piper)
    } else {
        Box::new(NullProvider)
    }
}

/// Back-compatible facade for the pre-Talkback call shape.
///
/// Kept because `TtsEngine::synthesize(settings, text)` was the entire
/// public surface of this module and deleting it would be a gratuitous
/// break for no gain.
pub struct TtsEngine;

impl TtsEngine {
    /// Returns base64-encoded WAV audio of `text`, or `None` when no
    /// engine is configured.
    pub fn synthesize(settings: &TtsSettings, text: &str) -> Result<Option<String>, TtsError> {
        Ok(resolve_provider(settings)
            .synthesize(text)?
            .map(|audio| audio.wav_base64))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unconfigured_setting_resolves_to_the_null_provider() {
        let provider = resolve_provider(&TtsSettings::default());
        assert_eq!(provider.name(), "none");
        assert!(!provider.is_configured());
        assert!(provider.synthesize("hello").unwrap().is_none());
    }

    #[test]
    fn blank_paths_are_treated_as_unconfigured() {
        let settings = TtsSettings {
            piper_binary_path: Some("   ".to_string()),
            piper_voice_path: Some(String::new()),
        };
        assert_eq!(resolve_provider(&settings).name(), "none");
    }

    #[test]
    fn a_half_configured_piper_is_not_selected() {
        let settings = TtsSettings {
            piper_binary_path: Some("/usr/bin/piper".to_string()),
            piper_voice_path: None,
        };
        assert_eq!(
            resolve_provider(&settings).name(),
            "none",
            "a binary without a voice model cannot speak"
        );
    }

    #[test]
    fn a_fully_configured_piper_is_selected() {
        let settings = TtsSettings {
            piper_binary_path: Some("/usr/bin/piper".to_string()),
            piper_voice_path: Some("/voices/en_US.onnx".to_string()),
        };
        let provider = resolve_provider(&settings);
        assert_eq!(provider.name(), "piper");
        assert!(provider.is_configured());
    }

    #[test]
    fn piper_reports_no_intra_utterance_streaming() {
        let settings = TtsSettings {
            piper_binary_path: Some("/usr/bin/piper".to_string()),
            piper_voice_path: Some("/voices/en_US.onnx".to_string()),
        };
        let capabilities = resolve_provider(&settings).capabilities();
        assert!(
            !capabilities.intra_utterance_streaming,
            "Piper writes a whole WAV and exits; claiming otherwise would \
             let the engine skip the phrase buffer"
        );
        assert!(capabilities.cancellable);
    }

    #[test]
    fn the_legacy_facade_still_degrades_to_none() {
        assert!(TtsEngine::synthesize(&TtsSettings::default(), "hi")
            .unwrap()
            .is_none());
    }

    #[test]
    fn empty_text_is_never_synthesized() {
        let settings = TtsSettings {
            piper_binary_path: Some("/nonexistent/piper".to_string()),
            piper_voice_path: Some("/nonexistent/voice.onnx".to_string()),
        };
        // Must not reach the (missing) binary at all, so no spawn error.
        assert!(resolve_provider(&settings).synthesize("   ").unwrap().is_none());
    }
}
