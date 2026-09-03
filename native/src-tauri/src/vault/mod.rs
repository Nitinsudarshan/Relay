use crate::sync::MutexExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use thiserror::Error;

/// Note type for Relay's universal dictation history — every successful
/// transcript, from any capture mode, regardless of whether text injection
/// also happened for it. Distinct from the LLM-cleaned "scribble"/"meeting"
/// note types, which remain unchanged.
pub const VOICE_NOTE_TYPE: &str = "voice_note";

/// Imported documents. One directory per artifact, holding the untouched
/// original plus `metadata.json`.
pub const FILES_DIR: &str = "files";

/// Web captures. Same `VaultFile` model and the same directory shape as
/// `files/`, kept in their own tree so that the Files surface, its dedupe
/// rules, and its text-extraction path stay exactly as they were.
pub const CAPTURES_DIR: &str = "captures";

pub mod scribble;
pub use scribble::*;

pub mod trash;
pub use trash::*;

pub mod file;
pub use file::*;

#[derive(Error, Debug)]
pub enum VaultError {
    #[error("Vault IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Frontmatter serialization error: {0}")]
    FrontmatterError(String),

    #[error("Note not found: {0}")]
    NotFound(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VaultNote {
    pub id: String,
    pub title: String,
    pub note_type: String,
    pub created_at: String,
    pub updated_at: String,
    pub tags: Vec<String>,
    pub source_audio: Option<String>,
    pub content: String,
    pub merged_from: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MergeRecord {
    pub id: String,
    pub merged_note_id: String,
    pub merged_at: String,
    pub primary_source: VaultNote,
    pub secondary_source: VaultNote,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnmergeResult {
    pub primary: VaultNote,
    pub secondary: VaultNote,
}

impl VaultNote {
    /// Builds a Voice Note from a raw transcript. Callers must already have
    /// guarded against an empty/whitespace-only transcript — this always
    /// creates a note. Voice Notes carry the verbatim transcript as
    /// `content` (unlike "scribble" notes, which store an LLM-cleaned
    /// rewrite) since they exist to be a truthful dictation history.
    pub fn new_voice_note(transcript: &str) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        // The hand-rolled frontmatter below embeds `title` in a single
        // quoted line (`title: "..."`, no escaping) — fine for the fixed,
        // developer-authored titles "scribble"/"meeting" notes use, but
        // this is the first note type whose title comes from arbitrary
        // speech, so a literal `"` or newline in the excerpt must not be
        // allowed to corrupt that line.
        let title: String = transcript
            .chars()
            .take(60)
            .map(|c| {
                if c == '"' || c == '\n' || c == '\r' {
                    ' '
                } else {
                    c
                }
            })
            .collect();
        Self {
            id: format!("note_{}", uuid::Uuid::new_v4()),
            title,
            note_type: VOICE_NOTE_TYPE.to_string(),
            created_at: now.clone(),
            updated_at: now,
            tags: Vec::new(),
            source_audio: None,
            content: transcript.to_string(),
            merged_from: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KanbanCard {
    pub id: String,
    pub title: String,
    pub assignee: String,
    pub status: String, // "todo", "in_progress", "done"
    pub priority: String,
    pub due_date: Option<String>,
    pub created_at: String,
    pub description: String,
    pub source_note_id: Option<String>,
}

pub struct VaultManager {
    // A `Mutex` (rather than a plain `PathBuf`) so the vault root can be
    // repointed at runtime — e.g. when the user picks a folder via the
    // Voice Note first-time setup, or changes Vault Directory Location in
    // Settings — without requiring an app restart. Every method already
    // only needed `&self`, so this is the only change needed to make that
    // safe; no call site elsewhere has to change.
    vault_dir: Mutex<PathBuf>,
}

impl VaultManager {
    pub fn new(vault_dir: PathBuf) -> Self {
        Self {
            vault_dir: Mutex::new(vault_dir),
        }
    }

    pub fn vault_dir(&self) -> PathBuf {
        self.vault_dir.lock_or_recover().clone()
    }

    /// Repoints this vault at a new root directory. Existing notes at the
    /// old location are left in place untouched — nothing is moved,
    /// migrated, or deleted (see docs/decisions.md).
    pub fn set_vault_dir(&self, new_dir: PathBuf) {
        *self.vault_dir.lock_or_recover() = new_dir;
    }

    pub fn init(&self) -> Result<(), VaultError> {
        let dir = self.vault_dir();
        fs::create_dir_all(dir.join("notes"))?;
        fs::create_dir_all(dir.join("kanban"))?;
        fs::create_dir_all(dir.join("scribbles"))?;
        fs::create_dir_all(dir.join("trash"))?;
        fs::create_dir_all(dir.join("merged_sources"))?;
        fs::create_dir_all(dir.join(FILES_DIR))?;
        fs::create_dir_all(dir.join(CAPTURES_DIR))?;
        Ok(())
    }

    pub fn save_note(&self, note: &VaultNote) -> Result<PathBuf, VaultError> {
        self.init()?;
        let file_path = self
            .vault_dir()
            .join("notes")
            .join(format!("{}.md", note.id));
        let merged_from_str = match &note.merged_from {
            Some(ids) => format!("{:?}", ids),
            None => "None".to_string(),
        };
        let frontmatter = format!(
            "---\nid: \"{}\"\ntitle: \"{}\"\ntype: \"{}\"\ncreated_at: \"{}\"\nupdated_at: \"{}\"\ntags: {:?}\nsource_audio: {:?}\nmerged_from: {}\n---\n\n{}",
            note.id, note.title, note.note_type, note.created_at, note.updated_at, note.tags, note.source_audio, merged_from_str, note.content
        );

        fs::write(&file_path, frontmatter)?;
        tracing::info!("Saved vault note to {:?}", file_path);
        Ok(file_path)
    }

    pub fn save_kanban_card(&self, card: &KanbanCard) -> Result<PathBuf, VaultError> {
        self.init()?;
        let file_path = self
            .vault_dir()
            .join("kanban")
            .join(format!("{}.md", card.id));
        let due_date_str = card.due_date.as_deref().unwrap_or("");
        let source_id_str = card.source_note_id.as_deref().unwrap_or("");

        let frontmatter = format!(
            "---\nid: \"{}\"\ntitle: \"{}\"\nassignee: \"{}\"\nstatus: \"{}\"\npriority: \"{}\"\ndue_date: \"{}\"\ncreated_at: \"{}\"\nsource_note_id: \"{}\"\n---\n\n{}",
            card.id, card.title, card.assignee, card.status, card.priority, due_date_str, card.created_at, source_id_str, card.description
        );

        fs::write(&file_path, frontmatter)?;
        tracing::info!("Saved Kanban card to {:?}", file_path);
        Ok(file_path)
    }

    pub fn get_note(&self, id: &str) -> Result<VaultNote, VaultError> {
        self.init()?;
        let file_path = self.vault_dir().join("notes").join(format!("{}.md", id));
        if !file_path.exists() {
            return Err(VaultError::NotFound(id.to_string()));
        }
        let content = fs::read_to_string(&file_path)?;
        Self::parse_note_md(&content)
            .ok_or_else(|| VaultError::FrontmatterError(format!("Failed to parse note {}", id)))
    }

    pub fn delete_note(&self, id: &str) -> Result<(), VaultError> {
        self.init()?;
        let file_path = self.vault_dir().join("notes").join(format!("{}.md", id));
        if file_path.exists() {
            fs::remove_file(&file_path)?;
            tracing::info!("Deleted vault note {:?}", file_path);
        }
        Ok(())
    }

    pub fn update_note_content(&self, id: &str, new_content: &str) -> Result<VaultNote, VaultError> {
        let mut note = self.get_note(id)?;
        note.content = new_content.to_string();
        note.updated_at = chrono::Utc::now().to_rfc3339();
        note.title = new_content
            .chars()
            .take(60)
            .map(|c| {
                if c == '"' || c == '\n' || c == '\r' {
                    ' '
                } else {
                    c
                }
            })
            .collect();
        self.save_note(&note)?;
        Ok(note)
    }

    pub fn merge_notes(&self, primary_id: &str, secondary_id: &str) -> Result<VaultNote, VaultError> {
        self.init()?;
        let note1 = self.get_note(primary_id)?;
        let note2 = self.get_note(secondary_id)?;

        // Chronological order: older note content first, newer note content second
        let (older, newer) = if note1.created_at <= note2.created_at {
            (&note1, &note2)
        } else {
            (&note2, &note1)
        };

        let merged_content = format!("{}\n\n{}", older.content.trim(), newer.content.trim());

        // Create pre-merge snapshot record
        let record = MergeRecord {
            id: format!("merge_{}", uuid::Uuid::new_v4()),
            merged_note_id: primary_id.to_string(),
            merged_at: chrono::Utc::now().to_rfc3339(),
            primary_source: note1.clone(),
            secondary_source: note2.clone(),
        };

        // Push record onto stack in merged_sources/{primary_id}.json
        let merged_sources_dir = self.vault_dir().join("merged_sources");
        fs::create_dir_all(&merged_sources_dir)?;
        let stack_path = merged_sources_dir.join(format!("{}.json", primary_id));

        let mut stack: Vec<MergeRecord> = if stack_path.exists() {
            let data = fs::read_to_string(&stack_path)?;
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Vec::new()
        };
        stack.push(record);
        fs::write(
            &stack_path,
            serde_json::to_string_pretty(&stack)
                .map_err(|e| VaultError::FrontmatterError(e.to_string()))?,
        )?;

        // Determine aggregated merged_from component note IDs
        let mut merged_from_ids = Vec::new();
        if let Some(ref ids) = note1.merged_from {
            merged_from_ids.extend(ids.clone());
        } else {
            merged_from_ids.push(note1.id.clone());
        }
        if let Some(ref ids) = note2.merged_from {
            merged_from_ids.extend(ids.clone());
        } else {
            merged_from_ids.push(note2.id.clone());
        }

        let mut merged_note = note1;
        merged_note.content = merged_content;
        merged_note.updated_at = chrono::Utc::now().to_rfc3339();
        merged_note.title = crate::pipeline::extract_deterministic_title(&merged_note.content);
        merged_note.merged_from = Some(merged_from_ids);

        self.save_note(&merged_note)?;
        self.delete_note(secondary_id)?;
        Ok(merged_note)
    }

    pub fn unmerge_notes(&self, merged_note_id: &str) -> Result<UnmergeResult, VaultError> {
        self.init()?;
        let merged_note = self.get_note(merged_note_id)?;

        if merged_note.merged_from.is_none() {
            return Err(VaultError::FrontmatterError(format!(
                "Note {} is not a merged note",
                merged_note_id
            )));
        }

        let stack_path = self
            .vault_dir()
            .join("merged_sources")
            .join(format!("{}.json", merged_note_id));
        if !stack_path.exists() {
            return Err(VaultError::NotFound(format!(
                "Merge history for note {} not found",
                merged_note_id
            )));
        }

        let data = fs::read_to_string(&stack_path)?;
        let mut stack: Vec<MergeRecord> = serde_json::from_str(&data)
            .map_err(|e| VaultError::FrontmatterError(e.to_string()))?;

        if stack.is_empty() {
            return Err(VaultError::NotFound(format!(
                "Merge stack for note {} is empty",
                merged_note_id
            )));
        }

        let record = stack.pop().unwrap();
        let primary_restored = record.primary_source;
        let secondary_restored = record.secondary_source;

        // Atomically save both restored original notes
        self.save_note(&primary_restored)?;
        self.save_note(&secondary_restored)?;

        // Update or remove stack file
        if stack.is_empty() {
            let _ = fs::remove_file(&stack_path);
        } else {
            fs::write(
                &stack_path,
                serde_json::to_string_pretty(&stack)
                    .map_err(|e| VaultError::FrontmatterError(e.to_string()))?,
            )?;
        }

        let _ = self.sync_scribbles_for_voice_note_unmerge(&primary_restored.id, &secondary_restored.id);

        Ok(UnmergeResult {
            primary: primary_restored,
            secondary: secondary_restored,
        })
    }

    pub fn list_notes(&self) -> Result<Vec<VaultNote>, VaultError> {
        self.init()?;
        let notes_dir = self.vault_dir().join("notes");
        let mut notes = Vec::new();

        if !notes_dir.exists() {
            return Ok(notes);
        }

        for entry in fs::read_dir(notes_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "md") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Some(note) = Self::parse_note_md(&content) {
                        notes.push(note);
                    }
                }
            }
        }

        Ok(notes)
    }

    /// Returns only notes of the given `note_type` (e.g. [`VOICE_NOTE_TYPE`]),
    /// newest first. `created_at` is an RFC3339 string with a fixed-width,
    /// fixed-offset format (see [`VaultNote::new_voice_note`]), so a plain
    /// string comparison already sorts chronologically.
    pub fn list_notes_by_type(&self, note_type: &str) -> Result<Vec<VaultNote>, VaultError> {
        let mut notes: Vec<VaultNote> = self
            .list_notes()?
            .into_iter()
            .filter(|n| n.note_type == note_type)
            .collect();
        notes.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(notes)
    }

    /// Ranks vault notes against `query` by simple term-overlap scoring and
    /// returns the top `top_k`.
    ///
    /// This stands in for the embedded LanceDB vector search decided in
    /// `docs/decisions.md` (Decision 6) — a real embedding pipeline is
    /// tracked as backlog rather than implemented here, but this keeps voice
    /// chat's grounding-in-your-own-notes behavior real (not mocked) in the
    /// meantime.
    pub fn search_notes(&self, query: &str, top_k: usize) -> Result<Vec<VaultNote>, VaultError> {
        let notes = self.list_notes()?;
        let query_terms = tokenize(query);
        if query_terms.is_empty() || notes.is_empty() {
            return Ok(Vec::new());
        }

        let mut scored: Vec<(usize, VaultNote)> = notes
            .into_iter()
            .map(|note| {
                let haystack = format!("{} {}", note.title, note.content).to_lowercase();
                let score = query_terms
                    .iter()
                    .filter(|t| haystack.contains(t.as_str()))
                    .count();
                (score, note)
            })
            .filter(|(score, _)| *score > 0)
            .collect();

        scored.sort_by_key(|a| std::cmp::Reverse(a.0));
        Ok(scored
            .into_iter()
            .take(top_k)
            .map(|(_, note)| note)
            .collect())
    }

    fn parse_note_md(content: &str) -> Option<VaultNote> {
        let parts: Vec<&str> = content.splitn(3, "---").collect();
        if parts.len() < 3 {
            return None;
        }

        let frontmatter = parts[1];
        let body = parts[2].trim_start_matches('\n').to_string();

        let mut id = String::new();
        let mut title = String::new();
        let mut note_type = "note".to_string();
        let mut created_at = String::new();
        let mut updated_at = String::new();
        let mut tags = Vec::new();
        let mut source_audio = None;
        let mut merged_from = None;

        for line in frontmatter.lines() {
            let line = line.trim();
            if let Some(v) = line.strip_prefix("id:") {
                id = v.trim().trim_matches('"').to_string();
            } else if let Some(v) = line.strip_prefix("title:") {
                title = v.trim().trim_matches('"').to_string();
            } else if let Some(v) = line.strip_prefix("type:") {
                note_type = v.trim().trim_matches('"').to_string();
            } else if let Some(v) = line.strip_prefix("created_at:") {
                created_at = v.trim().trim_matches('"').to_string();
            } else if let Some(v) = line.strip_prefix("updated_at:") {
                updated_at = v.trim().trim_matches('"').to_string();
            } else if let Some(v) = line.strip_prefix("tags:") {
                tags = parse_debug_string_list(v.trim());
            } else if let Some(v) = line.strip_prefix("source_audio:") {
                let v = v.trim();
                source_audio = if v == "None" {
                    None
                } else {
                    Some(
                        v.trim_start_matches("Some(")
                            .trim_end_matches(')')
                            .trim_matches('"')
                            .to_string(),
                    )
                };
            } else if let Some(v) = line.strip_prefix("merged_from:") {
                let v = v.trim();
                merged_from = if v == "None" || v.is_empty() {
                    None
                } else {
                    Some(parse_debug_string_list(v))
                };
            }
        }

        if id.is_empty() || title.is_empty() {
            return None;
        }

        Some(VaultNote {
            id,
            title,
            note_type,
            created_at,
            updated_at,
            tags,
            source_audio,
            content: body,
            merged_from,
        })
    }

    pub fn list_kanban_cards(&self) -> Result<Vec<KanbanCard>, VaultError> {
        self.init()?;
        let kanban_dir = self.vault_dir().join("kanban");
        let mut cards = Vec::new();

        if !kanban_dir.exists() {
            return Ok(cards);
        }

        for entry in fs::read_dir(kanban_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "md") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Some(card) = Self::parse_kanban_card_md(&content) {
                        cards.push(card);
                    }
                }
            }
        }

        Ok(cards)
    }

