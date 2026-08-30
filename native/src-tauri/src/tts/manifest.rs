//! The Relay-owned catalogue of voice runtimes and voices.
//!
//! One place decides what Relay is willing to download, from where, and
//! what it must hash to. The UI never constructs a URL; it names a voice
//! id and the installer looks it up here. That is the whole point: an
//! interface that can build download URLs is an interface that can be
//! talked into downloading something else.
//!
//! The catalogue is a checked-in JSON file compiled into the binary with
//! `include_str!`. Not a Tauri resource, not a runtime fetch — a resource
//! can go missing from a bundle and a fetch needs a server Relay does not
//! have. Compiled in, the manifest either parses at startup or the build
//! is broken, and there is no third state.
//!
//! ## Which upstream build the catalogue points at
//!
//! Relay installs a **standalone executable** and spawns it. That
//! narrows the field to one upstream distribution: `rhasspy/piper`'s
//! release archives, which contain `piper/piper.exe` (or `piper/piper`)
//! alongside the ONNX runtime and espeak-ng data it needs.
//!
//! The successor project `OHF-Voice/piper1-gpl` is **not** a drop-in
//! substitute here, and the difference is packaging rather than
//! preference: its release workflow uploads `dist/*`, which is Python
//! wheels and an sdist. `piper_tts-1.7.0-cp39-abi3-win_amd64.whl` is a
//! CPython package — `piper/*.py` plus an `espeakbridge.pyd` extension —
//! that needs an interpreter and `onnxruntime` installed to run at all.
//! There is no `.exe` in it, in any release from v1.3.0 to v1.7.0. Its
//! C++ CLI (`libpiper`) is built in CI but never published as a release
//! asset. Pointing the installer at a wheel would not be a rename; it
//! would be asking Relay to execute a zip full of Python.
//!
//! So each runtime here names its provenance explicitly — `repo`, `tag`
//! and the exact `asset` filename — and [`validate`] re-derives the
//! download URL from those three fields. An artifact cannot be swapped
//! for a different one without the swap showing up in the JSON, and an
//! asset whose extension disagrees with its declared `archive` kind is
//! refused rather than guessed at.
//!
//! ## Provisioning
//!
//! Checksums cannot be invented. `scripts/build-voice-manifest.mjs`
//! downloads each artifact, hashes it, and rewrites the JSON; until that
//! has been run, entries carry no `sha256` and [`validate`] rejects them,
//! which the installer surfaces as "voice setup isn't available in this
//! build" rather than attempting an unverified download. A manifest that
//! cannot be trusted is treated as no manifest at all.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The catalogue, compiled in at build time.
const MANIFEST_JSON: &str = include_str!("../../resources/voice-manifest.json");

/// Bumped when the shape changes incompatibly, so an old file fails loudly
/// instead of deserializing into something subtly wrong.
///
/// 2 added per-runtime release provenance and the `tar_gz` archive kind.
/// A version-1 file names neither, and its Linux entry claims to be a zip
/// when the artifact it points at is a tarball — exactly the kind of
/// quietly-wrong state this number exists to prevent.
pub const SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ManifestError {
    #[error("The voice catalogue could not be read: {0}")]
    Unreadable(String),

    #[error("The voice catalogue is version {found}, but this build expects {expected}")]
    SchemaMismatch { found: u32, expected: u32 },

    #[error("{0}")]
    Invalid(String),

    #[error("Automatic voice setup isn't available for {platform} yet")]
    UnsupportedPlatform { platform: String },

    #[error("Automatic voice setup isn't available for {arch} processors yet")]
    UnsupportedArch { platform: String, arch: String },

    #[error("No voice called {0} is available")]
    UnknownVoice(String),
}

/// A single downloadable file, with everything needed to verify it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    pub url: String,
    /// Lowercase hex SHA-256. Empty means "not provisioned" — see the
    /// module docs. Never optional in a released build.
    #[serde(default)]
    pub sha256: String,
    /// Expected size, for progress reporting and as a cheap first check.
    #[serde(default)]
    pub size_bytes: u64,
}

impl Artifact {
    /// Whether this artifact carries a usable pinned digest.
    ///
    /// A SHA-256 is 64 hex characters. Anything else — empty, a
    /// placeholder, a truncated paste — is not a checksum, and treating
    /// it as one is how an unverified binary gets executed.
    pub fn is_pinned(&self) -> bool {
        self.sha256.len() == 64 && self.sha256.chars().all(|c| c.is_ascii_hexdigit())
    }

