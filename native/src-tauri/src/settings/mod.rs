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
    /// Brings Relay's Captures surface forward from anywhere in the OS.
    ///
    /// Deliberately *not* the trigger for reading a web page. Browsers grant
    /// page access only in response to a gesture made inside the browser
    /// (`activeTab`), so an OS-level hotkey cannot read the tab a user is
    /// looking at without asking for permanent access to every site they
    /// visit — which Relay does not do. The in-browser shortcut owns that
    /// job; this one opens the surface that explains it and shows what has
    /// been captured. See `docs/capture.md`.
    #[serde(default = "default_capture_hotkey")]
    pub capture_hotkey: String,
}

fn default_show_hide_hotkey() -> String {
    "Ctrl+Shift+Space".to_string()
}

fn default_dictation_hotkey() -> String {
    "Ctrl+Space".to_string()
}

/// `Ctrl+Space+C` is not a registrable accelerator — the OS shortcut layer
/// takes modifiers plus one key, and `Space` is not a modifier — and
/// `Ctrl+Space` itself is already push-to-talk dictation. `Ctrl+Shift+C` is
/// the nearest free combination that reads as "capture".
fn default_capture_hotkey() -> String {
    "Ctrl+Shift+C".to_string()
}

impl Default for HotkeySettings {
    fn default() -> Self {
        Self {
            show_hide_hotkey: default_show_hide_hotkey(),
            dictation_hotkey: default_dictation_hotkey(),
            toggle_to_talk: false,
            capture_hotkey: default_capture_hotkey(),
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
    /// Decode quality preset: "fast", "balanced" or "quality".
    ///
    /// Trades decode time for how much borderline speech survives. Empty means
    /// the user has not chosen, and each surface uses the default that suits
    /// it — dictation cannot afford what a meeting can. A value set here is an
    /// override and applies everywhere.
    #[serde(default, alias = "sttPreset")]
    pub preset: String,
    /// Whether dictated text is offered to the Tier 2 cleanup layer.
    ///
    /// Off by default. The layer costs a model call before the text is usable
    /// and may change words, so it is something the user turns on rather than
    /// something they discover has been happening.
    #[serde(default, alias = "textTransform")]
    pub text_transform: bool,
    /// How far that cleanup may go — see `capture::rewrite::CleanupStyle`.
    /// Empty means `faithful`, the only style that cannot change meaning.
    #[serde(default, alias = "cleanupStyle")]
    pub cleanup_style: String,
}

impl SttSettings {
    /// The user's explicit preset choice, or `None` when they have not made
    /// one.
    ///
    /// The distinction is the whole point. A single global preset would force
    /// one answer onto surfaces with opposite constraints: push-to-talk is
    /// latency-bound because someone is waiting for the text, and a meeting is
    /// recall-bound because it is decoded once and read later. `None` lets each
    /// surface pick, and a set value is an override that wins everywhere — so
    /// there is one setting rather than one per surface.
    pub fn configured_preset(&self) -> Option<crate::capture::stt::SttPreset> {
        let raw = self.preset.trim();
        if raw.is_empty() {
            return None;
        }
        Some(crate::capture::stt::SttPreset::from_setting(raw))
    }
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
#[derive(Default)]
pub struct UiSettings {
    /// Which edge of the screen the floating pill anchors to.
    #[serde(default)]
    pub pill_position: PillPosition,
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
    ///
    /// Read only by the frontend, and correctly so: the tones are synthesized
    /// in the Web Audio API (`src/lib/soundEffects.ts`) by the pill that is
    /// already listening for the state change that should sound them
    /// (`DictationPill.tsx`, on `capture-state-changed`). A Rust reader would
    /// be a second audio path opened to duplicate a decision the frontend has
    /// already made. Audited and left alone deliberately — not another setting
    /// nothing reads.
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

/// Method used to inject text into the active target application.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum InjectionMethod {
    /// Instant atomic clipboard paste via Ctrl+V.
    /// Fast, reliable across modern XAML, web, electron, and rich-text applications without character drops.
    #[default]
    ClipboardPaste,
    /// Simulates keyboard typing events per character.
    /// Used when the target application disables clipboard access.
    Keystrokes,
}

fn default_injection_method() -> InjectionMethod {
    InjectionMethod::ClipboardPaste
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
    /// Injection method: instant clipboard paste (default) or simulated keystrokes.
    #[serde(default = "default_injection_method", alias = "injectionMethod")]
    pub injection_method: InjectionMethod,
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
            injection_method: default_injection_method(),
        }
    }
}

/// Web capture: the local bridge the Relay browser extension talks to.
///
/// Off by default. Capture needs a browser extension installed and paired
/// before it can do anything, so there is no case where opening a listening
/// socket before the user has asked for capture buys them something — and
/// every case where not opening one is the better default for a local-first
/// app.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CaptureSettings {
    /// Whether Relay listens on loopback for captures from the extension.
    #[serde(default, alias = "bridgeEnabled")]
    pub bridge_enabled: bool,
    /// Preferred loopback port. If it is taken, the bridge binds an
    /// ephemeral port instead and reports the one it got.
    #[serde(default = "default_capture_bridge_port", alias = "bridgePort")]
    pub bridge_port: u16,
    /// The shared secret the extension must present. Generated the first
    /// time capture is enabled; replacing it unpairs every browser.
    #[serde(default, alias = "pairingToken")]
    pub pairing_token: Option<String>,
    /// Whether to run Relay's analysis pass automatically once a capture has
    /// been stored. Storage never depends on it: turning this off costs you
    /// summaries and topics, never the captured content.
    #[serde(default = "default_true", alias = "analyzeOnCapture")]
    pub analyze_on_capture: bool,
}