    fn parse_kanban_card_md(content: &str) -> Option<KanbanCard> {
        let parts: Vec<&str> = content.split("---").collect();
        if parts.len() < 3 {
            return None;
        }

        let frontmatter = parts[1];
        let description = parts[2..].join("---").trim().to_string();

        let mut id = String::new();
        let mut title = String::new();
        let mut assignee = String::new();
        let mut status = "todo".to_string();
        let mut priority = "medium".to_string();
        let mut due_date = None;
        let mut created_at = String::new();
        let mut source_note_id = None;

        /// Frontmatter values are quoted; the value is what's left after the
        /// key, unquoted and trimmed.
        fn value_of(line: &str, key: &str) -> Option<String> {
            line.strip_prefix(key)
                .map(|rest| rest.trim().trim_matches('"').to_string())
        }

        for line in frontmatter.lines() {
            let line = line.trim();
            if let Some(v) = value_of(line, "id:") {
                id = v;
            } else if let Some(v) = value_of(line, "title:") {
                title = v;
            } else if let Some(v) = value_of(line, "assignee:") {
                assignee = v;
            } else if let Some(v) = value_of(line, "status:") {
                status = v;
            } else if let Some(v) = value_of(line, "priority:") {
                priority = v;
            } else if let Some(v) = value_of(line, "due_date:") {
                if !v.is_empty() {
                    due_date = Some(v);
                }
            } else if let Some(v) = value_of(line, "created_at:") {
                created_at = v;
            } else if let Some(v) = value_of(line, "source_note_id:") {
                if !v.is_empty() {
                    source_note_id = Some(v);
                }
            }
        }

        if id.is_empty() || title.is_empty() {
            return None;
        }

        Some(KanbanCard {
            id,
            title,
            assignee,
            status,
            priority,
            due_date,
            created_at,
            description,
            source_note_id,
        })
    }

    pub fn save_scribble(&self, scribble: &Scribble) -> Result<PathBuf, VaultError> {
        self.init()?;
        let file_path = self
            .vault_dir()
            .join("scribbles")
            .join(format!("{}.md", scribble.id));
        let content = scribble.format_markdown();
        fs::write(&file_path, content)?;
        tracing::info!("Saved scribble note to {:?}", file_path);
        Ok(file_path)
    }

    pub fn get_scribble(&self, id: &str) -> Result<Scribble, VaultError> {
        self.init()?;
        let file_path = self
            .vault_dir()
            .join("scribbles")
            .join(format!("{}.md", id));
        if !file_path.exists() {
            return Err(VaultError::NotFound(id.to_string()));
        }
        let content = fs::read_to_string(&file_path)?;
        let mut scribble = Scribble::parse_markdown(&content)
            .ok_or_else(|| VaultError::FrontmatterError(format!("Failed to parse scribble {}", id)))?;

        let scribbles_dir = self.vault_dir().join("scribbles");
        scribble.relationships.retain(|r| {
            scribbles_dir.join(format!("{}.md", r.target_id)).exists()
        });
        Ok(scribble)
    }

