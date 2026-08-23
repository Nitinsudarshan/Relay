use crate::meetings::resolver::{is_open, parse_rfc3339};
use crate::settings::MeetingSettings;
use crate::vault::{Meeting, VaultManager, MEETING_STATUS_RECORDING};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReminderKind {
    Upcoming,
    Unrecorded,
    Detected,
}

/// `Pending -> Fired -> { Snoozed(until) | Dismissed | Actioned | Expired }`.
/// `Expired` is passive data — the fire window passed with no interaction —
/// not automatically an interruption; see `is_still_actionable` for the one
/// case it's resurfaced.
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

#[derive(Debug, Clone, Serialize)]
pub struct ReminderEvent {
    pub meeting_id: String,
    pub kind: ReminderKind,
    pub title: String,
    pub provider: String,
    pub participants: Vec<String>,
    pub fire_at: DateTime<Utc>,
    pub status: ReminderStatus,
}

/// Every currently tracked reminder, one entry per (meeting, kind) — never
/// a single overwritable slot, which is what let one reminder erase
/// another in the removed `scheduler.rs`. Tauri-managed state.
#[derive(Default)]
pub struct ReminderQueue(pub Mutex<Vec<ReminderEvent>>);

/// What one `recompute_reminders` pass concluded. `changed` covers *any*
/// status transition (including into `Expired`), not just firings, so the
/// popup gets told to re-check and close itself rather than being left
/// showing a reminder that is no longer active.
#[derive(Debug, Clone)]
pub struct RecomputeOutcome {
    pub all: Vec<ReminderEvent>,
    pub newly_fired: Vec<ReminderEvent>,
    pub changed: bool,
}

/// Tracks which meeting ID, if any, the shared `AudioRecorder` is currently
/// recording *for* — the recorder itself only knows a generic mode string
/// ("meeting"), not a meeting ID, and per constraint 1 in
/// `meetings_implementation.md` it never will. This is a thin, meetings-
/// owned label sitting alongside the shared recorder, not inside it. Set
/// and cleared by `start_meeting_recording`/`stop_meeting_recording`.
#[derive(Default)]
pub struct ActiveMeetingRecording(pub Mutex<Option<String>>);

/// How long a `Fired`/`Snoozed` reminder can go un-actioned before it's
/// considered `Expired`.
const EXPIRE_AFTER_MINUTES: i64 = 10;
/// How long past a meeting's scheduled end an `Unrecorded` reminder is
/// still worth actively resurfacing, rather than staying a passive badge.
const ACTIONABLE_GRACE_MINUTES: i64 = 5;
/// A `Detected` reminder is only worth firing while the detection is
/// recent. The queue is in-memory, so it starts empty after a restart —
/// without this bound, any still-open meeting detected days ago would be
/// re-queued with its original `fire_at`, fire immediately, and pop the
/// notification again on every single restart.
const DETECTED_REMINDER_MAX_AGE_MINUTES: i64 = 15;

fn upcoming_fire_time(meeting: &Meeting, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let start = meeting.scheduled_start.as_deref().and_then(parse_rfc3339)?;
    let diff = (start - now).num_seconds();
    (diff > 0 && diff <= 90).then_some(now)
}

fn unrecorded_fire_time(meeting: &Meeting, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    if meeting.status == MEETING_STATUS_RECORDING {
        return None;
    }
    let start = meeting.scheduled_start.as_deref().and_then(parse_rfc3339)?;
    let diff = (now - start).num_seconds();
    (diff >= 120 && diff <= 300).then_some(now)
}

/// `None` once the detection is older than
/// `DETECTED_REMINDER_MAX_AGE_MINUTES`, which is what stops a long-open
/// detected meeting from re-notifying on every app restart.
fn detected_fire_time(meeting: &Meeting, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let detected_at = meeting.detected_at.as_deref().and_then(parse_rfc3339)?;
    ((now - detected_at).num_minutes() <= DETECTED_REMINDER_MAX_AGE_MINUTES).then_some(detected_at)
}

fn ensure_entry(entries: &mut Vec<ReminderEvent>, meeting: &Meeting, kind: ReminderKind, fire_at: DateTime<Utc>) {
    if entries.iter().any(|e| e.meeting_id == meeting.id && e.kind == kind) {
        return; // Already tracked — never overwritten, only added or promoted.
    }
    entries.push(ReminderEvent {
        meeting_id: meeting.id.clone(),
        kind,
        title: meeting.title.clone(),
        provider: meeting.provider.clone(),
        participants: meeting.participants.clone(),
        fire_at,
        status: ReminderStatus::Pending,
    });
}

