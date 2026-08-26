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
    #[serde(default = "default_show_hide_hotkey")]
    pub show_hide_hotkey: String,
    /// Push-to-talk: hold to dictate into whatever field currently has OS focus.
    #[serde(default = "default_dictation_hotkey")]
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

fn default_show_hide_hotkey() -> String {
    "Ctrl+Shift+Space".to_string()
}

fn default_dictation_hotkey() -> String {
    "Ctrl+Space".to_string()
}

impl Default for HotkeySettings {
    fn default() -> Self {
        Self {
            show_hide_hotkey: default_show_hide_hotkey(),
            dictation_hotkey: default_dictation_hotkey(),
            toggle_to_talk: false,
        }
    }
}

/// Local speech-to-text configuration (whisper.cpp via whisper-rs).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SttSettings {
    /// Path to a GGML Whisper model file (e.g. `ggml-small.bin`). Download
    /// one from https://huggingface.co/ggerganov/whisper.cpp/tree/main and
    /// point this at it — Relay does not bundle a model.
    pub whisper_model_path: Option<String>,
    /// Whether domain vocabulary initial prompting is enabled. Defaults to false.
    #[serde(default, alias = "enableInitialPrompt")]
    pub enable_initial_prompt: bool,
    /// Optional user-defined technical vocabulary prompt.
    #[serde(default, alias = "customInitialPrompt")]
    pub custom_initial_prompt: Option<String>,
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
    BottomLeft,
    #[default]
    BottomCenter,
    BottomRight,
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

/// Language and script preferences.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LanguageSettings {
    /// Primary language for dictation: ISO code (e.g. "en", "hi", "kn", "ta").
    #[serde(default = "default_primary_dictation_language", alias = "primaryDictationLanguage")]
    pub primary_dictation_language: String,

    /// Languages the user speaks: ISO codes (e.g. ["en", "hi"]).
    #[serde(default = "default_spoken_languages", alias = "spokenLanguages")]
    pub spoken_languages: Vec<String>,

    /// Target language for generated notes and summaries: ISO code (e.g. "en", "hi").
    #[serde(default = "default_notes_language", alias = "notesLanguage")]
    pub notes_language: String,

    /// Writing script rule for dictation/notes: "latin" (Romanized) or "native".
    #[serde(default = "default_output_script", alias = "outputScript")]
    pub output_script: String,
}

fn default_primary_dictation_language() -> String {
    "en".to_string()
}

fn default_spoken_languages() -> Vec<String> {
    vec!["en".to_string()]
}

fn default_notes_language() -> String {
    "en".to_string()
}

fn default_output_script() -> String {
    "latin".to_string()
}

impl Default for LanguageSettings {
    fn default() -> Self {
        Self {
            primary_dictation_language: default_primary_dictation_language(),
            spoken_languages: default_spoken_languages(),
            notes_language: default_notes_language(),
            output_script: default_output_script(),
        }
    }
}

/// Privacy-safe diagnostic telemetry and onboarding consent preferences.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticsSettings {
    /// Whether anonymous diagnostics (app crashes, version, platform) are enabled.
    /// STRICT PRIVACY: User notes, scribbles, and audio are never transmitted.
    #[serde(default = "default_allow_anonymous_diagnostics", alias = "allowAnonymousDiagnostics")]
    pub allow_anonymous_diagnostics: bool,

    /// Whether the user has completed or dismissed the initial first-run onboarding screen.
    #[serde(default, alias = "firstRunCompleted")]
    pub first_run_completed: bool,
}

fn default_allow_anonymous_diagnostics() -> bool {
    false
}

impl Default for DiagnosticsSettings {
    fn default() -> Self {
        Self {
            allow_anonymous_diagnostics: default_allow_anonymous_diagnostics(),
            first_run_completed: false,
        }
    }
}

/// Supabase Cloud configuration for Relay Hybrid authentication and sync.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CloudSettings {
    #[serde(default, alias = "supabaseUrl")]
    pub supabase_url: Option<String>,
    #[serde(default, alias = "supabaseAnonKey")]
    pub supabase_anon_key: Option<String>,
}

/// Audio feedback and sound effects preferences.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SoundSettings {
    /// Whether sound effects (start/stop tones) are played during dictation.
    #[serde(default = "default_dictation_sounds", alias = "dictationSounds")]
    pub dictation_sounds: bool,
}

fn default_dictation_sounds() -> bool {
    true
}

impl Default for SoundSettings {
    fn default() -> Self {
        Self {
            dictation_sounds: default_dictation_sounds(),
        }
    }
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
    #[serde(default)]
    pub language: LanguageSettings,
    #[serde(default)]
    pub diagnostics: DiagnosticsSettings,
    #[serde(default)]
    pub cloud: CloudSettings,
    #[serde(default)]
    pub sound: SoundSettings,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pill_position_defaults() {
        assert_eq!(PillPosition::default(), PillPosition::BottomCenter);
        assert_eq!(UiSettings::default().pill_position, PillPosition::BottomCenter);
    }

