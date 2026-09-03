//! ChatGPT Export Importer.
//!
//! Parses official ChatGPT data export archives (`conversations.json` or `.zip`),
//! reconstructs conversation trees into linear chronological turns, extracts
//! attachments/assets into the capture vault directory, and produces a canonical
//! `WebCapturePayload`.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use serde::{Deserialize, Serialize};

use crate::capture::web::{
    CaptureContent, CaptureContentKind, CaptureDiagnostics, CaptureMessage,
    ContentBlock, ExtractorInfo, WebCapturePayload, PROTOCOL_VERSION,
};
use super::text_to_blocks;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatGptExportConversation {
    pub id: Option<String>,
    pub conversation_id: Option<String>,
    pub title: Option<String>,
    pub create_time: Option<f64>,
    pub update_time: Option<f64>,
    pub current_node: Option<String>,
    #[serde(default)]
    pub mapping: HashMap<String, ChatGptNode>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatGptNode {
    pub id: String,
    pub message: Option<ChatGptMessage>,
    pub parent: Option<String>,
    #[serde(default)]
    pub children: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatGptMessage {
    pub id: String,
    pub author: ChatGptAuthor,
    pub create_time: Option<f64>,
    pub update_time: Option<f64>,
    pub content: Option<ChatGptContent>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatGptAuthor {
    pub role: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatGptContent {
    pub content_type: Option<String>,
    #[serde(default)]
    pub parts: Vec<serde_json::Value>,
    pub text: Option<String>,
}

/// Parses a list of ChatGPT conversations from raw JSON bytes.
pub fn parse_chatgpt_conversations(bytes: &[u8]) -> Result<Vec<ChatGptExportConversation>, String> {
    if let Ok(list) = serde_json::from_slice::<Vec<ChatGptExportConversation>>(bytes) {
        return Ok(list);
    }
    // Single conversation fallback
    if let Ok(single) = serde_json::from_slice::<ChatGptExportConversation>(bytes) {
        return Ok(vec![single]);
    }
    Err("Failed to parse ChatGPT conversations JSON".to_string())
}

/// Reconstructs the linear chronological conversation turn sequence from a ChatGPT mapping.
pub fn linearize_chatgpt_conversation(conv: &ChatGptExportConversation) -> Vec<&ChatGptNode> {
    let mut trail = Vec::new();
    let mut visited = HashSet::new();

    // 1. If current_node is provided and exists in mapping, walk backwards via parent
    if let Some(leaf_id) = &conv.current_node {
        let mut curr = Some(leaf_id);
        while let Some(id) = curr {
            if !visited.insert(id.clone()) {
                break; // cycle guard
            }
            if let Some(node) = conv.mapping.get(id) {
                trail.push(node);
                curr = node.parent.as_ref();
            } else {
                break;
            }
        }
        if !trail.is_empty() {
            trail.reverse();
            return trail;
        }
    }

    // 2. Fallback: find roots (nodes whose parent is None or missing from mapping)
    // and follow the first/longest branch
    let mut roots: Vec<&ChatGptNode> = conv
        .mapping
        .values()
        .filter(|n| n.parent.is_none() || !conv.mapping.contains_key(n.parent.as_ref().unwrap()))
        .collect();

    roots.sort_by(|a, b| {
        let ta = a.message.as_ref().and_then(|m| m.create_time).unwrap_or(0.0);
        let tb = b.message.as_ref().and_then(|m| m.create_time).unwrap_or(0.0);
        ta.partial_cmp(&tb).unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut curr_opt = roots.first().copied();
    while let Some(node) = curr_opt {
        if !visited.insert(node.id.clone()) {
            break;
        }
        trail.push(node);
        curr_opt = node.children.first().and_then(|child_id| conv.mapping.get(child_id));
    }

    trail
}

/// Converts a linearized ChatGPT conversation into canonical `WebCapturePayload`.
pub fn chatgpt_to_capture_payload(
    conv: &ChatGptExportConversation,
    assets_dir: Option<&Path>,
    available_assets: &HashMap<String, Vec<u8>>,
) -> WebCapturePayload {
    let nodes = linearize_chatgpt_conversation(conv);
    let mut messages: Vec<CaptureMessage> = Vec::new();
    let mut ordinal: u32 = 1;

    for node in nodes {
        let Some(msg) = &node.message else { continue };

        // Skip internal/empty system prompts
        let role_raw = msg.author.role.as_str();
        if role_raw == "system" {
            let is_empty = msg
                .content
                .as_ref()
                .map(|c| c.parts.is_empty() && c.text.as_deref().unwrap_or("").is_empty())
                .unwrap_or(true);
            if is_empty {
                continue;
            }
        }

        let role = match role_raw {
            "user" => "user".to_string(),
            "assistant" => "assistant".to_string(),
            "tool" => "tool".to_string(),
            other => other.to_string(),
        };

        let mut blocks = Vec::new();

        // 1. Parse text parts
        if let Some(content) = &msg.content {
            for part in &content.parts {
                if let Some(text) = part.as_str() {
                    if !text.trim().is_empty() {
                        blocks.extend(text_to_blocks(text));
                    }
                } else if let Some(obj) = part.as_object() {
                    // Check for image / asset object in multimodal parts
                    if let Some(image_url) = obj.get("image_url").and_then(|v| v.as_str()) {
                        blocks.push(ContentBlock::Image {
                            alt: Some("Uploaded image".to_string()),
                            caption: None,
                            src: if image_url.starts_with("http") {
                                Some(image_url.to_string())
                            } else {
                                None
                            },
                            reference: Some(image_url.to_string()),
                            width: None,
                            height: None,
                            origin: Some("user_upload".to_string()),
                            content_captured: false,
                            content_note: None,
                        });
                    }
                }
            }
            if let Some(text) = &content.text {
                if !text.trim().is_empty() && blocks.is_empty() {
                    blocks.extend(text_to_blocks(text));
                }
            }
        }

        // 2. Parse attachments from metadata
        if let Some(attachments) = msg.metadata.get("attachments").and_then(|a| a.as_array()) {
            for att in attachments {
                let name = att
                    .get("name")
                    .and_then(|n| n.as_str())
                    .or_else(|| att.get("file_name").and_then(|n| n.as_str()))
                    .map(|s| s.to_string());
                let size = att.get("size").and_then(|s| s.as_u64());
                let mime = att.get("mime_type").and_then(|m| m.as_str()).map(|s| s.to_string());

                let mut saved_to_disk = false;
                if let (Some(filename), Some(dir)) = (&name, assets_dir) {
                    if let Some(bytes) = available_assets.get(filename) {
                        let asset_path = dir.join(filename);
                        if std::fs::write(&asset_path, bytes).is_ok() {
                            saved_to_disk = true;
                        }
                    }
                }

                blocks.push(ContentBlock::Attachment {
                    name,
                    mime,
                    size_bytes: size,
                    href: None,
                    reference: None,
                    kind: Some("upload".to_string()),
                    preview: None,
                    content_captured: saved_to_disk,
                    content_note: if saved_to_disk {
                        Some("Imported from export archive into local vault".to_string())
                    } else {
                        None
                    },
                });
            }
        }

        if blocks.is_empty() {
            continue;
        }

        let timestamp = msg.create_time.map(|epoch| {
            let secs = epoch.trunc() as i64;
            let nsecs = (epoch.fract() * 1_000_000_000.0) as u32;
            chrono::DateTime::from_timestamp(secs, nsecs)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| chrono::Utc::now().to_rfc3339())
        });

        messages.push(CaptureMessage {
            role,
            blocks,
            timestamp,
            ordinal: Some(ordinal),
        });

        ordinal += 1;
    }

    let conv_id = conv
        .conversation_id
        .clone()
        .or_else(|| conv.id.clone())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let title = conv
        .title
        .clone()
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| "ChatGPT Conversation".to_string());

    let captured_at = conv
        .create_time
        .and_then(|epoch| {
            chrono::DateTime::from_timestamp(epoch.trunc() as i64, 0).map(|dt| dt.to_rfc3339())
        })
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

    WebCapturePayload {
        protocol_version: PROTOCOL_VERSION,
        captured_at: Some(captured_at),
        url: format!("https://chatgpt.com/c/{}", conv_id),
        title: Some(title),
        browser: Some("chatgpt_export".to_string()),
        extractor: ExtractorInfo {
            id: "chatgpt_export".to_string(),
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
                "Imported from official ChatGPT export archive.".to_string(),
                format!("Reconstructed conversation with {} turns.", ordinal.saturating_sub(1)),
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
    fn chatgpt_linearizes_branch_correctly() {
        let json = r#"[
            {
                "id": "conv-1",
                "title": "Test Chat",
                "create_time": 1700000000.0,
                "current_node": "node-3",
                "mapping": {
                    "node-1": {
                        "id": "node-1",
                        "parent": null,
                        "children": ["node-2"],
                        "message": {
                            "id": "m-1",
                            "author": { "role": "system", "name": null },
                            "create_time": 1700000000.0,
                            "content": { "content_type": "text", "parts": [""] }
                        }
                    },
                    "node-2": {
                        "id": "node-2",
                        "parent": "node-1",
                        "children": ["node-3"],
                        "message": {
                            "id": "m-2",
                            "author": { "role": "user", "name": null },
                            "create_time": 1700000005.0,
                            "content": { "content_type": "text", "parts": ["How do we design this?"] }
                        }
                    },
                    "node-3": {
                        "id": "node-3",
                        "parent": "node-2",
                        "children": [],
                        "message": {
                            "id": "m-3",
                            "author": { "role": "assistant", "name": null },
                            "create_time": 1700000010.0,
                            "content": { "content_type": "text", "parts": ["We use a canonical model.\n\n```rust\nfn build() {}\n```"] }
                        }
                    }
                }
            }
        ]"#;

        let convs = parse_chatgpt_conversations(json.as_bytes()).unwrap();
        assert_eq!(convs.len(), 1);
        let conv = &convs[0];
        assert_eq!(conv.title.as_deref(), Some("Test Chat"));

        let payload = chatgpt_to_capture_payload(conv, None, &HashMap::new());
        assert_eq!(payload.content.messages.len(), 2);
        assert_eq!(payload.content.messages[0].role, "user");
        assert_eq!(payload.content.messages[0].ordinal, Some(1));
        assert_eq!(payload.content.messages[1].role, "assistant");
        assert_eq!(payload.content.messages[1].ordinal, Some(2));
        assert!(payload.content.messages[1].blocks.iter().any(|b| matches!(b, ContentBlock::Code { .. })));
    }
}
