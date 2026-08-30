//! The Piper TTS provider.
//!
//! Local, offline and zero-cost: Relay spawns a Piper binary with a voice
//! model, writes text to its stdin and reads the WAV it produces. No
//! linking, so Piper's licence never touches Relay's — which matters now
//! that upstream has moved: `rhasspy/piper` was archived read-only in
//! October 2025 and active work continues at `OHF-Voice/piper1-gpl` under
//! GPL-3.0. Both binaries drive identically here, and GPL-3.0 is in any
//! case compatible with Relay's AGPL-3.0. See `docs/talkback/RESEARCH.md`.
//!
//! Piper synthesizes a whole utterance and exits — it has no streaming
//! mode this provider can use. Talkback works around that by feeding it
//! one *sentence* at a time (`talkback::chunk`), so time-to-first-audio is
//! the cost of the first sentence rather than the whole answer.
//!
//! ## Three things this file exists to get right on Windows
//!
//! 1. **No console window.** `CreateProcess` on a console subsystem binary
//!    pops a window unless `CREATE_NO_WINDOW` is set. One sentence per
//!    phrase means that would be a flashing window every few seconds.
//! 2. **The temp file always goes away.** A [`ScratchWav`] guard removes it
//!    on every path — success, error, early return, cancellation, panic.
//! 3. **A cancelled synthesis kills its child.** Barge-in has to stop the
//!    process, not wait politely for it and throw the audio away.

use super::discovery::{self, TtsProblem};
use super::{TtsAudio, TtsCapabilities, TtsError, TtsProvider, TtsVoice};
use crate::settings::TtsSettings;
use base64::Engine;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

/// How often a running synthesis is checked for cancellation.
///
/// Small enough that barge-in is bounded well under the ~250 ms budget in
/// `docs/talkback/BENCHMARKS.md`, large enough not to spin a core.
const CANCEL_POLL_MS: u64 = 15;

/// A synthesis that outruns this is not going to arrive in a conversation.
/// Bounds a Piper that has wedged on a malformed model rather than leaving
/// the turn hanging on it.
const SYNTHESIS_TIMEOUT_SECS: u64 = 30;

/// Removes a scratch WAV when it goes out of scope, however that happens.
///
/// Talkback synthesizes once per sentence, so a leak here is not a stray
/// file every now and then — it is one per sentence, for the life of the
/// application.
struct ScratchWav(PathBuf);

impl ScratchWav {
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScratchWav {
    fn drop(&mut self) {
        if self.0.exists() {
            let _ = std::fs::remove_file(&self.0);
        }
    }
}

pub struct PiperProvider {
    binary_path: Option<PathBuf>,
    voice_path: Option<PathBuf>,
    scratch_dir: PathBuf,
}

impl PiperProvider {
    /// Builds a provider from settings plus a resolved binary path.
    ///
    /// The binary is passed in rather than read from settings because it
    /// may have been *discovered* rather than configured
    /// (`tts::discovery`), and the provider should not care which.
    pub fn new(
        binary_path: Option<PathBuf>,
        voice_path: Option<PathBuf>,
        scratch_dir: PathBuf,
    ) -> Self {
        Self {
            binary_path,
            voice_path,
            scratch_dir,
        }
    }

    /// Settings-only construction, for callers with no config directory to
    /// hand. Discovery does not run, and scratch falls back to the system
    /// temp directory.
    pub fn from_settings(settings: &TtsSettings) -> Self {
        Self::new(
            non_empty(settings.piper_binary_path.as_deref()),
            non_empty(settings.piper_voice_path.as_deref()),
            std::env::temp_dir(),
        )
    }

    pub fn binary_path(&self) -> Option<&Path> {
        self.binary_path.as_deref()
    }

    pub fn voice_path(&self) -> Option<&Path> {
        self.voice_path.as_deref()
    }

    /// Everything wrong with this configuration, in the order a user would
    /// fix it.
    pub fn problems(&self) -> Vec<TtsProblem> {
        let mut problems = Vec::new();
        match &self.binary_path {
            None => problems.push(TtsProblem::BinaryMissing),
            Some(path) => problems.extend(discovery::validate_binary(path)),
        }
        match &self.voice_path {
            None => problems.push(TtsProblem::VoiceMissing),
            Some(path) => problems.extend(discovery::validate_voice(path)),
        }
        problems
    }

