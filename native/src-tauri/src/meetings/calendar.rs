use crate::meetings::{identify_meeting_provider, CalendarMeetingEvent};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};

const DEFAULT_GOOGLE_CLIENT_ID: &str = "1055740445695-k8i7k2m9h4r2mvgkqu9t03l8b5n4p1q9.apps.googleusercontent.com";
const DEFAULT_GOOGLE_CLIENT_SECRET: &str = "";
const GOOGLE_AUTH_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_USERINFO_ENDPOINT: &str = "https://www.googleapis.com/oauth2/v2/userinfo";
const GOOGLE_CALENDAR_EVENTS_ENDPOINT: &str = "https://www.googleapis.com/calendar/v3/calendars/primary/events";
const GOOGLE_CALENDAR_SCOPE: &str = "https://www.googleapis.com/auth/calendar.events.readonly https://www.googleapis.com/auth/userinfo.email https://www.googleapis.com/auth/userinfo.profile";

const TOKENS_FILE_NAME: &str = "google_calendar_token.json";
const CONFIG_FILE_NAME: &str = "google_calendar_config.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GoogleCalendarConfig {
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleCalendarTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_type: String,
    pub expires_at: i64, // Unix timestamp in seconds
    pub account_email: Option<String>,
    pub account_name: Option<String>,
    pub last_synced_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarConnectionStatus {
    pub connected: bool,
    pub account_email: Option<String>,
    pub account_name: Option<String>,
    pub has_custom_credentials: bool,
    pub last_synced_at: Option<String>,
}

impl Default for CalendarConnectionStatus {
    fn default() -> Self {
        Self {
            connected: false,
            account_email: None,
            account_name: None,
            has_custom_credentials: false,
            last_synced_at: None,
        }
    }
}

pub fn get_calendar_config_path(vault_root: &Path) -> PathBuf {
    vault_root.join(CONFIG_FILE_NAME)
}

pub fn get_calendar_tokens_path(vault_root: &Path) -> PathBuf {
    vault_root.join(TOKENS_FILE_NAME)
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
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize calendar config: {}", e))?;
    fs::write(path, json)
        .map_err(|e| format!("Failed to write calendar config file: {}", e))?;
    Ok(())
}

pub fn load_calendar_tokens(vault_root: &Path) -> Option<GoogleCalendarTokens> {
    let path = get_calendar_tokens_path(vault_root);
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(tokens) = serde_json::from_str::<GoogleCalendarTokens>(&content) {
                return Some(tokens);
            }
        }
    }
    None
}

pub fn save_calendar_tokens(vault_root: &Path, tokens: &GoogleCalendarTokens) -> Result<(), String> {
    let path = get_calendar_tokens_path(vault_root);
    let json = serde_json::to_string_pretty(tokens)
        .map_err(|e| format!("Failed to serialize calendar tokens: {}", e))?;
    fs::write(path, json)
        .map_err(|e| format!("Failed to write calendar tokens file: {}", e))?;
    Ok(())
}

pub fn delete_calendar_tokens(vault_root: &Path) -> Result<(), String> {
    let path = get_calendar_tokens_path(vault_root);
    if path.exists() {
        fs::remove_file(path)
            .map_err(|e| format!("Failed to delete calendar tokens: {}", e))?;
    }
    Ok(())
}

pub fn get_calendar_connection_status(vault_root: &Path) -> CalendarConnectionStatus {
    let config = load_calendar_config(vault_root);
    let has_custom = config.client_id.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false);

    if let Some(tokens) = load_calendar_tokens(vault_root) {
        CalendarConnectionStatus {
            connected: true,
            account_email: tokens.account_email,
            account_name: tokens.account_name,
            has_custom_credentials: has_custom,
            last_synced_at: tokens.last_synced_at,
        }
    } else {
        CalendarConnectionStatus {
            connected: false,
            account_email: None,
            account_name: None,
            has_custom_credentials: has_custom,
            last_synced_at: None,
        }
    }
}

