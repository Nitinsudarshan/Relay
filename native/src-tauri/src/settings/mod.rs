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

/// Performance and quality profile for Universal Dictation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DictationSttQuality {
    #[default]
    Fast,
    Accurate,
}

/// Local speech-to-text configuration (whisper.cpp via whisper-rs).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SttSettings {
    /// Path to a GGML Whisper model file (e.g. `ggml-small.bin`). Download
    /// one from https://huggingface.co/ggerganov/whisper.cpp/tree/main and
    /// point this at it — Relay does not bundle a model.
    pub whisper_model_path: Option<String>,
    /// Quality / performance profile specifically for Universal Dictation.
    /// Defaults to `Fast` (Base model) for low latency (~0.8s), while
    /// `Accurate` uses `ggml-small.bin` (~2.4s).
    #[serde(default, alias = "dictationQuality")]
    pub dictation_quality: DictationSttQuality,
    /// Explicit override for dictation thread count. Defaults to None (optimal thread pool).
    #[serde(default, alias = "dictationThreads")]
    pub dictation_threads: Option<i32>,
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

/// Clipboard injection and text retention preferences.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClipboardSettings {
    /// Automatically paste/type transcribed text into the active app when dictation finishes.
    #[serde(default = "default_auto_paste", alias = "autoPaste")]
    pub auto_paste: bool,
    /// Keep transcribed text in OS clipboard so you can paste it manually if needed.
    #[serde(default = "default_copy_to_clipboard", alias = "copyToClipboard")]
    pub copy_to_clipboard: bool,
}

fn default_auto_paste() -> bool {
    true
}

fn default_copy_to_clipboard() -> bool {
    true
}

impl Default for ClipboardSettings {
    fn default() -> Self {
        Self {
            auto_paste: default_auto_paste(),
            copy_to_clipboard: default_copy_to_clipboard(),
        }
    }
}

/// App launch and startup behavior preferences.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct StartupSettings {
    /// Start Relay in the background when logging into the OS.
    #[serde(default, alias = "launchAtLogin")]
    pub launch_at_login: bool,
    /// Launch Relay minimized without showing the main control panel window.
    #[serde(default, alias = "startMinimized")]
    pub start_minimized: bool,
}

/// Microphone input hardware and audio warm-up preferences.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AudioInputSettings {
    /// Prefer system built-in microphone for lower latency.
    #[serde(default = "default_prefer_builtin_mic", alias = "preferBuiltinMic")]
    pub prefer_builtin_mic: bool,
    /// Explicitly selected microphone device name (None = OS default).
    #[serde(default, alias = "selectedDevice")]
    pub selected_device: Option<String>,
    /// Keep microphone stream warm ("off", "15s", "30s", "1m", "5m") to avoid warm-up clipping.
    #[serde(default = "default_keep_microphone_warm", alias = "keepMicrophoneWarm")]
    pub keep_microphone_warm: String,
    /// Auto-learn corrections made in the target app into user dictionary.
    #[serde(default = "default_auto_learn_words", alias = "autoLearnWords")]
    pub auto_learn_words: bool,
}

fn default_prefer_builtin_mic() -> bool {
    true
}

fn default_keep_microphone_warm() -> String {
    "off".to_string()
}

fn default_auto_learn_words() -> bool {
    true
}

impl Default for AudioInputSettings {
    fn default() -> Self {
        Self {
            prefer_builtin_mic: default_prefer_builtin_mic(),
            selected_device: None,
            keep_microphone_warm: default_keep_microphone_warm(),
            auto_learn_words: default_auto_learn_words(),
        }
    }
}

/// Spoken trigger phrase -> text expansion snippet.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SnippetItem {
    pub id: String,
    pub trigger: String,
    pub snippet_text: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default = "default_snippet_enabled")]
    pub enabled: bool,
}

fn default_snippet_enabled() -> bool {
    true
}

