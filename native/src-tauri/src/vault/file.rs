use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

use crate::capture::web::CaptureProvenance;
use crate::vault::scribble::{ScribbleAiMetadata, ScribbleRelationship};
use crate::vault::VaultError;

/// `file_type` marker for a web capture, distinguishing it from an imported
/// document without needing a second model. Text extraction never runs on
/// one: its content is produced by `capture::web::normalize`, not by reading
/// bytes back off disk.
pub const CAPTURE_FILE_TYPE: &str = "webcapture";

/// Core data model representing an imported document file in Relay's Vault.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VaultFile {
    pub id: String,
    pub original_filename: String,
    pub file_type: String,
    pub mime_type: String,
    pub size_bytes: u64,
    pub content_hash: String,
    pub created_at: String,
    pub updated_at: String,
    pub last_known_source_path: String,
    pub vault_path: String,
    pub extraction_status: String, // "extracted" | "pending" | "failed" | "unsupported"
    pub processing_status: String, // "ready" | "processing" | "failed"
    pub content: String,
    pub summary: Option<String>,
    pub tags: Vec<String>,
    pub topics: Vec<String>,
    pub entities: Vec<String>,
    pub relationships: Vec<ScribbleRelationship>,
    pub ai_metadata: ScribbleAiMetadata,
    #[serde(default)]
    pub linked_scribble_id: Option<String>,
    /// Set when this artifact came from a capture rather than a file import.
    ///
    /// Provenance only — where the content came from and how completely it
    /// was acquired. Semantic fields (`summary`, `tags`, `topics`,
    /// `entities`) are produced later by analysis and are deliberately kept
    /// out of here, so re-analysing a capture can never rewrite the record of
    /// its source. `None` on every imported file, which is what keeps this
    /// backwards compatible with vaults written before captures existed.
    #[serde(default)]
    pub capture: Option<CaptureProvenance>,
}

impl VaultFile {
    pub fn new(
        source_path: &Path,
        vault_relative_path: &str,
        size_bytes: u64,
        content_hash: String,
    ) -> Result<Self, VaultError> {
        let filename = source_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("untitled")
            .to_string();

        let ext = source_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let mime_type = match ext.as_str() {
            "pdf" => "application/pdf",
            "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "doc" => "application/msword",
            "md" | "markdown" => "text/markdown",
            "txt" => "text/plain",
            _ => "application/octet-stream",
        }
        .to_string();

        let now = chrono::Utc::now().to_rfc3339();
        let file_id = format!("file_{}", uuid::Uuid::new_v4());

        Ok(Self {
            id: file_id,
            original_filename: filename,
            file_type: ext,
            mime_type,
            size_bytes,
            content_hash,
            created_at: now.clone(),
            updated_at: now,
            last_known_source_path: source_path.to_string_lossy().to_string(),
            vault_path: vault_relative_path.to_string(),
            extraction_status: "pending".to_string(),
            processing_status: "processing".to_string(),
            content: String::new(),
            summary: None,
            tags: Vec::new(),
            topics: Vec::new(),
            entities: Vec::new(),
            relationships: Vec::new(),
            ai_metadata: ScribbleAiMetadata::default(),
            linked_scribble_id: None,
            capture: None,
        })
    }

    /// Builds a Vault artifact from a normalized web capture.
    ///
    /// `content` is already the normalized markdown, and `extraction_status`
    /// is `"extracted"` on arrival: unlike an imported PDF, a capture's text
    /// does not have to be recovered from a binary later, so there is no
    /// pending or failed extraction state to represent.
    pub fn new_capture(
        id: String,
        original_filename: String,
        vault_relative_path: String,
        content: String,
        content_hash: String,
        provenance: CaptureProvenance,
    ) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id,
            original_filename,
            file_type: CAPTURE_FILE_TYPE.to_string(),
            mime_type: "application/json".to_string(),
            size_bytes: content.len() as u64,
            content_hash,
            created_at: provenance.captured_at.clone(),
            updated_at: now,
            last_known_source_path: provenance.url.clone(),
            vault_path: vault_relative_path,
            extraction_status: "extracted".to_string(),
            processing_status: "ready".to_string(),
            content,
            summary: None,
            tags: Vec::new(),
            topics: Vec::new(),
            entities: Vec::new(),
            relationships: Vec::new(),
            ai_metadata: ScribbleAiMetadata::default(),
            linked_scribble_id: None,
            capture: Some(provenance),
        }
    }

    /// Whether this artifact is a capture rather than an imported document.
    pub fn is_capture(&self) -> bool {
        self.capture.is_some()
    }
}

/// Calculates SHA-256 hash of a file for duplicate and modification detection.
pub fn calculate_file_hash(path: &Path) -> Result<String, VaultError> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