fn default_true() -> bool {
    true
}

fn default_capture_bridge_port() -> u16 {
    crate::capture::web::bridge::DEFAULT_PORT
}

impl Default for CaptureSettings {
    fn default() -> Self {
        Self {
            bridge_enabled: false,
            bridge_port: default_capture_bridge_port(),
            pairing_token: None,
            analyze_on_capture: true,
        }
    }
}

/// App launch and startup behavior preferences.
///
/// Both fields describe what happens *before* a webview exists, so neither
/// could ever have been honoured by the frontend that rendered their switches.
/// [`crate::startup`] is the reader they went without: it applies them from the
/// Tauri setup hook, and re-applies `launch_at_login` from `save_settings` so
/// the switch takes effect when it is flipped.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct StartupSettings {
    /// Start Relay in the background when logging into the OS.
    ///
    /// Written to the OS by `tauri-plugin-autostart` — the registry Run key on
    /// Windows, a LaunchAgent on macOS, a `.desktop` entry on Linux.
    #[serde(default, alias = "launchAtLogin")]
    pub launch_at_login: bool,
    /// Launch Relay minimized without showing the main control panel window.
    ///
    /// Withheld rather than minimized: the tray's "Show Relay" item and the
    /// show/hide hotkey both toggle on `Window::is_visible`, so hidden is the
    /// state they can bring back.
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
}

fn default_prefer_builtin_mic() -> bool {
    true
}

fn default_keep_microphone_warm() -> String {
    "off".to_string()
}

impl Default for AudioInputSettings {
    fn default() -> Self {
        Self {
            prefer_builtin_mic: default_prefer_builtin_mic(),
            selected_device: None,
            keep_microphone_warm: default_keep_microphone_warm(),
        }
    }
}

impl AudioInputSettings {
    pub fn parse_keep_warm_duration(&self) -> Option<std::time::Duration> {
        parse_keep_warm_duration_str(&self.keep_microphone_warm)
    }
}

