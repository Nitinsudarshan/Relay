use crate::meetings::{identify_meeting_provider, CalendarMeetingEvent};
use crate::oauth::{
    refresh_google_access_token, start_desktop_oauth_flow, KeyringTokenStore,
    OAuthTokens, TokenNamespace, SCOPE_CALENDAR_READONLY,
};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const GOOGLE_CALENDAR_EVENTS_ENDPOINT: &str =
    "https://www.googleapis.com/calendar/v3/calendars/primary/events";
const CONFIG_FILE_NAME: &str = "google_calendar_config.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GoogleCalendarConfig {
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}

pub type GoogleCalendarTokens = OAuthTokens;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CalendarConnectionState {
    NotConfigured,
    Disconnected,
    Authorizing,
    Connected,
    AuthError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarConnectionStatus {
    pub connected: bool,
    pub status: CalendarConnectionState,
    pub account_email: Option<String>,
    pub account_name: Option<String>,
    pub last_synced_at: Option<String>,
    pub error_message: Option<String>,
}

impl Default for CalendarConnectionStatus {
    fn default() -> Self {
        Self {
            connected: false,
            status: CalendarConnectionState::Disconnected,
            account_email: None,
            account_name: None,
            last_synced_at: None,
            error_message: None,
        }
    }
}

fn get_config_dir(vault_root: &Path) -> PathBuf {
    if let Some(parent) = vault_root.parent() {
        parent.join("config")
    } else {
        vault_root.to_path_buf()
    }
}

pub fn get_calendar_config_path(vault_root: &Path) -> PathBuf {
    get_config_dir(vault_root).join(CONFIG_FILE_NAME)
}

pub fn load_calendar_config(vault_root: &Path) -> GoogleCalendarConfig {
    let path = get_calendar_config_path(vault_root);
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(cfg) = serde_json::from_str::<GoogleCalendarConfig>(&content) {
                return cfg;
            }
        }
    }
    GoogleCalendarConfig::default()
}

pub fn save_calendar_config(vault_root: &Path, config: &GoogleCalendarConfig) -> Result<(), String> {
    let path = get_calendar_config_path(vault_root);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize calendar config: {}", e))?;
    fs::write(path, json)
        .map_err(|e| format!("Failed to write calendar config file: {}", e))?;
    Ok(())
}

pub fn load_calendar_tokens(vault_root: &Path) -> Option<GoogleCalendarTokens> {
    let config_dir = get_config_dir(vault_root);
    KeyringTokenStore::load(&config_dir, TokenNamespace::Calendar)
}

pub fn save_calendar_tokens(vault_root: &Path, tokens: &GoogleCalendarTokens) -> Result<(), String> {
    let config_dir = get_config_dir(vault_root);
    KeyringTokenStore::save(&config_dir, TokenNamespace::Calendar, tokens)
}

pub fn delete_calendar_tokens(vault_root: &Path) -> Result<(), String> {
    let config_dir = get_config_dir(vault_root);
    KeyringTokenStore::delete(&config_dir, TokenNamespace::Calendar)
}

pub fn get_calendar_connection_status(vault_root: &Path) -> CalendarConnectionStatus {
    let config = load_calendar_config(vault_root);
    let is_configured = crate::oauth::GoogleOAuthConfig::resolve_client_id(config.client_id).is_ok();

    if let Some(tokens) = load_calendar_tokens(vault_root) {
        CalendarConnectionStatus {
            connected: true,
            status: CalendarConnectionState::Connected,
            account_email: tokens.account_email,
            account_name: tokens.account_name,
            last_synced_at: tokens.last_synced_at,
            error_message: None,
        }
    } else if !is_configured {
        CalendarConnectionStatus {
            connected: false,
            status: CalendarConnectionState::NotConfigured,
            account_email: None,
            account_name: None,
            last_synced_at: None,
            error_message: Some("Google Calendar hasn't been configured for this Relay installation.".to_string()),
        }
    } else {
        CalendarConnectionStatus {
            connected: false,
            status: CalendarConnectionState::Disconnected,
            account_email: None,
            account_name: None,
            last_synced_at: None,
            error_message: None,
        }
    }
}

