//! What a source *is*, for the purpose of analysing it.
//!
//! # Why this is a view and not a record
//!
//! Relay already persists sources. A [`VaultFile`] is the artifact for an
//! imported document and for a web capture alike, `CaptureProvenance` records
//! where a captured one came from, and the meetings pipeline keeps its own
//! session records. Adding a fourth persisted source model would create a
//! second source of truth about the same files, and the migration would be the
//! whole task.
//!
//! So [`SourceDescriptor`] borrows. It is built from an artifact at analysis
//! time, answers the one question the analysis layer needs — *what am I
//! looking at, and how much of it does Relay actually have?* — and is dropped
//! afterwards. Nothing is written, nothing can drift, and a source that
//! predates this module describes itself correctly the first time it is read.
//!
//! # Identity
//!
//! A source's id is the artifact's id. That is what makes re-analysis produce
//! a new derived artifact rather than a new source, and it is why recapture
//! semantics (`version`, `previous_capture_id`) are untouched here: they
//! belong to the source, and analysis has no business changing them.

use crate::capture::web::source as web_source;
use crate::capture::web::CaptureProvenance;
use crate::vault::VaultFile;

/// The kind of thing a source is, in the vocabulary analysis reasons about.
///
/// Deliberately coarse. The fine distinctions that matter to one source type —
/// which chat host, which document format — live in [`SourceSubtype`] and in
/// the provenance itself. What this decides is which analyses are meaningful
/// at all: a repository has a stack, a conversation has turns, and neither
/// question makes sense of the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceType {
    Conversation,
    Repository,
    Document,
    WebPage,
    Meeting,
    Audio,
    /// A vault artifact whose kind Relay cannot establish. Not an error — an
    /// honest answer, and the reason analysis selection has a default rather
    /// than a panic.
    Unknown,
}

impl SourceType {
    /// The stable wire name, used in derived-data records and prompt
    /// applicability rules. Changing one of these is a data migration.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Conversation => "conversation",
            Self::Repository => "repository",
            Self::Document => "document",
            Self::WebPage => "web_page",
            Self::Meeting => "meeting",
            Self::Audio => "audio",
            Self::Unknown => "unknown",
        }
    }
}

/// The narrower classification within a [`SourceType`], where one exists.
///
/// This is what keeps a GitHub issue from being analysed as though it were the
/// repository. `capture/web/source.rs` already derives these from the URL at
/// capture time and stores the answer on the artifact; this type is that
/// answer, given a name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceSubtype {
    Repository,
    Issue,
    PullRequest,
    Discussion,
    Code,
    Article,
    Page,
    None,
}

impl SourceSubtype {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Repository => "repository",
            Self::Issue => "issue",
            Self::PullRequest => "pull_request",
            Self::Discussion => "discussion",
            Self::Code => "code",
            Self::Article => "article",
            Self::Page => "page",
            Self::None => "none",
        }
    }
}

/// How completely Relay holds the source, carried into analysis so a prompt
/// can be told what it is not looking at.
///
/// Mirrors `CaptureCoverage` rather than replacing it — §29's requirement is
/// that the context layer must not undo capture's honesty about completeness,
/// and it cannot respect a value it never receives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceCoverage {
    Complete,
    Partial,
    Unknown,
}

impl SourceCoverage {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Unknown => "unknown",
        }
    }

    /// Whether analysis must qualify what it says about absent material.
    pub fn is_incomplete(&self) -> bool {
        !matches!(self, Self::Complete)
    }
}

/// What downstream systems may do with the source's content.
///
/// Only two values, because only two matter: content Relay acquired from the
/// outside world is never an instruction, and content the user wrote or
/// dictated is theirs. This is the flag that decides whether
/// `source_boundary::wrap_external_source` is applied, so it is part of the
/// source contract rather than a per-caller decision that one caller can
/// forget to make.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceTrust {
    /// Captured or imported from outside Relay. Framed as data, always.
    ExternalUntrusted,
    /// Authored by the user inside Relay.
    UserAuthored,
}

impl SourceTrust {
    pub fn is_external(&self) -> bool {
        matches!(self, Self::ExternalUntrusted)
    }
}

