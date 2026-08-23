use crate::commands::{AppState, CommandError};
use crate::meetings::{detect_active_conferencing_windows, calendar};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_notification::NotificationExt;
use serde::Serialize;

fn format_provider(p: &str) -> &str {
    match p {
        "google_meet" => "Google Meet",
        "zoom" => "Zoom",
        "teams" => "Teams",
        "webex" => "Webex",
        "in_person" => "In Person",
        _ => "Meeting",
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MeetingReminderPayload {
    pub meeting_id: String,
    pub title: String,
    pub provider: String,
    pub kind: String, // "upcoming" | "unrecorded" | "detected"
    pub participants: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ReminderState {
    pub last_upcoming_notified: Option<chrono::DateTime<Utc>>,
    pub last_unrecorded_notified: Option<chrono::DateTime<Utc>>,
    pub last_detected_notified: Option<chrono::DateTime<Utc>>,
    pub dismissed: bool,
}

impl Default for ReminderState {
    fn default() -> Self {
        Self {
            last_upcoming_notified: None,
            last_unrecorded_notified: None,
            last_detected_notified: None,
            dismissed: false,
        }
    }
}

pub type ReminderMap = Mutex<HashMap<String, ReminderState>>;

#[derive(Default)]
pub struct ActiveReminderState(pub Mutex<Option<MeetingReminderPayload>>);

pub fn start_scheduler(app: AppHandle) {
    app.manage(ReminderMap::new(HashMap::new()));
    app.manage(ActiveReminderState(Mutex::new(None)));
    let app_handle = app.clone();

    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(15));
        loop {
            interval.tick().await;

            let state = match app_handle.try_state::<AppState>() {
                Some(s) => s,
                None => continue,
            };

            let reminders = match app_handle.try_state::<ReminderMap>() {
                Some(r) => r,
                None => continue,
            };

            let settings = state.settings.lock().unwrap().clone();
            
            if !settings.meetings.remind_before_meeting && 
               !settings.meetings.remind_if_unrecorded && 
               !settings.meetings.remind_on_detection {
                continue;
            }

            let is_recording_meeting = state.recorder.is_active() && state.recorder.active_mode().as_deref() == Some("meeting");

            // T-1 min check (calendar)
            let upcoming_events = if calendar::load_calendar_tokens(&state.vault.vault_dir()).is_some() {
                calendar::sync_real_google_calendar_events(&state.vault.vault_dir(), false).await.unwrap_or_default()
            } else {
                Vec::new()
            };

            let now = Utc::now();

            if settings.meetings.remind_before_meeting {
                for ev in &upcoming_events {
                    if let Ok(start_dt) = chrono::DateTime::parse_from_rfc3339(&ev.scheduled_start) {
                        let start_utc = start_dt.with_timezone(&Utc);
                        let diff_secs = (start_utc - now).num_seconds();

                        if diff_secs > 0 && diff_secs <= 90 {
                            let mut map = reminders.lock().unwrap();
                            let rs = map.entry(ev.id.clone()).or_default();
                            
                            if !rs.dismissed && rs.last_upcoming_notified.is_none() {
                                rs.last_upcoming_notified = Some(now);
                                let payload = MeetingReminderPayload {
                                    meeting_id: ev.id.clone(),
                                    title: ev.title.clone(),
                                    provider: ev.provider.clone(),
                                    kind: "upcoming".to_string(),
                                    participants: ev.participants.clone(),
                                };
                                if let Some(active) = app_handle.try_state::<ActiveReminderState>() {
                                    *active.0.lock().unwrap() = Some(payload.clone());
                                }
                                app_handle.notification()
                                    .builder()
                                    .title(format!("{} starts in 1 minute", ev.title))
                                    .body(format!("{} • {} participants", format_provider(&ev.provider), ev.participants.len()))
                                    .show()
                                    .unwrap_or_default();
                                crate::overlay::ensure_reminder_window(&app_handle);
                                let _ = app_handle.emit("meeting-reminder", &payload);
                            }
                        }
                    }
                }
            }

            // Detect running meetings
            let raw_found = detect_active_conferencing_windows();
            for (provider, title, _) in raw_found {
                let matched_event = upcoming_events.iter().find(|ev| {
                    if let Ok(start_dt) = chrono::DateTime::parse_from_rfc3339(&ev.scheduled_start) {
                        let start_utc = start_dt.with_timezone(&Utc);
                        let diff_mins = (now - start_utc).num_minutes();
                        if diff_mins >= -5 && diff_mins <= 60 && (ev.provider == provider || ev.title.to_lowercase().contains(&title.to_lowercase()) || title.to_lowercase().contains(&ev.title.to_lowercase())) {
                            return true;
                        }
                    }
                    false
                });

                let (meeting_id, final_title, participants, kind) = if let Some(ev) = matched_event {
                    (ev.id.clone(), ev.title.clone(), ev.participants.clone(), "unrecorded")
                } else {
                    let id = format!("detected_{}_{}", provider, title.replace(' ', "_"));
                    (id, title.clone(), Vec::new(), "detected")
                };

                let mut map = reminders.lock().unwrap();
                let rs = map.entry(meeting_id.clone()).or_default();
                
                if rs.dismissed {
                    continue;
                }

                if kind == "unrecorded" {
                    if settings.meetings.remind_if_unrecorded && !is_recording_meeting {
                        let ev = matched_event.unwrap();
                        if let Ok(start_dt) = chrono::DateTime::parse_from_rfc3339(&ev.scheduled_start) {
                            let start_utc = start_dt.with_timezone(&Utc);
                            let diff_secs = (now - start_utc).num_seconds();

                            if diff_secs >= 120 && diff_secs <= 300 && rs.last_unrecorded_notified.is_none() {
                                rs.last_unrecorded_notified = Some(now);
                                let payload = MeetingReminderPayload {
                                    meeting_id,
                                    title: final_title.clone(),
                                    provider: provider.clone(),
                                    kind: "unrecorded".to_string(),
                                    participants,
                                };
                                if let Some(active) = app_handle.try_state::<ActiveReminderState>() {
                                    *active.0.lock().unwrap() = Some(payload.clone());
                                }
                                app_handle.notification()
                                    .builder()
                                    .title("This meeting isn't being recorded")
                                    .body(format!("{} has been running 2 minutes.", final_title))
                                    .show()
                                    .unwrap_or_default();
                                crate::overlay::ensure_reminder_window(&app_handle);
                                let _ = app_handle.emit("meeting-reminder", &payload);
                            }
                        }
                    }
                } else if kind == "detected" {
                    if settings.meetings.remind_on_detection && !is_recording_meeting {
                        if rs.last_detected_notified.is_none() {
                            rs.last_detected_notified = Some(now);
                            let payload = MeetingReminderPayload {
                                meeting_id,
                                title: final_title.clone(),
                                provider: provider.clone(),
                                kind: "detected".to_string(),
                                participants,
                            };
                            if let Some(active) = app_handle.try_state::<ActiveReminderState>() {
                                *active.0.lock().unwrap() = Some(payload.clone());
                            }
                            app_handle.notification()
                                .builder()
                                .title(format!("{} meeting detected", format_provider(&provider)))
                                .body("")
                                .show()
                                .unwrap_or_default();
                            crate::overlay::ensure_reminder_window(&app_handle);
                            let _ = app_handle.emit("meeting-reminder", &payload);
                        }
                    }
                }
            }
        }
    });
}

