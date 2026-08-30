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
//!
//! ## Configuration is a product, not a settings key
//!
//! [`discovery`] finds a Piper installation Relay put in place itself,
//! and [`status`] reports exactly what is missing in terms a user can act
//! on. Between them, "how do I make Talkback speak?" has a deterministic
//! answer that never involves editing JSON.

use crate::settings::TtsSettings;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

pub mod discovery;
mod piper;

pub use discovery::{PiperOrigin, PiperVoice, TtsProblem};
pub use piper::PiperProvider;

#[derive(Error, Debug)]
pub enum TtsError {
    #[error("Failed to spawn the TTS process: {0}")]
    SpawnFailed(String),

    #[error("The TTS engine exited with an error: {0}")]
    SynthesisFailed(String),

    #[error("Failed to read synthesized audio: {0}")]
    IoError(#[from] std::io::Error),

    /// The configuration cannot work, whatever the phrase is.
    ///
    /// Distinct from [`SynthesisFailed`](Self::SynthesisFailed), whose
    /// permanence has to be guessed from the engine's stderr. Relay
    /// raises this when it has concluded so itself — a successful exit
    /// that produced no audio, for instance — so the judgement is not
    /// re-derived by matching against a string Relay wrote.
    #[error("{0}")]
    Unusable(String),

    /// The turn was superseded while this phrase was being synthesized.
    ///
    /// A distinct variant rather than a generic failure because
    /// cancellation is a *normal conversational event* — the user talked
    /// over the agent — and must never surface as an error to them
    /// (`docs/talkback/ARCHITECTURE.md` §8).
    #[error("Synthesis was cancelled")]
    Cancelled,
}

impl TtsError {
    /// Whether this failure will recur for every subsequent phrase.
    ///
    /// A missing binary or an unreadable model does not fix itself, so
    /// retrying once per sentence produces a stream of identical errors
    /// and a stream of process spawns. The engine latches TTS off on a
    /// permanent failure and leaves a transient one alone.
    pub fn is_permanent(&self) -> bool {
        match self {
            TtsError::Cancelled => false,
            TtsError::SpawnFailed(_) => true,
            TtsError::Unusable(_) => true,
            TtsError::IoError(_) => false,
            TtsError::SynthesisFailed(message) => {
                let message = message.to_lowercase();
                // Piper's own words for "this configuration is wrong",
                // as opposed to a transient failure on one phrase.
                [
                    "no such file",
                    "not found",
                    "cannot find",
                    "config",
                    "onnx",
                    "invalid",
                    "failed to load",
                    "unable to load",
                ]
                .iter()
                .any(|marker| message.contains(marker))
            }
        }
    }
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
    /// Whether an in-flight synthesis can be abandoned. Piper is true:
    /// the child process is killed rather than waited out.
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

