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
pub const SCHEMA_VERSION: u32 = 1;

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
    /// The download *is* the executable.
    Raw,
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
            if runtime.archive == ArchiveKind::Zip && runtime.executable_path.is_empty() {
                problems.push(format!("runtime {}: zip with no executable path", runtime.id));
            }
            problems.extend(runtime.artifact.problems(&format!("runtime {}", runtime.id)));
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

    fn runtime() -> RuntimeEntry {
        RuntimeEntry {
            id: "piper-1.6.0-windows-x86_64".to_string(),
            engine: "piper".to_string(),
            version: "1.6.0".to_string(),
            platform: "windows".to_string(),
            arch: "x86_64".to_string(),
            archive: ArchiveKind::Zip,
            executable_path: "piper/piper.exe".to_string(),
            artifact: artifact(&valid_sha()),
            license: "GPL-3.0-only".to_string(),
            source: "https://github.com/OHF-Voice/piper1-gpl".to_string(),
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
    fn a_zip_runtime_must_say_where_the_executable_is() {
        let mut broken = manifest();
        broken.runtimes[0].executable_path = String::new();
        assert!(broken
            .validate()
            .unwrap_err()
            .to_string()
            .contains("zip with no executable path"));
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
