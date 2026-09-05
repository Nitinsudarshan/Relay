//! Meeting reminder state machine.
//!
//! Implements the `Pending → Fired → {Snoozed | Dismissed | Actioned | Expired}`
//! lifecycle for meeting reminders. The queue is keyed by `(id, ReminderKind)`,
//! meaning one meeting can have multiple independent reminders and one reminder
//! can never overwrite another.
//!
//! Two reminder sources:
//!
//! * **Calendar events** (`CalendarEvent`) — Upcoming (fires ≤ 5 min before
//!   start) and Unrecorded (fires 2–15 min after start when no session covers
//!   the window).
//! * **Window detection** (`WindowMatch`) — Detected (fires when a conferencing
//!   window is found with no active recording).
//!
//! The authoritative function is [`recompute_reminders`]. It is called on every
//! engine tick. Only a genuine `Pending`/`Snoozed → Fired` transition returns
//! the entry in `newly_fired`; a tick that finds no new transitions returns an
//! empty `newly_fired`, so the caller emits exactly one OS notification per
//! logical state change, never on every poll.

use crate::meetings_v2::detection::WindowMatch;
use crate::settings::MeetingSettings;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Mutex;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Which kind of reminder this is. Each kind is independent: a meeting can
/// hold Upcoming AND Unrecorded reminders simultaneously.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReminderKind {
    /// The meeting is approaching — fires ≤ 5 min before scheduled start.
    Upcoming,
    /// The meeting started but no recording has been made.
    Unrecorded,
    /// A conferencing window was detected with no active recording.
    Detected,
}

/// `Pending → Fired → { Snoozed(until) | Dismissed | Actioned | Expired }`.
///
/// `Expired` is passive data — the fire window passed with no interaction —
/// not automatically an interruption on startup or wake. `is_still_actionable`
/// is the one narrow path where an expired Unrecorded reminder resurfaces.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ReminderStatus {
    Pending,
    Fired,
    Snoozed { until: DateTime<Utc> },
    Dismissed,
    Actioned,
    Expired,
}

/// A single entry in the reminder queue.
///
/// Keyed logically by `(id, kind)` — `id` is either a `CalendarEvent.id` for
/// calendar-backed reminders or a `MeetingSession.id` for session-backed ones.
/// The `provider` drives the notification copy.
#[derive(Debug, Clone, Serialize)]
pub struct ReminderEvent {
    /// The authoritative identity for this reminder:
    /// - calendar event ID for Upcoming / Unrecorded (calendar-backed)
    /// - session ID for Detected (window-backed, links to a running session)
    pub id: String,
    pub kind: ReminderKind,
    pub title: String,
    pub provider: String,
    pub participants: Vec<String>,
    pub fire_at: DateTime<Utc>,
    pub status: ReminderStatus,
}

/// Every currently tracked reminder, one entry per `(id, kind)`.
///
/// Never a single overwritable slot — using a keyed `Vec` is what prevents one
/// reminder from silently erasing another (the bug that motivated the original
/// rebuild). Tauri-managed state shared between the engine and the Tauri
/// commands.
#[derive(Default)]
pub struct ReminderQueue(pub Mutex<Vec<ReminderEvent>>);

/// Which meeting ID, if any, is currently being recorded. The `MeetingsV2Engine`
/// only knows session IDs, not calendar event IDs, so this tracks the *session*
/// ID. Set and cleared by `start_meeting_recording` / stop. Used by
/// `recompute_reminders` to suppress Detected reminders for the active session.
#[derive(Default)]
pub struct ActiveMeetingRecording(pub Mutex<Option<String>>);

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// How long a `Fired`/`Snoozed` reminder can go un-actioned before it is
/// treated as `Expired`.
const EXPIRE_AFTER_MINUTES: i64 = 15;

/// Upcoming reminder fires if within this many minutes of start.
const UPCOMING_WINDOW_MINUTES: i64 = 6;

/// Unrecorded reminder fires N minutes after start.
const UNRECORDED_FIRE_AFTER_MINUTES: i64 = 2;

/// Unrecorded reminder fires only within this window after start.
const UNRECORDED_FIRE_WITHIN_MINUTES: i64 = 15;