/// The analysis layer's view of one source.
///
/// Lifetimes rather than owned strings: this is constructed from an artifact
/// the caller already holds, immediately before an analysis, and copying a
/// document's title and origin to describe it would be pure waste.
#[derive(Debug, Clone)]
pub struct SourceDescriptor<'a> {
    /// The artifact id. Derived data references this, and re-analysis reuses
    /// it rather than minting a new one.
    pub id: &'a str,
    pub source_type: SourceType,
    pub subtype: SourceSubtype,
    pub title: &'a str,
    /// Where the source came from, in words: an application name, or the
    /// originating filename for an import.
    pub origin: &'a str,
    /// The canonical address of the source, when it has one. A capture's URL;
    /// `None` for a document that only ever existed as a file.
    pub canonical_location: Option<&'a str>,
    pub captured_at: &'a str,
    pub trust: SourceTrust,
    pub coverage: SourceCoverage,
    /// Plain-language statements about what was and was not acquired, carried
    /// straight from capture so analysis can be told the same truths the UI is.
    pub notes: &'a [String],
}

impl<'a> SourceDescriptor<'a> {
    /// Describes a vault artifact — a capture or an imported document.
    ///
    /// The classification comes from `capture_type`, which
    /// `capture/web/source.rs` derived from the URL at capture time after
    /// stripping userinfo and ports. Re-deriving it here from the URL string
    /// would be both duplicated work and strictly worse: a substring test
    /// against a host name is exactly what that module exists to avoid.
    pub fn from_vault_file(file: &'a VaultFile) -> Self {
        match file.capture.as_ref() {
            Some(provenance) => Self::from_capture(file, provenance),
            None => Self {
                id: &file.id,
                source_type: SourceType::Document,
                subtype: SourceSubtype::None,
                title: &file.original_filename,
                origin: &file.original_filename,
                canonical_location: None,
                captured_at: &file.created_at,
                // An imported document is a file the user chose to bring in.
                // It is theirs, and the boundary is not applied to it.
                trust: SourceTrust::UserAuthored,
                coverage: match file.extraction_status.as_str() {
                    "extracted" => SourceCoverage::Complete,
                    "pending" | "failed" | "unsupported" => SourceCoverage::Partial,
                    _ => SourceCoverage::Unknown,
                },
                notes: &[],
            },
        }
    }

    fn from_capture(file: &'a VaultFile, provenance: &'a CaptureProvenance) -> Self {
        let (source_type, subtype) = classify_capture(&provenance.capture_type);
        Self {
            id: &file.id,
            source_type,
            subtype,
            title: &provenance.page_title,
            origin: &provenance.application,
            canonical_location: Some(&provenance.url),
            captured_at: &provenance.captured_at,
            // Every web capture, from every domain. A recognisable host is not
            // authority — see `pipeline::source_boundary`.
            trust: SourceTrust::ExternalUntrusted,
            coverage: coverage_from_capture(&provenance.coverage),
            notes: &provenance.notes,
        }
    }

    /// A one-line description of the origin, for the source-boundary frame and
    /// for provenance on derived data.
    pub fn describe_origin(&self) -> String {
        match self.canonical_location {
            Some(url) => format!(
                "Captured {} from {} ({})",
                self.subtype.as_str(),
                self.origin,
                url
            ),
            None => format!("Imported {} \"{}\"", self.source_type.as_str(), self.origin),
        }
    }
}

/// Maps capture's stored classification onto the analysis vocabulary.
///
/// The interesting rows are the GitHub ones. An issue, a pull request and a
/// discussion are all *about* a repository without *being* one, and analysing
/// them with the repository profile would produce a stack and a feature list
/// for a bug report. They are conversations — threaded, authored, with a
/// position in time — which is the shape the conversation profile already
/// handles.
fn classify_capture(capture_type: &str) -> (SourceType, SourceSubtype) {
    match capture_type {
        web_source::CAPTURE_TYPE_CONVERSATION => {
            (SourceType::Conversation, SourceSubtype::None)
        }
        web_source::CAPTURE_TYPE_REPOSITORY => (SourceType::Repository, SourceSubtype::Repository),
        web_source::CAPTURE_TYPE_ISSUE => (SourceType::Conversation, SourceSubtype::Issue),
        web_source::CAPTURE_TYPE_PULL_REQUEST => {
            (SourceType::Conversation, SourceSubtype::PullRequest)
        }
        web_source::CAPTURE_TYPE_DISCUSSION => {
            (SourceType::Conversation, SourceSubtype::Discussion)
        }
        web_source::CAPTURE_TYPE_CODE => (SourceType::Document, SourceSubtype::Code),
        web_source::CAPTURE_TYPE_ARTICLE => (SourceType::WebPage, SourceSubtype::Article),
        web_source::CAPTURE_TYPE_PAGE => (SourceType::WebPage, SourceSubtype::Page),
        _ => (SourceType::WebPage, SourceSubtype::None),
    }
}

