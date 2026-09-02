//! Sanitization and normalization of a browser capture payload.
//!
//! Everything arriving here is attacker-controlled: a webpage can put any
//! bytes it likes in its own DOM, and the extension forwards what it finds.
//! This module is the boundary where that stops being true. It is a total
//! function over arbitrary input — there is no payload that makes it panic,
//! and no payload that gets HTML, scripts, or a non-`http(s)` URL into a
//! stored artifact.
//!
//! It is also where "how good is this capture?" is decided and written down,
//! so the artifact can tell the user whether they got the whole page.

use super::source::{self, DetectedSource};
use super::{
    CaptureContentKind, CaptureCoverage, CaptureProvenance, CapturedLink, ContentBlock,
    WebCaptureError, WebCapturePayload,
};

/// Ceiling on any single string that survives into an artifact. Long enough
/// for a whole source file pasted into a chat message, short enough that one
/// hostile field cannot dominate a capture.
const MAX_STRING_CHARS: usize = 100_000;
const MAX_TITLE_CHARS: usize = 300;
const MAX_URL_CHARS: usize = 2_048;
const MAX_BLOCKS: usize = 5_000;
const MAX_MESSAGES: usize = 2_000;
const MAX_LIST_ITEMS: usize = 1_000;
const MAX_TABLE_ROWS: usize = 500;
const MAX_TABLE_COLUMNS: usize = 40;
const MAX_LINKS: usize = 200;
const MAX_NOTES: usize = 20;
/// Ceiling on the rendered markdown. Beyond this the artifact stops being
/// something a person or a local model can work with.
const MAX_MARKDOWN_CHARS: usize = 2_000_000;

/// The result of turning a payload into something Relay can store.
#[derive(Debug, Clone)]
pub struct NormalizedCapture {
    pub title: String,
    /// Reading-order markdown. This is what search, Talkback, and analysis see.
    pub markdown: String,
    pub provenance: CaptureProvenance,
    /// The payload with every string sanitized and every cap applied — the
    /// source-faithful record, written next to the artifact and never
    /// rewritten by analysis.
    pub structured: WebCapturePayload,
}

/// Accumulates what had to be changed on the way in, so the artifact can say so.
#[derive(Debug, Default)]
struct Sanitizer {
    truncated: bool,
    removed_control_characters: bool,
    dropped_urls: usize,
    skipped_blocks: usize,
    downgraded_diagram_blocks: usize,
}

impl Sanitizer {
    /// Strips control and bidirectional-override characters, normalizes line
    /// endings, and enforces a length cap.
    ///
    /// Bidi overrides are removed rather than kept: they let a page render
    /// text that reads as the opposite of what it stores, which in a capture
    /// archive is a forgery primitive, not formatting.
    fn text(&mut self, input: &str, max_chars: usize) -> String {
        let mut out = String::with_capacity(input.len().min(max_chars));
        let mut count = 0usize;
        let mut chars = input.chars().peekable();

        while let Some(ch) = chars.next() {
            if count >= max_chars {
                self.truncated = true;
                break;
            }

            let normalized = match ch {
                '\r' => {
                    // CRLF and lone CR both become a single LF.
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    '\n'
                }
                '\n' | '\t' => ch,
                c if is_removable_control(c) => {
                    self.removed_control_characters = true;
                    continue;
                }
                c => c,
            };

            out.push(normalized);
            count += 1;
        }

        out.trim().to_string()
    }

    fn short_text(&mut self, input: &str) -> String {
        // Titles and labels are single-line by definition; a newline in one
        // is page formatting, not content.
        self.text(input, MAX_TITLE_CHARS).replace('\n', " ")
    }

    fn optional_text(&mut self, input: Option<&String>) -> Option<String> {
        let value = self.short_text(input?);
        if value.is_empty() {
            None
        } else {
            Some(value)
        }
    }

    /// Keeps only `http`/`https` URLs. A `javascript:`, `data:`, or `file:`
    /// target from a captured page is dropped, not stored and not rendered.
    fn url(&mut self, input: &str) -> Option<String> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return None;
        }
        if trimmed.chars().count() > MAX_URL_CHARS || source::parse_url(trimmed).is_none() {
            self.dropped_urls += 1;
            return None;
        }
        // A URL containing whitespace or control characters cannot be a real
        // link target and can be used to smuggle markdown syntax.
        if trimmed.chars().any(|c| c.is_whitespace() || is_removable_control(c)) {
            self.dropped_urls += 1;
            return None;
        }
        Some(trimmed.to_string())
    }
}

