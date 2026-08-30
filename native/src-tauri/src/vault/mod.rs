use crate::sync::MutexExt;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use thiserror::Error;

/// Note type for Relay's universal dictation history — every successful
/// transcript, from any capture mode, regardless of whether text injection
/// also happened for it. Distinct from the LLM-cleaned "scribble"/"meeting"
/// note types, which remain unchanged.
pub const VOICE_NOTE_TYPE: &str = "voice_note";

pub mod scribble;
pub use scribble::*;

pub mod trash;
pub use trash::*;

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
        Ok(())
    }

    pub fn save_note(&self, note: &VaultNote) -> Result<PathBuf, VaultError> {
        self.init()?;
        let file_path = self
            .vault_dir()
            .join("notes")
            .join(format!("{}.md", note.id));
        let frontmatter = format!(
            "---\nid: \"{}\"\ntitle: \"{}\"\ntype: \"{}\"\ncreated_at: \"{}\"\nupdated_at: \"{}\"\ntags: {:?}\nsource_audio: {:?}\n---\n\n{}",
            note.id, note.title, note.note_type, note.created_at, note.updated_at, note.tags, note.source_audio, note.content
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
        let note1 = self.get_note(primary_id)?;
        let note2 = self.get_note(secondary_id)?;

        // Chronological order: older note content first, newer note content second
        let (older, newer) = if note1.created_at <= note2.created_at {
            (&note1, &note2)
        } else {
            (&note2, &note1)
        };

        let merged_content = format!("{}\n\n{}", older.content.trim(), newer.content.trim());

        let mut merged_note = note1;
        merged_note.content = merged_content;
        merged_note.updated_at = chrono::Utc::now().to_rfc3339();
        merged_note.title = crate::pipeline::extract_deterministic_title(&merged_note.content);

        self.save_note(&merged_note)?;
        self.delete_note(secondary_id)?;
        Ok(merged_note)
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

        scored.sort_by(|a, b| b.0.cmp(&a.0));
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
            tracing::info!("Deleted scribble {:?}", file_path);
        }
        Ok(())
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

        scored.sort_by(|a, b| b.0.cmp(&a.0));
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
        // keep the first/primary scribble and retire the redundant secondary scribbles to trash.
        let mut target_scribble = matching_scribbles.remove(0);

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
}
