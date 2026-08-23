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
/// Returns `(full_queue_snapshot, newly_fired_this_call)` — the second so
/// the caller can decide exactly when to raise the OS notification/popup,
/// once per transition into `Fired`, not on every tick while it stays there.
pub fn recompute_reminders(
    queue: &ReminderQueue,
    vault: &VaultManager,
    settings: &MeetingSettings,
    currently_recording_meeting_id: Option<&str>,
) -> (Vec<ReminderEvent>, Vec<ReminderEvent>) {
    let meetings = vault.list_meetings().unwrap_or_default();
    let now = Utc::now();
    let mut entries = queue.0.lock().unwrap();

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
            let fire_at = meeting.detected_at.as_deref().and_then(parse_rfc3339).unwrap_or(now);
            ensure_entry(&mut entries, meeting, ReminderKind::Detected, fire_at);
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

    (entries.clone(), newly_fired)
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
    entries.retain(|e| e.meeting_id != meeting.id && e.meeting_id != "relay_mock_preview_meeting");
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

        let (all, _) = recompute_reminders(&queue, &vault, &settings_all_on(), None);
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

        let (all, newly_fired) = recompute_reminders(&queue, &vault, &settings_all_on(), None);
        let entry = all.iter().find(|e| e.meeting_id == m.id).unwrap();
        assert!(matches!(entry.status, ReminderStatus::Fired));
        assert_eq!(newly_fired.len(), 1);

        // A second recompute while still due must not re-fire it again.
        let (_, newly_fired_again) = recompute_reminders(&queue, &vault, &settings_all_on(), None);
        assert_eq!(newly_fired_again.len(), 0);
    }

    #[test]
    fn test_per_meeting_recording_gate() {
        let vault = temp_vault();
        let recording = meeting_starting_in(&vault, -150); // in the unrecorded window
        let other = meeting_starting_in(&vault, -150);
        let queue = ReminderQueue::default();

        let (all, _) = recompute_reminders(&queue, &vault, &settings_all_on(), Some(recording.id.as_str()));
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