/// Characters removed on the way in: C0 and C1 controls (except the tab and
/// newline handled separately), the byte-order mark, and the Unicode
/// bidirectional overrides.
fn is_removable_control(c: char) -> bool {
    matches!(c,
        '\u{0}'..='\u{8}'
        | '\u{B}'..='\u{1F}'
        | '\u{7F}'..='\u{9F}'
        | '\u{200E}' | '\u{200F}'
        | '\u{202A}'..='\u{202E}'
        | '\u{2066}'..='\u{2069}'
        | '\u{FEFF}'
    )
}

/// Maps the extension's declared strategy onto Relay's fidelity ladder.
///
/// - `structured` — a site-specific extractor recognised the page and
///   produced typed content (conversation turns, an issue, a repository).
/// - `generic` — the generic extractor found a main content region and
///   produced document blocks.
/// - `text_only` — everything else fell through to the page's visible text.
pub fn derive_fidelity(strategy: &str) -> &'static str {
    match strategy.trim().to_ascii_lowercase().as_str() {
        "site" | "conversation" | "repository" | "issue" | "structured" => "structured",
        "article" | "generic" => "generic",
        _ => "text_only",
    }
}

/// Decides what Relay calls this capture.
///
/// The URL wins when it is decisive (a GitHub pull request is a pull request
/// whatever its DOM looks like); otherwise the extracted content's own shape
/// decides; a page that is neither is just a page.
fn resolve_capture_type(detected: &DetectedSource, kind: CaptureContentKind) -> String {
    if kind == CaptureContentKind::Conversation {
        return source::CAPTURE_TYPE_CONVERSATION.to_string();
    }
    if let Some(from_url) = &detected.capture_type {
        return from_url.clone();
    }
    match kind {
        CaptureContentKind::Article => source::CAPTURE_TYPE_ARTICLE.to_string(),
        CaptureContentKind::Repository => source::CAPTURE_TYPE_REPOSITORY.to_string(),
        _ => source::CAPTURE_TYPE_PAGE.to_string(),
    }
}

/// Normalizes a role string into a stable, all-caps turn label.
///
/// Unrecognised roles are kept rather than coerced: a page that labels its
/// turns "Analyst" and "Client" is better preserved as those than flattened
/// into user/assistant.
fn role_label(role: &str) -> String {
    let lower = role.trim().to_ascii_lowercase();
    match lower.as_str() {
        "user" | "human" | "you" => "USER".to_string(),
        "assistant" | "ai" | "bot" | "model" => "ASSISTANT".to_string(),
        "system" => "SYSTEM".to_string(),
        "tool" | "function" => "TOOL".to_string(),
        "" => "PARTICIPANT".to_string(),
        other => other
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
            .take(40)
            .collect::<String>()
            .to_uppercase()
            .trim()
            .to_string(),
    }
}

/// Sanitizes one block, or drops it when there is nothing left to keep.
fn sanitize_block(s: &mut Sanitizer, block: &ContentBlock) -> Option<ContentBlock> {
    let cleaned = match block {
        ContentBlock::Heading { level, text } => {
            let text = s.short_text(text);
            if text.is_empty() {
                return None;
            }
            ContentBlock::Heading {
                level: (*level).clamp(1, 6),
                text,
            }
        }
        ContentBlock::Paragraph { text } => {
            let text = s.text(text, MAX_STRING_CHARS);
            if text.is_empty() {
                return None;
            }
            ContentBlock::Paragraph { text }
        }
        ContentBlock::List { ordered, items } => {
            let items: Vec<String> = items
                .iter()
                .take(MAX_LIST_ITEMS)
                .map(|i| s.text(i, MAX_STRING_CHARS).replace('\n', " "))
                .filter(|i| !i.is_empty())
                .collect();
            if items.is_empty() {
                return None;
            }
            if items.len() == MAX_LIST_ITEMS {
                s.truncated = true;
            }
            ContentBlock::List {
                ordered: *ordered,
                items,
            }
        }
        ContentBlock::Code { language, text } => {
            let text = s.text(text, MAX_STRING_CHARS);
            if text.is_empty() {
                return None;
            }
            ContentBlock::Code {
                language: sanitize_language(s, language.as_deref()),
                text,
            }
        }
        ContentBlock::Quote { text } => {
            let text = s.text(text, MAX_STRING_CHARS);
            if text.is_empty() {
                return None;
            }
            ContentBlock::Quote { text }
        }
        ContentBlock::Table { headers, rows } => {
            let headers: Vec<String> = headers
                .iter()
                .take(MAX_TABLE_COLUMNS)
                .map(|h| s.short_text(h))
                .collect();
            let rows: Vec<Vec<String>> = rows
                .iter()
                .take(MAX_TABLE_ROWS)
                .map(|row| {
                    row.iter()
                        .take(MAX_TABLE_COLUMNS)
                        .map(|cell| s.text(cell, MAX_TITLE_CHARS).replace('\n', " "))
                        .collect()
                })
                .filter(|row: &Vec<String>| row.iter().any(|c| !c.is_empty()))
                .collect();
            if headers.iter().all(|h| h.is_empty()) && rows.is_empty() {
                return None;
            }
            ContentBlock::Table { headers, rows }
        }
        ContentBlock::Image { alt, src } => {
            let alt = s.optional_text(alt.as_ref());
            let src = src.as_deref().and_then(|u| s.url(u));
            if alt.is_none() && src.is_none() {
                return None;
            }
            ContentBlock::Image { alt, src }
        }
        ContentBlock::Unknown => {
            s.skipped_blocks += 1;
            return None;
        }
    };
    Some(cleaned)
}

