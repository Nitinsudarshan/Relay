//! Background reminder engine for meeting notifications.
//!
//! This module is the clock. It runs a 30-second poll loop that:
//!
//! 1. Fetches upcoming calendar events (next 2 hours).
//! 2. Lists all recorded `MeetingSession`s.
//! 3. Detects active conferencing windows (Windows only).
//! 4. Calls `reminders::recompute_reminders` — the pure, tested state machine.
//! 5. For each entry that transitioned into `Fired` this tick, emits exactly
//!    one native Windows OS notification via `dispatch_native_reminder_notification`.
//!
//! Business logic lives entirely in `reminders.rs` and is tested independently.
//! The engine tick is intentionally dumb: it resolves signals, passes them to
//! the state machine, and forwards newly-fired entries to the notification layer.
//!
//! # Duplicate-notification invariant
//!
//! Only a genuine `Pending` / `Snoozed → Fired` transition produces a `newly_fired`
//! entry. A tick that finds no new transitions returns an empty list, so the
//! OS notification is emitted exactly once per logical reminder, never on every
//! poll.

use crate::commands::AppState;
use crate::meetings_v2::detection::detect_active_conferencing_windows;
use crate::meetings_v2::reminders::{self, ActiveMeetingRecording, ReminderEvent, ReminderKind, ReminderQueue};
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

/// Tauri event name for the meeting reminder — kept for any future internal
/// listeners (e.g. UI surfaces that need to react to a fired reminder).
pub const MEETING_REMINDER_EVENT: &str = "meeting-reminder";

// ---------------------------------------------------------------------------
// Notification dispatch
// ---------------------------------------------------------------------------

/// Derives a deterministic, stable i32 notification ID from a string.
///
/// Windows notification APIs accept an integer ID that controls replacement —
/// the same ID replaces any prior notification with that ID. Using the meeting
/// event/session ID as the seed means one logical meeting never accumulates
/// an unbounded collection of independent OS notifications.
fn hash_id(id: &str) -> i32 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    id.hash(&mut hasher);
    (hasher.finish() & 0x7FFF_FFFF) as i32
}

/// Normalises a raw provider string to a human-readable label for notification
/// copy.
fn format_provider(provider: &str) -> &'static str {
    match provider.to_lowercase().as_str() {
        "google_meet" | "google meet" => "Google Meet",
        "zoom" => "Zoom",
        "teams" => "Teams",
        "webex" => "Webex",
        _ => "In Person",
    }
}

/// Dispatches a single native Windows OS notification for a fired meeting
/// reminder.
///
/// Notification shape:
/// ```text
/// Title: <meeting title>
/// Body:  <Starts in 5 minutes | Meeting in progress | Meeting detected> · <Provider>
/// ```
///
/// Action type `"meeting-reminder"` is registered by the frontend on startup;
/// it provides ▶ Record, ◷ Snooze 5m, ◷ Snooze 15m, and Dismiss buttons.
/// The notification ID is derived deterministically from `entry.id` so
/// repeated engine ticks never stack extra notifications for the same meeting.
pub fn dispatch_native_reminder_notification(app: &AppHandle, entry: &ReminderEvent) {
    let provider = format_provider(&entry.provider);

    let kind_label = match entry.kind {
        ReminderKind::Upcoming => "Starts in 5 minutes",
        ReminderKind::Unrecorded => "Meeting in progress",
        ReminderKind::Detected => "Meeting detected",
    };

    let body = format!("{} · {}", kind_label, provider);
    let notif_id = hash_id(&entry.id);

    tracing::info!(
        "[reminders] Emitting native OS notification: '{}' ({}) — id={}",
        entry.title,
        body,
        notif_id
    );

    if let Err(e) = app
        .notification()
        .builder()
        .id(notif_id)
        .title(&entry.title)
        .body(&body)
        .action_type_id("meeting-reminder")
        .show()
    {
        tracing::error!("[reminders] Failed to show native meeting notification: {}", e);
    }
}

// ---------------------------------------------------------------------------
// Engine start
// ---------------------------------------------------------------------------

/// Starts the meetings reminder background loop.
///
/// Manages `ReminderQueue` and `ActiveMeetingRecording` as Tauri-managed state
/// so both the engine tick and the Tauri commands share the same instances.
pub fn start(app: AppHandle) {
    app.manage(ReminderQueue::default());
    app.manage(ActiveMeetingRecording::default());

    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            tick(&app).await;
        }
    });
}

// ---------------------------------------------------------------------------
// Engine tick
// ---------------------------------------------------------------------------

async fn tick(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let Some(queue) = app.try_state::<ReminderQueue>() else {
        return;
    };
    let Some(active_recording) = app.try_state::<ActiveMeetingRecording>() else {
        return;
    };

    let settings = state.settings.lock().unwrap().clone();
    let meeting_settings = &settings.meetings;

    // --- 1. Fetch upcoming calendar events ---
    //
    // Signal resolution always runs — even when all reminder toggles are off —
    // so meetings appear in the list without requiring reminders to be on.
    // But the actual recompute below is gated on the toggles.
    let calendar_events: Vec<crate::calendar::CalendarEvent> = {
        use crate::oauth::{KeyringTokenStore, TokenNamespace};
        if KeyringTokenStore::load(&state.config_dir, TokenNamespace::Calendar).is_some() {
            let now = chrono::Utc::now();
            // Look ahead 2 hours so an Upcoming reminder for an event starting
            // in 1:58 is already tracked before the 5-minute fire window opens.
            let lookahead = now + chrono::Duration::hours(2);
            // Also look back 20 min to catch Unrecorded events that just started.
            let lookback = now - chrono::Duration::minutes(20);
            match crate::calendar::google::events_between(&state.config_dir, lookback, lookahead)
                .await
            {
                Ok(evts) => evts,
                Err(e) => {
                    tracing::warn!("[reminders] Calendar fetch failed: {}", e);
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        }
    };

    // --- 2. List all recorded sessions ---
    let sessions = state
        .meetings_v2
        .store()
        .list_sessions()
        .unwrap_or_default();

    // --- 3. Window detection (Windows-only; returns [] elsewhere) ---
    let window_matches = detect_active_conferencing_windows();

    // Skip the state-machine work when all reminders are off.
    if !meeting_settings.remind_before_meeting
        && !meeting_settings.remind_if_unrecorded
        && !meeting_settings.remind_on_detection
    {
        return;
    }

    // --- 4. Recompute reminder state machine ---
    let active_session_id = active_recording.0.lock().unwrap().clone();

    let (_, newly_fired) = reminders::recompute_reminders(
        &queue,
        &calendar_events,
        &sessions,
        &window_matches,
        meeting_settings,
        active_session_id.as_deref(),
    );

    // --- 5. Dispatch one notification per genuine Fired transition ---
    for entry in newly_fired {
        dispatch_native_reminder_notification(app, &entry);
    }
}