pub fn parse_keep_warm_duration_str(setting: &str) -> Option<std::time::Duration> {
    match setting {
        "15s" => Some(std::time::Duration::from_secs(15)),
        "30s" => Some(std::time::Duration::from_secs(30)),
        "1m" => Some(std::time::Duration::from_secs(60)),
        "5m" => Some(std::time::Duration::from_secs(300)),
        _ => None,
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

/// A phrase Whisper keeps getting wrong, and what it should say instead.
///
/// Distinct from `AppSettings::dictionary`, which is a list of canonical words
/// used to prime the recognizer before it guesses. Priming helps the model
/// reach for "Supabase"; it does nothing once the model has already produced
/// "super base". A correction is the other half: a deterministic repair of a
/// form the recognizer keeps emitting.
///
/// Learned only from an explicit correction the user makes inside Relay, never
/// by watching what they type elsewhere.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VocabularyCorrection {
    /// What the recognizer produced, e.g. "super base".
    pub source: String,
    /// What it should have produced, e.g. "Supabase".
    pub replacement: String,
    /// Off keeps the entry visible and stops it being applied, so a correction
    /// that turns out to be wrong can be silenced without losing the record of
    /// having made it.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// When it was learned. The note this came from keeps its own record — see
    /// `vault::CorrectionRecord` — but a rule outlives the note that taught it,
    /// so it carries its own date.
    #[serde(default)]
    pub created_at: String,
}

impl VocabularyCorrection {
    pub fn new(source: &str, replacement: &str) -> Self {
        Self {
            source: source.trim().to_string(),
            replacement: replacement.trim().to_string(),
            enabled: true,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Whether this entry could ever do anything.
    ///
    /// An empty side, or a source identical to its replacement, is a no-op that
    /// would sit in the list looking like a rule.
    ///
    /// Case alone *is* a difference. "ollama" → "Ollama" is one of the repairs
    /// this exists for: matching ignores case and the replacement is written
    /// exactly as the user typed it, so recasing a word the recognizer keeps
    /// lowercasing is the whole mechanism working as intended.
    pub fn is_meaningful(&self) -> bool {
        !self.source.trim().is_empty()
            && !self.replacement.trim().is_empty()
            && self.source.trim() != self.replacement.trim()
    }

    /// Two corrections are the same rule when they read the same source,
    /// regardless of case — the recognizer's capitalisation of a phrase it got
    /// wrong is not a distinction worth two entries.
    pub fn same_source_as(&self, other: &str) -> bool {
        self.source.trim().eq_ignore_ascii_case(other.trim())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default)]
    pub provider: ProviderConfig,
    #[serde(default)]
    pub stt: SttSettings,
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
    pub capture: CaptureSettings,
    #[serde(default)]
    pub startup: StartupSettings,
    #[serde(default)]
    pub audio_input: AudioInputSettings,
    #[serde(default = "default_dictionary_words")]
    pub dictionary: Vec<String>,
    #[serde(default = "default_snippets")]
    pub snippets: Vec<SnippetItem>,
    /// Learned "what Whisper said" → "what it meant" repairs. Empty by
    /// default: every entry here was put there by the user correcting
    /// something inside Relay.
    #[serde(default, alias = "vocabularyCorrections")]
    pub vocabulary_corrections: Vec<VocabularyCorrection>,

}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            provider: ProviderConfig::default(),
            stt: SttSettings::default(),
            hotkeys: HotkeySettings::default(),
            ui: UiSettings::default(),
            vault: VaultSettings::default(),
            language: LanguageSettings::default(),
            diagnostics: DiagnosticsSettings::default(),
            cloud: CloudSettings::default(),
            sound: SoundSettings::default(),
            clipboard: ClipboardSettings::default(),
            capture: CaptureSettings::default(),
            startup: StartupSettings::default(),
            audio_input: AudioInputSettings::default(),
            dictionary: default_dictionary_words(),
            snippets: default_snippets(),
            vocabulary_corrections: Vec::new(),
        }
    }
}

impl AppSettings {
    /// Carries the stored capture configuration over a whole-settings save.
    ///
    /// `save_settings` writes whatever object the frontend sends. Capture is
    /// not edited through that path — the bridge, the port, the pairing token
    /// and the analyse toggle each have their own command — so a settings
    /// object serialized from a frontend that never loaded the capture
    /// section would otherwise silently switch capture off and destroy the
    /// pairing token, unpairing every browser.
    pub fn preserving_capture(mut self, stored: &CaptureSettings) -> Self {
        self.capture = stored.clone();
        self
    }

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

    /// Teaches Relay that `source` should read as `replacement`.
    ///
    /// Returns whether the vocabulary changed. Called only when the user ticks
    /// "Teach Relay this correction" — most corrections are ordinary edits, and
    /// "Thursday" to "Tuesday" is not something to repeat on every future
    /// transcript.
    ///
    /// `dictionary` is deliberately not touched. The two lists do different
    /// jobs: a dictionary word primes the recognizer before it guesses, and a
    /// correction repairs a guess it already made. Putting "super base" in the
    /// dictionary would prime Relay to produce the very phrase being corrected.
    ///
    /// Teaching the same source twice updates it in place rather than
    /// appending a second rule — two rules for one phrase would make the result
    /// depend on list order, which is not something a user can see or reason
    /// about. Re-teaching also re-enables an entry that had been switched off,
    /// because teaching it again is a clear statement that it is wanted.
    pub fn learn_correction(&mut self, source: &str, replacement: &str) -> bool {
        let candidate = VocabularyCorrection::new(source, replacement);
        if !candidate.is_meaningful() {
            return false;
        }
        match self
            .vocabulary_corrections
            .iter_mut()
            .find(|existing| existing.same_source_as(source))
        {
            Some(existing) => {
                existing.replacement = candidate.replacement;
                existing.enabled = true;
            }
            None => self.vocabulary_corrections.push(candidate),
        }
        true
    }