/// Restricts a code fence's language to an identifier-shaped token.
///
/// `mermaid` is deliberately downgraded: Relay's markdown view renders
/// mermaid fences to SVG and injects the result into the DOM, so a captured
/// page must never be able to reach that renderer. Captured diagram source is
/// still preserved — as text.
fn sanitize_language(s: &mut Sanitizer, language: Option<&str>) -> Option<String> {
    let raw = language?.trim().to_ascii_lowercase();
    let cleaned: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '#' | '-' | '_' | '.'))
        .take(24)
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    if cleaned == "mermaid" {
        s.downgraded_diagram_blocks += 1;
        return Some("text".to_string());
    }
    Some(cleaned)
}

fn sanitize_blocks(s: &mut Sanitizer, blocks: &[ContentBlock]) -> Vec<ContentBlock> {
    if blocks.len() > MAX_BLOCKS {
        s.truncated = true;
    }
    blocks
        .iter()
        .take(MAX_BLOCKS)
        .filter_map(|b| sanitize_block(s, b))
        .collect()
}

/// Renders one block as markdown, appending to `out`.
///
/// `heading_offset` pushes captured headings below the artifact's own H1 so a
/// page's `<h1>` cannot compete with the capture's title.
fn render_block(out: &mut String, block: &ContentBlock, heading_offset: u8) {
    match block {
        ContentBlock::Heading { level, text } => {
            let depth = (level.saturating_add(heading_offset)).clamp(2, 6) as usize;
            out.push_str(&"#".repeat(depth));
            out.push(' ');
            out.push_str(text);
            out.push_str("\n\n");
        }
        ContentBlock::Paragraph { text } => {
            out.push_str(text);
            out.push_str("\n\n");
        }
        ContentBlock::List { ordered, items } => {
            for (idx, item) in items.iter().enumerate() {
                if *ordered {
                    out.push_str(&format!("{}. {}\n", idx + 1, item));
                } else {
                    out.push_str(&format!("- {}\n", item));
                }
            }
            out.push('\n');
        }
        ContentBlock::Code { language, text } => {
            let fence = "`".repeat(longest_backtick_run(text).max(2) + 1);
            out.push_str(&fence);
            out.push_str(language.as_deref().unwrap_or(""));
            out.push('\n');
            out.push_str(text);
            if !text.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&fence);
            out.push_str("\n\n");
        }
        ContentBlock::Quote { text } => {
            for line in text.lines() {
                out.push_str("> ");
                out.push_str(line);
                out.push('\n');
            }
            out.push('\n');
        }
        ContentBlock::Table { headers, rows } => {
            let width = headers
                .len()
                .max(rows.iter().map(|r| r.len()).max().unwrap_or(0));
            if width == 0 {
                return;
            }
            let header_row: Vec<String> = (0..width)
                .map(|i| escape_cell(headers.get(i).map(String::as_str).unwrap_or("")))
                .collect();
            out.push_str(&format!("| {} |\n", header_row.join(" | ")));
            out.push_str(&format!("|{}\n", " --- |".repeat(width)));
            for row in rows {
                let cells: Vec<String> = (0..width)
                    .map(|i| escape_cell(row.get(i).map(String::as_str).unwrap_or("")))
                    .collect();
                out.push_str(&format!("| {} |\n", cells.join(" | ")));
            }
            out.push('\n');
        }
        ContentBlock::Image { alt, src } => {
            let alt = alt.as_deref().unwrap_or("image");
            match src {
                Some(src) => out.push_str(&format!("![{}]({})\n\n", escape_cell(alt), src)),
                // The source was dropped as unsafe; the caption is still content.
                None => out.push_str(&format!("*[image: {}]*\n\n", escape_cell(alt))),
            }
        }
        ContentBlock::Unknown => {}
    }
}

