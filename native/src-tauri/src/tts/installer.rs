//! Downloading and installing a local voice.
//!
//! Deliberately a **separate lifecycle layer**, not part of
//! [`PiperProvider`](super::PiperProvider). The provider's job is to turn
//! text into audio with whatever is installed; it should not also know how
//! to fetch things from the internet. Talkback → `TtsProvider` →
//! `PiperProvider` is unchanged by any of this — the installer only puts
//! files where [`discovery`](super::discovery) already looks for them.
//!
//! ## What "installed" has to mean
//!
//! Files existing is not installed. Every stage below must pass before
//! Relay reports a working voice:
//!
//! ```text
//! download → verify SHA-256 → extract → atomic move → pair check → SPEAK
//! ```
//!
//! The last one matters most. Relay runs a real synthesis through the
//! production provider before saying "Ready", so a voice that downloads
//! perfectly and cannot actually load is caught during setup rather than
//! in the middle of the user's first conversation.
//!
//! ## Never break a working installation
//!
//! Downloads land in `<root>/.staging` and are moved into place only
//! after verification. A failed or cancelled run deletes its staging
//! directory and leaves whatever was already installed untouched — the
//! user who retries a flaky download does not lose the voice they had.

use super::discovery;
use super::manifest::{ArchiveKind, Artifact, RuntimeEntry, VoiceEntry, VoiceManifest};
use super::TtsProvider;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum InstallError {
    #[error("Automatic voice setup isn't available in this build of Relay.")]
    NotProvisioned(String),

    #[error("{0}")]
    Unsupported(String),

    #[error("The download couldn't be completed. Check your connection and try again.")]
    Network(String),

    #[error(
        "A downloaded file didn't match what Relay expected and was discarded. \
         This usually means the download was interrupted — try again."
    )]
    ChecksumMismatch { label: String },

    #[error("The voice files couldn't be unpacked.")]
    Extract(String),

    #[error("Relay couldn't write to its voice folder.")]
    Io(String),

    #[error("The voice installed but couldn't speak. Try a different voice.")]
    SelfTestFailed(String),

    #[error("Setup was cancelled.")]
    Cancelled,
}

impl InstallError {
    /// A stable code for the frontend, so copy lives in one language and
    /// the UI never parses an error message.
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotProvisioned(_) => "NOT_PROVISIONED",
            Self::Unsupported(_) => "UNSUPPORTED",
            Self::Network(_) => "NETWORK",
            Self::ChecksumMismatch { .. } => "CHECKSUM",
            Self::Extract(_) => "EXTRACT",
            Self::Io(_) => "IO",
            Self::SelfTestFailed(_) => "SELF_TEST",
            Self::Cancelled => "CANCELLED",
        }
    }

    /// Whether retrying could plausibly work. A cancelled or corrupted
    /// download is worth another go; an unsupported machine is not.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Network(_) | Self::ChecksumMismatch { .. } | Self::Cancelled | Self::Io(_)
        )
    }

    /// The detail behind the user-facing message, for logs only.
    pub fn detail(&self) -> String {
        match self {
            Self::NotProvisioned(d)
            | Self::Unsupported(d)
            | Self::Network(d)
            | Self::Extract(d)
            | Self::Io(d)
            | Self::SelfTestFailed(d) => d.clone(),
            Self::ChecksumMismatch { label } => format!("checksum mismatch on {label}"),
            Self::Cancelled => "cancelled by the user".to_string(),
        }
    }
}

/// Which part of setup is running. Drives the UI's states directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallStage {
    Preparing,
    DownloadingEngine,
    DownloadingVoice,
    Installing,
    Validating,
    Testing,
    Done,
}

impl InstallStage {
    /// One line of user-facing copy per stage. Kept beside the enum so a
    /// new stage cannot ship without one.
    pub fn label(self) -> &'static str {
        match self {
            Self::Preparing => "Preparing…",
            Self::DownloadingEngine => "Downloading voice engine",
            Self::DownloadingVoice => "Downloading voice",
            Self::Installing => "Installing local voice…",
            Self::Validating => "Checking voice…",
            Self::Testing => "Testing voice…",
            Self::Done => "Local voice ready",
        }
    }
}

/// A progress report, emitted as setup runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallProgress {
    pub stage: InstallStage,
    /// What is happening, already phrased for display.
    pub label: String,
    /// The specific item, when one is being downloaded.
    #[serde(default)]
    pub item: Option<String>,
    /// Bytes received for the current item.
    pub received_bytes: u64,
    /// Expected bytes for the current item, when known.
    #[serde(default)]
    pub total_bytes: Option<u64>,
    /// Whole-setup progress, 0.0–1.0.
    pub overall: f32,
}

/// What a completed install produced.
#[derive(Debug, Clone, PartialEq)]
pub struct InstallOutcome {
    pub binary_path: PathBuf,
    pub voice_path: PathBuf,
    pub voice_id: String,
    pub runtime_version: String,
    /// True when a usable engine was already present and not re-downloaded.
    pub reused_runtime: bool,
}

/// Everything the installer needs from its caller.
///
/// A struct rather than eight arguments, and it carries the callbacks so
/// the installer itself has no idea Tauri exists — which is what lets the
/// whole flow be tested against a local HTTP server.
pub struct InstallRequest<'a> {
    pub manifest: &'a VoiceManifest,
    pub voice_id: &'a str,
    pub tts_root: &'a Path,
    /// Host platform/arch. Injected rather than read from `cfg!` so the
    /// unsupported-platform paths are testable.
    pub platform: &'a str,
    pub arch: &'a str,
    /// Called with every progress update.
    pub on_progress: &'a (dyn Fn(InstallProgress) + Send + Sync),
    /// Polled throughout; returning true abandons the install.
    pub is_cancelled: &'a (dyn Fn() -> bool + Send + Sync),
}

/// How often a download reports progress. Frequent enough to look live,
/// rare enough not to flood the event bus.
const PROGRESS_INTERVAL_MS: u128 = 120;

/// Read chunk size. Also the cancellation granularity for a download.
const READ_CHUNK: usize = 64 * 1024;

/// Refuses obviously wrong downloads before hashing gigabytes of them.
const MAX_ARTIFACT_BYTES: u64 = 512 * 1024 * 1024;

/// Removes a directory tree when dropped.
///
/// Staging must not survive a failure: a half-extracted engine left
/// behind would be picked up by the next run as though it were finished.
struct StagingDir(PathBuf);