fn coverage_from_capture(coverage: &crate::capture::web::CaptureCoverage) -> SourceCoverage {
    use crate::capture::web::CaptureCoverage;
    match coverage {
        // `FullDocument` is the only value carrying positive evidence that the
        // whole source was seen. `RenderedDom` is the *normal* case for a
        // virtualized conversation and still means "only what was on screen",
        // so it is incomplete — the same reading `docs/capture.md` §6 and the
        // capture UI already take.
        CaptureCoverage::FullDocument => SourceCoverage::Complete,
        CaptureCoverage::RenderedDom | CaptureCoverage::Partial | CaptureCoverage::Failed => {
            SourceCoverage::Partial
        }
        CaptureCoverage::Unknown => SourceCoverage::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::web::{CaptureCoverage, CaptureProvenance};

    fn provenance(capture_type: &str, url: &str) -> CaptureProvenance {
        CaptureProvenance {
            source_type: "web".to_string(),
            capture_type: capture_type.to_string(),
            application: "GitHub".to_string(),
            domain: "github.com".to_string(),
            url: url.to_string(),
            page_title: "owner/repo".to_string(),
            captured_at: "2026-09-03T10:00:00Z".to_string(),
            browser_captured_at: None,
            browser: None,
            extractor_id: "github".to_string(),
            extractor_version: 1,
            trust: "external_untrusted".to_string(),
            fidelity: "structured".to_string(),
            coverage: CaptureCoverage::FullDocument,
            notes: Vec::new(),
            message_count: None,
            block_count: 4,
            skipped_block_count: 0,
            truncated: false,
            canonical_url: None,
            author: None,
            published_at: None,
            language: None,
            version: 1,
            previous_capture_id: None,
            recapture_count: 0,
            traversal: None,
        }
    }

    fn capture_file(capture_type: &str, url: &str) -> VaultFile {
        VaultFile::new_capture(
            "cap_1".to_string(),
            "capture.json".to_string(),
            "captures/cap_1".to_string(),
            "# owner/repo".to_string(),
            "hash".to_string(),
            provenance(capture_type, url),
        )
    }

    #[test]
    fn a_repository_capture_describes_itself_as_a_repository() {
        let file = capture_file("repository", "https://github.com/owner/repo");
        let source = SourceDescriptor::from_vault_file(&file);
        assert_eq!(source.source_type, SourceType::Repository);
        assert_eq!(source.subtype, SourceSubtype::Repository);
        assert_eq!(source.id, "cap_1");
    }

    /// The distinction the substring router could not make. An issue page lives
    /// on github.com and is not a repository; analysing it for a tech stack
    /// would invent one.
    #[test]
    fn github_issues_and_pull_requests_are_conversations_not_repositories() {
        for (capture_type, expected_subtype) in [
            ("issue", SourceSubtype::Issue),
            ("pull_request", SourceSubtype::PullRequest),
            ("discussion", SourceSubtype::Discussion),
        ] {
            let file = capture_file(capture_type, "https://github.com/owner/repo/issues/42");
            let source = SourceDescriptor::from_vault_file(&file);
            assert_eq!(
                source.source_type,
                SourceType::Conversation,
                "{capture_type} should analyse as a conversation"
            );
            assert_eq!(source.subtype, expected_subtype);
        }
    }

    /// A URL that merely *contains* a host name is not a capture from that
    /// host. Classification comes from what capture recorded, so a page that
    /// mentions github.com in a query string cannot borrow its identity.
    #[test]
    fn a_url_mentioning_github_does_not_make_a_page_a_repository() {
        let file = capture_file("page", "https://evil.example/?ref=github.com");
        let source = SourceDescriptor::from_vault_file(&file);
        assert_eq!(source.source_type, SourceType::WebPage);
        assert_ne!(source.source_type, SourceType::Repository);
    }

    #[test]
    fn a_conversation_capture_is_external_and_an_import_is_not() {
        let capture = capture_file("conversation", "https://chatgpt.com/c/abc");
        assert!(SourceDescriptor::from_vault_file(&capture).trust.is_external());

        let mut document = capture.clone();
        document.capture = None;
        document.extraction_status = "extracted".to_string();
        let source = SourceDescriptor::from_vault_file(&document);
        assert_eq!(source.source_type, SourceType::Document);
        assert!(!source.trust.is_external());
    }

    #[test]
    fn partial_capture_coverage_survives_into_the_source_contract() {
        let mut file = capture_file("repository", "https://github.com/owner/repo");
        if let Some(p) = file.capture.as_mut() {
            p.coverage = CaptureCoverage::Partial;
        }
        let source = SourceDescriptor::from_vault_file(&file);
        assert_eq!(source.coverage, SourceCoverage::Partial);
        assert!(source.coverage.is_incomplete());
    }
}
