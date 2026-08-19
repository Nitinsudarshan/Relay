use crate::settings::TtsSettings;
use base64::Engine;
use std::io::Write;
use std::process::{Command, Stdio};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TtsError {
    #[error("Failed to spawn Piper process: {0}")]
    SpawnFailed(String),

    #[error("Piper exited with an error: {0}")]
    SynthesisFailed(String),

    #[error("Failed to read synthesized audio: {0}")]
    IoError(#[from] std::io::Error),
}

/// Local, zero-cost text-to-speech via a user-provided Piper (https://github.com/rhasspy/piper)
/// binary + voice model. Both must be configured in settings; if either is
/// missing this degrades gracefully to "no audio" rather than failing the
/// whole voice-chat response, since TTS is a nice-to-have on top of the text answer.
pub struct TtsEngine;

impl TtsEngine {
    /// Returns base64-encoded WAV audio of `text` spoken aloud, or `None` if
    /// Piper isn't configured.
    pub fn synthesize(settings: &TtsSettings, text: &str) -> Result<Option<String>, TtsError> {
        let (Some(binary), Some(voice)) = (&settings.piper_binary_path, &settings.piper_voice_path)
        else {
            return Ok(None);
        };
        if binary.trim().is_empty() || voice.trim().is_empty() || text.trim().is_empty() {
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
            return Err(TtsError::SynthesisFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        let audio_bytes = std::fs::read(&out_path)?;
        let _ = std::fs::remove_file(&out_path);

        Ok(Some(
            base64::engine::general_purpose::STANDARD.encode(audio_bytes),
        ))
    }
}