    /// Adds one canonical word to the user's dictionary.
    ///
    /// Returns whether the list changed. This is the *correct* spelling of
    /// something Relay should recognize — the same thing Settings › Dictionary
    /// adds, reached from a Voice Note instead — so it primes the recognizer
    /// and is matched case-insensitively against what is already there, since
    /// two spellings of one word prime nothing extra.
    pub fn add_dictionary_word(&mut self, word: &str) -> bool {
        let word = word.trim();
        if word.is_empty() {
            return false;
        }
        if self
            .dictionary
            .iter()
            .any(|existing| existing.trim().eq_ignore_ascii_case(word))
        {
            return false;
        }
        self.dictionary.push(word.to_string());
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn teaching_a_correction_records_the_mapping_not_just_the_word() {
        // The distinction the whole feature turns on: Whisper keeps producing
        // "super base", so the canonical spelling alone cannot repair it.
        let mut settings = AppSettings::default();
        assert!(settings.learn_correction("super base", "Supabase"));

        assert_eq!(settings.vocabulary_corrections.len(), 1);
        let learned = &settings.vocabulary_corrections[0];
        assert_eq!(learned.source, "super base");
        assert_eq!(learned.replacement, "Supabase");
        assert!(learned.enabled);
        assert!(!learned.created_at.is_empty());
    }

    #[test]
    fn teaching_a_correction_leaves_the_dictionary_alone() {
        // "super base" in the dictionary would prime the recognizer to produce
        // the exact phrase being corrected. The lists are not interchangeable.
        let mut settings = AppSettings::default();
        let before = settings.dictionary.clone();
        settings.learn_correction("super base", "Supabase");
        assert_eq!(settings.dictionary, before);
    }

    #[test]
    fn an_ordinary_correction_is_not_vocabulary_unless_it_is_taught() {
        // Applying a correction does not call this; only the checkbox does.
        // "Thursday" → "Tuesday" is the brief's own counter-example.
        let settings = AppSettings::default();
        assert!(
            settings.vocabulary_corrections.is_empty(),
            "nothing is learned by default"
        );
    }

    #[test]
    fn teaching_the_same_source_twice_updates_rather_than_duplicates() {
        let mut settings = AppSettings::default();
        settings.learn_correction("lance db", "Lance DB");
        settings.learn_correction("super base", "Supabase");
        assert_eq!(settings.vocabulary_corrections.len(), 2, "different sources are different rules");

        // Re-teaching replaces the replacement in place. Two rules for one
        // phrase would make the applied result depend on list order, which is
        // not something a user can see or reason about.
        settings.learn_correction("lance db", "LanceDB");
        assert_eq!(settings.vocabulary_corrections.len(), 2);
        assert_eq!(settings.vocabulary_corrections[0].source, "lance db");
        assert_eq!(settings.vocabulary_corrections[0].replacement, "LanceDB");

        // Case-insensitive on the source: the recognizer's capitalisation of a
        // phrase it got wrong is not a distinction worth two entries.
        settings.learn_correction("LANCE DB", "Lance DB");
        assert_eq!(settings.vocabulary_corrections.len(), 2);
        assert_eq!(settings.vocabulary_corrections[0].replacement, "Lance DB");
    }

    #[test]
    fn re_teaching_a_disabled_correction_switches_it_back_on() {
        let mut settings = AppSettings::default();
        settings.learn_correction("super base", "Supabase");
        settings.vocabulary_corrections[0].enabled = false;

        settings.learn_correction("super base", "Supabase");
        assert_eq!(settings.vocabulary_corrections.len(), 1);
        assert!(settings.vocabulary_corrections[0].enabled);
    }

    #[test]
    fn a_correction_that_could_never_do_anything_is_refused() {
        let mut settings = AppSettings::default();
        assert!(!settings.learn_correction("   ", "Supabase"));
        assert!(!settings.learn_correction("super base", ""));
        assert!(!settings.learn_correction("Supabase", "  Supabase  "), "identical is not a repair");
        assert!(settings.vocabulary_corrections.is_empty());
    }

    #[test]
    fn recasing_a_word_is_a_repair_worth_learning() {
        // The brief names "ollama" → "Ollama" as useful vocabulary, and the
        // correction UI offers to learn it. Refusing here would mean the
        // checkbox silently did nothing.
        let mut settings = AppSettings::default();
        assert!(settings.learn_correction("ollama", "Ollama"));
        assert_eq!(settings.vocabulary_corrections[0].replacement, "Ollama");
    }

    #[test]
    fn adding_a_dictionary_word_appends_it_once() {
        let mut settings = AppSettings {
            dictionary: vec!["Relay".to_string()],
            ..Default::default()
        };

        assert!(settings.add_dictionary_word("  Supabase  "));
        assert_eq!(settings.dictionary, vec!["Relay", "Supabase"]);

        // Already known, in any casing: two spellings prime nothing extra.
        assert!(!settings.add_dictionary_word("supabase"));
        assert!(!settings.add_dictionary_word("   "));
        assert_eq!(settings.dictionary, vec!["Relay", "Supabase"]);
    }

    #[test]
    fn adding_a_dictionary_word_leaves_learned_corrections_alone() {
        let mut settings = AppSettings::default();
        settings.learn_correction("super base", "Supabase");
        settings.add_dictionary_word("LanceDB");
        assert_eq!(settings.vocabulary_corrections.len(), 1);
    }

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
        assert_eq!(defaults.clipboard.injection_method, InjectionMethod::ClipboardPaste);
        assert!(!defaults.startup.launch_at_login);
        assert!(!defaults.startup.start_minimized);
        assert!(defaults.audio_input.prefer_builtin_mic);
        assert_eq!(defaults.audio_input.keep_microphone_warm, "off");
        assert!(!defaults.dictionary.is_empty());
    }

