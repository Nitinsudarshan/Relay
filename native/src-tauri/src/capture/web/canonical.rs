//! The web capture normalizer's analysis-facing output.
//!
//! `normalize.rs` produces the artifact: markdown for reading, and a sanitized
//! structured payload written next to it as evidence. This module turns that
//! same payload into [`CanonicalContent`] — the shape the analysis layer reads
//! without knowing anything about web captures.
//!
//! It is a second view of one normalization, not a second normalizer. §40 is
//! explicit that source-specific normalization feeding a common contract is the
//! goal, and that four unrelated normalizers is the thing to avoid.
//!
//! # What survives that markdown lost
//!
//! Turn ordinals, chiefly. Every derived item in a `ConversationContext` cites
//! `source_turn_ordinals`, and a model can only cite a turn whose number it was
//! shown. Feeding it flat prose meant those citations were produced from
//! whatever numbering the model inferred. Code blocks survive too, labelled, so
//! repository stack evidence no longer has to be recovered by scanning markdown
//! for the string `Cargo.toml`.

use super::{CaptureContentKind, ContentBlock, WebCapturePayload};
use crate::pipeline::analysis::{ArtifactKind, CanonicalContent, ContentArtifact, ContentSegment};

/// Builds the analysis-facing content for a captured payload.
///
/// `normalized_markdown` is the artifact's own markdown, passed in rather than
/// recomputed: it is already stored on the `VaultFile` and regenerating it here
/// would be a second implementation that could disagree with the first.
pub fn to_canonical(payload: &WebCapturePayload, normalized_markdown: &str) -> CanonicalContent {
    let title = payload
        .title
        .clone()
        .unwrap_or_else(|| payload.url.clone());

    let content = CanonicalContent::from_markdown(title, normalized_markdown)
        .with_artifacts(collect_artifacts(payload));

    match payload.content.kind {
        CaptureContentKind::Conversation => content.with_segments(conversation_segments(payload)),
        _ => content,
    }
}

/// One segment per message, numbered by the page's own turn ordinal where it
/// exposed one.
///
/// Falls back to position for a message with no ordinal. That is a real
/// compromise and worth naming: a conversation the page did not number gets
/// citations that are positions in what Relay captured, not in the original
/// thread. It is still better than the alternative, which is a model inventing
/// numbers from prose — and `CaptureCoverage` already records when turns are
/// missing.
fn conversation_segments(payload: &WebCapturePayload) -> Vec<ContentSegment> {
    payload
        .content
        .messages
        .iter()
        .enumerate()
        .filter_map(|(index, message)| {
            let text = blocks_to_text(&message.blocks);
            if text.trim().is_empty() {
                return None;
            }
            Some(ContentSegment {
                ordinal: message.ordinal.unwrap_or((index + 1) as u32),
                attribution: (!message.role.trim().is_empty()).then(|| message.role.clone()),
                text,
            })
        })
        .collect()
}

/// Code, tables and headings, kept addressable rather than flattened.
///
/// Walks the top-level blocks and every message's blocks, because a repository
/// capture puts its manifests in the former and a conversation puts its code in
/// the latter.
fn collect_artifacts(payload: &WebCapturePayload) -> Vec<ContentArtifact> {
    let mut artifacts = Vec::new();

    for block in &payload.content.blocks {
        push_artifact(&mut artifacts, block, None);
    }

    for (index, message) in payload.content.messages.iter().enumerate() {
        let ordinal = message.ordinal.unwrap_or((index + 1) as u32);
        for block in &message.blocks {
            push_artifact(&mut artifacts, block, Some(ordinal));
        }
    }

    artifacts
}

fn push_artifact(
    artifacts: &mut Vec<ContentArtifact>,
    block: &ContentBlock,
    segment_ordinal: Option<u32>,
) {
    match block {
        ContentBlock::Code { language, text } => {
            if text.trim().is_empty() {
                return;
            }
            artifacts.push(ContentArtifact {
                kind: ArtifactKind::Code,
                label: language.clone().filter(|l| !l.trim().is_empty()),
                text: text.clone(),
                segment_ordinal,
            });
        }
        ContentBlock::Heading { text, .. } => {
            if text.trim().is_empty() {
                return;
            }
            artifacts.push(ContentArtifact {
                kind: ArtifactKind::Heading,
                label: None,
                text: text.clone(),
                segment_ordinal,
            });
        }
        ContentBlock::Table { headers, rows } => {
            if headers.is_empty() && rows.is_empty() {
                return;
            }
            let mut text = headers.join(" | ");
            for row in rows {
                text.push('\n');
                text.push_str(&row.join(" | "));
            }
            artifacts.push(ContentArtifact {
                kind: ArtifactKind::Table,
                label: None,
                text,
                segment_ordinal,
            });
        }
        _ => {}
    }
}