#[tauri::command]
pub async fn dismiss_meeting_reminder(
    meeting_id: String,
    permanent: bool,
    reminders: State<'_, ReminderMap>,
    active: State<'_, ActiveReminderState>,
) -> Result<(), CommandError> {
    *active.0.lock().unwrap() = None;
    if permanent {
        let mut map = reminders.lock().unwrap();
        let rs = map.entry(meeting_id).or_default();
        rs.dismissed = true;
    }
    Ok(())
}

#[tauri::command]
pub async fn start_recording_from_reminder(
    meeting_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
    reminders: State<'_, ReminderMap>,
    active: State<'_, ActiveReminderState>,
) -> Result<(), CommandError> {
    *active.0.lock().unwrap() = None;
    // Dismiss permanent
    {
        let mut map = reminders.lock().unwrap();
        let rs = map.entry(meeting_id.clone()).or_default();
        rs.dismissed = true;
    }
    
    // The main window handles tab-switching automatically when start_recording is called.
    let _ = app.emit("switch-to-meetings-tab", &meeting_id);
    
    // Unminimize and focus main window
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    
    // The actual start of recording is delegated to the frontend or we can start it here.
    // The instructions say: "The latter focuses the main window, emits a tab-switch event, and starts recording."
    // Let's call the `start_meeting_recording` command directly.
    let _ = crate::commands::start_meeting_recording(app.clone(), meeting_id, state).await?;
    Ok(())
}

#[tauri::command]
pub async fn get_active_meeting_reminder(
    active: State<'_, ActiveReminderState>,
) -> Result<Option<MeetingReminderPayload>, CommandError> {
    let payload = active.0.lock().unwrap().clone();
    Ok(payload)
}

#[tauri::command]
pub async fn trigger_mock_meeting_reminder(
    kind: String,
    app_handle: AppHandle,
    active: State<'_, ActiveReminderState>,
) -> Result<(), CommandError> {
    let title = match kind.as_str() {
        "upcoming" => "Weekly Engineering Sync",
        "unrecorded" => "Candidate Tech Interview",
        "detected" => "Ad-hoc Architecture Review",
        _ => "Mock Meeting",
    };

    let provider = match kind.as_str() {
        "upcoming" => "google_meet",
        "unrecorded" => "zoom",
        "detected" => "teams",
        _ => "other",
    };

    let payload = MeetingReminderPayload {
        meeting_id: format!("mock_{}", kind),
        title: title.to_string(),
        provider: provider.to_string(),
        kind: kind.clone(),
        participants: vec!["Alice".to_string(), "Bob".to_string(), "Charlie".to_string()],
    };

    *active.0.lock().unwrap() = Some(payload.clone());
    
    let notif_title = match kind.as_str() {
        "upcoming" => format!("{} starts in 1 minute", title),
        "unrecorded" => "This meeting isn't being recorded".to_string(),
        "detected" => format!("{} meeting detected", format_provider(provider)),
        _ => "Mock Meeting".to_string(),
    };
    
    let notif_body = match kind.as_str() {
        "upcoming" => format!("{} • {} participants", format_provider(provider), payload.participants.len()),
        "unrecorded" => format!("{} has been running 2 minutes.", title),
        "detected" => "".to_string(),
        _ => "".to_string(),
    };

    app_handle.notification()
        .builder()
        .title(notif_title)
        .body(notif_body)
        .show()
        .unwrap_or_default();
    
    crate::overlay::ensure_reminder_window(&app_handle);
    let _ = app_handle.emit("meeting-reminder", &payload);

    Ok(())
}