    /// Synthesizes one phrase, abandoning it if `is_cancelled` becomes
    /// true.
    ///
    /// `Ok(None)` means "not configured" — a normal, non-error outcome —
    /// while `Err` means a configured engine actually failed.
    fn synthesize_cancellable(
        &self,
        text: &str,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<TtsAudio>, TtsError>;

    /// Synthesizes without the possibility of cancellation. For callers
    /// with nothing to cancel, such as the settings "test voice" button.
    fn synthesize(&self, text: &str) -> Result<Option<TtsAudio>, TtsError> {
        self.synthesize_cancellable(text, &|| false)
    }

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

    fn synthesize_cancellable(
        &self,
        _text: &str,
        _is_cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<TtsAudio>, TtsError> {
        Ok(None)
    }
}

/// The complete picture of local voice configuration, for the settings UI
/// and the Talkback page banner.
///
/// One type rather than five commands, because every question the UI asks
/// ("is it ready", "which engine", "which voice", "what is wrong",
/// "where do I put files") is answered from the same filesystem check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsStatus {
    /// Provider name: `piper` or `none`.
    pub engine: String,
    /// Whether Talkback can speak right now.
    pub ready: bool,
    #[serde(default)]
    pub binary_path: Option<String>,
    /// How the binary was found, so the UI can say "found automatically"
    /// rather than showing a bare path with no explanation.
    #[serde(default)]
    pub binary_origin: Option<PiperOrigin>,
    #[serde(default)]
    pub voice_path: Option<String>,
    /// Display name of the selected voice: `en_US-amy-medium`.
    #[serde(default)]
    pub voice_label: Option<String>,
    #[serde(default)]
    pub voice_language: Option<String>,
    /// Voices Relay can offer without the user browsing for a file.
    pub available_voices: Vec<PiperVoice>,
    /// Everything blocking readiness, already phrased for display.
    pub problems: Vec<String>,
    /// Where to put a Piper executable so Relay finds it automatically.
    pub install_dir: String,
    /// Where to put voice models so Relay finds them automatically.
    pub voices_dir: String,
    /// The executable filename to look for, so the instructions can name
    /// it exactly on the platform the user is on.
    pub executable_name: String,
}

/// Resolves the effective Piper paths: settings first, discovery second.
///
/// Returning the origin matters for the UI — "found in Relay's voice
/// folder" and "you chose this" need different copy, and a user should
/// not have to guess which one is in effect.
pub fn resolve_piper_paths(
    settings: &TtsSettings,
    tts_root: &Path,
    resource_dir: Option<&Path>,
) -> (Option<PathBuf>, Option<PiperOrigin>, Option<PathBuf>) {
    let configured_binary = settings
        .piper_binary_path
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(PathBuf::from);

    let (binary, origin) = match configured_binary {
        Some(path) => (Some(path), Some(PiperOrigin::Configured)),
        None => match discovery::discover(tts_root, resource_dir) {
            Some(found) => (Some(found.path), Some(found.origin)),
            None => (None, None),
        },
    };

    let configured_voice = settings
        .piper_voice_path
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(PathBuf::from);

    // With no voice chosen, a single voice in the managed directory is an
    // unambiguous default — requiring a pick when there is only one thing
    // to pick is a step for its own sake.
    let voice = configured_voice.or_else(|| {
        let available = discovery::voices_in(&discovery::managed_voices_dir(tts_root));
        match available.as_slice() {
            [only] if only.has_config => Some(PathBuf::from(&only.path)),
            _ => None,
        }
    });

    (binary, origin, voice)
}

/// Picks the provider for the current settings and installation.
///
/// One place to add Kokoro (or any future engine) once it has been
/// benchmarked, rather than a provider check at every call site.
pub fn resolve_provider(
    settings: &TtsSettings,
    tts_root: &Path,
    resource_dir: Option<&Path>,
) -> Box<dyn TtsProvider> {
    let (binary, _, voice) = resolve_piper_paths(settings, tts_root, resource_dir);
    let piper = PiperProvider::new(binary, voice, discovery::tts_scratch_dir(tts_root));
    if piper.is_configured() {
        Box::new(piper)
    } else {
        Box::new(NullProvider)
    }
}

/// Everything the voice settings UI needs, in one filesystem pass.
pub fn status(
    settings: &TtsSettings,
    tts_root: &Path,
    resource_dir: Option<&Path>,
) -> TtsStatus {
    let (binary, origin, voice) = resolve_piper_paths(settings, tts_root, resource_dir);
    let provider = PiperProvider::new(
        binary.clone(),
        voice.clone(),
        discovery::tts_scratch_dir(tts_root),
    );
    let problems = provider.problems();
    let ready = problems.is_empty();

    let voice_meta = voice.as_deref().and_then(discovery::voice_from_path);

    TtsStatus {
        engine: if ready { "piper".to_string() } else { "none".to_string() },
        ready,
        binary_path: binary.map(|p| p.to_string_lossy().to_string()),
        binary_origin: origin,
        voice_path: voice.map(|p| p.to_string_lossy().to_string()),
        voice_label: voice_meta.as_ref().map(|v| v.label.clone()),
        voice_language: voice_meta.and_then(|v| v.language),
        available_voices: discovery::available_voices(
            tts_root,
            settings.piper_voice_path.as_deref(),
        ),
        problems: problems.iter().map(TtsProblem::message).collect(),
        install_dir: discovery::managed_piper_dir(tts_root)
            .to_string_lossy()
            .to_string(),
        voices_dir: discovery::managed_voices_dir(tts_root)
            .to_string_lossy()
            .to_string(),
        executable_name: discovery::piper_executable_name().to_string(),
    }
}

/// The sentence the "Test voice" button speaks.
///
/// Deliberately mentions Relay and is a natural sentence rather than a
/// word: a user testing a voice is judging prosody, not phonemes.
pub const TEST_PHRASE: &str = "This is how Relay will sound when Talkback speaks.";

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("relay_tts_mod_{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
        fn path(&self) -> &Path {
            &self.0
        }
        fn touch(&self, relative: &str) -> PathBuf {
            let path = self.0.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"x").unwrap();
            path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn make_executable(path: &Path) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        #[cfg(not(unix))]
        let _ = path;
    }

    /// A complete managed installation: binary plus one voice.
    fn installed(temp: &TempDir) -> TtsSettings {
        let binary = temp.touch(&format!("piper/{}", discovery::piper_executable_name()));
        make_executable(&binary);
        temp.touch("voices/en_US-amy-medium.onnx");
        temp.touch("voices/en_US-amy-medium.onnx.json");
        TtsSettings::default()
    }

    #[test]
    fn an_empty_installation_is_not_ready_and_says_why() {
        let temp = TempDir::new();
        let status = status(&TtsSettings::default(), temp.path(), None);

        assert!(!status.ready);
        assert_eq!(status.engine, "none");
        assert!(
            !status.problems.is_empty(),
            "an unusable configuration must explain itself"
        );
        assert!(status.available_voices.is_empty());
    }

    #[test]
    fn status_always_tells_the_user_where_to_put_files() {
        let temp = TempDir::new();
        let status = status(&TtsSettings::default(), temp.path(), None);
        assert!(status.install_dir.contains("piper"));
        assert!(status.voices_dir.contains("voices"));
        assert_eq!(status.executable_name, discovery::piper_executable_name());
    }