pub fn default_snippets() -> Vec<SnippetItem> {
    vec![
        SnippetItem {
            id: "snip_linkedin".to_string(),
            trigger: "my linkedin".to_string(),
            snippet_text: "https://linkedin.com/in/you".to_string(),
            label: Some("My LinkedIn".to_string()),
            enabled: true,
        },
        SnippetItem {
            id: "snip_rewrite".to_string(),
            trigger: "rewrite prompt".to_string(),
            snippet_text: "Rewrite this to be more concise, clear, and professional:".to_string(),
            label: Some("Rewrite prompt".to_string()),
            enabled: true,
        },
        SnippetItem {
            id: "snip_intro".to_string(),
            trigger: "intro email".to_string(),
            snippet_text: "Hey, would love to find some time to chat later this week. Let me know what works best for you!".to_string(),
            label: Some("Intro email".to_string()),
            enabled: true,
        },
        SnippetItem {
            id: "snip_signoff".to_string(),
            trigger: "sign off".to_string(),
            snippet_text: "Best regards,\nAlex".to_string(),
            label: Some("Sign off".to_string()),
            enabled: true,
        },
    ]
}


/// Whether Relay tries to tell speakers apart in a meeting.
///
/// `Automatic` runs the cheap, non-biometric attribution: the microphone is the
/// local user, system audio is everyone else. It creates no voiceprints and
/// stores no biometric data. Diarization and a persistent voice library are
/// deliberately not options here yet — see
/// `Meeting-rules/meeting_speaker_identification.md` §6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeakerIdentification {
    #[default]
    Automatic,
    Off,
}

/// How a summary is shaped by default. Mirrors
/// `meetings_v2::processing::model::SummaryMode`, kept separate so the settings
/// file format does not move whenever the pipeline's internals do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefaultSummaryMode {
    Concise,
    #[default]
    Standard,
    Detailed,
}

/// A user-defined summary extension: a named presentation treatment layered on
/// top of a summary mode.
///
/// `instructions` shapes how the summary reads. It cannot change what was
/// extracted — the canonical meeting facts are produced before any extension is
/// applied — so a badly written extension can produce an awkward summary but
/// never a false one.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MeetingExtensionSetting {
    pub id: String,
    pub name: String,
    pub instructions: String,
}

/// Meeting behavior the user controls.
///
/// Deliberately small. The processing pipeline has seven internal stages; none
/// of them is a setting. What is exposed is what someone would actually want to
/// change: whether the debug view is visible, whether the readable transcript is
/// built, how long summaries are, and which presentations exist.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MeetingSettings {
    /// Whether the Raw Transcript tab is offered.
    ///
    /// This controls **visibility only**. The raw transcript is the diagnostic
    /// source artifact for everything derived from a meeting, and turning this
    /// off never deletes it — `transcript.jsonl` stays on disk and the pipeline
    /// keeps reading it.
    #[serde(default = "default_true", alias = "showRawTranscript")]
    pub show_raw_transcript: bool,
    /// Whether the speaker-labelled conversation transcript is built.
    #[serde(default = "default_true", alias = "generateConversationTranscript")]
    pub generate_conversation_transcript: bool,
    /// Whether a summary is generated automatically once a recording finishes.
    ///
    /// Either way this happens after the recording is safely persisted and never
    /// blocks the recorder, the live transcript, or opening the meeting.
    #[serde(default = "default_true", alias = "autoGenerateSummary")]
    pub auto_generate_summary: bool,
    #[serde(default, alias = "defaultSummaryMode")]
    pub default_summary_mode: DefaultSummaryMode,
    #[serde(default = "default_extension_id", alias = "defaultExtensionId")]
    pub default_extension_id: String,
    #[serde(default, alias = "speakerIdentification")]
    pub speaker_identification: SpeakerIdentification,
    /// The user's own extensions. The shipped ones live in code and are always
    /// available; this list only adds to them.
    #[serde(default)]
    pub extensions: Vec<MeetingExtensionSetting>,
}

fn default_true() -> bool {
    true
}

fn default_extension_id() -> String {
    "default".to_string()
}

