//! AI Conversation Import System.
//!
//! Handles official export packages from ChatGPT and Claude (.zip or .json),
//! allows inspecting multi-conversation archives, extracts available binary assets
//! (PDFs, code, images, docs) into the Relay vault, normalizes conversations into
//! canonical `WebCapturePayload`, and triggers structured context analysis.

pub mod chatgpt;
pub mod claude;

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use serde::{Deserialize, Serialize};

use crate::capture::web::ContentBlock;
use crate::commands::CommandError;
use crate::providers::LLMClient;
use crate::vault::VaultManager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationExportItem {
    pub id: String,
    pub title: String,
    pub message_count: usize,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub has_assets: bool,
    pub asset_count: usize,
    pub already_imported_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportInspection {
    pub provider: String,
    pub provider_display: String,
    pub total_conversations: usize,
    pub conversations: Vec<ConversationExportItem>,
}

/// Converts raw text or markdown prose into structured `ContentBlock`s.
pub fn text_to_blocks(text: &str) -> Vec<ContentBlock> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        // 1. Code fence
        if line.starts_with("```") {
            let lang = line.trim_start_matches('`').trim();
            let language = if lang.is_empty() {
                None
            } else {
                Some(lang.to_string())
            };
            let mut code_lines = Vec::new();
            i += 1;
            while i < lines.len() {
                if lines[i].trim().starts_with("```") {
                    i += 1;
                    break;
                }
                code_lines.push(lines[i]);
                i += 1;
            }
            blocks.push(ContentBlock::Code {
                language,
                text: code_lines.join("\n"),
            });
            continue;
        }

        // 2. Heading
        if line.starts_with('#') {
            let level = line.chars().take_while(|c| *c == '#').count() as u8;
            let heading_text = line.trim_start_matches('#').trim().to_string();
            if !heading_text.is_empty() {
                blocks.push(ContentBlock::Heading {
                    level: level.clamp(1, 6),
                    text: heading_text,
                });
                i += 1;
                continue;
            }
        }

        // 3. Blockquote
        if line.starts_with('>') {
            let mut quote_lines = Vec::new();
            while i < lines.len() && lines[i].trim().starts_with('>') {
                quote_lines.push(lines[i].trim().trim_start_matches('>').trim());
                i += 1;
            }
            blocks.push(ContentBlock::Quote {
                text: quote_lines.join("\n"),
            });
            continue;
        }

        // 4. List item
        if line.starts_with("- ") || line.starts_with("* ") || (line.len() > 2 && line.chars().next().is_some_and(|c| c.is_ascii_digit()) && line.contains(". ")) {
            let ordered = line.chars().next().is_some_and(|c| c.is_ascii_digit());
            let mut items = Vec::new();
            while i < lines.len() {
                let l = lines[i].trim();
                if l.starts_with("- ") || l.starts_with("* ") {
                    items.push(l[2..].trim().to_string());
                    i += 1;
                } else if let Some(dot_idx) = l.find(". ") {
                    if l[..dot_idx].chars().all(|c| c.is_ascii_digit()) {
                        items.push(l[dot_idx + 2..].trim().to_string());
                        i += 1;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            blocks.push(ContentBlock::List { ordered, items });
            continue;
        }

        // 5. Paragraph
        if !line.is_empty() {
            let mut para_lines = Vec::new();
            while i < lines.len() {
                let l = lines[i].trim();
                if l.is_empty() || l.starts_with("```") || l.starts_with('#') || l.starts_with('>') || l.starts_with("- ") || l.starts_with("* ") {
                    break;
                }
                para_lines.push(lines[i]);
                i += 1;
            }
            blocks.push(ContentBlock::Paragraph {
                text: para_lines.join(" "),
            });
            continue;
        }

        i += 1;
    }

    if blocks.is_empty() && !text.trim().is_empty() {
        blocks.push(ContentBlock::Paragraph {
            text: text.trim().to_string(),
        });
    }

    blocks
}

type ExtractedArchive = (Vec<u8>, Option<HashMap<String, Vec<u8>>>);

/// Reads file bytes either from raw JSON or from a ZIP archive containing `conversations.json`.
fn extract_conversations_json(path: &Path) -> Result<ExtractedArchive, CommandError> {
    let file = File::open(path).map_err(|e| CommandError::new("FILE_READ_FAILED", &e.to_string()))?;

    // Try reading as ZIP
    if let Ok(mut archive) = zip::ZipArchive::new(file) {
        let mut conv_bytes = None;
        let mut any_json_bytes = None;
        let mut assets = HashMap::new();

        for idx in 0..archive.len() {
            let Ok(mut entry) = archive.by_index(idx) else { continue };
            let name = entry.name().to_string();

            if name.ends_with(".json") && !name.contains("__MACOSX") {
                if name == "conversations.json" || name.ends_with("/conversations.json") {
                    let mut buf = Vec::new();
                    if entry.read_to_end(&mut buf).is_ok() {
                        conv_bytes = Some(buf);
                    }
                } else if any_json_bytes.is_none() {
                    let mut buf = Vec::new();
                    if entry.read_to_end(&mut buf).is_ok() {
                        any_json_bytes = Some(buf);
                    }
                }
            } else if !entry.is_dir() && entry.size() > 0 && entry.size() < 50_000_000 {
                // Buffer assets (up to 50MB per file)
                let base_name = name.split('/').next_back().unwrap_or(&name).to_string();
                let mut buf = Vec::new();
                if entry.read_to_end(&mut buf).is_ok() {
                    assets.insert(base_name, buf);
                }
            }
        }

        if let Some(bytes) = conv_bytes.or(any_json_bytes) {
            return Ok((bytes, Some(assets)));
        }
    }

    // Fallback: direct JSON file
    let mut bytes = Vec::new();
    let mut file = File::open(path).map_err(|e| CommandError::new("FILE_READ_FAILED", &e.to_string()))?;
    file.read_to_end(&mut bytes)
        .map_err(|e| CommandError::new("FILE_READ_FAILED", &e.to_string()))?;

    Ok((bytes, None))
}

/// Inspects an export file without writing to the vault, discovering conversations and providers.
pub fn inspect_export_file(path: &Path, vault: &VaultManager) -> Result<ExportInspection, CommandError> {
    let (bytes, assets) = extract_conversations_json(path)?;

    // Fetch existing captures for duplicate matching
    let existing_captures = vault.list_captures().unwrap_or_default();
    let find_existing = |url: &str| -> Option<String> {
        existing_captures.iter().find_map(|c| {
            if c.capture.as_ref().is_some_and(|p| p.url == url) {
                Some(c.id.clone())
            } else {
                None
            }
        })
    };

    let total_assets = assets.as_ref().map(|a| a.len()).unwrap_or(0);

    // 1. Try ChatGPT
    if let Ok(chatgpt_convs) = chatgpt::parse_chatgpt_conversations(&bytes) {
        if !chatgpt_convs.is_empty() && !chatgpt_convs[0].mapping.is_empty() {
            let mut list = Vec::new();
            for c in chatgpt_convs {
                let id = c.conversation_id.clone().or_else(|| c.id.clone()).unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                let title = c.title.clone().unwrap_or_else(|| "ChatGPT Conversation".to_string());
                let msg_count = chatgpt::linearize_chatgpt_conversation(&c).len();
                let created_at = c.create_time.and_then(|epoch| {
                    chrono::DateTime::from_timestamp(epoch.trunc() as i64, 0).map(|dt| dt.to_rfc3339())
                });
                let updated_at = c.update_time.and_then(|epoch| {
                    chrono::DateTime::from_timestamp(epoch.trunc() as i64, 0).map(|dt| dt.to_rfc3339())
                });
                let url = format!("https://chatgpt.com/c/{}", id);
                let already_imported_id = find_existing(&url);

                list.push(ConversationExportItem {
                    id,
                    title,
                    message_count: msg_count,
                    created_at,
                    updated_at,
                    has_assets: total_assets > 0,
                    asset_count: total_assets,
                    already_imported_id,
                });
            }
            return Ok(ExportInspection {
                provider: "chatgpt".to_string(),
                provider_display: "ChatGPT Export".to_string(),
                total_conversations: list.len(),
                conversations: list,
            });
        }
    }

    // 2. Try Claude
    if let Ok(claude_convs) = claude::parse_claude_conversations(&bytes) {
        if !claude_convs.is_empty() {
            let mut list = Vec::new();
            for c in claude_convs {
                let id = c.uuid.clone();
                let title = c.name.clone().unwrap_or_else(|| "Claude Conversation".to_string());
                let msg_count = c.chat_messages.len();
                let url = format!("https://claude.ai/chat/{}", id);
                let already_imported_id = find_existing(&url);

                let has_embedded_content = c.chat_messages.iter().any(|m| {
                    !m.files.is_empty() || m.attachments.iter().any(|a| a.extracted_content.is_some())
                });

                list.push(ConversationExportItem {
                    id,
                    title,
                    message_count: msg_count,
                    created_at: c.created_at,
                    updated_at: c.updated_at,
                    has_assets: total_assets > 0 || has_embedded_content,
                    asset_count: total_assets,
                    already_imported_id,
                });
            }
            return Ok(ExportInspection {
                provider: "claude".to_string(),
                provider_display: "Claude Export".to_string(),
                total_conversations: list.len(),
                conversations: list,
            });
        }
    }

    Err(CommandError::new("UNRECOGNIZED_EXPORT", "Could not recognize file as a ChatGPT or Claude export archive"))
}

/// Imports a selected conversation from an export package into the Relay Vault.
pub async fn import_export_conversation(
    export_path: &Path,
    target_conversation_id: &str,
    duplicate_mode: Option<&str>,
    vault: &VaultManager,
    settings: &crate::settings::AppSettings,
) -> Result<crate::vault::VaultFile, CommandError> {
    let (bytes, assets_map) = extract_conversations_json(export_path)?;
    let assets = assets_map.unwrap_or_default();

    // 1. Try ChatGPT
    if let Ok(chatgpt_convs) = chatgpt::parse_chatgpt_conversations(&bytes) {
        if let Some(target) = chatgpt_convs.into_iter().find(|c| {
            c.conversation_id.as_deref() == Some(target_conversation_id)
                || c.id.as_deref() == Some(target_conversation_id)
        }) {
            return save_and_analyze_imported(
                target.title.as_deref().unwrap_or("ChatGPT Conversation"),
                |assets_dir| chatgpt::chatgpt_to_capture_payload(&target, Some(assets_dir), &assets),
                export_path,
                duplicate_mode,
                vault,
                settings,
            )
            .await;
        }
    }

    // 2. Try Claude
    if let Ok(claude_convs) = claude::parse_claude_conversations(&bytes) {
        if let Some(target) = claude_convs.into_iter().find(|c| c.uuid == target_conversation_id) {
            return save_and_analyze_imported(
                target.name.as_deref().unwrap_or("Claude Conversation"),
                |assets_dir| claude::claude_to_capture_payload(&target, Some(assets_dir), &assets),
                export_path,
                duplicate_mode,
                vault,
                settings,
            )
            .await;
        }
    }

    Err(CommandError::new("CONVERSATION_NOT_FOUND", "Specified conversation ID was not found in the export"))
}

async fn save_and_analyze_imported<F>(
    _raw_title: &str,
    payload_builder: F,
    _export_path: &Path,
    duplicate_mode: Option<&str>,
    vault: &VaultManager,
    settings: &crate::settings::AppSettings,
) -> Result<crate::vault::VaultFile, CommandError>
where
    F: FnOnce(&Path) -> crate::capture::web::WebCapturePayload,
{
    // Generate new capture ID
    let capture_id = format!("capture_{}", uuid::Uuid::new_v4());
    let captures_base = vault.vault_dir().join("captures").join(&capture_id);
    let assets_dir = captures_base.join("assets");
    std::fs::create_dir_all(&assets_dir)
        .map_err(|e| CommandError::new("ASSET_DIR_FAILED", &e.to_string()))?;

    // Build payload with assets extracted to assets_dir
    let mut payload = payload_builder(&assets_dir);

    // If duplicate_mode == Some("new"), assign unique URL so it doesn't merge/update previous capture
    if duplicate_mode == Some("new") {
        payload.url = format!("{}#import-{}", payload.url, uuid::Uuid::new_v4());
    }

    // Normalize
    let normalized = crate::capture::web::normalize::normalize(&payload)
        .map_err(|e| CommandError::new("NORMALIZATION_FAILED", &e.to_string()))?;

    // Save capture to vault
    let vault_file = vault
        .save_capture(normalized)
        .map_err(|e| CommandError::new("SAVE_CAPTURE_FAILED", &e.to_string()))?;

    // Run context analysis
    let llm = LLMClient::new(settings.provider.clone());
    let context = crate::capture::web::context::extract_conversation_context(
        Some(&llm),
        &vault_file.id,
        &payload,
        &vault_file.content,
    )
    .await;

    let _ = vault.save_capture_context(&vault_file.id, &context);

    Ok(vault_file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_to_blocks_handles_mixed_markdown() {
        let text = "Here is an introduction.\n\n# Architecture\n\n```rust\nfn hello() {}\n```\n\n- Point 1\n- Point 2\n\n> Important quote";
        let blocks = text_to_blocks(text);
        assert_eq!(blocks.len(), 5);
        assert!(matches!(&blocks[0], ContentBlock::Paragraph { .. }));
        assert!(matches!(&blocks[1], ContentBlock::Heading { level: 1, .. }));
        assert!(matches!(&blocks[2], ContentBlock::Code { language: Some(l), .. } if l == "rust"));
        assert!(matches!(&blocks[3], ContentBlock::List { ordered: false, items } if items.len() == 2));
        assert!(matches!(&blocks[4], ContentBlock::Quote { .. }));
    }
}
