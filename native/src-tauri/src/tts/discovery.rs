//! Finding and validating a local Piper installation.
//!
//! This exists because "configure `piper_binary_path` in settings.json"
//! is not a product. A user who installs Relay and switches Talkback on
//! must be able to reach spoken answers through the UI, and the app must
//! be able to find an installation it put there itself.
//!
//! ## The distribution decision
//!
//! Piper is **not** bundled with Relay. The binary plus one voice is
//! ~40–100 MB depending on quality, the release artifacts are per-platform
//! and per-architecture, and Relay cannot verify a download it never
//! performed. What ships instead is a **managed location** Relay owns and
//! can discover without being told:
//!
//! ```text
//! <app-data>/Relay/tts/piper/piper.exe   the executable
//! <app-data>/Relay/tts/voices/*.onnx     voice models (+ .onnx.json sidecars)
//! ```
//!
//! [`discover`] looks there first, then beside Relay's own executable,
//! then in the Tauri resource directory (so a future bundled build needs
//! no code change), then on `PATH`. Settings always win over discovery, so
//! a user with Piper somewhere else is never overridden.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The executable Relay looks for. Piper ships `piper.exe` on Windows and
/// `piper` elsewhere.
pub fn piper_executable_name() -> &'static str {
    if cfg!(windows) {
        "piper.exe"
    } else {
        "piper"
    }
}

/// The per-user directory Relay keeps its voice installation in.
///
/// **Not** derived from `config_dir`, and that is deliberate. Relay's
/// config directory is process-relative (`current_dir()/.relay/config`),
/// which is fine while the vault lives beside a checkout but is wrong for
/// a packaged Windows app: launched from a Start Menu shortcut,
/// `current_dir()` is typically `C:\Windows\System32`, and launched from
/// its install directory it is under `Program Files` — neither writable
/// by a standard user, and both liable to change between launches.
///
/// Telling a user "put `piper.exe` in this folder" only works if the
/// folder is stable and writable, so the voice installation is anchored
/// to the OS per-user application-data directory instead. This is new
/// state with no existing installs, so there is nothing to migrate.
///
/// `config_dir` remains the fallback for a machine with no usable
/// app-data location.
pub fn default_tts_root(config_dir: &Path) -> PathBuf {
    app_data_dir()
        .map(|dir| dir.join("Relay").join("tts"))
        .unwrap_or_else(|| config_dir.join("tts"))
}

/// The OS per-user application-data directory.
///
/// Resolved from environment variables rather than by adding a
/// directories crate for three lookups.
fn app_data_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        // Roaming APPDATA, so a voice follows the user across machines in
        // a managed environment. LOCALAPPDATA is the fallback.
        std::env::var_os("APPDATA")
            .or_else(|| std::env::var_os("LOCALAPPDATA"))
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(|home| PathBuf::from(home).join(".config"))
                    .filter(|p| p.is_absolute())
            })
    }
}

/// Where Relay keeps a Piper installation it manages itself.
pub fn managed_piper_dir(tts_root: &Path) -> PathBuf {
    tts_root.join("piper")
}

/// Where Relay looks for voice models the user has added.
pub fn managed_voices_dir(tts_root: &Path) -> PathBuf {
    tts_root.join("voices")
}

/// Scratch space for synthesized audio.
///
/// Deliberately Relay-owned rather than the system temp directory: on
/// Windows a locked-down `%TEMP%` is a real failure mode, and one phrase
/// per sentence means a leak here would be noticeable within a single
/// conversation.
pub fn tts_scratch_dir(tts_root: &Path) -> PathBuf {
    tts_root.join("scratch")
}

/// Deletes leftover synthesis output.
///
/// Called at startup: a crash mid-synthesis leaves a WAV behind, and
/// nothing else will ever clean it up.
pub fn clear_scratch(tts_root: &Path) {
    let dir = tts_scratch_dir(tts_root);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    let mut removed = 0_usize;
    for entry in entries.flatten() {
        if entry.path().extension().is_some_and(|e| e == "wav") && std::fs::remove_file(entry.path()).is_ok() {
            removed += 1;
        }
    }
    if removed > 0 {
        tracing::info!("tts: cleared {} orphaned synthesis file(s)", removed);
    }
}

/// A voice model Relay can offer in the picker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PiperVoice {
    /// Absolute path to the `.onnx` model.
    pub path: String,
    /// Display name, derived from the filename: `en_US-amy-medium`.
    pub label: String,
    /// Language tag parsed from the filename (`en_US`), when it has one.
    #[serde(default)]
    pub language: Option<String>,
    /// Whether the model's required `.onnx.json` sidecar is present.
    pub has_config: bool,
}

