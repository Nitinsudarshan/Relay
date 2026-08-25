use serde::{Deserialize, Serialize};

pub mod calendar;
pub mod detection;
pub mod engine;
pub mod notification_service;
pub mod reminders;
pub mod resolver;

pub use notification_service::{MeetingReminderPayload, NotificationService};

pub use detection::{
    clean_meeting_window_title, detect_active_conferencing_windows, identify_meeting_provider,
    WindowMatch,
};

pub const PROVIDER_GOOGLE_MEET: &str = "google_meet";
pub const PROVIDER_ZOOM: &str = "zoom";
pub const PROVIDER_TEAMS: &str = "teams";
pub const PROVIDER_WEBEX: &str = "webex";
pub const PROVIDER_IN_PERSON: &str = "in_person";
pub const PROVIDER_OTHER: &str = "other";

/// Represents an upcoming calendar meeting event (e.g. from Google Calendar or other providers).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CalendarMeetingEvent {
    pub id: String,
    pub title: String,
    pub provider: String,
    pub meeting_url: Option<String>,
    pub scheduled_start: String,
    pub scheduled_end: String,
    pub participants: Vec<String>,
    pub recurrence_rule: Option<String>,
    pub calendar_series_id: Option<String>,
}