fn escape_cell(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ")
}

fn longest_backtick_run(text: &str) -> usize {
    let mut longest = 0;
    let mut current = 0;
    for ch in text.chars() {
        if ch == '`' {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    longest
}

fn blocks_have_text(blocks: &[ContentBlock]) -> bool {
    blocks.iter().any(|b| !matches!(b, ContentBlock::Unknown))
}

/// Sanitizes and normalizes a validated payload.
///
/// Returns [`WebCaptureError::EmptyCapture`] rather than an empty artifact: a
/// capture that saved nothing must fail loudly, because the whole point is
/// that the user can trust what is in the vault.
pub fn normalize(payload: &WebCapturePayload) -> Result<NormalizedCapture, WebCaptureError> {
    let detected =
        source::detect(&payload.url).ok_or_else(|| WebCaptureError::UnsupportedUrl(payload.url.clone()))?;

    let mut s = Sanitizer::default();

    let url = s
        .url(&payload.url)
        .ok_or_else(|| WebCaptureError::UnsupportedUrl(payload.url.clone()))?;
    let page_title = payload
        .title
        .as_deref()
        .map(|t| s.short_text(t))
        .filter(|t| !t.is_empty())
        .unwrap_or_default();

    let blocks = sanitize_blocks(&mut s, &payload.content.blocks);
    if payload.content.messages.len() > MAX_MESSAGES {
        s.truncated = true;
    }
    let messages: Vec<super::CaptureMessage> = payload
        .content
        .messages
        .iter()
        .take(MAX_MESSAGES)
        .map(|m| super::CaptureMessage {
            role: role_label(&m.role),
            blocks: sanitize_blocks(&mut s, &m.blocks),
            timestamp: s.optional_text(m.timestamp.as_ref()),
        })
        .filter(|m| blocks_have_text(&m.blocks))
        .collect();

    let links: Vec<CapturedLink> = payload
        .links
        .iter()
        .take(MAX_LINKS)
        .filter_map(|l| {
            let href = s.url(&l.href)?;
            let text = s.short_text(&l.text);
            Some(CapturedLink {
                text: if text.is_empty() { href.clone() } else { text },
                href,
            })
        })
        .collect();

    if blocks.is_empty() && messages.is_empty() {
        return Err(WebCaptureError::EmptyCapture);
    }

    let capture_type = resolve_capture_type(&detected, payload.content.kind);
    let title = derive_title(&mut s, &page_title, &blocks, &detected, &url);

    let captured_at = chrono::Utc::now().to_rfc3339();
    let markdown = render_markdown(
        &title,
        &detected,
        &capture_type,
        &url,
        &captured_at,
        &blocks,
        &messages,
        &links,
        &s,
        payload,
    );

    let mut notes: Vec<String> = payload
        .diagnostics
        .notes
        .iter()
        .take(MAX_NOTES)
        .map(|n| s.short_text(n))
        .filter(|n| !n.is_empty())
        .collect();
    append_relay_notes(&mut notes, &s, payload.diagnostics.coverage);

    let truncated = payload.diagnostics.truncated || s.truncated;
    let coverage = if truncated && payload.diagnostics.coverage == CaptureCoverage::FullDocument {
        // A capture that dropped content is not a full-document capture, no
        // matter what the extractor believed before the caps were applied.
        CaptureCoverage::Partial
    } else {
        payload.diagnostics.coverage
    };

    let provenance = CaptureProvenance {
        source_type: "web".to_string(),
        capture_type,
        application: detected.application.clone(),
        domain: detected.domain.clone(),
        url: url.clone(),
        page_title: if page_title.is_empty() {
            title.clone()
        } else {
            page_title
        },
        captured_at,
        browser_captured_at: s.optional_text(payload.captured_at.as_ref()),
        browser: s.optional_text(payload.browser.as_ref()),
        extractor_id: s.short_text(&payload.extractor.id),
        extractor_version: payload.extractor.version,
        fidelity: derive_fidelity(&payload.extractor.strategy).to_string(),
        coverage,
        notes,
        message_count: if messages.is_empty() {
            None
        } else {
            Some(messages.len())
        },
        block_count: blocks.len() + messages.iter().map(|m| m.blocks.len()).sum::<usize>(),
        skipped_block_count: s.skipped_blocks,
        truncated,
        canonical_url: payload
            .document
            .canonical_url
            .as_deref()
            .and_then(|u| s.url(u)),
        author: s.optional_text(payload.document.author.as_ref()),
        published_at: s.optional_text(payload.document.published_at.as_ref()),
        language: s.optional_text(payload.document.language.as_ref()),
        version: 1,
        previous_capture_id: None,
        recapture_count: 0,
    };

    let structured = WebCapturePayload {
        protocol_version: super::PROTOCOL_VERSION,
        captured_at: provenance.browser_captured_at.clone(),
        url,
        title: Some(provenance.page_title.clone()),
        browser: provenance.browser.clone(),
        extractor: payload.extractor.clone(),
        document: super::DocumentMetadata {
            canonical_url: provenance.canonical_url.clone(),
            site_name: s.optional_text(payload.document.site_name.as_ref()),
            author: provenance.author.clone(),
            published_at: provenance.published_at.clone(),
            description: payload
                .document
                .description
                .as_deref()
                .map(|d| s.text(d, MAX_STRING_CHARS))
                .filter(|d| !d.is_empty()),
            language: provenance.language.clone(),
        },
        content: super::CaptureContent {
            kind: payload.content.kind,
            blocks,
            messages,
        },
        links,
        diagnostics: super::CaptureDiagnostics {
            coverage,
            notes: provenance.notes.clone(),
            dom_text_length: payload.diagnostics.dom_text_length,
            truncated,
            elapsed_ms: payload.diagnostics.elapsed_ms,
        },
    };

    Ok(NormalizedCapture {
        title,
        markdown,
        provenance,
        structured,
    })
}

/// Falls back through every source of a title before giving up and naming the
/// capture after its own URL — an artifact with no name is worse than an ugly one.
fn derive_title(
    s: &mut Sanitizer,
    page_title: &str,
    blocks: &[ContentBlock],
    detected: &DetectedSource,
    url: &str,
) -> String {
    if !page_title.is_empty() {
        return page_title.to_string();
    }
    if let Some(ContentBlock::Heading { text, .. }) = blocks
        .iter()
        .find(|b| matches!(b, ContentBlock::Heading { .. }))
    {
        return text.clone();
    }
    if let Some(ContentBlock::Paragraph { text }) = blocks
        .iter()
        .find(|b| matches!(b, ContentBlock::Paragraph { .. }))
    {
        let first_line = text.lines().next().unwrap_or("");
        let snippet = s.short_text(first_line);
        if !snippet.is_empty() {
            return snippet.chars().take(80).collect();
        }
    }
    let path = source::parse_url(url).map(|p| p.path).unwrap_or_default();
    if path.len() > 1 {
        format!("{} — {}", detected.application, path.trim_matches('/'))
    } else {
        detected.application.clone()
    }
}

/// Adds Relay's own account of what happened during normalization, so the
/// artifact's notes are not limited to what the browser chose to report.
fn append_relay_notes(notes: &mut Vec<String>, s: &Sanitizer, coverage: CaptureCoverage) {
    if s.skipped_blocks > 0 {
        notes.push(format!(
            "{} content block(s) used a format this version of Relay does not understand and were skipped.",
            s.skipped_blocks
        ));
    }
    if s.truncated {
        notes.push(
            "The page was larger than Relay's capture limits, so some content was left out."
                .to_string(),
        );
    }
    if s.dropped_urls > 0 {
        notes.push(format!(
            "{} link target(s) were dropped because they were not ordinary http(s) URLs.",
            s.dropped_urls
        ));
    }
    if s.removed_control_characters {
        notes.push(
            "Invisible control or text-direction characters were removed from the captured text."
                .to_string(),
        );
    }
    if s.downgraded_diagram_blocks > 0 {
        notes.push(format!(
            "{} diagram block(s) were stored as plain text rather than as a rendered diagram.",
            s.downgraded_diagram_blocks
        ));
    }
    match coverage {
        CaptureCoverage::RenderedDom => notes.push(
            "Only the part of the page the browser had rendered was captured. Pages that load \
             content as you scroll may be incomplete."
                .to_string(),
        ),
        CaptureCoverage::Partial => notes
            .push("This capture is known to be incomplete — see the notes above.".to_string()),
        CaptureCoverage::Unknown => notes.push(
            "Relay could not tell how much of the page was captured.".to_string(),
        ),
        CaptureCoverage::FullDocument => {}
    }
}

/// Renders the artifact body.
///
/// The provenance header is part of the content on purpose: it is what makes
/// a capture findable by "that ChatGPT thread about pricing" in search and in
/// Talkback, both of which read `content` and nothing else.
#[allow(clippy::too_many_arguments)]
fn render_markdown(
    title: &str,
    detected: &DetectedSource,
    capture_type: &str,
    url: &str,
    captured_at: &str,
    blocks: &[ContentBlock],
    messages: &[super::CaptureMessage],
    links: &[CapturedLink],
    s: &Sanitizer,
    payload: &WebCapturePayload,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", title));
    out.push_str(&format!(
        "- **Source:** {} ({})\n",
        detected.application, detected.domain
    ));
    out.push_str(&format!("- **URL:** {}\n", url));
    out.push_str(&format!("- **Captured:** {}\n", captured_at));
    out.push_str(&format!("- **Type:** {}\n", capture_type));
    out.push_str(&format!(
        "- **Fidelity:** {}\n",
        derive_fidelity(&payload.extractor.strategy)
    ));
    if let Some(author) = &payload.document.author {
        if !author.trim().is_empty() {
            out.push_str(&format!("- **Author:** {}\n", author.trim()));
        }
    }
    out.push_str("\n---\n\n");

    if !messages.is_empty() {
        for message in messages {
            out.push_str(&format!("## {}\n\n", message.role));
            for block in &message.blocks {
                render_block(&mut out, block, 2);
            }
        }
    }

    for block in blocks {
        render_block(&mut out, block, 1);
    }

    if !links.is_empty() && messages.is_empty() {
        out.push_str("## Links\n\n");
        for link in links {
            out.push_str(&format!("- [{}]({})\n", escape_cell(&link.text), link.href));
        }
        out.push('\n');
    }

    if s.truncated || s.skipped_blocks > 0 {
        out.push_str(
            "\n---\n\n*Relay could not capture this page completely. See the capture's \
             provenance for what was left out.*\n",
        );
    }

    if out.chars().count() > MAX_MARKDOWN_CHARS {
        let mut truncated: String = out.chars().take(MAX_MARKDOWN_CHARS).collect();
        truncated.push_str("\n\n*[capture truncated by Relay]*\n");
        return truncated;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::web::{
        CaptureContent, CaptureDiagnostics, CaptureMessage, DocumentMetadata, ExtractorInfo,
    };

    fn payload(content: CaptureContent) -> WebCapturePayload {
        WebCapturePayload {
            protocol_version: 1,
            url: "https://example.com/post".to_string(),
            title: Some("A Post".to_string()),
            content,
            ..Default::default()
        }
    }

    fn para(text: &str) -> ContentBlock {
        ContentBlock::Paragraph {
            text: text.to_string(),
        }
    }

    #[test]
    fn renders_a_generic_page_with_a_provenance_header() {
        let p = payload(CaptureContent {
            kind: CaptureContentKind::Article,
            blocks: vec![
                ContentBlock::Heading {
                    level: 1,
                    text: "Intro".to_string(),
                },
                para("Body text."),
            ],
            messages: vec![],
        });
        let n = normalize(&p).unwrap();
        assert_eq!(n.title, "A Post");
        assert!(n.markdown.starts_with("# A Post\n"));
        assert!(n.markdown.contains("- **URL:** https://example.com/post"));
        assert!(n.markdown.contains("- **Source:** example.com (example.com)"));
        // A captured <h1> is pushed below the artifact's own title.
        assert!(n.markdown.contains("## Intro"));
        assert!(n.markdown.contains("Body text."));
        assert_eq!(n.provenance.capture_type, "article");
    }

    #[test]
    fn renders_a_conversation_as_ordered_role_labelled_turns() {
        let mut p = payload(CaptureContent {
            kind: CaptureContentKind::Conversation,
            blocks: vec![],
            messages: vec![
                CaptureMessage {
                    role: "user".to_string(),
                    blocks: vec![para("How do I capture a page?")],
                    timestamp: None,
                },
                CaptureMessage {
                    role: "assistant".to_string(),
                    blocks: vec![para("Press the shortcut.")],
                    timestamp: None,
                },
                CaptureMessage {
                    role: "human".to_string(),
                    blocks: vec![para("Thanks.")],
                    timestamp: None,
                },
            ],
        });
        p.url = "https://chatgpt.com/c/abc".to_string();
        let n = normalize(&p).unwrap();

        let user_one = n.markdown.find("## USER").unwrap();
        let assistant = n.markdown.find("## ASSISTANT").unwrap();
        let user_two = n.markdown.rfind("## USER").unwrap();
        assert!(user_one < assistant && assistant < user_two, "turn order lost");
        assert_eq!(n.provenance.capture_type, "conversation");
        assert_eq!(n.provenance.message_count, Some(3));
        assert_eq!(n.provenance.application, "ChatGPT");
    }

    #[test]
    fn keeps_unrecognised_roles_instead_of_coercing_them() {
        assert_eq!(role_label("Analyst"), "ANALYST");
        assert_eq!(role_label(""), "PARTICIPANT");
        assert_eq!(role_label("model"), "ASSISTANT");
    }

    #[test]
    fn empty_captures_fail_rather_than_saving_nothing() {
        let p = payload(CaptureContent::default());
        assert!(matches!(normalize(&p), Err(WebCaptureError::EmptyCapture)));

        let whitespace = payload(CaptureContent {
            kind: CaptureContentKind::Generic,
            blocks: vec![para("   \n  ")],
            messages: vec![],
        });
        assert!(matches!(
            normalize(&whitespace),
            Err(WebCaptureError::EmptyCapture)
        ));
    }

    #[test]
    fn strips_control_and_bidi_characters_and_says_so() {
        let p = payload(CaptureContent {
            kind: CaptureContentKind::Generic,
            blocks: vec![para("safe\u{202E}reversed\u{0}text")],
            messages: vec![],
        });
        let n = normalize(&p).unwrap();
        assert!(n.markdown.contains("safereversedtext"));
        assert!(!n.markdown.contains('\u{202E}'));
        assert!(n
            .provenance
            .notes
            .iter()
            .any(|note| note.contains("control or text-direction")));
    }

    #[test]
    fn drops_dangerous_link_and_image_targets() {
        let mut p = payload(CaptureContent {
            kind: CaptureContentKind::Generic,
            blocks: vec![
                para("body"),
                ContentBlock::Image {
                    alt: Some("logo".to_string()),
                    src: Some("javascript:alert(1)".to_string()),
                },
            ],
            messages: vec![],
        });
        p.links = vec![
            CapturedLink {
                text: "evil".to_string(),
                href: "javascript:alert(1)".to_string(),
            },
            CapturedLink {
                text: "docs".to_string(),
                href: "https://example.com/docs".to_string(),
            },
        ];
        let n = normalize(&p).unwrap();
        assert!(!n.markdown.contains("javascript:"));
        assert!(n.markdown.contains("*[image: logo]*"));
        assert!(n.markdown.contains("[docs](https://example.com/docs)"));
        assert_eq!(n.structured.links.len(), 1);
    }

    #[test]
    fn downgrades_mermaid_fences_so_captured_pages_cannot_reach_the_diagram_renderer() {
        let p = payload(CaptureContent {
            kind: CaptureContentKind::Generic,
            blocks: vec![ContentBlock::Code {
                language: Some("mermaid".to_string()),
                text: "graph TD; A-->B;".to_string(),
            }],
            messages: vec![],
        });
        let n = normalize(&p).unwrap();
        assert!(n.markdown.contains("```text"));
        assert!(!n.markdown.contains("```mermaid"));
        assert!(n.markdown.contains("graph TD; A-->B;"));
    }

    #[test]
    fn code_containing_backticks_cannot_escape_its_fence() {
        let p = payload(CaptureContent {
            kind: CaptureContentKind::Generic,
            blocks: vec![ContentBlock::Code {
                language: Some("md".to_string()),
                text: "```\nnested\n```".to_string(),
            }],
            messages: vec![],
        });
        let n = normalize(&p).unwrap();
        assert!(n.markdown.contains("````md"));
    }

    #[test]
    fn renders_tables_and_escapes_pipes() {
        let p = payload(CaptureContent {
            kind: CaptureContentKind::Generic,
            blocks: vec![ContentBlock::Table {
                headers: vec!["Key".into(), "Value".into()],
                rows: vec![vec!["a|b".into(), "c".into()], vec!["d".into()]],
            }],
            messages: vec![],
        });
        let n = normalize(&p).unwrap();
        assert!(n.markdown.contains("| Key | Value |"));
        assert!(n.markdown.contains("| a\\|b | c |"));
        // A short row is padded rather than producing a ragged table.
        assert!(n.markdown.contains("| d |  |"));
    }

    #[test]
    fn preserves_unicode_and_emoji_verbatim() {
        let p = payload(CaptureContent {
            kind: CaptureContentKind::Generic,
            blocks: vec![para("日本語のテキスト — ok ✅ Ünïcødé")],
            messages: vec![],
        });
        let n = normalize(&p).unwrap();
        assert!(n.markdown.contains("日本語のテキスト — ok ✅ Ünïcødé"));
    }

    #[test]
    fn reports_partial_coverage_when_content_had_to_be_dropped() {
        let mut p = payload(CaptureContent {
            kind: CaptureContentKind::Generic,
            blocks: (0..MAX_BLOCKS + 10).map(|i| para(&format!("p{i}"))).collect(),
            messages: vec![],
        });
        p.diagnostics = CaptureDiagnostics {
            coverage: CaptureCoverage::FullDocument,
            ..Default::default()
        };
        let n = normalize(&p).unwrap();
        assert!(n.provenance.truncated);
        assert_eq!(n.provenance.coverage, CaptureCoverage::Partial);
        assert_eq!(n.structured.content.blocks.len(), MAX_BLOCKS);
        assert!(n.markdown.contains("could not capture this page completely"));
    }

    #[test]
    fn counts_and_reports_blocks_it_does_not_understand() {
        let p = payload(CaptureContent {
            kind: CaptureContentKind::Generic,
            blocks: vec![para("kept"), ContentBlock::Unknown, ContentBlock::Unknown],
            messages: vec![],
        });
        let n = normalize(&p).unwrap();
        assert_eq!(n.provenance.skipped_block_count, 2);
        assert_eq!(n.provenance.block_count, 1);
        assert!(n.provenance.notes.iter().any(|note| note.contains("skipped")));
    }

    #[test]
    fn rendered_dom_coverage_is_always_disclosed() {
        let mut p = payload(CaptureContent {
            kind: CaptureContentKind::Conversation,
            blocks: vec![],
            messages: vec![CaptureMessage {
                role: "user".into(),
                blocks: vec![para("hi")],
                timestamp: None,
            }],
        });
        p.diagnostics.coverage = CaptureCoverage::RenderedDom;
        let n = normalize(&p).unwrap();
        assert!(n
            .provenance
            .notes
            .iter()
            .any(|note| note.contains("as you scroll")));
    }

    #[test]
    fn fidelity_follows_the_extractor_strategy() {
        assert_eq!(derive_fidelity("site"), "structured");
        assert_eq!(derive_fidelity("article"), "generic");
        assert_eq!(derive_fidelity("text"), "text_only");
        assert_eq!(derive_fidelity("something-new"), "text_only");
    }

    #[test]
    fn falls_back_through_title_sources() {
        let mut p = payload(CaptureContent {
            kind: CaptureContentKind::Generic,
            blocks: vec![ContentBlock::Heading {
                level: 1,
                text: "Heading Title".into(),
            }],
            messages: vec![],
        });
        p.title = None;
        assert_eq!(normalize(&p).unwrap().title, "Heading Title");

        let mut p2 = payload(CaptureContent {
            kind: CaptureContentKind::Generic,
            blocks: vec![para("First paragraph wins")],
            messages: vec![],
        });
        p2.title = Some("   ".to_string());
        assert_eq!(normalize(&p2).unwrap().title, "First paragraph wins");
    }

    #[test]
    fn provenance_carries_document_metadata_without_semantic_fields() {
        let mut p = payload(CaptureContent {
            kind: CaptureContentKind::Article,
            blocks: vec![para("body")],
            messages: vec![],
        });
        p.document = DocumentMetadata {
            canonical_url: Some("https://example.com/canonical".into()),
            site_name: Some("Example".into()),
            author: Some("A. Writer".into()),
            published_at: Some("2026-01-01".into()),
            description: Some("desc".into()),
            language: Some("en".into()),
        };
        p.extractor = ExtractorInfo {
            id: "generic".into(),
            version: 1,
            strategy: "article".into(),
        };
        let n = normalize(&p).unwrap();
        assert_eq!(n.provenance.author.as_deref(), Some("A. Writer"));
        assert_eq!(
            n.provenance.canonical_url.as_deref(),
            Some("https://example.com/canonical")
        );
        assert_eq!(n.provenance.fidelity, "generic");
        assert_eq!(n.provenance.extractor_id, "generic");
        assert!(n.markdown.contains("- **Author:** A. Writer"));
    }
}
