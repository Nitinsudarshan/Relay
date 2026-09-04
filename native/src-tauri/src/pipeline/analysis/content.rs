//! The content contract analysis reads.
//!
//! # Why a contract rather than one normalizer
//!
//! Relay normalizes in three places already — `capture/web/normalize.rs`,
//! `meetings_v2/processing/normalize.rs`, and the text extraction in
//! `vault/file.rs` — and each is right about its own input. A web capture
//! knows about message roles and rendered blocks; a meeting knows about
//! speakers and timestamps; a PDF knows about neither. Collapsing them into
//! one normalizer would mean one function that understands all three, which is
//! how a normalizer becomes the thing nobody can change.
//!
//! What was missing is the other end: a shape the *analysis* layer can read
//! without knowing which normalizer produced it. That is [`CanonicalContent`].
//! Source-specific normalizers keep their internals; they agree only on this.
//!
//! # What it preserves
//!
//! Flat markdown was the previous analysis-facing surface, and flattening
//! costs real information. The expensive loss is turn ordinals: every derived
//! item in a `ConversationContext` carries `source_turn_ordinals`, and a model
//! can only cite a turn it was shown the number of. So turns survive here as
//! structure rather than as prose the model has to re-parse.

use super::source::SourceDescriptor;

/// One addressable unit of a source.
///
/// A conversation turn, a document section, a meeting utterance. The `ordinal`
/// is what evidence references point at, which is why it is not optional: a
/// segment nothing can cite is a segment analysis cannot ground a claim in.
#[derive(Debug, Clone, PartialEq)]
pub struct ContentSegment {
    /// Position in the source, 1-based. Stable across re-analysis of the same
    /// source version, because it comes from the source, not from the analysis.
    pub ordinal: u32,
    /// Who produced this segment — a role in a conversation, a speaker in a
    /// meeting, a heading path in a document. `None` where the source has no
    /// such concept.
    pub attribution: Option<String>,
    pub text: String,
}

/// A structured artifact found in the source that is worth naming separately
/// from the prose around it.
///
/// Code blocks and tables are the two that matter today: a repository's stack
/// evidence lives in fenced manifests, and flattening one into a paragraph is
/// what forced the deterministic extractor to recover filenames by scanning
/// markdown for the string `Cargo.toml`.
#[derive(Debug, Clone, PartialEq)]
pub struct ContentArtifact {
    pub kind: ArtifactKind,
    /// A filename, language tag, or table caption where the source gave one.
    pub label: Option<String>,
    pub text: String,
    /// The segment this was found in, so a claim resting on it can cite it.
    pub segment_ordinal: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactKind {
    Code,
    Table,
    Heading,
    Link,
}

/// The analysis-facing view of a source's content.
///
/// `markdown` is always populated and is what a prose analysis reads.
/// `segments` and `artifacts` are populated where the normalizer had the
/// structure to populate them, and empty — not absent, not faked — where it
/// did not. An analysis that needs turn ordinals checks `segments` and says so
/// honestly when there are none, rather than inventing citations.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CanonicalContent {
    pub title: String,
    /// Reading-order markdown. The lowest common denominator every source can
    /// produce and every prompt can consume.
    pub markdown: String,
    pub segments: Vec<ContentSegment>,
    pub artifacts: Vec<ContentArtifact>,
}

