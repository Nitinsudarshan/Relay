//! The clock behind the reminders.
//!
//! One background task, started once at launch. Every tick it asks the two
//! sources what they know, hands both to `recompute`, and raises whatever
//! became due. It owns no state of its own beyond a cache of the calendar.
//!
//! The two sources are polled at different rates on purpose. Window detection
//! is a local syscall and is cheap enough to run every tick; the calendar is a
//! network call against somebody's Google account and is refreshed far less
//! often. Reminder *timing* stays precise either way, because it is computed
//! from event start times rather than from when the fetch happened.

use super::{enqueue_to_show, recompute, take_next_to_show, ReminderEvent, ReminderInputs, ReminderQueue};
use crate::calendar::CalendarEvent;
use crate::commands::AppState;
use crate::oauth::{KeyringTokenStore, TokenNamespace};
use crate::sync::MutexExt;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

/// How often the queue is reconciled. Matches the granularity a person would
/// notice: a reminder five minutes out is not made better by being fifteen
/// seconds more punctual, and a tighter loop only spends battery.
const TICK: Duration = Duration::from_secs(15);

/// How often the calendar is re-read.
///
/// Events are fetched hours ahead, so a two-minute cache cannot make a reminder
/// late; it only delays noticing an invitation accepted moments ago.
const CALENDAR_REFRESH: Duration = Duration::from_secs(120);

/// How far ahead events are fetched. Wide enough that a refresh failure leaves
/// hours of usable cache behind it.
const CALENDAR_LOOKAHEAD_HOURS: i64 = 12;

/// How far back, so a meeting that started before Relay did still produces its
/// unrecorded reminder.
const CALENDAR_LOOKBEHIND_HOURS: i64 = 1;

/// Starts the reminder loop. Called once, from `setup`.
pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut events: Vec<CalendarEvent> = Vec::new();
        let mut last_calendar_read: Option<Instant> = None;
        // Reminders that are due but have not been on screen yet. One card
        // shows at a time, so anything that comes due behind the current one
        // waits here rather than being dropped.
        let mut backlog: VecDeque<ReminderEvent> = VecDeque::new();
        let mut ticker = tokio::time::interval(TICK);

        loop {
            ticker.tick().await;

            let state = app.state::<AppState>();
            let settings = state.settings.lock_or_recover().meetings.clone();

            // Nothing to compute when every kind is off — and no calendar call
            // to make either, which is the point: switching reminders off in
            // settings stops Relay reading the calendar for them.
            if !settings.remind_before_meeting
                && !settings.remind_if_unrecorded
                && !settings.remind_on_detection
            {
                continue;
            }

            let wants_calendar = settings.remind_before_meeting || settings.remind_if_unrecorded;
            let due_for_refresh = last_calendar_read
                .is_none_or(|read_at| read_at.elapsed() >= CALENDAR_REFRESH);

            if wants_calendar && due_for_refresh {
                let config_dir = state.config_dir.clone();
                if KeyringTokenStore::load(&config_dir, TokenNamespace::Calendar).is_some() {
                    let now = chrono::Utc::now();
                    match crate::calendar::google::events_between(
                        &config_dir,
                        now - chrono::Duration::hours(CALENDAR_LOOKBEHIND_HOURS),
                        now + chrono::Duration::hours(CALENDAR_LOOKAHEAD_HOURS),
                    )
                    .await
                    {
                        Ok(fetched) => {
                            events = fetched;
                            last_calendar_read = Some(Instant::now());
                        }
                        Err(e) => {
                            // Keep the previous events and try again next tick:
                            // a transient network failure must not silently
                            // stop reminding somebody about their day.
                            tracing::warn!("[reminders] calendar refresh failed: {e}");
                            last_calendar_read = Some(Instant::now());
                        }
                    }
                } else {
                    events.clear();
                    last_calendar_read = Some(Instant::now());
                }
            }

            let windows = if settings.remind_on_detection || settings.remind_if_unrecorded {
                // A syscall walking every top-level window, off the async
                // runtime's threads.
                tauri::async_runtime::spawn_blocking(super::detection::detect_active_conferencing_windows)
                    .await
                    .unwrap_or_default()
            } else {
                Vec::new()
            };

            let is_recording = state.meetings_v2.is_recording();
            // Recordings that already exist, so a meeting captured and stopped
            // early is not then reported as unrecorded.
            let sessions = state.meetings_v2.store().list_sessions().unwrap_or_default();
            let queue = app.state::<Arc<ReminderQueue>>();
            let (_, newly_fired) = recompute(
                &queue,
                &ReminderInputs {
                    events: &events,
                    windows: &windows,
                    settings: &settings,
                    is_recording,
                    sessions: &sessions,
                    now: chrono::Utc::now(),
                },
            );

            enqueue_to_show(&mut backlog, newly_fired);

            let notifications = app.state::<Arc<super::NotificationService>>();
            // One card at a time, and only once the previous one has been
            // answered or timed out — so a second meeting starting in the same
            // minute waits its turn instead of replacing the first.
            if notifications.showing().is_some() {
                continue;
            }
            if let Some(entry) = take_next_to_show(&queue, &mut backlog) {
                tracing::info!("[reminders] raising '{}' ({:?})", entry.title, entry.kind);
                notifications.show(&app, &entry, is_recording, &settings);
            }
        }
    });
}
