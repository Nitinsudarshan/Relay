use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

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
    vault_dir: PathBuf,
}

impl VaultManager {
    pub fn new(vault_dir: PathBuf) -> Self {
        Self { vault_dir }
    }

    pub fn init(&self) -> Result<(), VaultError> {
        fs::create_dir_all(self.vault_dir.join("notes"))?;
        fs::create_dir_all(self.vault_dir.join("kanban"))?;
        Ok(())
    }

    pub fn save_note(&self, note: &VaultNote) -> Result<PathBuf, VaultError> {
        self.init()?;
        let file_path = self.vault_dir.join("notes").join(format!("{}.md", note.id));
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
            .vault_dir
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

    pub fn list_notes(&self) -> Result<Vec<VaultNote>, VaultError> {
        self.init()?;
        let notes_dir = self.vault_dir.join("notes");
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
        let kanban_dir = self.vault_dir.join("kanban");
        let mut cards = Vec::new();

        if !kanban_dir.exists() {
            return Ok(cards);
        }

        for entry in fs::read_dir(kanban_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "md") {
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

        for line in frontmatter.lines() {
            let line = line.trim();
            if line.starts_with("id:") {
                id = line["id:".len()..].trim().trim_matches('"').to_string();
            } else if line.starts_with("title:") {
                title = line["title:".len()..].trim().trim_matches('"').to_string();
            } else if line.starts_with("assignee:") {
                assignee = line["assignee:".len()..]
                    .trim()
                    .trim_matches('"')
                    .to_string();
            } else if line.starts_with("status:") {
                status = line["status:".len()..].trim().trim_matches('"').to_string();
            } else if line.starts_with("priority:") {
                priority = line["priority:".len()..]
                    .trim()
                    .trim_matches('"')
                    .to_string();
            } else if line.starts_with("due_date:") {
                let d = line["due_date:".len()..]
                    .trim()
                    .trim_matches('"')
                    .to_string();
                if !d.is_empty() {
                    due_date = Some(d);
                }
            } else if line.starts_with("created_at:") {
                created_at = line["created_at:".len()..]
                    .trim()
                    .trim_matches('"')
                    .to_string();
            } else if line.starts_with("source_note_id:") {
                let s = line["source_note_id:".len()..]
                    .trim()
                    .trim_matches('"')
                    .to_string();
                if !s.is_empty() {
                    source_note_id = Some(s);
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
        assert_eq!(loaded_cards[0].id, "card_101");
        assert_eq!(loaded_cards[0].title, "Build Tauri Rust Shell");
        assert_eq!(loaded_cards[0].assignee, "Nitin");

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
}