impl CanonicalContent {
    /// The minimum any source can offer: its text, with no claimed structure.
    ///
    /// Used for imported documents and for captures whose normalizer produced
    /// no addressable units. Deliberately leaves `segments` empty rather than
    /// synthesising one-per-paragraph ordinals, because a fabricated ordinal is
    /// a citation to something that does not exist.
    pub fn from_markdown(title: impl Into<String>, markdown: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            markdown: markdown.into(),
            segments: Vec::new(),
            artifacts: Vec::new(),
        }
    }

    pub fn with_segments(mut self, segments: Vec<ContentSegment>) -> Self {
        self.segments = segments;
        self
    }

    pub fn with_artifacts(mut self, artifacts: Vec<ContentArtifact>) -> Self {
        self.artifacts = artifacts;
        self
    }

    pub fn is_empty(&self) -> bool {
        self.markdown.trim().is_empty() && self.segments.is_empty()
    }

    /// Whether evidence can be cited by ordinal at all.
    ///
    /// An analysis asked to produce `source_turn_ordinals` from content with no
    /// segments should record insufficient evidence for that field rather than
    /// emitting numbers.
    pub fn supports_ordinal_citations(&self) -> bool {
        !self.segments.is_empty()
    }

    pub fn code_artifacts(&self) -> impl Iterator<Item = &ContentArtifact> {
        self.artifacts.iter().filter(|a| a.kind == ArtifactKind::Code)
    }

    /// The text an analysis prompt should be shown.
    ///
    /// When the source has addressable segments they are rendered with their
    /// ordinals, because that is the only way a model can cite them back. When
    /// it does not, the markdown is used as-is.
    pub fn as_prompt_body(&self) -> String {
        if self.segments.is_empty() {
            return self.markdown.clone();
        }

        let mut out = String::with_capacity(self.markdown.len() + self.segments.len() * 24);
        for segment in &self.segments {
            match &segment.attribution {
                Some(who) => {
                    out.push_str(&format!("[{}] {}: ", segment.ordinal, who));
                }
                None => out.push_str(&format!("[{}] ", segment.ordinal)),
            }
            out.push_str(segment.text.trim());
            out.push_str("\n\n");
        }
        out
    }

    /// A short statement of what is *not* here, for the prompt to be told.
    ///
    /// Returns `None` when the source is complete. This is the mechanism behind
    /// §29: capture already knows the capture was partial, and this is how that
    /// knowledge reaches the model instead of stopping at the UI.
    pub fn coverage_caveat(source: &SourceDescriptor<'_>) -> Option<String> {
        if !source.coverage.is_incomplete() {
            return None;
        }

        let mut caveat = String::from(
            "COVERAGE — this capture is incomplete. Relay did not necessarily see the \
             whole source. Do not state or imply that absent material does not exist; \
             report it as not available in the captured evidence.",
        );
        if !source.notes.is_empty() {
            caveat.push_str("\nWhat capture recorded about completeness:");
            for note in source.notes.iter().take(6) {
                caveat.push_str("\n- ");
                caveat.push_str(note);
            }
        }
        Some(caveat)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(ordinal: u32, who: &str, text: &str) -> ContentSegment {
        ContentSegment {
            ordinal,
            attribution: Some(who.to_string()),
            text: text.to_string(),
        }
    }

    #[test]
    fn markdown_only_content_admits_it_cannot_cite_ordinals() {
        let content = CanonicalContent::from_markdown("Report", "# Report\n\nBody text.");
        assert!(!content.supports_ordinal_citations());
        assert_eq!(content.as_prompt_body(), "# Report\n\nBody text.");
    }

    #[test]
    fn segmented_content_renders_ordinals_the_model_can_cite() {
        let content = CanonicalContent::from_markdown("Chat", "irrelevant").with_segments(vec![
            segment(1, "user", "We need offline support."),
            segment(2, "assistant", "SQLite would work."),
        ]);

        assert!(content.supports_ordinal_citations());
        let body = content.as_prompt_body();
        assert!(body.contains("[1] user: We need offline support."));
        assert!(body.contains("[2] assistant: SQLite would work."));
    }

    #[test]
    fn code_artifacts_are_addressable_without_scanning_markdown() {
        let content = CanonicalContent::from_markdown("Repo", "# repo").with_artifacts(vec![
            ContentArtifact {
                kind: ArtifactKind::Code,
                label: Some("Cargo.toml".to_string()),
                text: "[package]\nname = \"relay\"".to_string(),
                segment_ordinal: None,
            },
            ContentArtifact {
                kind: ArtifactKind::Heading,
                label: None,
                text: "Features".to_string(),
                segment_ordinal: None,
            },
        ]);

        let code: Vec<_> = content.code_artifacts().collect();
        assert_eq!(code.len(), 1);
        assert_eq!(code[0].label.as_deref(), Some("Cargo.toml"));
    }

    #[test]
    fn empty_content_is_recognised_as_empty() {
        assert!(CanonicalContent::from_markdown("t", "   ").is_empty());
        assert!(!CanonicalContent::from_markdown("t", "real").is_empty());
    }
}
