use serde::{Deserialize, Serialize};
use crate::vault::scribble::{Scribble, SOURCE_TYPE_MEETING, ScribbleAiMetadata};

pub const PROVIDER_GOOGLE_MEET: &str = "google_meet";
pub const PROVIDER_ZOOM: &str = "zoom";
pub const PROVIDER_TEAMS: &str = "teams";
pub const PROVIDER_WEBEX: &str = "webex";
pub const PROVIDER_IN_PERSON: &str = "in_person";
pub const PROVIDER_OTHER: &str = "other";

pub const MEETING_STATUS_SCHEDULED: &str = "scheduled";
pub const MEETING_STATUS_DETECTED: &str = "detected";
pub const MEETING_STATUS_RECORDING: &str = "recording";
pub const MEETING_STATUS_PROCESSING: &str = "processing";
pub const MEETING_STATUS_COMPLETED: &str = "completed";
pub const MEETING_STATUS_CANCELLED: &str = "cancelled";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MeetingActionItem {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub assignee: Option<String>,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default = "default_priority")]
    pub priority: String, // "high" | "medium" | "low"
    #[serde(default = "default_action_status")]
    pub status: String,   // "todo" | "done"
}

fn default_priority() -> String {
    "medium".to_string()
}

fn default_action_status() -> String {
    "todo".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MeetingSeries {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub calendar_series_id: Option<String>,
    #[serde(default)]
    pub recurrence_rule: Option<String>, // e.g. "Weekly on Mondays"
    pub created_at: String,
    pub updated_at: String,
}

impl MeetingSeries {
    pub fn new(title: &str, provider: Option<&str>, recurrence_rule: Option<&str>) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: format!("series_{}", uuid::Uuid::new_v4()),
            title: title.trim().to_string(),
            provider: provider.map(|p| p.trim().to_string()),
            calendar_series_id: None,
            recurrence_rule: recurrence_rule.map(|r| r.trim().to_string()),
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Meeting {
    pub id: String,
    #[serde(default)]
    pub series_id: Option<String>,
    pub title: String,
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default)]
    pub provider_metadata: serde_json::Value,
    #[serde(default)]
    pub calendar_event_id: Option<String>,
    #[serde(default)]
    pub scheduled_start: Option<String>,
    #[serde(default)]
    pub scheduled_end: Option<String>,
    #[serde(default)]
    pub actual_start: Option<String>,
    #[serde(default)]
    pub actual_end: Option<String>,
    #[serde(default = "default_meeting_status")]
    pub status: String,
    #[serde(default)]
    pub participants: Vec<String>,
    #[serde(default)]
    pub recording_path: Option<String>,
    #[serde(default)]
    pub transcript: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub decisions: Vec<String>,
    #[serde(default)]
    pub action_items: Vec<MeetingActionItem>,
    #[serde(default)]
    pub questions: Vec<String>,
    #[serde(default)]
    pub candidate_scribbles: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

fn default_provider() -> String {
    PROVIDER_OTHER.to_string()
}

fn default_meeting_status() -> String {
    MEETING_STATUS_SCHEDULED.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MeetingFrontmatter {
    pub id: String,
    #[serde(default)]
    pub series_id: Option<String>,
    pub title: String,
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default)]
    pub provider_metadata: serde_json::Value,
    #[serde(default)]
    pub calendar_event_id: Option<String>,
    #[serde(default)]
    pub scheduled_start: Option<String>,
    #[serde(default)]
    pub scheduled_end: Option<String>,
    #[serde(default)]
    pub actual_start: Option<String>,
    #[serde(default)]
    pub actual_end: Option<String>,
    #[serde(default = "default_meeting_status")]
    pub status: String,
    #[serde(default)]
    pub participants: Vec<String>,
    #[serde(default)]
    pub recording_path: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub decisions: Vec<String>,
    #[serde(default)]
    pub action_items: Vec<MeetingActionItem>,
    #[serde(default)]
    pub questions: Vec<String>,
    #[serde(default)]
    pub candidate_scribbles: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl Meeting {
    pub fn new(title: &str, provider: &str, series_id: Option<&str>) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: format!("meeting_{}", uuid::Uuid::new_v4()),
            series_id: series_id.map(|s| s.to_string()),
            title: title.trim().to_string(),
            provider: provider.to_string(),
            provider_metadata: serde_json::json!({}),
            calendar_event_id: None,
            scheduled_start: Some(now.clone()),
            scheduled_end: None,
            actual_start: None,
            actual_end: None,
            status: MEETING_STATUS_SCHEDULED.to_string(),
            participants: Vec::new(),
            recording_path: None,
            transcript: String::new(),
            notes: String::new(),
            summary: None,
            decisions: Vec::new(),
            action_items: Vec::new(),
            questions: Vec::new(),
            candidate_scribbles: Vec::new(),
            created_at: now.clone(),
            updated_at: now,
        }
    }

    /// Creates an atomic Scribble knowledge object from meeting content (notes, decision, action, or selection)
    /// while preserving source meeting provenance. The Meeting itself is NOT modified.
    pub fn create_scribble(
        &self,
        content: &str,
        custom_title: Option<&str>,
        segment: Option<&str>,
    ) -> Scribble {
        let now = chrono::Utc::now().to_rfc3339();
        let title = match custom_title {
            Some(t) if !t.trim().is_empty() => t.trim().to_string(),
            _ => {
                let first_line = content.lines().next().unwrap_or("Meeting Insight");
                let clean = first_line.trim_start_matches('#').trim();
                let words: Vec<&str> = clean.split_whitespace().take(6).collect();
                if !words.is_empty() {
                    words.join(" ")
                } else {
                    format!("Note from {}", self.title)
                }
            }
        };

        let mut source_metadata = serde_json::json!({
            "meeting_id": self.id,
            "meeting_title": self.title,
            "meeting_date": self.scheduled_start.as_deref().unwrap_or(&self.created_at),
            "provider": self.provider,
            "promoted_at": now,
        });

        if let Some(sid) = &self.series_id {
            source_metadata["meeting_series_id"] = serde_json::Value::String(sid.clone());
        }
        if let Some(seg) = segment {
            source_metadata["segment"] = serde_json::Value::String(seg.to_string());
        }

        Scribble {
            id: format!("scribble_{}", uuid::Uuid::new_v4()),
            title,
            content: content.to_string(),
            summary: None,
            source_type: SOURCE_TYPE_MEETING.to_string(),
            source_metadata,
            created_at: now.clone(),
            updated_at: now,
            tags: vec!["meeting".to_string()],
            topics: Vec::new(),
            entities: self.participants.clone(),
            relationships: Vec::new(),
            attachments: Vec::new(),
            status: "active".to_string(),
            ai_metadata: ScribbleAiMetadata {
                enrichment_status: "pending".to_string(),
                ..Default::default()
            },
        }
    }

    pub fn format_markdown(&self) -> String {
        let frontmatter_struct = MeetingFrontmatter {
            id: self.id.clone(),
            series_id: self.series_id.clone(),
            title: self.title.clone(),
            provider: self.provider.clone(),
            provider_metadata: self.provider_metadata.clone(),
            calendar_event_id: self.calendar_event_id.clone(),
            scheduled_start: self.scheduled_start.clone(),
            scheduled_end: self.scheduled_end.clone(),
            actual_start: self.actual_start.clone(),
            actual_end: self.actual_end.clone(),
            status: self.status.clone(),
            participants: self.participants.clone(),
            recording_path: self.recording_path.clone(),
            summary: self.summary.clone(),
            decisions: self.decisions.clone(),
            action_items: self.action_items.clone(),
            questions: self.questions.clone(),
            candidate_scribbles: self.candidate_scribbles.clone(),
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
        };

        let json_meta = serde_json::to_string_pretty(&frontmatter_struct)
            .unwrap_or_else(|_| "{}".to_string());

        let body = format!(
            "# Notes\n\n{}\n\n# Transcript\n\n{}",
            if self.notes.trim().is_empty() { "No meeting notes recorded yet." } else { self.notes.trim() },
            if self.transcript.trim().is_empty() { "No transcript recorded yet." } else { self.transcript.trim() }
        );

        format!("---\n{}\n---\n\n{}", json_meta, body)
    }

    pub fn parse_markdown(raw: &str) -> Option<Self> {
        let parts: Vec<&str> = raw.splitn(3, "---").collect();
        if parts.len() < 3 {
            return None;
        }

        let frontmatter_str = parts[1].trim();
        let body = parts[2].trim_start_matches('\n').to_string();

        let meta: MeetingFrontmatter = serde_json::from_str(frontmatter_str).ok()?;

        let mut notes = String::new();
        let mut transcript = String::new();

        if body.contains("# Notes") && body.contains("# Transcript") {
            let notes_part = body.split("# Transcript").next().unwrap_or("");
            let notes_clean = notes_part.replace("# Notes", "").trim().to_string();
            let trans_clean = body.split("# Transcript").nth(1).unwrap_or("").trim().to_string();

            if notes_clean != "No meeting notes recorded yet." {
                notes = notes_clean;
            }
            if trans_clean != "No transcript recorded yet." {
                transcript = trans_clean;
            }
        } else {
            notes = body;
        }

        Some(Meeting {
            id: meta.id,
            series_id: meta.series_id,
            title: meta.title,
            provider: meta.provider,
            provider_metadata: meta.provider_metadata,
            calendar_event_id: meta.calendar_event_id,
            scheduled_start: meta.scheduled_start,
            scheduled_end: meta.scheduled_end,
            actual_start: meta.actual_start,
            actual_end: meta.actual_end,
            status: meta.status,
            participants: meta.participants,
            recording_path: meta.recording_path,
            transcript,
            notes,
            summary: meta.summary,
            decisions: meta.decisions,
            action_items: meta.action_items,
            questions: meta.questions,
            candidate_scribbles: meta.candidate_scribbles,
            created_at: meta.created_at,
            updated_at: meta.updated_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meeting_markdown_roundtrip() {
        let mut meeting = Meeting::new("Quarterly Product Strategy", PROVIDER_GOOGLE_MEET, Some("series_prod_123"));
        meeting.participants = vec!["Nitin".to_string(), "Sarah".to_string(), "Alex".to_string()];
        meeting.notes = "Discussed Q4 roadmap and local AI architecture.".to_string();
        meeting.transcript = "Let's begin the meeting. The main topic is local STT and Knowledge Graph.".to_string();
        meeting.decisions = vec!["Adopt Whisper small model as default".to_string()];
        meeting.action_items = vec![MeetingActionItem {
            id: "act_1".to_string(),
            title: "Implement Google Calendar sync".to_string(),
            assignee: Some("Nitin".to_string()),
            due_date: Some("2026-08-25".to_string()),
            priority: "high".to_string(),
            status: "todo".to_string(),
        }];
        meeting.questions = vec!["What is the memory footprint on Windows?".to_string()];

        let md = meeting.format_markdown();
        let parsed = Meeting::parse_markdown(&md).expect("Should parse meeting markdown back");

        assert_eq!(parsed.id, meeting.id);
        assert_eq!(parsed.title, meeting.title);
        assert_eq!(parsed.provider, PROVIDER_GOOGLE_MEET);
        assert_eq!(parsed.series_id, Some("series_prod_123".to_string()));
        assert_eq!(parsed.participants.len(), 3);
        assert_eq!(parsed.decisions.len(), 1);
        assert_eq!(parsed.action_items.len(), 1);
        assert_eq!(parsed.action_items[0].title, "Implement Google Calendar sync");
        assert_eq!(parsed.questions.len(), 1);
        assert!(parsed.notes.contains("Q4 roadmap"));
        assert!(parsed.transcript.contains("local STT"));
    }

    #[test]
    fn test_create_scribble_from_meeting_preserves_provenance() {
        let meeting = Meeting::new("Architecture Sync", PROVIDER_ZOOM, Some("series_arch_456"));
        let scribble = meeting.create_scribble(
            "Relay must treat meetings as persistent first-class sources without destroying them when promoted.",
            Some("Meeting Persistence Rule"),
            Some("decision-1"),
        );

        assert_eq!(scribble.source_type, SOURCE_TYPE_MEETING);
        assert_eq!(scribble.title, "Meeting Persistence Rule");
        assert_eq!(scribble.source_metadata["meeting_id"], meeting.id);
        assert_eq!(scribble.source_metadata["meeting_series_id"], "series_arch_456");
        assert_eq!(scribble.source_metadata["provider"], PROVIDER_ZOOM);
        assert_eq!(scribble.source_metadata["segment"], "decision-1");
    }

    #[test]
    fn test_candidate_scribbles_metadata_only() {
        let mut meeting = Meeting::new("AI Roadmap", PROVIDER_TEAMS, None);
        meeting.candidate_scribbles = vec![
            "Local Whisper latency optimization technique".to_string(),
            "Vector hybrid retrieval strategy".to_string(),
        ];
        
        let md = meeting.format_markdown();
        let parsed = Meeting::parse_markdown(&md).expect("Should parse meeting markdown back");
        
        assert_eq!(parsed.candidate_scribbles.len(), 2);
        assert_eq!(parsed.candidate_scribbles[0], "Local Whisper latency optimization technique");
    }

    #[test]
    fn test_meeting_series_metadata() {
        let series = MeetingSeries::new("Weekly Architecture Forum", Some(PROVIDER_GOOGLE_MEET), Some("Weekly on Thursdays"));
        assert!(series.id.starts_with("series_"));
        assert_eq!(series.title, "Weekly Architecture Forum");
        assert_eq!(series.provider.as_deref(), Some(PROVIDER_GOOGLE_MEET));
        assert_eq!(series.recurrence_rule.as_deref(), Some("Weekly on Thursdays"));
    }
}