/// Initiates Google OAuth 2.0 PKCE authorization for Google Calendar (read-only events).
/// Uses centralized `crate::oauth` loopback flow.
pub async fn start_google_oauth_flow(
    vault_root: &Path,
    custom_client_id: Option<String>,
    custom_client_secret: Option<String>,
) -> Result<CalendarConnectionStatus, String> {
    let config = load_calendar_config(vault_root);
    let client_id = custom_client_id.or(config.client_id).filter(|s| !s.trim().is_empty());
    let client_secret = custom_client_secret.or(config.client_secret).filter(|s| !s.trim().is_empty());

    let result = start_desktop_oauth_flow(client_id, client_secret, SCOPE_CALENDAR_READONLY).await?;
    let mut tokens = result.tokens;
    if let Some(profile) = result.user_profile {
        tokens.account_email = Some(profile.email);
        tokens.account_name = profile.name;
    }
    tokens.last_synced_at = Some(Utc::now().to_rfc3339());

    save_calendar_tokens(vault_root, &tokens)?;

    Ok(CalendarConnectionStatus {
        connected: true,
        status: CalendarConnectionState::Connected,
        account_email: tokens.account_email,
        account_name: tokens.account_name,
        last_synced_at: tokens.last_synced_at,
        error_message: None,
    })
}

/// Refreshes the Google OAuth access token if expired or about to expire in < 60s.
pub async fn ensure_valid_access_token(
    vault_root: &Path,
) -> Result<String, String> {
    let mut tokens = load_calendar_tokens(vault_root)
        .ok_or_else(|| "Google Calendar is not connected".to_string())?;

    let now_ts = Utc::now().timestamp();
    if tokens.expires_at > now_ts + 60 {
        return Ok(tokens.access_token);
    }

    let refresh_token = tokens
        .refresh_token
        .as_ref()
        .ok_or_else(|| "No refresh token available. Please reconnect Google Calendar.".to_string())?;

    let config = load_calendar_config(vault_root);
    let client_id = config.client_id.filter(|s| !s.trim().is_empty());
    let client_secret = config.client_secret.filter(|s| !s.trim().is_empty());

    let refreshed = refresh_google_access_token(client_id, client_secret, refresh_token).await?;

    tokens.access_token = refreshed.access_token;
    tokens.expires_at = refreshed.expires_at;
    tokens.last_synced_at = Some(Utc::now().to_rfc3339());
    save_calendar_tokens(vault_root, &tokens)?;

    Ok(tokens.access_token)
}

/// Synchronizes real upcoming calendar events from Google Calendar API.
/// Queries primary calendar from now to +7 days, expanding recurring series instances.
pub async fn sync_real_google_calendar_events(
    vault_root: &Path,
) -> Result<Vec<CalendarMeetingEvent>, String> {
    if load_calendar_tokens(vault_root).is_none() {
        return Ok(Vec::new());
    }

    let access_token = ensure_valid_access_token(vault_root).await?;

    let now = Utc::now();
    let time_min = now.to_rfc3339();
    let time_max = (now + Duration::days(7)).to_rfc3339();

    let http = reqwest::Client::new();
    let mut response = http
        .get(GOOGLE_CALENDAR_EVENTS_ENDPOINT)
        .bearer_auth(&access_token)
        .query(&[
            ("timeMin", time_min.as_str()),
            ("timeMax", time_max.as_str()),
            ("singleEvents", "true"),
            ("orderBy", "startTime"),
            ("maxResults", "50"),
        ])
        .send()
        .await
        .map_err(|e| format!("Google Calendar events request failed: {}", e))?;

    // If 401 Unauthorized, attempt an immediate token refresh and retry once
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        if let Some(tokens) = load_calendar_tokens(vault_root) {
            if let Some(ref refresh_tok) = tokens.refresh_token {
                let config = load_calendar_config(vault_root);
                let client_id = config.client_id.filter(|s| !s.trim().is_empty());
                let client_secret = config.client_secret.filter(|s| !s.trim().is_empty());
                if let Ok(refreshed) = refresh_google_access_token(client_id, client_secret, refresh_tok).await {
                    let mut updated_tokens = tokens.clone();
                    updated_tokens.access_token = refreshed.access_token.clone();
                    updated_tokens.expires_at = refreshed.expires_at;
                    let _ = save_calendar_tokens(vault_root, &updated_tokens);

                    if let Ok(retry_resp) = http
                        .get(GOOGLE_CALENDAR_EVENTS_ENDPOINT)
                        .bearer_auth(&refreshed.access_token)
                        .query(&[
                            ("timeMin", time_min.as_str()),
                            ("timeMax", time_max.as_str()),
                            ("singleEvents", "true"),
                            ("orderBy", "startTime"),
                            ("maxResults", "50"),
                        ])
                        .send()
                        .await
                    {
                        response = retry_resp;
                    }
                }
            }
        }
    }

    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        // Purge invalid/stale tokens so UI prompts reconnect cleanly instead of staying in an error state
        let _ = delete_calendar_tokens(vault_root);
        return Err("Google Calendar credentials expired or invalid. Please click 'Connect Google Calendar' to re-authenticate.".to_string());
    }

    if !response.status().is_success() {
        let err = response.text().await.unwrap_or_default();
        return Err(format!("Google Calendar API error: {}", err));
    }

    let body_text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read Calendar response: {}", e))?;

    let events = parse_google_calendar_events_json(&body_text)?;

    if let Some(mut tokens) = load_calendar_tokens(vault_root) {
        tokens.last_synced_at = Some(Utc::now().to_rfc3339());
        let _ = save_calendar_tokens(vault_root, &tokens);
    }

    Ok(events)
}