impl StagingDir {
    fn create(root: &Path) -> Result<Self, InstallError> {
        let dir = root.join(".staging").join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&dir).map_err(|e| InstallError::Io(e.to_string()))?;
        Ok(Self(dir))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for StagingDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
        // Tidy the parent if this was the last staging directory. Best
        // effort: it fails harmlessly when another install is running.
        if let Some(parent) = self.0.parent() {
            let _ = std::fs::remove_dir(parent);
        }
    }
}

/// Clears staging left behind by a crash.
///
/// Called at startup. A killed process cannot run `Drop`, and without
/// this a hard crash mid-download would leak the partial file forever.
pub fn clear_staging(tts_root: &Path) {
    let staging = tts_root.join(".staging");
    if staging.exists() && std::fs::remove_dir_all(&staging).is_ok() {
        tracing::info!("tts: cleared interrupted voice setup");
    }
}

/// Runs the whole setup, blocking.
///
/// Call from a blocking task: this downloads, hashes and spawns a process.
pub fn install(request: InstallRequest<'_>) -> Result<InstallOutcome, InstallError> {
    // A manifest with no pinned checksums cannot be acted on safely, and
    // saying so is better than downloading something unverifiable.
    request.manifest.validate().map_err(|e| {
        InstallError::NotProvisioned(format!("the voice catalogue is not usable: {e}"))
    })?;

    let voice = request
        .manifest
        .voice(request.voice_id)
        .map_err(|e| InstallError::Unsupported(e.to_string()))?;
    let runtime = request
        .manifest
        .runtime_for(request.platform, request.arch)
        .map_err(|e| InstallError::Unsupported(e.to_string()))?;

    let report = |stage: InstallStage,
                  item: Option<String>,
                  received: u64,
                  total: Option<u64>,
                  overall: f32| {
        (request.on_progress)(InstallProgress {
            stage,
            label: stage.label().to_string(),
            item,
            received_bytes: received,
            total_bytes: total,
            overall: overall.clamp(0.0, 1.0),
        });
    };

    report(InstallStage::Preparing, None, 0, None, 0.0);
    check_cancelled(request.is_cancelled)?;

    std::fs::create_dir_all(request.tts_root).map_err(|e| InstallError::Io(e.to_string()))?;
    let staging = StagingDir::create(request.tts_root)?;

    // Skip the engine download when a working one is already installed —
    // a user adding a second voice should not re-fetch 20 MB of Piper.
    let installed_binary =
        discovery::managed_piper_dir(request.tts_root).join(discovery::piper_executable_name());
    let reused_runtime = discovery::is_executable_file(&installed_binary);

    // Weight the two phases by their real sizes so the bar does not jump.
    let engine_bytes = if reused_runtime { 0 } else { runtime.artifact.size_bytes };
    let total_bytes = (engine_bytes + voice.total_bytes()).max(1) as f32;
    let engine_share = engine_bytes as f32 / total_bytes;

    if reused_runtime {
        tracing::info!("tts: reusing the installed voice engine");
    } else {
        let archive = staging.path().join("engine-download");
        download_verified(
            &runtime.artifact,
            &archive,
            "voice engine",
            request.is_cancelled,
            &|received, total| {
                report(
                    InstallStage::DownloadingEngine,
                    Some("voice engine".to_string()),
                    received,
                    total,
                    engine_share * (received as f32 / engine_bytes.max(1) as f32),
                );
            },
        )?;

        report(InstallStage::Installing, None, 0, None, engine_share);
        install_runtime(runtime, &archive, staging.path(), request.tts_root)?;
    }

    // Voice: model and config, both verified, both moved atomically.
    let voices_dir = discovery::managed_voices_dir(request.tts_root);
    std::fs::create_dir_all(&voices_dir).map_err(|e| InstallError::Io(e.to_string()))?;

    let mut voice_received = 0_u64;
    let voice_total = voice.total_bytes().max(1);

    let staged_model = staging.path().join(voice.model_filename());
    download_verified(
        &voice.model,
        &staged_model,
        &format!("{} voice", voice.language_label),
        request.is_cancelled,
        &|received, total| {
            report(
                InstallStage::DownloadingVoice,
                Some(voice.display_name.clone()),
                received,
                total,
                engine_share
                    + (1.0 - engine_share) * ((voice_received + received) as f32 / voice_total as f32),
            );
        },
    )?;
    voice_received += voice.model.size_bytes;

    let staged_config = staging.path().join(voice.config_filename());
    download_verified(
        &voice.config,
        &staged_config,
        &format!("{} voice settings", voice.language_label),
        request.is_cancelled,
        &|received, total| {
            report(
                InstallStage::DownloadingVoice,
                Some(voice.display_name.clone()),
                received,
                total,
                engine_share
                    + (1.0 - engine_share) * ((voice_received + received) as f32 / voice_total as f32),
            );
        },
    )?;

    report(InstallStage::Validating, None, 0, None, 0.92);
    check_cancelled(request.is_cancelled)?;
    validate_voice_pair(&staged_model, &staged_config, voice)?;

    // Both files move together, model last: if the process dies between
    // the two, a config without a model is inert, whereas a model without
    // its config is a voice that looks installed and cannot load.
    let final_config = voices_dir.join(voice.config_filename());
    let final_model = voices_dir.join(voice.model_filename());
    move_into_place(&staged_config, &final_config)?;
    move_into_place(&staged_model, &final_model)?;

    let binary_path =
        discovery::managed_piper_dir(request.tts_root).join(discovery::piper_executable_name());

    // The one check that makes "Ready" mean something: speak, for real,
    // through the production provider.
    report(InstallStage::Testing, None, 0, None, 0.97);
    check_cancelled(request.is_cancelled)?;
    self_test(&binary_path, &final_model, request.tts_root)?;

    report(InstallStage::Done, None, 0, None, 1.0);

    Ok(InstallOutcome {
        binary_path,
        voice_path: final_model,
        voice_id: voice.id.clone(),
        runtime_version: runtime.version.clone(),
        reused_runtime,
    })
}

fn check_cancelled(is_cancelled: &dyn Fn() -> bool) -> Result<(), InstallError> {
    if is_cancelled() {
        Err(InstallError::Cancelled)
    } else {
        Ok(())
    }
}