    /// Builds the synthesis command.
    ///
    /// Separated so the argument list and the Windows process flags are
    /// testable without a Piper binary on the machine.
    fn command(&self, binary: &Path, voice: &Path, out_path: &Path) -> Command {
        let mut command = Command::new(binary);
        command
            .arg("--model")
            .arg(voice)
            .arg("--output_file")
            .arg(out_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        // Piper is a console-subsystem program. Without this flag Windows
        // flashes a console window for every sentence Relay speaks.
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        command
    }

    /// Waits for `child`, killing it if `is_cancelled` becomes true or the
    /// timeout elapses.
    ///
    /// `wait_with_output` cannot do this: it blocks until the process
    /// exits, which is exactly the wrong behaviour when the user has
    /// already started talking over the sentence being synthesized.
    fn wait_or_kill(
        mut child: Child,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<std::process::Output, TtsError> {
        let deadline =
            std::time::Instant::now() + std::time::Duration::from_secs(SYNTHESIS_TIMEOUT_SECS);

        loop {
            match child.try_wait() {
                Ok(Some(_)) => {
                    return child
                        .wait_with_output()
                        .map_err(|e| TtsError::SynthesisFailed(e.to_string()));
                }
                Ok(None) => {}
                Err(e) => return Err(TtsError::SynthesisFailed(e.to_string())),
            }

            if is_cancelled() {
                let _ = child.kill();
                let _ = child.wait();
                return Err(TtsError::Cancelled);
            }

            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(TtsError::SynthesisFailed(format!(
                    "Piper did not finish within {SYNTHESIS_TIMEOUT_SECS}s"
                )));
            }

            std::thread::sleep(std::time::Duration::from_millis(CANCEL_POLL_MS));
        }
    }
}

fn non_empty(value: Option<&str>) -> Option<PathBuf> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

impl TtsProvider for PiperProvider {
    fn name(&self) -> &'static str {
        "piper"
    }

    fn capabilities(&self) -> TtsCapabilities {
        TtsCapabilities {
            intra_utterance_streaming: false,
            cancellable: true,
            voices: true,
        }
    }

    fn is_configured(&self) -> bool {
        self.problems().is_empty()
    }

    fn synthesize_cancellable(
        &self,
        text: &str,
        is_cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<TtsAudio>, TtsError> {
        let (Some(binary), Some(voice)) = (&self.binary_path, &self.voice_path) else {
            return Ok(None);
        };
        let text = text.trim();
        if text.is_empty() {
            return Ok(None);
        }
        if is_cancelled() {
            return Err(TtsError::Cancelled);
        }

        // Created lazily: a user who never enables TTS should not get an
        // empty scratch directory in their vault config.
        std::fs::create_dir_all(&self.scratch_dir)?;
        let out = ScratchWav(
            self.scratch_dir
                .join(format!("relay_tts_{}.wav", uuid::Uuid::new_v4())),
        );

        let mut child = self
            .command(binary, voice, out.path())
            .spawn()
            .map_err(|e| TtsError::SpawnFailed(format!("{} ({})", e, binary.display())))?;

        // Dropping stdin is what tells Piper the input is complete; it
        // waits on EOF, so a held handle deadlocks the synthesis.
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(text.as_bytes())
                .map_err(|e| TtsError::SpawnFailed(e.to_string()))?;
        }

        let output = Self::wait_or_kill(child, is_cancelled)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(TtsError::SynthesisFailed(
                stderr.trim().chars().take(400).collect(),
            ));
        }

        // A zero-exit run that produced nothing is a broken configuration,
        // not a transient hiccup — classifying it as an I/O error would
        // make the engine retry it once per sentence forever.
        let audio_bytes = std::fs::read(out.path()).map_err(|e| {
            TtsError::Unusable(format!(
                "Piper reported success but wrote no audio ({e}). The voice model is probably \
                 not loadable."
            ))
        })?;
        if audio_bytes.is_empty() {
            return Err(TtsError::Unusable(
                "Piper produced an empty audio file. The voice model is probably not loadable."
                    .to_string(),
            ));
        }

        Ok(Some(TtsAudio {
            wav_base64: base64::engine::general_purpose::STANDARD.encode(audio_bytes),
            char_count: text.chars().count(),
        }))
        // `out` drops here, removing the file on every path above.
    }