/// Parses raw Google Calendar API JSON payload into Relay `CalendarMeetingEvent` items.
pub fn parse_google_calendar_events_json(
    json_str: &str,
) -> Result<Vec<CalendarMeetingEvent>, String> {
    #[derive(Deserialize)]
    struct GCalListResponse {
        #[serde(default)]
        items: Vec<GCalItem>,
    }

    #[derive(Deserialize)]
    struct GCalItem {
        id: String,
        summary: Option<String>,
        description: Option<String>,
        location: Option<String>,
        #[serde(rename = "hangoutLink")]
        hangout_link: Option<String>,
        #[serde(rename = "conferenceData")]
        conference_data: Option<GCalConferenceData>,
        start: Option<GCalTime>,
        end: Option<GCalTime>,
        #[serde(default)]
        attendees: Vec<GCalAttendee>,
        #[serde(rename = "recurringEventId")]
        recurring_event_id: Option<String>,
    }

    #[derive(Deserialize)]
    struct GCalConferenceData {
        #[serde(rename = "entryPoints")]
        #[serde(default)]
        entry_points: Vec<GCalEntryPoint>,
    }

    #[derive(Deserialize)]
    struct GCalEntryPoint {
        uri: Option<String>,
    }

    #[derive(Deserialize)]
    struct GCalTime {
        #[serde(rename = "dateTime")]
        date_time: Option<String>,
        date: Option<String>,
    }

    #[derive(Deserialize)]
    struct GCalAttendee {
        #[serde(rename = "displayName")]
        display_name: Option<String>,
        email: Option<String>,
    }

    let parsed: GCalListResponse = serde_json::from_str(json_str)
        .map_err(|e| format!("Failed to parse Google Calendar JSON items: {}", e))?;

    let mut result = Vec::new();

    for item in parsed.items {
        let title = item.summary.unwrap_or_else(|| "Untitled Meeting".to_string());

        let scheduled_start = item
            .start
            .and_then(|s| s.date_time.or_else(|| s.date.map(|d| format!("{}T00:00:00Z", d))))
            .unwrap_or_else(|| Utc::now().to_rfc3339());

        let scheduled_end = item
            .end
            .and_then(|e| e.date_time.or_else(|| e.date.map(|d| format!("{}T23:59:59Z", d))))
            .unwrap_or_else(|| (Utc::now() + Duration::minutes(30)).to_rfc3339());

        let mut meeting_url = item.hangout_link;
        if meeting_url.is_none() {
            if let Some(conf) = item.conference_data {
                meeting_url = conf.entry_points.into_iter().find_map(|ep| ep.uri);
            }
        }

        let combined_text = format!(
            "{}\n{}\n{}",
            item.description.as_deref().unwrap_or(""),
            item.location.as_deref().unwrap_or(""),
            meeting_url.as_deref().unwrap_or("")
        );

        let (provider, detected_url) = identify_meeting_provider(&combined_text);
        if meeting_url.is_none() {
            meeting_url = detected_url;
        }

        let participants: Vec<String> = item
            .attendees
            .into_iter()
            .map(|a| a.display_name.or(a.email).unwrap_or_default())
            .filter(|s| !s.trim().is_empty())
            .collect();

        result.push(CalendarMeetingEvent {
            id: item.id,
            title,
            provider,
            meeting_url,
            scheduled_start,
            scheduled_end,
            participants,
            recurrence_rule: item.recurring_event_id.as_ref().map(|_| "Recurring Series".to_string()),
            calendar_series_id: item.recurring_event_id,
        });
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meetings::PROVIDER_GOOGLE_MEET;

    #[test]
    fn test_parse_google_calendar_events_json() {
        let sample_json = r#"{
            "items": [
                {
                    "id": "gcal_event_101",
                    "summary": "Sprint Planning Sync",
                    "hangoutLink": "https://meet.google.com/abc-defg-hij",
                    "start": { "dateTime": "2026-08-25T10:00:00Z" },
                    "end": { "dateTime": "2026-08-25T11:00:00Z" },
                    "attendees": [
                        { "displayName": "Alex Rivera", "email": "alex@example.com" },
                        { "displayName": "Nitin Sudarshan", "email": "nitin@example.com" }
                    ],
                    "recurringEventId": "series_sprint_plan"
                },
                {
                    "id": "gcal_event_102",
                    "summary": "Candidate Tech Interview",
                    "description": "Please join Zoom session at https://zoom.us/j/9876543210 promptly.",
                    "start": { "dateTime": "2026-08-26T14:00:00Z" },
                    "end": { "dateTime": "2026-08-26T15:00:00Z" },
                    "attendees": [
                        { "displayName": "Candidate Jane", "email": "jane@example.com" }
                    ]
                }
            ]
        }"#;

        let events = parse_google_calendar_events_json(sample_json).expect("Should parse sample events");
        assert_eq!(events.len(), 2);

        assert_eq!(events[0].id, "gcal_event_101");
        assert_eq!(events[0].title, "Sprint Planning Sync");
        assert_eq!(events[0].provider, PROVIDER_GOOGLE_MEET);
        assert_eq!(events[0].meeting_url, Some("https://meet.google.com/abc-defg-hij".to_string()));
        assert_eq!(events[0].participants.len(), 2);
        assert_eq!(events[0].calendar_series_id, Some("series_sprint_plan".to_string()));

        assert_eq!(events[1].id, "gcal_event_102");
        assert_eq!(events[1].title, "Candidate Tech Interview");
        assert_eq!(events[1].provider, "zoom");
        assert_eq!(events[1].meeting_url, Some("https://zoom.us/j/9876543210".to_string()));
        assert_eq!(events[1].participants, vec!["Candidate Jane"]);
        assert_eq!(events[1].calendar_series_id, None);
    }

    #[test]
    fn test_calendar_connection_status_serialization() {
        let status = CalendarConnectionStatus {
            connected: true,
            status: CalendarConnectionState::Connected,
            account_email: Some("user@example.com".to_string()),
            account_name: Some("User Name".to_string()),
            last_synced_at: Some("2026-08-23T00:00:00Z".to_string()),
            error_message: None,
        };

        let json = serde_json::to_string(&status).expect("Serialization must succeed");
        assert!(json.contains(r#""status":"connected""#));
        assert!(json.contains(r#""connected":true"#));

        let auth_error = CalendarConnectionStatus {
            connected: false,
            status: CalendarConnectionState::AuthError,
            account_email: None,
            account_name: None,
            last_synced_at: None,
            error_message: Some("Authorization expired".to_string()),
        };

        let err_json = serde_json::to_string(&auth_error).expect("Serialization must succeed");
        assert!(err_json.contains(r#""status":"auth_error""#));
        assert!(err_json.contains(r#""connected":false"#));
    }

    #[test]
    fn test_calendar_status_not_configured_when_no_client_id() {
        let temp_dir = std::env::temp_dir().join(format!("relay_test_cal_{}", uuid::Uuid::new_v4()));
        let vault_root = temp_dir.join("vault");
        std::fs::create_dir_all(&vault_root).unwrap();

        // If no client ID configured in env or file, status is NotConfigured or Disconnected
        let status = get_calendar_connection_status(&vault_root);
        assert!(!status.connected);
        assert!(
            status.status == CalendarConnectionState::NotConfigured
                || status.status == CalendarConnectionState::Disconnected
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