    /// Whether this URL is safe to fetch over.
    ///
    /// HTTPS everywhere, with one carve-out: loopback. A URL that
    /// resolves to this machine cannot be intercepted by a network
    /// attacker, and allowing it is what lets the installer's download,
    /// verification and atomicity behaviour be tested end-to-end against
    /// a local server instead of a TLS fixture. The shipped catalogue
    /// contains no loopback URLs, and a test asserts that.
    fn transport_is_safe(&self) -> bool {
        self.url.starts_with("https://")
            || self.url.starts_with("http://127.0.0.1:")
            || self.url.starts_with("http://localhost:")
    }

    fn problems(&self, label: &str) -> Vec<String> {
        let mut problems = Vec::new();
        if !self.transport_is_safe() {
            problems.push(format!("{label}: download URL is not https"));
        }
        if !self.is_pinned() {
            problems.push(format!("{label}: no pinned SHA-256"));
        }
        if self.size_bytes == 0 {
            problems.push(format!("{label}: no expected size"));
        }
        problems
    }
}

/// How a runtime download is packaged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArchiveKind {
    /// A zip archive; the executable is extracted from `executable_path`.
    Zip,
    /// A gzipped tar. Upstream ships Windows as a zip and every Unix
    /// platform as a tarball, so supporting only one of the two means
    /// supporting only one of the platforms.
    TarGz,
    /// The download *is* the executable.
    Raw,
}

impl ArchiveKind {
    /// Whether the download has to be unpacked to find the executable.
    pub fn is_archive(self) -> bool {
        !matches!(self, Self::Raw)
    }

    /// The kind implied by a filename, or `None` when it names no
    /// packaging Relay can unpack.
    ///
    /// Used to check the declared `archive` against the asset actually
    /// named, so a manifest cannot claim `zip` while pointing at a
    /// tarball — or at a Python wheel.
    fn from_filename(name: &str) -> Option<Self> {
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
            Some(Self::TarGz)
        } else if lower.ends_with(".zip") {
            Some(Self::Zip)
        } else {
            None
        }
    }
}

/// Where a runtime artifact comes from, named rather than inferred.
///
/// The generator resolves the asset by this exact name and no other
/// rule — no pattern matching over a release's asset list, which is how
/// a wheel gets mistaken for an engine. [`VoiceManifest::validate`] then
/// re-derives the download URL from these three fields and requires the
/// artifact to carry it, so provenance is checked at load time on the
/// user's machine, not only at release time on ours.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseSource {
    /// `owner/name` on GitHub.
    pub repo: String,
    /// The release tag, pinned. Never "latest": a project is free to
    /// change what it publishes, and Relay is not free to run it.
    pub tag: String,
    /// The exact asset filename within that release.
    pub asset: String,
}

impl ReleaseSource {
    /// The canonical download URL for this asset.
    pub fn download_url(&self) -> String {
        format!(
            "https://github.com/{}/releases/download/{}/{}",
            self.repo, self.tag, self.asset
        )
    }

    /// The project page, for the licence notice in the UI.
    pub fn project_url(&self) -> String {
        format!("https://github.com/{}", self.repo)
    }

    fn problems(&self, label: &str) -> Vec<String> {
        let mut problems = Vec::new();
        let segments: Vec<&str> = self.repo.split('/').collect();
        if segments.len() != 2 || segments.iter().any(|s| s.is_empty()) {
            problems.push(format!("{label}: release repo must be owner/name"));
        }
        if self.tag.is_empty() {
            problems.push(format!("{label}: release tag is not pinned"));
        }
        // A path separator in either would let the derived URL point
        // somewhere other than this release.
        for (field, value) in [("tag", &self.tag), ("asset", &self.asset)] {
            if value.contains('/') || value.contains("..") {
                problems.push(format!("{label}: release {field} is not a plain name"));
            }
        }
        if self.asset.is_empty() {
            problems.push(format!("{label}: no release asset named"));
        }
        problems
    }
}

/// A speech engine build for one platform and architecture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeEntry {
    pub id: String,
    /// Which `TtsProvider` this feeds. Only `piper` today; the field
    /// exists so a second engine is a manifest entry, not a schema change.
    pub engine: String,
    pub version: String,
    /// `windows`, `macos`, `linux` — matched against [`current_platform`].
    pub platform: String,
    /// `x86_64`, `aarch64` — matched against [`current_arch`].
    pub arch: String,
    pub archive: ArchiveKind,
    /// Path of the executable inside the archive, forward-slashed.
    /// Ignored for [`ArchiveKind::Raw`].
    #[serde(default)]
    pub executable_path: String,
    /// The upstream release this artifact is taken from.
    pub release: ReleaseSource,
    pub artifact: Artifact,
    pub license: String,
    pub source: String,
}