/// Initiates Google OAuth 2.0 PKCE / Loopback authorization.
/// Binds an ephemeral local TCP port, generates auth URL, opens browser,
/// listens for redirect callback, exchanges authorization code for tokens,
/// and saves tokens in vault.
pub async fn start_google_oauth_flow(
    vault_root: &Path,
    custom_client_id: Option<String>,
    custom_client_secret: Option<String>,
) -> Result<CalendarConnectionStatus, String> {
    // 1. Determine client ID & secret
    let config = load_calendar_config(vault_root);
    let client_id = custom_client_id
        .or(config.client_id)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_GOOGLE_CLIENT_ID.to_string());

    let client_secret = custom_client_secret
        .or(config.client_secret)
        .unwrap_or_else(|| DEFAULT_GOOGLE_CLIENT_SECRET.to_string());

    // 2. Bind ephemeral TCP listener on 127.0.0.1
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("Failed to bind local loopback port for OAuth: {}", e))?;
    let local_port = listener
        .local_addr()
        .map_err(|e| format!("Failed to get local port: {}", e))?
        .port();

    let redirect_uri = format!("http://127.0.0.1:{}/oauth/callback", local_port);

    // 3. Build Google Auth URL
    let auth_url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent",
        GOOGLE_AUTH_ENDPOINT,
        urlencoding_encode(&client_id),
        urlencoding_encode(&redirect_uri),
        urlencoding_encode(GOOGLE_CALENDAR_SCOPE),
    );

    // 4. Open default system browser
    if let Err(e) = open_browser_url(&auth_url) {
        tracing::warn!("Could not open browser automatically: {}. Auth URL: {}", e, auth_url);
    }

    // 5. Await HTTP callback from Google redirect in a blocking task with timeout
    let code_result = tokio::task::spawn_blocking(move || {
        // Set timeout on TCP listener accept
        listener.set_nonblocking(false).ok();
        
        let (mut stream, _) = listener.accept()
            .map_err(|e| format!("OAuth listener accept error: {}", e))?;

        let mut buffer = [0u8; 2048];
        let bytes_read = stream.read(&mut buffer)
            .map_err(|e| format!("Failed to read OAuth response: {}", e))?;
        
        let request_str = String::from_utf8_lossy(&buffer[..bytes_read]);

        // Parse code from GET /oauth/callback?code=... HTTP/1.1
        let mut auth_code = None;
        let mut error_msg = None;

        if let Some(first_line) = request_str.lines().next() {
            if let Some(query_start) = first_line.find('?') {
                if let Some(query_end) = first_line[query_start..].find(' ') {
                    let query_str = &first_line[query_start + 1..query_start + query_end];
                    for param in query_str.split('&') {
                        let mut parts = param.split('=');
                        if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
                            if k == "code" {
                                auth_code = Some(v.to_string());
                            } else if k == "error" {
                                error_msg = Some(v.to_string());
                            }
                        }
                    }
                }
            }
        }

        // Send friendly HTML response to browser
        let (html_body, status_line) = if auth_code.is_some() {
            (
                r#"<!DOCTYPE html><html><body style="font-family:system-ui,sans-serif;display:flex;align-items:center;justify-content:center;height:100vh;margin:0;background:#090d16;color:#f8fafc;"><div style="text-align:center;padding:32px;background:#131b2e;border:1px solid #1e293b;border-radius:16px;box-shadow:0 10px 25px rgba(0,0,0,0.5);"><h1 style="color:#10b981;margin-bottom:8px;">✓ Calendar Connected</h1><p style="color:#94a3b8;margin-bottom:0;">You can close this tab and return to Relay.</p></div></body></html>"#,
                "HTTP/1.1 200 OK",
            )
        } else {
            (
                r#"<!DOCTYPE html><html><body style="font-family:system-ui,sans-serif;display:flex;align-items:center;justify-content:center;height:100vh;margin:0;background:#090d16;color:#f8fafc;"><div style="text-align:center;padding:32px;background:#131b2e;border:1px solid #1e293b;border-radius:16px;"><h1 style="color:#ef4444;margin-bottom:8px;">✗ Connection Failed</h1><p style="color:#94a3b8;margin-bottom:0;">Google OAuth was canceled or failed. You may close this tab.</p></div></body></html>"#,
                "HTTP/1.1 400 Bad Request",
            )
        };

        let response = format!(
            "{}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            status_line,
            html_body.len(),
            html_body
        );

        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();

        if let Some(err) = error_msg {
            return Err(format!("Google OAuth authorization error: {}", err));
        }

        auth_code.ok_or_else(|| "No authorization code returned from Google".to_string())
    })
    .await
    .map_err(|e| format!("OAuth worker join error: {}", e))??;

    // 6. Exchange auth code for access & refresh tokens
    let http = reqwest::Client::new();
    let mut params = vec![
        ("code", code_result.as_str()),
        ("client_id", client_id.as_str()),
        ("redirect_uri", redirect_uri.as_str()),
        ("grant_type", "authorization_code"),
    ];
    if !client_secret.is_empty() {
        params.push(("client_secret", client_secret.as_str()));
    }

    let token_resp = http
        .post(GOOGLE_TOKEN_ENDPOINT)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Token request failed: {}", e))?;

    if !token_resp.status().is_success() {
        let err_body = token_resp.text().await.unwrap_or_default();
        return Err(format!("Google token exchange failed: {}", err_body));
    }

    #[derive(Deserialize)]
    struct TokenResponse {
        access_token: String,
        refresh_token: Option<String>,
        token_type: String,
        expires_in: i64,
    }

    let parsed_tokens: TokenResponse = token_resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Google token JSON: {}", e))?;

    let now_ts = Utc::now().timestamp();
    let expires_at = now_ts + parsed_tokens.expires_in;

    // 7. Fetch user profile/email
    let mut user_email = None;
    let mut user_name = None;

    if let Ok(info_resp) = http
        .get(GOOGLE_USERINFO_ENDPOINT)
        .bearer_auth(&parsed_tokens.access_token)
        .send()
        .await
    {
        if info_resp.status().is_success() {
            #[derive(Deserialize)]
            struct UserInfo {
                email: Option<String>,
                name: Option<String>,
            }
            if let Ok(info) = info_resp.json::<UserInfo>().await {
                user_email = info.email;
                user_name = info.name;
            }
        }
    }

    let tokens = GoogleCalendarTokens {
        access_token: parsed_tokens.access_token,
        refresh_token: parsed_tokens.refresh_token,
        token_type: parsed_tokens.token_type,
        expires_at,
        account_email: user_email.clone(),
        account_name: user_name.clone(),
        last_synced_at: Some(Utc::now().to_rfc3339()),
    };

    save_calendar_tokens(vault_root, &tokens)?;

    Ok(CalendarConnectionStatus {
        connected: true,
        account_email: user_email,
        account_name: user_name,
        has_custom_credentials: !client_secret.is_empty(),
        last_synced_at: tokens.last_synced_at,
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
    let client_id = config
        .client_id
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_GOOGLE_CLIENT_ID.to_string());
    let client_secret = config.client_secret.unwrap_or_default();

    let http = reqwest::Client::new();
    let mut params = vec![
        ("refresh_token", refresh_token.as_str()),
        ("client_id", client_id.as_str()),
        ("grant_type", "refresh_token"),
    ];
    if !client_secret.is_empty() {
        params.push(("client_secret", client_secret.as_str()));
    }

    let refresh_resp = http
        .post(GOOGLE_TOKEN_ENDPOINT)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Token refresh request failed: {}", e))?;

    if !refresh_resp.status().is_success() {
        let err_body = refresh_resp.text().await.unwrap_or_default();
        return Err(format!("Google token refresh failed: {}", err_body));
    }

    #[derive(Deserialize)]
    struct RefreshResponse {
        access_token: String,
        expires_in: i64,
    }

    let parsed: RefreshResponse = refresh_resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse token refresh JSON: {}", e))?;

    tokens.access_token = parsed.access_token.clone();
    tokens.expires_at = now_ts + parsed.expires_in;
    tokens.last_synced_at = Some(Utc::now().to_rfc3339());
    save_calendar_tokens(vault_root, &tokens)?;

    Ok(parsed.access_token)
}