/// How long past a calendar event's scheduled end an Unrecorded reminder is
/// still worth actively resurfacing rather than staying as passive Expired data.
const ACTIONABLE_GRACE_MINUTES: i64 = 5;

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Whether an Unrecorded reminder for a calendar event is still worth
/// resurfacing. Only Unrecorded reminders are resurfaced — Upcoming and
/// Detected describe moments that have definitionally passed by the time
/// they expire.
fn is_still_actionable_calendar(
    entry: &ReminderEvent,
    ends_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> bool {
    entry.kind == ReminderKind::Unrecorded
        && ends_at
            .map(|end| (now - end).num_minutes() <= ACTIONABLE_GRACE_MINUTES)
            .unwrap_or(false)
}

/// Adds a reminder entry only if one for `(id, kind)` does not already exist.
/// This is the idempotency guard — once a `(id, kind)` is in the queue it is
/// only ever promoted or expired, never silently replaced.
fn ensure_entry(
    entries: &mut Vec<ReminderEvent>,
    id: &str,
    kind: ReminderKind,
    title: &str,
    provider: &str,
    participants: Vec<String>,
    fire_at: DateTime<Utc>,
) {
    if entries.iter().any(|e| e.id == id && e.kind == kind) {
        return;
    }
    entries.push(ReminderEvent {
        id: id.to_string(),
        kind,
        title: title.to_string(),
        provider: provider.to_string(),
        participants,
        fire_at,
        status: ReminderStatus::Pending,
    });
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Reconciles the reminder queue against current calendar events, recorded
/// sessions, and window detections.
///
/// Returns `(full_queue_snapshot, newly_fired_this_call)`. The second return
/// value is what the caller dispatches OS notifications for — only entries
/// that transitioned *into* `Fired` this call appear there, so repeated ticks
/// never produce repeated notifications.
///
/// # Arguments
///
/// * `queue` — shared mutable state (Tauri managed)
/// * `calendar_events` — upcoming calendar events fetched this tick (may be
///   empty when calendar is disconnected)
/// * `sessions` — all `MeetingSession` records (for Unrecorded detection)
/// * `window_matches` — currently visible conferencing windows
/// * `settings` — which reminder kinds are enabled
/// * `active_session_id` — the `MeetingSession.id` currently recording, if any;
///   suppresses Detected reminders for that session
pub fn recompute_reminders(
    queue: &ReminderQueue,
    calendar_events: &[crate::calendar::CalendarEvent],
    sessions: &[crate::meetings_v2::MeetingSession],
    window_matches: &[WindowMatch],
    settings: &MeetingSettings,
    active_session_id: Option<&str>,
) -> (Vec<ReminderEvent>, Vec<ReminderEvent>) {
    let now = Utc::now();
    let mut entries = queue.0.lock().unwrap();
    let mut newly_fired: Vec<ReminderEvent> = Vec::new();

    // --- 1. Populate new entries from calendar events ---

    for event in calendar_events {
        let starts = match event.starts() {
            Some(s) => s,
            None => continue,
        };
        let ends = event.ends();

        let attendee_names: Vec<String> = event
            .likely_attendees()
            .iter()
            .map(|a| a.name.clone())
            .collect();

        // Determine provider from conference URL or title.
        let provider = infer_provider_from_event(event);

        // Upcoming: fires if the event starts within the upcoming window.
        if settings.remind_before_meeting {
            let secs_until_start = (starts - now).num_seconds();
            if (0..=UPCOMING_WINDOW_MINUTES * 60).contains(&secs_until_start) {
                ensure_entry(
                    &mut entries,
                    &event.id,
                    ReminderKind::Upcoming,
                    &event.title,
                    &provider,
                    attendee_names.clone(),
                    now,
                );
            }
        }

        // Unrecorded: fires N minutes after start if no session covers this event.
        if settings.remind_if_unrecorded {
            let mins_since_start = (now - starts).num_minutes();
            if (UNRECORDED_FIRE_AFTER_MINUTES..=UNRECORDED_FIRE_WITHIN_MINUTES).contains(&mins_since_start) {
                // Only fire if no completed/active session overlaps this event.
                let covered = sessions.iter().any(|s| session_covers_event(s, starts, ends));
                if !covered {
                    ensure_entry(
                        &mut entries,
                        &event.id,
                        ReminderKind::Unrecorded,
                        &event.title,
                        &provider,
                        attendee_names.clone(),
                        now,
                    );
                }
            }
        }
    }

    // --- 2. Populate entries from window detections (Detected) ---

    if settings.remind_on_detection {
        for wm in window_matches {
            // Only fire for high-confidence matches.
            if wm.confidence < 0.75 {
                continue;
            }
            // Suppress if there is already an active recording session.
            if active_session_id.is_some() {
                continue;
            }
            // Use a synthetic key derived from the window title + provider
            // so the same real meeting window only fires once.
            let synthetic_id = format!("window::{}::{}", wm.provider, wm.title);
            ensure_entry(
                &mut entries,
                &synthetic_id,
                ReminderKind::Detected,
                &wm.title,
                &wm.provider,
                Vec::new(),
                now,
            );
        }
    }

    // --- 3. Advance state machine for every entry ---

    let calendar_ids: HashSet<&str> = calendar_events.iter().map(|e| e.id.as_str()).collect();

    for entry in entries.iter_mut() {
        let was_fired = matches!(entry.status, ReminderStatus::Fired);

        // Transitions: Pending or Snoozed → Fired.
        let due = match &entry.status {
            ReminderStatus::Pending => entry.fire_at <= now,
            ReminderStatus::Snoozed { until } => *until <= now,
            _ => false,
        };

        if due {
            entry.status = ReminderStatus::Fired;
        } else {
            // Expiry: a Fired entry not actioned within EXPIRE_AFTER_MINUTES
            // becomes Expired (passive data, not re-fired on wake).
            let stale = match &entry.status {
                ReminderStatus::Fired => {
                    (now - entry.fire_at).num_minutes() > EXPIRE_AFTER_MINUTES
                }
                ReminderStatus::Snoozed { until } => {
                    (now - *until).num_minutes() > EXPIRE_AFTER_MINUTES
                }
                _ => false,
            };
            if stale {
                // Only Unrecorded reminders for calendar events are ever
                // resurfaced — and only when the event is still actionable.
                let ends = if entry.kind == ReminderKind::Unrecorded
                    && calendar_ids.contains(entry.id.as_str())
                {
                    calendar_events
                        .iter()
                        .find(|e| e.id == entry.id)
                        .and_then(|e| e.ends())
                } else {
                    None
                };
                let resurface = is_still_actionable_calendar(entry, ends, now);
                entry.status = if resurface {
                    ReminderStatus::Fired
                } else {
                    ReminderStatus::Expired
                };
            }
        }

        // Collect newly fired (Pending/Snoozed → Fired transitions only).
        if !was_fired && matches!(entry.status, ReminderStatus::Fired) {
            newly_fired.push(entry.clone());
        }
    }

    // --- 4. Prune stale window-detection entries whose window is gone ---
    //
    // Window-detected entries use a synthetic key. If the window is no longer
    // visible (not in current window_matches), any Pending entry is removed.
    // Fired/Snoozed/Dismissed/Actioned/Expired entries are kept as history.
    let active_synthetic_ids: HashSet<String> = window_matches
        .iter()
        .map(|wm| format!("window::{}::{}", wm.provider, wm.title))
        .collect();

    entries.retain(|e| {
        if e.id.starts_with("window::") {
            // Keep unless it's still Pending and the window is gone.
            !matches!(e.status, ReminderStatus::Pending)
                || active_synthetic_ids.contains(&e.id)
        } else {
            // Calendar entries are retained as long as they're not fully
            // terminal. Expired/Dismissed/Actioned are kept as passive records.
            true
        }
    });

    (entries.clone(), newly_fired)
}

/// Whether a `MeetingSession` covers a calendar event's time window.
///
/// A session "covers" an event if it started before the event ended and
/// ended (or is still running) after the event started. Completed, Recording,
/// and Paused sessions all count.
fn session_covers_event(
    session: &crate::meetings_v2::MeetingSession,
    event_start: DateTime<Utc>,
    event_end: Option<DateTime<Utc>>,
) -> bool {
    use crate::meetings_v2::MeetingState;

    // Only active or completed sessions count.
    match session.state {
        MeetingState::Recording
        | MeetingState::Paused
        | MeetingState::Stopping
        | MeetingState::Finalizing
        | MeetingState::Completed
        | MeetingState::Recovered => {}
        _ => return false,
    }

    let session_start = session
        .started_at
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    let Some(ss) = session_start else {
        return false;
    };

    // Session must have started before event ends (or event has no end).
    let started_before_event_ends = event_end.map(|e| ss < e).unwrap_or(true);
    // Session must have started within a generous window of the event start.
    let close_to_event = (ss - event_start).num_minutes().abs() <= 30;

    started_before_event_ends && close_to_event
}

/// Infer the meeting provider from a `CalendarEvent`'s conference URL or title.
fn infer_provider_from_event(event: &crate::calendar::CalendarEvent) -> String {
    use crate::meetings_v2::detection::{identify_meeting_provider, PROVIDER_OTHER};

    if let Some(url) = &event.conference_url {
        let (provider, _) = identify_meeting_provider(url);
        if provider != PROVIDER_OTHER {
            return provider;
        }
    }
    // Fall back to title-based detection.
    let (provider, _) = identify_meeting_provider(&event.title);
    provider
}

// ---------------------------------------------------------------------------
// Mutation helpers (called by Tauri commands)
// ---------------------------------------------------------------------------

/// Mark a specific `(id, kind)` reminder as dismissed. A dismissed reminder
/// does not re-appear on engine ticks or application restart.
pub fn dismiss(queue: &ReminderQueue, id: &str, kind: ReminderKind) {
    let mut entries = queue.0.lock().unwrap();
    if let Some(e) = entries
        .iter_mut()
        .find(|e| e.id == id && e.kind == kind)
    {
        e.status = ReminderStatus::Dismissed;
    }
}

/// Snooze a specific `(id, kind)` reminder for `minutes`. The reminder
/// transitions `Fired → Snoozed` and will re-fire when `until` is reached.
pub fn snooze(queue: &ReminderQueue, id: &str, kind: ReminderKind, minutes: i64) {
    let mut entries = queue.0.lock().unwrap();
    if let Some(e) = entries
        .iter_mut()
        .find(|e| e.id == id && e.kind == kind)
    {
        e.status = ReminderStatus::Snoozed {
            until: Utc::now() + Duration::minutes(minutes),
        };
    }
}

/// Mark every actionable reminder for a given ID as `Actioned`. Called
/// when the user starts a recording, regardless of the entry point (list,
/// notification, tray). This keeps all surfaces in sync without any of them
/// needing to know about the others.
pub fn mark_meeting_actioned(queue: &ReminderQueue, id: &str) {
    let mut entries = queue.0.lock().unwrap();
    for e in entries.iter_mut().filter(|e| e.id == id) {
        if matches!(
            e.status,
            ReminderStatus::Pending | ReminderStatus::Fired | ReminderStatus::Snoozed { .. }
        ) {
            e.status = ReminderStatus::Actioned;
        }
    }
}

/// Developer / testing only: injects a pre-`Fired` reminder, bypassing all
/// timing and detection gates. Backs the `trigger_mock_meeting_reminder`
/// command so that the mock exercises the same `dispatch_native_reminder_notification`
/// path as a real reminder — not a synthetic frontend-only notification.
pub fn inject_mock_reminder(
    queue: &ReminderQueue,
    id: &str,
    kind: ReminderKind,
    title: &str,
    provider: &str,
) {
    let mut entries = queue.0.lock().unwrap();
    // Replace any existing entry for this (id, kind) so the mock is fresh.
    entries.retain(|e| !(e.id == id && e.kind == kind));
    entries.push(ReminderEvent {
        id: id.to_string(),
        kind,
        title: title.to_string(),
        provider: provider.to_string(),
        participants: Vec::new(),
        fire_at: Utc::now(),
        status: ReminderStatus::Fired,
    });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::model::CalendarEvent;
    use crate::meetings_v2::types::{MeetingSession, MeetingState};
    use crate::settings::MeetingSettings;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn all_on() -> MeetingSettings {
        MeetingSettings {
            remind_before_meeting: true,
            remind_if_unrecorded: true,
            remind_on_detection: true,
            ..Default::default()
        }
    }

    fn all_off() -> MeetingSettings {
        MeetingSettings {
            remind_before_meeting: false,
            remind_if_unrecorded: false,
            remind_on_detection: false,
            ..Default::default()
        }
    }

    fn fake_event(id: &str, starts_at: DateTime<Utc>, ends_at: DateTime<Utc>) -> CalendarEvent {
        CalendarEvent {
            id: id.to_string(),
            title: format!("Meeting {}", id),
            starts_at: starts_at.to_rfc3339(),
            ends_at: ends_at.to_rfc3339(),
            description: None,
            location: None,
            attendees: Vec::new(),
            conference_url: Some("https://zoom.us/j/12345".to_string()),
            organizer: None,
        }
    }

    fn upcoming_event(id: &str) -> CalendarEvent {
        let now = Utc::now();
        fake_event(
            id,
            now + Duration::minutes(3),
            now + Duration::minutes(63),
        )
    }

    fn just_started_event(id: &str) -> CalendarEvent {
        let now = Utc::now();
        fake_event(
            id,
            now - Duration::minutes(3),
            now + Duration::minutes(57),
        )
    }

    fn completed_session(started_offset_min: i64) -> MeetingSession {
        let now = Utc::now();
        let mut s = MeetingSession::new(
            format!("meet_{}", uuid::Uuid::new_v4()),
            Some("Test Recording".to_string()),
        );
        s.state = MeetingState::Completed;
        s.started_at = Some((now + Duration::minutes(started_offset_min)).to_rfc3339());
        s
    }

    // -----------------------------------------------------------------------
    // State machine correctness
    // -----------------------------------------------------------------------

    #[test]
    fn upcoming_reminder_fires_within_window() {
        let queue = ReminderQueue::default();
        let events = vec![upcoming_event("evt_1")];
        let settings = all_on();

        let (_, fired) = recompute_reminders(
            &queue,
            &events,
            &[],
            &[],
            &settings,
            None,
        );
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].id, "evt_1");
        assert_eq!(fired[0].kind, ReminderKind::Upcoming);
    }

    #[test]
    fn reminder_does_not_fire_twice_on_repeated_ticks() {
        let queue = ReminderQueue::default();
        let events = vec![upcoming_event("evt_2")];
        let settings = all_on();

        let (_, first) = recompute_reminders(&queue, &events, &[], &[], &settings, None);
        assert_eq!(first.len(), 1, "first tick fires");

        let (_, second) = recompute_reminders(&queue, &events, &[], &[], &settings, None);
        assert_eq!(second.len(), 0, "second tick must NOT re-fire");
    }

    #[test]
    fn snooze_cycle_fires_again_after_expiry() {
        let queue = ReminderQueue::default();
        let events = vec![upcoming_event("evt_3")];
        let settings = all_on();

        // First tick: fires.
        recompute_reminders(&queue, &events, &[], &[], &settings, None);

        // Snooze it — simulate past the snooze time by setting `until` in the past.
        {
            let mut entries = queue.0.lock().unwrap();
            let e = entries.iter_mut().find(|e| e.id == "evt_3").unwrap();
            e.status = ReminderStatus::Snoozed {
                until: Utc::now() - Duration::seconds(1),
            };
        }

        // Next tick: Snoozed → Fired again.
        let (_, refired) = recompute_reminders(&queue, &events, &[], &[], &settings, None);
        assert_eq!(refired.len(), 1, "snooze expired → should re-fire");
    }

    #[test]
    fn dismiss_is_permanent() {
        let queue = ReminderQueue::default();
        let events = vec![upcoming_event("evt_4")];
        let settings = all_on();

        // Fire it first.
        recompute_reminders(&queue, &events, &[], &[], &settings, None);

        // Dismiss.
        dismiss(&queue, "evt_4", ReminderKind::Upcoming);

        // Further ticks must not re-fire.
        let (_, refired) = recompute_reminders(&queue, &events, &[], &[], &settings, None);
        assert_eq!(refired.len(), 0);

        let state = {
            let entries = queue.0.lock().unwrap();
            entries
                .iter()
                .find(|e| e.id == "evt_4")
                .map(|e| matches!(e.status, ReminderStatus::Dismissed))
        };
        assert_eq!(state, Some(true));
    }

    #[test]
    fn mark_actioned_resolves_all_kinds_for_id() {
        let queue = ReminderQueue::default();
        let events = vec![upcoming_event("evt_5"), just_started_event("evt_5")];
        let settings = all_on();

        recompute_reminders(&queue, &events, &[], &[], &settings, None);
        mark_meeting_actioned(&queue, "evt_5");

        let entries = queue.0.lock().unwrap();
        for e in entries.iter().filter(|e| e.id == "evt_5") {
            assert!(
                matches!(e.status, ReminderStatus::Actioned),
                "kind {:?} should be Actioned",
                e.kind
            );
        }
    }

    #[test]
    fn multiple_meetings_are_isolated() {
        let queue = ReminderQueue::default();
        let events = vec![upcoming_event("evt_a"), upcoming_event("evt_b")];
        let settings = all_on();

        let (all, _) = recompute_reminders(&queue, &events, &[], &[], &settings, None);
        let ids: HashSet<&str> = all.iter().map(|e| e.id.as_str()).collect();
        assert!(ids.contains("evt_a"));
        assert!(ids.contains("evt_b"));

        // Dismiss one; the other must remain actionable.
        dismiss(&queue, "evt_a", ReminderKind::Upcoming);
        let (all2, _) = recompute_reminders(&queue, &events, &[], &[], &settings, None);
        let b_still_fired = all2
            .iter()
            .any(|e| e.id == "evt_b" && matches!(e.status, ReminderStatus::Fired));
        assert!(b_still_fired, "evt_b must be unaffected by evt_a dismissal");
    }

    #[test]
    fn unrecorded_fires_when_no_session_covers_event() {
        let queue = ReminderQueue::default();
        let events = vec![just_started_event("evt_6")];
        let settings = all_on();

        let (_, fired) = recompute_reminders(&queue, &events, &[], &[], &settings, None);
        assert!(
            fired.iter().any(|e| e.kind == ReminderKind::Unrecorded),
            "should fire Unrecorded when no session covers the event"
        );
    }

    #[test]
    fn unrecorded_suppressed_when_session_covers_event() {
        let queue = ReminderQueue::default();
        let events = vec![just_started_event("evt_7")];
        let settings = all_on();

        // A session that started ~3 min ago covers the event.
        let sessions = vec![completed_session(-3)];

        let (_, fired) =
            recompute_reminders(&queue, &events, &sessions, &[], &settings, None);
        let unrecorded_fired = fired.iter().any(|e| e.kind == ReminderKind::Unrecorded);
        assert!(!unrecorded_fired, "Unrecorded must not fire when a session covers the event");
    }

    #[test]
    fn settings_off_suppresses_upcoming() {
        let queue = ReminderQueue::default();
        let events = vec![upcoming_event("evt_8")];
        let mut settings = all_on();
        settings.remind_before_meeting = false;

        let (_, fired) = recompute_reminders(&queue, &events, &[], &[], &settings, None);
        assert!(
            fired.iter().all(|e| e.kind != ReminderKind::Upcoming),
            "Upcoming must not fire when remind_before_meeting is off"
        );
    }

    #[test]
    fn mock_injection_sets_fired_status() {
        let queue = ReminderQueue::default();
        inject_mock_reminder(&queue, "mock_id", ReminderKind::Upcoming, "Mock Meeting", "zoom");

        let entries = queue.0.lock().unwrap();
        let entry = entries.iter().find(|e| e.id == "mock_id").unwrap();
        assert!(matches!(entry.status, ReminderStatus::Fired));
    }

    #[test]
    fn second_reminder_does_not_overwrite_first() {
        let queue = ReminderQueue::default();
        let events = vec![
            upcoming_event("meeting_x"),
            upcoming_event("meeting_y"),
        ];
        let settings = all_on();

        let (all, _) = recompute_reminders(&queue, &events, &[], &[], &settings, None);
        // Both meetings must be independently tracked.
        assert!(all.iter().any(|e| e.id == "meeting_x"));
        assert!(all.iter().any(|e| e.id == "meeting_y"));
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn no_notification_on_pure_ticks_with_no_events() {
        let queue = ReminderQueue::default();
        let settings = all_on();

        for _ in 0..5 {
            let (_, fired) = recompute_reminders(&queue, &[], &[], &[], &settings, None);
            assert_eq!(fired.len(), 0, "no events → no notifications");
        }
    }

    #[test]
    fn disabled_settings_suppress_reminders() {
        let queue = ReminderQueue::default();
        let events = vec![upcoming_event("meeting_suppressed")];
        let settings = all_off();

        let (_, fired) = recompute_reminders(&queue, &events, &[], &[], &settings, None);
        assert_eq!(fired.len(), 0, "all reminders off -> suppressed");
    }
}
