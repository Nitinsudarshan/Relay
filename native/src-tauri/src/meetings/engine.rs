use crate::commands::AppState;
use crate::meetings::detect_active_conferencing_windows;
use crate::meetings::reminders::{self, ActiveMeetingRecording, ReminderKind, ReminderQueue};
use crate::meetings::resolver::{self, CandidateStore};
use crate::meetings::calendar;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

/// One logical reminder, two coordinated surfaces (meetings_implementation.md
/// §4.2): this event is what both the OS notification and the interactive
/// popup render from, so neither can drift from the other.
pub const MEETING_REMINDER_EVENT: &str = "meeting-reminder";

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

fn notification_copy(kind: ReminderKind, title: &str, provider: &str) -> (String, String) {
    match kind {
        ReminderKind::Upcoming => (
            format!("{} starts in 1 minute", title),
            format_provider(provider).to_string(),
        ),
        ReminderKind::Unrecorded => (
            "This meeting isn't being recorded".to_string(),
            format!("{} has been running a few minutes.", title),
        ),
        ReminderKind::Detected => (
            format!("{} meeting detected", format_provider(provider)),
            title.to_string(),
        ),
    }
}

/// Starts the meetings background loop: resolves calendar and window
/// signals through `resolver`, then reconciles the reminder queue through
/// `reminders::recompute_reminders`. This one interval is the clock;
/// business logic lives entirely in `resolver`/`reminders`, both unit
/// tested on their own. Calendar sync is safe to poll at this cadence —
/// `calendar.rs` already caches results for ~5 minutes internally, so this
/// doesn't add real API load beyond what existed before.
pub fn start(app: AppHandle) {
    app.manage(ReminderQueue::default());
    app.manage(ActiveMeetingRecording::default());
    app.manage(CandidateStore::default());

    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(15));
        loop {
            interval.tick().await;
            tick(&app).await;
        }
    });
}

async fn tick(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else { return };
    let Some(queue) = app.try_state::<ReminderQueue>() else { return };
    let Some(candidates) = app.try_state::<CandidateStore>() else { return };
    let Some(active_recording) = app.try_state::<ActiveMeetingRecording>() else { return };

    let settings = state.settings.lock().unwrap().clone();
    let meeting_settings = &settings.meetings;
    let vault_root = state.vault.vault_dir();

    // Signal resolution — keeping the meetings list current — always runs.
    // It's a separate concern from whether reminders are wanted: a user
    // who disables every reminder toggle still wants calendar/detected
    // meetings to show up in their list, just without the interruption.
    // A newly *created* meeting is emitted so an already-open meetings list
    // picks it up live (`useMeetingList.ts` listens for this event) instead
    // of only showing it after the user navigates away and back. Updates to
    // an existing record aren't emitted here — corroboration happens on
    // every tick, and re-emitting the same meeting 4x/minute would churn
    // the list for no visible benefit.
    if calendar::load_calendar_tokens(&vault_root).is_some() {
        if let Ok(events) = calendar::sync_real_google_calendar_events(&vault_root, false).await {
            for event in &events {
                if let Ok(resolved) = resolver::resolve_calendar_signal(&state.vault, event) {
                    emit_if_created(app, &resolved);
                }
            }
        }
    }

    for window_match in detect_active_conferencing_windows() {
        if let Ok(resolved) = resolver::resolve_window_signal(&state.vault, &candidates, &window_match) {
            emit_if_created(app, &resolved);
        }
    }

    if !meeting_settings.remind_before_meeting
        && !meeting_settings.remind_if_unrecorded
        && !meeting_settings.remind_on_detection
    {
        return;
    }

    let recording_id = active_recording.0.lock().unwrap().clone();
    let outcome =
        reminders::recompute_reminders(&queue, &state.vault, meeting_settings, recording_id.as_deref());

    // The OS notification is raised only for a genuine transition into
    // `Fired` — once per reminder, not every tick it stays up.
    for entry in &outcome.newly_fired {
        let (title, body) = notification_copy(entry.kind, &entry.title, &entry.provider);
        app.notification().builder().title(title).body(body).show().unwrap_or_default();
        crate::overlay::ensure_reminder_window(app);
    }

    // The popup, by contrast, is told to re-check on *any* status change —
    // including a reminder expiring or being resolved elsewhere — so it can
    // close itself instead of sitting there showing a stale reminder.
    if outcome.changed {
        let _ = app.emit(MEETING_REMINDER_EVENT, ());
    }
}

fn emit_if_created(app: &AppHandle, resolved: &resolver::ResolvedMeeting) {
    if resolved.was_created() {
        if let Some(meeting) = resolved.meeting() {
            let _ = app.emit(crate::commands::MEETING_UPDATED_EVENT, meeting);
        }
    }
}
