use crate::providers::ProviderConfig;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SettingsError {
    #[error("Settings IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Settings JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}

/// Global hotkeys. Syntax follows `tauri-plugin-global-shortcut`'s shortcut
/// string format, e.g. "Ctrl+Shift+Space", "Ctrl+Space".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeySettings {
    /// Toggles the main window's visibility/focus from anywhere in the OS.
    pub show_hide_hotkey: String,
    /// Push-to-talk: hold to dictate into whatever field currently has OS focus.
    pub dictation_hotkey: String,
    /// When true, the dictation hotkey toggles instead of requiring a
    /// press-and-hold: one press starts recording, a second press stops it,
    /// and simply releasing the key in between does nothing. Meant for
    /// longer dictations where holding a key down the whole time is
    /// tedious. Defaults to `false` (hold-to-talk), preserving existing
    /// behavior for anyone who hasn't opted in.
    #[serde(default)]
    pub toggle_to_talk: bool,
}

impl Default for HotkeySettings {
    fn default() -> Self {
        Self {
            show_hide_hotkey: "Ctrl+Shift+Space".to_string(),
            dictation_hotkey: "Ctrl+Space".to_string(),
            toggle_to_talk: false,
        }
    }
}

/// Local speech-to-text configuration (whisper.cpp via whisper-rs).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SttSettings {
    /// Path to a GGML Whisper model file (e.g. `ggml-base.en.bin`). Download
    /// one from https://huggingface.co/ggerganov/whisper.cpp/tree/main and
    /// point this at it — Relay does not bundle a model.
    pub whisper_model_path: Option<String>,
}

/// Local text-to-speech configuration (Piper). Both fields must be set for
/// the "speak back" feature in voice chat; otherwise it silently degrades
/// to text-only, matching the zero-cost-by-default constraint.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TtsSettings {
    pub piper_binary_path: Option<String>,
    pub piper_voice_path: Option<String>,
}

/// Which edge of the active monitor's work area the floating pill anchors
/// to. "Center" always means centered along that edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PillPosition {
    #[default]
    BottomCenter,
    TopCenter,
    LeftCenter,
    RightCenter,
}

/// General UI/window behavior that isn't tied to a specific capture engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiSettings {
    /// Which edge of the screen the floating pill anchors to.
    #[serde(default)]
    pub pill_position: PillPosition,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            pill_position: PillPosition::default(),
        }
    }
}

/// Where Relay's local Vault (notes, Kanban cards, Voice Notes) lives on
/// disk. `directory` is `None` until the user explicitly chooses or
/// confirms a location — via the Voice Note first-time setup flow, or
/// Settings → Vault & LanceDB — at which point it holds an absolute
/// filesystem path. Left unset, the app keeps using its existing
/// process-relative default so nothing already working moves silently.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VaultSettings {
    pub directory: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppSettings {
    #[serde(default)]
    pub provider: ProviderConfig,
    #[serde(default)]
    pub stt: SttSettings,
    #[serde(default)]
    pub tts: TtsSettings,
    #[serde(default)]
    pub hotkeys: HotkeySettings,
    #[serde(default)]
    pub ui: UiSettings,
    #[serde(default)]
    pub vault: VaultSettings,
}

impl AppSettings {
    pub fn load(path: &Path) -> Result<Self, SettingsError> {
        if !path.exists() {
            let defaults = Self::default();
            defaults.save(path)?;
            return Ok(defaults);
        }

        let content = fs::read_to_string(path)?;
        // Fall back to defaults on a corrupt/partial file rather than
        // refusing to start the app.
        Ok(serde_json::from_str(&content).unwrap_or_default())
    }

    pub fn save(&self, path: &Path) -> Result<(), SettingsError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}