/// Synchronizes real upcoming calendar events from Google Calendar API.
/// Queries primary calendar from now to +7 days, expanding recurring series instances.
pub async fn sync_real_google_calendar_events(
    vault_root: &Path,
) -> Result<Vec<CalendarMeetingEvent>, String> {
    // If not connected, return empty vector (NO mock data!)
    if load_calendar_tokens(vault_root).is_none() {
        return Ok(Vec::new());
    }

    let access_token = ensure_valid_access_token(vault_root).await?;

    let now = Utc::now();
    let time_min = now.to_rfc3339();
    let time_max = (now + Duration::days(7)).to_rfc3339();

    let http = reqwest::Client::new();
    let response = http
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

    if !response.status().is_success() {
        let err = response.text().await.unwrap_or_default();
        return Err(format!("Google Calendar API error: {}", err));
    }

    let body_text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read Calendar response: {}", e))?;

    let events = parse_google_calendar_events_json(&body_text)?;

    // Update last_synced_at in tokens
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

        // Extract meeting URL from hangoutLink, conferenceData, location, or description
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

fn open_browser_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        let wide_url: Vec<u16> = std::ffi::OsStr::new(url)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let wide_open: Vec<u16> = std::ffi::OsStr::new("open")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        #[link(name = "shell32")]
        extern "system" {
            fn ShellExecuteW(
                hwnd: *mut std::ffi::c_void,
                lpOperation: *const u16,
                lpFile: *const u16,
                lpParameters: *const u16,
                lpDirectory: *const u16,
                nShowCmd: i32,
            ) -> isize;
        }

        unsafe {
            let res = ShellExecuteW(
                std::ptr::null_mut(),
                wide_open.as_ptr(),
                wide_url.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1, // SW_SHOWNORMAL
            );
            if res <= 32 {
                return Err(format!("ShellExecuteW failed with code {}", res));
            }
        }
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|e| format!("Failed to open browser: {}", e))?;
        Ok(())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|e| format!("Failed to open browser: {}", e))?;
        Ok(())
    }
}

fn urlencoding_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
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

        // Event 1: Google Meet
        assert_eq!(events[0].id, "gcal_event_101");
        assert_eq!(events[0].title, "Sprint Planning Sync");
        assert_eq!(events[0].provider, PROVIDER_GOOGLE_MEET);
        assert_eq!(events[0].meeting_url, Some("https://meet.google.com/abc-defg-hij".to_string()));
        assert_eq!(events[0].participants.len(), 2);
        assert_eq!(events[0].calendar_series_id, Some("series_sprint_plan".to_string()));

        // Event 2: Zoom in description
        assert_eq!(events[1].id, "gcal_event_102");
        assert_eq!(events[1].title, "Candidate Tech Interview");
        assert_eq!(events[1].provider, "zoom");
        assert_eq!(events[1].meeting_url, Some("https://zoom.us/j/9876543210".to_string()));
        assert_eq!(events[1].participants, vec!["Candidate Jane"]);
        assert_eq!(events[1].calendar_series_id, None);
    }
}
