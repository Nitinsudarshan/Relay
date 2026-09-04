//! Reading events from Google Calendar.
//!
//! Read-only by construction: the scope Relay requests is
//! `calendar.events.readonly`, so nothing here can create, move or delete an
//! event even if it tried. That is deliberate — a meeting assistant that can
//! write to your calendar is a different, scarier product.
//!
//! Everything this returns is **external content**, per `rules/security.md`.
//! Titles and descriptions are written by whoever sent the invitation, which is
//! frequently not the user. They are stored and displayed; where they reach a
//! model they go inside the same untrusted-source boundary as a transcript.

use super::model::{AttendanceResponse, CalendarAttendee, CalendarEvent};
use crate::oauth::{
    refresh_google_access_token, KeyringTokenStore, OAuthTokens, TokenNamespace,
};
use serde::Deserialize;
use std::path::Path;

const EVENTS_ENDPOINT: &str = "https://www.googleapis.com/calendar/v3/calendars/primary/events";

/// Seconds of remaining validity below which the access token is refreshed
/// rather than used. A token that expires mid-request fails the request.
const REFRESH_WHEN_WITHIN_SECONDS: i64 = 120;

/// The Google Calendar API's own shapes, kept private so nothing outside this
/// module depends on Google's field names.
#[derive(Debug, Deserialize)]
struct EventsResponse {
    #[serde(default)]
    items: Vec<ApiEvent>,
}