    pub fn list_scribbles(&self) -> Result<Vec<Scribble>, VaultError> {
        self.init()?;
        let scribbles_dir = self.vault_dir().join("scribbles");
        let mut scribbles = Vec::new();

        if !scribbles_dir.exists() {
            return Ok(scribbles);
        }

        for entry in fs::read_dir(&scribbles_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "md") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Some(scribble) = Scribble::parse_markdown(&content) {
                        scribbles.push(scribble);
                    }
                }
            }
        }

        // Clean any dangling relationships pointing to merged, deleted, or trashed scribbles
        let valid_ids: std::collections::HashSet<String> = scribbles.iter().map(|s| s.id.clone()).collect();
        for s in &mut scribbles {
            s.relationships.retain(|r| valid_ids.contains(&r.target_id));
        }

        // Sort newest updated / created first
        scribbles.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(scribbles)
    }

    pub fn update_scribble(&self, scribble: &Scribble) -> Result<Scribble, VaultError> {
        let mut updated = scribble.clone();
        updated.updated_at = chrono::Utc::now().to_rfc3339();
        self.save_scribble(&updated)?;
        Ok(updated)
    }

    pub fn delete_scribble(&self, id: &str) -> Result<(), VaultError> {
        self.init()?;
        let file_path = self
            .vault_dir()
            .join("scribbles")
            .join(format!("{}.md", id));
        if file_path.exists() {
            fs::remove_file(&file_path)?;
            tracing::info!("Deleted scribble file {:?}", file_path);
        }
        Ok(())
    }

    pub fn import_vault_file(&self, source_path: &std::path::Path) -> Result<VaultFile, VaultError> {
        self.init()?;

        if !source_path.exists() || !source_path.is_file() {
            return Err(VaultError::NotFound(format!(
                "Source file not found or not a valid file: {:?}",
                source_path
            )));
        }

        let size_bytes = source_path.metadata()?.len();
        let content_hash = calculate_file_hash(source_path)?;

        // Check if file already exists in vault by hash
        if let Ok(existing_files) = self.list_vault_files() {
            if let Some(existing) = existing_files.into_iter().find(|f| f.content_hash == content_hash) {
                return Ok(existing);
            }
        }

        let filename = source_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("untitled")
            .to_string();

        let file_id = format!("file_{}", uuid::Uuid::new_v4());
        let vault_file_dir = self.vault_dir().join("files").join(&file_id);
        let original_dir = vault_file_dir.join("original");
        fs::create_dir_all(&original_dir)?;

        let dest_file_path = original_dir.join(&filename);
        // Copy the file to Vault — ORIGINAL FILE IS 100% UNTOUCHED!
        fs::copy(source_path, &dest_file_path)?;

        let vault_relative_path = format!("files/{}/original/{}", file_id, filename);
        let mut vault_file = VaultFile::new(source_path, &vault_relative_path, size_bytes, content_hash)?;
        vault_file.id = file_id;

        // Attempt text extraction
        match extract_text_from_file(&dest_file_path, &vault_file.file_type) {
            Ok(text) => {
                vault_file.content = text.clone();
                vault_file.extraction_status = "extracted".to_string();
                vault_file.processing_status = "ready".to_string();

                // Run deterministic knowledge extraction for initial fallback tags & summary
                let knowledge = crate::pipeline::extract_deterministic_knowledge(&text);
                vault_file.summary = knowledge.summary;
                vault_file.topics = knowledge.topics;
                vault_file.entities = knowledge.entities;
                vault_file.tags = vault_file.topics.clone();
            }
            Err(e) => {
                tracing::warn!("Text extraction for imported file {} failed/unsupported: {}", filename, e);
                vault_file.extraction_status = if vault_file.file_type == "doc" {
                    "unsupported".to_string()
                } else {
                    "failed".to_string()
                };
                vault_file.processing_status = "ready".to_string();
            }
        }

        self.save_vault_file(&vault_file)?;
        Ok(vault_file)
    }

    pub fn import_vault_file_bytes(
        &self,
        filename: &str,
        bytes: &[u8],
        source_path: Option<&str>,
    ) -> Result<VaultFile, VaultError> {
        self.init()?;

        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let content_hash = format!("{:x}", hasher.finalize());

        // Check if file already exists in vault by hash
        if let Ok(existing_files) = self.list_vault_files() {
            if let Some(existing) = existing_files.into_iter().find(|f| f.content_hash == content_hash) {
                return Ok(existing);
            }
        }

        let clean_filename = std::path::Path::new(filename)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(filename);

        let file_id = format!("file_{}", uuid::Uuid::new_v4());
        let vault_file_dir = self.vault_dir().join("files").join(&file_id);
        let original_dir = vault_file_dir.join("original");
        fs::create_dir_all(&original_dir)?;

        let dest_file_path = original_dir.join(clean_filename);
        fs::write(&dest_file_path, bytes)?;

        let vault_relative_path = format!("files/{}/original/{}", file_id, clean_filename);
        let dummy_path = std::path::PathBuf::from(source_path.unwrap_or(clean_filename));
        let mut vault_file = VaultFile::new(&dummy_path, &vault_relative_path, bytes.len() as u64, content_hash)?;
        vault_file.id = file_id;
        vault_file.original_filename = clean_filename.to_string();

        match extract_text_from_file(&dest_file_path, &vault_file.file_type) {
            Ok(text) => {
                vault_file.content = text.clone();
                vault_file.extraction_status = "extracted".to_string();
                vault_file.processing_status = "ready".to_string();

                let knowledge = crate::pipeline::extract_deterministic_knowledge(&text);
                vault_file.summary = knowledge.summary;
                vault_file.topics = knowledge.topics;
                vault_file.entities = knowledge.entities;
                vault_file.tags = vault_file.topics.clone();
            }
            Err(e) => {
                tracing::warn!("Text extraction for file {} failed/unsupported: {}", clean_filename, e);
                vault_file.extraction_status = if vault_file.file_type == "doc" {
                    "unsupported".to_string()
                } else {
                    "failed".to_string()
                };
                vault_file.processing_status = "ready".to_string();
            }
        }

        self.save_vault_file(&vault_file)?;
        Ok(vault_file)
    }

    /// Persists a normalized web capture as a Vault artifact.
    ///
    /// Acquisition is finished before interpretation begins: the raw
    /// structured payload and the normalized markdown are both on disk when
    /// this returns, and analysis is a separate call that is allowed to fail
    /// without costing the user the capture.
    ///
    /// Re-capture follows the convention `import_vault_file` already set —
    /// identical content is not duplicated. Identical content from the same
    /// URL bumps a counter on the existing artifact; *changed* content from
    /// the same URL becomes a new artifact that points back at the one it
    /// supersedes, because a page that changed is new information, not a
    /// duplicate.
    pub fn save_capture(
        &self,
        normalized: crate::capture::web::normalize::NormalizedCapture,
    ) -> Result<VaultFile, VaultError> {
        self.init()?;

        // Identity is the captured *content*, not the rendered artifact: the
        // markdown embeds Relay's own capture timestamp, so hashing it would
        // make every re-capture look like a change.
        let content_hash = capture_content_hash(&normalized);

        // `list_captures` is newest-first, so this finds the most recent
        // capture of the same URL — the one a new capture supersedes.
        let previous = self
            .list_captures()?
            .into_iter()
            .find(|c| {
                c.capture
                    .as_ref()
                    .is_some_and(|p| p.url == normalized.provenance.url)
            });

        if let Some(mut existing) = previous.clone() {
            if existing.content_hash == content_hash {
                if let Some(capture) = existing.capture.as_mut() {
                    capture.recapture_count = capture.recapture_count.saturating_add(1);
                }
                existing.updated_at = chrono::Utc::now().to_rfc3339();
                self.save_vault_file(&existing)?;
                tracing::info!(
                    "Capture of {} is unchanged; recorded a re-capture of {}",
                    normalized.provenance.url,
                    existing.id
                );
                return Ok(existing);
            }
        }

        let id = format!("capture_{}", uuid::Uuid::new_v4());
        let filename = capture_payload_filename(&normalized.title);
        let original_dir = self.vault_dir().join(CAPTURES_DIR).join(&id).join("original");
        fs::create_dir_all(&original_dir)?;

        let payload_json = serde_json::to_string_pretty(&normalized.structured)
            .map_err(|e| VaultError::FrontmatterError(e.to_string()))?;
        fs::write(original_dir.join(&filename), payload_json.as_bytes())?;

        let mut provenance = normalized.provenance;
        if let Some(prev) = previous {
            provenance.version = prev
                .capture
                .as_ref()
                .map(|p| p.version.saturating_add(1))
                .unwrap_or(1);
            provenance.previous_capture_id = Some(prev.id);
        }

        let vault_relative_path = format!("{}/{}/original/{}", CAPTURES_DIR, id, filename);
        let mut artifact = VaultFile::new_capture(
            id,
            filename,
            vault_relative_path,
            normalized.markdown,
            content_hash,
            provenance,
        );
        artifact.original_filename = normalized.title.clone();

        // Deterministic knowledge first, exactly as file import does, so a
        // capture is searchable and tagged even if no LLM is configured or
        // the later analysis pass fails.
        let knowledge = crate::pipeline::extract_deterministic_knowledge(&artifact.content);
        artifact.summary = knowledge.summary;
        artifact.topics = knowledge.topics;
        artifact.entities = knowledge.entities;
        artifact.tags = artifact.topics.clone();

        self.save_vault_file(&artifact)?;
        tracing::info!(
            "Saved capture {} from {} ({} chars)",
            artifact.id,
            artifact
                .capture
                .as_ref()
                .map(|c| c.url.as_str())
                .unwrap_or("unknown"),
            artifact.content.len()
        );
        Ok(artifact)
    }

    /// Reads back the raw structured payload stored alongside a capture.
    ///
    /// The payload is the source-faithful record: it is written once and
    /// never rewritten, so it stays valid as evidence of what the page said
    /// even after summaries and tags have been regenerated several times.
    pub fn get_capture_payload(
        &self,
        id: &str,
    ) -> Result<crate::capture::web::WebCapturePayload, VaultError> {
        let artifact = self.get_vault_file(id)?;
        if !artifact.is_capture() {
            return Err(VaultError::NotFound(format!("Capture {}", id)));
        }
        let path = self.vault_dir().join(&artifact.vault_path);
        let raw = fs::read_to_string(&path)?;
        serde_json::from_str(&raw).map_err(|e| VaultError::FrontmatterError(e.to_string()))
    }

    /// Re-runs normalization over a capture's stored payload.
    ///
    /// The point of keeping the raw payload is that improvements to
    /// normalization are retroactive: identity, provenance history, and
    /// created-at are preserved, and only the derived markdown changes.
    pub fn renormalize_capture(&self, id: &str) -> Result<VaultFile, VaultError> {
        let mut artifact = self.get_vault_file(id)?;
        let payload = self.get_capture_payload(id)?;
        let normalized = crate::capture::web::normalize::normalize(&payload)
            .map_err(|e| VaultError::FrontmatterError(e.to_string()))?;

        artifact.content_hash = capture_content_hash(&normalized);
        artifact.size_bytes = normalized.markdown.len() as u64;

        let mut provenance = normalized.provenance;
        if let Some(existing) = artifact.capture.as_ref() {
            provenance.captured_at = existing.captured_at.clone();
            provenance.version = existing.version;
            provenance.previous_capture_id = existing.previous_capture_id.clone();
            provenance.recapture_count = existing.recapture_count;
        }

        artifact.content = normalized.markdown;
        artifact.capture = Some(provenance);
        artifact.updated_at = chrono::Utc::now().to_rfc3339();
        self.save_vault_file(&artifact)?;
        Ok(artifact)
    }

    /// Where an artifact's directory lives, decided by what the artifact is
    /// rather than by where it happens to be found — captures and imported
    /// files share one model but never one tree.
    pub fn vault_file_dir(&self, file: &VaultFile) -> PathBuf {
        let subdir = if file.is_capture() { CAPTURES_DIR } else { FILES_DIR };
        self.vault_dir().join(subdir).join(&file.id)
    }

    /// Resolves an artifact id to its directory by looking in both trees.
    ///
    /// Every existing caller passes an id with no idea whether it names an
    /// imported file or a capture, and none of them should have to care:
    /// this is what lets `enrich_vault_file`, `summarize_vault_file`, and
    /// promotion to a Scribble work on captures without a second code path.
    fn find_vault_file_dir(&self, id: &str) -> Option<PathBuf> {
        for subdir in [FILES_DIR, CAPTURES_DIR] {
            let candidate = self.vault_dir().join(subdir).join(id);
            if candidate.join("metadata.json").exists() {
                return Some(candidate);
            }
        }
        None
    }

    pub fn save_vault_file(&self, file: &VaultFile) -> Result<PathBuf, VaultError> {
        self.init()?;
        let vault_file_dir = self.vault_file_dir(file);
        fs::create_dir_all(&vault_file_dir)?;

        let meta_path = vault_file_dir.join("metadata.json");
        let json_data = serde_json::to_string_pretty(file)
            .map_err(|e| VaultError::FrontmatterError(e.to_string()))?;
        fs::write(&meta_path, json_data)?;
        Ok(meta_path)
    }

    pub fn get_vault_file(&self, id: &str) -> Result<VaultFile, VaultError> {
        self.init()?;
        let meta_path = self
            .find_vault_file_dir(id)
            .map(|dir| dir.join("metadata.json"))
            .ok_or_else(|| VaultError::NotFound(format!("VaultFile {}", id)))?;
        let content = fs::read_to_string(&meta_path)?;
        let file: VaultFile = serde_json::from_str(&content)
            .map_err(|e| VaultError::FrontmatterError(e.to_string()))?;
        Ok(file)
    }

    /// Imported documents only. Captures live in their own tree and are
    /// listed by `list_captures`, so the Files surface never has to filter
    /// them out and never silently changed when captures shipped.
    pub fn list_vault_files(&self) -> Result<Vec<VaultFile>, VaultError> {
        self.read_vault_file_dir(FILES_DIR)
    }

    /// Web captures, newest first.
    pub fn list_captures(&self) -> Result<Vec<VaultFile>, VaultError> {
        self.read_vault_file_dir(CAPTURES_DIR)
    }

    fn read_vault_file_dir(&self, subdir: &str) -> Result<Vec<VaultFile>, VaultError> {
        self.init()?;
        let files_dir = self.vault_dir().join(subdir);
        let mut files = Vec::new();

        if !files_dir.exists() {
            return Ok(files);
        }

        for entry in fs::read_dir(&files_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let meta_path = path.join("metadata.json");
                if meta_path.exists() {
                    if let Ok(content) = fs::read_to_string(&meta_path) {
                        if let Ok(file) = serde_json::from_str::<VaultFile>(&content) {
                            files.push(file);
                        }
                    }
                }
            }
        }

        files.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(files)
    }

    pub fn delete_vault_file(&self, id: &str) -> Result<(), VaultError> {
        self.init()?;
        let Some(vault_file_dir) = self.find_vault_file_dir(id) else {
            return Ok(());
        };

        let trash_dir = self.vault_dir().join("trash");
        // The trash id encodes the item type, and restore reads it back to
        // decide which tree to put the directory into — so a capture has to
        // be trashed as a capture, not as a file.
        let (item_type, trash_id) = match self.get_vault_file(id) {
            Ok(file) if file.is_capture() => ("capture", format!("trash_capture_{}", id)),
            _ => ("file", format!("trash_file_{}", id)),
        };
        let meta_path = trash_dir.join(format!("{}.json", trash_id));
        let dest_dir = trash_dir.join(&trash_id);

        if let Ok(file) = self.get_vault_file(id) {
            let title = file
                .capture
                .as_ref()
                .map(|c| c.page_title.clone())
                .unwrap_or_else(|| file.original_filename.clone());
            let trash_item = TrashItem::new(id, item_type, &title, &file.content);
            let _ = fs::write(&meta_path, serde_json::to_string_pretty(&trash_item).unwrap_or_default());
        }

        let _ = fs::rename(&vault_file_dir, &dest_dir);
        Ok(())
    }

    pub fn reprocess_vault_file(&self, id: &str) -> Result<VaultFile, VaultError> {
        let mut file = self.get_vault_file(id)?;

        // A capture's text was never extracted from bytes on disk — it was
        // normalized from a structured payload. Re-running document
        // extraction on `capture.json` would only overwrite good content with
        // a failure, so re-normalization is the capture's own operation
        // (`renormalize_capture`) and this path leaves it alone.
        if file.is_capture() {
            return Ok(file);
        }

        let vault_copy_path = self.vault_dir().join(&file.vault_path);

        if vault_copy_path.exists() {
            match extract_text_from_file(&vault_copy_path, &file.file_type) {
                Ok(text) => {
                    file.content = text.clone();
                    file.extraction_status = "extracted".to_string();
                    let knowledge = crate::pipeline::extract_deterministic_knowledge(&text);
                    file.summary = knowledge.summary;
                    file.topics = knowledge.topics;
                    file.entities = knowledge.entities;
                    file.tags = file.topics.clone();
                }
                Err(e) => {
                    tracing::warn!("Reprocessing file {} failed: {}", id, e);
                    file.extraction_status = "failed".to_string();
                }
            }
        }

        file.updated_at = chrono::Utc::now().to_rfc3339();
        self.save_vault_file(&file)?;
        Ok(file)
    }

    pub fn create_scribble_from_file(&self, id: &str) -> Result<Scribble, VaultError> {
        let mut file = self.get_vault_file(id)?;

        // Return existing scribble if already created for this file
        if let Some(ref scribble_id) = file.linked_scribble_id {
            if let Ok(existing) = self.get_scribble(scribble_id) {
                return Ok(existing);
            }
        }
        if let Ok(all_scribbles) = self.list_scribbles() {
            if let Some(existing) = all_scribbles.into_iter().find(|s| {
                s.source_metadata.get("source_file_id").and_then(|v| v.as_str()) == Some(file.id.as_str())
            }) {
                file.linked_scribble_id = Some(existing.id.clone());
                let _ = self.save_vault_file(&file);
                return Ok(existing);
            }
        }

        let (initial_title, source_type, source_metadata) = match &file.capture {
            // A promoted capture keeps its provenance: the Scribble says it
            // came from a ChatGPT conversation, not from "a file".
            Some(capture) => (
                capture.page_title.clone(),
                capture_scribble_source_type(&capture.capture_type).to_string(),
                serde_json::json!({
                    "source_file_id": file.id,
                    "source_capture_id": file.id,
                    "source_modality": "WEB_CAPTURE",
                    "capture_type": capture.capture_type,
                    "application": capture.application,
                    "domain": capture.domain,
                    "url": capture.url,
                    "captured_at": capture.captured_at,
                    "fidelity": capture.fidelity,
                    // The trust level travels with the promotion. A Scribble
                    // made from a capture is still a record of what a website
                    // said, and the knowledge graph it joins has to be able to
                    // tell that from a fact the user asserted.
                    "trust": capture.trust,
                    "promoted_at": chrono::Utc::now().to_rfc3339(),
                }),
            ),
            None => (
                format!("Scribble: {}", file.original_filename),
                crate::vault::SOURCE_TYPE_FILE.to_string(),
                serde_json::json!({
                    "source_file_id": file.id,
                    "source_filename": file.original_filename,
                    "source_file_type": file.file_type,
                    "imported_at": file.created_at,
                    "source_modality": "FILE",
                    "promoted_at": chrono::Utc::now().to_rfc3339(),
                }),
            ),
        };
        let mut scribble = Scribble::new_text(&file.content, Some(&initial_title));
        scribble.source_type = source_type;
        scribble.source_metadata = source_metadata;
        scribble.topics = file.topics.clone();
        scribble.entities = file.entities.clone();
        scribble.tags = file.tags.clone();
        scribble.summary = file.summary.clone();

        self.save_scribble(&scribble)?;

        file.linked_scribble_id = Some(scribble.id.clone());
        self.save_vault_file(&file)?;

        Ok(scribble)
    }

    pub fn search_scribbles(&self, query: &str, top_k: usize) -> Result<Vec<Scribble>, VaultError> {
        let scribbles = self.list_scribbles()?;
        let query_terms = tokenize(query);
        if query_terms.is_empty() || scribbles.is_empty() {
            return Ok(Vec::new());
        }

        let mut scored: Vec<(usize, Scribble)> = scribbles
            .into_iter()
            .map(|scribble| {
                let haystack = format!(
                    "{} {} {} {} {} {}",
                    scribble.title,
                    scribble.content,
                    scribble.summary.as_deref().unwrap_or(""),
                    scribble.topics.join(" "),
                    scribble.entities.join(" "),
                    scribble.tags.join(" ")
                )
                .to_lowercase();

                let score = query_terms
                    .iter()
                    .filter(|t| haystack.contains(t.as_str()))
                    .count();
                (score, scribble)
            })
            .filter(|(score, _)| *score > 0)
            .collect();

        scored.sort_by_key(|a| std::cmp::Reverse(a.0));
        Ok(scored.into_iter().take(top_k).map(|(_, s)| s).collect())
    }

    pub fn search_knowledge(&self, query: &str) -> Result<KnowledgeSearchResult, VaultError> {
        let all_scribbles = self.list_scribbles()?;
        let query_terms = tokenize(query);
        if query_terms.is_empty() {
            return Ok(KnowledgeSearchResult {
                direct_matches: all_scribbles.clone(),
                related_scribbles: Vec::new(),
                matched_topics: Vec::new(),
                matched_entities: Vec::new(),
                total_count: all_scribbles.len(),
            });
        }

        let mut direct_matches = Vec::new();
        let mut matched_topics = Vec::new();
        let mut matched_entities = Vec::new();
        let mut matched_topic_set = std::collections::HashSet::new();

        for s in &all_scribbles {
            let haystack = format!("{} {}", s.title, s.content).to_lowercase();
            let matches_direct = query_terms.iter().any(|t| haystack.contains(t));

            if matches_direct {
                direct_matches.push(s.clone());
            }

            for topic in &s.topics {
                let topic_lower = topic.to_lowercase();
                if query_terms.iter().any(|t| topic_lower.contains(t)) && !matched_topic_set.contains(topic) {
                    matched_topic_set.insert(topic.clone());
                    matched_topics.push(topic.clone());
                }
            }

            for entity in &s.entities {
                let entity_lower = entity.to_lowercase();
                if query_terms.iter().any(|t| entity_lower.contains(t)) && !matched_entities.contains(entity) {
                    matched_entities.push(entity.clone());
                }
            }
        }

        // Related scribbles: scribbles that share topics/entities with direct matches or are connected via relationships in either direction
        let direct_ids: std::collections::HashSet<String> = direct_matches.iter().map(|s| s.id.clone()).collect();
        let direct_topics: std::collections::HashSet<String> = direct_matches.iter().flat_map(|s| s.topics.clone()).collect();
        let direct_target_ids: std::collections::HashSet<String> = direct_matches.iter().flat_map(|s| s.relationships.iter().map(|r| r.target_id.clone())).collect();
        let mut related_scribbles = Vec::new();

        for s in &all_scribbles {
            if direct_ids.contains(&s.id) {
                continue;
            }
            let shares_topic = s.topics.iter().any(|t| matched_topic_set.contains(t) || direct_topics.contains(t));
            let links_to_direct = s.relationships.iter().any(|r| direct_ids.contains(&r.target_id));
            let linked_from_direct = direct_target_ids.contains(&s.id);

            if shares_topic || links_to_direct || linked_from_direct {
                related_scribbles.push(s.clone());
            }
        }

        let total_count = direct_matches.len() + related_scribbles.len();

        Ok(KnowledgeSearchResult {
            direct_matches,
            related_scribbles,
            matched_topics,
            matched_entities,
            total_count,
        })
    }

    pub fn get_knowledge_graph(&self, filter: Option<&GraphFilter>) -> Result<KnowledgeGraphData, VaultError> {
        let scribbles = self.list_scribbles()?;
        Ok(KnowledgeGraphData::from_scribbles(&scribbles, filter))
    }

    pub fn add_scribble_relationship(
        &self,
        source_id: &str,
        relationship: ScribbleRelationship,
    ) -> Result<Scribble, VaultError> {
        let mut scribble = self.get_scribble(source_id)?;
        scribble.relationships.retain(|r| r.id != relationship.id && r.target_id != relationship.target_id);
        scribble.relationships.push(relationship);
        scribble.updated_at = chrono::Utc::now().to_rfc3339();
        self.save_scribble(&scribble)?;
        Ok(scribble)
    }

    pub fn remove_scribble_relationship(
        &self,
        source_id: &str,
        relationship_id: &str,
    ) -> Result<Scribble, VaultError> {
        let mut scribble = self.get_scribble(source_id)?;
        scribble.relationships.retain(|r| r.id != relationship_id);
        scribble.updated_at = chrono::Utc::now().to_rfc3339();
        self.save_scribble(&scribble)?;
        Ok(scribble)
    }

    /// Moves a scribble, voice note, or meeting to the 30-day Trash.
    pub fn move_to_trash(&self, item_type: &str, id: &str) -> Result<TrashItem, VaultError> {
        self.init()?;
        let trash_dir = self.vault_dir().join("trash");

        match item_type {
            "scribble" => {
                let scribble = self.get_scribble(id)?;
                let trash_item = TrashItem::new(id, "scribble", &scribble.title, &scribble.content);
                let meta_path = trash_dir.join(format!("{}.json", trash_item.id));
                let src_md = self.vault_dir().join("scribbles").join(format!("{}.md", id));
                let dest_md = trash_dir.join(format!("{}.md", trash_item.id));

                fs::write(&meta_path, serde_json::to_string_pretty(&trash_item).map_err(|e| VaultError::FrontmatterError(e.to_string()))?)?;
                if src_md.exists() {
                    fs::rename(&src_md, &dest_md)?;
                }

                // Clean up any relationships in remaining active scribbles pointing to this trashed scribble
                if let Ok(active_scribbles) = self.list_scribbles() {
                    for mut other in active_scribbles {
                        let original_len = other.relationships.len();
                        other.relationships.retain(|r| r.target_id != id);
                        if other.relationships.len() != original_len {
                            let _ = self.save_scribble(&other);
                        }
                    }
                }

                Ok(trash_item)
            }
            "voice_note" | "note" => {
                let note = self.get_note(id)?;
                let trash_item = TrashItem::new(id, "voice_note", &note.title, &note.content);
                let meta_path = trash_dir.join(format!("{}.json", trash_item.id));
                let src_md = self.vault_dir().join("notes").join(format!("{}.md", id));
                let dest_md = trash_dir.join(format!("{}.md", trash_item.id));

                fs::write(&meta_path, serde_json::to_string_pretty(&trash_item).map_err(|e| VaultError::FrontmatterError(e.to_string()))?)?;
                if src_md.exists() {
                    fs::rename(&src_md, &dest_md)?;
                }
                Ok(trash_item)
            }
            "meeting" | "meetings" | "meeting_v2" => {
                let session_store = crate::meetings_v2::SessionStore::new(self.vault_dir());
                let session = session_store.get_session(id).map_err(VaultError::NotFound)?;
                let snippet = session_store.get_full_transcript_text(id).unwrap_or_default();
                let trash_item = TrashItem::new(id, "meeting", &session.title, &snippet);
                let meta_path = trash_dir.join(format!("{}.json", trash_item.id));
                let src_dir = self.vault_dir().join("meetings_v2").join(id);
                let dest_dir = trash_dir.join(&trash_item.id);

                fs::write(&meta_path, serde_json::to_string_pretty(&trash_item).map_err(|e| VaultError::FrontmatterError(e.to_string()))?)?;
                if src_dir.exists() {
                    fs::rename(&src_dir, &dest_dir)?;
                }
                Ok(trash_item)
            }
            _ => Err(VaultError::NotFound(format!("Unknown item type: {}", item_type))),
        }
    }

    /// Lists all active items currently in Trash, auto-purging any items expired past 30 days.
    pub fn get_trash_items(&self) -> Result<Vec<TrashItem>, VaultError> {
        self.init()?;
        let _ = self.purge_expired_trash();
        let trash_dir = self.vault_dir().join("trash");
        let mut items = Vec::new();

        if !trash_dir.exists() {
            return Ok(items);
        }

        for entry in fs::read_dir(trash_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(item) = serde_json::from_str::<TrashItem>(&content) {
                        items.push(item);
                    }
                }
            }
        }

        items.sort_by(|a, b| b.deleted_at.cmp(&a.deleted_at));
        Ok(items)
    }

    /// Restores a deleted item from Trash back to its active folder.
    pub fn restore_trash_item(&self, trash_id: &str) -> Result<(), VaultError> {
        self.init()?;
        let trash_dir = self.vault_dir().join("trash");
        let meta_path = trash_dir.join(format!("{}.json", trash_id));
        if !meta_path.exists() {
            return Err(VaultError::NotFound(trash_id.to_string()));
        }

        let content = fs::read_to_string(&meta_path)?;
        let item: TrashItem = serde_json::from_str(&content).map_err(|e| VaultError::FrontmatterError(e.to_string()))?;

        let trash_md = trash_dir.join(format!("{}.md", trash_id));
        let trash_meeting_dir = trash_dir.join(trash_id);
        match item.item_type.as_str() {
            "scribble" => {
                let active_md = self.vault_dir().join("scribbles").join(format!("{}.md", item.original_id));
                if trash_md.exists() {
                    fs::rename(&trash_md, &active_md)?;
                }
            }
            "voice_note" | "note" => {
                let active_md = self.vault_dir().join("notes").join(format!("{}.md", item.original_id));
                if trash_md.exists() {
                    fs::rename(&trash_md, &active_md)?;
                }
            }
            "meeting" | "meetings" | "meeting_v2" => {
                let active_meeting_dir = self.vault_dir().join("meetings_v2").join(&item.original_id);
                if trash_meeting_dir.exists() {
                    fs::rename(&trash_meeting_dir, &active_meeting_dir)?;
                }
            }
            "file" => {
                let active_file_dir = self.vault_dir().join(FILES_DIR).join(&item.original_id);
                let trash_file_dir = trash_dir.join(format!("trash_file_{}", item.original_id));
                if trash_file_dir.exists() {
                    fs::rename(&trash_file_dir, &active_file_dir)?;
                }
            }
            "capture" => {
                let active_capture_dir = self.vault_dir().join(CAPTURES_DIR).join(&item.original_id);
                let trash_capture_dir = trash_dir.join(format!("trash_capture_{}", item.original_id));
                if trash_capture_dir.exists() {
                    fs::rename(&trash_capture_dir, &active_capture_dir)?;
                }
            }
            _ => {}
        }

        let _ = fs::remove_file(&meta_path);
        let _ = fs::remove_file(&trash_md);
        Ok(())
    }

    /// Permanently deletes a single item from Trash.
    pub fn delete_trash_item_permanently(&self, trash_id: &str) -> Result<(), VaultError> {
        self.init()?;
        let trash_dir = self.vault_dir().join("trash");
        let meta_path = trash_dir.join(format!("{}.json", trash_id));
        let trash_md = trash_dir.join(format!("{}.md", trash_id));
        let trash_meeting_dir = trash_dir.join(trash_id);

        if meta_path.exists() {
            fs::remove_file(&meta_path)?;
        }
        if trash_md.exists() {
            fs::remove_file(&trash_md)?;
        }
        if trash_meeting_dir.exists() && trash_meeting_dir.is_dir() {
            let _ = fs::remove_dir_all(&trash_meeting_dir);
        }
        Ok(())
    }

    /// Empties all items from Trash.
    pub fn empty_trash(&self) -> Result<usize, VaultError> {
        self.init()?;
        let trash_dir = self.vault_dir().join("trash");
        let mut count = 0;

        if !trash_dir.exists() {
            return Ok(0);
        }

        for entry in fs::read_dir(trash_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let _ = fs::remove_dir_all(&path);
            } else {
                if path.extension().is_some_and(|ext| ext == "json") {
                    count += 1;
                }
                let _ = fs::remove_file(&path);
            }
        }

        Ok(count)
    }

    /// Purges items in Trash that have expired past their 30-day window.
    pub fn purge_expired_trash(&self) -> Result<usize, VaultError> {
        let trash_dir = self.vault_dir().join("trash");
        if !trash_dir.exists() {
            return Ok(0);
        }

        let mut purged = 0;
        for entry in fs::read_dir(&trash_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(item) = serde_json::from_str::<TrashItem>(&content) {
                        if item.is_expired() {
                            let trash_md = trash_dir.join(format!("{}.md", item.id));
                            let trash_meeting_dir = trash_dir.join(&item.id);
                            let _ = fs::remove_file(&path);
                            let _ = fs::remove_file(&trash_md);
                            if trash_meeting_dir.exists() && trash_meeting_dir.is_dir() {
                                let _ = fs::remove_dir_all(&trash_meeting_dir);
                            }
                            purged += 1;
                        }
                    }
                }
            }
        }

        Ok(purged)
    }

    /// Synchronizes, consolidates, and refreshes any existing Scribbles derived from the merged Voice Notes.
    pub fn sync_scribbles_for_voice_note_merge(
        &self,
        primary_vn_id: &str,
        secondary_vn_id: &str,
    ) -> Result<Vec<String>, VaultError> {
        let primary_note = self.get_note(primary_vn_id)?;
        let scribbles = self.list_scribbles()?;

        let mut matching_scribbles = Vec::new();
        for s in scribbles {
            let mut matches = false;
            if let Some(src_id) = s.source_metadata.get("source_voice_note_id").and_then(|v| v.as_str()) {
                if src_id == primary_vn_id || src_id == secondary_vn_id {
                    matches = true;
                }
            }
            if !matches {
                if let Some(arr) = s.source_metadata.get("source_voice_note_ids").and_then(|v| v.as_array()) {
                    for v in arr {
                        if let Some(id_str) = v.as_str() {
                            if id_str == primary_vn_id || id_str == secondary_vn_id {
                                matches = true;
                                break;
                            }
                        }
                    }
                }
            }
            if matches {
                matching_scribbles.push(s);
            }
        }

        if matching_scribbles.is_empty() {
            return Ok(Vec::new());
        }

        let now = chrono::Utc::now().to_rfc3339();

        // If multiple scribbles exist (e.g. one for primary and one for secondary),
        // keep the primary scribble matching primary_vn_id and retire redundant secondary scribbles to trash.
        let primary_idx = matching_scribbles
            .iter()
            .position(|s| {
                s.source_metadata.get("source_voice_note_id").and_then(|v| v.as_str()) == Some(primary_vn_id)
            })
            .unwrap_or(0);
        let mut target_scribble = matching_scribbles.remove(primary_idx);

        // Collect all unique contributing voice note IDs
        let mut all_vn_ids = std::collections::HashSet::new();
        all_vn_ids.insert(primary_vn_id.to_string());
        all_vn_ids.insert(secondary_vn_id.to_string());

        if let Some(arr) = target_scribble.source_metadata.get("source_voice_note_ids").and_then(|v| v.as_array()) {
            for v in arr {
                if let Some(s) = v.as_str() {
                    all_vn_ids.insert(s.to_string());
                }
            }
        }

        // Clean up redundant scribbles if both notes were individually converted before merging
        for redundant in matching_scribbles {
            if let Some(arr) = redundant.source_metadata.get("source_voice_note_ids").and_then(|v| v.as_array()) {
                for v in arr {
                    if let Some(s) = v.as_str() {
                        all_vn_ids.insert(s.to_string());
                    }
                }
            }
            let _ = self.move_to_trash("scribble", &redundant.id);
        }

        let vn_ids_vec: Vec<String> = all_vn_ids.into_iter().collect();

        // Update target scribble's content, provenance, and timestamps
        target_scribble.content = primary_note.content.clone();
        target_scribble.updated_at = now.clone();
        target_scribble.source_type = crate::vault::SOURCE_TYPE_VOICE.to_string();
        target_scribble.source_metadata = serde_json::json!({
            "source_voice_note_id": primary_vn_id,
            "source_voice_note_ids": vn_ids_vec,
            "is_merged": true,
            "merged_at": now,
            "source_modality": "VOICE",
            "last_synced_at": now,
        });
        target_scribble.ai_metadata.enrichment_status = "pending".to_string();

        self.save_scribble(&target_scribble)?;
        Ok(vec![target_scribble.id])
    }

    /// Synchronizes Scribbles after unmerging Voice Notes, updating metadata and restoring any trashed scribbles if present.
    pub fn sync_scribbles_for_voice_note_unmerge(
        &self,
        primary_vn_id: &str,
        secondary_vn_id: &str,
    ) -> Result<(), VaultError> {
        let scribbles = self.list_scribbles()?;
        let primary_note = self.get_note(primary_vn_id)?;

        for mut s in scribbles {
            let mut matches = false;
            if let Some(arr) = s.source_metadata.get("source_voice_note_ids").and_then(|v| v.as_array()) {
                if arr.iter().any(|v| v.as_str() == Some(primary_vn_id) || v.as_str() == Some(secondary_vn_id)) {
                    matches = true;
                }
            }
            if matches {
                if primary_note.merged_from.is_none() {
                    s.content = primary_note.content.clone();
                    if let Some(obj) = s.source_metadata.as_object_mut() {
                        obj.insert("is_merged".to_string(), serde_json::Value::Bool(false));
                        obj.insert("source_voice_note_ids".to_string(), serde_json::json!([primary_vn_id]));
                    }
                } else if let Some(ref ids) = primary_note.merged_from {
                    if let Some(obj) = s.source_metadata.as_object_mut() {
                        obj.insert("source_voice_note_ids".to_string(), serde_json::json!(ids));
                    }
                }
                s.updated_at = chrono::Utc::now().to_rfc3339();
                let _ = self.save_scribble(&s);
            }
        }

        // Restore partner scribble from trash if it was moved to trash during merge
        let trash_dir = self.vault_dir().join("trash");
        let trash_items = self.get_trash_items().unwrap_or_default();
        for item in trash_items {
            if item.item_type == "scribble" {
                let trash_md = trash_dir.join(format!("{}.md", item.id));
                if trash_md.exists() {
                    if let Ok(content) = fs::read_to_string(&trash_md) {
                        if let Some(s) = Scribble::parse_markdown(&content) {
                            let matches = s.source_metadata.get("source_voice_note_id").and_then(|v| v.as_str()) == Some(secondary_vn_id)
                                || s.source_metadata.get("source_voice_note_id").and_then(|v| v.as_str()) == Some(primary_vn_id)
                                || s.source_metadata.get("source_voice_note_ids").and_then(|v| v.as_array())
                                    .is_some_and(|arr| arr.iter().any(|v| v.as_str() == Some(secondary_vn_id) || v.as_str() == Some(primary_vn_id)));
                            if matches {
                                let _ = self.restore_trash_item(&item.id);
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Merges two or more source Scribbles into a brand new synthesized Scribble.
    /// Preserves full provenance to the source Scribbles and links them with DERIVED_FROM.
    pub fn merge_scribbles(&self, source_ids: &[String]) -> Result<Scribble, VaultError> {
        if source_ids.len() < 2 {
            return Err(VaultError::FrontmatterError("At least 2 scribbles are required to merge.".to_string()));
        }

        let mut source_scribbles = Vec::new();
        for id in source_ids {
            source_scribbles.push(self.get_scribble(id)?);
        }

        let mut content_parts = Vec::new();
        let mut combined_tags = std::collections::HashSet::new();

        for s in &source_scribbles {
            let raw_t = s.title.trim().trim_start_matches('[').trim_end_matches(']').trim();
            let clean_t = if raw_t.is_empty()
                || raw_t == "Untitled Thought"
                || raw_t.starts_with("Generating title")
                || raw_t.starts_with("Synthesis:")
                || raw_t.starts_with("Consolidated:")
            {
                "Thought Section".to_string()
            } else {
                raw_t.to_string()
            };
            content_parts.push(format!("### {}\n\n{}", clean_t, s.content));
            for tag in &s.tags {
                combined_tags.insert(tag.clone());
            }
        }

        let combined_content = content_parts.join("\n\n---\n\n");
        let now = chrono::Utc::now().to_rfc3339();

        let initial_title = crate::pipeline::extract_deterministic_title(&combined_content);
        let mut merged_scribble = Scribble::new_text(&combined_content, Some(&initial_title));
        // Reset and re-rank topics and entities cleanly from the combined content
        merged_scribble.topics = crate::pipeline::extract_deterministic_topics(&combined_content, 7);
        merged_scribble.entities = crate::pipeline::extract_deterministic_entities(&combined_content, 7);
        merged_scribble.tags = combined_tags.into_iter().collect();
        merged_scribble.relationships = Vec::new();
        merged_scribble.source_type = crate::vault::SOURCE_TYPE_TEXT.to_string();
        merged_scribble.source_metadata = serde_json::json!({
            "creation_method": "merge",
            "source_scribble_ids": source_ids,
            "merged_at": now,
            "source_count": source_ids.len(),
            "source_modality": "MERGED_SCRIBBLE",
        });
        merged_scribble.ai_metadata.suggested_questions = crate::pipeline::extract_deterministic_questions(
            &combined_content,
            &initial_title,
            &merged_scribble.topics,
            &merged_scribble.entities,
        );
        merged_scribble.ai_metadata.enrichment_status = "pending".to_string();

        self.save_scribble(&merged_scribble)?;

        // Retire the source scribbles to Trash so they do not remain active as independent duplicates
        for id in source_ids {
            let _ = self.move_to_trash("scribble", id);
        }

        Ok(merged_scribble)
    }
}

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 2)
        .map(|w| w.to_string())
        .collect()
}

fn parse_debug_string_list(raw: &str) -> Vec<String> {
    raw.trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Content identity for a capture.
///
/// Hashes the sanitized structured content and the page title — what the page
/// said — rather than the rendered markdown, which carries the capture
/// timestamp and would therefore differ on every single capture.
fn capture_content_hash(
    normalized: &crate::capture::web::normalize::NormalizedCapture,
) -> String {
    let canonical = serde_json::json!({
        "title": normalized.title,
        "url": normalized.provenance.url,
        "content": normalized.structured.content,
    });
    let mut hasher = Sha256::new();
    hasher.update(canonical.to_string().as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Which Scribble source type a promoted capture should carry.
///
/// These constants already existed on the Scribble model — the capture system
/// is what finally populates them, rather than introducing a parallel
/// vocabulary for the same idea.
pub fn capture_scribble_source_type(capture_type: &str) -> &'static str {
    match capture_type {
        crate::capture::web::source::CAPTURE_TYPE_CONVERSATION => SOURCE_TYPE_BROWSER_CONVERSATION,
        _ => SOURCE_TYPE_BROWSER_PAGE,
    }
}

/// Builds a filesystem-safe name for a capture's stored payload.
///
/// Windows reserves more characters than POSIX does, and a page title is
/// arbitrary text from an untrusted source, so this is a strict allowlist
/// rather than a blocklist — and it can never produce a path segment that
/// escapes its directory.
fn capture_payload_filename(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_') {
                c
            } else {
                ' '
            }
        })
        .collect();

    let slug: String = cleaned
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
        .chars()
        .take(60)
        .collect();

    let slug = slug.trim_matches(['-', '.']).to_string();
    if slug.is_empty() {
        "capture.json".to_string()
    } else {
        format!("{}.json", slug)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voice_note_saved_and_filtered_by_type() {
        let temp_dir = std::env::temp_dir().join(format!("relay_test_{}", uuid::Uuid::new_v4()));
        let manager = VaultManager::new(temp_dir.clone());

        let voice_note = VaultNote::new_voice_note("Can you send me the report tomorrow?");
        assert_eq!(voice_note.note_type, VOICE_NOTE_TYPE);
        assert_eq!(voice_note.content, "Can you send me the report tomorrow?");
        manager.save_note(&voice_note).unwrap();

        let scribble_note = VaultNote {
            id: "note_scribble".to_string(),
            title: "Voice Scribble Note".to_string(),
            note_type: "scribble".to_string(),
            created_at: "2026-08-19T01:50:00Z".to_string(),
            updated_at: "2026-08-19T01:50:00Z".to_string(),
            tags: vec![],
            source_audio: None,
            content: "Some LLM-cleaned content".to_string(),
            merged_from: None,
        };
        manager.save_note(&scribble_note).unwrap();

        // Both land in the vault, but only the voice note comes back when
        // filtering by type — the Voice Note page must not show scribble
        // notes in its Transcript History.
        assert_eq!(manager.list_notes().unwrap().len(), 2);
        let voice_notes = manager.list_notes_by_type(VOICE_NOTE_TYPE).unwrap();
        assert_eq!(voice_notes.len(), 1);
        assert_eq!(voice_notes[0].id, voice_note.id);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_set_vault_dir_repoints_reads_and_writes() {
        let dir_a = std::env::temp_dir().join(format!("relay_test_{}", uuid::Uuid::new_v4()));
        let dir_b = std::env::temp_dir().join(format!("relay_test_{}", uuid::Uuid::new_v4()));
        let manager = VaultManager::new(dir_a.clone());

        manager
            .save_note(&VaultNote::new_voice_note("first note, in dir_a"))
            .unwrap();
        assert_eq!(manager.list_notes().unwrap().len(), 1);

        // Repointing must not touch whatever already exists at the old
        // location — it only changes where *future* reads/writes go.
        manager.set_vault_dir(dir_b.clone());
        assert_eq!(manager.vault_dir(), dir_b);
        assert_eq!(
            manager.list_notes().unwrap().len(),
            0,
            "the freshly repointed vault must start empty, not see dir_a's note"
        );

        manager
            .save_note(&VaultNote::new_voice_note("second note, in dir_b"))
            .unwrap();
        assert_eq!(manager.list_notes().unwrap().len(), 1);
        assert!(
            dir_a.join("notes").read_dir().unwrap().count() == 1,
            "dir_a's note must be untouched"
        );

        let _ = fs::remove_dir_all(dir_a);
        let _ = fs::remove_dir_all(dir_b);
    }

    #[test]
    fn test_kanban_card_serialization() {
        let card = KanbanCard {
            id: "card_101".to_string(),
            title: "Build Tauri Rust Shell".to_string(),
            assignee: "Nitin".to_string(),
            status: "in_progress".to_string(),
            priority: "high".to_string(),
            due_date: Some("2026-08-25".to_string()),
            created_at: "2026-08-19T01:50:00Z".to_string(),
            description: "Scaffold Rust domain modules per project rules.".to_string(),
            source_note_id: Some("note_001".to_string()),
        };

        let temp_dir = std::env::temp_dir().join(format!("relay_test_{}", uuid::Uuid::new_v4()));
        let manager = VaultManager::new(temp_dir.clone());
        let _card_path = manager.save_kanban_card(&card).unwrap();

        let loaded_cards = manager.list_kanban_cards().unwrap();
        assert_eq!(loaded_cards.len(), 1);
        let loaded = &loaded_cards[0];

        // Every field, not just the first three: the round trip is the only
        // thing standing between a frontmatter-parser change and silently
        // dropping a card's status, priority, or provenance.
        assert_eq!(loaded.id, "card_101");
        assert_eq!(loaded.title, "Build Tauri Rust Shell");
        assert_eq!(loaded.assignee, "Nitin");
        assert_eq!(loaded.status, "in_progress");
        assert_eq!(loaded.priority, "high");
        assert_eq!(loaded.due_date.as_deref(), Some("2026-08-25"));
        assert_eq!(loaded.created_at, "2026-08-19T01:50:00Z");
        assert_eq!(loaded.source_note_id.as_deref(), Some("note_001"));
        assert_eq!(
            loaded.description,
            "Scaffold Rust domain modules per project rules."
        );

        let _ = fs::remove_dir_all(temp_dir);
    }

    /// The optional fields are written as empty strings, never omitted, so the
    /// parser has to tell "no due date" from the string `""` — and a
    /// `source_note_id:` key must not be mistaken for the `id:` key that is a
    /// suffix of it.
    #[test]
    fn test_kanban_card_without_optional_fields_round_trips() {
        let card = KanbanCard {
            id: "card_102".to_string(),
            title: "Untriaged".to_string(),
            assignee: String::new(),
            status: "todo".to_string(),
            priority: "medium".to_string(),
            due_date: None,
            created_at: "2026-08-30T09:00:00Z".to_string(),
            description: "No owner, no deadline, no source.".to_string(),
            source_note_id: None,
        };

        let temp_dir = std::env::temp_dir().join(format!("relay_test_{}", uuid::Uuid::new_v4()));
        let manager = VaultManager::new(temp_dir.clone());
        manager.save_kanban_card(&card).unwrap();

        let loaded_cards = manager.list_kanban_cards().unwrap();
        assert_eq!(loaded_cards.len(), 1);
        let loaded = &loaded_cards[0];

        assert_eq!(loaded.id, "card_102");
        assert_eq!(loaded.assignee, "");
        assert_eq!(loaded.status, "todo");
        assert_eq!(loaded.priority, "medium");
        // Absent, not `Some("")` — an empty deadline is no deadline.
        assert_eq!(loaded.due_date, None);
        assert_eq!(loaded.source_note_id, None);
        assert_eq!(loaded.created_at, "2026-08-30T09:00:00Z");

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_note_roundtrip_and_search() {
        let temp_dir = std::env::temp_dir().join(format!("relay_test_{}", uuid::Uuid::new_v4()));
        let manager = VaultManager::new(temp_dir.clone());

        let note = VaultNote {
            id: "note_101".to_string(),
            title: "Rust Backend Architecture".to_string(),
            note_type: "scribble".to_string(),
            created_at: "2026-08-19T01:50:00Z".to_string(),
            updated_at: "2026-08-19T01:50:00Z".to_string(),
            tags: vec!["architecture".to_string(), "rust".to_string()],
            source_audio: None,
            content:
                "Relay's backend uses cpal for audio capture and whisper-rs for transcription."
                    .to_string(),
            merged_from: None,
        };
        manager.save_note(&note).unwrap();

        let notes = manager.list_notes().unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].id, "note_101");
        assert_eq!(notes[0].tags, vec!["architecture", "rust"]);
        assert!(notes[0].content.contains("whisper-rs"));

        let results = manager
            .search_notes("how does transcription work", 5)
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "note_101");

        let no_match = manager
            .search_notes("kanban calendar reminders", 5)
            .unwrap();
        assert!(no_match.is_empty());

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_note_update_delete_and_merge() {
        let temp_dir = std::env::temp_dir().join(format!("relay_test_{}", uuid::Uuid::new_v4()));
        let manager = VaultManager::new(temp_dir.clone());

        let mut note_a = VaultNote::new_voice_note("First part of thoughts.");
        note_a.created_at = "2026-08-20T10:00:00Z".to_string();
        manager.save_note(&note_a).unwrap();

        let mut note_b = VaultNote::new_voice_note("Second part of thoughts.");
        note_b.created_at = "2026-08-20T10:01:00Z".to_string();
        manager.save_note(&note_b).unwrap();

        // Update note A
        let updated = manager.update_note_content(&note_a.id, "Updated first part.").unwrap();
        assert_eq!(updated.content, "Updated first part.");

        // Merge notes A and B
        let merged = manager.merge_notes(&note_a.id, &note_b.id).unwrap();
        assert_eq!(merged.content, "Updated first part.\n\nSecond part of thoughts.");

        // Note B should now be deleted
        assert!(manager.get_note(&note_b.id).is_err());
        assert_eq!(manager.list_notes().unwrap().len(), 1);

        // Delete merged note
        manager.delete_note(&note_a.id).unwrap();
        assert_eq!(manager.list_notes().unwrap().len(), 0);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_voice_note_reversible_merge_basic_and_unmerge() {
        let temp_dir = std::env::temp_dir().join(format!("relay_test_vn_unmerge_{}", uuid::Uuid::new_v4()));
        let manager = VaultManager::new(temp_dir.clone());

        let mut note_a = VaultNote::new_voice_note("Verbatim text for Note A.");
        note_a.created_at = "2026-08-20T10:00:00Z".to_string();
        note_a.tags = vec!["tag1".to_string()];
        note_a.source_audio = Some("audio_a.wav".to_string());
        manager.save_note(&note_a).unwrap();

        let mut note_b = VaultNote::new_voice_note("Verbatim text for Note B.");
        note_b.created_at = "2026-08-20T10:01:00Z".to_string();
        note_b.tags = vec!["tag2".to_string()];
        note_b.source_audio = Some("audio_b.wav".to_string());
        manager.save_note(&note_b).unwrap();

        // 1. Merge A + B
        let merged = manager.merge_notes(&note_a.id, &note_b.id).unwrap();
        assert_eq!(merged.id, note_a.id);
        assert!(merged.merged_from.is_some());
        let merged_from = merged.merged_from.unwrap();
        assert_eq!(merged_from.len(), 2);
        assert_eq!(merged_from[0], note_a.id);
        assert_eq!(merged_from[1], note_b.id);
        assert!(manager.get_note(&note_b.id).is_err());

        // 2. Unmerge
        let unmerge_res = manager.unmerge_notes(&merged.id).unwrap();
        assert_eq!(unmerge_res.primary.id, note_a.id);
        assert_eq!(unmerge_res.primary.content, "Verbatim text for Note A.");
        assert_eq!(unmerge_res.primary.tags, vec!["tag1"]);
        assert_eq!(unmerge_res.primary.source_audio.as_deref(), Some("audio_a.wav"));
        assert!(unmerge_res.primary.merged_from.is_none());

        assert_eq!(unmerge_res.secondary.id, note_b.id);
        assert_eq!(unmerge_res.secondary.content, "Verbatim text for Note B.");
        assert_eq!(unmerge_res.secondary.tags, vec!["tag2"]);
        assert_eq!(unmerge_res.secondary.source_audio.as_deref(), Some("audio_b.wav"));
        assert!(unmerge_res.secondary.merged_from.is_none());

        // Both notes restored on disk
        let all_notes = manager.list_notes().unwrap();
        assert_eq!(all_notes.len(), 2);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_voice_note_merge_persistence_across_manager_reconstruction() {
        let temp_dir = std::env::temp_dir().join(format!("relay_test_vn_persist_{}", uuid::Uuid::new_v4()));
        let manager1 = VaultManager::new(temp_dir.clone());

        let note_a = VaultNote::new_voice_note("First session notes.");
        let note_b = VaultNote::new_voice_note("Second session notes.");
        manager1.save_note(&note_a).unwrap();
        manager1.save_note(&note_b).unwrap();
        let merged = manager1.merge_notes(&note_a.id, &note_b.id).unwrap();

        // Simulate Relay restart by instantiating new VaultManager
        let manager2 = VaultManager::new(temp_dir.clone());
        let reloaded_merged = manager2.get_note(&merged.id).unwrap();
        assert!(reloaded_merged.merged_from.is_some());

        // Unmerge on new VaultManager instance
        let unmerged = manager2.unmerge_notes(&merged.id).unwrap();
        assert_eq!(unmerged.primary.content, "First session notes.");
        assert_eq!(unmerged.secondary.content, "Second session notes.");
        assert_eq!(manager2.list_notes().unwrap().len(), 2);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_voice_note_invalid_and_corrupt_unmerge_failures() {
        let temp_dir = std::env::temp_dir().join(format!("relay_test_vn_err_{}", uuid::Uuid::new_v4()));
        let manager = VaultManager::new(temp_dir.clone());

        let note_normal = VaultNote::new_voice_note("Normal unmerged note.");
        manager.save_note(&note_normal).unwrap();

        // 1. Unmerging normal note should fail
        assert!(manager.unmerge_notes(&note_normal.id).is_err());

        // 2. Corrupted missing source history
        let note_a = VaultNote::new_voice_note("Note A");
        let note_b = VaultNote::new_voice_note("Note B");
        manager.save_note(&note_a).unwrap();
        manager.save_note(&note_b).unwrap();
        let merged = manager.merge_notes(&note_a.id, &note_b.id).unwrap();

        // Remove the stack file manually to simulate missing/corrupted stack
        let stack_path = temp_dir.join("merged_sources").join(format!("{}.json", merged.id));
        let _ = fs::remove_file(&stack_path);

        // Unmerge should fail safely without deleting active merged note
        assert!(manager.unmerge_notes(&merged.id).is_err());
        assert!(manager.get_note(&merged.id).is_ok());

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_voice_note_nested_merge_and_stepwise_unmerge() {
        let temp_dir = std::env::temp_dir().join(format!("relay_test_vn_nested_{}", uuid::Uuid::new_v4()));
        let manager = VaultManager::new(temp_dir.clone());

        let mut note_a = VaultNote::new_voice_note("Content A");
        note_a.created_at = "2026-08-20T10:00:00Z".to_string();
        let mut note_b = VaultNote::new_voice_note("Content B");
        note_b.created_at = "2026-08-20T10:01:00Z".to_string();
        let mut note_c = VaultNote::new_voice_note("Content C");
        note_c.created_at = "2026-08-20T10:02:00Z".to_string();

        manager.save_note(&note_a).unwrap();
        manager.save_note(&note_b).unwrap();
        manager.save_note(&note_c).unwrap();

        // A + B -> AB (id note_a.id)
        let merged_ab = manager.merge_notes(&note_a.id, &note_b.id).unwrap();
        assert_eq!(merged_ab.merged_from.as_ref().unwrap().len(), 2);

        // AB + C -> ABC (id note_a.id)
        let merged_abc = manager.merge_notes(&merged_ab.id, &note_c.id).unwrap();
        assert_eq!(merged_abc.merged_from.as_ref().unwrap().len(), 3);

        // First unmerge: ABC -> AB and C
        let unmerge1 = manager.unmerge_notes(&merged_abc.id).unwrap();
        assert_eq!(unmerge1.primary.id, note_a.id);
        assert_eq!(unmerge1.primary.merged_from.as_ref().unwrap().len(), 2);
        assert_eq!(unmerge1.secondary.id, note_c.id);
        assert_eq!(unmerge1.secondary.content, "Content C");

        // Second unmerge: AB -> A and B
        let unmerge2 = manager.unmerge_notes(&unmerge1.primary.id).unwrap();
        assert_eq!(unmerge2.primary.id, note_a.id);
        assert_eq!(unmerge2.primary.content, "Content A");
        assert!(unmerge2.primary.merged_from.is_none());

        assert_eq!(unmerge2.secondary.id, note_b.id);
        assert_eq!(unmerge2.secondary.content, "Content B");
        assert!(unmerge2.secondary.merged_from.is_none());

        // All 3 notes restored
        assert_eq!(manager.list_notes().unwrap().len(), 3);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_scribble_references_preserved_across_voice_note_merge_and_unmerge() {
        let temp_dir = std::env::temp_dir().join(format!("relay_test_scribble_unmerge_{}", uuid::Uuid::new_v4()));
        let manager = VaultManager::new(temp_dir.clone());

        // 1. Create Voice Note A and Scribble A
        let note_a = VaultNote::new_voice_note("Voice Note A transcript.");
        manager.save_note(&note_a).unwrap();
        let scribble_a = Scribble::from_voice_note(&note_a.id, &note_a.content, Some("Scribble A Title"));
        manager.save_scribble(&scribble_a).unwrap();

        // 2. Create Voice Note B and Scribble B
        let note_b = VaultNote::new_voice_note("Voice Note B transcript.");
        manager.save_note(&note_b).unwrap();
        let scribble_b = Scribble::from_voice_note(&note_b.id, &note_b.content, Some("Scribble B Title"));
        manager.save_scribble(&scribble_b).unwrap();

        assert_eq!(manager.list_scribbles().unwrap().len(), 2);

        // 3. Merge A + B -> AB
        let _merged_ab = manager.merge_notes(&note_a.id, &note_b.id).unwrap();
        let affected = manager.sync_scribbles_for_voice_note_merge(&note_a.id, &note_b.id).unwrap();
        assert_eq!(affected.len(), 1);

        // Active scribbles is now 1 (Scribble A updated), Scribble B moved to trash
        assert_eq!(manager.list_scribbles().unwrap().len(), 1);
        assert_eq!(manager.get_trash_items().unwrap().len(), 1);

        // 4. Unmerge AB -> A restored, B restored
        let unmerged = manager.unmerge_notes(&note_a.id).unwrap();
        assert_eq!(unmerged.primary.content, "Voice Note A transcript.");
        assert_eq!(unmerged.secondary.content, "Voice Note B transcript.");

        // 5. Verify Scribble A is restored to A's content and Scribble B is restored from trash
        let active_scribbles = manager.list_scribbles().unwrap();
        assert_eq!(active_scribbles.len(), 2);

        let restored_sa = manager.get_scribble(&scribble_a.id).unwrap();
        assert_eq!(restored_sa.content, "Voice Note A transcript.");
        let sa_vn_ids = restored_sa.source_metadata.get("source_voice_note_ids").unwrap().as_array().unwrap();
        assert_eq!(sa_vn_ids.len(), 1);
        assert_eq!(sa_vn_ids[0].as_str().unwrap(), note_a.id);

        let restored_sb = manager.get_scribble(&scribble_b.id).unwrap();
        assert_eq!(restored_sb.content, "Voice Note B transcript.");

        // 6. Test promoting a merged note directly to a Scribble
        let note_c = VaultNote::new_voice_note("Voice Note C transcript.");
        let note_d = VaultNote::new_voice_note("Voice Note D transcript.");
        manager.save_note(&note_c).unwrap();
        manager.save_note(&note_d).unwrap();

        let merged_cd = manager.merge_notes(&note_c.id, &note_d.id).unwrap();
        let scribble_cd = Scribble::from_voice_note(&merged_cd.id, &merged_cd.content, Some("Merged CD Scribble"));
        manager.save_scribble(&scribble_cd).unwrap();

        // Unmerge CD -> Scribble CD remains valid active scribble
        let _unmerged_cd = manager.unmerge_notes(&merged_cd.id).unwrap();
        let updated_scd = manager.get_scribble(&scribble_cd.id).unwrap();
        assert_eq!(updated_scd.content, "Voice Note C transcript.");

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_scribble_crud_and_relationships_in_vault() {
        let temp_dir = std::env::temp_dir().join(format!("relay_test_{}", uuid::Uuid::new_v4()));
        let manager = VaultManager::new(temp_dir.clone());

        // 1. Create text scribble
        let mut scribble1 = Scribble::new_text("Observation on local LLM speeds.", Some("LLM Latency"));
        scribble1.topics = vec!["AI".to_string(), "Performance".to_string()];
        scribble1.entities = vec!["Ollama".to_string(), "Relay".to_string()];
        manager.save_scribble(&scribble1).unwrap();

        // 2. Promote voice note to scribble
        let voice_note = VaultNote::new_voice_note("We should connect ideas automatically.");
        manager.save_note(&voice_note).unwrap();

        let mut scribble2 = Scribble::from_voice_note(&voice_note.id, &voice_note.content, Some("Idea Connections"));
        scribble2.topics = vec!["AI".to_string(), "Knowledge".to_string()];
        manager.save_scribble(&scribble2).unwrap();

        // 3. List scribbles
        let all_scribbles = manager.list_scribbles().unwrap();
        assert_eq!(all_scribbles.len(), 2);

        // 4. Add relationship between scribble1 and scribble2
        let rel = ScribbleRelationship {
            id: "rel_1_2".to_string(),
            target_id: scribble2.id.clone(),
            relationship_type: REL_RELATED_TO.to_string(),
            confidence: 0.95,
            source: "user".to_string(),
        };
        let updated1 = manager.add_scribble_relationship(&scribble1.id, rel.clone()).unwrap();
        assert_eq!(updated1.relationships.len(), 1);
        assert_eq!(updated1.relationships[0].target_id, scribble2.id);

        // 5. Knowledge Graph extraction
        let graph = manager.get_knowledge_graph(None).unwrap();
        assert!(graph.nodes.iter().any(|n| n.id == scribble1.id));
        assert!(graph.nodes.iter().any(|n| n.id == scribble2.id));
        assert!(graph.nodes.iter().any(|n| n.label == "AI")); // Topic node
        assert!(graph.nodes.iter().any(|n| n.label == "Ollama")); // Entity node

        // 6. Search knowledge
        let search_res = manager.search_knowledge("Latency").unwrap();
        assert_eq!(search_res.direct_matches.len(), 1);
        assert_eq!(search_res.direct_matches[0].id, scribble1.id);
        // scribble2 is related via rel_1_2 or topic AI
        assert!(search_res.related_scribbles.iter().any(|s| s.id == scribble2.id));

        // 7. Remove relationship
        let updated1_no_rel = manager.remove_scribble_relationship(&scribble1.id, "rel_1_2").unwrap();
        assert_eq!(updated1_no_rel.relationships.len(), 0);

        // 8. Delete scribble (direct removal)
        manager.delete_scribble(&scribble1.id).unwrap();
        assert_eq!(manager.list_scribbles().unwrap().len(), 1);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_trash_lifecycle_and_recovery() {
        let temp_dir = std::env::temp_dir().join(format!("relay_test_{}", uuid::Uuid::new_v4()));
        let manager = VaultManager::new(temp_dir.clone());

        // 1. Create a voice note, scribble, and meeting
        let voice_note = VaultNote::new_voice_note("Thought to be moved to trash.");
        manager.save_note(&voice_note).unwrap();

        let scribble = Scribble::new_text("Scribble to be soft deleted.", Some("Trash Test"));
        manager.save_scribble(&scribble).unwrap();

        let session_store = crate::meetings_v2::SessionStore::new(manager.vault_dir());
        let meeting_session = crate::meetings_v2::MeetingSession::new("meeting_trash_test".to_string(), Some("Sprint Retrospective".to_string()));
        session_store.save_session(&meeting_session).unwrap();

        assert_eq!(manager.list_notes().unwrap().len(), 1);
        assert_eq!(manager.list_scribbles().unwrap().len(), 1);
        assert_eq!(session_store.list_sessions().unwrap().len(), 1);

        // 2. Move scribble to trash
        let trash_scribble = manager.move_to_trash("scribble", &scribble.id).unwrap();
        assert_eq!(trash_scribble.item_type, "scribble");
        assert_eq!(trash_scribble.days_remaining(), 30);
        assert_eq!(manager.list_scribbles().unwrap().len(), 0);

        // 3. Move voice note to trash
        let trash_note = manager.move_to_trash("voice_note", &voice_note.id).unwrap();
        assert_eq!(trash_note.item_type, "voice_note");
        assert_eq!(manager.list_notes().unwrap().len(), 0);

        // 4. Move meeting to trash
        let trash_meeting = manager.move_to_trash("meeting", &meeting_session.id).unwrap();
        assert_eq!(trash_meeting.item_type, "meeting");
        assert_eq!(session_store.list_sessions().unwrap().len(), 0);

        // 5. List trash items
        let trash_items = manager.get_trash_items().unwrap();
        assert_eq!(trash_items.len(), 3);

        // 6. Restore meeting
        manager.restore_trash_item(&trash_meeting.id).unwrap();
        assert_eq!(session_store.list_sessions().unwrap().len(), 1);
        assert_eq!(manager.get_trash_items().unwrap().len(), 2);

        // 7. Restore voice note
        manager.restore_trash_item(&trash_note.id).unwrap();
        assert_eq!(manager.list_notes().unwrap().len(), 1);
        assert_eq!(manager.get_trash_items().unwrap().len(), 1);

        // 8. Permanently delete scribble
        manager.delete_trash_item_permanently(&trash_scribble.id).unwrap();
        assert_eq!(manager.get_trash_items().unwrap().len(), 0);
        assert_eq!(manager.list_scribbles().unwrap().len(), 0);

        // 9. Test Empty Trash
        let scribble2 = Scribble::new_text("Another to delete", Some("Empty Trash Test"));
        manager.save_scribble(&scribble2).unwrap();
        manager.move_to_trash("scribble", &scribble2.id).unwrap();
        assert_eq!(manager.get_trash_items().unwrap().len(), 1);

        let purged = manager.empty_trash().unwrap();
        assert_eq!(purged, 1);
        assert_eq!(manager.get_trash_items().unwrap().len(), 0);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_scribble_merge_preserves_provenance() {
        let temp_dir = std::env::temp_dir().join(format!("relay_test_{}", uuid::Uuid::new_v4()));
        let manager = VaultManager::new(temp_dir.clone());

        // 1. Create 2 scribbles
        let mut s1 = Scribble::new_text("First concept on local storage with RAG.", Some("Local Storage"));
        s1.topics = vec!["AI".to_string(), "Search".to_string()];
        s1.entities = vec!["Ollama".to_string()];
        manager.save_scribble(&s1).unwrap();

        let mut s2 = Scribble::new_text("Second concept on vector embeddings caching with LanceDB.", Some("Vector Embeddings"));
        s2.topics = vec!["AI".to_string(), "Performance".to_string()];
        s2.entities = vec!["LanceDB".to_string()];
        manager.save_scribble(&s2).unwrap();

        // 2. Merge s1 and s2
        let merged = manager.merge_scribbles(&[s1.id.clone(), s2.id.clone()]).unwrap();

        // 3. Verify merged attributes
        assert!(merged.content.contains("First concept on local storage"));
        assert!(merged.content.contains("Second concept on vector embeddings"));
        assert!(!merged.topics.is_empty());
        assert!(!merged.entities.is_empty());
        assert!(merged.entities.contains(&"LanceDB".to_string()) || merged.entities.contains(&"Ollama".to_string()));
        // Merge must NOT create graph edges to source scribbles
        assert_eq!(merged.relationships.len(), 0);

        // Verify source provenance metadata
        assert_eq!(merged.source_metadata.get("creation_method").unwrap().as_str().unwrap(), "merge");
        let source_ids = merged.source_metadata.get("source_scribble_ids").unwrap().as_array().unwrap();
        assert_eq!(source_ids.len(), 2);
        assert_eq!(source_ids[0].as_str().unwrap(), s1.id);
        assert_eq!(source_ids[1].as_str().unwrap(), s2.id);

        // Verify active list now has only the 1 merged scribble
        let active_scribbles = manager.list_scribbles().unwrap();
        assert_eq!(active_scribbles.len(), 1);
        assert_eq!(active_scribbles[0].id, merged.id);

        // Verify source scribbles are retired to Trash and recoverable
        let trash_items = manager.get_trash_items().unwrap();
        assert_eq!(trash_items.len(), 2);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_voice_note_merge_synchronizes_scribble() {
        let temp_dir = std::env::temp_dir().join(format!("relay_test_vn_merge_{}", uuid::Uuid::new_v4()));
        let manager = VaultManager::new(temp_dir.clone());

        // 1. Create Voice Note A and convert to Scribble A
        let note_a = VaultNote::new_voice_note("Voice Note A: Local-first architecture is critical.");
        manager.save_note(&note_a).unwrap();
        let scribble_a = Scribble::from_voice_note(&note_a.id, &note_a.content, None);
        manager.save_scribble(&scribble_a).unwrap();

        // 2. Create Voice Note B
        let note_b = VaultNote::new_voice_note("Voice Note B: Adding Google Calendar and cloud sync.");
        manager.save_note(&note_b).unwrap();

        // 3. Merge Note A and Note B
        let merged_note = manager.merge_notes(&note_a.id, &note_b.id).unwrap();
        assert!(merged_note.content.contains("Voice Note A"));
        assert!(merged_note.content.contains("Voice Note B"));

        // 4. Run sync_scribbles_for_voice_note_merge
        let affected = manager.sync_scribbles_for_voice_note_merge(&note_a.id, &note_b.id).unwrap();
        assert_eq!(affected.len(), 1);
        assert_eq!(affected[0], scribble_a.id);

        // 5. Verify Scribble A has been updated with merged content and provenance
        let updated_scribble = manager.get_scribble(&scribble_a.id).unwrap();
        assert!(updated_scribble.content.contains("Voice Note A"));
        assert!(updated_scribble.content.contains("Voice Note B"));
        assert!(updated_scribble.source_metadata.get("is_merged").unwrap().as_bool().unwrap());
        let vn_ids = updated_scribble.source_metadata.get("source_voice_note_ids").unwrap().as_array().unwrap();
        assert!(vn_ids.iter().any(|v| v.as_str().unwrap() == note_a.id));
        assert!(vn_ids.iter().any(|v| v.as_str().unwrap() == note_b.id));

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_voice_note_merge_consolidates_dual_scribbles() {
        let temp_dir = std::env::temp_dir().join(format!("relay_test_vn_dual_{}", uuid::Uuid::new_v4()));
        let manager = VaultManager::new(temp_dir.clone());

        // 1. Create Voice Note A and Scribble A
        let note_a = VaultNote::new_voice_note("Content A");
        manager.save_note(&note_a).unwrap();
        let scribble_a = Scribble::from_voice_note(&note_a.id, &note_a.content, None);
        manager.save_scribble(&scribble_a).unwrap();

        // 2. Create Voice Note B and Scribble B
        let note_b = VaultNote::new_voice_note("Content B");
        manager.save_note(&note_b).unwrap();
        let scribble_b = Scribble::from_voice_note(&note_b.id, &note_b.content, None);
        manager.save_scribble(&scribble_b).unwrap();

        assert_eq!(manager.list_scribbles().unwrap().len(), 2);

        // 3. Merge Note A and Note B
        let _ = manager.merge_notes(&note_a.id, &note_b.id).unwrap();

        // 4. Sync scribbles
        let affected = manager.sync_scribbles_for_voice_note_merge(&note_a.id, &note_b.id).unwrap();
        assert_eq!(affected.len(), 1);

        // 5. Active scribbles must now be 1, and the other retired to trash
        let active = manager.list_scribbles().unwrap();
        assert_eq!(active.len(), 1);
        let trash = manager.get_trash_items().unwrap();
        assert_eq!(trash.len(), 1);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_import_vault_file_lifecycle() {
        let temp_dir = std::env::temp_dir().join(format!("relay_test_files_{}", uuid::Uuid::new_v4()));
        let external_dir = std::env::temp_dir().join(format!("relay_test_ext_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&external_dir).unwrap();

        let external_file_path = external_dir.join("System_Architecture.md");
        let external_content = "# System Architecture\n\nRelay uses local-first storage with offline-first indexing.";
        fs::write(&external_file_path, external_content).unwrap();

        let manager = VaultManager::new(temp_dir.clone());

        // 1. Import file into Vault
        let imported = manager.import_vault_file(&external_file_path).unwrap();
        assert_eq!(imported.original_filename, "System_Architecture.md");
        assert_eq!(imported.file_type, "md");
        assert_eq!(imported.extraction_status, "extracted");
        assert!(imported.content.contains("System Architecture"));

        // 2. CRITICAL IMMUTABILITY GUARANTEE: Original external file must remain unchanged
        assert!(external_file_path.exists());
        let read_back_external = fs::read_to_string(&external_file_path).unwrap();
        assert_eq!(read_back_external, external_content);

        // 3. Vault copy must exist at vault path
        let vault_copy_full_path = temp_dir.join(&imported.vault_path);
        assert!(vault_copy_full_path.exists());

        // 4. Duplicate import test: Importing same file again returns existing VaultFile
        let re_imported = manager.import_vault_file(&external_file_path).unwrap();
        assert_eq!(re_imported.id, imported.id);

        // 5. Create Scribble from file
        let scribble = manager.create_scribble_from_file(&imported.id).unwrap();
        assert_eq!(scribble.source_type, "file");
        assert_eq!(
            scribble.source_metadata["source_file_id"].as_str().unwrap(),
            imported.id
        );

        // 6. Delete file to trash and restore
        manager.delete_vault_file(&imported.id).unwrap();
        assert!(manager.get_vault_file(&imported.id).is_err());
        // Original file outside Relay STILL exists!
        assert!(external_file_path.exists());

        let trash_items = manager.get_trash_items().unwrap();
        let trash_file_item = trash_items.iter().find(|t| t.original_id == imported.id).unwrap();
        manager.restore_trash_item(&trash_file_item.id).unwrap();

        let restored = manager.get_vault_file(&imported.id).unwrap();
        assert_eq!(restored.id, imported.id);

        let _ = fs::remove_dir_all(temp_dir);
        let _ = fs::remove_dir_all(external_dir);
    }
}