/// A voice Relay has explicitly validated and is willing to offer.
///
/// Deliberately a short curated list rather than a mirror of the whole
/// Piper voice repository: every entry here is one Relay has a checksum
/// for and has decided sounds acceptable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoiceEntry {
    /// Stable id, matching Piper's own naming: `en_US-amy-medium`.
    pub id: String,
    /// What the user sees: "English (US) — Amy".
    pub display_name: String,
    /// BCP-47-ish tag from the voice's own filename: `en_US`.
    pub language: String,
    /// "English (US)".
    pub language_label: String,
    /// One line of character, for the picker.
    #[serde(default)]
    pub description: String,
    /// Exactly one voice per manifest is the recommended default, so
    /// first-run setup never asks the user to choose.
    #[serde(default)]
    pub recommended: bool,
    pub model: Artifact,
    pub config: Artifact,
    pub license: String,
    pub source: String,
}

impl VoiceEntry {
    /// Total bytes this voice will download, for progress.
    pub fn total_bytes(&self) -> u64 {
        self.model.size_bytes + self.config.size_bytes
    }

    /// Filename the model is stored under. Derived from the id rather
    /// than the URL so a redirect or a mirror cannot change where the
    /// file lands.
    pub fn model_filename(&self) -> String {
        format!("{}.onnx", self.id)
    }

    pub fn config_filename(&self) -> String {
        format!("{}.onnx.json", self.id)
    }
}

/// The whole catalogue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoiceManifest {
    pub schema_version: u32,
    #[serde(default)]
    pub runtimes: Vec<RuntimeEntry>,
    #[serde(default)]
    pub voices: Vec<VoiceEntry>,
}

impl VoiceManifest {
    /// Parses and validates the compiled-in catalogue.
    pub fn load() -> Result<Self, ManifestError> {
        Self::parse(MANIFEST_JSON)
    }

    pub fn parse(json: &str) -> Result<Self, ManifestError> {
        let manifest: VoiceManifest =
            serde_json::from_str(json).map_err(|e| ManifestError::Unreadable(e.to_string()))?;
        if manifest.schema_version != SCHEMA_VERSION {
            return Err(ManifestError::SchemaMismatch {
                found: manifest.schema_version,
                expected: SCHEMA_VERSION,
            });
        }
        manifest.validate()?;
        Ok(manifest)
    }

    /// Rejects a catalogue that could not be trusted or acted on.
    ///
    /// Runs at load, so a manifest problem is a startup-visible fault
    /// rather than a mid-download surprise.
    pub fn validate(&self) -> Result<(), ManifestError> {
        let mut problems: Vec<String> = Vec::new();

        if self.runtimes.is_empty() {
            problems.push("no runtimes listed".to_string());
        }
        if self.voices.is_empty() {
            problems.push("no voices listed".to_string());
        }

        let mut runtime_ids = std::collections::HashSet::new();
        for runtime in &self.runtimes {
            if !runtime_ids.insert(&runtime.id) {
                problems.push(format!("duplicate runtime id {}", runtime.id));
            }
            if runtime.engine != "piper" {
                problems.push(format!(
                    "runtime {} names engine {}, which this build cannot install",
                    runtime.id, runtime.engine
                ));
            }
            let label = format!("runtime {}", runtime.id);
            if runtime.archive.is_archive() {
                problems.extend(executable_path_problems(&runtime.executable_path, &label));
            }
            problems.extend(runtime.release.problems(&label));
            problems.extend(archive_matches_asset(runtime, &label));
            problems.extend(artifact_matches_release(runtime, &label));
            problems.extend(runtime.artifact.problems(&label));
        }

        let mut voice_ids = std::collections::HashSet::new();
        for voice in &self.voices {
            if !voice_ids.insert(&voice.id) {
                problems.push(format!("duplicate voice id {}", voice.id));
            }
            problems.extend(voice.model.problems(&format!("voice {} model", voice.id)));
            problems.extend(voice.config.problems(&format!("voice {} config", voice.id)));
        }

        // Exactly one, so first-run setup is never ambiguous and never
        // has to pick arbitrarily.
        match self.voices.iter().filter(|v| v.recommended).count() {
            1 => {}
            0 => problems.push("no recommended voice".to_string()),
            n => problems.push(format!("{n} recommended voices; there must be exactly one")),
        }

        if problems.is_empty() {
            Ok(())
        } else {
            Err(ManifestError::Invalid(problems.join("; ")))
        }
    }

