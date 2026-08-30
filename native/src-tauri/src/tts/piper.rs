//! The Piper TTS provider.
//!
//! Local, offline and zero-cost: Relay spawns a user-supplied Piper
//! binary with a user-supplied voice model, writes text to its stdin and
//! reads the WAV it produces. No linking, so Piper's licence never
//! touches Relay's — which matters now that upstream has moved:
//! `rhasspy/piper` was archived read-only in October 2025 and active work
//! continues at `OHF-Voice/piper1-gpl` under GPL-3.0. Both binaries drive
//! identically here, and GPL-3.0 is in any case compatible with Relay's
//! AGPL-3.0. See `docs/talkback/RESEARCH.md` §B.
//!
//! Piper synthesizes a whole utterance and exits — it has no streaming
//! mode this provider can use. Talkback works around that by feeding it
//! one *sentence* at a time (`talkback::chunk`), so time-to-first-audio
//! is the cost of the first sentence rather than the whole answer.

use super::{TtsAudio, TtsCapabilities, TtsError, TtsProvider};
use crate::settings::TtsSettings;
use base64::Engine;
use std::io::Write;
use std::process::{Command, Stdio};

pub struct PiperProvider {
    binary_path: Option<String>,
    voice_path: Option<String>,
}

impl PiperProvider {
    pub fn from_settings(settings: &TtsSettings) -> Self {
        Self {
            binary_path: non_empty(settings.piper_binary_path.as_deref()),
            voice_path: non_empty(settings.piper_voice_path.as_deref()),
        }
    }
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

impl TtsProvider for PiperProvider {
    fn name(&self) -> &'static str {
        "piper"
    }

    fn capabilities(&self) -> TtsCapabilities {
        TtsCapabilities {
            intra_utterance_streaming: false,
            cancellable: true,
            voices: false,
        }
    }

    fn is_configured(&self) -> bool {
        self.binary_path.is_some() && self.voice_path.is_some()
    }

    fn synthesize(&self, text: &str) -> Result<Option<TtsAudio>, TtsError> {
        let (Some(binary), Some(voice)) = (&self.binary_path, &self.voice_path) else {
            return Ok(None);
        };
        let text = text.trim();
        if text.is_empty() {
            return Ok(None);
        }

        let out_path = std::env::temp_dir().join(format!("relay_tts_{}.wav", uuid::Uuid::new_v4()));

        let mut child = Command::new(binary)
            .arg("--model")
            .arg(voice)
            .arg("--output_file")
            .arg(&out_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| TtsError::SpawnFailed(e.to_string()))?;

        if let Some(stdin) = child.stdin.as_mut() {
            stdin
                .write_all(text.as_bytes())
                .map_err(|e| TtsError::SpawnFailed(e.to_string()))?;
        }

        let output = child.wait_with_output()?;
        if !output.status.success() {
            // Best-effort cleanup: a failed run can still have created a
            // partial file, and leaving it behind fills the temp dir one
            // phrase at a time now that Talkback synthesizes per sentence.
            let _ = std::fs::remove_file(&out_path);
            return Err(TtsError::SynthesisFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        let audio_bytes = std::fs::read(&out_path)?;
        let _ = std::fs::remove_file(&out_path);

        Ok(Some(TtsAudio {
            wav_base64: base64::engine::general_purpose::STANDARD.encode(audio_bytes),
            char_count: text.chars().count(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_requires_both_paths() {
        let both = PiperProvider::from_settings(&TtsSettings {
            piper_binary_path: Some("piper".into()),
            piper_voice_path: Some("voice.onnx".into()),
        });
        assert!(both.is_configured());

        let binary_only = PiperProvider::from_settings(&TtsSettings {
            piper_binary_path: Some("piper".into()),
            piper_voice_path: None,
        });
        assert!(!binary_only.is_configured());

        let neither = PiperProvider::from_settings(&TtsSettings::default());
        assert!(!neither.is_configured());
    }

    #[test]
    fn whitespace_paths_are_normalized_away() {
        let provider = PiperProvider::from_settings(&TtsSettings {
            piper_binary_path: Some("  ".into()),
            piper_voice_path: Some("\t\n".into()),
        });
        assert!(!provider.is_configured());
        assert!(provider.synthesize("hello").unwrap().is_none());
    }

    #[test]
    fn paths_are_trimmed_rather_than_rejected() {
        let provider = PiperProvider::from_settings(&TtsSettings {
            piper_binary_path: Some("  piper  ".into()),
            piper_voice_path: Some(" voice.onnx ".into()),
        });
        assert!(provider.is_configured());
    }

    #[test]
    fn an_unconfigured_provider_returns_none_not_an_error() {
        let provider = PiperProvider::from_settings(&TtsSettings::default());
        assert!(provider.synthesize("anything").unwrap().is_none());
    }

    #[test]
    fn blank_text_short_circuits_before_spawning() {
        // The binary does not exist; reaching spawn would be an Err.
        let provider = PiperProvider::from_settings(&TtsSettings {
            piper_binary_path: Some("/definitely/not/a/real/piper".into()),
            piper_voice_path: Some("/definitely/not/a/voice.onnx".into()),
        });
        assert!(provider.synthesize("   \n ").unwrap().is_none());
    }

    #[test]
    fn a_missing_binary_surfaces_as_a_spawn_error() {
        let provider = PiperProvider::from_settings(&TtsSettings {
            piper_binary_path: Some("/definitely/not/a/real/piper".into()),
            piper_voice_path: Some("/definitely/not/a/voice.onnx".into()),
        });
        assert!(matches!(
            provider.synthesize("hello"),
            Err(TtsError::SpawnFailed(_))
        ));
    }
}