/// Renders a message's blocks back to plain text for the segment body.
///
/// Deliberately simple: the structured detail that matters to analysis is
/// already lifted out into artifacts, and what a segment needs is readable
/// prose the model can attribute to a turn.
fn blocks_to_text(blocks: &[ContentBlock]) -> String {
    let mut out = String::new();
    for block in blocks {
        let piece = match block {
            ContentBlock::Paragraph { text } | ContentBlock::Quote { text } => text.clone(),
            ContentBlock::Heading { text, .. } => text.clone(),
            ContentBlock::List { items, .. } => items
                .iter()
                .map(|i| format!("- {i}"))
                .collect::<Vec<_>>()
                .join("\n"),
            ContentBlock::Code { language, text } => match language {
                Some(lang) if !lang.trim().is_empty() => format!("```{lang}\n{text}\n```"),
                _ => format!("```\n{text}\n```"),
            },
            ContentBlock::Table { headers, rows } => {
                let mut table = headers.join(" | ");
                for row in rows {
                    table.push('\n');
                    table.push_str(&row.join(" | "));
                }
                table
            }
            _ => String::new(),
        };
        if piece.trim().is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(&piece);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::web::{CaptureContent, CaptureMessage, ExtractorInfo};

    fn payload(kind: CaptureContentKind, blocks: Vec<ContentBlock>, messages: Vec<CaptureMessage>) -> WebCapturePayload {
        WebCapturePayload {
            protocol_version: 1,
            captured_at: Some("2026-09-03T10:00:00Z".to_string()),
            url: "https://github.com/owner/repo".to_string(),
            title: Some("owner/repo".to_string()),
            browser: None,
            extractor: ExtractorInfo {
                id: "github".to_string(),
                version: 1,
                strategy: "site".to_string(),
            },
            document: Default::default(),
            content: CaptureContent {
                kind,
                blocks,
                messages,
            },
            links: Vec::new(),
            diagnostics: Default::default(),
        }
    }

    fn message(role: &str, ordinal: Option<u32>, text: &str) -> CaptureMessage {
        CaptureMessage {
            role: role.to_string(),
            blocks: vec![ContentBlock::Paragraph {
                text: text.to_string(),
            }],
            timestamp: None,
            ordinal,
        }
    }

    /// The information flat markdown destroyed. Without ordinals reaching the
    /// model, `source_turn_ordinals` in a stored context are numbers the model
    /// made up.
    #[test]
    fn conversation_turns_keep_the_pages_own_ordinals() {
        let p = payload(
            CaptureContentKind::Conversation,
            vec![],
            vec![
                message("user", Some(3), "We need offline support."),
                message("assistant", Some(4), "SQLite would work."),
            ],
        );

        let content = to_canonical(&p, "# ignored");
        assert!(content.supports_ordinal_citations());
        assert_eq!(content.segments.len(), 2);
        assert_eq!(content.segments[0].ordinal, 3);
        assert_eq!(content.segments[1].ordinal, 4);
        assert_eq!(content.segments[0].attribution.as_deref(), Some("user"));

        let body = content.as_prompt_body();
        assert!(body.contains("[3] user: We need offline support."));
        assert!(body.contains("[4] assistant: SQLite would work."));
    }

    #[test]
    fn messages_without_page_ordinals_fall_back_to_position() {
        let p = payload(
            CaptureContentKind::Conversation,
            vec![],
            vec![message("user", None, "First"), message("assistant", None, "Second")],
        );
        let content = to_canonical(&p, "# ignored");
        assert_eq!(content.segments[0].ordinal, 1);
        assert_eq!(content.segments[1].ordinal, 2);
    }

    /// A repository capture is not a conversation, so it has no citable turns —
    /// and must say so rather than presenting section numbers as turn numbers.
    #[test]
    fn a_repository_capture_has_artifacts_but_no_turn_citations() {
        let p = payload(
            CaptureContentKind::Article,
            vec![
                ContentBlock::Heading {
                    level: 1,
                    text: "owner/repo".to_string(),
                },
                ContentBlock::Code {
                    language: Some("toml".to_string()),
                    text: "[package]\nname = \"relay\"".to_string(),
                },
            ],
            vec![],
        );

        let content = to_canonical(&p, "# owner/repo");
        assert!(!content.supports_ordinal_citations());

        let code: Vec<_> = content.code_artifacts().collect();
        assert_eq!(code.len(), 1);
        assert_eq!(code[0].label.as_deref(), Some("toml"));
        assert!(code[0].text.contains("name = \"relay\""));
    }

    #[test]
    fn empty_messages_are_dropped_rather_than_numbered() {
        let p = payload(
            CaptureContentKind::Conversation,
            vec![],
            vec![
                message("user", Some(1), "Real content"),
                message("assistant", Some(2), "   "),
            ],
        );
        let content = to_canonical(&p, "# ignored");
        assert_eq!(content.segments.len(), 1, "a blank turn is not a citable turn");
        assert_eq!(content.segments[0].ordinal, 1);
    }
}