/// Piper voice files are named `<lang>-<name>-<quality>.onnx` by
/// convention (`en_US-amy-medium.onnx`, `hi_IN-pratham-medium.onnx`).
/// Anything that does not match still gets a label — its filename — so an
/// unconventionally-named model is usable rather than hidden.
pub fn voice_from_path(path: &Path) -> Option<PiperVoice> {
    if path.extension()? != "onnx" {
        return None;
    }
    let stem = path.file_stem()?.to_string_lossy().to_string();
    let language = stem
        .split('-')
        .next()
        .filter(|tag| tag.contains('_') || tag.len() == 2)
        .map(str::to_string);

    Some(PiperVoice {
        path: path.to_string_lossy().to_string(),
        label: stem,
        language,
        has_config: voice_config_path(path).exists(),
    })
}

/// Piper requires a `<model>.onnx.json` beside the model. Without it the
/// binary exits with a parse error that says nothing useful, so Relay
/// checks for it up front and says the useful thing instead.
pub fn voice_config_path(model: &Path) -> PathBuf {
    let mut sidecar = model.as_os_str().to_os_string();
    sidecar.push(".json");
    PathBuf::from(sidecar)
}

/// Lists every voice model in `dir`, sorted by label.
pub fn voices_in(dir: &Path) -> Vec<PiperVoice> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut voices: Vec<PiperVoice> = entries
        .flatten()
        .filter_map(|entry| voice_from_path(&entry.path()))
        .collect();
    voices.sort_by(|a, b| a.label.cmp(&b.label));
    voices
}

/// Every voice Relay can offer: the managed directory, plus whatever sits
/// beside an already-configured model so a user who pointed Relay at their
/// own voices folder sees the rest of it.
pub fn available_voices(tts_root: &Path, configured: Option<&str>) -> Vec<PiperVoice> {
    let mut voices = voices_in(&managed_voices_dir(tts_root));

    if let Some(parent) = configured
        .map(Path::new)
        .and_then(|p| p.parent())
        .filter(|p| *p != managed_voices_dir(tts_root))
    {
        for voice in voices_in(parent) {
            if !voices.iter().any(|v| v.path == voice.path) {
                voices.push(voice);
            }
        }
    }

    voices.sort_by(|a, b| a.label.cmp(&b.label));
    voices
}

/// Where a discovered binary came from, so the UI can explain itself
/// rather than showing a bare path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PiperOrigin {
    /// The user set it explicitly in settings.
    Configured,
    /// Found in Relay's managed directory.
    Managed,
    /// Shipped alongside Relay's own executable, or in Tauri's resource
    /// directory. Nothing ships one today; the branch exists so bundling
    /// later needs packaging changes, not code changes.
    Bundled,
    /// Found on `PATH`.
    SystemPath,
}

/// A located Piper executable.
#[derive(Debug, Clone, PartialEq)]
pub struct DiscoveredPiper {
    pub path: PathBuf,
    pub origin: PiperOrigin,
}

/// Finds a Piper executable without being told where it is.
///
/// Order is deliberate: the managed directory first, because that is
/// where Relay's own setup flow puts things and a user who followed it
/// should not be second-guessed by a stale `PATH` entry. `PATH` is last
/// because it is the least predictable and the most likely to be a
/// different build than the one the voice models came with.
pub fn discover(tts_root: &Path, resource_dir: Option<&Path>) -> Option<DiscoveredPiper> {
    let name = piper_executable_name();

    let managed = managed_piper_dir(tts_root).join(name);
    if is_executable_file(&managed) {
        return Some(DiscoveredPiper {
            path: managed,
            origin: PiperOrigin::Managed,
        });
    }

    for base in bundled_search_roots(resource_dir) {
        for candidate in [base.join(name), base.join("piper").join(name)] {
            if is_executable_file(&candidate) {
                return Some(DiscoveredPiper {
                    path: candidate,
                    origin: PiperOrigin::Bundled,
                });
            }
        }
    }

    which_on_path(name).map(|path| DiscoveredPiper {
        path,
        origin: PiperOrigin::SystemPath,
    })
}

