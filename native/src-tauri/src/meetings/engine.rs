use crate::commands::AppState;
use crate::meetings::calendar;
use crate::meetings::detect_active_conferencing_windows;
use crate::meetings::notification_service::NotificationService;
use crate::meetings::reminders::{self, ActiveMeetingRecording, ReminderQueue};
use crate::meetings::resolver::{self, CandidateStore};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager};

pub const MEETING_REMINDER_EVENT: &str = "meeting-reminder";

/// Starts the meetings background loop: manages state services, resolves
/// calendar and window signals through `resolver`, then reconciles the
/// reminder queue through `reminders::recompute_reminders` and routes
/// through `NotificationService`.
pub fn start(app: AppHandle) {
    app.manage(ReminderQueue::default());
    app.manage(ActiveMeetingRecording::default());
    app.manage(CandidateStore::default());
    app.manage(Arc::new(NotificationService::new()));

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
    let Some(notification_service) = app.try_state::<Arc<NotificationService>>() else { return };

    let settings = state.settings.lock().unwrap().clone();
    let meeting_settings = &settings.meetings;
    let vault_root = state.vault.vault_dir();

    // Signal resolution — keeping the meetings list current — always runs.
    if calendar::load_calendar_tokens(&vault_root).is_some() {
        if let Ok(events) = calendar::sync_real_google_calendar_events(&vault_root, false).await {
            for event in &events {
                let _ = resolver::resolve_calendar_signal(&state.vault, event);
            }
        }
    }

    let active_windows = detect_active_conferencing_windows();
    for window_match in &active_windows {
        let _ = resolver::resolve_window_signal(&state.vault, &candidates, window_match);
    }

    if !meeting_settings.remind_before_meeting
        && !meeting_settings.remind_if_unrecorded
        && !meeting_settings.remind_on_detection
    {
        return;
    }

    let recording_id = active_recording.0.lock().unwrap().clone();
    let (_, newly_fired) = reminders::recompute_reminders(
        &queue,
        &state.vault,
        meeting_settings,
        recording_id.as_deref(),
        &active_windows,
    );

    let is_capture_active = state.recorder.is_active();
    for entry in newly_fired {
        notification_service.show_reminder(
            app,
            &entry,
            is_capture_active,
            recording_id.as_deref(),
            meeting_settings,
        );
    }
}
