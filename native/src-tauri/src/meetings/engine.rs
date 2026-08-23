use crate::commands::AppState;
use crate::meetings::detect_active_conferencing_windows;
use crate::meetings::reminders::{self, ActiveMeetingRecording, ReminderQueue};
use crate::meetings::resolver::{self, CandidateStore};
use crate::meetings::calendar;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// One logical reminder, two coordinated surfaces (meetings_implementation.md
/// §4.2): this event is what both the OS notification and the interactive
/// popup render from, so neither can drift from the other.
pub const MEETING_REMINDER_EVENT: &str = "meeting-reminder";

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
        tracing::info!(
            "[notifications] Triggering custom meeting reminder overlay for '{}' ({:?})",
            entry.title,
            entry.kind
        );
        crate::overlay::ensure_reminder_window(app);
        let _ = app.emit(MEETING_REMINDER_EVENT, &entry);
    }
}
