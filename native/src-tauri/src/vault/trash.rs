use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrashItem {
    pub id: String,
    pub original_id: String,
    pub item_type: String, // "scribble" | "voice_note"
    pub title: String,
    pub snippet: String,
    pub deleted_at: String,
    pub expires_at: String, // deleted_at + 30 days
}

impl TrashItem {
    pub fn new(original_id: &str, item_type: &str, title: &str, snippet: &str) -> Self {
        let now = chrono::Utc::now();
        let expires = now + chrono::Duration::days(30);

        Self {
            id: format!("trash_{}_{}", item_type, original_id),
            original_id: original_id.to_string(),
            item_type: item_type.to_string(),
            title: title.to_string(),
            snippet: snippet.chars().take(200).collect(),
            deleted_at: now.to_rfc3339(),
            expires_at: expires.to_rfc3339(),
        }
    }

    pub fn is_expired(&self) -> bool {
        if let Ok(exp) = chrono::DateTime::parse_from_rfc3339(&self.expires_at) {
            chrono::Utc::now() > exp.with_timezone(&chrono::Utc)
        } else {
            false
        }
    }

    pub fn days_remaining(&self) -> i64 {
        if let Ok(exp) = chrono::DateTime::parse_from_rfc3339(&self.expires_at) {
            let diff = exp.with_timezone(&chrono::Utc) - chrono::Utc::now();
            let secs = diff.num_seconds();
            if secs <= 0 {
                0
            } else {
                (secs + 86399) / 86400
            }
        } else {
            30
        }
    }
}