    /// The runtime build for this machine.
    pub fn runtime_for(
        &self,
        platform: &str,
        arch: &str,
    ) -> Result<&RuntimeEntry, ManifestError> {
        let platform_supported = self.runtimes.iter().any(|r| r.platform == platform);
        self.runtimes
            .iter()
            .find(|r| r.platform == platform && r.arch == arch)
            .ok_or_else(|| {
                if platform_supported {
                    ManifestError::UnsupportedArch {
                        platform: platform.to_string(),
                        arch: arch.to_string(),
                    }
                } else {
                    ManifestError::UnsupportedPlatform {
                        platform: platform.to_string(),
                    }
                }
            })
    }

    /// The build for the machine Relay is running on.
    pub fn runtime_for_host(&self) -> Result<&RuntimeEntry, ManifestError> {
        self.runtime_for(current_platform(), current_arch())
    }

    pub fn voice(&self, id: &str) -> Result<&VoiceEntry, ManifestError> {
        self.voices
            .iter()
            .find(|v| v.id == id)
            .ok_or_else(|| ManifestError::UnknownVoice(id.to_string()))
    }

    /// The voice first-run setup installs without asking.
    pub fn recommended_voice(&self) -> Option<&VoiceEntry> {
        self.voices.iter().find(|v| v.recommended)
    }
}

/// Rejects an executable path that could land outside the engine folder.
///
/// The path is joined onto a directory Relay owns, so `..`, a leading
/// slash or a drive letter would put the "executable" somewhere else
/// entirely. A backslash is refused too: the field is documented as
/// forward-slashed, and a `\` would be one path component on Unix and
/// two on Windows, which is precisely the kind of disagreement a
/// traversal check is supposed to have no room for.
fn executable_path_problems(path: &str, label: &str) -> Vec<String> {
    let mut problems = Vec::new();
    if path.is_empty() {
        problems.push(format!("{label}: archive with no executable path"));
        return problems;
    }
    if path.contains('\\') {
        problems.push(format!("{label}: executable path must be forward-slashed"));
    }
    if path.starts_with('/') || path.split('/').any(|part| part == ".." || part.is_empty()) {
        problems.push(format!("{label}: executable path escapes the engine folder"));
    }
    // `C:\...`, and the `\\server\share` form once backslashes are ruled out.
    if path.contains(':') {
        problems.push(format!("{label}: executable path is not relative"));
    }
    problems
}

/// Requires the declared packaging to agree with the asset's own name.
///
/// This is the check that would have caught pointing Windows at
/// `piper_tts-1.7.0-cp39-abi3-win_amd64.whl`: a wheel is a zip by format
/// but a Python package by content, and nothing in it is runnable. The
/// rule is deliberately about the *declared* asset rather than the bytes,
/// because a manifest that names a wheel is wrong before anything is
/// downloaded.
fn archive_matches_asset(runtime: &RuntimeEntry, label: &str) -> Vec<String> {
    let asset = &runtime.release.asset;
    let lower = asset.to_ascii_lowercase();

    if lower.ends_with(".whl") {
        return vec![format!(
            "{label}: {asset} is a Python wheel, not a standalone engine Relay can run"
        )];
    }

    match runtime.archive {
        ArchiveKind::Raw => Vec::new(),
        kind => match ArchiveKind::from_filename(asset) {
            Some(actual) if actual == kind => Vec::new(),
            Some(actual) => vec![format!(
                "{label}: declared {kind:?} but {asset} is {actual:?}"
            )],
            None => vec![format!(
                "{label}: {asset} is not an archive Relay knows how to unpack"
            )],
        },
    }
}

/// Requires the artifact URL to be the one its release provenance implies.
///
/// Loopback is exempt, for the same reason it is exempt from the HTTPS
/// rule: the installer's end-to-end tests serve artifacts from a local
/// server, and the shipped catalogue is separately asserted to contain no
/// loopback URLs.
fn artifact_matches_release(runtime: &RuntimeEntry, label: &str) -> Vec<String> {
    let url = &runtime.artifact.url;
    if url.is_empty() || url.starts_with("http://127.0.0.1:") || url.starts_with("http://localhost:")
    {
        return Vec::new();
    }
    let mut problems = Vec::new();
    let expected = runtime.release.download_url();
    if url != &expected {
        problems.push(format!("{label}: download URL is not {expected}"));
    }
    if runtime.source != runtime.release.project_url() {
        problems.push(format!(
            "{label}: source is not the project the artifact comes from"
        ));
    }
    problems
}