    #[test]
    fn a_managed_installation_is_discovered_and_ready_with_no_settings() {
        let temp = TempDir::new();
        let settings = installed(&temp);

        let status = status(&settings, temp.path(), None);
        assert!(status.ready, "problems: {:?}", status.problems);
        assert_eq!(status.engine, "piper");
        assert_eq!(status.binary_origin, Some(PiperOrigin::Managed));
        assert_eq!(status.voice_label.as_deref(), Some("en_US-amy-medium"));
        assert_eq!(status.voice_language.as_deref(), Some("en_US"));
        assert!(status.problems.is_empty());
    }

    #[test]
    fn a_sole_managed_voice_is_selected_without_being_configured() {
        let temp = TempDir::new();
        let settings = installed(&temp);
        let (_, _, voice) = resolve_piper_paths(&settings, temp.path(), None);
        assert!(
            voice.is_some_and(|v| v.ends_with("en_US-amy-medium.onnx")),
            "one available voice needs no picker"
        );
    }

    #[test]
    fn two_managed_voices_require_a_choice_rather_than_guessing() {
        let temp = TempDir::new();
        let settings = installed(&temp);
        temp.touch("voices/hi_IN-pratham-medium.onnx");
        temp.touch("voices/hi_IN-pratham-medium.onnx.json");

        let (_, _, voice) = resolve_piper_paths(&settings, temp.path(), None);
        assert!(voice.is_none(), "Relay must not pick a voice arbitrarily");

        let status = status(&settings, temp.path(), None);
        assert!(!status.ready);
        assert_eq!(status.available_voices.len(), 2, "both must be offered");
    }

    #[test]
    fn a_sole_voice_missing_its_sidecar_is_not_auto_selected() {
        let temp = TempDir::new();
        let binary = temp.touch(&format!("piper/{}", discovery::piper_executable_name()));
        make_executable(&binary);
        temp.touch("voices/en_US-amy-medium.onnx");

        let (_, _, voice) = resolve_piper_paths(&TtsSettings::default(), temp.path(), None);
        assert!(voice.is_none(), "an unusable voice must not be chosen");
    }

    #[test]
    fn explicit_settings_beat_discovery() {
        let temp = TempDir::new();
        installed(&temp);
        let elsewhere = temp.touch("custom/my-piper");
        make_executable(&elsewhere);

        let settings = TtsSettings {
            piper_binary_path: Some(elsewhere.to_string_lossy().to_string()),
            piper_voice_path: None,
        };
        let (binary, origin, _) = resolve_piper_paths(&settings, temp.path(), None);
        assert_eq!(binary.as_deref(), Some(elsewhere.as_path()));
        assert_eq!(origin, Some(PiperOrigin::Configured));
    }

    #[test]
    fn a_stale_configured_path_is_reported_rather_than_silently_rediscovered() {
        let temp = TempDir::new();
        installed(&temp);
        let settings = TtsSettings {
            piper_binary_path: Some(temp.path().join("gone/piper").to_string_lossy().to_string()),
            piper_voice_path: None,
        };

        let status = status(&settings, temp.path(), None);
        assert!(!status.ready);
        assert!(
            status.problems.iter().any(|p| p.contains("isn't at")),
            "silently falling back would hide the user's broken setting: {:?}",
            status.problems
        );
    }

    #[test]
    fn resolve_provider_returns_null_when_unconfigured() {
        let temp = TempDir::new();
        let provider = resolve_provider(&TtsSettings::default(), temp.path(), None);
        assert_eq!(provider.name(), "none");
        assert!(!provider.is_configured());
        assert!(provider.synthesize("hello").unwrap().is_none());
    }

    #[test]
    fn resolve_provider_returns_piper_when_installed() {
        let temp = TempDir::new();
        let settings = installed(&temp);
        let provider = resolve_provider(&settings, temp.path(), None);
        assert_eq!(provider.name(), "piper");
        assert!(provider.is_configured());
        assert!(provider.capabilities().cancellable);
        assert!(!provider.capabilities().intra_utterance_streaming);
    }

    #[test]
    fn permanent_failures_are_distinguished_from_transient_ones() {
        // These recur for every phrase, so the engine must stop retrying.
        assert!(TtsError::SpawnFailed("no such file".into()).is_permanent());
        assert!(TtsError::SynthesisFailed("model config not found".into()).is_permanent());
        assert!(TtsError::SynthesisFailed("failed to load onnx".into()).is_permanent());

        assert!(TtsError::Unusable("wrote no audio".into()).is_permanent());

        // These do not.
        assert!(!TtsError::Cancelled.is_permanent());
        assert!(!TtsError::SynthesisFailed("terminated by signal".into()).is_permanent());
        assert!(!TtsError::IoError(std::io::Error::other("disk busy")).is_permanent());
    }

    #[test]
    fn cancellation_is_never_treated_as_a_configuration_failure() {
        // The user talking over the agent must not disable their voice.
        assert!(!TtsError::Cancelled.is_permanent());
    }

    #[test]
    fn the_test_phrase_is_a_real_sentence() {
        assert!(TEST_PHRASE.ends_with('.'));
        assert!(TEST_PHRASE.split_whitespace().count() > 5);
    }
}