    #[test]
    fn test_injection_method_deserialization() {
        let json = r#"{"injection_method": "keystrokes"}"#;
        let s: ClipboardSettings = serde_json::from_str(json).unwrap();
        assert_eq!(s.injection_method, InjectionMethod::Keystrokes);

        let camel_json = r#"{"injectionMethod": "clipboard_paste"}"#;
        let s2: ClipboardSettings = serde_json::from_str(camel_json).unwrap();
        assert_eq!(s2.injection_method, InjectionMethod::ClipboardPaste);

        let empty_json = r#"{}"#;
        let s3: ClipboardSettings = serde_json::from_str(empty_json).unwrap();
        assert_eq!(s3.injection_method, InjectionMethod::ClipboardPaste);
    }

    #[test]
    fn capture_is_off_until_the_user_turns_it_on() {
        let defaults = AppSettings::default();
        assert!(
            !defaults.capture.bridge_enabled,
            "a fresh install must not open a listening socket"
        );
        assert!(defaults.capture.pairing_token.is_none());
        assert_eq!(defaults.capture.bridge_port, 8765);
        assert!(defaults.capture.analyze_on_capture);
    }

    #[test]
    fn capture_hotkey_defaults_to_a_registrable_combination() {
        // `Ctrl+Space+C` cannot be registered — a shortcut is modifiers plus
        // one key, and Space is not a modifier — and `Ctrl+Space` is already
        // push-to-talk.
        assert_eq!(HotkeySettings::default().capture_hotkey, "Ctrl+Shift+C");
        assert_eq!(HotkeySettings::default().dictation_hotkey, "Ctrl+Space");
    }

    #[test]
    fn settings_written_before_capture_existed_still_load() {
        let json = r#"{
            "hotkeys": { "dictation_hotkey": "Ctrl+Space" },
            "clipboard": { "auto_paste": true, "copy_to_clipboard": true }
        }"#;
        let loaded: AppSettings = serde_json::from_str(json).unwrap();
        assert!(!loaded.capture.bridge_enabled);
        assert_eq!(loaded.capture.bridge_port, 8765);
        assert!(loaded.capture.analyze_on_capture);
        assert_eq!(loaded.hotkeys.capture_hotkey, "Ctrl+Shift+C");
    }

    #[test]
    fn a_whole_settings_save_cannot_unpair_a_browser() {
        let stored = CaptureSettings {
            bridge_enabled: true,
            bridge_port: 9100,
            pairing_token: Some("deadbeef".to_string()),
            ..Default::default()
        };

        // A frontend that never loaded the capture section sends defaults.
        let incoming = AppSettings::default();
        assert!(!incoming.capture.bridge_enabled);

        let merged = incoming.preserving_capture(&stored);
        assert!(merged.capture.bridge_enabled);
        assert_eq!(merged.capture.bridge_port, 9100);
        assert_eq!(merged.capture.pairing_token.as_deref(), Some("deadbeef"));
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

        let app_settings = AppSettings {
            language: LanguageSettings {
                primary_dictation_language: "hi".to_string(),
                spoken_languages: vec!["hi".to_string(), "en".to_string()],
                notes_language: "en".to_string(),
                output_script: "latin".to_string(),
            },
            ..Default::default()
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