/// Extracts text content from a supported file format.
pub fn extract_text_from_file(file_path: &Path, file_type: &str) -> Result<String, VaultError> {
    match file_type.to_lowercase().as_str() {
        "md" | "markdown" | "txt" => {
            let bytes = fs::read(file_path)?;
            // Strip UTF-8 BOM if present
            let content_str = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
                String::from_utf8_lossy(&bytes[3..]).to_string()
            } else {
                String::from_utf8_lossy(&bytes).to_string()
            };
            Ok(content_str)
        }
        "pdf" => {
            let bytes = fs::read(file_path)?;
            match pdf_extract::extract_text_from_mem(&bytes) {
                Ok(text) => {
                    let cleaned = text.trim().to_string();
                    if cleaned.is_empty() {
                        Err(VaultError::FrontmatterError(
                            "PDF contains no selectable text (scanned image or empty PDF)".to_string(),
                        ))
                    } else {
                        Ok(cleaned)
                    }
                }
                Err(e) => Err(VaultError::FrontmatterError(format!(
                    "PDF text extraction failed: {}",
                    e
                ))),
            }
        }
        "docx" => {
            let file = fs::File::open(file_path)?;
            let mut archive = ZipArchive::new(file)
                .map_err(|e| VaultError::FrontmatterError(format!("Invalid DOCX archive: {}", e)))?;

            let mut document_xml = archive
                .by_name("word/document.xml")
                .map_err(|e| VaultError::FrontmatterError(format!("DOCX missing word/document.xml: {}", e)))?;

            let mut xml_content = String::new();
            document_xml
                .read_to_string(&mut xml_content)
                .map_err(|e| VaultError::FrontmatterError(format!("Failed to read DOCX document XML: {}", e)))?;

            let extracted = extract_docx_xml_text(&xml_content);
            if extracted.trim().is_empty() {
                Err(VaultError::FrontmatterError(
                    "DOCX document contains no text".to_string(),
                ))
            } else {
                Ok(extracted)
            }
        }
        "doc" => Err(VaultError::FrontmatterError(
            "Legacy .doc format is not supported for text extraction. Please convert the file to .docx or .pdf."
                .to_string(),
        )),
        other => Err(VaultError::FrontmatterError(format!(
            "Unsupported file extension '.{}'",
            other
        ))),
    }
}

/// Helper function to parse `<w:p>` paragraphs and `<w:t>` text nodes from `word/document.xml`.
fn extract_docx_xml_text(xml_content: &str) -> String {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let mut reader = Reader::from_str(xml_content);
    reader.config_mut().trim_text(true);

    let mut result = String::new();
    let mut in_text_node = false;
    let mut current_paragraph = String::new();

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = e.name();
                if name.as_ref() == b"w:t" {
                    in_text_node = true;
                }
            }
            Ok(Event::Text(e)) => {
                if in_text_node {
                    if let Ok(t) = e.unescape() {
                        current_paragraph.push_str(&t);
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name = e.name();
                if name.as_ref() == b"w:t" {
                    in_text_node = false;
                } else if name.as_ref() == b"w:p" {
                    let trimmed = current_paragraph.trim();
                    if !trimmed.is_empty() {
                        result.push_str(trimmed);
                        result.push_str("\n\n");
                    }
                    current_paragraph.clear();
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    if !current_paragraph.trim().is_empty() {
        result.push_str(current_paragraph.trim());
        result.push_str("\n\n");
    }

    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_text_markdown_and_txt() {
        let dir = std::env::temp_dir().join(format!("file_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let md_path = dir.join("doc.md");
        fs::write(&md_path, "# Relay Architecture\n\nLocal-first knowledge engine.").unwrap();

        let extracted = extract_text_from_file(&md_path, "md").unwrap();
        assert!(extracted.contains("Relay Architecture"));

        let hash = calculate_file_hash(&md_path).unwrap();
        assert_eq!(hash.len(), 64);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_extract_text_legacy_doc_returns_error() {
        let dir = std::env::temp_dir().join(format!("file_test_doc_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let doc_path = dir.join("legacy.doc");
        fs::write(&doc_path, b"binary doc contents").unwrap();

        let res = extract_text_from_file(&doc_path, "doc");
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("Legacy .doc format is not supported"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_docx_xml_text_extraction() {
        let xml = r#"
            <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:body>
                    <w:p><w:r><w:t>Project Blueprint</w:t></w:r></w:p>
                    <w:p><w:r><w:t>Local-first secure vault.</w:t></w:r></w:p>
                </w:body>
            </w:document>
        "#;
        let extracted = extract_docx_xml_text(xml);
        assert!(extracted.contains("Project Blueprint"));
        assert!(extracted.contains("Local-first secure vault."));
    }
}
