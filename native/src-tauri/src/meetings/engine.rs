use crate::commands::AppState;
use crate::meetings::calendar;
use crate::meetings::detect_active_conferencing_windows;
use crate::meetings::reminders::{self, ActiveMeetingRecording, ReminderQueue};
use crate::meetings::resolver::{self, CandidateStore};
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

pub const MEETING_REMINDER_EVENT: &str = "meeting-reminder";

fn hash_meeting_id(meeting_id: &str) -> i32 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    meeting_id.hash(&mut hasher);
    (hasher.finish() & 0x7FFFFFFF) as i32
}

/// Dispatches a single native Windows OS notification for a fired meeting reminder.
pub fn dispatch_native_reminder_notification(app: &AppHandle, entry: &reminders::ReminderEvent) {
    let provider_name = match entry.provider.to_lowercase().as_str() {
        "google_meet" | "google meet" => "Google Meet",
        "zoom" => "Zoom",
        "teams" => "Teams",
        "webex" => "Webex",
        _ => "In Person",
    };

    let kind_label = match entry.kind {
        reminders::ReminderKind::Upcoming => "Starts in 5 minutes",
        reminders::ReminderKind::Unrecorded => "Meeting in progress",
        reminders::ReminderKind::Detected => "Meeting detected",
    };

    let body = format!("{} · {}", kind_label, provider_name);
    let notif_id = hash_meeting_id(&entry.meeting_id);

    tracing::info!(
        "[notifications] Emitting native OS notification for '{}' ({})",
        entry.title,
        body
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
        tracing::error!("Failed to show native Windows meeting notification: {}", e);
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
    if calendar::load_calendar_tokens(&vault_root).is_some() {
        if let Ok(events) = calendar::sync_real_google_calendar_events(&vault_root, false).await {
            for event in &events {
                let _ = resolver::resolve_calendar_signal(&state.vault, event);
            }
        }
    }

    for window_match in detect_active_conferencing_windows() {
        let _ = resolver::resolve_window_signal(&state.vault, &candidates, &window_match);
    }

    if !meeting_settings.remind_before_meeting
        && !meeting_settings.remind_if_unrecorded
        && !meeting_settings.remind_on_detection
    {
        return;
    }

    let recording_id = active_recording.0.lock().unwrap().clone();
    let (_, newly_fired) =
        reminders::recompute_reminders(&queue, &state.vault, meeting_settings, recording_id.as_deref());

    for entry in newly_fired {
        dispatch_native_reminder_notification(app, &entry);
    }
}
