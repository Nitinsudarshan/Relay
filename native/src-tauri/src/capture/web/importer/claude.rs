//! Claude Export Importer.
//!
//! Parses official Claude data export archives (`conversations.json` or `.zip`),
//! processes turns and attachments, preserves embedded document content, and
//! outputs a canonical `WebCapturePayload`.

use std::collections::HashMap;
use std::path::Path;
use serde::{Deserialize, Serialize};

use crate::capture::web::{
    CaptureContent, CaptureContentKind, CaptureDiagnostics, CaptureMessage,
    ContentBlock, ExtractorInfo, WebCapturePayload, PROTOCOL_VERSION,
};
use super::text_to_blocks;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClaudeExportConversation {
    pub uuid: String,
    pub name: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    #[serde(default)]
    pub chat_messages: Vec<ClaudeChatMessage>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClaudeChatMessage {
    pub uuid: Option<String>,
    #[serde(default)]
    pub text: String,
    pub sender: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    #[serde(default)]
    pub index: Option<u32>,
    #[serde(default)]
    pub files: Vec<ClaudeFile>,
    #[serde(default)]
    pub attachments: Vec<ClaudeAttachment>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClaudeFile {
    pub file_name: Option<String>,
    pub file_size: Option<u64>,
    pub file_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClaudeAttachment {
    pub file_name: Option<String>,
    pub file_size: Option<u64>,
    pub file_type: Option<String>,
    pub extracted_content: Option<String>,
}

/// Parses Claude conversations from raw JSON bytes.
pub fn parse_claude_conversations(bytes: &[u8]) -> Result<Vec<ClaudeExportConversation>, String> {
    if let Ok(list) = serde_json::from_slice::<Vec<ClaudeExportConversation>>(bytes) {
        return Ok(list);
    }
    if let Ok(single) = serde_json::from_slice::<ClaudeExportConversation>(bytes) {
        return Ok(vec![single]);
    }
    Err("Failed to parse Claude conversations JSON".to_string())
}

/// Converts a Claude conversation into canonical `WebCapturePayload`.
pub fn claude_to_capture_payload(
    conv: &ClaudeExportConversation,
    assets_dir: Option<&Path>,
    available_assets: &HashMap<String, Vec<u8>>,
) -> WebCapturePayload {
    let mut messages: Vec<CaptureMessage> = Vec::new();
    let mut ordinal: u32 = 1;

    for msg in &conv.chat_messages {
        let role = match msg.sender.as_str() {
            "human" => "user".to_string(),
            "assistant" => "assistant".to_string(),
            other => other.to_string(),
        };

        let mut blocks = Vec::new();

        // 1. Text blocks
        if !msg.text.trim().is_empty() {
            blocks.extend(text_to_blocks(&msg.text));
        }

        // 2. Attached files and documents
        for file in &msg.files {
            let name = file.file_name.clone();
            let mut saved = false;
            if let (Some(filename), Some(dir)) = (&name, assets_dir) {
                if let Some(bytes) = available_assets.get(filename) {
                    let dest = dir.join(filename);
                    if std::fs::write(&dest, bytes).is_ok() {
                        saved = true;
                    }
                }
            }

            blocks.push(ContentBlock::Attachment {
                name,
                mime: file.file_type.clone(),
                size_bytes: file.file_size,
                href: None,
                reference: None,
                kind: Some("file".to_string()),
                preview: None,
                content_captured: saved,
                content_note: if saved {
                    Some("Extracted from Claude export archive into local vault".to_string())
                } else {
                    None
                },
            });
        }

        // 3. Attachments with extracted content
        for att in &msg.attachments {
            let name = att.file_name.clone();
            let mut saved = false;

            if let (Some(filename), Some(dir)) = (&name, assets_dir) {
                if let Some(bytes) = available_assets.get(filename) {
                    let dest = dir.join(filename);
                    if std::fs::write(&dest, bytes).is_ok() {
                        saved = true;
                    }
                } else if let Some(extracted) = &att.extracted_content {
                    // Save extracted content as text file in assets if original binary isn't in zip
                    let text_filename = format!("{}.txt", filename);
                    let dest = dir.join(&text_filename);
                    if std::fs::write(&dest, extracted.as_bytes()).is_ok() {
                        saved = true;
                    }
                }
            }

            let preview = att.extracted_content.as_ref().map(|c| {
                if c.len() > 300 {
                    format!("{}…", &c[..300])
                } else {
                    c.clone()
                }
            });

            blocks.push(ContentBlock::Attachment {
                name,
                mime: att.file_type.clone(),
                size_bytes: att.file_size,
                href: None,
                reference: None,
                kind: Some("attachment".to_string()),
                preview,
                content_captured: saved,
                content_note: if saved {
                    Some("Extracted content preserved in local vault".to_string())
                } else {
                    None
                },
            });

            // If extracted content is substantial, add a blockquote block to make it immediately searchable
            if let Some(extracted) = &att.extracted_content {
                if !extracted.trim().is_empty() {
                    let snippet = if extracted.len() > 2000 {
                        format!("Attached document extract:\n\n{}…", &extracted[..2000])
                    } else {
                        format!("Attached document extract:\n\n{}", extracted)
                    };
                    blocks.push(ContentBlock::Quote { text: snippet });
                }
            }
        }

        if blocks.is_empty() {
            continue;
        }

        messages.push(CaptureMessage {
            role,
            blocks,
            timestamp: msg.created_at.clone(),
            ordinal: Some(ordinal),
        });

        ordinal += 1;
    }

    let title = conv
        .name
        .clone()
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| "Claude Conversation".to_string());

    let captured_at = conv
        .created_at
        .clone()
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

    WebCapturePayload {
        protocol_version: PROTOCOL_VERSION,
        captured_at: Some(captured_at),
        url: format!("https://claude.ai/chat/{}", conv.uuid),
        title: Some(title),
        browser: Some("claude_export".to_string()),
        extractor: ExtractorInfo {
            id: "claude_export".to_string(),
            version: 1,
            strategy: "export".to_string(),
        },
        document: crate::capture::web::DocumentMetadata::default(),
        content: CaptureContent {
            kind: CaptureContentKind::Conversation,
            blocks: Vec::new(),
            messages,
        },
        links: Vec::new(),
        diagnostics: CaptureDiagnostics {
            coverage: crate::capture::web::CaptureCoverage::FullDocument,
            notes: vec![
                "Imported from official Claude export archive.".to_string(),
                format!("Imported conversation with {} turns.", ordinal.saturating_sub(1)),
            ],
            dom_text_length: Some(0),
            truncated: false,
            elapsed_ms: Some(0),
            traversal: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_parses_turns_and_attachments() {
        let json = r#"[
            {
                "uuid": "claude-uuid-1",
                "name": "Architecture Discussion",
                "created_at": "2024-03-01T12:00:00Z",
                "chat_messages": [
                    {
                        "uuid": "msg-1",
                        "text": "Review this architecture please",
                        "sender": "human",
                        "files": [
                            {
                                "file_name": "spec.md",
                                "file_size": 512,
                                "file_type": "text/markdown"
                            }
                        ],
                        "attachments": []
                    },
                    {
                        "uuid": "msg-2",
                        "text": "Looks solid. Keep local-first.",
                        "sender": "assistant",
                        "files": [],
                        "attachments": []
                    }
                ]
            }
        ]"#;

        let convs = parse_claude_conversations(json.as_bytes()).unwrap();
        assert_eq!(convs.len(), 1);
        let conv = &convs[0];
        assert_eq!(conv.name.as_deref(), Some("Architecture Discussion"));

        let payload = claude_to_capture_payload(conv, None, &HashMap::new());
        assert_eq!(payload.content.messages.len(), 2);
        assert_eq!(payload.content.messages[0].role, "user");
        assert_eq!(payload.content.messages[0].ordinal, Some(1));
        assert_eq!(payload.content.messages[1].role, "assistant");
        assert_eq!(payload.content.messages[1].ordinal, Some(2));
        assert!(payload.content.messages[0].blocks.iter().any(|b| matches!(b, ContentBlock::Attachment { .. })));
    }
}