/// Streams `artifact` to `destination`, hashing as it goes, and fails if
/// the digest does not match.
///
/// The file is written under a `.part` name and renamed only after the
/// hash matches, so nothing downstream can ever see a partial download.
fn download_verified(
    artifact: &Artifact,
    destination: &Path,
    label: &str,
    is_cancelled: &dyn Fn() -> bool,
    on_progress: &dyn Fn(u64, Option<u64>),
) -> Result<(), InstallError> {
    if !artifact.is_pinned() {
        return Err(InstallError::NotProvisioned(format!(
            "{label} has no pinned checksum"
        )));
    }
    check_cancelled(is_cancelled)?;

    let partial = destination.with_extension("part");
    let _ = std::fs::remove_file(&partial);

    let client = reqwest::blocking::Client::builder()
        .timeout(None)
        .connect_timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| InstallError::Network(e.to_string()))?;

    let mut response = client
        .get(&artifact.url)
        .send()
        .map_err(|e| InstallError::Network(e.to_string()))?;

    if !response.status().is_success() {
        return Err(InstallError::Network(format!(
            "{label}: HTTP {}",
            response.status()
        )));
    }

    let declared = response.content_length();
    if let Some(length) = declared {
        if length > MAX_ARTIFACT_BYTES {
            return Err(InstallError::Network(format!(
                "{label}: {length} bytes is larger than Relay will download"
            )));
        }
    }

    let mut file =
        std::fs::File::create(&partial).map_err(|e| InstallError::Io(e.to_string()))?;
    let mut hasher = Sha256::new();
    let mut received = 0_u64;
    let mut buffer = vec![0_u8; READ_CHUNK];
    let mut last_report = std::time::Instant::now();

    loop {
        // Cancellation is checked per chunk, so "Cancel" stops within a
        // read rather than at the end of the file.
        if is_cancelled() {
            drop(file);
            let _ = std::fs::remove_file(&partial);
            return Err(InstallError::Cancelled);
        }

        let read = std::io::Read::read(&mut response, &mut buffer)
            .map_err(|e| InstallError::Network(e.to_string()))?;
        if read == 0 {
            break;
        }

        received += read as u64;
        if received > MAX_ARTIFACT_BYTES {
            drop(file);
            let _ = std::fs::remove_file(&partial);
            return Err(InstallError::Network(format!("{label}: download too large")));
        }

        hasher.update(&buffer[..read]);
        file.write_all(&buffer[..read])
            .map_err(|e| InstallError::Io(e.to_string()))?;

        if last_report.elapsed().as_millis() >= PROGRESS_INTERVAL_MS {
            last_report = std::time::Instant::now();
            on_progress(received, declared.or(Some(artifact.size_bytes)));
        }
    }

    file.flush().map_err(|e| InstallError::Io(e.to_string()))?;
    drop(file);
    on_progress(received, declared.or(Some(artifact.size_bytes)));

    let digest = format!("{:x}", hasher.finalize());
    if !digest.eq_ignore_ascii_case(&artifact.sha256) {
        // Delete first, report second: a file that failed verification
        // must not be reachable by anything, including a later retry.
        let _ = std::fs::remove_file(&partial);
        tracing::warn!(
            "tts: {label} checksum mismatch (expected {}, got {})",
            artifact.sha256,
            digest
        );
        return Err(InstallError::ChecksumMismatch {
            label: label.to_string(),
        });
    }

    std::fs::rename(&partial, destination).map_err(|e| InstallError::Io(e.to_string()))?;
    Ok(())
}

/// Unpacks the engine and moves its executable into place.
fn install_runtime(
    runtime: &RuntimeEntry,
    archive: &Path,
    staging: &Path,
    tts_root: &Path,
) -> Result<(), InstallError> {
    let piper_dir = discovery::managed_piper_dir(tts_root);
    std::fs::create_dir_all(&piper_dir).map_err(|e| InstallError::Io(e.to_string()))?;
    let destination = piper_dir.join(discovery::piper_executable_name());

    let staged_exe = match runtime.archive {
        ArchiveKind::Raw => archive.to_path_buf(),
        ArchiveKind::Zip => {
            let extracted = staging.join("engine");
            extract_zip(archive, &extracted)?;
            find_extracted_executable(&extracted, &runtime.executable_path)?
        }
    };

    // The whole engine directory is replaced together with the
    // executable, because Piper needs its sibling libraries. Extracting
    // over a live installation would leave a mismatched pair if it
    // failed halfway.
    if runtime.archive == ArchiveKind::Zip {
        if let Some(source_dir) = staged_exe.parent() {
            copy_dir_contents(source_dir, &piper_dir)?;
        }
    } else {
        move_into_place(&staged_exe, &destination)?;
    }

    ensure_executable(&destination)?;

    if !discovery::is_executable_file(&destination) {
        return Err(InstallError::Extract(format!(
            "no runnable {} after unpacking",
            discovery::piper_executable_name()
        )));
    }
    Ok(())
}

/// Extracts a zip, refusing entries that would escape the target.
fn extract_zip(archive: &Path, into: &Path) -> Result<(), InstallError> {
    let file = std::fs::File::open(archive).map_err(|e| InstallError::Io(e.to_string()))?;
    let mut zip =
        zip::ZipArchive::new(file).map_err(|e| InstallError::Extract(e.to_string()))?;
    std::fs::create_dir_all(into).map_err(|e| InstallError::Io(e.to_string()))?;

    for index in 0..zip.len() {
        let mut entry = zip
            .by_index(index)
            .map_err(|e| InstallError::Extract(e.to_string()))?;

        // Zip-slip: an entry named `../../evil` would otherwise write
        // outside the staging directory. `enclosed_name` returns None for
        // anything that escapes.
        let Some(relative) = entry.enclosed_name() else {
            return Err(InstallError::Extract(format!(
                "archive entry {} has an unsafe path",
                entry.name()
            )));
        };
        let target = into.join(relative);

        if entry.is_dir() {
            std::fs::create_dir_all(&target).map_err(|e| InstallError::Io(e.to_string()))?;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| InstallError::Io(e.to_string()))?;
        }
        let mut out =
            std::fs::File::create(&target).map_err(|e| InstallError::Io(e.to_string()))?;
        std::io::copy(&mut entry, &mut out).map_err(|e| InstallError::Extract(e.to_string()))?;
    }
    Ok(())
}