/// Only an `Unrecorded` reminder for a meeting that hasn't reached its
/// scheduled end (or ended only moments ago) is worth actively resurfacing
/// once expired. An `Upcoming` or `Detected` reminder describes a moment
/// that has definitionally passed by the time it would expire, so neither
/// is ever resurfaced — showing "you missed a reminder" for something long
/// over can land more annoying than not reminding at all.
fn is_still_actionable(entry: &ReminderEvent, meeting: &Meeting, now: DateTime<Utc>) -> bool {
    entry.kind == ReminderKind::Unrecorded
        && meeting
            .scheduled_end
            .as_deref()
            .and_then(parse_rfc3339)
            .map(|end| (now - end).num_minutes() <= ACTIONABLE_GRACE_MINUTES)
            .unwrap_or(false)
}

/// Reconciles the reminder queue against what should exist right now,
/// given the meetings currently in the vault. Called reactively after a
/// signal loop (calendar sync, window detection) resolves a meeting record
/// — not on a fixed tick of its own; the caller's poll interval is the
/// clock, this function only reacts to it, and it's cheap enough to call
/// on every tick (a local vault read plus in-memory diffing, no network or
/// OS calls of its own).
///
/// Returns `RecomputeOutcome`: the full queue snapshot, the entries that
/// transitioned into `Fired` on *this* call (so the OS notification is
/// raised once per firing, not every tick), and whether anything at all
/// changed — the last of which is what lets the popup close itself when a
/// reminder expires or is resolved elsewhere, rather than sitting there
/// showing a reminder the backend no longer considers active.
pub fn recompute_reminders(
    queue: &ReminderQueue,
    vault: &VaultManager,
    settings: &MeetingSettings,
    currently_recording_meeting_id: Option<&str>,
) -> RecomputeOutcome {
    let meetings = vault.list_meetings().unwrap_or_default();
    let now = Utc::now();
    let mut entries = queue.0.lock().unwrap();
    let before: Vec<(String, ReminderKind, ReminderStatus)> = entries
        .iter()
        .map(|e| (e.meeting_id.clone(), e.kind, e.status.clone()))
        .collect();

    for meeting in meetings.iter().filter(|m| is_open(m)) {
        if currently_recording_meeting_id == Some(meeting.id.as_str()) {
            continue;
        }

        if settings.remind_before_meeting {
            if let Some(fire_at) = upcoming_fire_time(meeting, now) {
                ensure_entry(&mut entries, meeting, ReminderKind::Upcoming, fire_at);
            }
        }
        if settings.remind_if_unrecorded {
            if let Some(fire_at) = unrecorded_fire_time(meeting, now) {
                ensure_entry(&mut entries, meeting, ReminderKind::Unrecorded, fire_at);
            }
        }
        if settings.remind_on_detection && meeting.detection_source.as_deref() == Some("window_detector") {
            if let Some(fire_at) = detected_fire_time(meeting, now) {
                ensure_entry(&mut entries, meeting, ReminderKind::Detected, fire_at);
            }
        }
    }

    let mut newly_fired = Vec::new();

    for entry in entries.iter_mut() {
        let was_fired = matches!(entry.status, ReminderStatus::Fired);

        let due = match &entry.status {
            ReminderStatus::Pending => entry.fire_at <= now,
            ReminderStatus::Snoozed { until } => *until <= now,
            _ => false,
        };
        if due {
            entry.status = ReminderStatus::Fired;
        } else {
            let stale = match &entry.status {
                ReminderStatus::Fired => (now - entry.fire_at).num_minutes() > EXPIRE_AFTER_MINUTES,
                ReminderStatus::Snoozed { until } => (now - *until).num_minutes() > EXPIRE_AFTER_MINUTES,
                _ => false,
            };
            if stale {
                let meeting = meetings.iter().find(|m| m.id == entry.meeting_id);
                let resurface = meeting.map(|m| is_still_actionable(entry, m, now)).unwrap_or(false);
                entry.status = if resurface { ReminderStatus::Fired } else { ReminderStatus::Expired };
            }
        }

        if !was_fired && matches!(entry.status, ReminderStatus::Fired) {
            newly_fired.push(entry.clone());
        }
    }

    // Drop reminders for meetings that are no longer open (completed,
    // cancelled, or deleted) — nothing left to remind anyone about.
    let open_ids: HashSet<&str> = meetings.iter().filter(|m| is_open(m)).map(|m| m.id.as_str()).collect();
    entries.retain(|e| open_ids.contains(e.meeting_id.as_str()));

    let after: Vec<(String, ReminderKind, ReminderStatus)> = entries
        .iter()
        .map(|e| (e.meeting_id.clone(), e.kind, e.status.clone()))
        .collect();

    RecomputeOutcome {
        changed: before != after,
        all: entries.clone(),
        newly_fired,
    }
}