impl Default for MeetingSettings {
    fn default() -> Self {
        Self {
            // Both transcript switches default on: the pipeline is new, and
            // being able to compare raw against derived output is how its
            // quality gets judged.
            show_raw_transcript: true,
            generate_conversation_transcript: true,
            auto_generate_summary: true,
            default_summary_mode: DefaultSummaryMode::default(),
            default_extension_id: default_extension_id(),
            speaker_identification: SpeakerIdentification::default(),
            extensions: Vec::new(),
        }
    }
}

pub fn default_dictionary_words() -> Vec<String> {
    vec![
        "Relay".to_string(),
        "Whisper".to_string(),
        "Tauri".to_string(),
        "Rust".to_string(),
        "Supabase".to_string(),
        "LanceDB".to_string(),
        "Ollama".to_string(),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    #[serde(default)]
    pub clipboard: ClipboardSettings,
    #[serde(default)]
    pub startup: StartupSettings,
    #[serde(default)]
    pub audio_input: AudioInputSettings,
    #[serde(default)]
    pub meetings: MeetingSettings,
    #[serde(default = "default_dictionary_words")]
    pub dictionary: Vec<String>,
    #[serde(default = "default_snippets")]
    pub snippets: Vec<SnippetItem>,

}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            provider: ProviderConfig::default(),
            stt: SttSettings::default(),
            tts: TtsSettings::default(),
            hotkeys: HotkeySettings::default(),
            ui: UiSettings::default(),
            vault: VaultSettings::default(),
            language: LanguageSettings::default(),
            diagnostics: DiagnosticsSettings::default(),
            cloud: CloudSettings::default(),
            sound: SoundSettings::default(),
            clipboard: ClipboardSettings::default(),
            startup: StartupSettings::default(),
            audio_input: AudioInputSettings::default(),
            meetings: MeetingSettings::default(),
            dictionary: default_dictionary_words(),
            snippets: default_snippets(),
        }
    }
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

    /// Applies active snippet expansions to the given transcript.
    /// If an enabled snippet's trigger phrase is found (case-insensitive),
    /// it replaces the phrase with the snippet expansion text.
    pub fn expand_snippets(&self, transcript: &str) -> String {
        let mut result = transcript.to_string();
        for snippet in &self.snippets {
            if !snippet.enabled || snippet.trigger.trim().is_empty() {
                continue;
            }
            let trigger = snippet.trigger.trim();
            let lower_result = result.to_lowercase();
            let lower_trigger = trigger.to_lowercase();
            if let Some(pos) = lower_result.find(&lower_trigger) {
                let prefix = &result[..pos];
                let suffix = &result[pos + lower_trigger.len()..];
                result = format!("{}{}{}", prefix, snippet.snippet_text, suffix);
            }
        }
        result
    }

    /// Builds the combined STT initial prompt incorporating custom dictionary words.
    pub fn build_stt_prompt(&self) -> Option<String> {
        let mut terms: Vec<String> = self.dictionary.iter().filter(|w| !w.trim().is_empty()).cloned().collect();
        if let Some(custom) = &self.stt.custom_initial_prompt {
            if !custom.trim().is_empty() {
                terms.push(custom.trim().to_string());
            }
        }
        if terms.is_empty() {
            None
        } else {
            Some(terms.join(", "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snippet_expansion() {
        let settings = AppSettings::default();
        let transcript = "Here is my linkedin if you want to connect";
        let expanded = settings.expand_snippets(transcript);
        assert_eq!(expanded, "Here is https://linkedin.com/in/you if you want to connect");
    }

    #[test]
    fn test_disabled_snippet_not_expanded() {
        let mut settings = AppSettings::default();
        settings.snippets[0].enabled = false;
        let transcript = "Here is my linkedin";
        let expanded = settings.expand_snippets(transcript);
        assert_eq!(expanded, "Here is my linkedin");
    }

    #[test]
    fn test_clipboard_and_startup_defaults() {
        let defaults = AppSettings::default();
        assert!(defaults.clipboard.auto_paste);
        assert!(defaults.clipboard.copy_to_clipboard);
        assert!(!defaults.startup.launch_at_login);
        assert!(!defaults.startup.start_minimized);
        assert!(defaults.audio_input.prefer_builtin_mic);
        assert_eq!(defaults.audio_input.keep_microphone_warm, "off");
        assert!(defaults.audio_input.auto_learn_words);
        assert!(!defaults.dictionary.is_empty());
    }

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
        assert!(app_settings.clipboard.auto_paste);
        assert!(app_settings.clipboard.copy_to_clipboard);
        assert!(!app_settings.startup.launch_at_login);

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

    #[test]
    fn test_dictation_stt_settings_defaults_and_backward_compatibility() {
        // 1. Empty/legacy settings defaults cleanly
        let empty: AppSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(empty.stt.dictation_quality, DictationSttQuality::Fast);
        assert_eq!(empty.stt.dictation_threads, None);

        // 2. Legacy STT settings without dictation fields
        let legacy_json = r#"{
            "stt": {
                "whisper_model_path": "models/ggml-small.bin",
                "enable_initial_prompt": true
            }
        }"#;
        let legacy: AppSettings = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(legacy.stt.whisper_model_path.as_deref(), Some("models/ggml-small.bin"));
        assert_eq!(legacy.stt.dictation_quality, DictationSttQuality::Fast);
        assert_eq!(legacy.stt.dictation_threads, None);
        assert!(legacy.stt.enable_initial_prompt);

        // 3. Snake case explicit accurate quality & custom threads
        let accurate_json = r#"{
            "stt": {
                "dictation_quality": "accurate",
                "dictation_threads": 8
            }
        }"#;
        let acc: AppSettings = serde_json::from_str(accurate_json).unwrap();
        assert_eq!(acc.stt.dictation_quality, DictationSttQuality::Accurate);
        assert_eq!(acc.stt.dictation_threads, Some(8));

        // 4. CamelCase support from frontend
        let camel_json = r#"{
            "stt": {
                "dictationQuality": "accurate",
                "dictationThreads": 12
            }
        }"#;
        let camel: AppSettings = serde_json::from_str(camel_json).unwrap();
        assert_eq!(camel.stt.dictation_quality, DictationSttQuality::Accurate);
        assert_eq!(camel.stt.dictation_threads, Some(12));
    }

    #[test]
    fn test_pre_0_15_0_prompt_settings_backward_compatibility() {
        // Pre-0.15.0 settings containing prompt_settings, prompts, and custom options
        let pre_0_15_0_json = r#"{
            "prompt_settings": {
                "enabled": true,
                "promptHotkey": "Ctrl+Alt+Space"
            },
            "prompts": [
                {
                    "id": "prompt_custom",
                    "name": "Custom Action",
                    "prompt_body": "Do something with {{text}}",
                    "enabled": true
                }
            ],
            "hotkeys": {
                "show_hide_hotkey": "Ctrl+Shift+Space",
                "dictation_hotkey": "Ctrl+Space",
                "toggle_to_talk": true
            },
            "stt": {
                "dictation_quality": "fast",
                "dictation_threads": 8
            },
            "dictionary": ["Relay", "Tauri"]
        }"#;

        let loaded: AppSettings = serde_json::from_str(pre_0_15_0_json)
            .expect("Pre-0.15.0 settings payload must deserialize cleanly without errors");

        // Verify remaining settings survived unchanged
        assert_eq!(loaded.hotkeys.show_hide_hotkey, "Ctrl+Shift+Space");
        assert_eq!(loaded.hotkeys.dictation_hotkey, "Ctrl+Space");
        assert!(loaded.hotkeys.toggle_to_talk);
        assert_eq!(loaded.stt.dictation_quality, DictationSttQuality::Fast);
        assert_eq!(loaded.stt.dictation_threads, Some(8));
        assert_eq!(loaded.dictionary, vec!["Relay", "Tauri"]);
    }
}