/// Finds the executable in an extracted archive.
///
/// Prefers the manifest's declared path, then falls back to a search by
/// filename — upstream occasionally re-nests its release layout, and a
/// working install should not depend on that staying still.
fn find_extracted_executable(root: &Path, declared: &str) -> Result<PathBuf, InstallError> {
    let direct = declared
        .split('/')
        .fold(root.to_path_buf(), |path, part| path.join(part));
    if direct.is_file() {
        return Ok(direct);
    }

    let name = discovery::piper_executable_name();
    fn search(dir: &Path, name: &str, depth: usize) -> Option<PathBuf> {
        if depth > 4 {
            return None;
        }
        let entries = std::fs::read_dir(dir).ok()?;
        let mut directories = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                directories.push(path);
            } else if path.file_name().is_some_and(|n| n == name) {
                return Some(path);
            }
        }
        directories.into_iter().find_map(|d| search(&d, name, depth + 1))
    }

    search(root, name, 0).ok_or_else(|| {
        InstallError::Extract(format!("{name} was not in the downloaded archive"))
    })
}

/// Copies every file from `source` into `destination`, overwriting.
fn copy_dir_contents(source: &Path, destination: &Path) -> Result<(), InstallError> {
    std::fs::create_dir_all(destination).map_err(|e| InstallError::Io(e.to_string()))?;
    let entries = std::fs::read_dir(source).map_err(|e| InstallError::Io(e.to_string()))?;
    for entry in entries.flatten() {
        let from = entry.path();
        let to = destination.join(entry.file_name());
        if from.is_dir() {
            copy_dir_contents(&from, &to)?;
        } else {
            std::fs::copy(&from, &to).map_err(|e| InstallError::Io(e.to_string()))?;
            ensure_executable(&to)?;
        }
    }
    Ok(())
}

/// Marks a file runnable on Unix. A no-op on Windows, which has no
/// executable bit — but a zip made on Linux loses the mode, so an install
/// on any Unix host needs this or the binary is unrunnable.
fn ensure_executable(path: &Path) -> Result<(), InstallError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if path
            .extension()
            .is_some_and(|e| e == "dll" || e == "so" || e == "json" || e == "onnx")
        {
            return Ok(());
        }
        let mut permissions = std::fs::metadata(path)
            .map_err(|e| InstallError::Io(e.to_string()))?
            .permissions();
        permissions.set_mode(permissions.mode() | 0o755);
        std::fs::set_permissions(path, permissions).map_err(|e| InstallError::Io(e.to_string()))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// Moves a verified file into its final location, replacing what is there.
///
/// `rename` is atomic within a filesystem, which staging guarantees by
/// living under the same root. The copy fallback covers the odd case of a
/// root that spans devices.
fn move_into_place(from: &Path, to: &Path) -> Result<(), InstallError> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(|e| InstallError::Io(e.to_string()))?;
    }
    match std::fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(_) => {
            std::fs::copy(from, to).map_err(|e| InstallError::Io(e.to_string()))?;
            let _ = std::fs::remove_file(from);
            Ok(())
        }
    }
}

/// Checks that a downloaded model and config actually belong together.
///
/// Two files can both pass their checksums and still be the wrong pair —
/// a manifest edit that updated one URL and not the other, for instance.
/// Piper's config carries the sample rate and phoneme map for its own
/// model; a mismatch produces noise, not speech.
fn validate_voice_pair(
    model: &Path,
    config: &Path,
    voice: &VoiceEntry,
) -> Result<(), InstallError> {
    let raw = std::fs::read_to_string(config).map_err(|e| InstallError::Io(e.to_string()))?;
    let parsed: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| InstallError::Extract(format!("voice settings are not readable: {e}")))?;

    if parsed.get("audio").and_then(|a| a.get("sample_rate")).is_none() {
        return Err(InstallError::Extract(
            "voice settings are missing their audio section".to_string(),
        ));
    }

    // Piper's own config records the dataset the model was trained on.
    // When present it must match the voice we think we downloaded.
    if let Some(dataset) = parsed.get("dataset").and_then(|d| d.as_str()) {
        let expected = voice.id.rsplit_once('-').map(|(name, _)| name).unwrap_or(&voice.id);
        if !expected.eq_ignore_ascii_case(dataset)
            && !voice.id.to_lowercase().contains(&dataset.to_lowercase())
        {
            return Err(InstallError::Extract(format!(
                "voice settings describe '{dataset}', not '{}'",
                voice.id
            )));
        }
    }

    let size = std::fs::metadata(model)
        .map_err(|e| InstallError::Io(e.to_string()))?
        .len();
    if size == 0 {
        return Err(InstallError::Extract("the voice model is empty".to_string()));
    }

    Ok(())
}