/// The single reminder the popup should currently show, if any — the
/// earliest-firing `Fired` entry. There is deliberately no separate "active
/// reminder" slot distinct from the queue itself: deriving it from the
/// queue is what prevents the "second reminder silently replaces the
/// first" bug from having anywhere to reappear.
pub fn current_popup_reminder(queue: &ReminderQueue) -> Option<ReminderEvent> {
    let entries = queue.0.lock().unwrap();
    entries
        .iter()
        .filter(|e| matches!(e.status, ReminderStatus::Fired))
        .min_by_key(|e| e.fire_at)
        .cloned()
}

/// Which meeting the tray's "Start Recording" item should act on: the
/// reminder currently on screen if there is one, otherwise the meeting
/// that's actually happening now (or starting imminently). Without the
/// fallback the tray item is a silent no-op whenever no reminder happens to
/// be firing — which is most of the time, and a milder repeat of the very
/// "tray does nothing" bug it was added to fix.
pub fn tray_target_meeting_id(queue: &ReminderQueue, vault: &VaultManager) -> Option<String> {
    if let Some(current) = current_popup_reminder(queue) {
        return Some(current.meeting_id);
    }

    let now = Utc::now();
    let mut candidates: Vec<(i64, String)> = vault
        .list_meetings()
        .unwrap_or_default()
        .into_iter()
        .filter(|m| is_open(m) && m.status != MEETING_STATUS_RECORDING)
        .filter_map(|m| {
            let start = m.scheduled_start.as_deref().and_then(parse_rfc3339)?;
            let end = m
                .scheduled_end
                .as_deref()
                .and_then(parse_rfc3339)
                .unwrap_or(start + Duration::hours(1));
            // In progress, or starting within the next 15 minutes.
            let in_progress = start <= now && now <= end;
            let starting_soon = start > now && (start - now).num_minutes() <= 15;
            (in_progress || starting_soon).then(|| ((start - now).num_seconds().abs(), m.id))
        })
        .collect();

    candidates.sort_by_key(|(distance, _)| *distance);
    candidates.into_iter().next().map(|(_, id)| id)
}

pub fn dismiss(queue: &ReminderQueue, meeting_id: &str, kind: ReminderKind) {
    let mut entries = queue.0.lock().unwrap();
    if let Some(e) = entries.iter_mut().find(|e| e.meeting_id == meeting_id && e.kind == kind) {
        e.status = ReminderStatus::Dismissed;
    }
}

pub fn snooze(queue: &ReminderQueue, meeting_id: &str, kind: ReminderKind, minutes: i64) {
    let mut entries = queue.0.lock().unwrap();
    if let Some(e) = entries.iter_mut().find(|e| e.meeting_id == meeting_id && e.kind == kind) {
        e.status = ReminderStatus::Snoozed { until: Utc::now() + Duration::minutes(minutes) };
    }
}

/// Resolves every open reminder for a meeting as `Actioned` — called as a
/// side effect of starting its recording, regardless of whether that was
/// triggered from the popup, the meetings list, or the tray. This is what
/// keeps all three entry points in sync instead of only one of them
/// clearing state (the fix for Decision 45's Refactor #1/Improve #5).
pub fn mark_meeting_actioned(queue: &ReminderQueue, meeting_id: &str) {
    let mut entries = queue.0.lock().unwrap();
    for e in entries.iter_mut().filter(|e| e.meeting_id == meeting_id) {
        if matches!(e.status, ReminderStatus::Pending | ReminderStatus::Fired | ReminderStatus::Snoozed { .. }) {
            e.status = ReminderStatus::Actioned;
        }
    }
}