#[derive(Debug, Deserialize)]
struct ApiEvent {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    location: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    start: Option<ApiTime>,
    #[serde(default)]
    end: Option<ApiTime>,
    #[serde(default)]
    attendees: Vec<ApiAttendee>,
    #[serde(default)]
    organizer: Option<ApiOrganizer>,
    #[serde(default, rename = "hangoutLink")]
    hangout_link: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiTime {
    #[serde(default, rename = "dateTime")]
    date_time: Option<String>,
    /// Present instead of `dateTime` for all-day events.
    ///
    /// Never read: an entry with only a `date` is a holiday or a birthday, not
    /// a meeting somebody recorded, and [`convert`] drops it by requiring
    /// `date_time` on both bounds. Named here so the reason is visible at the
    /// shape rather than only at the filter.
    #[serde(default, rename = "date")]
    _all_day: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiAttendee {
    #[serde(default)]
    email: Option<String>,
    #[serde(default, rename = "displayName")]
    display_name: Option<String>,
    #[serde(default, rename = "responseStatus")]
    response_status: Option<String>,
    #[serde(default)]
    organizer: Option<bool>,
    #[serde(default, rename = "self")]
    is_self: Option<bool>,
    /// Rooms and equipment are invitees on the API and not people.
    #[serde(default)]
    resource: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct ApiOrganizer {
    #[serde(default, rename = "displayName")]
    display_name: Option<String>,
    #[serde(default)]
    email: Option<String>,
}

/// Converts one API event, or drops it.
///
/// Dropped: cancelled events, all-day entries, and anything without both
/// bounds. None of those is a meeting somebody recorded, and letting them
/// through only creates candidates that can be matched by mistake.
fn convert(event: ApiEvent) -> Option<CalendarEvent> {
    if event.status.as_deref() == Some("cancelled") {
        return None;
    }
    let starts_at = event.start.as_ref()?.date_time.clone()?;
    let ends_at = event.end.as_ref()?.date_time.clone()?;

    let attendees = event
        .attendees
        .into_iter()
        .filter(|a| a.resource != Some(true))
        .filter_map(|a| {
            let name = a
                .display_name
                .as_deref()
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(str::to_string)
                .or_else(|| a.email.as_deref().map(CalendarAttendee::name_from_email))?;

            Some(CalendarAttendee {
                name,
                email: a.email,
                response: AttendanceResponse::parse(a.response_status.as_deref().unwrap_or("")),
                is_organizer: a.organizer.unwrap_or(false),
                is_self: a.is_self.unwrap_or(false),
            })
        })
        .collect();

    Some(CalendarEvent {
        id: event.id.unwrap_or_default(),
        title: event
            .summary
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Untitled event".to_string()),
        starts_at,
        ends_at,
        description: event.description,
        location: event.location,
        attendees,
        conference_url: event.hangout_link,
        organizer: event
            .organizer
            .and_then(|o| o.display_name.or(o.email)),
    })
}

/// Whether a stored token needs refreshing before use.
pub fn needs_refresh(tokens: &OAuthTokens, now_unix: i64) -> bool {
    tokens.expires_at - now_unix <= REFRESH_WHEN_WITHIN_SECONDS
}

/// A usable access token, refreshing the stored one when it is close to expiry.
///
/// Returns a message naming the fix rather than a status code: the failure the
/// user will actually hit is a revoked grant, and "reconnect in Settings" is
/// what they need to read.
pub async fn access_token(config_dir: &Path) -> Result<String, String> {
    let tokens = KeyringTokenStore::load(config_dir, TokenNamespace::Calendar)
        .ok_or_else(|| "Google Calendar is not connected. Connect it in Settings › Meetings.".to_string())?;

    if !needs_refresh(&tokens, chrono::Utc::now().timestamp()) {
        return Ok(tokens.access_token);
    }

    let refresh_token = tokens.refresh_token.clone().ok_or_else(|| {
        "Google Calendar's access has expired and no refresh token was stored. Reconnect it \
in Settings › Meetings."
            .to_string()
    })?;

    let mut refreshed = refresh_google_access_token(None, None, &refresh_token)
        .await
        .map_err(|e| {
            format!("Google Calendar's access could not be renewed ({e}). Reconnect it in Settings › Meetings.")
        })?;

    // Google omits the refresh token on a refresh response; losing it here
    // would silently turn a working connection into one that fails at the next
    // expiry.
    if refreshed.refresh_token.is_none() {
        refreshed.refresh_token = Some(refresh_token);
    }
    refreshed.account_email = refreshed.account_email.or(tokens.account_email);
    refreshed.account_name = refreshed.account_name.or(tokens.account_name);

    KeyringTokenStore::save(config_dir, TokenNamespace::Calendar, &refreshed)?;
    Ok(refreshed.access_token)
}

/// Events overlapping a window, oldest first.
pub async fn events_between(
    config_dir: &Path,
    from: chrono::DateTime<chrono::Utc>,
    to: chrono::DateTime<chrono::Utc>,
) -> Result<Vec<CalendarEvent>, String> {
    let token = access_token(config_dir).await?;

    let response = reqwest::Client::new()
        .get(EVENTS_ENDPOINT)
        .bearer_auth(token)
        .query(&[
            ("timeMin", from.to_rfc3339()),
            ("timeMax", to.to_rfc3339()),
            ("singleEvents", "true".to_string()),
            ("orderBy", "startTime".to_string()),
            ("maxResults", "50".to_string()),
        ])
        .send()
        .await
        .map_err(|e| format!("Could not reach Google Calendar: {e}"))?;

    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(
            "Google Calendar rejected Relay's access. Reconnect it in Settings › Meetings."
                .to_string(),
        );
    }
    if !response.status().is_success() {
        return Err(format!(
            "Google Calendar returned {}. Nothing was changed.",
            response.status()
        ));
    }

    let parsed: EventsResponse = response
        .json()
        .await
        .map_err(|e| format!("Google Calendar's response could not be read: {e}"))?;

    Ok(parsed.items.into_iter().filter_map(convert).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn api_event(json: serde_json::Value) -> Option<CalendarEvent> {
        convert(serde_json::from_value(json).expect("fixture parses"))
    }

    #[test]
    fn a_normal_event_converts_with_its_people() {
        let converted = api_event(serde_json::json!({
            "id": "evt_1",
            "summary": "Placement review",
            "description": "Decide the launch date",
            "start": { "dateTime": "2026-09-04T09:30:00+05:30" },
            "end": { "dateTime": "2026-09-04T10:30:00+05:30" },
            "hangoutLink": "https://meet.google.com/abc-defg-hij",
            "organizer": { "displayName": "Pranjali Sharma" },
            "attendees": [
                { "displayName": "Pranjali Sharma", "email": "pranjali@example.com",
                  "responseStatus": "accepted", "organizer": true },
                { "email": "ayush.kumar@example.com", "responseStatus": "declined" },
                { "email": "nitin@navgurukul.org", "responseStatus": "accepted", "self": true }
            ]
        }))
        .expect("a scheduled meeting converts");

        assert_eq!(converted.title, "Placement review");
        assert_eq!(converted.attendees.len(), 3);
        assert_eq!(converted.organizer.as_deref(), Some("Pranjali Sharma"));
        assert_eq!(
            converted.conference_url.as_deref(),
            Some("https://meet.google.com/abc-defg-hij")
        );

        // A name is recovered where the calendar gave only an address.
        let ayush = &converted.attendees[1];
        assert_eq!(ayush.name, "Ayush Kumar");
        assert_eq!(ayush.response, AttendanceResponse::Declined);

        assert!(converted.attendees[2].is_self);
        assert_eq!(converted.likely_attendees().len(), 2, "the decline is excluded");
    }

    #[test]
    fn an_all_day_entry_is_not_a_meeting() {
        // Holidays and birthdays overlap every recording made that day, and
        // matching one would retitle the meeting.
        assert!(api_event(serde_json::json!({
            "id": "evt_holiday",
            "summary": "Public holiday",
            "start": { "date": "2026-09-04" },
            "end": { "date": "2026-09-05" }
        }))
        .is_none());
    }

    #[test]
    fn a_cancelled_event_is_dropped() {
        assert!(api_event(serde_json::json!({
            "id": "evt_dead",
            "summary": "Cancelled sync",
            "status": "cancelled",
            "start": { "dateTime": "2026-09-04T09:30:00Z" },
            "end": { "dateTime": "2026-09-04T10:30:00Z" }
        }))
        .is_none());
    }

    #[test]
    fn rooms_and_equipment_are_not_people() {
        let converted = api_event(serde_json::json!({
            "id": "evt_1",
            "summary": "Review",
            "start": { "dateTime": "2026-09-04T09:30:00Z" },
            "end": { "dateTime": "2026-09-04T10:30:00Z" },
            "attendees": [
                { "displayName": "Pranjali", "responseStatus": "accepted" },
                { "displayName": "Meeting Room 2", "email": "room2@resource.calendar.google.com",
                  "resource": true, "responseStatus": "accepted" }
            ]
        }))
        .unwrap();

        assert_eq!(converted.attendees.len(), 1);
        assert_eq!(converted.attendees[0].name, "Pranjali");
    }

    #[test]
    fn an_event_with_no_title_is_labelled_rather_than_left_blank() {
        let converted = api_event(serde_json::json!({
            "id": "evt_1",
            "start": { "dateTime": "2026-09-04T09:30:00Z" },
            "end": { "dateTime": "2026-09-04T10:30:00Z" }
        }))
        .unwrap();
        assert_eq!(converted.title, "Untitled event");
    }

    #[test]
    fn an_event_missing_a_bound_is_dropped_rather_than_guessed() {
        assert!(api_event(serde_json::json!({
            "id": "evt_1",
            "summary": "Half an event",
            "start": { "dateTime": "2026-09-04T09:30:00Z" }
        }))
        .is_none());
    }

    #[test]
    fn a_token_is_refreshed_before_it_expires_rather_than_after() {
        // A token that expires mid-request fails the request.
        let tokens = OAuthTokens {
            access_token: "a".into(),
            refresh_token: Some("r".into()),
            token_type: "Bearer".into(),
            expires_at: 1_000_000,
            scope: None,
            account_email: None,
            account_name: None,
            last_synced_at: None,
        };

        assert!(!needs_refresh(&tokens, 1_000_000 - 600), "still comfortably valid");
        assert!(needs_refresh(&tokens, 1_000_000 - 60), "about to expire");
        assert!(needs_refresh(&tokens, 1_000_000 + 10), "already expired");
    }
}