/// Speaks a sentence through the production provider.
///
/// Deliberately the same `PiperProvider` Talkback uses, not a lighter
/// check: "it downloaded" and "it can speak" are different claims, and
/// only the second one earns the word Ready.
fn self_test(binary: &Path, voice: &Path, tts_root: &Path) -> Result<(), InstallError> {
    let provider = super::PiperProvider::new(
        Some(binary.to_path_buf()),
        Some(voice.to_path_buf()),
        discovery::tts_scratch_dir(tts_root),
    );
    match provider.synthesize(super::TEST_PHRASE) {
        Ok(Some(audio)) if !audio.wav_base64.is_empty() => Ok(()),
        Ok(_) => Err(InstallError::SelfTestFailed(
            "the engine produced no audio".to_string(),
        )),
        Err(e) => Err(InstallError::SelfTestFailed(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let dir =
                std::env::temp_dir().join(format!("relay_installer_{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn sha256_of(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    /// A one-shot HTTP server that serves fixed bodies. Lets the real
    /// download path — streaming, hashing, cancelling — be tested without
    /// the internet.
    struct TestServer {
        port: u16,
        shutdown: Arc<AtomicBool>,
        handle: Option<std::thread::JoinHandle<()>>,
        requests: Arc<AtomicUsize>,
    }

    impl TestServer {
        fn start(routes: Vec<(String, Vec<u8>)>) -> Self {
            Self::start_inner(routes.into_iter().map(|(p, b)| (p, b, None)).collect())
        }

        /// A server that declares a `Content-Length` it has no intention of
        /// sending — the shape of a hostile or misconfigured mirror.
        fn start_declaring(path: &str, body: Vec<u8>, declared: u64) -> Self {
            Self::start_inner(vec![(path.to_string(), body, Some(declared))])
        }

        fn start_inner(routes: Vec<(String, Vec<u8>, Option<u64>)>) -> Self {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            listener.set_nonblocking(true).unwrap();
            let shutdown = Arc::new(AtomicBool::new(false));
            let requests = Arc::new(AtomicUsize::new(0));
            let stop = shutdown.clone();
            let counter = requests.clone();

            let handle = std::thread::spawn(move || {
                use std::io::{BufRead, BufReader, Write};
                while !stop.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            counter.fetch_add(1, Ordering::Relaxed);
                            let mut reader = BufReader::new(stream.try_clone().unwrap());
                            let mut line = String::new();
                            let _ = reader.read_line(&mut line);
                            let path = line.split_whitespace().nth(1).unwrap_or("/").to_string();

                            let body = routes
                                .iter()
                                .find(|(route, _, _)| *route == path)
                                .map(|(_, body, declared)| (body.clone(), *declared));

                            let response = match body {
                                Some((body, declared)) => {
                                    let mut out = format!(
                                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                        declared.unwrap_or(body.len() as u64)
                                    )
                                    .into_bytes();
                                    out.extend_from_slice(&body);
                                    out
                                }
                                None => b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
                            };
                            let _ = stream.write_all(&response);
                            let _ = stream.flush();
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(std::time::Duration::from_millis(5));
                        }
                        Err(_) => break,
                    }
                }
            });

            Self {
                port,
                shutdown,
                handle: Some(handle),
                requests,
            }
        }

        fn url(&self, path: &str) -> String {
            format!("http://127.0.0.1:{}{path}", self.port)
        }
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            self.shutdown.store(true, Ordering::Relaxed);
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    fn never_cancelled() -> impl Fn() -> bool + Send + Sync {
        || false
    }

    fn no_progress() -> impl Fn(u64, Option<u64>) + Send + Sync {
        |_, _| {}
    }

    // ── download_verified ───────────────────────────────────────────────

    #[test]
    fn a_verified_download_lands_at_its_destination() {
        let body = b"the voice engine".to_vec();
        let server = TestServer::start(vec![("/engine.zip".to_string(), body.clone())]);
        let temp = TempDir::new();
        let destination = temp.path().join("engine");

        let artifact = Artifact {
            url: server.url("/engine.zip"),
            sha256: sha256_of(&body),
            size_bytes: body.len() as u64,
        };

        download_verified(
            &artifact,
            &destination,
            "engine",
            &never_cancelled(),
            &no_progress(),
        )
        .expect("download");

        assert_eq!(std::fs::read(&destination).unwrap(), body);
        assert!(
            !destination.with_extension("part").exists(),
            "the partial file was left behind"
        );
    }

    #[test]
    fn a_corrupted_download_is_rejected_and_deleted() {
        // The central integrity property: a body that does not hash to
        // the pinned value never reaches the destination.
        let server = TestServer::start(vec![("/engine.zip".to_string(), b"tampered".to_vec())]);
        let temp = TempDir::new();
        let destination = temp.path().join("engine");

        let artifact = Artifact {
            url: server.url("/engine.zip"),
            sha256: sha256_of(b"what relay expected"),
            size_bytes: 8,
        };

        let error = download_verified(
            &artifact,
            &destination,
            "engine",
            &never_cancelled(),
            &no_progress(),
        )
        .unwrap_err();

        assert!(matches!(error, InstallError::ChecksumMismatch { .. }));
        assert!(!destination.exists(), "a corrupt artifact was kept");
        assert!(!destination.with_extension("part").exists(), "partial kept");
        assert!(error.is_retryable());
    }

    #[test]
    fn an_unpinned_artifact_is_never_downloaded() {
        let server = TestServer::start(vec![("/x".to_string(), b"anything".to_vec())]);
        let temp = TempDir::new();

        let artifact = Artifact {
            url: server.url("/x"),
            sha256: String::new(),
            size_bytes: 8,
        };
        let error = download_verified(
            &artifact,
            &temp.path().join("out"),
            "engine",
            &never_cancelled(),
            &no_progress(),
        )
        .unwrap_err();

        assert!(matches!(error, InstallError::NotProvisioned(_)));
        assert_eq!(
            server.requests.load(Ordering::Relaxed),
            0,
            "Relay contacted the server for an artifact it could not verify"
        );
    }

    #[test]
    fn a_missing_artifact_is_a_network_error_not_a_silent_success() {
        let server = TestServer::start(vec![]);
        let temp = TempDir::new();
        let artifact = Artifact {
            url: server.url("/gone"),
            sha256: sha256_of(b"x"),
            size_bytes: 1,
        };
        assert!(matches!(
            download_verified(
                &artifact,
                &temp.path().join("out"),
                "engine",
                &never_cancelled(),
                &no_progress()
            ),
            Err(InstallError::Network(_))
        ));
    }

    #[test]
    fn cancelling_stops_the_download_and_leaves_nothing_behind() {
        let body = vec![7_u8; 512 * 1024];
        let server = TestServer::start(vec![("/big".to_string(), body.clone())]);
        let temp = TempDir::new();
        let destination = temp.path().join("out");

        let artifact = Artifact {
            url: server.url("/big"),
            sha256: sha256_of(&body),
            size_bytes: body.len() as u64,
        };

        let error = download_verified(
            &artifact,
            &destination,
            "engine",
            &|| true,
            &no_progress(),
        )
        .unwrap_err();

        assert!(matches!(error, InstallError::Cancelled));
        assert!(!destination.exists());
        assert!(!destination.with_extension("part").exists());
        assert!(error.is_retryable(), "a cancelled setup must be retryable");
    }

    #[test]
    fn an_artifact_larger_than_relay_will_download_is_refused() {
        // The cap is checked against the declared length before a byte of
        // the body is read, so a hostile mirror cannot fill the disk.
        let body = b"not actually a gigabyte".to_vec();
        let server = TestServer::start_declaring("/huge", body.clone(), MAX_ARTIFACT_BYTES + 1);
        let temp = TempDir::new();
        let destination = temp.path().join("engine");

        let artifact = Artifact {
            url: server.url("/huge"),
            sha256: sha256_of(&body),
            size_bytes: body.len() as u64,
        };
        let error = download_verified(
            &artifact,
            &destination,
            "engine",
            &never_cancelled(),
            &no_progress(),
        )
        .unwrap_err();

        assert!(matches!(error, InstallError::Network(_)), "{error:?}");
        assert!(!destination.exists());
        assert!(
            !destination.with_extension("part").exists(),
            "nothing may be written for an artifact Relay refuses"
        );
    }

    #[test]
    fn progress_is_reported_with_a_total() {
        let body = vec![3_u8; 256 * 1024];
        let server = TestServer::start(vec![("/f".to_string(), body.clone())]);
        let temp = TempDir::new();
        type Reports = Arc<Mutex<Vec<(u64, Option<u64>)>>>;
        let seen: Reports = Arc::new(Mutex::new(Vec::new()));
        let recorder = seen.clone();

        let artifact = Artifact {
            url: server.url("/f"),
            sha256: sha256_of(&body),
            size_bytes: body.len() as u64,
        };
        download_verified(
            &artifact,
            &temp.path().join("out"),
            "engine",
            &never_cancelled(),
            &move |received, total| recorder.lock().unwrap().push((received, total)),
        )
        .unwrap();

        let reports = seen.lock().unwrap();
        let last = reports.last().expect("at least one progress report");
        assert_eq!(last.0, body.len() as u64);
        assert_eq!(last.1, Some(body.len() as u64));
    }

    #[test]
    fn an_interrupted_download_recovers_on_the_next_attempt() {
        let body = b"the voice engine".to_vec();
        let server = TestServer::start(vec![("/engine".to_string(), body.clone())]);
        let temp = TempDir::new();
        let destination = temp.path().join("engine");

        // Leave the debris a killed process would.
        std::fs::write(destination.with_extension("part"), b"half a file").unwrap();

        let artifact = Artifact {
            url: server.url("/engine"),
            sha256: sha256_of(&body),
            size_bytes: body.len() as u64,
        };
        download_verified(
            &artifact,
            &destination,
            "engine",
            &never_cancelled(),
            &no_progress(),
        )
        .expect("a stale partial file must not block a retry");

        assert_eq!(std::fs::read(&destination).unwrap(), body);
    }

    // ── staging ─────────────────────────────────────────────────────────

    #[test]
    fn staging_is_removed_when_the_install_ends() {
        let temp = TempDir::new();
        let staged_path;
        {
            let staging = StagingDir::create(temp.path()).unwrap();
            staged_path = staging.path().to_path_buf();
            std::fs::write(staged_path.join("partial"), b"x").unwrap();
            assert!(staged_path.exists());
        }
        assert!(!staged_path.exists(), "a failed install left staging behind");
    }

    #[test]
    fn clearing_staging_tolerates_a_missing_directory() {
        let temp = TempDir::new();
        clear_staging(temp.path());
        let staging = temp.path().join(".staging/leftover");
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("f"), b"x").unwrap();
        clear_staging(temp.path());
        assert!(!temp.path().join(".staging").exists());
    }

    // ── zip extraction ──────────────────────────────────────────────────

    fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
        use zip::write::SimpleFileOptions;
        let file = std::fs::File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        for (name, body) in entries {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            std::io::Write::write_all(&mut writer, body).unwrap();
        }
        writer.finish().unwrap();
    }

    #[test]
    fn a_zip_extracts_and_its_executable_is_found() {
        let temp = TempDir::new();
        let archive = temp.path().join("engine.zip");
        let exe = discovery::piper_executable_name();
        write_zip(
            &archive,
            &[
                (&format!("piper/{exe}"), b"binary"),
                ("piper/espeak-ng-data/phontab", b"data"),
            ],
        );

        let extracted = temp.path().join("out");
        extract_zip(&archive, &extracted).unwrap();
        let found = find_extracted_executable(&extracted, &format!("piper/{exe}")).unwrap();
        assert_eq!(std::fs::read(&found).unwrap(), b"binary");
        assert!(extracted.join("piper/espeak-ng-data/phontab").exists());
    }

    #[test]
    fn the_executable_is_found_even_if_upstream_moves_it() {
        let temp = TempDir::new();
        let archive = temp.path().join("engine.zip");
        let exe = discovery::piper_executable_name();
        // Nested one level deeper than the manifest declares.
        write_zip(&archive, &[(&format!("piper-1.7/bin/{exe}"), b"binary")]);

        let extracted = temp.path().join("out");
        extract_zip(&archive, &extracted).unwrap();
        let found = find_extracted_executable(&extracted, &format!("piper/{exe}")).unwrap();
        assert!(found.ends_with(exe));
    }

    #[test]
    fn an_archive_without_the_executable_is_an_error() {
        let temp = TempDir::new();
        let archive = temp.path().join("engine.zip");
        write_zip(&archive, &[("readme.txt", b"nothing useful")]);

        let extracted = temp.path().join("out");
        extract_zip(&archive, &extracted).unwrap();
        assert!(matches!(
            find_extracted_executable(&extracted, "piper/piper.exe"),
            Err(InstallError::Extract(_))
        ));
    }

    #[test]
    fn a_zip_cannot_write_outside_its_target_directory() {
        // Zip-slip. `enclosed_name` rejects the traversal, so the entry
        // never reaches the filesystem.
        let temp = TempDir::new();
        let archive = temp.path().join("evil.zip");
        write_zip(&archive, &[("../../escaped.txt", b"pwned")]);

        let extracted = temp.path().join("out");
        let result = extract_zip(&archive, &extracted);
        assert!(result.is_err(), "a traversal entry was extracted");
        assert!(!temp.path().join("escaped.txt").exists());
    }

    #[test]
    fn a_corrupt_archive_is_an_extract_error_not_a_panic() {
        let temp = TempDir::new();
        let archive = temp.path().join("broken.zip");
        std::fs::write(&archive, b"this is not a zip file").unwrap();
        assert!(matches!(
            extract_zip(&archive, &temp.path().join("out")),
            Err(InstallError::Extract(_))
        ));
    }

    // ── atomic move ─────────────────────────────────────────────────────

    #[test]
    fn moving_into_place_replaces_an_existing_file() {
        let temp = TempDir::new();
        let from = temp.path().join("new");
        let to = temp.path().join("voices/model.onnx");
        std::fs::write(&from, b"new model").unwrap();
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::write(&to, b"old model").unwrap();

        move_into_place(&from, &to).unwrap();
        assert_eq!(std::fs::read(&to).unwrap(), b"new model");
        assert!(!from.exists());
    }

    // ── voice pairing ───────────────────────────────────────────────────

    fn voice_entry(id: &str) -> VoiceEntry {
        VoiceEntry {
            id: id.to_string(),
            display_name: id.to_string(),
            language: "en_US".to_string(),
            language_label: "English (US)".to_string(),
            description: String::new(),
            recommended: true,
            model: Artifact {
                url: "https://x.invalid/m".to_string(),
                sha256: "a".repeat(64),
                size_bytes: 1,
            },
            config: Artifact {
                url: "https://x.invalid/c".to_string(),
                sha256: "b".repeat(64),
                size_bytes: 1,
            },
            license: "MIT".to_string(),
            source: "https://x.invalid".to_string(),
        }
    }

    fn write_pair(dir: &Path, config_json: &str, model_bytes: &[u8]) -> (PathBuf, PathBuf) {
        let model = dir.join("voice.onnx");
        let config = dir.join("voice.onnx.json");
        std::fs::write(&model, model_bytes).unwrap();
        std::fs::write(&config, config_json).unwrap();
        (model, config)
    }

    #[test]
    fn a_matching_model_and_config_validate() {
        let temp = TempDir::new();
        let (model, config) = write_pair(
            temp.path(),
            r#"{"audio":{"sample_rate":22050},"dataset":"amy"}"#,
            b"onnx bytes",
        );
        assert!(validate_voice_pair(&model, &config, &voice_entry("en_US-amy-medium")).is_ok());
    }

    #[test]
    fn a_config_from_a_different_voice_is_rejected() {
        // Both files can pass their own checksums and still be the wrong
        // pair — a manifest edit that changed one URL and not the other.
        let temp = TempDir::new();
        let (model, config) = write_pair(
            temp.path(),
            r#"{"audio":{"sample_rate":22050},"dataset":"ryan"}"#,
            b"onnx bytes",
        );
        let error =
            validate_voice_pair(&model, &config, &voice_entry("en_US-amy-medium")).unwrap_err();
        assert!(error.detail().contains("ryan"), "{}", error.detail());
    }

    #[test]
    fn a_config_missing_its_audio_section_is_rejected() {
        let temp = TempDir::new();
        let (model, config) = write_pair(temp.path(), r#"{"dataset":"amy"}"#, b"onnx");
        assert!(validate_voice_pair(&model, &config, &voice_entry("en_US-amy-medium")).is_err());
    }

    #[test]
    fn an_unparseable_config_is_rejected() {
        let temp = TempDir::new();
        let (model, config) = write_pair(temp.path(), "<html>404</html>", b"onnx");
        assert!(validate_voice_pair(&model, &config, &voice_entry("en_US-amy-medium")).is_err());
    }

    #[test]
    fn an_empty_model_is_rejected() {
        let temp = TempDir::new();
        let (model, config) =
            write_pair(temp.path(), r#"{"audio":{"sample_rate":22050}}"#, b"");
        assert!(validate_voice_pair(&model, &config, &voice_entry("en_US-amy-medium")).is_err());
    }

    // ── errors ──────────────────────────────────────────────────────────

    #[test]
    fn every_error_has_a_code_and_a_user_facing_message() {
        // Every detail below is deliberately the kind of string that must not
        // reach a user: a URL, a Windows path, a Rust error, a stack frame.
        const LEAKY: &str =
            r#"Err(reqwest https://example.com/a.zip -> C:\Users\nitin\AppData\x.onnx at installer.rs:412)"#;
        let errors = [
            InstallError::NotProvisioned(LEAKY.into()),
            InstallError::Network(LEAKY.into()),
            InstallError::ChecksumMismatch { label: "engine".into() },
            InstallError::Extract(LEAKY.into()),
            InstallError::Io(LEAKY.into()),
            InstallError::SelfTestFailed(LEAKY.into()),
            InstallError::Cancelled,
        ];
        for error in errors {
            assert!(!error.code().is_empty());
            let message = error.to_string();
            assert!(message.len() > 10, "too terse: {message}");
            // The detail is for the log. None of it may reach the message.
            assert!(!message.contains(&error.detail()), "detail leaked: {message}");
            assert!(!message.contains("Err("), "a Rust error leaked: {message}");
            assert!(!message.contains(".rs:"), "a stack frame leaked: {message}");
            assert!(!message.contains("http"), "a URL leaked: {message}");
            assert!(!message.contains('\\'), "a Windows path leaked: {message}");
            assert!(!message.contains(":\\"), "a drive letter leaked: {message}");
            assert!(!message.contains(".onnx"), "an internal filename leaked: {message}");
        }

        // `Unsupported` is the one variant whose detail *is* the message: it
        // names the machine, which the user needs, and nothing else.
        let unsupported =
            InstallError::Unsupported("Automatic voice setup isn't available for freebsd yet".into());
        let message = unsupported.to_string();
        assert!(message.contains("freebsd"));
        assert!(!message.contains("http"), "{message}");
        assert!(!message.contains('\\'), "{message}");
    }

    #[test]
    fn unsupported_machines_are_not_offered_a_retry() {
        assert!(!InstallError::Unsupported("aarch64 is not supported yet".into()).is_retryable());
        assert!(!InstallError::NotProvisioned("x".into()).is_retryable());
        assert!(InstallError::Network("x".into()).is_retryable());
        assert!(InstallError::ChecksumMismatch { label: "x".into() }.is_retryable());
    }

    #[test]
    fn every_stage_has_user_facing_copy() {
        for stage in [
            InstallStage::Preparing,
            InstallStage::DownloadingEngine,
            InstallStage::DownloadingVoice,
            InstallStage::Installing,
            InstallStage::Validating,
            InstallStage::Testing,
            InstallStage::Done,
        ] {
            assert!(!stage.label().is_empty(), "{stage:?} has no label");
        }
    }

    // ── install(), end to end ───────────────────────────────────────────

    #[test]
    fn an_unprovisioned_manifest_never_starts_an_install() {
        let temp = TempDir::new();
        let manifest = VoiceManifest {
            schema_version: super::super::manifest::SCHEMA_VERSION,
            runtimes: vec![],
            voices: vec![],
        };
        let error = install(InstallRequest {
            manifest: &manifest,
            voice_id: "en_US-amy-medium",
            tts_root: temp.path(),
            platform: "windows",
            arch: "x86_64",
            on_progress: &|_| {},
            is_cancelled: &|| false,
        })
        .unwrap_err();
        assert!(matches!(error, InstallError::NotProvisioned(_)));
        assert!(!error.is_retryable());
    }

    #[test]
    fn an_unsupported_platform_fails_before_any_download() {
        let temp = TempDir::new();
        let manifest = provisioned_manifest("https://x.invalid/e", "https://x.invalid/m", "https://x.invalid/c");
        let error = install(InstallRequest {
            manifest: &manifest,
            voice_id: "en_US-amy-medium",
            tts_root: temp.path(),
            platform: "freebsd",
            arch: "x86_64",
            on_progress: &|_| {},
            is_cancelled: &|| false,
        })
        .unwrap_err();
        assert!(matches!(error, InstallError::Unsupported(_)));
        assert!(error.to_string().contains("freebsd"));
    }

    #[test]
    fn an_unsupported_architecture_is_reported_distinctly() {
        let temp = TempDir::new();
        let manifest = provisioned_manifest("https://x.invalid/e", "https://x.invalid/m", "https://x.invalid/c");
        let error = install(InstallRequest {
            manifest: &manifest,
            voice_id: "en_US-amy-medium",
            tts_root: temp.path(),
            platform: "windows",
            arch: "aarch64",
            on_progress: &|_| {},
            is_cancelled: &|| false,
        })
        .unwrap_err();
        assert!(error.to_string().contains("aarch64"));
    }

    #[test]
    fn an_unknown_voice_is_refused() {
        let temp = TempDir::new();
        let manifest = provisioned_manifest("https://x.invalid/e", "https://x.invalid/m", "https://x.invalid/c");
        assert!(matches!(
            install(InstallRequest {
                manifest: &manifest,
                voice_id: "not-a-voice",
                tts_root: temp.path(),
                platform: "windows",
                arch: "x86_64",
                on_progress: &|_| {},
                is_cancelled: &|| false,
            }),
            Err(InstallError::Unsupported(_))
        ));
    }

    #[test]
    fn a_failed_install_leaves_an_existing_installation_untouched() {
        // The promise a retry depends on: a user with a working voice who
        // tries to add another must not lose the one they had.
        let temp = TempDir::new();
        let piper_dir = discovery::managed_piper_dir(temp.path());
        std::fs::create_dir_all(&piper_dir).unwrap();
        let existing = piper_dir.join(discovery::piper_executable_name());
        std::fs::write(&existing, b"the working engine").unwrap();

        let voices = discovery::managed_voices_dir(temp.path());
        std::fs::create_dir_all(&voices).unwrap();
        std::fs::write(voices.join("en_US-amy-medium.onnx"), b"working model").unwrap();

        // Point the voice download at a server that serves the wrong body.
        let server = TestServer::start(vec![("/m".to_string(), b"corrupt".to_vec())]);
        let manifest = provisioned_manifest(
            "https://x.invalid/e",
            &server.url("/m"),
            &server.url("/c"),
        );

        let error = install(InstallRequest {
            manifest: &manifest,
            voice_id: "en_US-amy-medium",
            tts_root: temp.path(),
            platform: "windows",
            arch: "x86_64",
            on_progress: &|_| {},
            is_cancelled: &|| false,
        })
        .unwrap_err();

        assert!(error.is_retryable(), "{error}");
        assert_eq!(
            std::fs::read(&existing).unwrap(),
            b"the working engine",
            "a failed install replaced a working engine"
        );
        assert_eq!(
            std::fs::read(voices.join("en_US-amy-medium.onnx")).unwrap(),
            b"working model",
            "a failed install replaced a working voice"
        );
        assert!(
            !temp.path().join(".staging").exists(),
            "staging survived a failure"
        );
    }

    #[test]
    fn an_existing_engine_is_reused_rather_than_re_downloaded() {
        let temp = TempDir::new();
        let piper_dir = discovery::managed_piper_dir(temp.path());
        std::fs::create_dir_all(&piper_dir).unwrap();
        let existing = piper_dir.join(discovery::piper_executable_name());
        std::fs::write(&existing, b"engine").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&existing, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        // The engine URL is unreachable; reaching it would fail the test.
        let server = TestServer::start(vec![("/m".to_string(), b"model".to_vec())]);
        let manifest = provisioned_manifest(
            "https://definitely.invalid/engine",
            &server.url("/m"),
            &server.url("/missing-config"),
        );

        let stages: Arc<Mutex<Vec<InstallStage>>> = Arc::new(Mutex::new(Vec::new()));
        let recorder = stages.clone();

        // Fails later (no config served), but must not fail on the engine.
        let _ = install(InstallRequest {
            manifest: &manifest,
            voice_id: "en_US-amy-medium",
            tts_root: temp.path(),
            platform: "windows",
            arch: "x86_64",
            on_progress: &move |p| recorder.lock().unwrap().push(p.stage),
            is_cancelled: &|| false,
        });

        let seen = stages.lock().unwrap();
        assert!(
            !seen.contains(&InstallStage::DownloadingEngine),
            "the engine was re-downloaded despite already being installed"
        );
        assert!(seen.contains(&InstallStage::DownloadingVoice));
    }

    #[test]
    fn cancelling_before_any_work_reports_cancelled() {
        let temp = TempDir::new();
        let manifest = provisioned_manifest("https://x.invalid/e", "https://x.invalid/m", "https://x.invalid/c");
        let error = install(InstallRequest {
            manifest: &manifest,
            voice_id: "en_US-amy-medium",
            tts_root: temp.path(),
            platform: "windows",
            arch: "x86_64",
            on_progress: &|_| {},
            is_cancelled: &|| true,
        })
        .unwrap_err();
        assert!(matches!(error, InstallError::Cancelled));
        assert!(!temp.path().join(".staging").exists());
    }

    /// A manifest whose artifacts are pinned to the given URLs. The
    /// checksums are real hashes of the test bodies where it matters.
    fn provisioned_manifest(engine: &str, model: &str, config: &str) -> VoiceManifest {
        use super::super::manifest::{RuntimeEntry, SCHEMA_VERSION};
        VoiceManifest {
            schema_version: SCHEMA_VERSION,
            runtimes: vec![RuntimeEntry {
                id: "piper-windows-x86_64".to_string(),
                engine: "piper".to_string(),
                version: "1.6.0".to_string(),
                platform: "windows".to_string(),
                arch: "x86_64".to_string(),
                archive: ArchiveKind::Zip,
                executable_path: "piper/piper.exe".to_string(),
                artifact: Artifact {
                    url: engine.to_string(),
                    sha256: sha256_of(b"engine archive"),
                    size_bytes: 14,
                },
                license: "GPL-3.0-only".to_string(),
                source: "https://github.com/OHF-Voice/piper1-gpl".to_string(),
            }],
            voices: vec![VoiceEntry {
                model: Artifact {
                    url: model.to_string(),
                    sha256: sha256_of(b"model"),
                    size_bytes: 5,
                },
                config: Artifact {
                    url: config.to_string(),
                    sha256: sha256_of(b"config"),
                    size_bytes: 6,
                },
                ..voice_entry("en_US-amy-medium")
            }],
        }
    }
}