/// Testing-only: injects an already-`Fired` reminder for a real, already
/// persisted meeting, bypassing the normal detection/timing gates —
/// backs Settings → Developer's "Mock Meeting Reminders" section so it can
/// exercise the popup's actual "Start Recording" path against a real vault
/// record, rather than a synthetic ID that would just reproduce Decision
/// 45's Broken #1 all over again.
pub fn inject_mock_reminder(queue: &ReminderQueue, meeting: &Meeting, kind: ReminderKind) {
    let mut entries = queue.0.lock().unwrap();
    entries.retain(|e| !(e.meeting_id == meeting.id && e.kind == kind));
    entries.push(ReminderEvent {
        meeting_id: meeting.id.clone(),
        kind,
        title: meeting.title.clone(),
        provider: meeting.provider.clone(),
        participants: meeting.participants.clone(),
        fire_at: Utc::now(),
        status: ReminderStatus::Fired,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meetings::PROVIDER_ZOOM;
    use crate::vault::Meeting;

    fn temp_vault() -> VaultManager {
        let dir = std::env::temp_dir().join(format!("relay_test_reminders_{}", uuid::Uuid::new_v4()));
        VaultManager::new(dir)
    }

    fn settings_all_on() -> MeetingSettings {
        MeetingSettings {
            remind_before_meeting: true,
            remind_if_unrecorded: true,
            remind_on_detection: true,
        }
    }

    fn meeting_starting_in(vault: &VaultManager, seconds: i64) -> Meeting {
        let mut m = Meeting::new("Standup", PROVIDER_ZOOM, None);
        m.scheduled_start = Some((Utc::now() + Duration::seconds(seconds)).to_rfc3339());
        m.scheduled_end = Some((Utc::now() + Duration::seconds(seconds + 1800)).to_rfc3339());
        vault.save_meeting(&m).unwrap();
        m
    }

    #[test]
    fn test_second_reminder_does_not_overwrite_first() {
        let vault = temp_vault();
        let m1 = meeting_starting_in(&vault, 30);
        let m2 = meeting_starting_in(&vault, 60);
        let queue = ReminderQueue::default();

        let all = recompute_reminders(&queue, &vault, &settings_all_on(), None).all;
        let ids: HashSet<&str> = all.iter().map(|e| e.meeting_id.as_str()).collect();
        assert!(ids.contains(m1.id.as_str()));
        assert!(ids.contains(m2.id.as_str()));
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_reminder_fires_within_window_and_stays_fired() {
        let vault = temp_vault();
        let m = meeting_starting_in(&vault, 30);
        let queue = ReminderQueue::default();

        let outcome = recompute_reminders(&queue, &vault, &settings_all_on(), None);
        let entry = outcome.all.iter().find(|e| e.meeting_id == m.id).unwrap();
        assert!(matches!(entry.status, ReminderStatus::Fired));
        assert_eq!(outcome.newly_fired.len(), 1);
        assert!(outcome.changed);

        // A second recompute while still due must not re-fire it again,
        // and must report nothing changed so the popup isn't churned.
        let again = recompute_reminders(&queue, &vault, &settings_all_on(), None);
        assert_eq!(again.newly_fired.len(), 0);
        assert!(!again.changed);
    }

    #[test]
    fn test_per_meeting_recording_gate() {
        let vault = temp_vault();
        let recording = meeting_starting_in(&vault, -150); // in the unrecorded window
        let other = meeting_starting_in(&vault, -150);
        let queue = ReminderQueue::default();

        let all = recompute_reminders(&queue, &vault, &settings_all_on(), Some(recording.id.as_str())).all;
        assert!(all.iter().all(|e| e.meeting_id != recording.id));
        assert!(all.iter().any(|e| e.meeting_id == other.id));
    }

    #[test]
    fn test_dismiss_and_snooze_are_independent_per_kind() {
        let vault = temp_vault();
        let m = meeting_starting_in(&vault, 30);
        let queue = ReminderQueue::default();
        recompute_reminders(&queue, &vault, &settings_all_on(), None);

        dismiss(&queue, &m.id, ReminderKind::Upcoming);
        let current = queue.0.lock().unwrap();
        let entry = current.iter().find(|e| e.meeting_id == m.id && e.kind == ReminderKind::Upcoming).unwrap();
        assert!(matches!(entry.status, ReminderStatus::Dismissed));
    }

    /// The queue is in-memory, so it starts empty after a restart. A
    /// still-open meeting detected days ago must not be re-queued and
    /// re-fired as if it were new.
    #[test]
    fn test_stale_detected_meeting_does_not_refire_after_restart() {
        let vault = temp_vault();
        let mut m = Meeting::from_window_detection(PROVIDER_ZOOM, "Zoom Meeting", "zoom:zoom meeting:old", 0.85);
        m.detected_at = Some((Utc::now() - Duration::days(3)).to_rfc3339());
        m.scheduled_start = Some((Utc::now() - Duration::days(3)).to_rfc3339());
        vault.save_meeting(&m).unwrap();

        // A fresh queue, exactly as it would be right after a restart.
        let queue = ReminderQueue::default();
        let outcome = recompute_reminders(&queue, &vault, &settings_all_on(), None);

        assert!(
            outcome.all.iter().all(|e| e.kind != ReminderKind::Detected),
            "a 3-day-old detection must not queue a Detected reminder"
        );
        assert!(outcome.newly_fired.is_empty());
    }

    #[test]
    fn test_freshly_detected_meeting_does_fire() {
        let vault = temp_vault();
        let m = Meeting::from_window_detection(PROVIDER_ZOOM, "Zoom Meeting", "zoom:zoom meeting:now", 0.85);
        vault.save_meeting(&m).unwrap();

        let queue = ReminderQueue::default();
        let outcome = recompute_reminders(&queue, &vault, &settings_all_on(), None);

        assert!(outcome.newly_fired.iter().any(|e| e.kind == ReminderKind::Detected));
    }

    /// An expiring reminder has to be reported as a change, or the popup
    /// never learns to close itself.
    #[test]
    fn test_expiry_is_reported_as_a_change() {
        let vault = temp_vault();
        let m = meeting_starting_in(&vault, 30);
        let queue = ReminderQueue::default();
        recompute_reminders(&queue, &vault, &settings_all_on(), None);

        // Backdate the fired entry past the expiry window.
        {
            let mut entries = queue.0.lock().unwrap();
            for e in entries.iter_mut().filter(|e| e.meeting_id == m.id) {
                e.fire_at = Utc::now() - Duration::minutes(EXPIRE_AFTER_MINUTES + 5);
            }
        }

        let outcome = recompute_reminders(&queue, &vault, &settings_all_on(), None);
        assert!(outcome.changed, "expiry must be reported so the popup can hide");
        assert!(outcome.newly_fired.is_empty(), "expiry is not a firing");
        let entry = outcome.all.iter().find(|e| e.meeting_id == m.id).unwrap();
        assert!(matches!(entry.status, ReminderStatus::Expired));
    }

    #[test]
    fn test_tray_falls_back_to_in_progress_meeting_with_no_reminder() {
        let vault = temp_vault();
        let queue = ReminderQueue::default();

        // In progress right now, but no reminder queued at all.
        let mut m = Meeting::new("Standup", PROVIDER_ZOOM, None);
        m.scheduled_start = Some((Utc::now() - Duration::minutes(10)).to_rfc3339());
        m.scheduled_end = Some((Utc::now() + Duration::minutes(20)).to_rfc3339());
        vault.save_meeting(&m).unwrap();

        assert_eq!(tray_target_meeting_id(&queue, &vault), Some(m.id));
    }

    #[test]
    fn test_tray_prefers_the_on_screen_reminder() {
        let vault = temp_vault();
        let reminded = meeting_starting_in(&vault, 30);
        let queue = ReminderQueue::default();
        recompute_reminders(&queue, &vault, &settings_all_on(), None);

        assert_eq!(tray_target_meeting_id(&queue, &vault), Some(reminded.id));
    }

    #[test]
    fn test_mark_meeting_actioned_resolves_all_kinds_for_that_meeting() {
        let vault = temp_vault();
        let m = meeting_starting_in(&vault, 30);
        let queue = ReminderQueue::default();
        recompute_reminders(&queue, &vault, &settings_all_on(), None);

        mark_meeting_actioned(&queue, &m.id);
        let current = queue.0.lock().unwrap();
        assert!(current.iter().filter(|e| e.meeting_id == m.id).all(|e| matches!(e.status, ReminderStatus::Actioned)));
    }
}