/// Directories a bundled Piper could plausibly live in.
///
/// Both the packaged and the `cargo tauri dev` layouts are covered: in a
/// packaged build the executable and its resources sit together, and in
/// development they are under `target/debug`.
fn bundled_search_roots(resource_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(dir) = resource_dir {
        roots.push(dir.to_path_buf());
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            roots.push(parent.to_path_buf());
        }
    }
    roots
}

/// Whether `path` is a file Relay could actually execute.
///
/// On Unix this checks the executable bit; on Windows any existing file
/// with the right name qualifies, since Windows has no such bit.
pub fn is_executable_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// A minimal `which`, so Relay does not gain a dependency for one lookup.
///
/// Honours `PATHEXT` on Windows, where `piper` and `piper.exe` are both
/// legitimate things to find on `PATH`.
fn which_on_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let direct = dir.join(name);
        if is_executable_file(&direct) {
            return Some(direct);
        }
        #[cfg(windows)]
        {
            let extensions = std::env::var("PATHEXT")
                .unwrap_or_else(|_| ".EXE;.CMD;.BAT".to_string());
            for extension in extensions.split(';').filter(|e| !e.is_empty()) {
                let candidate = dir.join(format!("{name}{}", extension.to_lowercase()));
                if is_executable_file(&candidate) {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

/// Why a configuration is not usable. One variant per thing the user can
/// actually fix, because "TTS failed" is not an actionable error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "detail")]
pub enum TtsProblem {
    /// No executable configured and none discovered.
    BinaryMissing,
    /// A path is set but nothing is there.
    BinaryNotFound(String),
    /// The path exists but is a directory, or is not executable.
    BinaryNotExecutable(String),
    /// No voice model configured and none available.
    VoiceMissing,
    VoiceNotFound(String),
    /// The model is there but its required `.onnx.json` sidecar is not.
    VoiceConfigMissing(String),
    /// The file does not look like a Piper voice at all.
    VoiceNotAModel(String),
}

impl TtsProblem {
    /// One sentence the UI can show verbatim, naming the fix.
    pub fn message(&self) -> String {
        match self {
            Self::BinaryMissing => {
                "No Piper executable found. Add one to Relay's voice folder or browse for it."
                    .to_string()
            }
            Self::BinaryNotFound(path) => {
                format!("The Piper executable isn't at {path} any more.")
            }
            Self::BinaryNotExecutable(path) => {
                format!("{path} isn't a runnable program.")
            }
            Self::VoiceMissing => {
                "No voice model selected. Add a Piper .onnx voice to Relay's voice folder."
                    .to_string()
            }
            Self::VoiceNotFound(path) => format!("The voice model isn't at {path} any more."),
            Self::VoiceConfigMissing(path) => format!(
                "{path} is missing its settings file. Piper voices need both the .onnx model and \
                 the matching .onnx.json file — download them together."
            ),
            Self::VoiceNotAModel(path) => {
                format!("{path} isn't a Piper voice model (.onnx expected).")
            }
        }
    }
}

/// Checks a binary path without running it.
pub fn validate_binary(path: &Path) -> Option<TtsProblem> {
    let display = path.to_string_lossy().to_string();
    if !path.exists() {
        return Some(TtsProblem::BinaryNotFound(display));
    }
    if !is_executable_file(path) {
        return Some(TtsProblem::BinaryNotExecutable(display));
    }
    None
}

/// Checks a voice model and its sidecar without running Piper.
pub fn validate_voice(path: &Path) -> Option<TtsProblem> {
    let display = path.to_string_lossy().to_string();
    if path.extension().is_none_or(|e| e != "onnx") {
        return Some(TtsProblem::VoiceNotAModel(display));
    }
    if !path.is_file() {
        return Some(TtsProblem::VoiceNotFound(display));
    }
    if !voice_config_path(path).is_file() {
        return Some(TtsProblem::VoiceConfigMissing(display));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch directory that removes itself. The crate has no
    /// `tempfile` dev-dependency, and adding one for this would be a
    /// dependency for a test helper.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("relay_tts_disc_{}", uuid::Uuid::new_v4()));
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

    #[test]
    fn the_executable_name_is_platform_correct() {
        if cfg!(windows) {
            assert_eq!(piper_executable_name(), "piper.exe");
        } else {
            assert_eq!(piper_executable_name(), "piper");
        }
    }

    #[test]
    fn managed_paths_are_relay_owned_and_distinct() {
        let root = Path::new("/relay/tts");
        assert_eq!(managed_piper_dir(root), Path::new("/relay/tts/piper"));
        assert_eq!(managed_voices_dir(root), Path::new("/relay/tts/voices"));
        assert_eq!(tts_scratch_dir(root), Path::new("/relay/tts/scratch"));
    }

    #[test]
    fn the_voice_root_is_absolute_and_stable() {
        // The setup flow prints this path and tells the user to put files
        // in it. A relative or launch-dependent path would make that
        // instruction wrong in a packaged build.
        let root = default_tts_root(Path::new("/relay/config"));
        assert!(root.is_absolute(), "{}", root.display());
    }

    #[test]
    fn the_voice_root_does_not_follow_the_process_working_directory() {
        // `config_dir` is `current_dir()/.relay/config`, which in a
        // packaged Windows app can be System32. The voice installation
        // must not inherit that.
        let from_one = default_tts_root(Path::new("/a/.relay/config"));
        let from_another = default_tts_root(Path::new("/b/.relay/config"));
        assert_eq!(
            from_one, from_another,
            "the install location moved with the working directory"
        );
    }

    #[test]
    fn the_voice_root_falls_back_to_the_config_dir_with_no_app_data() {
        // Only assert the fallback shape when the environment genuinely
        // has no app-data location; otherwise the real one is correct.
        if app_data_dir().is_none() {
            assert_eq!(
                default_tts_root(Path::new("/relay/config")),
                Path::new("/relay/config/tts")
            );
        }
    }

    #[test]
    fn discovery_finds_the_managed_binary() {
        let temp = TempDir::new();
        let binary = temp.touch(&format!("piper/{}", piper_executable_name()));
        make_executable(&binary);

        let found = discover(temp.path(), None).expect("managed binary discovered");
        assert_eq!(found.origin, PiperOrigin::Managed);
        assert_eq!(found.path, binary);
    }

    #[test]
    fn discovery_falls_back_to_the_resource_directory() {
        let temp = TempDir::new();
        let resources = TempDir::new();
        let binary = resources.touch(piper_executable_name());
        make_executable(&binary);

        let found = discover(temp.path(), Some(resources.path())).expect("bundled binary");
        assert_eq!(found.origin, PiperOrigin::Bundled);
    }

    #[test]
    fn discovery_prefers_managed_over_bundled() {
        let temp = TempDir::new();
        let resources = TempDir::new();
        let managed = temp.touch(&format!("piper/{}", piper_executable_name()));
        make_executable(&managed);
        let bundled = resources.touch(piper_executable_name());
        make_executable(&bundled);

        let found = discover(temp.path(), Some(resources.path())).unwrap();
        assert_eq!(
            found.path, managed,
            "a user's own installation must win over a bundled one"
        );
    }

    #[test]
    fn discovery_returns_nothing_when_there_is_nothing() {
        let temp = TempDir::new();
        // PATH is not controlled here, so only assert the managed and
        // bundled roots are empty — a machine with piper on PATH would
        // legitimately find it.
        let found = discover(temp.path(), None);
        assert!(found.is_none() || found.unwrap().origin == PiperOrigin::SystemPath);
    }

    #[test]
    fn a_directory_is_not_an_executable() {
        let temp = TempDir::new();
        let dir = temp.path().join("piper");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(!is_executable_file(&dir));
    }

    #[test]
    fn voice_metadata_is_parsed_from_the_filename() {
        let temp = TempDir::new();
        let model = temp.touch("voices/en_US-amy-medium.onnx");
        temp.touch("voices/en_US-amy-medium.onnx.json");

        let voice = voice_from_path(&model).expect("a voice");
        assert_eq!(voice.label, "en_US-amy-medium");
        assert_eq!(voice.language.as_deref(), Some("en_US"));
        assert!(voice.has_config);
    }

    #[test]
    fn a_hindi_voice_is_recognised() {
        let temp = TempDir::new();
        let model = temp.touch("voices/hi_IN-pratham-medium.onnx");
        let voice = voice_from_path(&model).unwrap();
        assert_eq!(voice.language.as_deref(), Some("hi_IN"));
    }

    #[test]
    fn a_voice_without_its_sidecar_is_listed_but_flagged() {
        let temp = TempDir::new();
        let model = temp.touch("voices/en_US-amy-medium.onnx");
        let voice = voice_from_path(&model).unwrap();
        assert!(!voice.has_config, "the missing sidecar must be visible");
    }

    #[test]
    fn non_model_files_are_not_voices() {
        let temp = TempDir::new();
        let readme = temp.touch("voices/README.md");
        assert!(voice_from_path(&readme).is_none());
    }

    #[test]
    fn voices_are_listed_sorted_and_deduplicated() {
        let temp = TempDir::new();
        for name in ["en_US-ryan-high", "en_US-amy-medium", "hi_IN-pratham-medium"] {
            temp.touch(&format!("voices/{name}.onnx"));
            temp.touch(&format!("voices/{name}.onnx.json"));
        }
        let voices = available_voices(temp.path(), None);
        assert_eq!(
            voices.iter().map(|v| v.label.as_str()).collect::<Vec<_>>(),
            vec!["en_US-amy-medium", "en_US-ryan-high", "hi_IN-pratham-medium"]
        );
    }

    #[test]
    fn voices_beside_a_configured_model_are_offered_too() {
        let temp = TempDir::new();
        let elsewhere = TempDir::new();
        elsewhere.touch("en_GB-alan-low.onnx");
        elsewhere.touch("en_GB-alan-low.onnx.json");
        let configured = elsewhere.path().join("en_GB-alan-low.onnx");

        let voices = available_voices(temp.path(), Some(&configured.to_string_lossy()));
        assert!(voices.iter().any(|v| v.label == "en_GB-alan-low"));
    }

    #[test]
    fn listing_a_missing_directory_is_empty_not_an_error() {
        assert!(voices_in(Path::new("/definitely/not/here")).is_empty());
        assert!(available_voices(Path::new("/definitely/not/here"), None).is_empty());
    }

    #[test]
    fn binary_validation_names_the_actual_problem() {
        let temp = TempDir::new();
        assert!(matches!(
            validate_binary(&temp.path().join("nope")),
            Some(TtsProblem::BinaryNotFound(_))
        ));

        let dir = temp.path().join("adir");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(matches!(
            validate_binary(&dir),
            Some(TtsProblem::BinaryNotExecutable(_))
        ));

        let good = temp.touch("piper-good");
        make_executable(&good);
        assert_eq!(validate_binary(&good), None);
    }

    #[test]
    fn voice_validation_catches_the_missing_sidecar() {
        let temp = TempDir::new();
        let model = temp.touch("voices/en_US-amy-medium.onnx");
        assert!(
            matches!(validate_voice(&model), Some(TtsProblem::VoiceConfigMissing(_))),
            "the sidecar is the most common Piper setup mistake and must be named"
        );

        temp.touch("voices/en_US-amy-medium.onnx.json");
        assert_eq!(validate_voice(&model), None);
    }

    #[test]
    fn voice_validation_rejects_the_wrong_file_type() {
        let temp = TempDir::new();
        let wrong = temp.touch("voices/voice.wav");
        assert!(matches!(
            validate_voice(&wrong),
            Some(TtsProblem::VoiceNotAModel(_))
        ));
    }

    #[test]
    fn every_problem_has_an_actionable_message() {
        let problems = [
            TtsProblem::BinaryMissing,
            TtsProblem::BinaryNotFound("p".into()),
            TtsProblem::BinaryNotExecutable("p".into()),
            TtsProblem::VoiceMissing,
            TtsProblem::VoiceNotFound("v".into()),
            TtsProblem::VoiceConfigMissing("v".into()),
            TtsProblem::VoiceNotAModel("v".into()),
        ];
        for problem in problems {
            let message = problem.message();
            assert!(message.len() > 20, "too terse to act on: {message}");
            assert!(message.ends_with('.'), "not a sentence: {message}");
        }
    }

    #[test]
    fn scratch_clearing_removes_only_wavs_and_tolerates_a_missing_dir() {
        let temp = TempDir::new();
        temp.touch("scratch/a.wav");
        temp.touch("scratch/keep.txt");
        clear_scratch(temp.path());
        assert!(!temp.path().join("scratch/a.wav").exists());
        assert!(temp.path().join("scratch/keep.txt").exists());

        clear_scratch(Path::new("/definitely/not/here"));
    }

    #[test]
    fn paths_with_spaces_and_non_ascii_survive_round_tripping() {
        let temp = TempDir::new();
        let model = temp.touch("My Voices/हिन्दी voice-medium.onnx");
        temp.touch("My Voices/हिन्दी voice-medium.onnx.json");
        assert_eq!(validate_voice(&model), None);

        let voice = voice_from_path(&model).unwrap();
        assert!(voice.has_config);
        assert!(voice.label.contains("हिन्दी"));
        assert!(Path::new(&voice.path).is_file(), "path did not round-trip");
    }
}