    fn voices(&self) -> Vec<TtsVoice> {
        let Some(parent) = self.voice_path.as_deref().and_then(Path::parent) else {
            return Vec::new();
        };
        discovery::voices_in(parent)
            .into_iter()
            .map(|voice| TtsVoice {
                id: voice.path,
                label: voice.label,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("relay_piper_{}", uuid::Uuid::new_v4()));
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

    /// A provider wired to a real, runnable stub so the process lifecycle
    /// can be exercised without Piper installed.
    #[cfg(unix)]
    fn stub_provider(temp: &TempDir, script: &str) -> PiperProvider {
        let binary = temp.path().join("piper-stub");
        std::fs::write(&binary, format!("#!/bin/sh\n{script}\n")).unwrap();
        make_executable(&binary);

        let voice = temp.touch("voices/en_US-test-medium.onnx");
        temp.touch("voices/en_US-test-medium.onnx.json");

        PiperProvider::new(
            Some(binary),
            Some(voice),
            temp.path().join("scratch"),
        )
    }

    fn never_cancelled() -> impl Fn() -> bool {
        || false
    }

    #[test]
    fn a_fully_configured_provider_reports_no_problems() {
        let temp = TempDir::new();
        let binary = temp.touch("piper");
        make_executable(&binary);
        let voice = temp.touch("voices/en_US-amy-medium.onnx");
        temp.touch("voices/en_US-amy-medium.onnx.json");

        let provider = PiperProvider::new(Some(binary), Some(voice), temp.path().join("scratch"));
        assert!(provider.problems().is_empty(), "{:?}", provider.problems());
        assert!(provider.is_configured());
    }

    #[test]
    fn a_missing_sidecar_makes_the_provider_unconfigured() {
        let temp = TempDir::new();
        let binary = temp.touch("piper");
        make_executable(&binary);
        let voice = temp.touch("voices/en_US-amy-medium.onnx");

        let provider = PiperProvider::new(Some(binary), Some(voice), temp.path().join("scratch"));
        assert!(!provider.is_configured());
        assert!(matches!(
            provider.problems().as_slice(),
            [TtsProblem::VoiceConfigMissing(_)]
        ));
    }

    #[test]
    fn nothing_configured_reports_both_problems() {
        let provider = PiperProvider::new(None, None, std::env::temp_dir());
        assert_eq!(
            provider.problems(),
            vec![TtsProblem::BinaryMissing, TtsProblem::VoiceMissing]
        );
        assert!(!provider.is_configured());
    }

    #[test]
    fn whitespace_settings_are_treated_as_unset() {
        let provider = PiperProvider::from_settings(&TtsSettings {
            piper_binary_path: Some("   ".into()),
            piper_voice_path: Some("\t\n".into()),
        });
        assert!(!provider.is_configured());
        assert!(provider.binary_path().is_none());
        assert!(provider.voice_path().is_none());
    }

    #[test]
    fn settings_paths_are_trimmed_rather_than_rejected() {
        let provider = PiperProvider::from_settings(&TtsSettings {
            piper_binary_path: Some("  /opt/piper/piper  ".into()),
            piper_voice_path: Some(" /voices/en.onnx ".into()),
        });
        assert_eq!(provider.binary_path(), Some(Path::new("/opt/piper/piper")));
        assert_eq!(provider.voice_path(), Some(Path::new("/voices/en.onnx")));
    }

    #[test]
    fn an_unconfigured_provider_returns_none_rather_than_erroring() {
        let provider = PiperProvider::new(None, None, std::env::temp_dir());
        assert!(provider
            .synthesize_cancellable("hello", &never_cancelled())
            .unwrap()
            .is_none());
    }

    #[test]
    fn blank_text_never_reaches_the_process() {
        let temp = TempDir::new();
        // A binary that does not exist: reaching spawn would be an Err.
        let provider = PiperProvider::new(
            Some(temp.path().join("nonexistent-piper")),
            Some(temp.path().join("voice.onnx")),
            temp.path().join("scratch"),
        );
        assert!(provider
            .synthesize_cancellable("   \n ", &never_cancelled())
            .unwrap()
            .is_none());
    }

    #[test]
    fn an_already_cancelled_turn_never_spawns() {
        let temp = TempDir::new();
        let provider = PiperProvider::new(
            Some(temp.path().join("nonexistent-piper")),
            Some(temp.path().join("voice.onnx")),
            temp.path().join("scratch"),
        );
        // Would be SpawnFailed if it reached the process.
        assert!(matches!(
            provider.synthesize_cancellable("hello", &|| true),
            Err(TtsError::Cancelled)
        ));
    }

    #[test]
    fn a_missing_binary_names_the_path_it_looked_for() {
        let temp = TempDir::new();
        let missing = temp.path().join("nonexistent-piper");
        let provider = PiperProvider::new(
            Some(missing.clone()),
            Some(temp.path().join("voice.onnx")),
            temp.path().join("scratch"),
        );
        match provider.synthesize_cancellable("hello", &never_cancelled()) {
            Err(TtsError::SpawnFailed(message)) => {
                assert!(
                    message.contains(&missing.display().to_string()),
                    "an unactionable error: {message}"
                );
            }
            other => panic!("expected a spawn failure, got {other:?}"),
        }
    }

    #[test]
    fn the_command_carries_the_model_and_output_arguments() {
        let temp = TempDir::new();
        let provider = PiperProvider::new(None, None, temp.path().to_path_buf());
        let command = provider.command(
            Path::new("/opt/piper/piper"),
            Path::new("/voices/en US-amy.onnx"),
            Path::new("/scratch/out.wav"),
        );
        let args: Vec<String> = command
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert_eq!(
            args,
            vec!["--model", "/voices/en US-amy.onnx", "--output_file", "/scratch/out.wav"],
            "a path with spaces must be one argument, not two"
        );
        assert_eq!(
            command.get_program().to_string_lossy(),
            "/opt/piper/piper"
        );
    }

    #[test]
    fn voices_are_enumerated_from_the_configured_model_directory() {
        let temp = TempDir::new();
        let voice = temp.touch("voices/en_US-amy-medium.onnx");
        temp.touch("voices/en_US-amy-medium.onnx.json");
        temp.touch("voices/hi_IN-pratham-medium.onnx");
        temp.touch("voices/hi_IN-pratham-medium.onnx.json");

        let provider = PiperProvider::new(None, Some(voice), temp.path().join("scratch"));
        let labels: Vec<String> = provider.voices().into_iter().map(|v| v.label).collect();
        assert_eq!(labels, vec!["en_US-amy-medium", "hi_IN-pratham-medium"]);
        assert!(provider.capabilities().voices);
    }

    #[test]
    fn voices_are_empty_when_no_model_is_configured() {
        let provider = PiperProvider::new(None, None, std::env::temp_dir());
        assert!(provider.voices().is_empty());
    }

    // The remaining tests drive a real child process through a shell stub.
    // Unix-only because they need a script interpreter; the logic they
    // cover (wait/kill/cleanup) is platform-independent.

    #[cfg(unix)]
    #[test]
    fn a_successful_synthesis_returns_audio_and_leaves_no_scratch_file() {
        let temp = TempDir::new();
        // Consume stdin, then write a non-empty file at --output_file.
        let provider = stub_provider(
            &temp,
            "cat > /dev/null; out=\"\"; while [ $# -gt 0 ]; do \
             if [ \"$1\" = \"--output_file\" ]; then out=\"$2\"; fi; shift; done; \
             printf 'RIFFfake' > \"$out\"",
        );

        let audio = provider
            .synthesize_cancellable("hello there", &never_cancelled())
            .unwrap()
            .expect("audio");
        assert!(!audio.wav_base64.is_empty());
        assert_eq!(audio.char_count, 11);

        let leftovers: Vec<_> = std::fs::read_dir(temp.path().join("scratch"))
            .map(|d| d.flatten().collect())
            .unwrap_or_default();
        assert!(leftovers.is_empty(), "scratch WAV was not cleaned up");
    }

    #[cfg(unix)]
    #[test]
    fn a_failing_piper_surfaces_its_stderr_and_still_cleans_up() {
        let temp = TempDir::new();
        let provider = stub_provider(
            &temp,
            "cat > /dev/null; echo 'model config not found' >&2; exit 1",
        );

        match provider.synthesize_cancellable("hello", &never_cancelled()) {
            Err(TtsError::SynthesisFailed(message)) => {
                assert!(message.contains("model config not found"), "{message}");
            }
            other => panic!("expected a synthesis failure, got {other:?}"),
        }

        let leftovers: Vec<_> = std::fs::read_dir(temp.path().join("scratch"))
            .map(|d| d.flatten().collect())
            .unwrap_or_default();
        assert!(leftovers.is_empty(), "a failed synthesis leaked a file");
    }

    #[cfg(unix)]
    #[test]
    fn a_successful_exit_with_no_audio_is_a_permanent_failure() {
        // The signature of an unloadable voice model. Classified as
        // permanent so the engine stops after one attempt instead of
        // spawning a process per sentence for the rest of the session.
        let temp = TempDir::new();
        let provider = stub_provider(&temp, "cat > /dev/null; exit 0");
        match provider.synthesize_cancellable("hello", &never_cancelled()) {
            Err(error @ TtsError::Unusable(_)) => {
                assert!(error.is_permanent(), "would retry a broken model forever");
                assert!(error.to_string().contains("voice model"));
            }
            other => panic!("expected a permanent synthesis failure, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn an_empty_output_file_is_a_permanent_failure_too() {
        let temp = TempDir::new();
        let provider = stub_provider(
            &temp,
            "cat > /dev/null; out=\"\"; while [ $# -gt 0 ]; do \
             if [ \"$1\" = \"--output_file\" ]; then out=\"$2\"; fi; shift; done; \
             : > \"$out\"",
        );
        match provider.synthesize_cancellable("hello", &never_cancelled()) {
            Err(error @ TtsError::Unusable(_)) => assert!(error.is_permanent()),
            other => panic!("expected a permanent synthesis failure, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn cancelling_mid_synthesis_kills_the_child_and_cleans_up() {
        let temp = TempDir::new();
        // A stub that would run far longer than the test.
        let provider = stub_provider(&temp, "cat > /dev/null; sleep 30");

        let started = std::time::Instant::now();
        let cancel_after = std::time::Instant::now() + std::time::Duration::from_millis(120);
        let result = provider
            .synthesize_cancellable("hello", &|| std::time::Instant::now() >= cancel_after);

        assert!(matches!(result, Err(TtsError::Cancelled)));
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "cancellation waited for the child instead of killing it: {:?}",
            started.elapsed()
        );

        let leftovers: Vec<_> = std::fs::read_dir(temp.path().join("scratch"))
            .map(|d| d.flatten().collect())
            .unwrap_or_default();
        assert!(leftovers.is_empty(), "a cancelled synthesis leaked a file");
    }

    #[cfg(unix)]
    #[test]
    fn repeated_synthesis_does_not_accumulate_scratch_files() {
        let temp = TempDir::new();
        let provider = stub_provider(
            &temp,
            "cat > /dev/null; out=\"\"; while [ $# -gt 0 ]; do \
             if [ \"$1\" = \"--output_file\" ]; then out=\"$2\"; fi; shift; done; \
             printf 'RIFFfake' > \"$out\"",
        );

        for _ in 0..8 {
            provider
                .synthesize_cancellable("a sentence", &never_cancelled())
                .unwrap();
        }

        let leftovers: Vec<_> = std::fs::read_dir(temp.path().join("scratch"))
            .map(|d| d.flatten().collect())
            .unwrap_or_default();
        assert!(
            leftovers.is_empty(),
            "one file per sentence would fill the disk over a conversation"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_voice_path_containing_spaces_synthesizes() {
        let temp = TempDir::new();
        let binary = temp.path().join("piper-stub");
        std::fs::write(
            &binary,
            "#!/bin/sh\ncat > /dev/null; out=\"\"; while [ $# -gt 0 ]; do \
             if [ \"$1\" = \"--output_file\" ]; then out=\"$2\"; fi; shift; done; \
             printf 'RIFFfake' > \"$out\"\n",
        )
        .unwrap();
        make_executable(&binary);

        let voice = temp.touch("My Voices/en US-amy medium.onnx");
        temp.touch("My Voices/en US-amy medium.onnx.json");

        let provider =
            PiperProvider::new(Some(binary), Some(voice), temp.path().join("scratch dir"));
        assert!(provider.is_configured());
        assert!(provider
            .synthesize_cancellable("hello", &never_cancelled())
            .unwrap()
            .is_some());
    }
}