    #[test]
    fn test_pill_position_serialization() {
        assert_eq!(
            serde_json::to_string(&PillPosition::BottomLeft).unwrap(),
            "\"bottom_left\""
        );
        assert_eq!(
            serde_json::to_string(&PillPosition::BottomCenter).unwrap(),
            "\"bottom_center\""
        );
        assert_eq!(
            serde_json::to_string(&PillPosition::BottomRight).unwrap(),
            "\"bottom_right\""
        );

        assert_eq!(
            serde_json::from_str::<PillPosition>("\"bottom_left\"").unwrap(),
            PillPosition::BottomLeft
        );
        assert_eq!(
            serde_json::from_str::<PillPosition>("\"bottom_center\"").unwrap(),
            PillPosition::BottomCenter
        );
        assert_eq!(
            serde_json::from_str::<PillPosition>("\"bottom_right\"").unwrap(),
            PillPosition::BottomRight
        );
    }

    #[test]
    fn test_language_settings_defaults() {
        let defaults = LanguageSettings::default();
        assert_eq!(defaults.primary_dictation_language, "en");
        assert_eq!(defaults.spoken_languages, vec!["en".to_string()]);
        assert_eq!(defaults.notes_language, "en");
        assert_eq!(defaults.output_script, "latin");
    }

    #[test]
    fn test_language_settings_backward_compatibility() {
        // Empty JSON should deserialize with full defaults
        let app_settings: AppSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(app_settings.language.primary_dictation_language, "en");
        assert_eq!(app_settings.language.spoken_languages, vec!["en".to_string()]);
        assert_eq!(app_settings.language.notes_language, "en");
        assert_eq!(app_settings.language.output_script, "latin");

        // Partial JSON with legacy settings and no language field
        let legacy_json = r#"{
            "stt": { "whisper_model_path": "models/ggml-base.bin" },
            "hotkeys": { "dictation_hotkey": "Ctrl+Space" }
        }"#;
        let loaded: AppSettings = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(loaded.language.primary_dictation_language, "en");
        assert_eq!(loaded.language.spoken_languages, vec!["en"]);
        assert_eq!(loaded.stt.whisper_model_path.as_deref(), Some("models/ggml-base.bin"));
    }

    #[test]
    fn test_language_settings_camel_case_aliases() {
        let camel_json = r#"{
            "language": {
                "primaryDictationLanguage": "hi",
                "spokenLanguages": ["en", "hi"],
                "notesLanguage": "en",
                "outputScript": "latin"
            }
        }"#;
        let loaded: AppSettings = serde_json::from_str(camel_json).unwrap();
        assert_eq!(loaded.language.primary_dictation_language, "hi");
        assert_eq!(loaded.language.spoken_languages, vec!["en", "hi"]);
        assert_eq!(loaded.language.notes_language, "en");
        assert_eq!(loaded.language.output_script, "latin");
    }

    #[test]
    fn test_language_settings_roundtrip() {
        let custom = LanguageSettings {
            primary_dictation_language: "kn".to_string(),
            spoken_languages: vec!["en".to_string(), "hi".to_string(), "kn".to_string()],
            notes_language: "en".to_string(),
            output_script: "latin".to_string(),
        };
        let json = serde_json::to_string(&custom).unwrap();
        let deserialized: LanguageSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(custom, deserialized);
    }

    #[test]
    fn test_language_settings_file_persistence_and_reload() {
        let dir = std::env::temp_dir().join(format!("relay_test_settings_{}", uuid::Uuid::new_v4()));
        let settings_path = dir.join("settings.json");

        let mut app_settings = AppSettings::default();
        app_settings.language = LanguageSettings {
            primary_dictation_language: "hi".to_string(),
            spoken_languages: vec!["hi".to_string(), "en".to_string()],
            notes_language: "en".to_string(),
            output_script: "latin".to_string(),
        };

        app_settings.save(&settings_path).expect("failed to save settings file");
        let reloaded = AppSettings::load(&settings_path).expect("failed to reload settings file");

        assert_eq!(reloaded.language.primary_dictation_language, "hi");
        assert_eq!(reloaded.language.spoken_languages, vec!["hi", "en"]);
        assert_eq!(reloaded.language.notes_language, "en");
        assert_eq!(reloaded.language.output_script, "latin");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_sound_settings_defaults_and_serialization() {
        let defaults = SoundSettings::default();
        assert!(defaults.dictation_sounds);

        let app_settings: AppSettings = serde_json::from_str("{}").unwrap();
        assert!(app_settings.sound.dictation_sounds);

        let custom_json = r#"{
            "sound": {
                "dictation_sounds": false
            }
        }"#;
        let loaded: AppSettings = serde_json::from_str(custom_json).unwrap();
        assert!(!loaded.sound.dictation_sounds);

        let camel_json = r#"{
            "sound": {
                "dictationSounds": false
            }
        }"#;
        let camel_loaded: AppSettings = serde_json::from_str(camel_json).unwrap();
        assert!(!camel_loaded.sound.dictation_sounds);
    }
}