/// This machine's platform, in the manifest's vocabulary.
pub fn current_platform() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

/// This machine's architecture, in the manifest's vocabulary.
pub fn current_arch() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "unsupported"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(sha: &str) -> Artifact {
        Artifact {
            url: "https://example.invalid/file".to_string(),
            sha256: sha.to_string(),
            size_bytes: 1024,
        }
    }

    fn valid_sha() -> String {
        "a".repeat(64)
    }

    fn release() -> ReleaseSource {
        ReleaseSource {
            repo: "rhasspy/piper".to_string(),
            tag: "2023.11.14-2".to_string(),
            asset: "piper_windows_amd64.zip".to_string(),
        }
    }

    fn runtime() -> RuntimeEntry {
        let release = release();
        RuntimeEntry {
            id: "piper-windows-x86_64".to_string(),
            engine: "piper".to_string(),
            version: "2023.11.14-2".to_string(),
            platform: "windows".to_string(),
            arch: "x86_64".to_string(),
            archive: ArchiveKind::Zip,
            executable_path: "piper/piper.exe".to_string(),
            artifact: Artifact {
                url: release.download_url(),
                ..artifact(&valid_sha())
            },
            source: release.project_url(),
            release,
            license: "MIT".to_string(),
        }
    }

    fn voice(id: &str, recommended: bool) -> VoiceEntry {
        VoiceEntry {
            id: id.to_string(),
            display_name: format!("English (US) — {id}"),
            language: "en_US".to_string(),
            language_label: "English (US)".to_string(),
            description: "Clear and neutral.".to_string(),
            recommended,
            model: artifact(&valid_sha()),
            config: artifact(&valid_sha()),
            license: "MIT".to_string(),
            source: "https://example.invalid".to_string(),
        }
    }

    fn manifest() -> VoiceManifest {
        VoiceManifest {
            schema_version: SCHEMA_VERSION,
            runtimes: vec![runtime()],
            voices: vec![voice("en_US-amy-medium", true)],
        }
    }

    #[test]
    fn a_well_formed_manifest_validates() {
        assert_eq!(manifest().validate(), Ok(()));
    }

    #[test]
    fn a_checksum_must_be_a_real_sha256() {
        assert!(artifact(&valid_sha()).is_pinned());
        // Every one of these has been mistaken for a checksum somewhere.
        for bad in ["", "TODO", "abc123", &"a".repeat(63), &"a".repeat(65), &"z".repeat(64)] {
            assert!(!artifact(bad).is_pinned(), "accepted {bad:?} as a checksum");
        }
    }

    #[test]
    fn an_unpinned_artifact_fails_validation() {
        // The central safety property: nothing without a checksum is ever
        // presented as installable.
        let mut broken = manifest();
        broken.runtimes[0].artifact.sha256 = String::new();
        let error = broken.validate().unwrap_err();
        assert!(error.to_string().contains("no pinned SHA-256"), "{error}");
    }

    #[test]
    fn a_voice_missing_either_checksum_fails_validation() {
        for field in 0..2 {
            let mut broken = manifest();
            if field == 0 {
                broken.voices[0].model.sha256 = "TODO".to_string();
            } else {
                broken.voices[0].config.sha256 = "TODO".to_string();
            }
            assert!(broken.validate().is_err(), "unpinned artifact accepted");
        }
    }

    #[test]
    fn a_non_https_url_fails_validation() {
        let mut broken = manifest();
        broken.runtimes[0].artifact.url = "http://example.invalid/piper.zip".to_string();
        let error = broken.validate().unwrap_err();
        assert!(error.to_string().contains("not https"), "{error}");
    }

    #[test]
    fn plain_http_is_allowed_only_for_loopback() {
        let mut artifact = artifact(&valid_sha());
        for url in [
            "http://evil.example/piper.zip",
            "http://127.0.0.1.evil.example/x",
            "http://localhost.evil.example/x",
        ] {
            artifact.url = url.to_string();
            assert!(
                !artifact.problems("x").is_empty(),
                "accepted a plain-http URL that is not loopback: {url}"
            );
        }
        for url in ["http://127.0.0.1:8080/x", "http://localhost:9000/x"] {
            artifact.url = url.to_string();
            assert!(artifact.problems("x").is_empty(), "rejected loopback: {url}");
        }
    }

    #[test]
    fn the_shipped_manifest_uses_https_for_every_download() {
        // The loopback carve-out exists for tests. Nothing Relay actually
        // ships may rely on it.
        let parsed: VoiceManifest = serde_json::from_str(MANIFEST_JSON).unwrap();
        for runtime in &parsed.runtimes {
            let url = &runtime.artifact.url;
            assert!(
                url.is_empty() || url.starts_with("https://"),
                "{} downloads over {url}",
                runtime.id
            );
        }
        for voice in &parsed.voices {
            for (label, artifact) in [("model", &voice.model), ("config", &voice.config)] {
                assert!(
                    artifact.url.starts_with("https://"),
                    "{} {label} downloads over {}",
                    voice.id,
                    artifact.url
                );
            }
        }
    }

    #[test]
    fn there_must_be_exactly_one_recommended_voice() {
        let mut none = manifest();
        none.voices[0].recommended = false;
        assert!(none.validate().unwrap_err().to_string().contains("no recommended voice"));

        let mut two = manifest();
        two.voices.push(voice("en_GB-alan-medium", true));
        assert!(two
            .validate()
            .unwrap_err()
            .to_string()
            .contains("2 recommended voices"));
    }

    #[test]
    fn duplicate_ids_fail_validation() {
        let mut duplicated = manifest();
        duplicated.voices.push(voice("en_US-amy-medium", false));
        assert!(duplicated
            .validate()
            .unwrap_err()
            .to_string()
            .contains("duplicate voice id"));
    }

    #[test]
    fn an_archive_runtime_must_say_where_the_executable_is() {
        for archive in [ArchiveKind::Zip, ArchiveKind::TarGz] {
            let mut broken = manifest();
            broken.runtimes[0].archive = archive;
            broken.runtimes[0].release.asset = match archive {
                ArchiveKind::TarGz => "piper_linux_x86_64.tar.gz".to_string(),
                _ => "piper_windows_amd64.zip".to_string(),
            };
            broken.runtimes[0].artifact.url = broken.runtimes[0].release.download_url();
            broken.runtimes[0].executable_path = String::new();
            assert!(
                broken
                    .validate()
                    .unwrap_err()
                    .to_string()
                    .contains("archive with no executable path"),
                "{archive:?} accepted without an executable path"
            );
        }
    }

    #[test]
    fn an_executable_path_may_not_escape_the_engine_folder() {
        // The path is joined onto a directory Relay owns. Every one of
        // these would put the "executable" somewhere else.
        for bad in [
            "../piper.exe",
            "piper/../../piper.exe",
            "/usr/bin/piper",
            "C:/Windows/System32/piper.exe",
            "piper\\piper.exe",
            "piper//piper.exe",
        ] {
            let mut broken = manifest();
            broken.runtimes[0].executable_path = bad.to_string();
            assert!(
                broken.validate().is_err(),
                "accepted an escaping executable path: {bad}"
            );
        }
        // And the ordinary case still passes.
        assert_eq!(manifest().validate(), Ok(()));
    }

    #[test]
    fn a_python_wheel_is_refused_as_an_engine() {
        // The bug this schema version exists for. Piper's successor
        // project publishes `piper_tts-1.7.0-cp39-abi3-win_amd64.whl` and
        // no executable at all. A wheel is a zip by format, so nothing
        // upstream of here would notice; it is a Python package by
        // content, so nothing in it can be spawned.
        let mut broken = manifest();
        broken.runtimes[0].release.asset =
            "piper_tts-1.7.0-cp39-abi3-win_amd64.whl".to_string();
        broken.runtimes[0].artifact.url = broken.runtimes[0].release.download_url();
        let error = broken.validate().unwrap_err().to_string();
        assert!(error.contains("Python wheel"), "{error}");
    }

    #[test]
    fn a_declared_archive_kind_must_match_the_asset_it_names() {
        // Renaming the expected asset to make a build pass is exactly the
        // failure mode this catches: the Linux artifact is a tarball, and
        // calling it a zip means `extract_zip` on gzip bytes.
        let mut broken = manifest();
        broken.runtimes[0].release.asset = "piper_linux_x86_64.tar.gz".to_string();
        broken.runtimes[0].artifact.url = broken.runtimes[0].release.download_url();
        let error = broken.validate().unwrap_err().to_string();
        assert!(error.contains("declared Zip"), "{error}");

        let mut unknown = manifest();
        unknown.runtimes[0].release.asset = "piper_tts-1.7.0.tar.bz2".to_string();
        unknown.runtimes[0].artifact.url = unknown.runtimes[0].release.download_url();
        assert!(unknown
            .validate()
            .unwrap_err()
            .to_string()
            .contains("not an archive Relay knows how to unpack"));
    }

    #[test]
    fn an_artifact_url_must_be_the_one_its_release_implies() {
        // Provenance is re-derived on the user's machine, so a manifest
        // that names one release and downloads from another is rejected
        // at load rather than trusted because it parsed.
        let mut broken = manifest();
        broken.runtimes[0].artifact.url =
            "https://github.com/someone-else/piper/releases/download/2023.11.14-2/piper_windows_amd64.zip"
                .to_string();
        let error = broken.validate().unwrap_err().to_string();
        assert!(error.contains("download URL is not"), "{error}");

        let mut mislabelled = manifest();
        mislabelled.runtimes[0].source = "https://github.com/OHF-Voice/piper1-gpl".to_string();
        assert!(mislabelled
            .validate()
            .unwrap_err()
            .to_string()
            .contains("source is not the project"));
    }

    #[test]
    fn a_release_must_be_pinned_to_a_plain_tag_and_asset() {
        for mutate in [
            (|r: &mut ReleaseSource| r.tag = String::new()) as fn(&mut ReleaseSource),
            |r: &mut ReleaseSource| r.repo = "piper".to_string(),
            |r: &mut ReleaseSource| r.asset = String::new(),
            |r: &mut ReleaseSource| r.tag = "../../other".to_string(),
            |r: &mut ReleaseSource| r.asset = "nested/piper.zip".to_string(),
        ] {
            let mut broken = manifest();
            mutate(&mut broken.runtimes[0].release);
            assert!(broken.validate().is_err(), "accepted a loose release pin");
        }
    }

    #[test]
    fn a_loopback_artifact_is_exempt_from_the_provenance_check() {
        // The installer's end-to-end tests serve artifacts locally. The
        // carve-out matches the one `transport_is_safe` already makes,
        // and a separate test asserts the shipped catalogue uses neither.
        let mut local = manifest();
        local.runtimes[0].artifact.url = "http://127.0.0.1:8080/engine.zip".to_string();
        assert_eq!(local.validate(), Ok(()));
    }

    #[test]
    fn the_shipped_manifest_installs_something_runnable_not_a_wheel() {
        // A regression guard on the catalogue itself, independent of
        // whether it has been provisioned yet.
        let parsed: VoiceManifest = serde_json::from_str(MANIFEST_JSON).unwrap();
        for runtime in &parsed.runtimes {
            assert!(
                !runtime.release.asset.to_ascii_lowercase().ends_with(".whl"),
                "{} points at a Python wheel",
                runtime.id
            );
            assert_eq!(
                ArchiveKind::from_filename(&runtime.release.asset),
                Some(runtime.archive),
                "{} declares {:?} but names {}",
                runtime.id,
                runtime.archive,
                runtime.release.asset
            );
            assert!(
                runtime.artifact.url.is_empty()
                    || runtime.artifact.url == runtime.release.download_url(),
                "{} downloads from somewhere other than the release it names",
                runtime.id
            );
            assert_eq!(runtime.source, runtime.release.project_url());
        }
    }

    #[test]
    fn an_unknown_engine_is_refused_rather_than_attempted() {
        let mut broken = manifest();
        broken.runtimes[0].engine = "kokoro".to_string();
        assert!(broken
            .validate()
            .unwrap_err()
            .to_string()
            .contains("cannot install"));
    }

    #[test]
    fn an_empty_manifest_is_invalid() {
        let empty = VoiceManifest {
            schema_version: SCHEMA_VERSION,
            runtimes: vec![],
            voices: vec![],
        };
        let error = empty.validate().unwrap_err().to_string();
        assert!(error.contains("no runtimes"));
        assert!(error.contains("no voices"));
    }

    #[test]
    fn a_future_schema_is_refused_rather_than_guessed_at() {
        let json = serde_json::to_string(&VoiceManifest {
            schema_version: SCHEMA_VERSION + 1,
            ..manifest()
        })
        .unwrap();
        assert!(matches!(
            VoiceManifest::parse(&json),
            Err(ManifestError::SchemaMismatch { .. })
        ));
    }

    #[test]
    fn malformed_json_is_an_error_not_a_panic() {
        assert!(matches!(
            VoiceManifest::parse("{not json"),
            Err(ManifestError::Unreadable(_))
        ));
    }

    #[test]
    fn an_unsupported_platform_is_named_in_the_error() {
        let error = manifest().runtime_for("freebsd", "x86_64").unwrap_err();
        assert!(matches!(error, ManifestError::UnsupportedPlatform { .. }));
        assert!(error.to_string().contains("freebsd"));
    }

    #[test]
    fn an_unsupported_arch_is_distinguished_from_an_unsupported_platform() {
        // "Windows on ARM" and "FreeBSD" need different messages: one is
        // a machine Relay may support later, the other is not a target.
        let error = manifest().runtime_for("windows", "aarch64").unwrap_err();
        assert!(matches!(error, ManifestError::UnsupportedArch { .. }));
        assert!(error.to_string().contains("aarch64"));
    }

    #[test]
    fn the_host_runtime_lookup_uses_real_host_values() {
        assert!(!current_platform().is_empty());
        assert!(!current_arch().is_empty());
        // On a supported host the lookup resolves; on anything else it
        // must fail cleanly rather than panic.
        let _ = manifest().runtime_for_host();
    }

    #[test]
    fn voices_are_looked_up_by_id_not_constructed() {
        let manifest = manifest();
        assert_eq!(manifest.voice("en_US-amy-medium").unwrap().id, "en_US-amy-medium");
        assert!(matches!(
            manifest.voice("../../etc/passwd"),
            Err(ManifestError::UnknownVoice(_))
        ));
    }

    #[test]
    fn filenames_come_from_the_id_not_the_url() {
        // A redirect or a mirror must not be able to change where a file
        // lands on disk.
        let voice = voice("en_US-amy-medium", true);
        assert_eq!(voice.model_filename(), "en_US-amy-medium.onnx");
        assert_eq!(voice.config_filename(), "en_US-amy-medium.onnx.json");
        assert!(!voice.model_filename().contains('/'));
        assert!(!voice.model_filename().contains('\\'));
    }

    #[test]
    fn total_bytes_covers_both_voice_files() {
        assert_eq!(voice("x", true).total_bytes(), 2048);
    }

    #[test]
    fn the_recommended_voice_is_findable() {
        assert_eq!(
            manifest().recommended_voice().map(|v| v.id.as_str()),
            Some("en_US-amy-medium")
        );
    }

    /// The catalogue Relay actually ships.
    ///
    /// It is allowed to be unprovisioned — checksums cannot be produced
    /// without downloading every artifact, which is a release step
    /// (`scripts/build-voice-manifest.mjs`). What it may never be is
    /// malformed, or provisioned *incorrectly*.
    #[test]
    fn the_shipped_manifest_parses_and_is_either_valid_or_honestly_unprovisioned() {
        let parsed: VoiceManifest =
            serde_json::from_str(MANIFEST_JSON).expect("the shipped manifest must be valid JSON");
        assert_eq!(parsed.schema_version, SCHEMA_VERSION);
        assert!(parsed.recommended_voice().is_some(), "no recommended voice");

        match parsed.validate() {
            Ok(()) => {
                // Provisioned: every artifact must then be pinned.
                for runtime in &parsed.runtimes {
                    assert!(runtime.artifact.is_pinned(), "{} unpinned", runtime.id);
                }
            }
            Err(error) => {
                // Unprovisioned is acceptable, but *only* because
                // checksums are missing — never because the file is
                // structurally wrong.
                let message = error.to_string();
                assert!(
                    message.contains("no pinned SHA-256") || message.contains("no expected size"),
                    "the shipped manifest is broken for a reason other than \
                     provisioning: {message}"
                );
            }
        }
    }

    #[test]
    fn the_shipped_manifest_names_a_licence_and_source_for_everything() {
        // Relay redistributes a download link, not the artifact, but a
        // user is still entitled to know what they are installing.
        let parsed: VoiceManifest = serde_json::from_str(MANIFEST_JSON).unwrap();
        for runtime in &parsed.runtimes {
            assert!(!runtime.license.is_empty(), "{} has no licence", runtime.id);
            assert!(runtime.source.starts_with("https://"));
        }
        for voice in &parsed.voices {
            assert!(!voice.license.is_empty(), "{} has no licence", voice.id);
            assert!(voice.source.starts_with("https://"));
        }
    }
}
